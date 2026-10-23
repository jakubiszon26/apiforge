//! The right panel: configuration and output data of the selected step.
//!
//! The inspector edits a copy of the node and returns [`Action`]s; the application
//! applies them as undoable edits.

mod binding;
mod fields;
mod response;

pub use response::sample_output;

use crate::{labels, theme, widgets};
use apiforge_core::{
    edit::{self, bind_variable_text, prune_template_inputs},
    format::{Binding, ConditionPredicate, Graph, Node, OptionalJson, Port, Project},
    graph::{
        FALSE_PORT, ORDER_PORT, TRUE_PORT, component_definition, formula_operator, input_ports,
        output_ports, predicate,
    },
    model::{Branch, BuildType, Match, NodeKind, Predicate, ValueType},
    path,
    run::{RunResult, StepResult},
};
use eframe::egui::{self, Id, RichText, Ui};
use std::collections::HashSet;

pub enum Action {
    UpdateNode(Box<Node>),
    /// A change to ordering edges of the board.
    UpdateGraph(Graph),
    EditComponent,
    AssertField {
        port: String,
        path: String,
        value: serde_json::Value,
    },
    AssignField {
        port: String,
        path: String,
    },
    Error(String),
}

pub struct Context<'a> {
    pub node: &'a Node,
    /// Nodes that bindings may reference.
    pub graph: &'a Graph,
    /// The board graph, for ordering connections.
    pub board: &'a Graph,
    pub project: &'a Project,
    pub parameters: &'a [String],
    pub run: Option<&'a RunResult>,
    pub step: Option<&'a StepResult>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Configuration,
    Outputs,
}

#[derive(Clone, Debug)]
struct ResponseChoiceState {
    source_id: String,
    port: String,
    search: String,
    focus_search: bool,
}

pub struct Inspector {
    node_id: String,
    tab: Tab,
    new_input: String,
    choosing_response: Option<ResponseChoiceState>,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            tab: Tab::Configuration,
            new_input: String::new(),
            choosing_response: None,
        }
    }
}

