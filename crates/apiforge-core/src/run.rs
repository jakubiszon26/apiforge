//! Run reports shared by the runner and the interface.
//!
//! Field names are part of the exported report JSON and stay stable, so reports
//! from different releases remain comparable.

use crate::{
    format::{Graph, Node, OptionalJson},
    model::NodeKind,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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

/// Why a step did not run. Each variant answers one branch of the runner that
/// builds a result for a step it does not execute. The step a variant names is
/// a node id of the same graph, except the producer of a Cleanup input, which
/// is a step of the main graph.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NotRunCause {
    /// The run stopped, under the policy *Stop*, after `by` failed. Every step
    /// of the graph the run did not reach after it carries this cause, except
    /// a step on a branch that was not taken and a step that reads an output
    /// it cannot use, which name that branch or that step (RUN-04).
    Stopped { by: String },
    /// `step`, which this step waits for, failed or did not run itself.
    BlockedBy { step: String },
    /// `condition`, a Data condition this step reads, is false.
    ConditionUnmet { condition: String },
    /// The step is on a branch that was not taken.
    OutsidePath,
    /// The run was cancelled or ran out of time before the step.
    Cancelled,
    /// A Cleanup step reads an output of `step` that does not exist.
    CleanupInputMissing { step: String },
}

impl NotRunCause {
    /// The step this cause points at; it has a cause of its own when it did not
    /// run either.
    pub fn step(&self) -> Option<&str> {
        match self {
            Self::Stopped { by: step }
            | Self::BlockedBy { step }
            | Self::ConditionUnmet { condition: step }
            | Self::CleanupInputMissing { step } => Some(step),
            Self::OutsidePath | Self::Cancelled => None,
        }
    }
}

/// The step that ran at the end of a cause chain: the one that failed, or
/// a false *Data condition*.
#[derive(Clone, Debug, PartialEq)]
pub struct EndStep {
    pub node_id: String,
    pub handle: String,
    /// The label, or the handle of a node without one.
    pub name: String,
    pub kind: NodeKind,
    pub status: StepStatus,
}

/// Where the cause of a step that did not run ends (LAY-19): the last link
/// of its chain and the step that ran at its end. A chain that ends without
/// a step (outside the path, cancelled) has none, and so has one that names
/// a step missing from the results or that loops.
#[derive(Clone, Debug, PartialEq)]
pub struct CauseEnd {
    pub last: NotRunCause,
    pub step: Option<EndStep>,
}

/// The cause ends of a run's steps, resolved once (LAY-19): each link names
/// a step of the same graph, which has a cause of its own when it did not
/// run either. A component call's steps are resolved among themselves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CauseEnds {
    /// By the node id of a step that did not run.
    ends: HashMap<String, CauseEnd>,
    /// By the node id of the step at an end, how many steps it kept from
    /// running.
    kept: HashMap<String, usize>,
    /// By the node id of a component call, its steps' ends.
    children: HashMap<String, CauseEnds>,
}

impl CauseEnds {
    /// Resolves every chain of `steps` in one pass: each step is visited
    /// once and a chain's end is shared by every step on it.
    pub fn of(steps: &[StepResult]) -> Self {
        #[derive(Clone, Copy)]
        enum Memo {
            Todo,
            /// On the chain being followed: reaching it again is a loop.
            Busy,
            Ran,
            /// The step whose cause is the last link, and the step at the
            /// end.
            Ends(usize, Option<usize>),
        }
        let mut index: HashMap<&str, usize> = HashMap::with_capacity(steps.len());
        for (at, step) in steps.iter().enumerate() {
            index.entry(step.node_id.as_str()).or_insert(at);
        }
        let mut memo = vec![Memo::Todo; steps.len()];
        let mut path = Vec::new();
        for start in 0..steps.len() {
            if !matches!(memo[start], Memo::Todo) {
                continue;
            }
            path.clear();
            let mut at = start;
            let found = loop {
                match memo[at] {
                    Memo::Ends(last, end) => break Memo::Ends(last, end),
                    Memo::Busy => break Memo::Ends(*path.last().unwrap_or(&at), None),
                    Memo::Ran => break Memo::Ran,
                    Memo::Todo => {}
                }
                let Some(cause) = &steps[at].cause else {
                    break Memo::Ran;
                };
                memo[at] = Memo::Busy;
                path.push(at);
                let Some(&next) = cause.step().and_then(|id| index.get(id)) else {
                    break Memo::Ends(at, None);
                };
                if steps[next].cause.is_none() {
                    break Memo::Ends(at, Some(next));
                }
                at = next;
            };
            if path.is_empty() {
                memo[start] = found;
            }
            for &on in &path {
                memo[on] = found;
            }
        }

        let mut ends = Self::default();
        for (step, memo) in steps.iter().zip(&memo) {
            if let Memo::Ends(last, end) = *memo
                && let Some(last) = &steps[last].cause
            {
                let end = end.map(|end| &steps[end]);
                if let Some(end) = end {
                    *ends.kept.entry(end.node_id.clone()).or_default() += 1;
                }
                ends.ends.entry(step.node_id.clone()).or_insert(CauseEnd {
                    last: last.clone(),
                    step: end.map(|end| EndStep {
                        node_id: end.node_id.clone(),
                        handle: end.handle.clone(),
                        name: end.display_name().to_owned(),
                        kind: end.kind,
                        status: end.status,
                    }),
                });
            }
            if let Some(children) = &step.children {
                ends.children
                    .entry(step.node_id.clone())
                    .or_insert_with(|| Self::of(children));
            }
        }
        ends
    }

