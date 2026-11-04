//! LAY-00 mock-up (throwaway branch `mock/lay-00-shapes`): four shapes of a
//! tidied scenario board, rendered by the `review_lay00_*` views below.
//!
//! A test-only switch replaces the positions of the main board's cards with
//! the shape's positions, draws ordering wires on a rail above each row
//! (shapes A and D) and paints a sequence badge on the cards. With the
//! switch off the board behaves as on `main`. Nothing here is product code.
#![allow(dead_code)]

use super::Camera;
use super::layout::{Card, WIDTH};
use super::paint;
use super::routing::{BoardWire, Routed};
use crate::theme;
use apiforge_core::{
    format::{Graph, Project},
    graph::dependency_index,
    model::{Branch, NodeKind},
};
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, pos2, vec2};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Columns by dependency layer, one row, ordering wires on a rail above.
    A,
    /// The same columns wrapped into rows.
    B,
    /// Layers top to bottom.
    C,
    /// A spine in execution order with checks and values hanging below.
    D,
}

#[derive(Clone, Default, Debug)]
pub struct Flags {
    pub shape: Option<Shape>,
    /// 1600 × 1000 rather than 960 × 540: wraps later.
    pub wide: bool,
    /// Show this card at 100 % in the middle of the board instead of fitting.
    pub focus: Option<String>,
}

/// What the last frame laid out, for the report.
#[derive(Clone, Copy, Default, Debug)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
}

thread_local! {
    static FLAGS: RefCell<Flags> = RefCell::new(Flags::default());
    static EXTRA: RefCell<Option<Rect>> = const { RefCell::new(None) };
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

/// The board's bounds for fitting: the cards and the rails above them.
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
        *cell.borrow_mut() = Metrics {
            width: bounds.width(),
            height: bounds.height(),
            zoom: needed,
        }
    });
    needed
}

const LAYER_GAP: f32 = 96.0;
const STACK_GAP: f32 = 24.0;
const ROW_GAP: f32 = 120.0;
const RAIL: f32 = 40.0;
const LANE: f32 = 10.0;

/// A rail wire: its points in board coordinates.
pub struct RailWire {
    pub points: Vec<Pos2>,
    pub branch: Option<Branch>,
}

/// What the shape adds to one frame of the board.
pub struct Plan {
    /// Sequence number and branch of each badged card.
    pub badges: HashMap<String, (usize, Option<Branch>)>,
    pub rail: bool,
    pub rails: Vec<RailWire>,
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

/// Lays out the main board in the switched-on shape. `None` when the switch is
/// off or the board is not a scenario's main flow.
pub fn arrange(
    graph: &Graph,
    reference: &Graph,
    project: &Project,
    parameters: &[String],
    cards: &mut [Card],
) -> Option<Plan> {
    let flags = get();
    let shape = flags.shape?;
    if !graph.nodes.iter().any(|node| node.kind == NodeKind::Start) {
        EXTRA.with(|cell| *cell.borrow_mut() = None);
        return None;
    }
    let order = sequence(graph);
    let seq: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index + 1))
        .collect();
    let index_of: HashMap<String, usize> = cards
        .iter()
        .enumerate()
        .map(|(index, card)| (card.id.clone(), index))
        .collect();

    // Today's Tidy up gives the columns.
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
    let heights: HashMap<String, f32> = cards
        .iter()
        .map(|card| (card.id.clone(), card.height()))
        .collect();
    let height = |id: &str| heights[id];

    // Row of each card and the rows' tops, for the rails.
    let mut row_of: HashMap<String, usize> = HashMap::new();
    let mut badged: HashSet<String> = HashSet::new();
    let pitch = WIDTH + LAYER_GAP;
    match shape {
        Shape::A => {
            for card in cards.iter_mut() {
                card.origin = tidy[card.id.as_str()];
                row_of.insert(card.id.clone(), 0);
                badged.insert(card.id.clone());
            }
        }
        Shape::B => {
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
        Shape::C => {
            let gap = LAYER_GAP;
            let mut layers: Vec<Vec<String>> = vec![Vec::new(); xs.len()];
            for card in cards.iter() {
                layers[column(&card.id)].push(card.id.clone());
            }
            for layer in &mut layers {
                layer.sort_by(|a, b| tidy[a.as_str()].y.total_cmp(&tidy[b.as_str()].y));
            }
            let widest = layers.iter().map(Vec::len).max().unwrap_or(1) as f32;
            let full = widest * WIDTH + (widest - 1.0) * gap;
            let mut top = 0.0;
            for (row, layer) in layers.iter().enumerate() {
                let n = layer.len() as f32;
                let left = (full - (n * WIDTH + (n - 1.0) * gap)) / 2.0;
                let mut bottom: f32 = 0.0;
                for (i, id) in layer.iter().enumerate() {
                    cards[index_of[id]].origin = pos2(left + i as f32 * (WIDTH + gap), top);
                    bottom = bottom.max(height(id));
                    row_of.insert(id.clone(), row);
                    badged.insert(id.clone());
                }
                top += bottom + LAYER_GAP;
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
            let number = seq[id.as_str()];
            let branch = branch_of(graph, &id);
            (id, (number, branch))
        })
        .collect();
    let rail = matches!(shape, Shape::A | Shape::D);
    let rails = if rail {
        rails(graph, cards, &row_of)
    } else {
        Vec::new()
    };
    let extra = rails
        .iter()
        .flat_map(|wire| wire.points.iter().copied())
        .fold(None::<Rect>, |acc, point| {
            let rect = Rect::from_center_size(point, vec2(1.0, 1.0));
            Some(acc.map_or(rect, |acc| acc.union(rect)))
        });
    // The badges stick out of the cards' top-left corners.
    let badges_bounds = cards
        .iter()
        .map(|card| Rect::from_center_size(card.origin, vec2(24.0, 24.0)))
        .reduce(Rect::union);
    let extra = match (extra, badges_bounds) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, b) => a.or(b),
    };
    EXTRA.with(|cell| *cell.borrow_mut() = extra);
    Some(Plan {
        badges,
        rail,
        rails,
    })
}

