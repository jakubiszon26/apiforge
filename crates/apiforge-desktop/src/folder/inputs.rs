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
    /// The name field takes the keyboard on the form's first frame.
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

/// A default as typed: a JSON value, and for text also the text itself
/// without quotes; nothing when the field is empty.
fn parse_default(text: &str, value_type: ValueType) -> Result<OptionalJson, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(OptionalJson::Missing);
    }
    match serde_json::from_str::<Value>(text) {
        Ok(value) if value_type != ValueType::String || value.is_string() => {
            Ok(OptionalJson::Present(value))
        }
        // Text may be typed without its quotes; begun with one, it is JSON.
        _ if value_type == ValueType::String && !text.starts_with('"') => {
            Ok(OptionalJson::Present(Value::String(text.to_owned())))
        }
        _ => Err(DEFAULT_ERROR),
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
fn draft(ui: &egui::Ui, id: egui::Id, owner: &str) -> Option<String> {
    ui.data(|data| data.get_temp::<(String, String)>(id))
        .filter(|(draft_owner, _)| draft_owner == owner)
        .map(|(_, text)| text)
}

fn keep_draft(ui: &egui::Ui, id: egui::Id, owner: &str, text: &str, focused: bool) {
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
fn unnamed_field_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(theme::SPACE_SM);
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(theme::FONT_LABEL),
        theme::MUTED,
    );
    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, galley, theme::MUTED);
}

fn heading(ui: &mut egui::Ui, caption: &str, title: &str) {
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
fn side_panel<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> (bool, R) {
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
    fn edit_component(&mut self, document_id: &str, next: Component) {
        self.component_action(
            document_id,
            publication::Action::Model {
                next: Box::new(Document::Component(next)),
                selection: None,
            },
        );
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
        let (close, change) = side_panel(ui, |ui| {
            ui.add_enabled_ui(editable, |ui| {
                input_form(ui, id, component, port, &view, &readers)
            })
            .inner
        });
        if close {
            self.selected_inputs.remove(document_id);
        }
        let Some(change) = change else {
            return;
        };
        if !editable && !matches!(change, Change::Select(_)) {
            return;
        }
        match change {
            Change::Port(next) => {
                self.edit_component(
                    document_id,
                    edit::replace_input(component, &port.name, next),
                );
            }
            Change::Rename(name) => {
                self.edit_component(
                    document_id,
                    edit::rename_input(component, &port.name, &name),
                );
                self.selected_inputs.insert(document_id.to_owned(), name);
            }
            Change::Action(on) => {
                let input = on.then_some(port.name.as_str());
                self.edit_component(document_id, edit::set_action(component, input));
            }
            Change::Delete => {
                self.selected_inputs.remove(document_id);
                self.edit_component(document_id, edit::delete_input(component, &port.name));
            }
            Change::Select(node_id) => {
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
    if form.focus {
        name.request_focus();
        form.focus = false;
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
    if let Err(message) = default {
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

/// Draws the panel of one input; returns what it asked for.
fn input_form(
    ui: &mut egui::Ui,
    id: egui::Id,
    component: &Component,
    port: &Port,
    view: &canvas::InputView,
    readers: &[(String, String)],
) -> Option<Change> {
    let mut change = None;
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
        change = Some(Change::Rename(name));
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
    let mut default = draft(ui, default_id, &port.name).unwrap_or_else(|| default_text(port));
    let mut default_problem = None;
    ui.columns(2, |columns| {
        widgets::field_label(&mut columns[0], "Type");
        if type_combo(&mut columns[0], id.with("type"), &mut next.value_type) {
            changed = true;
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
            }
            Ok(_) => {}
            Err(message) => default_problem = Some(message),
        }
    });
    if let Some(message) = default_problem {
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
        change = Some(Change::Action(fed));
    }
    if changed {
        change = Some(Change::Port(next));
    }

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
            change = Some(Change::Select(node_id.clone()));
        }
    }

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
                change = Some(Change::Delete);
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
            change = Some(Change::Delete);
        } else {
            ui.data_mut(|data| data.insert_temp(confirm_id, port.name.clone()));
        }
    }
    change
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_default_is_json_and_text_may_drop_its_quotes() {
        let present = |value| Ok(OptionalJson::Present(value));
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
        assert_eq!(parse_default("\"EU", ValueType::String), Err(DEFAULT_ERROR));
        assert_eq!(parse_default("EUR", ValueType::Any), Err(DEFAULT_ERROR));
        assert_eq!(
            parse_default("{\"a\": 1}", ValueType::Object),
            present(json!({"a": 1}))
        );
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
