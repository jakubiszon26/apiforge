//! LAY-00 mock-up, round 3 (throwaway branch `mock/lay-00-round-3`): shape D
//! with every proposal on one board — short satellites, continuation marks,
//! badges on every step, aggregates after a run, shared checks, `@after` on a
//! hidden check, focus, running, wire tracing, semantic zoom and branch bands.
//!
//! The switches live in `crate::mock_lay00`; with them off the board behaves
//! as on `main`. Every wire goes through the real router. Nothing here is
//! product code.
#![allow(dead_code)]

use super::Camera;
use super::layout::{Card, CardDetail, RowValue, SHORT_ROW, WIDTH};
use super::routing::{BoardWire, Routed, Start};
use crate::mock_lay00::{self as mock, Anchor, Flags};
use crate::theme;
use apiforge_core::{
    format::{Binding, Graph, Node},
    graph::{ORDER_PORT, WireKind, dependency_index, wires},
    model::{Branch, NodeKind, Predicate},
    run::{StepResult, StepStatus},
};
use eframe::egui::{
    self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// The selection's neighbours: a light outline that is not the accent.
pub const RELATED: Color32 = Color32::from_rgb(40, 160, 160);
/// A step in progress: a blue that is not the selection orange.
pub const RUNNING: Color32 = Color32::from_rgb(30, 110, 230);
/// The tint of a branch band: neutral grey-blue.
pub const BAND: Color32 = Color32::from_rgb(112, 128, 160);

const LAYER_GAP: f32 = 96.0;
const STACK_GAP: f32 = 20.0;
const ROW_GAP: f32 = 120.0;
const BAND_MARGIN: f32 = 24.0;
/// Dimmed cards keep this much of their opacity, dimmed wires this much.
const CARD_DIM: f32 = 0.6;
const WIRE_DIM: f32 = 0.25;

/// What the last fit measured, and the routes' real clearance.
#[derive(Clone, Copy, Default, Debug)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
    pub hidden_cards: usize,
    pub hidden_wires: usize,
    /// The smallest distance from a routed wire to a card it does not end on.
    pub clearance: f32,
    /// Route segments closer than 12 px to such a card.
    pub tight: usize,
    pub segments: usize,
}

thread_local! {
    static EMPTY: RefCell<HashMap<String, (NodeKind, f32, f32)>> = RefCell::new(HashMap::new());
    static METRICS: RefCell<Metrics> = RefCell::new(Metrics::default());
    static SPINE: RefCell<Vec<(String, usize, usize)>> = const { RefCell::new(Vec::new()) };
}

pub fn metrics() -> Metrics {
    METRICS.with(|cell| *cell.borrow())
}

/// The spine as laid out: id, row and column.
pub fn spine() -> Vec<(String, usize, usize)> {
    SPINE.with(|cell| cell.borrow().clone())
}

fn is_check(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Assert | NodeKind::Condition)
}

fn on_spine(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Start | NodeKind::Request | NodeKind::Component | NodeKind::If
    )
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

/// The checks that are gates: data conditions.
pub fn gates(graph: &Graph) -> HashSet<String> {
    graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Condition)
        .map(|node| node.id.clone())
        .collect()
}

/// The engine's static order: ready steps in document order.
pub fn sequence(graph: &Graph) -> Vec<String> {
    let ids: HashSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    let deps = dependency_index(graph);
    let mut done: HashSet<&str> = HashSet::new();
    let mut order = Vec::new();
    while let Some(next) = graph.nodes.iter().find(|node| {
        !done.contains(node.id.as_str())
            && deps[node.id.as_str()]
                .iter()
                .filter(|dep| ids.contains(*dep))
                .all(|dep| done.contains(dep))
    }) {
        done.insert(&next.id);
        order.push(next.id.clone());
    }
    for node in &graph.nodes {
        if !done.contains(node.id.as_str()) {
            order.push(node.id.clone());
        }
    }
    order
}

/// The checks of one subject after a run, or before one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Aggregate {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub shared: usize,
    /// Conditions: checks that block what reads them when unmet.
    pub gates: usize,
    pub run: bool,
}

impl Aggregate {
    pub fn of(
        checks: &[String],
        steps: Option<&[StepResult]>,
        subjects_of: &HashMap<String, Vec<String>>,
        gates: &HashSet<String>,
    ) -> Self {
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
            skipped: checks
                .iter()
                .filter(|id| {
                    matches!(
                        status(id),
                        Some(StepStatus::Skipped | StepStatus::Blocked | StepStatus::Cancelled)
                    )
                })
                .count(),
            shared: checks
                .iter()
                .filter(|id| subjects_of.get(*id).is_some_and(|s| s.len() > 1))
                .count(),
            gates: checks.iter().filter(|id| gates.contains(*id)).count(),
            run: checks.iter().any(|id| status(id).is_some()),
        }
    }

    fn noun(count: usize) -> &'static str {
        if count == 1 { "check" } else { "checks" }
    }

    /// The checks' statement: `2 checks ✓`, `✕ 1/3 failed · 1 skipped`,
    /// `3 checks` before a run; `· 1 shared` when one is shared.
    pub fn text(self, shared: bool) -> String {
        let mut text = if !self.run {
            format!("{} {}", self.total, Self::noun(self.total))
        } else if self.failed > 0 {
            format!("✕ {}/{} failed", self.failed, self.total)
        } else if self.skipped > 0 {
            format!("{} {} ✓", self.passed, Self::noun(self.passed))
        } else {
            format!("{} {} ✓", self.total, Self::noun(self.total))
        };
        let waiting = self.total - self.passed - self.failed - self.skipped;
        if self.run && self.failed == 0 && waiting > 0 {
            text = format!("{}/{} ✓ · {waiting} waiting", self.passed, self.total);
        }
        if self.run && self.skipped > 0 {
            text.push_str(&format!(" · {} skipped", self.skipped));
        }
        if shared && self.gates > 0 {
            text.push_str(&format!(" · {} gate", self.gates));
        }
        if shared && self.shared > 0 {
            text.push_str(&format!(" · {} shared", self.shared));
        }
        text
    }

    /// The short form for a pill at Overview.
    pub fn short(self) -> String {
        if !self.run {
            format!("{} {}", self.total, Self::noun(self.total))
        } else if self.failed > 0 {
            format!("✕ {}/{} failed", self.failed, self.total)
        } else if self.skipped > 0 {
            format!("{}/{} ✓", self.passed, self.total)
        } else {
            format!("{} {} ✓", self.total, Self::noun(self.total))
        }
    }

    pub fn color(self) -> Color32 {
        if !self.run {
            theme::MUTED
        } else if self.failed > 0 {
            theme::DANGER
        } else {
            theme::SUCCESS
        }
    }
}

/// An ordering wire on a hidden check, drawn from the check's subject.
pub struct AfterHidden {
    pub id: String,
    pub subject: String,
    pub target: String,
    pub label: String,
}

/// A continuation mark at the end or the start of a spine row.
pub struct Mark {
    /// The spine card the mark sits beside.
    pub beside: Rect,
    pub number: usize,
    pub end: bool,
}

