//! LAY-14 (#298): wire and input tracing. Pointing at a drawn line lights
//! it, rings the port at each end and states it in a box at the pointer, as
//! a chip does (1 = A); a click on a line or a chip keeps it, with the rest
//! of the board dimmed as for a selected step, until Esc or a click on the
//! empty board, and pointing alone dims nothing (2 = A); pointing at an
//! input row names every value feeding it, lines and chips alike, and a
//! click on the row still selects the card (3 = A). On `shop-30`, tidied,
//! its checks folded, at 60 %.
use super::lay16::{press, release};
use super::lay23::{Checks, board, graph, handle_of, id_of, revealed, show, tidied};
use super::*;
use crate::canvas::DrawnWire;
use crate::testing::MINIMUM;
use apiforge_core::graph::{WireKind, wires as document_wires};
use egui::{Pos2, Rect, Vec2, pos2, vec2};
use std::collections::BTreeSet;

/// The line `valid_cart → checkout`: `valid_cart.value.id` has two readers,
/// so it is drawn, not a chip.
const LINE: &str = "valid_cart.value.id → checkout.body.cartId";
/// The chip at `checkout`'s body: `expected_total` is read far and often.
const CHIP: &str = "expected_total → checkout.body.total";

fn opened(size: Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = tidied("shop-30", size, Checks::Collapsed);
    show(&mut harness, &id, "checkout", 0.6);
    (project, harness, id)
}

/// The line from `source` into `target` as the last frame drew it at rest.
fn line(harness: &FolderHarness, id: &str, source: &str, target: &str) -> DrawnWire {
    let graph = graph(harness, id);
    board(harness, id)
        .debug_wires()
        .iter()
        .find(|wire| {
            !wire.dashed
                && !wire.revealed
                && wire.source.as_deref().map(|s| handle_of(&graph, s)) == Some(source.to_owned())
                && handle_of(&graph, &wire.target) == target
        })
        .cloned()
        .unwrap_or_else(|| panic!("a line {source} → {target} is drawn"))
}

fn screen(harness: &FolderHarness, id: &str, world: Pos2) -> Pos2 {
    board(harness, id)
        .debug_to_screen(Rect::from_min_max(world, world))
        .min
}

fn distance(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let along = b - a;
    let length = along.length_sq();
    if length <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(along) / length).clamp(0.0, 1.0);
    point.distance(a + along * t)
}

/// Every wire the last frame drew, on screen.
fn routes(harness: &FolderHarness, id: &str) -> Vec<(String, Vec<Pos2>)> {
    board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| wire.points.len() >= 2)
        .map(|wire| {
            (
                wire.id.clone(),
                wire.points
                    .iter()
                    .map(|at| screen(harness, id, *at))
                    .collect(),
            )
        })
        .collect()
}

fn near_route(points: &[Pos2], at: Pos2) -> f32 {
    points
        .windows(2)
        .map(|pair| distance(at, pair[0], pair[1]))
        .fold(f32::INFINITY, f32::min)
}

/// What a point on the board must keep clear of: every card, chip and count.
fn obstacles(harness: &FolderHarness, id: &str) -> Vec<Rect> {
    let canvas = board(harness, id);
    canvas
        .debug_cards()
        .iter()
        .map(|(_, rect)| rect.expand(10.0))
        .chain(canvas.debug_tags().iter().map(|tag| tag.rect.expand(6.0)))
        .chain(
            canvas
                .debug_counts()
                .iter()
                .map(|count| canvas.debug_to_screen(count.world).expand(6.0)),
        )
        .collect()
}

/// A point on screen on the wire `wire`, inside the board, clear of every
/// card, chip and count, and at least 14 px from every other wire drawn.
fn point_on(harness: &FolderHarness, id: &str, wire: &str) -> Pos2 {
    let routes = routes(harness, id);
    let inside = board(harness, id).debug_board().shrink(24.0);
    let obstacles = obstacles(harness, id);
    let (_, points) = routes
        .iter()
        .find(|(other, _)| other == wire)
        .expect("the wire is drawn");
    points
        .windows(2)
        .flat_map(|pair| {
            (1..10).map(move |step| pair[0] + (pair[1] - pair[0]) * (step as f32 / 10.0))
        })
        .find(|at| {
            inside.contains(*at)
                && obstacles.iter().all(|rect| !rect.contains(*at))
                && routes
                    .iter()
                    .filter(|(other, _)| other != wire)
                    .all(|(_, points)| near_route(points, *at) > 14.0)
        })
        .expect("a point on the wire, in view and clear of the rest")
}

