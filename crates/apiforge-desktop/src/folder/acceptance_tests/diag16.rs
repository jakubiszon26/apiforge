//! DIAG-16 (#550): *Choose a response field* takes a typed path. What is
//! typed in the dialog's search and is not the path of a listed field is a
//! row *Use "…"* after the fields; Return takes the first row, a click takes
//! the row clicked, and the dialog closes. A text the language does not
//! accept as a path (`items[`) is the same row, disabled, with the reason,
//! and Return takes nothing. The hints stop promising a path can be typed
//! where no row takes it, and the line under the title says *No sample yet*
//! where the step has neither a sample nor a run.
//!
//! On the demo shop with a check `cart_id` that reads `cart`, at 1600 × 1000
//! unless a criterion says otherwise.
use super::lay23::select;
use super::*;
use egui_kittest::kittest::{NodeT, Queryable};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// `cart` has no sample and has not run.
const NO_SAMPLE: &str = r#"scenario "Typed path" {
  begin = start
  cart = POST "/carts" {
    body { productId: 123 }
  }
  cart_id = assert cart.body == "c-1"
}
"#;

/// `cart` with a sample.
const WITH_SAMPLE: &str = r#"scenario "Typed path" {
  begin = start
  cart = POST "/carts" {
    body { productId: 123 }
    sample 201 { id: "c-1", total: 172.5, items: [{ sku: "coffee", price: 12.5 }] }
  }
  cart_id = assert cart.body == "c-1"
}
"#;

/// The check's *Expected value* reads `cart` too.
const EXPECTED: &str = r#"scenario "Typed path" {
  begin = start
  cart = POST "/carts" {
    body { productId: 123 }
  }
  cart_id = assert cart.status == cart.body
}
"#;

const ACTUAL: &str = "Actual value";
const EXPECTED_VALUE: &str = "Expected value";
const TITLE: &str = "Choose a response field";
const NO_SAMPLE_HINT: &str = "Run the request or add a sample response to list its fields.";
const NO_MATCH_HINT: &str = "No field of the sample matches.";
const NEXT_RUN: &str = "A typed path · checked against the response on the next run";
const NOT_IN_SAMPLE: &str = "A typed path · not in the sample";
const NOT_A_PATH: &str = "Not a path: brackets hold a number or a quoted name, as in items[0]";

/// `text` as the demo shop's scenario, open on the board at `size`, the
/// check `cart_id` selected.
fn window(text: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), text).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = folder(&harness).workbench.active().unwrap().to_owned();
    assert_eq!(source_of(&harness, &id), text, "opened as written");
    select(&mut harness, &id, &["cart_id"]);
    (project, harness, id)
}

fn step(harness: &mut FolderHarness) {
    settle(harness);
    harness.step();
    harness.step();
}

/// The text a node carries: its name, or the text of a label.
fn said(node: &egui_kittest::kittest::AccessKitNode<'_>) -> String {
    node.label().or_else(|| node.value()).unwrap_or_default()
}

fn has_text(harness: &FolderHarness, text: &str) -> bool {
    harness
        .query_all_by(|node| said(node) == text)
        .next()
        .is_some()
}

/// Whether any text in the window mentions typing a path.
fn promises_typing(harness: &FolderHarness) -> Option<String> {
    harness
        .query_all_by(|node| said(node).to_lowercase().contains("type a path"))
        .next()
        .map(|node| said(&node.accesskit_node()))
}

/// Opens *Choose a response field* from the *Response field* button of the
/// `port` editor: the first button ending in ▾ under its *Step*.
fn open_dialog(harness: &mut FolderHarness, port: &str) {
    let step_box = format!("{port} step");
    harness.get_by_label(&step_box).scroll_to_me();
    step(harness);
    let below = harness.get_by_label(&step_box).rect().max.y;
    let button = harness
        .query_all_by(|node| {
            said(node).ends_with('▾') && node.role() == egui::accesskit::Role::Button
        })
        .filter(|node| node.rect().min.y >= below)
        .min_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
        .expect("the Response field button");
    button.click();
    step(harness);
    assert!(has_text(harness, TITLE), "the dialog is open");
}

/// Types `text` into the dialog's search, which has the focus.
fn type_search(harness: &mut FolderHarness, text: &str) {
    harness.event(egui::Event::Text(text.to_owned()));
    step(harness);
}

fn press_return(harness: &mut FolderHarness) {
    harness.key_press(egui::Key::Enter);
    step(harness);
}

fn undo(harness: &mut FolderHarness) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    step(harness);
}

