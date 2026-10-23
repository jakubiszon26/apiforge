//! Run reports shared by the runner and the interface.
//!
//! Field names follow the reference implementation's report JSON, so exported
//! reports stay comparable.

use crate::{
    format::{Graph, Node, OptionalJson},
    model::NodeKind,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum StepStatus {
    Passed,
    Failed,
    Error,
    Blocked,
    Skipped,
    Cancelled,
}

impl StepStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Passed => "Passed",
            Self::Failed => "Failed",
            Self::Error => "Error",
            Self::Blocked => "Blocked",
            Self::Skipped => "Skipped",
            Self::Cancelled => "Cancelled",
        }
    }

    /// Statuses that make the whole run fail.
    pub fn fails_run(self) -> bool {
        matches!(
            self,
            Self::Failed | Self::Error | Self::Blocked | Self::Cancelled
        )
    }

    /// Statuses after which dependants cannot use the step's outputs.
    pub fn is_unavailable(self) -> bool {
        matches!(
            self,
            Self::Error | Self::Blocked | Self::Skipped | Self::Cancelled
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Main,
    Cleanup,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum RunMode {
    /// Requests return their saved sample responses; nothing is sent.
    #[default]
    Simulation,
    /// Requests are sent to the configured API.
    Live,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Running,
    Passed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Passed => "Passed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
}

/// The request a step sent, after template expansion.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RequestSnapshot {
    pub method: String,
    pub url: String,
    pub headers: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "OptionalJson::is_missing")]
    pub body: OptionalJson,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StepResult {
    pub node_id: String,
    pub handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub kind: NodeKind,
    pub status: StepStatus,
    pub phase: Phase,
    /// Resolved inputs; missing values are absent.
    pub inputs: IndexMap<String, Value>,
    pub outputs: IndexMap<String, Value>,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "OptionalJson::is_missing")]
    pub expected: OptionalJson,
    #[serde(default, skip_serializing_if = "OptionalJson::is_missing")]
    pub actual: OptionalJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<RequestSnapshot>,
    /// Steps of a component's inner graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<StepResult>>,
}

impl StepResult {
    /// The label, or the handle of a node without one.
    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.handle)
    }

    /// A passed step of `node` without inputs or outputs yet.
    pub fn new(node: &Node, phase: Phase) -> Self {
        Self {
            node_id: node.id.clone(),
            handle: node.handle.clone(),
            label: node.label.clone(),
            kind: node.kind,
            status: StepStatus::Passed,
            phase,
            inputs: IndexMap::new(),
            outputs: IndexMap::new(),
            duration_ms: 0,
            message: None,
            expected: OptionalJson::Missing,
            actual: OptionalJson::Missing,
            request: None,
            children: None,
        }
    }
}

/// The graphs a run executed, frozen when it started.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RunSnapshot {
    pub graph: Graph,
    pub cleanup: Graph,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub id: String,
    pub scenario_id: String,
    pub mode: RunMode,
    /// RFC 3339 timestamps.
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    pub status: RunStatus,
    pub steps: Vec<StepResult>,
    /// A partial run (up to a node, or a single step); it never passes the full test.
    #[serde(default)]
    pub diagnostic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<RunSnapshot>,
    /// A run-level failure, such as an exceeded budget or a cleanup error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Provenance information that is not an error, e.g. that a single step used
    /// inputs from an earlier run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

impl RunResult {
    pub fn step(&self, node_id: &str) -> Option<&StepResult> {
        self.steps.iter().find(|step| step.node_id == node_id)
    }
}
