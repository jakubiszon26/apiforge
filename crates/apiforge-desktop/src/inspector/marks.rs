//! The mark of a field a check already reads (DIAG-12): the ✓ of its row
//! filled, outlined on a folded field with a checked field inside, and in
//! Run results in the colour of the checks' outcome.
//!
//! What is marked is decided once per edit of the document, by
//! [`CheckIndex::build`]; a row looks its field up and walks no graph.

use crate::icons::Icon;
use crate::theme;
use apiforge_core::{
    format::{Graph, Node, Project},
    graph::sequence,
    model::NodeKind,
    path::JsonPath,
    run::{RunResult, StepStatus},
    template_text::Scope,
};
use eframe::egui::{self, Color32, Ui};
use std::cell::Cell;
use std::collections::HashMap;

thread_local! {
    /// How many indexes this thread built: one per edit, never one per
    /// frame (criterion 7).
    static BUILDS: Cell<usize> = const { Cell::new(0) };
}

/// One check as a marked row names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedCheck {
    /// The check's node.
    pub id: String,
    /// What it holds, as DIAG-02 writes it under the step it reads.
    pub text: String,
}

/// The checks of one output of one step, by the path of the field.
#[derive(Clone, Debug, Default)]
pub struct PortMarks {
    /// The checks that name the field: indexes into [`CheckIndex::checks`].
    on: HashMap<String, Vec<usize>>,
    /// The checks that name a field inside it.
    inside: HashMap<String, Vec<usize>>,
}

/// Which checks read which field of which step's output, for one revision of
/// a document: `(step, port, path) → checks`.
#[derive(Clone, Debug, Default)]
pub struct CheckIndex {
    checks: Vec<IndexedCheck>,
    ports: HashMap<(String, String), PortMarks>,
}

fn is_check(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Assert | NodeKind::Condition)
}

impl CheckIndex {
    /// The index of a document's checks: those of `main`, then those of the
    /// scenario's `cleanup`, which read Main's steps too (LAY-20). Only the
    /// field a check names is marked, never the fields under it; a check of
    /// a component's inner step is the component's, not its caller's.
    pub fn build(main: &Graph, cleanup: Option<&Graph>, project: &Project) -> Self {
        BUILDS.with(|builds| builds.set(builds.get() + 1));
        let mut index = Self::default();
        index.add(main, main, project);
        if let Some(cleanup) = cleanup.filter(|graph| graph.nodes.iter().any(is_check)) {
            // A Cleanup check is written against Main's steps and its own.
            let mut both = main.clone();
            both.nodes.extend(cleanup.nodes.iter().cloned());
            index.add(cleanup, &both, project);
        }
        index
    }

    /// How many indexes this thread has built.
    #[cfg(test)]
    pub fn builds() -> usize {
        BUILDS.with(Cell::get)
    }

    fn add(&mut self, graph: &Graph, lookup: &Graph, project: &Project) {
        let nodes: HashMap<&str, &Node> = graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        for id in sequence(graph) {
            let Some(check) = nodes.get(id).filter(|node| is_check(node)) else {
                continue;
            };
            let scope = Scope {
                nodes: &lookup.nodes,
                project,
                request: Some(check),
            };
            let own = crate::canvas::own_step(check, lookup);
            let position = self.checks.len();
            self.checks.push(IndexedCheck {
                id: check.id.clone(),
                text: crate::canvas::check_text(check, scope, lookup, own).capped(80),
            });
            for binding in check.inputs.values() {
                for reference in binding.references() {
                    let Ok(path) = JsonPath::parse(reference.path) else {
                        continue;
                    };
                    let marks = self
                        .ports
                        .entry((reference.node_id.to_owned(), reference.port.to_owned()))
                        .or_default();
                    push(marks.on.entry(path.to_string()).or_default(), position);
                    let mut prefix = JsonPath::root();
                    for segment in &path.segments()[..path.segments().len().saturating_sub(1)] {
                        push(
                            marks.inside.entry(prefix.to_string()).or_default(),
                            position,
                        );
                        prefix.push_key(segment.clone());
                    }
                    if !path.is_root() {
                        push(
                            marks.inside.entry(prefix.to_string()).or_default(),
                            position,
                        );
                    }
                }
            }
        }
    }

