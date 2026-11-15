//! LAY-07 (#291): every step carries the engine's sequence number in a badge
//! at its card's top-left, the number of its row in Run results, checks
//! included (Q2 = A). While a run is on, the running step's badge holds an
//! activity glyph and its footer reads *Running…*, apart from the selection;
//! a step not reached yet is muted and shows its previous result as stale.

use super::lay16::{canvas, document, graph, id_of, select, show};
use super::*;
use crate::canvas::{BadgeState, DrawnBadge, DrawnText, TextKind};
use crate::theme;
use apiforge_core::{graph::sequence, run::Phase};
use apiforge_runner::RunnerService;
use egui::Rect;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// `source` on the board of the demo shop as a new install opens it, with
/// the runner of `gate`: compact cards, checks collapsed unless `checks`.
fn open(
    source: &str,
    size: egui::Vec2,
    checks: bool,
) -> (tempfile::TempDir, FolderHarness, Arc<Gate>) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    opened(project, size, checks)
}

/// The same with the board as *Tidy up* lays it out, nothing selected.
fn tidied(source: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, Arc<Gate>) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
    let (project, mut harness, gate) = opened(project, size, false);
    let document = document(&harness);
    app(&mut harness).selections.insert(document, Vec::new());
    settle(&mut harness);
    choose(&mut harness, "Tidy up");
    harness.step();
    (project, harness, gate)
}

fn opened(
    project: tempfile::TempDir,
    size: egui::Vec2,
    checks: bool,
) -> (tempfile::TempDir, FolderHarness, Arc<Gate>) {
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    if checks {
        app(&mut harness).debug_draw_checks();
    }
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let gate = Arc::new(Gate::default());
    let notify = gate.clone();
    app(&mut harness).runs.runner =
        Some(RunnerService::new(Arc::new(move || notify.notify())).expect("the runner starts"));
    (project, harness, gate)
}

/// The window's runner announces each step that starts and each result
/// through its observer. Held, the observer keeps the runner's thread at
/// the publication it was given, so a real run stops at any point and the
/// window's frames can be looked at in between.
#[derive(Default)]
struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}

#[derive(Default)]
struct GateState {
    /// Publications the runner has made.
    calls: usize,
    /// Publications the runner may go past.
    passed: usize,
    held: bool,
}

impl Gate {
    fn notify(&self) {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        let call = state.calls;
        self.changed.notify_all();
        while state.held && state.passed < call {
            state = self.changed.wait(state).unwrap();
        }
    }

    /// Holds the runner at its next publication.
    fn hold(&self) {
        let mut state = self.state.lock().unwrap();
        state.held = true;
        state.passed = state.calls;
    }

    /// Lets the runner run to its end.
    fn open(&self) {
        self.state.lock().unwrap().held = false;
        self.changed.notify_all();
    }

    /// Lets the runner past the publication it is held at.
    fn release(&self) {
        let mut state = self.state.lock().unwrap();
        state.passed = state.calls;
        self.changed.notify_all();
    }

    /// Waits until the runner is held at a publication not yet passed;
    /// `false` when the run ended first.
    fn wait_held(&self, harness: &FolderHarness) -> bool {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut state = self.state.lock().unwrap();
        while state.calls <= state.passed {
            let finished = folder(harness)
                .runs
                .handle
                .as_ref()
                .is_none_or(|handle| handle.is_finished());
            if finished {
                return false;
            }
            assert!(Instant::now() < deadline, "the runner publishes");
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(20))
                .unwrap()
                .0;
        }
        true
    }
}

/// Starts a simulation and holds it at the publication that announces
/// `handle` started: the runner names it running and has not executed it,
/// and the window has drawn one frame since.
fn run_until_started(harness: &mut FolderHarness, gate: &Gate, handle: &str) {
    let target = id_of(harness, handle);
    gate.hold();
    choose(harness, "Run simulation");
    loop {
        assert!(gate.wait_held(harness), "the run reaches {handle}");
        // The frame reads the runner's publication before it draws.
        harness.step();
        let running = folder(harness)
            .fresh_run(&document(harness))
            .and_then(|run| run.running);
        if running.as_deref() == Some(target.as_str()) {
            // The first frame after the runner's *started*.
            return;
        }
        gate.release();
    }
}

/// Lets a held run end and waits for its last report.
fn finish(harness: &mut FolderHarness, gate: &Gate) {
    gate.open();
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
    harness.step();
}