/// A point of the empty board: no card, chip, count or wire near it.
fn empty_point(harness: &FolderHarness, id: &str) -> Pos2 {
    let routes = routes(harness, id);
    let inside = board(harness, id).debug_board().shrink(40.0);
    let obstacles = obstacles(harness, id);
    (0..)
        .map(|row| inside.top() + 10.0 * row as f32)
        .take_while(|y| *y < inside.bottom())
        .flat_map(|y| {
            (0..)
                .map(move |column| pos2(inside.left() + 10.0 * column as f32, y))
                .take_while(|at| at.x < inside.right())
                .collect::<Vec<_>>()
        })
        .find(|at| {
            obstacles
                .iter()
                .all(|rect| !rect.expand(20.0).contains(*at))
                && routes
                    .iter()
                    .all(|(_, points)| near_route(points, *at) > 30.0)
        })
        .expect("some empty board in view")
}

fn hover(harness: &mut FolderHarness, at: Pos2) {
    harness.hover_at(at);
    harness.step();
    harness.step();
    harness.step();
}

fn away(harness: &mut FolderHarness) {
    harness.remove_cursor();
    harness.step();
    harness.step();
}

/// A click: the pointer leaves afterwards.
fn click(harness: &mut FolderHarness, at: Pos2) {
    press(harness, at);
    release(harness, at);
    harness.step();
    harness.step();
}

/// Ctrl+click (Cmd+click on macOS).
fn ctrl_click(harness: &mut FolderHarness, at: Pos2) {
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::COMMAND));
    click(harness, at);
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    harness.step();
}

fn escape(harness: &mut FolderHarness) {
    harness.key_press(egui::Key::Escape);
    settle(harness);
    harness.step();
}

/// The cards whose port the last frame ringed, and where.
fn rings(harness: &FolderHarness, id: &str) -> Vec<(String, Pos2)> {
    board(harness, id).debug_ringed_ports()
}

/// Whether the port of `card` at `at` (on screen) is ringed.
fn ringed(harness: &FolderHarness, id: &str, card: &str, at: Pos2) -> bool {
    rings(harness, id)
        .iter()
        .any(|(ringed, ring)| ringed == card && ring.distance(at) < 2.0)
}

/// Both ends of the wire `wire` are ringed, at the ports it joins.
fn assert_ends_ringed(harness: &FolderHarness, id: &str, wire: &str, context: &str) {
    let drawn = board(harness, id)
        .debug_wires()
        .iter()
        .find(|drawn| drawn.id == wire)
        .cloned()
        .unwrap_or_else(|| panic!("{context}: the wire is drawn"));
    let source = drawn.source.clone().expect("a wire from a step");
    let first = screen(harness, id, drawn.points[0]);
    let last = screen(harness, id, *drawn.points.last().unwrap());
    assert!(
        ringed(harness, id, &source, first),
        "{context}: its source port is ringed: {:?}",
        rings(harness, id)
    );
    assert!(
        ringed(harness, id, &drawn.target, last),
        "{context}: its input is ringed: {:?}",
        rings(harness, id)
    );
}

fn statements(harness: &FolderHarness, id: &str) -> BTreeSet<String> {
    board(harness, id)
        .debug_statements()
        .iter()
        .flat_map(|(text, _)| text.lines().map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

fn wire<'a>(harness: &'a FolderHarness, id: &str, wire: &str) -> &'a DrawnWire {
    board(harness, id)
        .debug_wires()
        .iter()
        .find(|drawn| drawn.id == wire)
        .unwrap_or_else(|| panic!("the wire {wire} is drawn"))
}

/// The cards the last frame drew on the board, by id: the ones in view.
fn cards_on_screen(harness: &FolderHarness, id: &str) -> Vec<String> {
    let canvas = board(harness, id);
    canvas
        .debug_cards()
        .iter()
        .filter(|(card, _)| canvas.debug_is_drawn(card))
        .map(|(card, _)| card.clone())
        .collect()
}

/// Nothing on the board is lit, dimmed, ringed or stated.
fn assert_quiet(harness: &FolderHarness, id: &str, context: &str) {
    let canvas = board(harness, id);
    assert!(
        canvas.debug_selected_wires().is_empty(),
        "{context}: nothing kept"
    );
    assert!(
        canvas.debug_statements().is_empty(),
        "{context}: no statement: {:?}",
        canvas.debug_statements()
    );
    assert_eq!(canvas.debug_statement(), None, "{context}");
    assert!(rings(harness, id).is_empty(), "{context}: no ring");
    for drawn in canvas.debug_wires() {
        assert!(
            !drawn.lit && !drawn.dimmed,
            "{context}: {} as at rest",
            drawn.id
        );
    }
    for card in cards_on_screen(harness, id) {
        assert!(!canvas.debug_dimmed(&card), "{context}: {card} not dimmed");
    }
}

