//! Project text files in the window: a tab per open file, its exact text in the
//! editor, and its save, close, recovery and conflict decisions.
//!
//! A text file is not a definition, so it has no diagram, no problems from the
//! language and no run. It is addressed by path, behind the `text:` tab prefix.

use super::*;

/// A frame's copy of what one open text file shows.
pub(super) struct TextSnapshot {
    pub(super) path: String,
    pub(super) text: String,
    pub(super) dirty: bool,
    pub(super) held: bool,
    pub(super) missing: bool,
    pub(super) conflict: Option<ExternalConflict>,
}

impl FolderApp {
    /// The path of the text file a tab shows, if it shows one. A document with
    /// this exact ID wins, so a text tab can never hide a definition.
    pub(super) fn text_of(&self, tab: &str) -> Option<&str> {
        let session = self.session.as_ref()?;
        if session.document(tab).is_some() {
            return None;
        }
        session
            .text_file(tab.strip_prefix(TEXT_TAB)?)
            .map(apiforge_project::TextFileSession::path)
    }

    pub(super) fn text_snapshot(&self, tab: &str) -> Option<TextSnapshot> {
        let path = self.text_of(tab)?;
        let file = self.session.as_ref()?.text_file(path)?;
        Some(TextSnapshot {
            path: file.path().to_owned(),
            text: file.text().to_owned(),
            dirty: file.is_dirty(),
            held: file.is_held(),
            missing: file.is_missing(),
            conflict: file.conflict(),
        })
    }

