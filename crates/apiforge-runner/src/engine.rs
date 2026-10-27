//! Serial execution of a scenario, or of a component on its own: scheduling,
//! node operations, components, cleanup.
//!
//! Operations run one at a time and each is awaited. A node becomes ready when all
//! its data and ordering dependencies inside the graph have finished; among ready
//! nodes the order of `graph.nodes` decides. Ordering edges activate a path (IF
//! branches select one, alternatives merge with OR); data references additionally
//! require producers whose results are usable. A graph without Start keeps the
//! legacy behaviour where every root runs.
//!
//! Raw values are used for execution. Only redacted copies leave the engine.

use crate::{
    http::{self, HttpError, Reached},
    plan::ExecutionPlan,
    redact::Redactor,
    request::{RunError, RunLimits, RunOptions, RunRequest, RunTarget, target_component, validate},
    support::{byte_size, duration_label, rfc3339_millis, sentence},
};
use apiforge_core::{
    binding::Reference,
    format::{Binding, Component, Graph, Node, OptionalJson, Project, Scenario, TemplatePart},
    graph::{
        BODY_INPUT, HEADERS_INPUT, QUERY_INPUT, URL_INPUT, VALUE_INPUT, ancestors_and_self,
        component_definition, condition_inputs, condition_predicates, input_ports, predicate,
    },
    journal::{EffectState, JournalEntry, JournalRun, JournalSink},
    js::{self, js_key_order},
    model::{BodyFormat, Branch, HttpMethod, Match, NodeKind, ValueType},
    operator::{Operator, OperatorError, RandomSource},
    path,
    predicate::PredicateError,
    run::{Phase, RequestSnapshot, RunResult, RunSnapshot, RunStatus, StepResult, StepStatus},
    template,
};
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime},
};
use thiserror::Error;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use url::Url;

/// Runs a scenario to completion and returns its redacted report.
///
/// `on_update` receives a redacted snapshot after the run starts, after every
/// top-level step and when the run finishes. Cancelling `cancel` stops the main
/// phase; cleanup still runs with its own budget. An error is returned only for a
/// request that fails validation, before anything is executed.
pub async fn run_scenario(
    request: RunRequest,
    options: RunOptions,
    cancel: CancellationToken,
    on_update: impl FnMut(&RunResult) + Send,
) -> Result<RunResult, RunError> {
    validate(&request)?;
    run_validated(request, options, cancel, on_update).await
}

/// [`run_scenario`] for a request that has already been validated.
pub(crate) async fn run_validated(
    request: RunRequest,
    options: RunOptions,
    cancel: CancellationToken,
    on_update: impl FnMut(&RunResult) + Send,
) -> Result<RunResult, RunError> {
    let standalone;
    let (scenario, missing) = match &request.target {
        RunTarget::Scenario => (
            request
                .project
                .scenarios
                .iter()
                .find(|scenario| scenario.id == request.scenario_id)
                .ok_or_else(|| RunError::ScenarioNotFound(request.scenario_id.clone()))?,
            None,
        ),
        RunTarget::Component { revision, inputs } => {
            let (scenario, missing) = component_run(target_component(&request, *revision)?, inputs);
            standalone = scenario;
            (&standalone, Some(missing))
        }
    };
    let mut engine = Engine::new(&request, scenario, options, on_update);
    engine.component_inputs = missing;
    Ok(engine.run(cancel).await)
}

/// A component run on its own executes its graph as a scenario without a
/// cleanup phase, its parameters the given inputs over the defaults written in
/// code. Also returns the required inputs that have neither (KTLO-23).
fn component_run(
    component: &Component,
    inputs: &IndexMap<String, Value>,
) -> (Scenario, Vec<String>) {
    let mut parameters = IndexMap::new();
    let mut missing = Vec::new();
    for port in &component.inputs {
        match inputs.get(&port.name).or(port.default()) {
            Some(value) => {
                parameters.insert(port.name.clone(), value.clone());
            }
            None if port.is_required() => missing.push(port.name.clone()),
            None => {}
        }
    }
    let scenario = Scenario {
        id: component.id.clone(),
        name: component.name.clone(),
        description: component.description.clone(),
        graph: component.graph.clone(),
        cleanup: Graph::default(),
        parameters,
        // A component has no policy of its own; the first failure stops it.
        error_policy: apiforge_core::model::ErrorPolicy::Stop,
    };
    (scenario, missing)
}

/// Resolved inputs of a node; `None` is a missing value, distinct from JSON null.
type Values = IndexMap<String, Option<Value>>;

type BoxFuture<'s, T> = Pin<Box<dyn Future<Output = T> + Send + 's>>;

/// Cancellation by the user or by an exhausted time budget.
#[derive(Clone, Debug)]
struct Signal {
    token: CancellationToken,
    deadline: Instant,
}

impl Signal {
    fn is_aborted(&self) -> bool {
        self.token.is_cancelled() || Instant::now() >= self.deadline
    }

    async fn aborted(&self) {
        tokio::select! {
            () = self.token.cancelled() => {}
            () = tokio::time::sleep_until(self.deadline) => {}
        }
    }
}

/// Where a graph executes: its arguments, finished steps and phase.
struct Context {
    /// Scenario parameters, or a component's arguments.
    parameters: Values,
    arguments: Values,
    /// Finished steps by node ID. Cleanup shares the map with the main phase.
    outputs: HashMap<String, StepResult>,
    signal: Signal,
    /// `id@revision` of every enclosing component.
    stack: Vec<String>,
    phase: Phase,
}