pub struct BandRect {
    pub rect: Rect,
    pub header: Option<String>,
}

pub struct Focus {
    pub selected: String,
    pub related: HashSet<String>,
}

/// What the mock-up adds to one frame of the main board.
pub struct Frame {
    pub flags: Flags,
    pub graph: Graph,
    pub hidden: HashSet<String>,
    pub seq: HashMap<String, usize>,
    pub checks_of: HashMap<String, Vec<String>>,
    pub subjects_of: HashMap<String, Vec<String>>,
    pub used_inputs: HashSet<(String, String)>,
    pub used_outputs: HashSet<(String, String)>,
    pub after_hidden: Vec<AfterHidden>,
    pub marks: Vec<Mark>,
    pub bands: Vec<BandRect>,
    /// Branch edges from an `if` into its band, which the band's one wire
    /// stands for, with the `if` and its branch.
    pub band_edges: HashMap<String, (String, Branch)>,
    /// The band each `if` branch enters: the first band rectangle.
    pub band_entries: Vec<(String, Branch, Rect)>,
    pub focus: Option<Focus>,
    /// Ends of every wire: source and target.
    pub ends: HashMap<String, (String, String)>,
    pub extra: Option<Rect>,
    pub rows: Vec<Rect>,
    /// Input rows of the cards, for input tracing.
    pub row_rects: HashMap<(String, String), Rect>,
}

/// Turns the main board into the round-3 board, or `None` when the switch is
/// off or the board is not a scenario's main flow.
pub fn frame(graph: &Graph, selection: &[String], cards: &mut Vec<Card>) -> Option<Frame> {
    let flags = mock::get();
    if !flags.on || !graph.nodes.iter().any(|node| node.kind == NodeKind::Start) {
        return None;
    }
    let hidden = if flags.collapsed {
        folded(graph)
    } else {
        HashSet::new()
    };
    cards.retain(|card| !hidden.contains(&card.id));
    let all_wires = wires(graph);
    let hidden_wires = all_wires
        .iter()
        .filter(|wire| hidden.contains(&wire.source) || hidden.contains(&wire.target))
        .count();
    let order_full = sequence(graph);
    let seq: HashMap<String, usize> = order_full
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index + 1))
        .collect();
    let kind: HashMap<&str, NodeKind> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.kind))
        .collect();
    let handle: HashMap<&str, &str> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.handle.as_str()))
        .collect();
    let subjects_of: HashMap<String, Vec<String>> = graph
        .nodes
        .iter()
        .filter(|node| is_check(node.kind))
        .map(|node| (node.id.clone(), subjects(graph, node)))
        .collect();

    // Satellites are short.
    for card in cards.iter_mut() {
        card.short = !on_spine(card.kind);
    }

    // Shape D: the spine in engine order, satellites under the latest spine
    // step they read directly, wrapped after `columns`.
    let order: Vec<String> = order_full
        .iter()
        .filter(|id| !hidden.contains(*id))
        .cloned()
        .collect();
    let spine: Vec<String> = order
        .iter()
        .filter(|id| on_spine(kind[id.as_str()]))
        .cloned()
        .collect();
    let slot: HashMap<String, usize> = spine
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let deps = dependency_index(graph);
    let mut hang: HashMap<String, usize> = HashMap::new();
    for id in &order {
        if slot.contains_key(id) {
            continue;
        }
        let sources = &deps[id.as_str()];
        let direct = sources
            .iter()
            .filter_map(|dep| slot.get(*dep).copied())
            .max();
        let column = direct
            .or_else(|| {
                sources
                    .iter()
                    .filter_map(|dep| hang.get(*dep).copied())
                    .max()
            })
            .unwrap_or(0);
        hang.insert(id.clone(), column);
    }
    let mut stacks: Vec<Vec<String>> = spine.iter().map(|id| vec![id.clone()]).collect();
    for id in &order {
        if let Some(column) = hang.get(id) {
            stacks[*column].push(id.clone());
        }
    }
    let index_of: HashMap<String, usize> = cards
        .iter()
        .enumerate()
        .map(|(index, card)| (card.id.clone(), index))
        .collect();
    let per = flags.columns.max(1);
    let pitch = WIDTH + LAYER_GAP;
    let rows = spine.len().div_ceil(per);
    let mut top = 0.0;
    let mut row_rects = Vec::new();
    let mut spine_layout = Vec::new();
    for row in 0..rows {
        let mut bottom: f32 = 0.0;
        for (i, stack) in stacks.iter().enumerate().filter(|(i, _)| i / per == row) {
            let x = (i % per) as f32 * pitch;
            let mut y = 0.0;
            for id in stack {
                let Some(index) = index_of.get(id) else {
                    continue;
                };
                cards[*index].origin = pos2(x, top + y);
                y += cards[*index].height() + STACK_GAP;
            }
            bottom = bottom.max(y - STACK_GAP);
            spine_layout.push((stack[0].clone(), row, i % per));
        }
        row_rects.push(Rect::from_min_size(
            pos2(0.0, top),
            vec2((per as f32) * pitch - LAYER_GAP, bottom),
        ));
        top += bottom + ROW_GAP;
    }
    SPINE.with(|cell| *cell.borrow_mut() = spine_layout);
    let rect_of = |id: &str| index_of.get(id).map(|index| cards[*index].rect());

    // Continuation marks: the last spine card of a row names the next step,
    // the first of the next row repeats it.
    let mut marks = Vec::new();
    for row in 1..rows {
        let first = &spine[row * per];
        let last = &spine[row * per - 1];
        let number = seq[first];
        if let (Some(last), Some(first)) = (rect_of(last), rect_of(first)) {
            marks.push(Mark {
                beside: last,
                number,
                end: true,
            });
            marks.push(Mark {
                beside: first,
                number,
                end: false,
            });
        }
    }

    // Bands: the steps an `if`'s branch edges lead to, with their satellites,
    // one rectangle per spine row.
    let mut bands = Vec::new();
    let mut band_edges = HashMap::new();
    let mut band_entries = Vec::new();
    if flags.bands {
        for node in graph.nodes.iter().filter(|node| node.kind == NodeKind::If) {
            for branch in [Branch::True, Branch::False] {
                let edges: Vec<_> = graph
                    .edges
                    .iter()
                    .filter(|edge| edge.source == node.id && edge.branch == Some(branch))
                    .collect();
                let members: HashSet<String> = edges
                    .iter()
                    .map(|edge| edge.target.clone())
                    .filter(|id| slot.contains_key(id))
                    .collect();
                if members.is_empty() {
                    continue;
                }
                for edge in &edges {
                    band_edges.insert(edge.id.clone(), (node.id.clone(), branch));
                }
                let mut by_row: Vec<(usize, Rect)> = Vec::new();
                for (i, stack) in stacks.iter().enumerate() {
                    if !members.contains(&stack[0]) {
                        continue;
                    }
                    let Some(rect) = stack
                        .iter()
                        .filter_map(|id| rect_of(id))
                        .reduce(|a, b| a.union(b))
                    else {
                        continue;
                    };
                    let row = i / per;
                    match by_row.iter_mut().find(|(r, _)| *r == row) {
                        Some((_, existing)) => *existing = existing.union(rect),
                        None => by_row.push((row, rect)),
                    }
                }
                let count = by_row.len();
                for (n, (_, rect)) in by_row.into_iter().enumerate() {
                    let mut rect = rect.expand(BAND_MARGIN);
                    if n == 0 {
                        rect.min.y -= 22.0;
                    }
                    // A band that wraps reaches to its continuation marks.
                    if count > 1 && n + 1 < count {
                        rect.max.x += MARK_ROOM;
                    }
                    if count > 1 && n > 0 {
                        rect.min.x -= MARK_ROOM;
                    }
                    let header = (n == 0).then(|| {
                        format!(
                            "{} → {}",
                            node.handle,
                            if branch == Branch::True { "Yes" } else { "No" }
                        )
                    });
                    if n == 0 {
                        band_entries.push((node.id.clone(), branch, rect));
                    }
                    bands.push(BandRect { rect, header });
                }
            }
        }
    }

    // `@after` on a hidden check: from the check's subject.
    let mut after_hidden = Vec::new();
    for edge in graph.edges.iter().filter(|edge| edge.branch.is_none()) {
        if !hidden.contains(&edge.source) || hidden.contains(&edge.target) {
            continue;
        }
        let Some(subject) = subjects_of
            .get(&edge.source)
            .and_then(|subjects| subjects.iter().max_by_key(|id| seq.get(*id)))
        else {
            continue;
        };
        if subject == &edge.target {
            continue;
        }
        after_hidden.push(AfterHidden {
            id: format!("lay00r3-after-{}", edge.id),
            subject: subject.clone(),
            target: edge.target.clone(),
            label: format!("after {}", handle[edge.source.as_str()]),
        });
    }

    // Ports in use.
    let mut used_inputs = HashSet::new();
    let mut used_outputs = HashSet::new();
    for wire in &all_wires {
        if wire.kind == WireKind::Data {
            if let Some(port) = &wire.source_port {
                used_outputs.insert((wire.source.clone(), port.clone()));
            }
            if let Some(input) = &wire.target_input {
                used_inputs.insert((wire.target.clone(), input.clone()));
            }
        }
    }

    // Focus: one selected step; its producers, consumers and checks.
    let focus = match selection {
        [selected] if kind.contains_key(selected.as_str()) => {
            let mut related: HashSet<String> = deps[selected.as_str()]
                .iter()
                .map(|dep| (*dep).to_owned())
                .collect();
            for node in &graph.nodes {
                if deps[node.id.as_str()].contains(selected.as_str()) {
                    related.insert(node.id.clone());
                }
            }
            for check in checks_of(graph).get(selected).into_iter().flatten() {
                related.insert(check.clone());
            }
            related.remove(selected);
            Some(Focus {
                selected: selected.clone(),
                related,
            })
        }
        _ => None,
    };

    // The badges and marks stick out of the cards a little.
    let mut extra = cards
        .iter()
        .map(|card| Rect::from_center_size(card.origin, vec2(16.0, 16.0)))
        .reduce(Rect::union);
    for mark in &marks {
        let rect = mark_rect(mark);
        extra = Some(extra.map_or(rect, |e| e.union(rect)));
    }
    for band in &bands {
        extra = Some(extra.map_or(band.rect, |e| e.union(band.rect)));
    }
    METRICS.with(|cell| {
        let mut metrics = cell.borrow_mut();
        metrics.hidden_cards = hidden.len();
        metrics.hidden_wires = hidden_wires;
    });
    let input_rows = cards
        .iter()
        .flat_map(|card| {
            card.inputs
                .iter()
                .enumerate()
                .map(|(index, row)| ((card.id.clone(), row.name.clone()), card.input_row(index)))
                .collect::<Vec<_>>()
        })
        .collect();
    Some(Frame {
        row_rects: input_rows,
        graph: graph.clone(),
        hidden,
        seq,
        checks_of: checks_of(graph),
        subjects_of,
        used_inputs,
        used_outputs,
        after_hidden,
        marks,
        bands,
        band_edges,
        band_entries,
        focus,
        ends: all_wires
            .into_iter()
            .map(|wire| (wire.id, (wire.source, wire.target)))
            .collect(),
        extra,
        rows: row_rects,
        flags,
    })
}

