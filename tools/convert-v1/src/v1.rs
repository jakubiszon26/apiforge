//! The version-one document format, frozen as it was when version 2 replaced it.
//!
//! These types only read version-one documents for conversion. Unknown fields are
//! rejected, so nothing in a source document is silently dropped.
use apiforge_core::model::{
    Branch, ErrorPolicy, HttpMethod, Match, Operator, Predicate, ValueType,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BuildType {
    #[default]
    Object,
    Array,
}

pub type Bindings = IndexMap<String, Binding>;

// Missing optional metadata is allowed. Explicit null in typed metadata is rejected,
// otherwise deserializing and exporting would silently erase it.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Binding {
    Expression {
        operator: Operator,
        inputs: Bindings,
    },
    Literal {
        value: Value,
    },
    Reference {
        #[serde(rename = "nodeId")]
        node_id: String,
        port: String,
        path: String,
    },
    Parameter {
        name: String,
    },
    Environment {
        name: String,
    },
}
// JSON defaults need explicit presence: Option<Value> would erase present(null).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Port {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: ValueType,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub label: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub contract_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "OptionalJson::is_missing")]
    pub default_value: OptionalJson,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub exposed: Option<bool>,
}

/// Missing and present(null) must round-trip independently in config/default fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum OptionalJson {
    #[default]
    Missing,
    Present(Value),
}
impl OptionalJson {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}
impl Serialize for OptionalJson {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Missing => serializer.serialize_none(),
            Self::Present(value) => value.serialize(serializer),
        }
    }
}
impl<'de> Deserialize<'de> for OptionalJson {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Value::deserialize(deserializer).map(Self::Present)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeConfig {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub method: Option<HttpMethod>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub url: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub headers: Option<IndexMap<String, String>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub query: Option<IndexMap<String, String>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeout_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "OptionalJson::is_missing")]
    pub sample_response: OptionalJson,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub sample_status: Option<u16>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub contract_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub path: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub operation: Option<Operator>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub predicate: Option<Predicate>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub tolerance: Option<f64>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub build_type: Option<BuildType>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub component_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub revision: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_ports: Option<Vec<Port>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub template_inputs: Option<Vec<Port>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub predicates: Option<Vec<ConditionPredicate>>,
    #[serde(
        rename = "match",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub match_mode: Option<Match>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConditionPredicate {
    pub input: String,
    pub expected: String,
    pub predicate: Predicate,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub path: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub position: Position,
    pub inputs: Bindings,
    pub config: NodeConfig,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OrderEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub branch: Option<Branch>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<OrderEdge>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub description: String,
    pub graph: Graph,
    pub cleanup: Graph,
    pub parameters: IndexMap<String, Value>,
    pub error_policy: ErrorPolicy,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Environment {
    pub base_url: String,
    pub variables: IndexMap<String, String>,
    pub secret_names: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub id: String,
    pub name: String,
    pub sample: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OutputPort {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: ValueType,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub label: Option<String>,
    #[serde(
        rename = "contractId",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub contract_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub required: Option<bool>,
    #[serde(
        rename = "defaultValue",
        default,
        skip_serializing_if = "OptionalJson::is_missing"
    )]
    pub default_value: OptionalJson,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub exposed: Option<bool>,
    pub binding: Binding,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComponentAction {
    pub input: String,
    #[serde(rename = "type")]
    pub value_type: ValueType,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub contract_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub revision: u32,
    pub name: String,
    pub description: String,
    pub inputs: Vec<Port>,
    pub outputs: Vec<OutputPort>,
    pub graph: Graph,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub action: Option<ComponentAction>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub scenarios: Vec<Scenario>,
    pub components: Vec<Component>,
    pub contracts: Vec<Contract>,
    pub environment: Environment,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workspace {
    pub version: u32,
    pub active_id: String,
    pub projects: Vec<Project>,
}