/// While `wire` is kept, every card not at its ends and every other line is
/// dimmed as LAY-10 dims; the wire is lit and its ends are not dimmed.
fn assert_kept(harness: &FolderHarness, id: &str, wires: &[String], ends: &[&str], context: &str) {
    let canvas = board(harness, id);
    let graph = graph(harness, id);
    for drawn in canvas.debug_wires() {
        if wires.contains(&drawn.id) {
            assert!(
                drawn.lit && !drawn.dimmed,
                "{context}: the kept wire {} is at full strength",
                drawn.id
            );
        } else {
            assert!(drawn.dimmed, "{context}: {} is dimmed", drawn.id);
        }
    }
    for card in cards_on_screen(harness, id) {
        let handle = handle_of(&graph, &card);
        assert_eq!(
            canvas.debug_dimmed(&card),
            !ends.contains(&handle.as_str()),
            "{context}: {handle} is dimmed unless it is an end"
        );
    }
}

/// Criterion 1: with the pointer within 6 px of the line `valid_cart →
/// checkout`, the line is drawn at full strength, the ports at both its
/// ends are ringed and the box at the pointer reads its statement; no card
/// and no other line changes. Moving the pointer away ends it.
#[test]
fn lay14_criterion_1_pointing_at_a_line_states_it() {
    let (_project, mut harness, id) = opened(WIDE);
    assert_quiet(&harness, &id, "at rest");
    let drawn = line(&harness, &id, "valid_cart", "checkout");
    let others: Vec<(String, Vec<Pos2>)> = routes(&harness, &id)
        .into_iter()
        .filter(|(other, _)| *other != drawn.id)
        .collect();
    let at = point_on(&harness, &id, &drawn.id) + vec2(0.0, 4.0);

    hover(&mut harness, at);
    let canvas = board(&harness, &id);
    assert_eq!(canvas.debug_statement().as_deref(), Some(LINE));
    let boxes = canvas.debug_statements();
    assert_eq!(boxes.len(), 1, "one box: {boxes:?}");
    assert_eq!(boxes[0].0, LINE);
    assert!(
        boxes[0].1.expand(40.0).contains(at),
        "the box stands at the pointer: {:?} for {at:?}",
        boxes[0].1
    );
    let lit = wire(&harness, &id, &drawn.id);
    assert!(lit.lit && !lit.dimmed, "the line at full strength");
    assert_ends_ringed(&harness, &id, &drawn.id, "pointing");
    let graph = graph(&harness, &id);
    let (source, target) = (
        id_of(&harness, &id, "valid_cart"),
        id_of(&harness, &id, "checkout"),
    );
    assert!(
        rings(&harness, &id)
            .iter()
            .all(|(card, _)| *card == source || *card == target),
        "only its ends are ringed: {:?}",
        rings(&harness, &id)
    );
    for other in canvas
        .debug_wires()
        .iter()
        .filter(|other| other.id != drawn.id)
    {
        assert!(!other.lit && !other.dimmed, "{} unchanged", other.id);
    }
    assert!(
        revealed(&harness, &id).is_empty(),
        "no chip's wire comes back"
    );
    assert_eq!(
        routes(&harness, &id)
            .into_iter()
            .filter(|(other, _)| *other != drawn.id)
            .collect::<Vec<_>>(),
        others,
        "no other line moves"
    );
    for card in cards_on_screen(&harness, &id) {
        assert!(
            !canvas.debug_dimmed(&card) && canvas.debug_ring(&card).is_none(),
            "{} unchanged",
            handle_of(&graph, &card)
        );
    }

    away(&mut harness);
    assert_quiet(&harness, &id, "the pointer gone");
}

