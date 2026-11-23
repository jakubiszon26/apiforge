//! The step list of Run results (REP-07): a step with the checks that run
//! after it under it, a check's row written as its card writes it
//! (DIAG-02), the handle in the hover and the details.
//!
//! What the list shows is arranged once per change of the run, not once a
//! frame: [`Arranged::update`] groups the rows and writes their texts.

use super::{RunGraphs, StepRow};
use crate::{not_run, theme};
use apiforge_core::{
    format::Graph,
    graph::{component_definition, sequence},
    model::NodeKind,
    run::{CauseEnds, Phase, RunResult, StepStatus},
};
use eframe::egui::{self, Color32, Sense, Stroke, Ui};
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// One line of the list: a row of the run, and the row of the step it is
/// listed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    /// An index into the rows the list was arranged from.
    pub row: usize,
    /// The step the check is listed under; `None` for a row that stands by
    /// itself: a step, or a check with no step.
    pub under: Option<usize>,
}

fn is_check(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Assert | NodeKind::Condition)
}

/// The key of the level a row is at: its own key up to its last step, the
/// same for the steps of one graph.
fn level(key: &str) -> &str {
    key.rfind('/').map_or("", |at| &key[..=at])
}

impl<'a> RunGraphs<'a> {
    /// The graph the steps of one level ran in: Main or Cleanup, through the
    /// component calls that hold them.
    pub(super) fn graph(&self, phase: Phase, calls: &[&str]) -> Option<&'a Graph> {
        let mut graph = match phase {
            Phase::Main => self.main,
            Phase::Cleanup => self.cleanup?,
        };
        for call in calls {
            let call = graph.nodes.iter().find(|node| node.id == *call)?;
            graph = &component_definition(self.project, call)?.graph;
        }
        Some(graph)
    }

    /// The graph a check of one level reads its steps in: Cleanup's checks
    /// read Main's steps too, as DIAG-12's index reads them.
    fn lookup(&self, phase: Phase, calls: &[&str]) -> Option<Graph> {
        let graph = self.graph(phase, calls)?;
        match (phase, calls.is_empty(), self.cleanup) {
            (Phase::Cleanup, true, Some(_)) => {
                let mut both = self.main.clone();
                both.nodes.extend(graph.nodes.iter().cloned());
                Some(both)
            }
            _ => Some(graph.clone()),
        }
    }

    /// The step each check of one level hangs under: the anchor of
    /// `canvas::collapsible` (LAY-11).
    fn anchors(&self, phase: Phase, calls: &[&str]) -> HashMap<String, String> {
        let Some(graph) = self.graph(phase, calls) else {
            return HashMap::new();
        };
        let later = if phase == Phase::Main && calls.is_empty() {
            self.cleanup
        } else {
            None
        };
        crate::canvas::collapsible(graph, later)
            .into_iter()
            .map(|check| (check.check, check.anchor))
            .collect()
    }
}

