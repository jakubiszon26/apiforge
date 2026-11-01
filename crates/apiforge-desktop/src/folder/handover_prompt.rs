//! The hand-over prompt when the window has unsaved work (MCP-08c, MCP-00
//! decision 13 A, mock-up M2).
//!
//! An agent that asks for the project while the person has unsaved work is
//! not refused at once: the window asks the person. *Save all and hand over*
//! saves everything as *Save all* does and then hands over as a clean window
//! does (MCP-08b); *Not now*, or Escape, keeps the person editing and the
//! agent hears why. Nothing is discarded, and nothing is declared saved that
//! is not: work a save cannot settle — unsaved work from an earlier session
//! waiting to be kept or discarded, unfinished inspector fields — is listed
//! and keeps the project here.
use super::*;

/// The agent's reason when the person keeps editing.
pub(super) const KEEP_EDITING: &str = "the person chose to keep editing";
/// The agent's reason when *Save all* failed: no path, no error text.
pub(super) const SAVE_FAILED: &str = "the window could not save the person's work";
const HEADING: &str = "An agent asks to edit this project";
const SAVE_ALL: &str = "Save all and hand over";
const NOT_NOW: &str = "Not now";
const EARLIER: &str = "unsaved work from an earlier session, to keep or discard";
/// The heading when a command asks (MCP-15 decision 2 A, mock-up).
const COMMAND_HEADING: &str = "A command asks for this project";

#[cfg(test)]
thread_local! {
    /// The asker is a command, not an agent (MCP-15 mock-up: the role the
    /// implementation carries in `Prompt::Handover`).
    pub(super) static MOCK_COMMAND_ASKS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// One line of the prompt: what is unsaved, and in what way.
#[derive(Clone, Debug, PartialEq, Eq)]
struct UnsavedItem {
    name: String,
    what: String,
}

impl UnsavedItem {
    fn new(name: impl Into<String>, what: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            what: what.into(),
        }
    }

    fn label(&self) -> String {
        format!("{} — {}", self.name, self.what)
    }
}

