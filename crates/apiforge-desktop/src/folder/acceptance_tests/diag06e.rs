//! DIAG-06e (#494) at the window: a delete on the board is refused while a
//! statement that stays reads what would go, naming every reader in the
//! board's banner; an `if` with statements in its braces asks once; *Start*
//! stays while its flow has a step; one Undo gives the file back byte for
//! byte. On `shop-30` in the demo shop, through the Delete key, as the person
//! deletes a selection.
use super::diag07::{graph, id_of};
use super::*;
use apiforge_core::graph::sequence;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";

/// Replaces `from`, which `text` holds exactly once, by `to`.
fn marked(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "`{from}` is written once");
    text.replacen(from, to, 1)
}

fn shop_30() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-30.afs");
    fs::read_to_string(path).unwrap()
}

/// `shop-30` with a free comment closing the braces of `paid`, a blank line
/// in them, and `refund`'s sample written before its headers (DIAG-06g).
fn shop_text() -> String {
    let text = shop_30();
    let text = marked(
        &text,
        "      headers { Authorization: \"Bearer {{login.body.token}}\" }\n      sample 200 { refunded: true, amount: 172.5 }\n",
        "      sample 200 { refunded: true, amount: 172.5 }\n      headers { Authorization: \"Bearer {{login.body.token}}\" }\n",
    );
    let text = marked(
        &text,
        "    receipt_total = assert receipt.body.total ~= cart.body.total within 0.01\n",
        "    receipt_total = assert receipt.body.total ~= cart.body.total within 0.01\n\n",
    );
    marked(
        &text,
        "      sample 204 {}\n    }\n  } else {\n",
        "      sample 204 {}\n    }\n\n    // Partial refunds are not covered yet.\n  } else {\n",
    )
}

/// `text` saved as a scenario of the demo shop and open on the board.
fn opened_with(text: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), text).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    assert_eq!(source_of(&harness, &id), text, "opened as written");
    (project, harness, id)
}

fn opened() -> (tempfile::TempDir, FolderHarness, String) {
    opened_with(&shop_text(), WIDE)
}

fn press(harness: &mut FolderHarness, modifiers: egui::Modifiers, key: egui::Key) {
    harness.key_press_modifiers(modifiers, key);
    settle(harness);
    harness.step();
}

/// Selects the statements `handles` on the board and presses Delete.
fn delete(harness: &mut FolderHarness, id: &str, handles: &[&str]) {
    let board = graph(harness, id);
    let ids = handles.iter().map(|handle| id_of(&board, handle)).collect();
    app(harness).selections.insert(id.to_owned(), ids);
    settle(harness);
    harness.step();
    press(harness, egui::Modifiers::NONE, egui::Key::Delete);
}

