//! Issue #288: selecting a step with the Inspector open changes nothing in
//! the document. No unsaved mark, the same source text, and the results of
//! the last run stay fresh, on every step and every tab of the reference
//! scenarios; an edit in the Inspector still makes them stale (UX-25).
use super::*;
use crate::testing::folder_harness_in;
use egui_kittest::kittest::NodeT;
use std::time::{Duration, Instant};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const TABS: [&str; 3] = ["Configuration", "Output data", "Checks"];

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// `source` saved as the demo shop's scenario, whose components it may call,
/// and open on its board.
fn open(source: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    let mut harness = folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    (project, harness, id)
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
}

fn dirty(harness: &FolderHarness, id: &str) -> bool {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .is_dirty()
}

fn fresh(harness: &FolderHarness, id: &str) -> bool {
    folder(harness).fresh_run(id).is_some()
}

/// The steps of one board, by node id and handle.
fn steps(harness: &FolderHarness, id: &str, cleanup: bool) -> Vec<(String, String)> {
    let model = folder(harness).snapshot(id).unwrap().model.unwrap();
    graph_of(&model, cleanup)
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.handle.clone()))
        .collect()
}

/// After a simulation, every step of each board selected in turn, by the
/// selection and by a click on its card, on each tab the Inspector offers,
/// leaves the document as it was and its results fresh.
fn assert_selection_changes_nothing(source: &str, what: &str, least: usize) {
    let (_project, mut harness, id) = open(source);
    simulate(&mut harness);
    let text = source_of(&harness, &id);
    assert!(!dirty(&harness, &id), "{what}: saved before any selection");
    assert!(fresh(&harness, &id), "{what}: the run's results are fresh");

    let mut seen = 0;
    for cleanup in [false, true] {
        if cleanup {
            if !has_label(&harness, "Cleanup") {
                continue;
            }
            choose(&mut harness, "Cleanup");
        }
        for (node, handle) in steps(&harness, &id, cleanup) {
            for tab in TABS {
                app(&mut harness).selections.remove(&id);
                settle(&mut harness);
                app(&mut harness)
                    .selections
                    .insert(id.clone(), vec![node.clone()]);
                settle(&mut harness);
                if let Some(button) = harness.query_by_label(tab) {
                    button.click();
                    settle(&mut harness);
                } else {
                    assert_eq!(tab, "Checks", "{what}: {handle} has the tab {tab}");
                }
                // A card clicked as a person would, with the Inspector open.
                let card = format!("{handle} (");
                let label = harness
                    .query_all_by_label_contains(&card)
                    .filter_map(|found| found.accesskit_node().label())
                    .find(|label| label.starts_with(&card));
                if let Some(label) = label {
                    harness.get_by_label(&label).click();
                    settle(&mut harness);
                }
                assert!(
                    !dirty(&harness, &id),
                    "{what}: selecting {handle} on {tab} marks the document changed"
                );
                assert_eq!(
                    source_of(&harness, &id),
                    text,
                    "{what}: selecting {handle} on {tab} changes the source"
                );
                assert!(
                    fresh(&harness, &id),
                    "{what}: selecting {handle} on {tab} stales the results"
                );
                seen += 1;
            }
        }
    }
    assert!(
        seen >= least * TABS.len(),
        "{what}: {seen} selections, fewer than its {least} steps on each tab"
    );
}

/// Criterion 1 on the reference scenarios `order-flow`, `shop-30` and the
/// copy of `order-flow` the issue was reported on.
#[test]
fn selecting_a_step_with_the_inspector_open_changes_nothing() {
    for (name, steps) in [
        ("fixtures/scenarios/lay/order-flow.afs", 14),
        ("fixtures/scenarios/lay/shop-30.afs", 30),
        ("docs/qa/2026-10-26/LAY/order-flow.afs", 14),
    ] {
        assert_selection_changes_nothing(&fixture(name), name, steps);
    }
}

/// Criterion 1 on the demo shop's own scenario, as the project opens.
#[test]
fn selecting_a_step_of_the_demo_shop_changes_nothing() {
    let project = crate::testing::shop_project();
    let source = fs::read_to_string(project.path().join(SCENARIO)).unwrap();
    assert_selection_changes_nothing(&source, "the demo shop", 1);
}

/// `order-flow` after a simulation, with `cart` selected on its
/// configuration.
fn cart_selected() -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = open(&fixture("fixtures/scenarios/lay/order-flow.afs"));
    simulate(&mut harness);
    assert!(fresh(&harness, &id));
    let cart = steps(&harness, &id, false)
        .into_iter()
        .find(|(_, handle)| handle == "cart")
        .expect("order-flow has cart")
        .0;
    app(&mut harness).selections.insert(id.clone(), vec![cart]);
    settle(&mut harness);
    choose(&mut harness, "Configuration");
    assert!(!dirty(&harness, &id) && fresh(&harness, &id));
    (project, harness, id)
}

/// Scrolls to the control named `label`, not the caption above it, and
/// clicks it.
fn press(harness: &mut FolderHarness, label: &str) {
    for click in [false, true] {
        {
            let mut controls = harness
                .query_all_by_label(label)
                .filter(|node| node.accesskit_node().role() != egui::accesskit::Role::Label);
            let control = controls
                .next()
                .unwrap_or_else(|| panic!("no control {label}"));
            assert!(controls.next().is_none(), "one control named {label}");
            if click {
                control.click();
            } else {
                control.scroll_to_me();
            }
        }
        settle(harness);
    }
}

/// Criterion 2: an edit made in the Inspector still marks the document
/// changed and makes the results stale, for the method and for the body
/// format, the two values the Inspector no longer writes on show.
#[test]
fn an_inspector_edit_still_stales_the_results() {
    for (control, option, written) in [
        ("Method", "PUT", "cart = PUT \"/cart\""),
        ("Body format", "Text", "body text { items"),
    ] {
        let (_project, mut harness, id) = cart_selected();
        press(&mut harness, control);
        press(&mut harness, option);
        assert!(
            dirty(&harness, &id),
            "{control}: the edit marks the document changed"
        );
        let source = source_of(&harness, &id);
        assert!(
            source.contains(written),
            "{control}: the edit reaches the source:\n{source}"
        );
        assert!(
            !fresh(&harness, &id),
            "{control}: the edit makes the results stale"
        );
    }
}