const MARK_ROOM: f32 = 64.0;

/// Where a continuation mark sits in the world.
fn mark_rect(mark: &Mark) -> Rect {
    let y = mark.beside.top() + 32.0;
    if mark.end {
        Rect::from_min_size(pos2(mark.beside.right() + 10.0, y - 12.0), vec2(48.0, 24.0))
    } else {
        Rect::from_min_size(pos2(mark.beside.left() - 58.0, y - 12.0), vec2(48.0, 24.0))
    }
}

/// Records the fit and returns the unclamped zoom, so the whole board shows.
pub fn fitted(bounds: Rect, needed: f32) -> f32 {
    METRICS.with(|cell| {
        let mut metrics = cell.borrow_mut();
        metrics.width = bounds.width();
        metrics.height = bounds.height();
        metrics.zoom = needed;
    });
    needed
}

impl Frame {
    /// The board's bounds for fitting: cards, badges, marks and bands.
    pub fn widen(&self, bounds: Rect) -> Rect {
        self.extra.map_or(bounds, |extra| bounds.union(extra))
    }

    /// The hand-placed camera, in the focus area's local coordinates.
    pub fn placed(&self, focus: Rect, cards: &[Card]) -> Option<(f32, Vec2)> {
        let (zoom, anchor) = self.flags.view.clone()?;
        match anchor {
            Anchor::Rows(first, last) => {
                let rows = self
                    .rows
                    .iter()
                    .skip(first)
                    .take(last + 1 - first)
                    .copied()
                    .reduce(Rect::union)?
                    .expand(BAND_MARGIN);
                let margin = 56.0;
                let left = if rows.width() * zoom + 2.0 * margin < focus.width() {
                    focus.center().x - rows.width() * zoom / 2.0
                } else {
                    focus.left() + margin
                };
                let top = focus.top() + 24.0;
                Some((zoom, vec2(left - rows.left() * zoom, top - rows.top() * zoom)))
            }
            Anchor::Cards(ids) => {
                let bounds = cards
                    .iter()
                    .filter(|card| ids.contains(&card.id))
                    .map(Card::rect)
                    .reduce(Rect::union)?;
                Some((zoom, focus.center().to_vec2() - bounds.center().to_vec2() * zoom))
            }
        }
    }

