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

/// `text` as the project's only scenario, open on its board at `size`.
fn labels_with(size: egui::Vec2, text: &str) -> (tempfile::TempDir, FolderHarness) {
    let (project, harness, _) = opened(size, None, text);
    (project, harness)
}

/// `text` as the project's only scenario, open on its board at `size`,
/// beside the published component `component`.
fn opened(
    size: egui::Vec2,
    component: Option<&str>,
    text: &str,
) -> (tempfile::TempDir, FolderHarness, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Label validation".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = apiforge_project::ProjectSession::open(directory.path()).unwrap();
    if let Some(component) = component {
        let id = session
            .create_document(DocumentKind::Component, "components/lookup.afs", component)
            .unwrap();
        session
            .publish(&id, apiforge_project::Origin::SESSION)
            .unwrap();
    }
    let id = session
        .create_document(DocumentKind::Scenario, LABELS, text)
        .unwrap();
    drop(session);
    let mut harness = crate::testing::folder_harness_in(directory.path(), size, Mode::Diagram);
    app(&mut harness).review_open(LABELS, None);
    settle(&mut harness);
    (directory, harness, id)
}

/// A simulation of the active scenario, finished, with Run results
/// maximised.
fn ran(harness: &mut FolderHarness) -> RunResult {
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

/// Whether a text of the details is written inside them, right of their
/// left edge and left of the window's right edge.
fn assert_inside(harness: &FolderHarness, text: &str, rect: egui::Rect) {
    let left = details(harness) - 1.0;
    let right = harness.ctx.content_rect().right();
    assert!(
        rect.left() >= left && rect.right() <= right,
        "{text:?} at {rect:?} is outside the details {left}..{right}"
    );
}

/// The rect of the details' text containing `text`.
fn details_rect(harness: &FolderHarness, text: &str) -> egui::Rect {
    let left = details(harness);
    harness
        .query_all_by_label_contains(text)
        .map(|node| node.rect())
        .find(|rect| rect.left() >= left - 1.0)
        .unwrap_or_else(|| panic!("the details write {text:?}"))
}

const LONG_LABEL: &str =
    "Printing a label of a parcel heavier than the courier takes on this route is refused";
const LONG_FIELD: &str = "reasons_the_courier_gave_for_refusing_the_parcel_at_the_counter";

/// A request with a long name and a check with a long name of a long field.
fn long() -> String {
    format!(
        r#"scenario "Long" {{
  begin = start
  @label("{LONG_LABEL}")
  refused_label_of_a_heavy_parcel = POST "https://parcels.example.test/parcels/pc-001/labels/for/the/courier/of/this/route" {{
    body {{ weight: 31 }}
    sample 422 {{ error: {{ {LONG_FIELD}: {{ first_reason_given: "The parcel weighs more than 30 kg and the van has no lift." }} }} }}
  }}
  the_first_reason_given_names_the_weight_of_the_parcel = assert refused_label_of_a_heavy_parcel.body.error.{LONG_FIELD}.first_reason_given == "The parcel weighs more than 25 kg and the van has no lift."
}}
"#
    )
}

/// Review of 2026-11-24: beside the list as it opens and beside the widest
/// one, at 960 × 540 and 1600 × 1000, what the details write of a check is
/// not cut, and the status beside the title covers none of it.
#[test]
fn rep07b_the_texts_of_a_check_are_not_cut() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness) = labels_with(size, &long());
        ran(&mut harness);
        choose_check(
            &mut harness,
            "the_first_reason_given_names_the_weight_of_the_parcel",
        );
        for widest in [false, true] {
            if widest {
                let from = harness.get_by_label("Resize the step list").rect().center();
                super::lay16::drag(&mut harness, from, from + egui::vec2(size.x, 0.0));
            }
            // The pill's text and the 8 px on each side of it.
            let status = harness
                .query_all_by_label("Failed")
                .map(|node| node.rect())
                .find(|rect| rect.left() >= details(&harness))
                .expect("the details say the status")
                .expand2(egui::vec2(8.0, 0.0));
            assert_inside(&harness, "Failed", status);
            for text in [
                "first_reason_given == ",
                " · of step 2 ",
                "refused_label_of_a_heavy_parcel.body.error.",
                "Response of ",
            ] {
                let rect = details_rect(&harness, text);
                assert_inside(&harness, text, rect);
                assert!(
                    !rect.intersects(status),
                    "{size:?}: {text:?} at {rect:?} is under the status at {status:?}"
                );
            }
            let title = details_rect(&harness, "first_reason_given == ");
            let line = details_rect(&harness, " · of step 2 ");
            let checked = details_rect(&harness, "refused_label_of_a_heavy_parcel.body.error.");
            assert!(
                title.bottom() <= line.top() + 0.5,
                "the line under the title"
            );
            assert!(line.bottom() <= checked.top(), "CHECKED under the line");
            for text in ["EXPECTED", "ACTUAL"] {
                let rect = details_rect(&harness, text);
                assert_inside(&harness, text, rect);
                assert!(checked.bottom() <= rect.top(), "{text} under CHECKED");
            }
        }
    }
}

/// A check in each of the places a check can be in.
const PLACES: &str = r#"scenario "Places" {
  on_error continue
  begin = start
  first = GET "https://shop.example.test/first" {
    sample 200 { size: 1, items: [] }
  }
  missing_field = assert first.body.nothing == 1
  second = GET "https://shop.example.test/second" {
    sample 200 { size: 2 }
  }
  across = assert first.body.size == second.body.size
  unsent = GET "https://shop.example.test/{{first.body.items[0].id}}" {
    sample 200 {}
  }
  of_unsent = assert unsent.status == 200
  cleanup {
    removed = DELETE "https://shop.example.test/first" {
      sample 204 {}
    }
    gone = assert removed.status == 200
    still = assert first.body.size == 3
  }
}
"#;

