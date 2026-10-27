//! The inputs a component needs before it runs on its own (KTLO-23b).
//!
//! Asked only when some input has no default (owner decision 1 of 2026-10-26):
//! one field per input, pre-filled with what was entered for this component
//! earlier in the session or else with its default. The runner takes JSON
//! values and the types written in code, so a field's text becomes a value of
//! its input's type: text as typed, a finite number, `true` or `false`, and
//! JSON for an object, a list or any value.

use super::{Action, Chrome, Outcome, autofocus, json_field, json_text, parse_json, text_field};
use crate::widgets;
use apiforge_core::{
    format::{Contract, Port},
    js,
    model::ValueType,
};
use eframe::egui::{self, Ui};
use indexmap::IndexMap;
use serde_json::Value;

/// The values entered for a component's inputs, by input name.
pub type RunInputs = IndexMap<String, Value>;

/// Where a field's first text came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Remembered,
    Default,
    Empty,
}

struct Field {
    name: String,
    /// The visible name: the input's label and name, or its name.
    title: String,
    value_type: ValueType,
    contract: Option<String>,
    source: Source,
    text: String,
}

impl Field {
    /// The accessible name of the field: its title and type.
    fn accessible_name(&self) -> String {
        format!("{} · {}", self.title, type_name(self.value_type))
    }

    fn value(&self) -> Option<Result<Value, String>> {
        value(self.value_type, &self.text)
    }
}

/// Asks for a component's input values before a run.
pub struct RunInputsDialog {
    eyebrow: String,
    fields: Vec<Field>,
    focused: bool,
}

impl RunInputsDialog {
    /// Whether a run of a component with `inputs` asks for them first: some
    /// input has no default.
    pub fn needed(inputs: &[Port]) -> bool {
        inputs.iter().any(|port| port.default().is_none())
    }

    /// One field per input, pre-filled from `remembered` or the default.
    pub fn new(
        component: &str,
        inputs: &[Port],
        contracts: &[Contract],
        remembered: Option<&RunInputs>,
    ) -> Self {
        let fields = inputs
            .iter()
            .map(|port| {
                let (source, value) = match (
                    remembered.and_then(|values| values.get(&port.name)),
                    port.default(),
                ) {
                    (Some(value), _) => (Source::Remembered, Some(value)),
                    (None, Some(value)) => (Source::Default, Some(value)),
                    (None, None) => (Source::Empty, None),
                };
                Field {
                    name: port.name.clone(),
                    title: match &port.label {
                        Some(label) if label != &port.name => format!("{label} · {}", port.name),
                        _ => port.name.clone(),
                    },
                    value_type: port.value_type,
                    contract: port.contract_id.as_ref().map(|id| {
                        contracts
                            .iter()
                            .find(|contract| &contract.id == id)
                            .map_or_else(|| id.clone(), |contract| contract.name.clone())
                    }),
                    source,
                    text: value
                        .map(|value| text(port.value_type, value))
                        .unwrap_or_default(),
                }
            })
            .collect();
        Self {
            eyebrow: format!("COMPONENT · {}", component.to_uppercase()),
            fields,
            focused: false,
        }
    }

    /// Applies with the value of every input once each has a valid one.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome<RunInputs> {
        let chrome = Chrome {
            id: "run-inputs-dialog",
            eyebrow: &self.eyebrow,
            title: "Run inputs",
            width: 520.0,
            apply_label: "Run",
            note: Some("The values are kept until the project is closed."),
        };
        let mut state = (&mut self.fields, &mut self.focused);
        match chrome.show(ctx, &mut state, body, |(fields, _)| {
            fields
                .iter()
                .all(|field| matches!(field.value(), Some(Ok(_))))
        }) {
            Action::Apply => Outcome::Applied(
                self.fields
                    .iter()
                    .filter_map(|field| Some((field.name.clone(), field.value()?.ok()?)))
                    .collect(),
            ),
            Action::Cancel => Outcome::Cancelled,
            Action::None => Outcome::Open,
        }
    }
}

fn body(ui: &mut Ui, (fields, focused): &mut (&mut Vec<Field>, &mut bool)) {
    widgets::hint(
        ui,
        "An input without a default needs a value before this component can run.",
    );
    let first_empty = fields.iter().position(|field| field.text.trim().is_empty());
    for (index, field) in fields.iter_mut().enumerate() {
        widgets::field_name(ui, &field.title);
        let kind = match &field.contract {
            Some(contract) => format!("{} · {contract} contract", type_name(field.value_type)),
            None => type_name(field.value_type).to_owned(),
        };
        widgets::hint(
            ui,
            match field.source {
                Source::Remembered => format!("{kind} · kept from the last run"),
                Source::Default => format!("{kind} · pre-filled from its default"),
                Source::Empty => format!("{kind} · no default, enter a value to run"),
            },
        );
        // An empty field only keeps Run disabled; a value that does not parse is marked.
        let error = field.value().and_then(Result::err);
        let response = ui
            .push_id(("run-input", index), |ui| match field.value_type {
                ValueType::Object | ValueType::Array | ValueType::Any => {
                    json_field(ui, &mut field.text, 4, error.is_some())
                }
                value_type => text_field(
                    ui,
                    &mut field.text,
                    placeholder(value_type),
                    f32::INFINITY,
                    error.is_some(),
                ),
            })
            .inner;
        let response = widgets::named(response, field.accessible_name());
        if first_empty.unwrap_or(0) == index {
            autofocus(&response, focused);
        }
        if let Some(error) = error {
            widgets::field_error(ui, error);
        }
    }
}

