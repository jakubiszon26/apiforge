//! DIAG-06a (#391) at the window: an ordering wire from a step's *Next*
//! dropped on a step of the same braces moves that step to directly after
//! it, with the checks that read only it; a move with a cycle is refused in
//! the board's banner; *Add step* on the bar puts the new step after the
//! selection; *Delete* on an ordering line is refused; and a wire dropped on
//! an `if` with statements, or on a step of other braces, is refused until
//! DIAG-06d. On `shop-30` in the demo shop and on the room project, through
//! the gestures a person makes: a wire carried from a socket found by its
//! name to the step's order socket.
use super::diag07::{board, empty_point};
use super::lay14::point_on;
use super::lay16::{centre, move_to, press, release};
use super::*;
use crate::canvas::Command;
use apiforge_core::{block::Block, format::Graph, format::Position, graph::sequence};
use egui::{Pos2, Rect};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const NOT_YET: &str =
    "Moving a condition with its steps, or a step into another branch, is not possible yet.";
const ORDER_LINE: &str = "The line is the order of the steps. Move or delete a step.";

fn lay(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(name);
    fs::read_to_string(path).unwrap()
}

/// The window on `project`, `path` open on the board and tidied up.
fn open_in(project: &std::path::Path, path: &str) -> (FolderHarness, String) {
    let _ = fs::remove_file(project.join(DIAGRAM));
    let mut harness = crate::testing::folder_harness_in(project, WIDE, Mode::Diagram);
    app(&mut harness).review_open(path, None);
    settle(&mut harness);
    let id = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("the project has {path}"))
        .id()
        .to_owned();
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, "Tidy up");
    settle(&mut harness);
    harness.step();
    (harness, id)
}

/// `text` as a scenario of the demo shop, open on the board.
fn shop_with(text: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), text).unwrap();
    let (harness, id) = open_in(project.path(), SCENARIO);
    assert_eq!(source_of(&harness, &id), text, "opened as written");
    (project, harness, id)
}

fn shop_30() -> (tempfile::TempDir, FolderHarness, String) {
    shop_with(&lay("shop-30.afs"))
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The room project, `path` open on the board, after `prepare`.
fn rooms(
    path: &str,
    prepare: impl FnOnce(&std::path::Path),
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = tempfile::tempdir().unwrap();
    copy_tree(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/rooms"),
        project.path(),
    );
    prepare(project.path());
    let (harness, id) = open_in(project.path(), path);
    (project, harness, id)
}

/// The room project's component with three steps that read nothing of one
/// another.
const SIGN_IN: &str = "component sign_in \"Sign in\" {
  description \"Open a session on the room booking API and return its token.\"
  @label(\"User\")
  input user: string = \"tester\"
  session = POST \"/sessions\" {
    body { user: param.user, password: \"secret\" }
    sample 201 { token: \"tok-1\" }
  }
  health = GET \"/health\" {
    sample 200 { ok: true }
  }
  audit = GET \"/audit\" {
    sample 200 { ok: true }
  }
  export token: string = session.body.token
}
";

fn component() -> (tempfile::TempDir, FolderHarness, String) {
    rooms("components/sign-in.afs", |root| {
        fs::write(root.join("components/sign-in.afs"), SIGN_IN).unwrap();
    })
}

/// The graph on the board: the cleanup's while the board shows it.
fn graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(
        snapshot.model.as_ref().unwrap(),
        folder(harness).cleanup(id),
    )
    .clone()
}

fn id_of(graph: &Graph, handle: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a statement `{handle}`"))
        .id
        .clone()
}

/// The steps of the board in the order they run.
fn steps(harness: &FolderHarness, id: &str) -> Vec<String> {
    let board = graph(harness, id);
    sequence(&board)
        .into_iter()
        .filter_map(|step| board.nodes.iter().find(|node| node.id == step))
        .filter(|node| !node.kind.is_data())
        .map(|node| node.handle.clone())
        .collect()
}

/// Every statement of the board in written order.
fn written(harness: &FolderHarness, id: &str) -> Vec<String> {
    graph(harness, id)
        .nodes
        .iter()
        .map(|node| node.handle.clone())
        .collect()
}

fn at(order: &[String], handle: &str) -> usize {
    order
        .iter()
        .position(|step| step == handle)
        .unwrap_or_else(|| panic!("`{handle}` in {order:?}"))
}