    /// The marks of `port` of the step `node`, if a check reads it.
    pub fn port(&self, node: &str, port: &str) -> Option<&PortMarks> {
        self.ports.get(&(node.to_owned(), port.to_owned()))
    }

    /// The checks that name the field at `path` of `port` of `node`.
    #[cfg(test)]
    pub fn on(&self, node: &str, port: &str, path: &str) -> Vec<&IndexedCheck> {
        self.port(node, port)
            .and_then(|marks| marks.on.get(path))
            .map(|checks| checks.iter().map(|&check| &self.checks[check]).collect())
            .unwrap_or_default()
    }
}

/// Lists `check` once, even where it names a field twice.
fn push(checks: &mut Vec<usize>, check: usize) {
    if checks.last() != Some(&check) {
        checks.push(check);
    }
}

/// What a tree of fields marks: the document's index and, in Run results,
/// the run whose outcomes colour the marks.
#[derive(Clone, Copy)]
pub struct Marks<'a> {
    pub index: &'a CheckIndex,
    /// `None` in the Inspector, whose mark says that a check exists, not how
    /// it ended.
    pub run: Option<&'a RunResult>,
}

/// How a mark is coloured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkTone {
    /// The Inspector's: a check exists.
    Plain,
    /// Every check of the field passed.
    Passed,
    /// One of them failed.
    Failed,
    /// Not every check ran, and none failed: the run does not know a check
    /// added since, or skipped it.
    NotRun,
}

impl MarkTone {
    pub(super) fn colour(self) -> Color32 {
        match self {
            Self::Plain => theme::LINK,
            Self::Passed => theme::SUCCESS,
            Self::Failed => theme::DANGER,
            Self::NotRun => theme::MUTED,
        }
    }
}

/// A tree's view of the index: its output's marks and the outcome of each
/// check, looked up once per tree and frame.
pub(super) struct TreeMarks<'a> {
    index: &'a CheckIndex,
    port: &'a PortMarks,
    /// The run's status of each check, when the tree is Run results'.
    outcomes: Option<HashMap<&'a str, StepStatus>>,
}

impl<'a> TreeMarks<'a> {
    pub(super) fn new(marks: Marks<'a>, node: &str, port: &str) -> Option<Self> {
        let port = marks.index.port(node, port)?;
        let outcomes = marks.run.map(|run| {
            run.steps
                .iter()
                .map(|step| (step.node_id.as_str(), step.status))
                .collect()
        });
        Some(Self {
            index: marks.index,
            port,
            outcomes,
        })
    }

