//! KTLO-19: the run results panel fills the window below the tab strip and
//! comes back at its height, driven through named widgets (ADR 0013); see
//! docs/tickets/KTLO-19.md. The audit of criterion 7 is in `accessibility_tests`.
use super::project_interaction_tests::choose;
use super::*;
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_within_window, folder, folder_harness, settle,
};
use egui_kittest::kittest::Queryable;

const MAXIMISE: &str = "Maximise run results";
const RESTORE: &str = "Restore run results";
/// The panel id of the maximised state, distinct from the normal one (appendix).
const MAXIMISED: &str = "report-maximised";
/// A block whose selection opens the inspector.
const BLOCK: &str = "Expected price (Value)";

/// The demo shop at `size` in `mode`, after a finished run of its scenario.
/// [`BLOCK`] is selected first, so Diagram shows its inspector in every test.
fn ran(size: egui::Vec2, mode: &str) -> (tempfile::TempDir, FolderHarness) {
    let project = shop();
    let mut harness = folder_harness(project.path(), size);
    click(&mut harness, "Diagram");
    click(&mut harness, BLOCK);
    click(&mut harness, mode);
    choose(&mut harness, "Run");
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(&harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(&mut harness);
    (project, harness)
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    settle(harness);
}

fn has(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label_contains(label).is_some()
}

/// Where the normal panel begins: the top of its bar's chevron.
fn bar_top(harness: &FolderHarness) -> f32 {
    harness.get_by_label("Run results").rect().top()
}

/// The widgets the maximised panel hides in `mode`.
fn hidden_in(mode: &str) -> &'static [&'static str] {
    match mode {
        "Code" => &["Resize project files", "Contents of"],
        "Split" => &["Resize code", "Contents of", "Diagram board", "Add step"],
        _ => &[
            "Library",
            "Diagram board",
            "Add step",
            "Zoom in",
            "Close Inspector",
        ],
    }
}

/// Criterion 1: the panel runs from the tab strip's bottom edge to the window's
/// bottom (inside the harness's own central-panel margin), and the widgets of
/// `mode` around it are gone from the tree.
fn assert_maximised(harness: &FolderHarness, mode: &str, context: &str) {
    assert!(
        has(harness, RESTORE) && !has(harness, MAXIMISE),
        "{context}: the button is *{RESTORE}*"
    );
    let load = |id: &str| {
        egui::PanelState::load(&harness.ctx, egui::Id::new(id)).map(|state| state.outer_rect)
    };
    let tabs = load("folder-tabs").expect("the tab strip is a panel");
    let panel = load(MAXIMISED).unwrap_or_else(|| panic!("{context}: no maximised panel"));
    let margin = egui::Frame::central_panel(&harness.ctx.global_style()).inner_margin;
    let bottom = harness.ctx.content_rect().bottom() - margin.bottomf();
    assert!(
        (panel.top() - tabs.bottom()).abs() <= 1.0 && (panel.bottom() - bottom).abs() <= 1.0,
        "{context}: the panel {panel:?} spans from the tab strip {tabs:?} to the bottom {bottom}"
    );
    for label in hidden_in(mode) {
        assert!(
            !has(harness, label),
            "{context}: {label:?} is not drawn while maximised"
        );
    }
}

/// Criterion 2: the bar is back where it was and so are the widgets of `mode`.
fn assert_restored(harness: &FolderHarness, mode: &str, top: f32, context: &str) {
    assert!(
        has(harness, MAXIMISE) && !has(harness, RESTORE),
        "{context}: the button is *{MAXIMISE}*"
    );
    let now = bar_top(harness);
    assert!(
        (now - top).abs() <= 1.0,
        "{context}: the panel's top returns to {top}, not {now}"
    );
    for label in hidden_in(mode) {
        assert!(has(harness, label), "{context}: {label:?} is back");
    }
}

/// Criteria 1, 2, 3 and 6: in every mode and at both windows, the button
/// maximises the panel; the button, Escape and the chevron (collapsing the
/// panel, then opening it again) restore it at its height.
#[test]
fn ktlo19_the_run_results_fill_the_window_and_come_back_at_their_height() {
    for (size, mode) in [
        (MINIMUM, "Diagram"),
        (WIDE, "Diagram"),
        (MINIMUM, "Split"),
        (WIDE, "Split"),
        (MINIMUM, "Code"),
        (WIDE, "Code"),
    ] {
        let (_project, mut harness) = ran(size, mode);
        let context = format!("{mode} at {size:?}");
        let top = bar_top(&harness);
        for restore in ["button", "Escape", "chevron"] {
            let context = format!("{context}, restored by the {restore}");
            click(&mut harness, MAXIMISE);
            assert_maximised(&harness, mode, &context);
            match restore {
                "button" => click(&mut harness, RESTORE),
                "Escape" => {
                    harness.key_press(Key::Escape);
                    settle(&mut harness);
                }
                _ => {
                    click(&mut harness, "Run results");
                    assert!(
                        !has(&harness, RESTORE),
                        "{context}: collapsed, not maximised"
                    );
                    click(&mut harness, "Run results");
                }
            }
            assert_restored(&harness, mode, top, &context);
        }
    }
}

