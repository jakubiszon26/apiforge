//! Native decisions over exact source/text candidates; exports keep raw bytes.
use super::*;
use apiforge_project::{ConflictDecision, ConflictPreview, ConflictTarget};

pub(super) struct ConflictView {
    target: ConflictTarget,
    preview: Option<ConflictPreview>,
    error: Option<String>,
}

impl ConflictView {
    fn refresh(&mut self, session: &ProjectSession) {
        match session.preview_conflict(self.target.clone()) {
            Ok(preview) => {
                self.preview = Some(preview);
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.to_string());
            }
        }
    }
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
        let Some(session) = &self.session else { return };
        let mut view = ConflictView {
            target,
            preview: None,
            error: None,
        };
        view.refresh(session);
        self.prompt = Some(Prompt::Conflict(Box::new(view)));
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
        }.show(context, &mut view, |ui, view| {
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
            if let Some(error) = &view.error { widgets::field_error(ui, error); }
            if let Some(status) = &self.export_status { widgets::hint(ui, status); }
            ui.add_space(theme::SPACE_SM);
            ui.horizontal_wrapped(|ui| {
                refresh = ui.button("Refresh preview").clicked();
                if deleted && ui.add_enabled(view.error.is_none(), egui::Button::new("Keep my version")).on_hover_text("Recreate this file on the next explicit save").clicked() {
                    decision = Some(ConflictDecision::KeepDeleted);
                }
            });
        }, |view| view.preview.is_some() && view.error.is_none());
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
        if refresh && let Some(session) = &self.session {
            view.refresh(session);
        }
        if let Some(decision) = decision
            && let (Some(session), Some(preview)) = (&mut self.session, &view.preview)
        {
            match session.resolve_conflict(preview, decision, DIAGRAM) {
                Ok(()) => {
                    let tab = match preview.target() {
                        ConflictTarget::Document(id) => id.clone(),
                        ConflictTarget::Text(path) => text_tab(path),
                    };
                    if decision == ConflictDecision::UseDisk
                        && preview.is_deleted()
                        && let ConflictTarget::Text(path) = preview.target()
                        && session.close_text_file(path).is_ok()
                    {
                        self.workbench.close(&tab);
                    }
                    code::clear_history(context, editor_id(&tab));
                    self.shown = None;
                    self.reload_field_drafts();
                    self.touched();
                    self.watcher.rescan();
                    return None;
                }
                Err(error) => view.error = Some(error.to_string()),
            }
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
