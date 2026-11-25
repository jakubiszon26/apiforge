//! REP-07c (#621): Run results open on a failed check with its title, its
//! request, the expected and the actual value and the marked field in view
//! at the default height; the selected row is in view in the list, a hidden
//! row is never the selection, and a check's empty Outputs or Inputs are no
//! tabs. Decided by the owner on 2026-11-24 (P3, 350 / 215 px). On the
//! `labels` reference scenario (FIX-01), nothing dragged.
use super::*;
use crate::testing::{LABELS, MINIMUM, assert_within_window};
use apiforge_core::run::StepStatus;
use egui_kittest::kittest::NodeT;
use std::path::Path;

const LAPTOP: egui::Vec2 = egui::vec2(1280.0, 800.0);
const MAXIMISE: &str = "Maximise run results";
const RESTORE: &str = "Restore run results";
const GRIP: &str = "Resize run report";
const RESIZE_LIST: &str = "Resize the step list";
const COLLAPSE: &str = "Collapse checks";
const EXPAND: &str = "Expand checks";
/// The sentence the box and the *Failed* pill say.
const SENTENCE: &str = "The comparison failed.";
/// The request whose check fails, and that check's predicate.
const WEIGHT_31: &str = "Weight 31 kg";
const PREDICATE: &str = "error.field == \"weight\"";

/// `labels` with `weight_31`'s response naming the field `mass`: its check
/// `weight_31_field` fails, every other check passes.
fn failing() -> String {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/labels.afs"),
    )
    .unwrap();
    let failing = text.replacen(
        r#"field: "weight", message: "Weight must be at most 30 kg.""#,
        r#"field: "mass", message: "Weight must be at most 30 kg.""#,
        1,
    );
    assert_ne!(failing, text, "weight_31's sample names its field");
    failing
}

/// Checks that passed, failed, ended in an error and did not run.
const PLACES: &str = r#"scenario "Places" {
  on_error continue
  begin = start
  first = GET "https://shop.example.test/first" {
    sample 200 { size: 1, items: [] }
  }
  wrong = assert first.body.size == 2
  typed = assert first.body.size is "string"
  missing_field = assert first.body.nothing == 1
  unsent = GET "https://shop.example.test/{{first.body.items[0].id}}" {
    sample 200 {}
  }
  of_unsent = assert unsent.status == 200
}
"#;

/// A check that fails and stops the run before another of the same request.
const STOPPED: &str = r#"scenario "Stopped" {
  on_error stop
  begin = start
  first = GET "https://shop.example.test/first" {
    sample 200 { size: 1 }
  }
  wrong = assert first.body.size == 2
  stopped = assert first.body.size == 1
}
"#;

/// `text` as the project's only scenario, open on its board at `size`.
fn opened(size: egui::Vec2, text: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Label validation".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = apiforge_project::ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, LABELS, text)
        .unwrap();
    drop(session);
    let mut harness = crate::testing::folder_harness_in(directory.path(), size, Mode::Diagram);
    app(&mut harness).review_open(LABELS, None);
    settle(&mut harness);
    (directory, harness, id)
}

/// `text` after a simulated run, Run results as they open.
fn ran(size: egui::Vec2, text: &str) -> (tempfile::TempDir, FolderHarness, String, RunResult) {
    let (project, mut harness, id) = opened(size, text);
    choose(&mut harness, "Run simulation");
    let result = finish_run(&mut harness);
    (project, harness, id, result)
}

/// The step of `handle` in `result`.
fn step_id(result: &RunResult, handle: &str) -> String {
    result
        .steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .node_id
        .clone()
}

/// Chooses `nodes` on the board.
fn choose_on_board(harness: &mut FolderHarness, id: &str, nodes: Vec<String>) {
    app(harness).selections.insert(id.to_owned(), nodes);
    settle(harness);
}

