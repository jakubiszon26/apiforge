//! Form fields of the inspector.

use super::marks::{Marks, RowMark, TreeMarks};
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
    Check {
        path: String,
        check: FieldCheck,
    },
    Assign {
        path: String,
    },
    /// A check of the field, chosen in the menu of its ✓ (DIAG-12).
    Select {
        check: String,
    },
}

/// What was chosen in the menu of ✓.
pub enum MenuChoice {
    /// A check to add, from *Add a check* (DIAG-08).
    Add(FieldCheck),
    /// A check of the field to select, from the list above it (DIAG-12).
    Select(String),
}

/// The menu *Add a check* that `button` opens on a field showing `value`
/// (DIAG-08): *equals ‹value›*, *is a ‹type›*, *exists*, *is not empty*,
/// each read as a sentence about `subject` before it is written. The one
/// place that draws it, for the Inspector's tree and Run results alike.
// The Checks tab's *Add check* opens it on a field no tree draws (DIAG-15).
#[allow(dead_code)]
pub fn check_menu(button: &egui::Response, subject: &str, value: &Value) -> Option<FieldCheck> {
    match check_menu_with(button, subject, value, &[]) {
        Some(MenuChoice::Add(check)) => Some(check),
        _ => None,
    }
}

/// [`check_menu`] on a field that `listed` checks already read: the menu
/// lists them first, as DIAG-02 writes them and in Run results with their
/// outcome, and a click selects one; *Add a check* follows (DIAG-12).
pub fn check_menu_with(
    button: &egui::Response,
    subject: &str,
    value: &Value,
    listed: &[super::marks::Listed],
) -> Option<MenuChoice> {
    let mut chosen = None;
    egui::Popup::menu(button).show(|ui| {
        // Escape closes the menu and leaves the selection under it.
        if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            ui.close();
        }
        widgets::name_menu(ui, "Add a check menu");
        ui.set_min_width(200.0);
        if !listed.is_empty() {
            widgets::caption(
                ui,
                if listed.len() == 1 {
                    "CHECK OF THIS FIELD"
                } else {
                    "CHECKS OF THIS FIELD"
                },
            );
            for check in listed {
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    &crate::labels::truncate(&check.text, 60),
                    0.0,
                    egui::TextFormat::simple(
                        egui::FontId::monospace(theme::FONT_LABEL),
                        theme::LINK,
                    ),
                );
                if let Some((word, colour)) = check.outcome {
                    job.append(
                        word,
                        12.0,
                        egui::TextFormat::simple(
                            egui::FontId::proportional(theme::FONT_META),
                            colour,
                        ),
                    );
                }
                let item = widgets::named(ui.button(job), check.text.as_str());
                if item.on_hover_text("Select this check").clicked() {
                    chosen = Some(MenuChoice::Select(check.id.clone()));
                    ui.close();
                }
            }
            ui.separator();
        }
        widgets::caption(ui, "ADD A CHECK");
        for check in FieldCheck::offered(value) {
            if ui.button(check_sentence(subject, &check)).clicked() {
                chosen = Some(MenuChoice::Add(check));
                ui.close();
            }
        }
    });
    chosen
}

/// The action a choice in the menu of the field at `path` makes.
fn menu_action(choice: MenuChoice, path: &str) -> TreeAction {
    match choice {
        MenuChoice::Add(check) => TreeAction::Check {
            path: path.to_owned(),
            check,
        },
        MenuChoice::Select(check) => TreeAction::Select { check },
    }
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
/// the actions are disabled, on their hover (DIAG-09). What is folded or
/// opened is kept per field of `document`. A field a check reads carries
/// the mark of `marks` (DIAG-12).
#[allow(clippy::too_many_arguments)]
pub fn json_tree(
    ui: &mut Ui,
    document: Id,
    node: &str,
    port: &str,
    heading: &str,
    value: &Value,
    read_only: Option<&str>,
    marks: Option<Marks<'_>>,
) -> Option<TreeAction> {
    draw_tree(
        ui,
        Tree {
            document,
            node,
            port,
            read_only,
            step: TREE_INDENT,
            results: None,
            marks: marks.and_then(|marks| TreeMarks::new(marks, node, port)),
        },
        heading,
        value,
    )
}

/// How Run results' rows of a value open before one is folded or opened by
/// hand (REP-08); in the Inspector the first row opens, the others are
/// folded (DIAG-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opening {
    /// The rows above the level given open, the others folded: Run results'
    /// for a large value (REP-08).
    Levels(usize),
    /// Every row open: Run results' (REP-08).
    Leaves,
    /// Every row folded: Run results' after *Collapse all* (REP-08).
    Closed,
}

