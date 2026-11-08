//! The *Checks* tab of a step's Inspector (LAY-08): the checks that read the
//! step, in words, with their sources, status and moment. A request and a call
//! of a component both have it, because both are subjects of a check (LAY-20).
//!
//! A check with several subjects is listed under each (Q6); a gate is listed
//! under its subject and counted apart; a check with no subject is listed
//! nowhere. A check of the scenario's Cleanup graph is listed under the Main
//! step it reads, with the moment word *Cleanup* and counted apart, so that
//! aggregation hides no failure (LAY-20). Nothing here edits the document.

use super::Context;
use crate::{labels, theme, widgets};
use apiforge_core::{
    format::{Binding, Graph, Node, OptionalJson},
    graph::{check_subjects, condition_inputs, condition_predicates, predicate, sequence},
    model::{Match, NodeKind, Predicate},
    run::{RunResult, RunStatus, StepResult, StepStatus},
};
use eframe::egui::{self, Color32, RichText, Ui};
use indexmap::IndexSet;
use std::collections::HashMap;

/// How a line of the list reads at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Quiet,
    Passed,
    Failed,
}

impl Tone {
    fn color(self) -> Color32 {
        match self {
            Self::Quiet => theme::MUTED,
            Self::Passed => theme::SUCCESS,
            Self::Failed => theme::DANGER,
        }
    }
}

/// Where a check stands in a run, for the line above the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Passed,
    Failed,
    NotRun,
    Waiting,
}

/// One check as the list says it under one subject.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    handle: String,
    /// The name and the moment: `receipt_total — Waiting for receipt · Shared`.
    title: String,
    predicate: String,
    status: String,
    tone: Tone,
    outcome: Outcome,
    sources: Vec<String>,
    shared_with: Vec<String>,
    gate: bool,
    /// Listed under a Main step but run in Cleanup: counted apart, and its
    /// card stays on the Cleanup board (LAY-20).
    cleanup: bool,
}

/// Names a step by its handle, wherever the check found it: a Cleanup check
/// may read a step of the main graph.
struct Names<'a> {
    graphs: Vec<&'a Graph>,
}

impl Names<'_> {
    fn step(&self, id: &str) -> String {
        self.graphs
            .iter()
            .flat_map(|graph| &graph.nodes)
            .find(|node| node.id == id)
            .map_or_else(|| id.to_owned(), |node| node.handle.clone())
    }

    fn reference(&self, id: &str, port: &str, path: &str) -> String {
        let mut text = self.step(id);
        if port != "value" {
            text.push('.');
            text.push_str(port);
        }
        if !path.is_empty() {
            text.push('.');
            text.push_str(path);
        }
        text
    }

    /// An operand in words. What is not a plain value keeps the board's
    /// short description; the sources line still names what it reads.
    fn operand(&self, binding: &Binding) -> String {
        match binding {
            Binding::Reference {
                node_id,
                port,
                path,
            } => self.reference(node_id, port, path),
            Binding::Literal { value } => labels::truncate(&value.to_string(), 80),
            other => labels::binding(other, self.graphs[0]),
        }
    }

    /// `cart.status equals 201`.
    fn comparison(
        &self,
        node: &Node,
        predicate: Predicate,
        actual: &str,
        expected: &str,
    ) -> Option<String> {
        let actual = self.operand(node.inputs.get(actual)?);
        let words = predicate_words(predicate);
        Some(match node.inputs.get(expected) {
            Some(expected) if predicate.takes_expected() => {
                format!("{actual} {words} {}", self.operand(expected))
            }
            _ => format!("{actual} {words}"),
        })
    }

    /// What the check holds: one comparison, or those of a Data condition.
    fn sentence(&self, node: &Node) -> String {
        if node.kind != NodeKind::Condition {
            return self
                .comparison(node, predicate(node), "actual", "expected")
                .unwrap_or_else(|| predicate_words(predicate(node)).to_owned());
        }
        let parts: Vec<String> = condition_predicates(node)
            .into_iter()
            .enumerate()
            .filter_map(|(index, predicate)| {
                let (actual, expected) = condition_inputs(index + 1);
                self.comparison(node, predicate, &actual, &expected)
            })
            .collect();
        parts.join(match node.config.match_mode.unwrap_or(Match::All) {
            Match::All => " and ",
            Match::Any => " or ",
        })
    }

    /// Everything the check reads, once each, in the order it reads it.
    fn sources(&self, node: &Node) -> Vec<String> {
        let sources: IndexSet<String> = node
            .inputs
            .values()
            .flat_map(Binding::references)
            .map(|source| self.reference(source.node_id, source.port, source.path))
            .collect();
        sources.into_iter().collect()
    }
}

