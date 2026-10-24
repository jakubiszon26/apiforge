//! Editing one argument: a literal, another step's output, a nested operator block,
//! an object, a list, a text template, a parameter or an environment value.

use super::{fields, response};
use crate::icons::Icon;
use crate::{dnd::Payload, theme, widgets};
use apiforge_core::{
    format::{Binding, Graph, Node, Port, Project, TemplatePart},
    graph::{ancestors_and_self, compatible, output_ports},
    model::ValueType,
    operator::{Operator, OperatorGroup},
    path,
    run::RunResult,
};
use eframe::egui::{self, Id, RichText, Stroke, Ui};
use serde_json::Value;

/// Nesting limit of composite bindings, one below the validator's 32 levels.
const MAX_DEPTH: usize = 31;

#[derive(Clone, Copy)]
pub struct Context<'a> {
    pub node: &'a Node,
    /// Nodes a reference may point to.
    pub graph: &'a Graph,
    pub project: &'a Project,
    pub parameters: &'a [String],
    pub run: Option<&'a RunResult>,
    /// Owning input slot, retained through object fields, arrays and operators.
    /// A nested field's display name must not hide a same-named request argument.
    pub input: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unset,
    Literal,
    Reference,
    Expression,
    Object,
    List,
    Template,
    Parameter,
    Environment,
    Argument,
}

const KINDS: [Kind; 10] = [
    Kind::Unset,
    Kind::Literal,
    Kind::Reference,
    Kind::Expression,
    Kind::Object,
    Kind::List,
    Kind::Template,
    Kind::Parameter,
    Kind::Environment,
    Kind::Argument,
];

impl Kind {
    fn of(binding: Option<&Binding>) -> Self {
        match binding {
            None => Self::Unset,
            Some(Binding::Literal { .. }) => Self::Literal,
            Some(Binding::Reference { .. }) => Self::Reference,
            Some(Binding::Expression { .. }) => Self::Expression,
            Some(Binding::Parameter { .. }) => Self::Parameter,
            Some(Binding::Environment { .. }) => Self::Environment,
            Some(Binding::Argument { .. }) => Self::Argument,
            Some(Binding::Object { .. }) => Self::Object,
            Some(Binding::Array { .. }) => Self::List,
            Some(Binding::Template { .. }) => Self::Template,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Unset => "Not set",
            Self::Literal => "Typed value",
            Self::Reference => "Result of another step",
            Self::Expression => "Operator block",
            Self::Parameter => "Parameter",
            Self::Environment => "Environment / secret",
            Self::Argument => "Request argument",
            Self::Object => "Object",
            Self::List => "List",
            Self::Template => "Text with values",
        }
    }
}

/// Steps whose outputs this node may use without creating a cycle.
fn sources<'a>(context: &Context<'a>) -> Vec<&'a Node> {
    context
        .graph
        .nodes
        .iter()
        .filter(|candidate| {
            candidate.id != context.node.id
                && !output_ports(candidate, context.project).is_empty()
                && !ancestors_and_self(context.graph, &candidate.id).contains(&context.node.id)
        })
        .collect()
}

fn argument_names(context: &Context<'_>) -> Vec<String> {
    if context.node.kind != apiforge_core::model::NodeKind::Request {
        return vec![];
    }
    context
        .node
        .inputs
        .keys()
        .filter(|name| {
            apiforge_core::graph::is_request_argument(name) && context.input != Some(name.as_str())
        })
        .cloned()
        .collect()
}

