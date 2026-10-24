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
    Imports { folder: String, files: Vec<PathBuf> },
    Exported(Result<PathBuf, String>),
}

impl FolderApp {
    pub(super) fn pick_imports(&mut self, folder: String) {
        let (sender, receiver) = channel();
        let repaint = self.ui_context.clone();
        std::thread::spawn(move || {
            let files = rfd::FileDialog::new()
                .set_title("Import scenarios and components")
                .add_filter("API Forge source", &["afs"])
                .pick_files()
                .unwrap_or_default();
            let _ = sender.send(Picked::Imports { folder, files });
            repaint.request_repaint();
        });
        self.picked.push(receiver);
    }

    /// Saves a copy of a document's unsaved text anywhere the user chooses.
    pub(super) fn export_text(&mut self, name: String, text: String) {
        let (sender, receiver) = channel();
        let repaint = self.ui_context.clone();
        std::thread::spawn(move || {
            let result = match rfd::FileDialog::new()
                .set_title("Export your version")
                .set_file_name(name)
                .save_file()
            {
                Some(path) => std::fs::write(&path, text)
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
        let mut finished = Vec::new();
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
            match picked {
                Picked::Imports { files, .. } if files.is_empty() => {}
                Picked::Imports { folder, files } => self.prepare_import(&files, &folder),
                Picked::Exported(Ok(path)) => {
                    self.notify(format!("Exported to {}.", path.display()), false)
                }
                Picked::Exported(Err(error)) => {
                    self.notify(format!("Could not export: {error}"), true)
                }
            }
        }
    }

    fn prepare_import(&mut self, files: &[PathBuf], folder: &str) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        match session.prepare_import(files, folder) {
            Ok(candidates) => {
                let rows = candidates
                    .into_iter()
                    .map(|candidate| ImportRow {
                        include: true,
                        kind: candidate.declaration.as_ref().map(|d| d.kind),
                        destination: candidate.destination.clone(),
                        handle: String::new(),
                        candidate,
                    })
                    .collect();
                self.prompt = Some(Prompt::Import { rows, error: None });
            }
            Err(error) => self.notify(format!("Cannot import: {error}"), true),
        }
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
            match self.relocate(&path, &target) {
                Ok(()) => return None,
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
            match self.relocate(&path, &moved_path(&path, &folder)) {
                Ok(()) => return None,
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
        mut error: Option<String>,
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
            match self
                .session
                .as_mut()
                .map(|session| session.create_folder(&path))
            {
                Some(Ok(())) => {
                    if !parent.is_empty() {
                        self.workbench.expanded.insert(parent.clone());
                    }
                    self.explorer.selected = Some(path);
                    self.watcher.rescan();
                    self.touched();
                    return None;
                }
                Some(Err(failure)) => error = Some(failure.to_string()),
                None => return None,
            }
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
        mut error: Option<String>,
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
            match self.delete_confirmed(&preview) {
                Ok(()) => return None,
                Err(message) => error = Some(message),
            }
        }
        Some(Prompt::Delete {
            preview,
            paths,
            error,
        })
    }

    /// Moves the previewed items to the trash and closes their tabs.
    pub(super) fn delete_confirmed(&mut self, preview: &DeletePreview) -> Result<(), String> {
        let session = self.session.as_mut().ok_or_else(String::new)?;
        session
            .delete(preview, DIAGRAM)
            .map_err(|error| error.to_string())?;
        let removed: Vec<String> = self
            .workbench
            .tabs()
            .iter()
            .filter(|tab| session.document(&tab.document_id).is_none())
            .map(|tab| tab.document_id.clone())
            .collect();
        for document_id in removed {
            self.close_tab(&document_id);
        }
        self.explorer.selected = None;
        self.watcher.rescan();
        self.notify("Moved to the trash.", false);
        Ok(())
    }

    pub(super) fn import_prompt(
        &mut self,
        context: &egui::Context,
        mut rows: Vec<ImportRow>,
        mut error: Option<String>,
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
        if confirm && let Some(session) = self.session.as_mut() {
            let items: Vec<ImportItem> = rows
                .iter()
                .filter(|row| row.include)
                .filter_map(|row| {
                    Some(ImportItem {
                        text: row.candidate.text.clone(),
                        kind: row.kind?,
                        destination: row.destination.trim().to_owned(),
                        handle: (row.candidate.handle_collision)
                            .then(|| row.handle.trim().to_owned()),
                    })
                })
                .collect();
            match session.import(&items, DIAGRAM) {
                Ok(ids) => {
                    if let Some(first) = ids.first() {
                        self.workbench.open(first);
                    }
                    for item in &items {
                        let folder = parent(&item.destination);
                        if !folder.is_empty() {
                            self.workbench.expanded.insert(folder.to_owned());
                        }
                    }
                    self.watcher.rescan();
                    self.notify(format!("Imported {}.", count(ids.len(), "file")), false);
                    self.touched();
                    return None;
                }
                Err(failure) => error = Some(failure.to_string()),
            }
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
                Ok(()) => return None,
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
                Ok(()) => return None,
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
        let session = self.session.as_mut().ok_or_else(String::new)?;
        let operation = operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
            .ok_or_else(String::new)?;
        let root = session.store().root().to_owned();
        let mut destinations = Vec::new();
        for entry in &operation.entries {
            let path = entry.document.path.as_str();
            if !root.join(path).exists() {
                continue;
            }
            let stem = path.trim_end_matches(".afs");
            let free = (2..)
                .map(|number| format!("{stem}-{number}.afs"))
                .find(|candidate| !root.join(candidate).exists())
                .expect("an unused name exists");
            destinations.push(RestoreDestination {
                document_id: entry.document.document_id.clone(),
                path: apiforge_storage::project::ProjectPath::new(free)
                    .map_err(|error| error.to_string())?,
            });
        }
        let ids = session
            .restore(operation_id, &destinations)
            .map_err(|error| error.to_string())?;
        if let Some(first) = ids.first() {
            self.workbench.open(first);
        }
        self.reload_field_drafts();
        self.watcher.rescan();
        self.notify("Restored from the trash.", false);
        self.touched();
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
        let (mut accept, mut keep, mut export) = (false, false, false);
        egui::Frame::new()
            .fill(theme::ACCENT_SOFT)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(match conflict {
                    ExternalConflict::Changed => {
                        "This file changed outside API Forge while it had unsaved changes. \
                         Saving is paused: use the version on disk, or export your version \
                         first."
                    }
                    ExternalConflict::Deleted => {
                        "This file was deleted outside API Forge while it had unsaved \
                         changes. Keep your version to recreate the file when you save, or \
                         accept the deletion."
                    }
                });
                ui.horizontal(|ui| {
                    match conflict {
                        ExternalConflict::Changed => {
                            accept = ui.button("Use the version on disk").clicked();
                        }
                        ExternalConflict::Deleted => {
                            keep = ui.add(widgets::primary_button("Keep my version")).clicked();
                            accept = ui.button("Accept the deletion").clicked();
                        }
                    }
                    export = ui.button("Export my version…").clicked();
                });
            });
        if export {
            self.export_text(file_name(&snapshot.path).to_owned(), snapshot.text.clone());
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let result = if accept {
            session.accept_external(document_id, DIAGRAM)
        } else if keep {
            session.keep_deleted(document_id)
        } else {
            return;
        };
        match result {
            Ok(()) => {
                self.shown = None;
                code::clear_history(ui.ctx(), editor_id(document_id));
                self.reload_field_drafts();
                self.touched();
            }
            Err(error) => self.notify(error.to_string(), true),
        }
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
