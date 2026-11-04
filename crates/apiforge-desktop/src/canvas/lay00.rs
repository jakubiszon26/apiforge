//! LAY-00 mock-up, round 2 (throwaway branch `mock/lay-00-shapes-2`): shapes
//! B and D of a tidied scenario board, with checks expanded or collapsed,
//! rendered by the `review_lay00r2_*` views below.
//!
//! A test-only switch replaces the positions of the main board's cards with
//! the shape's positions, hides the checks that have a subject when collapsed
//! (their subject's footer shows the aggregate), paints a sequence badge on
//! the cards and can place the camera by hand on the board's first two rows.
//! Every wire, ordering wires included, goes through the real router. With the
//! switch off the board behaves as on `main`. Nothing here is product code.
#![allow(dead_code)]

use super::Camera;
use super::layout::{Card, CardDetail, WIDTH};
use crate::theme;
use apiforge_core::{
    format::{Graph, Node, Project},
    graph::dependency_index,
    model::{Branch, NodeKind},
};
use eframe::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Tidy up's dependency columns wrapped into rows.
    B,
    /// A spine in execution order with checks and values hanging below.
    D,
}

#[derive(Clone, Default, Debug)]
pub struct Flags {
    pub shape: Option<Shape>,
    /// Columns per row: 4 for B and 5 for D at 1600 × 1000.
    pub wide: bool,
    /// Checks with a subject are hidden; the subject shows the aggregate.
    pub collapsed: bool,
    /// The camera at this zoom on the board's first two rows, not fitted.
    pub view: Option<f32>,
}

/// What the last fit measured, for the report.
#[derive(Clone, Copy, Default, Debug)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
    /// Cards and wires the collapse takes off the board.
    pub hidden_cards: usize,
    pub hidden_wires: usize,
}

thread_local! {
    static FLAGS: RefCell<Flags> = RefCell::new(Flags::default());
    static EXTRA: RefCell<Option<Rect>> = const { RefCell::new(None) };
    static ROWS: RefCell<Option<Rect>> = const { RefCell::new(None) };
    static METRICS: RefCell<Metrics> = RefCell::new(Metrics::default());
}

pub fn set(flags: Flags) {
    FLAGS.with(|cell| *cell.borrow_mut() = flags);
}

pub fn get() -> Flags {
    FLAGS.with(|cell| cell.borrow().clone())
}

pub fn metrics() -> Metrics {
    METRICS.with(|cell| *cell.borrow())
}

/// The board's bounds for fitting: the cards and the badges on them.
pub fn widen(bounds: Rect) -> Rect {
    EXTRA.with(|cell| cell.borrow().map_or(bounds, |extra| bounds.union(extra)))
}

/// Records the fit and returns the zoom to use: the unclamped one while the
/// mock-up is on, so that the whole board shows.
pub fn fitted(bounds: Rect, needed: f32, clamped: f32) -> f32 {
    if get().shape.is_none() {
        return clamped;
    }
    METRICS.with(|cell| {
        let mut metrics = cell.borrow_mut();
        metrics.width = bounds.width();
        metrics.height = bounds.height();
        metrics.zoom = needed;
    });
    needed
}

/// The hand-placed camera: zoom and pan that put the first two rows' top-left
/// corner near the focus area's, centred across when they are narrower.
pub fn placed(focus: Rect) -> Option<(f32, Vec2)> {
    let zoom = get().view?;
    let rows = ROWS.with(|cell| *cell.borrow())?;
    let margin = 40.0;
    let left = if rows.width() * zoom + 2.0 * margin < focus.width() {
        focus.center().x - rows.width() * zoom / 2.0
    } else {
        focus.left() + margin
    };
    let top = focus.top() + margin;
    Some((
        zoom,
        vec2(left - rows.left() * zoom, top - rows.top() * zoom),
    ))
}

const LAYER_GAP: f32 = 96.0;
const STACK_GAP: f32 = 24.0;
const ROW_GAP: f32 = 120.0;

/// What the shape adds to one frame of the board.
pub struct Plan {
    /// Sequence number and branch of each badged card.
    pub badges: HashMap<String, (usize, Option<Branch>)>,
    /// The aggregate of each subject whose checks are hidden.
    pub aggregates: HashMap<String, String>,
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

fn branch_of(graph: &Graph, id: &str) -> Option<Branch> {
    graph
        .edges
        .iter()
        .find(|edge| edge.target == id && edge.branch.is_some())
        .and_then(|edge| edge.branch)
}

fn on_spine(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Start | NodeKind::Request | NodeKind::Component | NodeKind::If
    )
}

