//! The bottom panel: live progress and step-by-step results of a scenario run,
//! or of a component run on its own.
//!
//! Draws the toggle bar, run summary, hierarchical step list (including component
//! inner steps and the cleanup phase) and the selected step's comparison, outputs,
//! inputs and HTTP request snapshot.

use crate::icons::Icon;
use crate::{
    labels,
    not_run::{self, NotRun},
    theme, widgets,
};
use apiforge_core::{
    edit::FieldCheck,
    format::{Graph, Node, OptionalJson, Project},
    graph::{condition_predicates, predicate},
    js::json_stringify,
    model::{ErrorPolicy, Match, NodeKind, Predicate},
    run::{CauseEnds, Phase, RunMode, RunResult, RunStatus, StepResult, StepStatus},
};
use eframe::egui::{self, Color32, RichText, Sense, Stroke, Ui};
use serde_json::{Map, Value};
use std::borrow::Cow;

mod list;
pub use list::Line;
#[cfg(test)]
pub use list::arrange;
/// How a check's row is marked, and a URL's path, for *Steps* (REQ-01a).
pub(crate) use list::{actual_text, mark, url_path};

/// Requests from the report panel to the application.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    SelectNode(String),
    /// A check of a field chosen in the menu *Add a check* (DIAG-08).
    AssertField {
        node_id: String,
        port: String,
        path: String,
        check: FieldCheck,
    },
    AssignField {
        node_id: String,
        port: String,
        path: String,
    },
    SetErrorPolicy(ErrorPolicy),
    ExportReport,
    /// The step list's edge was let go at this width, which the host
    /// remembers on this computer (REP-07).
    RememberListWidth(f32),
}

/// A field checked from these results since their run (UX-01): its row
/// reads *Check added*, and the check runs on the next run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedField {
    pub node_id: String,
    pub port: String,
    pub path: String,
}

/// Everything the report panel needs to draw one frame.
pub struct Context<'a> {
    pub run: Option<&'a RunResult>,
    /// The run's cause ends, resolved once per run (LAY-19); `None`
    /// resolves them from the run.
    pub ends: Option<&'a CauseEnds>,
    pub selected_node_id: Option<&'a str>,
    pub error_policy: ErrorPolicy,
    pub running: bool,
    /// The document changed since the run in another way than by checks
    /// added from these results.
    pub stale: bool,
    /// The fields checked from these results since the run, while nothing
    /// else changed (UX-01).
    pub checked: &'a [CheckedField],
    pub files_busy: bool,
    /// The run is a component's on its own (KTLO-23): it has no error policy,
    /// and the empty report does not speak of a scenario.
    pub component: bool,
    pub document_locked: bool,
    /// What the run executed, where a check's card reads what the check
    /// required (DIAG-03); without it the card reads as before.
    pub graphs: Option<RunGraphs<'a>>,
    /// The document's checks as it stands, which mark the fields of a
    /// step's outputs they read in the colour of their outcome in this run
    /// (DIAG-12); `None` marks nothing.
    pub checks: Option<&'a crate::inspector::CheckIndex>,
}

/// The graphs a run executed. A step's result carries only the values a
/// check compared; its predicate is read from the step's node (DIAG-03).
#[derive(Clone, Copy)]
pub struct RunGraphs<'a> {
    /// The scenario's graph, or the component's when it runs on its own.
    pub main: &'a Graph,
    /// The scenario's Cleanup graph.
    pub cleanup: Option<&'a Graph>,
    /// Where the revision a component call is pinned to is found.
    pub project: &'a Project,
}

impl<'a> RunGraphs<'a> {
    /// The node of `row`'s step: through the component calls that hold it,
    /// or `None` when one of them, or the node, is no longer there.
    fn node(&self, row: &StepRow<'_>) -> Option<&'a Node> {
        self.graph(row.step.phase, &row.calls)?
            .nodes
            .iter()
            .find(|node| node.id == row.step.node_id)
    }
}

/// What a check required, as its node says (DIAG-03).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Requirement {
    /// An assertion, an `if` or a Data condition of one check.
    One {
        predicate: Predicate,
        tolerance: f64,
    },
    /// A Data condition of several checks.
    Several { count: usize, mode: Match },
}

impl Requirement {
    /// `None` for a node that is not a check.
    pub(crate) fn of(node: &Node) -> Option<Self> {
        let tolerance = node.config.tolerance.unwrap_or(0.0);
        match node.kind {
            NodeKind::Assert | NodeKind::If => Some(Self::One {
                predicate: predicate(node),
                tolerance,
            }),
            NodeKind::Condition => match condition_predicates(node)[..] {
                [predicate] => Some(Self::One {
                    predicate,
                    tolerance,
                }),
                ref several => Some(Self::Several {
                    count: several.len(),
                    mode: node.config.match_mode.unwrap_or(Match::All),
                }),
            },
            _ => None,
        }
    }

    /// The EXPECTED side of the card.
    fn text(self, expected: &OptionalJson) -> String {
        self.text_with(expected, compact_json)
    }

    /// The requirement with the value written by `value`.
    pub(crate) fn text_with(self, expected: &OptionalJson, value: fn(&Value) -> String) -> String {
        match self {
            Self::One {
                predicate,
                tolerance,
            } => requirement_text(predicate, expected, tolerance, value),
            Self::Several { count, mode } => several_text(count, mode),
        }
    }

    /// `exists` and `nonempty` say what was required of a value that may be
    /// missing, so their box is shown even when neither side has a value.
    fn always_compared(self) -> bool {
        matches!(
            self,
            Self::One {
                predicate: Predicate::Exists | Predicate::NotEmpty,
                ..
            }
        )
    }
}

/// What one check required, as the EXPECTED side of its card says it: the
/// predicate's sign before the value, the value alone for equality (DIAG-03).
pub(crate) fn expected_text(
    predicate: Predicate,
    expected: &OptionalJson,
    tolerance: f64,
) -> String {
    requirement_text(predicate, expected, tolerance, compact_json)
}

/// [`expected_text`] with the value written by `value`.
pub(crate) fn requirement_text(
    predicate: Predicate,
    expected: &OptionalJson,
    tolerance: f64,
    value: fn(&Value) -> String,
) -> String {
    let operand = || match expected {
        // A type is named, not quoted.
        OptionalJson::Present(Value::String(name)) if predicate == Predicate::Type => name.clone(),
        OptionalJson::Present(expected) => value(expected),
        OptionalJson::Missing => "—".into(),
    };
    match predicate {
        Predicate::Equals => operand(),
        Predicate::NotEquals => format!("≠ {}", operand()),
        Predicate::Greater => format!("> {}", operand()),
        Predicate::Less => format!("< {}", operand()),
        Predicate::Approximately if tolerance != 0.0 => {
            format!("≈ {} ± {tolerance}", operand())
        }
        Predicate::Approximately => format!("≈ {}", operand()),
        Predicate::Contains => format!("contains {}", operand()),
        Predicate::Type => format!("is {}", operand()),
        Predicate::Exists => "exists".into(),
        Predicate::NotEmpty => "nonempty".into(),
    }
}

/// The EXPECTED side of a Data condition of several checks.
pub(crate) fn several_text(count: usize, mode: Match) -> String {
    match mode {
        Match::All => format!("all of {count} checks"),
        Match::Any => format!("any of {count} checks"),
    }
}

/// A step in the flattened step list; component children have `depth > 0` and keep
/// the top-level component node's identifier as `root_id`.
#[derive(Clone, Debug, PartialEq)]
pub struct StepRow<'a> {
    pub step: &'a StepResult,
    pub key: String,
    pub root_id: String,
    pub depth: usize,
    /// The node ids of the component calls that hold the step, outermost
    /// first; empty at the top level.
    pub calls: Vec<&'a str>,
}

impl StepRow<'_> {
    /// The cause ends of the step's own graph: the run's, or those of the
    /// component calls that hold it (LAY-19).
    fn level<'e>(&self, ends: &'e CauseEnds) -> Option<&'e CauseEnds> {
        self.calls
            .iter()
            .try_fold(ends, |ends, call| ends.within(call))
    }

    /// Why the step did not run; `None` for a step that ran.
    fn not_run<'e>(&self, ends: &'e CauseEnds) -> Option<NotRun<'e>> {
        NotRun::of(self.step, self.level(ends)?)
    }

    /// The step's first word: its cause's kind for a step that did not run,
    /// *Not met* for a false *Data condition*, otherwise its status
    /// (LAY-19).
    fn word(&self, ends: &CauseEnds) -> &'static str {
        match self.not_run(ends) {
            Some(not_run) => not_run.word(),
            None if not_run::unmet(self.step) => "Not met",
            None => self.step.status.label(),
        }
    }

    /// What the row says of the step: what its card says, the steps it kept
    /// from running after a step that ran (LAY-19).
    fn words(&self, ends: &CauseEnds) -> String {
        if let Some(not_run) = self.not_run(ends) {
            return not_run.words();
        }
        match self
            .level(ends)
            .map_or(0, |level| level.kept(&self.step.node_id))
        {
            0 => self.word(ends).to_owned(),
            count => format!("{} · {}", self.word(ends), not_run::count_text(count)),
        }
    }
}

/// Flattens top-level steps and any nested component steps in execution order.
pub fn flatten_steps(steps: &[StepResult]) -> Vec<StepRow<'_>> {
    let mut out = Vec::new();
    flatten_into(steps, &[], "", 0, &mut out);
    out
}

fn flatten_into<'a>(
    steps: &'a [StepResult],
    calls: &[&'a str],
    prefix: &str,
    depth: usize,
    out: &mut Vec<StepRow<'a>>,
) {
    for (index, step) in steps.iter().enumerate() {
        let key = format!("{prefix}{index}:{}", step.node_id);
        let root = calls.first().copied().unwrap_or(&step.node_id);
        out.push(StepRow {
            step,
            key: key.clone(),
            root_id: root.to_owned(),
            depth,
            calls: calls.to_vec(),
        });
        if let Some(children) = &step.children {
            let child_prefix = format!("{key}/");
            let mut inner = calls.to_vec();
            inner.push(&step.node_id);
            flatten_into(children, &inner, &child_prefix, depth + 1, out);
        }
    }
}

/// Chooses the active step row following the reference priority:
/// explicit key -> selected board node -> first failed/errored step -> first step.
pub fn resolve_selected<'a, 'b>(
    rows: &'b [StepRow<'a>],
    selected_key: Option<&str>,
    selected_node_id: Option<&str>,
) -> Option<&'b StepRow<'a>> {
    rows.iter()
        .find(|row| Some(row.key.as_str()) == selected_key)
        .or_else(|| selected_node_id.and_then(|id| rows.iter().find(|row| row.step.node_id == id)))
        .or_else(|| {
            rows.iter()
                .find(|row| matches!(row.step.status, StepStatus::Error | StepStatus::Failed))
        })
        .or_else(|| rows.first())
}

/// What *Expand all* and *Collapse all* last chose (REP-08). It holds while
/// another step is selected, so every step opens the same way, until the
/// next run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Folding {
    /// Neither: a value opens to its leaves, a large one to its second level.
    #[default]
    Default,
    ExpandAll,
    CollapseAll,
}

pub(crate) use crate::inspector::Opening;

/// The most nodes a value opens to its leaves by default (REP-08).
const OPEN_NODES: usize = 2000;

/// How a value's rows open before one is folded by hand (REP-08): to its
/// leaves, or to its second level when it has more than [`OPEN_NODES`]
/// nodes or more open rows than are drawn at once, or to its first where
/// even the second level's rows are not drawn at once (review round,
/// 2026-11-23); *Expand all* opens any value to its leaves and *Collapse
/// all* folds every row.
pub(crate) fn opening(folding: Folding, value: &Value) -> Opening {
    match folding {
        Folding::ExpandAll => Opening::Leaves,
        Folding::CollapseAll => Opening::Closed,
        Folding::Default => by_default(value).0,
    }
}