/// Why a step ended with an error. Messages are shown in reports.
#[derive(Debug, Error)]
enum StepError {
    #[error("Exceeded the limit of {0} steps (including component steps).")]
    StepLimit(usize),
    #[error("Missing required input: {0}.")]
    MissingInput(String),
    #[error("Input {name}: expected type {expected}.")]
    InputType { name: String, expected: ValueType },
    #[error("{}", sentence(&.0.to_string()))]
    Predicate(#[from] PredicateError),
    #[error("{}", sentence(&.0.to_string()))]
    Operator(#[from] OperatorError),
    /// An error inside an expression binding, prefixed with the operator.
    #[error("{operator}: {message}")]
    Expression { operator: String, message: String },
    /// A field, item or template value of a composite binding resolved to nothing.
    #[error("Missing value for {0}.")]
    MissingValue(String),
    #[error("The component revision does not exist.")]
    MissingComponent,
    #[error("Component recursion or nesting too deep.")]
    ComponentDepth,
    #[error("The graph contains unresolvable dependencies.")]
    Unresolvable,
    #[error("Invalid URL.")]
    InvalidUrl,
    #[error("Only HTTP/HTTPS addresses without credentials in the URL are allowed.")]
    UrlNotAllowed,
    #[error("GET and HEAD requests cannot have a body.")]
    BodyNotAllowed,
    #[error("No sample response for the simulation.")]
    MissingSample,
    #[error("Request timed out after {0} ms.")]
    Timeout(String),
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error("Cancelled while the request was being sent.")]
    CancelledWhileSending,
    /// The journal could not record a live request, so the request was not sent.
    #[error("The run journal could not be written: {0}")]
    Journal(String),
}

impl StepError {
    /// What the error implies about the tested API, for a live run's report. `None`
    /// is an error outside the sending stage, where there is nothing to say.
    fn reached_the_api(&self) -> Option<Reached> {
        match self {
            Self::Http(error) => Some(error.reached_the_api()),
            Self::Timeout(_) | Self::CancelledWhileSending => Some(Reached::Maybe),
            Self::Journal(_) => Some(Reached::No),
            _ => None,
        }
    }
}

/// Sentences a live run's report adds to a failed request, so that a reader knows
/// whether the tested API can be holding a change.
const MAY_HAVE_HAPPENED: &str = "The API may have performed this request.";
const NOTHING_SENT: &str = "Nothing was sent to the API.";

const OUTSIDE_PATH: &str = "Step outside the active execution path.";
const UNAVAILABLE: &str = "Unavailable data or an unmet condition of an earlier step.";
const STOPPED: &str = "Stopped after an error.";
const CANCELLED: &str = "Cancelled.";

struct Engine<'a, F> {
    request: &'a RunRequest,
    project: &'a Project,
    scenario: &'a Scenario,
    /// Project variables, then session secrets, then `baseUrl`.
    env: IndexMap<String, Value>,
    limits: RunLimits,
    transport: http::HttpTransport,
    random: Box<dyn RandomSource + Send>,
    redactor: Redactor,
    /// Records live requests before they are sent; `None` for a simulation or a
    /// session that has no place to keep a journal.
    journal: Option<Arc<dyn JournalSink>>,
    /// Live requests journalled so far, counted from one.
    journal_sequence: u64,
    /// A journal write that failed after its request had been sent. The run reports
    /// it, because the entry on disk then overstates what is unknown.
    journal_error: Option<String>,
    run: RunResult,
    /// A component run on its own: its required inputs without a value. A step
    /// of its graph that reads one fails as a missing input. `None` for a
    /// scenario, which also has a cleanup phase.
    component_inputs: Option<Vec<String>>,
    executed_steps: usize,
    report_bytes: usize,
    on_update: F,
}

impl<'a, F: FnMut(&RunResult) + Send> Engine<'a, F> {
    fn new(
        request: &'a RunRequest,
        scenario: &'a Scenario,
        options: RunOptions,
        on_update: F,
    ) -> Self {
        let project = &request.project;
        let environment = &project.environment;
        let mut env: IndexMap<String, Value> = environment
            .variables
            .iter()
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect();
        for (name, value) in &request.secrets {
            env.insert(name.clone(), Value::String(value.clone()));
        }
        env.insert(
            "baseUrl".into(),
            Value::String(environment.base_url.clone()),
        );

        let mut redactor = Redactor::new();
        for secret in request.secrets.values() {
            redactor.add(secret);
        }
        for name in &environment.secret_names {
            if let Some(Value::String(secret)) = env.get(name) {
                redactor.add(secret);
            }
        }
        if let Ok(document) = serde_json::to_value(project) {
            redactor.collect(&document);
        }

        let run = RunResult {
            id: uuid::Uuid::new_v4().to_string(),
            scenario_id: scenario.id.clone(),
            mode: request.mode,
            started_at: rfc3339_millis(SystemTime::now()),
            finished_at: None,
            status: RunStatus::Running,
            steps: Vec::new(),
            diagnostic: request.single_node_id.is_some() || request.target_node_id.is_some(),
            snapshot: Some(RunSnapshot {
                graph: scenario.graph.clone(),
                cleanup: scenario.cleanup.clone(),
            }),
            error: None,
            notice: None,
        };
        Self {
            request,
            project,
            scenario,
            env,
            limits: options.limits,
            transport: options.transport,
            random: options.random,
            redactor,
            // A simulation sends nothing, so there is nothing to record.
            journal: options
                .journal
                .filter(|_| request.mode == apiforge_core::run::RunMode::Live),
            journal_sequence: 0,
            journal_error: None,
            run,
            component_inputs: None,
            executed_steps: 0,
            report_bytes: 0,
            on_update,
        }
    }

    fn notify(&mut self) {
        let snapshot = self.redactor.redact_run(&self.run);
        (self.on_update)(&snapshot);
    }

