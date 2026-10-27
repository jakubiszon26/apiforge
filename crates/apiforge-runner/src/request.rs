//! What to run, with which limits, and validation before anything executes.

use crate::{
    http::{HttpTransport, TransportError},
    support::{SeededRandom, is_rfc3339},
};
use apiforge_core::{
    ImportError,
    format::Project,
    journal::JournalSink,
    js::utf16_len,
    operator::RandomSource,
    run::{RunMode, RunResult, RunStatus},
    validate_project,
};
use indexmap::IndexMap;
use serde_json::Value;
use std::{collections::HashSet, fmt, sync::Arc, time::Duration};
use thiserror::Error;

/// What a run executes (KTLO-23).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum RunTarget {
    /// The scenario that `RunRequest::scenario_id` names.
    #[default]
    Scenario,
    /// The component that `RunRequest::scenario_id` names, at `revision`, on
    /// its own: `inputs` are its arguments, the defaults written in code fill
    /// the rest, and there is no cleanup phase.
    Component {
        revision: u32,
        inputs: IndexMap<String, Value>,
    },
}

/// A request to run one scenario, or one component, of a frozen project.
#[derive(Clone, Debug, PartialEq)]
pub struct RunRequest {
    pub project: Project,
    /// The scenario, or with a component target the component, to run.
    pub scenario_id: String,
    pub target: RunTarget,
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
            target: RunTarget::Scenario,
            mode,
            target_node_id: None,
            single_node_id: None,
            previous_run: None,
            secrets: IndexMap::new(),
        }
    }
}

impl RunRequest {
    /// A full run of a component's revision with its input defaults.
    pub fn component(
        project: Project,
        component_id: impl Into<String>,
        revision: u32,
        mode: RunMode,
    ) -> Self {
        Self {
            target: RunTarget::Component {
                revision,
                inputs: IndexMap::new(),
            },
            ..Self::new(project, component_id, mode)
        }
    }
}

/// Execution limits. The defaults match the recorded run corpus.
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
    /// Records live requests before they are sent, so that a process which dies
    /// mid-run leaves evidence of what the tested API may have performed. `None`
    /// disables journalling, which is correct for a simulation and is the only
    /// option for a session with no workspace to keep a journal beside.
    pub journal: Option<Arc<dyn JournalSink>>,
}

impl RunOptions {
    /// Default limits, an unpredictably seeded random source and no journal.
    pub fn new(transport: HttpTransport) -> Self {
        Self {
            limits: RunLimits::default(),
            transport,
            random: Box::new(SeededRandom::from_entropy()),
            journal: None,
        }
    }

    /// The same options with a journal for live runs.
    #[must_use]
    pub fn with_journal(mut self, journal: Arc<dyn JournalSink>) -> Self {
        self.journal = Some(journal);
        self
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
    #[error("Component `{0}` does not exist")]
    ComponentNotFound(String),
    #[error("Component input `{0}` does not exist")]
    InputNotFound(String),
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
    match request.target {
        // A component's run holds no scenario; the project rules ask for one.
        RunTarget::Component { .. } if request.project.scenarios.is_empty() => {
            let mut project = request.project.clone();
            project
                .scenarios
                .push(apiforge_core::demos::empty_scenario("Component run"));
            validate_project(&project)?;
        }
        _ => validate_project(&request.project)?,
    }
    for (name, value) in &request.secrets {
        if utf16_len(value) > 100_000 {
            return Err(RunError::SecretTooLong(name.clone()));
        }
    }
    let graph = match &request.target {
        RunTarget::Scenario => {
            &request
                .project
                .scenarios
                .iter()
                .find(|scenario| scenario.id == request.scenario_id)
                .ok_or_else(|| RunError::ScenarioNotFound(request.scenario_id.clone()))?
                .graph
        }
        RunTarget::Component { revision, inputs } => {
            let component = target_component(request, *revision)?;
            if let Some(name) = inputs
                .keys()
                .find(|name| !component.inputs.iter().any(|port| &&port.name == name))
            {
                return Err(RunError::InputNotFound(name.clone()));
            }
            &component.graph
        }
    };
    if request.target_node_id.is_some() && request.single_node_id.is_some() {
        return Err(RunError::ConflictingModes);
    }
    for id in [&request.target_node_id, &request.single_node_id]
        .into_iter()
        .flatten()
    {
        if !graph.nodes.iter().any(|node| &node.id == id) {
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

/// The component revision a component target runs.
pub(crate) fn target_component(
    request: &RunRequest,
    revision: u32,
) -> Result<&apiforge_core::format::Component, RunError> {
    request
        .project
        .components
        .iter()
        .find(|component| component.id == request.scenario_id && component.revision == revision)
        .ok_or_else(|| RunError::ComponentNotFound(format!("{}@{revision}", request.scenario_id)))
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
