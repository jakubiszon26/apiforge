//! Version-one wire format. Unknown fields are rejected rather than silently lost.
use crate::model::*;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

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
impl Binding {
    pub fn references<'a>(&'a self, result: &mut Vec<(&'a str, &'a str)>) {
        match self {
            Self::Reference { node_id, port, .. } => result.push((node_id, port)),
            Self::Expression { inputs, .. } => inputs
                .values()
                .for_each(|binding| binding.references(result)),
            _ => {}
        }
    }
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

/// Runtime operation types are separate from the permissive legacy config layout.
#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Trigger,
    Request {
        method: HttpMethod,
        url: String,
        timeout_ms: f64,
    },
    Select {
        path: String,
    },
    Build {
        kind: BuildType,
    },
    Formula {
        operator: Operator,
    },
    Assert {
        predicate: Predicate,
    },
    Condition {
        predicate: Predicate,
    },
    If {
        predicate: Predicate,
    },
    Component {
        id: String,
        revision: u32,
    },
    Variable,
}
impl Node {
    /// Only use after import validation; missing component identity is an error.
    pub fn operation(&self) -> Option<Operation> {
        let c = &self.config;
        Some(match self.kind {
            NodeKind::Trigger => Operation::Trigger,
            NodeKind::Request => Operation::Request {
                method: c.method.unwrap_or(HttpMethod::Get),
                url: c.url.clone().unwrap_or_default(),
                timeout_ms: c.timeout_ms.unwrap_or(10_000.0),
            },
            NodeKind::Select => Operation::Select {
                path: c.path.clone().unwrap_or_default(),
            },
            NodeKind::Build => Operation::Build {
                kind: c.build_type.unwrap_or_default(),
            },
            NodeKind::Formula => Operation::Formula {
                operator: c.operation.unwrap_or(Operator::Multiply),
            },
            NodeKind::Assert => Operation::Assert {
                predicate: c.predicate.unwrap_or_default(),
            },
            NodeKind::Condition => Operation::Condition {
                predicate: c.predicate.unwrap_or_default(),
            },
            NodeKind::If => Operation::If {
                predicate: c.predicate.unwrap_or_default(),
            },
            NodeKind::Component => Operation::Component {
                id: c.component_id.clone()?,
                revision: c.revision?,
            },
            NodeKind::Variable => Operation::Variable,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ImportLimits {
    pub max_bytes: usize,
}
impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("Document exceeds the {limit}-byte import limit")]
    TooLarge { limit: usize },
    // Never include raw input fragments (which may contain credentials) in diagnostics.
    #[error("Invalid JSON or unsupported fields at line {line}, column {column}")]
    InvalidFormat { line: usize, column: usize },
    #[error("{path}: {message}")]
    InvalidDocument { path: String, message: &'static str },
}
impl ImportError {
    pub(crate) fn invalid(path: impl Into<String>, message: &'static str) -> Self {
        Self::InvalidDocument {
            path: path.into(),
            message,
        }
    }
}
#[derive(Clone)]
pub struct ValidatedDocument {
    workspace: Workspace,
    standalone: bool,
}
impl ValidatedDocument {
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        if self.standalone {
            serde_json::to_vec_pretty(&self.workspace.projects[0])
        } else {
            serde_json::to_vec_pretty(&self.workspace)
        }
    }
}

pub fn import_document(
    bytes: &[u8],
    limits: &ImportLimits,
) -> Result<ValidatedDocument, ImportError> {
    if bytes.len() > limits.max_bytes {
        return Err(ImportError::TooLarge {
            limit: limits.max_bytes,
        });
    }
    let parse_error = |error: serde_json::Error| ImportError::InvalidFormat {
        line: error.line(),
        column: error.column(),
    };
    // First inspect the envelope without an untagged enum, which hides useful parse failures.
    let value = serde_json::from_slice::<crate::strict_json::StrictValue>(bytes)
        .map_err(parse_error)?
        .0;
    if value.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(ImportError::invalid(
            "version",
            "Unsupported document version",
        ));
    }
    let standalone = value.get("projects").is_none();
    let workspace = if standalone {
        let project: Project = serde_json::from_slice(bytes).map_err(parse_error)?;
        Workspace {
            version: 1,
            active_id: project.id.clone(),
            projects: vec![project],
        }
    } else {
        serde_json::from_slice(bytes).map_err(parse_error)?
    };
    crate::validation::workspace(&workspace)?;
    Ok(ValidatedDocument {
        workspace,
        standalone,
    })
}
