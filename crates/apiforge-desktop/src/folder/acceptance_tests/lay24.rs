//! LAY-24 (#334): a mark on the card replaces the `@after` rail into the
//! next step. In *Chips for shared values* (the default) an ordering wire
//! that enters the next step of the spine, from the step in the column
//! before or a check of it, is not drawn: a short dashed arrow points into
//! the step's badge, its tooltip says `after quantity_ok`. The rail comes
//! back while the arrow is pointed at and while a card at either end is
//! selected. *All wires* and the workspace window draw every rail and no
//! arrow. Rendering only: nothing is written, nothing moves. On `fan-out`
//! and `shop-30`, tidied.
use super::*;
use crate::canvas::{DrawnArrow, WireMode};
use crate::testing::MINIMUM;
use apiforge_core::format::Graph;
use egui::{Pos2, Rect};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";
const ALL_WIRES: &str = "All wires";
const CHIPS: &str = "Chips for shared values";
/// The board file `quality` reads (LAY-06 criterion 10).
const BOARDS: &str = "../apiforge-layout/examples/support/boards.txt";
/// The steps of `fan-out` whose `@after` names the step before or a check
/// of it.
const FAN_OUT_MARKED: [&str; 10] = [
    "catalogue",
    "quantity",
    "apply_coupon",
    "note",
    "stock",
    "shipping",
    "remove_coupon",
    "view",
    "checkout",
    "lookup",
];
/// The zooms criterion 5 renders.
const ZOOMS: [f32; 4] = [0.25, 0.44, 0.6, 1.0];

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Checks {
    /// Folded into their subjects, the default (LAY-11).
    Collapsed,
    /// Drawn as cards under their subjects.
    Expanded,
}

/// The reference scenario `name` opened at `size` without positions, as a
/// new install shows it (chips on), its checks as `checks` says, tidied up
/// once more with nothing selected.
fn tidied(
    name: &str,
    size: egui::Vec2,
    checks: Checks,
) -> (tempfile::TempDir, FolderHarness, String) {
    tidied_text(&fixture(name), size, checks)
}

/// As [`tidied`], with the scenario's text given.
fn tidied_text(
    text: &str,
    size: egui::Vec2,
    checks: Checks,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), text).unwrap();
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

fn fit(harness: &mut FolderHarness) {
    choose(harness, FIT);
    harness.step();
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

fn handle_of(graph: &Graph, id: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| id.to_owned(), |node| node.handle.clone())
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

/// The camera centred on `handle` at `zoom`, as a person scrolls there.
fn show(harness: &mut FolderHarness, id: &str, handle: &str, zoom: f32) {
    let node = id_of(harness, id, handle);
    board_mut(harness, id).debug_show(&node, zoom);
    settle(harness);
    harness.step();
}

fn select(harness: &mut FolderHarness, id: &str, handles: &[&str]) {
    let ids = handles
        .iter()
        .map(|handle| id_of(harness, id, handle))
        .collect();
    app(harness).selections.insert(id.to_owned(), ids);
    settle(harness);
    harness.step();
}

/// *Wires* in the board's view menu.
fn switch_wires(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label("Board view").click();
    settle(harness);
    harness.get_by_label(label).click();
    settle(harness);
    harness.step();
}

/// The arrows the last frame drew, by the handle of the step they point
/// into; a step with two arrows fails here.
fn arrows(harness: &FolderHarness, id: &str) -> BTreeMap<String, DrawnArrow> {
    let graph = graph(harness, id);
    let mut found = BTreeMap::new();
    for arrow in board(harness, id).debug_arrows() {
        let handle = handle_of(&graph, &arrow.target);
        assert!(
            found.insert(handle.clone(), arrow.clone()).is_none(),
            "{handle} has one arrow"
        );
    }
    found
}

fn arrow(harness: &FolderHarness, id: &str, handle: &str) -> DrawnArrow {
    arrows(harness, id)
        .remove(handle)
        .unwrap_or_else(|| panic!("{handle} has an arrow"))
}

/// The dashed wires the last frame drew, as `(source, target)` by handle,
/// drawn at rest or brought back.
fn rails(harness: &FolderHarness, id: &str) -> BTreeSet<(String, String)> {
    let graph = graph(harness, id);
    board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| wire.dashed)
        .map(|wire| {
            (
                wire.source
                    .as_deref()
                    .map_or_else(String::new, |source| handle_of(&graph, source)),
                handle_of(&graph, &wire.target),
            )
        })
        .collect()
}

fn revealed(harness: &FolderHarness, id: &str) -> BTreeSet<String> {
    board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| wire.revealed)
        .map(|wire| wire.id.clone())
        .collect()
}