/// How a value opens with neither *Expand all* nor *Collapse all*, and
/// whether it has more than [`OPEN_NODES`] nodes.
fn by_default(value: &Value) -> (Opening, bool) {
    let large = more_nodes_than(value, OPEN_NODES);
    let opening = if !large && Opening::Leaves.drawn_whole(value) {
        Opening::Leaves
    } else if Opening::Levels(2).drawn_whole(value) {
        Opening::Levels(2)
    } else {
        Opening::Levels(1)
    };
    (opening, large)
}

/// Whether `value` has more than `limit` nodes, the value itself included.
fn more_nodes_than(value: &Value, limit: usize) -> bool {
    fn walk(value: &Value, left: &mut usize) -> bool {
        if *left == 0 {
            return true;
        }
        *left -= 1;
        match value {
            Value::Array(items) => items.iter().any(|item| walk(item, left)),
            Value::Object(fields) => fields.values().any(|item| walk(item, left)),
            _ => false,
        }
    }
    let mut left = limit;
    walk(value, &mut left)
}

/// The most of a value's JSON text Run results lay out (REP-08); *Copy*
/// takes all of it.
const JSON_SHOWN_CHARS: usize = 100_000;

/// The accessible names of the controls above a value (REP-08), distinct
/// from every other control of the window (#235).
pub(crate) const SHOW_TREE: &str = "Show as tree";
pub(crate) const SHOW_JSON: &str = "Show as JSON";
pub(crate) const COPY_JSON: &str = "Copy as JSON";
const CHOOSE_A_FIELD: &str = "Choose a field → add a check";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Outputs,
    Inputs,
    Request,
}

const MIN_REPORT_HEIGHT: f32 = 160.0;
/// The least height in a short workspace: what the bar, the run's header and
/// one step row take whole, the height egui used to hold the panel at
/// (owner, 2026-11-03, on UX-05).
const MIN_COMPACT_REPORT_HEIGHT: f32 = 185.0;
/// The least height Run results leave to the views above them, which are
/// drawn there whatever the panel's height (UX-36): a header row of 46 px
/// (the board's, or the Problems caption row under the code) and three
/// lines of code of 16 px with the code panel's 14 px of margins.
pub(crate) const VIEWS_LEAST_HEIGHT: f32 = 108.0;
const KEYBOARD_RESIZE_STEP: f32 = 32.0;
/// How near the workspace's top a drag that is released there fills the
/// workspace (UX-05, decided by the owner).
pub(crate) const DROP_SNAP: f32 = 64.0;

/// A drag of Run results by its grip or its edge, while it lasts (UX-05).
#[derive(Clone, Copy, Debug)]
struct Drag {
    /// The normal height when the drag began, which a panel maximised by the
    /// release returns to.
    height_before: f32,
    /// The pointer holds the panel's top within [`DROP_SNAP`] of the
    /// workspace's top; the panel itself stops lower, where the views above
    /// keep [`VIEWS_LEAST_HEIGHT`] (UX-36).
    snapped: bool,
    /// The most height the drag has seen allowed, which a release outside
    /// the zone keeps to: a notice under the tab strip that comes and goes
    /// during the drag (storage work) does not lower it for good.
    ceiling: f32,
}

/// Computes the next report panel height for a keyboard resize key.
pub(crate) fn keyboard_height(current: f32, key: egui::Key, min: f32, max: f32) -> Option<f32> {
    let low = min.min(max);
    let high = max.max(low);
    match key {
        egui::Key::ArrowUp => Some((current + KEYBOARD_RESIZE_STEP).clamp(low, high)),
        egui::Key::ArrowDown => Some((current - KEYBOARD_RESIZE_STEP).clamp(low, high)),
        egui::Key::Home => Some(low),
        egui::Key::End => Some(high),
        _ => None,
    }
}

pub struct ReportPanel {
    pub open: bool,
    height: f32,
    compact_height: f32,
    /// The panel fills the window below the tab strip (KTLO-19); never remembered.
    maximised: bool,
    /// A drag towards the top, which a release near it maximises (UX-05).
    drag: Option<Drag>,
    /// While the grip is dragged, how far under the panel's top it was
    /// pressed.
    grab: Option<f32>,
    #[cfg(test)]
    last_handle: Option<(egui::Id, egui::Rect, f32)>,
    /// The last frame's workspace, highest height and panel rect.
    #[cfg(test)]
    last_frame: Option<(egui::Rect, f32, egui::Rect)>,
    last_run_id: Option<String>,
    last_selected_node_id: Option<String>,
    selected_key: Option<String>,
    tab: DetailTab,
    /// *Expand all* or *Collapse all*, holding from step to step (REP-08).
    folding: Folding,
    /// Counts the choices of *Expand all* and *Collapse all*, and the runs:
    /// the rows folded or opened by hand before the last one are forgotten.
    epoch: u64,
    /// The trees drawn since the last change of `epoch`, whose rows' state
    /// is freed at the next.
    trees: std::collections::HashSet<egui::Id>,
    /// How each value of the run opens by default, and whether it is large,
    /// per step, tab and port: counted once, not every frame.
    openings: std::collections::HashMap<egui::Id, (Opening, bool)>,
    /// The value is shown as JSON text instead of a tree (REP-08); the
    /// folder window keeps it with the project's view.
    json: bool,
    /// The JSON text of the value shown, for the value it was written from.
    json_text: Option<(egui::Id, std::sync::Arc<String>)>,
    /// This frame's `Context::checked`, for the value rows.
    checked: Vec<CheckedField>,
    /// The step list's width (REP-07), before it is fitted to the panel.
    list_width: f32,
    /// The width the host last gave, from the preferences.
    adopted_width: Option<f32>,
    /// While the list's edge is dragged: where the press was, the width
    /// shown then and the width it had before it was fitted to the panel.
    edge_grab: Option<(f32, f32, f32)>,
    /// The steps whose checks are folded, by row key; none after a run.
    folded: std::collections::HashSet<String>,
    /// *Failed*: only the steps with a failed, blocked or errored row.
    failed_only: bool,
    /// The row the keyboard moved to, which takes the focus when drawn.
    focus: Option<String>,
    /// The node a row last asked the board to select: the board may answer
    /// with the step the check is folded into (LAY-11), which leaves the
    /// check's row chosen once.
    asked: Option<String>,
    /// The list arranged for the run shown.
    arranged: list::Arranged,
}

/// The step list's width before it is dragged (the owner's default of
/// 2026-11-23, about M1a's) and its least width (REP-07); the details keep
/// at least [`DETAILS_LEAST_WIDTH`].
const LIST_WIDTH: f32 = 440.0;
const LIST_LEAST_WIDTH: f32 = 240.0;
const DETAILS_LEAST_WIDTH: f32 = 400.0;
/// The width of the list's edge.
const LIST_EDGE: f32 = 9.0;
/// The accessible name of the list's edge.
pub(crate) const RESIZE_LIST: &str = "Resize the step list";

impl Default for ReportPanel {
    fn default() -> Self {
        Self {
            open: false,
            height: 320.0,
            compact_height: 180.0,
            maximised: false,
            drag: None,
            grab: None,
            #[cfg(test)]
            last_handle: None,
            #[cfg(test)]
            last_frame: None,
            last_run_id: None,
            last_selected_node_id: None,
            selected_key: None,
            tab: DetailTab::Outputs,
            folding: Folding::Default,
            epoch: 0,
            trees: Default::default(),
            openings: Default::default(),
            json: false,
            json_text: None,
            checked: Vec::new(),
            list_width: LIST_WIDTH,
            adopted_width: None,
            edge_grab: None,
            folded: Default::default(),
            failed_only: false,
            focus: None,
            asked: None,
            arranged: Default::default(),
        }
    }
}

impl ReportPanel {
    #[cfg(test)]
    pub(crate) fn debug_resize_handle(&self) -> (egui::Id, egui::Rect, f32) {
        self.last_handle.unwrap()
    }

    /// The remembered heights and the drag, for a test's failure message.
    #[cfg(test)]
    pub(crate) fn debug_state(&self) -> String {
        format!(
            "height {} compact {} drag {:?} maximised {}, last frame (workspace, \
             highest, panel) {:?}",
            self.height, self.compact_height, self.drag, self.maximised, self.last_frame
        )
    }

    /// Whether values are shown as JSON text instead of a tree (REP-08).
    pub fn json_view(&self) -> bool {
        self.json
    }

    /// Shows values as JSON text, or as a tree, as the project's view keeps
    /// it.
    pub fn set_json_view(&mut self, json: bool) {
        self.json = json;
    }

    /// The step list's width as the host remembers it (REP-07): taken when
    /// it differs from what the host gave before, so a drag in progress is
    /// not undone by the value it will replace.
    pub fn adopt_list_width(&mut self, width: Option<f32>) {
        if self.adopted_width != width {
            self.adopted_width = width;
            self.list_width = width.unwrap_or(LIST_WIDTH);
        }
    }

    /// Whether the open panel fills the window, so the caller draws nothing else.
    pub fn is_maximised(&self) -> bool {
        self.open && self.maximised
    }

    /// Returns a maximised panel to its normal height.
    pub fn restore(&mut self) {
        self.maximised = false;
    }

    /// The remembered height of the open panel in the layout `compact` names.
    fn normal_height(&self, compact: bool) -> f32 {
        if compact {
            self.compact_height
        } else {
            self.height
        }
    }

    fn set_normal_height(&mut self, compact: bool, height: f32) {
        if compact {
            self.compact_height = height;
        } else {
            self.height = height;
        }
    }

    /// Ends a drag (UX-05). Released in the zone, the panel is maximised as
    /// *Maximise run results* leaves it, and keeps the height it had before
    /// the drag for the restore; released anywhere else, it stays at the
    /// height it was released at (UX-36).
    fn release(&mut self, drag: Drag, compact: bool, min: f32, height: f32) {
        self.drag = None;
        if drag.snapped {
            self.maximised = true;
            self.set_normal_height(compact, drag.height_before);
        } else {
            self.set_normal_height(compact, height.clamp(min, drag.ceiling.max(min)));
        }
    }

    pub fn show(&mut self, ui: &mut Ui, context: &Context<'_>) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.sync_selection(context.run, context.selected_node_id) {
            // *Expand all* and *Collapse all* hold until the next run, whose
            // values open as by default (review round, 2026-11-23).
            self.folding = Folding::Default;
            self.openings.clear();
            self.forget_trees(ui.ctx());
        }
        if self.checked != context.checked {
            self.checked = context.checked.to_vec();
        }