/// The source passes the check and says the board's model.
fn passes(harness: &FolderHarness, id: &str) -> String {
    let text = source_of(harness, id);
    assert!(!text.contains("@after"), "{text}");
    let session = folder(harness).session.as_ref().unwrap();
    let document = session.document(id).unwrap();
    assert!(
        document
            .diagnostics()
            .iter()
            .all(|d| d.severity != apiforge_script::Severity::Error),
        "{text}\n{:#?}",
        document.diagnostics()
    );
    assert!(document.model_describes_text(), "{text}");
    text
}

fn screen(harness: &FolderHarness, id: &str, world: Pos2) -> Pos2 {
    board(harness, id)
        .debug_to_screen(Rect::from_min_max(world, world))
        .min
}

/// Brings the cards `handles` into view together: centred on the first, at
/// the largest zoom that shows them all inside the board.
fn show(harness: &mut FolderHarness, id: &str, handles: &[&str]) {
    let board_graph = graph(harness, id);
    let ids: Vec<String> = handles
        .iter()
        .map(|handle| id_of(&board_graph, handle))
        .collect();
    for zoom in [1.0, 0.8, 0.6, 0.45, 0.35, 0.25] {
        app(harness)
            .canvases
            .get_mut(id)
            .expect("the board is drawn")
            .debug_show(&ids[0], zoom);
        settle(harness);
        harness.step();
        let canvas = board(harness, id);
        let inside = canvas.debug_board().shrink(8.0);
        if ids.iter().all(|card| {
            canvas
                .debug_card_rect(card)
                .is_some_and(|rect| inside.contains_rect(rect))
        }) {
            return;
        }
    }
    panic!("{handles:?} fit on the board together");
}

/// Carries a wire from the socket `port` of `source` (`__order` for *Next*,
/// `__true` and `__false` for *Yes* and *No*) to the order socket of
/// `target`, and drops it there.
fn wire(harness: &mut FolderHarness, id: &str, source: &str, port: &str, target: &str) {
    show(harness, id, &[source, target]);
    let target_id = id_of(&graph(harness, id), target);
    let from = centre(harness, &format!("Output {port} of {source}"));
    let (socket, _) = board(harness, id).debug_order_sockets(&target_id);
    let to = screen(harness, id, socket.expect("the target has an order socket"));
    press(harness, from);
    move_to(harness, from, to);
    release(harness, to);
    harness.step();
}

/// The wire is refused with `message` in the board's banner, and nothing
/// changes: not the source, not the model.
fn refused(
    harness: &mut FolderHarness,
    id: &str,
    (source, port, target): (&str, &str, &str),
    message: &str,
) {
    let before = source_of(harness, id);
    let model = graph(harness, id);
    wire(harness, id, source, port, target);
    assert!(
        harness.query_by_label(message).is_some(),
        "the banner says: {message}"
    );
    assert_eq!(source_of(harness, id), before, "the source is unchanged");
    assert_eq!(graph(harness, id), model, "the model is unchanged");
}

/// One Undo gives the file back byte for byte.
fn undone(harness: &mut FolderHarness, id: &str, before: &str) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(harness);
    harness.step();
    assert_eq!(source_of(harness, id), before, "Undo gives the file back");
}

