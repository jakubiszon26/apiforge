//! A component's inputs in the Inspector (UX-31, UX-32, COMP-00 Q2 = A and
//! Q3): the *New input* form that *+ Add input* opens, and the panel of the
//! input selected in the INPUTS strip. The panel's edits are live: each is
//! written to the code as it is made, through the ordinary model edit, so
//! one undo takes back each. A name the code cannot take stays in its field
//! with an inline error while the code keeps the last valid name.

use super::*;
use crate::inspector::type_combo;
use apiforge_core::edit::check_input_name;
use apiforge_core::format::{Component, OptionalJson};
use apiforge_core::model::ValueType;
use serde_json::Value;

/// The *New input* form of one document, as typed so far.
#[derive(Clone, Debug)]
pub(super) struct NewInput {
    pub(super) name: String,
    value_type: ValueType,
    default: String,
    required: bool,
    exposed: bool,
    /// The name field is to take the keyboard: from the form's first frame
    /// until it has it.
    focus: bool,
}

impl Default for NewInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            value_type: ValueType::String,
            default: String::new(),
            required: false,
            exposed: true,
            focus: true,
        }
    }
}

const DEFAULT_ERROR: &str =
    "Not a valid default. Write a value such as 1, true, \"text\", [1, 2] or {\"a\": 1}.";
const REQUIRED_RULE: &str =
    "An input without a default is required. Give it a default to make it optional.";
const REQUIRED_HOVER: &str = "Callers must give a value";
const EXPOSED_HOVER: &str = "A block that calls the component lets its value be edited";

/// A default as typed: a JSON value of the input's type, and for text also
/// the text itself without quotes; nothing when the field is empty.
fn parse_default(text: &str, value_type: ValueType) -> Result<OptionalJson, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(OptionalJson::Missing);
    }
    match serde_json::from_str::<Value>(text) {
        Ok(value) if value_type != ValueType::String || value.is_string() => {
            default_fits(&value, value_type)?;
            Ok(OptionalJson::Present(value))
        }
        // Text may be typed without its quotes; begun with one, it is JSON.
        _ if value_type == ValueType::String && !text.starts_with('"') => {
            Ok(OptionalJson::Present(Value::String(text.to_owned())))
        }
        _ => Err(DEFAULT_ERROR.to_owned()),
    }
}

/// What a default of `value_type` must be, as a phrase and an example;
/// nothing for text and any data, which take a value as written, as the
/// language does.
fn expected(value_type: ValueType) -> Option<(&'static str, &'static str)> {
    match value_type {
        ValueType::Any | ValueType::String => None,
        ValueType::Number => Some(("a number", "1 or 2.5")),
        ValueType::Boolean => Some(("yes or no", "true or false")),
        ValueType::Object => Some(("an object", "{\"a\": 1}")),
        ValueType::Array => Some(("a list", "[1, 2]")),
    }
}

/// Whether `value` may be the default of an input of `value_type`.
fn default_fits(value: &Value, value_type: ValueType) -> Result<(), String> {
    let fits = match value_type {
        ValueType::Any | ValueType::String => true,
        ValueType::Number => value.is_number(),
        ValueType::Boolean => value.is_boolean(),
        ValueType::Object => value.is_object(),
        ValueType::Array => value.is_array(),
    };
    match expected(value_type) {
        Some((kind, example)) if !fits => Err(format!(
            "The default is not {kind}. Write one such as {example}, or clear it."
        )),
        _ => Ok(()),
    }
}

/// A default as the field shows it: as written in code.
fn default_text(port: &Port) -> String {
    match &port.default_value {
        OptionalJson::Present(value) => value.to_string(),
        OptionalJson::Missing => String::new(),
    }
}

/// `required` as the code states it: nothing when the input is that anyway,
/// which is required unless it has a default (KTLO-32).
fn required_attribute(required: bool, has_default: bool) -> Option<bool> {
    (required == has_default).then_some(required)
}

/// The text a field shows: what is being typed in it while it has the
/// keyboard, else what the code says. A draft belongs to the input it was
/// typed for.
pub(super) fn draft(ui: &egui::Ui, id: egui::Id, owner: &str) -> Option<String> {
    ui.data(|data| data.get_temp::<(String, String)>(id))
        .filter(|(draft_owner, _)| draft_owner == owner)
        .map(|(_, text)| text)
}