    /// The board's wires as this frame draws them: no edge an `if` band
    /// stands for, and the `@after` of hidden checks from their subjects.
    pub fn wires(&self, mut board: Vec<BoardWire>, cards: &[Card]) -> Vec<BoardWire> {
        board.retain(|wire| !self.band_edges.contains_key(&wire.id));
        let card_of = |id: &str| cards.iter().position(|card| card.id == id);
        for after in &self.after_hidden {
            let (Some(source), Some(target)) = (card_of(&after.subject), card_of(&after.target))
            else {
                continue;
            };
            board.push(BoardWire {
                id: after.id.clone(),
                start: Start::Output {
                    card: source,
                    port: ORDER_PORT.to_owned(),
                },
                target,
                input: ORDER_PORT.to_owned(),
                operand_path: Vec::new(),
                control: true,
                branch: None,
            });
        }
        board
    }

    fn lit_wire(&self, id: &str) -> bool {
        if let Some(trace) = &self.flags.trace {
            return trace == id;
        }
        let Some(focus) = &self.focus else {
            return true;
        };
        let ends = self.ends.get(id).cloned().or_else(|| {
            self.after_hidden
                .iter()
                .find(|after| after.id == id)
                .map(|after| (after.subject.clone(), after.target.clone()))
        });
        ends.is_some_and(|(source, target)| {
            (source == focus.selected && focus.related.contains(&target))
                || (target == focus.selected && focus.related.contains(&source))
        })
    }

    /// Alpha and width of a wire.
    pub fn wire_style(&self, id: &str) -> (f32, Option<f32>) {
        let dimming = self.focus.is_some() || self.flags.trace.is_some();
        if !dimming {
            return (1.0, None);
        }
        if self.lit_wire(id) {
            (1.0, Some(2.4))
        } else {
            (WIRE_DIM, None)
        }
    }

    pub fn dimmed(&self, id: &str) -> bool {
        self.focus
            .as_ref()
            .is_some_and(|focus| focus.selected != id && !focus.related.contains(id))
    }

    pub fn related(&self, id: &str) -> bool {
        if let Some(check) = &self.flags.hover_check
            && self.subjects_of.get(check).is_some_and(|s| s.iter().any(|s| s == id))
        {
            return true;
        }
        self.focus
            .as_ref()
            .is_some_and(|focus| focus.related.contains(id))
    }

    fn step<'a>(&self, id: &str, steps: Option<&'a [StepResult]>) -> Option<&'a StepResult> {
        if self.flags.not_run.iter().any(|n| n == id) || self.flags.running.as_deref() == Some(id)
        {
            return None;
        }
        steps.and_then(|steps| steps.iter().find(|step| step.node_id == id))
    }

    fn node(&self, id: &str) -> Option<&Node> {
        self.graph.nodes.iter().find(|node| node.id == id)
    }

    fn handle(&self, id: &str) -> String {
        self.node(id).map_or_else(|| id.to_owned(), |n| n.handle.clone())
    }

    /// Steps without a result count as not run for the aggregate.
    fn visible_steps(&self, steps: Option<&[StepResult]>) -> Option<Vec<StepResult>> {
        steps.map(|steps| {
            steps
                .iter()
                .filter(|step| !self.flags.not_run.contains(&step.node_id))
                .cloned()
                .collect()
        })
    }

    pub fn aggregate(&self, id: &str, steps: Option<&[StepResult]>) -> Option<Aggregate> {
        if !self.flags.collapsed {
            return None;
        }
        let checks = self.checks_of.get(id)?;
        let visible = self.visible_steps(steps);
        Some(Aggregate::of(
            checks,
            visible.as_deref(),
            &self.subjects_of,
            &gates(&self.graph),
        ))
    }

    /// The result line: `HTTP 201 · 2 checks ✓`.
    fn result_line(&self, card: &Card, steps: Option<&[StepResult]>) -> (String, Color32) {
        if self.flags.running.as_deref() == Some(card.id.as_str()) {
            return ("Running…".to_owned(), RUNNING);
        }
        let step = self.step(&card.id, steps);
        let first = match (step, card.kind) {
            (None, _) => "Not run".to_owned(),
            (Some(step), NodeKind::Request) => {
                let code = step
                    .outputs
                    .get("status")
                    .and_then(serde_json::Value::as_u64)
                    .or_else(|| {
                        self.node(&card.id)
                            .and_then(|n| n.config.sample_status)
                            .map(u64::from)
                    });
                match (step.status, code) {
                    (StepStatus::Passed, Some(code)) => format!("HTTP {code}"),
                    (status, _) => status.label().to_owned(),
                }
            }
            (Some(step), NodeKind::If) => match step.outputs.get("value") {
                Some(serde_json::Value::Bool(false)) => "→ No".to_owned(),
                _ => "→ Yes".to_owned(),
            },
            (Some(step), _) => format!("{} · {} ms", step.status.label(), step.duration_ms),
        };
        let color = match step.map(|s| s.status) {
            None => theme::MUTED,
            Some(StepStatus::Passed) => theme::SUCCESS,
            Some(StepStatus::Failed | StepStatus::Error) => theme::DANGER,
            Some(StepStatus::Blocked | StepStatus::Cancelled) => theme::WARNING,
            Some(StepStatus::Skipped) => theme::MUTED,
        };
        match self.aggregate(&card.id, steps) {
            Some(aggregate) => {
                let color = if aggregate.failed > 0 {
                    theme::DANGER
                } else {
                    color
                };
                (format!("{first} · {}", aggregate.text(true)), color)
            }
            None => (first, color),
        }
    }
}

fn band_color(step: Option<&StepResult>, running: bool) -> Color32 {
    if running {
        return RUNNING;
    }
    match step.map(|s| s.status) {
        Some(StepStatus::Passed) => theme::SUCCESS,
        Some(StepStatus::Failed | StepStatus::Error) => theme::DANGER,
        Some(StepStatus::Blocked | StepStatus::Cancelled) => theme::WARNING,
        Some(StepStatus::Skipped) | None => Color32::from_rgb(196, 196, 196),
    }
}

/// Text inside `rect`, ellipsised to its width.
fn text(painter: &Painter, rect: Rect, text: &str, font: FontId, color: Color32, align: egui::Align) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width().max(1.0));
    let galley = painter.layout_job(job);
    let x = match align {
        egui::Align::Min => rect.left(),
        egui::Align::Center => rect.center().x - galley.size().x / 2.0,
        egui::Align::Max => rect.right() - galley.size().x,
    };
    painter.galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}

