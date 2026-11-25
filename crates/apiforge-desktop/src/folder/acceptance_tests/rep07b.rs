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

/// Chooses the row of the check `handle` in the step list: under its step
/// its accessible name ends `· ‹handle› of ‹step›`, by itself `· ‹handle›`
/// (REP-07a).
fn choose_check(harness: &mut FolderHarness, handle: &str) {
    let label = harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter_map(|node| node.accesskit_node().label().map(|label| label.to_string()))
        .find(|label| {
            label.contains(&format!(" · {handle} of ")) || label.ends_with(&format!(" · {handle}"))
        })
        .unwrap_or_else(|| panic!("{handle} has a row in the step list"));
    harness.get_by_label(&label).scroll_to_me();
    settle(harness);
    harness.get_by_label(&label).click();
    settle(harness);
}

/// Where the selected step's details start: right of the list's edge. A
/// check with nothing in its tabs has none (REP-07c).
fn details(harness: &FolderHarness) -> f32 {
    harness
        .get_by_label(crate::report::RESIZE_LIST)
        .rect()
        .right()
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
            !details_show(&harness, "CHECKED"),
            "{size:?}: the title already says what was checked"
        );
        let title = details_rect(&harness, "error.field == \"weight\"");
        assert!(
            crate::testing::hover_until(
                &mut harness,
                |_| Some(title),
                |harness| harness
                    .query_by_label("weight_31.body.error.field")
                    .is_some(),
            ),
            "{size:?}: the title's hover says what was checked with every name"
        );
        harness.hover_at(egui::pos2(480.0, 3.0));
        settle(&mut harness);
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
const LONG_CHECK: &str = "the_first_reason_given_names_the_weight_of_the_parcel";
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
  {LONG_CHECK} = assert refused_label_of_a_heavy_parcel.body.error.{LONG_FIELD}.first_reason_given == "The parcel weighs more than 25 kg and the van has no lift."
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
        choose_check(&mut harness, LONG_CHECK);
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
            for text in ["first_reason_given == ", " · of step 2 ", "Response of "] {
                let rect = details_rect(&harness, text);
                assert_inside(&harness, text, rect);
                assert!(
                    !rect.intersects(status),
                    "{size:?}: {text:?} at {rect:?} is under the status at {status:?}"
                );
            }
            let title = details_rect(&harness, "first_reason_given == ");
            let line = details_rect(&harness, " · of step 2 ");
            assert!(
                title.bottom() <= line.top() + 0.5,
                "the line under the title"
            );
            // EXPECTED over ACTUAL, each wrapped whole, in one column.
            let expected = side(&harness, LONG_EXPECTED);
            let actual = side(&harness, LONG_ACTUAL);
            for (text, side) in [(LONG_EXPECTED, &expected), (LONG_ACTUAL, &actual)] {
                assert_inside(&harness, text, side.rect);
                assert!(!side.elided, "{size:?}: {text:?} is shown whole");
            }
            assert_eq!(expected.rect.left(), actual.rect.left(), "one column");
            assert!(line.bottom() <= expected.rect.top());
            assert!(expected.rect.bottom() <= actual.rect.top());
            assert_eq!(marked(&expected), ["25"], "{size:?}");
            assert_eq!(marked(&actual), ["30"], "{size:?}");
            for (caption, side) in [("EXPECTED", &expected), ("ACTUAL", &actual)] {
                let caption = details_rect(&harness, caption);
                assert!(caption.right() < side.rect.left(), "beside its caption");
                assert!(
                    caption.top() >= side.rect.top() - 2.0
                        && caption.bottom() <= side.rect.top() + 18.0,
                    "{size:?}: its caption stands at its first line"
                );
            }
        }
    }
}

const LONG_EXPECTED: &str = r#""The parcel weighs more than 25 kg and the van has no lift.""#;
const LONG_ACTUAL: &str = r#""The parcel weighs more than 30 kg and the van has no lift.""#;