/// The type as the dialog names it.
fn type_name(value_type: ValueType) -> &'static str {
    super::type_label(value_type)
}

fn placeholder(value_type: ValueType) -> &'static str {
    match value_type {
        ValueType::String => "Text",
        ValueType::Number => "Number",
        ValueType::Boolean => "true or false",
        _ => "JSON",
    }
}

/// The text a field shows for `value`: a text input's text as it is, any other
/// value as JSON.
fn text(value_type: ValueType, value: &Value) -> String {
    match (value_type, value) {
        (ValueType::String, Value::String(text)) => text.clone(),
        _ => json_text(value),
    }
}

/// The value `text` gives an input of `value_type`: `None` while it is empty,
/// an error message when it does not parse as that type.
fn value(value_type: ValueType, text: &str) -> Option<Result<Value, String>> {
    let trimmed = js::trim(text);
    if trimmed.is_empty() {
        return None;
    }
    Some(match value_type {
        ValueType::String => Ok(Value::String(text.to_owned())),
        ValueType::Number => trimmed
            .parse::<f64>()
            .ok()
            .and_then(js::number_value)
            .ok_or_else(|| "Enter a finite number.".to_owned()),
        ValueType::Boolean => match trimmed {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err("Enter true or false.".to_owned()),
        },
        ValueType::Any => parse_json(trimmed),
        ValueType::Object => parse_json(trimmed).and_then(|value| {
            value
                .is_object()
                .then_some(value)
                .ok_or_else(|| "Enter an object, e.g. { \"id\": 1 }.".to_owned())
        }),
        ValueType::Array => parse_json(trimmed).and_then(|value| {
            value
                .is_array()
                .then_some(value)
                .ok_or_else(|| "Enter a list, e.g. [1, 2].".to_owned())
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::format::OptionalJson;
    use serde_json::json;

    #[test]
    fn typed_text_becomes_a_value_of_the_inputs_type() {
        let ok = |value_type, text| value(value_type, text).unwrap().unwrap();
        let err = |value_type, text| value(value_type, text).unwrap().is_err();
        assert_eq!(ok(ValueType::String, " a b "), json!(" a b "), "as typed");
        assert_eq!(ok(ValueType::String, "123"), json!("123"));
        assert_eq!(ok(ValueType::Number, " 2 "), json!(2));
        assert_eq!(ok(ValueType::Number, "1.5"), json!(1.5));
        assert!(err(ValueType::Number, "two"));
        assert!(err(ValueType::Number, "inf"));
        assert_eq!(ok(ValueType::Boolean, "true"), json!(true));
        assert_eq!(ok(ValueType::Boolean, "false"), json!(false));
        assert!(err(ValueType::Boolean, "yes"));
        assert_eq!(ok(ValueType::Object, r#"{"id": 1}"#), json!({"id": 1}));
        assert!(err(ValueType::Object, "[1]"));
        assert!(err(ValueType::Object, "id: 1"));
        assert_eq!(ok(ValueType::Array, "[1, 2]"), json!([1, 2]));
        assert!(err(ValueType::Array, "{}"));
        assert_eq!(ok(ValueType::Any, "null"), json!(null));
        assert_eq!(ok(ValueType::Any, "\"a\""), json!("a"));
        assert!(err(ValueType::Any, "a"));
        for value_type in super::super::VALUE_TYPES {
            assert_eq!(value(value_type, "  "), None, "{value_type:?}: empty");
        }
    }

    #[test]
    fn fields_are_pre_filled_from_the_session_then_the_default() {
        let port = |name: &str, value_type, default: Option<Value>| Port {
            default_value: default.map_or(OptionalJson::Missing, OptionalJson::Present),
            ..Port::new(name, value_type)
        };
        let inputs = [
            port("word", ValueType::String, Some(json!("hi"))),
            port("count", ValueType::Number, Some(json!(1))),
            port("item", ValueType::Object, None),
        ];
        assert!(RunInputsDialog::needed(&inputs));
        assert!(!RunInputsDialog::needed(&inputs[..2]));
        let dialog = RunInputsDialog::new("Echo", &inputs, &[], None);
        let texts: Vec<&str> = dialog.fields.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(texts, ["hi", "1", ""]);
        let remembered = RunInputs::from([
            ("count".to_owned(), json!(3)),
            ("item".to_owned(), json!({"a": 1})),
        ]);
        let dialog = RunInputsDialog::new("Echo", &inputs, &[], Some(&remembered));
        let sources: Vec<Source> = dialog.fields.iter().map(|f| f.source).collect();
        assert_eq!(
            sources,
            [Source::Default, Source::Remembered, Source::Remembered]
        );
        assert_eq!(dialog.fields[1].text, "3");
        assert_eq!(
            serde_json::from_str::<Value>(&dialog.fields[2].text).unwrap(),
            json!({"a": 1})
        );
        assert_eq!(dialog.fields[2].accessible_name(), "item · Object");
    }
}
