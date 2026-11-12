//! Collapsed checks (LAY-11): which check cards the board folds into their
//! subject's footer, and what that footer then says.
//!
//! A check with a subject (LAY-08) is folded under it, unless another step
//! reads its value: a gate stays a visible node with its wire (the rule of
//! 4.7 item 4), and so does any check whose result is read later. A check
//! with no subject stays a card. A check with several subjects is counted
//! under each and hangs under the one the engine reaches last.
//!
//! Everything here reads the document alone, never a run, so the same checks
//! fold before and after a run; the aggregate adds the run's results.

use apiforge_core::{
    format::{Graph, Node},
    graph::{check_subjects, check_subjects_in, sequence},
    model::NodeKind,
    run::{StepResult, StepStatus},
};
use std::collections::HashMap;

/// The space between a subject and the checks hung under it, and between
/// those checks: twice the router's 12 px clearance, so a wire still passes.
pub const HANG_GAP: f32 = 24.0;

/// A check the board can fold into its subjects' footers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collapsible {
    /// The node id of the check.
    pub check: String,
    /// The steps it verifies, in the order it reads them.
    pub subjects: Vec<String>,
    /// The subject it hangs under: the one the engine reaches last.
    pub anchor: String,
}

fn is_check(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Assert | NodeKind::Condition)
}

/// The checks of `graph` that fold into a subject, in the engine's order.
/// `later` is a graph whose steps may read `graph`'s, as Cleanup reads Main:
/// a check a step there reads keeps its card too.
pub fn collapsible(graph: &Graph, later: Option<&Graph>) -> Vec<Collapsible> {
    let order = sequence(graph);
    let position: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    order
        .iter()
        .filter_map(|id| graph.nodes.iter().find(|node| node.id == *id))
        .filter(|node| is_check(node))
        .filter_map(|check| {
            let subjects = check_subjects(graph, check).subjects;
            if subjects.is_empty() || subjects.iter().any(|subject| subject.gate) {
                return None;
            }
            // A check whose result another step reads keeps its card and wire.
            let read = graph
                .nodes
                .iter()
                .chain(later.iter().flat_map(|later| &later.nodes))
                .any(|node| {
                    node.inputs
                        .values()
                        .any(|binding| binding.references_node(&check.id))
                });
            if read {
                return None;
            }
            let anchor = subjects
                .iter()
                .map(|subject| subject.step)
                .max_by_key(|step| position.get(step))?
                .to_owned();
            Some(Collapsible {
                check: check.id.clone(),
                subjects: subjects
                    .iter()
                    .map(|subject| subject.step.to_owned())
                    .collect(),
                anchor,
            })
        })
        .collect()
}

/// What became of the checks of one subject, for its footer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Aggregate {
    /// Checks counted under the subject, gates apart.
    pub checks: usize,
    pub passed: usize,
    pub failed: usize,
    /// Checks the run reached without running them: skipped, blocked or
    /// cancelled.
    pub skipped: usize,
    /// Checks with no result yet.
    pub waiting: usize,
    /// *Data conditions* another step reads, counted apart (LAY-08).
    pub gates: usize,
    /// Whether a run's results are known at all.
    pub run: bool,
}

impl Aggregate {
    /// A check of the subject failed: its footer and border turn red (Q15).
    pub fn failed(&self) -> bool {
        self.failed > 0
    }

    /// The checks line: `3 checks` before a run, never a tick for a check
    /// that did not run; `✓ 2 checks` when every one passed; `✕ 1/3 failed
    /// · 1 skipped` when one failed; else the state of each (`1 ✓ · 1
    /// waiting`). Gates follow, counted apart.
    pub fn text(&self) -> String {
        let mut parts = Vec::new();
        if self.checks > 0 {
            if !self.run {
                parts.push(plural(self.checks, "check"));
            } else if self.failed > 0 {
                parts.push(format!("✕ {}/{} failed", self.failed, self.checks));
                if self.skipped > 0 {
                    parts.push(format!("{} skipped", self.skipped));
                }
                if self.waiting > 0 {
                    parts.push(format!("{} waiting", self.waiting));
                }
            } else if self.skipped == 0 && self.waiting == 0 {
                parts.push(format!("✓ {}", plural(self.checks, "check")));
            } else {
                if self.passed > 0 {
                    parts.push(format!("{} ✓", self.passed));
                }
                if self.skipped > 0 {
                    parts.push(format!("{} skipped", self.skipped));
                }
                if self.waiting > 0 {
                    parts.push(format!("{} waiting", self.waiting));
                }
            }
        }
        if self.gates > 0 {
            parts.push(plural(self.gates, "gate"));
        }
        parts.join(" · ")
    }
}

