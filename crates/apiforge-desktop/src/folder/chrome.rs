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
            || session.text_files().iter().any(|file| file.is_dirty())
            || session.is_manifest_dirty();
        let active = self.workbench.active();
        let run_document = active
            .and_then(|id| session.document(id))
            .filter(|document| document.entry().kind == DocumentKind::Scenario)
            .map(|document| document.id().to_owned());
        let publish = active
            .and_then(|id| session.document(id))
            .filter(|document| document.entry().kind == DocumentKind::Component)
            .map(|document| {
                (
                    document.id().to_owned(),
                    session.publishable(document.id()).is_ok(),
                )
            });
        let active_unsaved = active
            .and_then(|active| session.document(active))
            .is_some_and(|document| document.is_dirty() || document.is_layout_dirty())
            || active
                .and_then(|active| active.strip_prefix(TEXT_TAB))
                .and_then(|path| session.text_file(path))
                .is_some_and(apiforge_project::TextFileSession::is_dirty);
        ui.horizontal(|ui| {
            self.navigation_menu(ui);
            // Keep room for the full scenario toolbar and the read-only badge.
            // A long project name must not paint into the Code tab at 960 px.
            let reserved = 620.0 + if read_only { 80.0 } else { 0.0 };
            let title_width = (ui.available_width() - reserved).clamp(60.0, 320.0);
            ui.allocate_ui_with_layout(
                egui::vec2(title_width, theme::CONTROL_HEIGHT),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(name)
                                .size(theme::FONT_BODY)
                                .family(egui::FontFamily::Proportional),
                        )
                        .truncate(),
                    )
                },
            )
            .inner
            .on_hover_text(self.root.display().to_string());
            if read_only {
                ui.label(
                    RichText::new("Read-only")
                        .size(theme::FONT_META)
                        .color(theme::WARNING),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.runs.handle.is_some() {
                    if ui.button("Cancel run").clicked()
                        && let Some(handle) = &self.runs.handle
                    {
                        handle.cancel();
                    }
                } else if let Some(id) = &run_document {
                    if ui
                        .add(widgets::action(Icon::Play, "Run").primary())
                        .on_hover_text(
                            "Simulate the selected scenario, including unsaved valid code",
                        )
                        .clicked()
                    {
                        self.request_run(id, false);
                    }
                    ui.menu_button("Run options", |ui| {
                        if ui.button("Run live requests").clicked() {
                            ui.close();
                            self.request_run(id, true);
                        }
                        if ui.button("Review legacy runs").clicked() {
                            ui.close();
                            self.review_legacy_runs();
                        }
                    });
                } else {
                    ui.menu_button("Runs", |ui| {
                        if ui.button("Review legacy runs").clicked() {
                            ui.close();
                            self.review_legacy_runs();
                        }
                    });
                }
                if let Some((id, enabled)) = &publish
                    && ui
                        .add_enabled(*enabled, egui::Button::new("Publish"))
                        .on_hover_text(
                            "Save this component and publish its next immutable revision",
                        )
                        .clicked()
                {
                    self.component_action(id, publication::Action::Publish);
                }
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
        let tabs: Vec<TabFace> = self
            .workbench
            .tabs()
            .iter()
            .filter_map(|tab| {
                let key = tab.document_id.clone();
                if let Some(document) = session.document(&key) {
                    return Some(TabFace {
                        path: document.entry().path.as_str().to_owned(),
                        unsaved: document.needs_recovery(),
                        invalid: document.analysis() == AnalysisState::Invalid,
                        conflict: document.conflict().is_some(),
                        key,
                    });
                }
                let file = session.text_file(key.strip_prefix(TEXT_TAB)?)?;
                Some(TabFace {
                    path: file.path().to_owned(),
                    unsaved: file.needs_recovery(),
                    invalid: false,
                    conflict: file.conflict().is_some(),
                    key,
                })
            })
            .collect();
        if tabs.is_empty() {
            ui.add_space(theme::CONTROL_HEIGHT - 4.0);
            self.followed_tab = None;
            return;
        }
        let active = self.workbench.active().map(str::to_owned);
        // Labels are laid out before anything is drawn, so the strip knows
        // where the active tab is and can reveal it in this same frame.
        let labels = tab_labels(&tabs);
        let font = egui::FontId::proportional(theme::FONT_BODY);
        let galleys: Vec<Arc<egui::Galley>> = tabs
            .iter()
            .zip(&labels)
            .map(|(tab, label)| {
                let mut job = egui::text::LayoutJob::single_section(
                    if tab.unsaved {
                        format!("{label}  \u{25cf}")
                    } else {
                        label.clone()
                    },
                    egui::TextFormat::simple(font.clone(), tab.colour()),
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(TAB_LABEL_WIDTH);
                ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
            })
            .collect();
        let widths: Vec<f32> = galleys
            .iter()
            .map(|galley| TAB_PADDING * 2.0 + galley.size().x + TAB_GAP + TAB_CLOSE)
            .collect();
        let mut scroll = egui::ScrollArea::horizontal()
            .id_salt("folder-tab-strip")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded);
        // The active tab must be reachable, so activating one scrolls it in.
        // A narrower window can push it out of view just as activation can.
        let width = ui.available_width();
        let resized = (width - self.tabs_width).abs() > 0.5;
        self.tabs_width = width;
        if active != self.followed_tab || resized {
            self.followed_tab = active.clone();
            if let Some(index) = active
                .as_deref()
                .and_then(|active| tabs.iter().position(|tab| tab.key == active))
            {
                let left: f32 = widths[..index].iter().sum::<f32>() + index as f32 * TAB_SPACING;
                let right = left + widths[index];
                let viewport = width;
                if left < self.tabs_offset {
                    scroll = scroll.horizontal_scroll_offset(left);
                } else if right > self.tabs_offset + viewport {
                    scroll = scroll.horizontal_scroll_offset(right - viewport);
                }
            }
        }
        let mut activate = None;
        let mut close = None;
        let output = scroll.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = TAB_SPACING;
                for ((tab, galley), width) in tabs.iter().zip(galleys).zip(&widths) {
                    let selected = active.as_deref() == Some(tab.key.as_str());
                    match tab_face(ui, tab, galley, *width, selected) {
                        Some(true) => close = Some(tab.key.clone()),
                        Some(false) => activate = Some(tab.key.clone()),
                        None => {}
                    }
                }
            });
        });
        self.tabs_offset = output.state.offset.x.max(0.0);
        if let Some(document_id) = activate {
            self.workbench.open(&document_id);
            self.followed_tab = Some(document_id);
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
            Prompt::Conflict(view) => self.conflict_prompt(context, *view),
            Prompt::Transaction(view) => self.transaction_prompt(context, *view),
            Prompt::Settings(dialog) => self.project_settings_prompt(context, *dialog),
            Prompt::Environment(dialog) => self.environment_prompt(context, *dialog),
            Prompt::Run { prepared, live } => self.run_prompt(context, *prepared, live),
            Prompt::Journal(review) => self.journal_prompt(context, *review),
            Prompt::AssignResult {
                document_id,
                cleanup,
                dialog,
            } => self.assign_result_prompt(context, document_id, cleanup, *dialog),
            Prompt::CommentLoss { ticket, action } => {
                self.comment_loss_prompt(context, ticket, action)
            }
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
            Prompt::Register { path, kind, error } => {
                self.register_prompt(context, path, kind, error)
            }
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
        let text = self.text_of(document_id).map(str::to_owned);
        let result = if save {
            self.save_document(document_id)
        } else {
            let document_id = document_id.to_owned();
            self.session
                .as_mut()
                .ok_or_else(String::new)
                .and_then(|session| {
                    match &text {
                        Some(path) => session.revert_text_file(path, DIAGRAM),
                        None => session.revert(&document_id, DIAGRAM),
                    }
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
        let (mut retry, mut discard, mut keep, mut export) = (false, false, false, false);
        let switching = self.switching();
        let response = dialogs::centered_modal(egui::Id::new("folder-close-failed"), context).show(
            context,
            |ui| {
                ui.set_width(460.0);
                ui.heading("Unsaved work is not protected");
                ui.add_space(theme::SPACE_SM);
                ui.label(&message);
                ui.label(
                    "Save your documents, export your work, retry, or discard unsaved changes.",
                );
                ui.add_space(theme::SPACE_MD);
                ui.horizontal_wrapped(|ui| {
                    keep = ui.add(widgets::primary_button("Keep editing")).clicked();
                    retry = ui.button("Try again").clicked();
                    export = ui
                        .add_enabled(
                            !self.runs.exporting,
                            egui::Button::new(if switching {
                                "Export and open folder"
                            } else {
                                "Export and close"
                            }),
                        )
                        .clicked();
                    discard = ui
                        .button(if switching {
                            "Discard unsaved work and open folder"
                        } else {
                            "Discard unsaved work and close"
                        })
                        .clicked();
                });
            },
        );
        if export {
            self.export_before_close();
        }
        if retry {
            if switching {
                self.navigation.failed = false;
            } else {
                self.close_requested = true;
            }
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
            self.runs = runs::Runs::default();
            self.finish_departure();
            return self.prompt.take();
        }
        if keep || response.should_close() {
            self.cancel_switch();
        }
        (!keep && !response.should_close()).then_some(Prompt::CloseFailed(message))
    }
}

/// Tab geometry: the name, its state and the close control share one surface,
/// so the control is always inside the tab the user is pointing at.
const TAB_PADDING: f32 = 10.0;
const TAB_GAP: f32 = 6.0;
pub(super) const TAB_CLOSE: f32 = 14.0;
const TAB_SPACING: f32 = 4.0;
const TAB_HEIGHT: f32 = 26.0;
/// A long name is bounded so that the other tabs and controls stay reachable;
/// the full path is on hover.
pub(super) const TAB_LABEL_WIDTH: f32 = 160.0;

/// What one tab shows this frame.
pub(super) struct TabFace {
    key: String,
    path: String,
    unsaved: bool,
    invalid: bool,
    conflict: bool,
}

impl TabFace {
    fn colour(&self) -> egui::Color32 {
        if self.conflict {
            theme::WARNING
        } else if self.invalid {
            theme::DANGER
        } else {
            theme::TEXT
        }
    }
}

/// Each tab's label: the file name, plus the least parent path that tells it
/// apart from another open file of the same name.
fn tab_labels(tabs: &[TabFace]) -> Vec<String> {
    let parts: Vec<Vec<&str>> = tabs
        .iter()
        .map(|tab| tab.path.split('/').collect())
        .collect();
    let suffix = |segments: &[&str], depth: usize| segments[segments.len() - depth..].join("/");
    parts
        .iter()
        .enumerate()
        .map(|(index, segments)| {
            let mut depth = 1;
            while depth < segments.len()
                && parts.iter().enumerate().any(|(other, candidate)| {
                    other != index
                        && candidate.len() >= depth
                        && suffix(candidate, depth) == suffix(segments, depth)
                })
            {
                depth += 1;
            }
            suffix(segments, depth)
        })
        .collect()
}

/// Draws one tab. Returns `Some(true)` when its close control was used,
/// `Some(false)` when the tab itself was activated.
fn tab_face(
    ui: &mut egui::Ui,
    tab: &TabFace,
    galley: Arc<egui::Galley>,
    width: f32,
    selected: bool,
) -> Option<bool> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, TAB_HEIGHT), egui::Sense::click_and_drag());
    let close_rect = egui::Rect::from_center_size(
        egui::pos2(
            rect.right() - TAB_PADDING - TAB_CLOSE / 2.0,
            rect.center().y,
        ),
        egui::Vec2::splat(TAB_CLOSE),
    );
    let close = ui.interact(
        close_rect,
        ui.id().with(("folder-tab-close", &tab.key)),
        egui::Sense::click(),
    );
    close.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            format!("Close {}", file_name(&tab.path)),
        )
    });
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            file_name(&tab.path),
        )
    });
    let painter = ui.painter_at(rect.union(close_rect));
    let fill = if selected {
        theme::WHITE
    } else if response.hovered() || close.hovered() {
        theme::FIELD
    } else {
        theme::PANEL
    };
    painter.rect_filled(rect, 6.0, fill);
    if selected {
        painter.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
    }
    painter.galley(
        egui::pos2(
            rect.left() + TAB_PADDING,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        tab.colour(),
    );
    Icon::Close.paint(
        &painter,
        close_rect,
        if close.hovered() {
            theme::TEXT
        } else {
            theme::MUTED
        },
    );
    let hover = if tab.conflict {
        format!("{} — changed outside API Forge", tab.path)
    } else {
        tab.path.clone()
    };
    let response = response.on_hover_text(&hover);
    let close = close.on_hover_text(format!("Close {} (Ctrl+W)", file_name(&tab.path)));
    if close.clicked() {
        return Some(true);
    }
    // The close control sits inside the tab, so a click on it is also a click
    // on the tab; it must not activate the file it is closing.
    if (response.clicked() && !close.contains_pointer()) || response.middle_clicked() {
        return Some(false);
    }
    None
}
