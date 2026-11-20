//! DIAG-06f (#496) at the window: *Duplicate* writes the copy directly
//! after its original, *Paste* after the selected step, else last in the
//! main flow; an `if` is copied with everything in its braces, and steps
//! copied without their `if` are pasted as steps of the place; a paste that
//! would leave a statement reading what is not there is refused in the
//! board's banner and changes nothing. On `shop-30` in the demo shop,
//! through Ctrl+C, Ctrl+V and Ctrl+D, as the person copies a selection.
use super::diag07::{graph, id_of};
use super::ux31::by_path;
use super::*;
use apiforge_core::{chain::braces, graph::sequence, model::Branch};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const OTHER: &str = "scenarios/other.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";

fn shop_30() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-30.afs");
    fs::read_to_string(path).unwrap()
}

/// `shop-30` saved as the scenario of the demo shop, `other` beside it,
/// and `shop-30` open on the board at `size`.
fn opened_at(size: egui::Vec2, other: Option<&str>) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), shop_30()).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    if let Some(other) = other {
        let mut session = apiforge_project::ProjectSession::open(project.path()).unwrap();
        session
            .create_document(DocumentKind::Scenario, OTHER, other)
            .unwrap();
    }
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = by_path(&harness, SCENARIO);
    app(&mut harness).activate_tab(&id);
    settle(&mut harness);
    assert_eq!(source_of(&harness, &id), shop_30(), "opened as written");
    (project, harness, id)
}

fn opened() -> (tempfile::TempDir, FolderHarness, String) {
    opened_at(WIDE, None)
}

/// Selects the statements `handles` of the main flow on the board.
fn select(harness: &mut FolderHarness, id: &str, handles: &[&str]) {
    let board = graph(harness, id);
    let ids = handles.iter().map(|handle| id_of(&board, handle)).collect();
    app(harness).selections.insert(id.to_owned(), ids);
    settle(harness);
    harness.step();
}

fn copy(harness: &mut FolderHarness) {
    harness.event(egui::Event::Copy);
    harness.step();
    settle(harness);
}

fn paste(harness: &mut FolderHarness) {
    harness.event(egui::Event::Paste(String::new()));
    harness.step();
    settle(harness);
    harness.step();
}

fn duplicate(harness: &mut FolderHarness) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    settle(harness);
    harness.step();
}

fn undo(harness: &mut FolderHarness) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(harness);
    harness.step();
}

/// The steps in the order they run.
fn steps(harness: &FolderHarness, id: &str) -> Vec<String> {
    let board = graph(harness, id);
    sequence(&board)
        .into_iter()
        .filter_map(|step| board.nodes.iter().find(|node| node.id == step))
        .filter(|node| !node.kind.is_data())
        .map(|node| node.handle.clone())
        .collect()
}

/// Every statement of the main flow in written order.
fn written(harness: &FolderHarness, id: &str) -> Vec<String> {
    graph(harness, id)
        .nodes
        .iter()
        .map(|node| node.handle.clone())
        .collect()
}

/// The braces the statement `handle` stands in, by the handle of its `if`.
fn braces_of(harness: &FolderHarness, id: &str, handle: &str) -> Option<(String, Branch)> {
    let board = graph(harness, id);
    let written = braces(&board);
    written
        .get(id_of(&board, handle).as_str())
        .map(|(x, branch)| {
            let x = board.nodes.iter().find(|node| node.id == *x).unwrap();
            (x.handle.clone(), *branch)
        })
}

fn selection(harness: &FolderHarness, id: &str) -> Vec<String> {
    folder(harness)
        .selections
        .get(id)
        .cloned()
        .unwrap_or_default()
}

/// The source passes the check: no error, a model.
fn passes(harness: &FolderHarness, id: &str) {
    let session = folder(harness).session.as_ref().unwrap();
    let document = session.document(id).unwrap();
    assert!(
        document
            .diagnostics()
            .iter()
            .all(|d| d.severity != apiforge_script::Severity::Error),
        "{}\n{:#?}",
        document.text(),
        document.diagnostics()
    );
    assert!(document.model().is_some());
}

