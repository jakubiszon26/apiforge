//! UX-42 (#520, the owner's decisions of 2026-11-21): the run mode is a
//! switch in the header. *Simulation | Live* stands left of the Run button,
//! which reads *▶ Run* in both modes, or ▶ alone where the header's row is
//! too short for the word (question 12 = A). The chevron *Run options* and
//! its menu are gone; *Review legacy runs* is an item of the Project menu,
//! and the *Runs* button that held only it goes with it
//! (docs/tickets/UX-42.md). Every test uses the demo shop and a
//! configuration directory of its own.
use super::super::accessibility_tests::tab_order;
use super::mcp08b::contains;
use super::mcp09::{agent_holds, manifest_id, set_box, window};
use super::ux02::{hover_shows, title};
use super::*;
use crate::testing::{MINIMUM, assert_within_window};
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::NodeT;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The Run button's accessible names: its mode's, as UX-02 named it.
const SIMULATE: &str = "Run simulation";
const LIVE: &str = "Run live";
/// The switch, its parts and their hover texts (UX-02's hints).
const GROUP: &str = "Run mode";
const SIMULATION: &str = "Simulation";
const LIVE_PART: &str = "Live";
const SIMULATION_HINT: &str = "Sample responses; no request is sent.";
const LIVE_HINT: &str = "Requests go to the project's API.";
/// UX-02's hover texts of the Run button.
const SIMULATE_HOVER: &str = "Simulate the selected scenario or component with sample responses, including unsaved valid code";
const LIVE_HOVER: &str = "Send the requests of the selected scenario or component to the API, including unsaved valid code";
/// What goes (decided on #520).
const OPTIONS: &str = "Run options";
const CAPTION: &str = "RUN MODE";
const LINE: &str = "The Run button runs this mode until you change it.";
const RUNS: &str = "Runs";
const REVIEW: &str = "Review legacy runs";
const BADGE: &str = "Live allowed";
/// The caption beside *Publish* on the demo's unchanged component.
const UNCHANGED: &str = "r1 · no changes";
/// The width of the project's name on the fullest header at 960 × 540 in
/// v0.2.0, measured on `main` `a6a5b91`: *De…*, in the narrowest room
/// UX-25 lets it have.
const V020_NAME_WIDTH: f32 = 27.2;
/// The least room between two controls of the header (criterion 6).
const GAP: f32 = 8.0;

fn mode_file(configuration: &Path) -> PathBuf {
    configuration.join("apiforge").join("run-mode.json")
}

fn stored(configuration: &Path) -> Value {
    serde_json::from_slice(&fs::read(mode_file(configuration)).unwrap()).unwrap()
}

fn part<'a>(harness: &'a FolderHarness, label: &'a str) -> egui_kittest::Node<'a> {
    harness.get_by_role_and_label(Role::RadioButton, label)
}

fn toggled(harness: &FolderHarness, label: &str) -> bool {
    part(harness, label).accesskit_node().toggled() == Some(Toggled::True)
}

/// Clicks the part `label` of the switch.
fn switch_to(harness: &mut FolderHarness, label: &str) {
    part(harness, label).click();
    settle(harness);
}

/// The Run button's accessible name for the mode it runs.
fn run_name(live: bool) -> &'static str {
    if live { LIVE } else { SIMULATE }
}

/// Whether the Run button is ▶ alone: a square, as an icon button is.
fn icon_alone(harness: &FolderHarness, run: &str) -> bool {
    let rect = harness.get_by_role_and_label(Role::Button, run).rect();
    rect.width() <= rect.height() + 1.0
}

/// The window on the demo shop with its component's tab active and the
/// person's *Live allowed* ticked or not.
fn component_window(
    root: &Path,
    size: egui::Vec2,
    configuration: &Path,
    allowed: bool,
) -> FolderHarness {
    let mut harness = window(root, size, configuration);
    if allowed {
        set_box(&mut harness, true);
    }
    assert_eq!(has_label(&harness, BADGE), allowed, "Live allowed");
    let component = only(&harness, DocumentKind::Component);
    app(&mut harness).activate_tab(&component);
    settle(&mut harness);
    harness
}