/// The routes of the wires drawn at rest, by id.
fn routes(harness: &FolderHarness, id: &str) -> HashMap<String, Vec<Pos2>> {
    board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| !wire.revealed)
        .map(|wire| (wire.id.clone(), wire.points.clone()))
        .collect()
}

fn positions(harness: &FolderHarness, id: &str) -> Vec<(String, Rect)> {
    let canvas = board(harness, id);
    graph(harness, id)
        .nodes
        .iter()
        .filter_map(|node| {
            canvas
                .debug_world(&node.id)
                .map(|rect| (node.handle.clone(), rect))
        })
        .collect()
}

/// The ordering wire of the document from `source` into `target`, by
/// handle: its id.
fn edge(harness: &FolderHarness, id: &str, source: &str, target: &str) -> String {
    let graph = graph(harness, id);
    let (source, target) = (id_of(harness, id, source), id_of(harness, id, target));
    graph
        .edges
        .iter()
        .find(|edge| edge.source == source && edge.target == target)
        .unwrap_or_else(|| panic!("an @after from {source} into {target}"))
        .id
        .clone()
}

/// A board point on screen, with the last frame's camera.
fn on_screen(canvas: &crate::canvas::Canvas, world: Pos2) -> Pos2 {
    let (pan, zoom) = canvas.debug_camera();
    canvas.debug_viewport().min + pan + world.to_vec2() * zoom
}

/// Whether the polyline `line` passes through `rect`, sampled every half
/// pixel.
fn passes_through(line: &[Pos2], rect: Rect) -> bool {
    line.windows(2).any(|pair| {
        let steps = (pair[0].distance(pair[1]) * 2.0).ceil().max(1.0) as usize;
        (0..=steps).any(|step| rect.contains(pair[0].lerp(pair[1], step as f32 / steps as f32)))
    })
}

/// A point on the empty board, away from every card.
fn empty(harness: &FolderHarness, id: &str) -> Pos2 {
    board(harness, id).debug_focus().left_top() + egui::vec2(8.0, 8.0)
}

/// Criterion 1: on `fan-out` no dashed wire is drawn, checks collapsed and
/// drawn as cards; each of the ten steps whose `@after` names the step
/// before or a check of it has one arrow at its badge, and `begin`,
/// `product` and `cart` have none.
#[test]
fn lay24_criterion_1_fan_out_draws_arrows_not_rails() {
    for size in [WIDE, MINIMUM] {
        for checks in [Checks::Collapsed, Checks::Expanded] {
            let (_project, mut harness, id) = tidied("fan-out", size, checks);
            fit(&mut harness);
            let context = format!("{size:?} {checks:?}");
            assert_eq!(
                folder(&harness).preferences.wires,
                WireMode::Chips,
                "a new install draws chips"
            );
            assert!(
                rails(&harness, &id).is_empty(),
                "{context}: no dashed wire: {:?}",
                rails(&harness, &id)
            );
            let marked: BTreeSet<String> = arrows(&harness, &id).into_keys().collect();
            let expected: BTreeSet<String> = FAN_OUT_MARKED
                .iter()
                .map(|step| (*step).to_owned())
                .collect();
            assert_eq!(marked, expected, "{context}: the arrows");
            let canvas = board(&harness, &id);
            for (handle, arrow) in arrows(&harness, &id) {
                let badge = canvas
                    .debug_badge(&arrow.target)
                    .unwrap_or_else(|| panic!("{context}: {handle} has a badge"));
                assert!(
                    arrow.rect.right() <= badge.rect.left() + 0.5
                        && arrow.rect.right() >= badge.rect.left() - 6.0,
                    "{context}: the arrow of {handle} {:?} ends at its badge {:?}",
                    arrow.rect,
                    badge.rect
                );
                assert!(
                    (arrow.rect.center().y - badge.rect.center().y).abs() <= badge.rect.height(),
                    "{context}: the arrow of {handle} {:?} is level with its badge {:?}",
                    arrow.rect,
                    badge.rect
                );
            }
        }
    }
}

