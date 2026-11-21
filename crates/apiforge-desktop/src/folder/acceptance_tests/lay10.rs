//! LAY-10 (#294): focus on selection. Selecting a step rings its direct
//! producers, readers and checks, and the steps its ordering wires reach,
//! with a halo in a hue that is not the selection's, their ports ringed;
//! its checks ring in the check colour. Every other card dims to 60 % and
//! every other wire to 25 %; a failed or running card never dims. Esc or a
//! click on the empty board restores the board (Q9 = A). On `shop-30` after
//! a failed simulation, checks drawn as cards (expanded, LAY-30's default)
//! and, where named, collapsed.

use super::lay07::{self, Gate};
use super::lay16::{canvas, document, graph, id_of, select, show};
use super::*;
use crate::theme;
use apiforge_core::{block::Block, format::Graph, graph::relations};
use egui::{Color32, Pos2, Rect};
use std::collections::BTreeSet;
use std::sync::Arc;

/// The check colour: the colour of a check block.
fn check_colour() -> Color32 {
    theme::block_color(Block::Comparison)
}

/// `shop-30` whose checkout answers with a total the check does not expect:
/// the request succeeds, `checkout_total` fails.
fn failing() -> String {
    let source = lay07::fixture("shop-30");
    let before = r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#;
    assert!(source.contains(before), "the fixture's checkout sample");
    source.replace(
        before,
        r#"sample 201 { orderId: "o-1", status: "created", total: 180.0 }"#,
    )
}

fn fit(harness: &mut FolderHarness) {
    choose(harness, "Fit the view");
    harness.step();
}

/// `shop-30` after a failed simulation, the whole board in view, its checks
/// drawn as cards unless `collapsed`.
fn after_failure_in(
    size: egui::Vec2,
    collapsed: bool,
) -> (tempfile::TempDir, FolderHarness, Arc<Gate>) {
    let (project, mut harness, gate) = lay07::open(&failing(), size, !collapsed);
    lay07::simulate(&mut harness, &gate);
    let failed = id_of(
        &harness,
        if collapsed {
            "checkout"
        } else {
            "checkout_total"
        },
    );
    fit(&mut harness);
    assert_eq!(
        canvas(&harness).debug_signal(&failed),
        Some(theme::DANGER),
        "the simulation failed at checkout_total"
    );
    (project, harness, gate)
}

fn after_failure(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, Arc<Gate>) {
    after_failure_in(size, false)
}

fn handle_of(graph: &Graph, id: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| id.to_owned(), |node| node.handle.clone())
}

/// What `handle` is related to: the steps LAY-22's `relations` names, and
/// the steps its ordering wires reach.
fn related(harness: &FolderHarness, handle: &str) -> BTreeSet<String> {
    let graph = graph(harness);
    let node = graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no step {handle}"));
    let found = relations(&graph, node);
    let mut ids: BTreeSet<String> = BTreeSet::new();
    ids.extend(found.producers.iter().map(|feed| feed.step.to_owned()));
    ids.extend(found.readers.iter().map(|feed| feed.step.to_owned()));
    ids.extend(found.checks.iter().map(|check| check.check.to_owned()));
    ids.extend(found.gates.iter().map(|gate| (*gate).to_owned()));
    ids.extend(found.subjects.iter().map(|subject| subject.step.to_owned()));
    ids.extend(found.values.iter().map(|value| (*value).to_owned()));
    for edge in &graph.edges {
        if edge.source == node.id {
            ids.insert(edge.target.clone());
        }
        if edge.target == node.id {
            ids.insert(edge.source.clone());
        }
    }
    ids.remove(&node.id);
    ids
}

/// The cards the last frame drew, by id.
fn drawn_cards(harness: &FolderHarness) -> Vec<String> {
    let canvas = canvas(harness);
    graph(harness)
        .nodes
        .iter()
        .filter(|node| canvas.debug_is_drawn(&node.id))
        .map(|node| node.id.clone())
        .collect()
}

fn ringed(harness: &FolderHarness) -> BTreeSet<String> {
    let canvas = canvas(harness);
    drawn_cards(harness)
        .into_iter()
        .filter(|id| canvas.debug_ring(id).is_some())
        .collect()
}

