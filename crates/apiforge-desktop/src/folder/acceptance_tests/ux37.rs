//! UX-37 (#271, owner decision of 2026-11-03 on #265, option B of
//! 2026-11-10): *Save as component* still refuses by default to drop an
//! output a later step reads. When every such reader can lose the reference
//! and stay valid (a check's *Actual*, a value's source), a checkbox under
//! the refusal saves anyway and leaves those readers unconnected; a request
//! that uses the value in its URL, headers, query or body gets the refusal
//! alone, naming it. The result is validated before anything is written.
use super::mcp08b::contains;
use super::ux31::{toggled, window};
use super::*;
use crate::testing::MINIMUM;
use apiforge_core::format::{Graph, Node};
use egui::accesskit::Toggled;
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const TITLE: &str = "Save as component";
const NAME: &str = "Component name";
const SAVE_AND_PUBLISH: &str = "Save and publish";
const FILE: &str = "components/remove-order.afs";

/// On `order-flow.afs`'s Cleanup, the check `gone` reads `removed.status`.
const REFUSAL: &str = "A step after the selection reads removed → status; keep it as an output.";
const LEAVE: &str = "Save anyway and leave \"gone\" unconnected";
/// The check, as it reads the request's status.
const CHECK: &str = "    gone = assert removed.status == 204\n";

/// In the demo, the cleanup's *Delete cart* uses the cart's id in its URL.
const DEMO_REFUSAL: &str =
    "A step after the selection reads Add to cart → cart.id; keep it as an output.";
const DEMO_REASON: &str = "\"Delete cart\" uses this in its URL.";

fn disabled(harness: &FolderHarness, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().is_disabled()
}

/// `fixtures/scenarios/lay/order-flow.afs`.
fn order_flow() -> String {
    fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/order-flow.afs"),
    )
    .unwrap()
}

fn scenario(harness: &FolderHarness, id: &str) -> (Graph, Graph) {
    match folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .model()
        .cloned()
    {
        Some(Document::Scenario(scenario)) => (scenario.graph, scenario.cleanup),
        other => panic!("a scenario, not {other:?}"),
    }
}

fn by_handle<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("{handle} is on the board"))
}

/// The number of each *Use output* row, ticked or not.
fn outputs(harness: &FolderHarness) -> Vec<(usize, bool)> {
    (1..)
        .map_while(|row| {
            let label = format!("Use output {row}");
            has_label(harness, &label)
                .then(|| (row, toggled(harness, &label) == Some(Toggled::True)))
        })
        .collect()
}

/// `order-flow.afs` in place of the demo scenario at `size`, on its Cleanup
/// board, with the dialog open for `removed`: the status `gone` reads
/// unticked, another output ticked in its place.
fn open_order_flow(root: &std::path::Path, size: egui::Vec2) -> (FolderHarness, String) {
    fs::write(root.join(SCENARIO), order_flow()).unwrap();
    let (mut harness, id) = window(root, size, SCENARIO, "Diagram");
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    open_dialog(&mut harness, &id);
    (harness, id)
}