impl Inspector {
    pub fn show(&mut self, ui: &mut Ui, context: &Context<'_>) -> Vec<Action> {
        if self.node_id != context.node.id {
            *self = Self {
                node_id: context.node.id.clone(),
                ..Self::default()
            };
        }
        let mut actions = Vec::new();
        let mut node = context.node.clone();

        ui.label(
            RichText::new(context.node.kind.label().to_uppercase())
                .size(11.0)
                .color(theme::MUTED),
        );
        ui.label(
            RichText::new(&context.node.name)
                .size(theme::FONT_PANEL_TITLE)
                .family(theme::bold_family()),
        );
        ui.add_space(theme::SPACE_SM);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Configuration, "Configuration");
            ui.selectable_value(&mut self.tab, Tab::Outputs, "Output data");
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width((ui.available_width() - theme::SPACE_SM).max(0.0));
                match self.tab {
                    Tab::Configuration => self.configuration(ui, context, &mut node, &mut actions),
                    Tab::Outputs => outputs(ui, context, &mut actions),
                }
            });
        self.response_choice(ui.ctx(), context, &mut node);

        if node != *context.node {
            actions.push(Action::UpdateNode(Box::new(prune_template_inputs(&node))));
        }
        actions
    }

    fn configuration(
        &mut self,
        ui: &mut Ui,
        context: &Context<'_>,
        node: &mut Node,
        actions: &mut Vec<Action>,
    ) {
        let id = Id::new(("inspector", &context.node.id));
        widgets::field_label(ui, "Block name");
        ui.add(
            crate::widgets::text_edit(&mut node.name)
                .id(id.with("name"))
                .desired_width(f32::INFINITY),
        );

        match node.kind {
            NodeKind::Trigger => widgets::hint(
                ui,
                "A manual trigger. Running the test starts this path. Connect Next to the first step.",
            ),
            NodeKind::If => widgets::hint(
                ui,
                "The comparison chooses the Yes or No output. False is not a test failure. Connect both branches.",
            ),
            NodeKind::Variable => {
                ui.add_space(6.0);
                if ui
                    .button("Take the value from a response…")
                    .on_hover_text("Bind this variable to a field of a request's response")
                    .clicked()
                {
                    let (source_id, port) = initial_response_choice(context);
                    self.choosing_response = Some(ResponseChoiceState {
                        source_id,
                        port,
                        search: String::new(),
                        focus_search: true,
                    });
                }
                widgets::hint(
                    ui,
                    "Choose a request and a response field. The variable reads the current value on every run.",
                );
            }
            NodeKind::Request => request(ui, id, context, node),
            NodeKind::Select => select(ui, id, context, node),
            NodeKind::Build => {
                widgets::field_label(ui, "Create");
                let mut build = node.config.build_type.unwrap_or_default();
                egui::ComboBox::from_id_salt(id.with("build"))
                    .selected_text(match build {
                        BuildType::Object => "An object of named fields",
                        BuildType::Array => "A list of the values in order",
                    })
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut build,
                            BuildType::Object,
                            "An object of named fields",
                        );
                        ui.selectable_value(
                            &mut build,
                            BuildType::Array,
                            "A list of the values in order",
                        );
                    });
                node.config.build_type = Some(build);
            }
            NodeKind::Formula => {
                widgets::field_label(ui, "Operation");
                let current = formula_operator(node);
                let mut chosen = current;
                binding::operator_combo(ui, id.with("formula"), &mut chosen);
                if chosen != current {
                    let rebuilt = Binding::Expression {
                        operator: current,
                        inputs: node.inputs.clone(),
                    }
                    .with_operator(chosen);
                    if let Binding::Expression { inputs, .. } = rebuilt {
                        node.inputs = inputs;
                    }
                    node.config.operation = Some(chosen);
                }
            }
            NodeKind::Assert | NodeKind::Condition => check(ui, id, node),
            NodeKind::Component => {
                let definition = component_definition(context.project, node);
                ui.add_space(6.0);
                widgets::hint(
                    ui,
                    definition.map_or("This revision's definition is missing.", |definition| {
                        definition.description.as_str()
                    }),
                );
                ui.horizontal(|ui| {
                    widgets::pill(
                        ui,
                        &format!("Revision {}", node.config.revision.unwrap_or_default()),
                        theme::LINK,
                    );
                    if ui
                        .add_enabled(
                            definition.is_some(),
                            egui::Button::new("Open the component ↗"),
                        )
                        .clicked()
                    {
                        actions.push(Action::EditComponent);
                    }
                });
                if let Some(definition) = definition {
                    let newest = context
                        .project
                        .components
                        .iter()
                        .filter(|candidate| candidate.id == definition.id)
                        .map(|candidate| candidate.revision)
                        .max()
                        .unwrap_or(definition.revision);
                    if newest > definition.revision
                        && ui
                            .button(format!("Update this instance to revision {newest}"))
                            .on_hover_text("Other instances keep their revision")
                            .clicked()
                    {
                        node.config.revision = Some(newest);
                    }
                }
            }
        }
        if node.kind == NodeKind::If {
            check(ui, id, node);
        }

        self.arguments(ui, id, context, node);
        // Keep the values being configured near the top; ordering remains
        // available below them as a keyboard alternative to canvas wires.
        if node.kind != NodeKind::Trigger || !context.board.nodes.is_empty() {
            flow_connections(ui, context, actions);
        }
        if node.kind == NodeKind::Condition {
            condition_predicates(ui, id, context, node);
        }
        if node.kind == NodeKind::Request {
            samples(ui, id, context, node);
        }
    }

    fn arguments(&mut self, ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
        let ports = input_ports(node, context.project);
        if ports.is_empty()
            && !matches!(
                node.kind,
                NodeKind::Request | NodeKind::Build | NodeKind::Condition
            )
        {
            return;
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            widgets::section_inline(ui, "Arguments");
            ui.label(
                RichText::new(ports.len().to_string())
                    .size(11.0)
                    .color(theme::MUTED),
            );
        });
        ui.add_space(2.0);
        let binding_context = binding::Context {
            node: context.node,
            graph: context.graph,
            project: context.project,
            parameters: context.parameters,
            run: context.run,
        };
        let mut remove = None;
        for port in &ports {
            let mut value = node.inputs.get(&port.name).cloned();
            if binding::editor(
                ui,
                &binding_context,
                port,
                &mut value,
                0,
                id.with(("argument", &port.name)),
            ) {
                match value {
                    Some(binding) => {
                        node.inputs.insert(port.name.clone(), binding);
                    }
                    None => {
                        node.inputs.shift_remove(&port.name);
                    }
                }
            }
            if node.kind == NodeKind::Request && port.name != "body" {
                if let Some(true) =
                    argument_settings(ui, id.with(("settings", &port.name)), node, &port.name)
                {
                    remove = Some(port.name.clone());
                }
            } else if node.kind == NodeKind::Build
                && ui
                    .small_button("Remove field")
                    .on_hover_text("Remove this field from the built data")
                    .clicked()
            {
                remove = Some(port.name.clone());
            }
            ui.add_space(4.0);
        }
        if let Some(name) = remove {
            node.inputs.shift_remove(&name);
            if let Some(ports) = &mut node.config.input_ports {
                ports.retain(|port| port.name != name);
            }
        }
        if matches!(
            node.kind,
            NodeKind::Request | NodeKind::Build | NodeKind::Condition
        ) {
            ui.horizontal(|ui| {
                let hint = if node.kind == NodeKind::Build {
                    "Field name"
                } else {
                    "Argument name"
                };
                let field = ui.add(
                    crate::widgets::text_edit(&mut self.new_input)
                        .hint_text(hint)
                        .desired_width(ui.available_width() - 56.0),
                );
                let name = self.new_input.trim().to_owned();
                let valid = !name.is_empty()
                    && !node.inputs.contains_key(&name)
                    && !ports.iter().any(|port| port.name == name);
                let submit =
                    field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if ui.add_enabled(valid, egui::Button::new("Add")).clicked() || (valid && submit) {
                    node.inputs.insert(
                        name.clone(),
                        Binding::literal(serde_json::Value::String(String::new())),
                    );
                    if node.kind == NodeKind::Request {
                        let mut port = Port::new(name, ValueType::Any).required(true);
                        port.exposed = Some(true);
                        node.config
                            .input_ports
                            .get_or_insert_with(Vec::new)
                            .push(port);
                    }
                    self.new_input.clear();
                }
            });
        }
    }

    /// The variable's "take the value from a response" dialog.
    fn response_choice(&mut self, ctx: &egui::Context, context: &Context<'_>, node: &mut Node) {
        let Some(state) = &mut self.choosing_response else {
            return;
        };
        let sources: Vec<&Node> = context
            .graph
            .nodes
            .iter()
            .filter(|candidate| {
                matches!(candidate.kind, NodeKind::Request | NodeKind::Component)
                    && candidate.id != node.id
                    && !apiforge_core::graph::ancestors_and_self(context.graph, &candidate.id)
                        .contains(&node.id)
            })
            .collect();
        let mut close = false;
        let mut chosen = None;
        crate::dialogs::centered_modal(Id::new("variable-response"), ctx).show(ctx, |ui| {
            ui.set_width(520.0_f32.min(ctx.content_rect().width() - 48.0));
            ui.label(
                RichText::new("Take the value from a response")
                    .size(16.0)
                    .family(theme::bold_family()),
            );
            let source = sources
                .iter()
                .find(|candidate| candidate.id == state.source_id);
            widgets::field_label(ui, "Request or component");
            egui::ComboBox::from_id_salt("variable-response-source")
                .selected_text(source.map_or("No request available", |node| node.name.as_str()))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for candidate in &sources {
                        if ui
                            .selectable_label(candidate.id == state.source_id, &candidate.name)
                            .clicked()
                        {
                            state.source_id = candidate.id.clone();
                            state.port = output_ports(candidate, context.project)
                                .first()
                                .map_or_else(|| "body".into(), |port| port.name.clone());
                        }
                    }
                });
            let mut visible_count = None;
            if let Some(source) = sources
                .iter()
                .find(|candidate| candidate.id == state.source_id)
            {
                let outputs = output_ports(source, context.project);
                widgets::field_label(ui, "Output");
                egui::ComboBox::from_id_salt("variable-response-port")
                    .selected_text(state.port.as_str())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for output in &outputs {
                            ui.selectable_value(
                                &mut state.port,
                                output.name.clone(),
                                output.display_name(),
                            );
                        }
                    });
                let data = response::source(source, &state.port, context.project, context.run);
                widgets::hint(ui, &data.origin);
                let fields = response::fields(data.value.as_ref());
                let focus_search = std::mem::replace(&mut state.focus_search, false);
                let (picked, count) = binding::searchable_field_list(
                    ui,
                    &mut state.search,
                    focus_search,
                    &fields.fields,
                    data.value.is_some(),
                    &state.port,
                    (ctx.content_rect().height() - 280.0).clamp(60.0, 320.0),
                );
                if fields.truncated { widgets::hint(ui, "Some fields are omitted by the 1000-field / 12-level limit. You can enter a reference path in the argument editor."); }
                visible_count = Some(count);
                if let Some(path) = picked {
                    chosen = Some(Binding::reference(
                        source.id.clone(),
                        state.port.clone(),
                        path,
                    ));
                }
            } else {
                widgets::hint(ui, "Add a request that does not depend on this variable.");
            }
            ui.separator();
            ui.horizontal(|ui| {
                if let Some(count) = visible_count {
                    widgets::hint(ui, format!("{count} fields"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });
        if let Some(binding) = chosen {
            node.inputs.insert("value".into(), binding);
            close = true;
        }
        if close {
            self.choosing_response = None;
        }
    }
}

fn initial_response_choice(context: &Context<'_>) -> (String, String) {
    if let Some(Binding::Reference { node_id, port, .. }) = context.node.inputs.get("value") {
        return (node_id.clone(), port.clone());
    }
    context
        .graph
        .nodes
        .iter()
        .find(|node| {
            matches!(node.kind, NodeKind::Request | NodeKind::Component)
                && node.id != context.node.id
        })
        .map_or_else(
            || (String::new(), "body".into()),
            |node| {
                let port = output_ports(node, context.project)
                    .first()
                    .map_or_else(|| "body".into(), |port| port.name.clone());
                (node.id.clone(), port)
            },
        )
}

/// A keyboard-accessible alternative to drawing control wires.
fn flow_connections(ui: &mut Ui, context: &Context<'_>, actions: &mut Vec<Action>) {
    let node = context.node;
    let board = context.board;
    if !board.nodes.iter().any(|candidate| candidate.id == node.id) {
        return;
    }
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(theme::FIELD)
        .corner_radius(8)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(RichText::new("Execution order").family(theme::bold_family()));
            let ports: &[(&str, &str)] = if node.kind == NodeKind::If {
                &[(TRUE_PORT, "Yes"), (FALSE_PORT, "No")]
            } else {
                &[(ORDER_PORT, "Next")]
            };
            for (port, label) in ports {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(*label).size(12.0));
                    egui::ComboBox::from_id_salt(("flow", &node.id, *port))
                        .selected_text("Connect to…")
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for target in board.nodes.iter().filter(|target| {
                                target.id != node.id && target.kind != NodeKind::Trigger
                            }) {
                                if ui.selectable_label(false, &target.name).clicked() {
                                    match edit::connect(
                                        board,
                                        context.project,
                                        &node.id,
                                        port,
                                        &target.id,
                                        ORDER_PORT,
                                    ) {
                                        Ok(graph) => actions.push(Action::UpdateGraph(graph)),
                                        Err(error) => {
                                            actions.push(Action::Error(error.to_string()))
                                        }
                                    }
                                }
                            }
                        });
                });
            }
            for edge in board.edges.iter().filter(|edge| edge.source == node.id) {
                ui.horizontal(|ui| {
                    let label = match edge.branch {
                        Some(Branch::True) => "Yes",
                        Some(Branch::False) => "No",
                        None => "Next",
                    };
                    let target = board
                        .nodes
                        .iter()
                        .find(|candidate| candidate.id == edge.target)
                        .map_or("Missing step", |node| node.name.as_str());
                    ui.label(
                        RichText::new(format!("{label} → {target}"))
                            .size(12.0)
                            .color(theme::LINK),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("✕")
                            .on_hover_text("Remove this ordering connection")
                            .clicked()
                        {
                            actions.push(Action::UpdateGraph(edit::remove_connections(
                                board,
                                &HashSet::from([edge.id.as_str()]),
                            )));
                        }
                    });
                });
            }
        });
}

