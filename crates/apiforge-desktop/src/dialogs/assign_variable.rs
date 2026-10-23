//! Assigning a response field to a variable block.
//!
//! The dialog only collects the variable name and target; the application runs
//! `edit::assign_response_variable` and, when the edit is rejected, shows the
//! reason with [`AssignVariableDialog::set_error`] and keeps the dialog open.

use super::{Chrome, Outcome, autofocus, submitted};
use crate::widgets;
use apiforge_core::{format::Graph, js};
use eframe::egui::{self, Ui};

const DEFAULT_NAME: &str = "Response value";

/// The variable to bind: its name, and an existing variable block to reuse or
/// `None` to create a new block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub name: String,
    pub target_id: Option<String>,
}

/// The last plain field of a path (`data.items[0].id` → `id`), as a name proposal.
pub fn suggested_name(path: &str) -> &str {
    let last = path.rsplit('.').next().unwrap_or_default();
    if last.is_empty() || last.contains(['[', ']']) {
        DEFAULT_NAME
    } else {
        last
    }
}

pub struct AssignVariableDialog {
    /// The response source: block, output port and field path.
    pub source_id: String,
    pub port: String,
    pub path: String,
    source_name: String,
    variables: Vec<(String, String)>,
    target: Option<String>,
    name: String,
    error: Option<String>,
    focused: bool,
}

impl AssignVariableDialog {
    /// Opens the dialog for a field of `source_id`'s `port` on `graph`.
    pub fn new(graph: &Graph, source_id: &str, port: &str, path: &str) -> Self {
        let source_name = graph
            .nodes
            .iter()
            .find(|node| node.id == source_id)
            .map_or_else(
                || source_id.to_owned(),
                |node| node.display_name().to_owned(),
            );
        Self {
            source_id: source_id.to_owned(),
            port: port.to_owned(),
            path: path.to_owned(),
            source_name,
            variables: graph
                .nodes
                .iter()
                .filter(|node| node.is_variable())
                .map(|node| (node.id.clone(), node.display_name().to_owned()))
                .collect(),
            target: None,
            name: suggested_name(path).to_owned(),
            error: None,
            focused: false,
        }
    }

    /// Shows why the application rejected the assignment.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    fn ready(&self) -> bool {
        !js::trim(&self.name).is_empty()
    }

    fn result(&self) -> Option<Assignment> {
        self.ready().then(|| Assignment {
            name: js::trim(&self.name).to_owned(),
            target_id: self.target.clone(),
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome<Assignment> {
        let field = if self.path.is_empty() {
            format!("{} → {}", self.source_name, self.port)
        } else {
            format!("{} → {}.{}", self.source_name, self.port, self.path)
        };
        let chrome = Chrome {
            id: "assign-variable-dialog",
            eyebrow: &field,
            title: "Assign the field to a variable",
            width: 440.0,
            apply_label: "Assign variable",
            note: None,
        };
        let mut submit = false;
        let action = chrome.show(
            ctx,
            self,
            |ui, dialog| submit = dialog.body(ui),
            AssignVariableDialog::ready,
        );
        match action {
            super::Action::Cancel => Outcome::Cancelled,
            super::Action::Apply => self.result().map_or(Outcome::Open, Outcome::Applied),
            super::Action::None if submit => self.result().map_or(Outcome::Open, Outcome::Applied),
            super::Action::None => Outcome::Open,
        }
    }

    fn body(&mut self, ui: &mut Ui) -> bool {
        widgets::field_label(ui, "Variable");
        let selected = self.target.as_ref().map_or("Create a new variable", |id| {
            self.variables
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map_or("Create a new variable", |(_, name)| name.as_str())
        });
        let mut target = self.target.clone();
        egui::ComboBox::from_id_salt("assign-variable-target")
            .width(ui.available_width() - 8.0)
            .selected_text(widgets::ellipsis(selected, 48))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut target, None, "Create a new variable");
                for (id, name) in &self.variables {
                    ui.selectable_value(&mut target, Some(id.clone()), name);
                }
            });
        if target != self.target {
            self.name = target
                .as_ref()
                .and_then(|id| self.variables.iter().find(|(candidate, _)| candidate == id))
                .map_or_else(
                    || suggested_name(&self.path).to_owned(),
                    |(_, name)| name.clone(),
                );
            self.target = target;
            self.error = None;
        }
        let error = (!self.ready()).then_some("Enter a variable name.");
        let field = super::labelled_field(ui, "Variable name", &mut self.name, "", error);
        if field.changed() {
            self.error = None;
        }
        autofocus(&field, &mut self.focused);
        widgets::hint(
            ui,
            "The variable reads this field from the response on every run and keeps its \
             type. Then drag its block into a comparison or another argument.",
        );
        if let Some(error) = &self.error {
            ui.add_space(4.0);
            widgets::error_box(ui, std::slice::from_ref(error));
        }
        submitted(ui, &field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::test_support::{press, run_frames};
    use apiforge_core::demos;

    #[test]
    fn names_are_proposed_from_the_last_plain_field() {
        assert_eq!(suggested_name("data.items[0].id"), "id");
        assert_eq!(suggested_name("total"), "total");
        assert_eq!(suggested_name("items[0]"), DEFAULT_NAME);
        assert_eq!(suggested_name(""), DEFAULT_NAME);
        assert_eq!(suggested_name("a."), DEFAULT_NAME);
    }

    #[test]
    fn choosing_a_variable_takes_its_name() {
        let project = demos::shop_project();
        let graph = &project.scenarios[0].graph;
        let source = graph
            .nodes
            .iter()
            .find(|node| node.kind != apiforge_core::model::NodeKind::Trigger)
            .unwrap();
        let mut dialog = AssignVariableDialog::new(graph, &source.id, "body", "cart.id");
        assert_eq!(dialog.name, "id");
        assert_eq!(
            dialog.result(),
            Some(Assignment {
                name: "id".into(),
                target_id: None
            })
        );
        dialog.name = "  ".into();
        assert_eq!(dialog.result(), None);
        run_frames(2, |ctx| assert_eq!(dialog.show(ctx), Outcome::Open));
    }

    #[test]
    fn ctrl_enter_applies_and_escape_cancels() {
        let project = demos::shop_project();
        let graph = &project.scenarios[0].graph;
        let id = graph.nodes[0].id.clone();
        let ctx = egui::Context::default();
        let mut dialog = AssignVariableDialog::new(graph, &id, "body", "");
        let mut outcome = Outcome::Open;
        press(&ctx, egui::Key::Enter, egui::Modifiers::COMMAND, |ctx| {
            outcome = dialog.show(ctx);
        });
        assert!(
            matches!(outcome, Outcome::Applied(Assignment { ref name, .. }) if name == DEFAULT_NAME)
        );
        press(&ctx, egui::Key::Escape, egui::Modifiers::NONE, |ctx| {
            outcome = dialog.show(ctx);
        });
        assert_eq!(outcome, Outcome::Cancelled);
    }
}
