//! Native decisions over exact source/text candidates; exports keep raw bytes.
use super::*;
use apiforge_project::{ConflictDecision, ConflictPreview, ConflictTarget};

pub(super) struct ConflictView {
    target: ConflictTarget,
    preview: Option<ConflictPreview>,
    error: Option<String>,
}

impl FolderApp {
    pub(super) fn conflict_notice(
        &mut self,
        ui: &mut egui::Ui,
        target: ConflictTarget,
        conflict: ExternalConflict,
        path: &str,
        text: &str,
    ) {
        egui::Frame::new().fill(theme::ACCENT_SOFT).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(match conflict {
                ExternalConflict::Changed => "This file changed outside API Forge. Your unsaved work is kept; saving waits for a conflict decision.",
                ExternalConflict::Deleted => "This file was deleted outside API Forge. Your unsaved work is kept; decide whether to recreate it or accept the deletion.",
            });
            ui.horizontal_wrapped(|ui| {
                if ui.add(widgets::primary_button("Review conflict…")).clicked() { self.review_conflict(target); }
                if ui.button("Export my version…").clicked() { self.export_text(file_name(path).into(), text.into()); }
            });
        });
    }

    pub(super) fn review_conflict(&mut self, target: ConflictTarget) {
        self.export_status = None;
        self.refresh_conflict(ConflictView {
            target,
            preview: None,
            error: None,
        });
    }

    fn refresh_conflict(&mut self, mut view: ConflictView) {
        let target = view.target.clone();
        self.working_project(
            "Reading conflict candidates…",
            move |session| session.preview_conflict(target),
            move |app, result| {
                match result {
                    Ok(preview) => {
                        view.preview = Some(preview);
                        view.error = None;
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                app.prompt = Some(Prompt::Conflict(Box::new(view)));
            },
        );
    }

    pub(super) fn conflict_prompt(
        &mut self,
        context: &egui::Context,
        mut view: ConflictView,
    ) -> Option<Prompt> {
        let (mut refresh, mut local, mut disk, mut decision) = (false, false, false, None);
        let deleted = view
            .preview
            .as_ref()
            .is_some_and(ConflictPreview::is_deleted);
        let action = dialogs::Chrome {
            id: "folder-file-conflict",
            eyebrow: "FILE CONFLICT",
            title: "Review file conflict",
            width: 560.0,
            apply_label: if deleted { "Accept deletion" } else { "Use version on disk" },
            note: Some("Cancel keeps your unsaved work and leaves the conflict unresolved."),
        }.show_with_footer(context, &mut view, |ui, view| {
            if let Some(error) = &view.error { widgets::field_error(ui, error); }
            if let Some(status) = &self.export_status { widgets::hint(ui, status); }
            if let Some(preview) = &view.preview {
                widgets::hint(ui, preview.path());
                ui.label("Your unsaved work is kept until you choose a version. Exporting a copy leaves this conflict unresolved.");
                candidate(ui, "Your unsaved version", Some(preview.local_bytes()));
                candidate(ui, "Version on disk", preview.disk_bytes());
                ui.horizontal_wrapped(|ui| {
                    local = ui.button("Export my version…").clicked();
                    disk = ui.add_enabled(preview.disk_bytes().is_some(), egui::Button::new("Export disk version…")).clicked();
                });
            }
        }, |view| view.preview.is_some() && view.error.is_none(), |ui, view| {
            ui.horizontal_wrapped(|ui| {
                refresh = ui.button("Refresh preview").clicked();
                if deleted && ui.add_enabled(view.error.is_none(), egui::Button::new("Keep my version")).on_hover_text("Recreate this file on the next explicit save").clicked() {
                    decision = Some(ConflictDecision::KeepDeleted);
                }
            });
        });
        match action {
            dialogs::Action::Cancel => return None,
            dialogs::Action::Apply => decision = Some(ConflictDecision::UseDisk),
            dialogs::Action::None => {}
        }
        if let Some(preview) = &view.preview {
            if local {
                self.export_bytes(
                    format!("local-{}", file_name(preview.path())),
                    preview.local_bytes().to_vec(),
                );
            }
            if disk && let Some(bytes) = preview.disk_bytes() {
                self.export_bytes(
                    format!("disk-{}", file_name(preview.path())),
                    bytes.to_vec(),
                );
            }
        }
        if refresh {
            self.refresh_conflict(view);
            return self.prompt.take();
        }
        if let Some(decision) = decision
            && let Some(preview) = view.preview.clone()
        {
            let reviewed = preview.clone();
            self.working_project(
                "Resolving file conflict…",
                move |session| {
                    session.resolve_conflict(&reviewed, decision, DIAGRAM)?;
                    if decision == ConflictDecision::UseDisk
                        && reviewed.is_deleted()
                        && let ConflictTarget::Text(path) = reviewed.target()
                    {
                        session.close_text_file(path)?;
                    }
                    Ok(())
                },
                move |app, result| match result {
                    Ok(()) => {
                        let tab = match preview.target() {
                            ConflictTarget::Document(id) => id.clone(),
                            ConflictTarget::Text(path) => text_tab(path),
                        };
                        if decision == ConflictDecision::UseDisk
                            && preview.is_deleted()
                            && matches!(preview.target(), ConflictTarget::Text(_))
                        {
                            app.workbench.close(&tab);
                        }
                        code::clear_history(&app.ui_context, editor_id(&tab));
                        app.shown = None;
                        app.reload_field_drafts();
                        app.touched();
                        app.watcher.rescan();
                        app.notify(
                            if decision == ConflictDecision::KeepDeleted {
                                "Kept your version. Use Save to recreate the file."
                            } else if preview.is_deleted() {
                                "Accepted the file deletion."
                            } else {
                                "Loaded the reviewed version on disk."
                            },
                            false,
                        );
                    }
                    Err(error) => {
                        view.error = Some(error.to_string());
                        app.prompt = Some(Prompt::Conflict(Box::new(view)));
                    }
                },
            );
            return self.prompt.take();
        }
        Some(Prompt::Conflict(Box::new(view)))
    }
}

pub(super) fn candidate(ui: &mut egui::Ui, title: &str, bytes: Option<&[u8]>) {
    widgets::field_label(ui, title);
    match bytes {
        None => {
            widgets::hint(ui, "File is absent.");
        }
        Some(bytes) => {
            let display = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(display).monospace())
                            .wrap()
                            .selectable(true),
                    );
                });
            if bytes.len() > 4096 {
                widgets::hint(
                    ui,
                    "Preview shows the first 4 KiB. Export includes every original byte.",
                );
            }
            if std::str::from_utf8(bytes).is_err() {
                widgets::hint(
                    ui,
                    "This candidate is not UTF-8. Export preserves its raw bytes.",
                );
            }
        }
    }
    ui.add_space(theme::SPACE_SM);
}
