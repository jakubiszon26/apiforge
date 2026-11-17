//! LAY-29 (#382): a wire never runs under its own card. Where cards lie on
//! each other — a card dragged onto another, or a board not tidied — the
//! router keeps a wire's own two cards in its way on every attempt and
//! leaves a port the other card lies over through its stub; a line with no
//! way round at all is drawn as the plain corner of an enclosed port. The
//! window draws what the layout crate's `lay29` tests measure. Where an
//! ordering line leaves its card is not this ticket's: the owner narrowed
//! it on 2026-11-17 to the part that does not depend on the language.
//! `review_lay29` renders the views of the pull request.
use super::*;
use crate::canvas::{Command, WireMode};
use apiforge_core::format::{Graph, Position};
use apiforge_layout::{GAP, Point, Rect as Block, Route, Routing, packed, under_own};

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

/// The scenario `text` at `size`, laid out from code and tidied once more,
/// its checks folded or drawn, in the *Wires* mode given.
fn tidied(
    text: &str,
    size: egui::Vec2,
    expanded: bool,
    wires: WireMode,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), text).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
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
    choose(&mut harness, TIDY);
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

fn board<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

fn graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

/// Lays every third step of the board over the same place, each a little
/// lower and to the right of the one before, as the layout crate's piles
/// of cards are made: a board where cards lie on each other.
fn pile(harness: &mut FolderHarness, id: &str) {
    let moves: Vec<(String, Position)> = graph(harness, id)
        .nodes
        .iter()
        .enumerate()
        .step_by(3)
        .map(|(index, node)| {
            (
                node.id.clone(),
                Position {
                    x: 300.0 + index as f64 * 24.0,
                    y: 200.0 + index as f64 * 16.0,
                },
            )
        })
        .collect();
    app(harness).active_board_command(Command::Move(moves));
    settle(harness);
    harness.step();
}

/// What the board at rest gave the router, routed again as the window
/// routes it: the wires with a piece under a card they join, and how many
/// wires have no route at all.
fn under_own_in_the_window(harness: &FolderHarness, id: &str) -> (Vec<(usize, usize)>, usize) {
    let (cards, wires) = board(harness, id).debug_geometry().clone();
    assert!(!wires.is_empty(), "the board draws lines");
    let routes = apiforge_layout::route(&cards, &wires, &Routing::default());
    let incomplete = routes.iter().filter(|route| !route.complete).count();
    (under_own(&routes, &wires, &cards), incomplete)
}

/// The lines the last frame drew, as routes, and the rectangles of its
/// cards and chips as the router was given them.
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

/// Criterion 3: the window draws no line under a card it joins. On
/// `shop-30` and `fan-out` with cards piled on each other, in both *Wires*
/// modes, no line has a piece inside a card it joins, and few have no
/// route; tidied, the same boards have none under their cards, every line
/// has a route and the gap of LAY-03 holds.
#[test]
fn lay29_criterion_3_the_window_draws_no_line_under_its_own_card() {
    for (name, mode) in [
        ("shop-30", WireMode::Chips),
        ("shop-30", WireMode::All),
        ("fan-out", WireMode::All),
    ] {
        let (_project, mut harness, id) = tidied(&fixture(name), WIDE, false, mode);
        let label = format!("{name}, {mode:?}");
        let (under, incomplete) = under_own_in_the_window(&harness, &id);
        assert_eq!(under, [], "{label}, tidied: lines under a card they join");
        assert_eq!(incomplete, 0, "{label}, tidied: every line has a route");
        let (routes, cards) = drawn(&harness, &id);
        assert_eq!(packed(&routes, &cards, GAP), [], "{label}: the gap holds");

        pile(&mut harness, &id);
        let (under, incomplete) = under_own_in_the_window(&harness, &id);
        let wires = board(&harness, &id).debug_geometry().1.len();
        assert_eq!(under, [], "{label}, piled: lines under a card they join");
        assert!(
            incomplete * 10 <= wires,
            "{label}, piled: {incomplete} of {wires} lines have no route"
        );
    }
}

/// Criterion 4: the views of the pull request, each asserting a card in
/// view: `shop-30` with every wire and its cards piled on each other, at
/// Fit and at 100 % on `receipt` in the large window, where the lines from
/// `order` into `receipt` and `refund` ran under both cards before, and at
/// 100 % on `login` in the smallest one, where the ordering line from
/// `begin` ran under `login` and now has no way round.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay29() {
    let shop = fixture("shop-30");
    // (capture, the window, the card shown at 100 %)
    let views = [
        ("piled-1600", WIDE, None),
        ("piled-receipt-1600", WIDE, Some("receipt")),
        ("piled-login-960", crate::testing::MINIMUM, Some("login")),
    ];
    for (label, size, close) in views {
        let (_project, mut harness, id) = tidied(&shop, size, false, WireMode::All);
        pile(&mut harness, &id);
        choose(&mut harness, FIT);
        harness.step();
        if let Some(handle) = close {
            let node = graph(&harness, &id)
                .nodes
                .iter()
                .find(|node| node.handle == handle)
                .unwrap_or_else(|| panic!("a step {handle}"))
                .id
                .clone();
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .expect("the board is drawn")
                .debug_show(&node, 1.0);
            settle(&mut harness);
            harness.step();
        }
        let (under, incomplete) = under_own_in_the_window(&harness, &id);
        let canvas = board(&harness, &id);
        outln!(
            "LAY29 {label}: lines {} under their own card {} without a route {incomplete} zoom {:.3}",
            canvas.debug_geometry().1.len(),
            under.len(),
            canvas.debug_zoom(),
        );
        let focus = canvas.debug_focus();
        assert!(
            canvas
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{label}: a card is in view"
        );
        crate::testing::capture(&mut harness, &format!("lay29-{label}"), None);
    }
}