/// The header's controls on its row, left to right, with the switch as one
/// control: the name, the badges, the mode tabs, the toolbar, the switch
/// and the Run button.
fn header(harness: &FolderHarness, run: &str) -> Vec<(String, egui::Rect)> {
    let mut controls = vec![("name".to_owned(), title(harness))];
    for label in [
        "Project",
        "Observing",
        BADGE,
        "Code",
        "Diagram",
        "Split",
        "Save",
        "Save all",
        UNCHANGED,
        "Publish",
    ] {
        if let Some(node) = harness
            .query_all_by_label(label)
            .find(|node| node.rect().top() < 40.0)
        {
            controls.push((label.to_owned(), node.rect()));
        }
    }
    controls.push((
        GROUP.to_owned(),
        harness
            .get_by_role_and_label(Role::RadioGroup, GROUP)
            .rect(),
    ));
    controls.push((
        run.to_owned(),
        harness.get_by_role_and_label(Role::Button, run).rect(),
    ));
    controls.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
    controls
}

/// Criterion 6: every control of the header within the window and at
/// least 8 px from its neighbour.
fn assert_spaced(harness: &FolderHarness, run: &str, what: &str) {
    let window = harness.ctx.content_rect();
    let controls = header(harness, run);
    for (label, rect) in &controls {
        assert!(
            window.contains_rect(*rect),
            "{what}: {label} {rect:?} is outside the window {window:?}"
        );
    }
    for pair in controls.windows(2) {
        let ((left, a), (right, b)) = (&pair[0], &pair[1]);
        assert!(
            b.left() - a.right() >= GAP - 0.5,
            "{what}: {left} ends at {}, {right} starts at {}",
            a.right(),
            b.left()
        );
    }
}

/// Criterion 1: on a scenario's and on a component's tab, at both
/// windows, the header holds *Simulation*, *Live* and the Run button, in
/// that order from the left, and no *Run options*.
#[test]
fn ux42_criterion_1_switch_and_button() {
    let configuration = tempfile::tempdir().unwrap();
    let directory = shop();
    for size in [MINIMUM, WIDE] {
        let mut harness = window(directory.path(), size, configuration.path());
        let component = only(&harness, DocumentKind::Component);
        for tab in [only(&harness, DocumentKind::Scenario), component] {
            app(&mut harness).activate_tab(&tab);
            settle(&mut harness);
            let what = format!("{tab} at {size:?}");
            let simulation = part(&harness, SIMULATION).rect();
            let live = part(&harness, LIVE_PART).rect();
            let run = harness.get_by_role_and_label(Role::Button, SIMULATE).rect();
            assert!(
                simulation.right() <= live.left() && live.right() <= run.left(),
                "{what}: Simulation {simulation:?}, Live {live:?}, Run {run:?}"
            );
            for row in [live, run] {
                assert!(
                    (row.center().y - simulation.center().y).abs() <= 1.0,
                    "{what}: one row"
                );
            }
            assert!(
                run.right() <= size.x && run.right() >= size.x - 40.0,
                "{what}: the Run button ends the row: {run:?}"
            );
            for gone in [OPTIONS, CAPTION, LINE] {
                assert!(!contains(&harness, gone), "{what}: no {gone}");
            }
        }
        drop(harness);
    }
}

