//! LAY-06 (#37): Tidy up gives shape D. The spine (Start, requests,
//! component calls, `if`s) runs in the engine's order left to right in rows
//! of five columns; every other card hangs under the spine step it reads
//! last; ordering wires run on the rail above the row; the last card of a
//! row names the next step. On the LAY-01 reference scenarios, written over
//! the demo shop's scenario without positions, collapsed (the default) and
//! expanded.
use super::*;
use crate::canvas::Command;
use apiforge_core::{
    format::{Graph, Node},
    graph::{check_subjects, sequence},
    model::NodeKind,
};
use std::collections::{BTreeMap, HashMap};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";
/// The board file `quality` reads (criterion 10).
const BOARDS: &str = "../apiforge-layout/examples/support/boards.txt";
/// Writing it again instead of comparing: `APIFORGE_WRITE_BOARDS=1`.
const WRITE: &str = "APIFORGE_WRITE_BOARDS";
const REFERENCE: [&str; 3] = ["order-flow", "shop-30", "shop-50"];

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// How the board shows its checks.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Checks {
    /// Folded into their subjects, the default (LAY-11).
    Collapsed,
    Expanded,
}

/// The reference scenario `name` opened at `size` without positions, its
/// checks as `checks` says, tidied up once more with nothing selected.
fn tidied(
    name: &str,
    size: egui::Vec2,
    checks: Checks,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), fixture(name)).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    if checks == Checks::Expanded {
        app(&mut harness).debug_draw_checks();
    }
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    harness.step();
    (project, harness, id)
}

fn main_graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

fn board<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

fn node<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
}

fn handle_of(graph: &Graph, id: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| id.to_owned(), |node| node.handle.clone())
}

/// Every drawn card's box in board coordinates, by handle.
fn world(harness: &FolderHarness, id: &str) -> BTreeMap<String, egui::Rect> {
    let canvas = board(harness, id);
    main_graph(harness, id)
        .nodes
        .iter()
        .filter_map(|node| {
            canvas
                .debug_world(&node.id)
                .map(|rect| (node.handle.clone(), rect))
        })
        .collect()
}

fn on_spine(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Start | NodeKind::Request | NodeKind::Component | NodeKind::If
    )
}

/// The spine of `graph` by handle, in the engine's order.
fn spine(graph: &Graph) -> Vec<String> {
    sequence(graph)
        .into_iter()
        .map(|id| graph.nodes.iter().find(|node| node.id == id).unwrap())
        .filter(|node| on_spine(node.kind))
        .map(|node| node.handle.clone())
        .collect()
}

/// The free space Tidy up leaves between two rows, which holds the rail.
fn row_gap() -> f32 {
    apiforge_layout::Rows::default().row_gap
}

/// What `shop-30`'s satellites hang under: each check under the subject
/// the engine reaches last, a value under the latest step it reads.
const SHOP_30_SATELLITES: [(&str, &str); 15] = [
    ("logged_in", "login"),
    ("known_user", "login"),
    ("listed", "catalogue"),
    ("first", "catalogue"),
    ("second", "catalogue"),
    ("expected_total", "promoted"),
    ("cart_created", "cart"),
    ("cart_total", "cart"),
    ("valid_cart", "cart_view"),
    ("checkout_total", "checkout"),
    ("checkout_status", "checkout"),
    ("checkout_created", "checkout"),
    ("receipt_total", "receipt"),
    ("not_created", "paid"),
    ("refund_amount", "refund"),
];