impl Opening {
    /// Whether a row `depth` levels under the first opens.
    pub fn open(self, depth: usize) -> bool {
        match self {
            Self::Levels(levels) => depth < levels,
            Self::Leaves => true,
            Self::Closed => false,
        }
    }

    /// Whether every row `value` shows opened this way, on the first page
    /// of each branch, is drawn in one frame (review round, 2026-11-23).
    pub fn drawn_whole(self, value: &Value) -> bool {
        fn count(value: &Value, opening: Opening, depth: usize, left: &mut usize) -> bool {
            if *left == 0 {
                return false;
            }
            *left -= 1;
            if depth >= 10 || !opening.open(depth) {
                return true;
            }
            match value {
                Value::Array(items) => items
                    .iter()
                    .take(JSON_PAGE_SIZE)
                    .all(|item| count(item, opening, depth + 1, left)),
                Value::Object(fields) => fields
                    .values()
                    .take(JSON_PAGE_SIZE)
                    .all(|item| count(item, opening, depth + 1, left)),
                _ => true,
            }
        }
        // The 400th row drawn already reads *Field display limit reached*.
        let mut left = JSON_ROWS_PER_FRAME - 1;
        count(value, self, 0, &mut left)
    }
}

/// Where a Run results tree lists what it kept in egui's memory: its rows'
/// folding, pages and values opened whole.
fn kept_id(document: Id) -> Id {
    document.with("kept")
}

/// Frees what the Run results tree `document` kept in egui's memory, once
/// its run or its choice of *Expand all* or *Collapse all* is over.
pub fn forget_results_tree(ctx: &egui::Context, document: Id) {
    ctx.data_mut(|data| {
        for id in data
            .remove_temp::<Vec<Id>>(kept_id(document))
            .unwrap_or_default()
        {
            data.remove::<bool>(id);
            data.remove::<usize>(id);
        }
    });
}

/// What Run results' rows do that the Inspector's do not (REP-08 on
/// DIAG-10's row): they open by `opening`; *Check* and *To variable* keep
/// their words where the panel has room and follow the value; a field
/// checked from these results reads *Check added* (UX-01).
#[derive(Clone, Copy)]
pub struct Results<'a> {
    pub opening: Opening,
    /// The rows name fields of the step's outputs, which can be checked.
    pub fields: bool,
    /// *Check* and *To variable* are offered, and names can be dragged: the
    /// document can be edited.
    pub offers: bool,
    /// Whether the field at a path was checked from these results.
    pub checked: &'a dyn Fn(&str) -> bool,
    /// The checks of the document, which mark the fields they read in the
    /// colour of their outcome (DIAG-12).
    pub marks: Option<Marks<'a>>,
}

/// [`json_tree`]'s rows in Run results: `value` of `node`'s `port`, its
/// first row named by the port.
pub fn results_tree(
    ui: &mut Ui,
    document: Id,
    node: &str,
    port: &str,
    value: &Value,
    results: Results<'_>,
) -> Option<TreeAction> {
    draw_tree(
        ui,
        Tree {
            document,
            node,
            port,
            read_only: None,
            step: TREE_INDENT,
            results: Some(results),
            marks: results
                .marks
                .filter(|_| results.fields)
                .and_then(|marks| TreeMarks::new(marks, node, port)),
        },
        port,
        value,
    )
}