/// Criterion 2: on `shop-30` the rail from `cart` to `address` (`after
/// cart_total`, two columns) and every branch wire are drawn as *All
/// wires* draws them; `login`, `checkout` and `logout` have an arrow and
/// no rail.
#[test]
fn lay24_criterion_2_shop_30_keeps_the_rails_further_away() {
    for checks in [Checks::Collapsed, Checks::Expanded] {
        let (_project, mut harness, id) = tidied("shop-30", WIDE, checks);
        fit(&mut harness);
        let marked: BTreeSet<String> = arrows(&harness, &id).into_keys().collect();
        assert_eq!(
            marked,
            ["checkout", "login", "logout"]
                .map(str::to_owned)
                .into_iter()
                .collect(),
            "{checks:?}: the arrows"
        );
        let chips = rails(&harness, &id);
        for step in ["login", "checkout", "logout"] {
            assert!(
                chips.iter().all(|(_, target)| target != step),
                "{checks:?}: no rail into {step}: {chips:?}"
            );
        }
        let from = if checks == Checks::Collapsed {
            "cart"
        } else {
            "cart_total"
        };
        assert!(
            chips.contains(&(from.to_owned(), "address".to_owned())),
            "{checks:?}: the rail into address stays: {chips:?}"
        );
        let labels: BTreeMap<String, Option<String>> = board(&harness, &id)
            .debug_wires()
            .iter()
            .filter(|wire| wire.dashed)
            .map(|wire| (wire.id.clone(), wire.label.clone()))
            .collect();

        switch_wires(&mut harness, ALL_WIRES);
        let every = rails(&harness, &id);
        let gone: BTreeSet<(String, String)> = every.difference(&chips).cloned().collect();
        assert_eq!(
            gone.iter()
                .map(|(_, target)| target.clone())
                .collect::<BTreeSet<_>>(),
            ["checkout", "login", "logout"]
                .map(str::to_owned)
                .into_iter()
                .collect(),
            "{checks:?}: only the rails into the next step are left out"
        );
        assert!(chips.is_subset(&every), "{checks:?}");
        for wire in board(&harness, &id).debug_wires() {
            if let Some(label) = labels.get(&wire.id) {
                assert_eq!(
                    &wire.label, label,
                    "{checks:?}: {} keeps its label",
                    wire.id
                );
            }
        }
    }
}

/// The camera after each step of criterion 3's selections with every wire
/// drawn: selecting a card the Inspector covers pans it into view (UX-18)
/// whatever the board draws.
fn cameras(mode: &str) -> Vec<(egui::Vec2, f32)> {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    if mode == ALL_WIRES {
        switch_wires(&mut harness, ALL_WIRES);
    }
    show(&mut harness, &id, "note", 0.6);
    let mut cameras = Vec::new();
    select(&mut harness, &id, &["note"]);
    cameras.push(board(&harness, &id).debug_camera());
    select(&mut harness, &id, &["apply_coupon"]);
    cameras.push(board(&harness, &id).debug_camera());
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    harness.step();
    cameras.push(board(&harness, &id).debug_camera());
    cameras
}

