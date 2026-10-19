//! Form fields of the inspector.

use crate::{dnd::Payload, theme, widgets};
use apiforge_core::{
    edit::{TextSelection, VariableBlock},
    format::Binding,
    js::{json_stringify, type_name},
    model::ValueType,
    path::JsonPath,
};
use eframe::egui::{self, Id, RichText, Ui};
use indexmap::IndexMap;
use serde_json::Value;

pub const VALUE_TYPES: [ValueType; 6] = [
    ValueType::Any,
    ValueType::String,
    ValueType::Number,
    ValueType::Boolean,
    ValueType::Object,
    ValueType::Array,
];

pub fn value_type_label(value_type: ValueType) -> &'static str {
    match value_type {
        ValueType::Any => "Any data",
        ValueType::String => "Text",
        ValueType::Number => "Number",
        ValueType::Boolean => "Yes / no",
        ValueType::Object => "Object",
        ValueType::Array => "List",
    }
}

/// The base type a literal edits as; null edits as "any".
pub fn literal_type(value: &Value) -> ValueType {
    match value {
        Value::Null => ValueType::Any,
        Value::Bool(_) => ValueType::Boolean,
        Value::Number(_) => ValueType::Number,
        Value::String(_) => ValueType::String,
        Value::Array(_) => ValueType::Array,
        Value::Object(_) => ValueType::Object,
    }
}

/// An empty literal of a type.
pub fn empty_literal(value_type: ValueType) -> Value {
    match value_type {
        ValueType::Any => Value::Null,
        ValueType::String => Value::String(String::new()),
        ValueType::Number => Value::from(0),
        ValueType::Boolean => Value::Bool(false),
        ValueType::Object => Value::Object(Default::default()),
        ValueType::Array => Value::Array(vec![]),
    }
}

pub fn type_combo(ui: &mut Ui, id: Id, value: &mut ValueType) -> bool {
    let before = *value;
    egui::ComboBox::from_id_salt(id)
        .selected_text(value_type_label(*value))
        .width(ui.available_width().min(160.0))
        .show_ui(ui, |ui| {
            for option in VALUE_TYPES {
                ui.selectable_value(value, option, value_type_label(option));
            }
        });
    *value != before
}

/// The expected JSON type of a "has type" comparison; "any" would not test anything.
pub fn assertion_type_combo(ui: &mut Ui, id: Id, value: &mut String) -> bool {
    let before = value.clone();
    let options = [
        ("string", "Text"),
        ("number", "Number"),
        ("boolean", "Yes / no"),
        ("object", "Object"),
        ("array", "List"),
        ("null", "Null"),
    ];
    let selected = options
        .iter()
        .find(|(name, _)| name == value)
        .map_or("Choose a type", |(_, label)| label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (name, label) in options {
                ui.selectable_value(value, name.to_owned(), label);
            }
        });
    *value != before
}