fn simulate(harness: &mut FolderHarness, gate: &Gate) {
    gate.open();
    choose(harness, "Run simulation");
    finish(harness, gate);
}

fn badge(harness: &FolderHarness, handle: &str) -> DrawnBadge {
    canvas(harness)
        .debug_badge(&id_of(harness, handle))
        .unwrap_or_else(|| panic!("{handle} has no badge"))
}

fn texts(harness: &FolderHarness, handle: &str) -> Vec<DrawnText> {
    canvas(harness).debug_texts(&id_of(harness, handle))
}

/// The result line `handle`'s card wrote in the last frame.
fn result_line(harness: &FolderHarness, handle: &str) -> DrawnText {
    texts(harness, handle)
        .into_iter()
        .find(|text| text.kind == TextKind::Result)
        .unwrap_or_else(|| panic!("{handle} wrote no result line"))
}

fn fit(harness: &mut FolderHarness) {
    choose(harness, "Fit the view");
    harness.step();
}

/// Criterion 1: after a simulation, every step's badge is the number of its
/// row in Run results, checks included, on the three reference scenarios.
#[test]
fn lay07_criterion_1_the_badge_equals_the_row() {
    for name in ["shop-30", "order-flow", "shop-50"] {
        let (_project, mut harness, gate) = open(&fixture(name), WIDE, true);
        simulate(&mut harness, &gate);
        let run = folder(&harness)
            .fresh_run(&document(&harness))
            .expect("a fresh run");
        // The board as tall as the window lets it be, every card in view.
        harness.get_by_label("Run results").click();
        settle(&mut harness);
        fit(&mut harness);
        let rows: Vec<&str> = run
            .steps
            .iter()
            .filter(|step| step.phase == Phase::Main)
            .map(|step| step.node_id.as_str())
            .collect();
        assert_eq!(
            rows.len(),
            graph(&harness).nodes.len(),
            "{name}: every step has a row"
        );
        for (index, id) in rows.iter().enumerate() {
            let badge = canvas(&harness)
                .debug_badge(id)
                .unwrap_or_else(|| panic!("{name}: step {id} has no badge"));
            assert_eq!(
                badge.text,
                (index + 1).to_string(),
                "{name}: the badge of {id} is its row"
            );
            assert_eq!(badge.state, BadgeState::Order, "{name}: {id}");
        }
    }
}

/// Criterion 2: a step added in code before step 5 renumbers the badges in
/// the next frame, and they stay the engine's order.
#[test]
fn lay07_criterion_2_badges_follow_an_edit() {
    let source = fixture("shop-30");
    let (_project, mut harness, _gate) = open(&source, WIDE, true);
    show(&mut harness, "catalogue", 0.6);
    assert_eq!(badge(&harness, "catalogue").text, "5", "step 5 before");
    show(&mut harness, "known_user", 0.6);
    assert_eq!(badge(&harness, "known_user").text, "4");

    let anchor = "  catalogue = GET \"/products\" {";
    assert!(source.contains(anchor), "the fixture's catalogue");
    let edited = source.replacen(
        anchor,
        &format!("  probe = GET \"/probe\" {{\n    sample 200 {{}}\n  }}\n{anchor}"),
        1,
    );
    let document = document(&harness);
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&document, &edited, CODE)
        .unwrap();
    settle(&mut harness);
    show(&mut harness, "probe", 0.6);
    assert_eq!(badge(&harness, "probe").text, "5", "the new step is 5");
    show(&mut harness, "catalogue", 0.6);
    assert_eq!(badge(&harness, "catalogue").text, "6", "step 5 is now 6");
    show(&mut harness, "known_user", 0.6);
    assert_eq!(
        badge(&harness, "known_user").text,
        "4",
        "before it, unchanged"
    );
    let graph = graph(&harness);
    let mut checked = 0;
    for (index, id) in sequence(&graph).into_iter().enumerate() {
        if let Some(badge) = canvas(&harness).debug_badge(id) {
            assert_eq!(badge.text, (index + 1).to_string(), "{id}");
            checked += 1;
        }
    }
    assert!(checked >= 5, "badges in view: {checked}");
}