fn position(list: &[String], handle: &str) -> usize {
    list.iter()
        .position(|h| h == handle)
        .unwrap_or_else(|| panic!("`{handle}` in {list:?}"))
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn diag06f_criterion_1_a_duplicate_runs_directly_after_its_original() {
    let (_project, mut harness, id) = opened();
    select(&mut harness, &id, &["cart"]);
    duplicate(&mut harness);
    passes(&harness, &id);
    let source = source_of(&harness, &id);
    assert!(
        source.contains(
            "  cart_total = assert cart.body.total ~= expected_total within 0.01\n  cart_2 = POST \"/cart\" {"
        ),
        "the copy is written after the original and its checks:\n{source}"
    );
    assert!(
        source.contains("  cart_created_2 = assert cart_2.status == 201\n  cart_view = GET"),
        "with the step's own check:\n{source}"
    );
    let order = steps(&harness, &id);
    assert_eq!(position(&order, "cart_2"), position(&order, "cart") + 1);
    assert_eq!(
        position(&order, "cart_view"),
        position(&order, "cart_2") + 1
    );
    assert_eq!(selection(&harness, &id).len(), 2, "the copies are selected");
}

#[test]
fn diag06f_criterion_1_a_selection_across_yes_and_no() {
    let (_project, mut harness, id) = opened();
    select(&mut harness, &id, &["receipt", "not_created"]);
    duplicate(&mut harness);
    passes(&harness, &id);
    assert_eq!(
        braces_of(&harness, &id, "receipt_2"),
        Some(("paid".to_owned(), Branch::True))
    );
    assert_eq!(
        braces_of(&harness, &id, "not_created_2"),
        Some(("paid".to_owned(), Branch::False))
    );
    let order = written(&harness, &id);
    assert_eq!(
        position(&order, "receipt_2"),
        position(&order, "receipt_total") + 1
    );
    assert_eq!(
        position(&order, "not_created_2"),
        position(&order, "not_created") + 1
    );
}

#[test]
fn diag06f_criterion_1_a_statement_in_a_selected_if_is_copied_once() {
    let (_project, mut harness, id) = opened();
    let before = written(&harness, &id).len();
    select(&mut harness, &id, &["paid", "payment"]);
    duplicate(&mut harness);
    passes(&harness, &id);
    // `paid`, the six statements of *Yes* and the one of *No*.
    assert_eq!(written(&harness, &id).len(), before + 8);
    let source = source_of(&harness, &id);
    assert_eq!(source.matches("payment_2 = POST").count(), 1, "{source}");
    assert!(!source.contains("payment_3"), "{source}");
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag06f_criterion_2_an_if_alone_is_copied_with_its_braces() {
    let (_project, mut harness, id) = opened();
    select(&mut harness, &id, &["paid"]);
    copy(&mut harness);
    app(&mut harness).selections.remove(&id);
    paste(&mut harness);
    passes(&harness, &id);
    for (handle, branch) in [
        ("payment_2", Branch::True),
        ("receipt_2", Branch::True),
        ("receipt_total_2", Branch::True),
        ("refund_2", Branch::True),
        ("refund_amount_2", Branch::True),
        ("logout_2", Branch::True),
        ("not_created_2", Branch::False),
    ] {
        assert_eq!(
            braces_of(&harness, &id, handle),
            Some(("paid_2".to_owned(), branch)),
            "{handle}"
        );
    }
    let order = steps(&harness, &id);
    assert_eq!(
        order.last().map(String::as_str),
        Some("logout_2"),
        "{order:?}"
    );
    assert_eq!(position(&order, "paid_2"), position(&order, "logout") + 1);
}

#[test]
fn diag06f_criterion_2_steps_without_their_if_are_steps_of_the_place() {
    let (_project, mut harness, id) = opened();
    select(&mut harness, &id, &["payment", "receipt"]);
    copy(&mut harness);
    select(&mut harness, &id, &["order"]);
    paste(&mut harness);
    passes(&harness, &id);
    assert_eq!(braces_of(&harness, &id, "payment_2"), None);
    assert_eq!(braces_of(&harness, &id, "receipt_2"), None);
    let source = source_of(&harness, &id);
    assert!(
        source.contains("\n  payment_2 = POST") && source.contains("\n  receipt_2 = GET"),
        "written as steps of the main flow:\n{source}"
    );
    let order = steps(&harness, &id);
    assert_eq!(position(&order, "payment_2"), position(&order, "order") + 1);
    assert_eq!(
        position(&order, "receipt_2"),
        position(&order, "payment_2") + 1
    );
    assert_eq!(position(&order, "paid"), position(&order, "receipt_2") + 1);
}

// Criterion 3 ----------------------------------------------------------------

#[test]
fn diag06f_criterion_3_paste_after_the_selected_step_else_last() {
    let (_project, mut harness, id) = opened();
    select(&mut harness, &id, &["cart_view"]);
    copy(&mut harness);
    // After the selected step.
    select(&mut harness, &id, &["address"]);
    paste(&mut harness);
    passes(&harness, &id);
    let order = steps(&harness, &id);
    assert_eq!(
        position(&order, "cart_view_2"),
        position(&order, "address") + 1
    );
    undo(&mut harness);
    // A check selected: after the step it is written under, and its checks.
    select(&mut harness, &id, &["checkout_total"]);
    paste(&mut harness);
    passes(&harness, &id);
    let order = written(&harness, &id);
    assert_eq!(
        position(&order, "cart_view_2"),
        position(&order, "checkout_created") + 1,
        "{order:?}"
    );
    undo(&mut harness);
    // Several selected: after the one that runs last.
    select(&mut harness, &id, &["checkout", "address", "valid_cart"]);
    paste(&mut harness);
    passes(&harness, &id);
    let order = steps(&harness, &id);
    assert_eq!(
        position(&order, "cart_view_2"),
        position(&order, "checkout") + 1
    );
    undo(&mut harness);
    // Nothing selected: last in the main flow.
    app(&mut harness).selections.remove(&id);
    paste(&mut harness);
    passes(&harness, &id);
    let order = steps(&harness, &id);
    assert_eq!(
        order.last().map(String::as_str),
        Some("cart_view_2"),
        "{order:?}"
    );
    assert_eq!(
        written(&harness, &id).last().map(String::as_str),
        Some("cart_view_2")
    );
    assert_eq!(braces_of(&harness, &id, "cart_view_2"), None);
}

// Criterion 5 ----------------------------------------------------------------

/// The paste is refused with `message` in the board's banner: the source,
/// the model, the selection and the history are as before.
fn refused(harness: &mut FolderHarness, id: &str, message: &str) {
    let before = source_of(harness, id);
    let board = graph(harness, id);
    let selected = selection(harness, id);
    let undoable = folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .can_undo();
    paste(harness);
    assert!(
        harness.query_by_label(message).is_some(),
        "the banner says: {message}"
    );
    assert_eq!(source_of(harness, id), before, "the source is unchanged");
    assert_eq!(graph(harness, id), board, "the model is unchanged");
    assert_eq!(
        selection(harness, id),
        selected,
        "the selection is unchanged"
    );
    assert_eq!(
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .document(id)
            .unwrap()
            .can_undo(),
        undoable,
        "the history is unchanged"
    );
}

#[test]
fn diag06f_criterion_5_a_step_below_the_place_is_refused() {
    let (_project, mut harness, id) = opened();
    select(&mut harness, &id, &["cart_view"]);
    copy(&mut harness);
    select(&mut harness, &id, &["login"]);
    refused(
        &mut harness,
        &id,
        "`cart_view` reads `cart.body.id`, which runs after this place. Paste it below `cart`.",
    );
}

#[test]
fn diag06f_criterion_5_from_another_scenario() {
    let other = "scenario \"Other\" {\n  begin = start\n  login = POST \"/login\" {\n    sample 200 { token: \"t\" }\n  }\n}\n";
    let (_project, mut harness, id) = opened_at(WIDE, Some(other));
    select(&mut harness, &id, &["cart_view"]);
    copy(&mut harness);
    let other = by_path(&harness, OTHER);
    app(&mut harness).review_open(OTHER, None);
    app(&mut harness).activate_tab(&other);
    settle(&mut harness);
    harness.step();
    refused(
        &mut harness,
        &other,
        "`cart_view` reads `cart`, which is not in this scenario. Copy it too.",
    );
    // Copied with what it reads, it is pasted.
    app(&mut harness).activate_tab(&id);
    settle(&mut harness);
    select(
        &mut harness,
        &id,
        &["catalogue", "first", "second", "cart", "cart_view"],
    );
    copy(&mut harness);
    app(&mut harness).activate_tab(&other);
    settle(&mut harness);
    paste(&mut harness);
    passes(&harness, &other);
    let source = source_of(&harness, &other);
    assert!(
        source.contains("cart_view = GET \"/cart/{{cart.body.id}}\""),
        "{source}"
    );
}

#[test]
fn diag06f_criterion_5_a_parameter_the_scenario_lacks_is_refused() {
    let (_project, mut harness, id) = opened();
    // A check of a parameter, written in the code of another scenario,
    // reaches this one through the clipboard of the window.
    let labels = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/labels.afs"),
    )
    .unwrap();
    let project = folder(&harness).context_project();
    let lowered = apiforge_script::lower(
        &apiforge_script::parse(&labels),
        &apiforge_script::LowerContext::folder(&project),
    );
    let Some(apiforge_script::Document::Scenario(scenario)) = lowered.document else {
        panic!("labels is a scenario");
    };
    let no_token = id_of(&scenario.graph, "no_token");
    let selected = std::collections::HashSet::from([no_token.as_str()]);
    app(&mut harness).clipboard = Some(apiforge_core::edit::copy_statements(
        &scenario.graph,
        &scenario.graph,
        &selected,
    ));
    refused(
        &mut harness,
        &id,
        "`no_token` reads `param.parcel_id`, which this scenario does not declare.",
    );
}

// Undo ------------------------------------------------------------------------

#[test]
fn diag06f_one_undo_takes_back_a_paste_and_a_duplicate() {
    let (_project, mut harness, id) = opened();
    let before = source_of(&harness, &id);
    select(&mut harness, &id, &["paid"]);
    duplicate(&mut harness);
    assert_ne!(source_of(&harness, &id), before);
    undo(&mut harness);
    assert_eq!(source_of(&harness, &id), before, "byte for byte");
    select(&mut harness, &id, &["cart_view"]);
    copy(&mut harness);
    select(&mut harness, &id, &["address"]);
    paste(&mut harness);
    assert_ne!(source_of(&harness, &id), before);
    undo(&mut harness);
    assert_eq!(source_of(&harness, &id), before, "byte for byte");
}
