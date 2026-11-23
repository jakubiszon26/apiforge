//! DIAG-03 (#356): the check card in Run results says what was required. The
//! EXPECTED side carries the predicate's sign before the value (`≠ 202`,
//! `> 0`, `nonempty`, `exists`), so a passed `!=` no longer reads as a
//! mismatch; plain equality is unchanged; a Data condition of several checks
//! reads `all of N checks`. The box of `exists` and `nonempty` is always shown,
//! with `(missing value)` when there is none. The Checks inspector's failed
//! line says the same requirement (`got 202, expected ≠ 202`).
use super::*;
use crate::testing::{MINIMUM, folder_harness_in};
use apiforge_core::run::StepStatus;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::time::{Duration, Instant};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// One request answered 422 and one answered 202, read by checks of every
/// kind the ticket names.
const SOURCE: &str = r#"scenario "Check cards" {
  on_error continue
  begin = start
  create = POST "/orders" {
    body { item: 1 }
    sample 422 { id: "557", count: 3 }
  }
  same = assert create.status == 422
  not_accepted = assert create.status != 202
  has_id = assert nonempty create.body.id
  has_items = assert create.body.count > 0
  coupon = assert exists create.body.coupon
  valid = condition all {
    create.status != 202,
    nonempty create.body.id,
  }
  again = GET "/orders/557" {
    sample 202 { id: "557" }
  }
  not_202 = assert again.status != 202
}
"#;

/// The scenario above as the demo shop's, open on its board at `size`.
fn open(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), SOURCE).unwrap();
    let mut harness = folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    // Checks on cards of their own, so that choosing a check's row selects
    // the check on the board rather than its subject: *Checks* on the bar.
    harness.get_by_label("Checks").click();
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    (project, harness, id)
}

fn simulate(harness: &mut FolderHarness) -> RunResult {
    choose(harness, "Run simulation");
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
    let active = folder(harness).workbench.active().unwrap().to_owned();
    folder(harness).run_result(&active).unwrap().clone()
}

/// Chooses `handle`'s row in the step list.
fn select(harness: &mut FolderHarness, handle: &str) {
    let label = step_row_label(harness, handle)
        .unwrap_or_else(|| panic!("{handle} has a row in the step list"));
    harness.get_by_label(&label).scroll_to_me();
    settle(harness);
    harness.get_by_label(&label).click();
    settle(harness);
}

fn text(node: &egui_kittest::Node<'_>) -> String {
    let node = node.accesskit_node();
    node.label().or_else(|| node.value()).unwrap_or_default()
}

/// The text under the comparison box's `caption`, or `None` when the card
/// has no box.
fn side(harness: &FolderHarness, caption: &str) -> Option<String> {
    let caption = harness.query_by_label(caption)?.rect();
    let below = harness
        .query_all_by_role(Role::Label)
        .filter(|node| {
            let rect = node.rect();
            (rect.left() - caption.left()).abs() <= 2.0
                && rect.top() >= caption.bottom() - 1.0
                && rect.top() <= caption.bottom() + 24.0
                && !text(node).is_empty()
        })
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .unwrap_or_else(|| panic!("a text under {caption:?}"));
    Some(text(&below))
}

/// What the selected step's card says on each side.
fn card(harness: &FolderHarness) -> (Option<String>, Option<String>) {
    (side(harness, "EXPECTED"), side(harness, "ACTUAL"))
}

fn sides(expected: &str, actual: &str) -> (Option<String>, Option<String>) {
    (Some(expected.to_owned()), Some(actual.to_owned()))
}

/// Criteria 2 and 3: each check's card states its requirement on the
/// EXPECTED side and the value on the ACTUAL side; a plain `==` reads as
/// before; the failed `exists` shows its box with `(missing value)`.
#[test]
fn diag03_each_check_card_states_its_requirement() {
    let (_project, mut harness, _id) = open(WIDE);
    let run = simulate(&mut harness);
    let status = |handle: &str| {
        run.steps
            .iter()
            .find(|step| step.handle == handle)
            .unwrap_or_else(|| panic!("{handle} ran"))
            .status
    };
    assert_eq!(status("not_accepted"), StepStatus::Passed);
    assert_eq!(status("not_202"), StepStatus::Failed);
    assert_eq!(status("coupon"), StepStatus::Failed);
    for (handle, expected) in [
        ("same", sides("422", "422")),
        ("not_accepted", sides("≠ 202", "422")),
        ("not_202", sides("≠ 202", "202")),
        ("has_id", sides("nonempty", "\"557\"")),
        ("has_items", sides("> 0", "3")),
        ("coupon", sides("exists", "(missing value)")),
        ("valid", sides("all of 2 checks", "422")),
    ] {
        select(&mut harness, handle);
        assert_eq!(card(&harness), expected, "the card of {handle}");
    }
    select(&mut harness, "not_202");
    assert!(
        harness
            .query_by_label_contains("The comparison failed.")
            .is_some(),
        "a failed check keeps its message"
    );
}

/// Owner question 2: the Checks tab of the request a failed `!=` reads
/// says the requirement, not the bare value.
#[test]
fn diag03_the_checks_tab_names_the_requirement_of_a_failed_check() {
    let (_project, mut harness, id) = open(WIDE);
    let run = simulate(&mut harness);
    let again = run
        .steps
        .iter()
        .find(|step| step.handle == "again")
        .unwrap()
        .node_id
        .clone();
    app(&mut harness).selections.insert(id, vec![again]);
    settle(&mut harness);
    super::open_checks_tab(&mut harness);
    assert!(
        harness
            .query_by_label_contains("got 202, expected ≠ 202")
            .is_some(),
        "the failed line names the requirement"
    );
}

/// Design review: the passed `!=`, the failed `!=` and `nonempty` at the
/// minimum window, for the owner's quiz (criterion 4). Run results are
/// maximised: at their normal height the card's box is below the fold.
#[test]
#[ignore = "DIAG-03: design review capture; run with tools/gui/review.sh"]
fn review_diag03_check_cards() {
    let (_project, mut harness, _id) = open(MINIMUM);
    simulate(&mut harness);
    harness.get_by_label("Maximise run results").click();
    settle(&mut harness);
    for (handle, name) in [
        ("not_accepted", "diag03-passed-not-equal-960x540"),
        ("not_202", "diag03-failed-not-equal-960x540"),
        ("has_id", "diag03-nonempty-960x540"),
    ] {
        select(&mut harness, handle);
        crate::testing::capture(&mut harness, name, None);
    }
}
