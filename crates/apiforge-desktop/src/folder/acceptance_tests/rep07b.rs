//! REP-07b (#381, the details): the details of a failed check name the
//! check, the request it runs after and the field it compared, give the
//! expected and the actual value, and show that request's response with the
//! checked field marked (DIAG-12's mark); the check's own Outputs and Inputs
//! stay below. Decided by the owner on 2026-11-19 (question 1 = A, mock-up
//! M1a) and split from the list (REP-07a) on 2026-11-23. On the `labels`
//! reference scenario (FIX-01).
use super::*;
use crate::testing::{LABELS, MINIMUM};
use egui_kittest::kittest::NodeT;
use std::path::Path;

const MAXIMISE: &str = "Maximise run results";
/// The request whose check fails, and that check.
const WEIGHT_31: &str = "Weight 31 kg";
const FAILED_CHECK: &str = "weight_31_field";

fn labels_text() -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/labels.afs"),
    )
    .unwrap()
}

/// `labels` with `weight_31`'s response naming the field `mass`: its check
/// `weight_31_field` fails, every other check passes.
fn failing() -> String {
    let text = labels_text();
    let failing = text.replacen(
        r#"field: "weight", message: "Weight must be at most 30 kg.""#,
        r#"field: "mass", message: "Weight must be at most 30 kg.""#,
        1,
    );
    assert_ne!(failing, text, "weight_31's sample names its field");
    failing
}

/// `text` as `labels`' only document, open on its board at `size`.
fn labels_with(size: egui::Vec2, text: &str) -> (tempfile::TempDir, FolderHarness) {
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Label validation".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = apiforge_project::ProjectSession::open(directory.path()).unwrap();
    session
        .create_document(DocumentKind::Scenario, LABELS, text)
        .unwrap();
    drop(session);
    let mut harness = crate::testing::folder_harness_in(directory.path(), size, Mode::Diagram);
    app(&mut harness).review_open(LABELS, None);
    settle(&mut harness);
    (directory, harness)
}

/// A simulation of the active scenario, finished, with Run results
/// maximised. The board shows every check first, so that choosing a
/// check's row selects the check there (a folded check's row stays chosen
/// with REP-07a).
fn ran(harness: &mut FolderHarness) -> RunResult {
    if folder(harness).preferences.checks_collapsed {
        harness.get_by_label("Checks").click();
        settle(harness);
    }
    assert!(!folder(harness).preferences.checks_collapsed);
    choose(harness, "Run simulation");
    let result = finish_run(harness);
    choose(harness, MAXIMISE);
    result
}

/// Chooses the row of the check `handle` in the step list: the row's
/// accessible name holds the handle whether it starts with it or with the
/// check's predicate (REP-07a).
fn choose_check(harness: &mut FolderHarness, handle: &str) {
    let label = harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter_map(|node| node.accesskit_node().label().map(|label| label.to_string()))
        .find(|label| {
            label.contains(" · ")
                && (label.starts_with(&format!("{handle} · "))
                    || label.contains(&format!(" · {handle} of ")))
        })
        .unwrap_or_else(|| panic!("{handle} has a row in the step list"));
    harness.get_by_label(&label).scroll_to_me();
    settle(harness);
    harness.get_by_label(&label).click();
    settle(harness);
}

/// Where the selected step's details start: the left of their first tab.
fn details(harness: &FolderHarness) -> f32 {
    harness
        .query_all_by_label_contains("Outputs  ")
        .map(|node| node.rect().left())
        .next()
        .expect("the step's details show their Outputs tab")
}

/// Whether the details show a text labelled exactly `text`.
fn details_show(harness: &FolderHarness, text: &str) -> bool {
    let left = details(harness);
    harness
        .query_all_by_label(text)
        .any(|node| node.rect().left() >= left - 1.0)
}

/// Whether the details show a text containing `text`.
fn details_mention(harness: &FolderHarness, text: &str) -> bool {
    let left = details(harness);
    harness
        .query_all_by_label_contains(text)
        .any(|node| node.rect().left() >= left - 1.0)
}

/// Criterion 5: the details of a failed check name the check, its request
/// and what was checked, the expected and the actual value, and show the
/// request's response with that field marked (DIAG-12's mark); the check's
/// own Outputs tab stays below.
#[test]
fn rep07b_criterion_5_a_failed_check_says_what_it_checked() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness) = labels_with(size, &failing());
        ran(&mut harness);
        choose_check(&mut harness, FAILED_CHECK);
        crate::inspector::debug_forget_marks();
        harness.step();
        assert!(
            details_show(&harness, "error.field == \"weight\""),
            "{size:?}: the details are titled by the check's predicate"
        );
        assert!(
            details_mention(&harness, &format!("Check {FAILED_CHECK} · of step ")),
            "{size:?}: the details name the check and its request"
        );
        assert!(
            details_mention(
                &harness,
                &format!("{WEIGHT_31} · POST /parcels/pc-001/labels → 422")
            ),
            "{size:?}: the request's name, method, path and status"
        );
        assert!(
            details_show(&harness, "weight_31.body.error.field"),
            "{size:?}: what was checked"
        );
        assert!(
            details_show(&harness, "\"weight\""),
            "{size:?}: the expected value"
        );
        assert!(
            details_show(&harness, "\"mass\""),
            "{size:?}: the actual value"
        );
        assert!(
            details_mention(&harness, &format!("Response of {WEIGHT_31}")),
            "{size:?}: the request's response is shown"
        );
        let marks = crate::inspector::debug_marks();
        assert!(
            marks.iter().any(|mark| mark.results
                && mark.port == "body"
                && mark.path == "error.field"
                && mark.tone == crate::inspector::MarkTone::Failed),
            "{size:?}: the checked field is marked failed: {marks:#?}"
        );
    }
}

/// A step that is not a check keeps its own details: its name as the title,
/// no *Response of* section.
#[test]
fn rep07b_a_request_keeps_its_details() {
    let (_project, mut harness) = labels_with(WIDE, &failing());
    ran(&mut harness);
    let label = harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter_map(|node| node.accesskit_node().label().map(|label| label.to_string()))
        .find(|label| label.starts_with(&format!("{WEIGHT_31} · ")))
        .expect("Weight 31 kg has a row");
    harness.get_by_label(&label).scroll_to_me();
    settle(&mut harness);
    harness.get_by_label(&label).click();
    settle(&mut harness);
    assert!(details_show(&harness, WEIGHT_31), "titled by its name");
    assert!(
        !details_mention(&harness, "Response of "),
        "a request's own Outputs are its response"
    );
    assert!(!details_mention(&harness, " · of step "), "no check's line");
}

/// Design review: the details of the failed check of `labels`, maximised,
/// at both windows.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rep07b_check_details() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        let (_project, mut harness) = labels_with(size, &failing());
        ran(&mut harness);
        choose_check(&mut harness, FAILED_CHECK);
        crate::testing::capture(&mut harness, &format!("rep07b-details-{suffix}"), None);
    }
}