/// Criterion 1: on `shop-30` the spine runs in the engine's order left to
/// right in rows of five 280 px columns, every satellite stands in the
/// column of its latest subject below it, and the board, marks included,
/// is at most 1920 px wide.
#[test]
fn lay06_criterion_1_the_spine_and_the_satellites() {
    let (_project, harness, id) = tidied("shop-30", WIDE, Checks::Expanded);
    let graph = main_graph(&harness, &id);
    let cards = world(&harness, &id);
    let spine = spine(&graph);
    assert!(spine.len() > 10, "shop-30 has a long spine: {spine:?}");
    let rect = |handle: &str| {
        *cards
            .get(handle)
            .unwrap_or_else(|| panic!("{handle} is drawn"))
    };
    let first = rect(&spine[0]);
    for (place, handle) in spine.iter().enumerate() {
        let (row, column) = (place / 5, place % 5);
        let card = rect(handle);
        assert_eq!(card.width(), 280.0, "{handle} is a 280 px column");
        assert_eq!(
            card.left(),
            first.left() + column as f32 * (280.0 + 96.0),
            "{handle} is step {place} of the spine, in column {column}"
        );
        let leader = rect(&spine[row * 5]);
        assert_eq!(card.top(), leader.top(), "{handle} is in row {row}");
        if row > 0 {
            let above = rect(&spine[(row - 1) * 5]);
            assert!(
                card.top() > above.bottom(),
                "row {row} is below row {}",
                row - 1
            );
        }
    }
    let satellites: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| !on_spine(node.kind))
        .map(|node| node.handle.as_str())
        .collect();
    assert_eq!(
        satellites.len(),
        SHOP_30_SATELLITES.len(),
        "every other card is a satellite: {satellites:?}"
    );
    for (satellite, subject) in SHOP_30_SATELLITES {
        let (card, under) = (rect(satellite), rect(subject));
        assert_eq!(
            card.left(),
            under.left(),
            "{satellite} is in the column of {subject}"
        );
        assert!(
            card.top() > under.bottom(),
            "{satellite} at {card:?} hangs below {subject} at {under:?}"
        );
    }
    // In each column the satellites keep the engine's order downwards.
    let order: HashMap<String, usize> = sequence(&graph)
        .into_iter()
        .enumerate()
        .map(|(place, id)| (handle_of(&graph, id), place))
        .collect();
    for (a, under_a) in SHOP_30_SATELLITES {
        for (b, under_b) in SHOP_30_SATELLITES {
            if under_a == under_b && order[a] < order[b] {
                assert!(rect(a).top() < rect(b).top(), "{a} stands above {b}");
            }
        }
    }
    let marks = board(&harness, &id).debug_marks();
    let bounds = cards
        .values()
        .copied()
        .chain(marks.iter().map(|mark| mark.rect))
        .reduce(egui::Rect::union)
        .unwrap();
    assert!(
        bounds.width() <= 1920.0,
        "the board is {} px wide",
        bounds.width()
    );
}

/// Criterion 2: a row is as tall as its tallest stack, a spine card and
/// the satellites under it at the heights the board draws them, and the
/// next row starts one row gap below it.
#[test]
fn lay06_criterion_2_row_heights() {
    for name in ["order-flow", "shop-30", "shop-50"] {
        let (_project, harness, id) = tidied(name, WIDE, Checks::Expanded);
        let graph = main_graph(&harness, &id);
        let cards = world(&harness, &id);
        let spine = spine(&graph);
        let rows: Vec<&[String]> = spine.chunks(5).collect();
        assert!(rows.len() > 1, "{name} wraps");
        let mut heights = Vec::new();
        for row in &rows {
            let top = cards[&row[0]].top();
            let columns: Vec<f32> = row.iter().map(|handle| cards[handle].left()).collect();
            // A stack: every card in the column between this row and the next.
            let next = rows
                .iter()
                .skip_while(|other| other[0] != row[0])
                .nth(1)
                .map_or(f32::INFINITY, |next| cards[&next[0]].top());
            let height = columns
                .iter()
                .map(|left| {
                    cards
                        .values()
                        .filter(|card| {
                            card.left() == *left && card.top() >= top && card.top() < next
                        })
                        .map(|card| card.bottom() - top)
                        .fold(0.0, f32::max)
                })
                .fold(0.0, f32::max);
            heights.push((top, height, next));
        }
        for (top, height, next) in &heights[..heights.len() - 1] {
            assert_eq!(
                next - top,
                height + row_gap(),
                "{name}: the row at {top} is {height} tall and the next starts at {next}"
            );
        }
        // The heights are the drawn ones: a value card is shorter than a
        // request card at the compact density.
        let project = folder(&harness).context_project();
        for (handle, card) in &cards {
            let node = node(&graph, handle);
            let drawn = crate::canvas::Card::build(
                node,
                &graph,
                &project,
                crate::canvas::CardDensity::Compact,
            )
            .rect()
            .height();
            assert_eq!(card.height(), drawn, "{name}: {handle} at its drawn height");
        }
    }
}

/// Criterion 3: `order-flow` (14 steps) fits 1600 × 1000 at 0.45 or more.
#[test]
fn lay06_criterion_3_fit_for_14_steps() {
    let (_project, mut harness, id) = tidied("order-flow", WIDE, Checks::Collapsed);
    choose(&mut harness, FIT);
    let zoom = board(&harness, &id).debug_zoom();
    assert!(zoom >= 0.45, "order-flow fits at {zoom}");
}