/// The lines of the list: a check is listed under the step it runs after,
/// the anchor of `canvas::collapsible`, after that step's own inner steps;
/// a check with no step stands by itself; everything else keeps the order
/// of the run. Without the graphs every row stands by itself.
pub fn arrange(rows: &[StepRow<'_>], graphs: Option<RunGraphs<'_>>) -> Vec<Line> {
    let mut anchors: HashMap<(Phase, &str), HashMap<String, String>> = HashMap::new();
    let mut seen: HashMap<(Phase, &str, &str), usize> = HashMap::new();
    let mut attached: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut moved = vec![false; rows.len()];
    for (index, row) in rows.iter().enumerate() {
        let at = level(&row.key);
        let phase = row.step.phase;
        if is_check(row.step.kind)
            && let Some(graphs) = graphs
        {
            let anchors = anchors
                .entry((phase, at))
                .or_insert_with(|| graphs.anchors(phase, &row.calls));
            if let Some(anchor) = anchors.get(&row.step.node_id)
                && let Some(&head) = seen.get(&(phase, at, anchor.as_str()))
            {
                attached.entry(head).or_default().push(index);
                moved[index] = true;
            }
        }
        seen.insert((phase, at, row.step.node_id.as_str()), index);
    }
    let mut lines = Vec::with_capacity(rows.len());
    // The steps whose checks wait for the end of their own inner steps.
    let mut open: Vec<usize> = Vec::new();
    let flush = |head: usize, lines: &mut Vec<Line>| {
        for &check in attached.get(&head).into_iter().flatten() {
            lines.push(Line {
                row: check,
                under: Some(head),
            });
        }
    };
    for (index, row) in rows.iter().enumerate() {
        if moved[index] {
            continue;
        }
        while let Some(&head) = open.last() {
            if rows[head].depth < row.depth {
                break;
            }
            open.pop();
            flush(head, &mut lines);
        }
        lines.push(Line {
            row: index,
            under: None,
        });
        if attached.contains_key(&index) {
            open.push(index);
        }
    }
    while let Some(head) = open.pop() {
        flush(head, &mut lines);
    }
    lines
}

/// What the list writes of one row, beside what the run says.
#[derive(Clone, Debug, Default)]
pub(super) struct Entry {
    /// The step's place in its graph's order, the number of its card on the
    /// board (LAY-07).
    pub number: Option<usize>,
    /// A check's predicate as its card writes it (DIAG-02).
    pub predicate: Option<String>,
    /// The second line of a value, an `if`, a component call or an export.
    pub second: Option<String>,
    /// The name of the component call that holds a nested row.
    pub within: Option<String>,
}

/// The list arranged for one state of a run.
#[derive(Default)]
pub(super) struct Arranged {
    key: Option<u64>,
    pub lines: Vec<Line>,
    pub entries: Vec<Entry>,
}

impl Arranged {
    /// Arranges `rows` again when the run, its rows or what the graphs say
    /// changed: the single-project window edits its graphs in place, so
    /// their contents are read, not their address.
    pub fn update(&mut self, run: &RunResult, rows: &[StepRow<'_>], graphs: Option<RunGraphs<'_>>) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        run.id.hash(&mut hasher);
        rows.len().hash(&mut hasher);
        for row in rows {
            (row.step.status as u8).hash(&mut hasher);
            row.step.outputs.len().hash(&mut hasher);
            row.step.request.is_some().hash(&mut hasher);
        }
        if let Some(graphs) = graphs {
            graphs.hash(rows, &mut hasher);
        }
        let key = hasher.finish();
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.lines = arrange(rows, graphs);
        self.entries = entries(rows, &self.lines, graphs);
    }
}

/// Writes what it is given into a hasher.
struct Hashing<'a, H>(&'a mut H);

