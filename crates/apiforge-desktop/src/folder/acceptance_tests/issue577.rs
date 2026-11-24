//! #577: Escape on an open suggestion list closes the list only. The field
//! keeps the focus and its text and the step stays selected, so the side panel
//! stays; a second Escape closes the panel as before. The list is one for the
//! one-line fields, the request body and the Code view.
use super::diag06a::{graph, id_of, shop_with};
use super::project_interaction_tests::reveal;
use super::*;
use egui_kittest::kittest::NodeT;

const URL: &str = "URL or path";
const BODY: &str = "Request body";
const CODE: &str = "Contents of";

const COMPLETION: &str = r#"scenario "Completion" {
  begin = start
  createCart = POST "/carts" {
    sample 201 { id: "c-1" }
  }
  readCart = GET "/carts" {
    sample 200 {}
  }
}
"#;

fn select(harness: &mut FolderHarness, id: &str, handle: &str) -> String {
    let node = id_of(&graph(harness, id), handle);
    app(harness)
        .selections
        .insert(id.to_owned(), vec![node.clone()]);
    settle(harness);
    harness.step();
    node
}

/// Whether a suggestion row `createCart` is open below the top of `field`.
/// The rows of the body and the code lie over the field, so only what is not
/// drawn elsewhere counts: a card and an outline row are left of the panel.
fn list_open(harness: &FolderHarness, field: &str) -> bool {
    let field = harness.get_by_label(field).rect();
    harness
        .query_all_by_label_contains("createCart")
        .any(|node| {
            let rect = node.rect();
            rect.top() > field.top() + 2.0 && rect.left() >= field.left() - 2.0
        })
}

fn value(harness: &FolderHarness, field: &str) -> String {
    harness
        .get_by_label(field)
        .accesskit_node()
        .value()
        .unwrap_or_default()
}

/// `readCart` selected and `text` typed in place of what `field` held, with
/// the suggestion list open.
fn typed(field: &str, text: &str) -> (tempfile::TempDir, FolderHarness, String, String) {
    let (project, mut harness, id) = shop_with(COMPLETION);
    let read = select(&mut harness, &id, "readCart");
    reveal(&mut harness, field);
    harness.get_by_label(field).focus();
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.key_press(egui::Key::Backspace);
    harness.get_by_label(field).type_text(text);
    settle(&mut harness);
    harness.step();
    assert!(list_open(&harness, field), "the suggestion list is open");
    (project, harness, id, read)
}

fn press(harness: &mut FolderHarness, key: egui::Key) {
    harness.key_press(key);
    settle(harness);
    harness.step();
}

/// The list is gone; the step, the focus and the text stay.
fn assert_only_the_list_closed(
    harness: &FolderHarness,
    id: &str,
    read: &str,
    field: &str,
    text: &str,
) {
    assert_eq!(
        folder(harness).selections.get(id),
        Some(&vec![read.to_owned()]),
        "the first Escape leaves the step selected"
    );
    assert!(
        harness.query_by_label(field).is_some(),
        "the side panel still shows the request"
    );
    assert!(
        !list_open(harness, field),
        "the first Escape closes the list"
    );
    assert!(
        harness.get_by_label(field).is_focused(),
        "the field keeps the focus"
    );
    assert_eq!(value(harness, field), text, "the field keeps its text");
}

/// The panel is closed, as Escape in a field with no list does.
fn assert_panel_closed(harness: &FolderHarness, id: &str, field: &str) {
    assert!(
        folder(harness)
            .selections
            .get(id)
            .is_none_or(|selection| selection.is_empty()),
        "Escape with no list open clears the selection"
    );
    assert!(
        harness.query_by_label(field).is_none(),
        "the side panel no longer shows the request"
    );
}

/// Criterion 1.
#[test]
fn issue577_escape_closes_the_suggestion_list_and_keeps_the_side_panel() {
    let (_project, mut harness, id, read) = typed(URL, "/carts/{{create");
    press(&mut harness, egui::Key::Escape);
    assert_only_the_list_closed(&harness, &id, &read, URL, "/carts/{{create");
}

/// Criterion 2.
#[test]
fn issue577_a_second_escape_closes_the_side_panel() {
    let (_project, mut harness, id, read) = typed(URL, "/carts/{{create");
    press(&mut harness, egui::Key::Escape);
    assert_only_the_list_closed(&harness, &id, &read, URL, "/carts/{{create");
    press(&mut harness, egui::Key::Escape);
    assert_panel_closed(&harness, &id, URL);
}