/// The rows of a tidied board: each spine card's handle with the top and
/// bottom of its row, the tallest card of the row taken.
fn rows_of(graph: &Graph, cards: &BTreeMap<String, egui::Rect>) -> HashMap<String, (f32, f32)> {
    let spine = spine(graph);
    let mut rows = HashMap::new();
    for row in spine.chunks(5) {
        let top = cards[&row[0]].top();
        let bottom = row
            .iter()
            .map(|handle| cards[handle].bottom())
            .fold(top, f32::max);
        for handle in row {
            rows.insert(handle.clone(), (top, bottom));
        }
    }
    rows
}

/// Criterion 4: no ordering wire runs through the gap between two cards of
/// a row, where the data wires run: an `@after` or an `if`'s branch between
/// two spine steps of a row rises from the top of its source to the rail
/// above the row and drops into the top of its target, every point of it
/// at or above the row's top. Every ordering wire is dashed, and one that
/// reaches a spine card arrives from above its row. Only a wire into a card
/// hung right under its source drops straight into it.
#[test]
fn lay06_criterion_4_order_wires_on_the_rail() {
    for name in ["order-flow", "shop-30"] {
        for checks in [Checks::Collapsed, Checks::Expanded] {
            let (_project, harness, id) = tidied(name, WIDE, checks);
            let graph = main_graph(&harness, &id);
            let cards = world(&harness, &id);
            let rows = rows_of(&graph, &cards);
            let edges: Vec<&str> = graph.edges.iter().map(|edge| edge.id.as_str()).collect();
            let mut railed = 0;
            for wire in board(&harness, &id).debug_wires() {
                let ordering = edges.contains(&wire.id.as_str());
                assert_eq!(
                    wire.dashed, ordering,
                    "{name} {checks:?}: {} is dashed exactly when it orders",
                    wire.id
                );
                if !ordering {
                    continue;
                }
                let source = handle_of(&graph, wire.source.as_deref().unwrap());
                let target = handle_of(&graph, &wire.target);
                let (from, to) = (cards[&source], cards[&target]);
                assert!(wire.points.len() >= 2, "{source} -> {target} is routed");
                let under = to.top() >= from.bottom() && to.left() == from.left();
                if under {
                    continue;
                }
                let context = format!("{name} {checks:?}: {source} -> {target} {:?}", wire.points);
                if let Some(&(top, _)) = rows.get(&target) {
                    // The last bend before the target is on the rail.
                    let rail = wire.points[wire.points.len() - 2];
                    assert!(rail.y < top, "{context} arrives from the rail above");
                }
                if let (Some(&(top, _)), Some(&(other, _))) = (rows.get(&source), rows.get(&target))
                    && top == other
                {
                    railed += 1;
                    assert!(
                        wire.points.iter().all(|point| point.y <= top + 0.5),
                        "{context} stays above its row"
                    );
                    assert!(
                        wire.points.iter().any(|point| point.y < top - 0.5),
                        "{context} runs on the rail"
                    );
                }
            }
            assert!(railed > 0, "{name} {checks:?} has ordering wires on a rail");
        }
    }
}

/// Criterion 5: the last card of each row but the last carries a mark
/// that names the first card of the next row, the next step in order.
#[test]
fn lay06_criterion_5_the_continuation_mark() {
    for name in ["order-flow", "shop-30", "shop-50"] {
        for checks in [Checks::Collapsed, Checks::Expanded] {
            let (_project, harness, id) = tidied(name, WIDE, checks);
            let graph = main_graph(&harness, &id);
            let cards = world(&harness, &id);
            let spine = spine(&graph);
            let expected: Vec<(String, String)> = spine
                .chunks(5)
                .zip(spine.chunks(5).skip(1))
                .map(|(row, next)| (row[row.len() - 1].clone(), next[0].clone()))
                .collect();
            let marks = board(&harness, &id).debug_marks();
            let pairs: Vec<(String, String)> = marks
                .iter()
                .map(|mark| (handle_of(&graph, &mark.last), handle_of(&graph, &mark.next)))
                .collect();
            assert_eq!(pairs, expected, "{name} {checks:?}");
            for (mark, (last, next)) in marks.iter().zip(&expected) {
                let card = cards[last];
                assert!(
                    mark.rect.left() > card.right() && mark.rect.top() >= card.top(),
                    "{name}: the mark of {last} at {:?} is beside it at {card:?}",
                    mark.rect
                );
                assert!(
                    mark.rect.bottom() <= card.bottom(),
                    "{name}: the mark of {last} is level with it"
                );
                let name_of_next = node(&graph, next).display_name().to_owned();
                assert!(
                    mark.text.contains(&name_of_next),
                    "{name}: the mark of {last} reads {:?}, not {name_of_next}",
                    mark.text
                );
            }
        }
    }
}