/// Shows the editor and updates `value`; returns whether it changed.
pub fn editor(
    ui: &mut Ui,
    context: &Context<'_>,
    port: &Port,
    value: &mut Option<Binding>,
    depth: usize,
    id: Id,
) -> bool {
    let root_context = Context {
        input: if depth == 0 {
            Some(&port.name)
        } else {
            context.input
        },
        ..*context
    };
    let context = &root_context;
    let mut changed = false;
    let frame = egui::Frame::new()
        .fill(if depth == 0 {
            theme::WHITE
        } else {
            theme::FIELD
        })
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(10));
    let response = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = if port.is_required() {
                    format!("{} *", port.display_name())
                } else {
                    port.display_name().to_owned()
                };
                ui.label(RichText::new(label).family(theme::bold_family()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(fields::value_type_label(port.value_type))
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                });
            });
            // Give source selection its own line: long argument names must not
            // compete with a dropdown in a narrow inspector or nested operator.
            changed |= kind_combo(ui, context, port, value, depth, id);
            ui.add_space(3.0);
            changed |= body(ui, context, port, value, depth, id);
            if value.is_none() && port.is_required() {
                ui.colored_label(
                    theme::DANGER,
                    RichText::new("Set this required argument.").size(11.0),
                );
            }
        })
        .response;

    // Dropping a variable replaces the source; dropping an operator wraps the value.
    if let Some(payload) = response.dnd_hover_payload::<Payload>() {
        let accepts = matches!(*payload, Payload::Variable { .. })
            || (matches!(*payload, Payload::Operator(_)) && depth < MAX_DEPTH);
        if accepts {
            ui.painter().rect_stroke(
                response.rect,
                8.0,
                Stroke::new(1.5, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
    }
    if let Some(payload) = response.dnd_release_payload::<Payload>() {
        match payload.as_ref() {
            Payload::Variable { binding, .. } => {
                *value = Some(binding.clone());
                changed = true;
            }
            Payload::Operator(operator) if depth < MAX_DEPTH => {
                *value = Some(wrap(*operator, value.take()));
                changed = true;
            }
            _ => {}
        }
    }
    changed
}

/// A new operator block with the current value in its first slot.
fn wrap(operator: Operator, current: Option<Binding>) -> Binding {
    let block = Binding::expression(operator);
    match (current, operator.spec().operand("left")) {
        (Some(current), Some(_)) => block.replace_at(&["left".into()], current).unwrap_or(block),
        _ => block,
    }
}

fn kind_combo(
    ui: &mut Ui,
    context: &Context<'_>,
    port: &Port,
    value: &mut Option<Binding>,
    depth: usize,
    id: Id,
) -> bool {
    let current = Kind::of(value.as_ref());
    let mut chosen = current;
    let available_sources = sources(context);
    egui::ComboBox::from_id_salt(id.with("kind"))
        .selected_text(current.label())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for kind in KINDS {
                let enabled = match kind {
                    Kind::Reference => !available_sources.is_empty(),
                    Kind::Argument => !argument_names(context).is_empty(),
                    Kind::Expression | Kind::Object | Kind::List | Kind::Template => {
                        depth < MAX_DEPTH
                    }
                    _ => true,
                };
                ui.add_enabled_ui(enabled, |ui| {
                    ui.selectable_value(&mut chosen, kind, kind.label());
                });
            }
        });
    if chosen == current {
        return false;
    }
    *value = match chosen {
        Kind::Unset => None,
        Kind::Literal => Some(Binding::literal(
            port.default()
                .cloned()
                .unwrap_or_else(|| fields::empty_literal(port.value_type)),
        )),
        Kind::Reference => available_sources.first().map(|source| {
            let output = output_ports(source, context.project)
                .first()
                .map_or_else(|| "value".to_owned(), |port| port.name.clone());
            Binding::reference(source.id.clone(), output, "")
        }),
        Kind::Expression => Some(wrap(Operator::Add, value.take())),
        Kind::Object => Some(Binding::Object {
            fields: indexmap::IndexMap::from([("field".to_owned(), Binding::literal(Value::Null))]),
        }),
        Kind::List => Some(Binding::Array {
            items: vec![Binding::literal(Value::Null)],
        }),
        Kind::Template => Some(Binding::Template {
            parts: template_from(value.take()),
        }),
        Kind::Parameter => Some(Binding::parameter(
            context
                .parameters
                .first()
                .cloned()
                .unwrap_or_else(|| port.name.clone()),
        )),
        Kind::Argument => {
            argument_names(context)
                .into_iter()
                .next()
                .map(|name| Binding::Argument {
                    name,
                    path: String::new(),
                })
        }
        Kind::Environment => Some(Binding::environment(
            context
                .project
                .environment
                .variables
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "baseUrl".into()),
        )),
    };
    true
}