fn dimmed(harness: &FolderHarness, handle: &str) -> bool {
    canvas(harness).debug_dimmed(&id_of(harness, handle))
}

/// The card is failed or running: it keeps its strength.
fn signalled(harness: &FolderHarness, id: &str) -> bool {
    canvas(harness).debug_signal(id) == Some(theme::DANGER)
}

/// The ports ringed on the card `id`, on screen.
fn ports_of(harness: &FolderHarness, id: &str) -> Vec<Pos2> {
    let canvas = canvas(harness);
    let rect = canvas
        .debug_card_rect(id)
        .unwrap_or_else(|| panic!("{id} is drawn"))
        .expand(12.0);
    canvas
        .debug_ringed_ports()
        .into_iter()
        .filter(|(card, at)| card == id && rect.contains(*at))
        .map(|(_, at)| at)
        .collect()
}

/// The focus of `handle` on the drawn board: its relations ring, every
/// other card dims unless it failed, its wires are lit and every other wire
/// is dimmed.
fn assert_focus(harness: &FolderHarness, handle: &str, context: &str) {
    let graph = graph(harness);
    let selected = id_of(harness, handle);
    let related = related(harness, handle);
    let canvas = canvas(harness);
    assert!(
        canvas.debug_selected(&selected),
        "{context}: {handle} has the selection's frame"
    );
    assert!(
        canvas.debug_handles(&selected),
        "{context}: {handle} keeps its handles"
    );
    assert!(!canvas.debug_dimmed(&selected), "{context}: {handle}");
    assert!(
        canvas.debug_ring(&selected).is_none(),
        "{context}: the selection is framed, not haloed"
    );
    let drawn = drawn_cards(harness);
    assert!(drawn.len() > 3, "{context}: cards drawn: {drawn:?}");
    let expected: BTreeSet<String> = drawn
        .iter()
        .filter(|id| related.contains(*id))
        .cloned()
        .collect();
    assert_eq!(
        ringed(harness)
            .iter()
            .map(|id| handle_of(&graph, id))
            .collect::<BTreeSet<_>>(),
        expected
            .iter()
            .map(|id| handle_of(&graph, id))
            .collect::<BTreeSet<_>>(),
        "{context}: the drawn cards ringed are the relations of {handle}"
    );
    for id in &drawn {
        let name = handle_of(&graph, id);
        if *id == selected || related.contains(id) {
            assert!(!canvas.debug_dimmed(id), "{context}: {name} is related");
            if let Some(ring) = canvas.debug_ring(id) {
                assert_ne!(ring, theme::ACCENT, "{context}: {name}'s halo");
            }
        } else if signalled(harness, id) {
            assert!(!canvas.debug_dimmed(id), "{context}: {name} failed");
        } else {
            assert!(canvas.debug_dimmed(id), "{context}: {name} is dimmed");
        }
    }
    let wires = canvas.debug_wires();
    assert!(!wires.is_empty(), "{context}: wires drawn");
    for wire in wires.iter().filter(|wire| !wire.points.is_empty()) {
        let source = wire.source.clone().unwrap_or_default();
        let mine = wire.target == selected || source == selected;
        if mine {
            assert!(
                wire.lit && !wire.dimmed,
                "{context}: {} → {} is the selection's",
                handle_of(&graph, &source),
                handle_of(&graph, &wire.target)
            );
        } else {
            assert!(
                wire.dimmed && !wire.lit,
                "{context}: {} → {} is dimmed",
                handle_of(&graph, &source),
                handle_of(&graph, &wire.target)
            );
        }
    }
}