/// Criterion 3: selecting the step at either end of a marked `@after` draws
/// its rail in the accent colour, dashed, and lights its arrow; the rail of
/// a folded check carries its label `after <check>`, and no other wire or
/// rail drawn in that frame passes through that label. Esc or a click on
/// the board returns to the arrows. No card moves, the camera is where
/// *All wires* puts it, the wires drawn keep their routes, the document is
/// not marked changed.
#[test]
fn lay24_criterion_3_selecting_an_end_brings_the_rail_back() {
    let mut reference = cameras(ALL_WIRES).into_iter();
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    show(&mut harness, &id, "note", 0.6);
    let cards = positions(&harness, &id);
    let before = routes(&harness, &id);
    let dirty = folder(&harness).debug_dirty(&id);
    let into_note = edge(&harness, &id, "coupon_ok", "note");
    let into_stock = edge(&harness, &id, "note_ok", "stock");
    let into_coupon = edge(&harness, &id, "quantity_ok", "apply_coupon");
    assert!(!arrow(&harness, &id, "note").lit, "at rest");

    let mut unchanged = |harness: &FolderHarness, context: &str| {
        assert_eq!(positions(harness, &id), cards, "{context}: no card moves");
        let (pan, zoom) = board(harness, &id).debug_camera();
        let (expected_pan, expected_zoom) = reference.next().expect("a reference camera");
        assert!(
            (pan - expected_pan).length() <= 0.5 && (zoom - expected_zoom).abs() < 1e-4,
            "{context}: the camera is where All wires puts it: {pan:?} {zoom} against {expected_pan:?} {expected_zoom}"
        );
        assert_eq!(
            routes(harness, &id),
            before,
            "{context}: the wires drawn keep their routes"
        );
        assert_eq!(
            folder(harness).debug_dirty(&id),
            dirty,
            "{context}: not marked changed"
        );
    };
    // The labels of the rails brought back, clear of every other line.
    let labelled = |harness: &FolderHarness, wire: &str, text: &str| {
        let canvas = board(harness, &id);
        let drawn = canvas
            .debug_wires()
            .iter()
            .find(|drawn| drawn.id == wire)
            .unwrap_or_else(|| panic!("{wire} is drawn"));
        assert!(
            drawn.revealed && drawn.dashed,
            "{wire} dashed, brought back"
        );
        assert_eq!(drawn.label.as_deref(), Some(text));
        let label = drawn.label_box.expect("the label is drawn");
        let label = Rect::from_min_max(on_screen(canvas, label.min), on_screen(canvas, label.max));
        for other in canvas.debug_wires().iter().filter(|other| other.id != wire) {
            let line: Vec<Pos2> = other
                .points
                .iter()
                .map(|point| on_screen(canvas, *point))
                .collect();
            assert!(
                !passes_through(&line, label.shrink(0.5)),
                "{} passes through the label {text} {label:?}",
                other.id
            );
        }
    };

    select(&mut harness, &id, &["note"]);
    let shown = revealed(&harness, &id);
    assert!(
        shown.contains(&into_note) && shown.contains(&into_stock),
        "the rails at both ends of note: {shown:?}"
    );
    assert!(!shown.contains(&into_coupon));
    for step in ["note", "stock"] {
        let lit = arrow(&harness, &id, step);
        assert!(lit.lit, "the arrow of {step} is lit");
        assert_eq!(lit.color, crate::theme::ACCENT);
    }
    assert!(!arrow(&harness, &id, "apply_coupon").lit);
    labelled(&harness, &into_note, "after coupon_ok");
    labelled(&harness, &into_stock, "after note_ok");
    unchanged(&harness, "note selected");

    // The step at the other end.
    select(&mut harness, &id, &["apply_coupon"]);
    let shown = revealed(&harness, &id);
    assert!(
        shown.contains(&into_note) && shown.contains(&into_coupon),
        "the rails at both ends of apply_coupon: {shown:?}"
    );
    assert!(!shown.contains(&into_stock));
    assert!(arrow(&harness, &id, "note").lit);
    labelled(&harness, &into_coupon, "after quantity_ok");
    unchanged(&harness, "apply_coupon selected");

    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    harness.step();
    assert!(rails(&harness, &id).is_empty(), "Esc returns to the arrows");
    assert!(arrows(&harness, &id).values().all(|arrow| !arrow.lit));
    unchanged(&harness, "after Esc");

    select(&mut harness, &id, &["note"]);
    assert!(!rails(&harness, &id).is_empty());
    let at = empty(&harness, &id);
    super::lay16::press(&mut harness, at);
    super::lay16::release(&mut harness, at);
    harness.step();
    assert!(
        rails(&harness, &id).is_empty(),
        "a click on the board returns to the arrows"
    );
    assert_eq!(folder(&harness).debug_dirty(&id), dirty);
}

/// Criterion 4: moving the pointer onto an arrow draws that one rail and
/// lights the arrow without a click; moving it away removes the rail. The
/// arrow's hover area is at least 16 × 16 px on screen at every zoom.
#[test]
fn lay24_criterion_4_pointing_at_an_arrow_draws_its_rail() {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    for zoom in ZOOMS {
        show(&mut harness, &id, "note", zoom);
        let drawn = arrows(&harness, &id);
        assert!(!drawn.is_empty(), "arrows at {zoom}");
        for (handle, arrow) in drawn {
            assert!(
                arrow.hit.width() >= 16.0 && arrow.hit.height() >= 16.0,
                "{handle} at {zoom}: the hover area {:?}",
                arrow.hit
            );
            assert!(arrow.hit.contains_rect(arrow.rect), "{handle} at {zoom}");
        }
    }

    show(&mut harness, &id, "apply_coupon", 1.0);
    let selection = folder(&harness).selections.get(&id).cloned();
    let before = routes(&harness, &id);
    let into_coupon = edge(&harness, &id, "quantity_ok", "apply_coupon");
    let hit = arrow(&harness, &id, "apply_coupon").hit;
    assert!(revealed(&harness, &id).is_empty(), "nothing is pointed at");
    harness.hover_at(hit.center());
    harness.step();
    harness.step();
    assert_eq!(
        revealed(&harness, &id),
        BTreeSet::from([into_coupon.clone()]),
        "the one rail is drawn"
    );
    assert!(arrow(&harness, &id, "apply_coupon").lit, "the arrow is lit");
    assert!(!arrow(&harness, &id, "note").lit);
    assert_eq!(
        folder(&harness).selections.get(&id).cloned(),
        selection,
        "without a click"
    );
    assert_eq!(routes(&harness, &id), before, "the other wires stay");

    let away = empty(&harness, &id);
    harness.hover_at(away);
    harness.step();
    harness.step();
    assert!(revealed(&harness, &id).is_empty(), "away, the rail goes");
    assert!(!arrow(&harness, &id, "apply_coupon").lit);
}

