//! Modal dialogs that edit a working copy of part of the document.
//!
//! A dialog never changes the document itself: [`Outcome::Applied`] hands the result
//! to the application, which applies it as one undoable edit. Every dialog closes on
//! Escape or Cancel, applies on its primary button or Ctrl+Enter, and keeps that
//! button disabled while the working copy is invalid.

pub mod assign_variable;
pub mod component;
pub mod environment;
pub mod settings;

pub use assign_variable::AssignVariableDialog;
pub use component::ComponentDialog;
pub use environment::{EnvironmentDialog, SessionSecrets};
pub use settings::SettingsDialog;

use crate::icons::Icon;
use crate::{theme, widgets};
use apiforge_core::{format::Contract, js, model::ValueType};
use eframe::egui::{self, Key, Modifiers, RichText, Ui};
use std::fmt;

/// What a dialog asks the application to do after a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome<T> {
    /// Keep showing the dialog.
    Open,
    /// Close the dialog and discard the working copy.
    Cancelled,
    /// Close the dialog and apply the result.
    Applied(T),
}

/// Identifiers the runner cannot use as object keys.
const RESERVED_NAMES: [&str; 3] = ["__proto__", "constructor", "prototype"];
/// Maximum identifier length, in UTF-16 units as in version-one documents.
pub const NAME_LIMIT: usize = 200;

/// Why a name in a list cannot be saved. The messages are user-facing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    Reserved,
    TooLong,
    Duplicate,
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "Enter a name.",
            Self::Reserved => "This name is reserved. Choose another one.",
            Self::TooLong => "Use at most 200 characters.",
            Self::Duplicate => "This name is already used.",
        })
    }
}

/// The problem with every name of a list, compared after trimming.
///
/// `reserved` lists names the caller keeps for itself on top of the built-in ones.
/// Every occurrence of a repeated name is reported, so both rows are highlighted.
pub fn name_errors(names: &[&str], reserved: &[&str]) -> Vec<Option<NameError>> {
    names
        .iter()
        .map(|name| {
            let name = js::trim(name);
            if name.is_empty() {
                Some(NameError::Empty)
            } else if RESERVED_NAMES.contains(&name) || reserved.contains(&name) {
                Some(NameError::Reserved)
            } else if js::utf16_len(name) > NAME_LIMIT {
                Some(NameError::TooLong)
            } else if names.iter().filter(|other| js::trim(other) == name).count() > 1 {
                Some(NameError::Duplicate)
            } else {
                None
            }
        })
        .collect()
}

/// The problem with a name about to be added to `existing`, if any.
pub fn new_name_error(name: &str, existing: &[&str], reserved: &[&str]) -> Option<NameError> {
    let mut names = existing.to_vec();
    names.push(name);
    name_errors(&names, reserved).pop().flatten()
}

/// Parses JSON typed into a field; the message is user-facing.
pub fn parse_json(text: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(text).map_err(|error| {
        format!(
            "Not valid JSON (line {}, column {}). Text needs quotes, e.g. \"abc\".",
            error.line(),
            error.column()
        )
    })
}

/// JSON shown in an editable field: compact for scalars, indented for containers.
pub fn json_text(value: &serde_json::Value) -> String {
    if value.is_object() || value.is_array() {
        serde_json::to_string_pretty(value).unwrap_or_default()
    } else {
        js::json_stringify(value)
    }
}

// ----- Dialog chrome -----------------------------------------------------------------

/// What the user did with the dialog chrome during a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    None,
    Cancel,
    Apply,
}

/// A modal with a header, a scrolling body and a footer with Cancel and the primary
/// button. `body` draws the form; `can_apply` is evaluated after the body so the
/// button reflects this frame's edits.
pub(crate) struct Chrome<'a> {
    pub(crate) id: &'static str,
    pub(crate) eyebrow: &'a str,
    pub(crate) title: &'a str,
    pub(crate) width: f32,
    pub(crate) apply_label: &'a str,
    /// A short note on the left of the footer.
    pub(crate) note: Option<&'a str>,
}

/// Creates a modal anchored to the center of the current `ctx.content_rect()` on every
/// frame and requests a repaint when the viewport changes.
pub(crate) fn centered_modal(id: egui::Id, ctx: &egui::Context) -> egui::Modal {
    let content = ctx.content_rect();
    let rect_key = id.with("last-content-rect");
    let last_rect: Option<egui::Rect> = ctx.data_mut(|data| data.get_temp(rect_key));
    if last_rect != Some(content) {
        ctx.data_mut(|data| data.insert_temp(rect_key, content));
        ctx.request_repaint();
    }
    let area = egui::Modal::default_area(id)
        .constrain_to(content)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
    egui::Modal::new(id).area(area)
}