/// One side of the comparison box as it is painted.
struct Side {
    rect: egui::Rect,
    rows: usize,
    elided: bool,
    /// The pieces of its text, and whether each is marked: bold, underlined
    /// and tinted.
    pieces: Vec<(String, bool)>,
}

/// The pieces of `side` that are marked.
fn marked(side: &Side) -> Vec<&str> {
    side.pieces
        .iter()
        .filter(|(_, marked)| *marked)
        .map(|(text, _)| text.as_str())
        .collect()
}

/// Every text painted beside EXPECTED or ACTUAL.
fn sides(harness: &FolderHarness) -> Vec<(String, Side)> {
    fn walk(shape: &egui::Shape, found: &mut Vec<(String, Side)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            egui::Shape::Text(text) => {
                let job = &text.galley.job;
                let pieces = job
                    .sections
                    .iter()
                    .map(|section| {
                        let format = &section.format;
                        let signs = [
                            format.underline.width > 0.0,
                            format.background != egui::Color32::TRANSPARENT,
                            format.font_id.family == crate::theme::bold_mono_family(),
                        ];
                        let range = section.byte_range.start.0..section.byte_range.end.0;
                        (job.text[range].to_owned(), signs == [true; 3])
                    })
                    .collect();
                found.push((
                    job.text.clone(),
                    Side {
                        rect: text.galley.rect.translate(text.pos.to_vec2()),
                        rows: text.galley.rows.len(),
                        elided: text.galley.elided,
                        pieces,
                    },
                ));
            }
            _ => {}
        }
    }
    let captions: Vec<egui::Rect> = ["EXPECTED", "ACTUAL"]
        .into_iter()
        .map(|caption| details_rect(harness, caption))
        .collect();
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, &mut found);
    }
    found.retain(|(_, side)| {
        captions.iter().any(|caption| {
            side.rect.left() > caption.right()
                && side.rect.top() <= caption.center().y
                && side.rect.bottom() >= caption.center().y
        })
    });
    found
}

/// The side of the comparison box whose text is `text`.
fn side(harness: &FolderHarness, text: &str) -> Side {
    let sides = sides(harness);
    let written: Vec<String> = sides.iter().map(|(text, _)| text.clone()).collect();
    sides
        .into_iter()
        .find(|(written, _)| written == text)
        .unwrap_or_else(|| panic!("the box writes {text:?}: {written:?}"))
        .1
}

/// Failed and passed checks of every predicate, over values that are hard
/// to compare by eye.
fn kinds() -> String {
    let long = "0123456789 ".repeat(200);
    r#"scenario "Kinds" {
  on_error continue
  begin = start
  r = GET "https://shop.example.test/kinds" {
    sample 200 { limit: { unit: "kg", max: 25, per: "parcel" }, weight: "31", count: 250, accent: "café 👩‍💻", spaced: "two  spaces", lines: "one\ntwo", long: "LONGb", code: null, tags: ["a", "c"], price: 9.5 }
  }
  object = assert r.body.limit == { max: 30, unit: "kg", per: "parcel" }
  kind = assert r.body.weight == 31
  grown = assert r.body.count == 25
  accent = assert r.body.accent == "cafè 👨‍💻"
  spaced = assert r.body.spaced == "two spaces"
  lines = assert r.body.lines == "one\ttwo"
  long = assert r.body.long == "LONGa"
  nothing = assert r.body.code == "too_heavy"
  wide = assert r.body.long == 1
  tags = assert r.body.tags == ["a", "b"]
  same = assert r.body.count == 250
  other = assert r.body.count != 250
  less = assert r.body.count < 100
  about = assert r.body.price ~= 9 within 0.1
  holds = assert r.body.spaced contains "three"
  typed = assert r.body.weight is "number"
  full = assert nonempty r.body.code
}
"#
    .replace("LONG", &long)
}