/// A multi-line JSON editor. Invalid text stays in the field with an error and the
/// last valid value is kept, so typing never corrupts the document.
pub fn json_editor(ui: &mut Ui, id: Id, value: &Value, rows: usize) -> Option<Value> {
    let serialised = serde_json::to_string_pretty(value).unwrap_or_default();
    #[derive(Clone, Default)]
    struct Draft {
        source: String,
        text: String,
        error: Option<String>,
    }
    let mut draft: Draft = ui.data_mut(|data| data.get_temp(id)).unwrap_or_default();
    if draft.source != serialised {
        draft = Draft {
            source: serialised.clone(),
            text: serialised,
            error: None,
        };
    }
    let response = ui.add(
        egui::TextEdit::multiline(&mut draft.text)
            .id(id.with("text"))
            .code_editor()
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
    let mut result = None;
    if response.changed() {
        match serde_json::from_str::<Value>(&draft.text) {
            Ok(parsed) => {
                draft.error = None;
                draft.source = serde_json::to_string_pretty(&parsed).unwrap_or_default();
                result = Some(parsed);
            }
            Err(error) => {
                draft.error = Some(format!(
                    "Invalid JSON at line {}, column {} — the last valid value is kept.",
                    error.line(),
                    error.column()
                ));
            }
        }
    }
    if let Some(error) = &draft.error {
        ui.colored_label(theme::DANGER, RichText::new(error).size(11.0));
    }
    ui.data_mut(|data| data.insert_temp(id, draft));
    result
}

/// What happened in a text field that accepts variables.
#[derive(Default)]
pub struct TextEvent {
    pub changed: bool,
    /// A variable dropped at the caret (character offsets).
    pub dropped: Option<(VariableBlock, usize, usize)>,
}

/// A single-line text field; dropping a variable inserts a `{{token}}` at the caret.
pub fn variable_text(ui: &mut Ui, id: Id, text: &mut String, hint: &str) -> TextEvent {
    let mut event = TextEvent::default();
    let output = egui::TextEdit::singleline(text)
        .id(id)
        .hint_text(hint)
        .desired_width(f32::INFINITY)
        .show(ui);
    let response = output.response;
    event.changed = response.changed();
    if response
        .dnd_hover_payload::<Payload>()
        .is_some_and(|payload| matches!(*payload, Payload::Variable { .. }))
    {
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            4.0,
            egui::Stroke::new(1.5, theme::ACCENT),
            egui::StrokeKind::Outside,
        );
    }
    if let Some(payload) = response.dnd_release_payload::<Payload>()
        && let Payload::Variable { label, binding } = payload.as_ref()
    {
        let length = text.chars().count();
        let (start, end) = output.cursor_range.map_or((length, length), |range| {
            let (a, b) = (range.primary.index, range.secondary.index);
            (a.min(b).into(), a.max(b).into())
        });
        event.dropped = Some((
            VariableBlock {
                label: label.clone(),
                binding: binding.clone(),
            },
            start,
            end,
        ));
    }
    event
}

pub fn selection(text: &str, start: usize, end: usize) -> TextSelection<'_> {
    TextSelection {
        value: text,
        start,
        end,
    }
}

pub type DroppedVariable = (String, VariableBlock, usize, usize);

/// Edits a string map (query parameters, headers). Returns the new map when it
/// changed, and a variable dropped into one of its values.
pub fn string_map(
    ui: &mut Ui,
    id: Id,
    label: &str,
    map: &IndexMap<String, String>,
) -> (Option<IndexMap<String, String>>, Option<DroppedVariable>) {
    let mut next = map.clone();
    let mut changed = false;
    let mut dropped = None;
    ui.label(RichText::new(label).strong());
    let mut remove = None;
    for (key, value) in next.iter_mut() {
        ui.horizontal(|ui| {
            ui.add_sized(
                [96.0, 22.0],
                egui::Label::new(RichText::new(key).monospace()).truncate(),
            )
            .on_hover_text(key);
            let field_width = ui.available_width() - 28.0;
            ui.allocate_ui(egui::vec2(field_width, 24.0), |ui| {
                let event =
                    variable_text(ui, id.with(("value", key)), value, "Value or {{argument}}");
                changed |= event.changed;
                if let Some((variable, start, end)) = event.dropped {
                    dropped = Some((key.clone(), variable, start, end));
                }
            });
            if ui
                .small_button("✕")
                .on_hover_text(format!("Remove {key}"))
                .clicked()
            {
                remove = Some(key.clone());
            }
        });
    }
    if let Some(key) = remove {
        next.shift_remove(&key);
        changed = true;
    }
    let new_id = id.with("new");
    let mut name: String = ui
        .data_mut(|data| data.get_temp(new_id))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let field = ui.add(
            egui::TextEdit::singleline(&mut name)
                .hint_text("Name")
                .desired_width(ui.available_width() - 60.0),
        );
        let trimmed = name.trim().to_owned();
        let valid = !trimmed.is_empty() && !next.contains_key(&trimmed);
        let submit = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if ui.add_enabled(valid, egui::Button::new("Add")).clicked() || (valid && submit) {
            next.insert(trimmed, String::new());
            name.clear();
            changed = true;
        }
    });
    ui.data_mut(|data| data.insert_temp(new_id, name));
    (changed.then_some(next), dropped)
}