fn draw_tree(ui: &mut Ui, mut tree: Tree<'_>, heading: &str, value: &Value) -> Option<TreeAction> {
    // 14 px a level wherever the panel has room; where the deepest row shown
    // would leave its name and value less than they need before ✓ and {},
    // every level steps in by the same smaller amount.
    let mut walked = JSON_ROWS_PER_FRAME;
    let deepest = deepest(ui.ctx(), &tree, value, &JsonPath::root(), 0, &mut walked);
    if deepest > 0 {
        let end = End::widest(ui, &tree);
        let spare = ui.available_width() - end - TREE_GAP - TREE_TEXT_MIN - TREE_GAP - TREE_FOLD;
        tree.step = TREE_INDENT.min(spare.max(0.0) / deepest as f32);
    }
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
    document: Id,
    node: &'a str,
    port: &'a str,
    read_only: Option<&'a str>,
    /// How far a child's row starts right of its parent's.
    step: f32,
    /// Run results' rows (REP-08); `None` in the Inspector.
    results: Option<Results<'a>>,
    /// The checks that read this output's fields (DIAG-12).
    marks: Option<TreeMarks<'a>>,
}

impl Tree<'_> {
    /// Where the field at `path` keeps whether it is open, per field of the
    /// step's output in its document, not per frame of the panel: a branch
    /// opened stays open while another step is selected.
    fn branch(&self, path: &str) -> Id {
        self.document
            .with(("json-branch", self.node, self.port, path))
    }

    /// Keeps `value` at `id`; Run results also list `id`, so that it is
    /// freed with its run ([`forget_results_tree`]).
    fn keep<T: Clone + Send + Sync + 'static>(&self, ctx: &egui::Context, id: Id, value: T) {
        ctx.data_mut(|data| {
            data.insert_temp(id, value);
            if self.results.is_some() {
                let kept = data.get_temp_mut_or_default::<Vec<Id>>(kept_id(self.document));
                if !kept.contains(&id) {
                    kept.push(id);
                }
            }
        });
    }

    /// Whether a row `depth` levels under the first opens before it is
    /// folded or opened by hand.
    fn opens(&self, depth: usize) -> bool {
        self.results
            .map_or(depth == 0, |results| results.opening.open(depth))
    }
}

const JSON_ROWS_PER_FRAME: usize = 400;
const JSON_PAGE_SIZE: usize = 100;

/// A row's height: one line, the height of ✓ and {}.
const TREE_ROW: f32 = 26.0;
/// The space between two rows.
const TREE_ROW_GAP: f32 = 2.0;
/// How far a child's row starts right of its parent's, at every level the
/// panel has room for.
const TREE_INDENT: f32 = 14.0;
/// What a row's name and value keep at least before ✓ and {}: the indent
/// gives way before they do.
const TREE_TEXT_MIN: f32 = 64.0;
/// The most of a value a row lays out on its one line: more than the widest
/// panel shows.
const TREE_CUT_CHARS: usize = 200;
/// The most of a value a row lays out opened, or shows in its hover.
const TREE_WHOLE_CHARS: usize = 4000;
/// The width of the fold mark.
const TREE_FOLD: f32 = 14.0;
/// The space between a row's parts.
const TREE_GAP: f32 = 8.0;
/// ✓ and {} are two compact buttons 2 px apart.
pub(super) const TREE_BUTTON: f32 = 26.0;
const TREE_ACTIONS: f32 = 2.0 * TREE_BUTTON + 2.0;
/// The space between Run results' *Check* and *To variable*.
const TREE_WORD_GAP: f32 = 4.0;
/// Names and values sit a point above the row's middle, so that the middle
/// of their small letters meets the middle of ✓ and {}.
const TREE_TEXT_LIFT: f32 = 1.0;

fn page_range(total: usize, page: usize) -> std::ops::Range<usize> {
    let start = page.min(total.saturating_sub(1) / JSON_PAGE_SIZE) * JSON_PAGE_SIZE;
    start..(start + JSON_PAGE_SIZE).min(total)
}

/// A value as its row writes it: a scalar as the language writes it, at
/// most `limit` characters and an ellipsis, an object or a list as its type
/// and size. A huge text is cut before it is written, not after.
fn tree_preview(value: &Value, limit: usize) -> String {
    match value {
        Value::Array(items) => format!("array · {}", items.len()),
        Value::Object(fields) => format!("object · {}", fields.len()),
        Value::String(text) if text.len() > limit => {
            let head: String = text.chars().take(limit).collect();
            crate::labels::truncate(&json_stringify(&Value::String(head)), limit)
        }
        scalar => crate::labels::truncate(&json_stringify(scalar), limit),
    }
}

