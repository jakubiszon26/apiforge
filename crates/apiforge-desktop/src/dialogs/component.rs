//! Saving selected blocks, or the component being edited, as a component revision.
//!
//! The builder proposes public arguments and outputs with
//! `edit::initialize_inputs`/`initialize_outputs`, lets the user adjust the
//! [`ComponentDraft`] and returns the materialised definition. The application adds
//! it with `edit::add_component`, which assigns the next revision when revising;
//! a rejection is shown with [`ComponentDialog::set_error`].

use super::{Chrome, Outcome, autofocus, contract_picker, type_picker};
use crate::{inspector::sample_output, theme, widgets};
use apiforge_core::{
    edit::{self, ComponentDraft, OutputRow},
    format::{Binding, Component, Graph, Project},
    graph::output_ports,
    js,
    model::ValueType,
};
use eframe::egui::{self, RichText, Ui};
use std::collections::HashSet;

/// A materialised definition and, when revising, the component it revises.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedComponent {
    pub component: Component,
    pub revision_of: Option<String>,
}

pub struct ComponentDialog {
    graph: Graph,
    selected: Vec<String>,
    initial: Option<Component>,
    draft: ComponentDraft,
    /// Whether ordering connections cross the selection boundary.
    outside_orders: bool,
    extra_outputs: usize,
    save_error: Option<String>,
    focused: bool,
}

impl ComponentDialog {
    /// Opens the builder for `selected` blocks of `graph`; `initial` is the
    /// component being revised, whose whole inner graph is then selected.
    pub fn new(
        graph: &Graph,
        selected: &[String],
        project: &Project,
        initial: Option<&Component>,
    ) -> Self {
        let ids: HashSet<&str> = selected.iter().map(String::as_str).collect();
        let inputs = edit::initialize_inputs(graph, &ids, project, initial);
        let outputs = edit::initialize_outputs(graph, &ids, project, initial);
        let action = initial
            .and_then(|initial| initial.action.as_ref())
            .and_then(|action| inputs.iter().find(|row| row.name == action.input))
            .map(|row| row.key.clone());
        let nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter(|node| ids.contains(node.id.as_str()))
            .collect();
        let name = initial.map_or_else(
            || match nodes.as_slice() {
                [node] => node.name.clone(),
                _ => "New component".to_owned(),
            },
            |initial| initial.name.clone(),
        );
        let outside_orders = graph
            .edges
            .iter()
            .any(|edge| ids.contains(edge.source.as_str()) != ids.contains(edge.target.as_str()));
        Self {
            draft: ComponentDraft {
                name,
                description: initial
                    .map(|initial| initial.description.clone())
                    .unwrap_or_default(),
                inputs,
                outputs,
                action,
                action_type: initial
                    .and_then(|initial| initial.action.as_ref())
                    .map_or(ValueType::Any, |action| action.value_type),
                action_contract: initial
                    .and_then(|initial| initial.action.as_ref())
                    .and_then(|action| action.contract_id.clone()),
            },
            graph: graph.clone(),
            selected: graph
                .nodes
                .iter()
                .filter(|node| ids.contains(node.id.as_str()))
                .map(|node| node.id.clone())
                .collect(),
            initial: initial.cloned(),
            outside_orders,
            extra_outputs: 0,
            save_error: None,
            focused: false,
        }
    }