fn body(
    ui: &mut Ui,
    context: &Context<'_>,
    port: &Port,
    value: &mut Option<Binding>,
    depth: usize,
    id: Id,
) -> bool {
    match value {
        None => false,
        Some(Binding::Literal { value: literal }) => literal_editor(ui, context, port, literal, id),
        Some(Binding::Expression { operator, inputs }) => {
            let mut unwrap = false;
            let changed =
                expression_editor(ui, context, port, operator, inputs, depth, id, &mut unwrap);
            if unwrap {
                *value = inputs.get("left").cloned();
                return true;
            }
            changed
        }
        Some(Binding::Reference {
            node_id,
            port: output,
            path,
        }) => reference_editor(ui, context, port, node_id, output, path, id),
        Some(Binding::Argument { name, path }) => {
            let names = argument_names(context);
            let mut changed =
                name_editor(ui, id, name, &names, "A named argument of this request.");
            widgets::field_label(ui, "Field (optional)");
            changed |= ui
                .add(
                    crate::widgets::text_edit(path)
                        .id(id.with("argument-path"))
                        .hint_text("e.g. items[0].id; empty = whole value")
                        .desired_width(f32::INFINITY),
                )
                .changed();
            changed
        }
        Some(Binding::Parameter { name, path }) => {
            let mut changed = name_editor(
                ui,
                id,
                name,
                context.parameters,
                "A scenario parameter or a component argument.",
            );
            widgets::field_label(ui, "Field (optional)");
            changed |= ui
                .add(
                    crate::widgets::text_edit(path)
                        .id(id.with("parameter-path"))
                        .hint_text("e.g. items[0].id; empty = whole value")
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                )
                .changed();
            changed
        }
        Some(Binding::Object { fields }) => object_editor(ui, context, fields, depth, id),
        Some(Binding::Array { items }) => list_editor(ui, context, items, depth, id),
        Some(Binding::Template { parts }) => {
            let changed = template_editor(ui, context, parts, depth, id);
            if !parts
                .iter()
                .any(|part| matches!(part, TemplatePart::Value { .. }))
            {
                // A template without values is plain text.
                let text = parts
                    .iter()
                    .map(|part| match part {
                        TemplatePart::Text { text } => text.as_str(),
                        TemplatePart::Value { .. } => "",
                    })
                    .collect();
                *value = Some(Binding::literal(Value::String(text)));
                return true;
            }
            changed
        }
        Some(Binding::Environment { name }) => {
            let environment = &context.project.environment;
            let names: Vec<String> = std::iter::once("baseUrl".to_owned())
                .chain(environment.variables.keys().cloned())
                .chain(environment.secret_names.iter().cloned())
                .collect();
            name_editor(
                ui,
                id,
                name,
                &names,
                "Secrets are only available in the current session.",
            )
        }
    }
}

fn literal_editor(
    ui: &mut Ui,
    context: &Context<'_>,
    port: &Port,
    literal: &mut Value,
    id: Id,
) -> bool {
    let mut changed = false;
    if context.node.kind.is_check()
        && apiforge_core::graph::predicate(context.node) == apiforge_core::model::Predicate::Type
        && port.name == "expected"
    {
        let mut text = literal.as_str().unwrap_or_default().to_owned();
        if fields::assertion_type_combo(ui, id.with("assert-type"), &mut text) {
            *literal = Value::String(text);
            changed = true;
        }
        return changed;
    }
    let mut value_type = fields::literal_type(literal);
    if port.value_type == ValueType::Any || value_type != port.value_type {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Type").size(11.0).color(theme::MUTED));
            if fields::type_combo(ui, id.with("literal-type"), &mut value_type) {
                *literal = fields::empty_literal(value_type);
                changed = true;
            }
        });
    }
    match literal {
        Value::String(text) => {
            changed |= ui
                .add(
                    crate::widgets::text_edit(text)
                        .id(id.with("text"))
                        .desired_width(f32::INFINITY),
                )
                .changed();
        }
        Value::Number(number) => {
            let key = id.with("number");
            let shown =
                apiforge_core::js::number_to_string(apiforge_core::js::number_as_f64(number));
            let mut text: String = ui
                .data_mut(|data| data.get_temp(key))
                .unwrap_or_else(|| shown.clone());
            let field = ui.add(
                crate::widgets::text_edit(&mut text)
                    .id(key)
                    .desired_width(f32::INFINITY),
            );
            let parsed = text
                .trim()
                .parse::<f64>()
                .ok()
                .and_then(apiforge_core::js::number_value);
            if field.changed()
                && let Some(parsed) = parsed.clone()
            {
                *literal = parsed;
                changed = true;
            }
            if parsed.is_none() {
                ui.colored_label(
                    theme::DANGER,
                    RichText::new("Enter a finite number.").size(11.0),
                );
            }
            if field.has_focus() {
                ui.data_mut(|data| data.insert_temp(key, text));
            } else {
                ui.data_mut(|data| data.remove::<String>(key));
            }
        }
        Value::Bool(flag) => {
            egui::ComboBox::from_id_salt(id.with("bool"))
                .selected_text(if *flag { "Yes (true)" } else { "No (false)" })
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    changed |= ui.selectable_value(flag, true, "Yes (true)").changed();
                    changed |= ui.selectable_value(flag, false, "No (false)").changed();
                });
        }
        Value::Null => {
            widgets::hint(ui, "An explicit null value.");
        }
        structured => {
            if let Some(next) = fields::json_editor(ui, id.with("json"), structured, 4) {
                if fields::literal_type(&next) == value_type {
                    *structured = next;
                    changed = true;
                } else {
                    ui.colored_label(
                        theme::DANGER,
                        RichText::new(format!(
                            "Enter {}.",
                            fields::value_type_label(value_type).to_lowercase()
                        ))
                        .size(11.0),
                    );
                }
            }
        }
    }
    changed
}

