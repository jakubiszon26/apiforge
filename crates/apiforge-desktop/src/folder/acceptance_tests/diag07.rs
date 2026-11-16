//! DIAG-07 (#392): a step added from a step's *Next* port is written directly
//! after that step, its checks and values, and runs directly after it. On
//! `shop-30`, through the board: a wire from `cart`'s *Next* dropped on the
//! empty board, *HTTP request* chosen in the menu it opens.
use super::lay16::{centre, move_to, press, release};
use super::*;
use apiforge_core::{format::Graph, model::NodeKind};
use egui::{Pos2, Rect, pos2};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// `shop-30` on the board of the demo shop, in the form the window writes
/// (its own text writes `refund` after the whole `if`, which the window
/// writes inside the branch, and leaves out `on_error`), tidied up and saved.
fn opened(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), fixture("shop-30")).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    let session = folder(&harness).session.as_ref().unwrap();
    let model = session.document(&id).unwrap().model().unwrap().clone();
    let written =
        apiforge_script::print_folder_document(session.context(), &model).expect("shop-30 prints");
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &written, CODE)
        .unwrap();
    settle(&mut harness);
    assert_eq!(source_of(&harness, &id), written);
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, "Tidy up");
    choose(&mut harness, "Save");
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

fn graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

fn id_of(graph: &Graph, handle: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone()
}

fn handle_of<'a>(graph: &'a Graph, id: &str) -> &'a str {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| panic!("a step {id}"), |node| node.handle.as_str())
}

fn board<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

/// A point of the empty board in view: no card, chip or count within 40 px.
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

/// The line on which `prefix` starts a statement.
fn line_of(text: &str, prefix: &str) -> usize {
    text.lines()
        .position(|line| line.trim_start().starts_with(prefix))
        .unwrap_or_else(|| panic!("`{prefix}` is written:\n{text}"))
}

/// Criteria 1, 2 and 5 through the board: the new request is written with
/// `@after(cart)` after `cart`'s checks and before `cart_view`, the simulated
/// run executes it next after `cart`, and undo restores the file byte for
/// byte.
#[test]
fn diag07_a_request_added_from_carts_next_runs_after_cart() {
    let (project, mut harness, id) = opened(WIDE);
    let path = project.path().join(SCENARIO);
    let saved = fs::read(&path).unwrap();
    let before = source_of(&harness, &id);
    let ids: Vec<String> = graph(&harness, &id)
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();

    let cart = id_of(&graph(&harness, &id), "cart");
    app(&mut harness)
        .canvases
        .get_mut(&id)
        .expect("the board is drawn")
        .debug_show(&cart, 1.0);
    settle(&mut harness);
    harness.step();
    let from = centre(&harness, "Output __order of cart");
    let to = empty_point(&harness, &id);
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    release(&mut harness, to);
    harness.step();
    assert!(board(&harness, &id).debug_menu_open(), "the menu opens");
    // The menu's entry, not the block library's: the one by the drop.
    harness
        .get_all_by_label("HTTP request")
        .min_by(|a, b| {
            let distance = |rect: Rect| rect.center().distance(to);
            distance(a.rect()).total_cmp(&distance(b.rect()))
        })
        .expect("the menu offers a request")
        .click();
    settle(&mut harness);
    harness.step();

    // Criterion 2: written after cart and its checks, before the next step.
    let after = graph(&harness, &id);
    assert_eq!(after.nodes.len(), ids.len() + 1, "one step is added");
    let added = after
        .nodes
        .iter()
        .find(|node| !ids.contains(&node.id))
        .expect("the new step");
    assert_eq!(added.kind, NodeKind::Request);
    let handle = added.handle.clone();
    let text = source_of(&harness, &id);
    let at = line_of(&text, &format!("{handle} = "));
    assert_eq!(
        text.lines().nth(at - 1).map(str::trim),
        Some("@after(cart)"),
        "{text}"
    );
    assert!(line_of(&text, "cart_total = ") < at - 1, "{text}");
    assert!(at < line_of(&text, "cart_view = "), "{text}");

    // Criterion 1: the simulated run executes it next after cart.
    choose(&mut harness, "Run simulation");
    let report = finish_run(&mut harness);
    let order: Vec<&str> = report
        .steps
        .iter()
        .filter(|step| step.phase == apiforge_core::run::Phase::Main)
        .map(|step| handle_of(&after, &step.node_id))
        .collect();
    let position = |handle: &str| {
        order
            .iter()
            .position(|step| *step == handle)
            .unwrap_or_else(|| panic!("{handle} runs: {order:?}"))
    };
    let new = position(&handle);
    assert!(position("cart") < new, "{order:?}");
    assert!(
        order[position("cart") + 1..new].iter().all(|step| matches!(
            after
                .nodes
                .iter()
                .find(|node| node.handle == *step)
                .map(|node| node.kind),
            Some(NodeKind::Assert | NodeKind::Condition | NodeKind::Value | NodeKind::Export)
        )),
        "only cart's checks and values run between: {order:?}"
    );
    assert!(new < position("cart_view"), "{order:?}");
    assert!(new < position("order"), "{order:?}");

    // Criterion 5: undo restores the file byte for byte.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut harness);
    harness.step();
    assert_eq!(source_of(&harness, &id), before, "the text is as it was");
    choose(&mut harness, "Save");
    settle(&mut harness);
    assert_eq!(fs::read(&path).unwrap(), saved, "the file is as it was");
}
