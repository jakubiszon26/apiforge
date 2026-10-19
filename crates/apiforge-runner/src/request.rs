//! What to run, with which limits, and validation before anything executes.

use crate::{
    http::{HttpTransport, TransportError},
    support::{SeededRandom, is_rfc3339},
};
use apiforge_core::{
    ImportError,
    format::Project,
    js::utf16_len,
    operator::RandomSource,
    run::{RunMode, RunResult, RunStatus},
    validate_project,
};
use indexmap::IndexMap;
use std::{collections::HashSet, fmt, time::Duration};
use thiserror::Error;

/// A request to run one scenario of a frozen project.
#[derive(Clone, Debug, PartialEq)]
pub struct RunRequest {
    pub project: Project,
    pub scenario_id: String,
    pub mode: RunMode,
    /// Run only this node and every node it depends on.
    pub target_node_id: Option<String>,
    /// Run only this node, reading its inputs from `previous_run`.
    pub single_node_id: Option<String>,
    /// An earlier, already redacted report of the same scenario and mode.
    pub previous_run: Option<RunResult>,
    /// Session secrets: environment values that are never published.
    pub secrets: IndexMap<String, String>,
}

impl RunRequest {
    /// A full run of a scenario without session secrets.
    pub fn new(project: Project, scenario_id: impl Into<String>, mode: RunMode) -> Self {
        Self {
            project,
            scenario_id: scenario_id.into(),
            mode,
            target_node_id: None,
            single_node_id: None,
            previous_run: None,
            secrets: IndexMap::new(),
        }
    }
}

/// Execution limits. The defaults are those of the reference implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunLimits {
    /// Executed steps, including steps inside components.
    pub max_steps: usize,
    /// Serialized size of all step results together.
    pub max_report_bytes: usize,
    /// Time budget of the main phase; exceeding it cancels the remaining steps.
    pub main_budget: Duration,
    /// Independent time budget of the cleanup phase.
    pub cleanup_budget: Duration,
    /// Request timeout when a request node does not configure one.
    pub default_request_timeout: Duration,
    pub max_response_bytes: usize,
    pub max_response_depth: usize,
    pub max_component_depth: usize,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            max_steps: 1000,
            max_report_bytes: 16 * 1024 * 1024,
            main_budget: Duration::from_secs(120),
            cleanup_budget: Duration::from_secs(10),
            default_request_timeout: Duration::from_secs(10),
            max_response_bytes: 2 * 1024 * 1024,
            max_response_depth: 100,
            max_component_depth: 16,
        }
    }
}

/// Everything a run needs besides the request itself.
pub struct RunOptions {
    pub limits: RunLimits,
    pub transport: HttpTransport,
    /// Source of the `random` operator.
    pub random: Box<dyn RandomSource + Send>,
}

impl RunOptions {
    /// Default limits and an unpredictably seeded random source.
    pub fn new(transport: HttpTransport) -> Self {
        Self {
            limits: RunLimits::default(),
            transport,
            random: Box::new(SeededRandom::from_entropy()),
        }
    }
}

impl fmt::Debug for RunOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunOptions")
            .field("limits", &self.limits)
            .field("transport", &self.transport)
            .finish_non_exhaustive()
    }
}

/// A run that cannot start. Nothing has been executed when this is returned.
#[derive(Debug, Error)]
pub enum RunError {
    #[error("Invalid project: {0}")]
    InvalidProject(#[from] ImportError),
    #[error("Scenario `{0}` does not exist")]
    ScenarioNotFound(String),
    #[error("Node `{0}` does not exist in the scenario")]
    NodeNotFound(String),
    #[error("Choose either a target node or a single node, not both")]
    ConflictingModes,
    #[error("Historical inputs are only available for a single step")]
    PreviousRunWithoutSingleNode,
    #[error("Invalid previous run: {0}")]
    InvalidPreviousRun(&'static str),
    #[error("Secret `{0}` exceeds 100000 UTF-16 units")]
    SecretTooLong(String),
    #[error("At most {0} runs can be active at once")]
    TooManyActiveRuns(usize),
    #[error(transparent)]
    Transport(#[from] TransportError),
}

/// Most steps an earlier report may contribute to a single-step run.
const MAX_PREVIOUS_STEPS: usize = 400;

/// Checks the request with the import rules plus the run-specific rules.
pub fn validate(request: &RunRequest) -> Result<(), RunError> {
    validate_project(&request.project)?;
    for (name, value) in &request.secrets {
        if utf16_len(value) > 100_000 {
            return Err(RunError::SecretTooLong(name.clone()));
        }
    }
    let scenario = request
        .project
        .scenarios
        .iter()
        .find(|scenario| scenario.id == request.scenario_id)
        .ok_or_else(|| RunError::ScenarioNotFound(request.scenario_id.clone()))?;
    if request.target_node_id.is_some() && request.single_node_id.is_some() {
        return Err(RunError::ConflictingModes);
    }
    for id in [&request.target_node_id, &request.single_node_id]
        .into_iter()
        .flatten()
    {
        if !scenario.graph.nodes.iter().any(|node| &node.id == id) {
            return Err(RunError::NodeNotFound(id.clone()));
        }
    }
    if let Some(previous) = &request.previous_run {
        if request.single_node_id.is_none() {
            return Err(RunError::PreviousRunWithoutSingleNode);
        }
        validate_previous(request, previous)?;
    }
    Ok(())
}

fn validate_previous(request: &RunRequest, previous: &RunResult) -> Result<(), RunError> {
    let invalid = RunError::InvalidPreviousRun;
    if previous.id.is_empty() || utf16_len(&previous.id) > 100_000 {
        return Err(invalid("missing identifier"));
    }
    if previous.scenario_id != request.scenario_id {
        return Err(invalid("it belongs to another scenario"));
    }
    if previous.mode != request.mode {
        return Err(invalid("it ran in another mode"));
    }
    if previous.status == RunStatus::Running {
        return Err(invalid("it has not finished"));
    }
    if !previous.finished_at.as_deref().is_some_and(is_rfc3339) {
        return Err(invalid("missing or malformed finish time"));
    }
    if previous.steps.len() > MAX_PREVIOUS_STEPS {
        return Err(invalid("too many steps"));
    }
    let mut seen = HashSet::new();
    if !previous.steps.iter().all(|step| seen.insert(&step.node_id)) {
        return Err(invalid("duplicate step identifier"));
    }
    Ok(())
}
