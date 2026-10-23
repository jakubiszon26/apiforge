//! The right panel: configuration and output data of the selected step.
//!
//! The inspector edits a copy of the node and returns [`Action`]s; the application
//! applies them as undoable edits.

mod binding;
mod fields;
mod response;

pub use response::sample_output;

use crate::icons::Icon;
use crate::{labels, theme, widgets};
use apiforge_core::{
    edit,
    format::{Binding, ConditionPredicate, Graph, Node, OptionalJson, Port, Project},
    graph::{
        BODY_INPUT, FALSE_PORT, HEADERS_INPUT, ORDER_PORT, QUERY_INPUT, TRUE_PORT, URL_INPUT,
        VALUE_INPUT, component_definition, condition_inputs, condition_predicates, input_ports,
        output_ports, predicate,
    },
    handle::is_valid_handle,
    model::{BodyFormat, Branch, Match, NodeKind, Predicate, ValueType},
    run::{RunResult, StepResult},
    template_text::{Scope, print_template},
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
    choosing_response: Option<ResponseChoiceState>,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            tab: Tab::Configuration,
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
            RichText::new(context.node.display_name())
                .size(theme::FONT_PANEL_TITLE)
                .family(theme::bold_family()),
        );
        ui.add_space(theme::SPACE_SM);
        ui.horizontal(|ui| {
            widgets::tab_value(ui, &mut self.tab, Tab::Configuration, "Configuration");
            widgets::tab_value(ui, &mut self.tab, Tab::Outputs, "Output data");
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
            actions.push(Action::UpdateNode(Box::new(node)));
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
        let mut label = node.label.clone().unwrap_or_default();
        if ui
            .add(
                crate::widgets::text_edit(&mut label)
                    .id(id.with("name"))
                    .hint_text(node.handle.as_str())
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            node.label = (!label.trim().is_empty()).then_some(label);
        }
        handle_field(ui, id.with("handle"), context, node);

        match node.kind {
            NodeKind::Trigger => widgets::hint(
                ui,
                "A manual trigger. Running the test starts this path. Connect Next to the first step.",
            ),
            NodeKind::If => widgets::hint(
                ui,
                "The comparison chooses the Yes or No output. False is not a test failure. Connect both branches.",
            ),
            NodeKind::Value => {
                ui.add_space(6.0);
                let mut variable = node.is_variable();
                if ui
                    .checkbox(&mut variable, "Variable")
                    .on_hover_text("List this value with the variables, so it can be used anywhere")
                    .changed()
                {
                    node.config.variable = variable.then_some(true);
                }
                if ui
                    .add(widgets::action(Icon::Variable, "Use a response value").quiet())
                    .on_hover_text("Take this value from a field of a request's response")
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
                    "A value can be data, a field of another step's result or a calculation. It is computed again on every run.",
                );
            }
            NodeKind::Request => request(ui, id, context, node),
            NodeKind::Assert => check(ui, id, node),
            NodeKind::Export => {
                widgets::field_label(ui, "Output type");
                let mut value_type = node.config.output_type.unwrap_or(ValueType::Any);
                if fields::type_combo(ui, id.with("output-type"), &mut value_type) {
                    node.config.output_type = (value_type != ValueType::Any).then_some(value_type);
                }
                contract_combo(
                    ui,
                    id.with("output-contract"),
                    context,
                    &mut node.config.contract_id,
                );
                widgets::hint(
                    ui,
                    "Steps using this component read the value as an output named like this block in code.",
                );
            }
            NodeKind::Condition => {}
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
                            widgets::action(Icon::ExternalLink, "Open component").quiet(),
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
                            .add(
                                widgets::action(
                                    Icon::Refresh,
                                    &format!("Update to revision {newest}"),
                                )
                                .quiet(),
                            )
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
            condition_checks(ui, id, context, node);
        }
        if node.kind == NodeKind::Request {
            samples(ui, id, context, node);
        }
    }

    fn arguments(&mut self, ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
        let scope = scope(context);
        // Fields shown as text in the request section are edited there; a value
        // that text cannot express falls back to an argument editor.
        let ports: Vec<Port> = input_ports(node, context.project)
            .into_iter()
            .filter(|port| match node.kind {
                NodeKind::Condition => false,
                NodeKind::Request if port.name != BODY_INPUT => node
                    .inputs
                    .get(&port.name)
                    .is_some_and(|binding| !has_text_form(binding, scope)),
                _ => true,
            })
            .collect();
        if ports.is_empty() {
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
        let binding_context = binding_context(context);
        for port in &ports {
            argument(
                ui,
                &binding_context,
                port,
                node,
                id.with(("argument", &port.name)),
            );
            ui.add_space(4.0);
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
                .selected_text(source.map_or("No request available", |node| node.display_name()))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for candidate in &sources {
                        if ui
                            .selectable_label(candidate.id == state.source_id, candidate.display_name())
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
                    if ui.add(widgets::action(Icon::Close, "Cancel").quiet()).clicked() {
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
                                if ui.selectable_label(false, target.display_name()).clicked() {
                                    match edit::connect(
                                        board,
                                        context.project,
                                        &node.id,
                                        port,
                                        &target.id,
                                        ORDER_PORT,
                                        &[],
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
                        .map_or("Missing step", |node| node.display_name());
                    ui.label(
                        RichText::new(format!("{label} → {target}"))
                            .size(12.0)
                            .color(theme::LINK),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                widgets::icon_button(Icon::Trash, "Remove")
                                    .compact()
                                    .danger(),
                            )
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
    let scope = scope(context);
    ui.add_space(6.0);
    widgets::field_label(ui, "Method and URL or path");
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
    let url = node
        .inputs
        .get(URL_INPUT)
        .cloned()
        .unwrap_or_else(|| Binding::literal(serde_json::json!("")));
    if let Some(next) =
        fields::template_field(ui, id.with("url"), &url, scope, "/orders/{{order.body.id}}")
    {
        node.inputs.insert(URL_INPUT.into(), next);
    }
    widgets::hint(
        ui,
        "A relative address uses the environment's base URL. Insert values as {{block.field}}, {{env.name}} or {{param.name}}; they are URL-encoded.",
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

    widgets::field_label(ui, "Body format");
    let mut format = node.config.body_format.unwrap_or_default();
    egui::ComboBox::from_id_salt(id.with("body-format"))
        .selected_text(body_format_label(format))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for option in [BodyFormat::Json, BodyFormat::Text] {
                ui.selectable_value(&mut format, option, body_format_label(option));
            }
        });
    node.config.body_format = (format != BodyFormat::default()).then_some(format);

    ui.add_space(6.0);
    for (label, input) in [
        ("Query parameters", QUERY_INPUT),
        ("Headers", HEADERS_INPUT),
    ] {
        let current = node.inputs.get(input);
        if current.is_some_and(|binding| !has_text_form(binding, scope)) {
            continue;
        }
        match fields::template_map(ui, id.with(input), label, current, scope) {
            Some(Some(next)) => {
                node.inputs.insert(input.into(), next);
            }
            Some(None) => {
                node.inputs.shift_remove(input);
            }
            None => {}
        }
        ui.add_space(4.0);
    }
    widgets::hint(
        ui,
        "Secrets belong in the environment: Authorization = Bearer {{env.token}}.",
    );
}

fn body_format_label(format: BodyFormat) -> &'static str {
    match format {
        BodyFormat::Json => "JSON",
        BodyFormat::Text => "Text",
    }
}

/// Whether a request field can be edited as text with `{{reference}}` tokens.
fn has_text_form(binding: &Binding, scope: Scope<'_>) -> bool {
    match binding {
        Binding::Object { fields } => fields
            .values()
            .all(|field| print_template(field, scope).is_some()),
        other => print_template(other, scope).is_some(),
    }
}

fn scope<'a>(context: &Context<'a>) -> Scope<'a> {
    Scope {
        nodes: &context.graph.nodes,
        project: context.project,
    }
}

fn binding_context<'a>(context: &Context<'a>) -> binding::Context<'a> {
    binding::Context {
        node: context.node,
        graph: context.graph,
        project: context.project,
        parameters: context.parameters,
        run: context.run,
    }
}

