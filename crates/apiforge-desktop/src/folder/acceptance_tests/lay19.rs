//! LAY-19: a failure shows its consequences. A step that did not run reads
//! the kind of the last link of its cause and the step at its end
//! (`Stopped · checkout_total failed`), the card that holds the failure
//! counts the steps it kept from running, and Run results count every step
//! (owner's decisions of 2026-11-17: 1 A, 2 A, 3 A, 4 A).

use super::lay16::{SCENARIO, canvas, document, id_of, shop_30, show, switch_density};
use super::*;
use crate::canvas::{DrawnText, TextKind};
use crate::theme;
use apiforge_core::model::NodeKind;
use apiforge_core::run::{NotRunCause, RunResult, RunStatus, StepResult, StepStatus};
use egui::{Color32, FontId, Rect};
use egui_kittest::kittest::NodeT;
use std::time::{Duration, Instant};

const CHECKOUT: &str = r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#;
const REFUND: &str = "    sample 200 { refunded: true, amount: 172.5 }\n";
const CART_VIEW: &str =
    r#"sample 200 { id: "c-1", total: 172.5, items: [{ productId: 123 }, { productId: 124 }] }"#;
const STOPPED: &str = "Stopped · checkout_total failed";
const FIT: &str = "Fit the view";

fn replaced(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "the fixture holds {from:?}");
    source.replacen(from, to, 1)
}

/// `shop-30` whose checkout answers with a total its check does not expect:
/// `checkout_total` fails and the policy *Stop* ends the run there.
fn failing() -> String {
    replaced(&shop_30(), CHECKOUT, &CHECKOUT.replace("172.5", "180.0"))
}

/// `source` under the policy *Continue*.
fn continued(source: &str) -> String {
    replaced(
        source,
        "  begin = start\n",
        "  on_error continue\n  begin = start\n",
    )
}

/// Under *Continue*, `refund` has no sample: the simulation errors there.
fn refund_without_sample() -> String {
    replaced(&continued(&shop_30()), REFUND, "")
}

/// Under *Continue*, the cart view answers with no items: the *Data
/// condition* `valid_cart` is false.
fn condition_unmet() -> String {
    replaced(
        &continued(&shop_30()),
        CART_VIEW,
        r#"sample 200 { id: "c-1", total: 172.5, items: [] }"#,
    )
}

/// A value that fails before *Start*, the run of #418.
fn value_before_start() -> String {
    replaced(
        &replaced(
            &shop_30(),
            "  begin = start\n",
            "  begin = start\n  bad = \"1\" + 1\n",
        ),
        "query { limit: 20 }",
        "query { limit: bad }",
    )
}

/// `source` on the board of the demo shop as a new install opens it:
/// compact cards, checks collapsed.
fn open(source: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    (project, harness)
}

fn simulate(harness: &mut FolderHarness) {
    choose(harness, "Run simulation");
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
    harness.step();
}

/// `source` opened at `size` and simulated.
fn ran(source: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let (project, mut harness) = open(source, size);
    simulate(&mut harness);
    (project, harness)
}

fn run(harness: &FolderHarness) -> RunResult {
    folder(harness)
        .fresh_run(&document(harness))
        .expect("a run")
}

fn step<'a>(run: &'a RunResult, handle: &str) -> &'a StepResult {
    run.steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no result for {handle}: {run:#?}"))
}

/// The step that ran at the end of a not-run step's cause, and the last
/// link; `None` for a step that ran.
fn end_of<'a>(
    run: &'a RunResult,
    step: &'a StepResult,
) -> Option<(&'a NotRunCause, Option<&'a StepResult>)> {
    let mut cause = step.cause.as_ref()?;
    for _ in 0..=run.steps.len() {
        let Some(next) = cause.step() else {
            return Some((cause, None));
        };
        let target = run.steps.iter().find(|step| step.node_id == next)?;
        match &target.cause {
            Some(further) => cause = further,
            None => return Some((cause, Some(target))),
        }
    }
    panic!("the cause of {} loops", step.handle);
}

