//! DIAG-18 (#552) reproduction: in a request's *URL or path* field the
//! completion list opens inside `{{ }}`; Return on the highlighted row
//! inserts it, but a click on the same row closes the list and inserts
//! nothing.
use super::diag06a::{graph, id_of, shop_with};
use super::project_interaction_tests::reveal;
use super::*;
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = r#"scenario "Completion" {
  begin = start
  createCart = POST "/carts" {
    sample 201 { id: "c-1" }
  }
  readCart = GET "/carts" {
    sample 200 {}
  }
}
"#;

const URL: &str = "URL or path";

/// The window with `readCart` selected and `/carts/{{create` typed into its
/// URL, the completion list open; returns the list's row for `createCart`.
fn typed() -> (tempfile::TempDir, FolderHarness, String, egui::Rect) {
    let (project, mut harness, id) = shop_with(SCENARIO);
    let read = id_of(&graph(&harness, &id), "readCart");
    app(&mut harness).selections.insert(id.clone(), vec![read]);
    settle(&mut harness);
    harness.step();
    reveal(&mut harness, URL);
    harness.get_by_label(URL).focus();
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.key_press(egui::Key::Backspace);
    harness.get_by_label(URL).type_text("/carts/{{create");
    settle(&mut harness);
    harness.step();
    let field = harness.get_by_label(URL).rect();
    // The list opens under the field: its row is the `createCart` below it,
    // in the Inspector, not the card on the board.
    let row = harness
        .query_all_by_label_contains("createCart")
        .map(|node| node.rect())
        .find(|rect| rect.top() >= field.bottom() - 2.0 && rect.left() >= field.left() - 2.0)
        .unwrap_or_else(|| {
            panic!(
                "the completion list offers createCart: {:?}",
                harness
                    .query_all_by_label_contains("createCart")
                    .map(|node| (node.accesskit_node().label(), node.rect()))
                    .collect::<Vec<_>>()
            )
        });
    (project, harness, id, row)
}

fn url_text(harness: &FolderHarness) -> String {
    harness
        .get_by_label(URL)
        .accesskit_node()
        .value()
        .unwrap_or_default()
}

/// The control: Return on the highlighted row inserts it.
#[test]
fn diag18_return_on_a_suggestion_inserts_it() {
    let (_project, mut harness, _id, _row) = typed();
    harness.key_press(egui::Key::Enter);
    settle(&mut harness);
    harness.step();
    assert!(
        url_text(&harness).starts_with("/carts/{{createCart"),
        "Return inserts the suggestion: {:?}",
        url_text(&harness)
    );
}

#[test]
#[ignore = "DIAG-18 #552: reproduction"]
fn diag18_a_click_on_a_suggestion_inserts_it() {
    let (_project, mut harness, _id, row) = typed();
    harness.hover_at(row.center());
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: row.center(),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: row.center(),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    settle(&mut harness);
    harness.step();
    assert!(
        url_text(&harness).starts_with("/carts/{{createCart"),
        "a click inserts the suggestion, as Return does: {:?}",
        url_text(&harness)
    );
}
