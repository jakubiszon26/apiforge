//! LAY-23 (#333), stage 1: a value read by three or more steps draws a quiet
//! chip `← name` beside each reader's input instead of a long wire; a reader
//! beside its source keeps its wire; the source counts its readers; the wire
//! comes back while the chip or the input is pointed at and while the reader
//! or the source is selected. A *Wires* setting in the board's view menu
//! (*All wires*, *Chips for shared values*), *Chips* the default. Rendering
//! only: nothing is written, nothing moves. On `fan-out` and `shop-30`,
//! tidied.
use super::*;
use crate::canvas::{Command, WireMode};
use crate::preferences::{Preference, Preferences};
use apiforge_core::format::{Binding, Graph};
use egui::{Pos2, Rect};
use std::collections::{BTreeSet, HashMap};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";
const ALL_WIRES: &str = "All wires";
const CHIPS: &str = "Chips for shared values";
/// The board file `quality` reads (LAY-06 criterion 10).
const BOARDS: &str = "../apiforge-layout/examples/support/boards.txt";

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

/// The chips the last frame drew, as `(reader, source text)` by handle.
fn chips(harness: &FolderHarness, id: &str) -> BTreeSet<(String, String)> {
    let graph = graph(harness, id);
    board(harness, id)
        .debug_tags()
        .iter()
        .map(|tag| (handle_of(&graph, &tag.tag.reader), tag.tag.text.clone()))
        .collect()
}