    /// Shows why the application rejected the definition.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.save_error = Some(message.into());
    }

    fn ids(&self) -> HashSet<&str> {
        self.selected.iter().map(String::as_str).collect()
    }

    fn errors(&self) -> Vec<String> {
        edit::validate_draft(&self.graph, &self.ids(), &self.draft)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn result(&self) -> Result<SavedComponent, String> {
        edit::materialize_component(&self.graph, &self.ids(), &self.draft, self.initial.as_ref())
            .map(|component| SavedComponent {
                component,
                revision_of: self.initial.as_ref().map(|initial| initial.id.clone()),
            })
            .map_err(|error| error.to_string())
    }

    pub fn show(&mut self, ctx: &egui::Context, project: &Project) -> Outcome<SavedComponent> {
        let eyebrow = format!("PROJECT LIBRARY · {} STEPS", self.selected.len());
        let (title, note) = if self.initial.is_some() {
            (
                "Save a new revision",
                "A new revision does not change existing instances.",
            )
        } else {
            (
                "Save as a component",
                "Creates revision 1. The original blocks stay in the scenario.",
            )
        };
        let chrome = Chrome {
            id: "component-dialog",
            eyebrow: &eyebrow,
            title,
            width: 680.0,
            apply_label: "Save component",
            note: Some(note),
        };
        let action = chrome.show(
            ctx,
            self,
            |ui, dialog| dialog.body(ui, project),
            |dialog| dialog.errors().is_empty(),
        );
        match action {
            super::Action::Cancel => Outcome::Cancelled,
            super::Action::Apply => match self.result() {
                Ok(saved) => Outcome::Applied(saved),
                Err(error) => {
                    self.save_error = Some(error);
                    Outcome::Open
                }
            },
            super::Action::None => Outcome::Open,
        }
    }

    fn node_name(&self, id: &str) -> String {
        self.graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map_or_else(|| "Removed step".to_owned(), |node| node.name.clone())
    }

    fn body(&mut self, ui: &mut Ui, project: &Project) {
        ui.horizontal_top(|ui| {
            let width = (ui.available_width() - 12.0) / 2.0;
            ui.vertical(|ui| {
                ui.set_width(width);
                widgets::field_label(ui, "Name");
                let invalid = js::trim(&self.draft.name).is_empty();
                let field = super::text_field(
                    ui,
                    &mut self.draft.name,
                    "e.g. Fetch a record",
                    width,
                    invalid,
                );
                autofocus(&field, &mut self.focused);
            });
            ui.vertical(|ui| {
                ui.set_width(width);
                widgets::field_label(ui, "Description");
                super::text_field(
                    ui,
                    &mut self.draft.description,
                    "What does this component do?",
                    width,
                    false,
                );
            });
        });

        widgets::section(ui, "1. Public arguments");
        widgets::hint(
            ui,
            "Choose the values a tester can change on every use. Data from outside the \
             selection must become arguments; the remaining values stay inside the component.",
        );
        if self.draft.inputs.is_empty() {
            widgets::hint(ui, "This fragment needs no public arguments.");
        }
        let contracts = &project.contracts;
        for row in &mut self.draft.inputs {
            ui.push_id(("input", row.key.clone()), |ui| {
                row_frame(ui, row.enabled, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!row.mandatory, |ui| {
                            ui.checkbox(&mut row.enabled, &row.label)
                                .on_disabled_hover_text(
                                    "Fed from outside the selection, so it must be public",
                                );
                        });
                        if row.mandatory {
                            widgets::pill(ui, "GRAPH BOUNDARY", theme::WARNING);
                        }
                    });
                    if !row.enabled {
                        return;
                    }
                    ui.horizontal(|ui| {
                        super::text_field(ui, &mut row.name, "argument name", 200.0, false)
                            .on_hover_text("Argument name");
                        type_picker(ui, "type", &mut row.value_type, 110.0);
                        contract_picker(
                            ui,
                            "contract",
                            &mut row.contract_id,
                            contracts,
                            "No contract",
                            150.0,
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut row.exposed, "Editable on the block");
                        ui.checkbox(&mut row.required, "Required");
                    });
                    if let Some(value) = &row.default_value {
                        widgets::hint(ui, format!("Default value: {}", js::json_stringify(value)));
                    }
                });
            });
        }

        widgets::section(ui, "2. Data returned to the scenario");
        widgets::hint(
            ui,
            "Return the whole response or a chosen field. Outputs can be connected to the \
             next steps.",
        );
        let names: Vec<(String, String)> = self
            .selected
            .iter()
            .map(|id| (id.clone(), self.node_name(id)))
            .collect();
        let nodes: Vec<_> = self
            .graph
            .nodes
            .iter()
            .filter(|node| self.selected.contains(&node.id))
            .collect();
        for row in &mut self.draft.outputs {
            let source = names
                .iter()
                .find(|(id, _)| *id == row.node_id)
                .map_or("Saved binding", |(_, name)| name.as_str());
            ui.push_id(("output", row.key.clone()), |ui| {
                row_frame(ui, row.enabled, |ui| {
                    ui.checkbox(&mut row.enabled, format!("{source} → {}", row.port));
                    if !row.enabled {
                        return;
                    }
                    widgets::field_label(ui, "Output source");
                    output_source(ui, row, &nodes, project);
                    ui.horizontal(|ui| {
                        super::text_field(ui, &mut row.name, "output name", 200.0, false)
                            .on_hover_text("Output name");
                        let width = ui.available_width() - 8.0;
                        super::text_field(
                            ui,
                            &mut row.path,
                            "path (optional), e.g. data.id",
                            width,
                            false,
                        )
                        .on_hover_text("Field path inside the output");
                    });
                    if let Some(node) = nodes.iter().find(|node| node.id == row.node_id) {
                        path_picker(ui, row, sample_output(node, &row.port, project).as_ref());
                    }
                    ui.horizontal(|ui| {
                        type_picker(ui, "type", &mut row.value_type, 110.0);
                        contract_picker(
                            ui,
                            "contract",
                            &mut row.contract_id,
                            contracts,
                            "No contract",
                            150.0,
                        );
                    });
                });
            });
        }
        let last_port = nodes.last().and_then(|node| {
            output_ports(node, project)
                .into_iter()
                .next()
                .map(|port| (node.id.clone(), port))
        });
        if ui
            .add_enabled(
                last_port.is_some(),
                egui::Button::new("+ Additional output"),
            )
            .clicked()
            && let Some((node_id, port)) = last_port
        {
            self.extra_outputs += 1;
            self.draft.outputs.push(OutputRow {
                key: format!("extra-output-{}", self.extra_outputs),
                enabled: true,
                name: format!("output_{}", self.draft.outputs.len() + 1),
                value_type: port.value_type,
                contract_id: port.contract_id.clone(),
                node_id,
                port: port.name,
                path: String::new(),
                original_binding: None,
                label: None,
            });
        }

        widgets::section(ui, "3. Availability in “Add action”");
        widgets::field_label(ui, "Argument fed automatically by the previous block");
        self.action_picker(ui, contracts);

        if self.outside_orders {
            widgets::hint(
                ui,
                "Ordering connections that leave the selection stay in the scenario. The \
                 component keeps only its internal order.",
            );
        }
        ui.add_space(6.0);
        if let Some(error) = &self.save_error {
            widgets::error_box(ui, std::slice::from_ref(error));
            ui.add_space(4.0);
        }
        widgets::error_box(ui, &self.errors());
    }

    fn action_picker(&mut self, ui: &mut Ui, contracts: &[apiforge_core::format::Contract]) {
        let enabled: Vec<(String, String, ValueType, Option<String>)> = self
            .draft
            .inputs
            .iter()
            .filter(|row| row.enabled)
            .map(|row| {
                (
                    row.key.clone(),
                    row.name.clone(),
                    row.value_type,
                    row.contract_id.clone(),
                )
            })
            .collect();
        const NONE: &str = "Library only — no contextual action";
        let selected = self
            .draft
            .action
            .as_ref()
            .and_then(|key| enabled.iter().find(|(candidate, ..)| candidate == key))
            .map_or(NONE, |(_, name, ..)| name.as_str());
        let mut action = self.draft.action.clone();
        egui::ComboBox::from_id_salt("component-action")
            .width(320.0)
            .selected_text(widgets::ellipsis(selected, 40))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut action, None, NONE);
                for (key, name, ..) in &enabled {
                    ui.selectable_value(&mut action, Some(key.clone()), name);
                }
            });
        if action != self.draft.action {
            let row = action
                .as_ref()
                .and_then(|key| enabled.iter().find(|(candidate, ..)| candidate == key));
            self.draft.action_type =
                row.map_or(ValueType::Any, |(_, _, value_type, _)| *value_type);
            self.draft.action_contract = row.and_then(|(.., contract)| contract.clone());
            self.draft.action = action;
        }
        if self.draft.action.is_some() {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    widgets::field_label(ui, "Show for data of type");
                    type_picker(ui, "action-type", &mut self.draft.action_type, 140.0);
                });
                ui.vertical(|ui| {
                    widgets::field_label(ui, "Or of the project contract");
                    contract_picker(
                        ui,
                        "action-contract",
                        &mut self.draft.action_contract,
                        contracts,
                        "Match by type",
                        180.0,
                    );
                });
            });
        }
    }
}