fn handles(harness: &FolderHarness, id: &str) -> Vec<String> {
    graph(harness, id)
        .nodes
        .iter()
        .map(|node| node.handle.clone())
        .collect()
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

/// The delete is refused with `message` in the board's banner and nothing
/// changes, neither the source nor the model.
fn refused(harness: &mut FolderHarness, id: &str, handles_deleted: &[&str], message: &str) {
    let before = source_of(harness, id);
    let board = graph(harness, id);
    delete(harness, id, handles_deleted);
    assert!(
        harness.query_by_label(message).is_some(),
        "the banner says: {message}"
    );
    assert_eq!(source_of(harness, id), before, "the source is unchanged");
    assert_eq!(graph(harness, id), board, "the model is unchanged");
    assert!(
        harness.query_by_label("Cancel").is_none(),
        "a refused delete asks nothing"
    );
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn diag06e_criterion_1_a_step_that_is_read_is_refused_naming_every_reader() {
    let (_project, mut harness, id) = opened();
    // `pay`-like readers in the main flow and `removed` in the cleanup;
    // `cart_created` reads only `cart` and would go with it.
    refused(
        &mut harness,
        &id,
        &["cart"],
        "`cart_total`, `cart_view`, `address`, `order`, `receipt_total` and `removed` read \
         `cart`. Change or delete them first.",
    );
}

#[test]
fn diag06e_criterion_1_a_reader_whose_input_is_optional_is_named() {
    let (_project, mut harness, id) = opened();
    // `checkout`'s body reads `expected_total`; the body is not dropped.
    refused(
        &mut harness,
        &id,
        &["expected_total"],
        "`cart_total`, `checkout`, `checkout_total` and `refund_amount` read \
         `expected_total`. Change or delete them first.",
    );
    assert!(
        source_of(&harness, &id)
            .contains("body { cartId: valid_cart.value.id, total: expected_total }")
    );
}

#[test]
fn diag06e_criterion_1_with_the_readers_selected_all_go_and_the_neighbours_join() {
    let (_project, mut harness, id) = opened();
    delete(&mut harness, &id, &["refund", "refund_amount"]);
    passes(&harness, &id);
    let after = handles(&harness, &id);
    assert!(!after.iter().any(|h| h == "refund" || h == "refund_amount"));
    let order = steps(&harness, &id);
    let receipt = order.iter().position(|step| step == "receipt").unwrap();
    assert_eq!(
        order[receipt + 1],
        "logout",
        "the neighbours join: {order:?}"
    );
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag06e_criterion_2() {
    let (_project, mut harness, id) = opened();
    // `receipt_total` reads `receipt`, which would go, and `cart`, which stays.
    refused(
        &mut harness,
        &id,
        &["receipt"],
        "`receipt_total` reads `receipt`. Change or delete it first.",
    );
}

// Criterion 3 ----------------------------------------------------------------

#[test]
fn diag06e_criterion_3_what_goes_is_closed_over_a_steps_own_statements() {
    let (_project, mut harness, id) = opened();
    // `listed`, `first` and `second` read only `catalogue` and would go
    // with it; `product`, `expected_total` and `cart` read them.
    refused(
        &mut harness,
        &id,
        &["catalogue"],
        "`product`, `expected_total` and `cart` read `first` and `second`. Change or delete \
         them first. `first` and `second` would go with `catalogue`.",
    );
}

const EXPANDS: &str = "scenario \"Expands\" {
  param limit = 5
  begin = start
  a = GET \"/a\" {
    sample 200 { n: 1 }
  }
  v = a.body.n
  timeout = 30
  ok = assert 1 == 1
  limited = param.limit
  b = POST \"/b\" {
    body { n: v }
    sample 200 {}
  }
  c = POST \"/c\" {
    body { wait: timeout }
    sample 200 {}
  }
}
";

#[test]
fn diag06e_criterion_3_through_a_value_and_what_reads_no_statement_stays() {
    let (_project, mut harness, id) = opened_with(EXPANDS, WIDE);
    refused(
        &mut harness,
        &id,
        &["a"],
        "`b` reads `v`. Change or delete it first. `v` would go with `a`.",
    );
    delete(&mut harness, &id, &["a", "b"]);
    passes(&harness, &id);
    assert_eq!(
        handles(&harness, &id),
        ["begin", "timeout", "ok", "limited", "c"],
        "`v` goes with `a`; the literal, the constant check and the parameter's value stay"
    );
}

// Criterion 5 ----------------------------------------------------------------

const QUESTION: &str = "Delete `paid` and the 4 steps inside it?";

#[test]
fn diag06e_criterion_5_an_if_with_steps_asks_once_and_cancel_changes_nothing() {
    let (_project, mut harness, id) = opened();
    let before = source_of(&harness, &id);
    delete(&mut harness, &id, &["paid"]);
    assert!(
        harness.query_by_label(QUESTION).is_some(),
        "the question is asked"
    );
    choose(&mut harness, "Cancel");
    harness.step();
    assert!(harness.query_by_label(QUESTION).is_none());
    assert_eq!(source_of(&harness, &id), before, "Cancel changes nothing");
}

#[test]
fn diag06e_criterion_5_delete_removes_all_of_it_and_joins_its_neighbours() {
    let (_project, mut harness, id) = opened();
    delete(&mut harness, &id, &["paid"]);
    choose(&mut harness, "Delete");
    harness.step();
    assert!(harness.query_by_label(QUESTION).is_none(), "asked once");
    passes(&harness, &id);
    let after = source_of(&harness, &id);
    assert!(!after.contains("paid = if"), "{after}");
    for inside in ["payment", "receipt", "refund", "logout", "not_created"] {
        assert!(
            !handles(&harness, &id).iter().any(|h| h == inside),
            "`{inside}`"
        );
    }
    assert_eq!(
        steps(&harness, &id).last().map(String::as_str),
        Some("order")
    );
}