/// The editor of one input: sets or removes it.
fn argument(ui: &mut Ui, context: &binding::Context<'_>, port: &Port, node: &mut Node, id: Id) {
    let mut value = node.inputs.get(&port.name).cloned();
    if binding::editor(ui, context, port, &mut value, 0, id) {
        match value {
            Some(binding) => {
                node.inputs.insert(port.name.clone(), binding);
            }
            None => {
                node.inputs.shift_remove(&port.name);
            }
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

/// The checks of a Data condition: each compares `actual<n>` with `expected<n>`.
fn condition_checks(ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
    let mut predicates = condition_predicates(node);
    ui.add_space(10.0);
    widgets::section_inline(ui, "Checks");
    if predicates.len() > 1 {
        let mut mode = node.config.match_mode.unwrap_or(Match::All);
        egui::ComboBox::from_id_salt(id.with("match"))
            .selected_text(match mode {
                Match::All => "All checks must hold",
                Match::Any => "At least one check must hold",
            })
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut mode, Match::All, "All checks must hold");
                ui.selectable_value(&mut mode, Match::Any, "At least one check must hold");
            });
        node.config.match_mode = Some(mode);
    }
    let binding_context = binding_context(context);
    let ports = input_ports(node, context.project);
    let port = |name: &str| ports.iter().find(|port| port.name == name).cloned();
    let mut removed = None;
    for (index, predicate) in predicates.iter_mut().enumerate() {
        let (actual, expected) = condition_inputs(index + 1);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Check {}", index + 1)).family(theme::bold_family()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_remove = node
                    .config
                    .predicates
                    .as_ref()
                    .is_some_and(|list| list.len() > 1);
                if ui
                    .add_enabled(
                        can_remove,
                        widgets::icon_button(Icon::Trash, "Remove check")
                            .compact()
                            .danger(),
                    )
                    .clicked()
                {
                    removed = Some(index);
                }
            });
        });
        egui::ComboBox::from_id_salt(id.with(("check", index)))
            .selected_text(predicate_label(*predicate))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for option in Predicate::ALL {
                    ui.selectable_value(predicate, option, predicate_label(option));
                }
            });
        if let Some(port) = port(&actual) {
            argument(
                ui,
                &binding_context,
                &port,
                node,
                id.with(("actual", index)),
            );
        }
        if predicate.takes_expected()
            && let Some(port) = port(&expected)
        {
            argument(
                ui,
                &binding_context,
                &port,
                node,
                id.with(("expected", index)),
            );
        }
    }
    if let Some(index) = removed {
        predicates.remove(index);
        // Later checks move up, so their inputs are renumbered.
        let mut inputs = node.inputs.clone();
        for position in index + 1..=predicates.len() + 1 {
            let (from_actual, from_expected) = condition_inputs(position + 1);
            let (to_actual, to_expected) = condition_inputs(position);
            for (from, to) in [(from_actual, to_actual), (from_expected, to_expected)] {
                match node.inputs.get(&from) {
                    Some(binding) => {
                        inputs.insert(to, binding.clone());
                    }
                    None => {
                        inputs.shift_remove(&to);
                    }
                }
                inputs.shift_remove(&from);
            }
        }
        node.inputs = inputs;
    }
    if ui
        .add(widgets::action(Icon::Plus, "Add check").quiet())
        .clicked()
    {
        predicates.push(Predicate::Equals);
        let (actual, expected) = condition_inputs(predicates.len());
        node.inputs
            .insert(actual, Binding::literal(serde_json::Value::Null));
        node.inputs
            .insert(expected, Binding::literal(serde_json::Value::Null));
    }
    if predicates.contains(&Predicate::Approximately) {
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
    node.config.predicates = Some(
        predicates
            .into_iter()
            .map(|predicate| ConditionPredicate { predicate })
            .collect(),
    );

    ui.add_space(6.0);
    let mut passes_data = node.inputs.contains_key(VALUE_INPUT);
    if ui
        .checkbox(&mut passes_data, "Pass on other data")
        .on_hover_text("Without it, the condition passes on the first checked value")
        .changed()
    {
        if passes_data {
            node.inputs.insert(
                VALUE_INPUT.into(),
                Binding::literal(serde_json::Value::Null),
            );
        } else {
            node.inputs.shift_remove(VALUE_INPUT);
        }
    }
    if passes_data && let Some(port) = port(VALUE_INPUT) {
        argument(ui, &binding_context, &port, node, id.with("passed-on"));
    }
    widgets::hint(
        ui,
        "An unmet condition means invalid data and blocks dependent steps.",
    );
}