    async fn run(&mut self, cancel: CancellationToken) -> RunResult {
        let request = self.request;
        let scenario = self.scenario;
        let main_deadline = Instant::now() + self.limits.main_budget;
        let mut context = Context {
            parameters: scenario
                .parameters
                .iter()
                .map(|(name, value)| (name.clone(), Some(value.clone())))
                .collect(),
            arguments: Values::new(),
            outputs: HashMap::new(),
            signal: Signal {
                token: cancel.clone(),
                deadline: main_deadline,
            },
            stack: Vec::new(),
            phase: Phase::Main,
        };

        let selection: Option<IndexSet<String>> = if let Some(single) = &request.single_node_id {
            // History is redacted; it shows what an earlier run saw, not real secrets.
            for step in request.previous_run.iter().flat_map(|run| &run.steps) {
                if step.phase == Phase::Main && &step.node_id != single {
                    context.outputs.insert(step.node_id.clone(), step.clone());
                }
            }
            self.run.notice = Some(match &request.previous_run {
                Some(previous) => format!(
                    "Historical inputs from run {}, {}.",
                    previous.id,
                    previous.finished_at.as_deref().unwrap_or("unknown time")
                ),
                None => "Single step: no historical inputs.".to_owned(),
            });
            Some(IndexSet::from([single.clone()]))
        } else {
            request
                .target_node_id
                .as_deref()
                .map(|target| ancestors_and_self(&scenario.graph, target))
        };
        self.notify();

        // A live run announces itself before it can send anything, so a process that
        // dies leaves the run's identity, scenario and start time behind. A journal
        // that cannot be written stops the run before the first request.
        if let Err(error) = self.journal_begin() {
            self.run.error = Some(error.to_string());
            self.run.status = RunStatus::Failed;
            self.run.finished_at = Some(rfc3339_millis(SystemTime::now()));
            self.notify();
            return self.redactor.redact_run(&self.run);
        }

        if let Err(error) = self
            .execute_graph(&scenario.graph, &mut context, true, selection.as_ref())
            .await
        {
            self.run.error = Some(error.to_string());
        }
        // Freeze the main phase verdict before cleanup starts. Cleanup has an
        // independent deadline and must not turn a successful main phase into a timeout.
        let main_budget_exceeded = Instant::now() >= main_deadline;
        // Cleanup gets its own budget, unaffected by cancellation of the main phase.
        // A component run on its own has no cleanup phase.
        if self.component_inputs.is_none() {
            self.journal_cleanup_started();
            context.phase = Phase::Cleanup;
            context.signal = Signal {
                token: CancellationToken::new(),
                deadline: Instant::now() + self.limits.cleanup_budget,
            };
            if let Err(error) = self
                .execute_graph(&scenario.cleanup, &mut context, true, None)
                .await
            {
                let cleanup = format!("Cleanup error: {error}");
                self.run.error = Some(match self.run.error.take() {
                    Some(previous) => format!("{previous} {cleanup}"),
                    None => cleanup,
                });
            }
        }
        if main_budget_exceeded {
            self.run.error = Some(format!(
                "The run exceeded the {} time limit.",
                duration_label(self.limits.main_budget)
            ));
        }
        self.run.status = if cancel.is_cancelled() {
            RunStatus::Cancelled
        } else if self.run.steps.iter().any(|step| step.status.fails_run())
            || self.run.error.is_some()
        {
            RunStatus::Failed
        } else {
            RunStatus::Passed
        };
        self.run.finished_at = Some(rfc3339_millis(SystemTime::now()));
        // The report is the record of a run that reached its end, so the journal
        // drops it. Only an interrupted run stays on disk.
        self.journal_finish();
        if let Some(problem) = self.journal_error.take() {
            let note = format!("Run journal: {problem}");
            self.run.error = Some(match self.run.error.take() {
                Some(previous) => format!("{previous} {note}"),
                None => note,
            });
        }
        self.notify();
        self.redactor.redact_run(&self.run)
    }

    /// Opens the journal entry of a live run. Simulations have no journal.
    fn journal_begin(&mut self) -> Result<(), StepError> {
        let Some(sink) = self.journal.clone() else {
            return Ok(());
        };
        let run = JournalRun::new(
            &self.run.id,
            &self.scenario.id,
            self.redactor.redact_str(&self.scenario.name),
            &self.run.started_at,
        );
        sink.begin_run(&run)
            .map_err(|error| StepError::Journal(error.to_string()))
    }

    /// Records one live request before it is sent.
    fn journal_record(&mut self, entry: &JournalEntry) -> Result<(), StepError> {
        let Some(sink) = self.journal.clone() else {
            return Ok(());
        };
        sink.record(&self.run.id, entry)
            .map_err(|error| StepError::Journal(error.to_string()))
    }

    /// Resolves what is known about a request that has already been sent.
    ///
    /// Refusing to send is no longer an option here, so a failed write is reported
    /// rather than fatal: the entry keeps its earlier state, which overstates the
    /// uncertainty instead of hiding it.
    fn journal_resolve(
        &mut self,
        entry: &mut JournalEntry,
        state: EffectState,
        status: Option<u16>,
        detail: Option<String>,
    ) {
        let Some(sink) = self.journal.clone() else {
            return;
        };
        entry.state = state;
        entry.status = status;
        entry.detail = detail.map(|detail| self.redactor.redact_str(&detail));
        if let Err(error) = sink.record(&self.run.id, entry) {
            self.journal_error.get_or_insert_with(|| error.to_string());
        }
    }

    fn journal_cleanup_started(&mut self) {
        let Some(sink) = self.journal.clone() else {
            return;
        };
        if let Err(error) = sink.cleanup_started(&self.run.id) {
            self.journal_error.get_or_insert_with(|| error.to_string());
        }
    }

    fn journal_finish(&mut self) {
        let Some(sink) = self.journal.clone() else {
            return;
        };
        if let Err(error) = sink.finish_run(&self.run.id) {
            self.journal_error.get_or_insert_with(|| error.to_string());
        }
    }