    /// Where the cause of the step `node_id` ends; `None` for a step that
    /// ran.
    pub fn end(&self, node_id: &str) -> Option<&CauseEnd> {
        self.ends.get(node_id)
    }

    /// Every end, one per step that did not run.
    pub fn ends(&self) -> impl Iterator<Item = &CauseEnd> {
        self.ends.values()
    }

    /// How many steps the step `node_id` kept from running.
    pub fn kept(&self, node_id: &str) -> usize {
        self.kept.get(node_id).copied().unwrap_or(0)
    }

    /// The ends inside the component call `node_id`.
    pub fn within(&self, node_id: &str) -> Option<&CauseEnds> {
        self.children.get(node_id)
    }
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
    /// Why the step did not run; absent for a step that ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<NotRunCause>,
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
            cause: None,
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
    /// The node id of the top-level step executing now, for an observer of a
    /// run in progress. Never written to a report; a finished run has none.
    #[serde(skip)]
    pub running: Option<String>,
}

impl RunResult {
    pub fn step(&self, node_id: &str) -> Option<&StepResult> {
        self.steps.iter().find(|step| step.node_id == node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(handle: &str, status: StepStatus, cause: Option<NotRunCause>) -> StepResult {
        StepResult {
            node_id: format!("id-{handle}"),
            handle: handle.to_owned(),
            label: None,
            kind: NodeKind::Request,
            status,
            phase: Phase::Main,
            inputs: IndexMap::new(),
            outputs: IndexMap::new(),
            duration_ms: 0,
            message: None,
            expected: OptionalJson::Missing,
            actual: OptionalJson::Missing,
            request: None,
            children: None,
            cause,
        }
    }

    fn blocked_by(handle: &str) -> Option<NotRunCause> {
        Some(NotRunCause::BlockedBy {
            step: format!("id-{handle}"),
        })
    }

    /// A chain of hundreds of steps, each blocked by the one before and the
    /// first stopped by the failure, resolves to the failure, in either
    /// order of the results.
    #[test]
    fn a_long_chain_resolves_to_its_end() {
        const LENGTH: usize = 600;
        let mut steps = vec![
            step("failed", StepStatus::Failed, None),
            step(
                "s0",
                StepStatus::Skipped,
                Some(NotRunCause::Stopped {
                    by: "id-failed".into(),
                }),
            ),
        ];
        for at in 1..LENGTH {
            steps.push(step(
                &format!("s{at}"),
                StepStatus::Skipped,
                blocked_by(&format!("s{}", at - 1)),
            ));
        }
        for order in [false, true] {
            if order {
                steps.reverse();
            }
            let ends = CauseEnds::of(&steps);
            assert_eq!(ends.kept("id-failed"), LENGTH);
            assert_eq!(ends.end("id-failed"), None);
            for at in 0..LENGTH {
                let end = ends
                    .end(&format!("id-s{at}"))
                    .expect("a step that did not run");
                assert!(matches!(end.last, NotRunCause::Stopped { .. }), "s{at}");
                assert_eq!(
                    end.step.as_ref().map(|end| end.handle.as_str()),
                    Some("failed")
                );
            }
        }
    }

    /// A loop, a missing step and a cause without a step end without a
    /// step; a component call's steps resolve among themselves.
    #[test]
    fn a_chain_without_an_end_step() {
        let mut call = step("call", StepStatus::Failed, None);
        call.children = Some(vec![
            step("inner", StepStatus::Error, None),
            step("reader", StepStatus::Blocked, blocked_by("inner")),
        ]);
        let steps = vec![
            step("a", StepStatus::Skipped, blocked_by("b")),
            step("b", StepStatus::Skipped, blocked_by("a")),
            step("lost", StepStatus::Skipped, blocked_by("nowhere")),
            step("other", StepStatus::Skipped, Some(NotRunCause::OutsidePath)),
            step("behind", StepStatus::Skipped, blocked_by("other")),
            call,
        ];
        let ends = CauseEnds::of(&steps);
        for handle in ["a", "b", "lost", "behind"] {
            let end = ends.end(&format!("id-{handle}")).expect(handle);
            assert_eq!(end.step, None, "{handle}");
        }
        assert_eq!(
            ends.end("id-behind").map(|end| &end.last),
            Some(&NotRunCause::OutsidePath)
        );
        let inner = ends.within("id-call").expect("the call's steps");
        assert_eq!(inner.kept("id-inner"), 1);
        assert_eq!(ends.kept("id-inner"), 0);
    }
}