/// Criterion 6: two Tidy ups of the same scenario are equal, in one window
/// and in two.
#[test]
fn lay06_criterion_6_deterministic() {
    for name in REFERENCE {
        let (_project, mut harness, id) = tidied(name, WIDE, Checks::Collapsed);
        let first = main_graph(&harness, &id);
        choose(&mut harness, TIDY);
        let second = main_graph(&harness, &id);
        let (_other, again, other_id) = tidied(name, WIDE, Checks::Collapsed);
        let third = main_graph(&again, &other_id);
        let positions = |graph: &Graph| -> BTreeMap<String, (i64, i64)> {
            graph
                .nodes
                .iter()
                .map(|node| {
                    (
                        node.handle.clone(),
                        (
                            node.position.x.round() as i64,
                            node.position.y.round() as i64,
                        ),
                    )
                })
                .collect()
        };
        assert_eq!(
            positions(&first),
            positions(&second),
            "{name}: Tidy up twice"
        );
        assert_eq!(
            positions(&first),
            positions(&third),
            "{name}: in two windows"
        );
    }
}

/// Criteria 8 and 12: `shop-30` with its checks collapsed, as LAY-11's
/// preference folds them by default, fits 1600 × 1000 at 0.50 or more.
/// Expanded, the fit is not asserted (0.49 when written); the quality
/// example reports it.
#[test]
fn lay06_criterion_8_fit_at_least_half_collapsed() {
    let (_project, mut harness, id) = tidied("shop-30", WIDE, Checks::Collapsed);
    assert!(
        folder(&harness).preferences.checks_collapsed,
        "the checks are collapsed by LAY-11's preference"
    );
    assert!(
        !board(&harness, &id).debug_hidden().is_empty(),
        "the board folds checks"
    );
    choose(&mut harness, FIT);
    let collapsed = board(&harness, &id).debug_zoom();
    assert!(collapsed >= 0.50, "shop-30 collapsed fits at {collapsed}");
}

/// Criterion 9: Tidy up ends with the board in view. The camera is moved
/// to an empty part of the board first; after Tidy up a card is in the
/// board's view and the zoom is the one Fit gives.
#[test]
fn lay06_criterion_9_tidy_up_ends_in_view() {
    for size in [WIDE, crate::testing::MINIMUM] {
        let (_project, mut harness, id) = tidied("shop-30", size, Checks::Collapsed);
        let graph = main_graph(&harness, &id);
        let begin = node(&graph, "begin").id.clone();
        let document = id.clone();
        let canvas = app(&mut harness).canvases.get_mut(&document).unwrap();
        canvas.debug_show(&begin, 1.0);
        canvas.debug_pan(egui::vec2(20_000.0, 20_000.0));
        settle(&mut harness);
        harness.step();
        let away = board(&harness, &id);
        assert!(
            !away
                .debug_cards()
                .iter()
                .any(|(_, rect)| away.debug_focus().intersects(*rect)),
            "the camera looks at an empty board"
        );
        choose(&mut harness, TIDY);
        harness.step();
        let after = board(&harness, &id);
        let focus = after.debug_focus();
        assert!(
            after
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{size:?}: after Tidy up a card is in view"
        );
        let zoom = after.debug_zoom();
        choose(&mut harness, FIT);
        let fitted = board(&harness, &id).debug_zoom();
        assert!(
            (zoom - fitted).abs() < 1e-3,
            "{size:?}: Tidy up ends at {zoom}, Fit at {fitted}"
        );
    }
}

fn side(side: apiforge_layout::Side) -> &'static str {
    match side {
        apiforge_layout::Side::Left => "left",
        apiforge_layout::Side::Right => "right",
        apiforge_layout::Side::Top => "top",
        apiforge_layout::Side::Bottom => "bottom",
    }
}

