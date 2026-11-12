//! LAY-11: collapse checks. Check cards with a subject fold into the
//! subject's footer, which sums them up and opens their list in the
//! Inspector; a failed check stays visible as a red footer and border; a gate
//! and an `@after` stay on the board (Q4 = A, Q5 = A, Q6 = A, Q15 = A).

use super::lay16::{
    SCENARIO, assert_disjoint_cards, canvas, document, graph, id_of, lines, shop_30, show, world,
    world_of,
};
use super::*;
use crate::preferences::{Preference, Preferences};
use apiforge_core::run::StepStatus;
use egui::Rect;
use std::time::{Duration, Instant};

/// `source` on the board of the demo shop, as a new install shows it: with
/// its checks collapsed.
fn open(source: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    (project, harness)
}

/// The checks of `checkout`, which `shop-30` reads in this order.
const CHECKOUT_CHECKS: [&str; 3] = ["checkout_total", "checkout_status", "checkout_created"];

/// `shop-30` whose checkout answers with a total the check does not expect:
/// the request succeeds, `checkout_total` fails.
fn failing() -> String {
    let source = shop_30();
    let before = r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#;
    assert!(source.contains(before), "the fixture's checkout sample");
    source.replace(
        before,
        r#"sample 201 { orderId: "o-1", status: "created", total: 180.0 }"#,
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

fn status_of(harness: &FolderHarness, handle: &str) -> Option<StepStatus> {
    let id = id_of(harness, handle);
    folder(harness)
        .fresh_run(&document(harness))?
        .steps
        .iter()
        .find(|step| step.node_id == id)
        .map(|step| step.status)
}

fn aggregate(harness: &FolderHarness, handle: &str) -> String {
    canvas(harness)
        .debug_aggregate(&id_of(harness, handle))
        .unwrap_or_else(|| panic!("{handle} has no checks line"))
}

fn hidden(harness: &FolderHarness, handle: &str) -> bool {
    canvas(harness)
        .debug_hidden()
        .contains(&id_of(harness, handle))
}

/// *Collapse checks* in the board's view menu, beside *Compact cards*.
fn switch_checks(harness: &mut FolderHarness) {
    harness.get_by_label("Board view").click();
    settle(harness);
    harness.get_by_label("Collapse checks").click();
    settle(harness);
    harness.step();
}

/// A click on the checks line in the footer of `handle`.
fn open_aggregate(harness: &mut FolderHarness, handle: &str) {
    show(harness, handle, 1.0);
    harness.get_by_label(&format!("Checks of {handle}")).click();
    settle(harness);
    harness.step();
}

fn selected(harness: &FolderHarness) -> Vec<String> {
    folder(harness)
        .selections
        .get(&document(harness))
        .cloned()
        .unwrap_or_default()
}

fn has(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label(label).is_some()
}

/// Criterion 1: after a failed simulation the failed check is folded, and
/// its subject's footer and border are red at 0.35, 0.60 and 1.0.
#[test]
fn lay11_criterion_1_a_failed_check_is_visible_collapsed() {
    let (_project, mut harness) = open(&failing(), WIDE);
    assert!(
        folder(&harness).preferences.checks_collapsed,
        "a new install opens with checks collapsed"
    );
    simulate(&mut harness);
    assert_eq!(status_of(&harness, "checkout"), Some(StepStatus::Passed));
    assert_eq!(
        status_of(&harness, "checkout_total"),
        Some(StepStatus::Failed)
    );
    let checkout = id_of(&harness, "checkout");
    for zoom in [0.35, 0.6, 1.0] {
        show(&mut harness, "checkout", zoom);
        assert!(hidden(&harness, "checkout_total"), "folded at {zoom}");
        assert!(
            canvas(&harness).debug_marked().contains(&checkout),
            "checkout is marked red at {zoom}"
        );
        assert!(
            canvas(&harness).debug_card_rect(&checkout).is_some(),
            "checkout is drawn at {zoom}"
        );
        let text = aggregate(&harness, "checkout");
        assert!(text.starts_with("✕ 1/3 failed"), "at {zoom}: {text}");
        if zoom >= 0.6 {
            assert!(
                lines(&harness, "checkout").contains(&text),
                "the footer reads {text} at {zoom}: {:?}",
                lines(&harness, "checkout")
            );
        }
    }
    // A step whose checks all passed is not marked.
    assert!(
        !canvas(&harness)
            .debug_marked()
            .contains(&id_of(&harness, "login"))
    );
}

/// Criterion 2: before a run every checks line is grey and has no tick;
/// part of the way through a run it names the state of every check.
#[test]
fn lay11_criterion_2_no_tick_before_a_run() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    show(&mut harness, "cart", 0.6);
    assert_eq!(aggregate(&harness, "cart"), "3 checks");
    assert_eq!(aggregate(&harness, "checkout"), "3 checks");
    assert_eq!(aggregate(&harness, "login"), "2 checks");
    for node in graph(&harness).nodes {
        if let Some(text) = canvas(&harness).debug_aggregate(&node.id) {
            assert!(
                !text.contains('✓') && !text.contains('✕'),
                "{} before a run: {text}",
                node.handle
            );
        }
    }
    assert!(canvas(&harness).debug_marked().is_empty());

    simulate(&mut harness);
    assert_eq!(aggregate(&harness, "login"), "✓ 2 checks");
    // The run as it stood when `logged_in` had passed and `known_user` had
    // not run yet.
    let known_user = id_of(&harness, "known_user");
    let order: Vec<String> = folder(&harness)
        .fresh_run(&document(&harness))
        .unwrap()
        .steps
        .iter()
        .map(|step| step.node_id.clone())
        .collect();
    let cut = order
        .iter()
        .position(|id| *id == known_user)
        .expect("known_user ran");
    let kept: Vec<String> = order[..cut].to_vec();
    let id = document(&harness);
    app(&mut harness).debug_cut_run(&id, |step| kept.contains(&step.node_id));
    settle(&mut harness);
    show(&mut harness, "login", 0.6);
    assert_eq!(aggregate(&harness, "login"), "1 ✓ · 1 waiting");
    assert!(
        lines(&harness, "login").contains(&"1 ✓ · 1 waiting".to_owned()),
        "{:?}",
        lines(&harness, "login")
    );
}

/// Criterion 3: *Collapse checks* moves no card that stays drawn and no
/// camera, and is remembered.
#[test]
fn lay11_criterion_3_the_toggle_moves_nothing_and_is_remembered() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    show(&mut harness, "cart", 0.6);
    let collapsed = world(&harness);
    let camera = canvas(&harness).debug_camera();
    assert!(hidden(&harness, "cart_created"));

    switch_checks(&mut harness);
    assert!(!folder(&harness).preferences.checks_collapsed);
    assert!(!hidden(&harness, "cart_created"));
    assert!(
        world(&harness).len() > collapsed.len(),
        "the checks are drawn again"
    );
    for (handle, rect) in &collapsed {
        assert_eq!(world_of(&harness, handle), *rect, "{handle} stays");
    }
    assert_eq!(canvas(&harness).debug_camera(), camera, "the camera stays");
    // Remembered: a board opened from now on shows its checks.
    assert!(!app(&mut harness).new_canvas().debug_collapsed());

    switch_checks(&mut harness);
    assert!(folder(&harness).preferences.checks_collapsed);
    assert_eq!(world(&harness), collapsed);
    assert_eq!(canvas(&harness).debug_camera(), camera);
    assert!(app(&mut harness).new_canvas().debug_collapsed());

    // And across a restart, through the preferences file.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let mut preferences = Preferences::at(Some(path.clone()));
    assert!(preferences.checks_collapsed, "collapsed by default");
    preferences.checks_collapsed = false;
    preferences.save(Preference::ChecksCollapsed);
    assert!(!Preferences::at(Some(path)).checks_collapsed);
}

