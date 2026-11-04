//! UX-19 (#227) and REP-01 (#135) mock-up review views (throwaway branch
//! `mock/ux-19`): Run results after a run that fails inside a component and
//! after one that passes, with the switches of `report::mock_ux19` on.
use super::project_interaction_tests::choose;
use super::*;
use crate::report::mock_ux19;
use crate::testing::{FolderHarness, MINIMUM, WIDE, capture, folder, folder_harness, settle};
use apiforge_core::format::{Binding, Position};
use apiforge_core::model::Predicate;
use egui_kittest::kittest::Queryable;
use serde_json::json;

/// The card selected on the board before the run.
const BLOCK: &str = "Expected price (Value)";

/// The demo shop whose *Add to cart* answers 400 to *Create cart* in
/// simulation, and checks for 201 in a new step *Cart created*: that step
/// fails, so the run fails inside the component.
fn failing_shop() -> (tempfile::TempDir, String) {
    let mut project = apiforge_core::demos::shop_project();
    let template = project.scenarios[0]
        .graph
        .nodes
        .iter()
        .find(|node| node.id == "price")
        .unwrap()
        .clone();
    let cart = project
        .components
        .iter_mut()
        .find(|component| component.id == "product-cart")
        .unwrap();
    let at = cart
        .graph
        .nodes
        .iter()
        .position(|node| node.id == "create")
        .unwrap();
    cart.graph.nodes[at].config.sample_status = Some(400);
    let create = cart.graph.nodes[at].handle.clone();
    let mut check = template;
    check.id = "created".into();
    check.handle = "cartCreated".into();
    check.label = Some("Cart created".into());
    check.position = Position { x: 800.0, y: 260.0 };
    check.inputs = [
        ("actual".to_owned(), Binding::reference("create", "status", "")),
        ("expected".to_owned(), Binding::literal(json!(201))),
    ]
    .into_iter()
    .collect();
    check.config.predicate = Some(Predicate::Equals);
    check.config.tolerance = None;
    cart.graph.nodes.insert(at + 1, check);
    let directory = tempfile::tempdir().unwrap();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    (directory, format!("{create}.status"))
}

fn scenario_file(harness: &FolderHarness) -> String {
    let session = folder(harness).session.as_ref().unwrap();
    let path = session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Scenario)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();
    path.rsplit('/').next().unwrap().to_owned()
}

/// Opens `root` at `size` in Diagram, selects [`BLOCK`] on the board and
/// runs the simulation with the mock-up's switches on.
fn ran(root: &std::path::Path, size: egui::Vec2, subject: &str) -> FolderHarness {
    let mut harness = folder_harness(root, size);
    mock_ux19::set(mock_ux19::Flags {
        on: true,
        file: scenario_file(&harness),
        check_subject: subject.to_owned(),
    });
    choose(&mut harness, "Diagram");
    choose(&mut harness, BLOCK);
    choose(&mut harness, "Run simulation");
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(&harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(&mut harness);
    // The scroll to the selected row lands on the frame after it is asked.
    for _ in 0..3 {
        harness.step();
    }
    settle(&mut harness);
    harness
}

fn rect_of(harness: &FolderHarness, label: &str) -> egui::Rect {
    harness.get_by_label_contains(label).rect()
}

#[test]
#[ignore = "design-review view"]
fn review_ux19_failed_run() {
    for (size, suffix) in [(WIDE, "1600x1000"), (MINIMUM, "960x540")] {
        let (project, subject) = failing_shop();
        let mut harness = ran(project.path(), size, &subject);
        capture(&mut harness, &format!("ux19-failed-run-{suffix}"), None);
        if size == WIDE {
            let top = rect_of(&harness, "  →  ");
            let region = egui::Rect::from_min_size(
                top.left_top() - egui::vec2(12.0, 10.0),
                egui::vec2(760.0, 150.0),
            );
            capture(&mut harness, "ux19-breadcrumb-crop", Some(region));
            choose(&mut harness, "Maximise run results");
            for _ in 0..3 {
                harness.step();
            }
            let product = rect_of(&harness, "Product · ");
            let region = egui::Rect::from_min_size(
                product.left_top() - egui::vec2(10.0, 10.0),
                egui::vec2(600.0, 300.0),
            );
            capture(&mut harness, "ux19-rows-crop", Some(region));
        }
    }
}

#[test]
#[ignore = "design-review view"]
fn review_ux19_passing_run() {
    let project = crate::testing::shop_project();
    let mut harness = ran(project.path(), WIDE, "");
    capture(&mut harness, "ux19-passing-run-1600x1000", None);
}

#[test]
#[ignore = "design-review view"]
fn review_ux19_rep01_component_row() {
    let (project, subject) = failing_shop();
    let mut harness = ran(project.path(), WIDE, &subject);
    let row = harness.get_by_label_contains("Add to cart · ");
    row.scroll_to_me();
    settle(&mut harness);
    harness.get_by_label_contains("Add to cart · ").click();
    settle(&mut harness);
    capture(&mut harness, "ux19-rep01-component-row-1600x1000", None);
}