fn predicate_words(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Equals => "equals",
        Predicate::NotEquals => "does not equal",
        Predicate::Greater => "is greater than",
        Predicate::Less => "is less than",
        Predicate::Exists => "exists",
        Predicate::NotEmpty => "is not empty",
        Predicate::Contains => "contains",
        Predicate::Type => "has type",
        Predicate::Approximately => "is approximately",
    }
}

fn json(value: &OptionalJson) -> Option<String> {
    match value {
        OptionalJson::Present(value) => Some(value.to_string()),
        OptionalJson::Missing => None,
    }
}

/// The result of a check in words.
fn status(step: Option<&StepResult>) -> (String, Tone, Outcome) {
    let Some(step) = step else {
        return ("Not run yet".to_owned(), Tone::Quiet, Outcome::Waiting);
    };
    match step.status {
        StepStatus::Passed => ("✓ Passed".to_owned(), Tone::Passed, Outcome::Passed),
        StepStatus::Failed | StepStatus::Error => {
            let label = step.status.label();
            let text = match (json(&step.actual), json(&step.expected)) {
                (Some(actual), Some(expected)) => {
                    format!("✕ {label}: got {actual}, expected {expected}")
                }
                _ => match &step.message {
                    Some(message) => format!("✕ {label}: {message}"),
                    None => format!("✕ {label}"),
                },
            };
            (text, Tone::Failed, Outcome::Failed)
        }
        other => {
            let text = match &step.message {
                Some(message) => format!("– {} ({message})", other.label()),
                None => format!("– {}", other.label()),
            };
            (text, Tone::Quiet, Outcome::NotRun)
        }
    }
}

/// When the check runs, as far as `run` says: its result once it has one, else
/// what it still waits for, which is its latest subject.
fn moment(check: &Node, latest: &str, names: &Names<'_>, run: Option<&RunResult>) -> String {
    if let Some(step) = run.and_then(|run| run.step(&check.id)) {
        return step.status.label().to_owned();
    }
    match run {
        Some(run) if run.status != RunStatus::Running => "Not run".to_owned(),
        Some(run) if run.step(latest).is_some() => "Next".to_owned(),
        _ => format!("Waiting for {}", names.step(latest)),
    }
}

/// A check listed under a Main step but run in Cleanup names its phase until it
/// has a result of its own; then the result takes that place (LAY-20).
fn cleanup_moment(check: &Node, run: Option<&RunResult>) -> String {
    match run.and_then(|run| run.step(&check.id)) {
        Some(step) => step.status.label().to_owned(),
        None => "Cleanup".to_owned(),
    }
}

/// The checks of one graph that read the step `subject`, in the order the engine
/// reaches them. `lookup` is where a check's sources are resolved: the Cleanup
/// pass resolves them in both graphs, because its subjects are Main steps.
/// `position` is the board's order, which says which subject comes last.
fn pass(
    checks: &Graph,
    lookup: &Graph,
    names: &Names<'_>,
    position: &HashMap<&str, usize>,
    subject: &str,
    run: Option<&RunResult>,
    cleanup: bool,
) -> Vec<Row> {
    sequence(checks)
        .iter()
        .filter_map(|id| checks.nodes.iter().find(|node| node.id == *id))
        .filter_map(|check| {
            let subjects = check_subjects(lookup, check).subjects;
            let own = subjects
                .iter()
                .find(|candidate| candidate.step == subject)?;
            let latest = subjects
                .iter()
                .map(|candidate| candidate.step)
                .max_by_key(|step| position.get(step))?;
            let (status, tone, outcome) = status(run.and_then(|run| run.step(&check.id)));
            let moment = if cleanup {
                cleanup_moment(check, run)
            } else {
                moment(check, latest, names, run)
            };
            let mut title = format!("{} — {moment}", check.handle);
            // Under its latest subject the line *Shared with* says it alone.
            if own.shared && latest != subject {
                title.push_str(" · Shared");
            }
            if own.gate {
                title.push_str(" · Gate");
            }
            Some(Row {
                handle: check.handle.clone(),
                title,
                predicate: names.sentence(check),
                status,
                tone,
                outcome,
                sources: names.sources(check),
                shared_with: subjects
                    .iter()
                    .filter(|candidate| candidate.step != subject)
                    .map(|candidate| names.step(candidate.step))
                    .collect(),
                gate: own.gate,
                cleanup,
            })
        })
        .collect()
}