/// Criterion 4: a click on `checkout`'s checks line selects it and opens the
/// Inspector's list with *Expand checks here*.
#[test]
fn lay11_criterion_4_the_aggregate_opens_the_list() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    open_aggregate(&mut harness, "checkout");
    assert_eq!(selected(&harness), vec![id_of(&harness, "checkout")]);
    for check in CHECKOUT_CHECKS {
        assert!(has(&harness, &format!("Check {check}")), "{check} listed");
    }
    assert!(has(&harness, "Expand checks here"));
}

/// Criterion 5: *Expand checks here* hangs `checkout`'s checks under it,
/// shifts the rows below by their height and keeps the camera; a restart
/// forgets it.
#[test]
fn lay11_criterion_5_expand_here_shifts_the_rows_below() {
    let (project, mut harness) = open(&shop_30(), WIDE);
    open_aggregate(&mut harness, "checkout");
    let before = world(&harness);
    let camera = canvas(&harness).debug_camera();
    let subject = world_of(&harness, "checkout");

    harness.get_by_label("Expand checks here").click();
    settle(&mut harness);
    harness.step();
    let after = world(&harness);
    assert_eq!(canvas(&harness).debug_camera(), camera, "the camera stays");
    assert_eq!(world_of(&harness, "checkout"), subject, "checkout stays");

    let mut top = subject.bottom();
    let mut height = 0.0;
    for check in CHECKOUT_CHECKS {
        assert!(!hidden(&harness, check), "{check} is drawn");
        let rect = world_of(&harness, check);
        assert_eq!(rect.left(), subject.left(), "{check} hangs under checkout");
        assert!(rect.top() > top, "{check} hangs in order");
        top = rect.bottom();
        height = rect.bottom() - subject.bottom();
    }
    assert!(height > 0.0);
    let (below, _) = before
        .iter()
        .find(|(_, rect)| rect.top() >= subject.bottom())
        .expect("a row lies below checkout");
    let shift = world_of_in(&after, below).top() - world_of_in(&before, below).top();
    assert!(
        shift >= height,
        "the rows below make room: {shift} for {height}"
    );
    for (handle, rect) in &before {
        let now = world_of_in(&after, handle);
        if rect.top() >= subject.bottom() {
            assert_eq!(*now, rect.translate(egui::vec2(0.0, shift)), "{handle}");
        } else {
            assert_eq!(now, rect, "{handle} above or beside stays");
        }
    }
    assert_disjoint_cards(&after, "with checkout's checks expanded");

    // Forgotten on restart: the same project opened again folds them.
    assert!(
        app(&mut harness)
            .new_canvas()
            .debug_expanded_here()
            .is_empty()
    );
    drop(harness);
    let mut again = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut again).review_open(super::lay16::SCENARIO, None);
    settle(&mut again);
    again.step();
    for check in CHECKOUT_CHECKS {
        assert!(hidden(&again, check), "{check} is folded after a restart");
    }
}