/// Criterion 3: during a real simulation held at the runner's *started* of
/// `cart_view`, its badge holds the activity glyph and its footer says
/// *Running…* one frame later, while `cart`, selected, keeps the selection
/// frame and its own number.
#[test]
fn lay07_criterion_3_the_running_step_is_marked_apart_from_the_selection() {
    let (_project, mut harness, gate) = open(&fixture("shop-30"), WIDE, false);
    select(&mut harness, &["cart"]);
    settle(&mut harness);
    show(&mut harness, "cart_view", 0.6);
    run_until_started(&mut harness, &gate, "cart_view");

    let running = badge(&harness, "cart_view");
    assert_eq!(
        running.state,
        BadgeState::Running,
        "the glyph, not a number"
    );
    let line = result_line(&harness, "cart_view");
    assert_eq!(line.text, "Running…");
    assert_ne!(line.color, theme::ACCENT, "not the selection's colour");
    assert!(
        !canvas(&harness).debug_selected(&id_of(&harness, "cart_view")),
        "the running step is not the selection"
    );

    let cart = id_of(&harness, "cart");
    assert!(
        canvas(&harness).debug_selected(&cart),
        "cart keeps the selection frame"
    );
    let selected = badge(&harness, "cart");
    assert_eq!(selected.state, BadgeState::Order, "cart ran: its number");
    assert_ne!(
        running.fill, selected.fill,
        "the running badge is told apart from a finished one"
    );
    assert_ne!(running.fill, theme::ACCENT, "nor shares the selection's");
    // A step not reached yet is muted.
    show_held(&mut harness, "address", 0.6);
    assert_eq!(badge(&harness, "address").state, BadgeState::Pending);

    finish(&mut harness, &gate);
    show(&mut harness, "cart_view", 0.6);
    assert_eq!(badge(&harness, "cart_view").state, BadgeState::Order);
    assert_ne!(result_line(&harness, "cart_view").text, "Running…");
}

/// Criterion 4: while a second run is on, a card the run has not reached
/// shows the previous run's result muted and marked as previous; a card it
/// has reached shows the current result.
#[test]
fn lay07_criterion_4_stale_results_are_told_from_current_ones() {
    let (_project, mut harness, gate) = open(&fixture("shop-30"), WIDE, false);
    simulate(&mut harness, &gate);
    show(&mut harness, "cart_view", 0.6);
    let before = result_line(&harness, "address");
    assert!(before.text.starts_with("HTTP 200"), "{before:?}");

    run_until_started(&mut harness, &gate, "cart_view");
    // Reached: this run's result, in its own colour.
    let cart = result_line(&harness, "cart");
    assert!(cart.text.starts_with("HTTP 201"), "{cart:?}");
    assert_eq!(cart.color, theme::SUCCESS);
    assert_eq!(badge(&harness, "cart").state, BadgeState::Order);
    // Not reached: the previous run's result, muted and marked.
    let address = result_line(&harness, "address");
    assert_eq!(
        address.text,
        format!("Previous run · {}", before.text),
        "the previous result, marked"
    );
    assert_eq!(address.color, theme::MUTED, "muted");
    assert_eq!(badge(&harness, "address").state, BadgeState::Pending);

    finish(&mut harness, &gate);
    show(&mut harness, "cart_view", 0.6);
    let after = result_line(&harness, "address");
    assert!(after.text.starts_with("HTTP 200"), "{after:?}");
    assert_eq!(after.color, theme::SUCCESS, "current again");
}

/// Criterion 5: at Fit of `shop-30` at 1600 × 1000 a two-digit badge is at
/// least 9 px high on screen and overlaps neither its card's name nor a
/// chip or a count of LAY-23.
#[test]
fn lay07_criterion_5_the_badge_has_a_minimum_size() {
    let (_project, harness, _gate) = open(&fixture("shop-30"), WIDE, false);
    let (_tidied_project, tidied_harness, _tidied_gate) = tidied(&fixture("shop-30"), WIDE);
    for (context, mut harness) in [("as opened", harness), ("tidied", tidied_harness)] {
        fit(&mut harness);
        assert_badges_clear(&harness, context);
    }
}

