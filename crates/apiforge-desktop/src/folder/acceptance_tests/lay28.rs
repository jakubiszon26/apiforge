//! LAY-28 (#348): lines in one corridor are ordered so they cross less. The
//! window draws the boards the layout crate's `lay28` tests measure: fewer
//! crossings on the reference scenarios of `fixtures/scenarios/lay/`, the
//! gap of LAY-03 as before. `review_lay28` renders the views of the pull
//! request, with #349's sockets.
use super::*;
use crate::canvas::WireMode;
use apiforge_core::format::Graph;
use apiforge_layout::{GAP, Point, Rect as Block, Route, crossing, packed};

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

/// The reference scenario `name` at `size`, laid out from code and tidied
/// once more, its checks folded or drawn, in the *Wires* mode given.
pub(super) fn tidied(
    name: &str,
    size: egui::Vec2,
    expanded: bool,
    wires: WireMode,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), fixture(name)).unwrap();
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

pub(super) fn board<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

pub(super) fn graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

/// The lines the last frame drew and the rectangles of its cards and chips,
/// as the router was given them.
pub(super) fn drawn(harness: &FolderHarness, id: &str) -> (Vec<Route>, Vec<Block>) {
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

fn crossings(routes: &[Route]) -> usize {
    let mut count = 0;
    for (index, a) in routes.iter().enumerate() {
        for b in &routes[index + 1..] {
            for (a1, a2) in a.segments() {
                for (b1, b2) in b.segments() {
                    count += usize::from(crossing(a1, a2, b1, b2).is_some());
                }
            }
        }
    }
    count
}

/// Criterion 5: the window draws the ordered boards. `shop-30` with every
/// wire has at most 82 crossings with its checks folded and 106 with them
/// drawn; with chips and the checks drawn, `shop-30` has at most 12 and
/// `fan-out` 5. The limits were 75, 105, 10 and 5 until LAY-29 (#382), by
/// which an ordering line leaves its socket at the bottom and goes round
/// its card. The bridges the board draws are those crossings, and no line
/// is nearer than 20 px to another or to a card.
#[test]
fn lay28_criterion_5_the_window_draws_the_ordered_boards() {
    for (name, wires, expanded, most) in [
        ("shop-30", WireMode::All, false, 82),
        ("shop-30", WireMode::All, true, 106),
        ("shop-30", WireMode::Chips, true, 12),
        ("fan-out", WireMode::Chips, true, 5),
    ] {
        let (_project, harness, id) = tidied(name, WIDE, expanded, wires);
        let (routes, cards) = drawn(&harness, &id);
        let label = format!("{name}, {wires:?}, checks drawn: {expanded}");
        assert!(!routes.is_empty(), "{label}: the board draws lines");
        let found = crossings(&routes);
        assert!(found <= most, "{label}: {found} crossings");
        let bridges: usize = board(&harness, &id)
            .debug_wires()
            .iter()
            .map(|wire| wire.bridges)
            .sum();
        assert_eq!(bridges, found, "{label}: a bridge at every crossing");
        assert_eq!(packed(&routes, &cards, GAP), [], "{label}: the gap holds");
    }
}

/// Criterion 6: the views of the pull request, at both sizes, each
/// asserting a card in view: `fan-out` and `shop-30` with their checks
/// drawn, at `checkout_ok` (a *Data condition* with three inputs) and at
/// Fit, and `shop-30` with every wire at Fit.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay28() {
    // (capture, scenario, wires, checks drawn, the card the view is centred
    // on at 100 %, or Fit)
    let views = [
        (
            "condition",
            "fan-out",
            WireMode::Chips,
            true,
            Some("checkout_ok"),
        ),
        (
            "condition-all-wires",
            "fan-out",
            WireMode::All,
            true,
            Some("checkout_ok"),
        ),
        ("fit-shop-30-checks", "shop-30", WireMode::Chips, true, None),
        (
            "fit-shop-30-all-wires",
            "shop-30",
            WireMode::All,
            false,
            None,
        ),
    ];
    for size in [WIDE, crate::testing::MINIMUM] {
        let suffix = if size == WIDE { "1600" } else { "960" };
        for (label, name, wires, expanded, centre) in views {
            let (_project, mut harness, id) = tidied(name, size, expanded, wires);
            choose(&mut harness, FIT);
            harness.step();
            if let Some(handle) = centre {
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
            let canvas = board(&harness, &id);
            let focus = canvas.debug_focus();
            assert!(
                canvas
                    .debug_cards()
                    .iter()
                    .any(|(_, rect)| focus.contains_rect(*rect)),
                "{label}-{suffix}: a card is in view"
            );
            crate::testing::capture(&mut harness, &format!("lay28-{label}-{suffix}"), None);
        }
    }
}