/// How many steps of the run end their cause at `handle`.
fn caused_by(run: &RunResult, handle: &str) -> usize {
    run.steps
        .iter()
        .filter(|step| {
            end_of(run, step).is_some_and(|(_, end)| end.is_some_and(|end| end.handle == handle))
        })
        .count()
}

fn steps_not_run(count: usize) -> String {
    if count == 1 {
        "1 step not run".to_owned()
    } else {
        format!("{count} steps not run")
    }
}

/// The result texts the card `handle` wrote, the camera centred on it at
/// `zoom`.
fn results_at(harness: &mut FolderHarness, handle: &str, zoom: f32) -> Vec<DrawnText> {
    show(harness, handle, zoom);
    results(harness, handle)
}

fn results(harness: &FolderHarness, handle: &str) -> Vec<DrawnText> {
    canvas(harness)
        .debug_texts(&id_of(harness, handle))
        .into_iter()
        .filter(|text| text.kind == TextKind::Result)
        .collect()
}

/// The count a card drew: `11 steps not run`, or `11 not run` where the
/// line has no room for the longer form.
fn count_of(texts: &[DrawnText]) -> Option<&DrawnText> {
    texts.iter().find(|text| text.text.ends_with(" not run"))
}

/// The result line of a card: its first result text that is not the count.
fn line_of(texts: &[DrawnText]) -> Option<&DrawnText> {
    texts.iter().find(|text| !text.text.ends_with(" not run"))
}

/// The words of every card wholly in view, by handle.
fn cards_in_view(harness: &FolderHarness) -> Vec<(String, Rect, Vec<DrawnText>)> {
    let board = canvas(harness);
    let view = board.debug_focus();
    let run = run(harness);
    run.steps
        .iter()
        .filter_map(|step| {
            let rect = board.debug_card_rect(&step.node_id)?;
            view.contains_rect(rect).then(|| {
                (
                    step.handle.clone(),
                    rect,
                    board
                        .debug_texts(&step.node_id)
                        .into_iter()
                        .filter(|text| text.kind == TextKind::Result)
                        .collect(),
                )
            })
        })
        .collect()
}

/// A node's accessible text: its label, or the value of a label widget.
fn text_of(node: egui_kittest::Node<'_>) -> Option<String> {
    let node = node.accesskit_node();
    node.label().or_else(|| node.value())
}

/// The accessible labels of the Run results rows of `name`: one at the top
/// level, more where a component holds a step of the same name.
fn rows(harness: &FolderHarness, name: &str) -> Vec<String> {
    let prefix = format!("{name} · ");
    harness
        .query_all_by_label_contains(&prefix)
        .filter_map(text_of)
        .filter(|label| label.starts_with(&prefix))
        .collect()
}

/// The accessible label of the one Run results row of `name`.
fn row(harness: &FolderHarness, name: &str) -> String {
    let found = rows(harness, name);
    assert_eq!(found.len(), 1, "one row of {name}: {found:?}");
    found.into_iter().next().unwrap()
}

/// What a not-run step reads, as the ticket's table words it.
fn expected_words(run: &RunResult, step: &StepResult) -> Option<String> {
    let (last, end) = end_of(run, step)?;
    let what = |end: Option<&StepResult>| {
        end.map(|end| {
            let unmet = end.kind == NodeKind::Condition && end.status == StepStatus::Blocked;
            format!(
                "{} {}",
                end.display_name(),
                if unmet { "unmet" } else { "failed" }
            )
        })
    };
    Some(match last {
        NotRunCause::Stopped { .. } => format!("Stopped · {}", what(end)?),
        NotRunCause::BlockedBy { .. } | NotRunCause::ConditionUnmet { .. } => {
            format!("Blocked · {}", what(end)?)
        }
        NotRunCause::OutsidePath => "Skipped · outside path".to_owned(),
        NotRunCause::Cancelled => "Cancelled".to_owned(),
        NotRunCause::CleanupInputMissing { .. } => {
            format!("Skipped · no value from {}", end?.display_name())
        }
    })
}

