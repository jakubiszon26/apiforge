//! The window around the views: header, tabs, notices, the document list
//! and the dialogs.

use super::*;
use crate::run_mode::RunMode;

impl FolderApp {
    pub(super) fn header(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let name = session.manifest().name.clone();
        let project = session.manifest().id.clone();
        let read_only = session.is_read_only();
        let observing = self.observing();
        // The Run button runs the project's mode (UX-02). Live requests are
        // the holder's to send while this window observes (MCP-08a): the
        // button then simulates, and the stored mode waits for the take-back.
        let live = self.run_mode.mode(&project) == RunMode::Live && !observing;
        let live_allowed = self.live_allowed();
        let unsaved = session
            .documents()
            .iter()
            .any(|document| document.is_dirty() || document.is_layout_dirty())
            || session.text_files().iter().any(|file| file.is_dirty())
            || session.is_manifest_dirty()
            // Typed in the board's header, and written by a save (WS-05a).
            || self.title_typed();
        let active = self.workbench.active();
        let run_document = active
            .and_then(|id| session.document(id))
            // A component runs on its own from its tab (KTLO-23).
            .filter(|document| {
                matches!(
                    document.entry().kind,
                    DocumentKind::Scenario | DocumentKind::Component
                )
            })
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
                .is_some_and(apiforge_project::TextFileSession::is_dirty)
            || self.title_typed();
        // The latest revision of any component, for the room *rN · no
        // changes* takes beside *Publish* (UX-25). A read-only window
        // publishes nothing, so it shows no caption and keeps no room.
        let latest_revision = session
            .manifest()
            .revisions
            .iter()
            .map(|revision| revision.revision)
            .max()
            .filter(|_| !read_only);
        let running = self.running_label(active);
        // Nothing to publish while the component equals its latest revision
        // (UX-25, plan decision 16 = A, COMP-00 Q6 = B). The caption says so
        // only when nothing else keeps *Publish* disabled.
        let unchanged = publish
            .as_ref()
            .filter(|(_, enabled)| *enabled)
            .map(|(id, _)| id.clone())
            .and_then(|id| self.unchanged_revision(&id));
        ui.horizontal(|ui| {
            self.navigation_menu(ui);
            // Keep room for the widest toolbar, a component's with Publish
            // (635 px), and the read-only and live badges, on every tab so
            // that the title keeps its width (KTLO-23 decision 6). A long
            // project name must not paint into the Code tab at 960 px.
            // The badges reserve what they measure, so *Observing* and
            // *Live allowed* together still leave the name its room at 960 px
            // (MCP-09, mock-up M3).
            let badge = |ui: &egui::Ui, text: &str| {
                let font = egui::FontId::proportional(theme::FONT_META);
                ui.fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap(text.to_owned(), font, theme::WARNING)
                        .size()
                        .x
                }) + ui.spacing().item_spacing.x
            };
            // The toolbar's 650 px were measured with *Run* and the text
            // button *Run options*. The switch *Simulation | Live* takes the
            // text button's place and *▶ Run* is that *Run*: the row
            // reserves the same in either mode, so the title does not move
            // when the mode changes (UX-02, UX-42).
            let options_button = ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(
                        "Run options".to_owned(),
                        egui::FontId::proportional(theme::FONT_BODY),
                        theme::TEXT,
                    )
                    .size()
                    .x
            }) + 2.0 * ui.spacing().button_padding.x;
            let mut reserved = 650.0 - options_button + mode_switch_width(ui);
            if read_only {
                reserved += badge(ui, if observing { "Observing" } else { "Read-only" });
            }
            if live_allowed {
                reserved += badge(ui, "Live allowed");
            }
            // *rN · no changes* beside *Publish*, reserved on every tab as
            // the toolbar is, for the highest revision the project has.
            if let Some(revision) = latest_revision {
                reserved += badge(ui, &no_changes(revision));
            }
            // With *Live allowed* and the caption's room at 960 px the name
            // gives way below its usual 60 px: it ends in an ellipsis and
            // shows whole on hover (UX-25, the owner's decision A).
            let minimum = if live_allowed && latest_revision.is_some() {
                TITLE_NARROWEST
            } else {
                60.0
            };
            let title_width = (ui.available_width() - reserved).clamp(minimum, 320.0);
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
                // Another process holds the project and this window follows
                // it (MCP-07); a folder this user cannot write is read-only.
                ui.label(
                    RichText::new(if observing { "Observing" } else { "Read-only" })
                        .size(theme::FONT_META)
                        .color(theme::WARNING),
                );
            }
            if live_allowed {
                // The person's consent is never hidden while it holds
                // (MCP-09, decision 15 A).
                ui.label(
                    RichText::new("Live allowed")
                        .size(theme::FONT_META)
                        .color(theme::WARNING),
                )
                .on_hover_text(
                    "An MCP agent may send live requests in this project. Change it in Project settings.",
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // The width the row takes left of the run controls, measured
                // as it is drawn: the *Running …* label gets what is left
                // (UX-04), and the Run button its form (UX-42). *Cancel run*
                // is narrower than the switch and the Run button, so the
                // title's reservation above holds.
                let rest_id = egui::Id::new("folder-header-rest");
                let rest = ui
                    .ctx()
                    .data(|data| data.get_temp::<f32>(rest_id))
                    .unwrap_or(0.0);
                if self.runs.handle.is_some() {
                    if ui.button("Cancel run").clicked()
                        && let Some(handle) = &self.runs.handle
                    {
                        handle.cancel();
                    }
                    if let Some((text, hover)) = &running {
                        let room = ui.available_width() - rest;
                        running_label(ui, text, *hover, room);
                    }
                } else if let Some(id) = &run_document {
                    // The Run button reads *▶ Run* where the row holds it
                    // with the 8 px a control keeps from its neighbour, and
                    // is ▶ alone where it does not (UX-42, question 12 = A):
                    // the choice follows the row's width as measured, not
                    // the kind of tab.
                    let whole = mode_switch_width(ui)
                        + ui.spacing().item_spacing.x
                        + run_width(ui, false);
                    let compact = ui.available_width() < rest + whole;
                    // Right to left the Run button is the rightmost: its
                    // place is taken first and the button added there after
                    // the switch, which Tab therefore reaches first.
                    let (_, place) = ui.allocate_space(egui::vec2(
                        run_width(ui, compact),
                        theme::CONTROL_HEIGHT,
                    ));
                    // The switch shows the mode the button runs; a click on
                    // a part is the person's choice even when it is the one
                    // shown, as *Simulation* is while observing. Live
                    // requests are the holder's to send while this window
                    // observes (MCP-08a).
                    let shown = if live {
                        RunMode::Live
                    } else {
                        RunMode::Simulation
                    };
                    let chosen = mode_switch(ui, shown, !observing);
                    let run = ui
                        .new_child(
                            egui::UiBuilder::new()
                                .max_rect(place)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        )
                        .add(run_button(live, compact))
                        .clicked();
                    // A click on the stored mode writes nothing: a *Live*
                    // kept while observing stays until another mode is
                    // chosen (UX-02).
                    let stored = self.run_mode.mode(&project);
                    if let Some(mode) = chosen.filter(|mode| *mode != stored) {
                        self.run_mode.set(&project, mode);
                    }
                    if run {
                        self.request_run(id, live);
                    }
                }
                // Measured between left edges: the available width stops at
                // zero when the row is too wide, an edge does not.
                let before_rest = ui.min_rect().left();
                if let Some((id, enabled)) = &publish {
                    let mut button = ui
                        .add_enabled(
                            *enabled && unchanged.is_none(),
                            egui::Button::new("Publish"),
                        )
                        .on_hover_text(
                            "Save this component and publish its next immutable revision",
                        );
                    if unchanged.is_some() {
                        button = button.on_disabled_hover_text(
                            "The component equals its latest revision: nothing to publish",
                        );
                    }
                    if button.clicked() && self.commit_title() {
                        self.component_action(id, publication::Action::Publish);
                    }
                }
                // Right to left: the caption stands at *Publish*'s left.
                if let Some(revision) = unchanged {
                    ui.label(
                        RichText::new(no_changes(revision))
                            .size(theme::FONT_META)
                            .color(theme::MUTED),
                    );
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
                let measured = before_rest - ui.min_rect().left();
                if (measured - rest).abs() > 0.5 {
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(rest_id, measured));
                    // A label laid out with the old width would overlap or
                    // cut short, and the Run button would keep a form the
                    // row no longer has room for (UX-42); this pass is drawn
                    // again with the new one.
                    if running.is_some() || run_document.is_some() {
                        ui.ctx().request_discard("UX-04: the header's row changed width");
                    }
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
            self.followed_tab = None;
            return;
        }
        let active = self.workbench.active().map(str::to_owned);
        // Labels are laid out before anything is drawn, so the strip knows
        // where the active tab is and can reveal it in this same frame.
        let labels = tab_labels(&tabs);
        let font = egui::FontId::proportional(theme::FONT_BODY);
        let texts: Vec<TabText> = tabs
            .iter()
            .zip(&labels)
            .map(|(tab, label)| {
                let format = egui::TextFormat::simple(font.clone(), tab.colour());
                let layout = |job| ui.ctx().fonts_mut(|fonts| fonts.layout_job(job));
                // The dot is laid out apart from the name, and the name is
                // shortened to leave it room, so that a long name cannot
                // truncate it away (UX-15).
                let dot = tab.unsaved.then(|| {
                    layout(egui::text::LayoutJob::single_section(
                        UNSAVED_DOT.to_owned(),
                        format.clone(),
                    ))
                });
                let room = TAB_LABEL_WIDTH - dot.as_ref().map_or(0.0, |dot| dot.size().x);
                let mut job = egui::text::LayoutJob::single_section(label.clone(), format);
                job.wrap = egui::text::TextWrapping::truncate_at_width(room);
                TabText {
                    name: layout(job),
                    dot,
                }
            })
            .collect();
        let widths: Vec<f32> = texts
            .iter()
            .map(|text| TAB_PADDING * 2.0 + text.width() + TAB_GAP + TAB_CLOSE)
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
                for ((tab, text), width) in tabs.iter().zip(texts).zip(&widths) {
                    let selected = active.as_deref() == Some(tab.key.as_str());
                    match tab_face(ui, tab, text, *width, selected) {
                        Some(true) => close = Some(tab.key.clone()),
                        Some(false) => activate = Some(tab.key.clone()),
                        None => {}
                    }
                }
            });
        });
        self.tabs_offset = output.state.offset.x.max(0.0);
        if let Some(document_id) = activate {
            self.navigated_to(&document_id);
            self.activate_tab(&document_id);
            self.followed_tab = Some(document_id);
            self.touched();
        }
        if let Some(document_id) = close {
            self.request_close_tab(&document_id);
        }
    }

    /// The storage indicator, shown only while a tab is open: with no file
    /// open nothing is at stake, and the indicator alone would bring the band
    /// back for a moment and move the central area (KTLO-24, round 2).
    pub(super) fn storage_indicator(&self) -> Option<&'static str> {
        self.storage
            .label()
            .filter(|_| !self.workbench.tabs().is_empty())
    }

    /// Whether [`Self::notices`] draws anything, by the same conditions, so
    /// the band under the app header is left out when it would carry neither
    /// a tab nor a notice (KTLO-24).
    pub(super) fn has_notices(&self) -> bool {
        if self.storage_indicator().is_some() {
            return true;
        }
        let Some(session) = self.session.as_ref() else {
            return false;
        };
        session.recovery_decision_required()
            || (session.recovery_unavailable() && !session.is_read_only())
            || session
                .recovered_drafts()
                .iter()
                .any(|(status, _)| *status == apiforge_project::DraftStatus::Orphan)
            || session.registry_state() != RegistryState::Current
            || self.notice().is_some()
    }

    /// The notices under the tabs; [`Self::has_notices`] says whether any is
    /// due and must change with them.
    pub(super) fn notices(&mut self, ui: &mut egui::Ui) {
        if let Some(label) = self.storage_indicator() {
            ui.horizontal(|ui| {
                ui.spinner();
                widgets::hint(ui, label);
            });
        }
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
                if ui.button("Restore backup").clicked() {
                    self.working_project(
                        "Restoring recovery backup…",
                        ProjectSession::restore_recovery_backup,
                        |app, result| match result {
                            Ok(()) => {
                                app.shown = None;
                                app.reload_field_drafts();
                                app.restore_text_tabs();
                            }
                            Err(error) => app.notify(error.to_string(), true),
                        },
                    );
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
        if let Some(notice) = self.notice() {
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
            self.notices.dismiss(self.workbench.active());
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
        self.working_project(
            "Reloading project…",
            move |session| {
                if discard {
                    session.discard_configuration()?;
                }
                session.reload_project()
            },
            |app, result| match result {
                Ok(()) => {
                    app.follow_external(ExternalOutcome {
                        project_reloaded: true,
                        ..Default::default()
                    });
                    app.watcher.rescan();
                }
                Err(error) => app.notify(error.to_string(), true),
            },
        );
    }

    pub(super) fn prompts(&mut self, context: &egui::Context) {
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        self.prompt = match prompt {
            Prompt::Working { ticket, label } => {
                dialogs::centered_modal(egui::Id::new("folder-working"), context).show(
                    context,
                    |ui| {
                        ui.set_width(340.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(label);
                        });
                    },
                );
                Some(Prompt::Working { ticket, label })
            }
            Prompt::Conflict(view) => self.conflict_prompt(context, *view),
            Prompt::Transaction(view) => self.transaction_prompt(context, *view),
            Prompt::Settings(dialog) => self.project_settings_prompt(context, *dialog),
            Prompt::Environment(dialog) => self.environment_prompt(context, *dialog),
            Prompt::Run { prepared, live } => self.run_prompt(context, *prepared, live),
            Prompt::RunInputs {
                prepared,
                live,
                dialog,
            } => self.run_inputs_prompt(context, *prepared, live, *dialog),
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
            Prompt::NewDocument { kind, name, error } => {
                self.new_document_prompt(context, kind, name, error)
            }
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
            Prompt::DeleteStatements {
                document_id,
                cleanup,
                ids,
                wires,
                question,
                gone,
            } => self.delete_statements_prompt(
                context,
                document_id,
                cleanup,
                wires,
                ConfirmedDelete {
                    ids,
                    question,
                    gone,
                },
            ),
            Prompt::Import { rows, error } => self.import_prompt(context, rows, error),
            Prompt::Trash { operations, error } => self.trash_prompt(context, operations, error),
            Prompt::Register { path, kind, error } => {
                self.register_prompt(context, path, kind, error)
            }
            Prompt::Handover { client, role } => self.handover_prompt(context, client, role),
            Prompt::SaveComponent(dialog) => self.save_component_prompt(context, dialog),
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
        let id = document_id.to_owned();
        if save {
            self.save_document_with(document_id, move |app, result| match result {
                Ok(saved)
                    if app
                        .session
                        .as_ref()
                        .is_some_and(|session| saved.matches(session, &id)) =>
                {
                    app.close_tab(&id)
                }
                Ok(_) => {
                    app.notify(
                        "Newer changes remain unsaved. Save them before closing this tab.",
                        true,
                    );
                    app.prompt = Some(Prompt::CloseTab(id));
                }
                Err(error) => app.notify(format!("Could not save before closing: {error}"), true),
            });
        } else {
            let text = self.text_of(document_id).map(str::to_owned);
            let target = id.clone();
            self.storage_project(
                "Discarding file changes…",
                apiforge_project::StorageMode::Exclusive,
                true,
                move |session| match text {
                    Some(path) => session.revert_text_file(&path, DIAGRAM),
                    None => session.revert(&target, DIAGRAM),
                },
                move |app, result| match result {
                    Ok(()) => {
                        app.shown = None;
                        inspector::forget_field_text(&app.ui_context);
                        app.reload_field_drafts();
                        app.close_tab(&id);
                    }
                    Err(error) => {
                        app.notify(format!("Could not discard the changes: {error}"), true)
                    }
                },
            );
        }
    }

    pub(super) fn new_document_prompt(
        &mut self,
        context: &egui::Context,
        kind: DocumentKind,
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
                let field = widgets::named(
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)),
                    "Name",
                );
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
            match self.create_document(kind, name.trim()) {
                Ok(()) => return self.prompt.take(),
                Err(message) => error = Some(message),
            }
        }
        Some(Prompt::NewDocument { kind, name, error })
    }

    /// Creates a scenario in `scenarios/` or a component in `components/`,
    /// whatever row of the explorer is selected (UX-22, decision 13).
    pub(super) fn create_document(&mut self, kind: DocumentKind, name: &str) -> Result<(), String> {
        if self.session.is_none() {
            return Err("No project is open.".into());
        }
        let name = name.to_owned();
        let failed = Prompt::NewDocument {
            kind,
            name: name.clone(),
            error: None,
        };
        self.working_project(
            "Creating document…",
            move |session| {
                let (folder, source) = match kind {
                    DocumentKind::Scenario => ("scenarios", scenario_template(&name)),
                    DocumentKind::Component => ("components", component_template(&name)),
                };
                let stem = file_stem(&name);
                let path = (1..=apiforge_storage::project::MAX_TREE_ENTRIES)
                    .map(|index| {
                        let name = if index == 1 {
                            format!("{stem}.afs")
                        } else {
                            format!("{stem}-{index}.afs")
                        };
                        format!("{folder}/{name}")
                    })
                    .find(|path| {
                        !session
                            .documents()
                            .iter()
                            .any(|doc| doc.entry().path.as_str().eq_ignore_ascii_case(path))
                            && !session.store().root().join(path).exists()
                    })
                    .ok_or(apiforge_project::ProjectError::Limit)?;
                let id = session.create_document(kind, &path, &source)?;
                Ok((id, path, folder))
            },
            move |app, result| match result {
                Ok((id, path, folder)) => {
                    app.workbench.expanded.insert(folder.to_owned());
                    app.explorer.selected = Some(path);
                    app.activate_tab(&id);
                    app.watcher.rescan();
                    app.touched();
                }
                Err(error) => app.restore_prompt(failed, error.to_string()),
            },
        );
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
            self.persistence.discard = true;
            self.persistence.departure = true;
            self.persistence.approved_export = None;
            self.persistence.departure_generation += 1;
            self.runs.exporting = false;
            self.navigation.failed = false;
            self.advance_departure();
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
/// What follows the name of a tab with unsaved changes.
const UNSAVED_DOT: &str = "  \u{25cf}";

/// A tab's text, laid out: the name, shortened to fit, and the unsaved dot
/// after it, which the shortening never reaches.
struct TabText {
    name: Arc<egui::Galley>,
    dot: Option<Arc<egui::Galley>>,
}

impl TabText {
    fn width(&self) -> f32 {
        self.name.size().x + self.dot.as_ref().map_or(0.0, |dot| dot.size().x)
    }
}

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
    text: TabText,
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
    let mut x = rect.left() + TAB_PADDING;
    for galley in std::iter::once(text.name).chain(text.dot) {
        let width = galley.size().x;
        painter.galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            tab.colour(),
        );
        x += width;
    }
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

