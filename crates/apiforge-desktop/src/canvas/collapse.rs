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
use std::collections::{HashMap, HashSet};

/// The space between a subject and the checks hung under it, and between
/// those checks: Tidy up's stack gap, so *Expand checks here* hangs them as
/// Tidy up does and they are one group with their subject (LAY-31).
pub const HANG_GAP: f32 = apiforge_layout::STACK_GAP;

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
    /// The places in the engine's order of the checks counted here that the
    /// board does not draw as cards (LAY-27): the numbers their badges would
    /// carry.
    pub numbers: Vec<usize>,
    /// The predicate of the subject's one check, as its line writes it
    /// (LAY-30): set while that check is folded and nothing else is counted.
    pub single: Option<String>,
}

impl Aggregate {
    /// A check of the subject failed: its footer and border turn red (Q15).
    pub fn failed(&self) -> bool {
        self.failed > 0
    }

    /// The numbers of the folded checks (LAY-27): `7`, `7–8`, `7, 9`,
    /// `7–9, 12`; empty when every check has its card.
    pub fn numbers_text(&self) -> String {
        self.number_parts().join(", ")
    }

    /// The numbers as they are written, ascending, one run of consecutive
    /// numbers per part: two or more as a range with an en dash.
    pub fn number_parts(&self) -> Vec<String> {
        let mut numbers = self.numbers.clone();
        numbers.sort_unstable();
        numbers.dedup();
        let mut parts = Vec::new();
        let mut index = 0;
        while index < numbers.len() {
            let mut end = index;
            while end + 1 < numbers.len() && numbers[end + 1] == numbers[end] + 1 {
                end += 1;
            }
            parts.push(if end > index {
                format!("{}–{}", numbers[index], numbers[end])
            } else {
                numbers[index].to_string()
            });
            index = end + 1;
        }
        parts
    }