/// Criterion 3, the request body.
#[test]
fn issue577_escape_in_the_body_keeps_the_panel() {
    let (_project, mut harness, id, read) = typed(BODY, "{{create");
    press(&mut harness, egui::Key::Escape);
    assert_only_the_list_closed(&harness, &id, &read, BODY, "{{create");
    press(&mut harness, egui::Key::Escape);
    assert_panel_closed(&harness, &id, BODY);
}

/// Criterion 3, the Code view: the list closes and the caret stays, so typing
/// goes on where it was; with no list open Escape leaves the editor as before.
#[test]
fn issue577_escape_in_the_code_view_keeps_the_caret() {
    let (_project, mut harness, id) = shop_with(COMPLETION);
    choose(&mut harness, "Code");
    let editor = editor_id(&id);
    harness.get_by_label_contains(CODE).focus();
    harness.step();
    let mut state = egui::text_edit::TextEditState::load(&harness.ctx, editor).unwrap_or_default();
    let end = COMPLETION.trim_end().chars().count() - 1;
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(end),
        )));
    state.store(&harness.ctx, editor);
    harness.step();
    harness
        .get_by_label_contains(CODE)
        .type_text("total = crea");
    settle(&mut harness);
    assert!(app(&mut harness).code.popup_open(), "the list is open");

    press(&mut harness, egui::Key::Escape);
    assert!(
        !app(&mut harness).code.popup_open(),
        "Escape closes the list"
    );
    assert!(
        harness.get_by_label_contains(CODE).is_focused(),
        "the editor keeps the focus"
    );
    harness.get_by_label_contains(CODE).type_text("X");
    settle(&mut harness);
    assert!(
        source_of(&harness, &id).contains("total = creaX"),
        "the caret stays: {}",
        source_of(&harness, &id)
    );

    press(&mut harness, egui::Key::Escape);
    assert!(
        !harness.get_by_label_contains(CODE).is_focused(),
        "Escape with no list open leaves the editor"
    );
}

/// The control for what an open list must not change in its field: the arrows
/// choose and Enter inserts in the one-line field, Tab inserts in the body.
#[test]
fn issue577_the_other_keys_of_the_list_stay() {
    let (_project, mut harness, _id, _read) = typed(URL, "/carts/{{create");
    for key in [egui::Key::ArrowDown, egui::Key::ArrowUp] {
        press(&mut harness, key);
        assert!(list_open(&harness, URL), "{key:?} moves in the list");
        assert!(
            harness.get_by_label(URL).is_focused(),
            "{key:?} keeps the focus"
        );
    }
    press(&mut harness, egui::Key::Enter);
    assert!(
        value(&harness, URL).starts_with("/carts/{{createCart"),
        "Enter inserts: {:?}",
        value(&harness, URL)
    );

    let (_project, mut harness, _id, _read) = typed(BODY, "{{create");
    press(&mut harness, egui::Key::Tab);
    assert!(
        value(&harness, BODY).starts_with("{{createCart"),
        "Tab inserts: {:?}",
        value(&harness, BODY)
    );
    assert!(
        harness.get_by_label(BODY).is_focused(),
        "Tab keeps the focus"
    );
}

/// The control for the one-line field, where Tab goes to the next control with
/// the list open or closed.
#[test]
fn issue577_tab_still_leaves_a_one_line_field() {
    let (_project, mut harness, _id, _read) = typed(URL, "/carts/{{create");
    press(&mut harness, egui::Key::Tab);
    assert!(!harness.get_by_label(URL).is_focused(), "Tab moves on");
    assert_eq!(value(&harness, URL), "/carts/{{create");
}

/// The control: Escape in a field with no list open closes the panel at once.
#[test]
fn issue577_escape_with_no_list_open_closes_the_panel_at_once() {
    let (_project, mut harness, id) = shop_with(COMPLETION);
    select(&mut harness, &id, "readCart");
    for field in [URL, BODY] {
        select(&mut harness, &id, "readCart");
        reveal(&mut harness, field);
        harness.get_by_label(field).focus();
        harness.step();
        assert!(harness.get_by_label(field).is_focused(), "{field}");
        press(&mut harness, egui::Key::Escape);
        assert_panel_closed(&harness, &id, field);
    }
}
