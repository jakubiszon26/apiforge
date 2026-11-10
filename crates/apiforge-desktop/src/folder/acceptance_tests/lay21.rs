//! LAY-21: every other card is compact too. Value, check, *Data condition*,
//! `if` and component call follow the compact geometry LAY-16 gave requests
//! (Q13 = A, Q14 = A, Q16 = A, the tray and the hidden footer).

use super::lay16::{
    assert_disjoint_cards, binding_of, canvas, centre, click_at, graph, has, id_of, lines, move_to,
    open, press, release, screen_of, select, shop_30, show, switch_density, world, world_of,
};
use super::*;
use crate::canvas::{Card, CardDensity, Command};
use apiforge_core::{
    block::Block,
    format::{Binding, Node, Position},
    model::NodeKind,
};
use egui::accesskit::Role;
use egui::{Rect, pos2};
use egui_kittest::kittest::NodeT;

/// A value card's box at the compact density: its header and one line.
const VALUE: f32 = 70.0;
/// A check's box: its header, the predicate in words and the result.
const CHECK: f32 = 96.0;

/// A request whose response a check reads, and a value nothing reads.
const FRESH: &str = r#"scenario "Fresh" {
  cart = POST "/cart" {
    body { items: 1 }
    sample 201 { id: "c-1" }
  }
  cart_ok = assert cart.body.id == "c-1"
  cart_id = cart.body.id
}
"#;

/// A component call and an `if` side by side, for the review: the two kinds
/// no mock-up drew.
const CALLS: &str = r#"scenario "Calls" {
  catalogue = GET "/products" {
    sample 200 { items: [{ id: 123, price: 100 }] }
  }
  var first = catalogue.body.items[0]
  product = product@1(id: first.id)
  promoted = active_promotion@1(product: product)
  on_sale = if promoted.promotion.price < first.price {
    sale = assert promoted.promotion.price > 0
  } else {
    full_price = assert first.price == 100
  }
}
"#;

/// Asserts that the last frame drew at least one card inside the viewport.
fn assert_cards_in_view(harness: &FolderHarness, context: &str) {
    let board = super::lay16::canvas(harness);
    let viewport = board.debug_viewport();
    assert!(
        board
            .debug_cards()
            .iter()
            .any(|(_, card)| viewport.contains_rect(*card)),
        "{context}: a whole card is in the viewport"
    );
}

/// `shop-30` tidied at the compact density, centred on `cart` at 60 % and
/// 100 %, at both sizes; then a component call and an `if`.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay21_board() {
    for (size, suffix) in [(WIDE, "1600"), (crate::testing::MINIMUM, "960")] {
        let (_project, mut harness) = open(&shop_30(), size);
        harness.get_by_label("Tidy up").click();
        settle(&mut harness);
        for (zoom, percent) in [(0.6, "60"), (1.0, "100")] {
            show(&mut harness, "cart", zoom);
            assert_cards_in_view(&harness, &format!("cart at {percent} % at {suffix}"));
            crate::testing::capture(
                &mut harness,
                &format!("lay21-cart-{percent}-{suffix}"),
                None,
            );
        }
        if suffix == "1600" {
            // A selected check and value: their hidden ports in the trays.
            select(&mut harness, &["cart_created", "first"]);
            show(&mut harness, "cart", 1.0);
            crate::testing::capture(&mut harness, "lay21-trays-100-1600", None);
            select(&mut harness, &[]);
        }
    }
    let (_project, mut harness) = open(CALLS, WIDE);
    harness.get_by_label("Tidy up").click();
    settle(&mut harness);
    for (zoom, percent) in [(0.6, "60"), (1.0, "100")] {
        show(&mut harness, "promoted", zoom);
        assert_cards_in_view(&harness, &format!("the calls at {percent} %"));
        let region = ["product", "promoted", "on_sale"]
            .into_iter()
            .filter_map(|handle| {
                super::lay16::canvas(&harness)
                    .debug_card_rect(&super::lay16::id_of(&harness, handle))
            })
            .reduce(Rect::union)
            .map(|rect| rect.expand(120.0));
        crate::testing::capture(
            &mut harness,
            &format!("lay21-call-and-if-{percent}-1600"),
            region,
        );
    }
}