impl<H: Hasher> std::io::Write for Hashing<'_, H> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl RunGraphs<'_> {
    /// Hashes what the list reads of the graphs: Main, Cleanup and the
    /// definitions of the components the rows ran in.
    fn hash(&self, rows: &[StepRow<'_>], hasher: &mut impl Hasher) {
        let mut write = |graph: &Graph| {
            // A graph always serialises; a failure only weakens the key.
            let _ = serde_json::to_writer(Hashing(&mut *hasher), graph);
        };
        write(self.main);
        if let Some(cleanup) = self.cleanup {
            write(cleanup);
        }
        let mut levels: Vec<(Phase, &[&str])> = Vec::new();
        for row in rows.iter().filter(|row| !row.calls.is_empty()) {
            let level = (row.step.phase, row.calls.as_slice());
            if !levels.contains(&level) {
                levels.push(level);
            }
        }
        for (phase, calls) in levels {
            if let Some(graph) = self.graph(phase, calls) {
                write(graph);
            }
        }
        for row in rows
            .iter()
            .filter(|row| row.step.kind == NodeKind::Component)
        {
            let component = self
                .graph(row.step.phase, &row.calls)
                .and_then(|graph| graph.nodes.iter().find(|node| node.id == row.step.node_id))
                .and_then(|call| component_definition(self.project, call));
            if let Some(component) = component {
                (&component.name, component.revision).hash(hasher);
            }
        }
    }
}

/// The texts of every row.
fn entries(rows: &[StepRow<'_>], lines: &[Line], graphs: Option<RunGraphs<'_>>) -> Vec<Entry> {
    struct Level {
        graph: Option<Graph>,
        lookup: Option<Graph>,
        numbers: HashMap<String, usize>,
    }
    let mut levels: HashMap<(Phase, String), Level> = HashMap::new();
    let under: HashMap<usize, usize> = lines
        .iter()
        .filter_map(|line| Some((line.row, line.under?)))
        .collect();
    let by_key: HashMap<&str, usize> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.key.as_str(), index))
        .collect();
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let mut entry = Entry {
                within: level(&row.key)
                    .strip_suffix('/')
                    .and_then(|parent| by_key.get(parent))
                    .map(|&parent| rows[parent].step.display_name().to_owned()),
                ..Entry::default()
            };
            entry.second = second_line(row, graphs);
            let Some(graphs) = graphs else {
                return entry;
            };
            let level = levels
                .entry((row.step.phase, level(&row.key).to_owned()))
                .or_insert_with(|| {
                    let graph = graphs.graph(row.step.phase, &row.calls).cloned();
                    let numbers = graph
                        .as_ref()
                        .map(|graph| {
                            sequence(graph)
                                .into_iter()
                                .enumerate()
                                .map(|(index, id)| (id.to_owned(), index + 1))
                                .collect()
                        })
                        .unwrap_or_default();
                    Level {
                        lookup: graphs.lookup(row.step.phase, &row.calls),
                        graph,
                        numbers,
                    }
                });
            entry.number = level.numbers.get(&row.step.node_id).copied();
            if is_check(row.step.kind)
                && let (Some(graph), Some(lookup)) = (&level.graph, &level.lookup)
                && let Some(check) = graph.nodes.iter().find(|node| node.id == row.step.node_id)
            {
                let scope = apiforge_core::template_text::Scope {
                    nodes: &lookup.nodes,
                    project: graphs.project,
                    request: Some(check),
                };
                // Under its step a check leaves the step's name out, as its
                // card does; by itself it keeps every name.
                let own = under
                    .contains_key(&index)
                    .then(|| crate::canvas::own_step(check, lookup))
                    .flatten();
                entry.predicate = Some(crate::canvas::check_text(check, scope, lookup, own).text());
            }
            entry
        })
        .collect()
}

/// The second line of a value, an `if`, a component call or an export:
/// the value, the branch taken, the component.
fn second_line(row: &StepRow<'_>, graphs: Option<RunGraphs<'_>>) -> Option<String> {
    let step = row.step;
    match step.kind {
        NodeKind::Value | NodeKind::Export => step.outputs.get("value").map(one_line),
        NodeKind::If => step
            .outputs
            .get("passed")
            .and_then(Value::as_bool)
            .map(|passed| format!("Branch taken: {}", if passed { "Yes" } else { "No" })),
        NodeKind::Component => {
            let graphs = graphs?;
            let graph = graphs.graph(step.phase, &row.calls)?;
            let call = graph.nodes.iter().find(|node| node.id == step.node_id)?;
            let component = component_definition(graphs.project, call)?;
            Some(format!(
                "Component {} · revision {}",
                component.name, component.revision
            ))
        }
        _ => None,
    }
}

/// A value on one line, at most 120 characters.
fn one_line(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_default();
    crate::labels::truncate(&text, 120)
}

/// The path of a request's URL, without its scheme and host. `://` is a
/// scheme's only after a scheme at the start: a letter, then letters,
/// digits, `+`, `-` or `.`; elsewhere it is part of the path or the query.
pub(super) fn url_path(url: &str) -> &str {
    let scheme = url.split_once("://").filter(|(scheme, _)| {
        let mut chars = scheme.chars();
        chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && chars.all(|char| char.is_ascii_alphanumeric() || matches!(char, '+' | '-' | '.'))
    });
    match scheme {
        Some((_, rest)) => rest.find('/').map_or("/", |at| &rest[at..]),
        None => url,
    }
}

