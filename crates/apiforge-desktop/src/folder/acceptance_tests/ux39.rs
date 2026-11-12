//! UX-39 (#275, the owner's wish of 2026-11-03 on #268, analysed on
//! 2026-11-10): after a paste leaves an input unconnected because the step it
//! read is missing from the target graph, that input shows *Missing step
//! “product”* on the card, in the diagnostic red. The marker is session state
//! of the document's board, never written: wiring the input, giving it a
//! value, a step of that name appearing or closing the document clears it.
use super::ux31::{by_path, window};
use super::*;
use crate::canvas::{Canvas, CardDensity, Command, DrawnText};
use apiforge_core::{block::Block, format::Position};

const CART: &str = "components/add-to-cart.afs";
const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const PASTE: &str = "Paste steps (Ctrl+V)";
/// What the notice bar said before UX-39, and still says.
const NOTICE: &str =
    "Missing step “product”: the input of Active promotion that read it is left unconnected.";
/// The input's line on a compact card.
const COMPACT_MARKER: &str = "Product · Missing step “product”";
/// The input row's value on a full card.
const FULL_MARKER: &str = "Missing step “product”";
/// The compact line of a required input without a value, as before UX-39.
const MISSING_INPUT: &str = "Product · Missing input";

fn model(harness: &FolderHarness, id: &str) -> Document {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .model()
        .expect("a valid model")
        .clone()
}

/// The identifier of the step `handle` on the main board of `document`.
fn id_of(harness: &FolderHarness, document: &str, handle: &str) -> String {
    graph_of(&model(harness, document), false)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone()
}

fn handles(harness: &FolderHarness, document: &str) -> Vec<String> {
    graph_of(&model(harness, document), false)
        .nodes
        .iter()
        .map(|node| node.handle.clone())
        .collect()
}

fn canvas<'a>(harness: &'a FolderHarness, document: &str) -> &'a Canvas {
    folder(harness)
        .canvases
        .get(document)
        .expect("the board is drawn")
}

/// Selects the steps named by their handles on the main board on screen.
fn select(harness: &mut FolderHarness, document: &str, handles: &[&str]) {
    let ids = handles
        .iter()
        .map(|handle| id_of(harness, document, handle))
        .collect();
    app(harness).selections.insert(document.to_owned(), ids);
    settle(harness);
}

fn copy(harness: &mut FolderHarness) {
    harness.event(egui::Event::Copy);
    harness.step();
    settle(harness);
}

fn notice(harness: &FolderHarness) -> String {
    folder(harness)
        .notice()
        .map(|notice| notice.text.clone())
        .unwrap_or_default()
}

/// Centres the card `handle` of `document` at `zoom`.
fn show(harness: &mut FolderHarness, document: &str, handle: &str, zoom: f32) {
    let id = id_of(harness, document, handle);
    app(harness)
        .canvases
        .get_mut(document)
        .expect("the board is drawn")
        .debug_show(&id, zoom);
    settle(harness);
    harness.step();
}

/// The lines the compact card `handle` wrote in the last frame.
fn lines(harness: &FolderHarness, document: &str, handle: &str) -> Vec<String> {
    canvas(harness, document).debug_lines(&id_of(harness, document, handle))
}

/// The texts the card `handle` wrote in the last frame.
fn texts(harness: &FolderHarness, document: &str, handle: &str) -> Vec<DrawnText> {
    canvas(harness, document).debug_texts(&id_of(harness, document, handle))
}

/// The text `text` the card `handle` wrote, if it wrote it.
fn written(harness: &FolderHarness, document: &str, handle: &str, text: &str) -> Option<DrawnText> {
    texts(harness, document, handle)
        .into_iter()
        .find(|drawn| drawn.text == text)
}

/// Whether any card of the board wrote a *Missing step* marker.
fn any_marker(harness: &FolderHarness, document: &str) -> bool {
    handles(harness, document).iter().any(|handle| {
        texts(harness, document, handle)
            .iter()
            .any(|drawn| drawn.text.contains("Missing step"))
    })
}

/// The demo shop with *Active promotion* copied from the scenario and pasted
/// into `add-to-cart.afs`, which has no step `product`, shown at `size`.
fn pasted_into_cart(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    pasted_into_cart_at(size, CardDensity::Compact)
}

