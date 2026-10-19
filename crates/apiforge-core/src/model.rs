//! Explicit domain vocabulary shared by import, the future runner and native UI.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An absent field is not JSON null. Never use Value's indexing fallback here.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedValue {
    Missing,
    Present(Value),
}

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
#[serde(rename_all = "camelCase")]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Random,
    Round,
    Equals,
    NotEquals,
    Greater,
    Less,
    AtLeast,
    AtMost,
    And,
    Or,
    Not,
    Concat,
    Letter,
    Length,
    Contains,
    Abs,
    Floor,
    Ceiling,
    Sqrt,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Ln,
    Log,
    Exp,
    Power10,
    TypeOf,
    Exists,
    Field,
}
impl Operator {
    pub fn input_names(self) -> &'static [&'static str] {
        match self {
            Self::Not
            | Self::Length
            | Self::Abs
            | Self::Floor
            | Self::Ceiling
            | Self::Sqrt
            | Self::Sin
            | Self::Cos
            | Self::Tan
            | Self::Asin
            | Self::Acos
            | Self::Atan
            | Self::Ln
            | Self::Log
            | Self::Exp
            | Self::Power10
            | Self::TypeOf
            | Self::Exists => &["left"],
            _ => &["left", "right"],
        }
    }
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