/// The deepest level among the rows `value` shows from `depth`: its open
/// branches on their shown page, to ten levels and within `budget` rows.
fn deepest(
    ctx: &egui::Context,
    tree: &Tree<'_>,
    value: &Value,
    path: &JsonPath,
    depth: usize,
    budget: &mut usize,
) -> usize {
    if *budget == 0 {
        return depth;
    }
    *budget -= 1;
    let count = match value {
        Value::Array(items) => items.len(),
        Value::Object(fields) => fields.len(),
        _ => 0,
    };
    if count == 0 || depth >= 10 {
        return depth;
    }
    let id = tree.branch(&path.to_string());
    let (open, page) = ctx.data(|data| {
        (
            data.get_temp::<bool>(id)
                .unwrap_or_else(|| tree.opens(depth)),
            data.get_temp::<usize>(id.with("page")).unwrap_or(0),
        )
    });
    if !open {
        return depth;
    }
    let range = page_range(count, page);
    let mut found = depth;
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate().take(range.end).skip(range.start) {
                let mut child = path.clone();
                child.push_index(index);
                found = found.max(deepest(ctx, tree, item, &child, depth + 1, budget));
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields.iter().take(range.end).skip(range.start) {
                let mut child = path.clone();
                child.push_key(key.clone());
                found = found.max(deepest(ctx, tree, item, &child, depth + 1, budget));
            }
        }
        _ => {}
    }
    found
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

/// ✓ and {} of the Inspector's row, at its right edge (DIAG-10).
#[allow(clippy::too_many_arguments)]
fn inspector_actions(
    row_ui: &mut Ui,
    tree: &Tree<'_>,
    actions_x: f32,
    middle: f32,
    shown: &str,
    value: &Value,
    text: &str,
    supported: bool,
    mark: Option<&RowMark>,
    action: &mut Option<TreeAction>,
) {
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
    // The ✓ opens the menu Add a check (DIAG-08); filled where a check
    // reads the field, it names the checks and its menu lists them first
    // (DIAG-12).
    let check = match mark {
        Some(mark) => {
            let check = put_at(row_ui, button(actions_x), |ui| {
                ui.add_enabled_ui(supported, |ui| {
                    super::marks::button(ui, "Add a check", mark, false, TREE_BUTTON)
                })
                .inner
            });
            match tree.read_only {
                Some(why) => check.on_disabled_hover_text(format!("{}\n\n{why}", mark.said)),
                None => check.on_hover_text(&mark.said),
            }
        }
        None => why(put_at(row_ui, button(actions_x), |ui| {
            ui.add_enabled(
                supported,
                widgets::icon_button(Icon::Check, "Add a check").compact(),
            )
        })),
    };
    let listed = mark.map_or(&[][..], |mark| &mark.listed);
    if let Some(choice) = check_menu_with(&check, shown, value, listed) {
        *action = Some(menu_action(choice, text));
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
        *action = Some(TreeAction::Assign {
            path: text.to_owned(),
        });
    }
}

/// How a row of Run results ends (REP-08 on DIAG-10's row, UX-01).
enum End {
    /// The Inspector's ✓ and {}.
    Column,
    /// *Check* and *To variable* in words.
    Words { check: f32, variable: f32 },
    /// *Check* and *To variable* as icons, where the words have no room.
    Icons,
    /// *Check added*: the field was checked from these results.
    Added(f32),
    /// A reserved name, which a check cannot read.
    Custom(f32),
    /// Nothing: a value that is not a step's output, or results the
    /// document no longer takes edits from.
    Nothing,
}

const CHECK_ADDED: &str = "Check added";
const CUSTOM_KEY: &str = "custom key";

/// The width `text` takes at `size`.
pub(super) fn text_width(ui: &Ui, text: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(size),
            theme::TEXT,
        )
        .size()
        .x
}

impl End {
    /// The widest end a row of `tree` takes where its words have no room:
    /// what its indent leaves room for.
    fn widest(ui: &Ui, tree: &Tree<'_>) -> f32 {
        let Some(results) = tree.results else {
            return TREE_ACTIONS;
        };
        if !results.fields {
            return 0.0;
        }
        let added = Self::added(ui);
        if results.offers {
            added.max(TREE_ACTIONS).max(Self::custom(ui))
        } else {
            added
        }
    }