/// A bordered row, tinted when its checkbox is on.
fn row_frame(ui: &mut Ui, enabled: bool, add_contents: impl FnOnce(&mut Ui)) {
    let fill = if enabled { theme::FIELD } else { theme::WHITE };
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

/// Chooses the block port an output reads, or keeps a saved non-reference binding.
fn output_source(
    ui: &mut Ui,
    row: &mut OutputRow,
    nodes: &[&apiforge_core::format::Node],
    project: &Project,
) {
    let saved = row
        .original_binding
        .as_ref()
        .filter(|binding| !matches!(binding, Binding::Reference { .. }))
        .map(|binding| format!("Saved binding ({})", binding_kind(binding)));
    let options: Vec<(String, String, String)> = nodes
        .iter()
        .flat_map(|node| {
            output_ports(node, project).into_iter().map(|port| {
                let label = format!(
                    "{} → {}",
                    node.name,
                    port.label.as_deref().unwrap_or(&port.name)
                );
                (node.id.clone(), port.name, label)
            })
        })
        .collect();
    let current = match &saved {
        Some(saved) if row.node_id.is_empty() => saved.clone(),
        _ => options
            .iter()
            .find(|(node, port, _)| *node == row.node_id && *port == row.port)
            .map_or_else(
                || "Removed step — choose a new source".to_owned(),
                |(.., label)| label.clone(),
            ),
    };
    let mut chosen = None;
    egui::ComboBox::from_id_salt("source")
        .width(ui.available_width() - 8.0)
        .selected_text(widgets::ellipsis(&current, 60))
        .show_ui(ui, |ui| {
            if let Some(saved) = &saved {
                let _ = ui.selectable_label(row.node_id.is_empty(), saved);
            }
            for (node, port, label) in &options {
                let active = *node == row.node_id && *port == row.port;
                if ui.selectable_label(active, label).clicked() && !active {
                    chosen = Some((node.clone(), port.clone()));
                }
            }
        });
    if let Some((node, port)) = chosen {
        row.node_id = node;
        row.port = port;
        row.original_binding = None;
    }
}

fn binding_kind(binding: &Binding) -> &'static str {
    match binding {
        Binding::Expression { .. } => "expression",
        Binding::Literal { .. } => "literal",
        Binding::Reference { .. } => "reference",
        Binding::Parameter { .. } => "parameter",
        Binding::Environment { .. } => "environment",
    }
}