/// As [`pasted_into_cart`], the boards drawn at `density` before the paste,
/// so the copy is placed clear of the cards at that density.
fn pasted_into_cart_at(
    size: egui::Vec2,
    density: CardDensity,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    let (mut harness, scenario) = window(project.path(), size, SCENARIO, "Diagram");
    app(&mut harness).switch_density(density);
    settle(&mut harness);
    select(&mut harness, &scenario, &["active_promotion"]);
    copy(&mut harness);
    let cart = by_path(&harness, CART);
    app(&mut harness).activate_tab(&cart);
    settle(&mut harness);
    choose(&mut harness, "Diagram");
    choose(&mut harness, PASTE);
    assert!(
        handles(&harness, &cart).contains(&"active_promotion".to_owned()),
        "the step is pasted into the component: {:?}",
        handles(&harness, &cart)
    );
    (project, harness, cart)
}

/// Criterion 1: after the paste, the card's *Product* input shows *Missing
/// step “product”* in the diagnostic red; the notice bar says what it said
/// before, and the input stays unconnected in the document.
#[test]
fn ux39_criterion_1_the_marker_names_the_missing_step() {
    let (_project, mut harness, cart) = pasted_into_cart(WIDE);
    assert_eq!(notice(&harness), NOTICE, "the notice bar is unchanged");
    let pasted = id_of(&harness, &cart, "active_promotion");
    let document = model(&harness, &cart);
    let node = graph_of(&document, false)
        .nodes
        .iter()
        .find(|node| node.id == pasted)
        .unwrap();
    assert!(
        !node.inputs.contains_key("product"),
        "the input is left unconnected: {:?}",
        node.inputs
    );

    show(&mut harness, &cart, "active_promotion", 1.0);
    let written_lines = lines(&harness, &cart, "active_promotion");
    assert!(
        written_lines.contains(&COMPACT_MARKER.to_owned()),
        "the input's line names the missing step: {written_lines:?}"
    );
    assert!(
        !written_lines.contains(&MISSING_INPUT.to_owned()),
        "the marker takes the place of the plain missing input: {written_lines:?}"
    );
    let marker =
        written(&harness, &cart, "active_promotion", COMPACT_MARKER).expect("the marker is drawn");
    assert_eq!(marker.color, crate::theme::DANGER, "in the diagnostic red");
    // Only the pasted card carries a marker.
    for handle in handles(&harness, &cart) {
        if handle == "active_promotion" {
            continue;
        }
        show(&mut harness, &cart, &handle, 1.0);
        assert!(
            !texts(&harness, &cart, &handle)
                .iter()
                .any(|drawn| drawn.text.contains("Missing step")),
            "{handle} has no marker"
        );
    }
}

/// Criterion 2: wiring the input or giving it a value clears its marker, and
/// so does a step named `product` appearing on the board.
#[test]
fn ux39_criterion_2_wiring_or_the_step_clears_it() {
    // Wired, from the component's own *product* input: the only Product the
    // component has.
    let (_project, mut harness, cart) = pasted_into_cart(WIDE);
    let pasted = id_of(&harness, &cart, "active_promotion");
    show(&mut harness, &cart, "active_promotion", 1.0);
    assert!(lines(&harness, &cart, "active_promotion").contains(&COMPACT_MARKER.to_owned()));
    app(&mut harness).active_board_command(Command::Bind {
        node: pasted.clone(),
        input: "product".into(),
        path: Vec::new(),
        binding: apiforge_core::format::Binding::parameter("product"),
    });
    settle(&mut harness);
    assert!(
        graph_of(&model(&harness, &cart), false)
            .nodes
            .iter()
            .any(|node| node.id == pasted && node.inputs.contains_key("product")),
        "the input is wired"
    );
    show(&mut harness, &cart, "active_promotion", 1.0);
    let after = lines(&harness, &cart, "active_promotion");
    assert!(
        !after.iter().any(|line| line.contains("Missing step")),
        "a wired input has no marker: {after:?}"
    );

    // Given a value: a second paste, then a literal.
    choose(&mut harness, PASTE);
    show(&mut harness, &cart, "active_promotion_2", 1.0);
    assert!(
        lines(&harness, &cart, "active_promotion_2").contains(&COMPACT_MARKER.to_owned()),
        "the second paste is marked too: {:?}",
        lines(&harness, &cart, "active_promotion_2")
    );
    let second = id_of(&harness, &cart, "active_promotion_2");
    app(&mut harness).active_board_command(Command::SetLiteral {
        node: second,
        input: "product".into(),
        path: Vec::new(),
        value: serde_json::json!(123),
    });
    settle(&mut harness);
    show(&mut harness, &cart, "active_promotion_2", 1.0);
    let after = lines(&harness, &cart, "active_promotion_2");
    assert!(
        !after.iter().any(|line| line.contains("Missing step")),
        "an input given a value has no marker: {after:?}"
    );

    // A step named `product` appears: in the scenario, *Product* is deleted
    // after *Active promotion* is copied, the copy is pasted, and a new
    // *Product* is added.
    let project = crate::testing::shop_project();
    let (mut harness, scenario) = window(project.path(), WIDE, SCENARIO, "Diagram");
    select(&mut harness, &scenario, &["active_promotion"]);
    copy(&mut harness);
    let product = id_of(&harness, &scenario, "product");
    app(&mut harness).active_board_command(Command::Delete {
        ids: vec![product],
        wires: Vec::new(),
    });
    settle(&mut harness);
    assert!(!handles(&harness, &scenario).contains(&"product".to_owned()));
    choose(&mut harness, PASTE);
    show(&mut harness, &scenario, "active_promotion_2", 1.0);
    assert!(
        lines(&harness, &scenario, "active_promotion_2").contains(&COMPACT_MARKER.to_owned()),
        "the copy is marked: {:?}",
        lines(&harness, &scenario, "active_promotion_2")
    );
    app(&mut harness).active_board_command(Command::AddNode {
        block: Block::Component,
        component: Some(("get-product".to_owned(), 1)),
        position: Position { x: 40.0, y: 600.0 },
        source: None,
    });
    settle(&mut harness);
    assert!(
        handles(&harness, &scenario).contains(&"product".to_owned()),
        "the added step is named product: {:?}",
        handles(&harness, &scenario)
    );
    show(&mut harness, &scenario, "active_promotion_2", 1.0);
    let after = lines(&harness, &scenario, "active_promotion_2");
    assert!(
        !after.iter().any(|line| line.contains("Missing step")),
        "the marker goes once a step of that name is there: {after:?}"
    );
    assert!(
        after.contains(&MISSING_INPUT.to_owned()),
        "the input is still unconnected, as an ordinary one: {after:?}"
    );
}