/// Owner's decision of 2026-11-24 (form A), rules 2 and 3: a failed
/// equality marks from the first character that differs to the last, and
/// names the kinds only where they differ; a member's place in an object,
/// half an escape or half a character is no difference. Every other
/// predicate, and a check that passed, marks nothing.
#[test]
fn rep07b_a_failed_equality_marks_what_differs() {
    let (_project, mut harness) = labels_with(WIDE, &kinds());
    ran(&mut harness);
    // The check; its expected side and what is marked of it; its actual one.
    type Case<'a> = (&'a str, &'a str, &'a [&'a str], &'a str, &'a [&'a str]);
    let cases: [Case<'_>; 8] = [
        // Members in the order the response has them.
        (
            "object",
            r#"{"unit":"kg","max":30,"per":"parcel"}"#,
            &["30"],
            r#"{"unit":"kg","max":25,"per":"parcel"}"#,
            &["25"],
        ),
        ("kind", "31", &["31"], r#""31""#, &[r#""31""#]),
        ("grown", "25", &[], "250", &["0"]),
        (
            "accent",
            r#""cafè 👨‍💻""#,
            &["è 👨‍💻"],
            r#""café 👩‍💻""#,
            &["é 👩‍💻"],
        ),
        ("spaced", r#""two spaces""#, &[], r#""two  spaces""#, &[" "]),
        (
            "lines",
            r#""one\ttwo""#,
            &[r"\t"],
            r#""one\ntwo""#,
            &[r"\n"],
        ),
        (
            "nothing",
            r#""too_heavy""#,
            &[r#""too_heavy""#],
            "null",
            &["null"],
        ),
        ("tags", r#"["a","b"]"#, &["b"], r#"["a","c"]"#, &["c"]),
    ];
    for (check, expected, left, actual, right) in cases {
        choose_check(&mut harness, check);
        assert_eq!(marked(&side(&harness, expected)), left, "{check}: expected");
        assert_eq!(marked(&side(&harness, actual)), right, "{check}: actual");
        let kinds: &[&str] = match check {
            "kind" => &["number", "text"],
            "nothing" => &["text"],
            _ => &[],
        };
        for word in ["number", "text", "list", "object", "true or false"] {
            assert_eq!(
                details_show(&harness, word),
                kinds.contains(&word),
                "{check}: {word}"
            );
        }
    }
    for (check, expected, actual) in [
        ("same", "250", "250"),
        ("other", "≠ 250", "250"),
        ("less", "< 100", "250"),
        ("about", "≈ 9 ± 0.1", "9.5"),
        ("holds", r#"contains "three""#, r#""two  spaces""#),
        ("typed", "is number", r#""31""#),
        ("full", "nonempty", "null"),
    ] {
        choose_check(&mut harness, check);
        for text in [expected, actual] {
            assert!(
                marked(&side(&harness, text)).is_empty(),
                "{check}: {text:?} is not marked"
            );
        }
    }
}

/// Rule 1: a value is one line of JSON, wrapped to at most six lines; past
/// that it ends with …, and the hover and the accessible name hold it
/// whole.
#[test]
fn rep07b_a_long_value_is_wrapped_to_six_lines() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness) = labels_with(size, &kinds());
        ran(&mut harness);
        choose_check(&mut harness, "long");
        let whole = ["a", "b"].map(|end| format!("\"{}{end}\"", "0123456789 ".repeat(200)));
        let shown: Vec<(String, Side)> = sides(&harness)
            .into_iter()
            .filter(|(text, _)| text.starts_with("\"0123456789 "))
            .collect();
        assert_eq!(shown.len(), 2);
        for (text, side) in &shown {
            assert_eq!(side.rows, 6, "{size:?}");
            assert!(side.elided, "{size:?}: it ends with …");
            assert_inside(&harness, text, side.rect);
        }
        for whole in &whole {
            assert!(details_show(&harness, whole), "{size:?}: the name is whole");
        }
        let value = shown[0].1.rect;
        let before = harness.query_all_by_label(&whole[0]).count();
        assert!(
            crate::testing::hover_until(
                &mut harness,
                |_| Some(value),
                |harness| harness.query_all_by_label(&whole[0]).count() > before,
            ),
            "{size:?}: the hover holds the whole value"
        );
    }
}

/// `labels` with a second check of `weight_31`'s message, of two steps,
/// which fails, and a check of a field the response has not.
fn crossed() -> String {
    let text = failing();
    let at = text.find("  @label(\"Unknown parcel\")").unwrap();
    format!(
        "{}  same_text = assert weight_0.body.error.message == weight_31.body.error.message\n  \
         hint = assert weight_0.body.error.hint == \"Ask the sender.\"\n{}",
        &text[..at],
        &text[at..]
    )
}

/// The results' marks drawn on the field `error.<name>` in the last frame.
fn mark_of(harness: &mut FolderHarness, name: &str) -> (crate::inspector::MarkTone, String) {
    crate::inspector::debug_forget_marks();
    harness.step();
    let path = format!("error.{name}");
    crate::inspector::debug_marks()
        .into_iter()
        .find(|mark| mark.results && mark.port == "body" && mark.path == path)
        .map(|mark| (mark.tone, mark.words))
        .unwrap_or_else(|| panic!("a mark on {path}"))
}

/// Test in the real window, 2026-11-24, finding 1: in a check's details the
/// mark of its own field says this check's result, whatever the other
/// checks of the field say; they are still listed by a click. The request's
/// own details keep DIAG-12's mark.
#[test]
fn rep07b_the_mark_of_the_checked_field_is_the_checks_own() {
    use crate::inspector::MarkTone;
    let (_project, mut harness) = labels_with(WIDE, &crossed());
    ran(&mut harness);
    choose_check(&mut harness, "weight_31_text");
    assert_eq!(
        mark_of(&mut harness, "message"),
        (MarkTone::Passed, "This check · 1 more".into()),
        "this check passed, the other one failed"
    );
    assert_eq!(
        mark_of(&mut harness, "field"),
        (MarkTone::Failed, "1 failed".into()),
        "another field's mark is DIAG-12's"
    );
    let message = harness
        .query_all_by_label("message")
        .map(|node| node.rect())
        .find(|rect| rect.left() >= details(&harness))
        .expect("the field's row");
    harness
        .query_all_by_label("Checks of this field")
        .find(|node| (node.rect().center().y - message.center().y).abs() < 6.0)
        .expect("the field's mark")
        .click();
    settle(&mut harness);
    for check in [
        "error.message == \"Weight must be at most 30 kg.\"",
        "weight_0.body.error.message == weight_31.body.error.message",
    ] {
        assert!(
            harness
                .query_all_by_label(check)
                .any(|node| node.accesskit_node().role() == egui::accesskit::Role::Button),
            "the menu lists {check}"
        );
    }
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);

    choose_check(&mut harness, "same_text");
    assert_eq!(
        mark_of(&mut harness, "message"),
        (MarkTone::Failed, "This check · 1 more".into())
    );
    choose_check(&mut harness, FAILED_CHECK);
    assert_eq!(
        mark_of(&mut harness, "field"),
        (MarkTone::Failed, "This check".into())
    );

    let label = harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter_map(|node| node.accesskit_node().label().map(|label| label.to_string()))
        .find(|label| label.starts_with(&format!("{WEIGHT_31} · ")))
        .unwrap();
    harness.get_by_label(&label).click();
    settle(&mut harness);
    assert_eq!(
        mark_of(&mut harness, "message"),
        (MarkTone::Failed, "1 failed".into()),
        "the request's own outputs"
    );
}

/// Finding 2: a check of two steps says where its expected value came from;
/// a check of one step, whose title is enough, does not.
#[test]
fn rep07b_a_check_of_two_steps_says_where_expected_came_from() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness) = labels_with(size, &crossed());
        ran(&mut harness);
        choose_check(&mut harness, "same_text");
        let origin = details_rect(&harness, "from weight_31.body.error.message");
        assert_inside(&harness, "from …", origin);
        let expected = details_rect(&harness, "\"Weight must be at most 30 kg.\"");
        let actual = details_rect(&harness, "\"Weight must be more than 0 kg.\"");
        assert!(
            origin.top() >= expected.top() - 2.0 && origin.bottom() <= actual.top() + 2.0,
            "{size:?}: beside or under EXPECTED: {origin:?}, {expected:?}, {actual:?}"
        );
        choose_check(&mut harness, FAILED_CHECK);
        assert!(!details_mention(&harness, "from weight_31"));
    }
}

