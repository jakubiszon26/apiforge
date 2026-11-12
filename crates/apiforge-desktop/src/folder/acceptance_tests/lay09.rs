//! LAY-09: semantic zoom. What a card says at Overview, Medium and Full, one
//! rule for the smallest text on screen, the failed and blocked borders at
//! every zoom, and a Fit that shows every card (Q7 = A, Q13 = A, Q15 = A;
//! re-scoped on 2026-11-10, R3 and R7).

use super::lay16::{
    SCENARIO, canvas, document, graph, id_of, lines, shop_30, show, switch_density, world,
};
use super::*;
use crate::canvas::{DrawnText, TextKind};
use crate::theme;
use apiforge_core::run::StepStatus;
use egui::{Color32, Rect};
use std::time::{Duration, Instant};

/// `source` on the board of the demo shop as a new install opens it: compact
/// cards, checks collapsed.
fn open(source: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    (project, harness)
}

fn shop_50() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-50.afs");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// `source` whose checkout answers with a total its check does not expect:
/// the request succeeds, `checkout_total` fails and the run stops there.
fn failing(source: &str, before: &str, after: &str) -> String {
    assert!(source.contains(before), "the fixture's checkout sample");
    source.replace(before, after)
}

fn failing_30() -> String {
    failing(
        &shop_30(),
        r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#,
        r#"sample 201 { orderId: "o-1", status: "created", total: 180.0 }"#,
    )
}

/// `shop-30` whose cart view answers with no items: the *Data condition*
/// `valid_cart` is not met and blocks.
fn blocked_30() -> String {
    failing(
        &shop_30(),
        r#"sample 200 { id: "c-1", total: 172.5, items: [{ productId: 123 }, { productId: 124 }] }"#,
        r#"sample 200 { id: "c-1", total: 172.5, items: [] }"#,
    )
}

fn failing_50() -> String {
    failing(
        &shop_50(),
        r#"sample 201 { orderId: "o-1", status: "created", total: 170 }"#,
        r#"sample 201 { orderId: "o-1", status: "created", total: 180 }"#,
    )
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

fn statuses(harness: &FolderHarness) -> Vec<(String, StepStatus)> {
    folder(harness)
        .fresh_run(&document(harness))
        .expect("a run")
        .steps
        .iter()
        .map(|step| (step.node_id.clone(), step.status))
        .collect()
}

fn texts(harness: &FolderHarness, handle: &str) -> Vec<DrawnText> {
    canvas(harness).debug_texts(&id_of(harness, handle))
}

/// The smallest text of `kind` a card wrote in the last frame, on screen.
fn size_of(texts: &[DrawnText], kind: TextKind) -> Option<f32> {
    texts
        .iter()
        .filter(|text| text.kind == kind)
        .map(|text| text.size)
        .reduce(f32::min)
}

fn result_text<'a>(texts: &'a [DrawnText], start: &str) -> Option<&'a DrawnText> {
    texts
        .iter()
        .find(|text| text.kind == TextKind::Result && text.text.starts_with(start))
}

/// Every card wholly in view in the last frame, with what it wrote; a card
/// off the board is not drawn.
fn drawn(harness: &FolderHarness) -> Vec<(String, Vec<DrawnText>)> {
    let board = canvas(harness);
    let view = board.debug_focus();
    board
        .debug_cards()
        .iter()
        .filter(|(_, rect)| view.contains_rect(*rect))
        .map(|(id, _)| (id.clone(), board.debug_texts(id)))
        .collect()
}

fn border(harness: &FolderHarness, id: &str) -> Option<Color32> {
    canvas(harness).debug_signal(id)
}