fn open_dialog(harness: &mut FolderHarness, id: &str) {
    let (_, cleanup) = scenario(harness, id);
    let removed = by_handle(&cleanup, "removed").id.clone();
    app(harness).selections.insert(id.to_owned(), vec![removed]);
    settle(harness);
    choose(harness, "Selection actions");
    choose(harness, TITLE);
    assert!(has_label(harness, NAME), "the dialog is open");
    replace_text(harness, NAME, "Remove order");
    let rows = outputs(harness);
    let (read, _) = *rows
        .iter()
        .find(|(_, ticked)| *ticked)
        .expect("the status gone reads is proposed");
    let (other, _) = *rows
        .iter()
        .find(|(_, ticked)| !*ticked)
        .expect("another output");
    choose(harness, &format!("Use output {read}"));
    choose(harness, &format!("Use output {other}"));
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

/// Criterion 1: unticking the output the check `gone` reads shows UX-30's
/// refusal and, under it, the checkbox, unticked; saving is disabled.
#[test]
fn ux37_criterion_1_the_refusal_stays_the_default() {
    for size in [MINIMUM, WIDE] {
        let directory = shop();
        let (harness, _) = open_order_flow(directory.path(), size);
        assert!(contains(&harness, REFUSAL), "the refusal at {size:?}");
        assert_eq!(
            toggled(&harness, LEAVE),
            Some(Toggled::False),
            "the checkbox at {size:?}"
        );
        assert!(
            harness.get_by_label(LEAVE).rect().top()
                >= harness.get_by_label_contains(REFUSAL).rect().top(),
            "the checkbox is under the refusal at {size:?}"
        );
        assert!(disabled(&harness, SAVE_AND_PUBLISH), "{size:?}");
    }
}

/// Criterion 1b: in the demo, the cleanup's *Delete cart* uses the cart's
/// id in its URL, so the refusal stands alone, naming it; no checkbox.
#[test]
fn ux37_criterion_1b_no_checkbox_for_a_template_reader() {
    let directory = shop();
    let (mut harness, id) = window(directory.path(), WIDE, SCENARIO, "Diagram");
    app(&mut harness)
        .selections
        .insert(id.clone(), vec!["cart".to_owned(), "price".to_owned()]);
    settle(&mut harness);
    choose(&mut harness, "Selection actions");
    choose(&mut harness, TITLE);
    replace_text(&mut harness, NAME, "Check cart price");
    choose(&mut harness, "Use output 1");
    choose(&mut harness, "Use output 2");
    assert!(contains(&harness, DEMO_REFUSAL));
    assert!(
        contains(&harness, DEMO_REASON),
        "the reason names the reader"
    );
    assert!(harness.query_by_label_contains("Save anyway").is_none());
    assert!(disabled(&harness, SAVE_AND_PUBLISH));
}

/// Criterion 2: ticked, saving is enabled; after *Save and publish* the
/// request is one call, the check's *Actual* is unconnected and diagnosed,
/// and every other step of both boards is as it was.
#[test]
fn ux37_criterion_2_saved_with_the_reader_unconnected() {
    let directory = shop();
    let (mut harness, id) = open_order_flow(directory.path(), WIDE);
    let (main, cleanup) = scenario(&harness, &id);
    choose(&mut harness, LEAVE);
    assert_eq!(toggled(&harness, LEAVE), Some(Toggled::True));
    assert!(contains(&harness, REFUSAL), "the refusal stays as a note");
    assert!(!disabled(&harness, SAVE_AND_PUBLISH));
    choose(&mut harness, SAVE_AND_PUBLISH);
    assert!(!has_label(&harness, NAME), "the dialog closed");
    assert_eq!(revisions(&harness, "remove_order"), 1);
    if let Some(session) = app(&mut harness).session.as_mut() {
        session.settle(&id).unwrap();
    }
    settle(&mut harness);

    let source = source_of(&harness, &id);
    assert!(source.contains("remove_order@1("), "{source}");
    let (after_main, after_cleanup) = scenario(&harness, &id);
    assert_eq!(after_main.nodes, main.nodes, "Main is as it was");
    let removed = by_handle(&cleanup, "removed").id.clone();
    let mut expected: Vec<Node> = cleanup
        .nodes
        .iter()
        .filter(|node| node.id != removed)
        .cloned()
        .collect();
    let gone = expected
        .iter_mut()
        .find(|node| node.handle == "gone")
        .unwrap();
    assert!(gone.inputs.shift_remove("actual").is_some());
    let after: Vec<Node> = after_cleanup
        .nodes
        .iter()
        .filter(|node| node.handle != "remove_order")
        .cloned()
        .collect();
    assert_eq!(after, expected, "only gone's Actual is unconnected");

    // Unconnected as removing its wire leaves it: an empty slot in the
    // source, which a run reports as the ordinary missing input.
    assert!(source.contains("    gone = assert ? == 204\n"), "{source}");
    let report = simulate(&mut harness);
    let gone = report
        .steps
        .iter()
        .find(|step| step.handle == "gone")
        .expect("the check runs");
    assert!(
        gone.message
            .as_deref()
            .is_some_and(|message| message.contains("is missing")),
        "{:?}",
        gone.message
    );
}

/// Runs the active tab's simulation and returns its report.
fn simulate(harness: &mut FolderHarness) -> RunResult {
    choose(harness, "Run simulation");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(std::time::Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    settle(harness);
    let active = folder(harness).workbench.active().unwrap().to_owned();
    folder(harness)
        .run_result(&active)
        .expect("the run leaves a report")
        .clone()
}

/// Criterion 3: one undo brings the request and the reference back.
#[test]
fn ux37_criterion_3_undo_restores() {
    let directory = shop();
    let (mut harness, id) = open_order_flow(directory.path(), WIDE);
    let before = scenario(&harness, &id);
    choose(&mut harness, LEAVE);
    choose(&mut harness, SAVE_AND_PUBLISH);
    assert_ne!(scenario(&harness, &id), before);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut harness);
    assert_eq!(scenario(&harness, &id), before, "one undo restores");
}

/// Criterion 4: the choice is never remembered: cancelled and reopened, the
/// dialog shows the checkbox unticked.
#[test]
fn ux37_criterion_4_never_remembered() {
    let directory = shop();
    let (mut harness, id) = open_order_flow(directory.path(), WIDE);
    choose(&mut harness, LEAVE);
    assert_eq!(toggled(&harness, LEAVE), Some(Toggled::True));
    choose(&mut harness, "Cancel");
    assert!(!has_label(&harness, NAME), "the dialog closed");
    open_dialog(&mut harness, &id);
    assert_eq!(toggled(&harness, LEAVE), Some(Toggled::False));
    assert!(disabled(&harness, SAVE_AND_PUBLISH));
}

/// Criterion 5: with the checkbox ticked, the scenario that would result is
/// validated before anything is written or published. Here `gone` becomes a
/// request that uses the status in its URL after the dialog was ticked, so
/// the result cannot stand: the dialog says so and stays open, and there is
/// no component file and no revision.
#[test]
fn ux37_criterion_5_nothing_is_published_when_the_call_cannot_be_placed() {
    let directory = shop();
    let (mut harness, id) = open_order_flow(directory.path(), WIDE);
    choose(&mut harness, LEAVE);
    let edited = source_of(&harness, &id).replacen(
        CHECK,
        "    gone = GET \"/check/{{removed.status}}\" {\n      sample 200 {}\n    }\n",
        1,
    );
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
    assert!(has_label(&harness, NAME), "the dialog stays open");
    assert!(
        contains(&harness, "nothing was saved"),
        "the dialog says why"
    );
    assert!(!directory.path().join(FILE).exists(), "no component file");
    assert_eq!(revisions(&harness, "remove_order"), 0, "no revision");
    assert_eq!(
        source_of(&harness, &id),
        edited,
        "the scenario is untouched"
    );
}