/// Whether the kind is a satellite: a value, a check, a *Data condition* or
/// an `if`.
fn satellite(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Value | NodeKind::Assert | NodeKind::Condition | NodeKind::If
    )
}

/// The step *Add step* puts on the board right of every card: its node.
fn add(harness: &mut FolderHarness, block: Block, component: Option<(&str, u32)>) -> Node {
    let before: Vec<String> = graph(harness).nodes.into_iter().map(|n| n.id).collect();
    let right = world(harness)
        .iter()
        .map(|(_, rect)| rect.right())
        .fold(0.0_f32, f32::max);
    let top = world_of(harness, "cart").top();
    app(harness).active_board_command(Command::AddNode {
        block,
        component: component.map(|(id, revision)| (id.to_owned(), revision)),
        position: Position {
            x: f64::from(right + 200.0),
            y: f64::from(top),
        },
        source: None,
    });
    settle(harness);
    harness.step();
    graph(harness)
        .nodes
        .into_iter()
        .find(|node| !before.contains(&node.id))
        .expect("Add step put a step on the board")
}

/// Criterion 1: a value is its header and one line, a check its header and
/// two lines, and neither is taller than `cart`.
#[test]
fn lay21_criterion_1_satellites_are_shorter_than_requests() {
    let (_project, harness) = open(&shop_30(), WIDE);
    let cart = world_of(&harness, "cart").height();
    errln!(
        "LAY-21 heights on shop-30: value first {}, check cart_created {}, Data condition \
         valid_cart {}, if paid {}, component call product {}, request cart {cart}",
        world_of(&harness, "first").height(),
        world_of(&harness, "cart_created").height(),
        world_of(&harness, "valid_cart").height(),
        world_of(&harness, "paid").height(),
        world_of(&harness, "product").height(),
    );
    let mut measured = 0;
    for node in graph(&harness).nodes {
        let height = world_of(&harness, &node.handle).height();
        match node.kind {
            NodeKind::Value => assert_eq!(height, VALUE, "{} is a header and a line", node.handle),
            NodeKind::Assert => {
                assert_eq!(height, CHECK, "{} is a header and two lines", node.handle)
            }
            _ => {}
        }
        if satellite(node.kind) {
            measured += 1;
            assert!(
                height < cart,
                "{} ({height} px) is not taller than cart ({cart} px)",
                node.handle
            );
        }
    }
    assert!(measured >= 15, "the board's satellites are measured");
}

/// Criterion 2: the lines name their source; none is anonymous.
#[test]
fn lay21_criterion_2_lines_name_their_source() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    show(&mut harness, "cart_created", 0.6);
    assert!(
        lines(&harness, "cart_created").contains(&"cart.status is equal to 201".to_owned()),
        "{:?}",
        lines(&harness, "cart_created")
    );
    show(&mut harness, "first", 0.6);
    assert!(
        lines(&harness, "first").contains(&"catalogue.body.items[0]".to_owned()),
        "{:?}",
        lines(&harness, "first")
    );
    for node in graph(&harness).nodes {
        if node.kind == NodeKind::Start {
            continue;
        }
        show(&mut harness, &node.handle, 0.6);
        let written = lines(&harness, &node.handle);
        assert!(!written.is_empty(), "{} is a compact card", node.handle);
        for line in &written {
            let anonymous = (line.starts_with("{ ") && line.ends_with(" fields }"))
                || (line.starts_with("[ ") && line.ends_with(" items ]"));
            assert!(
                !anonymous,
                "{}'s line {line:?} names no source",
                node.handle
            );
        }
    }
}