pub(super) fn keep_draft(ui: &egui::Ui, id: egui::Id, owner: &str, text: &str, focused: bool) {
    ui.data_mut(|data| {
        if focused {
            data.insert_temp(id, (owner.to_owned(), text.to_owned()));
        } else {
            data.remove::<(String, String)>(id);
        }
    });
}

/// A field's visible label when the field itself carries the same name, so
/// that a reader finds one widget by it.
pub(super) fn unnamed_field_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(theme::SPACE_SM);
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(theme::FONT_LABEL),
        theme::MUTED,
    );
    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, galley, theme::MUTED);
}

pub(super) fn heading(ui: &mut egui::Ui, caption: &str, title: &str) {
    ui.label(
        RichText::new(caption)
            .size(theme::FONT_META)
            .color(theme::MUTED),
    );
    ui.label(
        RichText::new(title)
            .size(theme::FONT_PANEL_TITLE)
            .family(theme::bold_family()),
    );
    ui.add_space(theme::SPACE_SM);
    ui.separator();
}

/// The Inspector beside the board; returns whether it was closed and what
/// `add_contents` returned.
pub(super) fn side_panel<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> (bool, R) {
    let mut close = false;
    let inner = egui::Panel::right("folder-inspector")
        .default_size(360.0)
        .size_range(300.0..=520.0)
        .frame(egui::Frame::new().fill(theme::WHITE).inner_margin(16))
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                close = ui
                    .add(widgets::icon_button(Icon::Close, "Close Inspector").compact())
                    .clicked();
            });
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width((ui.available_width() - theme::SPACE_SM).max(0.0));
                    add_contents(ui)
                })
                .inner
        })
        .inner;
    widgets::name_resize_handle(
        ui.ctx(),
        egui::Id::new("folder-inspector"),
        "Resize inspector",
    );
    (close, inner)
}

/// What the input's panel asked for in one frame.
enum Change {
    /// The input with new attributes, under its name.
    Port(Port),
    Rename(String),
    /// Whether the component's action is fed by this input.
    Action(bool),
    Delete,
    /// Select the card of a step that reads the input.
    Select(String),
}

impl FolderApp {
    /// A model edit of the component in `document_id`.
    pub(super) fn edit_component(&mut self, document_id: &str, next: Component) {
        self.component_action(
            document_id,
            publication::Action::Model {
                next: Box::new(Document::Component(next)),
                selection: None,
                check: None,
            },
        );
    }

    /// Opens the *New input* form in the Inspector, in place of a selected
    /// block or input or the component's settings.
    pub(super) fn open_new_input(&mut self, document_id: &str) {
        self.new_inputs.entry(document_id.to_owned()).or_default();
        self.selections.remove(document_id);
        self.selected_inputs.remove(document_id);
        self.component_settings.remove(document_id);
    }

    /// Selects the input `name`: its panel takes the Inspector.
    pub(super) fn open_input(&mut self, document_id: &str, name: String) {
        self.new_inputs.remove(document_id);
        self.component_settings.remove(document_id);
        self.selections.remove(document_id);
        self.selected_inputs.insert(document_id.to_owned(), name);
    }

    /// The *New input* form (COMP-00 Q2 = A): name, type, default,
    /// *Required* and *Editable on the block*, then *Add input* or *Cancel*.
    pub(super) fn new_input_panel(
        &mut self,
        ui: &mut egui::Ui,
        document_id: &str,
        component: &Component,
        editable: bool,
    ) {
        let Some(mut form) = self.new_inputs.get(document_id).cloned() else {
            return;
        };
        let id = egui::Id::new(("component-new-input", document_id));
        let (close, (submit, cancel)) = side_panel(ui, |ui| {
            ui.add_enabled_ui(editable, |ui| new_input_form(ui, id, component, &mut form))
                .inner
        });
        if close || cancel {
            self.new_inputs.remove(document_id);
            return;
        }
        let Some(port) = submit.then(|| form.port()).flatten() else {
            self.new_inputs.insert(document_id.to_owned(), form);
            return;
        };
        self.new_inputs.remove(document_id);
        let name = port.name.clone();
        self.edit_component(document_id, edit::add_input(component, port));
        // The new input opens in the Inspector, as its pill is selected.
        self.selections.remove(document_id);
        self.selected_inputs.insert(document_id.to_owned(), name);
    }