const SIMULATE_HOVER: &str = "Simulate the selected scenario or component with sample responses, including unsaved valid code";
const LIVE_HOVER: &str = "Send the requests of the selected scenario or component to the API, including unsaved valid code";

/// The text of the header's *Running …* label: grey, at the label size
/// (UX-04, decided on the UX-00 mock-up).
pub(super) fn running_text(text: &str) -> RichText {
    RichText::new(text)
        .size(theme::FONT_LABEL)
        .color(theme::MUTED)
}

/// The header's *Running …* label just left of *Cancel run* (UX-04). It
/// takes at most `room`; a longer text, a long file name on a narrow
/// window, is cut with an ellipsis and its hover starts with the whole text.
fn running_label(ui: &mut egui::Ui, text: &str, hover: Option<&str>, room: f32) {
    let whole = ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(
                text.to_owned(),
                egui::FontId::proportional(theme::FONT_LABEL),
                theme::MUTED,
            )
            .size()
            .x
    });
    let room = room.max(0.0);
    let label = ui
        .allocate_ui_with_layout(
            egui::vec2(room, theme::CONTROL_HEIGHT),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(running_text(text))
                        .truncate()
                        .show_tooltip_when_elided(false),
                )
            },
        )
        .inner;
    let hover = match (whole > room, hover) {
        (true, Some(hover)) => Some(format!("{text}\n{hover}")),
        (true, None) => Some(text.to_owned()),
        (false, hover) => hover.map(str::to_owned),
    };
    if let Some(hover) = hover {
        label.on_hover_text(hover);
    }
}

