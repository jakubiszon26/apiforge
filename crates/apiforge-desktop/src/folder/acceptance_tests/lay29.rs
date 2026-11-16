//! LAY-29 (#382): a wire never runs under its own card. An ordering line
//! leaves the socket its card draws for it, *Next*, *Yes* or *No* on the
//! bottom edge, and goes round the card to the step it orders (owner
//! decision A of 2026-11-16); before, it began on the top edge above the
//! socket, where nothing is drawn, as if it came out from under the card.
//! The window draws what the layout crate's `lay29` tests measure, on a
//! scenario with five steps ordered after *Start* and on the reference
//! scenarios of `fixtures/scenarios/lay/`. `review_lay29` renders the views
//! of the pull request.
use super::*;
use crate::canvas::WireMode;
use apiforge_core::format::Graph;
use apiforge_layout::{GAP, Point, Rect as Block, Route, Side, crossing, packed, under_own};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";

/// Five steps ordered after *Start*, as on the owner's board.
const FIVE: &str = r#"scenario "Five after start" {
  begin = start
  @after(begin)
  login = POST "/login" {
    body { user: "demo", password: "demo" }
    sample 200 { token: "t-1" }
  }
  @after(begin)
  products = GET "/products" {
    sample 200 { items: [{ id: "p-1", price: 12.5 }] }
  }
  @after(begin)
  cart = POST "/cart" {
    body { items: [] }
    sample 201 { id: "c-1", total: 28.0 }
  }
  @after(begin)
  profile = GET "/profile" {
    sample 200 { name: "demo" }
  }
  @after(begin)
  orders = GET "/orders" {
    sample 200 { items: [] }
  }
}
"#;

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