fn pill(painter: &Painter, center: Pos2, label: &str, color: Color32, size: f32, fill: Color32) -> Rect {
    let galley = painter.layout_no_wrap(label.to_owned(), FontId::proportional(size), color);
    let rect = Rect::from_center_size(center, galley.size() + vec2(size * 1.0, size * 0.4));
    painter.rect(
        rect,
        rect.height() / 2.0,
        fill,
        Stroke::new(0.75, color.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    rect
}

fn symbol(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Equals => "=",
        Predicate::NotEquals => "≠",
        Predicate::Greater => ">",
        Predicate::Less => "<",
        Predicate::Approximately => "≈",
        Predicate::Contains => "∋",
        _ => "·",
    }
}

fn describe(graph: &Graph, binding: &Binding) -> String {
    match binding {
        Binding::Reference {
            node_id,
            port,
            path,
        } => {
            let mut text = graph
                .nodes
                .iter()
                .find(|node| &node.id == node_id)
                .map_or_else(|| node_id.clone(), |node| node.handle.clone());
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
        Binding::Literal { value } => value.to_string(),
        Binding::Parameter { name, .. } => format!("input {name}"),
        Binding::Environment { name } => format!("env {name}"),
        _ => "…".to_owned(),
    }
}

/// The one row of a short satellite: what it computes or checks.
fn short_text(frame: &Frame, card: &Card) -> String {
    let Some(node) = frame.node(&card.id) else {
        return String::new();
    };
    match node.kind {
        NodeKind::Assert => {
            let predicate = node.config.predicate.unwrap_or_default();
            let operands: Vec<String> = node
                .inputs
                .iter()
                .filter(|(name, _)| !name.contains("tolerance"))
                .take(2)
                .map(|(_, binding)| describe(&frame.graph, binding))
                .collect();
            match operands.as_slice() {
                [one] => format!("{one} {}", predicate.label()),
                _ => operands.join(&format!(" {} ", symbol(predicate))),
            }
        }
        _ => card
            .inputs
            .iter()
            .find_map(|row| match &row.value {
                RowValue::Source(text) | RowValue::Summary(text) => Some(text.clone()),
                RowValue::Inline { text, .. } => Some(text.clone()),
                _ => None,
            })
            .or_else(|| card.subtitle.clone())
            .unwrap_or_default(),
    }
}

/// The sequence badge, astride the card's top-left corner.
fn badge(painter: &Painter, rect: Rect, number: usize, running: bool, z: f32) {
    let diameter = (22.0 * z).clamp(10.0, 14.0);
    let radius = diameter / 2.0;
    let center = rect.left_top() + vec2(3.0, 3.0);
    let fill = if running {
        RUNNING
    } else {
        Color32::from_rgb(92, 92, 92)
    };
    if running {
        // The activity glyph: a ring with a bright arc, like a spinner.
        painter.circle_stroke(center, radius + 3.0, Stroke::new(1.5, RUNNING.gamma_multiply(0.35)));
        let arc: Vec<Pos2> = (0..=12)
            .map(|step| {
                let angle = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::PI * 1.2 * step as f32 / 12.0;
                center + vec2(angle.cos(), angle.sin()) * (radius + 3.0)
            })
            .collect();
        painter.add(egui::Shape::line(arc, Stroke::new(2.0, RUNNING)));
    }
    painter.circle(center, radius, fill, Stroke::new(1.0, Color32::WHITE));
    let galley = painter.layout_no_wrap(
        number.to_string(),
        FontId::new(
            diameter * if number >= 10 { 0.6 } else { 0.7 },
            theme::bold_family(),
        ),
        Color32::WHITE,
    );
    painter.galley(center - galley.size() / 2.0, galley, Color32::WHITE);
}

/// One card of the round-3 board.
pub fn paint_card(
    painter: &Painter,
    card: &Card,
    camera: Camera,
    frame: &Frame,
    steps: Option<&[StepResult]>,
    selected: bool,
) {
    let z = camera.zoom;
    let detail = CardDetail::at_zoom(z);
    let rect = camera.rect(card.rect());
    let step = frame.step(&card.id, steps);
    let running = frame.flags.running.as_deref() == Some(card.id.as_str());
    let aggregate = frame.aggregate(&card.id, steps);
    let failed = aggregate.is_some_and(|a| a.failed > 0)
        || step.is_some_and(|s| matches!(s.status, StepStatus::Failed | StepStatus::Error));
    let radius = (10.0 * z).max(3.0);

    if !card.short && detail == CardDetail::Full {
        super::paint::card(painter, card, camera, false, false, step);
    } else {
        painter.rect_filled(
            rect.translate(vec2(0.0, 2.0 * z)),
            radius,
            Color32::from_black_alpha(8),
        );
        painter.rect_filled(rect, radius, theme::WHITE);
        painter.rect_stroke(rect, radius, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    }
    let clipped = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
    let color = theme::block_color(card.block);
    let local = |x: f32, y: f32| camera.to_screen(card.origin + vec2(x, y));
    let cell = |l: f32, t: f32, r: f32, b: f32| Rect::from_min_max(local(l, t), local(r, b));
    // The name never drops below 11 px on screen.
    let name_size = (16.0 * z).max(11.0);

    if card.short {
        let overview = detail == CardDetail::Overview;
        if overview {
            let band = Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + (8.0 * z).max(3.0)));
            clipped.rect_filled(band, 0.0, band_color(step, running));
        } else {
            let icon = Rect::from_min_size(local(12.0, 9.0), Vec2::splat(22.0 * z));
            clipped.rect_filled(icon, 5.0 * z, color.gamma_multiply(0.10));
            theme::block_icon(&clipped, icon, card.block, color);
        }
        let (glyph, glyph_color) = match step.map(|s| s.status) {
            _ if !is_check(card.kind) => ("", theme::MUTED),
            Some(StepStatus::Passed) => ("✓", theme::SUCCESS),
            Some(StepStatus::Failed | StepStatus::Error) => ("✕", theme::DANGER),
            Some(_) => ("–", theme::MUTED),
            None => ("", theme::MUTED),
        };
        let name_rect = if overview {
            rect.shrink2(vec2(6.0, 2.0))
        } else {
            cell(42.0, 4.0, WIDTH - 36.0, SHORT_ROW - 2.0)
        };
        text(
            &clipped,
            if overview {
                Rect::from_min_max(name_rect.left_top(), pos2(name_rect.right(), name_rect.center().y + name_size * 0.2))
            } else {
                name_rect
            },
            &card.name,
            FontId::new(if overview { name_size.min(rect.height() * 0.6) } else { 15.0 * z }, theme::bold_family()),
            theme::TEXT,
            if overview { egui::Align::Center } else { egui::Align::Min },
        );
        if !glyph.is_empty() {
            text(
                &clipped,
                cell(WIDTH - 36.0, 4.0, WIDTH - 12.0, SHORT_ROW - 2.0),
                glyph,
                FontId::new((16.0 * z).max(9.0), theme::bold_family()),
                glyph_color,
                egui::Align::Max,
            );
        }
        if !overview {
            text(
                &clipped,
                cell(12.0, SHORT_ROW - 2.0, WIDTH - 12.0, 66.0),
                &short_text(frame, card),
                FontId::monospace(12.0 * z),
                if glyph == "✕" { theme::DANGER } else { theme::LINK },
                egui::Align::Min,
            );
        }
        if overview && !glyph.is_empty() {
            text(
                &clipped,
                Rect::from_min_max(pos2(rect.left(), rect.center().y), rect.right_bottom()),
                glyph,
                FontId::new(10.0, theme::bold_family()),
                glyph_color,
                egui::Align::Center,
            );
        }
        if failed {
            painter.rect_stroke(rect, radius, Stroke::new(2.0, theme::DANGER), StrokeKind::Inside);
        }
        sockets(painter, card, camera, frame);
    } else {
        match detail {
            CardDetail::Overview => {
                let band_height = (14.0 * z).max(5.0);
                let band = Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + band_height));
                let r = radius as u8;
                clipped.rect_filled(
                    band,
                    egui::CornerRadius { nw: r, ne: r, sw: 0, se: 0 },
                    band_color(step, running),
                );
                let name_rect = Rect::from_center_size(
                    pos2(rect.center().x, rect.top() + band_height + name_size * 1.6),
                    vec2(rect.width() - 8.0, name_size * 1.4),
                );
                text(
                    &clipped,
                    name_rect,
                    &card.name,
                    FontId::new(name_size, theme::bold_family()),
                    theme::TEXT,
                    egui::Align::Center,
                );
                if let Some(aggregate) = aggregate {
                    let size = 9.5_f32.max(11.0 * z);
                    let label = aggregate.short();
                    let color = aggregate.color();
                    let fill = if aggregate.failed > 0 {
                        Color32::from_rgb(252, 232, 230)
                    } else {
                        theme::WHITE
                    };
                    pill(
                        &clipped,
                        pos2(rect.center().x, name_rect.bottom() + size * 1.1),
                        &label,
                        color,
                        size,
                        fill,
                    );
                } else if running {
                    text(
                        &clipped,
                        Rect::from_center_size(
                            pos2(rect.center().x, name_rect.bottom() + 10.0),
                            vec2(rect.width() - 8.0, 14.0),
                        ),
                        "Running…",
                        FontId::proportional(10.0),
                        RUNNING,
                        egui::Align::Center,
                    );
                }
                if failed {
                    painter.rect_stroke(rect, radius, Stroke::new(2.0, theme::DANGER), StrokeKind::Inside);
                }
                sockets(painter, card, camera, frame);
            }
            CardDetail::Compact => {
                {
                    let inputs = card
                        .inputs
                        .iter()
                        .filter(|row| frame.used_inputs.contains(&(card.id.clone(), row.name.clone())))
                        .count() as f32;
                    let outputs = card
                        .outputs
                        .iter()
                        .filter(|row| frame.used_outputs.contains(&(card.id.clone(), row.name.clone())))
                        .count() as f32;
                    let used = if card.subtitle.is_some() { 88.0 } else { 64.0 }
                        + inputs * 52.0
                        + outputs * 28.0
                        + 36.0;
                    EMPTY.with(|cell| {
                        cell.borrow_mut()
                            .insert(card.id.clone(), (card.kind, card.height(), (card.height() - used).max(0.0)));
                    });
                }
                let icon = Rect::from_min_size(local(16.0, 16.0), Vec2::splat(28.0 * z));
                clipped.rect_filled(icon, 7.0 * z, color.gamma_multiply(0.10));
                theme::block_icon(&clipped, icon, card.block, color);
                text(
                    &clipped,
                    cell(56.0, 12.0, WIDTH - 16.0, 50.0),
                    &card.name,
                    FontId::new(16.0 * z, theme::bold_family()),
                    theme::TEXT,
                    egui::Align::Min,
                );
                if let Some(subtitle) = &card.subtitle {
                    text(
                        &clipped,
                        cell(16.0, 60.0, WIDTH - 16.0, 86.0),
                        subtitle,
                        FontId::proportional(12.0 * z),
                        theme::LINK,
                        egui::Align::Min,
                    );
                }
                for (index, row) in card.inputs.iter().enumerate() {
                    if !frame.used_inputs.contains(&(card.id.clone(), row.name.clone())) {
                        continue;
                    }
                    let top = card.input_row_top(index);
                    let source = match &row.value {
                        RowValue::Source(text) | RowValue::Summary(text) => text.clone(),
                        _ => String::new(),
                    };
                    text(
                        &clipped,
                        cell(16.0, top + 14.0, WIDTH - 16.0, top + 44.0),
                        &format!("{} ← {source}", row.label),
                        FontId::proportional(12.0 * z),
                        theme::MUTED,
                        egui::Align::Min,
                    );
                }
                for (index, row) in card.outputs.iter().enumerate() {
                    if !frame.used_outputs.contains(&(card.id.clone(), row.name.clone())) {
                        continue;
                    }
                    let y = card.output_y(index);
                    text(
                        &clipped,
                        cell(16.0, y - 12.0, WIDTH - 16.0, y + 12.0),
                        &row.label,
                        FontId::proportional(12.0 * z),
                        theme::TEXT,
                        egui::Align::Max,
                    );
                }
                footer(&clipped, card, camera, frame, steps, failed);
                sockets(painter, card, camera, frame);
            }
            CardDetail::Full => {
                footer(&clipped, card, camera, frame, steps, failed);
            }
        }
        if failed {
            painter.rect_stroke(rect, radius, Stroke::new(2.0, theme::DANGER), StrokeKind::Inside);
        }
    }
    if running && !failed {
        painter.rect_stroke(rect, radius, Stroke::new(2.0, RUNNING), StrokeKind::Inside);
    }
    if let Some(number) = frame.seq.get(&card.id) {
        badge(painter, rect, *number, running, z);
    }
    if frame.dimmed(&card.id) {
        painter.rect_filled(
            rect.expand(4.0),
            radius,
            theme::BG.gamma_multiply(1.0 - CARD_DIM),
        );
    }
    if frame.related(&card.id) {
        painter.rect_stroke(rect.expand(2.0), radius + 2.0, Stroke::new(1.5, RELATED), StrokeKind::Outside);
    }
    if selected {
        painter.rect_stroke(rect.expand(3.0), radius + 3.0, Stroke::new(2.5, theme::ACCENT), StrokeKind::Outside);
        // The resize handles as a selected card shows them.
        for corner in [rect.left_top(), rect.right_top(), rect.left_bottom(), rect.right_bottom()] {
            let handle = Rect::from_center_size(corner, Vec2::splat(7.0));
            painter.rect(handle, 1.0, theme::WHITE, Stroke::new(1.5, theme::ACCENT), StrokeKind::Inside);
        }
    }
}