    /// The Inspector of an input selected in the INPUTS strip (COMP-00 Q3):
    /// each change is written to the code as it is made.
    pub(super) fn input_panel(
        &mut self,
        ui: &mut egui::Ui,
        document_id: &str,
        component: &Component,
        port: &Port,
        editable: bool,
    ) {
        let project = self.context_project();
        let read = canvas::input_readers(&component.graph, &port.name);
        // In the order of the board's steps.
        let readers: Vec<(String, String)> = component
            .graph
            .nodes
            .iter()
            .filter(|node| read.contains(&node.id))
            .map(|node| (node.id.clone(), node.display_name().to_owned()))
            .collect();
        let id = egui::Id::new(("component-input", document_id));
        let view = canvas::InputView::of(port, &project.contracts);
        let (close, changes) = side_panel(ui, |ui| {
            let mut changes = ui
                .add_enabled_ui(editable, |ui| input_form(ui, id, component, port, &view))
                .inner;
            // Going to a reading step is navigation, free on a read-only board.
            used_by(ui, &readers, &mut changes);
            ui.add_enabled_ui(editable, |ui| {
                delete_input(ui, id, port, &readers, &mut changes);
            });
            changes
        });
        if close {
            self.selected_inputs.remove(document_id);
        }
        // The frame's changes, applied in turn as one edit.
        let mut next = component.clone();
        let mut name = port.name.clone();
        let mut deleted = false;
        let mut select = None;
        for change in changes {
            match change {
                Change::Select(node_id) => select = Some(node_id),
                _ if !editable => {}
                Change::Port(port) => next = edit::replace_input(&next, &name, port),
                Change::Rename(to) => {
                    next = edit::rename_input(&next, &name, &to);
                    name = to;
                }
                Change::Action(on) => next = edit::set_action(&next, on.then_some(name.as_str())),
                Change::Delete => {
                    next = edit::delete_input(&next, &name);
                    deleted = true;
                }
            }
        }
        if next != *component {
            self.edit_component(document_id, next);
        }
        if deleted {
            self.selected_inputs.remove(document_id);
        } else if name != port.name {
            self.selected_inputs.insert(document_id.to_owned(), name);
        }
        if let Some(node_id) = select {
            self.selected_inputs.remove(document_id);
            self.selections
                .insert(document_id.to_owned(), vec![node_id.clone()]);
            if let Some(canvas) = self.canvases.get_mut(document_id) {
                canvas.reveal(node_id);
            }
            self.touched();
        }
    }
}

impl NewInput {
    /// The input the form declares, when it may be added.
    fn port(&self) -> Option<Port> {
        let default_value = parse_default(&self.default, self.value_type).ok()?;
        let has_default = !default_value.is_missing();
        Some(Port {
            default_value,
            required: (has_default && self.required).then_some(true),
            exposed: (!self.exposed).then_some(false),
            ..Port::new(self.name.trim(), self.value_type)
        })
    }
}