#[allow(clippy::too_many_arguments)]
fn expression_editor(
    ui: &mut Ui,
    context: &Context<'_>,
    port: &Port,
    operator: &mut Operator,
    inputs: &mut indexmap::IndexMap<String, Binding>,
    depth: usize,
    id: Id,
    unwrap: &mut bool,
) -> bool {
    let mut changed = false;
    let spec = operator.spec();
    let boolean = spec.output == ValueType::Boolean;
    egui::Frame::new()
        .fill(theme::OPERATOR.gamma_multiply(0.06))
        .stroke(Stroke::new(1.0, theme::OPERATOR.gamma_multiply(0.4)))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(spec.group.label().to_uppercase())
                        .size(11.0)
                        .color(theme::OPERATOR),
                );
                ui.label(
                    RichText::new(if boolean { "condition" } else { "value" })
                        .size(11.0)
                        .color(theme::MUTED),
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
                        *unwrap = true;
                    }
                });
            });
            let mut chosen = *operator;
            operator_combo(ui, id.with("operator"), &mut chosen);
            if chosen != *operator {
                let rebuilt = Binding::Expression {
                    operator: *operator,
                    inputs: inputs.clone(),
                }
                .with_operator(chosen);
                if let Binding::Expression {
                    operator: next,
                    inputs: slots,
                } = rebuilt
                {
                    *operator = next;
                    *inputs = slots;
                }
                changed = true;
            }
            for operand in operator.spec().operands {
                let mut slot = inputs.get(operand.name).cloned();
                let mut slot_port = Port::new(operand.name, operand.value_type)
                    .labelled(operand.label)
                    .required(operand.required);
                if let Some(default) = operand.default {
                    slot_port.default_value =
                        apiforge_core::format::OptionalJson::Present(default.to_value());
                }
                if editor(
                    ui,
                    context,
                    &slot_port,
                    &mut slot,
                    depth + 1,
                    id.with(operand.name),
                ) {
                    match slot {
                        Some(binding) => {
                            inputs.insert(operand.name.to_owned(), binding);
                        }
                        None => {
                            inputs.shift_remove(operand.name);
                        }
                    }
                    changed = true;
                }
            }
        });
    let output = operator.spec().output;
    if port.value_type != ValueType::Any && output != ValueType::Any && output != port.value_type {
        ui.colored_label(
            theme::DANGER,
            RichText::new(format!(
                "This operator returns {output}, but the argument needs {}.",
                port.value_type
            ))
            .size(11.0),
        );
    }
    changed
}

/// Template parts that keep the current text and append one value.
fn template_from(current: Option<Binding>) -> Vec<TemplatePart> {
    let mut parts = Vec::new();
    match current {
        Some(Binding::Literal {
            value: Value::String(text),
        }) if !text.is_empty() => parts.push(TemplatePart::Text { text }),
        Some(
            binding @ (Binding::Reference { .. }
            | Binding::Argument { .. }
            | Binding::Parameter { .. }
            | Binding::Environment { .. }
            | Binding::Expression { .. }),
        ) => {
            parts.push(TemplatePart::Value { binding });
            return parts;
        }
        _ => {}
    }
    parts.push(TemplatePart::Value {
        binding: Binding::literal(Value::String(String::new())),
    });
    parts
}

/// A row header with a remove button; returns whether removal was requested.
fn removable_row(ui: &mut Ui, title: impl Into<RichText>, enabled: bool) -> bool {
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.label(title.into().size(11.0).color(theme::MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            remove = ui
                .add_enabled(
                    enabled,
                    widgets::icon_button(Icon::Trash, "Remove")
                        .compact()
                        .danger(),
                )
                .clicked();
        });
    });
    remove
}

