//! GUI callers construct the runner on an ordinary thread, outside Tokio.

use apiforge_core::{
    ImportLimits, import_document,
    run::{RunMode, RunStatus, StepStatus},
};
use apiforge_runner::{RunRequest, RunnerService};
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[test]
fn service_starts_outside_an_async_runtime() {
    let service = RunnerService::new(Arc::new(|| {}))
        .expect("the native GUI must be able to initialize its runner");
    assert_eq!(service.active_runs(), 0);
    service.shutdown(Duration::from_secs(1));
}

#[test]
fn imported_workflow_finishes_through_the_gui_service() {
    let document = import_document(
        include_bytes!("../../../fixtures/compatibility/projects/workflow.json"),
        &ImportLimits::default(),
    )
    .expect("the React export must import");
    let project = document.workspace().projects[0].clone();
    let scenario_id = project.scenarios[0].id.clone();
    let (sender, notifications) = mpsc::channel();
    let service = RunnerService::new(Arc::new(move || {
        let _ = sender.send(());
    }))
    .expect("the runner must start on the GUI thread without a Tokio context");
    let handle = service
        .start(RunRequest::new(project, scenario_id, RunMode::Simulation))
        .expect("an imported scenario must be runnable");

    // Exercise the same latest-snapshot handoff used by the GUI. Waiting on the
    // notifications avoids timer-dependent sleeps and bounds a stuck-worker failure.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !handle.is_finished() {
        notifications
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("the runner must publish a finished report");
    }
    let report = handle.latest().expect("the final snapshot must survive");
    assert_eq!(report.status, RunStatus::Passed, "{report:?}");
    for id in [
        "flow-prices",
        "flow-cart",
        "flow-add",
        "flow-check",
        "flow-cleanup",
    ] {
        assert_eq!(
            report.step(id).expect(id).status,
            StepStatus::Passed,
            "{id}"
        );
    }
    assert_eq!(
        report.step("flow-else").unwrap().status,
        StepStatus::Skipped
    );
    service.shutdown(Duration::from_secs(1));
}
