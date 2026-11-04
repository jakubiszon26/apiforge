//! LAY-00 mock-up switches (throwaway branch `mock/lay-00-cards`). The design
//! review views in `folder::lay00_review` turn them on for their own thread;
//! with every switch off the window behaves as on `main`.
#![allow(dead_code)]

use apiforge_core::{
    format::{Graph, Node},
    graph::dependency_index,
    model::NodeKind,
    run::{StepResult, StepStatus},
};
use eframe::egui::Rect;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default, Debug)]
pub struct Flags {
    /// The *Checks* toggle in the board's toolbar.
    pub toggle: bool,
    /// Checks with a subject are folded into their subjects' aggregates.
    pub collapsed: bool,
    /// The proposed cards: sequence badges, status bands at Overview, the
    /// status text at Compact, borders for failed and blocked steps.
    pub proposed: bool,
    /// Focus on selection: neighbours ringed, everything else dimmed.
    pub focus: bool,
    /// A run in progress: this step runs, the steps without a result wait.
    pub running: Option<String>,
    /// The Inspector lists the checks of this step.
    pub inspector: Option<String>,
    /// Tinted bands behind the branches of an `if`.
    pub bands: bool,
    /// The camera: this zoom, centred on these cards.
    pub view: Option<(f32, Vec<String>)>,
    /// Fit the whole board every frame.
    pub fit: bool,
}

thread_local! {
    static FLAGS: RefCell<Flags> = RefCell::new(Flags::default());
    static STEPS: RefCell<Option<Vec<StepResult>>> = const { RefCell::new(None) };
    static RECTS: RefCell<HashMap<String, Rect>> = RefCell::new(HashMap::new());
    static INSPECTOR: std::cell::Cell<Option<Rect>> = const { std::cell::Cell::new(None) };
}

pub fn get() -> Flags {
    FLAGS.with(|flags| flags.borrow().clone())
}

pub fn set(flags: Flags) {
    FLAGS.with(|cell| *cell.borrow_mut() = flags);
}

pub fn update(change: impl FnOnce(&mut Flags)) {
    FLAGS.with(|cell| change(&mut cell.borrow_mut()));
}

/// Fake step results the board shows instead of the document's run.
pub fn set_steps(steps: Option<Vec<StepResult>>) {
    STEPS.with(|cell| *cell.borrow_mut() = steps);
}

pub fn steps() -> Option<Vec<StepResult>> {
    STEPS.with(|cell| cell.borrow().clone())
}

/// Where each card was drawn last frame, in points.
pub fn record(id: &str, rect: Rect) {
    RECTS.with(|cell| {
        cell.borrow_mut().insert(id.to_owned(), rect);
    });
}

pub fn rect_of(id: &str) -> Option<Rect> {
    RECTS.with(|cell| cell.borrow().get(id).copied())
}

pub fn clear_rects() {
    RECTS.with(|cell| cell.borrow_mut().clear());
}

pub fn set_inspector(rect: Option<Rect>) {
    INSPECTOR.with(|cell| cell.set(rect));
}

pub fn inspector_rect() -> Option<Rect> {
    INSPECTOR.with(std::cell::Cell::get)
}

pub fn is_check(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Assert | NodeKind::Condition)
}

/// The request or component-call steps a check references directly.
pub fn subjects(graph: &Graph, node: &Node) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for binding in node.inputs.values() {
        for reference in binding.references() {
            let subject = graph.nodes.iter().find(|n| n.id == reference.node_id);
            if subject.is_some_and(|n| matches!(n.kind, NodeKind::Request | NodeKind::Component))
                && !result.iter().any(|id| id == reference.node_id)
            {
                result.push(reference.node_id.to_owned());
            }
        }
    }
    result
}

/// Each subject's checks, in document order.
pub fn checks_of(graph: &Graph) -> HashMap<String, Vec<String>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    for node in graph.nodes.iter().filter(|node| is_check(node.kind)) {
        for subject in subjects(graph, node) {
            result.entry(subject).or_default().push(node.id.clone());
        }
    }
    result
}

/// The checks a collapsed board does not draw: those with a subject.
pub fn folded(graph: &Graph) -> HashSet<String> {
    graph
        .nodes
        .iter()
        .filter(|node| is_check(node.kind) && !subjects(graph, node).is_empty())
        .map(|node| node.id.clone())
        .collect()
}

/// The order steps run in: one at a time, a step is ready when its
/// dependencies have finished, and among ready steps document order decides.
pub fn sequence(graph: &Graph) -> HashMap<String, usize> {
    let index = dependency_index(graph);
    let ids: HashSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    let mut done: HashSet<&str> = HashSet::new();
    let mut result = HashMap::new();
    let mut number = 0;
    while done.len() < graph.nodes.len() {
        let Some(next) = graph.nodes.iter().find(|node| {
            !done.contains(node.id.as_str())
                && index.get(node.id.as_str()).is_none_or(|deps| {
                    deps.iter()
                        .all(|dep| !ids.contains(dep) || done.contains(dep))
                })
        }) else {
            break;
        };
        number += 1;
        done.insert(next.id.as_str());
        result.insert(next.id.clone(), number);
    }
    result
}

/// The checks of one subject after a run, or before one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aggregate {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

impl Aggregate {
    pub fn of(checks: &[String], steps: Option<&[StepResult]>) -> Self {
        let status = |id: &String| {
            steps.and_then(|steps| {
                steps
                    .iter()
                    .find(|step| &step.node_id == id)
                    .map(|s| s.status)
            })
        };
        Self {
            total: checks.len(),
            passed: checks
                .iter()
                .filter(|id| status(id) == Some(StepStatus::Passed))
                .count(),
            failed: checks
                .iter()
                .filter(|id| matches!(status(id), Some(StepStatus::Failed | StepStatus::Error)))
                .count(),
        }
    }

    pub fn text(self) -> String {
        if self.failed > 0 {
            format!("✕ {}/{} failed", self.failed, self.total)
        } else if self.total == 1 {
            "✓ 1 check".to_owned()
        } else {
            format!("✓ {} checks", self.total)
        }
    }
}

/// Producers and consumers of `id`, through data and ordering.
pub fn neighbours(graph: &Graph, id: &str) -> (Vec<String>, Vec<String>) {
    let index = dependency_index(graph);
    let producers = index
        .get(id)
        .map(|deps| deps.iter().map(|dep| (*dep).to_owned()).collect())
        .unwrap_or_default();
    let consumers = graph
        .nodes
        .iter()
        .filter(|node| {
            index
                .get(node.id.as_str())
                .is_some_and(|deps| deps.contains(id))
        })
        .map(|node| node.id.clone())
        .collect();
    (producers, consumers)
}