/// The wires the last frame drew at rest, as `(source, target)` by handle;
/// a wire drawn because it is pointed at or selected is not one of them.
fn wires(harness: &FolderHarness, id: &str) -> BTreeSet<(String, String)> {
    let graph = graph(harness, id);
    board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| !wire.dashed && !wire.revealed)
        .filter_map(|wire| {
            Some((
                handle_of(&graph, wire.source.as_deref()?),
                handle_of(&graph, &wire.target),
            ))
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

/// The steps that read `port` of `handle` in the document.
fn readers_in_document(graph: &Graph, handle: &str) -> BTreeSet<String> {
    let source = graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap()
        .id
        .clone();
    apiforge_core::graph::wires(graph)
        .into_iter()
        .filter(|wire| wire.source == source && wire.source_port.is_some())
        .map(|wire| handle_of(graph, &wire.target))
        .collect()
}

/// The reader with the most chips, the first by name among equals:
/// `checkout` on `fan-out`.
fn reader_with_chips(harness: &FolderHarness, id: &str) -> String {
    let graph = graph(harness, id);
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for tag in board(harness, id).debug_tags() {
        *counts
            .entry(handle_of(&graph, &tag.tag.reader))
            .or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(reader, _)| reader)
        .expect("the board draws chips")
}

/// Criterion 1: on `fan-out` no wire is drawn from `cartId` or `itemId` to
/// a reader that is not beside them; each such input carries a chip that
/// names its source, two chips where an address reads both; `quantity`,
/// beside them, keeps both wires; a port read by fewer than three steps
/// keeps all of its wires.
#[test]
fn lay23_criterion_1_a_fanned_out_port_draws_chips() {
    let (_project, harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    assert_eq!(
        folder(&harness).preferences.wires,
        WireMode::Chips,
        "a new install draws chips"
    );
    let graph = graph(&harness, &id);
    let drawn = wires(&harness, &id);
    let chips = chips(&harness, &id);
    for value in ["cartId", "itemId"] {
        let readers = readers_in_document(&graph, value);
        assert!(readers.len() >= 3, "{value} is read by {readers:?}");
        for reader in &readers {
            if reader == "quantity" {
                assert!(
                    drawn.contains(&(value.to_owned(), reader.clone())),
                    "quantity, beside {value}, keeps its wire: {drawn:?}"
                );
                continue;
            }
            if board(&harness, &id)
                .debug_world(&id_of(&harness, &id, reader))
                .is_none()
            {
                // A folded check is not drawn, and neither is its chip.
                continue;
            }
            assert!(
                !drawn.contains(&(value.to_owned(), reader.clone())),
                "no wire from {value} to {reader}"
            );
            assert!(
                chips.contains(&(reader.clone(), value.to_owned())),
                "{reader} carries a chip of {value}: {chips:?}"
            );
        }
    }
    // An address that reads both carries two chips.
    for reader in ["note", "stock"] {
        assert!(chips.contains(&(reader.to_owned(), "cartId".to_owned())));
        assert!(chips.contains(&(reader.to_owned(), "itemId".to_owned())));
    }
    // Fewer than three readers: every wire stays.
    for (source, target) in [("catalogue", "first"), ("checkout", "orderNumber")] {
        assert!(
            drawn.contains(&(source.to_owned(), target.to_owned())),
            "{source} -> {target} stays a wire: {drawn:?}"
        );
    }
    assert!(
        !chips.iter().any(|(_, text)| text.starts_with("catalogue")
            || text.starts_with("checkout")
            || text == "first"
            || text == "orderNumber"),
        "no chip for a port with fewer than three readers: {chips:?}"
    );
}

/// Criterion 2: the port of `cartId` reads `→ 9 readers`, the number of
/// steps that read it in the document; the *Checks* toggle changes neither
/// that number nor which wires are chips.
#[test]
fn lay23_criterion_2_the_source_counts_its_readers() {
    let mut seen: Vec<BTreeSet<(String, String)>> = Vec::new();
    for checks in [Checks::Collapsed, Checks::Expanded] {
        let (_project, harness, id) = tidied("fan-out", WIDE, checks);
        let graph = graph(&harness, &id);
        assert_eq!(readers_in_document(&graph, "cartId").len(), 9);
        let cart_id = id_of(&harness, &id, "cartId");
        let counts = board(&harness, &id).debug_counts();
        let count = counts
            .iter()
            .find(|count| count.count.source == cart_id)
            .unwrap_or_else(|| panic!("{checks:?}: cartId counts its readers"));
        assert_eq!(count.count.readers, 9, "{checks:?}");
        assert_eq!(count.count.label, "cartId → 9 readers", "{checks:?}");
        assert!(
            harness.query_by_label("cartId → 9 readers").is_some(),
            "{checks:?}: the mark is found by its name"
        );
        let item_id = id_of(&harness, &id, "itemId");
        let items = counts
            .iter()
            .find(|count| count.count.source == item_id)
            .expect("itemId counts its readers");
        assert_eq!(
            items.count.readers, 6,
            "{checks:?}: checks count as readers"
        );
        // Which wires are chips, on the readers both boards draw.
        let drawn_in_both: BTreeSet<String> = ["quantity", "apply_coupon", "note", "stock"]
            .into_iter()
            .chain(["shipping", "remove_coupon", "view", "checkout", "lookup"])
            .map(str::to_owned)
            .collect();
        seen.push(
            chips(&harness, &id)
                .into_iter()
                .filter(|(reader, _)| drawn_in_both.contains(reader))
                .collect(),
        );
    }
    assert_eq!(
        seen[0], seen[1],
        "the Checks toggle turns no wire into a chip"
    );
}

/// Criterion 3: pointing at a chip or at its input draws that wire from the
/// source's port to the input at full strength and shows the statement
/// `cartId → checkout.body.cartId`; moving the pointer away removes both.
#[test]
fn lay23_criterion_3_pointing_at_a_chip_draws_its_wire() {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    show(&mut harness, &id, "checkout", 1.0);
    let statement = "cartId → checkout.body.cartId";
    let checkout = id_of(&harness, &id, "checkout");
    let tag = board(&harness, &id)
        .debug_tags()
        .iter()
        .find(|tag| tag.tag.statement == statement)
        .cloned()
        .expect("checkout's body carries the chip of cartId");
    assert!(revealed(&harness, &id).is_empty(), "nothing is pointed at");
    let before = routes(&harness, &id);

    // The chip, found by its accessible name.
    harness.get_by_label(statement).hover();
    harness.step();
    harness.step();
    let shown = revealed(&harness, &id);
    assert!(
        tag.tag.wires.iter().all(|wire| shown.contains(wire)),
        "the chip's wire is drawn: {shown:?}"
    );
    assert_eq!(
        board(&harness, &id).debug_statement().as_deref(),
        Some(statement)
    );
    let wire = board(&harness, &id)
        .debug_wires()
        .iter()
        .find(|wire| wire.revealed)
        .cloned()
        .unwrap();
    assert_eq!(wire.target, checkout, "into the reader's input");
    assert_eq!(
        wire.source.as_deref(),
        Some(id_of(&harness, &id, "cartId").as_str()),
        "from the source's port"
    );
    assert!(wire.points.len() >= 2, "routed");
    assert_eq!(routes(&harness, &id), before, "the other wires stay");

    // Away from it: both go.
    let empty = board(&harness, &id).debug_focus().left_top() + egui::vec2(8.0, 8.0);
    harness.hover_at(empty);
    harness.step();
    harness.step();
    assert!(revealed(&harness, &id).is_empty());
    assert_eq!(board(&harness, &id).debug_statement(), None);

    // The input it reads into.
    let row = board(&harness, &id)
        .debug_input_row(&checkout, "body")
        .expect("checkout shows its body");
    harness.hover_at(row.center());
    harness.step();
    harness.step();
    let shown = revealed(&harness, &id);
    assert!(
        tag.tag.wires.iter().all(|wire| shown.contains(wire)),
        "pointing at the input draws the wire: {shown:?}"
    );
    assert!(
        board(&harness, &id)
            .debug_statement()
            .is_some_and(|text| text.contains(statement)),
        "{:?}",
        board(&harness, &id).debug_statement()
    );
    harness.hover_at(empty);
    harness.step();
    harness.step();
    assert!(revealed(&harness, &id).is_empty());
}

/// Criterion 4: selecting a reader draws the wires of all its chips;
/// selecting the source, or clicking the mark at its port, draws every
/// wire of the port; Esc or a click on the board returns to chips. No card
/// and no camera moves, the wires that were drawn keep their routes, and
/// the document is not marked changed.
#[test]
fn lay23_criterion_4_selection_brings_the_wires_back() {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    choose(&mut harness, FIT);
    harness.step();
    let cards = positions(&harness, &id);
    let camera = board(&harness, &id).debug_camera();
    let before = routes(&harness, &id);
    let dirty = folder(&harness).debug_dirty(&id);
    let tags = board(&harness, &id).debug_tags().to_vec();
    let of = |reader: &str| -> BTreeSet<String> {
        let reader = id_of(&harness, &id, reader);
        tags.iter()
            .filter(|tag| tag.tag.reader == reader)
            .flat_map(|tag| tag.tag.wires.clone())
            .collect()
    };
    let checkout = of("checkout");
    assert!(checkout.len() >= 2, "checkout reads cartId and itemId");
    let cart_id = id_of(&harness, &id, "cartId");
    let port: BTreeSet<String> = tags
        .iter()
        .filter(|tag| tag.tag.source == cart_id)
        .flat_map(|tag| tag.tag.wires.clone())
        .collect();

    let unchanged = |harness: &FolderHarness, context: &str| {
        assert_eq!(positions(harness, &id), cards, "{context}: no card moves");
        assert_eq!(
            board(harness, &id).debug_camera(),
            camera,
            "{context}: the camera stays"
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

    select(&mut harness, &id, &["checkout"]);
    assert_eq!(revealed(&harness, &id), checkout, "a reader selected");
    unchanged(&harness, "a reader selected");

    select(&mut harness, &id, &["cartId"]);
    assert_eq!(revealed(&harness, &id), port, "the source selected");
    unchanged(&harness, "the source selected");

    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    harness.step();
    assert!(revealed(&harness, &id).is_empty(), "Esc returns to chips");
    unchanged(&harness, "after Esc");

    // The mark at the port.
    show(&mut harness, &id, "cartId", 1.0);
    let camera_at = board(&harness, &id).debug_camera();
    harness.get_by_label("cartId → 9 readers").click();
    settle(&mut harness);
    harness.step();
    assert_eq!(revealed(&harness, &id), port, "the mark draws the port");
    assert_eq!(board(&harness, &id).debug_camera(), camera_at);
    // A click on the empty board.
    let empty = board(&harness, &id).debug_focus().left_top() + egui::vec2(8.0, 8.0);
    super::lay16::press(&mut harness, empty);
    super::lay16::release(&mut harness, empty);
    harness.step();
    assert!(
        revealed(&harness, &id).is_empty(),
        "a click on the board returns to chips"
    );
    assert_eq!(folder(&harness).debug_dirty(&id), dirty);
}

/// Criterion 5: nothing is written: after Tidy up, hovering and selecting,
/// the scenario file and `diagram.json` are byte for byte what they were,
/// and the document has no new proxy.
#[test]
fn lay23_criterion_5_nothing_is_written() {
    let (project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    choose(&mut harness, "Save");
    settle(&mut harness);
    let scenario = fs::read(project.path().join(SCENARIO)).unwrap();
    let diagram = fs::read(project.path().join(DIAGRAM)).unwrap();
    let proxies = graph(&harness, &id).proxies.len();
    assert!(!folder(&harness).debug_dirty(&id), "saved");

    show(&mut harness, &id, "checkout", 1.0);
    harness
        .get_by_label("cartId → checkout.body.cartId")
        .hover();
    harness.step();
    select(&mut harness, &id, &["checkout"]);
    select(&mut harness, &id, &["cartId"]);
    switch_wires(&mut harness, ALL_WIRES);
    switch_wires(&mut harness, CHIPS);
    settle(&mut harness);

    assert!(!folder(&harness).debug_dirty(&id), "not marked changed");
    assert_eq!(graph(&harness, &id).proxies.len(), proxies, "no proxy");
    assert_eq!(fs::read(project.path().join(SCENARIO)).unwrap(), scenario);
    assert_eq!(fs::read(project.path().join(DIAGRAM)).unwrap(), diagram);
}

/// Criterion 6: a chip's text is at least 9 px on screen at every zoom and
/// a chip never covers a card, a continuation mark or the tray of a
/// selected card: where the gap has no room it says the source step, then
/// the arrow alone. Fit shows every chip whole, the first column's
/// included.
#[test]
fn lay23_criterion_6_chips_fit_the_gap_at_every_zoom() {
    for name in ["fan-out", "shop-30"] {
        let (_project, mut harness, id) = tidied(name, WIDE, Checks::Collapsed);
        let graph = graph(&harness, &id);
        let mut shortened = false;
        let mut arrows = false;
        for zoom in [0.25, 0.44, 0.6, 1.0] {
            let centre = if name == "fan-out" {
                "checkout"
            } else {
                "cart"
            };
            show(&mut harness, &id, centre, zoom);
            let canvas = board(&harness, &id);
            let cards: Vec<Rect> = graph
                .nodes
                .iter()
                .filter_map(|node| canvas.debug_world(&node.id))
                .collect();
            let marks: Vec<Rect> = canvas.debug_marks().iter().map(|mark| mark.rect).collect();
            let drawn = canvas.debug_tags();
            assert!(!drawn.is_empty(), "{name} at {zoom} draws chips");
            for tag in drawn {
                let context = format!("{name} at {zoom}: {} {:?}", tag.tag.statement, tag.world);
                assert!(tag.font >= 9.0, "{context}: {} px", tag.font);
                for card in &cards {
                    assert!(!tag.world.intersects(*card), "{context} covers {card:?}");
                }
                for mark in &marks {
                    assert!(!tag.world.intersects(*mark), "{context} covers a mark");
                }
                let arrow_only = tag.shown == "←";
                let short = tag.shown == format!("← {}", tag.tag.short);
                assert!(
                    tag.shown == format!("← {}", tag.tag.text) || short || arrow_only,
                    "{context}: {}",
                    tag.shown
                );
                shortened |= short && tag.tag.short != tag.tag.text;
                arrows |= arrow_only;
            }
            for count in canvas.debug_counts() {
                for card in &cards {
                    assert!(
                        !count.world.intersects(*card),
                        "{name} at {zoom}: {} covers a card",
                        count.count.label
                    );
                }
                for mark in &marks {
                    assert!(
                        !count.world.intersects(*mark),
                        "{name} at {zoom}: {} covers a mark",
                        count.count.label
                    );
                }
            }
        }
        assert!(arrows, "{name}: at 0.25 a chip is the arrow alone");
        if name == "shop-30" {
            assert!(
                shortened,
                "{name}: a chip says the source step where it must"
            );
        }

        // A selected card's trays are not covered.
        let reader = reader_with_chips(&harness, &id);
        let reader = reader.as_str();
        show(&mut harness, &id, reader, 1.0);
        select(&mut harness, &id, &[reader]);
        let reader_id = id_of(&harness, &id, reader);
        if let Some(tray) = board(&harness, &id).debug_input_tray(&reader_id) {
            for tag in board(&harness, &id).debug_tags() {
                assert!(
                    !tag.world.intersects(tray),
                    "{name}: {} covers the tray of {reader}",
                    tag.tag.statement
                );
            }
        }
        select(&mut harness, &id, &[]);

        // Fit shows every chip whole.
        choose(&mut harness, FIT);
        harness.step();
        let canvas = board(&harness, &id);
        let focus = canvas.debug_focus();
        for tag in canvas.debug_tags() {
            assert!(
                focus.contains_rect(tag.rect),
                "{name} at Fit: {} {:?} is outside {focus:?}",
                tag.tag.statement,
                tag.rect
            );
        }
    }
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

/// Lines drawn, crossings (the router's bridges) and coincident pairs.
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

/// Criterion 7: with chips on, `fan-out` and `shop-30` have at most 0.6
/// crossings per line drawn, checks collapsed and drawn, and `shop-30` with
/// checks drawn at most 30 coincident pairs. `fan-out` joins the reference
/// scenarios and `quality` prints each reference board with chips on and
/// off.
#[test]
fn lay23_criterion_7_fewer_crossings_per_line() {
    for name in ["fan-out", "shop-30"] {
        for checks in [Checks::Collapsed, Checks::Expanded] {
            let (_project, harness, id) = tidied(name, WIDE, checks);
            let (lines, crossings, pairs) = measures(&harness, &id);
            let per_line = crossings as f32 / lines as f32;
            assert!(
                per_line <= 0.6,
                "{name} {checks:?}: {crossings} crossings on {lines} lines"
            );
            if name == "shop-30" && checks == Checks::Expanded {
                assert!(pairs <= 30, "{name} {checks:?}: {pairs} coincident pairs");
            }
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BOARDS);
    let boards = fs::read_to_string(&path).unwrap();
    for name in ["order-flow", "shop-30", "shop-50", "fan-out"] {
        for checks in ["collapsed", "expanded"] {
            for wires in ["all", "chips"] {
                let head = format!("board {name} compact {checks} ");
                assert!(
                    boards
                        .lines()
                        .any(|line| line.starts_with(&head) && line.ends_with(&format!(" {wires}"))),
                    "the board file holds {name} compact {checks} with {wires} wires"
                );
            }
        }
    }
    let readme = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/README.md"),
    )
    .unwrap();
    assert!(
        readme.contains("`fan-out`"),
        "fan-out is a reference scenario"
    );
}

/// Criterion 8: the view menu has *Wires* with *All wires* and *Chips for
/// shared values*, kept in the application preferences, *Chips* the
/// default; *All wires* draws exactly the wires the board draws today;
/// switching moves no card and no camera, and Tidy up gives the same
/// positions in either.
#[test]
fn lay23_criterion_8_the_setting_moves_nothing_and_restores_every_wire() {
    // The preference: on by default, remembered, and read as chips from a
    // file written before it existed.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    assert_eq!(Preferences::at(Some(path.clone())).wires, WireMode::Chips);
    let mut preferences = Preferences::at(Some(path.clone()));
    preferences.wires = WireMode::All;
    preferences.save(Preference::Wires);
    assert_eq!(Preferences::at(Some(path.clone())).wires, WireMode::All);
    fs::write(
        &path,
        br#"{"version": 1, "wireStyle": "orthogonal", "bridges": true, "codePanel": false, "compactCards": true, "checksCollapsed": true}"#,
    )
    .unwrap();
    assert_eq!(Preferences::at(Some(path)).wires, WireMode::Chips);

    let (_project, mut harness, id) = tidied("shop-30", WIDE, Checks::Collapsed);
    let cards = positions(&harness, &id);
    let camera = board(&harness, &id).debug_camera();
    let chipped: BTreeSet<String> = board(&harness, &id)
        .debug_tags()
        .iter()
        .flat_map(|tag| tag.tag.wires.clone())
        .collect();
    let at_rest: BTreeSet<String> = routes(&harness, &id).into_keys().collect();
    assert!(!chipped.is_empty(), "shop-30 draws chips");

    switch_wires(&mut harness, ALL_WIRES);
    assert_eq!(folder(&harness).preferences.wires, WireMode::All);
    assert_eq!(board(&harness, &id).wire_mode(), WireMode::All);
    assert!(board(&harness, &id).debug_tags().is_empty(), "no chip");
    assert!(board(&harness, &id).debug_counts().is_empty(), "no count");
    let every: BTreeSet<String> = routes(&harness, &id).into_keys().collect();
    let expected: BTreeSet<String> = at_rest.union(&chipped).cloned().collect();
    assert_eq!(every, expected, "All wires draws every wire");
    assert_eq!(positions(&harness, &id), cards, "no card moves");
    assert_eq!(
        board(&harness, &id).debug_camera(),
        camera,
        "the camera stays"
    );
    choose(&mut harness, TIDY);
    harness.step();
    let tidied_all = positions(&harness, &id);

    switch_wires(&mut harness, CHIPS);
    assert_eq!(folder(&harness).preferences.wires, WireMode::Chips);
    assert_eq!(positions(&harness, &id), tidied_all, "no card moves");
    let camera = board(&harness, &id).debug_camera();
    choose(&mut harness, TIDY);
    harness.step();
    assert_eq!(
        positions(&harness, &id),
        tidied_all,
        "Tidy up gives the same positions"
    );
    let _ = camera;
}

/// Criterion 9: editing follows the rule: a binding from a fanned-out port
/// to a far input connects and shows a chip; removing it removes the chip;
/// while a card is dragged no wire turns into a chip or back, and the rule
/// is applied when it is dropped.
#[test]
fn lay23_criterion_9_editing_follows_the_rule() {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    let item_id = id_of(&harness, &id, "itemId");
    let lookup = id_of(&harness, &id, "lookup");
    let reads = |harness: &FolderHarness| {
        chips(harness, &id).contains(&("lookup".to_owned(), "itemId".to_owned()))
    };
    assert!(!reads(&harness));
    app(&mut harness).active_board_command(Command::Bind {
        node: lookup.clone(),
        input: "query".into(),
        path: vec!["email".into()],
        binding: Binding::Reference {
            node_id: item_id,
            port: "value".into(),
            path: String::new(),
        },
    });
    settle(&mut harness);
    harness.step();
    assert!(
        readers_in_document(&graph(&harness, &id), "itemId").contains("lookup"),
        "the binding connects"
    );
    assert!(reads(&harness), "lookup shows a chip of itemId");
    app(&mut harness).active_board_command(Command::Bind {
        node: lookup,
        input: "query".into(),
        path: vec!["email".into()],
        binding: Binding::parameter("email"),
    });
    settle(&mut harness);
    harness.step();
    assert!(!reads(&harness), "removing the binding removes the chip");

    // `quantity` keeps its wires beside cartId; dragged far away it keeps
    // them until it is dropped.
    show(&mut harness, &id, "quantity", 0.6);
    let near = ("cartId".to_owned(), "quantity".to_owned());
    assert!(wires(&harness, &id).contains(&near));
    let card = board(&harness, &id)
        .debug_card_rect(&id_of(&harness, &id, "quantity"))
        .unwrap();
    let from = card.center_top() + egui::vec2(0.0, 12.0);
    let to = from + egui::vec2(0.0, 420.0);
    super::lay16::press(&mut harness, from);
    super::lay16::move_to(&mut harness, from, to);
    assert!(
        wires(&harness, &id).contains(&near),
        "while it is dragged the wire stays a wire"
    );
    assert!(
        !chips(&harness, &id).contains(&("quantity".to_owned(), "cartId".to_owned())),
        "and no chip appears"
    );
    super::lay16::release(&mut harness, to);
    settle(&mut harness);
    harness.step();
    assert!(
        !wires(&harness, &id).contains(&near),
        "dropped away from cartId, the wire is a chip"
    );
    assert!(chips(&harness, &id).contains(&("quantity".to_owned(), "cartId".to_owned())));
}

/// Criterion 10: the review views, each asserting a card in view.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay23() {
    // (scenario, size, capture, wires, zoomed to 60 % on a reader, that
    // reader selected)
    for (name, size, label, wires, zoomed, selected) in [
        (
            "fan-out",
            WIDE,
            "lay23-fan-out-chips-fit",
            CHIPS,
            false,
            false,
        ),
        (
            "fan-out",
            WIDE,
            "lay23-fan-out-chips-60",
            CHIPS,
            true,
            false,
        ),
        (
            "fan-out",
            WIDE,
            "lay23-fan-out-reader-selected-60",
            CHIPS,
            true,
            true,
        ),
        (
            "fan-out",
            WIDE,
            "lay23-fan-out-all-wires-fit",
            ALL_WIRES,
            false,
            false,
        ),
        (
            "shop-30",
            WIDE,
            "lay23-shop-30-chips-fit",
            CHIPS,
            false,
            false,
        ),
        (
            "shop-30",
            WIDE,
            "lay23-shop-30-chips-60",
            CHIPS,
            true,
            false,
        ),
        (
            "shop-30",
            WIDE,
            "lay23-shop-30-reader-selected-60",
            CHIPS,
            true,
            true,
        ),
        (
            "shop-30",
            WIDE,
            "lay23-shop-30-all-wires-fit",
            ALL_WIRES,
            false,
            false,
        ),
        (
            "fan-out",
            crate::testing::MINIMUM,
            "lay23-fan-out-chips-960",
            CHIPS,
            false,
            false,
        ),
        (
            "shop-30",
            crate::testing::MINIMUM,
            "lay23-shop-30-chips-960",
            CHIPS,
            false,
            false,
        ),
    ] {
        let (_project, mut harness, id) = tidied(name, size, Checks::Collapsed);
        let reader = reader_with_chips(&harness, &id);
        if wires == ALL_WIRES {
            switch_wires(&mut harness, ALL_WIRES);
        }
        choose(&mut harness, FIT);
        harness.step();
        if zoomed {
            show(&mut harness, &id, &reader, 0.6);
        }
        if selected {
            select(&mut harness, &id, &[reader.as_str()]);
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
        crate::testing::capture(&mut harness, label, None);
    }
}