/// Draws the *New input* form; returns whether *Add input* or *Cancel* was
/// chosen.
fn new_input_form(
    ui: &mut egui::Ui,
    id: egui::Id,
    component: &Component,
    form: &mut NewInput,
) -> (bool, bool) {
    heading(ui, "COMPONENT INPUT", "New input");
    let problem = |name: &str| {
        let name = name.trim();
        (!name.is_empty())
            .then(|| check_input_name(component, None, name).err())
            .flatten()
    };
    widgets::field_label(ui, "Name");
    let outlined = problem(&form.name).is_some();
    let name = widgets::validated(ui, outlined, |ui| {
        widgets::named(
            ui.add(
                widgets::text_edit(&mut form.name)
                    .id(id.with("name"))
                    .hint_text("e.g. productId")
                    .desired_width(f32::INFINITY),
            ),
            "Input name",
        )
    });
    // Asked for until the field has it: in the native window the click that
    // opened the form may still hold the focus on the frame the form opens.
    if form.focus {
        if name.has_focus() || !ui.is_enabled() {
            form.focus = false;
        } else {
            name.request_focus();
        }
    }
    let name_problem = problem(&form.name);
    match &name_problem {
        Some(problem) => widgets::field_error(ui, problem.to_string()),
        None => {
            let shown = form.name.trim();
            widgets::hint(
                ui,
                format!(
                    "Steps read it as param.{}.",
                    if shown.is_empty() { "name" } else { shown }
                ),
            );
        }
    }
    let mut default_response = None;
    ui.columns(2, |columns| {
        widgets::field_label(&mut columns[0], "Type");
        type_combo(&mut columns[0], id.with("type"), &mut form.value_type);
        widgets::field_label(&mut columns[1], "Default");
        let invalid = parse_default(&form.default, form.value_type).is_err();
        default_response = Some(widgets::validated(&mut columns[1], invalid, |ui| {
            widgets::named(
                ui.add(
                    widgets::text_edit(&mut form.default)
                        .id(id.with("default"))
                        .hint_text("None")
                        .desired_width(f32::INFINITY),
                ),
                "Default value",
            )
        }));
    });
    let default = parse_default(&form.default, form.value_type);
    if let Err(message) = &default {
        widgets::field_error(ui, message);
    }
    let has_default = matches!(default, Ok(OptionalJson::Present(_)));
    ui.add_space(theme::SPACE_SM);
    ui.horizontal(|ui| {
        if has_default {
            ui.checkbox(&mut form.required, "Required")
                .on_hover_text(REQUIRED_HOVER);
        } else {
            // KTLO-32: without a default the input is required.
            ui.add_enabled(false, egui::Checkbox::new(&mut true, "Required"))
                .on_disabled_hover_text(REQUIRED_RULE);
        }
        ui.checkbox(&mut form.exposed, "Editable on the block")
            .on_hover_text(EXPOSED_HOVER);
    });
    if !has_default {
        widgets::hint(ui, REQUIRED_RULE);
    }
    let valid = !form.name.trim().is_empty() && name_problem.is_none() && default.is_ok();
    let entered = (name.lost_focus() || default_response.is_some_and(|r| r.lost_focus()))
        && ui.input(|input| input.key_pressed(egui::Key::Enter));
    ui.add_space(theme::SPACE_MD);
    let (add, cancel) = ui
        .horizontal(|ui| {
            let add = ui
                .add_enabled(
                    valid,
                    widgets::primary_button("Add input").named("Add the new input"),
                )
                .clicked();
            let cancel = ui.add(egui::Button::new("Cancel")).clicked();
            (add, cancel)
        })
        .inner;
    (valid && (add || entered), cancel)
}