/// The height the panel says it has.
fn height(harness: &FolderHarness) -> f32 {
    harness
        .get_by_label(GRIP)
        .accesskit_node()
        .numeric_value()
        .expect("the grip says the panel's height") as f32
}

/// The panel, without its least margin.
fn panel(harness: &FolderHarness) -> egui::Rect {
    let window = harness.ctx.content_rect();
    let top = harness.get_by_label(GRIP).rect().center().y - 8.0;
    egui::Rect::from_min_max(
        egui::pos2(window.left(), top),
        egui::pos2(window.right(), top + height(harness)),
    )
    .shrink(6.0)
}

/// Where the details start: right of the list's edge.
fn details(harness: &FolderHarness) -> f32 {
    harness.get_by_label(RESIZE_LIST).rect().right()
}

/// The texts of the details containing `text`.
fn texts(harness: &FolderHarness, text: &str) -> Vec<egui::Rect> {
    let left = details(harness);
    harness
        .query_all_by_label_contains(text)
        .map(|node| node.rect())
        .filter(|rect| rect.left() >= left - 1.0)
        .collect()
}

/// The texts of the details labelled exactly `text`.
fn exactly(harness: &FolderHarness, text: &str) -> Vec<egui::Rect> {
    let left = details(harness);
    harness
        .query_all_by_label(text)
        .map(|node| node.rect())
        .filter(|rect| rect.left() >= left - 1.0)
        .collect()
}

/// The rows of the step list: each row's accessible label, its rectangle
/// and whether it is selected.
fn rows(harness: &FolderHarness) -> Vec<(String, egui::Rect, bool)> {
    let right = harness.get_by_label(RESIZE_LIST).rect().left();
    harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter(|node| node.rect().right() <= right + 1.0)
        .filter_map(|node| {
            let label = node.accesskit_node().label()?.to_string();
            let selected = node.accesskit_node().is_selected() == Some(true);
            label
                .contains(" · ")
                .then(|| (label, node.rect(), selected))
        })
        .collect()
}

/// The label and the rectangle of the one selected row.
fn selected(harness: &FolderHarness) -> (String, egui::Rect) {
    let chosen: Vec<_> = rows(harness)
        .into_iter()
        .filter(|(_, _, selected)| *selected)
        .collect();
    assert_eq!(chosen.len(), 1, "one row is selected: {chosen:?}");
    (chosen[0].0.clone(), chosen[0].1)
}

/// The label of the row that contains `text`.
fn row(harness: &FolderHarness, text: &str) -> String {
    rows(harness)
        .into_iter()
        .map(|(label, ..)| label)
        .find(|label| label.contains(text))
        .unwrap_or_else(|| panic!("a row says {text:?}: {:?}", rows(harness)))
}

/// Clicks the row that contains `text`.
fn click_row(harness: &mut FolderHarness, text: &str) {
    let label = row(harness, text);
    harness.get_by_label(&label).scroll_to_me();
    settle(harness);
    harness.get_by_label(&label).click();
    settle(harness);
}

/// What the list shows of its rows: under its controls where they stand
/// above it, down to its end.
fn list(harness: &FolderHarness) -> egui::Rect {
    let edge = harness.get_by_label(RESIZE_LIST).rect();
    let controls = harness
        .query_by_label(COLLAPSE)
        .or(harness.query_by_label(EXPAND))
        .map(|node| node.rect())
        .filter(|rect| rect.right() <= edge.left());
    egui::Rect::from_min_max(
        egui::pos2(0.0, controls.map_or(edge.top(), |rect| rect.bottom())),
        edge.left_bottom(),
    )
}

/// The selected row says `text` and is whole inside the list.
fn assert_in_view(harness: &FolderHarness, text: &str, context: &str) {
    let (label, rect) = selected(harness);
    assert!(
        label.contains(text),
        "{context}: {label:?} is not {text:?}'s"
    );
    let list = list(harness);
    assert!(
        rect.top() >= list.top() - 0.5 && rect.bottom() <= list.bottom() + 0.5,
        "{context}: the selected row at {rect:?} is outside the list {list:?}"
    );
}