/// Edits a nested binding in place; `None` from the nested editor becomes null.
fn nested(
    ui: &mut Ui,
    context: &Context<'_>,
    label: &str,
    binding: &mut Binding,
    depth: usize,
    id: Id,
) -> bool {
    let mut slot = Some(binding.clone());
    let port = Port::new(label, ValueType::Any);
    if editor(ui, context, &port, &mut slot, depth + 1, id) {
        *binding = slot.unwrap_or_else(|| Binding::literal(Value::Null));
        return true;
    }
    false
}

fn object_editor(
    ui: &mut Ui,
    context: &Context<'_>,
    fields: &mut indexmap::IndexMap<String, Binding>,
    depth: usize,
    id: Id,
) -> bool {
    let mut changed = false;
    let mut removed = None;
    let mut renamed = None;
    for (index, (name, child)) in fields.iter_mut().enumerate() {
        let row = id.with(("field", index));
        ui.horizontal(|ui| {
            let mut draft = name.clone();
            let field = ui.add(
                crate::widgets::text_edit(&mut draft)
                    .id(row.with("name"))
                    .hint_text("Field name")
                    .desired_width(ui.available_width() - 34.0),
            );
            if field.changed() {
                renamed = Some((index, draft));
            }
            if ui
                .add(
                    widgets::icon_button(Icon::Trash, "Remove field")
                        .compact()
                        .danger(),
                )
                .clicked()
            {
                removed = Some(index);
            }
        });
        changed |= nested(ui, context, name, child, depth, row);
    }
    if let Some((index, next)) = renamed
        && !next.is_empty()
        && !fields.contains_key(&next)
        && let Some((_, child)) = fields.shift_remove_index(index)
    {
        fields.shift_insert(index, next, child);
        changed = true;
    }
    if let Some(index) = removed {
        fields.shift_remove_index(index);
        changed = true;
    }
    if ui
        .add(widgets::action(Icon::Plus, "Add field").quiet())
        .clicked()
    {
        let name = (1..)
            .map(|number| format!("field{number}"))
            .find(|name| !fields.contains_key(name))
            .unwrap_or_default();
        fields.insert(name, Binding::literal(Value::Null));
        changed = true;
    }
    changed
}

fn list_editor(
    ui: &mut Ui,
    context: &Context<'_>,
    items: &mut Vec<Binding>,
    depth: usize,
    id: Id,
) -> bool {
    let mut changed = false;
    let mut removed = None;
    for (index, item) in items.iter_mut().enumerate() {
        if removable_row(ui, format!("Item {}", index + 1), true) {
            removed = Some(index);
        }
        changed |= nested(
            ui,
            context,
            &format!("Item {}", index + 1),
            item,
            depth,
            id.with(("item", index)),
        );
    }
    if let Some(index) = removed {
        items.remove(index);
        changed = true;
    }
    if ui
        .add(widgets::action(Icon::Plus, "Add item").quiet())
        .clicked()
    {
        items.push(Binding::literal(Value::Null));
        changed = true;
    }
    changed
}

/// Edits the parts of a text template: literal text and interpolated values.
fn template_editor(
    ui: &mut Ui,
    context: &Context<'_>,
    parts: &mut Vec<TemplatePart>,
    depth: usize,
    id: Id,
) -> bool {
    let mut changed = false;
    let mut removed = None;
    let mut value_number = 0;
    for (index, part) in parts.iter_mut().enumerate() {
        let row = id.with(("part", index));
        match part {
            TemplatePart::Text { text } => {
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            crate::widgets::text_edit(text)
                                .id(row.with("text"))
                                .hint_text("Text")
                                .desired_width(ui.available_width() - 34.0),
                        )
                        .changed();
                    if ui
                        .add(
                            widgets::icon_button(Icon::Trash, "Remove text")
                                .compact()
                                .danger(),
                        )
                        .clicked()
                    {
                        removed = Some(index);
                    }
                });
            }
            TemplatePart::Value { binding } => {
                value_number += 1;
                let label = format!("Value {value_number}");
                if removable_row(ui, label.as_str(), true) {
                    removed = Some(index);
                }
                changed |= nested(ui, context, &label, binding, depth, row);
            }
        }
    }
    if let Some(index) = removed {
        parts.remove(index);
        changed = true;
    }
    ui.horizontal(|ui| {
        if ui
            .add(widgets::action(Icon::Plus, "Text").quiet())
            .clicked()
        {
            parts.push(TemplatePart::Text {
                text: String::new(),
            });
            changed = true;
        }
        if ui
            .add(widgets::action(Icon::Plus, "Value").quiet())
            .clicked()
        {
            parts.push(TemplatePart::Value {
                binding: Binding::literal(Value::String(String::new())),
            });
            changed = true;
        }
    });
    changed
}