        let was_open = self.open;
        let maximised = self.is_maximised();
        // The workspace a drag can fill: everything under the tab strip, its
        // notices and the holder bar (UX-05).
        let workspace = ui.available_rect_before_wrap();
        let full = workspace.height();
        let compact = full < 460.0;
        let min_height = if compact {
            MIN_COMPACT_REPORT_HEIGHT
        } else {
            MIN_REPORT_HEIGHT
        };
        // The panel resizes freely between its least height and the height
        // that leaves the views above theirs (UX-36).
        let highest = (full - VIEWS_LEAST_HEIGHT).max(min_height);
        let panel_id = if maximised {
            egui::Id::new("report-maximised")
        } else if was_open {
            egui::Id::new(("report-expanded", compact))
        } else {
            egui::Id::new("report-collapsed")
        };
        let handle_id = ui.id().with("report-resize-handle");
        let edge = ui.ctx().read_response(panel_id.with("__resize"));
        let edge_held = edge.as_ref().is_some_and(egui::Response::dragged);
        let edge_let_go = edge.as_ref().is_some_and(egui::Response::drag_stopped);
        if was_open
            && !maximised
            && self.drag.is_none()
            && (edge_held
                || ui
                    .ctx()
                    .read_response(handle_id)
                    .is_some_and(|grip| grip.dragged()))
        {
            self.drag = Some(Drag {
                height_before: self.normal_height(compact).clamp(min_height, highest),
                snapped: false,
                ceiling: highest,
            });
        }
        // egui keeps the size a panel was last drawn at and prefers it to
        // `default_size`; a workspace shorter for a frame would keep the
        // panel short after it. Unless egui's own edge is being dragged, the
        // panel is drawn at the height kept here, so no other id or epoch is
        // needed to make egui forget a size.
        if was_open && !maximised && !edge_held && !edge_let_go {
            ui.ctx()
                .data_mut(|data| data.remove::<egui::PanelState>(panel_id));
        }
        let height = self.normal_height(compact).clamp(min_height, highest);
        let collapsed_height = if compact { 44.0 } else { 48.0 };
        let response = egui::Panel::bottom(panel_id)
            .resizable(self.open && !maximised)
            .default_size(if maximised {
                full
            } else if self.open {
                height
            } else {
                collapsed_height
            })
            .size_range(if maximised {
                full..=full
            } else if self.open {
                min_height..=highest
            } else {
                collapsed_height..=collapsed_height
            })
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(
                        if compact { 16 } else { 24 },
                        if compact { 6 } else { 8 },
                    )),
            )
            .show(ui, |ui| {
                ui.set_min_height(ui.available_height());
                self.toggle_bar(ui, context, &mut actions);
                if was_open {
                    ui.separator();
                    match context.run {
                        None => empty_state(ui),
                        Some(run) => self.run_contents(ui, run, context, &mut actions),
                    }
                }
            });
        // The panel's own edge, found unnamed by the audit of a component's run
        // results (KTLO-23).
        widgets::name_resize_handle(ui.ctx(), panel_id, "Resize run results");
        if !was_open {
            return actions;
        }
        let rect = response.response.rect;
        let actual_height = rect.height().clamp(min_height, highest);
        // The height to come back to is taken from the panel only while its
        // edge is dragged; the grip, the keyboard and a release set it
        // themselves. Taken every frame, a workspace that shrinks for a
        // moment (a notice under the tab strip while storage work runs)
        // shortened it for good. Maximised, it is kept for the restore.
        if !maximised && (edge_held || edge_let_go) {
            // egui draws the edge no higher than this frame allows; held at
            // that end in a frame whose workspace is shorter for a moment,
            // the height the drag held above it is kept, up to the most the
            // drag has seen allowed (UX-36).
            let kept = self.normal_height(compact);
            let ceiling = self.drag.map_or(highest, |drag| drag.ceiling.max(highest));
            let next = if actual_height >= highest - 0.5 && kept > highest {
                kept.min(ceiling)
            } else {
                actual_height
            };
            self.set_normal_height(compact, next);
        }
        // The grip: the panel's own handle, kept at the top of the maximised
        // panel so that it can be dragged back down (UX-05).
        let center = rect.center_top() + egui::vec2(0.0, 8.0);
        let handle_rect = egui::Rect::from_center_size(center, egui::vec2(80.0, 14.0));
        let handle = ui
            .interact(handle_rect, handle_id, Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::ResizeVertical)
            .on_hover_text("Resize run report (↑/↓, Home, End)");
        let current_height = if maximised {
            rect.height()
        } else {
            actual_height
        };
        handle.widget_info(move || {
            let mut info =
                egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Resize run report");
            info.value = Some(f64::from(current_height.round()));
            info
        });
        #[cfg(test)]
        {
            self.last_handle = Some((handle_id, handle_rect, current_height));
            self.last_frame = Some((workspace, highest, rect));
        }
        if handle.clicked() || handle.drag_started() {
            handle.request_focus();
        }
        // The grip follows the pointer by position, as egui's edge does: the
        // panel's top stays where the press landed on the grip, so a pointer
        // taken past either end of the range finds the panel again where it
        // left it (UX-05).
        if handle.dragged() && self.grab.is_none() {
            let origin = ui.input(|input| input.pointer.press_origin());
            self.grab = Some(origin.map_or(8.0, |origin| origin.y - rect.top()));
        } else if !handle.dragged() {
            self.grab = None;
        }
        // Where the pointer holds the panel's top, past the end of its range
        // too: the snap zone is reached by the pointer, since the panel stops
        // where the views above keep their least height (UX-36).
        let pointer_top = handle
            .interact_pointer_pos()
            .zip(self.grab)
            .filter(|_| handle.dragged())
            .map(|(pointer, grab)| pointer.y - grab)
            .or_else(|| {
                edge.as_ref()
                    .filter(|_| edge_held || edge_let_go)
                    .and_then(egui::Response::interact_pointer_pos)
                    .map(|pointer| pointer.y)
            });
        let wanted = pointer_top
            .filter(|_| handle.dragged())
            .map(|top| rect.bottom() - top);
        if maximised {
            // Dragged down, the panel is restored under the pointer and the
            // same grip goes on dragging it.
            if let Some(wanted) = wanted
                && wanted < rect.height() - 0.5
            {
                self.drag = Some(Drag {
                    height_before: self.normal_height(compact).clamp(min_height, highest),
                    snapped: false,
                    ceiling: highest,
                });
                self.maximised = false;
                self.set_normal_height(compact, wanted.clamp(min_height, highest));
                ui.ctx().request_repaint();
            }
        } else {
            if let Some(wanted) = wanted {
                // Up to the most height the drag has seen allowed: a frame
                // whose workspace is shorter for a moment draws the panel
                // lower, but does not lower the height the drag holds.
                let ceiling = self.drag.map_or(highest, |drag| drag.ceiling.max(highest));
                self.set_normal_height(compact, wanted.clamp(min_height, ceiling));
                ui.ctx().request_repaint();
            }
            if handle.has_focus() && self.drag.is_none() {
                // The keys move the panel over the same range as the drag and
                // never maximise it (UX-05 criterion 9, UX-36).
                let keys = [
                    egui::Key::ArrowUp,
                    egui::Key::ArrowDown,
                    egui::Key::Home,
                    egui::Key::End,
                ];
                let mut updated = None;
                ui.input_mut(|input| {
                    for key in keys {
                        if input.consume_key(egui::Modifiers::NONE, key)
                            && let Some(next) = keyboard_height(
                                updated.unwrap_or(actual_height),
                                key,
                                min_height,
                                highest,
                            )
                        {
                            updated = Some(next);
                        }
                    }
                });
                if let Some(next) = updated
                    && (next - actual_height).abs() > f32::EPSILON
                {
                    self.set_normal_height(compact, next);
                    ui.ctx().request_repaint();
                }
            }
            if let Some(drag) = self.drag.as_mut() {
                if let Some(top) = pointer_top {
                    drag.snapped = top <= workspace.top() + DROP_SNAP;
                }
                drag.ceiling = drag.ceiling.max(highest);
                let held = handle.dragged() || edge_held;
                let let_go = handle.drag_stopped() || edge_let_go;
                if let_go || !held {
                    let mut drag = *drag;
                    // A drag that ends without a release (the panel went
                    // away under it) maximises nothing.
                    drag.snapped &= let_go;
                    // The height the drag held, not the one this frame could
                    // draw: released in a frame whose workspace is shorter
                    // (a notice under the tab strip), the panel would keep
                    // that frame's clamp for good. Dragged by egui's edge,
                    // the held height is taken above, by the same rule.
                    let held_height = self.normal_height(compact);
                    self.release(drag, compact, min_height, held_height);
                    if drag.snapped {
                        // The grip moves to the top under a released pointer;
                        // the maximised panel is shown as the button leaves it.
                        handle.surrender_focus();
                    }
                    ui.ctx().request_repaint();
                } else if drag.snapped {
                    // Over the views above, which stay drawn under it.
                    let zone = egui::Rect::from_min_max(
                        workspace.min,
                        egui::pos2(workspace.max.x, rect.top()),
                    )
                    .shrink(4.0);
                    widgets::drop_zone(ui, zone, "Release to fill the workspace");
                }
            }
        }
        let accent = self.drag.is_some() || handle.has_focus();
        if accent {
            ui.painter().rect_stroke(
                handle_rect,
                4.0,
                Stroke::new(1.5, theme::ACCENT),
                egui::StrokeKind::Outside,
            );
        }
        ui.painter().line_segment(
            [
                center - egui::vec2(18.0, 0.0),
                center + egui::vec2(18.0, 0.0),
            ],
            Stroke::new(2.0, if accent { theme::ACCENT } else { theme::BORDER }),
        );
        actions
    }

    /// Follows the run and the board's selection; returns whether the run
    /// changed.
    fn sync_selection(&mut self, run: Option<&RunResult>, selected_node_id: Option<&str>) -> bool {
        let run_id = run.map(|run| run.id.as_str());
        let run_changed = self.last_run_id.as_deref() != run_id;
        let node_changed = self.last_selected_node_id.as_deref() != selected_node_id;
        if run_changed {
            self.last_run_id = run_id.map(str::to_owned);
            self.selected_key = None;
            self.json_text = None;
            // Every step's checks are shown after a run (REP-07).
            self.folded.clear();
            self.focus = None;
            self.asked = None;
        }
        // What a row asked for is answered by the next change of the
        // board's selection; the node itself may come first.
        let asked = if node_changed {
            self.last_selected_node_id = selected_node_id.map(str::to_owned);
            let asked = self.asked.take();
            if asked.as_deref() == selected_node_id {
                self.asked.clone_from(&asked);
            }
            asked
        } else {
            None
        };
        if (run_changed || node_changed)
            && let (Some(run), Some(node_id)) = (run, selected_node_id)
        {
            let rows = flatten_steps(&run.steps);
            // Clicking a component child selects its parent on the board. Keep
            // the explicit child row while that matching board selection settles.
            if !run_changed
                && rows.iter().any(|row| {
                    Some(row.key.as_str()) == self.selected_key.as_deref() && row.root_id == node_id
                })
            {
                return run_changed;
            }
            // A check folded into its step on the board selects that step
            // there (LAY-11) when its row asks for it; the check's row
            // listed under that step stays chosen (REP-07). A later choice
            // of the step on the board chooses the step's row.
            if !run_changed
                && let Some(index) = rows
                    .iter()
                    .position(|row| Some(row.key.as_str()) == self.selected_key.as_deref())
                && asked.as_deref() == Some(rows[index].root_id.as_str())
                && self.arranged.lines.iter().any(|line| {
                    line.row == index
                        && line.under.is_some_and(|head| {
                            rows.get(head)
                                .is_some_and(|head| head.step.node_id == node_id)
                        })
                })
            {
                return run_changed;
            }
            if let Some(item) = rows
                .iter()
                .find(|row| row.depth == 0 && row.step.node_id == node_id)
            {
                self.selected_key = Some(item.key.clone());
            }
        }
        run_changed
    }

    /// Forgets the rows folded or opened by hand, and frees what their trees
    /// kept in egui's memory: at a new run, *Expand all* or *Collapse all*.
    fn forget_trees(&mut self, ctx: &egui::Context) {
        self.epoch += 1;
        for tree in self.trees.drain() {
            crate::inspector::forget_results_tree(ctx, tree);
        }
    }

    fn toggle_bar(&mut self, ui: &mut Ui, context: &Context<'_>, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            let arrow = if self.open {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            };
            if ui
                .add(widgets::action(arrow, "Run results").quiet())
                .clicked()
            {
                self.open = !self.open;
                self.maximised = false;
            }
            if let Some(run) = context.run {
                let (label, color) = run_pill(run.status);
                widgets::pill(ui, label, color);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut policy = context.error_policy;
                ui.add_enabled_ui(!context.running && !context.document_locked, |ui| {
                    if context.component {
                        return;
                    }
                    let combo = egui::ComboBox::from_id_salt("report-error-policy")
                        .selected_text(error_policy_label(policy))
                        .width(190.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut policy,
                                ErrorPolicy::Continue,
                                error_policy_label(ErrorPolicy::Continue),
                            );
                            ui.selectable_value(
                                &mut policy,
                                ErrorPolicy::Stop,
                                error_policy_label(ErrorPolicy::Stop),
                            );
                        });
                    widgets::named(combo.response, "When a step fails");
                });
                if policy != context.error_policy {
                    actions.push(Action::SetErrorPolicy(policy));
                }
                ui.add_space(4.0);
                let name = if self.is_maximised() {
                    "Restore run results"
                } else {
                    "Maximise run results"
                };
                if ui
                    .add(widgets::icon_button(Icon::Fit, name).compact())
                    .clicked()
                {
                    self.maximised = !self.is_maximised();
                    self.open = true;
                }
                ui.add_space(8.0);
                if context.stale {
                    ui.label(
                        RichText::new("Graph changed · results from an earlier version")
                            .size(11.0)
                            .color(theme::WARNING),
                    );
                } else if !context.checked.is_empty() {
                    // Checks added from these results leave them usable
                    // (UX-01); they run on the next run.
                    ui.label(
                        RichText::new(checks_added(context.checked.len()))
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                }
            });
        });
    }

    fn run_contents(
        &mut self,
        ui: &mut Ui,
        run: &RunResult,
        context: &Context<'_>,
        actions: &mut Vec<Action>,
    ) {
        let resolved;
        let ends = match context.ends {
            Some(ends) => ends,
            None => {
                resolved = CauseEnds::of(&run.steps);
                &resolved
            }
        };
        let rows = flatten_steps(&run.steps);
        self.arranged.update(run, &rows, context.graphs);
        // *All* and *Failed* count the run's steps as its summary does:
        // every one, and those of the groups *Failed* keeps (REP-07).
        let groups = not_run::summary(&run.steps, ends);
        let all = groups.iter().map(|(_, count)| count).sum::<usize>();
        let failed = groups
            .iter()
            .filter(|(group, _)| list::counted(*group))
            .map(|(_, count)| count)
            .sum::<usize>();
        if failed == 0 {
            self.failed_only = false;
        }
        let heads: Vec<String> = self
            .arranged
            .lines
            .iter()
            .filter_map(|line| line.under)
            .map(|head| rows[head].key.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut controls = Controls {
            steps: all,
            failed,
            failed_only: self.failed_only,
            foldable: !heads.is_empty(),
            all_folded: !heads.is_empty() && heads.iter().all(|key| self.folded.contains(key)),
            below: false,
            chosen: None,
        };
        header_row(ui, run, ends, context.files_busy, actions, &mut controls);
        if run.diagnostic {
            ui.label(
                RichText::new(
                    "Diagnostic run · may use saved inputs · does not pass the full test",
                )
                .size(12.0)
                .color(theme::WARNING),
            );
        }
        if let Some(notice) = &run.notice {
            egui::Frame::new()
                .fill(theme::ACCENT_SOFT)
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(notice).size(12.0).color(theme::TEXT));
                });
        }
        if let Some(error) = &run.error {
            widgets::error_box(ui, std::slice::from_ref(error));
        }
        ui.add_space(4.0);

        let selected = resolve_selected(
            &rows,
            self.selected_key.as_deref(),
            context.selected_node_id,
        )
        .cloned();
        if self.tab == DetailTab::Request
            && selected
                .as_ref()
                .is_none_or(|row| row.step.request.is_none())
        {
            self.tab = DetailTab::Outputs;
        }

        // The outcomes that colour the marks, once per frame (DIAG-12).
        let outcomes = context.checks.map(|_| crate::inspector::Outcomes::of(run));
        let height = ui.available_height().max(80.0);
        let gap = ui.spacing().item_spacing.x;
        let available = ui.available_width();
        // Between its least width and half the panel, leaving the details
        // theirs (REP-07).
        let widest = (available / 2.0)
            .min(available - DETAILS_LEAST_WIDTH - LIST_EDGE - 2.0 * gap)
            .max(LIST_LEAST_WIDTH);
        let width = self.list_width.clamp(LIST_LEAST_WIDTH, widest);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(width);
                    if controls.below {
                        controls_above_list(ui, &mut controls);
                        ui.add_space(4.0);
                    }
                    self.step_list(ui, &rows, selected.as_ref(), ends, actions);
                },
            );
            self.list_edge(ui, width, widest, height, actions);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    // Keep messages and comparison details accessible even in a short panel.
                    egui::ScrollArea::vertical()
                        .id_salt("report-step-details")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let requirement = selected
                                .as_ref()
                                .and_then(|row| Requirement::of(context.graphs?.node(row)?));
                            self.step_details(
                                ui,
                                &run.id,
                                selected.as_ref(),
                                ends,
                                requirement,
                                !context.document_locked,
                                context.component,
                                context.checks.map(|index| crate::inspector::Marks {
                                    index,
                                    run: outcomes.as_ref(),
                                }),
                                actions,
                            );
                        });
                },
            );
        });
        match controls.chosen {
            Some(Chosen::All) => self.failed_only = false,
            Some(Chosen::Failed) => self.failed_only = true,
            Some(Chosen::Collapse) => self.folded.extend(heads),
            Some(Chosen::Expand) => self.folded.clear(),
            None => {}
        }
    }

    /// The list's edge, dragged between its least width and `widest`; let
    /// go, its width is remembered (REP-07).
    fn list_edge(
        &mut self,
        ui: &mut Ui,
        width: f32,
        widest: f32,
        height: f32,
        actions: &mut Vec<Action>,
    ) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(LIST_EDGE, height), Sense::hover());
        let edge = ui.interact(rect, egui::Id::new("report-list-edge"), Sense::drag());
        let value = f64::from(width.round());
        edge.widget_info(|| {
            let mut info = egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, RESIZE_LIST);
            info.value = Some(value);
            info
        });
        if edge.drag_started() {
            let origin = ui
                .input(|input| input.pointer.press_origin())
                .or(edge.interact_pointer_pos())
                .map_or(rect.center().x, |origin| origin.x);
            self.edge_grab = Some((origin, width, self.list_width));
        }
        if edge.dragged()
            && let (Some((origin, from, before)), Some(pointer)) =
                (self.edge_grab, edge.interact_pointer_pos())
        {
            // Where the edge cannot go, the width is what it was: a window
            // too narrow for it does not narrow it.
            let next = (from + pointer.x - origin).clamp(LIST_LEAST_WIDTH, widest);
            let next = if (next - from).abs() < 0.5 {
                before
            } else {
                next
            };
            if (next - self.list_width).abs() > f32::EPSILON {
                self.list_width = next;
                ui.ctx().request_repaint();
            }
        }
        // Remembered only when the drag changed the width.
        if edge.drag_stopped()
            && let Some((_, _, before)) = self.edge_grab.take()
            && (self.list_width - before).abs() >= 0.5
        {
            actions.push(Action::RememberListWidth(self.list_width));
        }
        let engaged = edge.hovered() || edge.dragged();
        ui.painter().line_segment(
            [rect.center_top(), rect.center_bottom()],
            Stroke::new(
                if engaged { 2.0 } else { 1.0 },
                if engaged {
                    theme::ACCENT
                } else {
                    theme::BORDER
                },
            ),
        );
        let _ = edge
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
            .on_hover_text("Drag to change the width of the step list");
    }

    fn step_list(
        &mut self,
        ui: &mut Ui,
        rows: &[StepRow<'_>],
        selected: Option<&StepRow<'_>>,
        ends: &CauseEnds,
        actions: &mut Vec<Action>,
    ) {
        let arranged = std::mem::take(&mut self.arranged);
        let lines = &arranged.lines;
        let entries = &arranged.entries;
        let bad: Vec<bool> = rows.iter().map(|row| list::bad(row, ends)).collect();
        // A step's checks, and how many of them did not pass.
        let mut checks = vec![(0usize, 0usize); rows.len()];
        for line in lines {
            if let Some(head) = line.under {
                checks[head].0 += 1;
                if bad[line.row] {
                    checks[head].1 += 1;
                }
            }
        }
        // *Failed*: the rows that did not pass, the steps they are listed
        // under and the calls that hold them.
        let kept: Vec<bool> = if self.failed_only {
            let mut kept = bad.clone();
            for line in lines {
                if let Some(head) = line.under
                    && bad[line.row]
                {
                    kept[head] = true;
                }
            }
            let by_key: std::collections::HashMap<&str, usize> = rows
                .iter()
                .enumerate()
                .map(|(index, row)| (row.key.as_str(), index))
                .collect();
            for index in 0..rows.len() {
                if !kept[index] {
                    continue;
                }
                let mut key = rows[index].key.as_str();
                while let Some(at) = key.rfind('/') {
                    key = &key[..at];
                    if let Some(&parent) = by_key.get(key) {
                        kept[parent] = true;
                    }
                }
            }
            kept
        } else {
            vec![true; rows.len()]
        };
        let visible = |folded: &std::collections::HashSet<String>| -> Vec<Line> {
            lines
                .iter()
                .copied()
                .filter(|line| kept[line.row])
                .filter(|line| {
                    line.under
                        .is_none_or(|head| !folded.contains(&rows[head].key))
                })
                .collect()
        };
        let mut shown = visible(&self.folded);

        // A focused row that a fold or *Failed* hides gives its focus to the
        // step it is listed under, or else to the nearest row shown (REP-07).
        if self.focus.is_none()
            && let Some(focused) = ui.memory(|memory| memory.focused())
            && let Some(at) = lines
                .iter()
                .position(|line| list::row_id(&rows[line.row].key) == focused)
            && !shown.contains(&lines[at])
        {
            let head = lines[at]
                .under
                .filter(|head| shown.iter().any(|line| line.row == *head));
            let nearest = || {
                (1..lines.len()).find_map(|distance| {
                    [at.checked_sub(distance), Some(at + distance)]
                        .into_iter()
                        .flatten()
                        .filter_map(|index| lines.get(index))
                        .find(|line| shown.contains(line))
                        .map(|line| line.row)
                })
            };
            if let Some(target) = head.or_else(nearest) {
                self.focus = Some(rows[target].key.clone());
            }
        }

        // The keyboard: up and down move between the rows, left and right
        // fold and open a step's checks (REP-07).
        let focused = ui.memory(|memory| memory.focused());
        if let Some(at) = shown
            .iter()
            .position(|line| Some(list::row_id(&rows[line.row].key)) == focused)
        {
            let id = list::row_id(&rows[shown[at].row].key);
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        tab: false,
                        escape: false,
                    },
                )
            });
            let mut target = None;
            ui.input_mut(|input| {
                let line = shown[at];
                let key = &rows[line.row].key;
                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    target = shown.get(at + 1).map(|line| line.row);
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    target = at.checked_sub(1).map(|before| shown[before].row);
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                    match line.under {
                        None if checks[line.row].0 > 0 => {
                            self.folded.insert(key.clone());
                        }
                        Some(head) => target = Some(head),
                        None => {}
                    }
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                    && line.under.is_none()
                {
                    self.folded.remove(key);
                }
            });
            if let Some(target) = target {
                let row = &rows[target];
                self.selected_key = Some(row.key.clone());
                self.focus = Some(row.key.clone());
                self.asked = Some(row.root_id.clone());
                actions.push(Action::SelectNode(row.root_id.clone()));
            }
            shown = visible(&self.folded);
        }

        egui::ScrollArea::vertical()
            .id_salt("report-steps")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if rows.is_empty() {
                    widgets::hint(ui, "Waiting for the first step…");
                    return;
                }
                let mut cleanup = false;
                for (index, line) in shown.iter().enumerate() {
                    let row = &rows[line.row];
                    if row.step.phase == Phase::Cleanup && row.depth == 0 && !cleanup {
                        cleanup = true;
                        ui.add_space(6.0);
                        widgets::caption(ui, "AFTER TEST · CLEANUP");
                        ui.add_space(2.0);
                    }
                    let shape = match line.under {
                        Some(_) => list::Shape::Check,
                        None if matches!(row.step.kind, NodeKind::Assert | NodeKind::Condition) => {
                            list::Shape::Lone
                        }
                        None => list::Shape::Head {
                            checks: checks[line.row].0,
                            failed: checks[line.row].1,
                            folded: self.folded.contains(&row.key),
                        },
                    };
                    let view = list::RowView {
                        row,
                        entry: &entries[line.row],
                        shape,
                        head: line.under.map(|head| &rows[head]),
                        active: selected.is_some_and(|active| active.key == row.key),
                        words: row.words(ends),
                        ran: row.step.cause.is_none(),
                        first: index == 0,
                    };
                    let drawn = list::draw(ui, &view);
                    if self.focus.as_deref() == Some(row.key.as_str()) {
                        self.focus = None;
                        drawn.row.request_focus();
                        drawn.row.scroll_to_me(None);
                    }
                    if let Some(fold) = drawn.fold
                        && fold.clicked()
                    {
                        if !self.folded.remove(&row.key) {
                            self.folded.insert(row.key.clone());
                        }
                    } else if drawn.row.clicked() {
                        drawn.row.request_focus();
                        self.selected_key = Some(row.key.clone());
                        self.asked = Some(row.root_id.clone());
                        actions.push(Action::SelectNode(row.root_id.clone()));
                    }
                }
            });
        self.arranged = arranged;
    }

    #[allow(clippy::too_many_arguments)]
    fn step_details(
        &mut self,
        ui: &mut Ui,
        run_id: &str,
        selected: Option<&StepRow<'_>>,
        ends: &CauseEnds,
        requirement: Option<Requirement>,
        editable: bool,
        component: bool,
        marks: Option<crate::inspector::Marks<'_>>,
        actions: &mut Vec<Action>,
    ) {
        let Some(selected) = selected else {
            widgets::hint(
                ui,
                if component {
                    "Results will appear while the component runs."
                } else {
                    "Results will appear while the scenario runs."
                },
            );
            return;
        };
        let step = selected.step;
        // A check or variable on a component's export would edit the
        // component's own graph (KTLO-23 criterion 11); exports appear at the
        // top level only when a component runs on its own.
        let checkable = selected.depth == 0 && step.kind != NodeKind::Export;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(step.display_name())
                        .size(14.0)
                        .family(theme::bold_family()),
                );
                if selected.depth > 0 {
                    widgets::hint(ui, format!("Step inside component · {}", step.node_id));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::pill(ui, selected.word(ends), status_color(step.status));
            });
        });

        // A step that did not run says why from its cause, never from the
        // engine's message (LAY-19).
        let message = match selected.not_run(ends) {
            Some(not_run) => Some(Cow::Owned(not_run.sentence())),
            None => step.message.as_deref().map(Cow::Borrowed),
        };
        if let Some(message) = message {
            let color = status_color(step.status);
            egui::Frame::new()
                .fill(color.gamma_multiply(0.08))
                .stroke(Stroke::new(1.0, color.gamma_multiply(0.25)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(message.as_ref())
                            .size(12.0)
                            .color(theme::TEXT),
                    );
                });
        }

        // A step whose node is not found (its component's definition is
        // gone) reads as before DIAG-03.
        if !step.expected.is_missing()
            || !step.actual.is_missing()
            || requirement.is_some_and(Requirement::always_compared)
        {
            let expected = match requirement {
                Some(requirement) => requirement.text(&step.expected),
                None => expected_text(Predicate::Equals, &step.expected, 0.0),
            };
            comparison_box(ui, &expected, &step.actual);
        }

        let outputs = format!("Outputs  {}", step.outputs.len());
        let inputs = format!("Inputs  {}", step.inputs.len());
        let hint = checkable && self.tab == DetailTab::Outputs;
        // The controls above the value share the tabs' line where it has
        // room for them, and take a line of their own under it otherwise.
        let gap = ui.spacing().item_spacing.x;
        let mut tabs =
            widgets::tab_value_width(ui, &outputs) + widgets::tab_value_width(ui, &inputs) + gap;
        if step.request.is_some() {
            tabs += widgets::tab_value_width(ui, "HTTP request") + gap;
        }
        let hint_width = if hint {
            text_width(ui, CHOOSE_A_FIELD, theme::FONT_LABEL) + gap
        } else {
            0.0
        };
        let one_line = ui.available_width() >= tabs + hint_width + bar_width(ui) + 16.0;
        let mut copy = false;
        ui.horizontal(|ui| {
            widgets::tab_value(ui, &mut self.tab, DetailTab::Outputs, &outputs);
            widgets::tab_value(ui, &mut self.tab, DetailTab::Inputs, &inputs);
            if step.request.is_some() {
                widgets::tab_value(ui, &mut self.tab, DetailTab::Request, "HTTP request");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if one_line {
                    copy = self.value_bar(ui, true);
                    ui.add_space(8.0);
                }
                if hint {
                    widgets::hint(ui, CHOOSE_A_FIELD);
                }
            });
        });
        if !one_line {
            // Details too narrow for one line of them (REP-07a) have two.
            let whole = ui.available_width() >= bar_width(ui);
            let line = egui::vec2(ui.available_width(), theme::CONTROL_HEIGHT);
            ui.allocate_ui_with_layout(
                line,
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| copy = self.value_bar(ui, whole),
            );
            if !whole {
                ui.allocate_ui_with_layout(
                    line,
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| self.view_switch(ui),
                );
            }
        }
        ui.separator();

        let json_id = egui::Id::new(("report-json", run_id, &selected.key, self.tab as u8));
        let json = (copy || self.json).then(|| self.json_text(json_id, step));
        if copy && let Some(text) = &json {
            ui.ctx().copy_text(text.as_str().to_owned());
        }
        let json = json.filter(|_| self.json);
        // What is folded or opened by hand is kept per step, tab and choice
        // of Expand all or Collapse all.
        let document = egui::Id::new((
            "report-tree",
            run_id,
            selected.key.as_str(),
            self.tab as u8,
            self.epoch,
        ));
        self.trees.insert(document);
        let folding = self.folding;
        let checked = &self.checked;
        let openings = &mut self.openings;
        let tab = self.tab;
        // How a port's value opens, counted once per run, step and tab.
        let mut opens = |port: &str, value: &Value| match folding {
            Folding::Default => *openings
                .entry(egui::Id::new((
                    "report-opening",
                    run_id,
                    selected.key.as_str(),
                    tab as u8,
                    port,
                )))
                .or_insert_with(|| by_default(value)),
            folding => (opening(folding, value), false),
        };

        ui.push_id(
            ("report-detail", run_id, &selected.key, tab as u8),
            |ui| match tab {
                DetailTab::Request => {
                    if let Some(request) = &step.request {
                        ui.horizontal(|ui| {
                            widgets::pill(ui, &request.method, theme::ACCENT);
                            ui.label(RichText::new(&request.url).monospace().size(12.0));
                        });
                        ui.add_space(4.0);
                        if let Some(text) = &json {
                            json_text(ui, text);
                            return;
                        }
                        let headers = request_headers(request);
                        let none = |_: &str| false;
                        let mut trees = vec![("headers", &headers)];
                        if let OptionalJson::Present(body) = &request.body {
                            trees.push(("body", body));
                        }
                        for (port, value) in trees {
                            let (opening, large) = opens(port, value);
                            large_line(ui, opening, large, port);
                            crate::inspector::results_tree(
                                ui,
                                document,
                                &step.node_id,
                                port,
                                value,
                                crate::inspector::Results {
                                    opening,
                                    fields: false,
                                    offers: false,
                                    checked: &none,
                                    marks: None,
                                },
                            );
                        }
                    }
                }
                DetailTab::Outputs | DetailTab::Inputs => {
                    let map = if tab == DetailTab::Inputs {
                        &step.inputs
                    } else {
                        &step.outputs
                    };
                    if map.is_empty() {
                        widgets::hint(
                            ui,
                            if tab == DetailTab::Inputs {
                                "No inputs for this step."
                            } else {
                                "No outputs for this step."
                            },
                        );
                        return;
                    }
                    if let Some(text) = &json {
                        json_text(ui, text);
                        return;
                    }
                    // The rows of a checkable step's outputs name their field,
                    // so a checked one reads *Check added* (UX-01) even while
                    // the document cannot be edited; only an editable one
                    // offers *Check* and *To variable*.
                    let fields = tab == DetailTab::Outputs && checkable;
                    for (port, value) in map {
                        let is_checked = |path: &str| {
                            checked.iter().any(|checked| {
                                checked.node_id == step.node_id
                                    && checked.port == *port
                                    && checked.path == path
                            })
                        };
                        let (opening, large) = opens(port, value);
                        large_line(ui, opening, large, port);
                        let chosen = crate::inspector::results_tree(
                            ui,
                            document,
                            &step.node_id,
                            port,
                            value,
                            crate::inspector::Results {
                                opening,
                                fields,
                                offers: fields && editable,
                                checked: &is_checked,
                                marks,
                            },
                        );
                        actions.extend(chosen.map(|chosen| match chosen {
                            crate::inspector::TreeAction::Check { path, check } => {
                                Action::AssertField {
                                    node_id: step.node_id.clone(),
                                    port: port.clone(),
                                    path,
                                    check,
                                }
                            }
                            crate::inspector::TreeAction::Assign { path } => Action::AssignField {
                                node_id: step.node_id.clone(),
                                port: port.clone(),
                                path,
                            },
                            // A check of the field, chosen in the menu of
                            // *Check*, is selected (DIAG-12).
                            crate::inspector::TreeAction::Select { check } => {
                                Action::SelectNode(check)
                            }
                        }));
                    }
                }
            },
        );
    }

    /// *Tree | JSON* when `switch`, *Expand all*, *Collapse all* and *Copy*,
    /// right to left (REP-08). Returns whether *Copy* was clicked.
    fn value_bar(&mut self, ui: &mut Ui, switch: bool) -> bool {
        let copy = ui
            .add(
                widgets::action(Icon::Copy, "Copy")
                    .compact()
                    .quiet()
                    .named(COPY_JSON)
                    .hover("Copy the value as JSON"),
            )
            .clicked();
        ui.add_enabled_ui(!self.json, |ui| {
            if ui
                .add(
                    widgets::action(Icon::ChevronRight, "Collapse all")
                        .compact()
                        .quiet(),
                )
                .clicked()
            {
                self.folding = Folding::CollapseAll;
                self.forget_trees(ui.ctx());
            }
            if ui
                .add(
                    widgets::action(Icon::ChevronDown, "Expand all")
                        .compact()
                        .quiet(),
                )
                .clicked()
            {
                self.folding = Folding::ExpandAll;
                self.forget_trees(ui.ctx());
            }
        });
        if switch {
            ui.add_space(8.0);
            self.view_switch(ui);
        }
        copy
    }

    /// *Tree | JSON*, right to left (REP-08).
    fn view_switch(&mut self, ui: &mut Ui) {
        if ui
            .add(
                widgets::action(Icon::Code, "JSON")
                    .compact()
                    .toggle(self.json)
                    .named(SHOW_JSON),
            )
            .clicked()
        {
            self.json = true;
        }
        if ui
            .add(
                widgets::action(Icon::Layers, "Tree")
                    .compact()
                    .toggle(!self.json)
                    .named(SHOW_TREE),
            )
            .clicked()
        {
            self.json = false;
        }
    }

    /// The JSON text of the value the tab shows, written once per value.
    fn json_text(&mut self, id: egui::Id, step: &StepResult) -> std::sync::Arc<String> {
        if let Some((kept, text)) = &self.json_text
            && *kept == id
        {
            return text.clone();
        }
        let value = match self.tab {
            DetailTab::Outputs => serde_json::to_value(&step.outputs),
            DetailTab::Inputs => serde_json::to_value(&step.inputs),
            DetailTab::Request => Ok(step.request.as_ref().map_or(Value::Null, |request| {
                let mut sent = Map::new();
                sent.insert("headers".into(), request_headers(request));
                if let OptionalJson::Present(body) = &request.body {
                    sent.insert("body".into(), body.clone());
                }
                Value::Object(sent)
            })),
        }
        .unwrap_or(Value::Null);
        let text = std::sync::Arc::new(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| json_stringify(&value)),
        );
        self.json_text = Some((id, text.clone()));
        text
    }
}