fn request(ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
    ui.add_space(6.0);
    widgets::field_label(ui, "Method and URL or path");
    ui.horizontal(|ui| {
        let mut method = node
            .config
            .method
            .unwrap_or(apiforge_core::model::HttpMethod::Get);
        egui::ComboBox::from_id_salt(id.with("method"))
            .selected_text(labels::method(method))
            .width(86.0)
            .show_ui(ui, |ui| {
                for option in labels::METHODS {
                    ui.selectable_value(&mut method, option, labels::method(option));
                }
            });
        node.config.method = Some(method);
        let mut url = node.config.url.clone().unwrap_or_default();
        let event = fields::variable_text(ui, id.with("url"), &mut url, "/endpoint/:argument");
        if event.changed {
            node.config.url = Some(url.clone());
        }
        if let Some((variable, start, end)) = event.dropped {
            *node = bind_variable_text(
                node,
                &variable,
                &fields::selection(&url, start, end),
                |mut next, text| {
                    next.config.url = Some(text);
                    next
                },
            );
        }
    });
    widgets::hint(
        ui,
        "A relative address uses the environment's base URL. Arguments: :argument or {{argument}}.",
    );

    widgets::field_label(ui, "Timeout (ms)");
    let mut timeout = node.config.timeout_ms.unwrap_or(10_000.0);
    if ui
        .add(
            egui::DragValue::new(&mut timeout)
                .range(1.0..=60_000.0)
                .speed(100.0),
        )
        .changed()
    {
        node.config.timeout_ms = Some(timeout.round());
    }

    ui.add_space(6.0);
    for (label, key) in [("Query parameters", "query"), ("Headers", "headers")] {
        let current = if key == "query" {
            node.config.query.clone()
        } else {
            node.config.headers.clone()
        }
        .unwrap_or_default();
        let (changed, dropped) = fields::string_map(ui, id.with(key), label, &current);
        let set = |node: &mut Node, map| {
            if key == "query" {
                node.config.query = Some(map);
            } else {
                node.config.headers = Some(map);
            }
        };
        if let Some(map) = changed {
            set(node, map);
        }
        if let Some((entry, variable, start, end)) = dropped {
            let text = current.get(&entry).cloned().unwrap_or_default();
            *node = bind_variable_text(
                node,
                &variable,
                &fields::selection(&text, start, end),
                |mut next, value| {
                    let map = if key == "query" {
                        &mut next.config.query
                    } else {
                        &mut next.config.headers
                    };
                    map.get_or_insert_with(Default::default)
                        .insert(entry.clone(), value);
                    next
                },
            );
        }
        ui.add_space(4.0);
    }
    widgets::hint(
        ui,
        "Values may use {{argument}}. Put tokens in an argument bound to Environment / secret, e.g. Authorization = Bearer {{token}}.",
    );
    let _ = context;
}