impl Chrome<'_> {
    pub(crate) fn show<S>(
        self,
        ctx: &egui::Context,
        state: &mut S,
        body: impl FnOnce(&mut Ui, &mut S),
        can_apply: impl FnOnce(&S) -> bool,
    ) -> Action {
        self.show_inner(ctx, state, body, can_apply, None::<fn(&mut Ui, &mut S)>)
    }

    /// Additional decisions stay outside the scrolling body. Reserve their row
    /// before measuring the body so recovery actions remain visible at 540 px.
    pub(crate) fn show_with_footer<S>(
        self,
        ctx: &egui::Context,
        state: &mut S,
        body: impl FnOnce(&mut Ui, &mut S),
        can_apply: impl FnOnce(&S) -> bool,
        footer: impl FnOnce(&mut Ui, &mut S),
    ) -> Action {
        self.show_inner(ctx, state, body, can_apply, Some(footer))
    }

    fn show_inner<S>(
        self,
        ctx: &egui::Context,
        state: &mut S,
        body: impl FnOnce(&mut Ui, &mut S),
        can_apply: impl FnOnce(&S) -> bool,
        footer: Option<impl FnOnce(&mut Ui, &mut S)>,
    ) -> Action {
        let width = self.width.min(ctx.content_rect().width() - 48.0).max(320.0);
        let footer_height = if footer.is_some() { 40.0 } else { 0.0 };
        let body_height = (ctx.content_rect().height() - 236.0 - footer_height).max(100.0);
        let frame = egui::Frame::popup(&ctx.global_style())
            .fill(theme::WHITE)
            .inner_margin(egui::Margin::same(20));
        let modal_id = egui::Id::new(self.id);
        let response = centered_modal(modal_id, ctx).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            let mut action = Action::None;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if !self.eyebrow.is_empty() {
                        widgets::caption(ui, self.eyebrow);
                    }
                    ui.label(
                        RichText::new(self.title)
                            .size(18.0)
                            .family(theme::bold_family()),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.add(widgets::icon_button(Icon::Close, "Close")).clicked() {
                        action = Action::Cancel;
                    }
                });
            });
            ui.add_space(4.0);
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt((self.id, "body"))
                .max_height(body_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 0,
                            right: 12,
                            top: 4,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            body(ui, state);
                        });
                });
            ui.separator();
            ui.add_space(4.0);
            let ready = can_apply(state);
            if let Some(note) = self.note {
                widgets::hint(ui, note);
                ui.add_space(4.0);
            }
            if let Some(footer) = footer {
                footer(ui, state);
                ui.add_space(4.0);
            }
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(ready, widgets::primary_button(self.apply_label))
                        .on_hover_text("Ctrl+Enter")
                        .clicked()
                    {
                        action = Action::Apply;
                    }
                    if ui
                        .add(widgets::action(Icon::Close, "Cancel").quiet())
                        .clicked()
                    {
                        action = Action::Cancel;
                    }
                });
            });
            (action, ready)
        });
        let (mut action, ready) = response.inner;
        if response.is_top_modal && !response.any_popup_open {
            ctx.input_mut(|input| {
                if input.consume_key(Modifiers::NONE, Key::Escape) {
                    action = Action::Cancel;
                } else if ready && input.consume_key(Modifiers::COMMAND, Key::Enter) {
                    action = Action::Apply;
                }
            });
        }
        action
    }
}

/// Focuses `response` the first time a dialog is shown.
fn autofocus(response: &egui::Response, focused: &mut bool) {
    if !*focused {
        response.request_focus();
        *focused = true;
    }
}

/// Whether Enter was pressed to leave this single-line field.
fn submitted(ui: &Ui, response: &egui::Response) -> bool {
    response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter))
}

/// A single-line text field filling `width`, outlined in red when `invalid`.
fn text_field(
    ui: &mut Ui,
    text: &mut String,
    hint: &str,
    width: f32,
    invalid: bool,
) -> egui::Response {
    widgets::validated(ui, invalid, |ui| {
        ui.add(
            crate::widgets::text_edit(text)
                .hint_text(hint)
                .desired_width(width),
        )
    })
}

