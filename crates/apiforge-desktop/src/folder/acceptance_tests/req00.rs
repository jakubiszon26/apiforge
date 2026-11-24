//! REQ-00 mock-up: design review captures of the Steps presentation.
//! Mock branch only: never merged.
use super::*;
use crate::mock;
use crate::testing::{LABELS, MINIMUM, capture};
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

const ROOMS: &str = "scenarios/double-booking.afs";

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn fixtures() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay")
}

fn labels(size: egui::Vec2, failing: bool) -> (tempfile::TempDir, FolderHarness, String) {
    let mut text = fs::read_to_string(fixtures().join("labels.afs")).unwrap();
    if failing {
        let from = r#"field: "weight", message: "Weight must be at most 30 kg.""#;
        assert!(text.contains(from));
        text = text.replacen(from, &from.replacen("weight", "mass", 1), 1);
    }
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Label validation".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = apiforge_project::ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(apiforge_project::DocumentKind::Scenario, LABELS, &text)
        .unwrap();
    drop(session);
    let mut harness = crate::testing::folder_harness_in(directory.path(), size, Mode::Diagram);
    app(&mut harness).review_open(LABELS, None);
    settle(&mut harness);
    (directory, harness, id.to_string())
}

fn rooms(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = tempfile::tempdir().unwrap();
    copy_tree(&fixtures().join("rooms"), project.path());
    let _ = fs::remove_file(project.path().join(".apiforge/diagram.json"));
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(ROOMS, None);
    settle(&mut harness);
    let id = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == ROOMS)
        .unwrap()
        .id()
        .to_owned();
    (project, harness, id.to_string())
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
    ] {
        switch.store(0, Relaxed);
    }
    mock::MENU.store(false, Relaxed);
    mock::SELECTED.lock().unwrap().clear();
}

fn pick(handle: &str) {
    *mock::SELECTED.lock().unwrap() = handle.to_owned();
}

fn shot(harness: &mut FolderHarness, name: &str) {
    harness.run_steps(4);
    capture(harness, name, None);
}

fn select(harness: &mut FolderHarness, id: &str, handle: &str) {
    let snapshot = folder(harness).snapshot(id).unwrap();
    let step = graph_of(snapshot.model.as_ref().unwrap(), false)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap()
        .id
        .clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
}

#[test]
#[ignore = "REQ-00 mock-up: design review capture; run with tools/gui/review.sh review_req00"]
fn review_req00() {
    for (size, at) in [(WIDE, "1600"), (MINIMUM, "960")] {
        // Today: the board, and a request in the Inspector.
        mock::ON.store(false, Relaxed);
        let (_project, mut harness, id) = labels(size, true);
        shot(&mut harness, &format!("today-labels-canvas-{at}"));
        select(&mut harness, &id, "weight_31");
        shot(&mut harness, &format!("today-labels-inspector-{at}"));
        app(&mut harness).selections.remove(&id);
        settle(&mut harness);

        // Steps before a run; question 1.
        reset();
        shot(&mut harness, &format!("steps-before-run-{at}"));
        for (rule, name) in [(0, "a-common"), (1, "b-above"), (2, "c-first")] {
            mock::SENDS.store(rule, Relaxed);
            shot(&mut harness, &format!("q1-{name}-{at}"));
        }
        reset();
        // Question 7.
        for (mode, name) in [(1, "a-line"), (2, "b-bar")] {
            mock::ADD.store(mode, Relaxed);
            shot(&mut harness, &format!("q7-{name}-{at}"));
        }
        reset();
        pick("weight_31");
        shot(&mut harness, &format!("request-not-run-{at}"));

        // After a run with one failed check.
        reset();
        mock::ON.store(false, Relaxed);
        simulate(&mut harness);
        shot(&mut harness, &format!("today-labels-run-{at}"));
        choose(&mut harness, "Run results");
        reset();
        shot(&mut harness, &format!("steps-after-run-{at}"));
        pick("weight_31");
        for (tab, name) in [(0, "body"), (1, "headers"), (2, "auth")] {
            mock::TAB.store(tab, Relaxed);
            shot(&mut harness, &format!("request-{name}-{at}"));
        }
        mock::TAB.store(0, Relaxed);
        // Question 4.
        for (fold, name) in [
            (0, "today-three"),
            (1, "a-rail"),
            (2, "b-tabs"),
            (3, "c-under"),
        ] {
            mock::FOLD.store(fold, Relaxed);
            shot(&mut harness, &format!("q4-{name}-{at}"));
        }
        mock::FOLD.store(if at == "960" { 1 } else { 0 }, Relaxed);
        // Question 3.
        for (alone, name) in [(1, "sent-alone"), (2, "changed")] {
            mock::ALONE.store(alone, Relaxed);
            shot(&mut harness, &format!("q3-{name}-{at}"));
        }
        mock::ALONE.store(0, Relaxed);
        mock::FOLD.store(0, Relaxed);
        mock::MENU.store(true, Relaxed);
        shot(&mut harness, &format!("q6-menu-{at}"));
        drop(harness);

        // The room scenario: a call, a value, an `if`, Cleanup.
        mock::ON.store(false, Relaxed);
        let (_project, mut harness, _id) = rooms(size);
        shot(&mut harness, &format!("today-rooms-canvas-{at}"));
        reset();
        shot(&mut harness, &format!("q5-a-all-{at}"));
        mock::KINDS.store(1, Relaxed);
        shot(&mut harness, &format!("q5-b-requests-{at}"));
        reset();
        mock::CLEANUP.store(1, Relaxed);
        shot(&mut harness, &format!("q8-b-tabs-{at}"));
        reset();
        mock::ADD.store(1, Relaxed);
        shot(&mut harness, &format!("q8-a-section-add-{at}"));
        reset();
        mock::ON.store(false, Relaxed);
        simulate(&mut harness);
        choose(&mut harness, "Run results");
        reset();
        shot(&mut harness, &format!("rooms-after-run-{at}"));
        pick("first");
        shot(&mut harness, &format!("rooms-request-{at}"));
        pick("cancelled");
        mock::FOLD.store(if at == "960" { 1 } else { 0 }, Relaxed);
        shot(&mut harness, &format!("rooms-cleanup-request-{at}"));
        mock::ON.store(false, Relaxed);
    }
}