/// The summary Run results should read: every step in one group.
fn expected_summary(run: &RunResult) -> Vec<String> {
    let mut groups: Vec<(&str, usize)> = [
        "passed",
        "failed",
        "not met",
        "stopped",
        "blocked",
        "skipped",
        "cancelled",
    ]
    .into_iter()
    .map(|group| (group, 0))
    .collect();
    for step in &run.steps {
        let group = match (end_of(run, step), step.status) {
            (Some((NotRunCause::Stopped { .. }, _)), _) => "stopped",
            (Some((NotRunCause::BlockedBy { .. } | NotRunCause::ConditionUnmet { .. }, _)), _) => {
                "blocked"
            }
            (Some((NotRunCause::OutsidePath | NotRunCause::CleanupInputMissing { .. }, _)), _) => {
                "skipped"
            }
            (Some((NotRunCause::Cancelled, _)), _) => "cancelled",
            (None, StepStatus::Passed) => "passed",
            (None, StepStatus::Failed | StepStatus::Error) => "failed",
            (None, StepStatus::Blocked) if step.kind == NodeKind::Condition => "not met",
            (None, StepStatus::Blocked) => "blocked",
            (None, StepStatus::Skipped) => "skipped",
            (None, StepStatus::Cancelled) => "cancelled",
        };
        groups
            .iter_mut()
            .find(|(name, _)| *name == group)
            .unwrap()
            .1 += 1;
    }
    let total: usize = groups.iter().map(|(_, count)| count).sum();
    assert_eq!(total, run.steps.len());
    groups
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(group, count)| format!("{count} {group}"))
        .collect()
}

/// Every summary label Run results show.
fn summary(harness: &FolderHarness) -> Vec<String> {
    let groups = [
        "passed",
        "failed",
        "not met",
        "stopped",
        "blocked",
        "skipped",
        "cancelled",
    ];
    let mut found = Vec::new();
    for group in groups {
        for node in harness.query_all_by_label_contains(&format!(" {group}")) {
            let Some(label) = text_of(node) else {
                continue;
            };
            if let Some(count) = label.strip_suffix(&format!(" {group}"))
                && count.parse::<usize>().is_ok()
            {
                found.push(label.clone());
            }
        }
    }
    found
}