fn world_of_in<'a>(cards: &'a [(String, Rect)], handle: &str) -> &'a Rect {
    cards
        .iter()
        .find(|(name, _)| name == handle)
        .map(|(_, rect)| rect)
        .unwrap_or_else(|| panic!("{handle} is drawn"))
}

/// Criterion 6: `receipt_total` reads `cart` and `receipt`, so it is counted
/// under both, and the Inspector names the other subject.
#[test]
fn lay11_criterion_6_a_shared_check_is_counted_twice() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    show(&mut harness, "cart", 0.6);
    assert!(hidden(&harness, "receipt_total"));
    assert_eq!(aggregate(&harness, "cart"), "3 checks");
    assert_eq!(aggregate(&harness, "receipt"), "1 check");

    open_aggregate(&mut harness, "receipt");
    assert_eq!(selected(&harness), vec![id_of(&harness, "receipt")]);
    assert!(has(&harness, "Check receipt_total"));
    assert!(has(&harness, "Shared with: cart"));

    open_aggregate(&mut harness, "cart");
    assert!(has(&harness, "Check receipt_total"));
    assert!(has(&harness, "Shared with: receipt"));
}

/// Criterion 7: the gate `valid_cart`, whose value `checkout` and `order`
/// read, stays with its wires; an `@after` on a folded check is drawn from
/// its subject with the check's name.
#[test]
fn lay11_criterion_7_gates_and_after_wires_stay() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    show(&mut harness, "valid_cart", 0.6);
    assert!(!hidden(&harness, "valid_cart"), "the gate is drawn");
    assert!(world_of(&harness, "valid_cart").height() < world_of(&harness, "cart").height());
    let (valid_cart, checkout, order) = (
        id_of(&harness, "valid_cart"),
        id_of(&harness, "checkout"),
        id_of(&harness, "order"),
    );
    let wires = canvas(&harness).debug_wires().to_vec();
    for target in [&checkout, &order] {
        assert!(
            wires
                .iter()
                .any(|wire| wire.source.as_ref() == Some(&valid_cart) && &wire.target == target),
            "the gate's wire to {target} is drawn"
        );
    }

    for (check, subject, step) in [
        ("cart_total", "cart", "address"),
        ("refund_amount", "refund", "logout"),
    ] {
        assert!(hidden(&harness, check), "{check} is folded");
        let (subject_id, step_id) = (id_of(&harness, subject), id_of(&harness, step));
        let wire = wires
            .iter()
            .find(|wire| wire.target == step_id && wire.label.is_some())
            .unwrap_or_else(|| panic!("the @after of {step} is drawn: {wires:#?}"));
        assert_eq!(wire.source.as_ref(), Some(&subject_id), "from {subject}");
        assert!(
            wire.label.as_ref().unwrap().contains(check),
            "named {check}: {:?}",
            wire.label
        );
    }
    let check_ids: Vec<String> = ["cart_created", "cart_total", "checkout_total"]
        .into_iter()
        .map(|handle| id_of(&harness, handle))
        .collect();
    assert!(
        wires.iter().all(|wire| !check_ids.contains(&wire.target)
            && wire
                .source
                .as_ref()
                .is_none_or(|source| !check_ids.contains(source))),
        "no wire ends on a folded check"
    );
}