fn plural(count: usize, one: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {one}s")
    }
}

/// The aggregate of every subject on the board, by the subject's node id. A
/// check with several subjects is counted under each (Q6 = A). `later` is as
/// in [`collapsible`]: a *Data condition* a step there reads is a gate.
pub fn aggregates(
    graph: &Graph,
    later: Option<&Graph>,
    steps: Option<&[StepResult]>,
) -> HashMap<String, Aggregate> {
    let mut graphs = vec![graph];
    graphs.extend(later);
    let mut result: HashMap<String, Aggregate> = HashMap::new();
    for check in graph.nodes.iter().filter(|node| is_check(node)) {
        let status = steps.and_then(|steps| {
            steps
                .iter()
                .find(|step| step.node_id == check.id)
                .map(|step| step.status)
        });
        for subject in check_subjects_in(&graphs, check).subjects {
            let aggregate = result.entry(subject.step.to_owned()).or_default();
            aggregate.run = steps.is_some();
            if subject.gate {
                aggregate.gates += 1;
                continue;
            }
            aggregate.checks += 1;
            match status {
                Some(StepStatus::Passed) => aggregate.passed += 1,
                Some(StepStatus::Failed | StepStatus::Error) => aggregate.failed += 1,
                Some(StepStatus::Skipped | StepStatus::Blocked | StepStatus::Cancelled) => {
                    aggregate.skipped += 1;
                }
                None => aggregate.waiting += 1,
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{
        format::{Binding, Position},
        graph::create_node,
    };

    fn node(kind: NodeKind, id: &str) -> Node {
        let mut node = create_node(kind, Position { x: 0.0, y: 0.0 }, None);
        node.id = id.into();
        node.handle = id.into();
        node
    }

    /// A Data condition that only a Cleanup step reads stays on the Main
    /// board, its wire with it; one nothing reads folds.
    #[test]
    fn a_check_a_later_graph_reads_keeps_its_card() {
        let mut condition = node(NodeKind::Condition, "valid");
        condition
            .inputs
            .insert("actual1".into(), Binding::reference("cart", "status", ""));
        let mut assert = node(NodeKind::Assert, "ok");
        assert
            .inputs
            .insert("actual".into(), Binding::reference("cart", "status", ""));
        let main = Graph {
            nodes: vec![node(NodeKind::Request, "cart"), condition, assert],
            edges: Vec::new(),
            proxies: Vec::new(),
        };
        let mut remove = node(NodeKind::Request, "remove");
        remove
            .inputs
            .insert("url".into(), Binding::reference("valid", "value", "id"));
        let cleanup = Graph {
            nodes: vec![remove],
            edges: Vec::new(),
            proxies: Vec::new(),
        };
        let folded = |later| -> Vec<String> {
            collapsible(&main, later)
                .into_iter()
                .map(|candidate| candidate.check)
                .collect()
        };
        assert_eq!(folded(None), vec!["valid", "ok"]);
        assert_eq!(folded(Some(&cleanup)), vec!["ok"]);
    }

    fn aggregate(checks: usize, passed: usize, failed: usize, skipped: usize) -> Aggregate {
        Aggregate {
            checks,
            passed,
            failed,
            skipped,
            waiting: checks - passed - failed - skipped,
            gates: 0,
            run: true,
        }
    }

    #[test]
    fn the_checks_line_never_ticks_a_check_that_did_not_run() {
        let before = Aggregate {
            checks: 3,
            waiting: 3,
            ..Aggregate::default()
        };
        assert_eq!(before.text(), "3 checks");
        assert_eq!(aggregate(2, 2, 0, 0).text(), "✓ 2 checks");
        assert_eq!(aggregate(1, 1, 0, 0).text(), "✓ 1 check");
        assert_eq!(aggregate(3, 1, 1, 1).text(), "✕ 1/3 failed · 1 skipped");
        assert_eq!(aggregate(2, 1, 0, 0).text(), "1 ✓ · 1 waiting");
        assert_eq!(aggregate(2, 0, 0, 0).text(), "2 waiting");
        assert_eq!(aggregate(2, 1, 0, 1).text(), "1 ✓ · 1 skipped");
        let gate = Aggregate {
            checks: 1,
            waiting: 1,
            gates: 1,
            ..Aggregate::default()
        };
        assert_eq!(gate.text(), "1 check · 1 gate");
    }
}