/// The row *Use "`text`"*: its name, whether it is enabled, its rectangle.
fn use_row(harness: &FolderHarness, text: &str) -> Option<(String, bool, egui::Rect)> {
    let name = format!("Use \"{text}\"");
    harness
        .query_all_by(|node| said(node).starts_with(&name))
        .next()
        .map(|node| {
            (
                said(&node.accesskit_node()),
                !node.accesskit_node().is_disabled(),
                node.rect(),
            )
        })
}

/// The rows of listed fields, top to bottom: `(path, rectangle)`.
fn field_rows(harness: &FolderHarness) -> Vec<(String, egui::Rect)> {
    let mut rows: Vec<(String, egui::Rect)> = harness
        .query_all_by(|node| {
            node.role() == egui::accesskit::Role::Button
                && said(node).contains(" — ")
                && !said(node).starts_with("Use \"")
        })
        .map(|node| {
            let label = said(&node.accesskit_node());
            (label.split(" — ").next().unwrap().to_owned(), node.rect())
        })
        .collect();
    rows.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
    rows
}

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
    step(harness);
}

fn can_undo(harness: &FolderHarness, id: &str) -> bool {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .can_undo()
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn diag16_return_takes_a_typed_path_with_no_sample_in_one_undo_step() {
    let (_project, mut harness, id) = window(NO_SAMPLE, WIDE);
    assert!(!can_undo(&harness, &id), "nothing to undo before");
    open_dialog(&mut harness, ACTUAL);
    type_search(&mut harness, "id");
    let (name, enabled, _) = use_row(&harness, "id").expect("a row Use \"id\"");
    assert!(enabled, "the row can be taken: {name}");
    assert!(name.contains(NEXT_RUN), "the row says what it is: {name}");
    press_return(&mut harness);
    assert!(!has_text(&harness, TITLE), "the dialog closes");
    assert_eq!(
        source_of(&harness, &id),
        NO_SAMPLE.replace("assert cart.body ==", "assert cart.body.id =="),
        "the reference's path is `id`"
    );
    undo(&mut harness);
    assert_eq!(source_of(&harness, &id), NO_SAMPLE, "one undo step");
    assert!(!can_undo(&harness, &id), "and only one");
}

#[test]
fn diag16_a_click_takes_a_typed_path() {
    let (_project, mut harness, id) = window(NO_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    type_search(&mut harness, "id");
    let (_, _, rect) = use_row(&harness, "id").expect("a row Use \"id\"");
    click_at(&mut harness, rect.center());
    assert!(!has_text(&harness, TITLE), "the dialog closes");
    assert_eq!(
        source_of(&harness, &id),
        NO_SAMPLE.replace("assert cart.body ==", "assert cart.body.id ==")
    );
    undo(&mut harness);
    assert_eq!(source_of(&harness, &id), NO_SAMPLE, "one undo step");
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag16_the_path_of_a_listed_field_shows_the_field_and_no_use_row() {
    let (_project, mut harness, _id) = window(WITH_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    type_search(&mut harness, "total");
    let rows = field_rows(&harness);
    assert!(
        rows.iter().any(|(path, _)| path == "total"),
        "the field total is listed: {rows:?}"
    );
    assert_eq!(use_row(&harness, "total"), None, "no Use row");
}

#[test]
fn diag16_other_matches_come_first_the_use_row_last_and_return_takes_the_first() {
    let (_project, mut harness, id) = window(WITH_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    type_search(&mut harness, "price");
    let rows = field_rows(&harness);
    assert!(
        rows.iter().any(|(path, _)| path == "items[0].price"),
        "items[0].price matches: {rows:?}"
    );
    let (_, enabled, use_rect) = use_row(&harness, "price").expect("a row Use \"price\"");
    assert!(enabled);
    assert!(
        rows.iter()
            .all(|(_, rect)| rect.max.y <= use_rect.min.y + 0.5),
        "the Use row is last: {rows:?} then {use_rect:?}"
    );
    let first = rows[0].0.clone();
    press_return(&mut harness);
    assert!(!has_text(&harness, TITLE), "the dialog closes");
    assert_eq!(
        source_of(&harness, &id),
        WITH_SAMPLE.replace(
            "assert cart.body ==",
            &format!("assert cart.body.{first} ==")
        ),
        "Return takes the first row, {first}"
    );
}

// Criterion 3 ----------------------------------------------------------------

#[test]
fn diag16_a_text_that_is_not_a_path_is_a_disabled_row_with_the_reason() {
    let (_project, mut harness, id) = window(NO_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    type_search(&mut harness, "items[");
    let (name, enabled, _) = use_row(&harness, "items[").expect("a row Use \"items[\"");
    assert!(!enabled, "the row is disabled: {name}");
    assert!(name.contains(NOT_A_PATH), "it says why: {name}");
    press_return(&mut harness);
    assert!(
        has_text(&harness, TITLE),
        "Return takes nothing: the dialog stays"
    );
    assert_eq!(source_of(&harness, &id), NO_SAMPLE, "nothing is written");
    assert!(!can_undo(&harness, &id), "nothing to undo");
}

// Criterion 4 ----------------------------------------------------------------

/// A path the sample does not have is taken. DIAG-05 (#374) is still open:
/// no diagnostic marks such a path today, so the document carries none,
/// and the dialog's row is what says it is not in the sample.
#[test]
fn diag16_a_typed_path_the_sample_does_not_have_is_taken() {
    let (_project, mut harness, id) = window(WITH_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    type_search(&mut harness, "coupon");
    let (name, enabled, _) = use_row(&harness, "coupon").expect("a row Use \"coupon\"");
    assert!(enabled, "{name}");
    assert!(name.contains(NOT_IN_SAMPLE), "it says so: {name}");
    press_return(&mut harness);
    let expected = WITH_SAMPLE.replace("assert cart.body ==", "assert cart.body.coupon ==");
    assert_eq!(source_of(&harness, &id), expected, "taken");
    let session = folder(&harness).session.as_ref().unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(
        document
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>(),
        Vec::<String>::new(),
        "no diagnostic marks the path today (DIAG-05, #374)"
    );
}

// Criterion 5 ----------------------------------------------------------------

#[test]
fn diag16_the_hints_do_not_promise_typing() {
    let (_project, mut harness, _id) = window(NO_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    assert!(
        has_text(&harness, NO_SAMPLE_HINT),
        "the hint with no sample"
    );
    assert_eq!(promises_typing(&harness), None);

    let (_project, mut harness, _id) = window(WITH_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    type_search(&mut harness, "zzz");
    assert!(has_text(&harness, NO_MATCH_HINT), "the hint with no match");
    assert_eq!(promises_typing(&harness), None);
}

// Criterion 6 ----------------------------------------------------------------

#[test]
fn diag16_the_expected_value_picker_takes_a_typed_path() {
    let (_project, mut harness, id) = window(EXPECTED, WIDE);
    open_dialog(&mut harness, EXPECTED_VALUE);
    type_search(&mut harness, "id");
    assert!(use_row(&harness, "id").is_some_and(|(_, enabled, _)| enabled));
    press_return(&mut harness);
    assert!(!has_text(&harness, TITLE), "the dialog closes");
    assert_eq!(
        source_of(&harness, &id),
        EXPECTED.replace("== cart.body\n", "== cart.body.id\n")
    );
    undo(&mut harness);
    assert_eq!(source_of(&harness, &id), EXPECTED, "one undo step");
}

#[test]
fn diag16_the_expected_value_picker_refuses_what_is_not_a_path() {
    let (_project, mut harness, id) = window(EXPECTED, WIDE);
    open_dialog(&mut harness, EXPECTED_VALUE);
    type_search(&mut harness, "items[");
    assert!(use_row(&harness, "items[").is_some_and(|(_, enabled, _)| !enabled));
    press_return(&mut harness);
    assert!(has_text(&harness, TITLE));
    assert_eq!(source_of(&harness, &id), EXPECTED);
}

// Criterion 7 ----------------------------------------------------------------

#[test]
fn diag16_the_line_under_the_title_says_no_sample_yet() {
    let (_project, mut harness, _id) = window(NO_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    assert!(has_text(&harness, "No sample yet"));
    assert!(!has_text(&harness, "Sample data"), "not Sample data");

    let (_project, mut harness, _id) = window(WITH_SAMPLE, WIDE);
    open_dialog(&mut harness, ACTUAL);
    assert!(has_text(&harness, "Sample data"), "a sample is Sample data");
}

// Criterion 8 ----------------------------------------------------------------

/// The dialog with `id` typed and no sample, and with `price` typed on the
/// sample, at 960 × 540 and 1600 × 1000, against the board's lower row.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag16_typed_path() {
    for (size, name) in [(crate::testing::MINIMUM, "960"), (WIDE, "1600")] {
        for (text, typed, case) in [
            (NO_SAMPLE, "id", "no-sample"),
            (WITH_SAMPLE, "price", "sample"),
            (NO_SAMPLE, "items[", "not-a-path"),
        ] {
            let (_project, mut harness, _id) = window(text, size);
            open_dialog(&mut harness, ACTUAL);
            type_search(&mut harness, typed);
            crate::testing::capture(&mut harness, &format!("diag16-{case}-{name}"), None);
        }
    }
}