/// Settings of a request's own argument; returns `Some(true)` to remove it.
fn argument_settings(ui: &mut Ui, id: Id, node: &mut Node, name: &str) -> Option<bool> {
    let ports = node.config.input_ports.as_mut()?;
    let port = ports.iter_mut().find(|port| port.name == name)?;
    let mut remove = false;
    egui::CollapsingHeader::new(RichText::new("Argument settings").size(11.0))
        .id_salt(id)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Type");
                fields::type_combo(ui, id.with("type"), &mut port.value_type);
            });
            let mut required = port.required.unwrap_or(false);
            if ui.checkbox(&mut required, "Required").changed() {
                port.required = Some(required);
            }
            let mut exposed = port.exposed.unwrap_or(false);
            if ui.checkbox(&mut exposed, "Show on the block").changed() {
                port.exposed = Some(exposed);
            }
            if ui
                .add(egui::Button::new(
                    RichText::new("Remove argument").color(theme::DANGER),
                ))
                .clicked()
            {
                remove = true;
            }
        });
    Some(remove)
}

fn select(ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
    ui.add_space(6.0);
    widgets::field_label(ui, "Path to the field");
    let mut field = node.config.path.clone().unwrap_or_default();
    let event = fields::variable_text(
        ui,
        id.with("path"),
        &mut field,
        "e.g. items[0].name; empty = whole value",
    );
    if event.changed {
        node.config.path = Some(field.clone());
    }
    if let Some((variable, start, end)) = event.dropped {
        *node = bind_variable_text(
            node,
            &variable,
            &fields::selection(&field, start, end),
            |mut next, text| {
                next.config.path = Some(text);
                next
            },
        );
    }
    // Suggest paths from the bound source's sample.
    if let Some(Binding::Reference {
        node_id,
        port,
        path: within,
    }) = node.inputs.get("value")
        && let Some(source) = context
            .graph
            .nodes
            .iter()
            .find(|candidate| &candidate.id == node_id)
    {
        let sample = response::sample_output(source, port, context.project)
            .and_then(|value| path::resolve(&value, within).map(|value| value.into_owned()));
        let paths = edit::sample_paths(sample.as_ref());
        if !paths.is_empty() {
            let current = node.config.path.clone().unwrap_or_default();
            egui::ComboBox::from_id_salt(id.with("sample-paths"))
                .selected_text(if current.is_empty() {
                    "Whole value".to_owned()
                } else {
                    current.clone()
                })
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(current.is_empty(), "Whole value")
                        .clicked()
                    {
                        node.config.path = Some(String::new());
                    }
                    for candidate in paths {
                        if ui
                            .selectable_label(candidate == current, &candidate)
                            .clicked()
                        {
                            node.config.path = Some(candidate);
                        }
                    }
                });
            widgets::hint(
                ui,
                "Structure from the sample — a suggestion, not a guarantee.",
            );
        }
    }
}

