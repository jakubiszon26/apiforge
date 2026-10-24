//! The code view: the active document as text, its problems and save state.

use super::*;

impl FolderApp {
    /// The active document as text, with its diagnostics and save state below.
    pub(super) fn code_area(&mut self, ui: &mut egui::Ui, active: Option<&str>) {
        let Some(document_id) = active else {
            egui::Frame::new().inner_margin(24).show(ui, |ui| {
                widgets::hint(ui, "Open a scenario or component from the document list.");
            });
            return;
        };
        let Some(snapshot) = self.snapshot(document_id) else {
            return;
        };
        egui::Panel::bottom(egui::Id::new((
            "folder-diagnostics",
            self.workbench.mode as u8,
        )))
        .default_size(self.workbench.panels.diagnostics)
        .size_range(72.0..=360.0)
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(12, 8)),
        )
        .show(ui, |ui| {
            self.workbench.panels.diagnostics = ui.available_height() + 16.0;
            self.diagnostics_panel(ui, document_id, &snapshot);
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ui, |ui| self.code_view(ui, document_id, &snapshot));
    }

    pub(super) fn code_view(&mut self, ui: &mut egui::Ui, document_id: &str, snapshot: &Snapshot) {
        // A held draft can belong to a missing file; keeping it recreates it.
        if snapshot.held {
            self.draft_banner(ui, document_id);
        }
        if let Some(unavailable) = snapshot.unavailable {
            widgets::hint(
                ui,
                match unavailable {
                    Unavailable::Missing => "This file is missing from the project folder.",
                    Unavailable::NotUtf8 => "This file is not UTF-8 text and cannot be edited.",
                    Unavailable::TooLarge => {
                        "This file exceeds the 1 MiB source limit and cannot be edited."
                    }
                },
            );
            return;
        }
        if let Some(conflict) = snapshot.conflict {
            self.conflict_banner(ui, document_id, conflict, snapshot);
        }
        let id = editor_id(document_id);
        if self.shown.as_deref() != Some(document_id) {
            self.code = code::CodeEditor::default();
            self.shown = Some(document_id.to_owned());
            self.revealed = None;
            if let Some(tab) = self.workbench.tab_mut(document_id) {
                let selection = clamp_selection(&snapshot.text, &tab.selection);
                self.code.select(selection.anchor, selection.caret);
                self.scroll_restore = Some(tab.scroll);
            }
        }
        let project = self.context_project();
        let cleanup = self.cleanup(document_id);
        let nodes = snapshot
            .model
            .as_ref()
            .map(|model| graph_of(model, cleanup).nodes.clone())
            .unwrap_or_default();
        let parameters = snapshot
            .model
            .as_ref()
            .map(parameters_of)
            .unwrap_or_default();
        let names = crate::shapes::Names::of_board(&project, &nodes, &parameters, None);
        let run = self.fresh_run(document_id);
        let shapes = crate::shapes::BoardShapes {
            project: &project,
            nodes: &nodes,
            steps: run.as_ref().map_or(&[], |run| run.steps.as_slice()),
        };
        let mut text = snapshot.text.clone();
        let mut scroll = egui::ScrollArea::both()
            .id_salt(("folder-code-scroll", document_id))
            .animated(false)
            .auto_shrink([false, false]);
        if let Some(offset) = self.scroll_restore.take() {
            scroll = scroll.vertical_scroll_offset(offset);
        }
        let output = ui
            .add_enabled_ui(!snapshot.held, |ui| {
                scroll.show(ui, |ui| {
                    self.code.show(
                        ui,
                        id,
                        &mut text,
                        &code::Context {
                            mode: apiforge_script::complete::Mode::Script,
                            scope: names.scope(&shapes),
                            diagnostics: &snapshot.diagnostics,
                            line_numbers: true,
                            wrap: false,
                            font_size: 13.0,
                            plain: false,
                        },
                    )
                })
            })
            .inner;
        let offset = output.state.offset.y.max(0.0);
        let outcome = output.inner;
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if outcome.changed {
            match session.edit_source(document_id, &text, CODE) {
                Ok(_) => {
                    self.analyser.typed(document_id, Instant::now());
                    self.changed_at = Some(Instant::now());
                }
                Err(error) => {
                    self.notice = Some(Notice {
                        text: error.to_string(),
                        error: true,
                    });
                    text = snapshot.text.clone();
                }
            }
        }
        if let Some((anchor, caret)) = code::selection(ui.ctx(), id, &text) {
            let selection = TextSelection { anchor, caret };
            let moved = self
                .workbench
                .active_tab()
                .is_some_and(|tab| tab.selection != selection || tab.scroll != offset);
            if let Some(tab) = self.workbench.tab_mut(document_id) {
                tab.selection = selection.clone();
                tab.scroll = offset;
            }
            if session
                .document(document_id)
                .is_some_and(|document| document.selection() != &selection)
            {
                let _ = session.set_selection(document_id, selection);
            }
            if moved {
                self.changed_at = Some(Instant::now());
            }
        }
        self.sync_selection(document_id, &text, snapshot, outcome.cursor);
    }

    /// Keeps the board's selection and the caret pointing at the same block.
    /// With the caret in the text, the board follows it; otherwise a block
    /// selected on the board reveals its statement.
    pub(super) fn sync_selection(
        &mut self,
        document_id: &str,
        text: &str,
        snapshot: &Snapshot,
        cursor: Option<usize>,
    ) {
        let Some(model) = &snapshot.model else {
            return;
        };
        if let Some(cursor) = cursor {
            let Some(handle) = apiforge_script::complete::statement_at(text, cursor) else {
                return;
            };
            let found = [false, true].into_iter().find_map(|cleanup| {
                graph_of(model, cleanup)
                    .nodes
                    .iter()
                    .find(|node| node.handle == handle)
                    .map(|node| (cleanup, node.id.clone()))
            });
            if let Some((cleanup, node_id)) = found {
                if let Some(tab) = self.workbench.tab_mut(document_id)
                    && tab.cleanup != cleanup
                    && matches!(model, Document::Scenario(_))
                {
                    tab.cleanup = cleanup;
                }
                self.selections
                    .insert(document_id.to_owned(), vec![node_id]);
            }
            self.revealed = Some(handle);
            return;
        }
        let cleanup = self.cleanup(document_id);
        let Some(handle) = self
            .selections
            .get(document_id)
            .and_then(|selection| selection.first())
            .and_then(|id| {
                graph_of(model, cleanup)
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
            })
            .map(|node| node.handle.clone())
        else {
            return;
        };
        if self.revealed.as_deref() == Some(handle.as_str()) {
            return;
        }
        if let Some(range) = apiforge_script::complete::statement_range(text, &handle) {
            self.code.reveal(range.start);
            self.revealed = Some(handle);
        }
    }

    pub(super) fn draft_banner(&mut self, ui: &mut egui::Ui, document_id: &str) {
        let mut decision = None;
        egui::Frame::new()
            .fill(theme::ACCENT_SOFT)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    "Unsaved work from an earlier session was written against a different \
                     version of this file. Keep it to continue editing it, or discard it.",
                );
                ui.horizontal(|ui| {
                    if ui
                        .add(widgets::primary_button("Keep unsaved work"))
                        .clicked()
                    {
                        decision = Some(true);
                    }
                    if ui.button("Discard it").clicked() {
                        decision = Some(false);
                    }
                });
            });
        let Some(keep) = decision else {
            return;
        };
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let result = if keep {
            session.keep_draft(document_id)
        } else {
            session.discard_draft(document_id)
        };
        match result {
            Ok(()) => {
                // The text changes under the editor; start it afresh.
                self.shown = None;
                code::clear_history(ui.ctx(), editor_id(document_id));
                self.touched();
            }
            Err(error) => self.notify(error.to_string(), true),
        }
    }

    pub(super) fn diagnostics_panel(
        &mut self,
        ui: &mut egui::Ui,
        document_id: &str,
        snapshot: &Snapshot,
    ) {
        let read_only = self
            .session
            .as_ref()
            .is_some_and(ProjectSession::is_read_only);
        let errors = snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count();
        let warnings = snapshot.diagnostics.len() - errors;
        let mut status = vec![if snapshot.dirty {
            "Unsaved changes".to_owned()
        } else if snapshot.layout_dirty {
            "Unsaved layout".to_owned()
        } else {
            "Saved".to_owned()
        }];
        match snapshot.analysis {
            AnalysisState::Pending => status.push("Checking…".into()),
            AnalysisState::Invalid if errors > 0 => status.push(count(errors, "error")),
            _ => {}
        }
        if warnings > 0 {
            status.push(count(warnings, "warning"));
        }
        if read_only {
            status.push("Read-only".into());
        }
        ui.horizontal(|ui| {
            widgets::caption(ui, "PROBLEMS");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(status.join("  ·  "))
                        .size(theme::FONT_LABEL)
                        .color(if snapshot.analysis == AnalysisState::Invalid {
                            theme::DANGER
                        } else {
                            theme::MUTED
                        }),
                );
            });
        });
        if snapshot.diagnostics.is_empty() {
            widgets::hint(
                ui,
                if snapshot.analysis == AnalysisState::Pending {
                    "Checking the code…"
                } else {
                    "No problems."
                },
            );
            return;
        }
        let mut reveal = None;
        let mut fix = None;
        egui::ScrollArea::vertical()
            .id_salt(("folder-problems", document_id))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (index, diagnostic) in snapshot.diagnostics.iter().enumerate() {
                    let (line, column) = line_column(&snapshot.text, diagnostic.range.start);
                    let colour = if diagnostic.severity == Severity::Error {
                        theme::DANGER
                    } else {
                        theme::WARNING
                    };
                    let response = ui
                        .horizontal(|ui| {
                            ui.label(RichText::new("●").color(colour));
                            let response = ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "Line {line}, column {column}   {}",
                                        diagnostic.message
                                    ))
                                    .size(theme::FONT_LABEL),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if diagnostic.fix.is_some()
                                && ui
                                    .add_enabled(
                                        snapshot.analysis != AnalysisState::Pending
                                            && !read_only
                                            && !snapshot.held
                                            && snapshot.conflict.is_none(),
                                        egui::Button::new("Apply fix"),
                                    )
                                    .clicked()
                            {
                                fix = Some(index);
                            }
                            response
                        })
                        .inner;
                    if response.clicked() {
                        reveal = Some(diagnostic.range.clone());
                    }
                }
            });
        if let Some(index) = fix {
            self.apply_source_fix(document_id, index);
        }
        if let Some(range) = reveal {
            let start = range.start.min(snapshot.text.len());
            let end = range.end.clamp(start, snapshot.text.len());
            self.code.select(start, end);
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(editor_id(document_id)));
        }
    }
}
