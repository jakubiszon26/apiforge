//! LAY-19 design review (mock-up, not for `main`): what a failure cost. A
//! step that did not run is named by the end of its cause chain, and the
//! step at that end counts the steps it kept from running.
use apiforge_core::run::{NotRunCause, StepResult, StepStatus};
use std::{cell::RefCell, collections::HashMap};

/// The variant the review renders: `None` is `main`.
pub fn variant() -> Option<char> {
    std::env::var("LAY19")
        .ok()
        .and_then(|value| value.chars().next())
        .filter(|variant| matches!(variant, 'a' | 'b'))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Class {
    Stopped,
    Blocked,
    Unmet,
    Skipped,
    Cancelled,
}

impl Class {
    pub fn word(&self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Blocked | Self::Unmet => "Blocked",
            Self::Skipped => "Skipped",
            Self::Cancelled => "Cancelled",
        }
    }

    pub fn summary(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Blocked | Self::Unmet => "blocked",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NotRun {
    pub class: Class,
    /// The step at the end of the chain: the one that failed, or the false
    /// *Data condition*.
    pub root: Option<String>,
    pub root_name: Option<String>,
}

impl NotRun {
    /// The words on the card and in Run results.
    pub fn text(&self, variant: char) -> String {
        let name = self.root_name.as_deref().unwrap_or("a step");
        match (&self.class, variant) {
            (Class::Skipped, _) => "Skipped · outside path".to_owned(),
            (Class::Cancelled, _) => "Cancelled".to_owned(),
            (Class::Unmet, 'a') => format!("Blocked · {name} unmet"),
            (Class::Unmet, _) => "Blocked · condition unmet".to_owned(),
            (class, 'a') => format!("{} · {name} failed", class.word()),
            (class, _) => class.word().to_owned(),
        }
    }

    /// The sentence in the step's details.
    pub fn sentence(&self) -> String {
        let name = self.root_name.as_deref().unwrap_or("a step");
        match self.class {
            Class::Stopped => {
                format!("Did not run: {name} failed and the error policy Stop ended the run.")
            }
            Class::Blocked => format!("Did not run: it needs {name}, which failed."),
            Class::Unmet => format!("Did not run: the condition {name} is not met."),
            Class::Skipped => "Did not run: it is on the branch that was not taken.".to_owned(),
            Class::Cancelled => "Did not run: the run was cancelled.".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Consequences {
    pub not_run: HashMap<String, NotRun>,
    /// By the step at the end of a chain, the steps it kept from running.
    pub cost: HashMap<String, usize>,
}

impl Consequences {
    pub fn of(steps: &[StepResult]) -> Self {
        let mut result = Self::default();
        for step in steps {
            let Some(mut cause) = step.cause.as_ref() else {
                continue;
            };
            let mut found = None;
            for _ in 0..=steps.len() {
                let Some(id) = cause.step() else {
                    found = Some(NotRun {
                        class: if *cause == NotRunCause::Cancelled {
                            Class::Cancelled
                        } else {
                            Class::Skipped
                        },
                        root: None,
                        root_name: None,
                    });
                    break;
                };
                let Some(target) = steps.iter().find(|step| step.node_id == id) else {
                    break;
                };
                if let Some(next) = target.cause.as_ref() {
                    cause = next;
                    continue;
                }
                found = Some(NotRun {
                    class: match cause {
                        NotRunCause::Stopped { .. } => Class::Stopped,
                        NotRunCause::ConditionUnmet { .. } => Class::Unmet,
                        _ => Class::Blocked,
                    },
                    root: Some(target.node_id.clone()),
                    root_name: Some(target.display_name().to_owned()),
                });
                break;
            }
            if let Some(found) = found {
                if let Some(root) = &found.root {
                    *result.cost.entry(root.clone()).or_default() += 1;
                }
                result.not_run.insert(step.node_id.clone(), found);
            }
        }
        result
    }

    /// `11 steps not run`.
    pub fn cost_text(count: usize) -> String {
        if count == 1 {
            "1 step not run".to_owned()
        } else {
            format!("{count} steps not run")
        }
    }

    /// The summary's parts after *passed* and *failed*, in a fixed order.
    pub fn summary(&self, steps: &[StepResult]) -> Vec<(String, Class)> {
        [
            Class::Stopped,
            Class::Blocked,
            Class::Skipped,
            Class::Cancelled,
        ]
        .into_iter()
        .filter_map(|class| {
            let count = steps
                .iter()
                .filter(|step| {
                    self.not_run
                        .get(&step.node_id)
                        .is_some_and(|found| found.class.summary() == class.summary())
                })
                .count();
            (count > 0).then(|| (format!("{count} {}", class.summary()), class))
        })
        .collect()
    }
}

/// A *Data condition* that ran and is false has no cause of its own.
pub fn unmet(step: &StepResult) -> bool {
    step.status == StepStatus::Blocked && step.cause.is_none()
}

thread_local! {
    /// The run the Run results panel is drawing.
    pub static SHOWN: RefCell<Option<Consequences>> = const { RefCell::new(None) };
}