/// Chooses a named data contract of the project, or none.
fn contract_combo(ui: &mut Ui, id: Id, context: &Context<'_>, contract: &mut Option<String>) {
    widgets::field_label(ui, "Data contract");
    let selected = contract
        .as_ref()
        .and_then(|id| {
            context
                .project
                .contracts
                .iter()
                .find(|candidate| &candidate.id == id)
        })
        .map_or("No named contract", |contract| contract.name.as_str());
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(contract, None, "No named contract");
            for candidate in &context.project.contracts {
                ui.selectable_value(contract, Some(candidate.id.clone()), &candidate.name);
            }
        });
}

fn samples(ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
    ui.add_space(8.0);
    egui::CollapsingHeader::new(RichText::new("Contract and sample response").strong())
        .id_salt(id.with("samples"))
        .show(ui, |ui| {
            contract_combo(ui, id.with("contract"), context, &mut node.config.contract_id);
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

/// Edits the block's handle, its name in scenario code. An invalid or taken handle
/// stays a draft in the field and is not applied.
fn handle_field(ui: &mut Ui, id: Id, context: &Context<'_>, node: &mut Node) {
    widgets::field_label(ui, "Name in code");
    let mut draft: String = ui
        .data_mut(|data| data.get_temp(id))
        .unwrap_or_else(|| node.handle.clone());
    let field = ui.add(
        crate::widgets::text_edit(&mut draft)
            .id(id.with("field"))
            .font(egui::TextStyle::Monospace)
            .desired_width(f32::INFINITY),
    );
    let taken = |handle: &str| {
        context
            .graph
            .nodes
            .iter()
            .chain(&context.board.nodes)
            .any(|other| other.id != node.id && other.handle == handle)
    };
    let problem = if !is_valid_handle(&draft) {
        Some("Use letters, digits and _, starting with a letter or _; not a reserved word.")
    } else if taken(&draft) {
        Some("Another block already uses this name.")
    } else {
        None
    };
    match problem {
        Some(message) => {
            ui.colored_label(theme::DANGER, RichText::new(message).size(11.0));
        }
        None if field.changed() => node.handle = draft.clone(),
        None => {}
    }
    if field.has_focus() || problem.is_some() {
        ui.data_mut(|data| data.insert_temp(id, draft));
    } else {
        ui.data_mut(|data| data.remove::<String>(id));
    }
}
