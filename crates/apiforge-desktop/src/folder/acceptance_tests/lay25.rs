//! LAY-25 (#335): a value used far away is saved in a variable, proposed by
//! the board. A wire or a response field dropped on a far input asks first
//! (*Read via variable*, under Enter, or *Wire directly*); a port whose field
//! three or more steps read, one of them far, shows `+ var` under its count;
//! the right-click menu saves every such value at once. Each is one undoable
//! edit; a variable's far reader is a chip whatever the count; *Read
//! directly again* takes a variable back. On `far-value` and `shop-30`.
use super::lay16::{centre, click_at, move_to, press, release};
use super::*;
use crate::dnd::Payload;
use apiforge_core::format::{Binding, Graph, Node};
use egui::{Pos2, Rect, pos2};
use egui_kittest::kittest::NodeT;
use std::collections::BTreeSet;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";
const SAVE_SHARED: &str = "Save shared values as variables";
const READ_DIRECTLY: &str = "Read directly again";

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// `source` on the board of the demo shop, as a new install opens it
/// (compact cards, checks collapsed, chips on), without positions, tidied
/// up once more with nothing selected.
fn tidied_source(source: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    harness.step();
    (project, harness, id)
}

fn tidied(name: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    tidied_source(&fixture(name), size)
}

fn fit(harness: &mut FolderHarness) {
    choose(harness, FIT);
    harness.step();
}

fn main_graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

fn cleanup_graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), true).clone()
}

/// Main and Cleanup, the nodes of both.
fn nodes(harness: &FolderHarness, id: &str) -> Vec<Node> {
    let mut nodes = main_graph(harness, id).nodes;
    nodes.extend(cleanup_graph(harness, id).nodes);
    nodes
}

fn node<'a>(nodes: &'a [Node], handle: &str) -> &'a Node {
    nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
}

fn id_of(harness: &FolderHarness, id: &str, handle: &str) -> String {
    node(&nodes(harness, id), handle).id.clone()
}

fn variables(harness: &FolderHarness, id: &str) -> BTreeSet<String> {
    nodes(harness, id)
        .iter()
        .filter(|node| node.is_variable())
        .map(|node| node.handle.clone())
        .collect()
}

/// The steps of Main and Cleanup that read `port` and `path` of `handle`
/// exactly, by handle.
fn readers(harness: &FolderHarness, id: &str, handle: &str, port: &str, path: &str) -> Vec<String> {
    let nodes = nodes(harness, id);
    let source = node(&nodes, handle).id.clone();
    nodes
        .iter()
        .filter(|node| {
            node.inputs.values().any(|binding| {
                binding.references().iter().any(|reference| {
                    reference.node_id == source && reference.port == port && reference.path == path
                })
            })
        })
        .map(|node| node.handle.clone())
        .collect()
}

fn text(harness: &FolderHarness, id: &str) -> String {
    source_of(harness, id)
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

fn undo(harness: &mut FolderHarness) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(harness);
    harness.step();
}

fn redo(harness: &mut FolderHarness) {
    harness.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    settle(harness);
    harness.step();
}

fn has(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label(label).is_some()
}

fn has_containing(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label_contains(label).is_some()
}

/// The label of the one widget whose label contains `part`.
fn label_containing(harness: &FolderHarness, part: &str) -> String {
    harness
        .get_by_label_contains(part)
        .accesskit_node()
        .label()
        .unwrap_or_default()
        .to_owned()
}

/// Carries a wire from the output `output` of `source` to the input
/// `input` of `target`, past the target card so that its hidden inputs
/// show, and releases it there.
fn drag_wire(
    harness: &mut FolderHarness,
    id: &str,
    source: &str,
    output: &str,
    target: &str,
    input: &str,
) {
    let from = centre(harness, &format!("Output {output} of {source}"));
    let card = board(harness, id)
        .debug_card_rect(&id_of(harness, id, target))
        .unwrap_or_else(|| panic!("{target} is drawn"));
    let near = pos2(card.left() - 12.0, card.center().y);
    press(harness, from);
    move_to(harness, from, near);
    let socket = centre(harness, &format!("Input {input} of {target}"));
    move_to(harness, near, socket);
    release(harness, socket);
    harness.step();
}

/// Carries a field of a response, as the response tree hands it to a drag
/// (`Payload::Variable` with the field's reference), to the input `input`
/// of `target` and releases it there.
fn drop_field(
    harness: &mut FolderHarness,
    id: &str,
    binding: Binding,
    label: &str,
    target: &str,
    input: &str,
) {
    let card = board(harness, id)
        .debug_card_rect(&id_of(harness, id, target))
        .unwrap_or_else(|| panic!("{target} is drawn"));
    egui::DragAndDrop::set_payload(
        &harness.ctx,
        Payload::Variable {
            label: label.to_owned(),
            binding,
        },
    );
    let near = pos2(card.left() - 12.0, card.center().y);
    harness.hover_at(near);
    harness.step();
    harness.hover_at(card.center());
    harness.step();
    let row = centre(harness, &format!("Input {input} of {target}"));
    harness.hover_at(row);
    harness.step();
    release(harness, row);
    harness.step();
}

/// `cart.id` as the response tree of `cart` hands it to a drag.
fn cart_id(harness: &FolderHarness, id: &str) -> Binding {
    Binding::reference(id_of(harness, id, "cart"), "cart", "id")
}