/// Criterion 3: zooming from the board's minimum to 1.0 moves no card of
/// any kind.
#[test]
fn lay21_criterion_3_zooming_moves_no_card() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    let before = world(&harness);
    assert!(before.len() >= 30, "the whole board is measured");
    for zoom in [0.25, 0.3, 0.44, 0.46, 0.6, 0.74, 0.76, 0.9, 1.0] {
        show(&mut harness, "paid", zoom);
        assert_eq!(world(&harness), before, "at zoom {zoom} no card moved");
        assert_eq!(world_of(&harness, "first").height(), VALUE, "at {zoom}");
        assert_eq!(
            world_of(&harness, "cart_created").height(),
            CHECK,
            "at {zoom}"
        );
    }
}

/// Criterion 4: a wire carried near a new check lands on *Actual value*
/// through the tray; a selected value shows its hidden output in the tray.
#[test]
fn lay21_criterion_4_the_tray_reaches_hidden_ports() {
    let (_project, mut harness) = open(FRESH, WIDE);
    let check = add(&mut harness, Block::Comparison, None);
    let name = check.display_name().to_owned();
    // *Add step* selects the new step, which opens its tray.
    select(&mut harness, &[]);
    show(&mut harness, &check.handle, 0.6);
    let actual = format!("Input actual of {name}");
    assert!(!has(&harness, &actual), "at rest the tray is closed");

    let card = screen_of(&harness, &check.handle);
    let from = centre(&harness, "Output body of cart");
    let near = pos2(card.left() - 40.0, card.center().y);
    press(&mut harness, from);
    move_to(&mut harness, from, near);
    assert!(has(&harness, &actual), "a wire carried near opens the tray");
    let target = centre(&harness, &actual);
    assert!(target.x < card.left(), "Actual value is left of the card");
    move_to(&mut harness, near, target);
    release(&mut harness, target);
    match binding_of(&harness, &check.handle, "actual") {
        Some(Binding::Reference { node_id, port, .. }) => {
            assert_eq!(node_id, id_of(&harness, "cart"));
            assert_eq!(port, "body");
        }
        other => panic!("the check's actual value reads cart.body, not {other:?}"),
    }

    show(&mut harness, "cart_id", 0.6);
    assert!(
        !has(&harness, "Output value of cart_id"),
        "an output nothing reads is hidden at rest"
    );
    select(&mut harness, &["cart_id"]);
    let card = screen_of(&harness, "cart_id");
    let socket = harness.get_by_label("Output value of cart_id").rect();
    assert!(
        socket.center().x > card.right(),
        "the hidden output stands in the tray right of the card"
    );
}

/// Criterion 5: a double-click on a check's line opens the Inspector on
/// that input; no inline field, no footer; right-click opens *Add step*.
#[test]
fn lay21_criterion_5_editing_moves_to_the_inspector() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    show(&mut harness, "cart_created", 1.0);
    let inline = harness
        .query_all_by_role(Role::TextInput)
        .filter(|node| {
            node.accesskit_node()
                .label()
                .is_some_and(|label| label.ends_with(" of cart_created"))
        })
        .count();
    assert_eq!(inline, 0, "the card has no inline field");
    assert!(!has(&harness, "Add a step connected to cart_created"));
    assert!(!has(
        &harness,
        "Add the next step (next) after cart_created"
    ));

    let id = id_of(&harness, "cart_created");
    let row = canvas(&harness)
        .debug_input_row(&id, "actual")
        .expect("the check's line carries its actual value");
    click_at(&mut harness, row.center(), egui::PointerButton::Primary, 2);
    assert_eq!(
        folder(&harness)
            .selections
            .get(&super::lay16::document(&harness)),
        Some(&vec![id]),
        "the double-click selects the check"
    );
    assert_eq!(
        folder(&harness).debug_inspector_focus().as_deref(),
        Some("actual"),
        "the Inspector opens on the actual value"
    );

    show(&mut harness, "cart_created", 1.0);
    let card = screen_of(&harness, "cart_created");
    click_at(
        &mut harness,
        card.center(),
        egui::PointerButton::Secondary,
        1,
    );
    assert!(
        canvas(&harness).debug_menu_open(),
        "right-click opens Add step"
    );
}