/// Finding 3: a check whose field the response has not shows what it
/// expected and that the value is missing, nothing marked.
#[test]
fn rep07b_a_missing_field_has_its_two_rows() {
    let (_project, mut harness) = labels_with(WIDE, &crossed());
    ran(&mut harness);
    choose_check(&mut harness, "hint");
    for text in ["\"Ask the sender.\"", "(missing value)"] {
        assert!(marked(&side(&harness, text)).is_empty(), "{text}");
    }
}

/// Finding 4: results of an earlier version, which offer no check, do not
/// ask for a field to be chosen.
#[test]
fn rep07b_stale_results_do_not_offer_to_add_a_check() {
    const OFFER: &str = "Choose a field → add a check";
    let (_project, mut harness, id) = opened(WIDE, None, &failing());
    ran(&mut harness);
    choose_check(&mut harness, FAILED_CHECK);
    assert!(details_mention(&harness, OFFER));
    let edited = failing().replacen("\"Weight 31 kg\"", "\"Weight 32 kg\"", 1);
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited, apiforge_project::Origin::SESSION)
        .unwrap();
    settle(&mut harness);
    choose_check(&mut harness, FAILED_CHECK);
    assert!(
        folder(&harness).fresh_run(&id).is_none(),
        "the edit makes the results stale"
    );
    assert!(!details_mention(&harness, OFFER));
}