/// A request's headers as the object the tree and the JSON show.
fn request_headers(request: &apiforge_core::run::RequestSnapshot) -> Value {
    Value::Object(
        request
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect::<Map<String, Value>>(),
    )
}

/// The width `text` takes at `size`.
fn text_width(ui: &Ui, text: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(size),
            theme::TEXT,
        )
        .size()
        .x
}

/// The width of [`ReportPanel::value_bar`].
fn bar_width(ui: &Ui) -> f32 {
    let gap = ui.spacing().item_spacing.x;
    ["Copy", "Collapse all", "Expand all", "JSON", "Tree"]
        .into_iter()
        .map(|label| 20.0 + 14.0 + 6.0 + text_width(ui, label, theme::FONT_LABEL) + gap)
        .sum::<f32>()
        + 8.0
        + gap
}

/// The line over a value opened to its second or first level only
/// (REP-08): `large`, it has more than [`OPEN_NODES`] nodes; otherwise more
/// open rows than are drawn at once.
fn large_line(ui: &mut Ui, opening: Opening, large: bool, port: &str) {
    let Opening::Levels(levels) = opening else {
        return;
    };
    let level = if levels >= 2 { "second" } else { "first" };
    let why = if large {
        format!("has more than {OPEN_NODES} values")
    } else {
        "has more values than are shown at once".to_owned()
    };
    widgets::hint(ui, format!("{port} {why} · opened to its {level} level"));
}

