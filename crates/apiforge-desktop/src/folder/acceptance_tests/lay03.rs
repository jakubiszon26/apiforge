//! LAY-03 (#34): lines are always far enough apart to trace. A hard gap of
//! 20 board px between any two parallel segments of the lines drawn, rails
//! and wires alike, and between a segment and a card; Tidy up widens the gaps
//! between columns, rows and stacked cards until the lines fit. On the
//! reference scenarios of `fixtures/scenarios/lay/`.
use super::*;
use crate::canvas::{Command, WireMode};
use apiforge_core::format::Graph;
use apiforge_layout::{Breach, GAP, Point, Rect as Block, Route, packed};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// The reference scenario `name` opened at `size` without positions, its
/// checks folded or drawn, in the *Wires* mode given.
fn opened(
    name: &str,
    size: egui::Vec2,
    expanded: bool,
    wires: WireMode,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), fixture(name)).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    open(project, size, expanded, wires)
}

fn open(
    project: tempfile::TempDir,
    size: egui::Vec2,
    expanded: bool,
    wires: WireMode,
) -> (tempfile::TempDir, FolderHarness, String) {
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).debug_wires(wires);
    if expanded {
        app(&mut harness).debug_draw_checks();
    }
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

/// [`opened`], tidied up once more with nothing selected.
fn tidied(
    name: &str,
    size: egui::Vec2,
    expanded: bool,
    wires: WireMode,
) -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = opened(name, size, expanded, wires);
    tidy(&mut harness);
    (project, harness, id)
}

fn tidy(harness: &mut FolderHarness) {
    choose(harness, TIDY);
    settle(harness);
    harness.step();
}

/// Where every step stands, by handle.
fn positions(harness: &FolderHarness, id: &str) -> Vec<(String, (f64, f64))> {
    graph(harness, id)
        .nodes
        .iter()
        .map(|node| (node.handle.clone(), (node.position.x, node.position.y)))
        .collect()
}

/// The lines the last frame drew, those brought back included, and the
/// rectangles of its cards and chips, as the router was given them.
fn drawn(harness: &FolderHarness, id: &str) -> (Vec<Route>, Vec<Block>) {
    let canvas = board(harness, id);
    let routes = canvas
        .debug_wires()
        .iter()
        .filter(|wire| !wire.points.is_empty())
        .map(|wire| Route {
            points: wire
                .points
                .iter()
                .map(|point| Point::new(point.x, point.y))
                .collect(),
            ..Route::default()
        })
        .collect();
    (routes, canvas.debug_geometry().0.clone())
}

/// The breaches of the gap among the lines drawn (criterion 1).
fn breaches(harness: &FolderHarness, id: &str) -> Vec<Breach> {
    let (routes, cards) = drawn(harness, id);
    assert!(!routes.is_empty(), "the board draws lines");
    for route in &routes {
        for (from, to) in route.segments() {
            assert!(
                (from.x - to.x).abs() < 0.01 || (from.y - to.y).abs() < 0.01,
                "every line is routed, none drawn directly: {:?}",
                route.points
            );
        }
    }
    packed(&routes, &cards, GAP)
}

fn assert_apart(harness: &FolderHarness, id: &str, label: &str) {
    let found = breaches(harness, id);
    assert!(
        found.is_empty(),
        "{label}: {} places where lines are nearer than {GAP} px, the first {:?}",
        found.len(),
        found.first()
    );
}

fn graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

fn board<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

fn board_mut<'a>(harness: &'a mut FolderHarness, id: &str) -> &'a mut crate::canvas::Canvas {
    app(harness)
        .canvases
        .get_mut(id)
        .expect("the board is drawn")
}