    /// The mark of the row of `path`: its own checks, or, folded, those of
    /// the fields inside it.
    pub(super) fn row(&self, path: &str, folded: bool) -> Option<RowMark> {
        let (checks, inside) = match self.port.on.get(path) {
            Some(checks) => (checks, false),
            None if folded => (self.port.inside.get(path)?, true),
            None => return None,
        };
        let checks: Vec<&IndexedCheck> = checks
            .iter()
            .map(|&check| &self.index.checks[check])
            .collect();
        let status = |check: &IndexedCheck| {
            self.outcomes
                .as_ref()
                .and_then(|outcomes| outcomes.get(check.id.as_str()).copied())
        };
        let failed = checks
            .iter()
            .filter(|check| matches!(status(check), Some(StepStatus::Failed | StepStatus::Error)))
            .count();
        let tone = if self.outcomes.is_none() {
            MarkTone::Plain
        } else if failed > 0 {
            MarkTone::Failed
        } else if checks
            .iter()
            .all(|check| status(check) == Some(StepStatus::Passed))
        {
            MarkTone::Passed
        } else {
            MarkTone::NotRun
        };
        let count = checks.len();
        let plural = if count == 1 { "check" } else { "checks" };
        let words = if inside {
            format!("{count} {plural} inside")
        } else if failed > 0 {
            format!("{failed} failed")
        } else {
            format!("{count} {plural}")
        };
        let results = self.outcomes.is_some();
        let listed: Vec<Listed> = if inside {
            Vec::new()
        } else {
            checks
                .iter()
                .map(|check| Listed {
                    id: check.id.clone(),
                    text: check.text.clone(),
                    outcome: results.then(|| {
                        let status = status(check);
                        (
                            outcome(status),
                            match status {
                                Some(StepStatus::Passed) => theme::SUCCESS,
                                Some(StepStatus::Failed | StepStatus::Error) => theme::DANGER,
                                _ => theme::MUTED,
                            },
                        )
                    }),
                })
                .collect()
        };
        let said = if inside {
            words.clone()
        } else {
            listed
                .iter()
                .map(|check| match check.outcome {
                    Some((word, _)) => format!("{}  ·  {word}", check.text),
                    None => check.text.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        Some(RowMark {
            inside,
            tone,
            words,
            said,
            listed,
        })
    }
}

/// A check's outcome in a word.
fn outcome(status: Option<StepStatus>) -> &'static str {
    status.map_or("Not run", StepStatus::label)
}

/// One check the menu of ✓ lists above *Add a check*.
#[derive(Clone, Debug)]
pub struct Listed {
    pub id: String,
    pub text: String,
    /// In Run results, the check's outcome in a word and its colour.
    pub outcome: Option<(&'static str, Color32)>,
}

/// The mark of one row.
pub(super) struct RowMark {
    /// Outlined: the checks are of fields inside the folded row.
    pub inside: bool,
    pub tone: MarkTone,
    /// *2 checks*, *1 failed*, *1 check inside*.
    pub words: String,
    /// What the hover and assistive technology say: the checks, one a
    /// line, or the words of an outlined mark.
    pub said: String,
    /// The checks its menu lists.
    pub listed: Vec<Listed>,
}

/// The width a mark with `words` takes as Run results' *Check*.
pub(super) fn words_width(ui: &Ui, words: &str) -> f32 {
    MARK_TEXT_X + 14.0 + super::fields::text_width(ui, words, theme::FONT_LABEL)
}

/// Where a mark's words start from its button's left.
const MARK_TEXT_X: f32 = 30.0;
/// The side of the filled or outlined square.
const BADGE: f32 = 18.0;

/// The ✓ or *Check* of a marked row: a button named `name` that opens the
/// menu, its ✓ a filled square, or an outlined one for the checks inside a
/// folded field, followed by `words` where it has them. It stands exactly
/// where the unmarked one does.
pub(super) fn button(
    ui: &mut Ui,
    name: &str,
    mark: &RowMark,
    words: bool,
    width: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, super::fields::TREE_BUTTON),
        egui::Sense::click(),
    );
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    let said = mark.said.clone();
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_description(said));
    if ui.is_rect_visible(rect) {
        let colour = if enabled {
            mark.tone.colour()
        } else {
            theme::MUTED
        };
        let engaged = enabled && (response.hovered() || response.is_pointer_button_down_on());
        let painter = ui.painter();
        if words {
            let pill = rect.shrink2(egui::vec2(0.0, 2.0));
            painter.rect_filled(
                pill,
                6.0,
                colour.gamma_multiply(if engaged { 0.2 } else { 0.1 }),
            );
        } else if engaged {
            painter.rect_filled(rect.shrink(0.5), 5.0, theme::FIELD);
        }
        if enabled && response.has_focus() {
            painter.rect_stroke(
                rect.shrink(0.5),
                5.0,
                egui::Stroke::new(1.5, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        let centre = if words {
            egui::pos2(rect.left() + MARK_TEXT_X / 2.0, rect.center().y)
        } else {
            rect.center()
        };
        badge(painter, centre, mark, colour);
        if words {
            painter.text(
                egui::pos2(rect.left() + MARK_TEXT_X, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &mark.words,
                egui::FontId::proportional(theme::FONT_LABEL),
                colour,
            );
        }
    }
    response
}

/// A filled square with ✓ (✕ where a check failed), or an outlined one with
/// a dot for the checks inside a folded field.
fn badge(painter: &egui::Painter, centre: egui::Pos2, mark: &RowMark, colour: Color32) {
    let rect = egui::Rect::from_center_size(centre, egui::vec2(BADGE, BADGE));
    if mark.inside {
        painter.rect_filled(rect, 5.0, theme::WHITE);
        painter.rect_stroke(
            rect,
            5.0,
            egui::Stroke::new(1.4, colour),
            egui::StrokeKind::Inside,
        );
        painter.circle_filled(centre, 3.0, colour);
    } else {
        painter.rect_filled(rect, 5.0, colour);
        let icon = if mark.tone == MarkTone::Failed {
            Icon::Close
        } else {
            Icon::Check
        };
        icon.paint(
            painter,
            egui::Rect::from_center_size(centre, egui::vec2(13.0, 13.0)),
            theme::WHITE,
        );
    }
}

/// A mark drawn in the last frames, for the tests.
#[cfg(test)]
#[derive(Clone, Debug)]
pub struct DrawnMark {
    /// Run results' tree, or the Inspector's.
    pub results: bool,
    pub port: String,
    pub path: String,
    pub words: String,
    pub inside: bool,
    pub tone: MarkTone,
    /// The mark is an icon, without words.
    pub icon: bool,
    /// The row's end draws the mark: not behind *Check added* or an end
    /// that draws nothing.
    pub shown: bool,
}

#[cfg(test)]
thread_local! {
    static DRAWN: std::cell::RefCell<Vec<DrawnMark>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(super) fn record(mark: DrawnMark) {
    DRAWN.with(|drawn| drawn.borrow_mut().push(mark));
}

/// The marks this thread drew since [`debug_forget_marks`].
#[cfg(test)]
pub fn debug_marks() -> Vec<DrawnMark> {
    DRAWN.with(|drawn| drawn.borrow().clone())
}

/// Forgets the marks drawn so far.
#[cfg(test)]
pub fn debug_forget_marks() {
    DRAWN.with(|drawn| drawn.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::demos;

    fn document(text: &str) -> (Graph, Graph, Project) {
        let project = demos::shop_project();
        let lowered = apiforge_script::lower(
            &apiforge_script::parse(text),
            &apiforge_script::LowerContext::folder(&project),
        );
        let Some(apiforge_script::Document::Scenario(scenario)) = lowered.document else {
            panic!("the text is a scenario: {:?}", lowered.diagnostics);
        };
        (scenario.graph, scenario.cleanup, project)
    }

    #[test]
    fn a_check_marks_the_field_it_names_and_its_parents_inside() {
        let (main, cleanup, project) = document(
            "scenario \"S\" {\n  begin = start\n  cart = GET \"/c\" {\n    sample 200 { owner: { name: \"Ann\" }, items: [1] }\n  }\n  named = assert cart.body.owner.name == \"Ann\"\n  some = assert nonempty cart.body.items\n  cleanup {\n    kept = assert exists cart.body.items[0]\n  }\n}\n",
        );
        let index = CheckIndex::build(&main, Some(&cleanup), &project);
        let cart = main
            .nodes
            .iter()
            .find(|node| node.handle == "cart")
            .unwrap()
            .id
            .clone();
        let texts = |path: &str| {
            index
                .on(&cart, "body", path)
                .into_iter()
                .map(|check| check.text.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(texts("owner.name"), ["owner.name == \"Ann\""]);
        assert_eq!(texts("items"), ["nonempty items"]);
        assert_eq!(texts("items[0]"), ["exists items[0]"]);
        assert!(texts("owner").is_empty(), "the parent is not named");
        let port = index.port(&cart, "body").unwrap();
        assert_eq!(port.inside["owner"].len(), 1);
        assert_eq!(port.inside[""].len(), 3);
        assert!(!port.inside.contains_key("owner.name"));
    }
}