    fn added(ui: &Ui) -> f32 {
        14.0 + 4.0 + text_width(ui, CHECK_ADDED, theme::FONT_LABEL)
    }

    fn custom(ui: &Ui) -> f32 {
        text_width(ui, CUSTOM_KEY, 11.0)
    }

    /// The end of the row of the field at `path`, which leaves `room` from
    /// its name to the panel's edge.
    fn of(
        ui: &Ui,
        tree: &Tree<'_>,
        path: &str,
        supported: bool,
        mark: Option<&RowMark>,
        room: f32,
    ) -> Self {
        let Some(results) = tree.results else {
            return Self::Column;
        };
        // A compact action button's natural width.
        let word = |label: &str| 20.0 + 14.0 + 6.0 + text_width(ui, label, theme::FONT_LABEL);
        if !results.fields {
            Self::Nothing
        } else if supported && (results.checked)(path) {
            Self::Added(Self::added(ui))
        } else if !results.offers {
            Self::Nothing
        } else if !supported {
            Self::Custom(Self::custom(ui))
        } else {
            // A marked field's *Check* reads *2 checks* or *1 failed*
            // (DIAG-12).
            let check = mark.map_or_else(
                || word("Check"),
                |mark| super::marks::words_width(ui, &mark.words),
            );
            let variable = word("To variable");
            if room - TREE_TEXT_MIN - TREE_GAP >= check + TREE_WORD_GAP + variable {
                Self::Words { check, variable }
            } else {
                Self::Icons
            }
        }
    }

    fn width(&self) -> f32 {
        match *self {
            Self::Column | Self::Icons => TREE_ACTIONS,
            Self::Words { check, variable } => check + TREE_WORD_GAP + variable,
            Self::Added(width) | Self::Custom(width) => width,
            Self::Nothing => 0.0,
        }
    }