/// What kind of line a row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shape {
    /// A step: a request, a value, a component call and the rest. `checks`
    /// are listed under it, `failed` of them did not pass; `folded` hides
    /// them.
    Head {
        checks: usize,
        failed: usize,
        folded: bool,
    },
    /// A check under the step it runs after.
    Check,
    /// A check with no step.
    Lone,
}

/// One row as the list draws it.
pub(super) struct RowView<'a> {
    pub row: &'a StepRow<'a>,
    pub entry: &'a Entry,
    pub shape: Shape,
    /// The step a check is listed under.
    pub head: Option<&'a StepRow<'a>>,
    pub active: bool,
    pub words: String,
    /// The step did not run: it took no time.
    pub ran: bool,
    pub first: bool,
}

/// What a click on a row asks for.
pub(super) struct Drawn {
    pub row: egui::Response,
    /// ▸ or ▾ of a step with checks under it.
    pub fold: Option<egui::Response>,
}

const NUMBER_WIDTH: f32 = 22.0;
const FOLD_WIDTH: f32 = 14.0;
const HEAD_HEIGHT: f32 = 28.0;
const CHECK_HEIGHT: f32 = 24.0;
const TWO_LINES: f32 = 40.0;
const INDENT: f32 = 14.0;

/// The id of the row of `key`, the same in every frame.
pub(super) fn row_id(key: &str) -> egui::Id {
    egui::Id::new(("report-row", key))
}

/// Whether a step says what it is on a second line (REP-07).
fn two_lines(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Value | NodeKind::If | NodeKind::Component | NodeKind::Export
    )
}

fn kind_icon(kind: NodeKind) -> Option<crate::icons::Icon> {
    use crate::icons::Icon;
    match kind {
        NodeKind::Value => Some(Icon::Variable),
        NodeKind::If => Some(Icon::Wire),
        NodeKind::Component => Some(Icon::Layers),
        NodeKind::Export => Some(Icon::Export),
        _ => None,
    }
}

/// The accessible name of a row: a step's name, a check's predicate, then
/// what the row says of it; a check names itself (its label or handle) and
/// its step, a
/// nested row the call that holds it, so that no two rows share a name
/// (UX-27).
pub(super) fn label(view: &RowView<'_>) -> String {
    let step = view.row.step;
    let mut label = match (&view.entry.predicate, view.shape) {
        (Some(predicate), Shape::Check) => format!(
            "{predicate} · {} · {} of {}",
            view.words,
            step.display_name(),
            view.head.map_or("", |head| head.step.display_name())
        ),
        (Some(predicate), _) => format!("{predicate} · {} · {}", view.words, step.display_name()),
        (None, _) if view.ran => format!(
            "{} · {} · {} ms",
            step.display_name(),
            view.words,
            step.duration_ms
        ),
        (None, _) => format!("{} · {}", step.display_name(), view.words),
    };
    if let Some(within) = &view.entry.within {
        label.push_str(" · in ");
        label.push_str(within);
    }
    label
}

/// The colour of a step's status.
pub(super) fn status_colour(status: StepStatus) -> Color32 {
    super::status_color(status)
}

/// Whether a row did not pass in the way *Failed* keeps: failed, errored,
/// blocked or not met.
pub(super) fn bad(row: &StepRow<'_>, ends: &CauseEnds) -> bool {
    let Some(level) = row.level(ends) else {
        return matches!(
            row.step.status,
            StepStatus::Failed | StepStatus::Error | StepStatus::Blocked
        );
    };
    matches!(
        not_run::Group::of(row.step, level),
        not_run::Group::Failed | not_run::Group::Blocked | not_run::Group::NotMet
    )
}

/// The groups of the summary *Failed* counts.
pub(super) fn counted(group: not_run::Group) -> bool {
    matches!(
        group,
        not_run::Group::Failed | not_run::Group::Blocked | not_run::Group::NotMet
    )
}

