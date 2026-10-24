//! The window around the views: header, tabs, notices, the document list
//! and the dialogs.

use super::*;

impl FolderApp {
    pub(super) fn header(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let name = session.manifest().name.clone();
        let read_only = session.is_read_only();
        let unsaved = session
            .documents()
            .iter()
            .any(|document| document.is_dirty() || document.is_layout_dirty())
            || session.is_manifest_dirty();
        let active_unsaved = self
            .workbench
            .active()
            .and_then(|active| session.document(active))
            .is_some_and(|document| document.is_dirty() || document.is_layout_dirty());
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(name)
                    .size(theme::FONT_TITLE)
                    .family(theme::bold_family()),
            )
            .on_hover_text(self.root.display().to_string());
            if read_only {
                ui.label(
                    RichText::new("Read-only")
                        .size(theme::FONT_META)
                        .color(theme::WARNING),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !read_only && unsaved,
                        widgets::action(Icon::Save, "Save all"),
                    )
                    .on_hover_text("Ctrl+Shift+S")
                    .clicked()
                {
                    self.save_all();
                }
                if ui
                    .add_enabled(
                        !read_only && active_unsaved,
                        widgets::action(Icon::Save, "Save").primary(),
                    )
                    .on_hover_text("Ctrl+S")
                    .clicked()
                {
                    self.save_active();
                }
                ui.add_space(theme::SPACE_LG);
                let before = self.workbench.mode;
                // Right to left: the last added is the leftmost.
                widgets::tab_value(ui, &mut self.workbench.mode, Mode::Split, "Split");
                widgets::tab_value(ui, &mut self.workbench.mode, Mode::Diagram, "Diagram");
                widgets::tab_value(ui, &mut self.workbench.mode, Mode::Code, "Code")
                    .on_hover_text("Ctrl+E switches Code and Diagram");
                if self.workbench.mode != before {
                    self.touched();
                }
            });
        });
    }

    pub(super) fn tab_strip(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let tabs: Vec<(String, String, String, bool, bool, bool)> = self
            .workbench
            .tabs()
            .iter()
            .filter_map(|tab| {
                let document = session.document(&tab.document_id)?;
                let path = document.entry().path.as_str().to_owned();
                Some((
                    tab.document_id.clone(),
                    file_name(&path).to_owned(),
                    path,
                    document.needs_recovery(),
                    document.analysis() == AnalysisState::Invalid,
                    document.conflict().is_some(),
                ))
            })
            .collect();
        if tabs.is_empty() {
            ui.add_space(theme::CONTROL_HEIGHT - 4.0);
            return;
        }
        let active = self.workbench.active().map(str::to_owned);
        let mut activate = None;
        let mut close = None;
        egui::ScrollArea::horizontal()
            .id_salt("folder-tab-strip")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (document_id, name, path, unsaved, invalid, conflict) in &tabs {
                        let selected = active.as_deref() == Some(document_id.as_str());
                        let mut label = RichText::new(if *unsaved {
                            format!("{name}  ●")
                        } else {
                            name.clone()
                        })
                        .size(theme::FONT_BODY);
                        if *conflict {
                            label = label.color(theme::WARNING);
                        } else if *invalid {
                            label = label.color(theme::DANGER);
                        }
                        let response = ui
                            .add(egui::Button::selectable(selected, label))
                            .on_hover_text(path);
                        if response.clicked() {
                            activate = Some(document_id.clone());
                        }
                        if response.middle_clicked() {
                            close = Some(document_id.clone());
                        }
                        if ui
                            .add(
                                widgets::icon_button(Icon::Close, &format!("Close {name}"))
                                    .compact(),
                            )
                            .clicked()
                        {
                            close = Some(document_id.clone());
                        }
                        ui.add_space(theme::SPACE_SM);
                    }
                });
            });
        if let Some(document_id) = activate {
            self.workbench.open(&document_id);
            self.touched();
        }
        if let Some(document_id) = close {
            self.request_close_tab(&document_id);
        }
    }

    pub(super) fn notices(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let backup = session.recovery_decision_required();
        let unavailable = session.recovery_unavailable() && !session.is_read_only();
        let orphans: Vec<String> = session
            .recovered_drafts()
            .iter()
            .filter(|(status, _)| *status == apiforge_project::DraftStatus::Orphan)
            .map(|(_, draft)| draft.document_id.clone())
            .collect();
        if backup {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    theme::WARNING,
                    "Unsaved work from an earlier session is in the recovery backup.",
                );
                if ui.button("Restore backup").clicked()
                    && let Some(session) = self.session.as_mut()
                {
                    match session.restore_recovery_backup() {
                        Ok(()) => self.shown = None,
                        Err(error) => {
                            self.notice = Some(Notice {
                                text: error.to_string(),
                                error: true,
                            })
                        }
                    }
                }
            });
        } else if unavailable {
            ui.colored_label(
                theme::DANGER,
                "Local recovery data is unreadable. Unsaved work is protected only by saving.",
            );
        }
        if !orphans.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    theme::WARNING,
                    "Unsaved work from an earlier session belongs to a document that is no \
                     longer in the project.",
                );
                if ui.button("Discard it").clicked()
                    && let Some(session) = self.session.as_mut()
                {
                    for document_id in &orphans {
                        let _ = session.discard_draft(document_id);
                    }
                    self.changed_at = Some(Instant::now());
                }
            });
        }
        self.registry_notice(ui);
        let mut dismiss = false;
        if let Some(notice) = &self.notice {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    if notice.error {
                        theme::DANGER
                    } else {
                        theme::MUTED
                    },
                    &notice.text,
                );
                dismiss = ui
                    .add(widgets::icon_button(Icon::Close, "Dismiss").compact())
                    .clicked();
            });
        }
        if dismiss {
            self.notice = None;
        }
    }

    /// The project file changed, became invalid or disappeared outside API
    /// Forge. Writes wait; unsaved work stays in the window and in recovery.
    fn registry_notice(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let state = session.registry_state();
        let dirty_settings = session.is_manifest_dirty();
        let message = match state {
            RegistryState::Current => return,
            RegistryState::Changed => {
                "The project file changed outside API Forge. Saving is paused until the \
                 project is reloaded; unsaved work is kept."
            }
            RegistryState::Invalid => {
                "The project file is not valid. Saving and running are paused until it is \
                 repaired; unsaved work is kept."
            }
            RegistryState::Missing => {
                "The project file is missing. Saving and running are paused until it is \
                 restored; unsaved work is kept."
            }
        };
        let (mut reload, mut discard) = (false, false);
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(theme::WARNING, message);
            if state == RegistryState::Changed {
                if dirty_settings {
                    discard = ui.button("Discard settings changes and reload").clicked();
                } else {
                    reload = ui.button("Reload project").clicked();
                }
            }
        });
        if !(reload || discard) {
            return;
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let result = if discard {
            session
                .discard_configuration()
                .and_then(|()| session.reload_project())
        } else {
            session.reload_project()
        };
        match result {
            Ok(()) => {
                self.follow_external(ExternalOutcome {
                    project_reloaded: true,
                    ..Default::default()
                });
                self.watcher.rescan();
            }
            Err(error) => self.notify(error.to_string(), true),
        }
    }

    pub(super) fn prompts(&mut self, context: &egui::Context) {
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        self.prompt = match prompt {
            Prompt::CloseTab(document_id) => self.close_tab_prompt(context, document_id),
            Prompt::NewDocument {
                kind,
                folder,
                name,
                error,
            } => self.new_document_prompt(context, kind, folder, name, error),
            Prompt::CloseFailed(message) => self.close_failed_prompt(context, message),
            Prompt::Rename { path, name, error } => self.rename_prompt(context, path, name, error),
            Prompt::MoveTo {
                path,
                folder,
                error,
            } => self.move_prompt(context, path, folder, error),
            Prompt::NewFolder {
                parent,
                name,
                error,
            } => self.new_folder_prompt(context, parent, name, error),
            Prompt::Delete {
                preview,
                paths,
                error,
            } => self.delete_prompt(context, preview, paths, error),
            Prompt::Import { rows, error } => self.import_prompt(context, rows, error),
            Prompt::Trash { operations, error } => self.trash_prompt(context, operations, error),
        };
    }

    pub(super) fn close_tab_prompt(
        &mut self,
        context: &egui::Context,
        document_id: String,
    ) -> Option<Prompt> {
        let name = self
            .snapshot(&document_id)
            .map(|snapshot| file_name(&snapshot.path).to_owned())
            .unwrap_or_default();
        let read_only = self
            .session
            .as_ref()
            .is_some_and(ProjectSession::is_read_only);
        let (mut save, mut discard, mut cancel) = (false, false, false);
        let response = dialogs::centered_modal(egui::Id::new("folder-close-tab"), context).show(
            context,
            |ui| {
                ui.set_width(420.0);
                ui.heading(format!("Save changes to {name}?"));
                ui.add_space(theme::SPACE_SM);
                ui.label("Discarding returns the document to its saved version.");
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    save = ui
                        .add_enabled(!read_only, widgets::primary_button("Save"))
                        .clicked();
                    discard = ui.button("Discard").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            },
        );
        if response.should_close() {
            cancel = true;
        }
        if save || discard {
            self.close_tab_with(&document_id, save);
            return None;
        }
        (!cancel).then_some(Prompt::CloseTab(document_id))
    }

    /// Closes a tab after saving its document, or after discarding its unsaved
    /// work. A failure keeps the tab open and says why.
    pub(super) fn close_tab_with(&mut self, document_id: &str, save: bool) {
        let result = if save {
            self.save_document(document_id)
        } else {
            self.session
                .as_mut()
                .ok_or_else(String::new)
                .and_then(|session| {
                    session
                        .revert(document_id, DIAGRAM)
                        .map_err(|error| format!("Could not discard the changes: {error}"))
                })
        };
        match result {
            Ok(()) => self.close_tab(document_id),
            Err(message) => self.notify(message, true),
        }
    }

    pub(super) fn new_document_prompt(
        &mut self,
        context: &egui::Context,
        kind: DocumentKind,
        folder: Option<String>,
        mut name: String,
        mut error: Option<String>,
    ) -> Option<Prompt> {
        let (mut create, mut cancel) = (false, false);
        let response = dialogs::centered_modal(egui::Id::new("folder-new-document"), context).show(
            context,
            |ui| {
                ui.set_width(420.0);
                ui.heading(match kind {
                    DocumentKind::Scenario => "New scenario",
                    DocumentKind::Component => "New component",
                });
                widgets::field_label(ui, "Name");
                let field =
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                    create = true;
                }
                field.request_focus();
                if let Some(error) = &error {
                    widgets::field_error(ui, error);
                }
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    create |= ui
                        .add_enabled(!name.trim().is_empty(), widgets::primary_button("Create"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            },
        );
        if response.should_close() || cancel {
            return None;
        }
        if create && !name.trim().is_empty() {
            match self.create_document(kind, folder.as_deref(), name.trim()) {
                Ok(()) => return None,
                Err(message) => error = Some(message),
            }
        }
        Some(Prompt::NewDocument {
            kind,
            folder,
            name,
            error,
        })
    }

    pub(super) fn create_document(
        &mut self,
        kind: DocumentKind,
        folder: Option<&str>,
        name: &str,
    ) -> Result<(), String> {
        let session = self.session.as_mut().ok_or_else(String::new)?;
        let (default_folder, source) = match kind {
            DocumentKind::Scenario => ("scenarios", scenario_template(name)),
            DocumentKind::Component => ("components", component_template(name)),
        };
        let folder = folder.unwrap_or(default_folder);
        let join = |name: String| {
            if folder.is_empty() {
                name
            } else {
                format!("{folder}/{name}")
            }
        };
        let stem = file_stem(name);
        let taken = |path: &str| {
            session
                .documents()
                .iter()
                .any(|document| document.entry().path.as_str().eq_ignore_ascii_case(path))
                || session.store().root().join(path).exists()
        };
        let path = (1..)
            .map(|index| match index {
                1 => join(format!("{stem}.afs")),
                _ => join(format!("{stem}-{index}.afs")),
            })
            .find(|path| !taken(path))
            .expect("an unused name exists");
        let document_id = session
            .create_document(kind, &path, &source)
            .map_err(|error| error.to_string())?;
        if !folder.is_empty() {
            self.workbench.expanded.insert(folder.to_owned());
        }
        self.explorer.selected = Some(path);
        self.workbench.open(&document_id);
        self.watcher.rescan();
        self.touched();
        Ok(())
    }

    pub(super) fn close_failed_prompt(
        &mut self,
        context: &egui::Context,
        message: String,
    ) -> Option<Prompt> {
        let (mut retry, mut discard, mut keep) = (false, false, false);
        let response = dialogs::centered_modal(egui::Id::new("folder-close-failed"), context).show(
            context,
            |ui| {
                ui.set_width(460.0);
                ui.heading("Unsaved work is not protected");
                ui.add_space(theme::SPACE_SM);
                ui.label(&message);
                ui.label(
                    "Save your documents, try again, or close and discard every unsaved \
                     change.",
                );
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    keep = ui.add(widgets::primary_button("Keep editing")).clicked();
                    retry = ui.button("Try again").clicked();
                    discard = ui.button("Discard unsaved work and close").clicked();
                });
            },
        );
        if retry {
            self.close_requested = true;
            return None;
        }
        if discard {
            if let Some(session) = self.session.take()
                && let Err((session, error)) = session.discard_unsaved_and_close()
            {
                // Nothing was discarded: the window stays usable for Keep
                // editing or Try again.
                self.session = Some(*session);
                return Some(Prompt::CloseFailed(error.to_string()));
            }
            self.close_requested = true;
            return None;
        }
        (!keep && !response.should_close()).then_some(Prompt::CloseFailed(message))
    }
}