/// Criterion 1: at 0.35 the failed card has the red 2 px border and the
/// blocked one the amber one; at 0.25 a name is at least 11 px high on
/// screen, a step number and the result line at least 9 px.
#[test]
fn lay09_criterion_1_signals_and_minimum_sizes_at_overview() {
    // A failed check stops the run: the steps after it are skipped, none is
    // blocked. A *Data condition* that is not met blocks.
    let (_blocked_project, mut other) = open(&blocked_30(), WIDE);
    simulate(&mut other);
    show(&mut other, "valid_cart", 0.35);
    let blocked: Vec<String> = statuses(&other)
        .into_iter()
        .filter(|(_, status)| *status == StepStatus::Blocked)
        .map(|(id, _)| id)
        .filter(|id| canvas(&other).debug_card_rect(id).is_some())
        .collect();
    assert!(!blocked.is_empty(), "the run blocks a drawn step");
    for id in &blocked {
        assert_eq!(border(&other, id), Some(theme::WARNING), "blocked {id}");
    }

    let (_project, mut harness) = open(&failing_30(), WIDE);
    simulate(&mut harness);
    let checkout = id_of(&harness, "checkout");
    show(&mut harness, "checkout", 0.35);
    // Collapsed, the failed check is its subject's red border (Q15 = A).
    assert_eq!(border(&harness, &checkout), Some(theme::DANGER));
    // Every check drawn: the failed check itself is red.
    app(&mut harness).debug_draw_checks();
    settle(&mut harness);
    show(&mut harness, "checkout_total", 0.35);
    assert_eq!(
        border(&harness, &id_of(&harness, "checkout_total")),
        Some(theme::DANGER)
    );
    // A step that passed with its checks has no signal border.
    assert_eq!(border(&harness, &id_of(&harness, "login")), None);

    show(&mut harness, "login", 0.25);
    let cards = drawn(&harness);
    assert!(!cards.is_empty());
    for (id, texts) in &cards {
        let name = size_of(texts, TextKind::Name).unwrap_or_else(|| panic!("{id} has a name"));
        assert!(name >= 11.0, "{id}'s name is {name} px at 25 %");
        if let Some(line) = size_of(texts, TextKind::Result) {
            assert!(line >= 9.0, "{id}'s result line is {line} px at 25 %");
        }
    }
    // At Overview the result line is the checks pill under the name (Q5).
    let login = texts(&harness, "login");
    let pill = result_text(&login, "✓ 2 checks").expect("login's checks pill at 25 %");
    assert!(pill.size >= 9.0, "the pill is {} px", pill.size);
    // A step number takes the same rule; LAY-07's badge draws the first.
    for (id, texts) in &cards {
        if let Some(number) = size_of(texts, TextKind::Number) {
            assert!(
                number >= crate::canvas::NUMBER_MIN,
                "{id}'s number is {number} px"
            );
        }
    }
}

/// Criterion 2: no text is smaller after zooming in across a threshold: the
/// name's, the number's and the result line's heights are monotone.
#[test]
fn lay09_criterion_2_sizes_are_monotone_across_thresholds() {
    let (_project, mut harness) = open(&failing_30(), WIDE);
    simulate(&mut harness);
    for density in ["compact", "full"] {
        if density == "full" {
            switch_density(&mut harness);
            settle(&mut harness);
        }
        for (below, above) in [(0.44, 0.46), (0.74, 0.76)] {
            for handle in ["login", "cart", "begin", "first", "valid_cart", "paid"] {
                show(&mut harness, handle, below);
                let before = texts(&harness, handle);
                show(&mut harness, handle, above);
                let after = texts(&harness, handle);
                for kind in [TextKind::Name, TextKind::Number, TextKind::Result] {
                    if let Some(small) = size_of(&before, kind) {
                        let large = size_of(&after, kind).unwrap_or_else(|| {
                            panic!("{density}: {handle}'s {kind:?} vanished at {above}")
                        });
                        assert!(
                            large >= small,
                            "{density}: {handle}'s {kind:?} shrank from {small} px at \
                             {below} to {large} px at {above}"
                        );
                    }
                }
            }
        }
    }
}

