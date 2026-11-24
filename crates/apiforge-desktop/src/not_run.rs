//! What a step that did not run says, and what a failure cost (LAY-19).
//!
//! The words come from the step's cause, followed to its end, and never
//! from the engine's message: a step after a stop reads `Stopped ·
//! checkout_total failed` although the engine names the step before it.
//! The card that holds the failure counts the steps it kept from running,
//! as many as the rows of Run results whose cause ends there. The ends are
//! resolved once per run (`CauseEnds`); here they are only read.

use apiforge_core::{
    model::NodeKind,
    run::{CauseEnd, CauseEnds, EndStep, NotRunCause, StepResult, StepStatus},
};
use std::collections::HashMap;

/// A *Data condition* that ran and is false: the steps that read it did not
/// run, and it reads *Not met* (4 A).
pub fn unmet(step: &StepResult) -> bool {
    step.kind == NodeKind::Condition && step.status == StepStatus::Blocked && step.cause.is_none()
}

/// The end of a chain is a false *Data condition*.
fn unmet_end(end: &EndStep) -> bool {
    end.kind == NodeKind::Condition && end.status == StepStatus::Blocked
}

/// `11 steps not run`.
pub fn count_text(count: usize) -> String {
    if count == 1 {
        "1 step not run".to_owned()
    } else {
        format!("{count} steps not run")
    }
}

/// `11 not run`, where the longer form has no room.
pub fn short_count_text(count: usize) -> String {
    format!("{count} not run")
}

/// The steps a step kept from running, on the card that holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cost {
    pub count: usize,
    /// A false *Data condition* (amber), not a failure (red).
    pub unmet: bool,
}

/// By card, the steps kept from running by the step at the end of their
/// cause; a folded check's on the card it is folded into, which `holder`
/// names.
pub fn costs<'a>(
    ends: &'a CauseEnds,
    holder: impl Fn(&str) -> Option<&'a str>,
) -> HashMap<&'a str, Cost> {
    let mut costs: HashMap<&str, Cost> = HashMap::new();
    for end in ends.ends().filter_map(|end| end.step.as_ref()) {
        let card = holder(&end.node_id).unwrap_or(&end.node_id);
        let cost = costs.entry(card).or_insert(Cost {
            count: 0,
            unmet: true,
        });
        cost.count += 1;
        cost.unmet &= unmet_end(end);
    }
    costs
}

/// A group of Run results' summary; every step is in one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Passed,
    Failed,
    NotMet,
    Stopped,
    Blocked,
    Skipped,
    Cancelled,
}

impl Group {
    pub const ALL: [Self; 7] = [
        Self::Passed,
        Self::Failed,
        Self::NotMet,
        Self::Stopped,
        Self::Blocked,
        Self::Skipped,
        Self::Cancelled,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::NotMet => "not met",
            Self::Stopped => "stopped",
            Self::Blocked => "blocked",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }

    /// The group of `step`, with the ends of its graph.
    pub fn of(step: &StepResult, ends: &CauseEnds) -> Self {
        if let Some(not_run) = NotRun::of(step, ends) {
            return not_run.group();
        }
        match step.status {
            StepStatus::Passed => Self::Passed,
            StepStatus::Failed | StepStatus::Error => Self::Failed,
            StepStatus::Blocked if unmet(step) => Self::NotMet,
            StepStatus::Blocked => Self::Blocked,
            StepStatus::Skipped => Self::Skipped,
            StepStatus::Cancelled => Self::Cancelled,
        }
    }
}

/// Run results' summary: how many steps each group holds, in a fixed
/// order, empty groups left out.
pub fn summary(steps: &[StepResult], ends: &CauseEnds) -> Vec<(Group, usize)> {
    let mut counts = [0; Group::ALL.len()];
    for step in steps {
        let group = Group::of(step, ends);
        if let Some(at) = Group::ALL.iter().position(|each| *each == group) {
            counts[at] += 1;
        }
    }
    Group::ALL
        .into_iter()
        .zip(counts)
        .filter(|(_, count)| *count > 0)
        .collect()
}

/// Why a step did not run: the last link of its cause and the step at its
/// end.
#[derive(Clone, Copy, Debug)]
pub struct NotRun<'a>(&'a CauseEnd);

impl<'a> NotRun<'a> {
    /// `None` for a step that ran.
    pub fn of(step: &StepResult, ends: &'a CauseEnds) -> Option<Self> {
        ends.end(&step.node_id).map(Self)
    }