/// A move done by a wire: the source changed, passes the check, and one
/// Undo gives it back.
fn moved(
    harness: &mut FolderHarness,
    id: &str,
    (source, port, target): (&str, &str, &str),
) -> Vec<String> {
    let before = source_of(harness, id);
    wire(harness, id, source, port, target);
    assert!(
        harness.query_by_label(NOT_YET).is_none(),
        "the move is not refused"
    );
    let text = passes(harness, id);
    assert_ne!(text, before, "the move changes the source");
    let order = steps(harness, id);
    undone(harness, id, &before);
    // Redo, so that the caller sees the move.
    harness.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    settle(harness);
    harness.step();
    assert_eq!(source_of(harness, id), text, "Redo gives the move back");
    order
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn diag06a_criterion_1_forwards_in_the_main_flow() {
    let (_project, mut harness, id) = shop_30();
    let order = moved(&mut harness, &id, ("cart", "__order", "address"));
    assert_eq!(at(&order, "address"), at(&order, "cart") + 1, "{order:?}");
    assert_eq!(
        at(&order, "cart_view"),
        at(&order, "address") + 1,
        "{order:?}"
    );
    assert_eq!(
        at(&order, "checkout"),
        at(&order, "cart_view") + 1,
        "{order:?}"
    );
}

#[test]
fn diag06a_criterion_1_backwards_in_the_main_flow() {
    let (_project, mut harness, id) = shop_30();
    let order = moved(&mut harness, &id, ("address", "__order", "cart_view"));
    assert_eq!(at(&order, "address"), at(&order, "cart") + 1, "{order:?}");
    assert_eq!(
        at(&order, "cart_view"),
        at(&order, "address") + 1,
        "{order:?}"
    );
    assert_eq!(
        at(&order, "checkout"),
        at(&order, "cart_view") + 1,
        "{order:?}"
    );
}

#[test]
fn diag06a_criterion_1_in_a_branch() {
    let (_project, mut harness, id) = shop_30();
    let order = moved(&mut harness, &id, ("payment", "__order", "refund"));
    assert_eq!(
        order[at(&order, "paid")..],
        ["paid", "payment", "refund", "receipt", "logout"]
    );
    let order = moved(&mut harness, &id, ("logout", "__order", "refund"));
    assert_eq!(
        order[at(&order, "paid")..],
        ["paid", "payment", "receipt", "logout", "refund"]
    );
    // First in the braces, from *Yes*.
    let order = moved(&mut harness, &id, ("paid", "__true", "logout"));
    assert_eq!(
        order[at(&order, "paid")..],
        ["paid", "logout", "payment", "receipt", "refund"]
    );
}

#[test]
fn diag06a_criterion_1_in_the_cleanup() {
    let (_project, mut harness, id) = rooms("scenarios/booking-day.afs", |_| {});
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    harness.step();
    let order = moved(&mut harness, &id, ("cancelled", "__order", "released"));
    assert_eq!(order, ["cancelled", "released"]);
    assert_eq!(written(&harness, &id), ["cancelled", "gone", "released"]);
}

#[test]
fn diag06a_criterion_1_in_a_component() {
    let (_project, mut harness, id) = component();
    let order = moved(&mut harness, &id, ("session", "__order", "audit"));
    assert_eq!(order, ["session", "audit", "health"]);
    let order = moved(&mut harness, &id, ("health", "__order", "session"));
    assert_eq!(order, ["audit", "health", "session"]);
}

// Criterion 3 ----------------------------------------------------------------

#[test]
fn diag06a_criterion_3() {
    let (_project, mut harness, id) = shop_30();
    moved(&mut harness, &id, ("catalogue", "__order", "cart"));
    let order = written(&harness, &id);
    // `cart_created` reads only `cart` and goes with it; `cart_total` also
    // reads `expected_total` and stays where it is written.
    assert_eq!(at(&order, "cart"), at(&order, "second") + 1, "{order:?}");
    assert_eq!(
        at(&order, "cart_created"),
        at(&order, "cart") + 1,
        "{order:?}"
    );
    assert_eq!(
        at(&order, "product"),
        at(&order, "cart_created") + 1,
        "{order:?}"
    );
    assert_eq!(
        at(&order, "cart_total"),
        at(&order, "expected_total") + 1,
        "{order:?}"
    );
    let text = source_of(&harness, &id);
    let line = |prefix: &str| super::diag07::line_of(&text, prefix);
    assert!(line("second = ") < line("cart = ") && line("cart = ") < line("product = "));
    assert!(line("cart_created = ") < line("product = "), "{text}");
    assert!(line("expected_total = ") < line("cart_total = "), "{text}");
}

// Criterion 4 ----------------------------------------------------------------

const PAY: &str = r#"scenario "Pay" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1", total: 10 }
  }
  cart_id = cart.body.id
  pay = POST "/pay/{{cart.body.id}}" {
    sample 200 { ok: true }
  }
  done = GET "/done" {
    sample 200 {}
  }
  receipt = GET "/receipts/{{cart_id}}" {
    sample 200 {}
  }
}
"#;

#[test]
fn diag06a_criterion_4_the_step_above_what_it_reads() {
    let (_project, mut harness, id) = shop_with(PAY);
    refused(
        &mut harness,
        &id,
        ("begin", "__order", "pay"),
        "`pay` reads `cart.body.id`; it stays below `cart`.",
    );
}

#[test]
fn diag06a_criterion_4_a_step_below_a_step_that_reads_it() {
    let (_project, mut harness, id) = shop_with(PAY);
    refused(
        &mut harness,
        &id,
        ("done", "__order", "cart"),
        "`pay` reads `cart.body.id`; it stays below `cart`.",
    );
}

#[test]
fn diag06a_criterion_4_through_a_value() {
    let (_project, mut harness, id) = shop_with(PAY);
    refused(
        &mut harness,
        &id,
        ("begin", "__order", "receipt"),
        "`receipt` reads `cart_id`; it stays below `cart`.",
    );
}

// Criterion 5 ----------------------------------------------------------------