    /// Executes the nodes of a graph (or the `selected` subset) in dependency order.
    fn execute_graph<'s>(
        &'s mut self,
        graph: &'a Graph,
        context: &'s mut Context,
        publish: bool,
        selected: Option<&'s IndexSet<String>>,
    ) -> BoxFuture<'s, Result<Vec<StepResult>, StepError>>
    where
        'a: 's,
    {
        Box::pin(async move {
            let plan = ExecutionPlan::new(graph);
            let mut pending: Vec<&'a Node> = graph
                .nodes
                .iter()
                .filter(|node| selected.is_none_or(|selected| selected.contains(&node.id)))
                .collect();
            let graph_ids: HashSet<&str> = pending.iter().map(|node| node.id.as_str()).collect();
            let mut results = Vec::with_capacity(pending.len());
            let mut stopped = false;
            while !pending.is_empty() {
                let index = pending
                    .iter()
                    .position(|node| {
                        plan.dependencies_of(&node.id)
                            .all(|id| !graph_ids.contains(id) || context.outputs.contains_key(id))
                    })
                    .ok_or(StepError::Unresolvable)?;
                let node = pending.remove(index);
                let bypass_gate = self.request.single_node_id.as_deref() == Some(node.id.as_str())
                    && selected.is_some_and(|selected| selected.len() == 1);
                let gate = self.gate(graph, &plan, node, context, bypass_gate);
                let status = if context.signal.is_aborted() {
                    Some(StepStatus::Cancelled)
                } else if gate.inactive || stopped || gate.missing_cleanup_input {
                    Some(StepStatus::Skipped)
                } else if gate.blocked {
                    Some(match context.phase {
                        Phase::Cleanup => StepStatus::Skipped,
                        Phase::Main => StepStatus::Blocked,
                    })
                } else {
                    None
                };
                let step = match status {
                    Some(status) => {
                        let mut step = StepResult::new(node, context.phase);
                        step.status = status;
                        step.message = Some(
                            if gate.inactive {
                                OUTSIDE_PATH
                            } else if status == StepStatus::Cancelled {
                                CANCELLED
                            } else if gate.blocked || gate.missing_cleanup_input {
                                UNAVAILABLE
                            } else {
                                STOPPED
                            }
                            .to_owned(),
                        );
                        step
                    }
                    None => self.execute(node, context).await,
                };
                if self.scenario.error_policy == apiforge_core::model::ErrorPolicy::Stop
                    && context.phase == Phase::Main
                    && matches!(
                        step.status,
                        StepStatus::Failed | StepStatus::Error | StepStatus::Blocked
                    )
                {
                    stopped = true;
                }
                context.outputs.insert(node.id.clone(), step.clone());
                if publish {
                    self.run.steps.push(step.clone());
                    self.notify();
                }
                results.push(step);
            }
            Ok(results)
        })
    }

    /// Decides whether a ready node is on the active path and has usable inputs.
    fn gate(
        &self,
        graph: &Graph,
        plan: &ExecutionPlan<'_>,
        node: &Node,
        context: &Context,
        bypass: bool,
    ) -> Gate {
        let dependencies: Vec<&str> = plan.dependencies_of(&node.id).collect();
        let incoming: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.target == node.id)
            .collect();
        let takes_branch = |branch: Option<Branch>, source: &StepResult| {
            source.outputs.get("passed") == Some(&Value::Bool(branch == Some(Branch::True)))
        };
        let active_order = incoming
            .iter()
            .filter(|edge| {
                context.outputs.get(&edge.source).is_some_and(|source| {
                    !matches!(
                        source.status,
                        StepStatus::Skipped
                            | StepStatus::Blocked
                            | StepStatus::Error
                            | StepStatus::Cancelled
                    ) && (edge.branch.is_none() || takes_branch(edge.branch, source))
                })
            })
            .count();
        // Alternative branches merge with OR. An inactive branch is never a failure.
        let inactive = !bypass
            && (!plan.is_reachable(&node.id)
                || (!incoming.is_empty()
                    && active_order == 0
                    && incoming.iter().all(|edge| {
                        context.outputs.get(&edge.source).is_some_and(|source| {
                            source.status == StepStatus::Skipped
                                || (edge.branch.is_some()
                                    && source.status == StepStatus::Passed
                                    && !takes_branch(edge.branch, source))
                        })
                    }))
                || (incoming.is_empty()
                    && dependencies.iter().any(|id| {
                        context
                            .outputs
                            .get(*id)
                            .is_some_and(|source| source.status == StepStatus::Skipped)
                    })));
        let references: Vec<Reference<'_>> =
            node.inputs.values().flat_map(Binding::references).collect();
        let blocked = dependencies.iter().any(|id| {
            let Some(source) = context.outputs.get(*id) else {
                return true;
            };
            let mut used = references
                .iter()
                .filter(|reference| reference.node_id == *id)
                .peekable();
            let data_dependency = used.peek().is_some();
            if !data_dependency && active_order > 0 && source.status == StepStatus::Skipped {
                return false;
            }
            if context.phase == Phase::Cleanup {
                // A component may create a resource and fail later. Exported resource
                // IDs remain usable for cleanup, while main-phase steps stay blocked.
                // An order-only cleanup dependency means "after completion".
                if !data_dependency || used.all(|reference| is_available(reference, context)) {
                    return false;
                }
            }
            source.status.is_unavailable()
        });
        let missing_cleanup_input = context.phase == Phase::Cleanup
            && references
                .iter()
                .any(|reference| !is_available(reference, context));
        Gate {
            inactive,
            blocked,
            missing_cleanup_input,
        }
    }

    /// Executes one node. Failures become the step's status and message.
    async fn execute(&mut self, node: &'a Node, context: &mut Context) -> StepResult {
        let started = Instant::now();
        let mut step = StepResult::new(node, context.phase);
        if let Err(error) = self.operate(node, context, &mut step).await {
            step.status = if context.signal.is_aborted() {
                StepStatus::Cancelled
            } else {
                StepStatus::Error
            };
            // A live request that failed leaves the reader one question: can the
            // tested API be holding a change? The error's category answers it.
            let effect = match error.reached_the_api() {
                Some(Reached::Maybe) => Some(MAY_HAVE_HAPPENED),
                Some(Reached::No) => Some(NOTHING_SENT),
                Some(Reached::Yes) | None => None,
            };
            step.message = Some(match effect {
                Some(effect) => format!("{error} {effect}"),
                None => error.to_string(),
            });
        }
        step.duration_ms = milliseconds(started.elapsed());
        let size = serde_json::to_vec(&step).map_or(usize::MAX, |bytes| bytes.len());
        if self.report_bytes.saturating_add(size) > self.limits.max_report_bytes {
            step.status = StepStatus::Error;
            step.message = Some(format!(
                "Exceeded the run data limit of {}.",
                byte_size(self.limits.max_report_bytes)
            ));
            step.inputs.clear();
            step.outputs.clear();
            step.children = None;
            step.request = None;
            step.expected = OptionalJson::Missing;
            step.actual = OptionalJson::Missing;
        } else {
            self.report_bytes += size;
        }
        step
    }

    async fn operate(
        &mut self,
        node: &'a Node,
        context: &mut Context,
        step: &mut StepResult,
    ) -> Result<(), StepError> {
        self.executed_steps += 1;
        if self.executed_steps > self.limits.max_steps {
            return Err(StepError::StepLimit(self.limits.max_steps));
        }
        // Only the component's own graph reads its inputs; a call binds them.
        if context.stack.is_empty()
            && let Some(missing) = &self.component_inputs
            && let Some(name) =
                node.inputs
                    .values()
                    .flat_map(Binding::leaves)
                    .find_map(|(_, leaf)| match leaf {
                        Binding::Parameter { name, .. } if missing.contains(name) => Some(name),
                        _ => None,
                    })
        {
            return Err(StepError::MissingInput(name.clone()));
        }
        context.arguments.clear();
        let mut values = Values::with_capacity(node.inputs.len());
        // Resolve local arguments in dependency order, irrespective of their
        // insertion order. The document validator rejects missing names/cycles.
        if node.kind == NodeKind::Request {
            let mut pending: Vec<_> = node
                .inputs
                .iter()
                .filter(|(name, _)| apiforge_core::graph::is_request_argument(name))
                .collect();
            while !pending.is_empty() {
                let index = pending.iter().position(|(_, binding)| binding.leaves().iter().all(|(_, leaf)| {
                    !matches!(leaf, Binding::Argument { name, .. } if !context.arguments.contains_key(name))
                })).ok_or_else(|| StepError::MissingInput("cyclic request arguments".into()))?;
                let (name, binding) = pending.remove(index);
                let value = self.resolve(binding, context)?;
                context.arguments.insert(name.clone(), value.clone());
                values.insert(name.clone(), value);
            }
        }
        for (name, binding) in &node.inputs {
            if !values.contains_key(name) {
                let value = self.resolve(binding, context)?;
                values.insert(name.clone(), value);
            }
        }
        let ports = input_ports(node, self.project);
        for port in &ports {
            if values.get(&port.name).is_none_or(Option::is_none)
                && let Some(default) = port.default()
            {
                values.insert(port.name.clone(), Some(default.clone()));
            }
        }
        let values: Values = js_key_order(values).into_iter().collect();
        step.inputs = compact(&values);
        // Checks may inspect missing fields explicitly. Other operations require
        // their declared inputs.
        if !matches!(
            node.kind,
            NodeKind::Assert | NodeKind::Condition | NodeKind::If
        ) {
            for port in &ports {
                match values.get(&port.name).and_then(Option::as_ref) {
                    None if port.is_required() => {
                        return Err(StepError::MissingInput(port.name.clone()));
                    }
                    Some(value) if !has_type(value, port.value_type) => {
                        return Err(StepError::InputType {
                            name: port.name.clone(),
                            expected: port.value_type,
                        });
                    }
                    _ => {}
                }
            }
        }
        let input = |name: &str| values.get(name).cloned().flatten();
        match node.kind {
            NodeKind::Start => {}
            NodeKind::Value | NodeKind::Export => step.outputs = single_output(input(VALUE_INPUT)),
            NodeKind::Request => self.request(node, &values, step, context).await?,
            NodeKind::Assert | NodeKind::Condition | NodeKind::If => {
                self.check(node, &values, step)?
            }
            NodeKind::Component => self.component(node, values, step, context).await?,
        }
        Ok(())
    }

    fn check(&self, node: &Node, values: &Values, step: &mut StepResult) -> Result<(), StepError> {
        let config = &node.config;
        let tolerance = config.tolerance.unwrap_or(0.0);
        let input = |name: &str| values.get(name).cloned().flatten();
        // A condition's first check stands for the node in its value and report.
        let (actual_name, expected_name) = match node.kind {
            NodeKind::Condition => condition_inputs(1),
            _ => ("actual".to_owned(), "expected".to_owned()),
        };
        let passed = if node.kind == NodeKind::Condition {
            let predicates = condition_predicates(node);
            let mut checks = Vec::with_capacity(predicates.len());
            for (index, predicate) in predicates.iter().enumerate() {
                let (actual, expected) = condition_inputs(index + 1);
                checks.push(predicate.check(
                    input(&actual).as_ref(),
                    input(&expected).as_ref(),
                    tolerance,
                )?);
            }
            match config.match_mode {
                Some(Match::Any) => checks.contains(&true),
                _ => checks.iter().all(|passed| *passed),
            }
        } else {
            predicate(node).check(
                input("actual").as_ref(),
                input("expected").as_ref(),
                tolerance,
            )?
        };
        let single = node.kind != NodeKind::Condition || condition_predicates(node).len() == 1;
        let actual = match node.kind {
            NodeKind::Condition if node.inputs.contains_key(VALUE_INPUT) => input(VALUE_INPUT),
            _ => input(&actual_name),
        };
        let expected = input(&expected_name).filter(|_| single);
        step.outputs = single_output(actual.clone());
        step.outputs.insert("passed".into(), Value::Bool(passed));
        if let Some(actual) = actual {
            step.actual = OptionalJson::Present(actual);
        }
        if let Some(expected) = expected {
            step.expected = OptionalJson::Present(expected);
        }
        if node.kind == NodeKind::If {
            step.message = Some(
                if passed {
                    "Branch selected: yes."
                } else {
                    "Branch selected: no."
                }
                .to_owned(),
            );
        } else if !passed {
            let (status, message) = if node.kind == NodeKind::Condition {
                (
                    StepStatus::Blocked,
                    "Invalid data: the input condition is not met.",
                )
            } else {
                (StepStatus::Failed, "The comparison failed.")
            };
            step.status = status;
            step.message = Some(message.to_owned());
        }
        Ok(())
    }

    async fn component(
        &mut self,
        node: &'a Node,
        values: Values,
        step: &mut StepResult,
        context: &Context,
    ) -> Result<(), StepError> {
        let component =
            component_definition(self.project, node).ok_or(StepError::MissingComponent)?;
        let key = format!("{}@{}", component.id, component.revision);
        if context.stack.contains(&key) || context.stack.len() >= self.limits.max_component_depth {
            return Err(StepError::ComponentDepth);
        }
        let mut stack = context.stack.clone();
        stack.push(key);
        let mut nested = Context {
            parameters: values,
            arguments: Values::new(),
            outputs: HashMap::new(),
            signal: context.signal.clone(),
            stack,
            phase: context.phase,
        };
        let children = self
            .execute_graph(&component.graph, &mut nested, false, None)
            .await?;
        step.children = Some(children);
        // An Export that ran passes its value out under its handle.
        for export in component
            .graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Export)
        {
            if let Some(value) = nested
                .outputs
                .get(&export.id)
                .and_then(|step| step.outputs.get(VALUE_INPUT))
            {
                step.outputs.insert(export.handle.clone(), value.clone());
            }
        }
        step.outputs = js_key_order(std::mem::take(&mut step.outputs))
            .into_iter()
            .collect();
        let children = step.children.as_deref().unwrap_or_default();
        let any = |status: StepStatus| children.iter().any(|child| child.status == status);
        if let Some(status) = [
            StepStatus::Cancelled,
            StepStatus::Error,
            StepStatus::Blocked,
            StepStatus::Failed,
        ]
        .into_iter()
        .find(|status| any(*status))
        {
            step.status = status;
            step.message = Some("Check the results of the component's inner steps.".to_owned());
        }
        Ok(())
    }

    async fn request(
        &mut self,
        node: &Node,
        values: &Values,
        step: &mut StepResult,
        context: &Context,
    ) -> Result<(), StepError> {
        let config = &node.config;
        // Values interpolated into a URL template are percent-encoded; a URL from
        // any other binding is used as it is.
        let address = match node.inputs.get(URL_INPUT) {
            Some(Binding::Template { parts }) => self.render(parts, context, true)?,
            _ => values
                .get(URL_INPUT)
                .cloned()
                .flatten()
                .map(|value| template::template_text(&value))
                .unwrap_or_default(),
        };
        // A configured base URL is an API prefix, including its path.
        let address = if has_http_scheme(&address) {
            address
        } else {
            format!(
                "{}/{}",
                self.project.environment.base_url.trim_end_matches('/'),
                address.trim_start_matches('/')
            )
        };
        let mut url = Url::parse(&address).map_err(|_| StepError::InvalidUrl)?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(StepError::UrlNotAllowed);
        }
        for (name, value) in text_fields(values, QUERY_INPUT) {
            set_search_param(&mut url, &name, &value);
        }
        self.redactor.collect_query(&url);
        let headers: IndexMap<String, String> = text_fields(values, HEADERS_INPUT).collect();
        let mut headers: IndexMap<String, String> = js_key_order(headers).into_iter().collect();
        let format = config.body_format.unwrap_or_default();
        let body = match (values.get(BODY_INPUT), format) {
            (Some(Some(body)), BodyFormat::Json) => Some(body.clone()),
            (Some(Some(body)), BodyFormat::Text) => {
                Some(Value::String(template::template_text(body)))
            }
            _ => None,
        };
        if body.is_some()
            && !headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("content-type"))
        {
            let content_type = match format {
                BodyFormat::Json => "application/json",
                BodyFormat::Text => "text/plain",
            };
            headers.insert("Content-Type".into(), content_type.into());
        }
        self.redactor.collect(&text_map(&headers));
        if let Some(body) = &body {
            self.redactor.collect(body);
        }
        let method = config.method.unwrap_or(HttpMethod::Get);
        if body.is_some() && matches!(method, HttpMethod::Get | HttpMethod::Head) {
            return Err(StepError::BodyNotAllowed);
        }
        step.request = Some(RequestSnapshot {
            method: http::method_name(method).to_owned(),
            url: url.to_string(),
            headers: headers.clone(),
            body: body
                .clone()
                .map_or(OptionalJson::Missing, OptionalJson::Present),
        });

        if self.request.mode == apiforge_core::run::RunMode::Simulation {
            let OptionalJson::Present(sample) = &config.sample_response else {
                return Err(StepError::MissingSample);
            };
            step.outputs = IndexMap::from([
                ("body".to_owned(), sample.clone()),
                (
                    "status".to_owned(),
                    Value::from(config.sample_status.unwrap_or(200)),
                ),
                ("headers".to_owned(), Value::Object(Map::new())),
                ("duration".to_owned(), Value::from(0)),
            ]);
            self.redactor.collect(&outputs_value(&step.outputs));
            return Ok(());
        }

        let (timeout, timeout_label) = match config.timeout_ms {
            Some(milliseconds) => (
                Duration::try_from_secs_f64(milliseconds / 1000.0)
                    .unwrap_or(self.limits.default_request_timeout),
                js::number_to_string(milliseconds),
            ),
            None => (
                self.limits.default_request_timeout,
                self.limits.default_request_timeout.as_millis().to_string(),
            ),
        };
        let transport = self.transport.clone();
        let limit = self.limits.max_response_bytes;
        let body_text = body.as_ref().map(|body| match (format, body) {
            (BodyFormat::Text, Value::String(text)) => text.clone(),
            _ => js::json_stringify(body),
        });
        let started = Instant::now();
        // The request can change the tested API, so it is journalled before it is
        // sent and resolved on every way out of the exchange below.
        let mut entry = JournalEntry {
            sequence: self.journal_sequence + 1,
            node_id: node.id.clone(),
            handle: node.handle.clone(),
            label: node.label.clone(),
            phase: context.phase,
            method: http::method_name(method).to_owned(),
            url: self.redactor.redact_str(url.as_str()),
            started_at: rfc3339_millis(SystemTime::now()),
            state: EffectState::InFlight,
            status: None,
            detail: None,
        };
        if self.journal.is_some() {
            self.journal_record(&entry)?;
            self.journal_sequence += 1;
        }
        let exchange = async move {
            let response = transport.send(method, url, &headers, body_text).await?;
            let status = response.status().as_u16();
            let headers = http::response_headers(&response);
            let body = http::read_body(response, limit).await;
            Ok::<_, HttpError>((status, headers, body))
        };
        let outcome = tokio::select! {
            biased;
            () = context.signal.aborted() => {
                // Bytes were already on their way; only the API knows what it did.
                self.journal_resolve(
                    &mut entry,
                    EffectState::Uncertain,
                    None,
                    Some("cancelled while the request was being sent".to_owned()),
                );
                return Err(StepError::CancelledWhileSending);
            }
            outcome = tokio::time::timeout(timeout, exchange) => outcome,
        };
        let sent = match outcome {
            Err(_elapsed) => {
                self.journal_resolve(
                    &mut entry,
                    EffectState::Uncertain,
                    None,
                    Some(format!("no response within {timeout_label} ms")),
                );
                return Err(StepError::Timeout(timeout_label));
            }
            Ok(Err(error)) => {
                let state = match error.reached_the_api() {
                    Reached::No => EffectState::NotSent,
                    Reached::Maybe => EffectState::Uncertain,
                    Reached::Yes => EffectState::Completed,
                };
                let status = error.response_status();
                self.journal_resolve(&mut entry, state, status, Some(error.to_string()));
                return Err(StepError::Http(error));
            }
            Ok(Ok(sent)) => sent,
        };
        let (status, headers, body) = sent;
        // A status is proof that the API performed the request, whatever the body
        // turns out to be.
        self.journal_resolve(&mut entry, EffectState::Completed, Some(status), None);
        let headers = text_map(&headers);
        self.redactor.collect(&headers);
        let body = http::decode_body(&body?, self.limits.max_response_depth)?;
        step.outputs = IndexMap::from([
            ("body".to_owned(), body),
            ("status".to_owned(), Value::from(status)),
            ("headers".to_owned(), headers),
            (
                "duration".to_owned(),
                Value::from(milliseconds(started.elapsed())),
            ),
        ]);
        self.redactor.collect(&outputs_value(&step.outputs));
        Ok(())
    }

    /// Resolves a binding against the context; `Ok(None)` is a missing value.
    fn resolve(
        &mut self,
        binding: &Binding,
        context: &Context,
    ) -> Result<Option<Value>, StepError> {
        Ok(match binding {
            Binding::Expression { operator, inputs } => self
                .evaluate(*operator, inputs, context)
                .map_err(|error| StepError::Expression {
                operator: operator_id(*operator),
                message: error.to_string(),
            })?,
            Binding::Literal { value } => Some(value.clone()),
            Binding::Parameter { name, path: field } => context
                .parameters
                .get(name)
                .cloned()
                .flatten()
                .and_then(|value| path::resolve(&value, field).map(|value| value.into_owned())),
            Binding::Environment { name } => self.env.get(name).cloned(),
            Binding::Argument { name, path: field } => context
                .arguments
                .get(name)
                .cloned()
                .flatten()
                .and_then(|value| path::resolve(&value, field).map(|value| value.into_owned())),
            Binding::Reference {
                node_id,
                port,
                path,
            } => reference_value(
                &Reference {
                    node_id,
                    port,
                    path,
                },
                context,
            ),
            Binding::Object { fields } => {
                let mut object = Map::with_capacity(fields.len());
                for (name, child) in fields {
                    let value = self
                        .resolve(child, context)?
                        .ok_or_else(|| StepError::MissingValue(format!("field `{name}`")))?;
                    object.insert(name.clone(), value);
                }
                Some(Value::Object(object))
            }
            Binding::Array { items } => {
                let mut array = Vec::with_capacity(items.len());
                for (index, child) in items.iter().enumerate() {
                    array.push(
                        self.resolve(child, context)?
                            .ok_or_else(|| StepError::MissingValue(format!("item {index}")))?,
                    );
                }
                Some(Value::Array(array))
            }
            Binding::Template { parts } => Some(Value::String(self.render(parts, context, false)?)),
        })
    }

    /// Concatenates a template. URL templates percent-encode interpolated values
    /// like `encodeURIComponent`; their literal text is kept as written.
    fn render(
        &mut self,
        parts: &[TemplatePart],
        context: &Context,
        encode: bool,
    ) -> Result<String, StepError> {
        let mut text = String::new();
        let mut values = 0;
        for part in parts {
            match part {
                TemplatePart::Text { text: literal } => text.push_str(literal),
                TemplatePart::Value { binding } => {
                    values += 1;
                    let value = self.resolve(binding, context)?.ok_or_else(|| {
                        StepError::MissingValue(format!("template value {values}"))
                    })?;
                    let value = template::template_text(&value);
                    if encode {
                        text.push_str(&template::encode_uri_component(&value));
                    } else {
                        text.push_str(&value);
                    }
                }
            }
        }
        Ok(text)
    }

    /// Evaluates an expression eagerly: every operand is resolved, in order.
    fn evaluate(
        &mut self,
        operator: Operator,
        inputs: &IndexMap<String, Binding>,
        context: &Context,
    ) -> Result<Option<Value>, StepError> {
        let mut operands = IndexMap::with_capacity(inputs.len());
        for (name, child) in inputs {
            let value = self.resolve(child, context)?;
            operands.insert(name.as_str(), value);
        }
        let operand = |name: &str| operands.get(name).cloned().flatten();
        Ok(operator.evaluate(
            operand("left").as_ref(),
            operand("right").as_ref(),
            self.random.as_mut(),
        )?)
    }
}