/// Picks a field path from the sample of the output's source, when there is one.
fn path_picker(ui: &mut Ui, row: &mut OutputRow, sample: Option<&serde_json::Value>) {
    let paths = edit::sample_paths(sample);
    if paths.is_empty() {
        return;
    }
    let current = if row.path.is_empty() {
        "Whole value".to_owned()
    } else if paths.contains(&row.path) {
        row.path.clone()
    } else {
        "Custom path".to_owned()
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Field from the sample")
                .size(11.0)
                .color(theme::MUTED),
        );
        egui::ComboBox::from_id_salt("sample-path")
            .width(220.0)
            .selected_text(widgets::ellipsis(&current, 32))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut row.path, String::new(), "Whole value");
                for path in &paths {
                    ui.selectable_value(&mut row.path, path.clone(), path);
                }
            });
    });
    widgets::hint(
        ui,
        "The structure comes from a sample — a hint, not a guarantee.",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::test_support::{press, run_frames};
    use apiforge_core::{demos, model::NodeKind};

    fn shop() -> (Project, Graph, Vec<String>) {
        let project = demos::shop_project();
        let graph = project.scenarios[0].graph.clone();
        let selected = graph
            .nodes
            .iter()
            .filter(|node| node.kind != NodeKind::Trigger)
            .take(1)
            .map(|node| node.id.clone())
            .collect();
        (project, graph, selected)
    }

    #[test]
    fn a_single_block_proposes_its_name_and_saves_revision_one() {
        let (project, graph, selected) = shop();
        let mut dialog = ComponentDialog::new(&graph, &selected, &project, None);
        assert_eq!(
            dialog.draft.name,
            graph
                .nodes
                .iter()
                .find(|n| n.id == selected[0])
                .unwrap()
                .name
        );
        run_frames(2, |ctx| {
            assert_eq!(dialog.show(ctx, &project), Outcome::Open)
        });
        let saved = dialog.result().expect("the proposal is valid");
        assert_eq!(saved.revision_of, None);
        assert_eq!(saved.component.revision, 1);
        let next = edit::add_component(&project, &saved.component, None).expect("valid project");
        assert_eq!(next.components.len(), project.components.len() + 1);
    }

    #[test]
    fn validation_errors_disable_saving() {
        let (project, graph, selected) = shop();
        let mut dialog = ComponentDialog::new(&graph, &selected, &project, None);
        dialog.draft.name = " ".into();
        for row in &mut dialog.draft.outputs {
            row.enabled = false;
        }
        let errors = dialog.errors();
        assert!(errors.iter().any(|error| error.contains("component name")));
        assert!(errors.iter().any(|error| error.contains("output")));
        assert!(dialog.result().is_err());
        let ctx = egui::Context::default();
        let mut outcome = Outcome::Open;
        press(&ctx, egui::Key::Enter, egui::Modifiers::COMMAND, |ctx| {
            outcome = dialog.show(ctx, &project);
        });
        assert_eq!(outcome, Outcome::Open);
    }

    #[test]
    fn revising_a_component_keeps_its_identity_until_saved() {
        let project = demos::demo_projects()
            .into_iter()
            .find(|project| !project.components.is_empty())
            .expect("a demo with components");
        let component = project.components[0].clone();
        let selected: Vec<String> = component.graph.nodes.iter().map(|n| n.id.clone()).collect();
        let mut dialog =
            ComponentDialog::new(&component.graph, &selected, &project, Some(&component));
        assert_eq!(dialog.draft.name, component.name);
        run_frames(2, |ctx| {
            assert_eq!(dialog.show(ctx, &project), Outcome::Open)
        });
        let saved = dialog.result().expect("an unchanged revision is valid");
        assert_eq!(saved.revision_of.as_deref(), Some(component.id.as_str()));
        let next = edit::add_component(&project, &saved.component, saved.revision_of.as_deref())
            .expect("valid project");
        let latest = next.components.last().unwrap();
        assert_eq!(latest.id, component.id);
        assert!(latest.revision > component.revision);
    }

    #[test]
    fn escape_cancels() {
        let (project, graph, selected) = shop();
        let ctx = egui::Context::default();
        let mut dialog = ComponentDialog::new(&graph, &selected, &project, None);
        let mut outcome = Outcome::Open;
        press(&ctx, egui::Key::Escape, egui::Modifiers::NONE, |ctx| {
            outcome = dialog.show(ctx, &project);
        });
        assert_eq!(outcome, Outcome::Cancelled);
    }
}