/// Draws one row and returns its responses.
pub(super) fn draw(ui: &mut Ui, view: &RowView<'_>) -> Drawn {
    let step = view.row.step;
    let indent = view.row.depth as f32 * INDENT;
    let height = match view.shape {
        Shape::Check => CHECK_HEIGHT,
        _ if two_lines(step.kind) => TWO_LINES,
        _ => HEAD_HEIGHT,
    };
    if view.first {
        // nothing above the first row
    } else if view.shape != Shape::Check {
        ui.add_space(4.0);
    }
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let id = row_id(&view.row.key);
    let response = ui.interact(rect, id, Sense::click());
    let label = label(view);
    let active = view.active;
    let description = description(view);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
    ui.ctx().accesskit_node_builder(id, |node| {
        node.set_selected(active);
        if let Some(description) = description {
            node.set_description(description);
        }
    });
    let failed_check =
        view.shape == Shape::Check && matches!(step.status, StepStatus::Failed | StepStatus::Error);
    let painter = ui.painter().clone();
    if active {
        painter.rect_filled(rect, 6.0, theme::ACCENT_SOFT);
    } else if failed_check {
        painter.rect_filled(rect, 6.0, theme::DANGER.gamma_multiply(0.08));
    } else if response.hovered() {
        painter.rect_filled(rect, 6.0, theme::BG);
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(0.5),
            6.0,
            Stroke::new(1.5, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    let left = rect.left() + indent;
    let badge_x = left + NUMBER_WIDTH + FOLD_WIDTH + 10.0;
    let text_left = badge_x + 14.0;
    let top_line = if two_lines(step.kind) && view.shape != Shape::Check {
        rect.top() + 14.0
    } else {
        rect.center().y
    };
    let colour = status_colour(step.status);
    // The number of a step, never of a check under it.
    if view.shape != Shape::Check
        && let Some(number) = view.entry.number
    {
        painter.text(
            egui::pos2(left + NUMBER_WIDTH, top_line),
            egui::Align2::RIGHT_CENTER,
            number.to_string(),
            egui::FontId::monospace(10.5),
            theme::MUTED,
        );
    }
    let radius = if view.shape == Shape::Check { 6.5 } else { 8.0 };
    let badge = egui::pos2(badge_x, top_line);
    painter.circle_filled(badge, radius, colour.gamma_multiply(0.15));
    painter.text(
        badge,
        egui::Align2::CENTER_CENTER,
        super::status_mark(step.status),
        egui::FontId::proportional(if radius < 8.0 { 9.5 } else { 11.0 }),
        colour,
    );

    // The right end: the duration of a step that ran, what the row says
    // of its checks or of itself.
    let mut right = rect.right() - 8.0;
    if view.ran && view.shape != Shape::Check {
        let galley = painter.layout_no_wrap(
            format!("{} ms", step.duration_ms),
            egui::FontId::monospace(10.5),
            theme::MUTED,
        );
        right -= galley.size().x;
        painter.galley(
            egui::pos2(right, top_line - galley.size().y / 2.0),
            galley,
            theme::MUTED,
        );
        right -= 10.0;
    }
    let tail: Option<(String, Color32)> = match view.shape {
        Shape::Check if failed_check => Some((
            match &step.actual {
                apiforge_core::format::OptionalJson::Present(value) => {
                    format!("actual {}", one_line(value))
                }
                apiforge_core::format::OptionalJson::Missing => "actual (missing value)".into(),
            },
            theme::DANGER,
        )),
        Shape::Check | Shape::Lone if step.status != StepStatus::Passed || !view.ran => {
            Some((view.words.clone(), theme::MUTED))
        }
        Shape::Check | Shape::Lone => None,
        Shape::Head { .. } if step.status != StepStatus::Passed || !view.ran => {
            Some((view.words.clone(), colour))
        }
        Shape::Head { checks: 0, .. } => None,
        Shape::Head { checks, failed, .. } if failed > 0 => {
            Some((format!("{failed} of {checks} failed"), theme::DANGER))
        }
        Shape::Head { checks, .. } => Some((
            if checks == 1 {
                "1 check".to_owned()
            } else {
                format!("{checks} checks")
            },
            theme::MUTED,
        )),
    };
    // The tail takes at most a third of the row.
    let room = (right - text_left).max(0.0);
    if let Some((text, colour)) = tail {
        let mut job = egui::text::LayoutJob::simple_singleline(
            text,
            egui::FontId::proportional(11.0),
            colour,
        );
        job.wrap.max_width = (room / 3.0).max(24.0);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        let galley = painter.layout_job(job);
        right -= galley.size().x;
        painter.galley(
            egui::pos2(right, top_line - galley.size().y / 2.0),
            galley,
            colour,
        );
        right -= 10.0;
    }
    // The status a request received.
    if matches!(view.shape, Shape::Head { .. })
        && let Some(status) = step.outputs.get("status").and_then(Value::as_u64)
        && step.kind == NodeKind::Request
    {
        let galley = painter.layout_no_wrap(
            status.to_string(),
            egui::FontId::new(11.0, theme::bold_family()),
            theme::MUTED,
        );
        right -= galley.size().x;
        painter.galley(
            egui::pos2(right, top_line - galley.size().y / 2.0),
            galley,
            theme::MUTED,
        );
        right -= 10.0;
    }

    let name_colour = if active { theme::ACCENT } else { theme::TEXT };
    // One piece of the first line, cut with … where the row ends.
    let mut x = text_left;
    let piece = |x: &mut f32, text: &str, font: egui::FontId, colour: Color32, gap: f32| {
        let room = right - *x - gap;
        if room < 16.0 {
            return;
        }
        *x += gap;
        let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, colour);
        job.wrap.max_width = room;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        let galley = painter.layout_job(job);
        let width = galley.size().x;
        painter.galley(
            egui::pos2(*x, top_line - galley.size().y / 2.0),
            galley,
            colour,
        );
        *x += width;
    };
    if let Some(predicate) = &view.entry.predicate {
        piece(
            &mut x,
            predicate,
            egui::FontId::monospace(11.5),
            name_colour,
            0.0,
        );
    } else {
        piece(
            &mut x,
            step.display_name(),
            egui::FontId::new(12.5, theme::bold_family()),
            name_colour,
            0.0,
        );
        if step.kind == NodeKind::Start {
            piece(
                &mut x,
                "start",
                egui::FontId::proportional(11.0),
                theme::MUTED,
                8.0,
            );
        }
        if let Some(request) = &step.request {
            piece(
                &mut x,
                &request.method,
                egui::FontId::new(10.5, theme::bold_family()),
                theme::LINK,
                8.0,
            );
            piece(
                &mut x,
                url_path(&request.url),
                egui::FontId::proportional(11.0),
                theme::MUTED,
                5.0,
            );
        }
    }
    // The second line of a value, an `if`, a call or an export: the icon of
    // its kind, then what it holds, chose or calls.
    if view.shape != Shape::Check
        && two_lines(step.kind)
        && let Some(icon) = kind_icon(step.kind)
    {
        let middle = rect.top() + 30.0;
        icon.paint(
            &painter,
            egui::Rect::from_center_size(
                egui::pos2(text_left + 6.0, middle),
                egui::vec2(12.0, 12.0),
            ),
            theme::MUTED,
        );
        if let Some(second) = &view.entry.second {
            let mut job = egui::text::LayoutJob::simple_singleline(
                second.clone(),
                egui::FontId::proportional(11.0),
                theme::MUTED,
            );
            job.wrap.max_width = (rect.right() - 8.0 - text_left - 18.0).max(16.0);
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            job.wrap.overflow_character = Some('…');
            let galley = painter.layout_job(job);
            painter.galley(
                egui::pos2(text_left + 18.0, middle - galley.size().y / 2.0),
                galley,
                theme::MUTED,
            );
        }
    }

    // ▸ or ▾: the fold of a step's checks.
    let fold = match view.shape {
        Shape::Head { checks, folded, .. } if checks > 0 => {
            let fold_rect = egui::Rect::from_center_size(
                egui::pos2(left + NUMBER_WIDTH + 3.0 + FOLD_WIDTH / 2.0, top_line),
                egui::vec2(FOLD_WIDTH + 2.0, 18.0),
            );
            let fold = ui.interact(fold_rect, id.with("fold"), Sense::click());
            let name = if folded {
                format!("Open the checks of {}", step.display_name())
            } else {
                format!("Fold the checks of {}", step.display_name())
            };
            fold.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            let icon = if folded {
                crate::icons::Icon::ChevronRight
            } else {
                crate::icons::Icon::ChevronDown
            };
            icon.paint(
                &painter,
                egui::Rect::from_center_size(fold_rect.center(), egui::vec2(12.0, 12.0)),
                if fold.hovered() {
                    theme::TEXT
                } else {
                    theme::MUTED
                },
            );
            Some(
                fold.on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(if folded {
                        "Show its checks (→)"
                    } else {
                        "Fold its checks (←)"
                    }),
            )
        }
        _ => None,
    };
    let hover = hover(view);
    Drawn {
        row: response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(hover),
        fold,
    }
}

/// What assistive technology says after a row's name: a request's method,
/// path and status, a step's second line.
fn description(view: &RowView<'_>) -> Option<String> {
    let step = view.row.step;
    if let Some(request) = &step.request {
        let mut said = format!("{} {}", request.method, url_path(&request.url));
        if let Some(status) = step.outputs.get("status").and_then(Value::as_u64) {
            said.push_str(&format!(" · {status}"));
        }
        if let Shape::Head { checks, .. } = view.shape
            && checks > 0
        {
            said.push_str(&format!(
                " · {checks} {}",
                if checks == 1 { "check" } else { "checks" }
            ));
        }
        return Some(said);
    }
    view.entry.second.clone()
}

/// The hover of a row: a check's handle and its step, a step's name.
fn hover(view: &RowView<'_>) -> String {
    let step = view.row.step;
    match (view.shape, view.head) {
        (Shape::Check, Some(head)) => format!(
            "{} · {} · of {}",
            step.handle,
            view.words,
            head.step.display_name()
        ),
        (Shape::Lone, _) => format!("{} · {}", step.handle, view.words),
        _ => match &step.request {
            Some(request) => format!(
                "{} · {} {}",
                step.display_name(),
                request.method,
                request.url
            ),
            None => step.display_name().to_owned(),
        },
    }
}

/// The *All | Failed* switch: two buttons side by side, the chosen one
/// filled.
pub(super) fn segment(ui: &mut Ui, text: &str, on: bool, enabled: bool) -> egui::Response {
    let font = egui::FontId::proportional(theme::FONT_LABEL);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), theme::TEXT);
    let size = egui::vec2(galley.size().x + 20.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, text));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if on {
            theme::ACCENT_SOFT
        } else if enabled && response.hovered() {
            theme::FIELD
        } else {
            Color32::TRANSPARENT
        };
        let border = if enabled && response.has_focus() {
            Stroke::new(1.5, theme::ACCENT)
        } else if on {
            Stroke::new(1.0, theme::ACCENT)
        } else {
            Stroke::new(1.0, theme::BORDER)
        };
        painter.rect(rect.shrink(0.5), 5, fill, border, egui::StrokeKind::Inside);
        let colour = if !enabled {
            theme::MUTED
        } else if on {
            theme::ACCENT
        } else {
            theme::TEXT
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            font,
            colour,
        );
    }
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// The width a segment of `text` takes.
pub(super) fn segment_width(ui: &Ui, text: &str) -> f32 {
    super::text_width(ui, text, theme::FONT_LABEL) + 20.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_path_leaves_out_its_scheme_and_host() {
        assert_eq!(
            url_path("https://parcels.example.test/parcels/pc-001/labels"),
            "/parcels/pc-001/labels"
        );
        assert_eq!(url_path("http://localhost:3333"), "/");
        assert_eq!(url_path("/orders/1"), "/orders/1");
    }

    /// Review of REP-07a: `://` is a scheme's only at the start of the URL.
    #[test]
    fn a_url_path_reads_a_scheme_only_at_its_start() {
        assert_eq!(
            url_path("/login?next=https://example.test/home"),
            "/login?next=https://example.test/home"
        );
        assert_eq!(
            url_path("https://example.test/login?next=http://other.test/a"),
            "/login?next=http://other.test/a"
        );
        assert_eq!(
            url_path("{{env.base}}/redirect?to=http://x.test/y"),
            "{{env.base}}/redirect?to=http://x.test/y"
        );
    }
}