/// The scheduler's verdict on a ready node.
struct Gate {
    /// Not on the active path: skipped, never a failure.
    inactive: bool,
    /// A dependency's result is unusable.
    blocked: bool,
    /// A cleanup step references data that does not exist.
    missing_cleanup_input: bool,
}

fn reference_value(reference: &Reference<'_>, context: &Context) -> Option<Value> {
    context
        .outputs
        .get(reference.node_id)
        .and_then(|step| step.outputs.get(reference.port))
        .and_then(|value| path::resolve(value, reference.path))
        .map(|value| value.into_owned())
}

fn is_available(reference: &Reference<'_>, context: &Context) -> bool {
    context
        .outputs
        .get(reference.node_id)
        .and_then(|step| step.outputs.get(reference.port))
        .is_some_and(|value| path::resolve(value, reference.path).is_some())
}

/// Fields of an object input (headers or query) as text, like `String(value)`.
fn text_fields<'v>(values: &'v Values, input: &str) -> impl Iterator<Item = (String, String)> + 'v {
    values
        .get(input)
        .and_then(Option::as_ref)
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, value)| (name.clone(), template::template_text(value)))
}

fn compact(values: &Values) -> IndexMap<String, Value> {
    values
        .iter()
        .filter_map(|(name, value)| Some((name.clone(), value.clone()?)))
        .collect()
}

