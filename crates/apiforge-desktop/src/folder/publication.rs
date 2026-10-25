//! Publication and version-bound decisions before destructive source printing.
use super::*;

#[derive(Clone)]
pub(super) enum Action {
    Publish,
    Model {
        next: Box<Document>,
        selection: Option<Vec<String>>,
    },
    History {
        undo: bool,
    },
}

impl FolderApp {
    pub(super) fn component_action(&mut self, id: &str, action: Action) {
        if self.watcher.recovery_required {
            self.notify(
                "Resolve project recovery and reload before changing the diagram.",
                true,
            );
            return;
        }
        if self.persistence.closing || self.storage.blocks_edits() {
            return;
        }
        if matches!(action, Action::Publish) {
            let document_id = id.to_owned();
            let target = document_id.clone();
            self.storage_project(
                "Publishing component…",
                apiforge_project::StorageMode::Exclusive,
                true,
                move |session| session.publish(&target, DIAGRAM),
                move |app, result| {
                    app.finish_component_action(
                        &document_id,
                        Action::Publish,
                        result.map(|published| {
                            Some(format!("Published revision {}.", published.revision))
                        }),
                    )
                },
            );
            return;
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let result = match &action {
            Action::Publish => unreachable!("publication is queued before borrowing the session"),
            Action::Model { next, .. } => session
                .apply_model_edit(id, *next.clone(), DIAGRAM)
                .map(|()| None),
            Action::History { undo } => if *undo {
                session.undo(id, DIAGRAM)
            } else {
                session.redo(id, DIAGRAM)
            }
            .map(|_| None),
        };
        self.finish_component_action(id, action, result);
    }

    fn finish_component_action(
        &mut self,
        id: &str,
        action: Action,
        result: apiforge_project::Result<Option<String>>,
    ) {
        match result {
            Ok(message) => {
                if let Action::Model {
                    selection: Some(selection),
                    ..
                } = action
                {
                    self.selections.insert(id.to_owned(), selection);
                }
                if let Some(message) = message {
                    self.notify(message, false);
                }
                self.touched();
            }
            Err(apiforge_project::Error::CommentsWouldBeLost { .. }) => {
                if let Some(session) = &self.session
                    && let Ok(input) = session.analysis_for(id)
                {
                    self.prompt = Some(Prompt::CommentLoss {
                        ticket: input.ticket,
                        action: Box::new(action),
                    });
                }
            }
            Err(error) => self.notify(error.to_string(), true),
        }
    }

    pub(super) fn comment_loss_prompt(
        &mut self,
        context: &egui::Context,
        ticket: apiforge_project::AnalysisTicket,
        action: Box<Action>,
    ) -> Option<Prompt> {
        let mut accept = false;
        let mut cancel = false;
        let response = dialogs::centered_modal(egui::Id::new("folder-comment-loss"), context).show(context, |ui| {
            ui.set_width(420.0);
            ui.heading("Continue without unattached comments?");
            ui.label("Printing this definition removes comments that belong to no statement. The original text is kept until you continue. Cancel to attach them to statements or copy them first.");
            ui.add_space(theme::SPACE_MD);
            ui.horizontal(|ui| {
                accept = ui.add(widgets::primary_button("Continue")).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if cancel || response.should_close() {
            return None;
        }
        if accept {
            self.confirm_comment_loss(&ticket, *action);
            return None;
        }
        Some(Prompt::CommentLoss { ticket, action })
    }

    pub(super) fn confirm_comment_loss(
        &mut self,
        ticket: &apiforge_project::AnalysisTicket,
        action: Action,
    ) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if !session
            .analysis_for(&ticket.document_id)
            .is_ok_and(|input| input.ticket == *ticket)
        {
            self.notify("The document or its dependencies changed. Try the action again to review the current source.", false);
            return;
        }
        match session.accept_comment_loss(&ticket.document_id) {
            Ok(()) => self.component_action(&ticket.document_id, action),
            Err(error) => self.notify(error.to_string(), true),
        }
    }

    pub(super) fn apply_source_fix(&mut self, id: &str, diagnostic: usize) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let before = session
            .document(id)
            .map(|document| document.text().to_owned())
            .unwrap_or_default();
        match session.apply_fix(id, diagnostic, CODE) {
            Ok(_) => {
                // A programmatic edit joins the same undo history as typing.
                let mut state =
                    egui::text_edit::TextEditState::load(&self.ui_context, editor_id(id))
                        .unwrap_or_default();
                let range = state.cursor.char_range().unwrap_or_default();
                let mut undoer = state.undoer();
                undoer.add_undo(&(range, before));
                state.set_undoer(undoer);
                state.store(&self.ui_context, editor_id(id));
                self.analyser.typed(id, Instant::now());
                self.touched();
                self.ui_context
                    .memory_mut(|memory| memory.request_focus(editor_id(id)));
            }
            Err(error) => self.notify(error.to_string(), true),
        }
    }
}