/// Criterion 1: after the failed simulation under *Stop*, the card that
/// holds the failure reads the number of steps whose cause ends at
/// `checkout_total`, eleven: `checkout` with checks collapsed,
/// `checkout_total` with checks drawn. A step behind an unrelated false
/// *Data condition* is not counted.
#[test]
fn lay19_criterion_1_the_count_equals_the_caused_rows() {
    let (_project, mut harness) = ran(&failing(), WIDE);
    let result = run(&harness);
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(caused_by(&result, "checkout_total"), 11, "{result:#?}");

    let checkout = results_at(&mut harness, "checkout", 0.6);
    let count = count_of(&checkout).unwrap_or_else(|| panic!("checkout's count: {checkout:?}"));
    assert_eq!(count.text, steps_not_run(11));
    assert_eq!(count.color, theme::DANGER, "the failure's red");
    // Only the card that holds the failure counts.
    for handle in ["order", "address", "cart", "login"] {
        let texts = results_at(&mut harness, handle, 0.6);
        assert_eq!(count_of(&texts), None, "{handle}: {texts:?}");
    }

    // Every check drawn: the failed check holds its own count.
    app(&mut harness).debug_draw_checks();
    settle(&mut harness);
    let total = results_at(&mut harness, "checkout_total", 0.6);
    assert_eq!(
        count_of(&total).map(|count| count.text.as_str()),
        Some(steps_not_run(11).as_str()),
        "{total:?}"
    );
    let checkout = results_at(&mut harness, "checkout", 0.6);
    assert_eq!(count_of(&checkout), None, "{checkout:?}");

    // Under *Continue*, a false *Data condition* and an unrelated request
    // that fails: each counts the steps behind it only.
    let source = replaced(
        &condition_unmet(),
        "  cleanup {\n",
        "  extra = GET \"/extra\" {\n  }\n  extra_ok = assert extra.status == 200\n  cleanup {\n",
    );
    let (_project, mut harness) = ran(&source, WIDE);
    let result = run(&harness);
    assert_eq!(
        step(&result, "extra").status,
        StepStatus::Error,
        "{result:#?}"
    );
    let unmet = caused_by(&result, "valid_cart");
    assert!(unmet > 1, "steps behind the condition: {result:#?}");
    assert_eq!(caused_by(&result, "extra"), 1);
    let condition = results_at(&mut harness, "valid_cart", 0.6);
    let count = count_of(&condition).unwrap_or_else(|| panic!("valid_cart: {condition:?}"));
    assert_eq!(count.text, steps_not_run(unmet));
    assert_eq!(count.color, theme::WARNING, "a condition's amber");
    let extra = results_at(&mut harness, "extra", 0.6);
    let count = count_of(&extra).unwrap_or_else(|| panic!("extra: {extra:?}"));
    assert_eq!(count.text, steps_not_run(1));
    assert_eq!(count.color, theme::DANGER);
}

/// Criterion 2: each card that did not run and has a result line reads the
/// kind of the last link of its cause and the step at its end, without a
/// duration: under *Stop* all eleven read `Stopped · checkout_total failed`
/// although eight carry `BlockedBy`; under *Continue* a request's readers
/// read `Blocked · refund failed`; behind a false *Data condition*,
/// `Blocked · valid_cart unmet`.
#[test]
fn lay19_criterion_2_each_card_names_the_end_of_its_cause() {
    let (_project, mut harness) = ran(&failing(), WIDE);
    let result = run(&harness);
    let not_run: Vec<&StepResult> = result
        .steps
        .iter()
        .filter(|step| step.cause.is_some())
        .collect();
    assert_eq!(not_run.len(), 11);
    let blocked = not_run
        .iter()
        .filter(|step| matches!(step.cause, Some(NotRunCause::BlockedBy { .. })))
        .count();
    assert!(blocked >= 8, "the engine names the step before: {blocked}");
    let mut read = 0;
    for step in &not_run {
        assert_eq!(expected_words(&result, step).as_deref(), Some(STOPPED));
        if canvas(&harness).debug_hidden().contains(&step.node_id) {
            // A folded check has no card.
            continue;
        }
        let texts = results_at(&mut harness, &step.handle, 0.6);
        let Some(line) = line_of(&texts) else {
            // A Value or an IF card has no result line (3 A).
            assert!(
                matches!(step.kind, NodeKind::If | NodeKind::Value),
                "{}: {texts:?}",
                step.handle
            );
            continue;
        };
        assert_eq!(line.text, STOPPED, "{}", step.handle);
        read += 1;
    }
    assert!(read >= 5, "order, payment, receipt, refund, logout: {read}");

    let (_project, mut harness) = ran(&refund_without_sample(), WIDE);
    let result = run(&harness);
    assert_eq!(step(&result, "refund").status, StepStatus::Error);
    let logout = results_at(&mut harness, "logout", 0.6);
    assert_eq!(
        line_of(&logout).map(|line| line.text.as_str()),
        Some("Blocked · refund failed"),
        "{logout:?}"
    );

    let (_project, mut harness) = ran(&condition_unmet(), WIDE);
    let checkout = results_at(&mut harness, "checkout", 0.6);
    assert_eq!(
        line_of(&checkout).map(|line| line.text.as_str()),
        Some("Blocked · valid_cart unmet"),
        "{checkout:?}"
    );
    // The condition ran and reads *Not met* where it read *Blocked* (4 A).
    let condition = results_at(&mut harness, "valid_cart", 0.6);
    let line = line_of(&condition).expect("valid_cart's line");
    assert!(line.text.starts_with("Not met"), "{condition:?}");
}

