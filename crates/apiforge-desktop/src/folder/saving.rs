//! Saving, local protection and close acknowledgement over the storage queue.
use super::*;
use apiforge_project::StorageMode;

#[derive(Default)]
pub(super) struct Persistence {
    pub closing: bool,
    pub close_ready: bool,
    pub discard: bool,
    pub departure: bool,
    pub approved_export: Option<Vec<u8>>,
    pub departure_generation: u64,
    recovery_pending: bool,
}

struct Protection {
    error: Option<String>,
    view: Option<WorkspaceState>,
}

pub(super) enum SavedVersion {
    Document {
        version: u64,
        model: Option<Box<Document>>,
        fields: Vec<FieldRecovery>,
    },
    Text {
        path: String,
        version: u64,
    },
}

impl SavedVersion {
    pub fn matches(&self, session: &ProjectSession, id: &str) -> bool {
        match self {
            Self::Document {
                version,
                model,
                fields,
            } => session.document(id).is_some_and(|doc| {
                doc.source_version() == *version
                    && doc.model() == model.as_deref()
                    && doc.field_drafts() == fields
            }),
            Self::Text { path, version } => session
                .text_file(path)
                .is_some_and(|file| file.version() == *version),
        }
    }
}

impl FolderApp {
    pub(super) fn save_active(&mut self) {
        match self.workbench.active().map(str::to_owned) {
            Some(id) => {
                if let Err(error) = self.save_document(&id) {
                    self.notify(error, true);
                }
            }
            None => self.save_configuration(),
        }
    }

    /// Accepts a save request. Only its callback acknowledges durable success.
    pub(super) fn save_document(&mut self, id: &str) -> Result<(), String> {
        if self.session.is_none() {
            return Err("No project is open.".into());
        }
        let name = self
            .text_of(id)
            .map(str::to_owned)
            .or_else(|| {
                self.session
                    .as_ref()?
                    .document(id)
                    .map(|doc| doc.entry().path.as_str().to_owned())
            })
            .unwrap_or_else(|| id.into());
        self.save_document_with(id, move |app, result| match result {
            Ok(_) => app.touched(),
            Err(error) => app.notify(
                format!("Could not save {}: {error}", file_name(&name)),
                true,
            ),
        });
        Ok(())
    }

    pub(super) fn save_document_with(
        &mut self,
        id: &str,
        complete: impl FnOnce(&mut Self, apiforge_project::Result<SavedVersion>) + 'static,
    ) {
        let text = self.text_of(id).map(str::to_owned);
        let id = id.to_owned();
        self.storage_project(
            "Saving file…",
            StorageMode::PreserveEdits,
            false,
            move |session| match text {
                Some(path) => {
                    session.save_text_file(&path)?;
                    let version = session
                        .text_file(&path)
                        .ok_or(apiforge_project::Error::UnknownTextFile)?
                        .version();
                    Ok(SavedVersion::Text { path, version })
                }
                None => {
                    let saved = session.save(&id)?;
                    let doc = session
                        .document(&id)
                        .ok_or(apiforge_project::Error::UnknownDocument)?;
                    Ok(SavedVersion::Document {
                        version: saved.source_version,
                        model: doc.model().cloned().map(Box::new),
                        fields: doc.field_drafts().to_vec(),
                    })
                }
            },
            complete,
        );
    }

    pub(super) fn save_configuration(&mut self) {
        if self.session.is_none() {
            return;
        }
        self.storage_project(
            "Saving project settings…",
            StorageMode::PreserveEdits,
            false,
            ProjectSession::save_configuration,
            |app, result| {
                if let Err(error) = result {
                    app.notify(
                        format!("Could not save the project settings: {error}"),
                        true,
                    );
                }
            },
        );
    }

    pub(super) fn save_all(&mut self) {
        if self.session.is_none() {
            return;
        }
        self.storage_project(
            "Saving project…",
            StorageMode::PreserveEdits,
            false,
            |session| {
                let pending: Vec<_> = session
                    .documents()
                    .iter()
                    .filter(|doc| doc.is_dirty() || doc.is_layout_dirty())
                    .map(|doc| (doc.id().to_owned(), doc.entry().path.as_str().to_owned()))
                    .collect();
                let texts: Vec<_> = session
                    .text_files()
                    .iter()
                    .filter(|file| file.is_dirty())
                    .map(|file| file.path().to_owned())
                    .collect();
                let (mut saved, mut failures) = (0, Vec::new());
                for (id, path) in pending {
                    match session.save(&id) {
                        Ok(_) => saved += 1,
                        Err(error) => failures.push(format!("{}: {error}", file_name(&path))),
                    }
                }
                for path in texts {
                    match session.save_text_file(&path) {
                        Ok(_) => saved += 1,
                        Err(error) => failures.push(format!("{}: {error}", file_name(&path))),
                    }
                }
                if session.is_manifest_dirty()
                    && let Err(error) = session.save_configuration()
                {
                    failures.push(format!("Project settings: {error}"));
                }
                Ok((saved, failures))
            },
            |app, result| match result {
                Ok((saved, failures)) if failures.is_empty() => {
                    if saved > 0 {
                        app.notify(format!("Saved {}.", count(saved, "file")), false);
                    }
                    app.touched();
                }
                Ok((_, failures)) => app.notify(failures.join("\n"), true),
                Err(error) => app.notify(format!("Could not save the project: {error}"), true),
            },
        );
    }