/// A value as indented JSON text that can be selected (REP-08), at most
/// [`JSON_SHOWN_CHARS`] of it.
fn json_text(ui: &mut Ui, text: &str) {
    let shown = match text.char_indices().nth(JSON_SHOWN_CHARS) {
        Some((end, _)) => &text[..end],
        None => text,
    };
    egui::Frame::new()
        .fill(theme::FIELD)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(
                egui::Label::new(
                    RichText::new(shown)
                        .monospace()
                        .size(theme::FONT_LABEL)
                        .color(theme::TEXT),
                )
                .selectable(true)
                .wrap(),
            );
        });
    if shown.len() < text.len() {
        widgets::hint(
            ui,
            format!("Showing the first {JSON_SHOWN_CHARS} characters · Copy takes all of them"),
        );
    }
}

/// The bar's text while only checks were added since the run (UX-01).
fn checks_added(count: usize) -> String {
    if count == 1 {
        "1 check added · it runs on the next run".into()
    } else {
        format!("{count} checks added · they run on the next run")
    }
}

fn empty_state(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(16.0);
        ui.label(
            RichText::new("Your scenario is ready for its first run")
                .size(13.5)
                .family(theme::bold_family()),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(
                "Run a simulation or the live API. Each step's data and comparison results will appear here.",
            )
            .size(12.0)
            .color(theme::MUTED),
        );
    });
}