/// Selects `handles` and adds an HTTP request with *Add step* on the bar;
/// returns the new step's handle.
fn add_step(harness: &mut FolderHarness, id: &str, handles: &[&str]) -> String {
    let before = graph(harness, id);
    let ids = handles
        .iter()
        .map(|handle| id_of(&before, handle))
        .collect();
    app(harness).selections.insert(id.to_owned(), ids);
    settle(harness);
    harness.step();
    let button = centre(harness, "Add step");
    harness.get_by_label("Add step").click();
    settle(harness);
    harness.step();
    assert!(board(harness, id).debug_menu_open(), "the menu opens");
    // The menu's entry, not the block library's: the one by the button.
    harness
        .get_all_by_label("HTTP request")
        .min_by(|a, b| {
            let distance = |rect: Rect| rect.center().distance(button);
            distance(a.rect()).total_cmp(&distance(b.rect()))
        })
        .expect("the menu offers a request")
        .click();
    settle(harness);
    harness.step();
    let after = graph(harness, id);
    assert_eq!(
        after.nodes.len(),
        before.nodes.len() + 1,
        "one step is added"
    );
    let handle = after
        .nodes
        .iter()
        .find(|node| !before.nodes.iter().any(|old| old.id == node.id))
        .expect("the new step")
        .handle
        .clone();
    passes(harness, id);
    handle
}

#[test]
fn diag06a_criterion_5_a_step_selected() {
    let (_project, mut harness, id) = shop_30();
    let new = add_step(&mut harness, &id, &["cart"]);
    let order = steps(&harness, &id);
    assert_eq!(at(&order, &new), at(&order, "cart") + 1, "{order:?}");
    let text = source_of(&harness, &id);
    let line = |prefix: &str| super::diag07::line_of(&text, prefix);
    assert!(line("cart_total = ") < line(&format!("{new} = ")), "{text}");
    assert!(line(&format!("{new} = ")) < line("cart_view = "), "{text}");
}

#[test]
fn diag06a_criterion_5_an_if_selected() {
    let (_project, mut harness, id) = shop_30();
    let new = add_step(&mut harness, &id, &["paid"]);
    let order = steps(&harness, &id);
    assert_eq!(order.last(), Some(&new), "after the whole if: {order:?}");
    let text = source_of(&harness, &id);
    let written_at = super::diag07::line_of(&text, &format!("{new} = "));
    assert!(
        super::diag07::line_of(&text, "not_created = ") < written_at,
        "{text}"
    );
    assert!(
        text.lines()
            .nth(written_at)
            .unwrap()
            .starts_with(&format!("  {new} = ")),
        "in the main flow, not in the braces: {text}"
    );
}

#[test]
fn diag06a_criterion_5_a_step_in_braces_selected() {
    let (_project, mut harness, id) = shop_30();
    let new = add_step(&mut harness, &id, &["receipt"]);
    let order = steps(&harness, &id);
    assert_eq!(at(&order, &new), at(&order, "receipt") + 1, "{order:?}");
    let text = source_of(&harness, &id);
    assert!(
        text.contains(&format!("\n    {new} = ")),
        "in its braces: {text}"
    );
}

#[test]
fn diag06a_criterion_5_a_check_selected() {
    let (_project, mut harness, id) = shop_30();
    let new = add_step(&mut harness, &id, &["checkout_total"]);
    let order = steps(&harness, &id);
    assert_eq!(at(&order, &new), at(&order, "checkout") + 1, "{order:?}");
}

#[test]
fn diag06a_criterion_5_several_selected() {
    let (_project, mut harness, id) = shop_30();
    let new = add_step(&mut harness, &id, &["checkout", "login", "cart"]);
    let order = steps(&harness, &id);
    assert_eq!(at(&order, &new), at(&order, "checkout") + 1, "{order:?}");
}

#[test]
fn diag06a_criterion_5_nothing_selected() {
    let (_project, mut harness, id) = shop_30();
    let new = add_step(&mut harness, &id, &[]);
    let order = steps(&harness, &id);
    assert_eq!(order.last(), Some(&new), "{order:?}");
}

// Criterion 6 ----------------------------------------------------------------

/// A request added from the output `port` of `source`, as the menu of a
/// wire dropped on the empty board adds it.
fn add_from(harness: &mut FolderHarness, id: &str, source: &str, port: &str) -> String {
    let before = graph(harness, id);
    app(harness).active_board_command(Command::AddNode {
        block: Block::Request,
        component: None,
        position: Position { x: 40.0, y: 40.0 },
        source: Some((id_of(&before, source), port.to_owned())),
    });
    settle(harness);
    harness.step();
    let after = graph(harness, id);
    passes(harness, id);
    after
        .nodes
        .iter()
        .find(|node| !before.nodes.iter().any(|old| old.id == node.id))
        .expect("the new step")
        .handle
        .clone()
}