    /// Draws the end from `x` on the row whose middle is `middle`.
    #[allow(clippy::too_many_arguments)]
    fn show(
        &self,
        ui: &mut Ui,
        x: f32,
        middle: f32,
        shown: &str,
        value: &Value,
        path: &str,
        mark: Option<&RowMark>,
    ) -> Option<TreeAction> {
        let at = |x: f32, width: f32| {
            egui::Rect::from_min_size(
                egui::pos2(x, middle - TREE_BUTTON / 2.0),
                egui::vec2(width, TREE_BUTTON),
            )
        };
        let (check_width, variable_width, words) = match *self {
            Self::Words { check, variable } => (check, variable, true),
            Self::Icons => (TREE_BUTTON, TREE_BUTTON, false),
            Self::Added(width) => {
                // A check from these results waits for the next run; the
                // edit is undone like any other (UX-01).
                put_at(ui, at(x, width), |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let (icon, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        Icon::Check.paint(ui.painter(), icon, theme::SUCCESS);
                        ui.label(
                            RichText::new(CHECK_ADDED)
                                .size(theme::FONT_LABEL)
                                .color(theme::SUCCESS),
                        );
                    })
                    .response
                });
                return None;
            }
            Self::Custom(width) => {
                put_at(ui, at(x, width), |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.label(RichText::new(CUSTOM_KEY).size(11.0).color(theme::MUTED))
                            .on_hover_text("Reserved field name. Select the parent object instead.")
                    })
                    .inner
                });
                return None;
            }
            Self::Column | Self::Nothing => return None,
        };
        let button = |icon: Icon, label: &'static str| {
            if words {
                widgets::action(icon, label).compact().quiet()
            } else {
                widgets::icon_button(icon, label).compact()
            }
        };
        let mut chosen = None;
        // *Check* opens the menu Add a check (DIAG-08); on a marked field
        // it reads *2 checks* or *1 failed* in the outcome's colour, names
        // the checks and lists them first in its menu (DIAG-12).
        let check = match mark {
            Some(mark) => put_at(ui, at(x, check_width), |ui| {
                super::marks::button(ui, "Check", mark, words, check_width)
            })
            .on_hover_text(&mark.said),
            None => {
                let check_hover = format!("Check {shown}");
                put_at(ui, at(x, check_width), |ui| {
                    ui.add(button(Icon::Check, "Check").hover(&check_hover))
                })
            }
        };
        let listed = mark.map_or(&[][..], |mark| &mark.listed);
        if let Some(choice) = check_menu_with(&check, shown, value, listed) {
            chosen = Some(menu_action(choice, path));
        }
        let variable_hover = format!("Assign {shown} to a variable");
        let variable = put_at(
            ui,
            at(x + check_width + TREE_WORD_GAP, variable_width),
            |ui| ui.add(button(Icon::Variable, "To variable").hover(&variable_hover)),
        );
        if variable.clicked() {
            chosen = Some(TreeAction::Assign {
                path: path.to_owned(),
            });
        }
        chosen
    }
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
    let state_id = tree.branch(&text);
    let mut open = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(state_id))
        .unwrap_or_else(|| tree.opens(depth));
    let whole_id = state_id.with("whole");
    let whole = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(whole_id))
        .unwrap_or(false);

    // Where the row's parts go: ✓ and {} at the panel's right edge, always;
    // the name keeps what it needs up to most of the room left of them, the
    // value takes the rest. Both are cut where they do not fit. In Run
    // results the row ends in what [`End`] says, after the value and no
    // further right than the panel's edge (REP-08).
    let width = ui.available_width();
    let left = ui.cursor().min.x;
    let fold_x = left + depth as f32 * tree.step;
    let name_x = fold_x + TREE_FOLD + TREE_GAP;
    // The mark of a field a check reads, or of a folded one with checked
    // fields inside (DIAG-12).
    let mark = tree
        .marks
        .as_ref()
        .and_then(|marks| marks.row(&text, child_count > 0 && !open));
    let end = End::of(
        ui,
        tree,
        &text,
        supported,
        mark.as_ref(),
        width - (name_x - left),
    );
    let actions_x = left
        + (width - end.width()).max(0.0)
        + if matches!(end, End::Nothing) {
            TREE_GAP
        } else {
            0.0
        };
    let room = (actions_x - TREE_GAP - name_x).max(0.0);
    let name_text = if depth == 0 && tree.results.is_none() {
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
        (room * 0.6).max(room.min(24.0)),
        egui::FontSelection::Default,
    );
    let value_x = name_x + name_galley.size().x + TREE_GAP;
    let value_room = (actions_x - TREE_GAP - value_x).max(0.0);
    let value_text = |limit: usize| {
        RichText::new(tree_preview(value, limit))
            .monospace()
            .size(theme::FONT_LABEL)
            .color(tree_colour(value))
    };
    let cut_galley = egui::WidgetText::from(value_text(TREE_CUT_CHARS)).into_galley(
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
        egui::WidgetText::from(value_text(TREE_WHOLE_CHARS)).into_galley(
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
        let x = left + level as f32 * tree.step + TREE_FOLD / 2.0;
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
            tree.keep(row_ui.ctx(), state_id, open);
        }
    }

    let shown = if text.is_empty() { tree.port } else { &text };
    // A path is shown whole up to 200 characters, in the drag's label and
    // the hovers.
    let shown = &crate::labels::truncate(shown, 200);
    let kind = type_name(Some(value));
    let name_rect = egui::Rect::from_min_size(
        egui::pos2(name_x, middle - TREE_TEXT_LIFT - name_galley.size().y / 2.0),
        name_galley.size(),
    );
    // Run results' names are dragged only where the document takes the edit.
    let drag = supported && tree.results.is_none_or(|results| results.offers);
    let field = put_at(row_ui, name_rect, |ui| {
        ui.add(
            egui::Label::new(name_galley)
                .sense(if drag {
                    egui::Sense::click_and_drag()
                } else {
                    egui::Sense::hover()
                })
                .show_tooltip_when_elided(false),
        )
    });
    // A name that can be dragged takes keyboard focus, so it is named.
    let field = widgets::named(field, name);
    if drag {
        field.dnd_set_drag_payload(Payload::Variable {
            label: shown.to_owned(),
            binding: Binding::reference(tree.node, tree.port, text.clone()),
        });
        field.on_hover_text(format!("{shown}\n{kind} · drag onto an argument"));
    } else {
        field.on_hover_text(format!("{shown}\n{kind}"));
    }

    let value_width = value_galley.size().x;
    let value_rect = egui::Rect::from_min_size(
        egui::pos2(value_x, middle - TREE_TEXT_LIFT - line / 2.0),
        value_galley.size(),
    );
    let shown_value = put_at(row_ui, value_rect, |ui| {
        ui.add(
            egui::Label::new(value_galley)
                .sense(if cut {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                })
                .show_tooltip_when_elided(false),
        )
    });
    // Pointing at a cut value shows it whole, up to as much as it opens to,
    // written only while the hover is shown.
    if cut
        && shown_value
            .on_hover_ui(|ui| {
                ui.label(
                    RichText::new(tree_preview(value, TREE_WHOLE_CHARS))
                        .monospace()
                        .size(theme::FONT_LABEL),
                );
            })
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    {
        tree.keep(row_ui.ctx(), whole_id, !whole);
    }

    if tree.results.is_some() {
        // Run results: right after the value, at most at the panel's edge.
        let x = (value_x + value_width + TREE_GAP).min(actions_x);
        if let Some(chosen) = end.show(row_ui, x, middle, shown, value, &text, mark.as_ref()) {
            *action = Some(chosen);
        }
    } else {
        inspector_actions(
            row_ui,
            tree,
            actions_x,
            middle,
            shown,
            value,
            &text,
            supported,
            mark.as_ref(),
            action,
        );
    }
    #[cfg(test)]
    if let Some(mark) = &mark
        && matches!(end, End::Column | End::Words { .. } | End::Icons)
    {
        super::marks::record(super::marks::DrawnMark {
            results: tree.results.is_some(),
            port: tree.port.to_owned(),
            path: text.clone(),
            words: mark.words.clone(),
            inside: mark.inside,
            tone: mark.tone,
            icon: !matches!(end, End::Words { .. }),
        });
    }

    if !open || child_count == 0 {
        return;
    }
    // What stands among the children starts where their names do.
    let inset = (depth + 1) as f32 * tree.step + TREE_FOLD + TREE_GAP;
    if depth >= 10 {
        ui.horizontal(|ui| {
            ui.add_space(inset);
            widgets::hint(ui, "Nested deeper than 10 levels.");
        });
        return;
    }
    let page_key = state_id.with("page");
    let kept_page = ui.ctx().data(|data| data.get_temp::<usize>(page_key));
    let mut page = kept_page.unwrap_or(0);
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
        if page != kept_page.unwrap_or(0) {
            tree.keep(ui.ctx(), page_key, page);
        }
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
            document: Id::new("document"),
            node: "n",
            port: "body",
            read_only: None,
            step: TREE_INDENT,
            results: None,
            marks: None,
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

    #[test]
    fn a_huge_text_is_cut_before_it_is_written() {
        let huge = serde_json::json!("A".repeat(1_000_000));
        for limit in [TREE_CUT_CHARS, TREE_WHOLE_CHARS] {
            let shown = tree_preview(&huge, limit);
            assert_eq!(shown.chars().count(), limit + 1, "{limit} and an ellipsis");
            assert!(shown.starts_with("\"AAA") && shown.ends_with('…'));
        }
        let short = serde_json::json!("o-1042");
        assert_eq!(tree_preview(&short, TREE_CUT_CHARS), "\"o-1042\"");
    }

    #[test]
    fn two_documents_whose_steps_share_an_id_keep_their_folding_apart() {
        let context = egui::Context::default();
        let value = serde_json::json!({"customer": {"name": "Anna", "email": "a@b.test"}});
        let tree = |document: &str| Tree {
            document: Id::new(document),
            node: "node-1",
            port: "body",
            read_only: None,
            step: TREE_INDENT,
            results: None,
            marks: None,
        };
        context.data_mut(|data| data.insert_temp(tree("first").branch("customer"), true));
        let mut visited = Vec::new();
        let mut output = context.run_ui(Default::default(), |ui| {
            for document in ["first", "second"] {
                let mut budget = JSON_ROWS_PER_FRAME;
                tree_row(
                    ui,
                    &tree(document),
                    &value,
                    &JsonPath::root(),
                    "Response",
                    0,
                    true,
                    &mut None,
                    &mut budget,
                );
                visited.push(JSON_ROWS_PER_FRAME - budget);
            }
        });
        output.textures_delta.clear();
        assert_eq!(
            visited,
            [4, 2],
            "customer is open in the first document only"
        );
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