/// The badge that says how a run ran. A simulation shares its violet with
/// *Run simulation* (UX-02).
pub(crate) fn mode_pill(mode: RunMode) -> (&'static str, Color32) {
    match mode {
        RunMode::Simulation => ("SIMULATION", theme::SIMULATION),
        RunMode::Live => ("LIVE API", theme::ACCENT),
    }
}

/// The list's controls above the run: *All | Failed* and *Collapse
/// checks* (REP-07), and what was chosen in them.
struct Controls {
    /// The run's steps, as its summary counts them.
    steps: usize,
    /// The steps of the summary's failed, blocked and not met groups.
    failed: usize,
    failed_only: bool,
    /// Some step has checks under it.
    foldable: bool,
    all_folded: bool,
    /// The run's line has no room for them: they stand above the list.
    below: bool,
    chosen: Option<Chosen>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chosen {
    All,
    Failed,
    Collapse,
    Expand,
}

impl Controls {
    fn all_text(&self) -> String {
        format!("All {}", self.steps)
    }

    fn failed_text(&self) -> String {
        format!("Failed {}", self.failed)
    }

    fn fold_text(&self) -> &'static str {
        if self.all_folded {
            "Expand checks"
        } else {
            "Collapse checks"
        }
    }

    /// The width they take on a line.
    fn width(&self, ui: &Ui) -> f32 {
        let gap = ui.spacing().item_spacing.x;
        list::segment_width(ui, &self.all_text())
            + list::segment_width(ui, &self.failed_text())
            + 8.0
            + 20.0
            + 14.0
            + 6.0
            + text_width(ui, self.fold_text(), theme::FONT_LABEL)
            + 3.0 * gap
    }
}

/// *All | Failed* and *Collapse checks*, left to right.
fn list_controls(ui: &mut Ui, controls: &mut Controls) {
    switch(ui, controls);
    ui.add_space(8.0);
    fold_control(ui, controls);
}

/// The controls above a list when the run's line has no room for them:
/// on one line, or *Collapse checks* on a line of its own when the list is
/// narrower than they are, so that none is cut (REP-07).
fn controls_above_list(ui: &mut Ui, controls: &mut Controls) {
    if controls.width(ui) <= ui.available_width() {
        ui.horizontal(|ui| list_controls(ui, controls));
    } else {
        ui.horizontal(|ui| switch(ui, controls));
        ui.add_space(2.0);
        ui.horizontal(|ui| fold_control(ui, controls));
    }
}

/// *All | Failed*: two buttons side by side.
fn switch(ui: &mut Ui, controls: &mut Controls) {
    ui.spacing_mut().item_spacing.x = 0.0;
    if list::segment(ui, &controls.all_text(), !controls.failed_only, true).clicked() {
        controls.chosen = Some(Chosen::All);
    }
    let enabled = controls.failed > 0;
    let failed = ui.add_enabled_ui(enabled, |ui| {
        list::segment(ui, &controls.failed_text(), controls.failed_only, enabled)
    });
    if failed.inner.clicked() {
        controls.chosen = Some(Chosen::Failed);
    }
}

/// *Collapse checks*, or *Expand checks* when every step's are folded.
fn fold_control(ui: &mut Ui, controls: &mut Controls) {
    let (icon, chosen) = if controls.all_folded {
        (Icon::ChevronDown, Chosen::Expand)
    } else {
        (Icon::ChevronRight, Chosen::Collapse)
    };
    let fold = ui.add_enabled_ui(controls.foldable, |ui| {
        ui.add(
            widgets::action(icon, controls.fold_text())
                .compact()
                .quiet()
                .hover("Fold or show the checks under every step"),
        )
    });
    if fold.inner.clicked() {
        controls.chosen = Some(chosen);
    }
}

fn header_row(
    ui: &mut Ui,
    run: &RunResult,
    ends: &CauseEnds,
    files_busy: bool,
    actions: &mut Vec<Action>,
    controls: &mut Controls,
) {
    ui.horizontal(|ui| {
        let (text, colour) = mode_pill(run.mode);
        widgets::pill(ui, text, colour);
        let started = short_time(&run.started_at);
        let timing = total_duration_ms(run)
            .map(|ms| format!("{started} · {ms} ms"))
            .unwrap_or_else(|| started.to_owned());
        ui.label(RichText::new(timing).size(12.0).color(theme::MUTED));
        if run.mode == RunMode::Simulation {
            ui.label(
                RichText::new("Sample responses")
                    .size(12.0)
                    .color(theme::MUTED),
            )
            .on_hover_text("No HTTP requests are sent in simulation mode.");
        }
        let groups = not_run::summary(&run.steps, ends);
        let gap = ui.spacing().item_spacing.x;
        // What stays right of the run's own words: the summary and Export.
        let summary = groups
            .iter()
            .map(|(group, count)| text_width(ui, &format!("{count} {}", group.label()), 12.0) + gap)
            .sum::<f32>()
            + 20.0
            + 14.0
            + 6.0
            + text_width(ui, "Export", theme::FONT_LABEL)
            + gap;
        controls.below = ui.available_width() < summary + controls.width(ui) + 16.0;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    !files_busy,
                    widgets::action(Icon::Export, "Export").compact().quiet(),
                )
                .on_hover_text("Export JSON report with masked secrets")
                .clicked()
            {
                actions.push(Action::ExportReport);
            }
            // Every step in one group, the steps that did not run by their
            // cause (LAY-19); right to left, so the last group first.
            for (group, count) in groups.into_iter().rev() {
                group_label(ui, group, count);
            }
            if !controls.below {
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add_space((ui.available_width() - controls.width(ui)).max(0.0));
                    list_controls(ui, controls);
                });
            }
        });
    });
}

/// One group of the summary, `11 stopped`.
fn group_label(ui: &mut Ui, group: not_run::Group, count: usize) {
    use not_run::Group;
    let color = match group {
        Group::Passed => theme::SUCCESS,
        Group::Failed => theme::DANGER,
        Group::NotMet | Group::Blocked | Group::Cancelled => theme::WARNING,
        Group::Stopped | Group::Skipped => theme::MUTED,
    };
    ui.label(
        RichText::new(format!("{count} {}", group.label()))
            .size(12.0)
            .strong()
            .color(color),
    );
}

