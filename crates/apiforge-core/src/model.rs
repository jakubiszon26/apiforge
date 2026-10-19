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
    Select,
    Build,
    Formula,
    Assert,
    Condition,
    Component,
    Variable,
    Trigger,
    If,
}
impl NodeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Request => "Request HTTP",
            Self::Select => "Wybierz dane",
            Self::Build => "Zbuduj dane",
            Self::Formula => "Operator",
            Self::Assert => "Porównanie",
            Self::Condition => "Warunek danych",
            Self::Component => "Komponent",
            Self::Variable => "Zmienna",
            Self::Trigger => "Start",
            Self::If => "IF / ELSE",
        }
    }
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
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BuildType {
    #[default]
    Object,
    Array,
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
