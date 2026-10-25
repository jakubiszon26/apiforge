//! Deliberate transaction completion; the original intent and foreign copies are
//! verified by storage before normal project use resumes.
use super::*;
use apiforge_storage::project::{
    ProjectStore, RecoveryCandidateStatus, TransactionRecoveryPreview,
};

pub(super) struct TransactionView {
    root: PathBuf,
    store: ProjectStore,
    preview: Option<TransactionRecoveryPreview>,
    error: Option<String>,
    finished: bool,
    archives: usize,
}

impl TransactionView {
    fn refresh(&mut self) {
        match self.store.preview_transaction_recovery() {
            Ok(preview) => {
                self.finished = preview.is_none();
                self.preview = preview;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.preview = None;
                self.finished = false;
            }
        }
    }
}

impl FolderApp {
    pub(super) fn review_transaction(&mut self, root: PathBuf) {
        self.export_status = None;
        let existing = self
            .session
            .as_ref()
            .filter(|session| session.store().root() == root)
            .map(|session| session.store().clone());
        self.working_io(
            "Reading transaction recovery…",
            move || {
                let store = match existing {
                    Some(store) => Ok(store),
                    None => ProjectStore::open(&root),
                }?;
                let mut view = TransactionView {
                    root,
                    store,
                    preview: None,
                    error: None,
                    finished: false,
                    archives: 0,
                };
                view.refresh();
                Ok::<_, apiforge_storage::project::ProjectError>(view)
            },
            |app, result| match result {
                Ok(Ok(view)) => app.prompt = Some(Prompt::Transaction(Box::new(view))),
                Ok(Err(error)) => app.notify(format!("Cannot inspect recovery: {error}"), true),
                Err(error) => app.notify(error, true),
            },
        );
    }