/// A point of the empty board, in the part the camera fits: no card, chip
/// or count within 40 px.
fn empty_point(harness: &FolderHarness, id: &str) -> Pos2 {
    let canvas = board(harness, id);
    let focus = canvas.debug_focus().shrink(20.0);
    let cards: Vec<Rect> = canvas.debug_cards().iter().map(|(_, rect)| *rect).collect();
    let tags: Vec<Rect> = canvas.debug_tags().iter().map(|tag| tag.rect).collect();
    let counts: Vec<Rect> = canvas
        .debug_counts()
        .iter()
        .map(|count| canvas.debug_to_screen(count.world))
        .collect();
    let mut y = focus.top();
    while y < focus.bottom() {
        let mut x = focus.left();
        while x < focus.right() {
            let at = pos2(x, y);
            if cards
                .iter()
                .chain(&tags)
                .chain(&counts)
                .all(|rect| !rect.expand(40.0).contains(at))
            {
                return at;
            }
            x += 20.0;
        }
        y += 20.0;
    }
    panic!("the board has an empty place in view");
}

/// Opens the menu a right-click on the empty board opens.
fn right_click_board(harness: &mut FolderHarness, id: &str) {
    let at = empty_point(harness, id);
    click_at(harness, at, egui::PointerButton::Secondary, 1);
    harness.step();
    assert!(board(harness, id).debug_menu_open(), "the menu opens");
}

/// What a step reads at `input`, by handle.
fn input_of(harness: &FolderHarness, id: &str, handle: &str, input: &str) -> Option<Binding> {
    node(&nodes(harness, id), handle).inputs.get(input).cloned()
}

fn reads_variable(
    harness: &FolderHarness,
    id: &str,
    handle: &str,
    input: &str,
    variable: &str,
) -> bool {
    let variable = id_of(harness, id, variable);
    input_of(harness, id, handle, input).is_some_and(|binding| {
        binding
            .references()
            .iter()
            .any(|reference| reference.node_id == variable && reference.path.is_empty())
    })
}

/// The statement after the one that starts with `start`, trimmed.
fn line_after(text: &str, start: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let index = lines
        .iter()
        .position(|line| line.starts_with(start))
        .unwrap_or_else(|| panic!("a line {start}"));
    // The statement ends at the first line that closes it at its depth.
    let mut depth = 0i32;
    for (offset, line) in lines[index..].iter().enumerate() {
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        if depth <= 0 {
            return lines
                .get(index + offset + 1)
                .copied()
                .unwrap_or_default()
                .to_owned();
        }
    }
    String::new()
}

/// Criterion 1, a wire: a wire from `session`'s port released on an input of
/// a step that is not beside `session` opens the choice inside the board.
/// Enter writes the variable, makes the new reader read it and selects it;
/// Esc writes nothing; the second row connects as today.
#[test]
fn lay25_criterion_1_a_far_wire_asks_first() {
    for size in [WIDE, crate::testing::MINIMUM] {
        let (_project, mut harness, id) = tidied("far-value", size);
        fit(&mut harness);
        let before = text(&harness, &id);

        // Esc: nothing connected, nothing written.
        drag_wire(
            &mut harness,
            &id,
            "session",
            "body",
            "remove_coupon",
            "body",
        );
        assert!(
            has(&harness, "Read via variable sessionBody"),
            "{size:?}: the choice is open"
        );
        let offer = board(&harness, &id).debug_offer().expect("the choice");
        assert!(
            board(&harness, &id).debug_viewport().contains_rect(offer),
            "{size:?}: the choice {offer:?} is inside the board"
        );
        assert!(
            has(&harness, "Wire directly to session.body"),
            "{size:?}: the second row"
        );
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
        harness.step();
        assert!(
            board(&harness, &id).debug_offer().is_none(),
            "Esc closes it"
        );
        assert_eq!(text(&harness, &id), before, "{size:?}: Esc writes nothing");

        // Enter: the first row.
        drag_wire(
            &mut harness,
            &id,
            "session",
            "body",
            "remove_coupon",
            "body",
        );
        harness.key_press(egui::Key::Enter);
        settle(&mut harness);
        harness.step();
        let after = text(&harness, &id);
        assert_eq!(after.matches("var sessionBody = session.body").count(), 1);
        assert!(reads_variable(
            &harness,
            &id,
            "remove_coupon",
            "body",
            "sessionBody"
        ));
        assert_eq!(
            folder(&harness)
                .selections
                .get(&id)
                .cloned()
                .unwrap_or_default(),
            vec![id_of(&harness, &id, "sessionBody")],
            "{size:?}: the variable is selected"
        );
        undo(&mut harness);
        assert_eq!(text(&harness, &id), before);

        // The second row connects as today.
        drag_wire(
            &mut harness,
            &id,
            "session",
            "body",
            "remove_coupon",
            "body",
        );
        harness
            .get_by_label("Wire directly to session.body")
            .click();
        settle(&mut harness);
        harness.step();
        assert!(variables(&harness, &id).is_empty(), "{size:?}: no variable");
        assert_eq!(
            input_of(&harness, &id, "remove_coupon", "body"),
            Some(Binding::reference(
                id_of(&harness, &id, "session"),
                "body",
                ""
            )),
            "{size:?}: wired directly"
        );
    }
}

