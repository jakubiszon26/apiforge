//! Form fields of the inspector.

use crate::icons::Icon;
use crate::{dnd::Payload, theme, widgets};
use apiforge_core::{
    edit::FieldCheck,
    format::Binding,
    js::{json_stringify, type_name},
    model::ValueType,
    path::JsonPath,
    template_text::{Scope, parse_template, print_reference, print_template},
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

/// The language's words for the types of a value, in the order the window
/// lists them (DIAG-08): the words of `is "<type>"`.
pub const TYPE_WORDS: [&str; 6] = ["string", "number", "boolean", "object", "array", "null"];

/// A value type in the language's word: `any` where the type is not set.
pub fn value_type_label(value_type: ValueType) -> &'static str {
    value_type.as_str()
}

/// The type of a literal in the language's word: a literal of no base type
/// is `null` (see [`literal_type`]).
fn literal_type_label(value_type: ValueType) -> &'static str {
    match value_type {
        ValueType::Any => "null",
        other => other.as_str(),
    }
}

/// `word` with its article: *a number*, *an object*.
pub fn with_article(word: &str) -> String {
    match word.chars().next() {
        Some('a' | 'e' | 'i' | 'o' | 'u') => format!("an {word}"),
        _ => format!("a {word}"),
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
    labelled_type_combo(ui, id, value, value_type_label)
}

/// The type of a literal: `null`, then the five base types (DIAG-08).
pub fn literal_type_combo(ui: &mut Ui, id: Id, value: &mut ValueType) -> bool {
    labelled_type_combo(ui, id, value, literal_type_label)
}

fn labelled_type_combo(
    ui: &mut Ui,
    id: Id,
    value: &mut ValueType,
    label: fn(ValueType) -> &'static str,
) -> bool {
    let before = *value;
    let combo = egui::ComboBox::from_id_salt(id)
        .selected_text(label(*value))
        .width(ui.available_width().min(160.0))
        .show_ui(ui, |ui| {
            for option in VALUE_TYPES {
                ui.selectable_value(value, option, label(option));
            }
        });
    widgets::named(combo.response, "Value type");
    *value != before
}

/// The expected JSON type of a "has type" comparison, in the language's
/// words; "any" would not test anything.
pub fn assertion_type_combo(ui: &mut Ui, id: Id, value: &mut String) -> bool {
    let before = value.clone();
    let selected = TYPE_WORDS
        .into_iter()
        .find(|name| name == value)
        .unwrap_or("Choose a type");
    let combo = egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for name in TYPE_WORDS {
                ui.selectable_value(value, name.to_owned(), name);
            }
        });
    widgets::named(combo.response, "Expected type");
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

/// Forgets the text the template and body fields keep between frames. Needed
/// when the owner of their unfinished text replaces it, as a discard does:
/// otherwise a field would show and register its old text again.
pub fn forget_field_text(context: &egui::Context) {
    context.data_mut(|data| {
        data.remove_by_type::<TemplateDraft>();
        data.remove_by_type::<BodyDraft>();
    });
}

/// The template text field's draft while it is focused or invalid.
#[derive(Clone, Default)]
struct TemplateDraft {
    source: Option<Binding>,
    text: String,
    error: Option<String>,
}

