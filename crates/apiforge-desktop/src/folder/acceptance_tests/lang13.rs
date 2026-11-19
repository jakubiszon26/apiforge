//! LANG-13 (#386), criterion 9 at the window: on `shop-30`, a block from the
//! library runs last and a step from a card's *Next* is inserted into the
//! chain; either way the source holds no `@after` and passes the check.
use super::diag07::{board, empty_point, graph, id_of, opened};
use super::lay16::{centre, move_to, press, release};
use super::*;
use apiforge_core::{format::Graph, graph::sequence, model::NodeKind};
use egui::Rect;

/// The handles of the steps in the order the engine reaches them.
fn steps(graph: &Graph) -> Vec<String> {
    sequence(graph)
        .into_iter()
        .map(|id| graph.nodes.iter().find(|node| node.id == id).unwrap())
        .filter(|node| !node.kind.is_data())
        .map(|node| node.handle.clone())
        .collect()
}

/// The source holds no `@after`, has no error and says the board's model.
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

fn added(before: &Graph, after: &Graph) -> String {
    assert_eq!(
        after.nodes.len(),
        before.nodes.len() + 1,
        "one step is added"
    );
    let node = after
        .nodes
        .iter()
        .find(|node| !before.nodes.iter().any(|old| old.id == node.id))
        .expect("the new step");
    assert_eq!(node.kind, NodeKind::Request);
    node.handle.clone()
}

#[test]
fn lang13_criterion_9_a_block_from_the_library_runs_last() {
    let (_project, mut harness, id) = opened(WIDE);
    let before = graph(&harness, &id);
    harness.get_by_label("HTTP request").click();
    settle(&mut harness);
    harness.step();
    let after = graph(&harness, &id);
    let handle = added(&before, &after);
    let text = passes(&harness, &id);
    assert_eq!(steps(&after).last(), Some(&handle), "{text}");
}

#[test]
fn lang13_criterion_9_a_step_from_next_is_inserted_into_the_chain() {
    let (_project, mut harness, id) = opened(WIDE);
    let before = graph(&harness, &id);
    let cart = id_of(&before, "cart");
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
    let after = graph(&harness, &id);
    let handle = added(&before, &after);
    let text = passes(&harness, &id);
    let order = steps(&after);
    let position = |step: &str| {
        order
            .iter()
            .position(|s| s == step)
            .unwrap_or_else(|| panic!("{step} runs: {order:?}"))
    };
    assert_eq!(position(&handle), position("cart") + 1, "{order:?}\n{text}");
    assert_eq!(position("cart_view"), position(&handle) + 1, "{order:?}");
}