/// Criterion 3: at Medium `catalogue` shows its configured query though no
/// wire feeds it; `login` shows its two result lines, grey before a run.
#[test]
fn lay09_criterion_3_compact_shows_the_configured_inputs_and_two_lines() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    show(&mut harness, "catalogue", 0.6);
    assert!(
        lines(&harness, "catalogue").contains(&"Query · 1 parameter".to_owned()),
        "{:?}",
        lines(&harness, "catalogue")
    );
    show(&mut harness, "login", 0.6);
    let before = texts(&harness, "login");
    let results: Vec<&DrawnText> = before
        .iter()
        .filter(|text| text.kind == TextKind::Result)
        .collect();
    assert_eq!(results.len(), 2, "two result lines: {before:?}");
    assert!(results.iter().all(|text| text.color == theme::MUTED));
    assert!(results.iter().any(|text| text.text == "2 checks"));
    assert!(
        results
            .iter()
            .all(|text| !text.text.contains('✓') && !text.text.contains('✕')),
        "no tick before a run: {results:?}"
    );

    simulate(&mut harness);
    show(&mut harness, "login", 0.6);
    let after = texts(&harness, "login");
    let execution = result_text(&after, "HTTP 200").expect("login's execution line");
    assert_eq!(execution.color, theme::SUCCESS);
    assert!(result_text(&after, "✓ 2 checks").is_some(), "{after:?}");
}

/// Criterion 4: a successful request with a failed check reads its HTTP
/// status and `✕ 1/3 failed` in red with the red border at 0.35, 0.60 and
/// 1.0; its band keeps the HTTP colour.
#[test]
fn lay09_criterion_4_a_failed_check_on_a_green_request() {
    let (_project, mut harness) = open(&failing_30(), WIDE);
    simulate(&mut harness);
    let checkout = id_of(&harness, "checkout");
    for zoom in [0.35, 0.6, 1.0] {
        show(&mut harness, "checkout", zoom);
        assert_eq!(
            border(&harness, &checkout),
            Some(theme::DANGER),
            "at {zoom}"
        );
        assert_eq!(
            canvas(&harness).debug_band(&checkout),
            Some(theme::SUCCESS),
            "the band keeps the HTTP colour at {zoom}"
        );
        let texts = texts(&harness, "checkout");
        let failed = result_text(&texts, "✕ 1/3 failed")
            .unwrap_or_else(|| panic!("the checks line at {zoom}: {texts:?}"));
        assert_eq!(failed.color, theme::DANGER, "red at {zoom}");
        if zoom >= 0.6 {
            let execution = result_text(&texts, "HTTP 201")
                .unwrap_or_else(|| panic!("the execution line at {zoom}: {texts:?}"));
            assert_eq!(execution.color, theme::SUCCESS, "at {zoom}");
        }
    }
}

/// Criterion 5: zooming from 0.2 to 1.0 moves no card.
#[test]
fn lay09_criterion_5_zooming_moves_no_card() {
    let (_project, mut harness) = open(&failing_30(), WIDE);
    simulate(&mut harness);
    show(&mut harness, "cart", 0.2);
    let reference = world(&harness);
    assert!(!reference.is_empty());
    for zoom in [0.25, 0.35, 0.44, 0.46, 0.6, 0.74, 0.76, 1.0] {
        show(&mut harness, "cart", zoom);
        assert_eq!(world(&harness), reference, "at {zoom}");
    }
}

/// Criterion 6: a full check card's subtitle names its subject and
/// predicate, the same words as the compact card's line (LAY-21).
#[test]
fn lay09_criterion_6_a_check_names_its_subject() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    app(&mut harness).debug_draw_checks();
    settle(&mut harness);
    show(&mut harness, "cart_created", 0.6);
    let line = "cart.status is equal to 201".to_owned();
    assert!(lines(&harness, "cart_created").contains(&line));
    switch_density(&mut harness);
    settle(&mut harness);
    for zoom in [0.6, 1.0] {
        show(&mut harness, "cart_created", zoom);
        let texts = texts(&harness, "cart_created");
        assert!(
            texts
                .iter()
                .any(|text| text.kind == TextKind::Subtitle && text.text == line),
            "the full card's subtitle at {zoom}: {texts:?}"
        );
    }
}