/// Criterion 3: a step on the branch not taken reads `Skipped · outside
/// path`, in a passing and in a failing run.
#[test]
fn lay19_criterion_3_a_branch_not_taken_is_skipped() {
    let refund_fails = replaced(
        &shop_30(),
        "sample 200 { refunded: true, amount: 172.5 }",
        "sample 200 { refunded: true, amount: 170 }",
    );
    for (name, source, status) in [
        ("passing", shop_30(), RunStatus::Passed),
        ("failing", refund_fails, RunStatus::Failed),
    ] {
        let (_project, mut harness) = ran(&source, WIDE);
        let result = run(&harness);
        assert_eq!(result.status, status, "{name}");
        assert_eq!(
            step(&result, "not_created").cause,
            Some(NotRunCause::OutsidePath)
        );
        let texts = results_at(&mut harness, "not_created", 0.6);
        assert_eq!(
            line_of(&texts).map(|line| line.text.as_str()),
            Some("Skipped · outside path"),
            "{name}: {texts:?}"
        );
        assert_eq!(
            row(&harness, "not_created"),
            "not_created · Skipped · outside path"
        );
    }
}

/// Criterion 4: Run results' summary counts add up to the number of rows;
/// each not-run row reads what its card reads; the failed row reads its
/// count; a not-run step's details say why in one sentence.
#[test]
fn lay19_criterion_4_the_summary_agrees() {
    let (_project, mut harness) = ran(&failing(), WIDE);
    let result = run(&harness);
    let expected = expected_summary(&result);
    assert_eq!(
        expected,
        ["20 passed", "1 failed", "11 stopped"],
        "{result:#?}"
    );
    let mut shown = summary(&harness);
    shown.sort();
    let mut sorted = expected.clone();
    sorted.sort();
    assert_eq!(shown, sorted);

    for step in result.steps.iter().filter(|step| step.cause.is_some()) {
        assert_eq!(
            row(&harness, step.display_name()),
            format!("{} · {STOPPED}", step.display_name())
        );
    }
    assert!(
        row(&harness, "checkout_total").starts_with("checkout_total · Failed · 11 steps not run"),
        "{}",
        row(&harness, "checkout_total")
    );
    // A step that ran keeps its duration.
    assert!(
        row(&harness, "login").ends_with(" ms"),
        "{}",
        row(&harness, "login")
    );

    // The details of `order` say why it did not run.
    let order = id_of(&harness, "order");
    let document = document(&harness);
    app(&mut harness).selections.insert(document, vec![order]);
    settle(&mut harness);
    harness.step();
    assert!(
        harness
            .query_by_label(
                "Did not run: checkout_total failed and the error policy Stop ended the run."
            )
            .is_some(),
        "the details' sentence"
    );

    // A false *Data condition*: `1 not met`, its own row and count.
    let (_project, harness) = ran(&condition_unmet(), WIDE);
    let result = run(&harness);
    let expected = expected_summary(&result);
    assert!(expected.contains(&"1 not met".to_owned()), "{expected:?}");
    let mut shown = summary(&harness);
    shown.sort();
    let mut sorted = expected;
    sorted.sort();
    assert_eq!(shown, sorted);
    let unmet = caused_by(&result, "valid_cart");
    assert!(
        row(&harness, "valid_cart")
            .starts_with(&format!("valid_cart · Not met · {}", steps_not_run(unmet))),
        "{}",
        row(&harness, "valid_cart")
    );
    for step in result.steps.iter().filter(|step| step.cause.is_some()) {
        let words = expected_words(&result, step).expect("a not-run step's words");
        assert_eq!(
            row(&harness, step.display_name()),
            format!("{} · {words}", step.display_name())
        );
    }
}

