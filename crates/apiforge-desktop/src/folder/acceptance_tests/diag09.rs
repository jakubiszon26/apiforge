//! DIAG-09 (#399): before a scenario has ever run, the Inspector's *Output
//! data* shows the step's sample, and *Add comparison* and *Assign to
//! variable* on a field of it do what they do after a run: the check, or the
//! variable, is written and the file passes `check`. Where the board takes
//! no edit, both buttons are disabled and their hover says why. On the demo
//! shop, never run, through the Inspector's buttons.
use super::mcp08b::{AGENT, agent, await_endpoint, contains, hand_over, read_only};
use super::*;
use apiforge_core::format::{Binding, Graph, Node};
use apiforge_core::model::NodeKind;
use apiforge_storage::project::HolderRole;
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const COMPARISON: &str = "Add comparison";
const VARIABLE: &str = "Assign to variable";
/// The hover of a disabled button on a board whose code has errors.
const LAST_VALID: &str =
    "The code has errors. This is the last valid version; it is read-only until the code is fixed.";
/// The hover of a disabled button in a window that observes an agent.
const PAUSED: &str = "Editing is paused here while another process holds the project.";

/// The demo shop on the board of its scenario, never run.
fn opened() -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    let mut harness = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    assert!(
        folder(&harness).fresh_run(&id).is_none(),
        "the scenario has never run"
    );
    (project, harness, id)
}

fn graph(harness: &FolderHarness, id: &str, cleanup: bool) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), cleanup).clone()
}

fn node<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
}

/// `handle` of the board (Cleanup when `cleanup`) selected, the Inspector on
/// *Output data*.
fn output_data(harness: &mut FolderHarness, id: &str, cleanup: bool, handle: &str) {
    if cleanup {
        choose(harness, "Cleanup");
    }
    let step = node(&graph(harness, id, cleanup), handle).id.clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
    super::inspector_tab(harness, "Output data")
        .expect("the Inspector has an Output data tab")
        .click();
    settle(harness);
}

/// The button `name` on the row of `field` in the Inspector's tree: the
/// nearest one under the field's label, which stands on the row above its
/// value and buttons.
fn button<'a>(harness: &'a FolderHarness, field: &str, name: &'a str) -> egui_kittest::Node<'a> {
    let edge = harness.get_by_label("Resize inspector").rect().left();
    let label = harness
        .query_all_by_label(field)
        .find(|node| node.rect().left() > edge)
        .unwrap_or_else(|| panic!("the Inspector shows the field {field}"));
    let y = label.rect().center().y;
    harness
        .query_all_by_label(name)
        .filter(|button| button.rect().left() > edge && button.rect().center().y > y)
        .min_by(|a, b| (a.rect().center().y - y).total_cmp(&(b.rect().center().y - y)))
        .unwrap_or_else(|| panic!("the {field} row has {name}"))
}

fn click(harness: &mut FolderHarness, field: &str, name: &str) {
    button(harness, field, name).click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
}

/// The steps `after` has and `before` has not.
fn added<'a>(before: &Graph, after: &'a Graph) -> Vec<&'a Node> {
    after
        .nodes
        .iter()
        .filter(|node| before.nodes.iter().all(|old| old.id != node.id))
        .collect()
}

fn reads(binding: Option<&Binding>, source: &str, port: &str, path: &str) -> bool {
    matches!(
        binding,
        Some(Binding::Reference { node_id, port: read, path: field })
            if node_id == source && read == port && field == path
    )
}

/// The project saved, then read back as `apiforge check` reads it: every
/// document has a valid definition.
fn save_and_check(harness: &mut FolderHarness, root: &std::path::Path) {
    choose(harness, "Save");
    settle(harness);
    let session = apiforge_project::ProjectSession::open_saved_read_only(root).unwrap();
    for document in session.documents() {
        let (document, valid) =
            apiforge_mcp::folder::check_document(&session, document.id()).unwrap();
        assert!(
            valid,
            "{} passes check: {:?}\n{}",
            document.entry().path.as_str(),
            document.diagnostics(),
            document.text()
        );
    }
}

/// *Add comparison* on `field` of `handle`'s `port` adds one check of that
/// field, which expects the sample's `value`, and the file passes `check`.
fn assert_comparison(
    harness: &mut FolderHarness,
    root: &std::path::Path,
    id: &str,
    cleanup: bool,
    (handle, port, field): (&str, &str, &str),
    value: serde_json::Value,
) {
    let before = graph(harness, id, cleanup);
    let subject = node(&before, handle).id.clone();
    click(harness, field, COMPARISON);
    let after = graph(harness, id, cleanup);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "one step added: {new:?}");
    let check = new[0];
    assert_eq!(check.kind, NodeKind::Assert, "the step is a check");
    assert!(
        reads(check.inputs.get("actual"), &subject, port, field),
        "the check reads {handle}.{port}.{field}: {:?}",
        check.inputs
    );
    assert_eq!(
        check.inputs.get("expected"),
        Some(&Binding::Literal { value }),
        "the check expects the sample's value"
    );
    save_and_check(harness, root);
}