/// Criterion 3: the marker is not written. The saved file names no missing
/// step; closing and reopening the document, or reopening the project in a
/// new window, shows an ordinary unconnected input.
#[test]
fn ux39_criterion_3_not_persisted() {
    let (project, mut harness, cart) = pasted_into_cart(WIDE);
    show(&mut harness, &cart, "active_promotion", 1.0);
    assert!(lines(&harness, &cart, "active_promotion").contains(&COMPACT_MARKER.to_owned()));
    choose(&mut harness, "Save");
    let saved = fs::read_to_string(project.path().join(CART)).unwrap();
    assert!(
        saved.contains("active_promotion = active_promotion@1("),
        "the paste is saved:\n{saved}"
    );
    assert!(
        !saved.contains("Missing"),
        "the marker is not saved:\n{saved}"
    );

    // Closed and opened again in the same window.
    app(&mut harness).close_tab(&cart);
    settle(&mut harness);
    app(&mut harness).review_open(CART, None);
    settle(&mut harness);
    choose(&mut harness, "Diagram");
    show(&mut harness, &cart, "active_promotion", 1.0);
    let reopened = lines(&harness, &cart, "active_promotion");
    assert!(
        reopened.contains(&MISSING_INPUT.to_owned()),
        "a reopened document shows an ordinary unconnected input: {reopened:?}"
    );
    assert!(!any_marker(&harness, &cart));
    drop(harness);

    // A new window over the saved project.
    let (mut harness, cart) = window(project.path(), WIDE, CART, "Diagram");
    show(&mut harness, &cart, "active_promotion", 1.0);
    let fresh = lines(&harness, &cart, "active_promotion");
    assert!(
        fresh.contains(&MISSING_INPUT.to_owned()),
        "a new window shows an ordinary unconnected input: {fresh:?}"
    );
    assert!(!any_marker(&harness, &cart));
}

/// Criterion 4: the marker is drawn at both densities, at Medium and at Full
/// zoom: the input's line on a compact card, the input row's value on a full
/// card, both in the diagnostic red.
#[test]
fn ux39_criterion_4_both_densities() {
    let (_project, mut harness, cart) = pasted_into_cart(WIDE);
    assert!(
        canvas(&harness, &cart).debug_compact(),
        "the board starts compact"
    );
    for zoom in [0.6, 1.0] {
        show(&mut harness, &cart, "active_promotion", zoom);
        let marker =
            written(&harness, &cart, "active_promotion", COMPACT_MARKER).unwrap_or_else(|| {
                panic!(
                    "compact at {zoom}: {:?}",
                    lines(&harness, &cart, "active_promotion")
                )
            });
        assert_eq!(marker.color, crate::theme::DANGER, "compact at {zoom}");
    }

    app(&mut harness).switch_density(CardDensity::Full);
    settle(&mut harness);
    assert!(!canvas(&harness, &cart).debug_compact());
    for zoom in [0.6, 1.0] {
        show(&mut harness, &cart, "active_promotion", zoom);
        let marker =
            written(&harness, &cart, "active_promotion", FULL_MARKER).unwrap_or_else(|| {
                panic!(
                    "full at {zoom}: {:?}",
                    texts(&harness, &cart, "active_promotion")
                )
            });
        assert_eq!(marker.color, crate::theme::DANGER, "full at {zoom}");
        assert!(
            !texts(&harness, &cart, "active_promotion")
                .iter()
                .any(|drawn| drawn.text == "Value missing"),
            "the marker takes the place of *Value missing* at {zoom}"
        );
    }
}