/// Criterion 1: selecting `cart` rings `login`, `first`, `second`,
/// `cart_total` and `order` with the halo and their ports, `cart_total` in
/// the check colour; every card not related is dimmed; the selection keeps
/// its handles. The hub `login` the same; selecting `begin` rings `login`
/// and lights the dashed wire between them.
#[test]
fn lay10_criterion_1_selecting_cart_rings_its_relations() {
    let (_project, mut harness, _gate) = after_failure(WIDE);
    select(&mut harness, &["cart"]);
    fit(&mut harness);
    assert_focus(&harness, "cart", "cart");
    for handle in ["login", "first", "second", "cart_total", "order"] {
        let id = id_of(&harness, handle);
        let ring = canvas(&harness)
            .debug_ring(&id)
            .unwrap_or_else(|| panic!("{handle} is ringed"));
        if handle == "cart_total" {
            assert_eq!(ring, check_colour(), "a check rings in the check colour");
        } else {
            assert_ne!(ring, check_colour(), "{handle} is not a check");
        }
        assert!(
            !ports_of(&harness, &id).is_empty(),
            "{handle}'s port to cart is ringed: {:?}",
            canvas(&harness).debug_ringed_ports()
        );
    }
    for handle in ["catalogue", "refund", "logout"] {
        assert!(dimmed(&harness, handle), "{handle} is dimmed");
    }

    // The hub: almost every step reads `login`.
    select(&mut harness, &["login"]);
    fit(&mut harness);
    assert_focus(&harness, "login", "login");
    assert!(!dimmed(&harness, "cart"), "cart reads login");

    // Start: its ordering wire into `login`.
    select(&mut harness, &["begin"]);
    fit(&mut harness);
    assert_focus(&harness, "begin", "begin");
    let begin = id_of(&harness, "begin");
    let login = id_of(&harness, "login");
    assert!(
        canvas(&harness).debug_ring(&login).is_some(),
        "begin rings login"
    );
    assert!(
        canvas(&harness).debug_wires().iter().any(|wire| {
            wire.dashed
                && wire.source.as_deref() == Some(begin.as_str())
                && wire.target == login
                && wire.lit
                && !wire.points.is_empty()
        }),
        "the dashed wire from begin to login is lit: {:#?}",
        canvas(&harness).debug_wires()
    );

    // Checks collapsed: a folded check is no card to ring.
    let (_project, mut harness, _gate) = after_failure_in(WIDE, true);
    for handle in ["cart", "login"] {
        select(&mut harness, &[handle]);
        fit(&mut harness);
        assert_focus(&harness, handle, &format!("{handle}, checks collapsed"));
    }
}

/// Criterion 2: a failed card and the running card are never dimmed,
/// whatever is selected.
#[test]
fn lay10_criterion_2_failed_and_running_are_never_dimmed() {
    let (_project, mut harness, gate) = after_failure(WIDE);
    let checkout = id_of(&harness, "checkout_total");
    for handle in ["refund", "logout", "catalogue", "cart"] {
        select(&mut harness, &[handle]);
        fit(&mut harness);
        assert!(
            !related(&harness, handle).contains(&checkout),
            "{handle} is not related to checkout"
        );
        assert_eq!(
            canvas(&harness).debug_signal(&checkout),
            Some(theme::DANGER)
        );
        assert!(
            !dimmed(&harness, "checkout_total"),
            "{handle} selected: the failed check is not dimmed"
        );
        assert!(
            drawn_cards(&harness)
                .iter()
                .any(|id| canvas(&harness).debug_dimmed(id)),
            "{handle} selected: other cards are dimmed"
        );
    }

    // A run held at `catalogue`, with `refund` selected.
    select(&mut harness, &["refund"]);
    fit(&mut harness);
    lay07::run_until_started(&mut harness, &gate, "catalogue");
    assert!(
        !related(&harness, "refund").contains(&id_of(&harness, "catalogue")),
        "catalogue is not related to refund"
    );
    assert!(!dimmed(&harness, "catalogue"), "the running step");
    assert!(
        dimmed(&harness, "product"),
        "a step not related, not running"
    );
    lay07::finish(&mut harness, &gate);
    assert!(!dimmed(&harness, "checkout_total"), "failed again");
}