/// Criterion 1: after Tidy up no two lines of a reference board run nearer
/// than 20 px, and none nearer than 20 px to a card: as a new install shows
/// the board (compact cards, checks folded, chips), with every wire and
/// every check drawn, and at the full density.
#[test]
fn lay03_criterion_1_tidied_boards_keep_the_gap() {
    for name in ["order-flow", "shop-30", "shop-50", "fan-out", "far-value"] {
        for (expanded, wires) in [(false, WireMode::Chips), (true, WireMode::All)] {
            let (_project, harness, id) = tidied(name, WIDE, expanded, wires);
            assert_apart(
                &harness,
                &id,
                &format!("{name}, {wires:?}, checks drawn: {expanded}"),
            );
        }
    }
    let (_project, mut harness, id) = tidied("shop-30", WIDE, true, WireMode::All);
    app(&mut harness).active_board_command(Command::SetCompactCards(false));
    settle(&mut harness);
    tidy(&mut harness);
    assert_apart(&harness, &id, "shop-30 at the full density");
}

/// Criterion 2: wires of one port are one line per reader from their turns
/// onwards: no two lines lie on each other, the port's own straight piece
/// aside. On `order-flow` with every wire drawn, where `products` feeds
/// `first` and `second` from one port, and on the demo shop board.
#[test]
fn lay03_criterion_2_one_line_per_reader() {
    let (_project, harness, id) = tidied("order-flow", WIDE, false, WireMode::All);
    let (routes, _) = drawn(&harness, &id);
    let shared = routes.iter().enumerate().any(|(index, a)| {
        routes[index + 1..]
            .iter()
            .any(|b| a.points.first() == b.points.first())
    });
    assert!(shared, "a port of order-flow has two readers");
    assert_apart(&harness, &id, "order-flow, all wires");

    let project = crate::testing::shop_project();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let (_project, mut harness, id) = open(project, WIDE, false, WireMode::All);
    tidy(&mut harness);
    assert_apart(&harness, &id, "the demo shop");
}

/// The gaps between the spine's columns in the first row, left to right.
fn column_gaps(harness: &FolderHarness, id: &str) -> Vec<f32> {
    let cards = board(harness, id).debug_geometry().0.clone();
    let top = cards.iter().map(|card| card.min.y).fold(f32::MAX, f32::min);
    let mut row: Vec<&Block> = cards
        .iter()
        .filter(|card| (card.min.y - top).abs() < 0.5)
        .collect();
    row.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
    row.windows(2)
        .map(|pair| pair[1].min.x - pair[0].max.x)
        .collect()
}

/// Criterion 4: Tidy up widens the gap between two columns where the lines
/// need it, in steps of 20 px, and leaves 100 px where they do not; twice
/// the same Tidy up moves nothing.
#[test]
fn lay03_criterion_4_tidy_up_widens_where_lines_need_it() {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, false, WireMode::All);
    let gaps = column_gaps(&harness, &id);
    assert_eq!(gaps.len(), 4, "five columns: {gaps:?}");
    assert!(
        gaps.iter().any(|gap| *gap > 100.5),
        "a gap is widened: {gaps:?}"
    );
    assert!(
        gaps.iter().any(|gap| (*gap - 100.0).abs() < 0.5),
        "a gap no line needs stays 100 px: {gaps:?}"
    );
    for gap in &gaps {
        assert!(
            *gap >= 99.5 && (gap / 20.0 - (gap / 20.0).round()).abs() < 0.01,
            "in steps of 20 px: {gaps:?}"
        );
    }
    let before = positions(&harness, &id);
    tidy(&mut harness);
    assert_eq!(positions(&harness, &id), before, "the same Tidy up twice");
}

/// Criterion 4: a scenario opened from code is laid out as Tidy up lays it
/// out, with the room the lines of a new install's board need.
#[test]
fn lay03_criterion_4_opening_gives_the_tidied_board() {
    for name in ["fan-out", "shop-30"] {
        let (_project, mut harness, id) = opened(name, WIDE, false, WireMode::Chips);
        assert_apart(&harness, &id, &format!("{name} as opened"));
        let before = positions(&harness, &id);
        tidy(&mut harness);
        assert_eq!(positions(&harness, &id), before, "{name}");
    }
}