    /// The checks line: the folded checks' numbers first (LAY-27), then
    /// `3 checks` before a run, never a tick for a check that did not run;
    /// `✓ 2 checks` when every one passed; `✕ 1/3 failed · 1 skipped` when
    /// one failed; else the state of each (`1 ✓ · 1 waiting`). Gates
    /// follow, counted apart. A step whose one check is folded, with no
    /// gate, writes it instead of the count (`9 · ✓ status == 409`), unless
    /// it did not run (LAY-30, Q1 = A, Q3 = A).
    pub fn text(&self) -> String {
        let mut parts = Vec::new();
        if !self.numbers.is_empty() {
            parts.push(self.numbers_text());
        }
        if let Some(single) = self.written() {
            parts.push(if self.failed > 0 {
                format!("✕ {single}")
            } else if self.passed > 0 {
                format!("✓ {single}")
            } else {
                single.to_owned()
            });
        } else if self.checks > 0 {
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

    /// The predicate the line writes in place of the count, if any.
    fn written(&self) -> Option<&str> {
        self.single
            .as_deref()
            .filter(|_| self.checks == 1 && self.gates == 0 && self.skipped == 0)
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
/// `folded` holds the checks the board does not draw and `numbers` every
/// step's place in the engine's order: a folded check is named by its
/// number in each line it is counted in (LAY-27).
pub fn aggregates(
    graph: &Graph,
    later: Option<&Graph>,
    steps: Option<&[StepResult]>,
    folded: &HashSet<&str>,
    numbers: &HashMap<&str, usize>,
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
            if folded.contains(check.id.as_str())
                && let Some(number) = numbers.get(check.id.as_str())
            {
                aggregate.numbers.push(*number);
            }
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

/// The one check each subject counts while it is folded (LAY-30), by the
/// subject's node id: the check its line may write. `later` is as in
/// [`aggregates`]; a subject with a gate or a second check has none.
pub fn singles(
    graph: &Graph,
    later: Option<&Graph>,
    folded: &HashSet<&str>,
) -> HashMap<String, String> {
    let mut graphs = vec![graph];
    graphs.extend(later);
    let mut counted: HashMap<String, Vec<&str>> = HashMap::new();
    for check in graph.nodes.iter().filter(|node| is_check(node)) {
        for subject in check_subjects_in(&graphs, check).subjects {
            let entry = counted.entry(subject.step.to_owned()).or_default();
            // A gate counts too, so that its subject writes no predicate.
            entry.push(if subject.gate { "" } else { check.id.as_str() });
        }
    }
    counted
        .into_iter()
        .filter_map(|(subject, checks)| match checks.as_slice() {
            [one] if !one.is_empty() && folded.contains(one) => Some((subject, (*one).to_owned())),
            _ => None,
        })
        .collect()
}

/// A check as the line of its subject `subject` writes it (LAY-30): the
/// operands as a card's line prints a reference, the subject's own name
/// left out, and the sign the script prints (`status == 409`,
/// `body.items[0].id == first.body.id`, `nonempty body.items`). `None` for
/// a check the line does not write: a *Data condition* keeps the count.
pub fn written(
    graph: &Graph,
    project: &apiforge_core::format::Project,
    check: &Node,
    subject: &str,
) -> Option<String> {
    if check.kind != NodeKind::Assert {
        return None;
    }
    let scope = apiforge_core::template_text::Scope {
        nodes: &graph.nodes,
        project,
        request: None,
    };
    let operand = |name: &str| {
        check.inputs.get(name).map_or_else(
            || "?".to_owned(),
            |binding| super::layout::own_source_text(binding, scope, graph, subject),
        )
    };
    let predicate = apiforge_core::graph::predicate(check);
    let sign = apiforge_script::predicate_symbol(predicate);
    let mut text = if predicate.takes_expected() {
        format!("{} {sign} {}", operand("actual"), operand("expected"))
    } else {
        format!("{sign} {}", operand("actual"))
    };
    if predicate == apiforge_core::model::Predicate::Approximately {
        let tolerance = check.config.tolerance.unwrap_or(0.0);
        text.push_str(&format!(
            " within {}",
            apiforge_core::js::number_to_string(tolerance)
        ));
    }
    Some(text)
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
            numbers: Vec::new(),
            single: None,
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

    /// LAY-27 criterion 1: the numbers of the folded checks, ascending, a
    /// run of two or more as a range with an en dash, then ` · ` and the
    /// line as before.
    #[test]
    fn lay27_criterion_1_the_numbers_are_written_as_decided() {
        let numbered = |numbers: &[usize]| Aggregate {
            checks: numbers.len(),
            waiting: numbers.len(),
            numbers: numbers.to_vec(),
            ..Aggregate::default()
        };
        assert_eq!(numbered(&[7]).numbers_text(), "7");
        assert_eq!(numbered(&[7, 8]).numbers_text(), "7–8");
        assert_eq!(numbered(&[7, 9]).numbers_text(), "7, 9");
        assert_eq!(numbered(&[7, 8, 9, 12]).numbers_text(), "7–9, 12");
        assert_eq!(numbered(&[12, 9, 7, 8]).numbers_text(), "7–9, 12");
        assert_eq!(numbered(&[7]).text(), "7 · 1 check");
        assert_eq!(numbered(&[7, 8]).text(), "7–8 · 2 checks");
        assert_eq!(numbered(&[21, 23]).text(), "21, 23 · 2 checks");
        let run = Aggregate {
            numbers: vec![7, 8, 9],
            ..aggregate(3, 1, 1, 1)
        };
        assert_eq!(run.text(), "7–9 · ✕ 1/3 failed · 1 skipped");
        let passed = Aggregate {
            numbers: vec![7, 8],
            ..aggregate(2, 2, 0, 0)
        };
        assert_eq!(passed.text(), "7–8 · ✓ 2 checks");
        // Every check drawn: the line as before, byte for byte.
        assert_eq!(numbered(&[]).numbers_text(), "");
        assert_eq!(aggregate(2, 2, 0, 0).text(), "✓ 2 checks");
    }

    /// The aggregate names only the folded checks, by their numbers, under
    /// each subject they count in.
    #[test]
    fn lay27_the_aggregate_names_the_folded_checks() {
        let mut first = node(NodeKind::Assert, "first");
        first
            .inputs
            .insert("actual".into(), Binding::reference("cart", "status", ""));
        let mut shared = node(NodeKind::Assert, "shared");
        shared
            .inputs
            .insert("actual".into(), Binding::reference("cart", "status", ""));
        shared
            .inputs
            .insert("expected".into(), Binding::reference("view", "status", ""));
        let main = Graph {
            nodes: vec![
                node(NodeKind::Request, "cart"),
                first,
                node(NodeKind::Request, "view"),
                shared,
            ],
            edges: Vec::new(),
            proxies: Vec::new(),
        };
        let numbers: HashMap<&str, usize> = sequence(&main)
            .into_iter()
            .enumerate()
            .map(|(index, id)| (id, index + 1))
            .collect();
        let folded: HashSet<&str> = ["first", "shared"].into_iter().collect();
        let all = aggregates(&main, None, None, &folded, &numbers);
        let sorted = |id: &str| {
            let mut numbers = all[id].numbers.clone();
            numbers.sort_unstable();
            numbers
        };
        assert_eq!(sorted("cart"), vec![numbers["first"], numbers["shared"]]);
        assert_eq!(sorted("view"), vec![numbers["shared"]]);
        let drawn = aggregates(&main, None, None, &HashSet::new(), &numbers);
        assert!(drawn.values().all(|aggregate| aggregate.numbers.is_empty()));
        assert_eq!(drawn["cart"].text(), "2 checks");
    }

    /// LAY-30 Q1 = A: a step with exactly one folded check writes it after
    /// its number, with ✓ or ✕ after a run; a check that did not run, a
    /// gate beside it or a second check leave the count (Q3 = A).
    #[test]
    fn lay30_one_check_is_written_on_the_line() {
        let one = |run: Aggregate| Aggregate {
            numbers: vec![9],
            single: Some("status == 409".into()),
            ..run
        };
        let before = one(Aggregate {
            checks: 1,
            waiting: 1,
            ..Aggregate::default()
        });
        assert_eq!(before.text(), "9 · status == 409");
        assert_eq!(one(aggregate(1, 1, 0, 0)).text(), "9 · ✓ status == 409");
        assert_eq!(one(aggregate(1, 0, 1, 0)).text(), "9 · ✕ status == 409");
        assert_eq!(one(aggregate(1, 0, 0, 1)).text(), "9 · 1 skipped");
        let gate = Aggregate {
            gates: 1,
            ..before.clone()
        };
        assert_eq!(gate.text(), "9 · 1 check · 1 gate");
        let two = Aggregate {
            checks: 2,
            waiting: 2,
            numbers: vec![7, 11],
            ..before.clone()
        };
        assert_eq!(two.text(), "7, 11 · 2 checks");
        // Shown as a card, the check is not written: the line as before.
        let drawn = Aggregate {
            numbers: Vec::new(),
            single: None,
            ..before
        };
        assert_eq!(drawn.text(), "1 check");
    }

    /// LAY-30: the predicate as the code writes it, the subject's own name
    /// left out; another step's value keeps its name, so a check of two
    /// steps reads on both.
    #[test]
    fn lay30_the_line_leaves_out_its_own_name() {
        use apiforge_core::model::Predicate;
        let request = |id: &str| node(NodeKind::Request, id);
        let assert =
            |id: &str, predicate: Predicate, actual: Binding, expected: Option<Binding>| {
                let mut check = node(NodeKind::Assert, id);
                check.config.predicate = Some(predicate);
                check.inputs.insert("actual".into(), actual);
                if let Some(expected) = expected {
                    check.inputs.insert("expected".into(), expected);
                }
                check
            };
        let refused = assert(
            "refused",
            Predicate::Equals,
            Binding::reference("second", "status", ""),
            Some(Binding::Literal { value: 409.into() }),
        );
        let one_booking = assert(
            "one_booking",
            Predicate::Equals,
            Binding::reference("calendar", "body", "items[0].id"),
            Some(Binding::reference("first", "body", "id")),
        );
        let has_floors = assert(
            "has_floors",
            Predicate::NotEmpty,
            Binding::reference("floors", "body", "items"),
            None,
        );
        let has_screen = assert(
            "has_screen",
            Predicate::Contains,
            Binding::reference("equipment", "body", "items"),
            Some(Binding::Literal {
                value: "screen".into(),
            }),
        );
        let graph = Graph {
            nodes: vec![
                request("first"),
                request("second"),
                request("calendar"),
                request("floors"),
                request("equipment"),
                refused.clone(),
                one_booking.clone(),
                has_floors.clone(),
                has_screen.clone(),
            ],
            edges: Vec::new(),
            proxies: Vec::new(),
        };
        let project = apiforge_core::demos::empty_project();
        let line = |check: &Node, subject: &str| written(&graph, &project, check, subject);
        assert_eq!(line(&refused, "second").as_deref(), Some("status == 409"));
        assert_eq!(
            line(&one_booking, "calendar").as_deref(),
            Some("body.items[0].id == first.body.id")
        );
        assert_eq!(
            line(&one_booking, "first").as_deref(),
            Some("calendar.body.items[0].id == body.id")
        );
        assert_eq!(
            line(&has_floors, "floors").as_deref(),
            Some("nonempty body.items")
        );
        assert_eq!(
            line(&has_screen, "equipment").as_deref(),
            Some(r#"body.items contains "screen""#)
        );
    }
}