/// *Assign to variable* on `field` of `handle`'s `port`, the dialog applied
/// with its suggested name, adds one variable reading that field, and the
/// file passes `check`.
fn assert_variable(
    harness: &mut FolderHarness,
    root: &std::path::Path,
    id: &str,
    cleanup: bool,
    (handle, port, field): (&str, &str, &str),
) {
    let before = graph(harness, id, cleanup);
    let subject = node(&before, handle).id.clone();
    click(harness, field, VARIABLE);
    assert!(
        has_label(harness, "Assign variable"),
        "the dialog to name the variable opens"
    );
    choose(harness, "Assign variable");
    let after = graph(harness, id, cleanup);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "one step added: {new:?}");
    let variable = new[0];
    assert!(
        variable.is_variable(),
        "the step is a variable: {variable:?}"
    );
    assert!(
        variable
            .inputs
            .values()
            .any(|binding| reads(Some(binding), &subject, port, field)),
        "the variable reads {handle}.{port}.{field}: {:?}",
        variable.inputs
    );
    save_and_check(harness, root);
}

/// Criterion 1: on the demo shop never run, *Add comparison* on `name` in
/// the sample of *Product* adds one check of that field under the step,
/// expecting the sample's value; the file passes `check`.
#[test]
fn diag09_criterion_1_comparison_before_a_run() {
    let (project, mut harness, id) = opened();
    output_data(&mut harness, &id, false, "product");
    assert!(
        !button(&harness, "name", COMPARISON)
            .accesskit_node()
            .is_disabled(),
        "the button is enabled on a board that takes edits"
    );
    assert_comparison(
        &mut harness,
        project.path(),
        &id,
        false,
        ("product", "product", "name"),
        serde_json::json!("Whole bean coffee"),
    );
}

/// Criterion 2: *Assign to variable* on the same field adds the variable.
#[test]
fn diag09_criterion_2_variable_before_a_run() {
    let (project, mut harness, id) = opened();
    output_data(&mut harness, &id, false, "product");
    assert_variable(
        &mut harness,
        project.path(),
        &id,
        false,
        ("product", "product", "name"),
    );
}

/// Criterion 3: the same on *Delete cart*, a request of the cleanup, whose
/// sample is `{ "deleted": true }`.
#[test]
fn diag09_criterion_3_cleanup() {
    let (project, mut harness, id) = opened();
    output_data(&mut harness, &id, true, "delete_cart");
    assert_comparison(
        &mut harness,
        project.path(),
        &id,
        true,
        ("delete_cart", "body", "deleted"),
        serde_json::json!(true),
    );
    assert_variable(
        &mut harness,
        project.path(),
        &id,
        true,
        ("delete_cart", "body", "deleted"),
    );
}

/// Both buttons on `field`'s row are disabled, and pointing at each shows
/// `why` in a tooltip, besides any banner saying it; a click changes nothing.
fn assert_never_silent(harness: &mut FolderHarness, id: &str, field: &str, why: &str) {
    let said = |harness: &FolderHarness| harness.query_all_by_label_contains(why).count();
    for name in [COMPARISON, VARIABLE] {
        assert!(
            button(harness, field, name).accesskit_node().is_disabled(),
            "{name} is disabled"
        );
        let elsewhere = said(harness);
        let shown = crate::testing::hover_until(
            harness,
            |harness| Some(button(harness, field, name).rect()),
            |harness| said(harness) > elsewhere,
        );
        assert!(shown, "pointing at {name} says why: {why}");
        harness.remove_cursor();
        settle(harness);
        let before = graph(harness, id, false);
        button(harness, field, name).click();
        settle(harness);
        assert_eq!(graph(harness, id, false), before, "{name} changes nothing");
        assert!(!has_label(harness, "Assign variable"), "no dialog opens");
    }
}

/// Criterion 4: where the board takes no edit, both buttons are disabled
/// with a hover that says why — on a board whose code has errors, which
/// shows its last valid version read-only, and in a window that observes
/// an agent holding the project. They are never enabled and silent.
#[test]
fn diag09_criterion_4_never_silent() {
    // Read-only: the code has errors.
    let (_project, mut harness, id) = opened();
    output_data(&mut harness, &id, false, "product");
    let broken = format!("{}\n}}\n", source_of(&harness, &id));
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &broken, CODE)
        .unwrap();
    settle(&mut harness);
    assert!(
        !folder(&harness).snapshot(&id).unwrap().editable,
        "the board takes no edit"
    );
    assert_never_silent(&mut harness, &id, "name", LAST_VALID);
    drop(harness);

    // Observing: an agent took the project while Output data was open.
    let project = crate::testing::shop_project();
    let root = project.path();
    let mut harness = crate::testing::folder_harness_in(root, WIDE, Mode::Diagram);
    await_endpoint(&mut harness, root, HolderRole::Window);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    output_data(&mut harness, &id, false, "product");
    let mut server = agent(root, "Claude Code");
    hand_over(&mut harness, &mut server, "Written by the agent.");
    super::mcp08b::await_until(
        &mut harness,
        std::time::Instant::now(),
        "the agent's save",
        read_only,
    );
    assert!(contains(&harness, &format!("{AGENT} · ")), "observing");
    assert_never_silent(&mut harness, &id, "name", PAUSED);
    drop(harness);
    drop(server);
}