/// The footer: the two-statement result line, red-tinted when a check failed.
fn footer(
    painter: &Painter,
    card: &Card,
    camera: Camera,
    frame: &Frame,
    steps: Option<&[StepResult]>,
    failed: bool,
) {
    let z = camera.zoom;
    let footer = camera.rect(card.footer());
    let full = CardDetail::at_zoom(z) == CardDetail::Full;
    let right = if full {
        camera.rect(card.action_button()).left() - 6.0 * z
    } else {
        footer.right() - 12.0 * z
    };
    let area = Rect::from_min_max(footer.left_top() + vec2(1.0, 1.0), pos2(right, footer.bottom() - 1.0));
    painter.rect_filled(
        area,
        0.0,
        if failed {
            Color32::from_rgb(252, 234, 231)
        } else {
            theme::WHITE
        },
    );
    painter.line_segment(
        [footer.left_top(), footer.right_top()],
        Stroke::new(0.75, if failed { theme::DANGER.gamma_multiply(0.5) } else { theme::BORDER }),
    );
    let (line, color) = frame.result_line(card, steps);
    text(
        painter,
        Rect::from_min_max(footer.left_top() + vec2(16.0 * z, 0.0), pos2(right, footer.bottom())),
        &line,
        if failed {
            FontId::new(12.0 * z, theme::bold_family())
        } else {
            FontId::proportional(12.0 * z)
        },
        color,
        egui::Align::Min,
    );
}