/// Criterion 4: switching the mode or the active tab while maximised restores
/// the layout.
#[test]
fn ktlo19_switching_the_mode_or_the_tab_restores_the_layout() {
    for (from, to) in [("Diagram", "Split"), ("Split", "Code"), ("Code", "Diagram")] {
        let (_project, mut harness) = ran(WIDE, from);
        click(&mut harness, MAXIMISE);
        click(&mut harness, to);
        let top = bar_top(&harness);
        assert!(
            has(&harness, MAXIMISE) && !has(&harness, RESTORE),
            "{from} → {to}: restored"
        );
        for label in hidden_in(to) {
            assert!(has(&harness, label), "{from} → {to}: {label:?} is drawn");
        }
        click(&mut harness, MAXIMISE);
        harness.key_press(Key::Escape);
        settle(&mut harness);
        assert_restored(&harness, to, top, &format!("{from} → {to}, then Escape"));
    }

    let (_project, mut harness) = ran(WIDE, "Diagram");
    let scenario = folder(&harness).workbench.active().unwrap().to_owned();
    let component = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Component)
        .unwrap()
        .id()
        .to_owned();
    let app = harness.state_mut().as_mut().unwrap();
    app.activate_tab(&component);
    app.activate_tab(&scenario);
    settle(&mut harness);
    let top = bar_top(&harness);
    click(&mut harness, MAXIMISE);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    settle(&mut harness);
    assert_eq!(
        folder(&harness).workbench.active(),
        Some(component.as_str())
    );
    assert!(!has(&harness, RESTORE), "another tab is not maximised");
    assert!(
        has(&harness, "Diagram board"),
        "another tab shows its board"
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    settle(&mut harness);
    assert_restored(&harness, "Diagram", top, "back on the scenario's tab");
}

/// Criterion 5: a step chosen while maximised is the board's selection, with
/// its inspector open, once the layout is restored.
#[test]
fn ktlo19_a_step_chosen_while_maximised_is_selected_on_the_board() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness) = ran(size, "Diagram");
        let id = folder(&harness).workbench.active().unwrap().to_owned();
        let step = folder(&harness)
            .run_result(&id)
            .unwrap()
            .steps
            .iter()
            .find(|step| step.display_name() == "Active promotion")
            .expect("the demo runs Active promotion")
            .node_id
            .clone();
        click(&mut harness, MAXIMISE);
        harness.get_by_label_contains("Active promotion · ").click();
        settle(&mut harness);
        click(&mut harness, RESTORE);
        assert_eq!(
            folder(&harness)
                .selections
                .get(&id)
                .cloned()
                .unwrap_or_default(),
            vec![step],
            "{size:?}: the chosen step is the board's selection"
        );
        assert!(
            has(&harness, "Close Inspector"),
            "{size:?}: its inspector is open"
        );
    }
}

/// Criterion 7 (layout half): at 960 × 540 maximised, every control of the bar
/// is inside the window, and so is every step row once scrolled to.
#[test]
fn ktlo19_the_maximised_bar_and_step_list_fit_the_minimum_window() {
    for mode in ["Diagram", "Split", "Code"] {
        let (_project, mut harness) = ran(MINIMUM, mode);
        click(&mut harness, MAXIMISE);
        assert_within_window(&harness, &["Run results", RESTORE, "Export"]);
        let window = harness.ctx.content_rect();
        for policy in harness.query_all_by_role(egui::accesskit::Role::ComboBox) {
            assert!(
                window.contains_rect(policy.rect()),
                "{mode}: the policy {:?}",
                policy.rect()
            );
        }
        let id = folder(&harness).workbench.active().unwrap().to_owned();
        let rows: Vec<String> =
            crate::report::flatten_steps(&folder(&harness).run_result(&id).unwrap().steps)
                .iter()
                .map(|row| {
                    format!(
                        "{} · {} · {} ms",
                        row.step.display_name(),
                        row.step.status.label(),
                        row.step.duration_ms
                    )
                })
                .collect();
        for (index, name) in rows.iter().enumerate() {
            // Rows of one name are told apart by their order.
            let nth = rows[..index].iter().filter(|other| *other == name).count();
            harness
                .get_all_by_label(name)
                .nth(nth)
                .unwrap()
                .scroll_to_me();
            settle(&mut harness);
            let rect = harness.get_all_by_label(name).nth(nth).unwrap().rect();
            assert!(
                window.contains_rect(rect),
                "{mode}: the row {name:?} at {rect:?}"
            );
        }
    }
}

/// Design review: the bar at its normal height, then maximised, in each mode at
/// both windows (KTLO-19).
#[test]
#[ignore = "KTLO-19: design review capture; run with tools/gui/review.sh"]
fn review_run_results_maximised() {
    for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
        for mode in ["Diagram", "Split", "Code"] {
            let (_project, mut harness) = ran(size, mode);
            let name = format!("run-results-{}", mode.to_lowercase());
            crate::testing::capture(&mut harness, &format!("{name}-normal-{suffix}"), None);
            click(&mut harness, MAXIMISE);
            crate::testing::capture(&mut harness, &format!("{name}-maximised-{suffix}"), None);
        }
    }
}