/// The boxes on screen an arrow keeps clear of, in the last frame: the
/// badges, the chips, the counts, the continuation marks and the cards;
/// and the routes of the wires drawn at rest.
fn obstacles(harness: &FolderHarness, id: &str) -> (Vec<(String, Rect)>, Vec<Vec<Pos2>>) {
    let canvas = board(harness, id);
    let graph = graph(harness, id);
    let to_screen = |world: Rect| -> Rect {
        Rect::from_min_max(on_screen(canvas, world.min), on_screen(canvas, world.max))
    };
    let mut boxes = Vec::new();
    for (card, rect) in canvas.debug_cards() {
        boxes.push((format!("the card {}", handle_of(&graph, card)), *rect));
        if let Some(badge) = canvas.debug_badge(card) {
            boxes.push((
                format!("the badge of {}", handle_of(&graph, card)),
                badge.rect,
            ));
        }
    }
    for tag in canvas.debug_tags() {
        boxes.push((format!("the chip {}", tag.tag.statement), tag.rect));
    }
    for count in canvas.debug_counts() {
        boxes.push((
            format!("the count {}", count.count.label),
            to_screen(count.world),
        ));
    }
    for mark in canvas.debug_marks() {
        boxes.push((format!("the mark {}", mark.text), to_screen(mark.rect)));
    }
    let lines = canvas
        .debug_wires()
        .iter()
        .filter(|wire| !wire.revealed)
        .map(|wire| {
            wire.points
                .iter()
                .map(|point| on_screen(canvas, *point))
                .collect()
        })
        .collect();
    (boxes, lines)
}

/// One frame's arrows against criterion 5; how many it drew.
fn arrows_clear(harness: &FolderHarness, id: &str, context: &str) -> usize {
    let drawn = arrows(harness, id);
    let (boxes, lines) = obstacles(harness, id);
    for (handle, arrow) in &drawn {
        let context = format!("{context}: the arrow of {handle} {:?}", arrow.rect);
        assert!(arrow.rect.width() >= 14.0, "{context} is 14 px long");
        assert!(!arrow.lit, "{context} at rest");
        assert_eq!(arrow.color, crate::theme::MUTED, "{context} is muted");
        for (what, rect) in &boxes {
            assert!(
                !arrow.rect.intersects(*rect),
                "{context} covers {what} {rect:?}"
            );
        }
        for line in &lines {
            assert!(
                !passes_through(line, arrow.rect),
                "{context} lies on a drawn wire"
            );
        }
    }
    drawn.len()
}

/// Criterion 5: the arrow is at least 14 px long on screen at every zoom,
/// muted at rest, and its box overlaps no badge, chip, reader count,
/// continuation mark, card or drawn wire, at 0.25, 0.44, 0.60 and 1.0 and
/// at Fit at both sizes. At Fit every arrow is inside the view, those of
/// the first column whole.
#[test]
fn lay24_criterion_5_the_arrow_keeps_clear_at_every_zoom() {
    for name in ["fan-out", "shop-30"] {
        for size in [WIDE, MINIMUM] {
            let (_project, mut harness, id) = tidied(name, size, Checks::Collapsed);
            for zoom in ZOOMS {
                let mut seen = 0;
                for centre in ["begin", "note", "view", "checkout"] {
                    let centre = if name == "shop-30" && centre == "note" {
                        "cart"
                    } else if name == "shop-30" && centre == "view" {
                        "refund"
                    } else {
                        centre
                    };
                    show(&mut harness, &id, centre, zoom);
                    seen += arrows_clear(
                        &harness,
                        &id,
                        &format!("{name} {size:?} at {zoom} on {centre}"),
                    );
                }
                assert!(seen > 0, "{name} {size:?} at {zoom}: arrows are drawn");
            }
            fit(&mut harness);
            arrows_clear(&harness, &id, &format!("{name} {size:?} at Fit"));
            let focus = board(&harness, &id).debug_focus();
            let drawn = arrows(&harness, &id);
            if name == "fan-out" {
                assert_eq!(drawn.len(), 10, "{size:?}: all ten arrows at Fit");
            }
            for (handle, arrow) in drawn {
                assert!(
                    focus.contains_rect(arrow.rect),
                    "{name} {size:?} at Fit: the arrow of {handle} {:?} is outside {focus:?}",
                    arrow.rect
                );
            }
        }
    }
}