/// Criterion 6: the wires brought back while a card is selected keep the
/// gap from the lines at rest, from each other and from the cards.
#[test]
fn lay03_criterion_6_lines_brought_back_keep_the_gap() {
    for name in ["shop-30", "fan-out"] {
        let (_project, mut harness, id) = tidied(name, WIDE, false, WireMode::Chips);
        // The reader with the most chips.
        let mut readers: Vec<String> = board(&harness, &id)
            .debug_tags()
            .iter()
            .map(|tag| tag.tag.reader.clone())
            .collect();
        readers.sort();
        let reader = readers
            .iter()
            .max_by_key(|reader| readers.iter().filter(|other| other == reader).count())
            .expect("the board draws chips")
            .clone();
        app(&mut harness)
            .selections
            .insert(id.clone(), vec![reader]);
        settle(&mut harness);
        harness.step();
        let back = board(&harness, &id)
            .debug_wires()
            .iter()
            .filter(|wire| wire.revealed && !wire.points.is_empty())
            .count();
        assert!(back > 0, "{name}: the selection brings wires back");
        assert_apart(&harness, &id, &format!("{name} with a reader selected"));
    }
}

/// The camera centred on `handle` at `zoom`, as a person scrolls there.
fn show(harness: &mut FolderHarness, id: &str, handle: &str, zoom: f32) {
    let node = graph(harness, id)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone();
    board_mut(harness, id).debug_show(&node, zoom);
    settle(harness);
    harness.step();
}

/// Criterion 8: the four situations of the owner's finding, on fixtures, at
/// both sizes, each asserting a card in view.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay03() {
    // (capture, scenario, wires, checks drawn, the card the view is centred
    // on at 100 %, or Fit)
    let views = [
        // 1. Rails along the top of a row.
        (
            "1-row-top",
            "fan-out",
            WireMode::Chips,
            false,
            Some("stock"),
        ),
        // 2. A vertical and a horizontal rail round one corner.
        (
            "2-corner",
            "fan-out",
            WireMode::Chips,
            false,
            Some("apply_coupon"),
        ),
        // The same two with every rail drawn: since LAY-24 the default view
        // draws an arrow for a rail into the next step.
        (
            "1-row-top-all-wires",
            "fan-out",
            WireMode::All,
            false,
            Some("stock"),
        ),
        (
            "2-corner-all-wires",
            "fan-out",
            WireMode::All,
            false,
            Some("apply_coupon"),
        ),
        // 3. The lines under the last row.
        (
            "3-under-last-row",
            "shop-30",
            WireMode::All,
            false,
            Some("receipt"),
        ),
        // 4. A corner where three or four lines arrive.
        (
            "4-many-arrive",
            "shop-30",
            WireMode::All,
            false,
            Some("second"),
        ),
        ("fit-fan-out", "fan-out", WireMode::Chips, false, None),
        ("fit-shop-30", "shop-30", WireMode::All, false, None),
        ("fit-shop-30-checks", "shop-30", WireMode::Chips, true, None),
        ("fit-fan-out-checks", "fan-out", WireMode::Chips, true, None),
    ];
    for size in [WIDE, crate::testing::MINIMUM] {
        let suffix = if size == WIDE { "1600" } else { "960" };
        for (label, name, wires, expanded, centre) in views {
            let (_project, mut harness, id) = tidied(name, size, expanded, wires);
            choose(&mut harness, FIT);
            harness.step();
            if let Some(handle) = centre {
                show(&mut harness, &id, handle, 1.0);
            }
            let canvas = board(&harness, &id);
            let focus = canvas.debug_focus();
            assert!(
                canvas
                    .debug_cards()
                    .iter()
                    .any(|(_, rect)| focus.contains_rect(*rect)),
                "{label}-{suffix}: a card is in view"
            );
            crate::testing::capture(&mut harness, &format!("lay03-{label}-{suffix}"), None);
        }
    }
}