/// What the user chose in the output tree.
pub enum TreeAction {
    Check { path: String, value: Value },
    Assign { path: String },
}

/// A JSON value as a tree of fields with "Check" and "To variable" actions. Field
/// names can be dragged onto arguments as a reference to `node`'s `port`.
pub fn json_tree(ui: &mut Ui, node: &str, port: &str, value: &Value) -> Option<TreeAction> {
    let mut action = None;
    tree_row(
        ui,
        node,
        port,
        value,
        &JsonPath::root(),
        0,
        true,
        &mut action,
    );
    action
}

#[allow(clippy::too_many_arguments)]
fn tree_row(
    ui: &mut Ui,
    node: &str,
    port: &str,
    value: &Value,
    path: &JsonPath,
    depth: usize,
    supported: bool,
    action: &mut Option<TreeAction>,
) {
    let text = path.to_string();
    let preview = match value {
        Value::Array(items) => format!("List · {}", items.len()),
        Value::Object(fields) => format!("Object · {}", fields.len()),
        scalar => crate::labels::truncate(&json_stringify(scalar), 60),
    };
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 12.0);
        let label = if text.is_empty() {
            "$".to_owned()
        } else {
            text.clone()
        };
        let field = ui.add(
            egui::Label::new(RichText::new(&label).monospace().color(theme::LINK))
                .sense(egui::Sense::click_and_drag())
                .truncate(),
        );
        if supported {
            field.dnd_set_drag_payload(Payload::Variable {
                label: if text.is_empty() {
                    port.to_owned()
                } else {
                    text.clone()
                },
                binding: Binding::reference(node, port, text.clone()),
            });
        }
        let field = field.on_hover_text(format!(
            "{} · drag onto an argument",
            type_name(Some(value))
        ));
        let _ = field;
        ui.label(RichText::new(preview).size(11.0).color(theme::MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(supported, egui::Button::new("To variable").small())
                .on_hover_text("Assign this field to a variable")
                .clicked()
            {
                *action = Some(TreeAction::Assign { path: text.clone() });
            }
            if ui
                .add_enabled(supported, egui::Button::new("Check").small())
                .on_hover_text(if supported {
                    "Add a comparison for this field"
                } else {
                    "Reserved field name; compare the parent object instead"
                })
                .clicked()
            {
                *action = Some(TreeAction::Check {
                    path: text.clone(),
                    value: value.clone(),
                });
            }
        });
    });
    if depth >= 10 {
        return;
    }
    let reserved = |key: &str| ["__proto__", "constructor", "prototype"].contains(&key);
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().take(100).enumerate() {
                let mut child = path.clone();
                child.push_index(index);
                tree_row(ui, node, port, item, &child, depth + 1, supported, action);
            }
            if items.len() > 100 {
                widgets::hint(ui, "Showing the first 100 items.");
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields.iter().take(100) {
                let mut child = path.clone();
                child.push_key(key.clone());
                tree_row(
                    ui,
                    node,
                    port,
                    item,
                    &child,
                    depth + 1,
                    supported && !reserved(key),
                    action,
                );
            }
            if fields.len() > 100 {
                widgets::hint(ui, "Showing the first 100 fields.");
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn literal_types_round_trip_through_empty_values() {
        for value_type in VALUE_TYPES {
            assert_eq!(literal_type(&empty_literal(value_type)), value_type);
        }
        assert_eq!(literal_type(&json!(null)), ValueType::Any);
    }
}