fn check(ui: &mut Ui, id: Id, node: &mut Node) {
    ui.add_space(6.0);
    widgets::field_label(ui, "Check");
    let current = predicate(node);
    let mut chosen = current;
    egui::ComboBox::from_id_salt(id.with("predicate"))
        .selected_text(predicate_label(current))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for option in Predicate::ALL {
                ui.selectable_value(&mut chosen, option, predicate_label(option));
            }
        });
    if chosen != current {
        node.config.predicate = Some(chosen);
        if chosen == Predicate::Type
            && matches!(node.inputs.get("expected"), Some(Binding::Literal { .. }))
        {
            node.inputs.insert(
                "expected".into(),
                Binding::literal(serde_json::json!("string")),
            );
        }
    }
    if chosen == Predicate::Approximately {
        widgets::field_label(ui, "Allowed difference");
        let mut tolerance = node.config.tolerance.unwrap_or(0.01);
        if ui
            .add(
                egui::DragValue::new(&mut tolerance)
                    .range(0.0..=f64::MAX)
                    .speed(0.01),
            )
            .changed()
        {
            node.config.tolerance = Some(tolerance.max(0.0));
        }
    }
    if node.kind == NodeKind::Condition {
        widgets::hint(
            ui,
            "An unmet condition means invalid data and blocks dependent steps.",
        );
    }
}

