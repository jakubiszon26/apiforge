//! #284: the Inspector of a Cleanup block resolves and offers the earlier
//! Cleanup steps as sources, by the rule the Cleanup board's cards follow
//! since UX-16; the Main board's Inspector is unchanged.
use super::ux31::window;
use super::*;
use crate::testing::MINIMUM;
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
/// The combo box choosing the step *Actual* reads.
const STEP: &str = "Actual value step";
/// The combo box choosing the output of that step.
const OUTPUT: &str = "Actual value output";

/// A Cleanup whose second check reads the first, so the first must not
/// offer it.
const DEPENDANT: &str = "scenario \"#284\" {\n  @label(\"Product\")\n  response = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  cleanup {\n    @label(\"Delete cart\")\n    gone = DELETE \"/shop/carts/1\" { sample 200 { deleted: true } }\n    @label(\"Cart was deleted\")\n    deleted = assert gone.status == 200\n    @label(\"Deletion confirmed\")\n    confirmed = assert deleted.passed == true\n  }\n}\n";

/// The demo's Cleanup reads a Main step from its check instead of the
/// Cleanup request, so choosing the request is a change.
const MAIN_SOURCE: &str = "scenario \"#284\" {\n  @label(\"Product\")\n  response = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  cleanup {\n    @label(\"Delete cart\")\n    gone = DELETE \"/shop/carts/1\" { sample 200 { deleted: true } }\n    @label(\"Cart was deleted\")\n    deleted = assert response.status == 200\n  }\n}\n";

/// The demo shop, its scenario replaced by `text` when one is given, open in
/// Diagram on the Cleanup board when `cleanup`.
fn open(text: Option<&str>, cleanup: bool) -> (tempfile::TempDir, FolderHarness, String) {
    let directory = shop();
    if let Some(text) = text {
        fs::write(directory.path().join(SCENARIO), text).unwrap();
    }
    let (mut harness, id) = window(directory.path(), MINIMUM, SCENARIO, "Diagram");
    if cleanup {
        harness.get_by_label("Cleanup").click();
        settle(&mut harness);
    }
    (directory, harness, id)
}

/// The node named `name` on the board on screen.
fn node(
    harness: &FolderHarness,
    id: &str,
    cleanup: bool,
    name: &str,
) -> apiforge_core::format::Node {
    graph_of(&model(folder(harness), id), cleanup)
        .nodes
        .iter()
        .find(|node| node.display_name() == name)
        .unwrap_or_else(|| panic!("{name} is on the board"))
        .clone()
}

/// Selects the step named `name`, which opens its Inspector.
fn select(harness: &mut FolderHarness, id: &str, cleanup: bool, name: &str) {
    let node = node(harness, id, cleanup, name).id;
    app(harness).selections.insert(id.to_owned(), vec![node]);
    settle(harness);
}

/// What the step combo box named `combo` shows.
fn shown(harness: &FolderHarness, combo: &str) -> String {
    harness
        .get_by_label(combo)
        .accesskit_node()
        .value()
        .unwrap_or_default()
}

/// The steps the combo box `combo` offers once open: the names whose count
/// grew when it opened, as the cards on the board carry the same names.
fn offered(harness: &mut FolderHarness, combo: &str, names: &[&str]) -> Vec<String> {
    let count = |harness: &FolderHarness, name: &str| harness.query_all_by_label(name).count();
    let before: Vec<usize> = names.iter().map(|name| count(harness, name)).collect();
    choose(harness, combo);
    names
        .iter()
        .zip(before)
        .filter(|(name, before)| count(harness, name) > *before)
        .map(|(name, _)| (*name).to_owned())
        .collect()
}

fn text(harness: &FolderHarness, id: &str) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .text()
        .to_owned()
}

/// Criterion 1: the Inspector of *Cart was deleted* names the Cleanup step
/// its *Actual* reads.
#[test]
fn a_cleanup_blocks_inspector_resolves_a_cleanup_source() {
    let (_directory, mut harness, id) = open(None, true);
    select(&mut harness, &id, true, "Cart was deleted");
    assert_eq!(shown(&harness, STEP), "Delete cart");
}

/// Criterion 2: the *Step* list offers the earlier Cleanup step and the Main
/// steps, and neither the block itself nor a step that depends on it.
#[test]
fn a_cleanup_blocks_inspector_offers_earlier_cleanup_steps() {
    let names = [
        "Delete cart",
        "Product",
        "Add to cart",
        "Expected price",
        "Cart was deleted",
    ];
    let (_directory, mut harness, id) = open(None, true);
    select(&mut harness, &id, true, "Cart was deleted");
    assert_eq!(
        offered(&mut harness, STEP, &names),
        ["Delete cart", "Product", "Add to cart", "Expected price"]
    );

    let names = [
        "Delete cart",
        "Product",
        "Cart was deleted",
        "Deletion confirmed",
    ];
    let (_directory, mut harness, id) = open(Some(DEPENDANT), true);
    select(&mut harness, &id, true, "Cart was deleted");
    assert_eq!(
        offered(&mut harness, STEP, &names),
        ["Delete cart", "Product"]
    );
}

/// Criterion 3: choosing *Delete cart → HTTP status* in the Inspector of
/// *Cart was deleted* writes the text that wiring the same output on the
/// board writes.
#[test]
fn choosing_a_cleanup_source_in_the_inspector_equals_the_wire() {
    let (_directory, mut chosen, id) = open(Some(MAIN_SOURCE), true);
    select(&mut chosen, &id, true, "Cart was deleted");
    assert_eq!(shown(&chosen, STEP), "Product");
    for (combo, item) in [(STEP, "Delete cart"), (OUTPUT, "HTTP status")] {
        choose(&mut chosen, combo);
        chosen
            .query_all_by_label(item)
            .last()
            .unwrap_or_else(|| panic!("the list offers {item}"))
            .click();
        settle(&mut chosen);
    }
    let by_inspector = text(&chosen, &id);
    assert!(
        by_inspector.contains("assert gone.status == 200"),
        "{by_inspector}"
    );

    let (_directory, mut wired, id) = open(Some(MAIN_SOURCE), true);
    let source = node(&wired, &id, true, "Delete cart").id;
    let target = node(&wired, &id, true, "Cart was deleted").id;
    // The board wires only an input no wire holds: the Main wire goes first.
    app(&mut wired).active_board_command(canvas::Command::SetLiteral {
        node: target.clone(),
        input: "actual".into(),
        path: Vec::new(),
        value: serde_json::json!(0),
    });
    settle(&mut wired);
    app(&mut wired).active_board_command(canvas::Command::Connect {
        source,
        source_port: "status".into(),
        target,
        target_input: "actual".into(),
        target_path: Vec::new(),
    });
    settle(&mut wired);
    assert_eq!(by_inspector, text(&wired, &id));
}

/// Criterion 4: on the Main board the list is what it was: Main steps only,
/// without the step itself or its dependants.
#[test]
fn the_main_boards_inspector_is_unchanged() {
    let names = [
        "Product",
        "Active promotion",
        "Add to cart",
        "Expected price",
        "Promotional price",
        "Delete cart",
    ];
    let (_directory, mut harness, id) = open(None, false);
    select(&mut harness, &id, false, "Promotional price");
    assert_eq!(shown(&harness, STEP), "Add to cart");
    assert_eq!(
        offered(&mut harness, STEP, &names),
        [
            "Product",
            "Active promotion",
            "Add to cart",
            "Expected price"
        ]
    );
}