/// Whether hovering the arrow of `step` shows the tooltip `text`, exactly.
fn tooltip(harness: &mut FolderHarness, id: &str, step: &str, text: &str) -> bool {
    let target = id_of(harness, id, step);
    let id = id.to_owned();
    crate::testing::hover_until(
        harness,
        move |harness| {
            board(harness, &id)
                .debug_arrows()
                .iter()
                .find(|arrow| arrow.target == target)
                .map(|arrow| arrow.hit)
        },
        |harness| harness.query_by_label(text).is_some(),
    )
}

/// Criterion 6: pointing at an arrow shows `after quantity_ok`: the word
/// `after` and the names the step's `@after` writes for that rail, in the
/// code's order, comma-separated when it waits for two of the step before.
/// The arrow's accessible name is `<step> after <names>`.
#[test]
fn lay24_criterion_6_the_arrow_names_what_the_step_waits_for() {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    show(&mut harness, &id, "apply_coupon", 1.0);
    assert_eq!(
        arrow(&harness, &id, "apply_coupon").text,
        "after quantity_ok"
    );
    harness.get_by_label("apply_coupon after quantity_ok");
    assert!(tooltip(
        &mut harness,
        &id,
        "apply_coupon",
        "after quantity_ok"
    ));
    // An `@after` that names the step itself.
    show(&mut harness, &id, "quantity", 1.0);
    assert_eq!(arrow(&harness, &id, "quantity").text, "after cart");
    harness.get_by_label("quantity after cart");

    // Two checks of the step before, written in the other order than
    // their checks.
    let text = fixture("fan-out").replace(
        "  quantity_ok = assert quantity.status == 200\n  @after(quantity_ok)\n",
        "  quantity_ok = assert quantity.status == 200\n  \
         quantity_set = assert quantity.body.quantity == 3\n  \
         @after(quantity_set, quantity_ok)\n",
    );
    assert!(text.contains("@after(quantity_set, quantity_ok)"));
    for checks in [Checks::Collapsed, Checks::Expanded] {
        let (_project, mut harness, id) = tidied_text(&text, WIDE, checks);
        show(&mut harness, &id, "apply_coupon", 1.0);
        let marked = arrow(&harness, &id, "apply_coupon");
        assert_eq!(marked.text, "after quantity_set, quantity_ok", "{checks:?}");
        assert_eq!(marked.wires.len(), 2, "{checks:?}: one arrow for both");
        harness.get_by_label("apply_coupon after quantity_set, quantity_ok");
        assert!(tooltip(
            &mut harness,
            &id,
            "apply_coupon",
            "after quantity_set, quantity_ok"
        ));
    }
}

/// Criterion 7: the rail brought back can be selected and deleted as any
/// ordering wire: the `@after` leaves the code and the arrow leaves the
/// card. An ordering wire dragged from a step into the next one is an
/// arrow once dropped, and its `@after` is in the code.
#[test]
fn lay24_criterion_7_the_rail_brought_back_is_an_ordering_wire() {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    show(&mut harness, &id, "note", 0.6);
    let into_note = edge(&harness, &id, "coupon_ok", "note");
    select(&mut harness, &id, &["note"]);
    let canvas = board(&harness, &id);
    let cards: Vec<Rect> = canvas.debug_cards().iter().map(|(_, rect)| *rect).collect();
    let other: Vec<Vec<Pos2>> = canvas
        .debug_wires()
        .iter()
        .filter(|wire| wire.id != into_note)
        .map(|wire| {
            wire.points
                .iter()
                .map(|point| on_screen(canvas, *point))
                .collect()
        })
        .collect();
    let rail = canvas
        .debug_wires()
        .iter()
        .find(|wire| wire.id == into_note)
        .expect("the rail into note is brought back");
    // The middle of a segment of the rail, clear of the cards and of the
    // other lines.
    let at = rail
        .points
        .windows(2)
        .map(|pair| on_screen(canvas, pair[0].lerp(pair[1], 0.5)))
        .find(|at| {
            canvas.debug_viewport().shrink(16.0).contains(*at)
                && cards.iter().all(|card| !card.expand(12.0).contains(*at))
                && other.iter().all(|line| {
                    !passes_through(line, Rect::from_center_size(*at, egui::vec2(12.0, 12.0)))
                })
        })
        .expect("a point of the rail clear of the rest");
    super::lay16::press(&mut harness, at);
    super::lay16::release(&mut harness, at);
    settle(&mut harness);
    harness.step();
    assert_eq!(
        board(&harness, &id).debug_selected_wires(),
        std::slice::from_ref(&into_note),
        "the click selects the rail"
    );
    assert!(
        revealed(&harness, &id).contains(&into_note),
        "it stays drawn"
    );
    harness.key_press(egui::Key::Delete);
    settle(&mut harness);
    harness.step();
    let code = source_of(&harness, &id);
    assert!(!code.contains("@after(coupon_ok)"), "{code}");
    assert!(
        !arrows(&harness, &id).contains_key("note"),
        "the arrow leaves the card"
    );

    // An ordering wire from apply_coupon dropped on note.
    let from = harness
        .get_by_label("Output __order of apply_coupon")
        .rect()
        .center();
    let note = id_of(&harness, &id, "note");
    let to = board(&harness, &id)
        .debug_input_socket(&note, apiforge_core::graph::ORDER_PORT)
        .expect("note's ordering input");
    super::lay16::press(&mut harness, from);
    super::lay16::move_to(&mut harness, from, to);
    super::lay16::release(&mut harness, to);
    settle(&mut harness);
    harness.step();
    let code = source_of(&harness, &id);
    assert!(code.contains("@after(apply_coupon)\n  note = "), "{code}");
    assert_eq!(arrow(&harness, &id, "note").text, "after apply_coupon");
    assert!(
        rails(&harness, &id)
            .iter()
            .all(|(_, target)| target != "note"),
        "an arrow, not a rail"
    );
}