fn comparison_box(ui: &mut Ui, expected: &str, actual: &OptionalJson) {
    egui::Frame::new()
        .fill(theme::FIELD)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let col_width = ((ui.available_width() - 28.0) * 0.5).max(80.0);
                ui.allocate_ui(egui::vec2(col_width, 36.0), |ui| {
                    ui.vertical(|ui| {
                        widgets::subcaption(ui, "EXPECTED");
                        ui.add(
                            egui::Label::new(
                                RichText::new(labels::truncate(expected, 120)).monospace(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(expected);
                    });
                });
                ui.label(RichText::new("→").color(theme::MUTED));
                ui.allocate_ui(egui::vec2(col_width, 36.0), |ui| {
                    ui.vertical(|ui| {
                        widgets::subcaption(ui, "ACTUAL");
                        let text = match actual {
                            OptionalJson::Present(value) => compact_json(value),
                            OptionalJson::Missing => "(missing value)".into(),
                        };
                        ui.add(
                            egui::Label::new(
                                RichText::new(labels::truncate(&text, 120)).monospace(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(text);
                    });
                });
            });
        });
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| json_stringify(value))
}

fn run_pill(status: RunStatus) -> (&'static str, Color32) {
    match status {
        RunStatus::Passed => ("PASSED", theme::SUCCESS),
        RunStatus::Running => ("RUNNING", theme::ACCENT),
        RunStatus::Cancelled => ("CANCELLED", theme::WARNING),
        RunStatus::Failed => ("FAILED", theme::DANGER),
    }
}

pub(crate) fn status_color(status: StepStatus) -> Color32 {
    match status {
        StepStatus::Passed => theme::SUCCESS,
        StepStatus::Failed | StepStatus::Error => theme::DANGER,
        StepStatus::Cancelled => theme::WARNING,
        StepStatus::Blocked | StepStatus::Skipped => theme::MUTED,
    }
}

fn status_mark(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Passed => "✓",
        StepStatus::Failed | StepStatus::Error => "✕",
        StepStatus::Blocked | StepStatus::Skipped | StepStatus::Cancelled => "−",
    }
}

fn error_policy_label(policy: ErrorPolicy) -> &'static str {
    match policy {
        ErrorPolicy::Continue => "Continue remaining steps",
        ErrorPolicy::Stop => "Stop on error",
    }
}

/// Extracts `HH:MM:SS` from an RFC 3339 timestamp when present.
fn short_time(rfc3339: &str) -> &str {
    rfc3339
        .split_once('T')
        .and_then(|(_, rest)| rest.get(..8))
        .unwrap_or(rfc3339)
}

/// Computes the elapsed milliseconds when the run has finished.
fn total_duration_ms(run: &RunResult) -> Option<u64> {
    let finished = run.finished_at.as_deref()?;
    if let (Some(start), Some(end)) = (day_millis(&run.started_at), day_millis(finished))
        && end >= start
    {
        return Some(end - start);
    }
    Some(run.steps.iter().map(|step| step.duration_ms).sum())
}