/// Criterion 2: a click on a part chooses the mode, writes it as UX-02
/// does (`run-mode.json`, this project only), recolours the Run button and
/// starts no run. A click on the chosen part writes nothing.
#[test]
fn ux42_criterion_2_a_click_changes_the_mode_only() {
    let configuration = tempfile::tempdir().unwrap();
    let directory = shop();
    let root = directory.path();
    let other = tempfile::tempdir().unwrap();
    apiforge_project::prepare_project(other.path(), &apiforge_core::demos::tickets_project())
        .unwrap();
    let mut harness = window(root, WIDE, configuration.path());
    let active = folder(&harness).workbench.active().unwrap().to_owned();
    assert!(toggled(&harness, SIMULATION), "a project never set");
    switch_to(&mut harness, SIMULATION);
    assert!(
        !mode_file(configuration.path()).exists(),
        "the chosen part writes nothing"
    );

    for (label, run, mode) in [
        (LIVE_PART, LIVE, "live"),
        (SIMULATION, SIMULATE, "simulation"),
    ] {
        switch_to(&mut harness, label);
        assert!(toggled(&harness, label), "{label} is chosen");
        assert!(
            harness.query_by_role_and_label(Role::Button, run).is_some(),
            "the Run button runs {mode}"
        );
        assert!(
            folder(&harness).runs.handle.is_none()
                && folder(&harness).run_result(&active).is_none(),
            "{label}: nothing runs"
        );
        let stored = stored(configuration.path());
        assert_eq!(
            stored["projects"][manifest_id(root)]["mode"],
            mode,
            "{stored}"
        );
        assert!(
            stored["projects"].get(manifest_id(other.path())).is_none(),
            "{stored}"
        );
    }
    // The recolouring: the button's fill is its mode's (UX-02).
    for compact in [false, true] {
        assert_eq!(
            super::super::chrome::run_button(false, compact).primary_fill(),
            theme::SIMULATION
        );
        assert_eq!(
            super::super::chrome::run_button(true, compact).primary_fill(),
            theme::ACCENT
        );
    }
}

/// Criterion 3: the parts are one radio group named *Run mode*, each part
/// named *Simulation* or *Live* with its state and its hint on hover; Tab
/// reaches the group after *Project* and before the Run button; Left and
/// Right move the choice.
#[test]
fn ux42_criterion_3_keyboard_and_names() {
    let configuration = tempfile::tempdir().unwrap();
    let directory = shop();
    let root = directory.path();
    let mut harness = window(root, WIDE, configuration.path());
    let group = harness.get_by_role_and_label(Role::RadioGroup, GROUP);
    let parts: Vec<String> = group
        .children_recursive()
        .filter(|node| node.accesskit_node().role() == Role::RadioButton)
        .filter_map(|node| node.accesskit_node().label())
        .collect();
    assert_eq!(parts, [SIMULATION, LIVE_PART], "the group's parts");
    assert!(toggled(&harness, SIMULATION));
    assert!(!toggled(&harness, LIVE_PART));
    assert!(hover_shows(&mut harness, SIMULATION, SIMULATION_HINT));
    assert!(hover_shows(&mut harness, LIVE_PART, LIVE_HINT));

    part(&harness, SIMULATION).focus();
    settle(&mut harness);
    harness.key_press(Key::ArrowRight);
    settle(&mut harness);
    assert!(toggled(&harness, LIVE_PART), "Right chooses Live");
    assert!(part(&harness, LIVE_PART).is_focused(), "the focus follows");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, LIVE)
            .is_some()
    );
    assert_eq!(
        stored(configuration.path())["projects"][manifest_id(root)]["mode"],
        "live"
    );
    harness.key_press(Key::ArrowLeft);
    settle(&mut harness);
    assert!(toggled(&harness, SIMULATION), "Left chooses Simulation");
    assert!(part(&harness, SIMULATION).is_focused());
    assert_eq!(
        stored(configuration.path())["projects"][manifest_id(root)]["mode"],
        "simulation"
    );
    assert!(
        folder(&harness).runs.handle.is_none(),
        "the arrows run nothing"
    );
    drop(harness);

    let (context, mut app) = open(root);
    let visited = tab_order(&mut app, &context, 23);
    let mut at = 0;
    for name in ["Project", SIMULATION, SIMULATE, "Code"] {
        at += visited[at..]
            .iter()
            .position(|visit| visit == name)
            .unwrap_or_else(|| panic!("{name} after {at} in {visited:#?}"));
    }
}