/// The ports in use: everything else is hidden below Full.
fn sockets(painter: &Painter, card: &Card, camera: Camera, frame: &Frame) {
    let z = camera.zoom;
    let radius = (4.0 * z).max(1.5);
    let mut points = Vec::new();
    for row in &card.inputs {
        if frame.used_inputs.contains(&(card.id.clone(), row.name.clone()))
            && let Some(anchor) = card.input_anchor(&row.name, &row.path)
        {
            points.push(anchor);
        }
    }
    for row in &card.outputs {
        if frame.used_outputs.contains(&(card.id.clone(), row.name.clone()))
            && let Some(anchor) = card.output_anchor(&row.name)
        {
            points.push(anchor);
        }
    }
    for point in points {
        painter.circle(
            camera.to_screen(point),
            radius,
            theme::WHITE,
            Stroke::new(1.0, theme::SOCKET),
        );
    }
}

/// Bands go under everything.
pub fn paint_bands(painter: &Painter, frame: &Frame, camera: Camera) {
    let z = camera.zoom;
    for band in &frame.bands {
        let rect = camera.rect(band.rect);
        painter.rect(
            rect,
            14.0 * z,
            BAND.gamma_multiply(0.10),
            Stroke::new(1.0, BAND.gamma_multiply(0.45)),
            StrokeKind::Inside,
        );
        if let Some(header) = &band.header {
            painter.text(
                rect.left_top() + vec2(10.0 * z, 5.0 * z),
                Align2::LEFT_TOP,
                header,
                FontId::new((12.0 * z).max(9.0), theme::bold_family()),
                Color32::from_rgb(80, 92, 120),
            );
        }
    }
}

/// The one dashed orthogonal wire from an `if` into each band.
pub fn paint_band_wires(painter: &Painter, frame: &Frame, cards: &[Card], camera: Camera) {
    let z = camera.zoom;
    for (source, branch, band) in &frame.band_entries {
        let Some(card) = cards.iter().find(|card| &card.id == source) else {
            continue;
        };
        let port = match branch {
            Branch::True => apiforge_core::graph::TRUE_PORT,
            Branch::False => apiforge_core::graph::FALSE_PORT,
        };
        let Some(start) = card.output_anchor(port) else {
            continue;
        };
        // Down out of the port, across under the `if`, up the band's edge.
        let below = start.y + 20.0 + if *branch == Branch::False { 12.0 } else { 0.0 };
        let mut points = vec![start, pos2(start.x, below)];
        let entry_y = band.top() + 40.0;
        if band.left() > card.rect().right() {
            let x = band.left() - 16.0 - if *branch == Branch::False { 8.0 } else { 0.0 };
            points.push(pos2(x, below));
            points.push(pos2(x, entry_y));
            points.push(pos2(band.left(), entry_y));
        } else {
            points.push(pos2(band.left() - 16.0, below));
            points.push(pos2(band.left() - 16.0, entry_y));
            points.push(pos2(band.left(), entry_y));
        }
        let color = match branch {
            Branch::True => theme::BRANCH_YES,
            Branch::False => theme::BRANCH_NO,
        };
        let screen: Vec<Pos2> = points.iter().map(|p| camera.to_screen(*p)).collect();
        super::paint::dashes(painter, &screen, Stroke::new(1.6, color), z);
        let end = screen[screen.len() - 1];
        let size = 6.0 * z.max(0.6);
        painter.add(egui::Shape::convex_polygon(
            vec![end, end - vec2(size, size * 0.5), end - vec2(size, -size * 0.5)],
            color,
            Stroke::NONE,
        ));
    }
}

/// Continuation marks: `↳ 10` at a row's end, `10 ↲` at the next row's start.
pub fn paint_marks(painter: &Painter, frame: &Frame, camera: Camera) {
    let z = camera.zoom;
    for mark in &frame.marks {
        let rect = camera.rect(mark_rect(mark));
        let label = if mark.end {
            format!("↳ {}", mark.number)
        } else {
            format!("{} ↲", mark.number)
        };
        let size = (13.0 * z).max(9.0);
        pill(painter, rect.center(), &label, theme::MUTED, size, theme::WHITE);
    }
}

/// A point at half the length of a polyline.
fn midpoint(points: &[Pos2]) -> Option<Pos2> {
    let total: f32 = points.windows(2).map(|p| p[0].distance(p[1])).sum();
    let mut left = total / 2.0;
    for pair in points.windows(2) {
        let length = pair[0].distance(pair[1]);
        if length >= left && length > 0.0 {
            return Some(pair[0] + (pair[1] - pair[0]) * (left / length));
        }
        left -= length;
    }
    points.first().copied()
}

/// The field a data wire carries: `login.body.token`.
fn field_of(frame: &Frame, wire_id: &str) -> Option<String> {
    let (source, target) = frame.ends.get(wire_id)?;
    let node = frame.node(target)?;
    let source_handle = frame.handle(source);
    for binding in node.inputs.values() {
        for reference in binding.references() {
            if reference.node_id == source {
                let mut text = source_handle.clone();
                if reference.port != "value" {
                    text.push('.');
                    text.push_str(reference.port);
                }
                if !reference.path.is_empty() {
                    text.push('.');
                    text.push_str(reference.path);
                }
                return Some(text);
            }
        }
    }
    None
}

/// Labels on wires, drawn over the cards: `after cart_total` on the ordering
/// wire of a hidden check, and the traced wire's field and rings.
pub fn paint_wire_labels(painter: &Painter, frame: &Frame, routed: &[Routed], camera: Camera) {
    let z = camera.zoom;
    for after in &frame.after_hidden {
        let Some(wire) = routed.iter().find(|wire| wire.id == after.id) else {
            continue;
        };
        let Some(middle) = midpoint(&wire.points) else {
            continue;
        };
        let alpha = frame.wire_style(&after.id).0;
        pill(
            painter,
            camera.to_screen(middle),
            &after.label,
            if alpha < 1.0 { Color32::from_rgb(170, 178, 190) } else { Color32::from_rgb(90, 100, 120) },
            (12.0 * z).max(9.0),
            theme::WHITE,
        );
    }
    if let Some(trace) = &frame.flags.trace
        && let Some(wire) = routed.iter().find(|wire| &wire.id == trace)
    {
        let color = Color32::from_rgb(60, 80, 110);
        let line: Vec<Pos2> = wire.points.iter().map(|p| camera.to_screen(*p)).collect();
        painter.add(egui::Shape::line(line, Stroke::new(2.4, color)));
        if let Some((card, row)) = &frame.flags.trace_row
            && let Some(rect) = frame.row_rects.get(&(card.clone(), row.clone()))
        {
            let rect = camera.rect(*rect);
            painter.rect(
                rect,
                4.0,
                color.gamma_multiply(0.08),
                Stroke::new(1.5, color),
                StrokeKind::Inside,
            );
        }
        for end in [wire.points.first(), wire.points.last()].into_iter().flatten() {
            painter.circle_stroke(camera.to_screen(*end), (8.0 * z).max(5.0), Stroke::new(2.0, color));
        }
        if let (Some(middle), Some(label)) = (midpoint(&wire.points), field_of(frame, trace)) {
            pill(painter, camera.to_screen(middle), &label, color, (12.0 * z).max(10.0), theme::WHITE);
        }
    }
}