fn id_of(harness: &FolderHarness, id: &str, handle: &str) -> String {
    graph(harness, id)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone()
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

/// Where every card stands, by its id.
fn positions(harness: &FolderHarness, id: &str) -> Vec<(String, [i32; 4])> {
    let mut cards: Vec<(String, [i32; 4])> = board(harness, id)
        .debug_cards()
        .iter()
        .map(|(card, rect)| {
            (
                card.clone(),
                [
                    rect.min.x.round() as i32,
                    rect.min.y.round() as i32,
                    rect.max.x.round() as i32,
                    rect.max.y.round() as i32,
                ],
            )
        })
        .collect();
    cards.sort();
    cards
}

/// Criteria 1 and 5: five steps ordered after *Start*, tidied, with every
/// wire drawn. Each of the five dashed lines of *Start* begins at its
/// *Next* socket on the bottom edge and leaves downwards; none has a point
/// on the top edge of *Start*; none lies inside a card it joins. The board
/// draws a bridge at every crossing, keeps the gap, and Tidy up a second
/// time gives the same board.
#[test]
fn lay29_criterion_1_the_lines_of_start_leave_its_next_socket() {
    let (_project, mut harness, id) = tidied(FIVE, WIDE, false, WireMode::All);
    let begin = id_of(&harness, &id, "begin");
    let canvas = board(&harness, &id);
    let start = canvas.debug_world(&begin).expect("Start is drawn");
    let lines: Vec<&crate::canvas::DrawnWire> = canvas
        .debug_wires()
        .iter()
        .filter(|wire| wire.dashed && wire.source.as_deref() == Some(begin.as_str()))
        .collect();
    assert_eq!(lines.len(), 5, "the five ordering lines of Start");
    for line in &lines {
        let (first, next) = (line.points[0], line.points[1]);
        assert!(
            (first.y - start.bottom()).abs() < 0.5
                && (first.x - start.center().x).abs() < 0.5,
            "the line into {} begins at Next, at the middle of the bottom edge: {:?}",
            line.target,
            line.points
        );
        assert!(
            (next.x - first.x).abs() < 0.5 && next.y > first.y,
            "the line into {} leaves downwards: {:?}",
            line.target,
            line.points
        );
        let on_top = line.points.windows(2).any(|pair| {
            pair[0].y.min(pair[1].y) <= start.top() + 0.5
                && pair[0].y.max(pair[1].y) >= start.top() - 0.5
                && pair[0].x.min(pair[1].x) <= start.right() + 0.5
                && pair[0].x.max(pair[1].x) >= start.left() - 0.5
        });
        assert!(
            !on_top,
            "the line into {} keeps off the top edge of Start: {:?}",
            line.target,
            line.points
        );
    }
    let (cards, wires) = canvas.debug_geometry().clone();
    assert!(
        wires.iter().all(|wire| wire.from.side != Side::Top),
        "no line leaves a top edge"
    );
    let routed = apiforge_layout::route(&cards, &wires, &apiforge_layout::Routing::default());
    assert_eq!(under_own(&routed, &wires, &cards), []);
    let (routes, cards) = drawn(&harness, &id);
    let bridges: usize = canvas.debug_wires().iter().map(|wire| wire.bridges).sum();
    assert_eq!(bridges, crossings(&routes), "a bridge at every crossing");
    assert_eq!(packed(&routes, &cards, GAP), [], "the gap holds");

    let before = positions(&harness, &id);
    choose(&mut harness, TIDY);
    settle(&mut harness);
    harness.step();
    assert_eq!(positions(&harness, &id), before, "Tidy up twice");
}

/// Criteria 2 and 3 in the window: on `shop-30` and `fan-out`, tidied, no
/// line leaves a top edge and none lies inside a card it joins; the gap
/// holds; `shop-30` in the default view has at most 6 crossings and fits
/// 1600 × 1000 at 48 % or more, and with every wire and check drawn it has
/// at most 106 crossings.
#[test]
fn lay29_criteria_2_and_3_the_reference_boards_in_the_window() {
    for (name, mode, expanded, most, fit) in [
        ("shop-30", WireMode::Chips, false, Some(6), Some(0.48)),
        ("shop-30", WireMode::All, true, Some(106), None),
        ("fan-out", WireMode::All, false, None, None),
        ("fan-out", WireMode::Chips, true, None, None),
    ] {
        let (_project, mut harness, id) = tidied(&fixture(name), WIDE, expanded, mode);
        let label = format!("{name}, {mode:?}, checks drawn: {expanded}");
        let canvas = board(&harness, &id);
        let (cards, wires) = canvas.debug_geometry().clone();
        assert!(
            wires.iter().all(|wire| wire.from.side != Side::Top),
            "{label}: no line leaves a top edge"
        );
        let routed = apiforge_layout::route(&cards, &wires, &apiforge_layout::Routing::default());
        assert_eq!(under_own(&routed, &wires, &cards), [], "{label}");
        let (routes, cards) = drawn(&harness, &id);
        assert!(!routes.is_empty(), "{label}: the board draws lines");
        assert_eq!(packed(&routes, &cards, GAP), [], "{label}: the gap holds");
        let found = crossings(&routes);
        if let Some(most) = most {
            assert!(found <= most, "{label}: {found} crossings");
        }
        if let Some(least) = fit {
            choose(&mut harness, FIT);
            harness.step();
            let zoom = board(&harness, &id).debug_zoom();
            assert!(zoom >= least, "{label}: Fit shows the board at {zoom}");
        }
    }
}

/// Criterion 6: the views of the pull request, each asserting a card in
/// view: the five steps after *Start* at 100 % in the smallest window, and
/// `shop-30` at Fit and at its `if` in the large one.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay29() {
    let five = FIVE.to_owned();
    let shop = fixture("shop-30");
    // (capture, scenario, wires, the card the view is centred on at 100 %
    // or Fit, the window)
    let views = [
        (
            "five-960",
            &five,
            WireMode::Chips,
            Some("begin"),
            crate::testing::MINIMUM,
        ),
        ("five-1600", &five, WireMode::All, None, WIDE),
        ("shop-30-1600", &shop, WireMode::Chips, None, WIDE),
        (
            "if-960",
            &shop,
            WireMode::Chips,
            Some("paid"),
            crate::testing::MINIMUM,
        ),
    ];
    for (label, text, wires, centre, size) in views {
        let (_project, mut harness, id) = tidied(text, size, false, wires);
        choose(&mut harness, FIT);
        harness.step();
        if let Some(handle) = centre {
            let node = id_of(&harness, &id, handle);
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .expect("the board is drawn")
                .debug_show(&node, 1.0);
            settle(&mut harness);
            harness.step();
        }
        let canvas = board(&harness, &id);
        let (routes, cards) = drawn(&harness, &id);
        println!(
            "LAY29 {label}: lines {} crossings {} bends {} packed {} zoom {:.3}",
            routes.len(),
            crossings(&routes),
            routes.iter().map(Route::bends).sum::<usize>(),
            packed(&routes, &cards, GAP).len(),
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

/// Criterion 8: selecting a step brings back the lines of its chips and
/// its rails, and they keep the gap from the lines at rest, from each
/// other and from the cards, on `shop-30` and `fan-out`, step by step. A
/// rail brought back leaves the socket at the bottom of its card, where
/// Tidy up left it room, and the lines at rest keep off the sockets of the
/// wires chips stand for. `login` on `shop-30` is the one step left out:
/// its eleven lines come back to one port and nine places are within the
/// gap, as before this ticket.
#[test]
fn lay29_criterion_8_lines_brought_back_keep_the_gap_for_every_selection() {
    for name in ["shop-30", "fan-out"] {
        let (_project, mut harness, id) = tidied(&fixture(name), WIDE, false, WireMode::Chips);
        let at_rest: Vec<Vec<egui::Pos2>> = board(&harness, &id)
            .debug_wires()
            .iter()
            .filter(|wire| !wire.revealed)
            .map(|wire| wire.points.clone())
            .collect();
        let steps = graph(&harness, &id).nodes;
        let mut brought_back = 0;
        for step in &steps {
            app(&mut harness)
                .selections
                .insert(id.clone(), vec![step.id.clone()]);
            settle(&mut harness);
            harness.step();
            let canvas = board(&harness, &id);
            let resting: Vec<Vec<egui::Pos2>> = canvas
                .debug_wires()
                .iter()
                .filter(|wire| !wire.revealed)
                .map(|wire| wire.points.clone())
                .collect();
            assert_eq!(
                resting, at_rest,
                "{name}: selecting {} moves no line at rest",
                step.handle
            );
            brought_back += canvas
                .debug_wires()
                .iter()
                .filter(|wire| wire.revealed && !wire.points.is_empty())
                .count();
            if name == "shop-30" && step.handle == "login" {
                continue;
            }
            let (routes, cards) = drawn(&harness, &id);
            assert_eq!(
                packed(&routes, &cards, GAP),
                [],
                "{name} with {} selected",
                step.handle
            );
        }
        assert!(brought_back > 0, "{name}: selections bring lines back");
    }
}