/// Review of 2026-11-24: at Run results' normal height, choosing each
/// check's row, scrolled to, leaves the window at rest.
#[test]
fn rep07b_a_check_chosen_at_the_normal_height_settles() {
    for size in [MINIMUM, egui::vec2(1280.0, 800.0)] {
        for (text, checks) in [
            (crossed(), &["weight_31_text", "same_text"][..]),
            (kinds(), &["object", "long"]),
        ] {
            let (_project, mut harness) = labels_with(size, &text);
            choose(&mut harness, "Run simulation");
            finish_run(&mut harness);
            for check in checks {
                choose_check(&mut harness, check);
                let rect = |harness: &FolderHarness| details_rect(harness, "Outputs  ");
                let at = rect(&harness);
                for _ in 0..6 {
                    harness.step();
                    assert_eq!(rect(&harness), at, "{size:?}: {check} moves");
                    // Only the folder's own watch of its files and its
                    // storage ask for a frame.
                    for cause in harness.ctx.repaint_causes() {
                        assert!(
                            cause.file.contains("folder"),
                            "{size:?}: {check} asks for repaint: {cause:?}"
                        );
                    }
                }
            }
        }
    }
}

/// A response of over 2000 values, checked far from its first rows.
fn big() -> String {
    let items: Vec<String> = (0..400)
        .map(|at| {
            format!(
                "{{ id: {at}, name: \"Thing {at}\", price: 1, tags: [\"a\", \"b\"], on: true }}"
            )
        })
        .collect();
    format!(
        r#"scenario "Big" {{
  on_error continue
  begin = start
  list = GET "https://shop.example.test/big" {{
    sample 200 {{ items: [{}] }}
  }}
  far = assert list.body.items[200].name == "Thing 201"
  last = assert list.body.items[399].name == "Thing 400"
}}
"#,
        items.join(", ")
    )
}