/// The font a count is drawn in.
fn bold(size: f32) -> FontId {
    FontId::new(size, theme::bold_family())
}

fn width(harness: &FolderHarness, text: &str, font: FontId) -> f32 {
    harness.ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font, Color32::BLACK)
            .size()
            .x
    })
}

/// The count and the cause of every card in view sit on the result line,
/// inside the card's box; the count is whole and a cause shows at least its
/// first word.
fn assert_fits(harness: &FolderHarness, context: &str) {
    let result = run(harness);
    let mut counted = 0;
    for (handle, card, texts) in cards_in_view(harness) {
        let step = step(&result, &handle);
        if let Some(count) = count_of(&texts) {
            counted += 1;
            assert!(
                card.contains_rect(count.rect),
                "{context}: {handle}'s count {:?} leaves its card {card:?}",
                count.rect
            );
            let whole = width(harness, &count.text, bold(count.size));
            assert!(
                count.rect.width() + 0.5 >= whole,
                "{context}: {handle}'s count is cut: {} of {whole} px",
                count.rect.width()
            );
            if let Some(line) = line_of(&texts) {
                assert!(
                    line.rect.right() <= count.rect.left() + 0.5
                        || line.rect.top() >= count.rect.bottom() - 0.5
                        || line.rect.bottom() <= count.rect.top() + 0.5,
                    "{context}: {handle}'s line {:?} runs under its count {:?}",
                    line.rect,
                    count.rect
                );
            }
        }
        if step.cause.is_some()
            && let Some(line) = line_of(&texts)
        {
            assert!(
                card.contains_rect(line.rect),
                "{context}: {handle}'s cause leaves its card"
            );
            let first = line.text.split(' ').next().unwrap_or_default();
            let least = width(harness, first, FontId::proportional(line.size));
            assert!(
                line.rect.width() + 0.5 >= least,
                "{context}: {handle}'s cause shows less than {first:?}"
            );
        }
    }
    assert!(counted > 0, "{context}: a count in view");
}

/// Criterion 5: at Compact and at Full the count and the cause are on the
/// result line and inside the card's box, the count whole, a long cause cut
/// after its first word; at Overview the count is drawn only where 9 px
/// text has room, and a card that did not run has no word. Checked at
/// 1600 × 1000 and 960 × 540.
#[test]
fn lay19_criterion_5_fits_the_box() {
    // A long name for the failed check makes every cause too long.
    let long = replaced(
        &failing(),
        "  checkout_total = assert",
        "  checkout_total_matches_the_expected_amount = assert",
    );
    for size in [WIDE, crate::testing::MINIMUM] {
        for (name, source) in [("short", failing()), ("long", long.clone())] {
            let (_project, mut harness) = ran(&source, size);
            for density in ["compact", "full"] {
                if density == "full" {
                    switch_density(&mut harness);
                    settle(&mut harness);
                }
                for zoom in [0.6, 1.0] {
                    for centre in ["checkout", "order", "refund"] {
                        show(&mut harness, centre, zoom);
                        assert_fits(
                            &harness,
                            &format!("{size:?} {name} {density} {zoom} at {centre}"),
                        );
                    }
                }
            }
        }

        // At Overview: Fit, with Run results collapsed at the small size.
        let (_project, mut harness) = ran(&failing(), size);
        if size != WIDE {
            harness.get_by_label("Run results").click();
            settle(&mut harness);
        }
        harness.get_by_label(FIT).click();
        settle(&mut harness);
        harness.step();
        let mut counts = 0;
        for (handle, card, texts) in cards_in_view(&harness) {
            for text in &texts {
                assert!(
                    !["Stopped", "Blocked", "Skipped", "Cancelled"]
                        .iter()
                        .any(|word| text.text.starts_with(word)),
                    "{size:?} at Fit: {handle} writes {:?}",
                    text.text
                );
            }
            if let Some(count) = count_of(&texts) {
                counts += 1;
                assert_eq!(handle, "checkout", "{size:?}");
                assert_eq!(count.text, "11 not run");
                assert!(count.size >= 9.0, "{size:?}: {} px", count.size);
                assert!(card.contains_rect(count.rect), "{size:?}: inside the card");
                assert!(
                    count.rect.width() + 0.5 >= width(&harness, &count.text, bold(count.size)),
                    "{size:?}: whole"
                );
            }
        }
        if size == WIDE {
            assert_eq!(counts, 1, "at 29 % the failed card has room");
        }
    }
}