/// Criterion 4: while the window observes, *Live* is disabled with UX-02's
/// hover, the switch shows *Simulation*, and the stored *Live* returns
/// when the window holds the project again.
#[test]
fn ux42_criterion_4_observing() {
    let configuration = tempfile::tempdir().unwrap();
    let directory = shop();
    let root = directory.path();
    let mut harness = window(root, WIDE, configuration.path());
    switch_to(&mut harness, LIVE_PART);
    drop(harness);

    let owner = agent_holds(root);
    let mut harness = window(root, WIDE, configuration.path());
    assert!(has_label(&harness, "Observing"));
    assert!(
        part(&harness, LIVE_PART).accesskit_node().is_disabled(),
        "Live is disabled"
    );
    assert!(hover_shows(&mut harness, LIVE_PART, runs::LIVE_PAUSED));
    assert!(toggled(&harness, SIMULATION), "the switch shows Simulation");
    assert!(!toggled(&harness, LIVE_PART));
    assert!(
        harness
            .query_by_role_and_label(Role::Button, SIMULATE)
            .is_some()
    );
    let before = fs::read(mode_file(configuration.path())).unwrap();
    part(&harness, LIVE_PART).click();
    settle(&mut harness);
    assert_eq!(
        fs::read(mode_file(configuration.path())).unwrap(),
        before,
        "a disabled Live writes nothing"
    );
    assert_eq!(
        stored(configuration.path())["projects"][manifest_id(root)]["mode"],
        "live",
        "the stored mode is kept"
    );
    drop(harness);
    drop(owner);

    let harness = window(root, WIDE, configuration.path());
    assert!(!has_label(&harness, "Observing"));
    assert!(toggled(&harness, LIVE_PART), "Live returns");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, LIVE)
            .is_some()
    );
}