/// Second test in the real window, 2026-11-24, finding 1: the response is
/// opened to the checked field, on its page, however large it is and also
/// with *Expand all*; the heading says that the field is marked only while
/// its row is shown.
#[test]
fn rep07b_a_large_response_is_opened_to_the_checked_field() {
    use crate::inspector::MarkTone;
    let (_project, mut harness) = labels_with(WIDE, &big());
    ran(&mut harness);
    let drawn = |harness: &mut FolderHarness, path: &str| {
        crate::inspector::debug_forget_marks();
        harness.step();
        crate::inspector::debug_marks().into_iter().any(|mark| {
            mark.path == path
                && mark.results
                && mark.shown
                && mark.tone == MarkTone::Failed
                && mark.words == "This check"
        })
    };
    let marked = "Response of list · the checked field is marked";
    for (check, path) in [("far", "items[200].name"), ("last", "items[399].name")] {
        choose_check(&mut harness, check);
        assert!(drawn(&mut harness, path), "{check}");
        assert!(details_show(&harness, marked), "{check}");
    }
    // Expand all opens everything, but not past the rows drawn at once.
    for label in ["Expand all", "Collapse all"] {
        harness.get_by_label(label).scroll_to_me();
        settle(&mut harness);
        harness.get_by_label(label).click();
        settle(&mut harness);
        let expanded = label == "Expand all";
        assert_eq!(drawn(&mut harness, "items[399].name"), expanded, "{label}");
        assert_eq!(details_show(&harness, marked), expanded, "{label}");
        assert!(details_mention(&harness, "Response of list"));
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
    assert!(
        details_show(&harness, "Response of first · the checked field is marked"),
        "the response that holds what it checked"
    );

    // A field the response has not: nothing is marked.
    choose_check(&mut harness, "missing_field");
    assert!(details_mention(
        &harness,
        "Check missing_field · of step 2 first · GET /first → 200"
    ));
    assert!(details_show(&harness, "Response of first"));

    // Its step did not run: there is no response.
    choose_check(&mut harness, "of_unsent");
    assert!(details_mention(
        &harness,
        "Check of_unsent · of step 6 unsent"
    ));
    assert!(!details_mention(&harness, "Response of "));

    // Cleanup: its own request, and a step of Main. The check that reads
    // only Main's step runs first.
    choose_check(&mut harness, "gone");
    assert!(details_mention(
        &harness,
        "Check gone · of step 2 removed · DELETE /first → 204"
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

    // A check written again is another check than the one that ran.
    edit(&mut harness, &failing());
    assert!(details_show(&harness, &format!("Response of {WEIGHT_31}")));
    choose(&mut harness, "Run simulation");
    finish_run(&mut harness);
    choose_check(&mut harness, FAILED_CHECK);
    assert!(details_show(&harness, &marked), "a second run's details");
}

/// Design review: the details of the failed check of `labels` and of a
/// check with long names, maximised, at both windows.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rep07b_check_details() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        for (name, text, check) in [
            ("details", failing(), FAILED_CHECK),
            ("long", long(), LONG_CHECK),
            ("object", kinds(), "object"),
            ("kinds", kinds(), "kind"),
            ("cut", kinds(), "long"),
            ("spaces", kinds(), "spaced"),
            ("passed", crossed(), "weight_31_text"),
            ("two-steps", crossed(), "same_text"),
            ("missing", crossed(), "hint"),
            ("big", big(), "far"),
            ("rows", kinds(), "wide"),
        ] {
            let (_project, mut harness) = labels_with(size, &text);
            ran(&mut harness);
            choose_check(&mut harness, check);
            crate::testing::capture(&mut harness, &format!("rep07b-{name}-{suffix}"), None);
            if name == "details" {
                harness.get_by_label("Copy as JSON").scroll_to_me();
                settle(&mut harness);
                let name = format!("rep07b-{name}-end-{suffix}");
                crate::testing::capture(&mut harness, &name, None);
            }
        }
    }
}