/// Operators grouped by catalogue group.
pub fn operator_combo(ui: &mut Ui, id: Id, value: &mut Operator) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.label())
        .width(ui.available_width())
        .height(360.0)
        .show_ui(ui, |ui| {
            for group in OperatorGroup::ALL {
                ui.label(
                    RichText::new(group.label().to_uppercase())
                        .size(11.0)
                        .color(theme::MUTED),
                );
                for operator in Operator::ALL
                    .into_iter()
                    .filter(|operator| operator.spec().group == group)
                {
                    ui.selectable_value(value, operator, operator.label());
                }
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn reference_editor(
    ui: &mut Ui,
    context: &Context<'_>,
    port: &Port,
    node_id: &mut String,
    output: &mut String,
    field: &mut String,
    id: Id,
) -> bool {
    let mut changed = false;
    let candidates = sources(context);
    let source = context.graph.nodes.iter().find(|node| &node.id == node_id);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Step").size(11.0).color(theme::MUTED));
        egui::ComboBox::from_id_salt(id.with("source"))
            .selected_text(source.map_or("Unavailable source", |node| node.display_name()))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for candidate in &candidates {
                    if ui
                        .selectable_label(&candidate.id == node_id, candidate.display_name())
                        .clicked()
                        && &candidate.id != node_id
                    {
                        *node_id = candidate.id.clone();
                        *output = output_ports(candidate, context.project)
                            .first()
                            .map_or_else(|| "value".into(), |port| port.name.clone());
                        field.clear();
                        changed = true;
                    }
                }
            });
    });
    // Resolve again after the selection: output choices and response fields
    // must describe the source chosen in this frame, not the previous one.
    let source = context.graph.nodes.iter().find(|node| &node.id == node_id);
    let outputs = source
        .map(|source| output_ports(source, context.project))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Output").size(11.0).color(theme::MUTED));
        egui::ComboBox::from_id_salt(id.with("output"))
            .selected_text(
                outputs
                    .iter()
                    .find(|port| &port.name == output)
                    .map_or(output.as_str(), |port| port.display_name()),
            )
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for candidate in &outputs {
                    changed |= ui
                        .selectable_value(output, candidate.name.clone(), candidate.display_name())
                        .changed();
                }
            });
    });
    if let Some(source) = source {
        let data = response::source(source, output, context.project, context.run);
        let selected = data
            .value
            .as_ref()
            .and_then(|value| path::resolve(value, field).map(|value| value.into_owned()));
        let button = egui::Button::new(
            RichText::new(format!(
                "{}   ▾",
                if field.is_empty() {
                    format!("whole {output}")
                } else {
                    field.clone()
                }
            ))
            .monospace(),
        )
        .truncate()
        .min_size(egui::vec2(ui.available_width(), 26.0));
        widgets::field_label(ui, "Response field");
        if ui
            .add(button)
            .on_hover_text(format!("{field}\nChoose a field from the response"))
            .clicked()
        {
            ui.data_mut(|memory| {
                memory.insert_temp(id.with("picker"), String::new());
                memory.insert_temp(id.with("picker").with("focus-search"), true);
            });
        }
        let preview = selected
            .as_ref()
            .map(|value| {
                format!(
                    " · {}: {}",
                    response::type_label(apiforge_core::js::type_name(Some(value))),
                    crate::labels::literal_summary(value)
                )
            })
            .unwrap_or_default();
        widgets::hint(ui, format!("{}{preview}", data.origin));
        if let Some(path) = field_picker(ui.ctx(), id.with("picker"), &data, output) {
            *field = path;
            changed = true;
        }
    }
    egui::CollapsingHeader::new(RichText::new("Edit path manually").size(11.0))
        .id_salt(id.with("manual-path"))
        .show(ui, |ui| {
            changed |= ui
                .add(
                    crate::widgets::text_edit(field)
                        .hint_text("e.g. data.items[0].name; empty = whole value")
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                )
                .changed();
        });
    if field.is_empty()
        && let Some(output_port) = outputs.iter().find(|candidate| &candidate.name == output)
        && !compatible(output_port, port)
    {
        ui.colored_label(
            theme::DANGER,
            RichText::new(
                "The output type does not match this argument. Choose a field or another source.",
            )
            .size(11.0),
        );
    }
    changed
}