/// Draws the fields of one input's panel; returns what they asked for.
fn input_form(
    ui: &mut egui::Ui,
    id: egui::Id,
    component: &Component,
    port: &Port,
    view: &canvas::InputView,
) -> Vec<Change> {
    let mut changes = Vec::new();
    heading(ui, "COMPONENT INPUT", &port.name);

    // The name: written as typed while it is valid and unique.
    widgets::field_label(ui, "Name");
    let name_id = id.with("name");
    let mut name = draft(ui, name_id, &port.name).unwrap_or_else(|| port.name.clone());
    let outlined = check_input_name(component, Some(&port.name), &name).is_err();
    let response = widgets::validated(ui, outlined, |ui| {
        widgets::named(
            ui.add(
                widgets::text_edit(&mut name)
                    .id(name_id.with("field"))
                    .desired_width(f32::INFINITY),
            ),
            "Input name",
        )
    });
    let problem = check_input_name(component, Some(&port.name), &name).err();
    keep_draft(ui, name_id, &port.name, &name, response.has_focus());
    match &problem {
        Some(problem @ apiforge_core::edit::InputNameError::Taken(_)) => widgets::field_error(
            ui,
            format!(
                "{problem} The code keeps {} until the name is unique.",
                port.name
            ),
        ),
        Some(problem) => {
            widgets::field_error(ui, format!("{problem} The code keeps {}.", port.name))
        }
        None => widgets::hint(ui, format!("Steps read it as param.{}.", port.name)),
    }
    if response.changed() && problem.is_none() && name != port.name {
        changes.push(Change::Rename(name));
    }

    let mut next = port.clone();
    let mut changed = false;
    unnamed_field_label(ui, "Label on the block");
    let mut label = port.label.clone().unwrap_or_default();
    if widgets::named(
        ui.add(
            widgets::text_edit(&mut label)
                .id(id.with("label"))
                .hint_text("Shown instead of the name")
                .desired_width(f32::INFINITY),
        ),
        "Label on the block",
    )
    .changed()
    {
        next.label = (!label.is_empty()).then_some(label);
        changed = true;
    }

    let default_id = id.with("default");
    let type_error_id = id.with("type-error");
    let mut default = draft(ui, default_id, &port.name).unwrap_or_else(|| default_text(port));
    let mut default_problem = None;
    ui.columns(2, |columns| {
        widgets::field_label(&mut columns[0], "Type");
        if type_combo(&mut columns[0], id.with("type"), &mut next.value_type) {
            // A type the default does not fit waits for the default.
            let refused = match &port.default_value {
                OptionalJson::Present(value) => default_fits(value, next.value_type).err(),
                OptionalJson::Missing => None,
            };
            match refused {
                Some(_) => {
                    let kind = expected(next.value_type).map_or("", |(kind, _)| kind);
                    let message = format!(
                        "The default {} is not {kind}. Change or clear the default first.",
                        default_text(port),
                    );
                    columns[0].data_mut(|data| {
                        data.insert_temp(type_error_id, (port.name.clone(), message))
                    });
                    next.value_type = port.value_type;
                }
                None => {
                    columns[0].data_mut(|data| data.remove::<(String, String)>(type_error_id));
                    changed = true;
                }
            }
        }
        widgets::field_label(&mut columns[1], "Default");
        let outlined = parse_default(&default, next.value_type).is_err();
        let response = widgets::validated(&mut columns[1], outlined, |ui| {
            widgets::named(
                ui.add(
                    widgets::text_edit(&mut default)
                        .id(default_id.with("field"))
                        .hint_text("None")
                        .desired_width(f32::INFINITY),
                ),
                "Default value",
            )
        });
        keep_draft(
            &columns[1],
            default_id,
            &port.name,
            &default,
            response.has_focus(),
        );
        match parse_default(&default, next.value_type) {
            Ok(value) if response.changed() && value != port.default_value => {
                next.default_value = value;
                changed = true;
                columns[1].data_mut(|data| data.remove::<(String, String)>(type_error_id));
            }
            Ok(_) => {}
            Err(message) => default_problem = Some(message),
        }
    });
    if let Some(message) = default_problem {
        widgets::field_error(ui, message);
    }
    if let Some(message) = draft(ui, type_error_id, &port.name) {
        widgets::field_error(ui, message);
    }
    if let Some(contract) = &view.contract {
        ui.horizontal(|ui| {
            widgets::hint(ui, "Contract");
            widgets::pill(ui, contract, theme::LINK);
        });
    }

    ui.add_space(theme::SPACE_SM);
    let has_default = !next.default_value.is_missing();
    // KTLO-32: an input without a default is required unless it says not.
    let forced = !has_default && port.required != Some(false);
    ui.horizontal(|ui| {
        let mut required = port.is_required_input();
        let response = ui.add_enabled(!forced, egui::Checkbox::new(&mut required, "Required"));
        let response = if forced {
            response.on_disabled_hover_text(REQUIRED_RULE)
        } else {
            response.on_hover_text(REQUIRED_HOVER)
        };
        if response.changed() {
            next.required = required_attribute(required, has_default);
            changed = true;
        }
        let mut exposed = port.exposed.unwrap_or(true);
        if ui
            .checkbox(&mut exposed, "Editable on the block")
            .on_hover_text(EXPOSED_HOVER)
            .changed()
        {
            next.exposed = (!exposed).then_some(false);
            changed = true;
        }
    });
    if forced {
        widgets::hint(ui, REQUIRED_RULE);
    }
    let action = component
        .action
        .as_ref()
        .map(|action| action.input.as_str());
    let mut fed = action == Some(port.name.as_str());
    let hover = match action {
        Some(other) if other != port.name => format!(
            "Dropped after a block's output, the component is connected through this input. \
             A component has one: checking it moves it from {other}."
        ),
        _ => "Dropped after a block's output, the component is connected through this input."
            .to_owned(),
    };
    if ui
        .checkbox(&mut fed, "Fed by the previous block (action)")
        .on_hover_text(hover)
        .changed()
    {
        changes.push(Change::Action(fed));
    }
    if changed {
        changes.push(Change::Port(next));
    }
    changes
}