/// Review of 2026-11-24: the details name the step of a check that reads
/// two, of one in Cleanup and of one whose step did not run, and show the
/// response that holds the checked field; they say a field is marked only
/// where one is.
#[test]
fn rep07b_the_details_of_a_check_wherever_it_is() {
    let (_project, mut harness) = labels_with(WIDE, PLACES);
    let result = ran(&mut harness);
    assert_eq!(result.status, RunStatus::Failed);

    // Two steps: it runs after the second, what it checked is in the first.
    choose_check(&mut harness, "across");
    assert!(details_mention(
        &harness,
        "Check across · of step 4 second · GET /second → 200"
    ));
    assert!(details_show(&harness, "first.body.size"), "what it checked");
    assert!(
        details_show(&harness, "Response of first · the checked field is marked"),
        "the response that holds what it checked"
    );

    // A field the response has not: nothing is marked.
    choose_check(&mut harness, "missing_field");
    assert!(details_show(&harness, "first.body.nothing"));
    assert!(details_show(&harness, "(missing value)"));
    assert!(details_show(&harness, "Response of first"));

    // Its step did not run: there is no response.
    choose_check(&mut harness, "of_unsent");
    assert!(details_mention(
        &harness,
        "Check of_unsent · of step 6 unsent"
    ));
    assert!(!details_mention(&harness, "Response of "));

    // Cleanup: its own request, and a step of Main.
    choose_check(&mut harness, "gone");
    assert!(details_mention(
        &harness,
        "Check gone · of step 1 removed · DELETE /first → 204"
    ));
    assert!(details_show(
        &harness,
        "Response of removed · the checked field is marked"
    ));
    choose_check(&mut harness, "still");
    assert!(details_show(&harness, "first.body.size == 3"), "every name");
    assert!(
        details_show(&harness, "Check still"),
        "it runs after no step"
    );
    assert!(details_show(
        &harness,
        "Response of first · the checked field is marked"
    ));
}

/// Review of 2026-11-24: a check inside a component call names the request
/// of its component; its fields, which the scenario's checks do not read,
/// are not said to be marked.
#[test]
fn rep07b_a_check_inside_a_component_call_names_its_request() {
    let (_project, mut harness, _) = opened(
        WIDE,
        Some(
            r#"component lookup "Look up" {
  fetch = GET "https://shop.example.test/lookup" {
    sample 200 { size: 1 }
  }
  sized = assert fetch.body.size == 2
  export size: number = fetch.body.size
}
"#,
        ),
        r#"scenario "Call" {
  begin = start
  looked_up = lookup@1()
}
"#,
    );
    ran(&mut harness);
    choose_check(&mut harness, "sized");
    assert!(details_show(&harness, "size == 2"));
    assert!(details_mention(
        &harness,
        "Check sized · of step 1 fetch · GET /lookup → 200"
    ));
    assert!(details_show(&harness, "fetch.body.size"));
    assert!(details_show(&harness, "Response of fetch"));
}

/// Review of 2026-11-24: after the run, the details keep what the run
/// executed: the check deleted since, its step deleted since, and a source
/// that does not read; a deleted check's field is not said to be marked. A
/// second run's details are its own.
#[test]
fn rep07b_the_details_keep_what_the_run_executed() {
    let (_project, mut harness, id) = opened(WIDE, None, &failing());
    ran(&mut harness);
    choose_check(&mut harness, FAILED_CHECK);
    let line = format!("Check {FAILED_CHECK} · of step 6 {WEIGHT_31} · POST");
    let marked = format!("Response of {WEIGHT_31} · the checked field is marked");
    assert!(details_show(&harness, &marked));
    let check = r#"  weight_31_field = assert weight_31.body.error.field == "weight"
"#;
    let without_check = failing().replacen(check, "", 1);
    assert_ne!(without_check, failing());
    let edit = |harness: &mut FolderHarness, text: &str| {
        app(harness)
            .session
            .as_mut()
            .unwrap()
            .edit_source(&id, text, apiforge_project::Origin::SESSION)
            .unwrap();
        settle(harness);
        choose_check(harness, FAILED_CHECK);
    };

    edit(&mut harness, &without_check);
    assert!(details_mention(&harness, &line), "the check is deleted");
    assert!(details_show(&harness, &format!("Response of {WEIGHT_31}")));

    // Its request is deleted too: the text from its label to the next one.
    let from = without_check.find("  @label(\"Weight 31 kg\")").unwrap();
    let to = without_check.find("  @label(\"Weight 0 kg\")").unwrap();
    let without_step = format!("{}{}", &without_check[..from], &without_check[to..]);
    edit(&mut harness, &without_step);
    assert!(details_mention(&harness, &line), "the step is deleted");
    assert!(details_show(&harness, &format!("Response of {WEIGHT_31}")));

    edit(&mut harness, &format!("{}\n}}\n", failing()));
    assert!(details_mention(&harness, &line), "the source does not read");

    edit(&mut harness, &failing());
    assert!(details_show(&harness, &marked), "the check is back");
    choose(&mut harness, "Run simulation");
    finish_run(&mut harness);
    choose_check(&mut harness, FAILED_CHECK);
    assert!(details_show(&harness, &marked), "a second run's details");
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