/// Every word a card in view or Run results write about the run: the
/// result texts of the cards around `centres`, the rows, the summary and
/// the details of `order`.
fn everything(harness: &mut FolderHarness, centres: &[&str]) -> Vec<String> {
    let mut words = Vec::new();
    for centre in centres {
        show(harness, centre, 0.6);
        for (handle, _, texts) in cards_in_view(harness) {
            for text in texts {
                words.push(format!("{centre}: {handle}: {}", text.text));
            }
        }
    }
    let result = run(harness);
    for step in &result.steps {
        words.extend(rows(harness, step.display_name()));
    }
    words.extend(summary(harness));
    let order = id_of(harness, "order");
    let document = document(harness);
    app(harness).selections.insert(document, vec![order]);
    settle(harness);
    harness.step();
    words.extend(
        harness
            .query_all_by_label_contains("Did not run")
            .filter_map(text_of),
    );
    words
}

/// Criterion 6: the window reads the cause and never a message: with every
/// `message` removed from the run, the cards and Run results read the same.
#[test]
fn lay19_criterion_6_no_message_is_read() {
    let (_project, mut harness) = ran(&failing(), WIDE);
    let centres = ["checkout", "order", "refund", "logout"];
    let before = everything(&mut harness, &centres);
    assert!(
        !before
            .iter()
            .any(|text| text.contains("outside the active execution path")),
        "the engine's sentence is not shown: {before:#?}"
    );
    assert!(before.iter().any(|text| text.starts_with("Did not run")));
    let document = document(&harness);
    app(&mut harness).debug_edit_run(&document, |run| {
        for step in &mut run.steps {
            step.message = None;
        }
    });
    settle(&mut harness);
    harness.step();
    let after = everything(&mut harness, &centres);
    assert_eq!(before, after);
}

/// Criterion 7: a run the person cancelled marks the steps not reached
/// `Cancelled`, not `Stopped`, and no card counts them.
#[test]
fn lay19_criterion_7_cancelled_is_its_own_cause() {
    let (_project, mut harness) = ran(&shop_30(), WIDE);
    let document = document(&harness);
    // What the engine writes for the steps it did not reach when the
    // signal is aborted.
    app(&mut harness).debug_edit_run(&document, |run| {
        let from = run
            .steps
            .iter()
            .position(|step| step.handle == "order")
            .expect("order");
        for step in run.steps[from..]
            .iter_mut()
            .filter(|step| step.phase == apiforge_core::run::Phase::Main)
        {
            step.status = StepStatus::Cancelled;
            step.cause = Some(NotRunCause::Cancelled);
            step.message = Some("Run cancelled before this step.".to_owned());
            step.outputs.clear();
            step.duration_ms = 0;
        }
        run.status = RunStatus::Cancelled;
    });
    settle(&mut harness);
    harness.step();
    let result = run(&harness);
    let cancelled = result
        .steps
        .iter()
        .filter(|step| step.cause == Some(NotRunCause::Cancelled))
        .count();
    assert!(cancelled > 5);
    for handle in ["order", "refund", "logout"] {
        let texts = results_at(&mut harness, handle, 0.6);
        assert_eq!(
            line_of(&texts).map(|line| line.text.as_str()),
            Some("Cancelled"),
            "{handle}: {texts:?}"
        );
        assert_eq!(row(&harness, handle), format!("{handle} · Cancelled"));
    }
    for centre in ["checkout", "order", "refund", "login"] {
        show(&mut harness, centre, 0.6);
        for (handle, _, texts) in cards_in_view(&harness) {
            assert_eq!(count_of(&texts), None, "{handle}: {texts:?}");
        }
    }
    let shown = summary(&harness);
    assert!(
        shown.contains(&format!("{cancelled} cancelled")),
        "{shown:?}"
    );
    assert!(
        !shown.iter().any(|label| label.ends_with("stopped")),
        "{shown:?}"
    );
}