/// Criterion 6: a component call has one line per configured, wired or
/// missing input and keeps *Open*; a missing input reads *Missing input*.
#[test]
fn lay21_criterion_6_a_component_call_is_compact() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    let cart = world_of(&harness, "cart").height();
    for node in graph(&harness)
        .nodes
        .into_iter()
        .filter(|node| node.kind == NodeKind::Component)
    {
        show(&mut harness, &node.handle, 0.6);
        assert!(
            has(
                &harness,
                &format!("Open the steps of {}", node.display_name())
            ),
            "{} keeps Open",
            node.handle
        );
        let written = lines(&harness, &node.handle);
        assert!(
            written.iter().any(|line| line.starts_with("Component · r")),
            "{}: {written:?}",
            node.handle
        );
        for input in node.inputs.keys() {
            assert!(
                canvas(&harness).debug_input_row(&node.id, input).is_some(),
                "{}'s configured {input} has its line: {written:?}",
                node.handle
            );
        }
        assert!(
            world_of(&harness, &node.handle).height() < cart,
            "{} is compact",
            node.handle
        );
    }

    // *Active promotion* requires its product and has no default for it.
    let call = add(
        &mut harness,
        Block::Component,
        Some(("active-promotion", 1)),
    );
    show(&mut harness, &call.handle, 0.6);
    let written = lines(&harness, &call.handle);
    assert!(
        written.iter().any(|line| line.ends_with("Missing input")),
        "a call with its required input missing says so: {written:?}"
    );
    assert!(has(
        &harness,
        &format!("Open the steps of {}", call.display_name())
    ));
}

/// Criterion 7: *Compact cards* off restores today's geometry of every kind.
#[test]
fn lay21_criterion_7_the_switch_restores_every_kind() {
    use apiforge_script::{Document, LowerContext, lower, parse};
    let (_project, mut harness) = open(&shop_30(), WIDE);
    assert_eq!(world_of(&harness, "first").height(), VALUE, "compact first");

    switch_density(&mut harness);
    settle(&mut harness);
    harness.step();
    assert!(!folder(&harness).preferences.compact_cards);

    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&parse(&shop_30()), &LowerContext::folder(&project));
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("shop-30 lowers: {:#?}", lowered.diagnostics);
    };
    let reference = &scenario.graph;
    let mut kinds = Vec::new();
    for node in &reference.nodes {
        let today = Card::build(node, reference, &project, CardDensity::Full).height();
        assert_eq!(
            world_of(&harness, &node.handle).height(),
            today,
            "{} ({:?}) has today's height",
            node.handle,
            node.kind
        );
        if !kinds.contains(&node.kind) {
            kinds.push(node.kind);
        }
    }
    assert!(kinds.len() >= 7, "every kind is compared: {kinds:?}");
    assert_disjoint_cards(&world(&harness), "after the switch to full cards");
}

/// Criterion 8: `shop-30` tidied has no card collision; the board's size
/// and fit at 1600 × 1000 are printed.
#[test]
fn lay21_criterion_8_no_collision() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    harness.get_by_label("Tidy up").click();
    settle(&mut harness);
    harness.step();
    let cards = world(&harness);
    assert_disjoint_cards(&cards, "shop-30 tidied at the compact density");
    let cart = world_of(&harness, "cart").height();
    for node in graph(&harness)
        .nodes
        .iter()
        .filter(|node| satellite(node.kind))
    {
        assert!(
            world_of(&harness, &node.handle).height() < cart,
            "{} is compact on the measured board",
            node.handle
        );
    }
    let bounds = cards
        .iter()
        .map(|(_, rect)| *rect)
        .reduce(Rect::union)
        .unwrap();
    let focus = canvas(&harness).debug_focus();
    let fit = (focus.width() / bounds.width()).min(focus.height() / bounds.height());
    errln!(
        "LAY-21 shop-30 compact: board {:.0} × {:.0}; fit at 1600 × 1000 {fit:.3}",
        bounds.width(),
        bounds.height(),
    );
}