/// A one-line field for text with `{{reference}}` tokens, e.g. a URL or a header
/// value. Dropping a variable inserts its reference at the caret. Returns the new
/// binding when the text changed and parses; invalid text stays in the field with
/// its error and changes nothing.
///
/// A binding whose values are not plain references cannot be shown as text; the
/// field then says so and returns `None`.
/// A one-line field holding a template, with completion inside its `{{ }}`.
pub fn template_field(
    ui: &mut Ui,
    id: Id,
    name: &str,
    binding: &Binding,
    scope: Scope<'_>,
    hint: &str,
    completion: apiforge_script::complete::Scope<'_>,
) -> Option<Binding> {
    let Some(printed) = print_template(binding, scope) else {
        widgets::hint(ui, "Calculated value — edit it as an argument below.");
        return None;
    };
    let mut draft: TemplateDraft =
        ui.data_mut(|data| data.get_temp(id))
            .unwrap_or_else(|| TemplateDraft {
                source: Some(binding.clone()),
                text: printed.clone(),
                error: None,
            });
    if draft.source.as_ref() != Some(binding) {
        draft = TemplateDraft {
            source: Some(binding.clone()),
            text: printed.clone(),
            error: None,
        };
    }
    let recovered = super::drafts::text(ui, id, &printed);
    let conflict = recovered.as_ref().is_some_and(|(_, conflict)| *conflict);
    let mut restored = false;
    if let Some((text, _)) = recovered {
        restored = draft.text != text || draft.error.is_none();
        draft.text = text;
    }
    // The popup takes its keys before the field sees them, so Enter chooses a
    // completion instead of committing a half-typed name.
    let mut completions: crate::code::Completions = ui
        .data_mut(|data| data.get_temp(id.with("completions")))
        .unwrap_or_default();
    let keys = completions.take_keys(ui, id.with("text"));
    completions.prepare(ui, id.with("text"), &draft.text);
    let output = crate::widgets::text_edit(&mut draft.text)
        .id(id.with("text"))
        .hint_text(hint)
        .interactive(!conflict)
        .font(egui::TextStyle::Monospace)
        .desired_width(f32::INFINITY)
        .show(ui);
    widgets::named(output.response.response.clone(), name);
    let response = output.response.clone();
    let mut changed = response.changed();
    changed |= completions.update(
        ui,
        id.with("text"),
        &mut draft.text,
        &crate::code::Context {
            mode: apiforge_script::complete::Mode::Template,
            scope: completion,
            diagnostics: &[],
            line_numbers: false,
            wrap: true,
            font_size: 12.0,
            plain: false,
            interactive: true,
            changed: None,
        },
        &output,
        keys,
    );
    ui.data_mut(|data| data.insert_temp(id.with("completions"), completions));
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
        && let Payload::Variable { binding, .. } = payload.as_ref()
        && let Some(reference) = print_reference(binding, scope)
    {
        let length = draft.text.chars().count();
        let (start, end) = output.cursor_range.map_or((length, length), |range| {
            let (a, b) = (range.primary.index, range.secondary.index);
            (a.min(b).into(), a.max(b).into())
        });
        let byte = |offset: usize| {
            draft
                .text
                .char_indices()
                .nth(offset)
                .map_or(draft.text.len(), |(index, _)| index)
        };
        let (start, end) = (byte(start), byte(end));
        draft
            .text
            .replace_range(start..end, &format!("{{{{{reference}}}}}"));
        changed = true;
    }
    let mut result = None;
    if (changed || restored) && !conflict {
        match parse_template(&draft.text, scope) {
            Ok(next) => {
                draft.error = None;
                draft.source = Some(next.clone());
                result = (next != *binding).then_some(next);
            }
            Err(error) => draft.error = Some(error.to_string()),
        }
    }
    if let Some(error) = &draft.error {
        ui.colored_label(theme::DANGER, RichText::new(error).size(11.0));
    }
    super::drafts::store(
        ui,
        id,
        &printed,
        &draft.text,
        conflict || draft.error.is_some(),
    );
    if response.has_focus() || draft.error.is_some() || conflict {
        ui.data_mut(|data| data.insert_temp(id, draft));
    } else {
        ui.data_mut(|data| data.remove::<TemplateDraft>(id));
    }
    result
}

/// Edits an object of text fields, such as request headers or query parameters.
/// Returns the new binding when it changed; `Some(None)` when the last field was
/// removed.
pub fn template_map(
    ui: &mut Ui,
    id: Id,
    label: &str,
    binding: Option<&Binding>,
    scope: Scope<'_>,
    completion: apiforge_script::complete::Scope<'_>,
) -> Option<Option<Binding>> {
    let mut fields = match binding {
        Some(Binding::Object { fields }) => fields.clone(),
        _ => IndexMap::new(),
    };
    let mut changed = false;
    ui.label(RichText::new(label).strong());
    let mut remove = None;
    for (key, value) in fields.iter_mut() {
        ui.horizontal(|ui| {
            ui.add_sized(
                [ui.available_width() - 30.0, 22.0],
                egui::Label::new(RichText::new(key.as_str()).monospace()).truncate(),
            )
            .on_hover_text(key.as_str());
            if ui
                .add(
                    widgets::icon_button(Icon::Trash, "Remove")
                        .compact()
                        .danger(),
                )
                .clicked()
            {
                remove = Some(key.clone());
            }
        });
        if let Some(next) = template_field(
            ui,
            id.with(("value", key.as_str())),
            &format!("{label}: {key}"),
            value,
            scope,
            "Value or {{block.field}}",
            apiforge_script::complete::Scope {
                handles: completion.handles,
                environment: completion.environment,
                parameters: completion.parameters,
                inputs: completion.inputs,
                components: completion.components,
                shapes: completion.shapes,
            },
        ) {
            *value = next;
            changed = true;
        }
        ui.add_space(3.0);
    }
    if let Some(key) = remove {
        fields.shift_remove(&key);
        changed = true;
    }
    let new_id = id.with("new");
    let mut name: String = ui
        .data_mut(|data| data.get_temp(new_id))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let field = widgets::named(
            ui.add(
                crate::widgets::text_edit(&mut name)
                    .hint_text("Name")
                    .desired_width(ui.available_width() - 80.0),
            ),
            format!("New name in {label}"),
        );
        let trimmed = name.trim().to_owned();
        let valid = !trimmed.is_empty() && !fields.contains_key(&trimmed);
        let submit = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if ui
            .add_enabled(valid, widgets::action(Icon::Plus, "Add"))
            .clicked()
            || (valid && submit)
        {
            fields.insert(trimmed, Binding::literal(Value::String(String::new())));
            name.clear();
            changed = true;
        }
    });
    ui.data_mut(|data| data.insert_temp(new_id, name));
    changed.then(|| (!fields.is_empty()).then_some(Binding::Object { fields }))
}