/// The WCAG contrast ratio of two opaque colours.
fn contrast(a: Color32, b: Color32) -> f32 {
    fn luminance(color: Color32) -> f32 {
        let channel = |value: u8| {
            let value = f32::from(value) / 255.0;
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `under` with the premultiplied `veil` painted over it.
fn under_veil(under: Color32, veil: Color32) -> Color32 {
    let keep = 1.0 - f32::from(veil.a()) / 255.0;
    let mix = |over: u8, under: u8| {
        (f32::from(over) + f32::from(under) * keep)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(
        mix(veil.r(), under.r()),
        mix(veil.g(), under.g()),
        mix(veil.b(), under.b()),
    )
}

/// Criterion 3: on a dimmed card the name's contrast against the card
/// stays at least 3:1: the dimming is an alpha over the paint.
#[test]
fn lay10_criterion_3_names_stay_legible_when_dimmed() {
    let (_project, mut harness, _gate) = after_failure(WIDE);
    select(&mut harness, &["cart"]);
    for (handle, zoom) in [("refund", 0.6), ("catalogue", 1.0), ("logout", 0.35)] {
        show(&mut harness, handle, zoom);
        let id = id_of(&harness, handle);
        assert!(dimmed(&harness, handle), "{handle} is dimmed");
        let veil = canvas(&harness)
            .debug_veil(&id)
            .unwrap_or_else(|| panic!("{handle} has a veil"));
        // About 60 % of the card is kept.
        let kept = 1.0 - f32::from(veil.a()) / 255.0;
        assert!(
            (0.5..=0.7).contains(&kept),
            "{handle}: the card keeps {kept}"
        );
        let name = canvas(&harness)
            .debug_texts(&id)
            .into_iter()
            .find(|text| text.kind == crate::canvas::TextKind::Name)
            .unwrap_or_else(|| panic!("{handle} wrote its name"));
        let ratio = contrast(under_veil(name.color, veil), under_veil(theme::WHITE, veil));
        assert!(
            ratio >= 3.0,
            "{handle} at {zoom}: the dimmed name reads at {ratio}:1"
        );
    }
}

/// No card dimmed or ringed, no wire dimmed or lit, nothing selected.
fn assert_restored(harness: &FolderHarness, context: &str) {
    let document = document(harness);
    assert!(
        folder(harness)
            .selections
            .get(&document)
            .is_none_or(Vec::is_empty),
        "{context}: the selection is cleared"
    );
    let canvas = canvas(harness);
    let drawn = drawn_cards(harness);
    assert!(drawn.len() > 3, "{context}: cards drawn");
    for id in &drawn {
        assert!(!canvas.debug_dimmed(id), "{context}: {id} at full strength");
        assert!(
            canvas.debug_ring(id).is_none(),
            "{context}: {id} not ringed"
        );
        assert!(canvas.debug_veil(id).is_none(), "{context}: {id}");
    }
    for wire in canvas.debug_wires() {
        assert!(!wire.dimmed && !wire.lit, "{context}: {}", wire.id);
    }
    assert!(canvas.debug_ringed_ports().is_empty(), "{context}: ports");
}

/// A point of the board in view with no card on it.
fn empty_point(harness: &FolderHarness) -> Pos2 {
    let canvas = canvas(harness);
    let focus = canvas.debug_focus();
    let cards: Vec<Rect> = canvas
        .debug_cards()
        .iter()
        .map(|(_, rect)| rect.expand(24.0))
        .collect();
    let tags: Vec<Rect> = canvas
        .debug_tags()
        .iter()
        .map(|tag| canvas.debug_to_screen(tag.world).expand(12.0))
        .collect();
    let step = 16.0;
    let mut y = focus.top() + step;
    while y < focus.bottom() - step {
        let mut x = focus.left() + step;
        while x < focus.right() - step {
            let at = Pos2::new(x, y);
            if !cards.iter().chain(&tags).any(|rect| rect.contains(at)) {
                return at;
            }
            x += step;
        }
        y += step;
    }
    panic!("no empty point on the board");
}

/// Criterion 4: Esc restores every card and wire to full strength and
/// clears the selection; a click on the empty board does the same.
#[test]
fn lay10_criterion_4_esc_restores() {
    let (_project, mut harness, _gate) = after_failure(WIDE);
    select(&mut harness, &["cart"]);
    fit(&mut harness);
    assert!(dimmed(&harness, "refund"), "focus is on");
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    harness.step();
    assert_restored(&harness, "Esc");

    select(&mut harness, &["cart"]);
    fit(&mut harness);
    assert!(dimmed(&harness, "refund"), "focus is on again");
    let at = empty_point(&harness);
    super::lay16::press(&mut harness, at);
    super::lay16::release(&mut harness, at);
    settle(&mut harness);
    harness.step();
    assert_restored(&harness, "a click on the empty board");
}

/// Criterion 6: selecting a check rings its subjects and value sources in
/// the check colour.
#[test]
fn lay10_criterion_6_a_check_rings_its_subjects() {
    let (_project, mut harness, _gate) = after_failure(WIDE);
    select(&mut harness, &["cart_total"]);
    fit(&mut harness);
    assert_focus(&harness, "cart_total", "cart_total");
    for handle in ["cart", "expected_total"] {
        assert_eq!(
            canvas(&harness).debug_ring(&id_of(&harness, handle)),
            Some(check_colour()),
            "{handle} rings in the check colour"
        );
    }
    // `address` is no subject. Since LANG-13 it follows the step before it,
    // not the check, so no ordering relation rings it either.
    let address = canvas(&harness).debug_ring(&id_of(&harness, "address"));
    assert_ne!(address, Some(check_colour()), "address is not a subject");
    assert_ne!(address, Some(theme::ACCENT), "nor the selection");
}

/// The views of the owner's acceptance: `shop-30` tidied with its checks
/// drawn, after a failed simulation, the Run results folded; `cart`
/// selected at 60 % at 1600 × 1000 and 960 × 540, and `begin` selected
/// with its ordering wire into `login`.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay10() {
    let assert_a_card_in_view = |harness: &FolderHarness, context: &str| {
        let board = canvas(harness);
        let view = board.debug_focus().expand(0.5);
        assert!(
            board
                .debug_cards()
                .iter()
                .any(|(_, card)| view.contains_rect(*card)),
            "{context}: a whole card is in the view"
        );
    };
    for (size, selected, name) in [
        (WIDE, "cart", "lay10-cart-60-1600"),
        (crate::testing::MINIMUM, "cart", "lay10-cart-60-960"),
        (WIDE, "begin", "lay10-begin-60-1600"),
    ] {
        let project = crate::testing::shop_project();
        fs::write(
            project
                .path()
                .join("scenarios/promotion-and-cart-price.afs"),
            failing(),
        )
        .unwrap();
        fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
        let (_project, mut harness, gate) = lay07::opened(project, size, true);
        let document = document(&harness);
        app(&mut harness).selections.insert(document, Vec::new());
        settle(&mut harness);
        choose(&mut harness, "Tidy up");
        harness.step();
        lay07::simulate(&mut harness, &gate);
        // The board as tall as the window lets it be.
        harness.get_by_label("Run results").click();
        settle(&mut harness);
        select(&mut harness, &[selected]);
        show(&mut harness, selected, 0.6);
        assert_a_card_in_view(&harness, name);
        assert!(
            canvas(&harness).debug_handles(&id_of(&harness, selected)),
            "{name}: the selection is drawn"
        );
        crate::testing::capture(&mut harness, name, None);
    }
}

/// 0.2.1 mock-up: the selected card in the look v0.2.0 ships and in three
/// others, on a failed step at 100 % and on `cart` with its relations at
/// 60 %.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rel021_selection() {
    for (variant, look) in [(0, "today"), (1, "a-ink"), (2, "b-blue"), (3, "c-accent")] {
        crate::mock::set(&crate::mock::SELECTION, variant);
        for (selected, zoom, collapsed, view) in [
            ("checkout", 1.0, true, "failed-100"),
            ("cart", 1.0, true, "passed-100"),
            ("cart", 0.6, false, "focus-60"),
        ] {
            let project = crate::testing::shop_project();
            fs::write(
                project
                    .path()
                    .join("scenarios/promotion-and-cart-price.afs"),
                failing(),
            )
            .unwrap();
            fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
            let (_project, mut harness, gate) = lay07::opened(project, WIDE, !collapsed);
            let document = document(&harness);
            app(&mut harness).selections.insert(document, Vec::new());
            settle(&mut harness);
            choose(&mut harness, "Tidy up");
            harness.step();
            lay07::simulate(&mut harness, &gate);
            harness.get_by_label("Run results").click();
            settle(&mut harness);
            select(&mut harness, &[selected]);
            show(&mut harness, selected, zoom);
            let id = id_of(&harness, selected);
            let card = canvas(&harness)
                .debug_cards()
                .iter()
                .find(|(card, _)| *card == id)
                .map(|(_, rect)| *rect)
                .unwrap();
            let region = if zoom < 1.0 {
                card.expand2(egui::vec2(430.0, 250.0))
            } else {
                card.expand2(egui::vec2(170.0, 70.0))
            };
            crate::testing::capture(
                &mut harness,
                &format!("rel021-selection-{look}-{view}"),
                Some(region),
            );
        }
    }
    crate::mock::set(&crate::mock::SELECTION, 0);
}

/// 0.2.1 mock-up: a check of a field's type from the Inspector's Output
/// data, as v0.2.0 offers it and in two other forms.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rel021_type_check() {
    for (variant, look) in [(0, "today"), (1, "a-menu"), (2, "b-chip")] {
        crate::mock::set(&crate::mock::TYPE_CHECK, variant);
        let (_project, mut harness, gate) = lay07::open(&lay07::fixture("shop-30"), WIDE, false);
        lay07::simulate(&mut harness, &gate);
        harness.get_by_label("Run results").click();
        settle(&mut harness);
        select(&mut harness, &["cart"]);
        show(&mut harness, "cart", 1.0);
        harness.get_by_label("Output data").click();
        settle(&mut harness);
        if variant == 1 {
            let button = harness
                .query_all_by_label("Add comparison")
                .nth(2)
                .expect("a field row");
            button.click();
            settle(&mut harness);
        }
        let region = egui::Rect::from_min_max(egui::pos2(1232.0, 98.0), egui::pos2(1592.0, 780.0));
        crate::testing::capture(
            &mut harness,
            &format!("rel021-type-check-{look}"),
            Some(region),
        );
    }
    crate::mock::set(&crate::mock::TYPE_CHECK, 0);
}

/// Writes the demo project a first start prepares, for reading.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rel021_demo_dump() {
    let project = crate::testing::shop_project();
    let out = std::path::PathBuf::from(std::env::var("REL021_DEMO").unwrap());
    for file in [
        "scenarios/promotion-and-cart-price.afs",
        "components/product.afs",
        "components/active-promotion.afs",
        "components/add-to-cart.afs",
    ] {
        let target = out.join(file);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(project.path().join(file), target).unwrap();
    }
    std::process::Command::new("cp")
        .arg("-r")
        .arg(project.path())
        .arg(out.join("project"))
        .status()
        .unwrap();
    let (_project, mut harness, _gate) = lay07::opened(project, WIDE, false);
    settle(&mut harness);
    crate::testing::capture(&mut harness, "rel021-demo-today-1600", None);
}