fn single_output(value: Option<Value>) -> IndexMap<String, Value> {
    value
        .map(|value| IndexMap::from([("value".to_owned(), value)]))
        .unwrap_or_default()
}

fn has_type(value: &Value, expected: ValueType) -> bool {
    match expected {
        ValueType::Any => true,
        ValueType::String => value.is_string(),
        ValueType::Number => value.is_number(),
        ValueType::Boolean => value.is_boolean(),
        ValueType::Object => value.is_object(),
        ValueType::Array => value.is_array(),
    }
}

/// Whether the address is absolute: `http://` or `https://`, case-insensitively.
fn has_http_scheme(address: &str) -> bool {
    ["http://", "https://"].iter().any(|scheme| {
        address
            .get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
    })
}

/// `URLSearchParams.set`: replaces the first parameter of that name and removes the
/// others, or appends it; the whole query is then re-serialised as a form.
fn set_search_param(url: &mut Url, name: &str, value: &str) {
    let mut pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    match pairs.iter().position(|(key, _)| key == name) {
        Some(index) => {
            pairs[index].1 = value.to_owned();
            let mut position = 0;
            pairs.retain(|(key, _)| {
                position += 1;
                key != name || position - 1 == index
            });
        }
        None => pairs.push((name.to_owned(), value.to_owned())),
    }
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(&pairs)
        .finish();
    url.set_query((!query.is_empty()).then_some(query.as_str()));
}