    /// Opens a project file as text in its own tab, or activates the tab it
    /// already has. A file that cannot be edited as text says so.
    pub(super) fn open_text(&mut self, path: &str) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        match session.open_text_file(path) {
            Ok(_) => {
                self.workbench.open(&text_tab(path));
                self.touched();
            }
            Err(error) => self.notify(
                format!("Cannot open {} as text: {error}", file_name(path)),
                true,
            ),
        }
    }

    /// Opens the text files of restored tabs and of unsaved work held in
    /// recovery, so nothing unsaved stays hidden. A tab whose file can no
    /// longer be opened is dropped.
    pub(super) fn restore_text_tabs(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let mut paths: Vec<String> = self
            .workbench
            .tabs()
            .iter()
            .filter_map(|tab| tab.document_id.strip_prefix(TEXT_TAB))
            .map(str::to_owned)
            .collect();
        for draft in session.recovered_text_drafts() {
            if !paths.iter().any(|path| path == draft) {
                paths.push(draft.to_owned());
            }
        }
        for path in paths {
            let Some(session) = self.session.as_mut() else {
                return;
            };
            match session.open_text_file(&path) {
                Ok(_) => self.workbench.open(&text_tab(&path)),
                Err(_) => self.workbench.close(&text_tab(&path)),
            }
        }
    }

    pub(super) fn save_text(&mut self, path: &str) -> Result<(), String> {
        let session = self.session.as_mut().ok_or_else(String::new)?;
        session
            .save_text_file(path)
            .map(|_| ())
            .map_err(|error| format!("Could not save {}: {error}", file_name(path)))
    }

    /// The active text file, with its state below and its decisions above.
    pub(super) fn text_area(&mut self, ui: &mut egui::Ui, snapshot: &TextSnapshot) {
        egui::Panel::bottom(egui::Id::new((
            "folder-text-status",
            self.workbench.mode as u8,
        )))
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(12, 8)),
        )
        .show(ui, |ui| self.text_status(ui, snapshot));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ui, |ui| self.text_view(ui, snapshot));
    }

    fn text_status(&mut self, ui: &mut egui::Ui, snapshot: &TextSnapshot) {
        let read_only = self
            .session
            .as_ref()
            .is_some_and(ProjectSession::is_read_only);
        let mut status = vec![if snapshot.dirty {
            "Unsaved changes".to_owned()
        } else {
            "Saved".to_owned()
        }];
        status.push("Text file".into());
        if snapshot.missing {
            status.push("Not in the folder".into());
        }
        if read_only {
            status.push("Read-only".into());
        }
        ui.horizontal(|ui| {
            widgets::caption(ui, "FILE");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(status.join("  ·  "))
                        .size(theme::FONT_LABEL)
                        .color(theme::MUTED),
                );
            });
        });
        widgets::hint(
            ui,
            "Saved exactly as it is here. This file is not a scenario or a component, so it \
             has no diagram and cannot be run.",
        );
    }

    fn text_view(&mut self, ui: &mut egui::Ui, snapshot: &TextSnapshot) {
        if snapshot.held {
            self.text_draft_banner(ui, &snapshot.path);
        }
        if let Some(conflict) = snapshot.conflict {
            self.text_conflict_banner(ui, snapshot, conflict);
        }
        let tab = text_tab(&snapshot.path);
        let id = editor_id(&tab);
        if self.shown.as_deref() != Some(tab.as_str()) {
            self.code = code::CodeEditor::default();
            self.shown = Some(tab.clone());
            self.revealed = None;
            if let Some(state) = self.workbench.tab_mut(&tab) {
                let selection = clamp_selection(&snapshot.text, &state.selection);
                self.code.select(selection.anchor, selection.caret);
                self.scroll_restore = Some(state.scroll);
            }
        }
        let mut text = snapshot.text.clone();
        let mut scroll = egui::ScrollArea::both()
            .id_salt(("folder-text-scroll", &snapshot.path))
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
                            plain: true,
                            line_numbers: true,
                            wrap: false,
                            font_size: 13.0,
                            ..code::Context::default()
                        },
                    )
                })
            })
            .inner;
        let offset = output.state.offset.y.max(0.0);
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if output.inner.changed {
            match session.edit_text(&snapshot.path, &text, CODE) {
                Ok(_) => self.changed_at = Some(Instant::now()),
                Err(error) => self.notify(error.to_string(), true),
            }
        }
        if let Some((anchor, caret)) = code::selection(ui.ctx(), id, &text) {
            let selection = TextSelection { anchor, caret };
            let moved = self
                .workbench
                .active_tab()
                .is_some_and(|state| state.selection != selection || state.scroll != offset);
            if let Some(state) = self.workbench.tab_mut(&tab) {
                state.selection = selection.clone();
                state.scroll = offset;
            }
            if let Some(session) = self.session.as_mut() {
                let _ = session.set_text_selection(&snapshot.path, selection);
            }
            if moved {
                self.changed_at = Some(Instant::now());
            }
        }
    }

    fn text_draft_banner(&mut self, ui: &mut egui::Ui, path: &str) {
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
            session.keep_text_draft(path)
        } else {
            session.discard_text_draft(path)
        };
        match result {
            Ok(()) => {
                // The text changes under the editor; start it afresh.
                self.shown = None;
                code::clear_history(ui.ctx(), editor_id(&text_tab(path)));
                self.touched();
            }
            Err(error) => self.notify(error.to_string(), true),
        }
    }

    /// The file changed or disappeared outside API Forge while it had unsaved
    /// work. Nothing is merged and nothing is overwritten without a decision.
    fn text_conflict_banner(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &TextSnapshot,
        conflict: ExternalConflict,
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
                        "This file changed outside API Forge while it had unsaved changes here. \
                         Saving is paused."
                    }
                    ExternalConflict::Deleted => {
                        "This file was deleted outside API Forge while it had unsaved changes \
                         here. Saving is paused."
                    }
                });
                ui.horizontal(|ui| {
                    match conflict {
                        ExternalConflict::Changed => {
                            accept = ui.button("Use the version on disk").clicked();
                        }
                        ExternalConflict::Deleted => {
                            keep = ui
                                .add(widgets::primary_button("Keep my version"))
                                .on_hover_text("Saving recreates the file")
                                .clicked();
                            accept = ui.button("Accept the deletion").clicked();
                        }
                    }
                    export = ui.button("Export a copy…").clicked();
                });
            });
        if export {
            self.export_text(file_name(&snapshot.path).to_owned(), snapshot.text.clone());
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let result = if accept {
            session.accept_external_text(&snapshot.path, DIAGRAM)
        } else if keep {
            session.keep_deleted_text(&snapshot.path)
        } else {
            return;
        };
        match result {
            Ok(()) => {
                self.shown = None;
                code::clear_history(ui.ctx(), editor_id(&text_tab(&snapshot.path)));
                // An accepted deletion leaves nothing to edit.
                if accept && conflict == ExternalConflict::Deleted {
                    self.close_tab(&text_tab(&snapshot.path));
                }
                self.touched();
            }
            Err(error) => self.notify(error.to_string(), true),
        }
    }
}