/// The board file of criterion 10: each reference scenario tidied at both
/// densities, with its checks collapsed and expanded, as the board gives it
/// to the router.
fn board_file() -> String {
    let mut text = String::from(
        "# The reference scenarios of fixtures/scenarios/lay/ as the desktop tidies them\n\
         # (LAY-06): each card and chip at the size the board draws it, each wire as the\n\
         # ports the router is given, and the continuation marks. The quality example\n\
         # routes and measures these. Written by the desktop's acceptance test\n\
         # lay06_criterion_10_quality_on_real_card_sizes with APIFORGE_WRITE_BOARDS=1;\n\
         # do not edit by hand.\n",
    );
    for name in REFERENCE {
        for density in ["compact", "full"] {
            for checks in [Checks::Collapsed, Checks::Expanded] {
                let (_project, mut harness, id) = tidied(name, WIDE, checks);
                if density == "full" {
                    app(&mut harness).active_board_command(Command::SetCompactCards(false));
                    settle(&mut harness);
                    choose(&mut harness, TIDY);
                    harness.step();
                }
                let graph = main_graph(&harness, &id);
                let canvas = board(&harness, &id);
                let (rectangles, wires) = canvas.debug_geometry();
                let names: Vec<String> = canvas
                    .debug_cards()
                    .iter()
                    .map(|(card, _)| handle_of(&graph, card))
                    .collect();
                let checks = match checks {
                    Checks::Collapsed => "collapsed",
                    Checks::Expanded => "expanded",
                };
                text.push_str(&format!(
                    "board {name} {density} {checks} {}\n",
                    graph.nodes.len()
                ));
                for (index, rect) in rectangles.iter().enumerate() {
                    let kind = if index < names.len() { "card" } else { "chip" };
                    text.push_str(&format!(
                        "{kind} {} {} {} {} {}\n",
                        rect.min.x,
                        rect.min.y,
                        rect.width(),
                        rect.height(),
                        names.get(index).map_or("-", String::as_str)
                    ));
                }
                for wire in wires {
                    text.push_str(&format!(
                        "wire {} {} {} {} {} {}\n",
                        wire.from.block,
                        side(wire.from.side),
                        wire.from.offset,
                        wire.to.block,
                        side(wire.to.side),
                        wire.to.offset
                    ));
                }
                for mark in canvas.debug_marks() {
                    text.push_str(&format!(
                        "mark {} {} {} {}\n",
                        mark.rect.min.x,
                        mark.rect.min.y,
                        mark.rect.width(),
                        mark.rect.height()
                    ));
                }
                text.push_str("end\n");
            }
        }
    }
    text
}

/// Criterion 10: `quality` measures the boards the desktop draws, at the
/// heights LAY-16 and LAY-21 give each card, compact and full: the file it
/// reads is what this window tidies and routes, and it is current.
#[test]
fn lay06_criterion_10_quality_on_real_card_sizes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BOARDS);
    let text = board_file();
    // The two densities differ, so the file holds the real sizes.
    let heights = |density: &str| -> Vec<String> {
        text.split("end\n")
            .filter(|board| board.contains(&format!("board shop-30 {density} expanded")))
            .flat_map(|board| board.lines().filter(|line| line.starts_with("card ")))
            .map(|line| line.split(' ').nth(4).unwrap().to_owned())
            .collect()
    };
    assert_ne!(
        heights("compact"),
        heights("full"),
        "compact cards are shorter"
    );
    if std::env::var_os(WRITE).is_some() {
        fs::write(&path, &text).unwrap();
    }
    let committed =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert!(
        committed == text,
        "{} is not what the desktop tidies; write it again with {WRITE}=1",
        path.display()
    );
}

/// Pairs of parallel segments of two wires within 0.5 px of each other for
/// at least 20 px, as `quality` counts them.
fn coincident(wires: &[&[egui::Pos2]]) -> usize {
    let mut pairs = 0;
    for (index, a) in wires.iter().enumerate() {
        for b in &wires[index + 1..] {
            for p in a.windows(2) {
                for q in b.windows(2) {
                    let horizontal = |s: &[egui::Pos2]| (s[0].y - s[1].y).abs() < 0.01;
                    let vertical = |s: &[egui::Pos2]| (s[0].x - s[1].x).abs() < 0.01;
                    let overlap = |a1: f32, a2: f32, b1: f32, b2: f32| {
                        a1.max(a2).min(b1.max(b2)) - a1.min(a2).max(b1.min(b2))
                    };
                    let (gap, length) = if horizontal(p) && horizontal(q) {
                        (
                            (p[0].y - q[0].y).abs(),
                            overlap(p[0].x, p[1].x, q[0].x, q[1].x),
                        )
                    } else if vertical(p) && vertical(q) {
                        (
                            (p[0].x - q[0].x).abs(),
                            overlap(p[0].y, p[1].y, q[0].y, q[1].y),
                        )
                    } else {
                        continue;
                    };
                    if gap < 0.5 && length >= 20.0 {
                        pairs += 1;
                    }
                }
            }
        }
    }
    pairs
}