/// A labelled single-line field with its validation message below it.
fn labelled_field(
    ui: &mut Ui,
    label: &str,
    text: &mut String,
    hint: &str,
    error: Option<&str>,
) -> egui::Response {
    widgets::field_label(ui, label);
    let response = widgets::named(
        text_field(ui, text, hint, f32::INFINITY, error.is_some()),
        label,
    );
    if let Some(error) = error {
        widgets::field_error(ui, error);
    }
    response
}

/// A multi-line JSON editor outlined in red when `invalid`.
fn json_field(ui: &mut Ui, text: &mut String, rows: usize, invalid: bool) -> egui::Response {
    widgets::validated(ui, invalid, |ui| {
        ui.add(
            crate::widgets::text_area(text)
                .code_editor()
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        )
    })
}

pub const VALUE_TYPES: [ValueType; 6] = [
    ValueType::Any,
    ValueType::String,
    ValueType::Number,
    ValueType::Boolean,
    ValueType::Object,
    ValueType::Array,
];

/// A friendly name of a value type.
pub fn type_label(value_type: ValueType) -> &'static str {
    match value_type {
        ValueType::Any => "Any",
        ValueType::String => "Text",
        ValueType::Number => "Number",
        ValueType::Boolean => "Yes / no",
        ValueType::Object => "Object",
        ValueType::Array => "List",
    }
}

fn type_picker(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: &mut ValueType,
    width: f32,
) {
    egui::ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(type_label(*value))
        .show_ui(ui, |ui| {
            for option in VALUE_TYPES {
                ui.selectable_value(value, option, type_label(option));
            }
        });
}

/// A contract picker; `None` is shown as `empty`.
fn contract_picker(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: &mut Option<String>,
    contracts: &[Contract],
    empty: &str,
    width: f32,
) {
    let selected = value
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(|id| {
            contracts
                .iter()
                .find(|contract| contract.id == id)
                .map_or_else(|| format!("Missing contract ({id})"), |c| c.name.clone())
        })
        .unwrap_or_else(|| empty.to_owned());
    egui::ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(widgets::ellipsis(&selected, 28))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, empty);
            for contract in contracts {
                ui.selectable_value(value, Some(contract.id.clone()), &contract.name);
            }
        });
}

#[cfg(test)]
pub(crate) mod test_support {
    use eframe::egui;

    /// Runs `frames` passes of a 1280×800 window with no input.
    pub fn run_frames(frames: usize, mut show: impl FnMut(&egui::Context)) {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        for _ in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| show(ui.ctx()));
            output.textures_delta.clear();
        }
    }

    /// Runs one frame in which `key` is pressed with `modifiers`.
    pub fn press(
        ctx: &egui::Context,
        key: egui::Key,
        modifiers: egui::Modifiers,
        mut show: impl FnMut(&egui::Context),
    ) {
        crate::theme::apply(ctx);
        if ctx.cumulative_pass_nr() == 0 {
            let warm = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(warm, |ui| show(ui.ctx()));
            output.textures_delta.clear();
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| show(ui.ctx()));
        output.textures_delta.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn names_are_checked_after_trimming() {
        let long = "x".repeat(201);
        let errors = name_errors(
            &["token", " token ", "", "__proto__", "baseUrl", &long, "ok"],
            &["baseUrl"],
        );
        assert_eq!(
            errors,
            vec![
                Some(NameError::Duplicate),
                Some(NameError::Duplicate),
                Some(NameError::Empty),
                Some(NameError::Reserved),
                Some(NameError::Reserved),
                Some(NameError::TooLong),
                None,
            ]
        );
        assert_eq!(name_errors(&["x".repeat(200).as_str()], &[]), vec![None]);
    }

    #[test]
    fn new_names_must_not_clash() {
        assert_eq!(new_name_error("a", &["a"], &[]), Some(NameError::Duplicate));
        assert_eq!(new_name_error(" ", &[], &[]), Some(NameError::Empty));
        assert_eq!(new_name_error("b", &["a"], &[]), None);
    }

    #[test]
    fn json_fields_round_trip() {
        assert_eq!(parse_json(" 42 "), Ok(json!(42)));
        assert!(parse_json("abc").unwrap_err().contains("line 1"));
        assert_eq!(json_text(&json!("a")), "\"a\"");
        assert_eq!(
            parse_json(&json_text(&json!({"a": [1]}))),
            Ok(json!({"a": [1]}))
        );
    }
}