/// Criterion 1, a field: `cart.id` dragged from the response tree onto
/// a far input opens the choice; Enter writes `var cartId = cart.id`
/// after `cart`, every step that read the value reads `cartId`, the new one
/// included, and the variable is selected.
#[test]
fn lay25_criterion_1_a_far_field_asks_first() {
    let (_project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    let before = text(&harness, &id);
    let direct = readers(&harness, &id, "cart", "cart", "id");
    assert_eq!(direct.len(), 9, "nine steps read cart.id");

    let value = cart_id(&harness, &id);
    drop_field(
        &mut harness,
        &id,
        value.clone(),
        "id",
        "remove_coupon",
        "body",
    );
    assert!(has(&harness, "Read via variable cartId"), "the choice");
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    harness.step();
    assert_eq!(text(&harness, &id), before, "Esc writes nothing");

    drop_field(&mut harness, &id, value, "id", "remove_coupon", "body");
    harness.key_press(egui::Key::Enter);
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    assert_eq!(
        line_after(&after, "cart = add_to_cart@1"),
        "var cartId = cart.id",
        "the variable follows its source"
    );
    assert!(
        readers(&harness, &id, "cart", "cart", "id") == vec!["cartId".to_owned()],
        "only the variable reads cart.id"
    );
    for reader in direct
        .iter()
        .chain(std::iter::once(&"remove_coupon".to_owned()))
    {
        assert!(
            nodes(&harness, &id)
                .iter()
                .find(|node| &node.handle == reader)
                .unwrap()
                .inputs
                .values()
                .any(|binding| binding.references_node(&id_of(&harness, &id, "cartId"))),
            "{reader} reads cartId"
        );
    }
    assert_eq!(
        folder(&harness)
            .selections
            .get(&id)
            .cloned()
            .unwrap_or_default(),
        vec![id_of(&harness, &id, "cartId")]
    );
}

/// Criterion 2: a wire dropped on a step beside its source connects as
/// today, with no choice.
#[test]
fn lay25_criterion_2_a_near_wire_connects() {
    let (_project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    drag_wire(&mut harness, &id, "session", "body", "catalogue", "body");
    assert!(board(&harness, &id).debug_offer().is_none(), "no choice");
    assert!(variables(&harness, &id).is_empty());
    assert_eq!(
        input_of(&harness, &id, "catalogue", "body"),
        Some(Binding::reference(
            id_of(&harness, &id, "session"),
            "body",
            ""
        ))
    );
}

/// Criterion 3: where a variable already holds exactly the value, the first
/// row reads *Read cartId* and no variable is added.
#[test]
fn lay25_criterion_3_a_held_value_is_read_through_its_variable() {
    let (_project, mut harness, id) = tidied("far-value-saved", WIDE);
    fit(&mut harness);
    let before = variables(&harness, &id);
    let value = cart_id(&harness, &id);
    drop_field(&mut harness, &id, value, "id", "remove_coupon", "body");
    assert!(
        has(&harness, "Read cartId"),
        "the first row names the variable"
    );
    harness.key_press(egui::Key::Enter);
    settle(&mut harness);
    harness.step();
    assert_eq!(variables(&harness, &id), before, "no variable is added");
    assert!(reads_variable(
        &harness,
        &id,
        "remove_coupon",
        "body",
        "cartId"
    ));
}

/// Criterion 4: a request's or a component call's output with a field read
/// by three or more steps, one of them far, shows `+ var` under its count;
/// the mark covers no card, port, chip or count; its menu has one row per
/// such field with the line it writes and the number of steps; a row does
/// the edit of criterion 1.
#[test]
fn lay25_criterion_4_the_port_proposes_its_fields() {
    for (name, source, zoom) in [
        ("far-value", "cart", 0.6),
        ("shop-30", "login", 0.6),
        ("far-value", "cart", 1.0),
    ] {
        let (_project, mut harness, id) = tidied(name, WIDE);
        fit(&mut harness);
        show(&mut harness, &id, source, zoom);
        let canvas = board(&harness, &id);
        let marks = canvas.debug_var_marks().to_vec();
        let source_id = id_of(&harness, &id, source);
        assert!(
            marks.iter().any(|mark| mark.source == source_id),
            "{name}: {source} shows + var"
        );
        // Only requests and component calls with such a field show it.
        let nodes = nodes(&harness, &id);
        for mark in &marks {
            let handle = &nodes
                .iter()
                .find(|node| node.id == mark.source)
                .unwrap()
                .handle;
            assert!(
                ["cart", "login"].contains(&handle.as_str()) || name == "shop-30",
                "{name}: {handle} shows no mark"
            );
        }
        let session = nodes.iter().find(|node| node.handle == "session");
        if let Some(session) = session {
            assert!(
                marks.iter().all(|mark| mark.source != session.id),
                "{name}: two readers of currency make no mark"
            );
        }
        let cards: Vec<Rect> = canvas.debug_cards().iter().map(|(_, rect)| *rect).collect();
        let tags: Vec<Rect> = canvas.debug_tags().iter().map(|tag| tag.rect).collect();
        let counts: Vec<Rect> = harness
            .query_all_by_label_contains(" readers")
            .map(|node| node.rect())
            .collect();
        let viewport = canvas.debug_viewport();
        for mark in marks
            .iter()
            .filter(|mark| viewport.contains_rect(mark.rect))
        {
            for card in &cards {
                assert!(
                    !mark.rect.intersects(card.expand(3.0)),
                    "{name} at {zoom}: + var {:?} covers a card or its port {card:?}",
                    mark.rect
                );
            }
            for other in tags.iter().chain(&counts) {
                assert!(
                    !mark.rect.intersects(*other),
                    "{name} at {zoom}: + var {:?} covers a chip or a count {other:?}",
                    mark.rect
                );
            }
            for other in marks.iter().filter(|other| other.rect != mark.rect) {
                assert!(
                    !mark.rect.intersects(other.rect),
                    "{name}: two marks overlap"
                );
            }
        }
    }

    // far-value: the menu of cart's mark, and a row does the edit.
    let (_project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    show(&mut harness, &id, "cart", 0.6);
    let before = text(&harness, &id);
    harness
        .get_by_label("Save a value of cart as a variable")
        .click();
    settle(&mut harness);
    harness.step();
    let cart_row = label_containing(&harness, "var cartId = cart.id");
    assert!(cart_row.contains("9 steps"), "{cart_row}");
    let product_row = label_containing(&harness, "var productId = cart.productId");
    assert!(product_row.contains("6 steps"), "{product_row}");
    assert!(!has_containing(&harness, "currency"), "two readers: no row");
    harness
        .get_by_label_contains("var cartId = cart.id")
        .click();
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    assert_eq!(
        line_after(&after, "cart = add_to_cart@1"),
        "var cartId = cart.id"
    );
    assert_eq!(
        readers(&harness, &id, "cart", "cart", "id"),
        vec!["cartId".to_owned()]
    );
    assert_eq!(
        folder(&harness)
            .selections
            .get(&id)
            .cloned()
            .unwrap_or_default(),
        vec![id_of(&harness, &id, "cartId")]
    );
    undo(&mut harness);
    assert_eq!(text(&harness, &id), before);

    // shop-30: login's mark lists token.
    let (_project, mut harness, id) = tidied("shop-30", WIDE);
    fit(&mut harness);
    show(&mut harness, &id, "login", 0.6);
    harness
        .get_by_label("Save a value of login as a variable")
        .click();
    settle(&mut harness);
    harness.step();
    assert!(has_containing(&harness, "var token = login.body.token"));
}

/// Criterion 5: *Save shared values as variables (N)* stands at the bottom
/// of the menu a right-click on the empty board opens, in sight whatever is
/// searched, and not in the menu opened from a port. On `shop-30` it saves
/// `token`, `cartId` and `orderId` in one edit, each after its source, the
/// reading steps changing in their references only.
#[test]
fn lay25_criterion_5_the_menu_saves_every_shared_value() {
    for size in [WIDE, crate::testing::MINIMUM] {
        let (_project, mut harness, id) = tidied("shop-30", size);
        fit(&mut harness);
        right_click_board(&mut harness, &id);
        let entry = format!("{SAVE_SHARED} (3)");
        assert!(has(&harness, &entry), "{size:?}: the entry");
        let search = harness.get_by_label("Find an action").rect();
        let shown = harness.get_by_label(&entry).rect();
        assert!(shown.top() > search.bottom(), "{size:?}: below the list");
        assert!(
            harness.ctx.content_rect().contains_rect(shown),
            "{size:?}: in sight {shown:?}"
        );
        // Searched for nothing it matches, it is still there.
        harness.get_by_label("Find an action").type_text("zzzz");
        settle(&mut harness);
        harness.step();
        assert!(
            has(&harness, &entry),
            "{size:?}: in sight whatever is searched"
        );
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
        harness.step();
    }

    // Not in the menu a wire dropped on the empty board opens.
    let (_project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    let from = centre(&harness, "Output body of session");
    let to = empty_point(&harness, &id);
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    release(&mut harness, to);
    harness.step();
    assert!(
        board(&harness, &id).debug_menu_open(),
        "the menu from a port"
    );
    assert!(
        !has_containing(&harness, SAVE_SHARED),
        "no entry from a port"
    );

    // shop-30: three values in one edit.
    let (_project, mut harness, id) = tidied("shop-30", WIDE);
    fit(&mut harness);
    let before = text(&harness, &id);
    let main_before = main_graph(&harness, &id);
    let cleanup_before = cleanup_graph(&harness, &id);
    assert_eq!(main_before.nodes.len(), 30);
    right_click_board(&mut harness, &id);
    harness.get_by_label(&format!("{SAVE_SHARED} (3)")).click();
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    for (source, line) in [
        ("login = POST", "var token = login.body.token"),
        ("cart = POST", "var cartId = cart.body.id"),
        ("order = GET", "var orderId = order.body.id"),
    ] {
        assert_eq!(after.matches(line).count(), 1, "{line}");
        assert_eq!(
            line_after(&after, source),
            line,
            "{line} follows its source"
        );
    }
    assert_eq!(
        after.lines().count(),
        before.lines().count() + 3,
        "the text gains three lines"
    );
    let main_after = main_graph(&harness, &id);
    assert_eq!(main_after.nodes.len(), 33, "30 steps become 33");
    // Every other step is as it was, but for what it reads.
    let variables: Vec<(&str, &str, &str, &str)> = vec![
        ("token", "login", "body", "token"),
        ("cartId", "cart", "body", "id"),
        ("orderId", "order", "body", "id"),
    ];
    let all_after: Vec<Node> = main_after
        .nodes
        .iter()
        .chain(&cleanup_graph(&harness, &id).nodes)
        .cloned()
        .collect();
    let all_before: Vec<Node> = main_before
        .nodes
        .iter()
        .chain(&cleanup_before.nodes)
        .cloned()
        .collect();
    let id_by_handle = |nodes: &[Node], handle: &str| node(nodes, handle).id.clone();
    let mut changed = 0;
    for old in &all_before {
        let new = node(&all_after, &old.handle);
        let mut restored = new.clone();
        for (input, binding) in &new.inputs {
            let mapped = binding
                .map_leaves(&mut |leaf, _| {
                    for (variable, source, port, path) in &variables {
                        if let Binding::Reference {
                            node_id,
                            path: read,
                            ..
                        } = leaf
                            && *node_id == id_by_handle(&all_after, variable)
                        {
                            let joined = if read.is_empty() {
                                (*path).to_owned()
                            } else {
                                format!("{path}.{read}")
                            };
                            return Some(Binding::reference(
                                id_by_handle(&all_after, source),
                                *port,
                                joined,
                            ));
                        }
                    }
                    Some(leaf.clone())
                })
                .unwrap();
            restored.inputs.insert(input.clone(), mapped);
        }
        assert_eq!(
            restored.inputs, old.inputs,
            "{} changes in its references only",
            old.handle
        );
        assert_eq!(restored.kind, old.kind);
        if new.inputs != old.inputs {
            changed += 1;
        }
    }
    // Sixteen reads in Main: ten of token, three of cart.body.id and three
    // of order.body.id, by ten steps (those that read an id read token
    // too); the Cleanup step `removed` reads token and cart.body.id.
    assert_eq!(changed, 11, "the reading steps of Main and Cleanup");
    assert!(
        readers(&harness, &id, "login", "body", "token") == vec!["token".to_owned()],
        "Cleanup reads token too"
    );
}

/// Criterion 6: a value a variable already holds gets no second variable,
/// the steps still reading it move to it; a clash of names between two
/// values of one run is settled in document order; no card moves; the new
/// variables are selected.
#[test]
fn lay25_criterion_6_held_values_and_clashes() {
    // far-value with cartId already held, read directly by every step.
    let source = fixture("far-value").replace(
        "  cart = add_to_cart@1(product: product, quantity: 2)\n",
        "  cart = add_to_cart@1(product: product, quantity: 2)\n  var cartId = cart.id\n",
    );
    let (_project, mut harness, id) = tidied_source(&source, WIDE);
    fit(&mut harness);
    let world = |harness: &FolderHarness| -> Vec<(String, Rect)> {
        nodes(harness, &id)
            .iter()
            .filter_map(|node| {
                board(harness, &id)
                    .debug_world(&node.id)
                    .map(|rect| (node.id.clone(), rect))
            })
            .collect()
    };
    let cards = world(&harness);
    right_click_board(&mut harness, &id);
    harness.get_by_label(&format!("{SAVE_SHARED} (2)")).click();
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    assert_eq!(after.matches("var cartId").count(), 1, "no second variable");
    assert_eq!(
        readers(&harness, &id, "cart", "cart", "id"),
        vec!["cartId".to_owned()],
        "the readers moved to cartId"
    );
    assert!(after.contains("var productId = cart.productId"));
    let now = world(&harness);
    let moved: Vec<&String> = cards
        .iter()
        .filter(|(card, rect)| {
            now.iter()
                .find(|(other, _)| other == card)
                .is_some_and(|(_, now)| now.min != rect.min)
        })
        .map(|(card, _)| card)
        .collect();
    assert!(moved.is_empty(), "no card moves: {moved:?}");
    let selected = folder(&harness)
        .selections
        .get(&id)
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        selected,
        vec![id_of(&harness, &id, "productId")],
        "the new variable is selected"
    );

    // Two values named token: the first in document order keeps it.
    let source = r#"scenario "Two tokens" {
  begin = start
  login = POST "/login" {
    sample 200 { token: "t-1" }
  }
  refresh = POST "/refresh" {
    sample 200 { token: "t-2" }
  }
  a = GET "/a/{{login.body.token}}/{{refresh.body.token}}" {
    sample 200 {}
  }
  b = GET "/b/{{login.body.token}}/{{refresh.body.token}}" {
    sample 200 {}
  }
  c = GET "/c/{{login.body.token}}/{{refresh.body.token}}" {
    sample 200 {}
  }
}
"#;
    let (_project, mut harness, id) = tidied_source(source, WIDE);
    fit(&mut harness);
    right_click_board(&mut harness, &id);
    harness.get_by_label(&format!("{SAVE_SHARED} (2)")).click();
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    assert!(after.contains("var token = login.body.token"), "{after}");
    assert!(
        after.contains("var refreshToken = refresh.body.token"),
        "{after}"
    );
    assert!(after.contains("/c/{{token}}/{{refreshToken}}"), "{after}");
}

/// Criterion 7: with nothing to save the entry is there, disabled, `(0)`,
/// with the reason on hover; on a board that cannot be edited it is
/// disabled too. Nothing is written.
#[test]
fn lay25_criterion_7_nothing_to_save() {
    let (_project, mut harness, id) = tidied("far-value-saved", WIDE);
    fit(&mut harness);
    let before = text(&harness, &id);
    right_click_board(&mut harness, &id);
    let entry = format!("{SAVE_SHARED} (0)");
    assert!(
        harness.get_by_label(&entry).accesskit_node().is_disabled(),
        "disabled with (0)"
    );
    assert!(
        super::ux02::hover_shows(&mut harness, &entry, "No value"),
        "the reason on hover"
    );
    harness.get_by_label(&entry).click();
    settle(&mut harness);
    assert_eq!(text(&harness, &id), before, "nothing is written");

    // A board that cannot be edited: the source does not parse.
    let (_project, mut harness, id) = tidied("shop-30", WIDE);
    fit(&mut harness);
    let broken = format!("{}\n}}\n", text(&harness, &id));
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &broken, CODE)
        .unwrap();
    settle(&mut harness);
    harness.step();
    assert!(!folder(&harness).snapshot(&id).unwrap().editable);
    board_mut(&mut harness, &id).debug_open_menu();
    harness.step();
    harness.step();
    let entry = format!("{SAVE_SHARED} (3)");
    if has(&harness, &entry) {
        assert!(
            harness.get_by_label(&entry).accesskit_node().is_disabled(),
            "disabled on a board that cannot be edited"
        );
    }
    assert_eq!(text(&harness, &id), broken, "nothing is written");
}

/// Criterion 8: the name is the last field, the step and the field where
/// the field alone says nothing or is taken, then a number.
#[test]
fn lay25_criterion_8_names() {
    let source = r#"scenario "Names" {
  begin = start
  login = POST "/login" {
    sample 200 { token: "t-1", id: "u-1" }
  }
  token = "taken"
  loginStatus = "taken"
  a = GET "/a/{{login.body.token}}/{{login.body.id}}/{{login.status}}" {
    sample 200 {}
  }
  b = GET "/b/{{login.body.token}}/{{login.body.id}}/{{login.status}}" {
    sample 200 {}
  }
  c = GET "/c/{{login.body.token}}/{{login.body.id}}/{{login.status}}" {
    sample 200 {}
  }
}
"#;
    let (_project, mut harness, id) = tidied_source(source, WIDE);
    fit(&mut harness);
    right_click_board(&mut harness, &id);
    harness.get_by_label(&format!("{SAVE_SHARED} (3)")).click();
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    assert!(
        after.contains("var loginToken = login.body.token"),
        "taken: {after}"
    );
    assert!(after.contains("var loginId = login.body.id"), "id: {after}");
    assert!(
        after.contains("var loginStatus_2 = login.status"),
        "then a number: {after}"
    );

    // far-value and shop-30: cartId and token.
    for (name, line) in [
        ("far-value", "var cartId = cart.id"),
        ("shop-30", "var token = login.body.token"),
    ] {
        let (_project, mut harness, id) = tidied(name, WIDE);
        fit(&mut harness);
        right_click_board(&mut harness, &id);
        harness.get_by_label_contains(SAVE_SHARED).click();
        settle(&mut harness);
        harness.step();
        assert!(text(&harness, &id).contains(line), "{name}: {line}");
    }
}

