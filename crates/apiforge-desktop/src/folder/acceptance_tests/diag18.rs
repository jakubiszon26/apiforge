//! DIAG-18 (#552): a click on a row of the completion list inserts it, as
//! Return on the highlighted row does, in a request's *URL or path* field
//! (inside `{{ }}`) and in the Code view, which share the list. Before the
//! fix the click took the keyboard focus from the editor first, which closed
//! the list without inserting anything.
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

/// A press and a release on `at`, each in a frame of its own, as a mouse
/// gives them.
fn click_at(harness: &mut FolderHarness, at: egui::Pos2) {
    harness.hover_at(at);
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    settle(harness);
    harness.step();
}

#[test]
fn diag18_a_click_on_a_suggestion_inserts_it() {
    let (_project, mut harness, _id, row) = typed();
    click_at(&mut harness, row.center());
    assert!(
        url_text(&harness).starts_with("/carts/{{createCart"),
        "a click inserts the suggestion, as Return does: {:?}",
        url_text(&harness)
    );
    assert!(
        harness.get_by_label(URL).is_focused(),
        "the field keeps the keyboard"
    );
}

/// The same list in the Code view: a click on its `createCart` row inserts
/// the name where the word was being typed.
#[test]
fn diag18_a_click_on_a_suggestion_in_code_inserts_it() {
    let (_project, mut harness, id) = shop_with(SCENARIO);
    harness.get_by_label("Code").click();
    settle(&mut harness);
    harness.get_by_label_contains("Contents of").focus();
    harness.step();
    let line = "sample 200 {}";
    let end = SCENARIO.find(line).unwrap() + line.len();
    app(&mut harness).code.reveal(end);
    settle(&mut harness);
    harness.key_press(egui::Key::Enter);
    harness.event(egui::Event::Text("total = create".into()));
    settle(&mut harness);
    harness.step();
    let popup = folder(&harness)
        .code
        .popup_rect()
        .expect("the completion list opens");
    let row = harness
        .query_all_by_label_contains("createCart")
        .map(|node| node.rect())
        .find(|rect| popup.contains(rect.center()))
        .unwrap_or_else(|| panic!("the completion list {popup:?} offers createCart"));

    click_at(&mut harness, row.center());
    let source = source_of(&harness, &id);
    assert!(
        source.contains("total = createCart"),
        "a click inserts the suggestion: {source}"
    );
    assert!(
        !folder(&harness).code.popup_open(),
        "the list closes once a suggestion is chosen"
    );
    assert!(
        harness.get_by_label_contains("Contents of").is_focused(),
        "the editor keeps the keyboard"
    );
}