/// *a*, *a and b*, *a, b and c*.
fn listed(parts: &[&str]) -> String {
    match parts {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

impl FolderApp {
    /// While the configuration is saved, remembers it, so the prompt can say
    /// which setting is unsaved.
    pub(super) fn note_saved_settings(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if session.is_manifest_dirty() {
            return;
        }
        let manifest = session.manifest();
        let current = self.saved_settings.as_ref().is_some_and(|saved| {
            saved.id == manifest.id
                && saved.name == manifest.name
                && saved.environment == manifest.environment
                && saved.contracts == manifest.contracts
        });
        if !current {
            self.saved_settings = Some(Box::new(manifest.clone()));
        }
    }

    /// The unsaved work of the session, in the order of the tabs first:
    /// documents, text files, work from an earlier session, settings.
    fn unsaved_items(&self) -> Vec<UnsavedItem> {
        let Some(session) = self.session.as_ref() else {
            return Vec::new();
        };
        let tabs = self.workbench.tabs();
        let mut documents: Vec<_> = session.documents().iter().collect();
        documents.sort_by_key(|document| {
            tabs.iter()
                .position(|tab| tab.document_id == document.id())
                .unwrap_or(usize::MAX)
        });
        let mut items = Vec::new();
        for document in documents {
            let mut what = Vec::new();
            if document.is_dirty() {
                what.push("code");
            }
            if document.is_layout_dirty() {
                what.push("layout");
            }
            if !document.field_drafts().is_empty() {
                what.push("fields");
            }
            if !what.is_empty() {
                items.push(UnsavedItem::new(
                    document.entry().path.as_str(),
                    format!("unsaved {}", listed(&what)),
                ));
            }
        }
        for file in session.text_files() {
            if file.is_held() {
                items.push(UnsavedItem::new(file.path(), EARLIER));
            } else if file.is_dirty() {
                items.push(UnsavedItem::new(file.path(), "unsaved text"));
            }
        }
        for path in session.recovered_text_drafts() {
            items.push(UnsavedItem::new(path, EARLIER));
        }
        for (_, draft) in session.recovered_drafts() {
            items.push(UnsavedItem::new(draft.path.as_str(), EARLIER));
        }
        if session.is_manifest_dirty() {
            let manifest = session.manifest();
            let mut what = Vec::new();
            if let Some(saved) = self
                .saved_settings
                .as_ref()
                .filter(|saved| saved.id == manifest.id)
            {
                if saved.name != manifest.name {
                    what.push("name");
                }
                if saved.environment != manifest.environment {
                    what.push("environment");
                }
                if saved.contracts != manifest.contracts {
                    what.push("contracts");
                }
            }
            let what = if what.is_empty() {
                "unsaved changes".to_owned()
            } else {
                listed(&what)
            };
            items.push(UnsavedItem::new("Project settings", what));
        }
        items
    }

    /// An agent asks while there is unsaved work: the person is asked, now
    /// or once the prompt open now is closed.
    pub(super) fn ask_person(&mut self, client: Option<String>) {
        match &mut self.prompt {
            Some(Prompt::Handover { client: shown, .. }) => *shown = client,
            Some(_) => {}
            None => self.prompt = Some(Prompt::Handover { client }),
        }
    }

    pub(super) fn handover_prompt(
        &mut self,
        context: &egui::Context,
        client: Option<String>,
    ) -> Option<Prompt> {
        // The list follows the session: work saved meanwhile leaves it, and
        // once nothing is unsaved the window hands over as a clean one does.
        let writable = self
            .session
            .as_ref()
            .is_some_and(|session| !session.is_read_only());
        let unsaved = self.unsaved_items();
        if !writable || unsaved.is_empty() {
            return None;
        }
        #[cfg(test)]
        let command = MOCK_COMMAND_ASKS.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let command = false;
        let (heading, sentence) = if command {
            let name = match client.as_deref() {
                Some(client) => format!("apiforge {client}"),
                None => "The apiforge command".to_owned(),
            };
            (
                COMMAND_HEADING,
                format!("{name} wants to send live requests from this project. Unsaved work here:"),
            )
        } else {
            let who = holder::agent_name(client.as_deref());
            (
                HEADING,
                format!("{who} wants to edit this project. Unsaved work here:"),
            )
        };
        let (mut save, mut not_now) = (false, false);
        let list_height = context.content_rect().height() * 0.4;
        let response = dialogs::centered_modal(egui::Id::new("folder-handover"), context).show(
            context,
            |ui| {
                ui.set_width(460.0);
                ui.heading(heading);
                ui.add_space(theme::SPACE_SM);
                ui.label(sentence);
                ui.add_space(theme::SPACE_SM);
                egui::ScrollArea::vertical()
                    .max_height(list_height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for item in &unsaved {
                            ui.horizontal_top(|ui| {
                                ui.add_space(theme::SPACE_LG);
                                ui.label("•");
                                ui.add(egui::Label::new(item.label()).wrap());
                            });
                        }
                    });
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        not_now = ui.button(NOT_NOW).clicked();
                        save = ui.add(widgets::primary_button(SAVE_ALL)).clicked();
                    });
                });
            },
        );
        if save {
            self.save_all_and_hand_over();
            return None;
        }
        if not_now || response.should_close() {
            self.answer_waiting(KEEP_EDITING);
            return None;
        }
        Some(Prompt::Handover { client })
    }

    /// *Save all and hand over*: saves as *Save all* does; when everything
    /// is saved, the waiting request is answered as a clean window answers
    /// it. A failed save, or work a save cannot settle, keeps the project
    /// here and the agent hears why.
    fn save_all_and_hand_over(&mut self) {
        self.handover.saving = true;
        self.save_all_then(|app, saved| {
            app.handover.saving = false;
            if !saved {
                app.answer_waiting(SAVE_FAILED);
                return;
            }
            if app
                .session
                .as_ref()
                .is_some_and(|session| !session.is_read_only() && session.has_unsaved_work())
            {
                app.notify(
                    "Saved. The project stays here: unsaved work from an earlier session \
                     or unfinished fields still wait for you.",
                    true,
                );
                app.answer_waiting(handover::UNSAVED_WORK);
            }
        });
    }
}