    /// The first word: the kind of the last link.
    pub fn word(&self) -> &'static str {
        match self.0.last {
            NotRunCause::Stopped { .. } => "Stopped",
            NotRunCause::BlockedBy { .. } | NotRunCause::ConditionUnmet { .. } => "Blocked",
            NotRunCause::OutsidePath | NotRunCause::CleanupInputMissing { .. } => "Skipped",
            NotRunCause::Cancelled => "Cancelled",
        }
    }

    fn group(&self) -> Group {
        match self.0.last {
            NotRunCause::Stopped { .. } => Group::Stopped,
            NotRunCause::BlockedBy { .. } | NotRunCause::ConditionUnmet { .. } => Group::Blocked,
            NotRunCause::OutsidePath | NotRunCause::CleanupInputMissing { .. } => Group::Skipped,
            NotRunCause::Cancelled => Group::Cancelled,
        }
    }

    /// `checkout_total failed`, `valid_cart unmet`, the step named by `name`.
    fn end(&self, name: impl Fn(&EndStep) -> String) -> Option<String> {
        self.0.step.as_ref().map(|end| {
            format!(
                "{} {}",
                name(end),
                if unmet_end(end) { "unmet" } else { "failed" }
            )
        })
    }

    /// The words of the card and of the row: `Stopped · checkout_total
    /// failed`, `Skipped · outside path`, `Cancelled`.
    pub fn words(&self) -> String {
        self.words_naming(|end| end.name.clone())
    }

    /// The same words, the step at the end of the cause named by `name`: a
    /// list that shows no handles names a check as it writes it.
    pub fn words_naming(&self, name: impl Fn(&EndStep) -> String) -> String {
        let word = self.word();
        let rest = match self.0.last {
            NotRunCause::OutsidePath => Some("outside path".to_owned()),
            NotRunCause::Cancelled => None,
            NotRunCause::CleanupInputMissing { .. } => self
                .0
                .step
                .as_ref()
                .map(|end| format!("no value from {}", name(end))),
            _ => self.end(&name),
        };
        match rest {
            Some(rest) => format!("{word} · {rest}"),
            None => word.to_owned(),
        }
    }

    /// The sentence of the step's details.
    pub fn sentence(&self) -> String {
        self.sentence_naming(|end| end.name.clone())
    }

    /// The same sentence, the step named by `name`.
    pub fn sentence_naming(&self, name: impl Fn(&EndStep) -> String) -> String {
        let end = self.0.step.as_ref();
        let name = end.map(name);
        let name = name.as_deref();
        let condition = end.is_some_and(unmet_end);
        match (&self.0.last, name) {
            (NotRunCause::Stopped { .. }, Some(name)) if condition => format!(
                "Did not run: the condition {name} is not met and the error policy Stop ended the run."
            ),
            (NotRunCause::Stopped { .. }, Some(name)) => {
                format!("Did not run: {name} failed and the error policy Stop ended the run.")
            }
            (NotRunCause::Stopped { .. }, None) => {
                "Did not run: the error policy Stop ended the run.".to_owned()
            }
            (NotRunCause::BlockedBy { .. } | NotRunCause::ConditionUnmet { .. }, Some(name))
                if condition =>
            {
                format!("Did not run: the condition {name} is not met.")
            }
            (NotRunCause::BlockedBy { .. } | NotRunCause::ConditionUnmet { .. }, Some(name)) => {
                format!("Did not run: it needs {name}, which failed.")
            }
            (NotRunCause::BlockedBy { .. } | NotRunCause::ConditionUnmet { .. }, None) => {
                "Did not run: a step it needs did not run.".to_owned()
            }
            (NotRunCause::OutsidePath, _) => {
                "Did not run: it is on the branch that was not taken.".to_owned()
            }
            (NotRunCause::Cancelled, _) => {
                "Did not run: the run was cancelled before it reached this step.".to_owned()
            }
            (NotRunCause::CleanupInputMissing { .. }, Some(name)) => {
                format!("Did not run: it reads a value of {name}, which was never produced.")
            }
            (NotRunCause::CleanupInputMissing { .. }, None) => {
                "Did not run: it reads a value that was never produced.".to_owned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::run::Phase;
    use indexmap::IndexMap;

    fn step(
        handle: &str,
        kind: NodeKind,
        status: StepStatus,
        cause: Option<NotRunCause>,
    ) -> StepResult {
        StepResult {
            node_id: format!("id-{handle}"),
            handle: handle.to_owned(),
            label: None,
            kind,
            status,
            phase: Phase::Main,
            inputs: IndexMap::new(),
            outputs: IndexMap::new(),
            duration_ms: 0,
            message: None,
            expected: Default::default(),
            actual: Default::default(),
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

    #[test]
    fn a_chain_reads_its_last_link_and_its_end() {
        let steps = vec![
            step("total", NodeKind::Assert, StepStatus::Failed, None),
            step(
                "order",
                NodeKind::Request,
                StepStatus::Skipped,
                Some(NotRunCause::Stopped {
                    by: "id-total".into(),
                }),
            ),
            step(
                "paid",
                NodeKind::If,
                StepStatus::Skipped,
                blocked_by("order"),
            ),
            step(
                "pay",
                NodeKind::Request,
                StepStatus::Skipped,
                blocked_by("paid"),
            ),
            step("valid", NodeKind::Condition, StepStatus::Blocked, None),
            step(
                "guarded",
                NodeKind::Request,
                StepStatus::Blocked,
                Some(NotRunCause::ConditionUnmet {
                    condition: "id-valid".into(),
                }),
            ),
            step(
                "behind",
                NodeKind::Request,
                StepStatus::Blocked,
                blocked_by("guarded"),
            ),
            step(
                "else",
                NodeKind::Assert,
                StepStatus::Skipped,
                Some(NotRunCause::OutsidePath),
            ),
        ];
        let ends = CauseEnds::of(&steps);
        let words = |handle: &str| {
            let step = steps.iter().find(|step| step.handle == handle).unwrap();
            NotRun::of(step, &ends).map(|not_run| not_run.words())
        };
        assert_eq!(words("total"), None);
        assert_eq!(words("pay").as_deref(), Some("Stopped · total failed"));
        assert_eq!(words("behind").as_deref(), Some("Blocked · valid unmet"));
        assert_eq!(words("else").as_deref(), Some("Skipped · outside path"));
        assert_eq!(ends.kept("id-total"), 3);
        assert_eq!(ends.kept("id-valid"), 2);
        let costs = costs(&ends, |_| None);
        assert_eq!(
            costs.get("id-valid"),
            Some(&Cost {
                count: 2,
                unmet: true
            })
        );
        assert_eq!(
            summary(&steps, &ends),
            [
                (Group::Failed, 1),
                (Group::NotMet, 1),
                (Group::Stopped, 3),
                (Group::Blocked, 2),
                (Group::Skipped, 1)
            ]
        );
    }
}