/// Criterion 8: Fit on `shop-50` at 960 × 540 shows every card and the
/// failed card's red border; `shop-30` collapsed at 1600 × 1000 too.
#[test]
fn lay09_criterion_8_fit_shows_every_card() {
    for (source, size, context) in [
        (
            failing_50(),
            crate::testing::MINIMUM,
            "shop-50 at 960 × 540",
        ),
        (failing_30(), WIDE, "shop-30 at 1600 × 1000"),
    ] {
        let (_project, mut harness) = open(&source, size);
        simulate(&mut harness);
        // The board as tall as the window lets it be: Run results folded to
        // its bar. Open, it leaves `shop-50` 115 px, which needs 0.026.
        harness.get_by_label("Run results").click();
        settle(&mut harness);
        harness.get_by_label("Fit the view").click();
        settle(&mut harness);
        harness.step();
        let board = canvas(&harness);
        let focus = board.debug_focus().expand(0.5);
        let cards = board.debug_cards();
        assert!(!cards.is_empty(), "{context}");
        let all = cards
            .iter()
            .map(|(_, rect)| *rect)
            .reduce(Rect::union)
            .unwrap();
        assert!(
            focus.contains_rect(all),
            "{context}: the cards' box {all:?} is in {focus:?} at {}",
            board.debug_zoom()
        );
        let checkout = id_of(&harness, "checkout");
        assert_eq!(
            border(&harness, &checkout),
            Some(theme::DANGER),
            "{context}"
        );
        assert!(
            graph(&harness).nodes.iter().any(|node| node.id == checkout),
            "{context}"
        );
    }
}

/// Whether a whole card is in the board's view, as every capture of a board
/// asserts (R4).
fn assert_a_card_in_view(harness: &FolderHarness, context: &str) {
    let board = canvas(harness);
    let view = board.debug_focus().expand(0.5);
    assert!(
        board
            .debug_cards()
            .iter()
            .any(|(_, card)| view.contains_rect(*card)),
        "{context}: a whole card is in the view"
    );
}

/// `shop-30` after a failed simulation at Fit (Overview, checks collapsed)
/// at both sizes, `shop-50` at Fit at 960 × 540, and `shop-30` at Medium and
/// at Full around the failed check's subject.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay09_zoom() {
    for (source, size, name) in [
        (failing_30(), WIDE, "lay09-shop-30-fit-1600"),
        (
            failing_30(),
            crate::testing::MINIMUM,
            "lay09-shop-30-fit-960",
        ),
        (
            failing_50(),
            crate::testing::MINIMUM,
            "lay09-shop-50-fit-960",
        ),
    ] {
        let (_project, mut harness) = open(&source, size);
        simulate(&mut harness);
        if size == crate::testing::MINIMUM {
            harness.get_by_label("Run results").click();
            settle(&mut harness);
        }
        harness.get_by_label("Fit the view").click();
        settle(&mut harness);
        harness.step();
        assert_a_card_in_view(&harness, name);
        crate::testing::capture(&mut harness, name, None);
    }
    let (_project, mut harness) = open(&failing_30(), WIDE);
    simulate(&mut harness);
    for (zoom, name) in [
        (0.6, "lay09-checkout-medium-60-1600"),
        (1.0, "lay09-checkout-full-100-1600"),
    ] {
        show(&mut harness, "checkout", zoom);
        assert_a_card_in_view(&harness, name);
        crate::testing::capture(&mut harness, name, None);
    }
}