/// What the user chose in the output tree.
pub enum TreeAction {
    Check { path: String, check: FieldCheck },
    Assign { path: String },
}

/// The menu *Add a check* that `button` opens on a field showing `value`
/// (DIAG-08): *equals ‹value›*, *is a ‹type›*, *exists*, *is not empty*,
/// each read as a sentence about `subject` before it is written. The one
/// place that draws it, for the Inspector's tree and Run results alike.
pub fn check_menu(button: &egui::Response, subject: &str, value: &Value) -> Option<FieldCheck> {
    let mut chosen = None;
    egui::Popup::menu(button).show(|ui| {
        // Escape closes the menu and leaves the selection under it.
        if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            ui.close();
        }
        widgets::name_menu(ui, "Add a check menu");
        ui.set_min_width(200.0);
        widgets::caption(ui, "ADD A CHECK");
        for check in FieldCheck::offered(value) {
            if ui.button(check_sentence(subject, &check)).clicked() {
                chosen = Some(check);
                ui.close();
            }
        }
    });
    chosen
}

/// A check of `subject` as the menu reads it: *total is a number*.
fn check_sentence(subject: &str, check: &FieldCheck) -> String {
    let subject = crate::labels::truncate(subject, 48);
    match check {
        FieldCheck::Equals(value) => format!(
            "{subject} equals {}",
            crate::labels::truncate(&json_stringify(value), 40)
        ),
        FieldCheck::Type(word) if word == "null" => format!("{subject} is null"),
        FieldCheck::Type(word) => format!("{subject} is {}", with_article(word)),
        FieldCheck::Exists => format!("{subject} exists"),
        FieldCheck::NotEmpty => format!("{subject} is not empty"),
    }
}

/// A JSON value as a tree of fields, one row a field (DIAG-10): its fold
/// mark, its own name, its value as the language writes it and coloured by
/// its type, and ✓ (*Add a check*) and {} (*Assign to variable*). The first
/// row is the output itself, named `heading`. Field names can be dragged
/// onto arguments as a reference to `node`'s `port`. `read_only` says why
/// the actions are disabled, on their hover (DIAG-09).
pub fn json_tree(
    ui: &mut Ui,
    node: &str,
    port: &str,
    heading: &str,
    value: &Value,
    read_only: Option<&str>,
) -> Option<TreeAction> {
    let tree = Tree {
        node,
        port,
        read_only,
    };
    let mut action = None;
    let mut budget = JSON_ROWS_PER_FRAME;
    let spacing = ui.spacing().item_spacing.y;
    ui.spacing_mut().item_spacing.y = TREE_ROW_GAP;
    tree_row(
        ui,
        &tree,
        value,
        &JsonPath::root(),
        heading,
        0,
        true,
        &mut action,
        &mut budget,
    );
    ui.spacing_mut().item_spacing.y = spacing;
    if budget == 0 {
        widgets::hint(
            ui,
            "Field display limit reached. Collapse a branch to inspect other fields.",
        );
    }
    action
}

/// What every row of one tree shares.
struct Tree<'a> {
    node: &'a str,
    port: &'a str,
    read_only: Option<&'a str>,
}

