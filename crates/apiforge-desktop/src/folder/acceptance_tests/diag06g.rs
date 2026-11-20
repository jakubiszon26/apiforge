//! DIAG-06g (#502), criterion 1 at the window: on `shop-30`, the `if` `paid`
//! deleted on the board, with a free comment, a blank line and attributes in
//! a non-canonical order in its braces. Ctrl+Z gives the file back byte for
//! byte; Ctrl+Shift+Z gives the edited file back byte for byte.
use super::diag07::{graph, id_of};
use super::*;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";

/// Replaces `from`, which `text` holds exactly once, by `to`.
fn marked(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "`{from}` is written once");
    text.replacen(from, to, 1)
}

/// `shop-30` with a free comment closing the braces of `paid`, a blank line
/// in them, and `refund`'s sample written before its headers.
fn shop_text() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-30.afs");
    let text = fs::read_to_string(path).unwrap();
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

/// The marked `shop-30` saved in the demo shop and open on the board.
fn opened() -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), shop_text()).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    assert_eq!(source_of(&harness, &id), shop_text(), "opened as written");
    (project, harness, id)
}

fn press(harness: &mut FolderHarness, modifiers: egui::Modifiers, key: egui::Key) {
    harness.key_press_modifiers(modifiers, key);
    settle(harness);
    harness.step();
}

#[test]
fn diag06g_criterion_1_deleting_an_if_in_the_window() {
    let (_project, mut harness, id) = opened();
    let before = source_of(&harness, &id);

    let paid = id_of(&graph(&harness, &id), "paid");
    app(&mut harness).selections.insert(id.clone(), vec![paid]);
    settle(&mut harness);
    harness.step();
    press(&mut harness, egui::Modifiers::NONE, egui::Key::Delete);
    // Deleting an `if` with steps in its braces asks once (DIAG-06e).
    choose(&mut harness, "Delete");
    // A whole-file print asks before dropping the free comment.
    if harness.query_by_label("Continue").is_some() {
        choose(&mut harness, "Continue");
        settle(&mut harness);
        harness.step();
    }
    let after = source_of(&harness, &id);
    assert!(!after.contains("paid = if"), "the if is deleted:\n{after}");

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