/// Criterion 8: a value that fails before *Start* (the run of #418) is the
/// end of every cause: the cards read `Stopped · bad failed` and Run results
/// count them as stopped, with the engine as it is today.
#[test]
fn lay19_criterion_8_a_failure_before_start() {
    let (_project, mut harness) = ran(&value_before_start(), WIDE);
    let result = run(&harness);
    assert_eq!(result.status, RunStatus::Failed);
    assert!(
        matches!(
            step(&result, "bad").status,
            StepStatus::Failed | StepStatus::Error
        ),
        "{result:#?}"
    );
    let not_run = result
        .steps
        .iter()
        .filter(|step| step.cause.is_some())
        .count();
    assert_eq!(caused_by(&result, "bad"), not_run, "{result:#?}");
    for handle in ["login", "checkout", "logout"] {
        let texts = results_at(&mut harness, handle, 0.6);
        assert_eq!(
            line_of(&texts).map(|line| line.text.as_str()),
            Some("Stopped · bad failed"),
            "{handle}: {texts:?}"
        );
    }
    let shown = summary(&harness);
    assert!(shown.contains(&format!("{not_run} stopped")), "{shown:?}");
    let mut sorted = shown;
    sorted.sort();
    let mut expected = expected_summary(&result);
    expected.sort();
    assert_eq!(sorted, expected);
}

/// LAY-19's design-review view: `shop-30` after the failed simulation under
/// *Stop*, at 60 % around the failure and at Fit, at both window sizes, and
/// Run results maximised with `order` chosen.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay19() {
    let stage = std::env::var("LAY19_STAGE").unwrap_or_else(|_| "after".into());
    for (size, suffix) in [(WIDE, "1600x1000"), (crate::testing::MINIMUM, "960x540")] {
        let project = crate::testing::shop_project();
        fs::write(project.path().join(SCENARIO), failing()).unwrap();
        fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
        let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
        app(&mut harness).review_open(SCENARIO, None);
        settle(&mut harness);
        let document = document(&harness);
        app(&mut harness)
            .selections
            .insert(document.clone(), Vec::new());
        settle(&mut harness);
        choose(&mut harness, "Tidy up");
        settle(&mut harness);
        harness.step();
        simulate(&mut harness);
        show(&mut harness, "order", 0.6);
        crate::testing::capture(
            &mut harness,
            &format!("lay19-{stage}-stop-60-{suffix}"),
            None,
        );
        if size != WIDE {
            harness.get_by_label("Run results").click();
            settle(&mut harness);
        }
        harness.get_by_label(FIT).click();
        settle(&mut harness);
        harness.step();
        crate::testing::capture(
            &mut harness,
            &format!("lay19-{stage}-stop-fit-{suffix}"),
            None,
        );
        if size == WIDE {
            let order = id_of(&harness, "order");
            app(&mut harness).selections.insert(document, vec![order]);
            settle(&mut harness);
            harness.get_by_label("Maximise run results").click();
            settle(&mut harness);
            harness.step();
            crate::testing::capture(
                &mut harness,
                &format!("lay19-{stage}-stop-results-{suffix}"),
                None,
            );
        }
    }
}