/// The narrowest the project name gets in the header: at 960 px with *Live
/// allowed*, the run controls and the room for *rN · no changes* (UX-25).
const TITLE_NARROWEST: f32 = 36.0;

/// The caption beside a disabled *Publish* (COMP-00 Q6 = B).
fn no_changes(revision: u32) -> String {
    format!("r{revision} · no changes")
}

fn run_label(live: bool) -> &'static str {
    if live { "Run live" } else { "Run simulation" }
}

/// The header's Run button, *▶ Run*, in the violet that also marks a
/// simulation in Run results or in the accent for a live run (UX-02). It
/// is named for its mode, *Run simulation* or *Run live*, and its hover
/// says what it sends; where the row is too short for its word it is ▶
/// alone, and its one tooltip gives that name first (UX-42).
pub(super) fn run_button(live: bool, compact: bool) -> widgets::ActionButton<'static> {
    let name = run_label(live);
    let hover = if live { LIVE_HOVER } else { SIMULATE_HOVER };
    let button = if compact {
        widgets::icon_button(Icon::Play, name).hover(hover)
    } else {
        widgets::action(Icon::Play, "Run").named(name).hover(hover)
    }
    .primary();
    if live {
        button
    } else {
        button.fill(theme::SIMULATION)
    }
}

/// The width [`run_button`] takes.
fn run_width(ui: &egui::Ui, compact: bool) -> f32 {
    if compact {
        return theme::CONTROL_HEIGHT;
    }
    let bold = egui::FontId::new(theme::FONT_BODY, theme::bold_family());
    // The icon's 16 px, 10 px either side and 6 px before the word.
    20.0 + 16.0
        + 6.0
        + ui.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap("Run".to_owned(), bold, theme::TEXT)
                .size()
                .x
        })
}