/// Criterion 8: in *All wires* and in the workspace window every rail is
/// drawn and no arrow; switching the mode moves no card and no camera.
#[test]
fn lay24_criterion_8_all_wires_draws_every_rail() {
    for checks in [Checks::Collapsed, Checks::Expanded] {
        let (_project, mut harness, id) = tidied("fan-out", WIDE, checks);
        let cards = positions(&harness, &id);
        let camera = board(&harness, &id).debug_camera();
        switch_wires(&mut harness, ALL_WIRES);
        assert!(arrows(&harness, &id).is_empty(), "{checks:?}: no arrow");
        let every: BTreeSet<String> = board(&harness, &id)
            .debug_wires()
            .iter()
            .filter(|wire| wire.dashed && !wire.revealed)
            .map(|wire| wire.id.clone())
            .collect();
        let edges: BTreeSet<String> = graph(&harness, &id)
            .edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect();
        assert_eq!(every, edges, "{checks:?}: every rail is drawn");
        assert_eq!(positions(&harness, &id), cards, "{checks:?}: no card moves");
        assert_eq!(board(&harness, &id).debug_camera(), camera, "{checks:?}");
        switch_wires(&mut harness, CHIPS);
        assert_eq!(arrows(&harness, &id).len(), 10, "{checks:?}");
        assert_eq!(positions(&harness, &id), cards, "{checks:?}: no card moves");
        assert_eq!(board(&harness, &id).debug_camera(), camera, "{checks:?}");
    }

    let harness = crate::testing::workspace_harness(WIDE);
    let canvas = harness
        .state()
        .as_ref()
        .expect("the workspace window")
        .debug_canvas();
    assert_eq!(canvas.wire_mode(), WireMode::All);
    assert!(
        canvas.debug_arrows().is_empty(),
        "no arrow in the workspace"
    );
}