/// Ordering wires along a rail above each row.
fn rails(graph: &Graph, cards: &[Card], row_of: &HashMap<String, usize>) -> Vec<RailWire> {
    let card = |id: &str| cards.iter().find(|card| card.id == id);
    let rows = row_of.values().copied().max().map_or(1, |max| max + 1);
    let row_top: Vec<f32> = (0..rows)
        .map(|row| {
            cards
                .iter()
                .filter(|card| row_of.get(&card.id) == Some(&row))
                .map(|card| card.origin.y)
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    let right_edge = cards
        .iter()
        .map(|card| card.rect().right())
        .fold(f32::NEG_INFINITY, f32::max);
    // Whether the top centre of a card can rise to its rail without crossing
    // another card.
    let clear_above = |rect: Rect, rail: f32| {
        let strip = Rect::from_min_max(
            pos2(rect.center().x - 2.0, rail),
            pos2(rect.center().x + 2.0, rect.top() - 1.0),
        );
        !cards
            .iter()
            .any(|other| other.rect() != rect && other.rect().intersects(strip))
    };
    // Spread the ends that share a card.
    let mut outs: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut ins: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        outs.entry(edge.source.as_str()).or_default().push(index);
        ins.entry(edge.target.as_str()).or_default().push(index);
    }
    let spread = |list: &[usize], index: usize| {
        let at = list.iter().position(|i| *i == index).unwrap_or(0) as f32;
        (at - (list.len() as f32 - 1.0) / 2.0) * 14.0
    };
    // Lanes: per row, intervals already taken on each lane.
    let mut taken: Vec<Vec<Vec<(f32, f32)>>> = vec![Vec::new(); rows];
    let mut free_lane = |spans: &[(usize, f32, f32)]| {
        let mut lane = 0;
        loop {
            let fits = spans.iter().all(|(row, a, b)| {
                taken[*row]
                    .get(lane)
                    .is_none_or(|list| list.iter().all(|(c, d)| *b + 8.0 < *c || *d + 8.0 < *a))
            });
            if fits {
                for (row, a, b) in spans {
                    while taken[*row].len() <= lane {
                        taken[*row].push(Vec::new());
                    }
                    taken[*row][lane].push((*a, *b));
                }
                return lane;
            }
            lane += 1;
        }
    };
    let mut result = Vec::new();
    let mut side = 0usize;
    for (index, edge) in graph.edges.iter().enumerate() {
        let (Some(source), Some(target)) = (card(&edge.source), card(&edge.target)) else {
            continue;
        };
        let (Some(&rs), Some(&rt)) = (row_of.get(&source.id), row_of.get(&target.id)) else {
            continue;
        };
        let s = source.rect();
        let t = target.rect();
        side += 1;
        let jog = 14.0 + (side % 4) as f32 * 7.0;
        // Where the wire leaves the source and reaches the rail column.
        let (s_start, sx) = if clear_above(s, row_top[rs] - RAIL - 40.0) {
            let x = s.center().x + spread(&outs[edge.source.as_str()], index);
            (vec![pos2(x, s.top())], x)
        } else {
            let x = s.right() + jog;
            let y = s.top() + 22.0;
            (vec![pos2(s.right(), y), pos2(x, y)], x)
        };
        let (t_end, tx) = if clear_above(t, row_top[rt] - RAIL - 40.0) {
            let x = t.center().x + spread(&ins[edge.target.as_str()], index);
            (vec![pos2(x, t.top())], x)
        } else {
            let x = t.left() - jog;
            let y = t.top() + 22.0;
            (vec![pos2(x, y), pos2(t.left(), y)], x)
        };
        let rail_y = |row: usize, lane: usize| row_top[row] - RAIL - lane as f32 * LANE;
        let mut points = s_start;
        if rs == rt {
            let lane = free_lane(&[(rs, sx.min(tx), sx.max(tx))]);
            let y = rail_y(rs, lane);
            points.push(pos2(sx, y));
            points.push(pos2(tx, y));
        } else {
            let lane = free_lane(&[(rs, sx, right_edge + 60.0), (rt, tx, right_edge + 60.0)]);
            let far = right_edge + 30.0 + lane as f32 * LANE;
            let (ys, yt) = (rail_y(rs, lane), rail_y(rt, lane));
            points.push(pos2(sx, ys));
            points.push(pos2(far, ys));
            points.push(pos2(far, yt));
            points.push(pos2(tx, yt));
        }
        points.extend(t_end);
        result.push(RailWire {
            points,
            branch: edge.branch,
        });
    }
    result
}

/// Takes the ordering wires off the router when they go on the rail.
pub fn router_wires(wires: Vec<BoardWire>, plan: Option<&Plan>) -> Vec<BoardWire> {
    match plan {
        Some(plan) if plan.rail => wires.into_iter().filter(|wire| !wire.control).collect(),
        _ => wires,
    }
}

pub fn paint_rails(painter: &Painter, plan: Option<&Plan>, camera: Camera) {
    let Some(plan) = plan else { return };
    for (index, wire) in plan.rails.iter().enumerate() {
        let point = |p: Pos2| apiforge_layout::Point::new(p.x, p.y);
        let pieces = wire
            .points
            .windows(2)
            .map(|pair| apiforge_layout::Piece::Line {
                from: point(pair[0]),
                to: point(pair[1]),
            })
            .collect();
        let routed = Routed {
            id: format!("lay00-rail-{index}"),
            points: wire.points.clone(),
            pieces,
            bridges: Vec::new(),
            control: true,
            branch: wire.branch,
            routed: true,
        };
        paint::routed_wire(painter, &routed, camera, false);
    }
}

/// The sequence badge: a circle with the number at the card's top-left corner.
pub fn paint_badge(painter: &Painter, plan: Option<&Plan>, card: &Card, camera: Camera) {
    let Some((number, branch)) = plan.and_then(|plan| plan.badges.get(&card.id)) else {
        return;
    };
    let radius = (10.0 * camera.zoom).max(8.0);
    let center = camera.to_screen(card.origin + vec2(2.0, 2.0));
    painter.circle(
        center,
        radius,
        theme::ACCENT,
        Stroke::new(1.5, Color32::WHITE),
    );
    painter.text(
        center,
        Align2::CENTER_CENTER,
        number.to_string(),
        FontId::proportional(radius * if *number >= 10 { 0.95 } else { 1.15 }),
        Color32::WHITE,
    );
    if let Some(branch) = branch {
        let (text, color) = match branch {
            Branch::True => ("yes", theme::BRANCH_YES),
            Branch::False => ("no", theme::BRANCH_NO),
        };
        let font = FontId::proportional(radius * 1.0);
        let at = center + vec2(radius + 3.0, 0.0);
        let galley = painter.layout_no_wrap(text.to_owned(), font, Color32::WHITE);
        let rect = Rect::from_min_size(
            at - vec2(0.0, galley.size().y / 2.0 + 1.0),
            galley.size() + vec2(8.0, 2.0),
        );
        painter.rect_filled(rect, rect.height() / 2.0, color);
        painter.galley(rect.min + vec2(4.0, 1.0), galley, Color32::WHITE);
    }
}

#[cfg(test)]
mod review {
    use super::*;
    use crate::folder::Mode;
    use crate::testing::{MINIMUM, WIDE, capture, folder_harness_in, settle, shop_project};
    use egui_kittest::kittest::Queryable;

    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay")
            .join(format!("{name}.afs"));
        std::fs::read_to_string(path).unwrap()
    }

    fn note(line: &str) {
        let path = crate::testing::capture_dir().join("../lay00-metrics.txt");
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
    }

    /// One view: the scenario in the shop project, the shape on, fitted.
    fn view(
        shape: Shape,
        letter: &str,
        scenario: &str,
        size: eframe::egui::Vec2,
        crop: Option<&str>,
    ) {
        let wide = size == WIDE;
        set(Flags {
            shape: Some(shape),
            wide,
            focus: None,
        });
        let project = shop_project();
        // The scenario replaces the demo's, so it is the registered file the
        // window opens.
        std::fs::write(project.path().join(SCENARIO), fixture(scenario)).unwrap();
        let mut harness = folder_harness_in(project.path(), size, Mode::Diagram);
        harness
            .state_mut()
            .as_mut()
            .unwrap()
            .review_open(SCENARIO, None);
        settle(&mut harness);
        if let Some(fit) = harness.query_by_label("Fit the view") {
            fit.click();
            settle(&mut harness);
        }
        let suffix = if wide { "1600x1000" } else { "960x540" };
        let metrics = metrics();
        note(&format!(
            "{letter} {scenario} {suffix}: board {:.0} x {:.0} px, fit zoom {:.3}",
            metrics.width, metrics.height, metrics.zoom
        ));
        capture(
            &mut harness,
            &format!("lay00-{letter}-{scenario}-{suffix}"),
            None,
        );
        if let Some(check) = crop {
            set(Flags {
                shape: Some(shape),
                wide,
                focus: Some(check.to_owned()),
            });
            harness.step();
            settle(&mut harness);
            let board = harness.get_by_label("Diagram board").rect();
            let center = board.center();
            let region = Rect::from_center_size(center, vec2(1300.0, 760.0)).intersect(board);
            capture(
                &mut harness,
                &format!("lay00-{letter}-{scenario}-crop"),
                Some(region),
            );
        }
        set(Flags::default());
    }

    macro_rules! views {
        ($($name:ident: $shape:expr, $letter:literal, $scenario:literal, $size:expr, $crop:expr;)*) => {
            $(
                #[test]
                #[ignore = "design review view (LAY-00 mock-up); run with tools/gui/review.sh"]
                fn $name() {
                    view($shape, $letter, $scenario, $size, $crop);
                }
            )*
        };
    }

    views! {
        review_lay00_a_order_flow_1600x1000: Shape::A, "a", "order-flow", WIDE, None;
        review_lay00_a_order_flow_960x540: Shape::A, "a", "order-flow", MINIMUM, None;
        review_lay00_a_shop_30_1600x1000: Shape::A, "a", "shop-30", WIDE, Some("cart_total");
        review_lay00_a_shop_30_960x540: Shape::A, "a", "shop-30", MINIMUM, None;
        review_lay00_b_order_flow_1600x1000: Shape::B, "b", "order-flow", WIDE, None;
        review_lay00_b_order_flow_960x540: Shape::B, "b", "order-flow", MINIMUM, None;
        review_lay00_b_shop_30_1600x1000: Shape::B, "b", "shop-30", WIDE, Some("cart_total");
        review_lay00_b_shop_30_960x540: Shape::B, "b", "shop-30", MINIMUM, None;
        review_lay00_b_shop_50_1600x1000: Shape::B, "b", "shop-50", WIDE, None;
        review_lay00_c_order_flow_1600x1000: Shape::C, "c", "order-flow", WIDE, None;
        review_lay00_c_order_flow_960x540: Shape::C, "c", "order-flow", MINIMUM, None;
        review_lay00_c_shop_30_1600x1000: Shape::C, "c", "shop-30", WIDE, Some("cart_total");
        review_lay00_c_shop_30_960x540: Shape::C, "c", "shop-30", MINIMUM, None;
        review_lay00_d_order_flow_1600x1000: Shape::D, "d", "order-flow", WIDE, None;
        review_lay00_d_order_flow_960x540: Shape::D, "d", "order-flow", MINIMUM, None;
        review_lay00_d_shop_30_1600x1000: Shape::D, "d", "shop-30", WIDE, Some("cart_total");
        review_lay00_d_shop_30_960x540: Shape::D, "d", "shop-30", MINIMUM, None;
        review_lay00_d_shop_50_1600x1000: Shape::D, "d", "shop-50", WIDE, None;
    }
}
