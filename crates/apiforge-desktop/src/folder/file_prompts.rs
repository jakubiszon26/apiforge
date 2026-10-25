//! Dialogs for explorer file operations, import, the trash and external
//! conflicts. Every operation is one transaction; none is undone by Ctrl+Z.

use super::explorer::{moved_path, parent};
use super::*;
use apiforge_project::{DeletePreview, ExternalConflict, ImportCandidate, ImportItem};
use apiforge_storage::project::{RestoreDestination, TrashIndex};
use std::sync::mpsc::{Receiver, channel};

/// One file offered in the import dialog, with the user's choices.
pub(super) struct ImportRow {
    pub candidate: ImportCandidate,
    pub include: bool,
    pub kind: Option<DocumentKind>,
    pub destination: String,
    /// A replacement handle for a colliding component.
    pub handle: String,
}

/// Work done by a file dialog on its own thread.
pub(super) enum Picked {
    Folder {
        generation: u64,
        path: Option<PathBuf>,
    },
    Imports {
        identity: apiforge_project::SessionIdentity,
        folder: String,
        files: Vec<PathBuf>,
    },
    Exported(Result<PathBuf, String>),
    CloseExported {
        generation: u64,
        result: Option<Result<PathBuf, String>>,
        contents: Vec<u8>,
    },
}

