//! REQ-17 and REQ-18 mock-up: design review captures of the exploratory
//! request and of *Save as component* from a request, on REQ-00's Steps
//! panes. Mock branch only: never merged.
use super::*;
use crate::mock;
use crate::testing::{MINIMUM, capture};
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

const FILE: &str = "scenarios/try-carts.afs";
const TEXT: &str = r#"scenario "Try carts" {
  begin = start
  @label("Create cart")
  create_cart = POST "https://shop.example.test/shop/carts" {
    headers { "Content-Type": "application/json", "X-Client": "tester-7" }
    body { productId: 123, quantity: 2 }
    sample 201 { id: "c-4f2a", productId: 123, quantity: 2, total: 59.8 }
  }
}
"#;

fn project(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Shop tests".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = apiforge_project::ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(apiforge_project::DocumentKind::Scenario, FILE, TEXT)
        .unwrap();
    drop(session);
    let mut harness = crate::testing::folder_harness_in(directory.path(), size, Mode::Diagram);
    app(&mut harness).review_open(FILE, None);
    settle(&mut harness);
    (directory, harness, id.to_string())
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

fn reset() {
    mock::ON.store(true, Relaxed);
    for switch in [
        &mock::SENDS,
        &mock::FOLD,
        &mock::KINDS,
        &mock::ADD,
        &mock::CLEANUP,
        &mock::TAB,
        &mock::ALONE,
        &mock::SCRATCH,
        &mock::COMPONENT,
    ] {
        switch.store(0, Relaxed);
    }
    mock::MENU.store(false, Relaxed);
    mock::KEEP.store(false, Relaxed);
    mock::LIVE.store(true, Relaxed);
    mock::SELECTED.lock().unwrap().clear();
}

fn shot(harness: &mut FolderHarness, name: &str) {
    harness.run_steps(4);
    capture(harness, name, None);
}

#[test]
#[ignore = "REQ-17 mock-up: design review capture; run with tools/gui/review.sh review_req17"]
fn review_req17() {
    for (size, at) in [(WIDE, "1600"), (MINIMUM, "960")] {
        mock::ON.store(false, Relaxed);
        let (_project, mut harness, id) = project(size);
        let snapshot = folder(&harness).snapshot(&id).unwrap();
        let model = snapshot.model.clone().unwrap();
        let step = graph_of(&model, false)
            .nodes
            .iter()
            .find(|node| node.handle == "create_cart")
            .unwrap()
            .id
            .clone();
        app(&mut harness)
            .selections
            .insert(id.clone(), vec![step.clone()]);
        settle(&mut harness);
        simulate(&mut harness);
        shot(&mut harness, &format!("req17-today-{at}"));
        // Today's dialog for one request (UX-30).
        app(&mut harness).open_save_component(&id, &model, false, &[step]);
        settle(&mut harness);
        shot(&mut harness, &format!("req18-today-{at}"));
        app(&mut harness).prompt = None;
        settle(&mut harness);

        choose(&mut harness, "Run results");
        harness
            .get_by_role_and_label(egui::accesskit::Role::RadioButton, "Live")
            .click();
        settle(&mut harness);
        // REQ-17 A: a request with no file.
        reset();
        mock::SCRATCH.store(1, Relaxed);
        shot(&mut harness, &format!("req17-a-{at}"));
        mock::KEEP.store(true, Relaxed);
        shot(&mut harness, &format!("req17-a-keep-{at}"));
        mock::KEEP.store(false, Relaxed);
        // REQ-18 A and B.
        mock::COMPONENT.store(1, Relaxed);
        shot(&mut harness, &format!("req18-a-{at}"));
        mock::COMPONENT.store(2, Relaxed);
        shot(&mut harness, &format!("req18-b-{at}"));

        // REQ-17 B: a scenario made for the try, in Steps as decided.
        reset();
        *mock::SELECTED.lock().unwrap() = "create_cart".into();
        mock::ALONE.store(1, Relaxed);
        mock::FOLD.store(if at == "960" { 1 } else { 0 }, Relaxed);
        shot(&mut harness, &format!("req17-b-{at}"));
        mock::ON.store(false, Relaxed);
    }
}