/// Scrolls the list to its first row, as a person does.
fn scroll_to_top(harness: &mut FolderHarness) {
    let at = list(harness).center();
    harness.hover_at(at);
    harness.step();
    for _ in 0..10 {
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 400.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    settle(harness);
}

/// The mark of the checked field's row in the details.
fn mark(harness: &FolderHarness) -> egui::Rect {
    let field = exactly(harness, "field")
        .into_iter()
        .next()
        .expect("the field's row");
    exactly(harness, "Checks of this field")
        .into_iter()
        .find(|rect| (rect.center().y - field.center().y).abs() < 6.0)
        .expect("the field's mark")
        .union(field)
}

/// Criterion 1: a failed check with its box does not say *The comparison
/// failed.*, which the box and the *Failed* pill say; every other message
/// stays.
#[test]
fn rep07c_criterion_1_the_box_says_it() {
    for size in [MINIMUM, LAPTOP, WIDE] {
        let (_project, harness, _, result) = ran(size, &failing());
        let failed = result
            .steps
            .iter()
            .find(|step| step.handle == "weight_31_field")
            .unwrap();
        assert_eq!(failed.status, StepStatus::Failed);
        assert_eq!(failed.message.as_deref(), Some(SENTENCE));
        assert!(selected(&harness).0.contains(PREDICATE), "{size:?}");
        assert!(
            texts(&harness, SENTENCE).is_empty(),
            "{size:?}: the box and the pill say it"
        );
        for text in ["Failed", "EXPECTED", "ACTUAL", "\"weight\"", "\"mass\""] {
            assert!(!exactly(&harness, text).is_empty(), "{size:?}: {text}");
        }
    }

    let (_project, mut harness, _, _) = ran(WIDE, PLACES);
    choose(&mut harness, MAXIMISE);
    click_row(&mut harness, " · wrong of ");
    assert!(texts(&harness, SENTENCE).is_empty());
    assert!(!exactly(&harness, "EXPECTED").is_empty());
    // What the box does not say stays, as it was before.
    for (row, message) in [
        (
            " · typed of ",
            "The comparison failed: expected string, actual number (1).",
        ),
        (
            " · missing_field of ",
            "A value required for the comparison is missing.",
        ),
        (
            " · of_unsent of ",
            "Did not run: it needs unsent, which failed.",
        ),
        ("unsent · Error", "Missing value for template value 1."),
    ] {
        click_row(&mut harness, row);
        assert_eq!(
            exactly(&harness, message).len(),
            1,
            "{row} keeps {message:?}"
        );
    }
}

/// Criterion 2: a new panel is 350 px high, 215 px in a compact window; at
/// 1280 × 800 and 1600 × 1000 the title, the request's line, the box and the
/// marked field are inside it, nothing scrolled; at 960 × 540 the title, the
/// request's line and the box's first side are, and the mark is scrolled to.
#[test]
fn rep07c_criterion_2_default_height() {
    for (size, default) in [(MINIMUM, 215.0), (LAPTOP, 350.0), (WIDE, 350.0)] {
        let (_project, mut harness, _, _) = ran(size, &failing());
        assert_eq!(height(&harness), default, "{size:?}");
        let panel = panel(&harness);
        assert!(
            harness.ctx.content_rect().bottom() - panel.bottom() < 20.0,
            "{size:?}: the panel ends the window"
        );
        let request = format!(
            "Check weight_31_field · of step 6 {WEIGHT_31} · POST /parcels/pc-001/labels → 422"
        );
        assert_within_window(&harness, &[PREDICATE, &request, "EXPECTED"]);
        let compact = size == MINIMUM;
        let mut shown = vec![PREDICATE, request.as_str(), "EXPECTED", "\"weight\""];
        if !compact {
            shown.extend([
                "ACTUAL",
                "\"mass\"",
                "Response of Weight 31 kg · the checked field is marked",
            ]);
        }
        for text in shown {
            // The response and the inputs write the values again, below.
            let rect = exactly(&harness, text)
                .into_iter()
                .min_by(|a, b| a.top().total_cmp(&b.top()))
                .unwrap_or_else(|| panic!("{size:?}: the details write {text:?}"));
            assert!(
                panel.contains_rect(rect),
                "{size:?}: {text:?} at {rect:?} is outside the panel {panel:?}"
            );
        }
        if !compact {
            let mark = mark(&harness);
            assert!(
                panel.contains_rect(mark),
                "{size:?}: the marked line at {mark:?} is outside the panel {panel:?}"
            );
            continue;
        }
        let field = exactly(&harness, "field")[0];
        harness
            .query_all_by_label("Checks of this field")
            .find(|node| (node.rect().center().y - field.center().y).abs() < 6.0)
            .expect("the field's mark")
            .scroll_to_me();
        settle(&mut harness);
        let mark = mark(&harness);
        assert!(
            panel.contains_rect(mark),
            "{size:?}: scrolled to, the marked line at {mark:?} is outside {panel:?}"
        );
    }
}

/// Criterion 3: a height the person dragged stays through a run, a
/// selection, and maximise and back.
#[test]
fn rep07c_criterion_3_a_dragged_height_stays() {
    for size in [MINIMUM, LAPTOP, WIDE] {
        let (_project, mut harness, id, result) = ran(size, &failing());
        let from = harness.get_by_label(GRIP).rect().center();
        super::lay16::drag(&mut harness, from, from + egui::vec2(0.0, -20.0));
        let dragged = height(&harness);
        let default = if size == MINIMUM { 215.0 } else { 350.0 };
        assert!(
            (dragged - default - 20.0).abs() < 2.0,
            "{size:?}: {default} → {dragged}"
        );
        choose(&mut harness, "Run simulation");
        finish_run(&mut harness);
        assert_eq!(height(&harness), dragged, "{size:?}: after a run");
        choose_on_board(&mut harness, &id, vec![step_id(&result, "weight_0")]);
        assert_eq!(height(&harness), dragged, "{size:?}: after a selection");
        choose(&mut harness, MAXIMISE);
        assert!(height(&harness) > dragged);
        choose(&mut harness, RESTORE);
        assert_eq!(height(&harness), dragged, "{size:?}: maximised and back");
    }
}

/// Criterion 4: the selected row is whole inside the list when the run
/// opens on its first failure, when *Failed* is switched on and off and
/// when a step is chosen on the board; a person's scroll is left alone.
#[test]
fn rep07c_criterion_4_the_selected_row_is_in_view() {
    for size in [MINIMUM, LAPTOP, WIDE] {
        let context = |what: &str| format!("{size:?}, {what}");
        let (_project, mut harness, id, result) = ran(size, &failing());
        assert_in_view(&harness, PREDICATE, &context("as the run opens"));

        scroll_to_top(&mut harness);
        let (_, away) = selected(&harness);
        assert!(
            away.bottom() > list(&harness).bottom() || size == WIDE,
            "{size:?}: scrolled away"
        );
        for _ in 0..4 {
            harness.step();
        }
        assert_eq!(selected(&harness).1, away, "{size:?}: the scroll stays");

        harness.get_by_label("Failed 1").click();
        settle(&mut harness);
        assert_eq!(rows(&harness).len(), 2);
        assert_in_view(&harness, PREDICATE, &context("Failed on"));
        harness
            .get_by_label(&format!("All {}", result.steps.len()))
            .click();
        settle(&mut harness);
        assert_in_view(&harness, PREDICATE, &context("Failed off"));

        let last = result.steps.last().unwrap();
        choose_on_board(&mut harness, &id, vec![last.node_id.clone()]);
        assert_in_view(&harness, &last.handle, &context("chosen on the board"));
        // Another run opens with its selected row in view too.
        scroll_to_top(&mut harness);
        choose_on_board(&mut harness, &id, Vec::new());
        choose(&mut harness, "Run simulation");
        finish_run(&mut harness);
        assert_in_view(&harness, PREDICATE, &context("a second run"));
    }
}

/// The details are the request's: its name is their title, and no check's
/// line is written.
fn assert_request(harness: &FolderHarness, context: &str) {
    let (label, _) = selected(harness);
    assert!(
        label.starts_with(&format!("{WEIGHT_31} · ")),
        "{context}: {label:?} is selected"
    );
    assert_eq!(exactly(harness, WEIGHT_31).len(), 1, "{context}: the title");
    assert!(texts(harness, " · of step ").is_empty(), "{context}");
}

/// Criterion 5: a check folded away leaves its request selected and its
/// details shown, by *Collapse checks*, by ▸ and by the keyboard, and opened
/// again the request stays; a row the list does not show has no details.
#[test]
fn rep07c_criterion_5_no_details_without_a_marked_row() {
    let fold = format!("Fold the checks of {WEIGHT_31}");
    let open = format!("Open the checks of {WEIGHT_31}");
    for size in [LAPTOP, WIDE] {
        let (_project, mut harness, id, result) = ran(size, &failing());
        choose(&mut harness, MAXIMISE);
        assert!(selected(&harness).0.contains(PREDICATE));
        harness.get_by_label(COLLAPSE).click();
        settle(&mut harness);
        assert_request(&harness, "Collapse checks");
        assert_eq!(
            folder(&harness).selections.get(&id).cloned(),
            Some(vec![step_id(&result, "weight_31")]),
            "the board shows the request, as ArrowLeft leaves it"
        );
        harness.get_by_label(EXPAND).click();
        settle(&mut harness);
        assert_request(&harness, "opened again");

        click_row(&mut harness, PREDICATE);
        harness.get_by_label(&fold).click();
        settle(&mut harness);
        assert_request(&harness, "▸");
        harness.get_by_label(&open).click();
        settle(&mut harness);
        assert_request(&harness, "▾");

        click_row(&mut harness, PREDICATE);
        harness.key_press(egui::Key::ArrowLeft);
        settle(&mut harness);
        assert_request(&harness, "ArrowLeft");
        harness.key_press(egui::Key::ArrowLeft);
        settle(&mut harness);
        assert_request(&harness, "ArrowLeft again folds");

        // Chosen on the board while its request is folded.
        choose_on_board(&mut harness, &id, vec![step_id(&result, "weight_31_field")]);
        assert_request(&harness, "a folded check chosen on the board");
        harness.get_by_label(&open).click();
        settle(&mut harness);
        assert_request(&harness, "opened after the board");

        // A row *Failed* leaves out.
        click_row(&mut harness, "Weight 0 kg · ");
        harness.get_by_label("Failed 1").click();
        settle(&mut harness);
        let (label, _) = selected(&harness);
        assert!(
            label.starts_with(&format!("{WEIGHT_31} · ")) || label.starts_with(PREDICATE),
            "Failed: {label:?}"
        );
        assert!(exactly(&harness, "Weight 0 kg").is_empty(), "no details");
    }

    // The keyboard on the request while its check, chosen before, is
    // selected.
    let (_project, mut harness, _, _) = ran(WIDE, &failing());
    choose(&mut harness, MAXIMISE);
    let request = row(&harness, &format!("{WEIGHT_31} · "));
    harness.get_by_label(&request).focus();
    harness.step();
    harness.key_press(egui::Key::ArrowLeft);
    settle(&mut harness);
    assert_request(&harness, "ArrowLeft on the request");
}

/// Criterion 6: *Outputs* and *Inputs* of a check with nothing in them are
/// not drawn; a request's tabs are as before.
#[test]
fn rep07c_criterion_6_empty_tabs_of_a_check() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _, result) = ran(size, PLACES);
        choose(&mut harness, MAXIMISE);
        let step = |handle: &str| {
            result
                .steps
                .iter()
                .find(|step| step.handle == handle)
                .unwrap()
        };
        let tabs = |harness: &FolderHarness| -> Vec<String> {
            ["Outputs  ", "Inputs  ", "HTTP request"]
                .into_iter()
                .filter(|tab| !texts(harness, tab).is_empty())
                .map(str::to_owned)
                .collect()
        };
        let none = |harness: &FolderHarness| {
            texts(harness, "No outputs").is_empty() && texts(harness, "No inputs").is_empty()
        };

        assert!(step("of_unsent").outputs.is_empty() && step("of_unsent").inputs.is_empty());
        click_row(&mut harness, " · of_unsent of ");
        assert!(tabs(&harness).is_empty(), "{size:?}: neither");
        assert!(none(&harness), "{size:?}: nothing is said of them");
        assert!(
            harness.query_by_label(crate::report::COPY_JSON).is_none(),
            "{size:?}: nothing to copy"
        );

        assert!(step("missing_field").outputs.is_empty());
        assert_eq!(step("missing_field").inputs.len(), 1);
        click_row(&mut harness, " · missing_field of ");
        assert_eq!(tabs(&harness), ["Inputs  "], "{size:?}: inputs only");
        assert!(none(&harness));
        assert!(
            !texts(&harness, "expected").is_empty(),
            "{size:?}: the inputs are shown"
        );

        click_row(&mut harness, " · wrong of ");
        assert_eq!(tabs(&harness), ["Outputs  ", "Inputs  "], "{size:?}: both");
        assert!(
            !texts(&harness, "passed").is_empty(),
            "{size:?}: the outputs"
        );

        click_row(&mut harness, "first · ");
        assert_eq!(
            tabs(&harness),
            ["Outputs  ", "Inputs  ", "HTTP request"],
            "{size:?}: a request"
        );
        assert!(step("unsent").outputs.is_empty());
        click_row(&mut harness, "unsent · Error");
        assert_eq!(
            tabs(&harness)[..2],
            ["Outputs  ", "Inputs  "],
            "{size:?}: a request with nothing received"
        );
        assert!(!texts(&harness, "No outputs").is_empty(), "{size:?}");

        // A check that did not run after its request did: the request's
        // response is shown, and keeps the controls of its tree.
        let (_project, mut harness, _, result) = ran(size, STOPPED);
        choose(&mut harness, MAXIMISE);
        let stopped = result.steps.iter().find(|step| step.handle == "stopped");
        let stopped = stopped.expect("the run lists the step it stopped before");
        assert!(stopped.outputs.is_empty() && stopped.inputs.is_empty());
        click_row(&mut harness, " · stopped of ");
        assert!(tabs(&harness).is_empty(), "{size:?}: neither");
        assert!(none(&harness), "{size:?}: nothing is said of them");
        assert!(!texts(&harness, "Response of first").is_empty());
        assert_eq!(exactly(&harness, "size").len(), 1, "{size:?}");
        harness.get_by_label("Collapse all").scroll_to_me();
        settle(&mut harness);
        harness.get_by_label("Collapse all").click();
        settle(&mut harness);
        assert!(exactly(&harness, "size").is_empty(), "{size:?}: collapsed");
    }
}

/// Design review: Run results as they open on a failed check, whole and the
/// panel alone, at the three windows of the owner's boards.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rep07c_opens_on_a_failure() {
    for (size, suffix) in [(MINIMUM, "960"), (LAPTOP, "1280"), (WIDE, "1600")] {
        let (_project, mut harness, _, _) = ran(size, &failing());
        crate::testing::capture(&mut harness, &format!("rep07c-window-{suffix}"), None);
        let panel = panel(&harness).expand(6.0);
        crate::testing::capture(&mut harness, &format!("rep07c-panel-{suffix}"), Some(panel));
    }
}