    pub(super) fn transaction_screen(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(32)).show(ui, |ui| {
            ui.set_max_width(600.0);
            ui.heading("Project recovery required");
            widgets::hint(ui, self.root.display().to_string());
            ui.label("An interrupted file operation must be resolved before this project can be used. Your open documents remain in memory.");
            ui.horizontal_wrapped(|ui| {
                if ui.add(widgets::primary_button("Review transaction…")).clicked() { self.review_transaction(self.root.clone()); }
                if ui.button("Export unsaved work…").clicked() && let Some(session) = &self.session {
                    match session.export_unsaved() {
                        Ok(bytes) => self.export_bytes("apiforge-unsaved-work.json".into(), bytes),
                        Err(error) => self.notify(error.to_string(), true),
                    }
                }
                if self.runs.handle.is_some() && ui.button("Cancel run").clicked() && let Some(handle) = &self.runs.handle { handle.cancel(); }
            });
            if let Some(notice) = &self.notice { widgets::hint(ui, &notice.text); }
        });
    }

    pub(super) fn transaction_prompt(
        &mut self,
        context: &egui::Context,
        mut view: TransactionView,
    ) -> Option<Prompt> {
        let (mut refresh, mut finish, mut evidence, mut reopen, mut discard) =
            (false, false, false, false, false);
        let mut export = None;
        let same_session = self
            .session
            .as_ref()
            .is_some_and(|session| session.store().root() == view.root);
        let dirty_settings = same_session
            && self
                .session
                .as_ref()
                .is_some_and(ProjectSession::is_manifest_dirty);
        let running = self.runs.handle.is_some();
        let action = dialogs::Chrome {
            id: "folder-transaction-recovery",
            eyebrow: "PROJECT RECOVERY",
            title: if view.finished { "Reload project after recovery" } else { "Recover interrupted file operation" },
            width: 620.0,
            apply_label: if view.finished { if same_session { "Reload project" } else { "Open project" } } else { "Finish intended operation" },
            note: Some("Cancel preserves the files and leaves recovery unresolved until the project can be reloaded."),
        }.show_with_footer(context, &mut view, |ui, view| {
            if let Some(error) = &view.error { widgets::field_error(ui, error); }
            if let Some(status) = &self.export_status { widgets::hint(ui, status); }
            widgets::hint(ui, view.root.display().to_string());
            if view.store.is_read_only() {
                ui.colored_label(theme::WARNING, "Read-only: another writer must finish recovery. Candidates can still be exported.");
            }
            if let Some(preview) = &view.preview {
                ui.label("Finish applies the original intended operation. Any changed file is first preserved as a durable conflict copy. Exporting alone does not approve replacement.");
                for (index, candidate) in preview.candidates().iter().enumerate() {
                    ui.separator();
                    ui.label(candidate.path().as_str());
                    widgets::hint(ui, match candidate.status() {
                        RecoveryCandidateStatus::Expected => "Original file still present",
                        RecoveryCandidateStatus::Applied => "Intended change already applied",
                        RecoveryCandidateStatus::Approved => "Foreign version already preserved and approved",
                        RecoveryCandidateStatus::Conflict => "Changed outside API Forge; preserve before replacement",
                        RecoveryCandidateStatus::Unavailable => "Cannot safely read this target; manual recovery is required",
                    });
                    ui.collapsing("Inspect candidates", |ui| {
                        state_candidate(ui, "Current file", candidate.current(), candidate.current_bytes());
                        state_candidate(ui, "Intended file", Some(candidate.target()), candidate.intended_bytes());
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(candidate.current_bytes().is_some(), egui::Button::new("Export current…")).clicked() { export = Some((index, false)); }
                        if ui.add_enabled(candidate.intended_bytes().is_some(), egui::Button::new("Export intended…")).clicked() { export = Some((index, true)); }
                    });
                }
            } else if view.finished {
                widgets::hint(ui, format!("The project can be reloaded. {} foreign file state(s) were preserved by this decision.", view.archives));
            } else {
                ui.label("The recovery data cannot be interpreted safely. Export the original evidence for manual recovery; existing files remain untouched.");
            }
            if running { widgets::hint(ui, "Cancel the active run and wait for cleanup before finishing or reopening the project."); }
            if dirty_settings && view.finished { widgets::hint(ui, "Public settings are unsaved. Export your work first, or explicitly discard those settings to reload."); }
        }, |view| {
            if view.finished { !dirty_settings && !running } else {
                !view.store.is_read_only() && !running && view.error.is_none() && view.preview.as_ref().is_some_and(|preview| preview.candidates().iter().all(|candidate| candidate.status() != RecoveryCandidateStatus::Unavailable))
            }
        }, |ui, view| {
            ui.horizontal_wrapped(|ui| {
                refresh = ui.button("Refresh preview").clicked();
                evidence = ui.button("Export recovery evidence…").clicked();
                if view.finished && dirty_settings { discard = ui.add_enabled(!running, egui::Button::new("Discard settings and reload")).clicked(); }
            });
        });
        match action {
            dialogs::Action::Cancel => return None,
            dialogs::Action::Apply if view.finished => reopen = true,
            dialogs::Action::Apply => finish = true,
            dialogs::Action::None => {}
        }
        if let Some((index, intended)) = export
            && let Some(preview) = &view.preview
        {
            let candidate = &preview.candidates()[index];
            let bytes = if intended {
                candidate.intended_bytes()
            } else {
                candidate.current_bytes()
            };
            if let Some(bytes) = bytes {
                self.export_bytes(
                    format!(
                        "{}-{}",
                        if intended { "intended" } else { "current" },
                        file_name(candidate.path().as_str())
                    ),
                    bytes.to_vec(),
                );
            }
        }
        if evidence || refresh || finish {
            self.working_io(if finish { "Finishing transaction recovery…" } else { "Reading transaction recovery…" }, move || {
                let bytes = if evidence { Some(view.store.export_transaction_evidence().map_err(|error| error.to_string())) } else { None };
                if refresh { view.refresh(); }
                if finish && let Some(preview) = &view.preview {
                    match view.store.finish_transaction_recovery(preview) {
                        Ok(completed) => { view.finished = true; view.archives = completed.archives.len(); view.preview = None; view.error = None; },
                        Err(error) => view.error = Some(format!("Recovery did not finish. Refresh and review the candidates again: {error}")),
                    }
                }
                (view, bytes)
            }, |app, result| match result {
                Ok((mut view, bytes)) => {
                    if let Some(bytes) = bytes { match bytes {
                        Ok(bytes) => app.export_bytes("apiforge-transaction-evidence.json".into(), bytes),
                        Err(error) => view.error = Some(format!("Cannot prepare evidence export: {error}")),
                    } }
                    app.prompt = Some(Prompt::Transaction(Box::new(view)));
                }
                Err(error) => app.notify(error, true),
            });
            return self.prompt.take();
        }
        if reopen || discard {
            if same_session {
                self.working_project(
                    "Reloading recovered project…",
                    move |session| {
                        if discard {
                            session.discard_configuration()?;
                        }
                        session.reload_project()
                    },
                    move |app, result| match result {
                        Ok(()) => {
                            app.watcher.recovery_required = false;
                            app.follow_external(ExternalOutcome {
                                project_reloaded: true,
                                ..Default::default()
                            });
                            app.watcher.rescan();
                            app.notify(
                                "Recovered the file operation and reloaded the project.",
                                false,
                            );
                        }
                        Err(error) => {
                            view.error = Some(format!(
                                "The operation finished, but reload failed: {error}"
                            ));
                            app.prompt = Some(Prompt::Transaction(Box::new(view)));
                        }
                    },
                );
                return self.prompt.take();
            } else {
                let root = view.root.clone();
                // Release the temporary writer before the normal opener acquires
                // ownership; old-project recovery still precedes any switch.
                drop(view);
                if self.session.is_some() {
                    self.request_switch(root);
                } else {
                    *self = Self::open(context, root);
                }
                return self.prompt.take();
            }
        }
        Some(Prompt::Transaction(Box::new(view)))
    }
}

fn state_candidate(
    ui: &mut egui::Ui,
    title: &str,
    state: Option<&apiforge_storage::project::FileState>,
    bytes: Option<&[u8]>,
) {
    if bytes.is_some() || matches!(state, Some(apiforge_storage::project::FileState::Missing)) {
        super::conflict_view::candidate(ui, title, bytes);
    } else {
        widgets::field_label(ui, title);
        widgets::hint(
            ui,
            if matches!(state, Some(apiforge_storage::project::FileState::Directory)) {
                "Directory"
            } else {
                "File bytes are unavailable for inspection."
            },
        );
    }
}