impl FolderApp {
    pub(super) fn export_before_close(&mut self) {
        if self.runs.exporting {
            return;
        }
        let Some(session) = &self.session else { return };
        let contents = match session.export_unsaved() {
            Ok(contents) => contents,
            Err(error) => {
                self.notify(format!("Could not prepare export: {error}"), true);
                return;
            }
        };
        let (sender, receiver) = channel();
        let repaint = self.ui_context.clone();
        self.runs.exporting = true;
        self.persistence.departure_generation += 1;
        let generation = self.persistence.departure_generation;
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .set_title("Export unsaved project work")
                .set_file_name("apiforge-recovery-export.json")
                .save_file()
                .map(|path| {
                    apiforge_storage::write_atomic(&path, &contents)
                        .map(|()| path)
                        .map_err(|error| error.to_string())
                });
            let _ = sender.send(Picked::CloseExported {
                generation,
                result,
                contents,
            });
            repaint.request_repaint();
        });
        self.picked.push(receiver);
    }

    pub(super) fn complete_close_export(
        &mut self,
        result: Option<Result<PathBuf, String>>,
        contents: &[u8],
    ) {
        self.runs.exporting = false;
        match result {
            Some(Ok(_))
                if self
                    .session
                    .as_ref()
                    .and_then(|session| session.export_unsaved().ok())
                    .as_deref()
                    == Some(contents) =>
            {
                self.persistence.approved_export = Some(contents.to_vec());
                self.persistence.departure = true;
                self.navigation.failed = false;
                self.prompt = None;
                self.advance_departure();
            }
            Some(Ok(_)) => self.notify(
                "Work changed during export. Export the current work before closing.",
                true,
            ),
            Some(Err(error)) => self.notify(format!("Could not export: {error}"), true),
            None => {}
        }
    }

    pub(super) fn pick_imports(&mut self, folder: String) {
        let Some(session) = &self.session else { return };
        let identity = session.storage_identity();
        let (sender, receiver) = channel();
        let repaint = self.ui_context.clone();
        std::thread::spawn(move || {
            let files = rfd::FileDialog::new()
                .set_title("Import scenarios and components")
                .add_filter("API Forge source", &["afs"])
                .pick_files()
                .unwrap_or_default();
            let _ = sender.send(Picked::Imports {
                identity,
                folder,
                files,
            });
            repaint.request_repaint();
        });
        self.picked.push(receiver);
    }

    /// Saves an exact copy without acknowledging a conflict or a save baseline.
    pub(super) fn export_text(&mut self, name: String, text: String) {
        self.export_bytes(name, text.into_bytes());
    }

    pub(super) fn export_bytes(&mut self, name: String, bytes: Vec<u8>) {
        self.export_status = None;
        let (sender, receiver) = channel();
        let repaint = self.ui_context.clone();
        std::thread::spawn(move || {
            let result = match rfd::FileDialog::new()
                .set_title("Export recovery candidate")
                .set_file_name(name)
                .save_file()
            {
                Some(path) => apiforge_storage::write_atomic(&path, &bytes)
                    .map(|()| path)
                    .map_err(|error| error.to_string()),
                None => return,
            };
            let _ = sender.send(Picked::Exported(result));
            repaint.request_repaint();
        });
        self.picked.push(receiver);
    }

    /// Results of file dialogs that finished since the last frame.
    pub(super) fn follow_dialogs(&mut self) {
        let mut finished = std::mem::take(&mut self.picked_ready);
        self.picked
            .retain(|receiver: &Receiver<Picked>| match receiver.try_recv() {
                Ok(picked) => {
                    finished.push(picked);
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            });
        for picked in finished {
            if matches!(self.prompt, Some(Prompt::Working { .. }))
                && matches!(picked, Picked::Folder { .. } | Picked::Imports { .. })
            {
                self.picked_ready.push(picked);
                continue;
            }
            match picked {
                Picked::Folder { generation, path } => {
                    if !self.persistence.closing
                        && generation == self.open_generation
                        && let Some(path) = path
                    {
                        self.request_switch(path);
                    }
                }
                Picked::CloseExported {
                    generation,
                    result,
                    contents,
                } => {
                    if generation == self.persistence.departure_generation {
                        self.complete_close_export(result, &contents)
                    }
                }
                Picked::Imports { files, .. } if files.is_empty() => {}
                Picked::Imports {
                    identity,
                    folder,
                    files,
                } => {
                    if !self.persistence.closing && self.session_matches(&identity) {
                        self.prepare_import(&files, &folder);
                    }
                }
                Picked::Exported(Ok(path)) => {
                    let message = format!("Exported to {}.", path.display());
                    self.export_status = Some(message.clone());
                    self.notify(message, false)
                }
                Picked::Exported(Err(error)) => {
                    let message = format!("Could not export: {error}");
                    self.export_status = Some(message.clone());
                    self.notify(message, true)
                }
            }
        }
    }

    fn prepare_import(&mut self, files: &[PathBuf], folder: &str) {
        if self.session.is_none() {
            return;
        }
        let files = files.to_vec();
        let folder = folder.to_owned();
        self.storage_project(
            "Reading import files…",
            apiforge_project::StorageMode::Read,
            false,
            move |session| session.prepare_import(&files, &folder),
            |app, result| match result {
                Ok(candidates) => {
                    app.prompt = Some(Prompt::Import {
                        rows: candidates
                            .into_iter()
                            .map(|candidate| ImportRow {
                                include: true,
                                kind: candidate
                                    .declaration
                                    .as_ref()
                                    .map(|declaration| declaration.kind),
                                destination: candidate.destination.clone(),
                                handle: String::new(),
                                candidate,
                            })
                            .collect(),
                        error: None,
                    })
                }
                Err(error) => app.notify(format!("Cannot import: {error}"), true),
            },
        );
    }

    pub(super) fn rename_prompt(
        &mut self,
        context: &egui::Context,
        path: String,
        mut name: String,
        mut error: Option<String>,
    ) -> Option<Prompt> {
        let (mut confirm, mut cancel) = (false, false);
        let response =
            dialogs::centered_modal(egui::Id::new("folder-rename"), context).show(context, |ui| {
                ui.set_width(420.0);
                ui.heading(format!("Rename {}", file_name(&path)));
                widgets::hint(
                    ui,
                    "Renaming a file keeps the document's identity and does not rename \
                     anything declared inside it.",
                );
                ui.add_space(theme::SPACE_SM);
                widgets::field_label(ui, "Name");
                let field =
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                    confirm = true;
                }
                field.request_focus();
                if let Some(error) = &error {
                    widgets::field_error(ui, error);
                }
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    confirm |= ui
                        .add_enabled(
                            !name.trim().is_empty() && !name.contains('/'),
                            widgets::primary_button("Rename"),
                        )
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if response.should_close() || cancel {
            return None;
        }
        if confirm && !name.trim().is_empty() && !name.contains('/') {
            let folder = parent(&path);
            let target = if folder.is_empty() {
                name.trim().to_owned()
            } else {
                format!("{folder}/{}", name.trim())
            };
            let failed = Prompt::Rename {
                path: path.clone(),
                name: name.clone(),
                error: None,
            };
            match self.relocate_with(&path, &target, Some(failed)) {
                Ok(()) => return self.prompt.take(),
                Err(message) => error = Some(message),
            }
        }
        Some(Prompt::Rename { path, name, error })
    }

    pub(super) fn move_prompt(
        &mut self,
        context: &egui::Context,
        path: String,
        mut folder: String,
        mut error: Option<String>,
    ) -> Option<Prompt> {
        let folders: Vec<String> = std::iter::once(String::new())
            .chain(
                self.watcher
                    .scan
                    .iter()
                    .flat_map(|scan| scan.entries.iter())
                    .filter(|entry| entry.is_directory() && entry.portable)
                    .map(|entry| entry.path.clone())
                    .filter(|candidate| {
                        candidate != &path && !candidate.starts_with(&format!("{path}/"))
                    }),
            )
            .collect();
        let (mut confirm, mut cancel) = (false, false);
        let response =
            dialogs::centered_modal(egui::Id::new("folder-move"), context).show(context, |ui| {
                ui.set_width(420.0);
                ui.heading(format!("Move {}", file_name(&path)));
                widgets::field_label(ui, "Into folder");
                egui::ComboBox::from_id_salt("folder-move-target")
                    .width(ui.available_width())
                    .selected_text(display_folder(&folder))
                    .show_ui(ui, |ui| {
                        for candidate in &folders {
                            ui.selectable_value(
                                &mut folder,
                                candidate.clone(),
                                display_folder(candidate),
                            );
                        }
                    });
                if let Some(error) = &error {
                    widgets::field_error(ui, error);
                }
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    confirm = ui
                        .add_enabled(folder != parent(&path), widgets::primary_button("Move"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if response.should_close() || cancel {
            return None;
        }
        if confirm {
            let failed = Prompt::MoveTo {
                path: path.clone(),
                folder: folder.clone(),
                error: None,
            };
            match self.relocate_with(&path, &moved_path(&path, &folder), Some(failed)) {
                Ok(()) => return self.prompt.take(),
                Err(message) => error = Some(message),
            }
        }
        Some(Prompt::MoveTo {
            path,
            folder,
            error,
        })
    }

    pub(super) fn new_folder_prompt(
        &mut self,
        context: &egui::Context,
        parent: String,
        mut name: String,
        error: Option<String>,
    ) -> Option<Prompt> {
        let (mut confirm, mut cancel) = (false, false);
        let response = dialogs::centered_modal(egui::Id::new("folder-new-folder"), context).show(
            context,
            |ui| {
                ui.set_width(420.0);
                ui.heading("New folder");
                widgets::hint(ui, format!("In {}", display_folder(&parent)));
                widgets::field_label(ui, "Name");
                let field =
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                    confirm = true;
                }
                field.request_focus();
                if let Some(error) = &error {
                    widgets::field_error(ui, error);
                }
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    confirm |= ui
                        .add_enabled(!name.trim().is_empty(), widgets::primary_button("Create"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            },
        );
        if response.should_close() || cancel {
            return None;
        }
        if confirm && !name.trim().is_empty() {
            let path = moved_path(name.trim(), &parent);
            let target = path.clone();
            let failed = Prompt::NewFolder {
                parent: parent.clone(),
                name: name.clone(),
                error: None,
            };
            self.working_project(
                "Creating folder…",
                move |session| session.create_folder(&target),
                move |app, result| match result {
                    Ok(()) => {
                        if !parent.is_empty() {
                            app.workbench.expanded.insert(parent);
                        }
                        app.explorer.selected = Some(path);
                        app.watcher.rescan();
                        app.touched();
                    }
                    Err(error) => app.restore_prompt(failed, error.to_string()),
                },
            );
            return self.prompt.take();
        }
        Some(Prompt::NewFolder {
            parent,
            name,
            error,
        })
    }

    pub(super) fn delete_prompt(
        &mut self,
        context: &egui::Context,
        preview: Box<DeletePreview>,
        paths: Vec<String>,
        error: Option<String>,
    ) -> Option<Prompt> {
        let names: HashMap<&str, String> = self
            .session
            .as_ref()
            .map(|session| {
                session
                    .documents()
                    .iter()
                    .map(|document| (document.id(), document.entry().path.as_str().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let unsaved: Vec<String> = preview
            .unsaved_documents()
            .iter()
            .filter_map(|id| names.get(id.as_str()).cloned())
            .collect();
        let documents: Vec<String> = preview
            .documents()
            .iter()
            .map(|entry| entry.path.as_str().to_owned())
            .collect();
        let files: Vec<String> = preview
            .files()
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect();
        let folders = preview.directories().len();
        let (mut confirm, mut cancel) = (false, false);
        let response =
            dialogs::centered_modal(egui::Id::new("folder-delete"), context).show(context, |ui| {
                ui.set_width(480.0);
                ui.heading(match paths.as_slice() {
                    [path] => format!("Delete {}?", file_name(path)),
                    _ => format!("Delete {} items?", paths.len()),
                });
                ui.add_space(theme::SPACE_SM);
                ui.label(
                    "Deleted items move to this project's local trash and can be restored \
                     from there. Published component revisions are kept.",
                );
                ui.add_space(theme::SPACE_SM);
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        if folders > 0 {
                            widgets::subcaption(ui, &count(folders, "folder"));
                        }
                        if !documents.is_empty() {
                            widgets::subcaption(ui, "Documents");
                            for path in &documents {
                                ui.label(path);
                            }
                        }
                        if !files.is_empty() {
                            widgets::subcaption(ui, "Other files");
                            for path in &files {
                                ui.label(path);
                            }
                        }
                    });
                if !unsaved.is_empty() {
                    ui.add_space(theme::SPACE_SM);
                    ui.colored_label(
                        theme::WARNING,
                        format!(
                            "Unsaved changes in {} go to the trash with them, unsaved.",
                            unsaved.join(", ")
                        ),
                    );
                }
                if let Some(error) = &error {
                    widgets::field_error(ui, error);
                }
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    confirm = ui.add(widgets::primary_button("Delete")).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if response.should_close() || cancel {
            return None;
        }
        if confirm {
            let failed = Prompt::Delete {
                preview: preview.clone(),
                paths: paths.clone(),
                error: None,
            };
            self.queue_delete(*preview, Some(failed));
            return self.prompt.take();
        }
        Some(Prompt::Delete {
            preview,
            paths,
            error,
        })
    }

    #[cfg(test)]
    pub(super) fn delete_confirmed(&mut self, preview: &DeletePreview) -> Result<(), String> {
        if self.session.is_none() {
            return Err("No project is open.".into());
        }
        self.queue_delete(preview.clone(), None);
        Ok(())
    }

    fn queue_delete(&mut self, preview: DeletePreview, failed: Option<Prompt>) {
        self.working_project(
            "Moving items to trash…",
            move |session| session.delete(&preview, DIAGRAM),
            move |app, result| match result {
                Ok(_) => {
                    app.follow_external(ExternalOutcome {
                        project_reloaded: true,
                        ..Default::default()
                    });
                    app.explorer.selected = None;
                    app.watcher.rescan();
                    app.notify("Moved to the trash.", false);
                    app.touched();
                }
                Err(error) => {
                    if let Some(prompt) = failed {
                        app.restore_prompt(prompt, error.to_string());
                    } else {
                        app.notify(format!("Could not delete: {error}"), true);
                    }
                }
            },
        );
    }

    pub(super) fn import_prompt(
        &mut self,
        context: &egui::Context,
        mut rows: Vec<ImportRow>,
        error: Option<String>,
    ) -> Option<Prompt> {
        let (mut confirm, mut cancel) = (false, false);
        let response =
            dialogs::centered_modal(egui::Id::new("folder-import"), context).show(context, |ui| {
                ui.set_width(560.0);
                ui.heading("Import files");
                widgets::hint(
                    ui,
                    "Each file is copied into the project with a new identity. The originals \
                     are not changed and no component is published.",
                );
                ui.add_space(theme::SPACE_SM);
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for (index, row) in rows.iter_mut().enumerate() {
                            ui.push_id(index, |ui| import_row(ui, row));
                            ui.separator();
                        }
                    });
                if let Some(error) = &error {
                    widgets::field_error(ui, error);
                }
                ui.add_space(theme::SPACE_MD);
                let ready = rows.iter().any(|row| row.include)
                    && rows.iter().filter(|row| row.include).all(|row| {
                        row.kind.is_some()
                            && (!row.candidate.handle_collision || !row.handle.trim().is_empty())
                    });
                ui.horizontal(|ui| {
                    confirm = ui
                        .add_enabled(ready, widgets::primary_button("Import"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if response.should_close() || cancel {
            return None;
        }
        if confirm {
            let items: Vec<ImportItem> = rows
                .iter()
                .filter(|row| row.include)
                .filter_map(|row| {
                    Some(ImportItem {
                        text: row.candidate.text.clone(),
                        kind: row.kind?,
                        destination: row.destination.trim().to_owned(),
                        handle: row
                            .candidate
                            .handle_collision
                            .then(|| row.handle.trim().to_owned()),
                    })
                })
                .collect();
            let folders: Vec<_> = items
                .iter()
                .map(|item| parent(&item.destination).to_owned())
                .filter(|folder| !folder.is_empty())
                .collect();
            self.working_project(
                "Importing files…",
                move |session| session.import(&items, DIAGRAM),
                move |app, result| match result {
                    Ok(ids) => {
                        if let Some(id) = ids.first() {
                            app.activate_tab(id);
                        }
                        app.workbench.expanded.extend(folders);
                        app.watcher.rescan();
                        app.notify(format!("Imported {}.", count(ids.len(), "file")), false);
                        app.touched();
                    }
                    Err(error) => {
                        app.restore_prompt(Prompt::Import { rows, error: None }, error.to_string())
                    }
                },
            );
            return self.prompt.take();
        }
        Some(Prompt::Import { rows, error })
    }

    pub(super) fn register_prompt(
        &mut self,
        context: &egui::Context,
        path: String,
        mut kind: DocumentKind,
        mut error: Option<String>,
    ) -> Option<Prompt> {
        let (mut confirm, mut cancel) = (false, false);
        let response = dialogs::centered_modal(egui::Id::new("folder-register"), context).show(
            context,
            |ui| {
                ui.set_width(420.0);
                ui.heading(format!("Register {}", file_name(&path)));
                widgets::hint(
                    ui,
                    "The file does not start with a scenario or component declaration. \
                     Choose what it is; the file is not changed.",
                );
                ui.horizontal(|ui| {
                    ui.radio_value(&mut kind, DocumentKind::Scenario, "Scenario");
                    ui.radio_value(&mut kind, DocumentKind::Component, "Component");
                });
                if let Some(error) = &error {
                    widgets::field_error(ui, error);
                }
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    confirm = ui.add(widgets::primary_button("Register")).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            },
        );
        if response.should_close() || cancel {
            return None;
        }
        if confirm {
            match self.register(&path, kind) {
                Ok(()) => return self.prompt.take(),
                Err(message) => error = Some(message),
            }
        }
        Some(Prompt::Register { path, kind, error })
    }

    pub(super) fn trash_prompt(
        &mut self,
        context: &egui::Context,
        operations: Vec<TrashIndex>,
        mut error: Option<String>,
    ) -> Option<Prompt> {
        let writable = self
            .session
            .as_ref()
            .is_some_and(|session| !session.is_read_only());
        let (mut restore, mut close) = (None, false);
        let response =
            dialogs::centered_modal(egui::Id::new("folder-trash"), context).show(context, |ui| {
                ui.set_width(520.0);
                ui.heading("Trash");
                widgets::hint(
                    ui,
                    "Deleted items stay here until restored. The trash is local to this \
                     computer and is not emptied automatically.",
                );
                ui.add_space(theme::SPACE_SM);
                let pending: Vec<_> = operations
                    .iter()
                    .filter(|operation| operation.restored_at_unix_ms.is_none())
                    .collect();
                if pending.is_empty() {
                    widgets::hint(ui, "The trash is empty.");
                }
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for operation in pending {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    for entry in &operation.entries {
                                        ui.label(entry.document.path.as_str());
                                    }
                                    for file in &operation.files {
                                        ui.colored_label(theme::MUTED, file.path.as_str());
                                    }
                                    if operation.entries.is_empty() && operation.files.is_empty() {
                                        for directory in &operation.directories {
                                            ui.colored_label(
                                                theme::MUTED,
                                                format!("{}/", directory.as_str()),
                                            );
                                        }
                                    }
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add_enabled(writable, egui::Button::new("Restore"))
                                            .clicked()
                                        {
                                            restore = Some(operation.operation_id.clone());
                                        }
                                    },
                                );
                            });
                            ui.separator();
                        }
                    });
                if let Some(error) = &error {
                    widgets::field_error(ui, error);
                }
                ui.add_space(theme::SPACE_MD);
                close = ui.button("Close").clicked();
            });
        if response.should_close() || close {
            return None;
        }
        if let Some(operation_id) = restore {
            match self.restore(&operation_id, &operations) {
                Ok(()) => return self.prompt.take(),
                Err(message) => error = Some(message),
            }
        }
        Some(Prompt::Trash { operations, error })
    }

    /// Restores an operation; occupied document paths get the next free name.
    pub(super) fn restore(
        &mut self,
        operation_id: &str,
        operations: &[TrashIndex],
    ) -> Result<(), String> {
        if self.session.is_none() {
            return Err("No project is open.".into());
        }
        let operation = operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
            .cloned()
            .ok_or("This trash operation is no longer available.")?;
        let failed = Prompt::Trash {
            operations: operations.to_vec(),
            error: None,
        };
        self.working_project(
            "Restoring items…",
            move |session| {
                let root = session.store().root();
                let mut destinations = Vec::new();
                for entry in &operation.entries {
                    let path = entry.document.path.as_str();
                    if !root.join(path).exists() {
                        continue;
                    }
                    let stem = path.trim_end_matches(".afs");
                    let free = (2..=apiforge_storage::project::MAX_TREE_ENTRIES)
                        .map(|number| format!("{stem}-{number}.afs"))
                        .find(|path| !root.join(path).exists())
                        .ok_or(apiforge_project::ProjectError::Limit)?;
                    destinations.push(RestoreDestination {
                        document_id: entry.document.document_id.clone(),
                        path: apiforge_storage::project::ProjectPath::new(free)?,
                    });
                }
                session.restore(&operation.operation_id, &destinations)
            },
            move |app, result| match result {
                Ok(ids) => {
                    if let Some(id) = ids.first() {
                        app.activate_tab(id);
                    }
                    app.reload_field_drafts();
                    app.restore_text_tabs();
                    app.watcher.rescan();
                    app.notify("Restored from the trash.", false);
                    app.touched();
                }
                Err(error) => app.restore_prompt(failed, error.to_string()),
            },
        );
        Ok(())
    }

    /// The saved file changed or vanished while this document had unsaved work.
    pub(super) fn conflict_banner(
        &mut self,
        ui: &mut egui::Ui,
        document_id: &str,
        conflict: ExternalConflict,
        snapshot: &Snapshot,
    ) {
        self.conflict_notice(
            ui,
            apiforge_project::ConflictTarget::Document(document_id.into()),
            conflict,
            &snapshot.path,
            &snapshot.text,
        );
    }
}

fn import_row(ui: &mut egui::Ui, row: &mut ImportRow) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut row.include, "");
        ui.label(
            row.candidate
                .source
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
        .on_hover_text(row.candidate.source.display().to_string());
    });
    if !row.include {
        return;
    }
    ui.indent("details", |ui| {
        if row.candidate.declaration.is_none() {
            ui.colored_label(
                theme::WARNING,
                "The file does not start with a scenario or component declaration. Choose \
                 what it is.",
            );
            ui.horizontal(|ui| {
                ui.radio_value(&mut row.kind, Some(DocumentKind::Scenario), "Scenario");
                ui.radio_value(&mut row.kind, Some(DocumentKind::Component), "Component");
            });
        }
        widgets::field_label(ui, "Destination");
        ui.add(egui::TextEdit::singleline(&mut row.destination).desired_width(f32::INFINITY));
        if row.candidate.handle_collision {
            let current = row
                .candidate
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.handle.clone())
                .unwrap_or_default();
            ui.colored_label(
                theme::WARNING,
                format!("Another component is already called {current}. Choose a new name."),
            );
            ui.add(
                egui::TextEdit::singleline(&mut row.handle)
                    .hint_text(format!("{current}Copy"))
                    .desired_width(f32::INFINITY),
            );
        }
    });
}

fn display_folder(folder: &str) -> String {
    if folder.is_empty() {
        "Project root".into()
    } else {
        format!("{folder}/")
    }
}