fn is_check(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Assert | NodeKind::Condition)
}

/// The steps of both graphs in one, so that a Cleanup check's subjects are
/// found. `check_subjects` reads the nodes alone.
fn both(board: &Graph, cleanup: &Graph) -> Graph {
    Graph {
        nodes: board.nodes.iter().chain(&cleanup.nodes).cloned().collect(),
        edges: Vec::new(),
        proxies: Vec::new(),
    }
}

/// The checks of the step `subject`: those of its own board, then those the
/// scenario's Cleanup graph holds for it, which `cleanup` names while the board
/// shows Main (LAY-20).
fn rows(
    board: &Graph,
    graph: &Graph,
    cleanup: Option<&Graph>,
    subject: &str,
    run: Option<&RunResult>,
) -> Vec<Row> {
    let mut graphs = vec![board, graph];
    graphs.extend(cleanup);
    let names = Names { graphs };
    let position: HashMap<&str, usize> = sequence(board)
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    let mut rows = pass(board, board, &names, &position, subject, run, false);
    // A Cleanup graph without a check needs no pass of its own.
    if let Some(cleanup) = cleanup.filter(|graph| graph.nodes.iter().any(is_check)) {
        let lookup = both(board, cleanup);
        rows.extend(pass(
            cleanup, &lookup, &names, &position, subject, run, true,
        ));
    }
    rows
}

fn plural(count: usize, one: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {one}s")
    }
}

/// The line above the list. Gates and the checks of Cleanup are counted apart
/// from the checks; a run turns the count of checks into what became of them.
fn summary(rows: &[Row], run: bool) -> (String, Tone) {
    let checks: Vec<&Row> = rows
        .iter()
        .filter(|row| !row.gate && !row.cleanup)
        .collect();
    let count = |outcome: Outcome| checks.iter().filter(|row| row.outcome == outcome).count();
    let (failed, passed) = (count(Outcome::Failed), count(Outcome::Passed));
    let (not_run, waiting) = (count(Outcome::NotRun), count(Outcome::Waiting));
    let mut parts = Vec::new();
    if run && failed > 0 {
        parts.push(format!("✕ {failed}/{} failed", checks.len()));
    } else if run && !checks.is_empty() {
        parts.push(format!("{passed}/{} ✓", checks.len()));
    } else if !checks.is_empty() {
        parts.push(plural(checks.len(), "check"));
    }
    if run && not_run > 0 {
        parts.push(format!("{not_run} skipped"));
    }
    if run && waiting > 0 {
        parts.push(format!("{waiting} waiting"));
    }
    let shared = checks
        .iter()
        .filter(|row| !row.shared_with.is_empty())
        .count();
    if shared > 0 {
        parts.push(format!("{shared} shared"));
    }
    let in_cleanup = rows.iter().filter(|row| row.cleanup).count();
    if in_cleanup > 0 {
        parts.push(format!("{in_cleanup} in Cleanup"));
    }
    let gates = rows.iter().filter(|row| row.gate && !row.cleanup).count();
    if gates > 0 {
        parts.push(plural(gates, "gate"));
    }
    let tone = if rows.iter().any(|row| row.tone == Tone::Failed) {
        Tone::Failed
    } else if run && failed == 0 && waiting == 0 && not_run == 0 {
        Tone::Passed
    } else {
        Tone::Quiet
    };
    (parts.join(" · "), tone)
}