    pub(super) fn capture_view(&mut self) -> Option<WorkspaceState> {
        let session = self.session.as_ref()?;
        let ids: Vec<_> = self
            .workbench
            .tabs()
            .iter()
            .map(|tab| tab.document_id.clone())
            .collect();
        for id in ids {
            let camera = self
                .canvases
                .get(&id)
                .and_then(Canvas::camera)
                .map(|(x, y, zoom)| CameraState { x, y, zoom });
            if let Some(tab) = self.workbench.tab_mut(&id)
                && camera.is_some()
            {
                tab.camera = camera;
            }
        }
        let mut state = self.workbench.state(&session.manifest().id);
        // A mode requested for a capture is not the project's to remember.
        if let Some((shown, own)) = self.shown_instead
            && state.mode == shown
        {
            state.mode = own;
        }
        Some(state)
    }

    pub(super) fn protect(&mut self, now: Instant) {
        if self.persistence.closing || self.persistence.recovery_pending {
            return;
        }
        let Some(changed) = self.changed_at else {
            return;
        };
        let due = changed + RECOVERY_DELAY;
        if now < due {
            self.ui_context.request_repaint_after(due - now);
            return;
        }
        self.changed_at = None;
        if self
            .session
            .as_ref()
            .is_none_or(ProjectSession::is_read_only)
        {
            return;
        }
        self.persistence.recovery_pending = true;
        let state = self
            .capture_view()
            .filter(|state| self.saved_view.as_ref() != Some(state));
        self.storage_project(
            "Protecting unsaved work…",
            StorageMode::PreserveEdits,
            false,
            move |session| {
                let error = session.save_recovery().err().map(|error| error.to_string());
                let _ = session.save_last_valid();
                let view =
                    state.filter(|state| session.store().save_workspace_state(state).is_ok());
                Ok(Protection { error, view })
            },
            |app, result| {
                app.persistence.recovery_pending = false;
                match result {
                    Ok(protection) => {
                        if let Some(view) = protection.view {
                            app.saved_view = Some(view);
                        }
                        if let Some(error) = protection.error {
                            app.notify(format!("Unsaved work is not protected: {error}"), true);
                        }
                    }
                    Err(error) => {
                        app.notify(format!("Unsaved work is not protected: {error}"), true)
                    }
                }
            },
        );
    }

    pub fn prepare_close(&mut self) -> bool {
        self.poll_storage();
        self.poll_run();
        if self.persistence.close_ready {
            return true;
        }
        self.persistence.closing = true;
        self.watcher.stop();
        self.watcher.drain_stopped();
        if let Some(handle) = &self.runs.handle {
            handle.cancel();
            return false;
        }
        if self.storage.is_busy() || self.opening || self.watcher.is_busy() {
            return false;
        }
        if let Some(approved) = self.persistence.approved_export.take() {
            if self
                .session
                .as_ref()
                .and_then(|session| session.export_unsaved().ok())
                .as_deref()
                != Some(approved.as_slice())
            {
                self.persistence.closing = false;
                self.persistence.departure = false;
                self.watcher.resume();
                self.close_error = Some(
                    "Work changed after export. Export the current work before leaving.".into(),
                );
                return false;
            }
            self.session = None;
            self.runs = runs::Runs::default();
            self.persistence.close_ready = true;
            return true;
        }
        if self.session.is_none() {
            self.persistence.close_ready = true;
            return true;
        }
        let state = self.capture_view();
        let discard = self.persistence.discard;
        self.storage_project(
            "Protecting work before closing…",
            StorageMode::Exclusive,
            true,
            move |session| {
                if !session.is_read_only() {
                    if let Some(state) = state {
                        let _ = session.store().save_workspace_state(&state);
                    }
                    let _ = session.save_last_valid();
                }
                if discard {
                    session.discard_protection()
                } else {
                    session.protect_close()
                }
            },
            |app, result| match result {
                Ok(()) => {
                    app.session = None;
                    app.runs = runs::Runs::default();
                    app.persistence.close_ready = true;
                }
                Err(error) => {
                    // A departure from the window's own close dialog reports
                    // here: advance_departure stops looking once departure is
                    // cleared, so it would miss an error this completion
                    // leaves when the frame polls storage before it.
                    let leaving = app.persistence.departure && !app.switching();
                    app.persistence.closing = false;
                    app.persistence.discard = false;
                    app.persistence.departure = false;
                    app.watcher.resume();
                    if leaving {
                        app.prompt = Some(Prompt::CloseFailed(error.to_string()));
                    } else {
                        app.close_error = Some(error.to_string());
                    }
                }
            },
        );
        false
    }

    pub(super) fn advance_departure(&mut self) {
        if !self.persistence.departure {
            return;
        }
        if self.prepare_close() {
            self.persistence.departure = false;
            self.finish_departure();
        } else if let Some(error) = self.close_error() {
            self.prompt = Some(Prompt::CloseFailed(error));
        }
    }

    pub fn close_error(&mut self) -> Option<String> {
        self.close_error.take()
    }

    pub fn abort_close(&mut self, error: String) {
        self.prompt = Some(Prompt::CloseFailed(error));
    }

    pub fn save_pending(&self) -> bool {
        self.storage.is_busy()
            || self.watcher.is_busy()
            || self.persistence.closing
            || self.runs.exporting
            || self.runs.handle.is_some()
            || self
                .runs
                .runner
                .as_ref()
                .is_some_and(|runner| runner.active_runs() > 0)
    }
}
