//! Explicit domain vocabulary shared by import, the future runner and native UI.
//!
//! A value that may be absent is an `Option<Value>`: `None` is a missing field and
//! `Some(Value::Null)` is a present JSON null. Never use `Value` indexing, which
//! turns a missing field into null.
use serde::{Deserialize, Serialize};
use std::fmt;

pub use crate::operator::Operator;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Request,
    /// A named value: data, a selection or a calculation ([ADR 0004]).
    ///
    /// [ADR 0004]: ../../../docs/decisions/0004-document-format-v2.md
    Value,
    Assert,
    Condition,
    Component,
    /// An output of the component whose graph holds it.
    Export,
    Trigger,
    If,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ValueType {
    Any,
    String,
    Number,
    Boolean,
    Object,
    Array,
}

impl ValueType {
    /// Name used in documents, equal to the corresponding JSON type name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Object => "object",
            Self::Array => "array",
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Predicate {
    #[default]
    Equals,
    NotEquals,
    Greater,
    Less,
    Exists,
    NotEmpty,
    Contains,
    Type,
    Approximately,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}
/// How a request sends its body.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BodyFormat {
    /// The body value as JSON, `Content-Type: application/json` by default.
    #[default]
    Json,
    /// The body value as text (`String(value)`), `Content-Type: text/plain` by default.
    Text,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Match {
    All,
    Any,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorPolicy {
    Stop,
    Continue,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Branch {
    True,
    False,
}