fn day_millis(rfc3339: &str) -> Option<u64> {
    let (_, time) = rfc3339.split_once('T')?;
    let hours: u64 = time.get(0..2)?.parse().ok()?;
    let minutes: u64 = time.get(3..5)?.parse().ok()?;
    let seconds: u64 = time.get(6..8)?.parse().ok()?;
    let millis: u64 = if time.as_bytes().get(8) == Some(&b'.') {
        let digits: String = time[9..]
            .chars()
            .take_while(char::is_ascii_digit)
            .take(3)
            .collect();
        match digits.len() {
            1 => digits.parse::<u64>().ok()? * 100,
            2 => digits.parse::<u64>().ok()? * 10,
            3 => digits.parse::<u64>().ok()?,
            _ => 0,
        }
    } else {
        0
    };
    Some(((hours * 60 + minutes) * 60 + seconds) * 1000 + millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{model::NodeKind, run::RequestSnapshot};
    use indexmap::IndexMap;
    use serde_json::json;

    /// A passed step of a node with the given identifier, label and kind.
    fn step(id: &str, label: &str, kind: NodeKind, phase: Phase) -> StepResult {
        let mut node = apiforge_core::graph::create_node(
            kind,
            apiforge_core::format::Position { x: 0.0, y: 0.0 },
            None,
        );
        node.id = id.to_owned();
        node.handle = apiforge_core::handle::handle_base(label);
        node.label = Some(label.to_owned());
        StepResult::new(&node, phase)
    }

    /// DIAG-03: a step inside a component call finds its node in the
    /// revision the call is pinned to; a call whose revision is gone finds
    /// nothing, and its card reads as before.
    #[test]
    fn a_step_inside_a_call_finds_its_node_in_the_called_revision() {
        use apiforge_core::model::Predicate;
        let mut project = apiforge_core::demos::shop_project();
        let scenario = project.scenarios[0].clone();
        let call = scenario
            .graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Component)
            .expect("the demo shop calls a component")
            .clone();
        let definition = project
            .components
            .iter_mut()
            .find(|component| {
                Some(component.id.as_str()) == call.config.component_id.as_deref()
                    && Some(component.revision) == call.config.revision
            })
            .expect("the call's revision is in the project");
        let mut check = apiforge_core::graph::create_node(
            NodeKind::Assert,
            apiforge_core::format::Position { x: 0.0, y: 0.0 },
            None,
        );
        check.id = "inner-check".into();
        check.config.predicate = Some(Predicate::NotEquals);
        definition.graph.nodes.push(check.clone());

        let mut outer = StepResult::new(&call, Phase::Main);
        outer.children = Some(vec![StepResult::new(&check, Phase::Main)]);
        let steps = vec![outer];
        let rows = flatten_steps(&steps);
        let inner = rows.iter().find(|row| row.depth == 1).unwrap();
        assert_eq!(inner.calls, vec![call.id.as_str()]);
        assert_eq!(inner.root_id, call.id);

        let graphs = RunGraphs {
            main: &scenario.graph,
            cleanup: Some(&scenario.cleanup),
            project: &project,
        };
        let node = graphs.node(inner).expect("the check is found");
        assert_eq!(
            Requirement::of(node),
            Some(Requirement::One {
                predicate: Predicate::NotEquals,
                tolerance: 0.0
            })
        );

        let mut gone = scenario.clone();
        for node in &mut gone.graph.nodes {
            if node.id == call.id {
                node.config.revision = Some(999);
            }
        }
        let graphs = RunGraphs {
            main: &gone.graph,
            cleanup: Some(&gone.cleanup),
            project: &project,
        };
        assert!(graphs.node(inner).is_none(), "the revision is gone");
    }

    /// Review of REP-07a: the list is arranged again when the graphs it
    /// reads change in place, as the single-project window edits its
    /// scenario: a check's row follows its predicate with no new run.
    #[test]
    fn the_list_reads_a_check_again_after_an_edit_in_place() {
        use apiforge_core::model::Predicate;
        let project = apiforge_core::demos::shop_project();
        let mut scenario = project.scenarios[0].clone();
        let at = scenario
            .graph
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Assert)
            .expect("the demo shop has a check");
        let check = scenario.graph.nodes[at].id.clone();
        let mut run = sample_run();
        run.steps = scenario
            .graph
            .nodes
            .iter()
            .map(|node| StepResult::new(node, Phase::Main))
            .collect();
        let rows = flatten_steps(&run.steps);
        let index = rows
            .iter()
            .position(|row| row.step.node_id == check)
            .unwrap();
        let mut arranged = list::Arranged::default();
        let mut text = |scenario: &apiforge_core::format::Scenario| {
            arranged.update(
                &run,
                &rows,
                Some(RunGraphs {
                    main: &scenario.graph,
                    cleanup: Some(&scenario.cleanup),
                    project: &project,
                }),
            );
            arranged.entries[index].predicate.clone().unwrap()
        };
        let before = text(&scenario);
        let edited = &mut scenario.graph.nodes[at].config.predicate;
        *edited = Some(if *edited == Some(Predicate::NotEquals) {
            Predicate::Equals
        } else {
            Predicate::NotEquals
        });
        let after = text(&scenario);
        assert_ne!(before, after, "the row reads the edited predicate");
    }

    /// Review of REP-07a: above a list narrower than they are, *All*,
    /// *Failed* and *Collapse checks* wrap and stay within its width, each
    /// as wide as on a line with room for them.
    #[test]
    fn the_controls_above_a_narrow_list_stay_within_it() {
        use egui_kittest::kittest::Queryable;
        const LABELS: [&str; 3] = ["All 128", "Failed 37", "Collapse checks"];
        let drawn = |width: f32| {
            let area = std::rc::Rc::new(std::cell::Cell::new(egui::Rect::NOTHING));
            let kept = area.clone();
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(720.0, 240.0))
                .build_ui(move |ui| {
                    crate::theme::apply(ui.ctx());
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, 160.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(width);
                            kept.set(ui.max_rect());
                            let mut controls = Controls {
                                steps: 128,
                                failed: 37,
                                failed_only: false,
                                foldable: true,
                                all_folded: false,
                                below: true,
                                chosen: None,
                            };
                            controls_above_list(ui, &mut controls);
                        },
                    );
                });
            harness.run();
            crate::testing::assert_disjoint(&harness, &LABELS);
            let rects: Vec<egui::Rect> = LABELS
                .iter()
                .map(|label| harness.get_by_label(label).rect())
                .collect();
            (area.get(), rects)
        };
        let (_, roomy) = drawn(640.0);
        let (area, narrow) = drawn(LIST_LEAST_WIDTH);
        for ((label, rect), room) in LABELS.iter().zip(&narrow).zip(&roomy) {
            assert!(
                rect.left() >= area.left() - 0.5 && rect.right() <= area.right() + 0.5,
                "{label} at {rect:?} stays within the list {area:?}"
            );
            assert!(
                (rect.width() - room.width()).abs() < 0.5,
                "{label} keeps its width: {} vs {}",
                rect.width(),
                room.width()
            );
        }
    }

    fn sample_run() -> RunResult {
        let mut child = step("inner-req", "Inner request", NodeKind::Request, Phase::Main);
        child.outputs.insert("status".into(), json!(200));

        let mut comp = step("comp-1", "Auth component", NodeKind::Component, Phase::Main);
        comp.duration_ms = 12;
        comp.outputs.insert(
            "token".into(),
            json!({"value": "abc", "__proto__": "ignored"}),
        );
        comp.children = Some(vec![child]);

        let mut check = step("assert-1", "Check status", NodeKind::Assert, Phase::Main);
        check.status = StepStatus::Failed;
        check.message = Some("Expected 200, got 500".into());
        check.expected = OptionalJson::Present(json!(200));
        check.actual = OptionalJson::Present(json!(500));
        check.request = Some(RequestSnapshot {
            method: "POST".into(),
            url: "http://localhost:3333/orders".into(),
            headers: IndexMap::from([("content-type".into(), "application/json".into())]),
            body: OptionalJson::Present(json!({"sku": "A1"})),
        });

        let cleanup = step("clean-1", "Delete order", NodeKind::Request, Phase::Cleanup);

        RunResult {
            id: "run-1".into(),
            scenario_id: "scen-1".into(),
            mode: RunMode::Simulation,
            started_at: "2026-04-19T12:00:00.100Z".into(),
            finished_at: Some("2026-04-19T12:00:00.245Z".into()),
            status: RunStatus::Failed,
            steps: vec![comp, check, cleanup],
            diagnostic: true,
            snapshot: None,
            error: Some("Step failed".into()),
            notice: Some("Using saved inputs from the previous run.".into()),
            running: None,
        }
    }

    #[test]
    fn selecting_a_child_keeps_its_details_when_the_board_selects_its_parent() {
        let run = sample_run();
        let mut panel = ReportPanel::default();
        panel.sync_selection(Some(&run), None);
        let child_key = flatten_steps(&run.steps)[1].key.clone();
        panel.selected_key = Some(child_key.clone());
        panel.sync_selection(Some(&run), Some("comp-1"));
        assert_eq!(panel.selected_key.as_deref(), Some(child_key.as_str()));
        panel.sync_selection(Some(&run), Some("assert-1"));
        let rows = flatten_steps(&run.steps);
        assert_eq!(
            resolve_selected(&rows, panel.selected_key.as_deref(), None)
                .unwrap()
                .step
                .node_id,
            "assert-1"
        );
    }

    #[test]
    fn collapsing_the_report_preserves_its_expanded_height() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut panel = ReportPanel {
            open: true,
            height: 330.0,
            ..Default::default()
        };
        let context = Context {
            run: None,
            ends: None,
            selected_node_id: None,
            error_policy: ErrorPolicy::Continue,
            running: false,
            stale: false,
            checked: &[],
            checks: None,
            files_busy: false,
            component: false,
            document_locked: false,
            graphs: None,
        };
        let draw = |panel: &mut ReportPanel| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                panel.show(ui, &context);
            });
            output.textures_delta.clear();
        };
        draw(&mut panel);
        let expanded = panel.height;
        assert!((expanded - 330.0).abs() < 2.0);
        panel.open = false;
        draw(&mut panel);
        assert_eq!(panel.height, expanded);
        panel.open = true;
        draw(&mut panel);
        assert!((panel.height - expanded).abs() < 2.0);
    }

    #[test]
    fn keyboard_height_adjusts_and_clamps_within_bounds() {
        assert_eq!(
            keyboard_height(320.0, egui::Key::ArrowUp, 160.0, 560.0),
            Some(352.0)
        );
        assert_eq!(
            keyboard_height(320.0, egui::Key::ArrowDown, 160.0, 560.0),
            Some(288.0)
        );
        assert_eq!(
            keyboard_height(550.0, egui::Key::ArrowUp, 160.0, 560.0),
            Some(560.0)
        );
        assert_eq!(
            keyboard_height(170.0, egui::Key::ArrowDown, 160.0, 560.0),
            Some(160.0)
        );
        assert_eq!(
            keyboard_height(320.0, egui::Key::Home, 160.0, 560.0),
            Some(160.0)
        );
        assert_eq!(
            keyboard_height(320.0, egui::Key::End, 160.0, 560.0),
            Some(560.0)
        );
        assert_eq!(keyboard_height(320.0, egui::Key::Enter, 160.0, 560.0), None);
    }

    #[test]
    fn flatten_steps_preserves_component_children_and_root_id() {
        let run = sample_run();
        let rows = flatten_steps(&run.steps);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].step.node_id, "comp-1");
        assert_eq!(rows[0].root_id, "comp-1");
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].step.node_id, "inner-req");
        assert_eq!(rows[1].root_id, "comp-1");
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[2].step.node_id, "assert-1");
        assert_eq!(rows[3].step.phase, Phase::Cleanup);
    }

    #[test]
    fn resolve_selected_prefers_explicit_then_board_then_failed() {
        let run = sample_run();
        let rows = flatten_steps(&run.steps);
        let default = resolve_selected(&rows, None, None).unwrap();
        assert_eq!(default.step.node_id, "assert-1");

        let by_node = resolve_selected(&rows, None, Some("clean-1")).unwrap();
        assert_eq!(by_node.step.node_id, "clean-1");

        let by_key = resolve_selected(&rows, Some(&rows[1].key), Some("clean-1")).unwrap();
        assert_eq!(by_key.step.node_id, "inner-req");
    }

    #[test]
    fn duration_and_short_time_parse_rfc3339_timestamps() {
        let run = sample_run();
        assert_eq!(short_time(&run.started_at), "12:00:00");
        assert_eq!(total_duration_ms(&run), Some(145));
    }

    #[test]
    fn report_panel_renders_empty_and_populated_states_headlessly() {
        let mut panel = ReportPanel {
            open: true,
            ..Default::default()
        };
        let run = sample_run();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let empty = Context {
                    run: None,
                    ends: None,
                    selected_node_id: None,
                    error_policy: ErrorPolicy::Continue,
                    running: false,
                    stale: false,
                    checked: &[],
                    checks: None,
                    files_busy: false,
                    component: false,
                    document_locked: false,
                    graphs: None,
                };
                assert!(panel.show(ui, &empty).is_empty());
            });
            out.textures_delta.clear();
        }
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let populated = Context {
                    run: Some(&run),
                    ends: None,
                    selected_node_id: Some("comp-1"),
                    error_policy: ErrorPolicy::Stop,
                    running: false,
                    stale: true,
                    checked: &[],
                    checks: None,
                    files_busy: false,
                    component: false,
                    document_locked: false,
                    graphs: None,
                };
                let _ = panel.show(ui, &populated);
            });
            out.textures_delta.clear();
        }
    }
    /// UX-01: a checked row reads *Check added* while the document cannot be
    /// edited (another process holds the project, say), and no row offers
    /// *Check* or *To variable* then.
    #[test]
    fn a_checked_row_reads_check_added_while_the_document_is_locked() {
        fn painted(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
            match shape {
                egui::epaint::Shape::Vec(shapes) => {
                    shapes.iter().for_each(|shape| painted(shape, out))
                }
                egui::epaint::Shape::Text(text) => out.push(text.galley.job.text.clone()),
                _ => {}
            }
        }
        let run = sample_run();
        let checked = [CheckedField {
            node_id: "comp-1".into(),
            port: "token".into(),
            path: "value".into(),
        }];
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        // Maximised, so the rows below the step's header are in view.
        let mut panel = ReportPanel {
            open: true,
            maximised: true,
            ..Default::default()
        };
        for locked in [false, true] {
            let mut texts = Vec::new();
            for _ in 0..2 {
                texts.clear();
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1000.0),
                    )),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    let context = Context {
                        run: Some(&run),
                        ends: None,
                        selected_node_id: Some("comp-1"),
                        error_policy: ErrorPolicy::Continue,
                        running: false,
                        stale: false,
                        checked: &checked,
                        checks: None,
                        files_busy: false,
                        component: false,
                        document_locked: locked,
                        graphs: None,
                    };
                    let _ = panel.show(ui, &context);
                });
                out.textures_delta.clear();
                for clipped in &out.shapes {
                    painted(&clipped.shape, &mut texts);
                }
            }
            assert!(
                texts.iter().any(|text| text == "Check added"),
                "locked {locked}: {texts:?}"
            );
            assert!(
                texts
                    .iter()
                    .any(|text| text == "1 check added · it runs on the next run")
            );
            let offered = texts.iter().filter(|text| *text == "Check").count();
            if locked {
                assert_eq!(offered, 0, "a locked document offers no Check");
            } else {
                assert_eq!(offered, 1, "the token row still offers Check");
            }
        }
    }

    /// A workspace that is shorter for a frame (a notice under the tab strip
    /// while storage work runs) clamps the panel for that frame only: after
    /// it the panel is back at the height it keeps (UX-05, seen on CI). The
    /// panel is 500 px high, which a 560 px workspace cannot hold with the
    /// views' least height above it (UX-36).
    #[test]
    fn a_workspace_shorter_for_a_frame_does_not_keep_the_panel_short() {
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let mut panel = ReportPanel {
            open: true,
            height: 500.0,
            ..Default::default()
        };
        let report_context = Context {
            run: None,
            ends: None,
            selected_node_id: None,
            error_policy: ErrorPolicy::Continue,
            running: false,
            stale: false,
            checked: &[],
            checks: None,
            files_busy: false,
            component: false,
            document_locked: false,
            graphs: None,
        };
        let mut draw = |height: f32| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, height),
                    )),
                    ..Default::default()
                },
                |ui| {
                    panel.show(ui, &report_context);
                },
            );
            output.textures_delta.clear();
            panel.debug_resize_handle().2
        };
        for _ in 0..3 {
            draw(700.0);
        }
        assert_eq!(draw(700.0), 500.0, "the height it keeps");
        assert_eq!(
            draw(560.0),
            560.0 - VIEWS_LEAST_HEIGHT,
            "clamped to the shorter workspace"
        );
        draw(700.0);
        assert_eq!(draw(700.0), 500.0, "back at its height");
    }

    /// Run results held at their highest, by the grip and by egui's edge,
    /// and released in a frame whose workspace is shorter (a notice under
    /// the tab strip while storage work runs) stay at their highest once the
    /// workspace is back, as the drag's ceiling promises: the release takes
    /// the height the drag held, not the height that one frame could draw
    /// (UX-36, seen on CI).
    #[test]
    fn a_release_in_a_shorter_frame_keeps_the_highest_height() {
        // Where each is pressed: the grip's centre, 8 px under the panel's
        // top, and the edge on the top, left of the grip.
        for (way, pressed) in [
            ("grip", egui::pos2(600.0, 700.0 - 320.0 + 8.0)),
            ("edge", egui::pos2(100.0, 700.0 - 320.0)),
        ] {
            let context = egui::Context::default();
            crate::theme::apply(&context);
            let mut panel = ReportPanel {
                open: true,
                ..Default::default()
            };
            let report_context = Context {
                run: None,
                ends: None,
                selected_node_id: None,
                error_policy: ErrorPolicy::Continue,
                running: false,
                stale: false,
                checked: &[],
                checks: None,
                files_busy: false,
                component: false,
                document_locked: false,
                graphs: None,
            };
            let mut draw = |height: f32, events: Vec<egui::Event>| {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1200.0, height),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        panel.show(ui, &report_context);
                    },
                );
                output.textures_delta.clear();
                (panel.debug_resize_handle().2, panel.debug_state())
            };
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let highest = 700.0 - VIEWS_LEAST_HEIGHT;
            draw(700.0, vec![]);
            draw(700.0, vec![]);
            draw(700.0, vec![egui::Event::PointerMoved(pressed)]);
            draw(700.0, vec![button(pressed, true)]);
            // Above the highest top, outside the snap zone.
            let mut at = pressed;
            while at.y > DROP_SNAP + 30.0 {
                at.y = (at.y - 32.0).max(DROP_SNAP + 30.0);
                draw(700.0, vec![egui::Event::PointerMoved(at)]);
            }
            let (held, state) = draw(700.0, vec![]);
            assert_eq!(held, highest, "{way}: held at the highest: {state}");
            draw(660.0, vec![button(at, false), egui::Event::PointerGone]);
            draw(700.0, vec![]);
            let (released, state) = draw(700.0, vec![]);
            assert_eq!(released, highest, "{way}: released at the highest: {state}");
        }
    }

    #[test]
    fn resize_handle_receives_pointer_focus_and_keyboard_in_both_layouts() {
        for screen_height in [900.0, 400.0] {
            let context = egui::Context::default();
            crate::theme::apply(&context);
            let mut panel = ReportPanel {
                open: true,
                ..Default::default()
            };
            let report_context = Context {
                run: None,
                ends: None,
                selected_node_id: None,
                error_policy: ErrorPolicy::Continue,
                running: false,
                stale: false,
                checked: &[],
                checks: None,
                files_busy: false,
                component: false,
                document_locked: false,
                graphs: None,
            };
            let draw = |panel: &mut ReportPanel, events| {
                let mut id = egui::Id::NULL;
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1200.0, screen_height),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        id = ui.id().with("report-resize-handle");
                        panel.show(ui, &report_context);
                    },
                );
                output.textures_delta.clear();
                id
            };
            draw(&mut panel, vec![]);
            let id = draw(&mut panel, vec![]);
            let rect = context.read_response(id).unwrap().rect;
            let click = |pressed| egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            draw(
                &mut panel,
                vec![egui::Event::PointerMoved(rect.center()), click(true)],
            );
            draw(&mut panel, vec![click(false)]);
            assert!(
                context.memory(|memory| memory.has_focus(id)),
                "resize grip did not get focus at {screen_height}"
            );
            let before = if screen_height < 460.0 {
                panel.compact_height
            } else {
                panel.height
            };
            draw(
                &mut panel,
                vec![egui::Event::Key {
                    key: egui::Key::ArrowUp,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            draw(&mut panel, vec![]);
            let after = if screen_height < 460.0 {
                panel.compact_height
            } else {
                panel.height
            };
            assert!(
                after > before,
                "keyboard resize had no effect at {screen_height}: {before} -> {after}"
            );
        }
    }
}