/// Criterion 5: *Review legacy runs* is in the Project menu after *Test
/// environment…* and opens what it opened from the run menu: the review of
/// the migrated legacy journal's unresolved runs. No *Runs* button is drawn
/// on any tab, one that can run or one that cannot, or with no tab.
#[test]
fn ux42_criterion_5_review_legacy_runs_in_the_project_menu() {
    let configuration = tempfile::tempdir().unwrap();
    let directory = shop();
    fs::write(directory.path().join("README.md"), "# Shop\n").unwrap();
    let legacy_data = legacy_journal();
    let mut harness = window(directory.path(), WIDE, configuration.path());
    app(&mut harness).use_legacy_data_directory(legacy_data.path());

    let scenario = only(&harness, DocumentKind::Scenario);
    let component = only(&harness, DocumentKind::Component);
    app(&mut harness).open_text("README.md");
    settle(&mut harness);
    let text = folder(&harness).workbench.active().unwrap().to_owned();
    for tab in [scenario, component, text] {
        app(&mut harness).activate_tab(&tab);
        settle(&mut harness);
        assert!(!has_label(&harness, RUNS), "{tab}: no Runs button");
        assert!(!has_label(&harness, REVIEW), "{tab}: not in the header");
    }
    let tabs: Vec<String> = folder(&harness)
        .workbench
        .tabs()
        .iter()
        .map(|tab| tab.document_id.clone())
        .collect();
    for tab in tabs {
        app(&mut harness).close_tab(&tab);
    }
    settle(&mut harness);
    assert!(folder(&harness).workbench.active().is_none(), "no tab");
    assert!(!has_label(&harness, RUNS), "no tab: no Runs button");

    choose(&mut harness, "Project");
    let environment = harness.get_by_label("Test environment…").rect();
    let review = harness.get_by_label(REVIEW).rect();
    assert!(
        review.top() >= environment.bottom(),
        "after Test environment…: {environment:?}, {review:?}"
    );
    assert!(
        (review.left() - environment.left()).abs() <= 1.0,
        "an item of the menu"
    );
    choose(&mut harness, REVIEW);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(folder(&harness).prompt, Some(Prompt::Journal(_))) {
        assert!(Instant::now() < deadline, "the legacy runs open");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(&mut harness);
    assert!(contains(&harness, "Unresolved legacy runs"));
}

/// A data directory with a migrated legacy workspace whose journal holds
/// one interrupted run, as `folder::tests` prepares it.
fn legacy_journal() -> tempfile::TempDir {
    let data = tempfile::tempdir().unwrap();
    let legacy =
        apiforge_storage::WorkspaceStore::open_exclusive(data.path().join("workspace.json"))
            .unwrap();
    legacy
        .save(
            &apiforge_core::import_document(
                include_bytes!("../../../../../fixtures/workspace.json"),
                &apiforge_core::ImportLimits::default(),
            )
            .unwrap()
            .into_workspace(),
        )
        .unwrap();
    let journal = apiforge_storage::JournalStore::open(legacy.clone()).unwrap();
    use apiforge_core::journal::{JournalRun, JournalSink};
    journal
        .store
        .begin_run(&JournalRun::new(
            "legacy",
            "main",
            "Unassigned",
            "2026-10-24T00:00:00Z",
        ))
        .unwrap();
    let _ = apiforge_project::migrate_workspace(&legacy).unwrap();
    drop(journal);
    drop(legacy);
    data
}

/// Criterion 6: on a component's tab with *Publish*, *r1 · no changes*
/// and *Live allowed* at 960 × 540, in both modes, every control of the
/// header is within the window and 8 px from its neighbour, the name keeps
/// its v0.2.0 width, and the Run button is ▶ alone with its mode's name and
/// hover. On every other header at 960 × 540 and 1600 × 1000, and on that
/// one at 1100 px, it reads *▶ Run* with the same spacing: the choice
/// follows the row's width, not the kind of tab.
#[test]
fn ux42_criterion_6_the_fullest_header_fits() {
    let configuration = tempfile::tempdir().unwrap();
    let directory = shop();
    let root = directory.path();
    let mut harness = component_window(root, MINIMUM, configuration.path(), true);
    for live in [false, true] {
        switch_to(&mut harness, if live { LIVE_PART } else { SIMULATION });
        let run = run_name(live);
        let what = format!("{run} on the component's tab at 960 × 540");
        assert!(has_label(&harness, UNCHANGED), "{what}: the caption");
        assert!(has_label(&harness, "Publish"), "{what}: Publish");
        assert_spaced(&harness, run, &what);
        assert!(
            (title(&harness).width() - V020_NAME_WIDTH).abs() <= 1.0,
            "{what}: the name is {} px wide, in v0.2.0 {V020_NAME_WIDTH}",
            title(&harness).width()
        );
        assert!(icon_alone(&harness, run), "{what}: ▶ alone");
        // Counted before any hover, whose tooltip would already name it.
        let names = harness.query_all_by_label(run).count();
        assert!(
            crate::testing::hover_until(
                &mut harness,
                |harness| harness
                    .query_by_role_and_label(Role::Button, run)
                    .map(|node| node.rect()),
                |harness| harness.query_all_by_label(run).count() > names,
            ),
            "{what}: named on hover"
        );
        assert!(
            hover_shows(
                &mut harness,
                run,
                if live { LIVE_HOVER } else { SIMULATE_HOVER }
            ),
            "{what}: the mode's hover"
        );
        harness.hover_at(egui::pos2(480.0, 3.0));
        settle(&mut harness);
    }
    drop(harness);

    // Every other header holds *▶ Run*.
    for (size, allowed) in [
        (MINIMUM, false),
        (WIDE, true),
        (WIDE, false),
        (egui::vec2(1100.0, 540.0), true),
    ] {
        // A configuration of its own: the consent is kept per person.
        let configuration = tempfile::tempdir().unwrap();
        let mut harness = component_window(root, size, configuration.path(), allowed);
        let scenario = only(&harness, DocumentKind::Scenario);
        let component = only(&harness, DocumentKind::Component);
        for tab in [&component, &scenario] {
            app(&mut harness).activate_tab(tab);
            settle(&mut harness);
            for live in [false, true] {
                switch_to(&mut harness, if live { LIVE_PART } else { SIMULATION });
                let run = run_name(live);
                let what = format!("{run}, {tab}, Live allowed {allowed}, at {size:?}");
                assert_spaced(&harness, run, &what);
                assert!(!icon_alone(&harness, run), "{what}: ▶ Run");
            }
        }
        drop(harness);
    }
    // The scenario's tab at 960 × 540 with *Live allowed*.
    let mut harness = window(root, MINIMUM, configuration.path());
    set_box(&mut harness, true);
    let scenario = only(&harness, DocumentKind::Scenario);
    app(&mut harness).activate_tab(&scenario);
    settle(&mut harness);
    for live in [false, true] {
        switch_to(&mut harness, if live { LIVE_PART } else { SIMULATION });
        let run = run_name(live);
        let what = format!("{run} on the scenario's tab at 960 × 540 with Live allowed");
        assert_spaced(&harness, run, &what);
        assert!(!icon_alone(&harness, run), "{what}: ▶ Run");
        assert_within_window(&harness, &[BADGE, run]);
    }
}

/// Criterion 7: the title does not move when the mode changes: in either
/// mode the row reserves the same width, on both kinds of tab, at both
/// windows, with and without *Live allowed*.
#[test]
fn ux42_criterion_7_title_stays() {
    let directory = shop();
    for size in [MINIMUM, WIDE] {
        for allowed in [false, true] {
            let configuration = tempfile::tempdir().unwrap();
            let mut harness =
                component_window(directory.path(), size, configuration.path(), allowed);
            let scenario = only(&harness, DocumentKind::Scenario);
            let component = only(&harness, DocumentKind::Component);
            // The name keeps one place on every tab (KTLO-23); the mode
            // tabs stand where the tab's toolbar leaves them, in either mode.
            let mut titles = Vec::new();
            for tab in [&scenario, &component] {
                app(&mut harness).activate_tab(tab);
                settle(&mut harness);
                let mut rows = Vec::new();
                for part in [SIMULATION, LIVE_PART] {
                    switch_to(&mut harness, part);
                    titles.push(title(&harness));
                    rows.push(harness.get_by_label("Code").rect());
                }
                assert_eq!(
                    rows[0], rows[1],
                    "{tab} at {size:?}, Live allowed {allowed}: Code moved"
                );
            }
            assert!(
                titles.windows(2).all(|pair| pair[0] == pair[1]),
                "at {size:?}, Live allowed {allowed}: {titles:?}"
            );
            drop(harness);
        }
    }
}

/// Review of #530: hovering the ▶-alone Run button opens one tooltip, the
/// button's name first and UX-02's hover text on the line under it, in
/// either mode; *▶ Run*'s hover is UX-02's text, in one tooltip too.
#[test]
fn ux42_the_icon_alone_has_one_tooltip_name_first() {
    fn tooltips(harness: &FolderHarness) -> usize {
        harness.ctx.memory(|memory| {
            memory
                .areas()
                .visible_layer_ids()
                .iter()
                .filter(|layer| layer.order == egui::Order::Tooltip)
                .count()
        })
    }
    fn hover(harness: &mut FolderHarness, run: &str, hover: &str) {
        harness.hover_at(egui::pos2(480.0, 3.0));
        settle(harness);
        assert!(
            crate::testing::hover_until(
                harness,
                |harness| harness
                    .query_by_role_and_label(Role::Button, run)
                    .map(|node| node.rect()),
                |harness| contains(harness, hover),
            ),
            "{run}: its hover"
        );
    }
    let configuration = tempfile::tempdir().unwrap();
    let directory = shop();
    let root = directory.path();
    let mut harness = component_window(root, MINIMUM, configuration.path(), true);
    for live in [false, true] {
        switch_to(&mut harness, if live { LIVE_PART } else { SIMULATION });
        let run = run_name(live);
        let text = if live { LIVE_HOVER } else { SIMULATE_HOVER };
        assert!(icon_alone(&harness, run), "{run}: ▶ alone");
        hover(&mut harness, run, text);
        assert_eq!(tooltips(&harness), 1, "{run}: one tooltip");
        let button = harness.get_by_role_and_label(Role::Button, run).rect();
        let name = harness
            .query_all_by_label(run)
            .map(|node| node.rect())
            .find(|rect| *rect != button)
            .expect("the tooltip names the button");
        let line = harness.get_by_label(text).rect();
        assert!(
            name.bottom() <= line.top(),
            "{run}: the name {name:?} above the text {line:?}"
        );
    }
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(&mut harness);
    drop(harness);

    let configuration = tempfile::tempdir().unwrap();
    let mut harness = window(root, WIDE, configuration.path());
    assert!(!icon_alone(&harness, SIMULATE));
    hover(&mut harness, SIMULATE, SIMULATE_HOVER);
    assert_eq!(tooltips(&harness), 1, "▶ Run: one tooltip");
}