/// Criterion 2: pointing at `checkout`'s *Body* row states both values that
/// feed it, the line's and the chip's, one per line, and draws both wires
/// lit with their ports ringed.
#[test]
fn lay14_criterion_2_a_row_names_every_value() {
    let (_project, mut harness, id) = opened(WIDE);
    let drawn = line(&harness, &id, "valid_cart", "checkout");
    let checkout = id_of(&harness, &id, "checkout");
    let chip = board(&harness, &id)
        .debug_tags()
        .iter()
        .find(|tag| tag.tag.statement == CHIP)
        .cloned()
        .expect("checkout's body carries the chip of expected_total");
    let row = board(&harness, &id)
        .debug_input_row(&checkout, "body")
        .expect("checkout shows its body");

    hover(&mut harness, row.center());
    let text = board(&harness, &id)
        .debug_statement()
        .expect("the row states its values");
    assert_eq!(
        text.lines().collect::<BTreeSet<_>>(),
        BTreeSet::from([LINE, CHIP]),
        "one line per value: {text:?}"
    );
    assert_eq!(
        statements(&harness, &id),
        BTreeSet::from([LINE.to_owned(), CHIP.to_owned()])
    );
    for traced in std::iter::once(&drawn.id).chain(&chip.tag.wires) {
        let lit = wire(&harness, &id, traced);
        assert!(lit.lit && !lit.dimmed, "{traced} at full strength");
        assert_ends_ringed(&harness, &id, traced, "the row");
    }
    for card in cards_on_screen(&harness, &id) {
        assert!(
            !board(&harness, &id).debug_dimmed(&card),
            "pointing dims nothing"
        );
    }
    away(&mut harness);
    assert_quiet(&harness, &id, "the pointer gone");

    // A click on the row still selects the card (3 = A).
    click(&mut harness, row.center());
    assert_eq!(
        folder(&harness)
            .selections
            .get(&id)
            .cloned()
            .unwrap_or_default(),
        vec![checkout],
        "the click selects checkout"
    );
    assert!(board(&harness, &id).debug_selected_wires().is_empty());
}

/// Criterion 3: a click on the line, or on the chip `expected_total` at
/// `checkout`, keeps the statement, the rings and the wire after the pointer
/// has left; every card not at its ends and every other line is dimmed;
/// Esc ends it, and so does a click on the empty board. Ctrl+click adds a
/// second one with its own statement. Delete still removes a kept line.
#[test]
fn lay14_criterion_3_a_click_keeps_it_until_esc() {
    let (_project, mut harness, id) = opened(WIDE);
    let drawn = line(&harness, &id, "valid_cart", "checkout");
    let at = point_on(&harness, &id, &drawn.id);
    let empty = empty_point(&harness, &id);
    let ends = ["valid_cart", "checkout"];

    // The line.
    click(&mut harness, at);
    assert_eq!(
        board(&harness, &id).debug_selected_wires(),
        std::slice::from_ref(&drawn.id),
        "the click keeps the line"
    );
    assert_eq!(statements(&harness, &id), BTreeSet::from([LINE.to_owned()]));
    assert_ends_ringed(&harness, &id, &drawn.id, "kept");
    assert_kept(
        &harness,
        &id,
        std::slice::from_ref(&drawn.id),
        &ends,
        "kept",
    );
    escape(&mut harness);
    assert_quiet(&harness, &id, "after Esc");

    click(&mut harness, at);
    assert_eq!(statements(&harness, &id), BTreeSet::from([LINE.to_owned()]));
    click(&mut harness, empty);
    assert_quiet(&harness, &id, "after a click on the empty board");

    // The chip.
    let chip = board(&harness, &id)
        .debug_tags()
        .iter()
        .find(|tag| tag.tag.statement == CHIP)
        .cloned()
        .expect("checkout's body carries the chip of expected_total");
    click(&mut harness, chip.rect.center());
    let mut kept = board(&harness, &id).debug_selected_wires().to_vec();
    kept.sort();
    let mut wires = chip.tag.wires.clone();
    wires.sort();
    assert_eq!(kept, wires, "the click keeps the chip's wire");
    assert_eq!(statements(&harness, &id), BTreeSet::from([CHIP.to_owned()]));
    for traced in &chip.tag.wires {
        assert_ends_ringed(&harness, &id, traced, "the chip kept");
    }
    assert_kept(
        &harness,
        &id,
        &chip.tag.wires,
        &["expected_total", "checkout"],
        "the chip kept",
    );

    // Ctrl+click adds the line, with its own statement.
    let at = point_on(&harness, &id, &drawn.id);
    ctrl_click(&mut harness, at);
    let kept = board(&harness, &id).debug_selected_wires().to_vec();
    assert!(
        kept.contains(&drawn.id) && chip.tag.wires.iter().all(|wire| kept.contains(wire)),
        "both kept: {kept:?}"
    );
    let boxes = board(&harness, &id).debug_statements();
    assert_eq!(boxes.len(), 2, "a box each: {boxes:?}");
    assert_eq!(
        statements(&harness, &id),
        BTreeSet::from([LINE.to_owned(), CHIP.to_owned()])
    );
    assert_ends_ringed(&harness, &id, &drawn.id, "both kept");
    escape(&mut harness);
    assert_quiet(&harness, &id, "after Esc");

    // Delete removes a kept line.
    click(&mut harness, at);
    harness.key_press(egui::Key::Delete);
    settle(&mut harness);
    harness.step();
    let graph = graph(&harness, &id);
    let (source, target) = (
        id_of(&harness, &id, "valid_cart"),
        id_of(&harness, &id, "checkout"),
    );
    assert!(
        !document_wires(&graph)
            .iter()
            .any(|wire| wire.kind == WireKind::Data
                && wire.source == source
                && wire.target == target),
        "Delete removed the line"
    );
}