fn predicate_label(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Equals => "Is equal to",
        Predicate::NotEquals => "Is not equal to",
        Predicate::Greater => "Is greater than",
        Predicate::Less => "Is less than",
        Predicate::Exists => "Exists (null counts)",
        Predicate::NotEmpty => "Is not empty",
        Predicate::Contains => "Contains",
        Predicate::Type => "Has type",
        Predicate::Approximately => "Equals within a tolerance",
    }
}

fn condition_predicates(ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
    ui.add_space(8.0);
    widgets::section(ui, "Additional conditions");
    let ports: Vec<String> = input_ports(node, context.project)
        .into_iter()
        .map(|port| port.name)
        .collect();
    let mut matching = node.config.match_mode.unwrap_or(Match::All);
    egui::ComboBox::from_id_salt(id.with("match"))
        .selected_text(if matching == Match::All {
            "All conditions"
        } else {
            "Any condition"
        })
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut matching, Match::All, "All conditions");
            ui.selectable_value(&mut matching, Match::Any, "Any condition");
        });
    if node.config.match_mode.is_some() || matching != Match::All {
        node.config.match_mode = Some(matching);
    }
    let mut predicates = node.config.predicates.clone().unwrap_or_default();
    let mut remove = None;
    let mut dropped_on_path = None;
    for (index, condition) in predicates.iter_mut().enumerate() {
        let row = id.with(("condition", index));
        egui::Frame::new()
            .fill(theme::FIELD)
            .corner_radius(6)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                widgets::field_label(ui, "Actual argument");
                name_combo(ui, row.with("input"), &mut condition.input, &ports);
                widgets::field_label(ui, "Comparison");
                let mut predicate = condition.predicate;
                egui::ComboBox::from_id_salt(row.with("predicate"))
                    .selected_text(predicate_label(predicate))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for option in Predicate::ALL {
                            ui.selectable_value(&mut predicate, option, predicate_label(option));
                        }
                    });
                condition.predicate = predicate;
                widgets::field_label(ui, "Expected argument");
                name_combo(ui, row.with("expected"), &mut condition.expected, &ports);
                let mut field = condition.path.clone().unwrap_or_default();
                let event =
                    fields::variable_text(ui, row.with("path"), &mut field, "Path (optional)");
                if event.changed {
                    condition.path = Some(field.clone());
                }
                if let Some((variable, start, end)) = event.dropped {
                    dropped_on_path = Some((index, field, variable, start, end));
                }
                if ui.small_button("Remove condition").clicked() {
                    remove = Some(index);
                }
            });
    }
    if let Some(index) = remove {
        predicates.remove(index);
    }
    if ui.button("+ Condition").clicked() {
        predicates.push(ConditionPredicate {
            input: "actual".into(),
            expected: "expected".into(),
            predicate: predicate(node),
            path: None,
        });
    }
    if node.config.predicates.is_some() || !predicates.is_empty() {
        node.config.predicates = Some(predicates);
    }
    if let Some((index, current_path, variable, start, end)) = dropped_on_path {
        *node = bind_variable_text(
            node,
            &variable,
            &fields::selection(&current_path, start, end),
            |mut next, text| {
                if let Some(list) = &mut next.config.predicates
                    && let Some(item) = list.get_mut(index)
                {
                    item.path = Some(text);
                }
                next
            },
        );
    }
    widgets::hint(
        ui,
        "With a list, the engine checks these conditions instead of the single check above.",
    );
}