/// *Used by*: a link to each step that reads the input.
fn used_by(ui: &mut egui::Ui, readers: &[(String, String)], changes: &mut Vec<Change>) {
    ui.add_space(theme::SPACE_SM);
    widgets::field_label(ui, "Used by");
    if readers.is_empty() {
        widgets::hint(ui, "No step reads this input yet.");
    }
    for (node_id, reader) in readers {
        let accessible = format!("Select the step {reader}");
        if ui
            .add(
                widgets::action(Icon::ArrowLeft, reader)
                    .quiet()
                    .named(&accessible),
            )
            .on_hover_text("Select its card on the board")
            .clicked()
        {
            changes.push(Change::Select(node_id.clone()));
        }
    }
}

/// *Delete input*, which asks first when a step reads the input.
fn delete_input(
    ui: &mut egui::Ui,
    id: egui::Id,
    port: &Port,
    readers: &[(String, String)],
    changes: &mut Vec<Change>,
) {
    ui.add_space(theme::SPACE_LG);
    let confirm_id = id.with("delete");
    let confirming = ui
        .data(|data| data.get_temp::<String>(confirm_id))
        .is_some_and(|name| name == port.name);
    if confirming {
        let names: Vec<&str> = readers.iter().map(|(_, name)| name.as_str()).collect();
        let (who, verb, them) = match names.as_slice() {
            [one] => ((*one).to_owned(), "reads", "it"),
            [rest @ .., last] => (format!("{} and {last}", rest.join(", ")), "read", "them"),
            [] => (String::new(), "reads", "it"),
        };
        widgets::error_box(
            ui,
            &[format!(
                "{who} {verb} {name}. Deleting the input also takes param.{name} out of {them}.",
                name = port.name
            )],
        );
        ui.horizontal(|ui| {
            if ui
                .add(widgets::action(Icon::Trash, "Delete anyway").danger())
                .clicked()
            {
                changes.push(Change::Delete);
                ui.data_mut(|data| data.remove::<String>(confirm_id));
            }
            if ui.add(egui::Button::new("Cancel")).clicked() {
                ui.data_mut(|data| data.remove::<String>(confirm_id));
            }
        });
    } else if ui
        .add(
            widgets::action(Icon::Trash, "Delete input")
                .quiet()
                .danger(),
        )
        .clicked()
    {
        if readers.is_empty() {
            changes.push(Change::Delete);
        } else {
            ui.data_mut(|data| data.insert_temp(confirm_id, port.name.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_default_is_json_of_its_type_and_text_may_drop_its_quotes() {
        let present = |value| Ok(OptionalJson::Present(value));
        let invalid = Err(DEFAULT_ERROR.to_owned());
        assert_eq!(
            parse_default("", ValueType::Number),
            Ok(OptionalJson::Missing)
        );
        assert_eq!(parse_default(" 2 ", ValueType::Number), present(json!(2)));
        assert_eq!(
            parse_default("\"EUR\"", ValueType::String),
            present(json!("EUR"))
        );
        assert_eq!(
            parse_default("EUR", ValueType::String),
            present(json!("EUR"))
        );
        assert_eq!(parse_default("12", ValueType::String), present(json!("12")));
        assert_eq!(parse_default("\"EU", ValueType::String), invalid);
        assert_eq!(parse_default("EUR", ValueType::Any), invalid);
        assert_eq!(parse_default("true", ValueType::Any), present(json!(true)));
        assert_eq!(
            parse_default("{\"a\": 1}", ValueType::Object),
            present(json!({"a": 1}))
        );
        // A value of another kind is not the input's default.
        for (text, value_type) in [
            ("\"EUR\"", ValueType::Number),
            ("1", ValueType::Boolean),
            ("[1]", ValueType::Object),
            ("{}", ValueType::Array),
        ] {
            assert!(
                parse_default(text, value_type)
                    .unwrap_err()
                    .starts_with("The default is not"),
                "{text} for {value_type:?}"
            );
        }
    }

    #[test]
    fn required_is_written_only_against_the_rule() {
        // With a default an input is optional; without one, required.
        assert_eq!(required_attribute(true, true), Some(true));
        assert_eq!(required_attribute(false, true), None);
        assert_eq!(required_attribute(true, false), None);
        assert_eq!(required_attribute(false, false), Some(false));
    }
}