/// The run mode switch's parts: the mode, its name and its hover text, the
/// hints of UX-02's menu.
const MODE_PARTS: [(RunMode, &str, &str); 2] = [
    (
        RunMode::Simulation,
        "Simulation",
        "Sample responses; no request is sent.",
    ),
    (RunMode::Live, "Live", "Requests go to the project's API."),
];
/// The room either side of a part's name, and around the parts.
const PART_PADDING: f32 = 10.0;
const SWITCH_INSET: f32 = 2.0;

fn part_width(ui: &egui::Ui, name: &str) -> f32 {
    ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(
                name.to_owned(),
                egui::FontId::proportional(theme::FONT_LABEL),
                theme::TEXT,
            )
            .size()
            .x
    }) + 2.0 * PART_PADDING
}

/// The width [`mode_switch`] takes, the same in either mode.
fn mode_switch_width(ui: &egui::Ui) -> f32 {
    MODE_PARTS
        .iter()
        .map(|(_, name, _)| part_width(ui, name))
        .sum::<f32>()
        + 2.0 * SWITCH_INSET
}

/// The two-part switch *Simulation | Live* left of the Run button (UX-42),
/// one radio group, *Run mode*, for assistive technology. The part `shown`
/// is white with its name in its mode's colour, the other muted; *Live* is
/// disabled with MCP-08a's reason unless `live_enabled`. Tab reaches the
/// part shown, and Left and Right move the choice. Returns the part
/// clicked or moved to; it runs nothing.
fn mode_switch(ui: &mut egui::Ui, shown: RunMode, live_enabled: bool) -> Option<RunMode> {
    let (rect, group) = ui.allocate_exact_size(
        egui::vec2(mode_switch_width(ui), theme::CONTROL_HEIGHT),
        egui::Sense::hover(),
    );
    group.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::RadioGroup, ui.is_enabled(), "Run mode")
    });
    ui.painter().rect_filled(rect, 8, theme::FIELD);
    let mut parts = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .accessibility_parent(group.id),
    );
    let id = |mode: RunMode| egui::Id::new(("folder-run-mode", mode == RunMode::Live));
    let mut chosen = None;
    let mut focus = None;
    let mut left = rect.left() + SWITCH_INSET;
    for (mode, name, hint) in MODE_PARTS {
        let width = part_width(ui, name);
        let segment = egui::Rect::from_min_max(
            egui::pos2(left, rect.top() + SWITCH_INSET),
            egui::pos2(left + width, rect.bottom() - SWITCH_INSET),
        );
        left += width;
        let on = mode == shown;
        let enabled = mode == RunMode::Simulation || live_enabled;
        // Only the part shown is a focus stop: the arrows move between the
        // two, as in any radio group.
        let sense = if on {
            egui::Sense::click()
        } else {
            egui::Sense::CLICK
        };
        let response = parts
            .add_enabled_ui(enabled, |ui| ui.interact(segment, id(mode), sense))
            .inner;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, on, name)
        });
        let colour = match mode {
            RunMode::Live => theme::ACCENT,
            RunMode::Simulation => theme::SIMULATION,
        };
        let painter = ui.painter();
        if on {
            painter.rect(
                segment,
                6,
                theme::WHITE,
                egui::Stroke::new(1.0, theme::BORDER),
                egui::StrokeKind::Inside,
            );
        } else if enabled && response.hovered() {
            painter.rect_filled(segment, 6, theme::BORDER);
        }
        if response.has_focus() {
            painter.rect_stroke(
                segment.shrink(0.5),
                6,
                egui::Stroke::new(1.5, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        painter.text(
            segment.center(),
            egui::Align2::CENTER_CENTER,
            name,
            if on {
                egui::FontId::new(theme::FONT_LABEL, theme::bold_family())
            } else {
                egui::FontId::proportional(theme::FONT_LABEL)
            },
            if on {
                colour
            } else if enabled {
                theme::MUTED
            } else {
                theme::MUTED.gamma_multiply(0.5)
            },
        );
        if response.has_focus() {
            // The arrows move the choice, not the focus to a neighbour.
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        ..Default::default()
                    },
                );
            });
            let (back, forth) = ui.input(|input| {
                (
                    input.key_pressed(egui::Key::ArrowLeft),
                    input.key_pressed(egui::Key::ArrowRight),
                )
            });
            let to = match (back, forth) {
                (true, false) => Some(RunMode::Simulation),
                (false, true) if live_enabled => Some(RunMode::Live),
                _ => None,
            };
            if let Some(to) = to.filter(|to| *to != shown) {
                chosen = Some(to);
                focus = Some(to);
            }
        }
        let response = if enabled {
            response
                .on_hover_text(hint)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response.on_disabled_hover_text(runs::LIVE_PAUSED)
        };
        if response.clicked() {
            chosen = Some(mode);
        }
    }
    // After both parts: the one moved to is no focus stop in this frame, and
    // egui takes the focus from a widget that is not.
    if let Some(to) = focus {
        ui.memory_mut(|memory| memory.request_focus(id(to)));
    }
    chosen
}