const JSON_ROWS_PER_FRAME: usize = 400;
const JSON_PAGE_SIZE: usize = 100;

/// A row's height: one line, the height of ✓ and {}.
const TREE_ROW: f32 = 26.0;
/// The space between two rows.
const TREE_ROW_GAP: f32 = 2.0;
/// How far a child's row starts right of its parent's, at every level.
const TREE_INDENT: f32 = 14.0;
/// The width of the fold mark.
const TREE_FOLD: f32 = 14.0;
/// The space between a row's parts.
const TREE_GAP: f32 = 8.0;
/// ✓ and {} are two compact buttons 2 px apart.
const TREE_BUTTON: f32 = 26.0;
const TREE_ACTIONS: f32 = 2.0 * TREE_BUTTON + 2.0;
/// Names and values sit a point above the row's middle, so that the middle
/// of their small letters meets the middle of ✓ and {}.
const TREE_TEXT_LIFT: f32 = 1.0;

fn page_range(total: usize, page: usize) -> std::ops::Range<usize> {
    let start = page.min(total.saturating_sub(1) / JSON_PAGE_SIZE) * JSON_PAGE_SIZE;
    start..(start + JSON_PAGE_SIZE).min(total)
}

/// A value as its row writes it: a scalar as the language writes it, an
/// object or a list as its type and size.
fn tree_preview(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("array · {}", items.len()),
        Value::Object(fields) => format!("object · {}", fields.len()),
        scalar => json_stringify(scalar),
    }
}

/// A value's colour by its type.
fn tree_colour(value: &Value) -> egui::Color32 {
    match value {
        Value::String(_) => theme::VALUE_STRING,
        Value::Number(_) => theme::VALUE_NUMBER,
        Value::Bool(_) | Value::Null => theme::VALUE_LITERAL,
        Value::Array(_) | Value::Object(_) => theme::MUTED,
    }
}

/// What `add` adds, laid out exactly over `rect` of `ui`.
fn put_at(
    ui: &mut Ui,
    rect: egui::Rect,
    add: impl FnOnce(&mut Ui) -> egui::Response,
) -> egui::Response {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
        add,
    )
    .inner
}

