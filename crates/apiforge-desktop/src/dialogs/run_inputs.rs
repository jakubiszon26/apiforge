//! Mock-up KTLO-23b (throwaway, never merged): the Run inputs dialog asked
//! before a component run when some input has no default.

use super::{Action, Chrome, Outcome, json_field, text_field, type_label};
use crate::widgets;
use apiforge_core::format::{OptionalJson, Port};
use apiforge_core::model::ValueType;
use eframe::egui::{self, Ui};
use serde_json::Value;

pub struct Field {
    name: String,
    label: Option<String>,
    value_type: ValueType,
    contract: Option<String>,
    has_default: bool,
    text: String,
}

pub struct RunInputsDialog {
    component: String,
    fields: Vec<Field>,
}

impl RunInputsDialog {
    /// One field per input, pre-filled from its default.
    pub fn new(component: &str, inputs: &[Port], contracts: &[(String, String)]) -> Self {
        let fields = inputs
            .iter()
            .map(|port| {
                let default = match &port.default_value {
                    OptionalJson::Present(value) => Some(value.clone()),
                    OptionalJson::Missing => None,
                };
                Field {
                    name: port.name.clone(),
                    label: port.label.clone(),
                    value_type: port.value_type,
                    contract: port.contract_id.as_ref().map(|id| {
                        contracts
                            .iter()
                            .find(|(contract, _)| contract == id)
                            .map_or_else(|| id.clone(), |(_, name)| name.clone())
                    }),
                    has_default: default.is_some(),
                    text: default.as_ref().map(super::json_text).unwrap_or_default(),
                }
            })
            .collect();
        Self {
            component: component.into(),
            fields,
        }
    }

    /// Types `text` into the field of input `name` (for the mock-up's second view).
    pub fn set(&mut self, name: &str, text: &str) {
        if let Some(field) = self.fields.iter_mut().find(|field| field.name == name) {
            field.text = text.into();
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome<()> {
        let eyebrow = format!("COMPONENT · {}", self.component.to_uppercase());
        let chrome = Chrome {
            id: "run-inputs-dialog",
            eyebrow: &eyebrow,
            title: "Run inputs",
            width: 520.0,
            apply_label: "Run",
            note: Some("The values are kept until the project is closed."),
        };
        match chrome.show(ctx, &mut self.fields, body, |fields| {
            fields.iter().all(|field| error(field).is_none())
        }) {
            Action::Apply => Outcome::Applied(()),
            Action::Cancel => Outcome::Cancelled,
            Action::None => Outcome::Open,
        }
    }
}

fn body(ui: &mut Ui, fields: &mut Vec<Field>) {
    widgets::hint(
        ui,
        "An input without a default needs a value before this component can run.",
    );
    for (index, field) in fields.iter_mut().enumerate() {
        let title = match &field.label {
            Some(label) => format!("{label} · {}", field.name),
            None => field.name.clone(),
        };
        let label = widgets::field_name(ui, &title);
        let kind = match &field.contract {
            Some(contract) => format!("{} · {contract} contract", type_label(field.value_type)),
            None => type_label(field.value_type).to_owned(),
        };
        widgets::hint(
            ui,
            if field.has_default {
                format!("{kind} · pre-filled from its default")
            } else {
                format!("{kind} · no default, enter a value to run")
            },
        );
        // An empty field only keeps Run disabled; a value that does not parse is marked.
        let shown = error(field).filter(|_| !field.text.trim().is_empty());
        let invalid = shown.is_some();
        let response = ui
            .push_id(("run-input", index), |ui| match field.value_type {
                ValueType::Object | ValueType::Array | ValueType::Any => {
                    json_field(ui, &mut field.text, 4, invalid)
                }
                _ => text_field(
                    ui,
                    &mut field.text,
                    type_label(field.value_type),
                    f32::INFINITY,
                    invalid,
                ),
            })
            .inner;
        response.labelled_by(label.id);
        if let Some(message) = shown {
            widgets::field_error(ui, message);
        }
    }
}

fn error(field: &Field) -> Option<&'static str> {
    let text = field.text.trim();
    if text.is_empty() {
        return Some("Enter a value: this input has no default.");
    }
    match field.value_type {
        ValueType::Number => text.parse::<f64>().is_err().then_some("Enter a number."),
        ValueType::String => None,
        _ => serde_json::from_str::<Value>(text)
            .is_err()
            .then_some("Not valid JSON."),
    }
}