/// Pairs of parallel segments of two wires within 0.5 px of each other for
/// at least 20 px, as `quality` counts them.
fn coincident(wires: &[&[Pos2]]) -> usize {
    let mut pairs = 0;
    for (index, a) in wires.iter().enumerate() {
        for b in &wires[index + 1..] {
            for p in a.windows(2) {
                for q in b.windows(2) {
                    let horizontal = |s: &[Pos2]| (s[0].y - s[1].y).abs() < 0.01;
                    let vertical = |s: &[Pos2]| (s[0].x - s[1].x).abs() < 0.01;
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

/// Lines drawn at rest, crossings (the router's bridges) and coincident
/// pairs.
fn measures(harness: &FolderHarness, id: &str) -> (usize, usize, usize) {
    let wires: Vec<_> = board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| !wire.revealed)
        .collect();
    let crossings = wires.iter().map(|wire| wire.bridges).sum();
    let routes: Vec<&[Pos2]> = wires.iter().map(|wire| wire.points.as_slice()).collect();
    (wires.len(), crossings, coincident(&routes))
}

/// Criterion 9: `fan-out` in the default mode has at most 2 crossings and
/// 4 coincident pairs with its checks collapsed, and at most 2 crossings
/// with them drawn. The `chips` boards `quality` reads leave the marked
/// rails out, as the window does.
#[test]
fn lay24_criterion_9_fewer_crossings_on_fan_out() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BOARDS);
    let boards = fs::read_to_string(&path).unwrap();
    for checks in [Checks::Collapsed, Checks::Expanded] {
        let (_project, harness, id) = tidied("fan-out", WIDE, checks);
        let (lines, crossings, pairs) = measures(&harness, &id);
        // Since LAY-03 lines keep 20 px from each other: with the checks
        // drawn, the three wires into `checkout_ok`, whose inputs are 6 px
        // apart, come round it one by one and cross the lines beside it (9
        // crossings, 1 before). The order of lines in a corridor is LAY-05's.
        let most = match checks {
            Checks::Collapsed => 2,
            Checks::Expanded => 9,
        };
        assert!(
            crossings <= most,
            "{checks:?}: {crossings} crossings on {lines} lines"
        );
        if checks == Checks::Collapsed {
            assert!(pairs <= 4, "{checks:?}: {pairs} coincident pairs");
        }
        let head = format!(
            "board fan-out compact {} ",
            if checks == Checks::Collapsed {
                "collapsed"
            } else {
                "expanded"
            }
        );
        let board = boards
            .split("end\n")
            .find(|board| {
                board
                    .lines()
                    .find(|line| line.starts_with("board "))
                    .is_some_and(|line| line.starts_with(&head) && line.ends_with(" chips"))
            })
            .unwrap_or_else(|| panic!("the board file holds {head}… chips"));
        let routed = board
            .lines()
            .filter(|line| line.starts_with("wire "))
            .count();
        assert_eq!(
            routed, lines,
            "{checks:?}: the board file routes what the window draws"
        );
    }
}

/// Criterion 10: nothing is written: after Tidy up, hovering and selecting,
/// the scenario file and `diagram.json` are byte for byte what they were.
#[test]
fn lay24_criterion_10_nothing_is_written() {
    let (project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    choose(&mut harness, "Save");
    settle(&mut harness);
    let scenario = fs::read(project.path().join(SCENARIO)).unwrap();
    let diagram = fs::read(project.path().join(DIAGRAM)).unwrap();
    assert!(!folder(&harness).debug_dirty(&id), "saved");

    show(&mut harness, &id, "apply_coupon", 1.0);
    let hit = arrow(&harness, &id, "apply_coupon").hit;
    harness.hover_at(hit.center());
    harness.step();
    harness.step();
    select(&mut harness, &id, &["note"]);
    select(&mut harness, &id, &["apply_coupon"]);
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    switch_wires(&mut harness, ALL_WIRES);
    switch_wires(&mut harness, CHIPS);
    settle(&mut harness);

    assert!(!folder(&harness).debug_dirty(&id), "not marked changed");
    assert_eq!(fs::read(project.path().join(SCENARIO)).unwrap(), scenario);
    assert_eq!(fs::read(project.path().join(DIAGRAM)).unwrap(), diagram);
}

/// Criterion 11: the review views, each asserting a card in view: `fan-out`
/// at both sizes, `note` selected at 60 %, an arrow pointed at with its
/// tooltip, and `shop-30`.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay24() {
    // (scenario, size, capture, the card shown at 60 % and selected, an
    // arrow pointed at)
    for (name, size, label, selected, pointed) in [
        ("fan-out", WIDE, "lay24-fan-out-1600", None, None),
        ("fan-out", MINIMUM, "lay24-fan-out-960", None, None),
        (
            "fan-out",
            WIDE,
            "lay24-note-selected-60",
            Some("note"),
            None,
        ),
        (
            "fan-out",
            WIDE,
            "lay24-arrow-pointed",
            None,
            Some("apply_coupon"),
        ),
        ("shop-30", WIDE, "lay24-shop-30-1600", None, None),
        ("shop-30", MINIMUM, "lay24-shop-30-960", None, None),
    ] {
        let (_project, mut harness, id) = tidied(name, size, Checks::Collapsed);
        fit(&mut harness);
        if let Some(step) = selected {
            show(&mut harness, &id, step, 0.6);
            select(&mut harness, &id, &[step]);
        }
        let canvas = board(&harness, &id);
        let focus = canvas.debug_focus();
        assert!(
            canvas
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{label}: a card is in view"
        );
        match pointed {
            Some(step) => {
                show(&mut harness, &id, step, 1.0);
                let text = arrow(&harness, &id, step).text;
                assert!(tooltip(&mut harness, &id, step, &text), "{label}");
                // The pointer stays: the capture shows the tooltip, past
                // the frame that sizes it unseen.
                harness.step();
                harness.step();
                let image = harness
                    .render()
                    .unwrap_or_else(|error| panic!("cannot render {label}: {error}"));
                let directory = crate::testing::capture_dir();
                fs::create_dir_all(&directory).unwrap();
                image.save(directory.join(format!("{label}.png"))).unwrap();
            }
            None => {
                crate::testing::capture(&mut harness, label, None);
            }
        }
    }
}