fn name_combo(ui: &mut Ui, id: Id, value: &mut String, names: &[String]) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.as_str())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for name in names {
                ui.selectable_value(value, name.clone(), name);
            }
        });
}

fn samples(ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
    ui.add_space(8.0);
    egui::CollapsingHeader::new(RichText::new("Contract and sample response").strong())
        .id_salt(id.with("samples"))
        .show(ui, |ui| {
            widgets::field_label(ui, "Data contract");
            let mut contract = node.config.contract_id.clone();
            let selected = contract
                .as_ref()
                .and_then(|id| context.project.contracts.iter().find(|candidate| &candidate.id == id))
                .map_or("No named contract", |contract| contract.name.as_str());
            egui::ComboBox::from_id_salt(id.with("contract"))
                .selected_text(selected)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut contract, None, "No named contract");
                    for candidate in &context.project.contracts {
                        ui.selectable_value(&mut contract, Some(candidate.id.clone()), &candidate.name);
                    }
                });
            node.config.contract_id = contract;
            widgets::field_label(ui, "Sample HTTP status");
            let mut status = node.config.sample_status.unwrap_or(200);
            if ui.add(egui::DragValue::new(&mut status).range(100..=599)).changed() {
                node.config.sample_status = Some(status);
            }
            widgets::field_label(ui, "Sample body");
            let current = match &node.config.sample_response {
                OptionalJson::Present(value) => value.clone(),
                OptionalJson::Missing => serde_json::Value::Null,
            };
            if let Some(value) = fields::json_editor(ui, id.with("sample-body"), &current, 8) {
                node.config.sample_response = OptionalJson::Present(value);
            }
            widgets::hint(ui, "Samples drive suggestions and simulation. A field in the sample does not create an assertion.");
        });
}

fn outputs(ui: &mut Ui, context: &Context<'_>, actions: &mut Vec<Action>) {
    let node = context.node;
    widgets::hint(
        ui,
        if context.step.is_some() {
            "Data from the last run. Editing the block does not update it."
        } else {
            "Sample data — not from an API call."
        },
    );
    let mut any = false;
    for port in output_ports(node, context.project) {
        let value = match context.step {
            Some(step) => step.outputs.get(&port.name).cloned(),
            None => response::sample_output(node, &port.name, context.project),
        };
        let Some(value) = value else { continue };
        any = true;
        widgets::section(ui, port.display_name());
        if let Some(action) = fields::json_tree(ui, &node.id, &port.name, &value) {
            actions.push(match action {
                fields::TreeAction::Check { path, value } => Action::AssertField {
                    port: port.name.clone(),
                    path,
                    value,
                },
                fields::TreeAction::Assign { path } => Action::AssignField {
                    port: port.name.clone(),
                    path,
                },
            });
        }
    }
    if !any {
        ui.add_space(12.0);
        widgets::hint(
            ui,
            "Run the step to see its data. A request can also have a sample response.",
        );
    }
}