/// Criterion 8: the Run results row of a folded check selects its subject
/// and opens the list.
#[test]
fn lay11_criterion_8_run_results_navigates_to_the_subject() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    simulate(&mut harness);
    assert!(hidden(&harness, "logged_in"));
    harness.get_by_label_contains("logged_in · Passed").click();
    settle(&mut harness);
    harness.step();
    assert_eq!(selected(&harness), vec![id_of(&harness, "login")]);
    assert!(has(&harness, "Check logged_in"), "the list is open");
    assert!(has(&harness, "Check known_user"));
}

/// Criterion 9: Tidy up with checks collapsed places the folded checks, so
/// showing them needs no second Tidy up.
#[test]
fn lay11_criterion_9_tidy_up_hangs_hidden_checks() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    assert!(folder(&harness).preferences.checks_collapsed);
    harness.get_by_label("Tidy up").click();
    settle(&mut harness);
    harness.step();
    assert_disjoint_cards(&world(&harness), "shop-30 tidied with checks collapsed");

    switch_checks(&mut harness);
    let expanded = world(&harness);
    assert_eq!(
        expanded.len(),
        graph(&harness).nodes.len(),
        "every card is drawn"
    );
    assert_disjoint_cards(&expanded, "the folded checks shown");

    harness.get_by_label("Tidy up").click();
    settle(&mut harness);
    harness.step();
    assert_eq!(
        world(&harness),
        expanded,
        "Tidy up with the checks shown moves nothing"
    );
}

/// Design review: `shop-30` collapsed at Fit, a failed check's red footer at
/// 60 %, and `checkout`'s checks expanded here.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay11_board() {
    let assert_cards_in_view = |harness: &FolderHarness, context: &str| {
        let board = canvas(harness);
        let viewport = board.debug_viewport();
        assert!(
            board
                .debug_cards()
                .iter()
                .any(|(_, card)| viewport.contains_rect(*card)),
            "{context}: a whole card is in the viewport"
        );
    };
    let (_project, mut harness) = open(&failing(), WIDE);
    harness.get_by_label("Tidy up").click();
    settle(&mut harness);
    // Tidy up leaves the camera where it was until LAY-06.
    choose(&mut harness, "Fit the view");
    harness.step();
    assert_cards_in_view(&harness, "shop-30 collapsed and tidied");
    crate::testing::capture(&mut harness, "lay11-collapsed-fit-1600", None);

    simulate(&mut harness);
    show(&mut harness, "checkout", 0.6);
    assert_cards_in_view(&harness, "checkout at 60 %");
    crate::testing::capture(&mut harness, "lay11-failed-check-60-1600", None);

    open_aggregate(&mut harness, "checkout");
    harness.get_by_label("Expand checks here").click();
    settle(&mut harness);
    harness.step();
    show(&mut harness, "checkout_status", 0.6);
    assert_cards_in_view(&harness, "checkout expanded here");
    crate::testing::capture(&mut harness, "lay11-expand-here-60-1600", None);

    let (_project, mut harness) = open(&failing(), crate::testing::MINIMUM);
    simulate(&mut harness);
    show(&mut harness, "checkout", 0.6);
    assert_cards_in_view(&harness, "checkout at 60 % at 960");
    crate::testing::capture(&mut harness, "lay11-failed-check-60-960", None);
}