fn assert_badges_clear(harness: &FolderHarness, context: &str) {
    let board = canvas(harness);
    let view = board.debug_focus().expand(0.5);
    let mut two_digits = 0;
    let chips: Vec<Rect> = board
        .debug_tags()
        .iter()
        .map(|tag| tag.rect)
        .chain(
            board
                .debug_counts()
                .iter()
                .map(|count| board.debug_to_screen(count.world)),
        )
        .collect();
    for (id, rect) in board.debug_cards() {
        if !view.contains_rect(*rect) {
            continue;
        }
        let badge = board
            .debug_badge(id)
            .unwrap_or_else(|| panic!("{id} has no badge"));
        let texts = board.debug_texts(id);
        if badge.text.len() == 2 {
            two_digits += 1;
            let number = texts
                .iter()
                .find(|text| text.kind == TextKind::Number)
                .unwrap_or_else(|| panic!("{id} writes its number"));
            assert!(
                number.size >= crate::canvas::NUMBER_MIN,
                "{id}'s number is {} px",
                number.size
            );
            assert!(
                badge.rect.height() >= 9.0,
                "{id}'s badge is {} px",
                badge.rect.height()
            );
        }
        for name in texts.iter().filter(|text| text.kind == TextKind::Name) {
            assert!(
                !badge.rect.intersects(name.rect),
                "{context}, {id}: the badge {:?} overlaps the name {:?}",
                badge.rect,
                name.rect
            );
        }
        for chip in &chips {
            assert!(
                !badge.rect.intersects(*chip),
                "{context}, {id}: the badge {:?} overlaps a chip or a count {chip:?}",
                badge.rect
            );
        }
    }
    assert!(
        two_digits >= 10,
        "{context}: two-digit badges in view: {two_digits}"
    );
}

/// Criterion 6, design review: `shop-30` at 60 % during a second run held at
/// `cart_view`, with `cart` selected: finished, running and previous results
/// side by side, at 1600 × 1000 and 960 × 540; and the badges at Fit.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay07() {
    let assert_a_card_in_view = |harness: &FolderHarness, context: &str| {
        let board = canvas(harness);
        let view = board.debug_focus().expand(0.5);
        assert!(
            board
                .debug_cards()
                .iter()
                .any(|(_, card)| view.contains_rect(*card)),
            "{context}: a whole card is in the view"
        );
    };
    for (size, name) in [
        (WIDE, "lay07-running-60-1600"),
        (crate::testing::MINIMUM, "lay07-running-60-960"),
    ] {
        let (_project, mut harness, gate) = tidied(&fixture("shop-30"), size);
        simulate(&mut harness, &gate);
        select(&mut harness, &["cart"]);
        settle(&mut harness);
        run_until_started(&mut harness, &gate, "cart_view");
        // The board as tall as the window lets it be.
        harness.get_by_label("Run results").click();
        show_held(&mut harness, "cart_view", 0.6);
        assert_a_card_in_view(&harness, name);
        assert_eq!(badge(&harness, "cart_view").state, BadgeState::Running);
        crate::testing::capture(&mut harness, name, None);
        finish(&mut harness, &gate);
    }
    let (_project, mut harness, _gate) = tidied(&fixture("shop-30"), WIDE);
    fit(&mut harness);
    assert_a_card_in_view(&harness, "lay07-fit-1600");
    crate::testing::capture(&mut harness, "lay07-fit-1600", None);
}

/// Review of #338: the previous run shown beside a run is one of the same
/// mode, as the engine reuses previous results only for the same mode; a
/// simulation's results are never the muted past of a live run.
#[test]
fn lay07_a_previous_run_of_another_mode_is_not_shown() {
    use apiforge_core::run::{RunMode, RunResult, RunStatus};
    let run = |id: &str, mode: RunMode| RunResult {
        id: id.into(),
        scenario_id: "scen-1".into(),
        mode,
        started_at: "2026-11-15T00:00:00.000Z".into(),
        finished_at: None,
        status: RunStatus::Running,
        steps: Vec::new(),
        diagnostic: false,
        snapshot: None,
        error: None,
        notice: None,
        running: None,
    };
    let live = run("run-2", RunMode::Live);
    let simulated = run("run-1", RunMode::Simulation);
    assert!(super::super::runs::previous_of(&live, Some(&simulated)).is_none());
    let earlier = run("run-0", RunMode::Live);
    assert_eq!(
        super::super::runs::previous_of(&live, Some(&earlier)).map(|run| run.id.as_str()),
        Some("run-0")
    );
}

/// The camera centred on `handle` at `zoom` while the run is held, a few
/// frames and no `settle`, whose wait the held runner would not end.
fn show_held(harness: &mut FolderHarness, handle: &str, zoom: f32) {
    let id = id_of(harness, handle);
    super::lay16::canvas_mut(harness).debug_show(&id, zoom);
    for _ in 0..4 {
        harness.step();
    }
}