/// Criterion 9: one undo after each gesture restores the text byte for
/// byte; redo applies it again.
#[test]
fn lay25_criterion_9_one_undo() {
    // The drop.
    let (_project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    let before = text(&harness, &id);
    let value = cart_id(&harness, &id);
    drop_field(&mut harness, &id, value, "id", "remove_coupon", "body");
    harness.key_press(egui::Key::Enter);
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    assert_ne!(after, before);
    undo(&mut harness);
    assert_eq!(text(&harness, &id), before, "the drop: one undo");
    redo(&mut harness);
    assert_eq!(text(&harness, &id), after, "the drop: redo");

    // The port.
    let (_project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    show(&mut harness, &id, "cart", 0.6);
    let before = text(&harness, &id);
    harness
        .get_by_label("Save a value of cart as a variable")
        .click();
    settle(&mut harness);
    harness.step();
    harness
        .get_by_label_contains("var productId = cart.productId")
        .click();
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    assert_ne!(after, before);
    undo(&mut harness);
    assert_eq!(text(&harness, &id), before, "the port: one undo");
    redo(&mut harness);
    assert_eq!(text(&harness, &id), after, "the port: redo");

    // The menu.
    let (_project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    let before = text(&harness, &id);
    right_click_board(&mut harness, &id);
    harness.get_by_label(&format!("{SAVE_SHARED} (2)")).click();
    settle(&mut harness);
    harness.step();
    let after = text(&harness, &id);
    assert_ne!(after, before);
    undo(&mut harness);
    assert_eq!(text(&harness, &id), before, "the menu: one undo");
    redo(&mut harness);
    assert_eq!(text(&harness, &id), after, "the menu: redo");

    // The menu on shop-30, Main and Cleanup together. Its text is not the
    // one the scenario printer writes (`on_error stop` is left out, `refund`
    // follows its branch with `@after`), so a board edit and its undo
    // write the printer's text, as every board edit does (ADR 0012): one
    // undo restores both graphs.
    let (_project, mut harness, id) = tidied("shop-30", WIDE);
    fit(&mut harness);
    // The graphs as sets: the printer orders the statements of the branch.
    let graphs = |harness: &FolderHarness| {
        let mut graphs = (main_graph(harness, &id), cleanup_graph(harness, &id));
        for graph in [&mut graphs.0, &mut graphs.1] {
            graph.nodes.sort_by(|a, b| a.id.cmp(&b.id));
            graph.edges.sort_by(|a, b| a.id.cmp(&b.id));
        }
        graphs
    };
    let before = graphs(&harness);
    right_click_board(&mut harness, &id);
    harness.get_by_label(&format!("{SAVE_SHARED} (3)")).click();
    settle(&mut harness);
    harness.step();
    let after = graphs(&harness);
    assert_ne!(after, before);
    undo(&mut harness);
    assert_eq!(graphs(&harness), before, "shop-30: one undo");
    redo(&mut harness);
    assert_eq!(graphs(&harness), after, "shop-30: redo");
}

/// Criterion 10: a variable's far reader is a chip whatever the number of
/// readers; a reader beside a variable that no far step reads keeps its
/// wire, a variable read far is read through chips at every reader (LAY-32);
/// no gesture adds a proxy.
#[test]
fn lay25_criterion_10_a_variable_is_read_through_chips() {
    let (_project, mut harness, id) = tidied("far-value-saved", WIDE);
    fit(&mut harness);
    let graph = main_graph(&harness, &id);
    let handle = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map_or_else(|| id.to_owned(), |node| node.handle.clone())
    };
    let canvas = board(&harness, &id);
    let chips: BTreeSet<(String, String)> = canvas
        .debug_tags()
        .iter()
        .map(|tag| (handle(&tag.tag.reader), tag.tag.text.clone()))
        .collect();
    for reader in ["checkout", "lookup"] {
        assert!(
            chips.contains(&(reader.to_owned(), "currency".to_owned())),
            "{reader}: a chip of currency, read by two steps: {chips:?}"
        );
    }
    // fan-out: `first`, read by one step beside it, keeps its wire; `cartId`,
    // read far, gives `quantity` beside it a chip (LAY-32).
    let (_project, harness, id) = tidied("fan-out", WIDE);
    let graph = main_graph(&harness, &id);
    let handle = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map_or_else(|| id.to_owned(), |node| node.handle.clone())
    };
    let wires: BTreeSet<(String, String)> = board(&harness, &id)
        .debug_wires()
        .iter()
        .filter(|wire| !wire.dashed && !wire.revealed)
        .filter_map(|wire| Some((handle(wire.source.as_deref()?), handle(&wire.target))))
        .collect();
    assert!(
        wires.contains(&("first".to_owned(), "product".to_owned())),
        "product beside first keeps its wire: {wires:?}"
    );
    assert!(
        !wires.contains(&("cartId".to_owned(), "quantity".to_owned())),
        "quantity reads cartId through its chip: {wires:?}"
    );
    assert!(
        board(&harness, &id)
            .debug_tags()
            .iter()
            .any(|tag| { handle(&tag.tag.reader) == "quantity" && tag.tag.text == "cartId" }),
        "quantity carries a chip of cartId"
    );

    // No gesture adds a proxy.
    let (project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    let value = cart_id(&harness, &id);
    drop_field(&mut harness, &id, value, "id", "remove_coupon", "body");
    harness.key_press(egui::Key::Enter);
    settle(&mut harness);
    harness.step();
    show(&mut harness, &id, "cart", 0.6);
    harness
        .get_by_label("Save a value of cart as a variable")
        .click();
    settle(&mut harness);
    harness.step();
    harness
        .get_by_label_contains("var productId = cart.productId")
        .click();
    settle(&mut harness);
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    settle(&mut harness);
    assert!(main_graph(&harness, &id).proxies.is_empty(), "no proxy");
    assert!(cleanup_graph(&harness, &id).proxies.is_empty(), "no proxy");
    let diagram: serde_json::Value =
        serde_json::from_slice(&fs::read(project.path().join(DIAGRAM)).unwrap()).unwrap();
    fn proxies(value: &serde_json::Value) -> usize {
        match value {
            serde_json::Value::Object(fields) => fields
                .iter()
                .map(|(key, value)| match value {
                    serde_json::Value::Array(items) if key == "proxies" => items.len(),
                    other => proxies(other),
                })
                .sum(),
            serde_json::Value::Array(items) => items.iter().map(proxies).sum(),
            _ => 0,
        }
    }
    assert_eq!(proxies(&diagram), 0, "diagram.json has no proxy");
}

/// Criterion 10, as the saved board is fitted (owner, #335): a chip of a
/// variable's far reader says `← name`, else the number of the variable's
/// badge (`← 7`), else the arrow alone. At 1600 × 1000 every one names
/// its variable or its number; at 960 × 540 the arrow is allowed too, and
/// no chip covers a card (LAY-23 criterion 6).
#[test]
fn lay25_criterion_10_a_variable_chip_names_its_variable() {
    for size in [WIDE, crate::testing::MINIMUM] {
        let (_project, mut harness, id) = tidied("far-value", size);
        fit(&mut harness);
        right_click_board(&mut harness, &id);
        harness.get_by_label_contains(SAVE_SHARED).click();
        settle(&mut harness);
        harness.step();
        app(&mut harness).selections.insert(id.clone(), Vec::new());
        settle(&mut harness);
        fit(&mut harness);
        let graph = main_graph(&harness, &id);
        let canvas = board(&harness, &id);
        let saved: std::collections::BTreeMap<String, (String, String)> = graph
            .nodes
            .iter()
            .filter(|node| node.is_variable())
            .map(|node| {
                let badge = canvas
                    .debug_badge(&node.id)
                    .unwrap_or_else(|| panic!("{size:?}: {} has a badge", node.handle));
                (node.id.clone(), (node.handle.clone(), badge.text))
            })
            .collect();
        let cards: Vec<Rect> = graph
            .nodes
            .iter()
            .filter_map(|node| canvas.debug_world(&node.id))
            .collect();
        let zoom = canvas.debug_zoom();
        let mut named: BTreeSet<String> = BTreeSet::new();
        for tag in canvas.debug_tags() {
            let Some((name, number)) = saved.get(&tag.tag.source) else {
                continue;
            };
            let mut allowed = vec![format!("← {name}"), format!("← {number}")];
            if size != WIDE {
                allowed.push("←".to_owned());
            }
            assert!(
                allowed.contains(&tag.shown),
                "{size:?} at {zoom}: the chip at {} says {:?}, not one of {allowed:?}",
                tag.tag.reader,
                tag.shown
            );
            for card in &cards {
                assert!(
                    !tag.world.intersects(*card),
                    "{size:?} at {zoom}: {} covers {card:?}",
                    tag.tag.statement
                );
            }
            named.insert(name.clone());
        }
        for name in ["cartId", "productId"] {
            assert!(named.contains(name), "{size:?}: chips of {name}: {named:?}");
        }
    }
}

/// Criterion 11: *Read directly again* on `cartId` makes its readers read
/// `cart.id` and removes the variable, in one undoable edit; it is
/// absent on a variable that holds anything but a plain reference.
#[test]
fn lay25_criterion_11_read_directly_again() {
    let source = fixture("far-value-saved").replace(
        "  var productId = cart.productId\n",
        "  var productId = cart.productId\n  var double = cart.productId * 2\n",
    );
    let (_project, mut harness, id) = tidied_source(&source, WIDE);
    fit(&mut harness);
    let before = text(&harness, &id);
    let cart_id = id_of(&harness, &id, "cartId");
    let readers_of_variable: Vec<String> = nodes(&harness, &id)
        .iter()
        .filter(|node| {
            node.inputs
                .values()
                .any(|binding| binding.references_node(&cart_id))
        })
        .map(|node| node.handle.clone())
        .collect();
    assert_eq!(readers_of_variable.len(), 9);

    let double = id_of(&harness, &id, "double");
    app(&mut harness)
        .selections
        .insert(id.clone(), vec![double]);
    settle(&mut harness);
    harness.step();
    assert!(!has(&harness, READ_DIRECTLY), "absent on a calculation");

    app(&mut harness)
        .selections
        .insert(id.clone(), vec![cart_id]);
    settle(&mut harness);
    harness.step();
    harness.get_by_label(READ_DIRECTLY).click();
    settle(&mut harness);
    harness.step();
    assert!(
        !variables(&harness, &id).contains("cartId"),
        "the variable is gone"
    );
    let mut direct = readers(&harness, &id, "cart", "cart", "id");
    direct.sort();
    let mut expected = readers_of_variable.clone();
    expected.sort();
    assert_eq!(direct, expected, "its readers read cart.id");
    assert!(!text(&harness, &id).contains("cartId}}"));
    undo(&mut harness);
    assert_eq!(text(&harness, &id), before, "one undo");
}

/// Criterion 12: after a simulation each new variable has its row in Run
/// results and every badge equals its row (LAY-07).
#[test]
fn lay25_criterion_12_the_variable_has_its_row() {
    let (_project, mut harness, id) = tidied("far-value", WIDE);
    fit(&mut harness);
    right_click_board(&mut harness, &id);
    harness.get_by_label(&format!("{SAVE_SHARED} (2)")).click();
    settle(&mut harness);
    harness.step();
    app(&mut harness).runs.runner = Some(
        apiforge_runner::RunnerService::new(std::sync::Arc::new(|| {})).expect("the runner starts"),
    );
    choose(&mut harness, "Run simulation");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while folder(&harness).runs.handle.is_some() {
        assert!(std::time::Instant::now() < deadline, "the run finishes");
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    settle(&mut harness);
    harness.step();
    let run = folder(&harness).fresh_run(&id).expect("a fresh run");
    let rows: Vec<String> = run
        .steps
        .iter()
        .filter(|step| step.phase == apiforge_core::run::Phase::Main)
        .map(|step| step.node_id.clone())
        .collect();
    let graph = main_graph(&harness, &id);
    assert_eq!(rows.len(), graph.nodes.len(), "every step has a row");
    assert_eq!(graph.nodes.len(), 25, "23 steps and two variables");
    for variable in ["cartId", "productId"] {
        assert!(
            rows.contains(&id_of(&harness, &id, variable)),
            "{variable} has its row"
        );
    }
    fit(&mut harness);
    // A check folded into its step has no card and no badge (LAY-11).
    for (index, step) in rows.iter().enumerate() {
        if board(&harness, &id).debug_world(step).is_none() {
            continue;
        }
        let badge = board(&harness, &id)
            .debug_badge(step)
            .unwrap_or_else(|| panic!("{step} has a badge"));
        assert_eq!(badge.text, (index + 1).to_string(), "{step}: badge = row");
    }
    for variable in ["cartId", "productId"] {
        assert!(
            board(&harness, &id)
                .debug_badge(&id_of(&harness, &id, variable))
                .is_some(),
            "{variable} has a badge"
        );
    }
}

/// Criterion 13: the language reference says when to save a value.
#[test]
fn lay25_criterion_13_the_reference_says_when_to_save_a_value() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/language/reference.md");
    let reference = fs::read_to_string(path).unwrap();
    let start = reference
        .find("## Values and var")
        .expect("a section Values and var");
    let section = &reference[start..];
    let section = &section[..section[4..]
        .find("\n#")
        .map_or(section.len(), |end| end + 4)];
    assert!(section.contains("var "), "{section}");
    assert!(section.contains("three"), "when to save: {section}");
}

/// Criterion 13: the review views, the choice, the port menu, the
/// right-click menu and the saved board at both sizes, each asserting a
/// card in view.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay25() {
    for size in [WIDE, crate::testing::MINIMUM] {
        let suffix = if size == WIDE { "1600" } else { "960" };
        // The choice at the drop.
        let (_project, mut harness, id) = tidied("far-value", size);
        fit(&mut harness);
        let value = cart_id(&harness, &id);
        drop_field(&mut harness, &id, value, "id", "remove_coupon", "body");
        assert!(board(&harness, &id).debug_offer().is_some(), "the choice");
        card_in_view(&harness, &id, "choice");
        crate::testing::capture(&mut harness, &format!("lay25-choice-{suffix}"), None);

        // The port's menu.
        let (_project, mut harness, id) = tidied("far-value", size);
        fit(&mut harness);
        show(&mut harness, &id, "cart", 0.6);
        harness
            .get_by_label("Save a value of cart as a variable")
            .click();
        settle(&mut harness);
        harness.step();
        card_in_view(&harness, &id, "port");
        crate::testing::capture(&mut harness, &format!("lay25-port-{suffix}"), None);

        // The right-click menu, then the saved board.
        let (_project, mut harness, id) = tidied("far-value", size);
        fit(&mut harness);
        right_click_board(&mut harness, &id);
        card_in_view(&harness, &id, "menu");
        crate::testing::capture(&mut harness, &format!("lay25-menu-{suffix}"), None);
        harness.get_by_label_contains(SAVE_SHARED).click();
        settle(&mut harness);
        harness.step();
        app(&mut harness).selections.insert(id.clone(), Vec::new());
        settle(&mut harness);
        fit(&mut harness);
        card_in_view(&harness, &id, "saved");
        crate::testing::capture(&mut harness, &format!("lay25-saved-{suffix}"), None);
    }
}

fn card_in_view(harness: &FolderHarness, id: &str, label: &str) {
    let canvas = board(harness, id);
    let focus = canvas.debug_focus();
    assert!(
        canvas
            .debug_cards()
            .iter()
            .any(|(_, rect)| focus.contains_rect(*rect)),
        "{label}: a card is in view"
    );
}