/// Filters `fields` by a case-insensitive substring over path, preview and type label.
pub(crate) fn filter_response_fields<'a>(
    fields: &'a [apiforge_core::edit::ResponseField],
    query: &str,
) -> Vec<&'a apiforge_core::edit::ResponseField> {
    let query = query.trim().to_lowercase();
    fields
        .iter()
        .filter(|field| {
            query.is_empty()
                || format!(
                    "{} {} {}",
                    field.path,
                    field.preview,
                    response::type_label(field.value_type)
                )
                .to_lowercase()
                .contains(&query)
        })
        .collect()
}

/// Renders the searchable list of response fields and returns `(chosen_path, visible_count)`.
pub(crate) fn searchable_field_list(
    ui: &mut Ui,
    search: &mut String,
    focus_search: bool,
    fields: &[apiforge_core::edit::ResponseField],
    has_source_value: bool,
    port: &str,
    max_height: f32,
) -> (Option<String>, usize) {
    let mut chosen = None;
    let field = ui.add(
        crate::widgets::text_edit(search)
            .hint_text("Search names, paths or values…")
            .desired_width(f32::INFINITY),
    );
    if focus_search {
        field.request_focus();
    }
    let visible = filter_response_fields(fields, search);
    let visible_count = visible.len();
    egui::ScrollArea::vertical()
        .max_height(max_height)
        .show_rows(ui, 46.0, visible_count, |ui, range| {
            for field in &visible[range] {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 46.0),
                    egui::Sense::click(),
                );
                if response.hovered() {
                    ui.painter().rect_filled(rect, 4.0, theme::BG);
                }
                let label = if field.path.is_empty() {
                    format!("Whole {port}")
                } else {
                    field.path.clone()
                };
                let summary = format!(
                    "{} · {}",
                    response::type_label(field.value_type),
                    field.preview
                );
                let a11y_label = format!("{label} — {summary}");
                response.widget_info(move || {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &a11y_label)
                });
                let inset = 8.0 + field.depth.min(6) as f32 * 8.0;
                // Two lines keep long JSON paths separate from their type
                // and value. Each row is clipped to its own hit target.
                let painter = ui.painter().with_clip_rect(rect.shrink(4.0));
                painter.text(
                    rect.left_top() + egui::vec2(inset, 5.0),
                    egui::Align2::LEFT_TOP,
                    &label,
                    egui::FontId::monospace(12.0),
                    theme::LINK,
                );
                painter.text(
                    rect.left_top() + egui::vec2(inset, 25.0),
                    egui::Align2::LEFT_TOP,
                    &summary,
                    egui::FontId::proportional(12.0),
                    theme::MUTED,
                );
                let response = response.on_hover_text(format!("{label}\n{}", field.preview));
                if response.clicked() {
                    chosen = Some(field.path.clone());
                }
            }
        });
    if visible.is_empty() {
        widgets::hint(
            ui,
            if !has_source_value {
                "Run the request or add a sample response. You can also type a path."
            } else {
                "No matching field. Change the search or type a path."
            },
        );
    }
    (chosen, visible_count)
}