#[test]
fn diag06a_criterion_6_next_with_a_successor() {
    let (_project, mut harness, id) = shop_30();
    let new = add_from(&mut harness, &id, "cart", "__order");
    let order = steps(&harness, &id);
    assert_eq!(at(&order, &new), at(&order, "cart") + 1, "{order:?}");
    assert_eq!(at(&order, "cart_view"), at(&order, &new) + 1, "{order:?}");
}

#[test]
fn diag06a_criterion_6_from_yes_and_no() {
    let (_project, mut harness, id) = shop_30();
    let yes = add_from(&mut harness, &id, "paid", "__true");
    let no = add_from(&mut harness, &id, "paid", "__false");
    let text = source_of(&harness, &id);
    let line = |prefix: &str| super::diag07::line_of(&text, prefix);
    assert!(line("logout = ") < line(&format!("{yes} = ")), "{text}");
    assert!(line(&format!("{yes} = ")) < line("} else {"), "{text}");
    assert!(line("not_created = ") < line(&format!("{no} = ")), "{text}");
}

#[test]
fn diag06a_criterion_6_in_the_cleanup() {
    let (_project, mut harness, id) = rooms("scenarios/booking-day.afs", |_| {});
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    harness.step();
    let new = add_from(&mut harness, &id, "released", "__order");
    assert_eq!(
        steps(&harness, &id),
        ["released", new.as_str(), "cancelled"]
    );
}

#[test]
fn diag06a_criterion_6_in_a_component() {
    let (_project, mut harness, id) = component();
    let new = add_from(&mut harness, &id, "session", "__order");
    assert_eq!(
        steps(&harness, &id),
        ["session", new.as_str(), "health", "audit"]
    );
}

// Criterion 7 ----------------------------------------------------------------

/// The line of the chain from `source` to `target`, as the last frame drew it.
fn chain_line(harness: &FolderHarness, id: &str, source: &str, target: &str) -> String {
    let board_graph = graph(harness, id);
    let (source, target) = (id_of(&board_graph, source), id_of(&board_graph, target));
    board(harness, id)
        .debug_wires()
        .iter()
        .find(|wire| {
            wire.chain && wire.source.as_deref() == Some(source.as_str()) && wire.target == target
        })
        .unwrap_or_else(|| panic!("the line is drawn"))
        .id
        .clone()
}

fn click(harness: &mut FolderHarness, at: Pos2) {
    press(harness, at);
    release(harness, at);
    harness.step();
}

#[test]
fn diag06a_criterion_7() {
    let (_project, mut harness, id) = shop_30();
    show(&mut harness, &id, &["cart", "cart_view"]);
    let line = chain_line(&harness, &id, "cart", "cart_view");
    let at = point_on(&harness, &id, &line);
    click(&mut harness, at);
    assert_eq!(
        board(&harness, &id).debug_selected_wires(),
        std::slice::from_ref(&line),
        "the click selects the line"
    );
    let before = source_of(&harness, &id);
    let model = graph(&harness, &id);
    harness.key_press(egui::Key::Delete);
    settle(&mut harness);
    harness.step();
    assert!(
        harness.query_by_label(ORDER_LINE).is_some(),
        "the banner says: {ORDER_LINE}"
    );
    assert_eq!(source_of(&harness, &id), before, "the source is unchanged");
    assert_eq!(graph(&harness, &id), model, "the model is unchanged");
    // The line is still there and can be selected again.
    let line = chain_line(&harness, &id, "cart", "cart_view");
    let at = point_on(&harness, &id, &line);
    click(&mut harness, at);
    assert_eq!(
        board(&harness, &id).debug_selected_wires(),
        std::slice::from_ref(&line),
        "the line stays selectable"
    );
    let _ = empty_point(&harness, &id);
}

// Criterion 8 ----------------------------------------------------------------

#[test]
fn diag06a_criterion_8_an_if_with_statements_in_its_braces() {
    let (_project, mut harness, id) = shop_30();
    refused(&mut harness, &id, ("login", "__order", "paid"), NOT_YET);
}

#[test]
fn diag06a_criterion_8_a_step_of_other_braces() {
    let (_project, mut harness, id) = shop_30();
    refused(&mut harness, &id, ("cart", "__order", "payment"), NOT_YET);
    refused(
        &mut harness,
        &id,
        ("payment", "__order", "address"),
        NOT_YET,
    );
    refused(&mut harness, &id, ("paid", "__false", "address"), NOT_YET);
}