#[allow(clippy::too_many_arguments)]
fn tree_row(
    ui: &mut Ui,
    tree: &Tree<'_>,
    value: &Value,
    path: &JsonPath,
    name: &str,
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
    let child_count = match value {
        Value::Array(items) => items.len(),
        Value::Object(fields) => fields.len(),
        _ => 0,
    };
    // Kept per field of the step's output, not per frame of the panel: a
    // branch opened stays open while another step is selected.
    let state_id = Id::new(("json-branch", tree.node, tree.port, &text));
    let mut open = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(state_id))
        .unwrap_or(depth == 0);
    let whole_id = state_id.with("whole");
    let whole = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(whole_id))
        .unwrap_or(false);

    // Where the row's parts go: the name keeps what it needs up to most of
    // the room, the value takes the rest up to ✓ and {}.
    let width = ui.available_width();
    let left = ui.cursor().min.x;
    let fold_x = left + depth as f32 * TREE_INDENT;
    let name_x = fold_x + TREE_FOLD + TREE_GAP;
    let actions_x = (left + width - TREE_ACTIONS).max(name_x);
    let room = (actions_x - TREE_GAP - name_x).max(0.0);
    let name_text = if depth == 0 {
        RichText::new(name)
            .size(theme::FONT_BODY)
            .color(theme::TEXT)
    } else {
        RichText::new(name)
            .monospace()
            .size(theme::FONT_LABEL)
            .color(theme::LINK)
    };
    let name_galley = egui::WidgetText::from(name_text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (room * 0.6).max(24.0),
        egui::FontSelection::Default,
    );
    let value_x = name_x + name_galley.size().x + TREE_GAP;
    let value_room = (actions_x - TREE_GAP - value_x).max(24.0);
    let value_text = RichText::new(tree_preview(value))
        .monospace()
        .size(theme::FONT_LABEL)
        .color(tree_colour(value));
    let cut_galley = egui::WidgetText::from(value_text.clone()).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        value_room,
        egui::FontSelection::Default,
    );
    let line = cut_galley.size().y;
    // A value cut at the panel's edge opens over as many lines as it needs
    // on a click, and a second click cuts it again (DIAG-10, proposal 1).
    let cut = cut_galley.elided;
    let whole = whole && cut;
    let value_galley = if whole {
        egui::WidgetText::from(value_text).into_galley(
            ui,
            Some(egui::TextWrapMode::Wrap),
            value_room,
            egui::FontSelection::Default,
        )
    } else {
        cut_galley
    };
    let height = TREE_ROW.max(value_galley.size().y + TREE_ROW - line);
    let (row, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let middle = row.top() + TREE_ROW / 2.0;

    // A thin guide line through the row for each level it is nested in,
    // under the fold mark of that level's row.
    for level in 0..depth {
        let x = left + level as f32 * TREE_INDENT + TREE_FOLD / 2.0;
        ui.painter().line_segment(
            [
                egui::pos2(x, row.top() - TREE_ROW_GAP),
                egui::pos2(x, row.bottom()),
            ],
            egui::Stroke::new(1.0, theme::BORDER),
        );
    }

    let mut row_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(state_id)
            .max_rect(row)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let row_ui = &mut row_ui;

    if child_count > 0 {
        let mark = egui::Rect::from_center_size(
            egui::pos2(fold_x + TREE_FOLD / 2.0, middle),
            egui::vec2(TREE_FOLD, 20.0),
        );
        let label = if open {
            "Collapse field"
        } else {
            "Expand field"
        };
        let toggle = row_ui.interact(mark, state_id.with("fold"), egui::Sense::click());
        toggle.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        row_ui.painter().text(
            mark.center(),
            egui::Align2::CENTER_CENTER,
            if open { "▾" } else { "▸" },
            egui::FontId::proportional(theme::FONT_LABEL),
            if toggle.hovered() {
                theme::TEXT
            } else {
                theme::MUTED
            },
        );
        if toggle.on_hover_text(label).clicked() {
            open = !open;
            row_ui
                .ctx()
                .data_mut(|data| data.insert_temp(state_id, open));
        }
    }

    let shown = if text.is_empty() { tree.port } else { &text };
    let kind = type_name(Some(value));
    let name_rect = egui::Rect::from_min_size(
        egui::pos2(name_x, middle - TREE_TEXT_LIFT - name_galley.size().y / 2.0),
        name_galley.size(),
    );
    let field = put_at(row_ui, name_rect, |ui| {
        ui.add(
            egui::Label::new(name_galley)
                .sense(egui::Sense::click_and_drag())
                .show_tooltip_when_elided(false),
        )
    });
    if supported {
        field.dnd_set_drag_payload(Payload::Variable {
            label: shown.to_owned(),
            binding: Binding::reference(tree.node, tree.port, text.clone()),
        });
    }
    field.on_hover_text(format!("{shown}\n{kind} · drag onto an argument"));

    let value_rect = egui::Rect::from_min_size(
        egui::pos2(value_x, middle - TREE_TEXT_LIFT - line / 2.0),
        value_galley.size(),
    );
    let shown_value = put_at(row_ui, value_rect, |ui| {
        ui.add(egui::Label::new(value_galley).sense(if cut {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        }))
    });
    if cut
        && shown_value
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    {
        row_ui
            .ctx()
            .data_mut(|data| data.insert_temp(whole_id, !whole));
    }

    // ✓ and {}, on every row, on its first line: beside the name when the
    // value is open over several. Where the step takes no edit both are
    // disabled, and say why (DIAG-09).
    let why = |response: egui::Response| match tree.read_only {
        Some(why) => response.on_disabled_hover_text(why),
        None => response,
    };
    let button = |x: f32| {
        egui::Rect::from_center_size(
            egui::pos2(x + TREE_BUTTON / 2.0, middle),
            egui::vec2(TREE_BUTTON, TREE_BUTTON),
        )
    };
    // The ✓ opens the menu Add a check (DIAG-08).
    let check = why(put_at(row_ui, button(actions_x), |ui| {
        ui.add_enabled(
            supported,
            widgets::icon_button(Icon::Check, "Add a check").compact(),
        )
    }));
    if let Some(check) = check_menu(&check, shown, value) {
        *action = Some(TreeAction::Check {
            path: text.clone(),
            check,
        });
    }
    let variable = why(put_at(
        row_ui,
        button(actions_x + TREE_BUTTON + 2.0),
        |ui| {
            ui.add_enabled(
                supported,
                widgets::icon_button(Icon::Variable, "Assign to variable").compact(),
            )
        },
    ));
    if variable.clicked() {
        *action = Some(TreeAction::Assign { path: text.clone() });
    }

    if !open || child_count == 0 {
        return;
    }
    // What stands among the children starts where their names do.
    let inset = (depth + 1) as f32 * TREE_INDENT + TREE_FOLD + TREE_GAP;
    if depth >= 10 {
        ui.horizontal(|ui| {
            ui.add_space(inset);
            widgets::hint(ui, "Nested deeper than 10 levels.");
        });
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
            ui.add_space(inset);
            if ui
                .add_enabled(
                    range.start > 0,
                    widgets::icon_button(Icon::ArrowLeft, "Previous 100 fields").compact(),
                )
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
                    widgets::icon_button(Icon::ChevronRight, "Next 100 fields").compact(),
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
                    tree,
                    item,
                    &child,
                    &format!("[{index}]"),
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
                    tree,
                    item,
                    &child,
                    key,
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
        let tree = Tree {
            node: "n",
            port: "body",
            read_only: None,
        };
        let mut output = context.run_ui(Default::default(), |ui| {
            let mut budget = JSON_ROWS_PER_FRAME;
            let mut action = None;
            tree_row(
                ui,
                &tree,
                &value,
                &JsonPath::root(),
                "Response",
                0,
                true,
                &mut action,
                &mut budget,
            );
            visited = JSON_ROWS_PER_FRAME - budget;
            let mut budget = 1;
            tree_row(
                ui,
                &tree,
                &value,
                &JsonPath::root(),
                "Response",
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

/// Request bodies accept input.* in the same text the user sends. Invalid edits
/// remain drafts; the last valid binding stays in the document and run snapshot.
pub fn request_body_editor(
    ui: &mut Ui,
    id: Id,
    binding: Option<&Binding>,
    format: apiforge_core::model::BodyFormat,
    scope: Scope<'_>,
    completion: apiforge_script::complete::Scope<'_>,
) -> Option<Option<Binding>> {
    use apiforge_core::{body_text, model::BodyFormat};
    let mut draft: BodyDraft = ui.data_mut(|data| data.get_temp(id)).unwrap_or_default();
    // Source changes (undo, another editor, switching formats) invalidate a draft.
    // Cache the projection: large bodies are not printed/parsed every paint.
    if draft.source.as_ref().is_none_or(|(source, source_format)| {
        source.as_ref() != binding || *source_format != format
    }) {
        let printed = match binding {
            None => Some(String::new()),
            Some(binding) => match format {
                BodyFormat::Json => body_text::print_json_body(binding, scope),
                BodyFormat::Text => body_text::print_text_body(binding, scope),
            },
        };
        draft = BodyDraft {
            source: Some((binding.cloned(), format)),
            editable: printed.is_some(),
            printed: printed.clone().unwrap_or_default(),
            text: printed.unwrap_or_default(),
            error: None,
        };
    }
    let source = draft.printed.clone();
    let recovered = super::drafts::text(ui, id, &source);
    let conflict = recovered.as_ref().is_some_and(|(_, conflict)| *conflict);
    let mut restored = false;
    if let Some((text, _)) = recovered {
        restored = draft.text != text || draft.error.is_none();
        draft.text = text;
    }
    if !draft.editable {
        ui.data_mut(|data| data.insert_temp(id, draft));
        widgets::hint(
            ui,
            "This body uses a calculated value. Edit it with Body value source below.",
        );
        return None;
    }
    // The body is code: it is coloured, it completes, and it says what a name
    // means, in the mode its format implies.
    let mut editor: crate::code::CodeEditor = ui
        .data_mut(|data| data.get_temp(id.with("editor")))
        .unwrap_or_default();
    let outcome = ui
        .add_enabled_ui(!conflict, |ui| {
            editor.show(
                ui,
                id.with("text"),
                &mut draft.text,
                &crate::code::Context {
                    // Both projections parse references inside {{...}} placeholders.
                    mode: apiforge_script::complete::Mode::Template,
                    scope: completion,
                    diagnostics: &[],
                    line_numbers: false,
                    wrap: true,
                    font_size: 12.0,
                    plain: false,
                    interactive: true,
                    changed: None,
                },
            )
        })
        .inner;
    widgets::named(outcome.response.clone(), "Request body");
    ui.data_mut(|data| data.insert_temp(id.with("editor"), editor));
    let mut result = None;
    if (outcome.changed || restored) && !conflict {
        match draft.parse(format, scope) {
            Ok(next) => {
                draft.error = None;
                // Keep the user's spacing while they continue typing.
                draft.printed = match next.as_ref() {
                    None => String::new(),
                    Some(binding) => match format {
                        BodyFormat::Json => body_text::print_json_body(binding, scope),
                        BodyFormat::Text => body_text::print_text_body(binding, scope),
                    }
                    .unwrap_or_default(),
                };
                draft.source = Some((next.clone(), format));
                if next.as_ref() != binding {
                    result = Some(next);
                }
            }
            Err(message) => draft.error = Some(message),
        }
    }
    if let Some(error) = &draft.error {
        ui.colored_label(theme::DANGER, RichText::new(error).size(11.0));
        // Also on the field itself, which stays in view when the body is long.
        outcome.response.on_hover_text(error.clone());
    }
    widgets::hint(
        ui,
        if format == BodyFormat::Json {
            "Use {{param.name}}, {{block.field}} or {{input.name}} as a JSON value (quotes are optional). A whole placeholder keeps its JSON type; text around it produces a string. Empty = no body."
        } else {
            "Insert request arguments as {{input.name}}. Empty = no body."
        },
    );
    super::drafts::store(
        ui,
        id,
        &source,
        &draft.text,
        conflict || draft.error.is_some(),
    );
    ui.data_mut(|data| data.insert_temp(id, draft));
    result
}

#[derive(Clone, Default)]
struct BodyDraft {
    editable: bool,
    printed: String,
    source: Option<(Option<Binding>, apiforge_core::model::BodyFormat)>,
    text: String,
    error: Option<String>,
}
impl BodyDraft {
    fn parse(
        &self,
        format: apiforge_core::model::BodyFormat,
        scope: Scope<'_>,
    ) -> Result<Option<Binding>, String> {
        use apiforge_core::{body_text, model::BodyFormat};
        // Whitespace can be an intentional text body.
        if self.text.is_empty() || (format == BodyFormat::Json && self.text.trim().is_empty()) {
            return Ok(None);
        }
        match format {
            BodyFormat::Json => body_text::parse_json_body(&self.text, scope),
            BodyFormat::Text => body_text::parse_text_body(&self.text, scope),
        }
        .map(Some)
        .map_err(|error| format!("{} — the last valid body is kept.", error.message))
    }
}

#[cfg(test)]
mod body_editor_tests {
    use super::*;
    use apiforge_core::{
        demos,
        format::Position,
        graph,
        model::{BodyFormat, NodeKind},
    };

    #[test]
    fn typing_body_json_resolves_input_arguments_and_invalid_drafts_keep_the_last_body() {
        let project = demos::empty_project();
        let mut request = graph::create_node(NodeKind::Request, Position { x: 0.0, y: 0.0 }, None);
        request
            .inputs
            .insert("productId".into(), Binding::literal(serde_json::json!(123)));
        let scope = Scope {
            nodes: &[],
            project: &project,
            request: Some(&request),
        };
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let id = Id::new("body-input-test");
        let mut value = None;
        let mut render = |events: Vec<egui::Event>| {
            let mut result = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(300.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        result = request_body_editor(
                            ui,
                            id,
                            value.as_ref(),
                            BodyFormat::Json,
                            scope,
                            apiforge_script::complete::Scope::default(),
                        );
                    });
                },
            );
            output.textures_delta.clear();
            if let Some(next) = &result {
                value = next.clone();
            }
            result
        };
        assert!(render(vec![]).is_none());
        ctx.memory_mut(|memory| memory.request_focus(id.with("text")));
        let result = render(vec![egui::Event::Text(
            r#"{"productId": "{{input.productId}}"}"#.into(),
        )]);
        let Some(Some(Binding::Object { fields })) = result else {
            panic!("body input did not produce bindings")
        };
        assert_eq!(
            fields["productId"],
            Binding::Argument {
                name: "productId".into(),
                path: String::new()
            }
        );
        assert!(render(vec![egui::Event::Text(",".into())]).is_none());
        assert!(render(vec![]).is_none());
        drop(render);
        let draft = ctx.data_mut(|data| data.get_temp::<BodyDraft>(id)).unwrap();
        assert!(draft.text.ends_with(','));
        assert!(draft.error.is_some());
        assert!(matches!(value, Some(Binding::Object { .. })));
    }
}

#[cfg(test)]
mod draft_regressions {
    use super::*;
    use crate::inspector::drafts;
    use apiforge_core::{demos, model::BodyFormat};

    fn owner() -> crate::buffers::Context {
        crate::buffers::Context::Scenario {
            project: "test".into(),
            id: "scenario".into(),
        }
    }

    fn render(ctx: &egui::Context, events: Vec<egui::Event>, url: &str) {
        let project = demos::empty_project();
        let scope = Scope {
            nodes: &[],
            project: &project,
            request: None,
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                drafts::set_owner(ui.ctx(), &owner());
                assert!(
                    template_field(
                        ui,
                        Id::new("url-draft"),
                        "URL",
                        &Binding::literal(Value::String(url.into())),
                        scope,
                        "URL",
                        Default::default()
                    )
                    .is_none()
                );
                assert!(
                    request_body_editor(
                        ui,
                        Id::new("body-draft"),
                        None,
                        BodyFormat::Json,
                        scope,
                        Default::default()
                    )
                    .is_none()
                );
            },
        );
        output.textures_delta.clear();
    }

    #[test]
    fn invalid_url_and_body_survive_storage_and_rehydrate_in_a_new_ui() {
        let ctx = egui::Context::default();
        render(&ctx, vec![], "");
        ctx.memory_mut(|m| m.request_focus(Id::new("url-draft").with("text")));
        render(
            &ctx,
            vec![egui::Event::Paste("/unfinished/{{missing}}".into())],
            "",
        );
        ctx.memory_mut(|m| m.request_focus(Id::new("body-draft").with("text")));
        render(
            &ctx,
            vec![egui::Event::Paste(
                "{\"note\": \"unfinished\", \"quantity\":".into(),
            )],
            "",
        );
        let entries = drafts::entries(&ctx);
        assert_eq!(entries.len(), 2);
        let dir = tempfile::tempdir().unwrap();
        let store =
            apiforge_storage::WorkspaceStore::open_exclusive(dir.path().join("workspace.json"))
                .unwrap();
        store
            .save_drafts(
                &apiforge_storage::Drafts {
                    version: 1,
                    entries,
                },
                &[],
            )
            .unwrap();
        let loaded = store.load_drafts().unwrap().unwrap().drafts;
        let next = egui::Context::default();
        drafts::restore(&next, Some(&loaded));
        render(&next, vec![], "");
        assert_eq!(
            next.data_mut(|d| d.get_temp::<TemplateDraft>(Id::new("url-draft")))
                .unwrap()
                .text,
            "/unfinished/{{missing}}"
        );
        let body = next
            .data_mut(|d| d.get_temp::<BodyDraft>(Id::new("body-draft")))
            .unwrap();
        assert!(body.text.contains("unfinished"));
        assert!(body.error.is_some());
        // External edits expose a conflict without overwriting either version.
        render(&next, vec![], "/updated");
        assert_eq!(drafts::entries(&next).len(), 2);
        assert!(
            next.data_mut(|d| d.get_temp::<TemplateDraft>(Id::new("url-draft")))
                .unwrap()
                .text
                .contains("missing")
        );
    }

    #[test]
    fn json_body_completion_only_offers_references_inside_placeholders() {
        let parameters = vec!["quantity".into()];
        let scope = apiforge_script::complete::Scope {
            parameters: &parameters,
            ..Default::default()
        };
        let bare = "{\"quantity\": param.";
        assert!(
            apiforge_script::complete::complete(&apiforge_script::complete::Request {
                source: bare,
                cursor: bare.len(),
                mode: apiforge_script::complete::Mode::Template,
                scope,
            })
            .is_empty()
        );
        let source = "{\"quantity\": {{param.}}}";
        let found = apiforge_script::complete::complete(&apiforge_script::complete::Request {
            source,
            cursor: source.find(".}").unwrap() + 1,
            mode: apiforge_script::complete::Mode::Template,
            scope: apiforge_script::complete::Scope {
                parameters: &parameters,
                ..Default::default()
            },
        });
        let completion = found.iter().find(|c| c.label == "quantity").unwrap();
        let mut text = source.to_owned();
        text.replace_range(completion.replace.clone(), &completion.insert);
        let project = demos::empty_project();
        let mut project = project;
        project.scenarios[0]
            .parameters
            .insert("quantity".into(), Value::from(3));
        let parsed = BodyDraft {
            text,
            ..Default::default()
        }
        .parse(
            BodyFormat::Json,
            Scope {
                nodes: &[],
                project: &project,
                request: None,
            },
        );
        assert!(parsed.is_ok(), "{parsed:?}");
    }
}