fn row(ui: &mut Ui, row: &Row) {
    let border = if row.tone == Tone::Failed {
        theme::DANGER.gamma_multiply(0.5)
    } else {
        theme::BORDER
    };
    let frame = egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(
                RichText::new(&row.title)
                    .family(theme::bold_family())
                    .size(theme::FONT_BODY)
                    .color(theme::TEXT),
            );
            if !row.shared_with.is_empty() {
                ui.label(
                    RichText::new(format!("Shared with: {}", row.shared_with.join(", ")))
                        .size(theme::FONT_META)
                        .color(theme::LINK),
                );
            }
            ui.label(
                RichText::new(&row.predicate)
                    .size(theme::FONT_LABEL)
                    .color(theme::LINK),
            );
            ui.label(
                RichText::new(&row.status)
                    .size(theme::FONT_LABEL)
                    .color(row.tone.color()),
            );
            ui.label(
                RichText::new(format!("Sources: {}", row.sources.join(", ")))
                    .size(theme::FONT_META)
                    .color(theme::MUTED),
            );
        });
    let name = format!("Check {}", row.handle);
    ui.interact(
        frame.response.rect,
        ui.id().with(("check", &row.handle)),
        egui::Sense::hover(),
    )
    .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
}

/// The list of the selected step.
pub(super) fn show(ui: &mut Ui, context: &Context<'_>) {
    let rows = rows(
        context.board,
        context.graph,
        context.cleanup,
        &context.node.id,
        context.run,
    );
    if rows.is_empty() {
        widgets::hint(ui, "No check reads this step yet.");
        return;
    }
    // Three rows, one of them shared, fit the Inspector of the smallest window.
    ui.spacing_mut().item_spacing.y = 3.0;
    let (text, tone) = summary(&rows, context.run.is_some());
    ui.label(
        RichText::new(text)
            .family(theme::bold_family())
            .size(theme::FONT_LABEL)
            .color(tone.color()),
    );
    for line in &rows {
        row(ui, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{
        format::Position,
        graph::create_node,
        run::{Phase, RunMode},
    };

    fn request(id: &str) -> Node {
        let mut node = create_node(NodeKind::Request, Position { x: 0.0, y: 0.0 }, None);
        node.id = id.into();
        node.handle = id.into();
        node
    }

    /// `shared` reads `first` and `second`; `second` waits for `first`.
    fn board() -> Graph {
        let mut second = request("second");
        second
            .inputs
            .insert("url".into(), Binding::reference("first", "body", "id"));
        let mut shared = create_node(NodeKind::Assert, Position { x: 0.0, y: 0.0 }, None);
        shared.id = "shared".into();
        shared.handle = "shared".into();
        shared
            .inputs
            .insert("actual".into(), Binding::reference("first", "body", "a"));
        shared
            .inputs
            .insert("expected".into(), Binding::reference("second", "body", "a"));
        Graph {
            nodes: vec![request("first"), second, shared],
            edges: Vec::new(),
            proxies: Vec::new(),
        }
    }

    fn run(board: &Graph, done: &[&str], status: RunStatus) -> RunResult {
        RunResult {
            id: "run".into(),
            scenario_id: "scenario".into(),
            mode: RunMode::Simulation,
            started_at: String::new(),
            finished_at: None,
            status,
            steps: done
                .iter()
                .map(|id| {
                    let node = board.nodes.iter().find(|node| node.id == *id).unwrap();
                    let mut step = StepResult::new(node, Phase::Main);
                    step.status = StepStatus::Passed;
                    step
                })
                .collect(),
            diagnostic: false,
            snapshot: None,
            error: None,
            notice: None,
        }
    }

    fn title(board: &Graph, subject: &str, run: Option<&RunResult>) -> String {
        rows(board, board, None, subject, run)[0].title.clone()
    }

    /// The moment follows a run under way: a shared check still waits for its
    /// later subject when the earlier one has finished.
    #[test]
    fn the_moment_follows_a_run_under_way() {
        let board = board();
        let early = run(&board, &["first"], RunStatus::Running);
        assert_eq!(
            title(&board, "first", Some(&early)),
            "shared — Waiting for second · Shared"
        );
        let late = run(&board, &["first", "second"], RunStatus::Running);
        assert_eq!(
            title(&board, "first", Some(&late)),
            "shared — Next · Shared"
        );
        assert_eq!(title(&board, "second", Some(&late)), "shared — Next");
        let finished = run(&board, &["first", "second"], RunStatus::Passed);
        assert_eq!(
            title(&board, "second", Some(&finished)),
            "shared — Not run",
            "a finished run that left the check out never reaches it"
        );
    }

    /// The summary says what became of the checks, and gates stand apart.
    #[test]
    fn the_summary_counts_what_became_of_the_checks() {
        let line = |gate: bool, outcome: Outcome, tone: Tone, shared: bool| Row {
            handle: String::new(),
            title: String::new(),
            predicate: String::new(),
            status: String::new(),
            tone,
            outcome,
            sources: Vec::new(),
            shared_with: if shared {
                vec!["cart".into()]
            } else {
                Vec::new()
            },
            gate,
            cleanup: false,
        };
        let passed = line(false, Outcome::Passed, Tone::Passed, false);
        let waiting = line(false, Outcome::Waiting, Tone::Quiet, true);
        let failed = line(false, Outcome::Failed, Tone::Failed, false);
        let skipped = line(false, Outcome::NotRun, Tone::Quiet, false);
        let gate = line(true, Outcome::Waiting, Tone::Quiet, false);

        assert_eq!(
            summary(&[waiting.clone(), gate.clone()], false),
            ("1 check · 1 shared · 1 gate".to_owned(), Tone::Quiet)
        );
        assert_eq!(
            summary(std::slice::from_ref(&gate), false).0,
            "1 gate",
            "no check to count"
        );
        assert_eq!(
            summary(&[passed.clone(), passed.clone(), waiting], true),
            ("2/3 ✓ · 1 waiting · 1 shared".to_owned(), Tone::Quiet)
        );
        assert_eq!(
            summary(&[failed, skipped, passed.clone()], true),
            ("✕ 1/3 failed · 1 skipped".to_owned(), Tone::Failed)
        );
        assert_eq!(
            summary(std::slice::from_ref(&passed), true),
            ("1/1 ✓".to_owned(), Tone::Passed)
        );

        // A check of Cleanup listed under a Main step is counted apart, like a
        // gate, and its failure still colours the line (LAY-20).
        let in_cleanup = Row {
            cleanup: true,
            ..line(false, Outcome::Waiting, Tone::Quiet, false)
        };
        assert_eq!(
            summary(&[passed.clone(), passed.clone(), in_cleanup.clone()], false),
            ("2 checks · 1 in Cleanup".to_owned(), Tone::Quiet)
        );
        assert_eq!(
            summary(&[passed.clone(), in_cleanup, gate], false),
            ("1 check · 1 in Cleanup · 1 gate".to_owned(), Tone::Quiet)
        );
        let failed_in_cleanup = Row {
            cleanup: true,
            ..line(false, Outcome::Failed, Tone::Failed, false)
        };
        assert_eq!(
            summary(&[passed, failed_in_cleanup], true),
            ("1/1 ✓ · 1 in Cleanup".to_owned(), Tone::Failed),
            "the count leaves it out, the colour does not"
        );
    }

    /// A Cleanup check that reads a Main step is listed under that step, with
    /// the moment word *Cleanup* until it has a result (LAY-20).
    #[test]
    fn a_cleanup_check_is_listed_under_the_main_step_it_reads() {
        let board = board();
        let mut kept = create_node(NodeKind::Assert, Position { x: 0.0, y: 0.0 }, None);
        kept.id = "kept".into();
        kept.handle = "kept".into();
        kept.inputs
            .insert("actual".into(), Binding::reference("first", "body", "id"));
        kept.inputs.insert(
            "expected".into(),
            Binding::literal(serde_json::json!("c-1")),
        );
        let cleanup = Graph {
            nodes: vec![request("removed"), kept],
            edges: Vec::new(),
            proxies: Vec::new(),
        };

        let listed = rows(&board, &board, Some(&cleanup), "first", None);
        let titles: Vec<&str> = listed.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(
            titles,
            ["shared — Waiting for second · Shared", "kept — Cleanup"],
            "the Cleanup check comes after the Main checks"
        );
        assert_eq!(
            listed.last().unwrap().sources,
            ["first.body.id"],
            "the Cleanup check names the Main step it reads"
        );
        assert_eq!(
            summary(&listed, false).0,
            "1 check · 1 shared · 1 in Cleanup"
        );

        // Nothing is listed under a step of the Cleanup graph itself: that
        // board lists its own checks.
        assert!(rows(&board, &board, Some(&cleanup), "removed", None).is_empty());
        // The result takes the moment's place once the check has one.
        let done = run(&cleanup, &["kept"], RunStatus::Passed);
        assert_eq!(
            rows(&board, &board, Some(&cleanup), "first", Some(&done))
                .last()
                .unwrap()
                .title,
            "kept — Passed"
        );
    }
}
