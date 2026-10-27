//! The code view: the active document as text, its problems and save state.

use super::*;

/// The heights the Problems section can be dragged between, while it is not
/// expanded.
const PROBLEMS_HEIGHTS: std::ops::RangeInclusive<f32> = 72.0..=360.0;
/// The most code an expanded Problems section leaves above itself (KTLO-18).
const CODE_STRIP: f32 = 96.0;
/// The code panel's margin above and below the editor, and the editor's own
/// margin above its first line.
const CODE_MARGIN: f32 = 6.0;
const EDITOR_MARGIN: f32 = 2.0;
const CODE_FONT: f32 = 13.0;

/// The height of the code panel above an expanded Problems section: as many
/// whole lines as fit in [`CODE_STRIP`] with the margins around them, so the
/// last one is not cut through (KTLO-18's rendering note).
fn code_strip(ui: &egui::Ui) -> f32 {
    let row = ui
        .ctx()
        .fonts_mut(|fonts| fonts.row_height(&egui::FontId::monospace(CODE_FONT)));
    let margins = 2.0 * CODE_MARGIN + EDITOR_MARGIN;
    let rows = ((CODE_STRIP - margins) / row).floor().max(1.0);
    rows * row + margins
}

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
        // Expanded, the section takes the code panel's height less a strip of
        // code, so the line a problem points to is still visible when it is
        // clicked (KTLO-18). The epoch is part of the id, as it is for the run
        // results panel, so egui forgets a height dragged under the other state.
        let expanded = self.problems_expanded;
        let ceiling = (ui.available_height() - code_strip(ui)).max(*PROBLEMS_HEIGHTS.start());
        let diagnostics = egui::Id::new((
            "folder-diagnostics",
            self.workbench.mode as u8,
            self.problems_epoch,
        ));
        let mut reveal = None;
        let shown = egui::Panel::bottom(diagnostics)
            .resizable(true)
            .default_size(if expanded {
                ceiling
            } else {
                self.workbench.panels.diagnostics
            })
            .size_range(if expanded {
                *PROBLEMS_HEIGHTS.start()..=ceiling
            } else {
                PROBLEMS_HEIGHTS
            })
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                if expanded {
                    // A short list would leave the panel's fill short of the
                    // height it was given.
                    ui.set_min_height(ui.available_height());
                }
                reveal = self.diagnostics_panel(ui, document_id, &snapshot);
            });
        // An expanded height is never remembered, so restoring returns the
        // height the section had, and so does the next window. The outer
        // height is what `default_size` takes, so a restore gives it back
        // exactly; the inner height plus margins missed the separator's pixel.
        if !expanded {
            self.workbench.panels.diagnostics = shown.response.rect.height();
        }
        widgets::name_resize_handle(ui.ctx(), diagnostics, "Resize problems");
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(8, CODE_MARGIN as i8)))
            .show(ui, |ui| self.code_view(ui, document_id, &snapshot));
        // Asked for after the editor is drawn: the click on the row is a click
        // outside the editor, which makes it give up the keyboard in the frame
        // it lands in, and with it the selection.
        if let Some(range) = reveal {
            let start = range.start.min(snapshot.text.len());
            let end = range.end.clamp(start, snapshot.text.len());
            self.code.select(start, end);
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(editor_id(document_id)));
            ui.ctx().request_repaint();
        }
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
        // Taken for this frame and put back after drawing: a model is replaced,
        // never changed in place, so an identical one has the same names.
        let completion = self
            .completion_names
            .take()
            .filter(|names| {
                names.describes(document_id, snapshot.model.as_ref(), &project, cleanup)
            })
            .unwrap_or_else(|| {
                CompletionNames::of(
                    document_id,
                    snapshot.model.clone(),
                    project.clone(),
                    cleanup,
                )
            });
        let run = self.fresh_run(document_id);
        let shapes = crate::shapes::BoardShapes {
            project: &project,
            nodes: &completion.nodes,
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
            .add_enabled_ui(
                !snapshot.held && !self.persistence.closing && !self.storage.blocks_edits(),
                |ui| {
                    scroll.show(ui, |ui| {
                        self.code.show(
                            ui,
                            id,
                            &mut text,
                            &code::Context {
                                mode: apiforge_script::complete::Mode::Script,
                                scope: completion.names.scope(&shapes),
                                diagnostics: &snapshot.diagnostics,
                                line_numbers: true,
                                wrap: false,
                                font_size: CODE_FONT,
                                plain: false,
                            },
                        )
                    })
                },
            )
            .inner;
        self.completion_names = Some(completion);
        widgets::name(ui, id, &format!("Contents of {}", snapshot.path));
        if self.persistence.closing || self.storage.blocks_edits() {
            return;
        }
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
            let Some(handle) = self.statement_at(document_id, text, cursor) else {
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
                    && matches!(model.as_ref(), Document::Scenario(_))
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

    /// The handle of the statement under the caret, found again only when the
    /// source or the caret changed. Finding it parses the whole source, so a
    /// large one being typed into is not searched until its analysis settles.
    fn statement_at(&mut self, document_id: &str, text: &str, cursor: usize) -> Option<String> {
        let document = self.session.as_ref()?.document(document_id)?;
        let version = document.source_version();
        if text.len() > code::MAX_COLOURED_BYTES && document.analysis() == AnalysisState::Pending {
            return None;
        }
        if let Some(found) = &self.caret_statement
            && found.document_id == document_id
            && found.version == version
            && found.cursor == cursor
        {
            return found.handle.clone();
        }
        let handle = apiforge_script::complete::statement_at(text, cursor);
        self.caret_statement = Some(CaretStatement {
            document_id: document_id.to_owned(),
            version,
            cursor,
            handle: handle.clone(),
        });
        handle
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
    ) -> Option<std::ops::Range<usize>> {
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
        let expanded = self.problems_expanded;
        let mut toggle = false;
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
                // Between the caption and the status, pointing the way the
                // section moves (KTLO-18).
                let (icon, label) = if expanded {
                    (Icon::ChevronDown, "Restore problems")
                } else {
                    (Icon::ChevronUp, "Expand problems")
                };
                toggle = ui
                    .add(widgets::icon_button(icon, label).compact())
                    .clicked();
            });
        });
        if toggle {
            self.problems_expanded = !expanded;
            self.problems_epoch = self.problems_epoch.wrapping_add(1);
            // The new panel id has no resize response from the previous frame,
            // so its handle would stay unnamed for one frame.
            ui.ctx().request_repaint();
        }
        if snapshot.diagnostics.is_empty() {
            widgets::hint(
                ui,
                if snapshot.analysis == AnalysisState::Pending {
                    "Checking the code…"
                } else {
                    "No problems."
                },
            );
            return None;
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
        reveal
    }
}