/// 0.2.1 mock-up: the proposed first-start scenario on the board, tidied,
/// before and after a simulation.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rel021_demo_proposed() {
    let source = fs::read_to_string(std::env::var("REL021_PROPOSED").unwrap()).unwrap();
    for size in [WIDE, crate::testing::MINIMUM] {
        let project = crate::testing::shop_project();
        fs::write(
            project
                .path()
                .join("scenarios/promotion-and-cart-price.afs"),
            &source,
        )
        .unwrap();
        fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
        let (_project, mut harness, gate) = lay07::opened(project, size, false);
        let document = document(&harness);
        app(&mut harness).selections.insert(document, Vec::new());
        settle(&mut harness);
        choose(&mut harness, "Tidy up");
        harness.step();
        fit(&mut harness);
        settle(&mut harness);
        let width = size.x as u32;
        crate::testing::capture(&mut harness, &format!("rel021-demo-proposed-{width}"), None);
        if size == WIDE {
            lay07::simulate(&mut harness, &gate);
            harness.get_by_label("Run results").click();
            settle(&mut harness);
            fit(&mut harness);
            settle(&mut harness);
            crate::testing::capture(&mut harness, "rel021-demo-proposed-run-1600", None);
            harness.get_by_label("Code").click();
            settle(&mut harness);
            crate::testing::capture(&mut harness, "rel021-demo-proposed-code-1600", None);
        }
    }
}
