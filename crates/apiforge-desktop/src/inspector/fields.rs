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
        crate::widgets::text_area(&mut draft.text)
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
    let output = crate::widgets::text_edit(text)
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
                [ui.available_width() - 30.0, 22.0],
                egui::Label::new(RichText::new(key).monospace()).truncate(),
            )
            .on_hover_text(key);
            if ui
                .small_button("✕")
                .on_hover_text(format!("Remove {key}"))
                .clicked()
            {
                remove = Some(key.clone());
            }
        });
        let event = variable_text(ui, id.with(("value", key)), value, "Value or {{argument}}");
        changed |= event.changed;
        if let Some((variable, start, end)) = event.dropped {
            dropped = Some((key.clone(), variable, start, end));
        }
        ui.add_space(3.0);
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
            crate::widgets::text_edit(&mut name)
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
    let mut budget = JSON_ROWS_PER_FRAME;
    tree_row(
        ui,
        node,
        port,
        value,
        &JsonPath::root(),
        0,
        true,
        &mut action,
        &mut budget,
    );
    if budget == 0 {
        widgets::hint(
            ui,
            "Field display limit reached. Collapse a branch to inspect other fields.",
        );
    }
    action
}

const JSON_ROWS_PER_FRAME: usize = 400;
const JSON_PAGE_SIZE: usize = 100;

fn page_range(total: usize, page: usize) -> std::ops::Range<usize> {
    let start = page.min(total.saturating_sub(1) / JSON_PAGE_SIZE) * JSON_PAGE_SIZE;
    start..(start + JSON_PAGE_SIZE).min(total)
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
    budget: &mut usize,
) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    let text = path.to_string();
    let preview = match value {
        Value::Array(items) => format!("List · {}", items.len()),
        Value::Object(fields) => format!("Object · {}", fields.len()),
        scalar => crate::labels::truncate(&json_stringify(scalar), 60),
    };
    let child_count = match value {
        Value::Array(items) => items.len(),
        Value::Object(fields) => fields.len(),
        _ => 0,
    };
    let state_id = ui.id().with(("json-branch", node, port, &text));
    let mut open = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(state_id))
        .unwrap_or(depth == 0);
    ui.horizontal(|ui| {
        ui.add_space(depth.min(4) as f32 * 8.0);
        if child_count > 0
            && ui
                .small_button(if open { "▾" } else { "▸" })
                .on_hover_text(if open {
                    "Collapse field"
                } else {
                    "Expand field"
                })
                .clicked()
        {
            open = !open;
            ui.ctx().data_mut(|data| data.insert_temp(state_id, open));
        }

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
            "{label}\n{} · drag onto an argument",
            type_name(Some(value))
        ));
        let _ = field;
    });
    ui.horizontal(|ui| {
        // Actions have a fixed budget; a long value gets ellipsised rather than
        // pushing the buttons outside the inspector.
        ui.add_sized(
            [(ui.available_width() - 142.0).max(24.0), 22.0],
            egui::Label::new(RichText::new(&preview).size(11.0).color(theme::MUTED)).truncate(),
        )
        .on_hover_text(&preview);
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
    ui.add_space(3.0);
    if !open || child_count == 0 {
        return;
    }
    if depth >= 10 {
        if matches!(value, Value::Array(items) if !items.is_empty())
            || matches!(value, Value::Object(fields) if !fields.is_empty())
        {
            widgets::hint(ui, "Nested deeper than 10 levels.");
        }
        return;
    }
    let page_key = state_id.with("page");
    let mut page = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(page_key))
        .unwrap_or(0);
    let range = page_range(child_count, page);
    // A newer response may be shorter than the previous page selection.
    page = range.start / JSON_PAGE_SIZE;
    if child_count > JSON_PAGE_SIZE {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(range.start > 0, egui::Button::new("Previous 100").small())
                .clicked()
            {
                page = page.saturating_sub(1);
            }
            widgets::hint(
                ui,
                format!("{}–{} of {}", range.start + 1, range.end, child_count),
            );
            if ui
                .add_enabled(
                    range.end < child_count,
                    egui::Button::new("Next 100").small(),
                )
                .clicked()
            {
                page += 1;
            }
        });
        ui.ctx().data_mut(|data| data.insert_temp(page_key, page));
    }
    let reserved = |key: &str| ["__proto__", "constructor", "prototype"].contains(&key);
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate().take(range.end).skip(range.start) {
                if *budget == 0 {
                    break;
                }
                let mut child = path.clone();
                child.push_index(index);
                tree_row(
                    ui,
                    node,
                    port,
                    item,
                    &child,
                    depth + 1,
                    supported,
                    action,
                    budget,
                );
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields.iter().take(range.end).skip(range.start) {
                if *budget == 0 {
                    break;
                }
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
                    budget,
                );
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

#[cfg(test)]
mod tree_tests {
    use super::*;

    #[test]
    fn pagination_covers_the_tail_and_clamps_after_the_source_shrinks() {
        assert_eq!(page_range(250, 0), 0..100);
        assert_eq!(page_range(250, 1), 100..200);
        assert_eq!(page_range(250, 2), 200..250);
        assert_eq!(page_range(20, 100), 0..20);
        assert_eq!(page_range(0, 0), 0..0);
    }

    #[test]
    fn json_tree_only_visits_open_branches_and_honors_its_frame_budget() {
        let context = egui::Context::default();
        let value = serde_json::json!({"nested": vec![serde_json::json!({"x": [1,2,3]}); 1000]});
        let mut visited = 0;
        let mut limited = 0;
        let mut output = context.run_ui(Default::default(), |ui| {
            let mut budget = JSON_ROWS_PER_FRAME;
            let mut action = None;
            tree_row(
                ui,
                "n",
                "body",
                &value,
                &JsonPath::root(),
                0,
                true,
                &mut action,
                &mut budget,
            );
            visited = JSON_ROWS_PER_FRAME - budget;
            let mut budget = 1;
            tree_row(
                ui,
                "n",
                "body",
                &value,
                &JsonPath::root(),
                0,
                true,
                &mut action,
                &mut budget,
            );
            limited = budget;
        });
        output.textures_delta.clear();
        assert_eq!(
            visited, 2,
            "root and nested summary, without visiting the closed subtree"
        );
        assert_eq!(limited, 0);
    }
}