/// A modal list of response fields; returns the chosen path.
fn field_picker(
    ctx: &egui::Context,
    id: Id,
    source: &response::Source,
    port: &str,
) -> Option<String> {
    let mut search: String = ctx.data_mut(|data| data.get_temp(id))?;
    let mut chosen = None;
    let mut close = false;
    let fields = response::fields(source.value.as_ref());
    let focus_search =
        ctx.data_mut(|data| data.remove_temp::<bool>(id.with("focus-search"))) == Some(true);
    crate::dialogs::centered_modal(id.with("modal"), ctx).show(ctx, |ui| {
        ui.set_width(520.0_f32.min(ctx.content_rect().width() - 48.0));
        ui.label(
            RichText::new("Choose a response field")
                .size(16.0)
                .family(theme::bold_family()),
        );
        widgets::hint(ui, &source.origin);
        let (picked, visible_len) = searchable_field_list(
            ui,
            &mut search,
            focus_search,
            &fields.fields,
            source.value.is_some(),
            port,
            (ctx.content_rect().height() - 220.0).clamp(60.0, 380.0),
        );
        if fields.truncated { widgets::hint(ui, "Some fields are omitted by the 1000-field / 12-level limit. Type a path to select another field."); }
        chosen = picked;
        ui.separator();
        widgets::hint(ui, "The chosen field is read again on each run.");
        ui.horizontal(|ui| {
            widgets::hint(ui, format!("{visible_len} fields"));
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
    if chosen.is_some() || close {
        ctx.data_mut(|data| data.remove::<String>(id));
    } else {
        ctx.data_mut(|data| data.insert_temp(id, search));
    }
    chosen
}

fn name_editor(ui: &mut Ui, id: Id, name: &mut String, suggestions: &[String], hint: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui
            .add(
                crate::widgets::text_edit(name)
                    .id(id.with("name"))
                    .desired_width(ui.available_width() - 34.0),
            )
            .changed();
        egui::ComboBox::from_id_salt(id.with("names"))
            .selected_text("")
            .width(24.0)
            .show_ui(ui, |ui| {
                for suggestion in suggestions {
                    if ui
                        .selectable_label(suggestion == name, suggestion)
                        .clicked()
                    {
                        *name = suggestion.clone();
                        changed = true;
                    }
                }
            });
    });
    widgets::hint(ui, hint);
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_editor_fits_a_narrow_panel_without_changing_the_binding() {
        let workspace = apiforge_core::import_document(
            include_bytes!("../../../../fixtures/workspace.json"),
            &apiforge_core::ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        let project = &workspace.projects[0];
        let graph = &project.scenarios[0].graph;
        let node = graph
            .nodes
            .iter()
            .find(|node| node.kind.is_check())
            .unwrap();
        let port = Port::new("actual", ValueType::Any)
            .labelled("Actual value")
            .required(true);
        let original = node.inputs.get("actual").cloned();
        let mut value = original.clone();
        let context = egui::Context::default();
        theme::apply(&context);
        let bindings = Context {
            node,
            graph,
            project,
            parameters: &[],
            run: None,
            input: None,
        };
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let right = ui.max_rect().right();
                    assert!(!editor(
                        ui,
                        &bindings,
                        &port,
                        &mut value,
                        0,
                        Id::new("narrow")
                    ));
                    assert!(
                        ui.min_rect().right() <= right + 1.0,
                        "argument editor overflows the panel: {:?}",
                        ui.min_rect()
                    );
                });
            },
        );
        output.textures_delta.clear();
        assert_eq!(value, original);
    }

    #[test]
    fn response_field_filter_matches_path_preview_and_type_case_insensitively() {
        let sample = serde_json::json!({
            "product": {
                "id": "p-1",
                "price": 120,
                "active": true
            }
        });
        let fields = response::fields(Some(&sample));
        assert_eq!(
            filter_response_fields(&fields.fields, "").len(),
            fields.fields.len()
        );
        let price_matches = filter_response_fields(&fields.fields, "PRICE");
        assert!(
            price_matches
                .iter()
                .any(|field| field.path == "product.price")
        );
        let bool_matches = filter_response_fields(&fields.fields, "yes / no");
        assert!(
            bool_matches
                .iter()
                .any(|field| field.path == "product.active")
        );
        assert!(filter_response_fields(&fields.fields, "zzzz-missing").is_empty());
    }
}

#[cfg(test)]
mod request_argument_scope_tests {
    use super::*;
    #[test]
    fn nested_body_fields_can_select_arguments_with_the_same_name() {
        let project = apiforge_core::demos::empty_project();
        let mut node = apiforge_core::graph::create_node(
            apiforge_core::model::NodeKind::Request,
            apiforge_core::format::Position { x: 0.0, y: 0.0 },
            None,
        );
        node.inputs
            .insert("productId".into(), Binding::literal(serde_json::json!(123)));
        let graph = Graph {
            nodes: vec![node.clone()],
            edges: vec![],
            proxies: Vec::new(),
        };
        let context = Context {
            node: &node,
            graph: &graph,
            project: &project,
            parameters: &[],
            run: None,
            input: Some("body"),
        };
        // The nested field is also productId. Only the owning input participates
        // in self-reference exclusion, so the body can still select this source.
        assert_eq!(argument_names(&context), ["productId"]);
        assert!(
            argument_names(&Context {
                input: Some("productId"),
                ..context
            })
            .is_empty()
        );
    }
}