/// Criterion 11: on `shop-30` Tidy up crosses no more wires per wire than
/// today's 2.05 (117 crossings on 57 wires), collapsed and expanded, and
/// leaves at most 97 coincident pairs on the board a new install shows,
/// with its checks collapsed. Expanded, the coincident pairs are not
/// asserted (112 when written; the quality example reports them): most are
/// wires leaving one port together, which LAY-03 separates, and the board
/// the previous Tidy up gives at the sizes the desktop draws has more of
/// them (123) than the 97 measured on full cards. The owner accepted this
/// and moved the expanded cap to LAY-03 (#34).
#[test]
fn lay06_criterion_11_no_more_crossings_than_today() {
    for checks in [Checks::Collapsed, Checks::Expanded] {
        let (_project, harness, id) = tidied("shop-30", WIDE, checks);
        let wires = board(&harness, &id).debug_wires();
        let crossings: usize = wires.iter().map(|wire| wire.bridges).sum();
        let per_wire = crossings as f32 / wires.len() as f32;
        let routes: Vec<&[egui::Pos2]> = wires.iter().map(|wire| wire.points.as_slice()).collect();
        let pairs = coincident(&routes);
        assert!(
            per_wire <= 2.05,
            "{checks:?}: {crossings} crossings on {} wires",
            wires.len()
        );
        if checks == Checks::Collapsed {
            assert!(pairs <= 97, "{checks:?}: {pairs} coincident pairs");
        }
    }
}

/// Criterion 13: every check with a subject hangs strictly under the
/// subject LAY-11 folds it into, in its column and below it, on every
/// reference scenario.
#[test]
fn lay06_criterion_13_satellites_hang_under_their_subject() {
    for name in REFERENCE {
        let (_project, harness, id) = tidied(name, WIDE, Checks::Expanded);
        let graph = main_graph(&harness, &id);
        let cards = world(&harness, &id);
        let folded = crate::canvas::collapsible(&graph, None);
        assert!(!folded.is_empty(), "{name} has checks to fold");
        for candidate in &folded {
            let (check, subject) = (
                handle_of(&graph, &candidate.check),
                handle_of(&graph, &candidate.anchor),
            );
            let (card, under) = (cards[&check], cards[&subject]);
            assert_eq!(
                card.left(),
                under.left(),
                "{name}: {check} in the column of {subject}"
            );
            assert!(
                card.top() > under.bottom(),
                "{name}: {check} below {subject}"
            );
            // Nothing of another column's spine stands between them.
            for (other, rect) in &cards {
                if on_spine(node(&graph, other).kind) && other != &subject {
                    assert!(
                        !(rect.left() == under.left()
                            && rect.top() > under.bottom()
                            && rect.top() < card.top()),
                        "{name}: {other} stands between {subject} and {check}"
                    );
                }
            }
        }
        // A gate hangs under the step it verifies too.
        for check in graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Condition)
        {
            let subjects = check_subjects(&graph, check).subjects;
            if let Some(subject) = subjects.last() {
                let subject = handle_of(&graph, subject.step);
                assert_eq!(
                    cards[&check.handle].left(),
                    cards[&subject].left(),
                    "{name}: {} in the column of {subject}",
                    check.handle
                );
            }
        }
    }
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay06() {
    for (name, size, checks, label) in [
        (
            "shop-30",
            WIDE,
            Checks::Collapsed,
            "lay06-shop-30-1600-collapsed",
        ),
        (
            "shop-30",
            WIDE,
            Checks::Expanded,
            "lay06-shop-30-1600-expanded",
        ),
        (
            "order-flow",
            WIDE,
            Checks::Collapsed,
            "lay06-order-flow-1600",
        ),
        (
            "order-flow",
            crate::testing::MINIMUM,
            Checks::Collapsed,
            "lay06-order-flow-960",
        ),
    ] {
        let (_project, mut harness, id) = tidied(name, size, checks);
        choose(&mut harness, FIT);
        harness.step();
        let canvas = board(&harness, &id);
        let focus = canvas.debug_focus();
        assert!(
            canvas
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{label}: a card is in view"
        );
        crate::testing::capture(&mut harness, label, None);
    }
}
