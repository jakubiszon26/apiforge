//! Reproductions for defects found in AB-01 rounds 2 to 4 (#537), no fixes.
//! #577 (Escape on a suggestion list also closes the side panel) and #578 (a
//! warning leaves Problems after an edit on the diagram) fail as reported
//! and are ignored under their issues until the fix. #579 (the wheel does
//! not scroll the side panel over a field) does not reproduce in the
//! harness: its three tests pass and stay as controls, so the defect is
//! looked for in the native window, not in the panel's widgets.
use super::diag06a::{graph, id_of, shop_with};
use super::project_interaction_tests::{replace_text, reveal};
use super::*;
use apiforge_script::FAR_READ;

const URL: &str = "URL or path";

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

/// Whether a suggestion row `createCart` is open under the URL field.
fn list_open(harness: &FolderHarness) -> bool {
    let field = harness.get_by_label(URL).rect();
    harness
        .query_all_by_label_contains("createCart")
        .any(|node| {
            let rect = node.rect();
            rect.top() >= field.bottom() - 2.0 && rect.left() >= field.left() - 2.0
        })
}

#[test]
#[ignore = "#577: Escape on a suggestion list also closes the side panel"]
fn issue577_escape_closes_the_suggestion_list_and_keeps_the_side_panel() {
    let (_project, mut harness, id) = shop_with(COMPLETION);
    let read = select(&mut harness, &id, "readCart");
    reveal(&mut harness, URL);
    harness.get_by_label(URL).focus();
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.key_press(egui::Key::Backspace);
    harness.get_by_label(URL).type_text("/carts/{{create");
    settle(&mut harness);
    harness.step();
    assert!(list_open(&harness), "the suggestion list is open");

    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    harness.step();

    assert_eq!(
        folder(&harness).selections.get(&id),
        Some(&vec![read]),
        "the first Escape leaves the step selected"
    );
    assert!(
        harness.query_by_label(URL).is_some(),
        "the side panel still shows the request"
    );
    assert!(!list_open(&harness), "the first Escape closes the list");
}

fn far_reads(harness: &FolderHarness, id: &str) -> usize {
    folder(harness)
        .snapshot(id)
        .unwrap()
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.message == FAR_READ)
        .count()
}

#[test]
#[ignore = "#578: a warning leaves Problems after an edit on the diagram"]
fn issue578_far_read_warnings_stay_after_an_edit_in_the_side_panel() {
    let source = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/far-value.afs"),
    )
    .unwrap();
    let (_project, mut harness, id) = shop_with(&source);
    let before = far_reads(&harness, &id);
    assert!(before > 0, "the fixture opens with far-read warnings");

    select(&mut harness, &id, "catalogue");
    replace_text(&mut harness, URL, "/products/all");
    settle(&mut harness);
    harness.step();
    assert!(
        source_of(&harness, &id).contains("GET \"/products/all\""),
        "the edit is written"
    );

    assert_eq!(
        far_reads(&harness, &id),
        before,
        "the lines with far reads are untouched, so their warnings stay"
    );
}

const TALL: &str = r#"scenario "Tall panel" {
  begin = start
  create = POST "/carts" {
    headers { Accept: "application/json", Trace: "1", Tenant: "t-1" }
    query { a: 1, b: 2, c: 3 }
    body { one: 1, two: 2, three: 3, four: 4, five: 5, six: 6 }
    sample 201 { id: "c-1", one: 1, two: 2, three: 3, four: 4 }
  }
}
"#;

/// One wheel step towards the panel's top with the pointer at `at`.
fn wheel_up(harness: &mut FolderHarness, at: egui::Pos2) {
    harness.hover_at(at);
    // The pointer rests first, as a person's does: what a field shows on
    // hover is up when the wheel turns.
    for _ in 0..12 {
        harness.step();
    }
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 120.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    settle(harness);
    harness.step();
}

/// The side panel's offset before and after one wheel step with the pointer
/// on the field `label`, scrolled into view first so the panel has room to
/// move back up.
fn wheel_over(label: &str, focus: bool) -> (f32, f32) {
    let (_project, mut harness, id) = shop_with(TALL);
    harness.set_size(egui::vec2(1280.0, 800.0));
    settle(&mut harness);
    select(&mut harness, &id, "create");
    reveal(&mut harness, label);
    harness.step();
    if focus {
        harness.get_by_label(label).focus();
        settle(&mut harness);
        harness.step();
    }
    let before = folder(&harness).debug_inspector_offset();
    assert!(before > 0.0, "{label} is below the first screen of the panel");
    let field = harness.get_by_label(label).rect();
    assert!(
        field.top() > 98.0 && field.bottom() < 800.0,
        "{label} is in view: {field:?}"
    );
    wheel_up(&mut harness, field.center());
    (before, folder(&harness).debug_inspector_offset())
}

#[test]
fn issue579_headless_the_wheel_over_a_single_line_field_scrolls_the_side_panel() {
    let (before, after) = wheel_over("Headers: Trace", false);
    assert!(after < before, "pointer over a one-line field: {before} -> {after}");
}

#[test]
fn issue579_headless_the_wheel_over_the_body_scrolls_the_side_panel() {
    let (before, after) = wheel_over("Request body", false);
    assert!(after < before, "pointer over the body: {before} -> {after}");
}

#[test]
fn issue579_headless_the_wheel_over_the_focused_body_scrolls_the_side_panel() {
    let (before, after) = wheel_over("Request body", true);
    assert!(after < before, "pointer over the focused body: {before} -> {after}");
}