// Collapse, copied from `mock/lay-00-cards`.

fn is_check(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Assert | NodeKind::Condition)
}

/// The request or component-call steps a check references directly.
fn subjects(graph: &Graph, node: &Node) -> Vec<String> {
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
fn checks_of(graph: &Graph) -> HashMap<String, Vec<String>> {
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

/// Takes the folded checks off a main board when collapsed.
pub fn hide(graph: &Graph, cards: &mut Vec<Card>) {
    let flags = get();
    if flags.shape.is_none()
        || !flags.collapsed
        || !graph.nodes.iter().any(|node| node.kind == NodeKind::Start)
    {
        return;
    }
    let hidden = folded(graph);
    cards.retain(|card| !hidden.contains(&card.id));
}

/// The graph without the folded checks, for laying out what is visible.
fn visible(graph: &Graph, hidden: &HashSet<String>) -> Graph {
    let mut result = graph.clone();
    result.nodes.retain(|node| !hidden.contains(&node.id));
    result
        .edges
        .retain(|edge| !hidden.contains(&edge.source) && !hidden.contains(&edge.target));
    result
}

/// Lays out the main board in the switched-on shape. `None` when the switch is
/// off or the board is not a scenario's main flow.
pub fn arrange(
    full: &Graph,
    reference: &Graph,
    project: &Project,
    parameters: &[String],
    cards: &mut [Card],
) -> Option<Plan> {
    let flags = get();
    let shape = flags.shape?;
    if !full.nodes.iter().any(|node| node.kind == NodeKind::Start) {
        EXTRA.with(|cell| *cell.borrow_mut() = None);
        ROWS.with(|cell| *cell.borrow_mut() = None);
        return None;
    }
    let hidden = if flags.collapsed {
        folded(full)
    } else {
        HashSet::new()
    };
    let shown = visible(full, &hidden);
    let hidden_wires = apiforge_core::graph::wires(full)
        .iter()
        .filter(|wire| hidden.contains(&wire.source) || hidden.contains(&wire.target))
        .count();
    METRICS.with(|cell| {
        let mut metrics = cell.borrow_mut();
        metrics.hidden_cards = hidden.len();
        metrics.hidden_wires = hidden_wires;
    });
    let graph = &shown;
    // Numbers and the spine's order come from the whole scenario: what the
    // engine runs does not change when checks are folded.
    let order: Vec<String> = sequence(full)
        .into_iter()
        .filter(|id| !hidden.contains(id))
        .collect();
    let seq: HashMap<String, usize> = sequence(full)
        .into_iter()
        .enumerate()
        .map(|(index, id)| (id, index + 1))
        .collect();
    let index_of: HashMap<String, usize> = cards
        .iter()
        .enumerate()
        .map(|(index, card)| (card.id.clone(), index))
        .collect();
    let heights: HashMap<String, f32> = cards
        .iter()
        .map(|card| (card.id.clone(), card.height()))
        .collect();
    let height = |id: &str| heights[id];

    let mut row_of: HashMap<String, usize> = HashMap::new();
    let mut badged: HashSet<String> = HashSet::new();
    let pitch = WIDTH + LAYER_GAP;
    match shape {
        Shape::B => {
            // Tidy up's columns over the visible graph.
            let tidied = crate::board_edit::Board {
                graph,
                reference,
                project,
                parameters,
            }
            .tidy_up(&[])
            .unwrap_or_else(|| graph.clone());
            let tidy: HashMap<&str, Pos2> = tidied
                .nodes
                .iter()
                .map(|node| {
                    (
                        node.id.as_str(),
                        pos2(node.position.x as f32, node.position.y as f32),
                    )
                })
                .collect();
            let xs: BTreeSet<i64> = tidy.values().map(|p| p.x.round() as i64).collect();
            let xs: Vec<i64> = xs.into_iter().collect();
            let column = |id: &str| xs.binary_search(&(tidy[id].x.round() as i64)).unwrap();
            let per = if flags.wide { 4 } else { 3 };
            let rows = xs.len().div_ceil(per);
            let mut top = 0.0;
            for row in 0..rows {
                let members: Vec<String> = cards
                    .iter()
                    .filter(|card| column(&card.id) / per == row)
                    .map(|card| card.id.clone())
                    .collect();
                let min_y = members
                    .iter()
                    .map(|id| tidy[id.as_str()].y)
                    .fold(f32::INFINITY, f32::min);
                let mut bottom: f32 = 0.0;
                for id in &members {
                    let y = tidy[id.as_str()].y - min_y;
                    let x = (column(id) % per) as f32 * pitch;
                    cards[index_of[id]].origin = pos2(x, top + y);
                    bottom = bottom.max(y + height(id));
                    row_of.insert(id.clone(), row);
                    badged.insert(id.clone());
                }
                top += bottom + ROW_GAP;
            }
        }
        Shape::D => {
            let per = if flags.wide { 5 } else { 3 };
            let kind: HashMap<&str, NodeKind> = graph
                .nodes
                .iter()
                .map(|node| (node.id.as_str(), node.kind))
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
            // Satellites in sequence order hang under the latest spine step
            // they read directly, else under their sources' columns.
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
            let rows = spine.len().div_ceil(per);
            let mut top = 0.0;
            for row in 0..rows {
                let mut bottom: f32 = 0.0;
                for (i, stack) in stacks.iter().enumerate().filter(|(i, _)| i / per == row) {
                    let x = (i % per) as f32 * pitch;
                    let mut y = 0.0;
                    for id in stack {
                        cards[index_of[id]].origin = pos2(x, top + y);
                        y += height(id) + STACK_GAP;
                        row_of.insert(id.clone(), row);
                    }
                    bottom = bottom.max(y - STACK_GAP);
                }
                top += bottom + ROW_GAP;
            }
            badged.extend(spine);
        }
    }

    let badges = badged
        .into_iter()
        .map(|id| {
            let number = seq[&id];
            let branch = branch_of(full, &id);
            (id, (number, branch))
        })
        .collect();
    let aggregates = if flags.collapsed {
        checks_of(full)
            .into_iter()
            .map(|(subject, checks)| {
                let text = if checks.len() == 1 {
                    "✓ 1 check".to_owned()
                } else {
                    format!("✓ {} checks", checks.len())
                };
                (subject, text)
            })
            .collect()
    } else {
        HashMap::new()
    };
    // The badges stick out of the cards' top-left corners a little.
    let extra = cards
        .iter()
        .map(|card| Rect::from_center_size(card.origin, vec2(16.0, 16.0)))
        .reduce(Rect::union);
    EXTRA.with(|cell| *cell.borrow_mut() = extra);
    let rows = cards
        .iter()
        .filter(|card| row_of.get(&card.id).is_some_and(|row| *row <= 1))
        .map(Card::rect)
        .reduce(Rect::union);
    ROWS.with(|cell| *cell.borrow_mut() = rows);
    Some(Plan { badges, aggregates })
}

/// The sequence badge and the checks' aggregate of one card.
pub fn paint_extras(painter: &Painter, plan: Option<&Plan>, card: &Card, camera: Camera) {
    let Some(plan) = plan else { return };
    let z = camera.zoom;
    // Below 30 % the card's name is all there is.
    if z < 0.3 {
        return;
    }
    let rect = camera.rect(card.rect());
    if let Some(text) = plan.aggregates.get(&card.id) {
        paint_aggregate(painter, card, rect, text, camera);
    }
    let Some((number, branch)) = plan.badges.get(&card.id) else {
        return;
    };
    // At most 14 px and a third of the card's height on screen.
    let diameter = 14.0_f32.min(rect.height() / 3.0);
    let radius = diameter / 2.0;
    // Astride the top-left corner, clear of the icon at (16, 16).
    let center = rect.left_top() + vec2(2.0, 2.0);
    painter.circle(
        center,
        radius,
        theme::ACCENT,
        Stroke::new(1.0, Color32::WHITE),
    );
    painter.text(
        center,
        Align2::CENTER_CENTER,
        number.to_string(),
        FontId::new(
            diameter * if *number >= 10 { 0.6 } else { 0.7 },
            theme::bold_family(),
        ),
        Color32::WHITE,
    );
    if let Some(branch) = branch {
        let (text, color) = match branch {
            Branch::True => ("yes", theme::BRANCH_YES),
            Branch::False => ("no", theme::BRANCH_NO),
        };
        let font = FontId::proportional(diameter * 0.7);
        let galley = painter.layout_no_wrap(text.to_owned(), font, Color32::WHITE);
        let at = center + vec2(radius + 3.0, 0.0);
        let pill = Rect::from_min_size(
            at - vec2(0.0, diameter / 2.0),
            vec2(galley.size().x + 8.0, diameter),
        );
        painter.rect_filled(pill, pill.height() / 2.0, color);
        painter.galley(pill.center() - galley.size() / 2.0, galley, Color32::WHITE);
    }
}

/// The aggregate: right-aligned in the footer at Compact, a pill under the
/// name at Overview.
fn paint_aggregate(painter: &Painter, card: &Card, rect: Rect, text: &str, camera: Camera) {
    let z = camera.zoom;
    let clipped = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
    let color = theme::SUCCESS;
    let size = (11.0 * z).max(9.0);
    let galley = clipped.layout_no_wrap(text.to_owned(), FontId::proportional(size), color);
    match CardDetail::at_zoom(z) {
        CardDetail::Overview => {
            let name = (16.0 * z).max(7.5);
            let center = rect.center() + vec2(0.0, name * 0.9 + size);
            let pill =
                Rect::from_center_size(center, galley.size() + vec2(size * 1.1, size * 0.45));
            clipped.rect(
                pill,
                pill.height() / 2.0,
                color.gamma_multiply(0.10),
                Stroke::new(0.75, color.gamma_multiply(0.45)),
                StrokeKind::Inside,
            );
            clipped.galley(pill.center() - galley.size() / 2.0, galley, color);
        }
        CardDetail::Compact | CardDetail::Full => {
            let footer = camera.rect(card.footer());
            let at = pos2(
                footer.right() - 16.0 * z - galley.size().x,
                footer.center().y - galley.size().y / 2.0,
            );
            clipped.galley(at, galley, color);
        }
    }
}

#[cfg(test)]
mod review {
    use super::*;
    use crate::folder::Mode;
    use crate::testing::{WIDE, capture, folder_harness_in, settle, shop_project};
    use egui_kittest::kittest::Queryable;

    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay")
            .join(format!("{name}.afs"));
        std::fs::read_to_string(path).unwrap()
    }

    /// The board's size and fit, appended next to the build, not the captures.
    fn note(line: &str) {
        let Some(target) = std::env::var_os("CARGO_TARGET_DIR") else {
            return;
        };
        let path = std::path::PathBuf::from(target).join("lay00r2-metrics.txt");
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
    }

    /// One shape and state on one scenario at 1600 × 1000: measured at fit,
    /// captured on rows 1–2 at the given zooms, and at fit when asked.
    fn view(shape: Shape, scenario: &str, collapsed: bool, zooms: &[u32], fit: bool) {
        let letter = match shape {
            Shape::B => "b",
            Shape::D => "d",
        };
        let state = if collapsed { "collapsed" } else { "expanded" };
        let flags = Flags {
            shape: Some(shape),
            wide: true,
            collapsed,
            view: None,
        };
        set(flags.clone());
        let project = shop_project();
        // The scenario replaces the demo's, so it is the registered file the
        // window opens.
        std::fs::write(project.path().join(SCENARIO), fixture(scenario)).unwrap();
        let mut harness = folder_harness_in(project.path(), WIDE, Mode::Diagram);
        harness
            .state_mut()
            .as_mut()
            .unwrap()
            .review_open(SCENARIO, None);
        settle(&mut harness);
        if let Some(button) = harness.query_by_label("Fit the view") {
            button.click();
            settle(&mut harness);
        }
        let metrics = metrics();
        note(&format!(
            "{letter} {scenario} {state}: board {:.0} x {:.0} px, fit zoom {:.3}, hidden {} cards {} wires",
            metrics.width, metrics.height, metrics.zoom, metrics.hidden_cards, metrics.hidden_wires
        ));
        let name = format!("lay00r2-{letter}-{scenario}-{state}");
        if fit {
            capture(&mut harness, &format!("{name}-fit"), None);
        }
        for zoom in zooms {
            set(Flags {
                view: Some(*zoom as f32 / 100.0),
                ..flags.clone()
            });
            harness.step();
            settle(&mut harness);
            capture(&mut harness, &format!("{name}-{zoom}"), None);
        }
        set(Flags::default());
    }

    #[test]
    #[ignore = "design-review view"]
    fn review_lay00r2_d_shop_30_expanded() {
        view(Shape::D, "shop-30", false, &[60, 35], false);
    }

    #[test]
    #[ignore = "design-review view"]
    fn review_lay00r2_d_shop_30_collapsed() {
        view(Shape::D, "shop-30", true, &[60, 35], true);
    }

    #[test]
    #[ignore = "design-review view"]
    fn review_lay00r2_b_shop_30_expanded() {
        view(Shape::B, "shop-30", false, &[60, 35], false);
    }

    #[test]
    #[ignore = "design-review view"]
    fn review_lay00r2_b_shop_30_collapsed() {
        view(Shape::B, "shop-30", true, &[60, 35], true);
    }

    #[test]
    #[ignore = "design-review view"]
    fn review_lay00r2_d_order_flow_collapsed() {
        view(Shape::D, "order-flow", true, &[60], false);
    }
}
