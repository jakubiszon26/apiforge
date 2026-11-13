//! #322: *Save and publish* places the call only after the component file is
//! written and its revision published; when the edit that places the call is
//! refused, a component nobody asked to keep is left behind. The edit is
//! checked first: a refusal writes no file, publishes no revision and reopens
//! the dialog with the reason.
use super::mcp08b::contains;
use super::ux31::window;
use super::*;
use apiforge_core::format::Graph;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const NAME: &str = "Component name";
const SAVE_AND_PUBLISH: &str = "Save and publish";
const FILE: &str = "components/remove-order.afs";
/// On `order-flow.afs`'s Cleanup, the check reads the request's status.
const CHECK: &str = "    gone = assert removed.status == 204\n";
/// The check, reading the response's body, which the dialog does not keep.
const BODY_CHECK: &str = "    gone = assert removed.body == {}\n";

fn cleanup(harness: &FolderHarness, id: &str) -> Graph {
    match folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .model()
        .cloned()
    {
        Some(Document::Scenario(scenario)) => scenario.cleanup,
        other => panic!("a scenario, not {other:?}"),
    }
}

fn revisions(harness: &FolderHarness, handle: &str) -> usize {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .manifest()
        .revisions
        .iter()
        .filter(|revision| revision.handle == handle)
        .count()
}

/// Criterion 1: the dialog is opened for `removed` with what it proposes,
/// the status `gone` reads; then `gone` is changed to read the body, which
/// the component does not output, so its call cannot replace the step.
/// *Save and publish* keeps the dialog open with the reason, and there is no
/// component file, no revision, and the scenario is as it was.
#[test]
fn a_refused_call_publishes_nothing() {
    let directory = shop();
    let root = directory.path();
    fs::write(
        root.join(SCENARIO),
        fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/scenarios/lay/order-flow.afs"),
        )
        .unwrap(),
    )
    .unwrap();
    let (mut harness, id) = window(root, WIDE, SCENARIO, "Diagram");
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    let removed = cleanup(&harness, &id)
        .nodes
        .iter()
        .find(|node| node.handle == "removed")
        .expect("removed is on the Cleanup board")
        .id
        .clone();
    app(&mut harness)
        .selections
        .insert(id.clone(), vec![removed]);
    settle(&mut harness);
    choose(&mut harness, "Selection actions");
    choose(&mut harness, "Save as component");
    replace_text(&mut harness, NAME, "Remove order");

    let edited = source_of(&harness, &id).replacen(CHECK, BODY_CHECK, 1);
    assert_ne!(
        edited,
        source_of(&harness, &id),
        "the check is in the source"
    );
    {
        let session = app(&mut harness).session.as_mut().unwrap();
        session.edit_source(&id, &edited, CODE).unwrap();
        session.settle(&id).unwrap();
    }
    settle(&mut harness);

    choose(&mut harness, SAVE_AND_PUBLISH);
    assert!(has_label(&harness, NAME), "the dialog reopens");
    assert!(
        contains(&harness, "nothing was saved"),
        "the dialog says why"
    );
    assert!(!root.join(FILE).exists(), "no component file");
    assert_eq!(revisions(&harness, "remove_order"), 0, "no revision");
    assert_eq!(
        source_of(&harness, &id),
        edited,
        "the scenario is untouched"
    );
}