/// Criterion 4: tracing moves no card and no camera, and the box is whole
/// inside the board at 960 × 540 and 1600 × 1000, also when the pointer is
/// at the board's edge.
#[test]
fn lay14_criterion_4_tracing_moves_nothing_and_fits() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = opened(size);
        let graph = graph(&harness, &id);
        let places = |harness: &FolderHarness| {
            let canvas = board(harness, &id);
            (
                canvas.debug_camera(),
                graph
                    .nodes
                    .iter()
                    .map(|node| canvas.debug_world(&node.id))
                    .collect::<Vec<_>>(),
            )
        };
        let before = places(&harness);
        let drawn = line(&harness, &id, "valid_cart", "checkout");
        let at = point_on(&harness, &id, &drawn.id);
        let inside = |harness: &FolderHarness, context: &str| {
            let canvas = board(harness, &id);
            let boxes = canvas.debug_statements();
            assert!(!boxes.is_empty(), "{size:?} {context}: a box is shown");
            for (text, rect) in boxes {
                assert!(
                    canvas.debug_board().contains_rect(rect),
                    "{size:?} {context}: {text:?} at {rect:?} inside {:?}",
                    canvas.debug_board()
                );
            }
        };

        hover(&mut harness, at);
        inside(&harness, "pointing");
        assert_eq!(places(&harness), before, "{size:?}: pointing moves nothing");
        click(&mut harness, at);
        inside(&harness, "kept");
        assert_eq!(places(&harness), before, "{size:?}: a click moves nothing");
        escape(&mut harness);

        // The same point of the line at the board's right edge, then at its
        // top edge.
        let edge = board(&harness, &id).debug_board();
        for (name, offset) in [
            ("right edge", vec2(edge.right() - 3.0 - at.x, 0.0)),
            ("top edge", vec2(0.0, edge.top() + 3.0 - at.y)),
        ] {
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .expect("the board is drawn")
                .debug_pan(offset);
            settle(&mut harness);
            harness.step();
            hover(&mut harness, at + offset);
            assert_eq!(
                board(&harness, &id).debug_statement().as_deref(),
                Some(LINE),
                "{size:?} {name}: the line is pointed at"
            );
            inside(&harness, name);
            away(&mut harness);
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .expect("the board is drawn")
                .debug_pan(-offset);
            settle(&mut harness);
            harness.step();
        }
    }
}

/// A capture that keeps the pointer where it is: what is pointed at is what
/// the view shows.
fn save(harness: &mut FolderHarness, name: &str) {
    harness.step();
    let image = harness
        .render()
        .unwrap_or_else(|error| panic!("cannot render {name}: {error}"));
    let directory = crate::testing::capture_dir();
    fs::create_dir_all(&directory).unwrap();
    image.save(directory.join(format!("{name}.png"))).unwrap();
}

/// The views of the QA section: a line pointed at, a line kept, the row of
/// `checkout`'s body pointed at and the chip `expected_total` kept, at
/// 1600 × 1000 and 960 × 540.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay14() {
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        let (_project, mut harness, id) = opened(size);
        let drawn = line(&harness, &id, "valid_cart", "checkout");
        let at = point_on(&harness, &id, &drawn.id);
        hover(&mut harness, at);
        save(&mut harness, &format!("lay14-wire-point-after-{suffix}"));
        click(&mut harness, at);
        save(&mut harness, &format!("lay14-wire-pin-after-{suffix}"));
        escape(&mut harness);

        let checkout = id_of(&harness, &id, "checkout");
        let row = board(&harness, &id)
            .debug_input_row(&checkout, "body")
            .expect("checkout shows its body");
        hover(&mut harness, row.center());
        save(&mut harness, &format!("lay14-row-point-after-{suffix}"));
        away(&mut harness);

        let chip = board(&harness, &id)
            .debug_tags()
            .iter()
            .find(|tag| tag.tag.statement == CHIP)
            .cloned()
            .expect("checkout's body carries the chip of expected_total");
        click(&mut harness, chip.rect.center());
        save(&mut harness, &format!("lay14-chip-click-after-{suffix}"));
        escape(&mut harness);
    }
}