#[test]
fn diag06e_criterion_5_a_reader_outside_refuses_before_any_question() {
    let text = "scenario \"Outside\" {
  begin = start
  a = GET \"/a\" {
    sample 200 {}
  }
  x = if a.status == 200 {
    inner = GET \"/inner\" {
      sample 200 { id: 1 }
    }
  }
  after = GET \"/after/{{inner.body.id}}\" {
    sample 200 {}
  }
}
";
    let (_project, mut harness, id) = opened_with(text, WIDE);
    refused(
        &mut harness,
        &id,
        &["x"],
        "`after` reads `inner`. Change or delete it first. `inner` would go with `x`.",
    );
}

#[test]
fn diag06e_criterion_5_an_if_with_empty_braces_is_deleted_without_a_question() {
    let text = "scenario \"Empty\" {
  begin = start
  a = GET \"/a\" {
    sample 200 {}
  }
  x = if a.status == 200 {
  }
  b = GET \"/b\" {
    sample 200 {}
  }
}
";
    let (_project, mut harness, id) = opened_with(text, WIDE);
    delete(&mut harness, &id, &["x"]);
    assert!(
        harness.query_by_label("Cancel").is_none(),
        "nothing is asked"
    );
    passes(&harness, &id);
    assert_eq!(steps(&harness, &id), ["begin", "a", "b"]);
}

// Criterion 6 ----------------------------------------------------------------

#[test]
fn diag06e_criterion_6() {
    let (_project, mut harness, id) = opened();
    refused(
        &mut harness,
        &id,
        &["begin"],
        "A scenario starts at Start. Delete its steps first.",
    );
    let (_project, mut harness, id) =
        opened_with("scenario \"Alone\" {\n  begin = start\n}\n", WIDE);
    delete(&mut harness, &id, &["begin"]);
    passes(&harness, &id);
    assert!(handles(&harness, &id).is_empty(), "Start alone is deleted");
}

// Criterion 7 ----------------------------------------------------------------

#[test]
fn diag06e_criterion_7() {
    let (_project, mut harness, id) = opened();
    let before = source_of(&harness, &id);
    delete(&mut harness, &id, &["paid"]);
    choose(&mut harness, "Delete");
    harness.step();
    let after = source_of(&harness, &id);
    assert!(!after.contains("paid = if"), "the if is deleted:\n{after}");
    assert!(
        !after.contains("Partial refunds"),
        "its free comment goes with its braces"
    );
    press(&mut harness, egui::Modifiers::COMMAND, egui::Key::Z);
    assert_eq!(
        source_of(&harness, &id),
        before,
        "Undo gives the file back byte for byte"
    );
    press(
        &mut harness,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    assert_eq!(
        source_of(&harness, &id),
        after,
        "Redo gives the edited file back byte for byte"
    );
}

/// The confirmation is bound to what the question named: when a statement
/// enters the braces between the question and *Delete*, the question is
/// asked again with the new count and nothing is deleted.
#[test]
fn diag06e_criterion_5_a_changed_if_is_asked_about_again() {
    let (_project, mut harness, id) = opened();
    delete(&mut harness, &id, &["paid"]);
    assert!(
        harness.query_by_label(QUESTION).is_some(),
        "the question is asked"
    );
    // Meanwhile a step is written in the braces of `paid`.
    let grown = marked(
        &source_of(&harness, &id),
        "    logout = POST \"/logout\" {\n",
        "    audit = GET \"/audit\" {\n      sample 200 {}\n    }\n    logout = POST \"/logout\" {\n",
    );
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &grown, CODE)
        .unwrap();
    settle(&mut harness);
    assert_eq!(source_of(&harness, &id), grown);
    choose(&mut harness, "Delete");
    harness.step();
    assert_eq!(
        source_of(&harness, &id),
        grown,
        "nothing is deleted without asking again"
    );
    assert!(
        harness
            .query_by_label("Delete `paid` and the 5 steps inside it?")
            .is_some(),
        "the question is asked again with the new count"
    );
    choose(&mut harness, "Delete");
    harness.step();
    passes(&harness, &id);
    let after = source_of(&harness, &id);
    assert!(
        !after.contains("paid = if") && !after.contains("audit"),
        "{after}"
    );
}