/// The marker on a check's one compact line, and on an input of which only
/// one slot read the missing step: *Cart item*'s `productId` field, pasted
/// from the component into the scenario, which has no `product_id`.
#[test]
fn ux39_a_check_and_a_nested_slot_are_marked() {
    let (_project, mut harness, cart) = pasted_into_cart(WIDE);
    let scenario = by_path(&harness, SCENARIO);
    app(&mut harness).activate_tab(&scenario);
    settle(&mut harness);
    select(&mut harness, &scenario, &["promotional_price"]);
    copy(&mut harness);
    app(&mut harness).activate_tab(&cart);
    settle(&mut harness);
    choose(&mut harness, PASTE);
    show(&mut harness, &cart, "promotional_price", 1.0);
    let check = lines(&harness, &cart, "promotional_price");
    let marker = check
        .iter()
        .find(|line| line.contains("Missing step “"))
        .unwrap_or_else(|| panic!("the check's line names a missing step: {check:?}"));
    assert_eq!(
        written(&harness, &cart, "promotional_price", marker).map(|drawn| drawn.color),
        Some(crate::theme::DANGER)
    );

    select(&mut harness, &cart, &["cart_item"]);
    copy(&mut harness);
    app(&mut harness).activate_tab(&scenario);
    settle(&mut harness);
    choose(&mut harness, PASTE);
    let pasted = id_of(&harness, &scenario, "cart_item");
    let document = model(&harness, &scenario);
    let node = graph_of(&document, false)
        .nodes
        .iter()
        .find(|node| node.id == pasted)
        .unwrap();
    assert!(
        !node.inputs.is_empty(),
        "the input keeps what it could: {:?}",
        node.inputs
    );
    show(&mut harness, &scenario, "cart_item", 1.0);
    let item = lines(&harness, &scenario, "cart_item");
    let marker = item
        .iter()
        .find(|line| line.contains("Missing step “product_id”"))
        .unwrap_or_else(|| panic!("the slot that read the missing step is marked: {item:?}"));
    assert_eq!(
        written(&harness, &scenario, "cart_item", marker).map(|drawn| drawn.color),
        Some(crate::theme::DANGER),
        "the nested slot's marker is in the diagnostic red"
    );
}

/// Asserts that the last frame drew the card `handle` whole inside the
/// viewport.
fn assert_card_in_view(harness: &FolderHarness, document: &str, handle: &str) {
    let board = canvas(harness, document);
    let card = board
        .debug_card_rect(&id_of(harness, document, handle))
        .unwrap_or_else(|| panic!("{handle} is drawn"));
    assert!(
        board.debug_viewport().contains_rect(card),
        "{handle} {card:?} is in the viewport {:?}",
        board.debug_viewport()
    );
}

/// The marker on a compact card at 100 % and on a full card at 100 %, after
/// *Active promotion* is pasted into `add-to-cart.afs`.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux39_marker() {
    let (_project, mut harness, cart) = pasted_into_cart(WIDE);
    let region = |harness: &FolderHarness| {
        canvas(harness, &cart)
            .debug_card_rect(&id_of(harness, &cart, "active_promotion"))
            .map(|rect| rect.expand(160.0))
    };
    show(&mut harness, &cart, "active_promotion", 1.0);
    assert_card_in_view(&harness, &cart, "active_promotion");
    let around = region(&harness);
    crate::testing::capture(&mut harness, "ux39-compact-100-1600", around);
    crate::testing::capture(&mut harness, "ux39-compact-window-1600", None);
    drop(harness);

    let (_project, mut harness, cart) = pasted_into_cart_at(WIDE, CardDensity::Full);
    let region = |harness: &FolderHarness| {
        canvas(harness, &cart)
            .debug_card_rect(&id_of(harness, &cart, "active_promotion"))
            .map(|rect| rect.expand(160.0))
    };
    show(&mut harness, &cart, "active_promotion", 1.0);
    assert_card_in_view(&harness, &cart, "active_promotion");
    let around = region(&harness);
    crate::testing::capture(&mut harness, "ux39-full-100-1600", around);
}