fn text_map(values: &IndexMap<String, String>) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect(),
    )
}

fn outputs_value(outputs: &IndexMap<String, Value>) -> Value {
    Value::Object(
        outputs
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
    )
}

/// The document name of an operator, e.g. `multiply`.
fn operator_id(operator: Operator) -> String {
    match serde_json::to_value(operator) {
        Ok(Value::String(name)) => name,
        _ => format!("{operator:?}"),
    }
}

/// Whole milliseconds, rounded like `Math.round`.
fn milliseconds(duration: Duration) -> u64 {
    (duration.as_secs_f64() * 1000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_shared_argument_does_not_send_disconnected_http_requests() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let received = Arc::clone(&requests);
        let server = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0; 4096];
                let _ = stream.read(&mut buffer).await;
                received.fetch_add(1, Ordering::SeqCst);
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    )
                    .await;
            }
        });
        let mut project: Project =
            serde_json::from_slice(include_bytes!("../../../fixtures/projects/shop.json")).unwrap();
        project.scenarios.truncate(1);
        let scenario = &mut project.scenarios[0];
        scenario.graph = serde_json::from_value(serde_json::json!({
            "nodes": [
                {"id": "start", "kind": "start", "handle": "begin", "position": {"x": 0, "y": 0}, "inputs": {}, "config": {}},
                {"id": "active", "kind": "request", "handle": "active", "position": {"x": 0, "y": 0}, "inputs": {"url": {"kind": "template", "parts": [{"kind": "text", "text": url}, {"kind": "value", "binding": {"kind": "reference", "nodeId": "shared", "port": "value", "path": ""}}]}}, "config": {}},
                {"id": "shared", "kind": "value", "handle": "shared", "position": {"x": 0, "y": 0}, "inputs": {"value": {"kind": "literal", "value": 1}}, "config": {}},
                {"id": "disconnected", "kind": "request", "handle": "disconnected", "position": {"x": 0, "y": 0}, "inputs": {"url": {"kind": "template", "parts": [{"kind": "text", "text": url}, {"kind": "value", "binding": {"kind": "reference", "nodeId": "shared", "port": "value", "path": ""}}]}}, "config": {}}
            ],
            "edges": [{"id": "start-active", "source": "start", "target": "active"}]
        })).unwrap();
        scenario.cleanup = Graph {
            nodes: vec![],
            edges: vec![],
            proxies: Vec::new(),
        };
        let scenario_id = scenario.id.clone();
        let result = run_scenario(
            RunRequest::new(project, scenario_id, apiforge_core::run::RunMode::Live),
            RunOptions::new(http::HttpTransport::new().unwrap()),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
        server.abort();
        assert_eq!(result.status, RunStatus::Passed);
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            result
                .steps
                .iter()
                .find(|step| step.node_id == "disconnected")
                .unwrap()
                .status,
            StepStatus::Skipped
        );
    }

    #[tokio::test]
    async fn cleanup_time_does_not_consume_the_main_phase_budget() {
        let mut project: Project =
            serde_json::from_slice(include_bytes!("../../../fixtures/projects/shop.json")).unwrap();
        project.scenarios.truncate(1);
        let scenario = &mut project.scenarios[0];
        scenario.graph = Graph {
            nodes: vec![],
            edges: vec![],
            proxies: Vec::new(),
        };
        scenario.cleanup = Graph {
            nodes: vec![
                serde_json::from_value(serde_json::json!({
                    "id": "cleanup-start", "kind": "start", "handle": "cleanup_start",
                    "position": {"x": 0, "y": 0}, "inputs": {}, "config": {}
                }))
                .unwrap(),
            ],
            edges: vec![],
            proxies: Vec::new(),
        };
        let scenario_id = scenario.id.clone();
        let request = RunRequest::new(
            project,
            scenario_id,
            apiforge_core::run::RunMode::Simulation,
        );
        let mut options = RunOptions::new(http::HttpTransport::new().unwrap());
        options.limits.main_budget = Duration::from_millis(200);
        options.limits.cleanup_budget = Duration::from_secs(2);
        let mut waited = false;
        let result = run_scenario(request, options, CancellationToken::new(), |report| {
            if !waited && report.steps.iter().any(|step| step.phase == Phase::Cleanup) {
                // Deterministically cross the main deadline while publishing cleanup,
                // without depending on a live server or scheduler timing.
                std::thread::sleep(Duration::from_millis(250));
                waited = true;
            }
        })
        .await
        .unwrap();
        assert!(waited);
        assert_eq!(result.status, RunStatus::Passed);
        assert_eq!(result.error, None);
    }

    #[test]
    fn search_params_are_set_like_url_search_params() {
        let mut url = Url::parse("http://h/p?a=1&b=2&a=3&c=x%20y").unwrap();
        set_search_param(&mut url, "a", "new value");
        assert_eq!(url.as_str(), "http://h/p?a=new+value&b=2&c=x+y");
        set_search_param(&mut url, "d", "ż&=");
        assert_eq!(
            url.as_str(),
            "http://h/p?a=new+value&b=2&c=x+y&d=%C5%BC%26%3D"
        );
        let mut url = Url::parse("http://h/p").unwrap();
        set_search_param(&mut url, "", "");
        assert_eq!(url.as_str(), "http://h/p?=");
    }

    #[test]
    fn absolute_addresses_are_detected_case_insensitively() {
        assert!(has_http_scheme("HTTPS://example.com"));
        assert!(has_http_scheme("http://x"));
        assert!(!has_http_scheme("/http://x"));
        assert!(!has_http_scheme("ftp://x"));
        assert!(!has_http_scheme("ht"));
    }

    #[test]
    fn operator_ids_use_document_names() {
        assert_eq!(operator_id(Operator::Power10), "power10");
        assert_eq!(operator_id(Operator::TypeOf), "typeOf");
    }
}