/// The real clearance of the routes: the smallest distance from a routed
/// segment to a card the wire does not end on, sampled every 2 px.
pub fn measure_clearance(routed: &[Routed], wires: &[BoardWire], cards: &[Card]) {
    let mut smallest = f32::INFINITY;
    let mut tight = 0;
    let mut segments = 0;
    for route in routed.iter().filter(|route| route.routed) {
        let Some(wire) = wires.iter().find(|wire| wire.id == route.id) else {
            continue;
        };
        let source = match &wire.start {
            Start::Output { card, .. } => Some(*card),
            Start::Chip(_) => None,
        };
        let others: Vec<Rect> = cards
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index) != source && *index != wire.target)
            .map(|(_, card)| card.rect())
            .collect();
        for pair in route.points.windows(2) {
            segments += 1;
            let length = pair[0].distance(pair[1]);
            let samples = (length / 2.0).ceil().max(1.0) as usize;
            let mut nearest = f32::INFINITY;
            for step in 0..=samples {
                let point = pair[0] + (pair[1] - pair[0]) * (step as f32 / samples as f32);
                for rect in &others {
                    let dx = (rect.left() - point.x).max(point.x - rect.right()).max(0.0);
                    let dy = (rect.top() - point.y).max(point.y - rect.bottom()).max(0.0);
                    nearest = nearest.min((dx * dx + dy * dy).sqrt());
                }
            }
            if nearest < 11.5 {
                tight += 1;
            }
            smallest = smallest.min(nearest);
        }
    }
    METRICS.with(|cell| {
        let mut metrics = cell.borrow_mut();
        metrics.clearance = smallest;
        metrics.tight = tight;
        metrics.segments = segments;
    });
}

/// The last segment of the field one step reads from another.
fn read_field(frame: &Frame, reader: &str, source: &str) -> Option<String> {
    let node = frame.node(reader)?;
    for binding in node.inputs.values() {
        for reference in binding.references() {
            if reference.node_id == source {
                let path = if reference.path.is_empty() {
                    reference.port
                } else {
                    reference.path
                };
                return Some(path.rsplit('.').next().unwrap_or(path).to_owned());
            }
        }
    }
    None
}

/// Off-screen markers: a chip on the board's edge nearest each related card
/// out of view, and `← Back to …` after a jump.
pub fn paint_markers(painter: &Painter, frame: &Frame, cards: &[Card], camera: Camera, board: Rect) {
    #[derive(Clone, Copy, PartialEq)]
    enum Edge {
        Up,
        Down,
        Left,
        Right,
    }
    let inner = board.shrink(10.0);
    if frame.flags.markers
        && let Some(focus) = &frame.focus
    {
        let mut by_edge: Vec<(Edge, Vec<(Pos2, String, bool)>)> = Vec::new();
        let mut related: Vec<&String> = focus.related.iter().collect();
        related.sort_by_key(|id| frame.seq.get(*id));
        for id in related {
            let Some(card) = cards.iter().find(|card| &card.id == id) else {
                continue;
            };
            let rect = camera.rect(card.rect());
            if rect.intersects(board) {
                continue;
            }
            let center = rect.center();
            let edge = if center.y < board.top() {
                Edge::Up
            } else if center.y > board.bottom() {
                Edge::Down
            } else if center.x < board.left() {
                Edge::Left
            } else {
                Edge::Right
            };
            let producer = read_field(frame, &focus.selected, id);
            let (field, is_source) = match producer {
                Some(field) => (Some(field), true),
                None => (read_field(frame, id, &focus.selected), false),
            };
            let name = frame.handle(id);
            let label = match field {
                Some(field) => format!("{name} · {field}"),
                None => name,
            };
            match by_edge.iter_mut().find(|(e, _)| *e == edge) {
                Some((_, list)) => list.push((center, label, is_source)),
                None => by_edge.push((edge, vec![(center, label, is_source)])),
            }
        }
        for (edge, list) in by_edge {
            let arrow = match edge {
                Edge::Up => "↑",
                Edge::Down => "↓",
                Edge::Left => "←",
                Edge::Right => "→",
            };
            let chips: Vec<(Pos2, String)> = if list.len() > 2 {
                let sources = list.iter().all(|(_, _, s)| *s);
                let readers = list.iter().all(|(_, _, s)| !*s);
                let noun = if sources {
                    "sources"
                } else if readers {
                    "readers"
                } else {
                    "related"
                };
                let center = list[0].0;
                vec![(center, format!("{arrow} {} {noun}", list.len()))]
            } else {
                list.into_iter()
                    .map(|(center, label, _)| (center, format!("{arrow} {label}")))
                    .collect()
            };
            let mut last: Option<Rect> = None;
            for (center, label) in chips {
                let mut at = match edge {
                    Edge::Up => pos2(center.x.clamp(inner.left() + 80.0, inner.right() - 80.0), inner.top() + 12.0),
                    Edge::Down => pos2(center.x.clamp(inner.left() + 80.0, inner.right() - 80.0), inner.bottom() - 12.0),
                    Edge::Left => pos2(inner.left() + 70.0, center.y.clamp(inner.top() + 20.0, inner.bottom() - 20.0)),
                    Edge::Right => pos2(inner.right() - 70.0, center.y.clamp(inner.top() + 20.0, inner.bottom() - 20.0)),
                };
                if let Some(previous) = last
                    && (previous.center().x - at.x).abs() < 170.0
                    && matches!(edge, Edge::Up | Edge::Down)
                {
                    at.x = previous.right() + 90.0;
                }
                let rect = pill(painter, at, &label, Color32::from_rgb(20, 120, 120), 12.0, Color32::from_rgb(232, 247, 247));
                last = Some(rect);
            }
        }
    }
    if let Some(from) = &frame.flags.jumped_from {
        let label = format!("← Back to {}", frame.handle(from));
        let galley = painter.layout_no_wrap(label.clone(), FontId::proportional(13.0), theme::ACCENT);
        let center = inner.left_top() + vec2(galley.size().x / 2.0 + 14.0, 18.0);
        pill(painter, center, &label, theme::ACCENT, 13.0, theme::ACCENT_SOFT);
    }
}

/// Each card drawn at Compact: kind, height and the height left empty.
pub fn empty() -> HashMap<String, (NodeKind, f32, f32)> {
    EMPTY.with(|cell| cell.borrow().clone())
}
