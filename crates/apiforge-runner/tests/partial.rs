//! REQ-04a (#599): a run through a step, and one request alone with its
//! checks (ADR 0016).

use apiforge_core::{
    demos::empty_project,
    format::{Project, Scenario},
    journal::{JournalEntry, JournalRun, JournalSink, JournalSinkError},
    run::{NotRunCause, Phase, RunMode, RunResult, RunStatus, StepResult, StepStatus},
};
use apiforge_runner::{
    HttpTransport, Partial, RunError, RunOptions, RunRequest, RunnerService, run_scenario,
};
use apiforge_script::{Document, LowerContext, lower, parse};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

// Projects -----------------------------------------------------------------------

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(path)
}

/// The scenario `text` alone in an empty folder project.
fn scenario(text: &str) -> (Project, Scenario) {
    let mut project = empty_project();
    project.scenarios.clear();
    project.environment.base_url = "http://127.0.0.1:9".into();
    let result = lower(&parse(text), &LowerContext::folder(&project));
    assert!(!result.has_errors(), "{:#?}", result.diagnostics);
    let Some(Document::Scenario(scenario)) = result.document else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    (project, scenario)
}

fn labels() -> (Project, Scenario) {
    scenario(&fs::read_to_string(fixture("scenarios/lay/labels.afs")).expect("labels"))
}

/// The project of `single-workflow-add`, its scenario and its earlier report.
fn workflow() -> (Project, Scenario, RunResult) {
    let text = fs::read_to_string(fixture("runs/single-workflow-add.json")).expect("the fixture");
    let mut document: Value = serde_json::from_str(&text).expect("JSON");
    let request = document["request"].take();
    let project: Project = serde_json::from_value(request["project"].clone()).expect("a project");
    let scenario = project
        .scenarios
        .iter()
        .find(|scenario| request["scenarioId"] == scenario.id.as_str())
        .expect("the scenario")
        .clone();
    let previous = serde_json::from_value(request["previousRun"].clone()).expect("a report");
    (project, scenario, previous)
}

fn id(scenario: &Scenario, handle: &str) -> String {
    scenario
        .graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no node `{handle}`"))
        .id
        .clone()
}

// Running ------------------------------------------------------------------------

fn full(project: &Project, scenario: &Scenario, mode: RunMode) -> RunRequest {
    RunRequest::new(project.clone(), scenario.id.clone(), mode)
}

fn through(project: &Project, scenario: &Scenario, mode: RunMode, handle: &str) -> RunRequest {
    RunRequest {
        partial: Some(Partial::Through {
            node_id: id(scenario, handle),
        }),
        ..full(project, scenario, mode)
    }
}

fn alone(
    project: &Project,
    scenario: &Scenario,
    mode: RunMode,
    handle: &str,
    previous: Option<&RunResult>,
) -> RunRequest {
    RunRequest {
        partial: Some(Partial::Alone {
            node_id: id(scenario, handle),
            previous: previous.cloned().map(Box::new),
        }),
        ..full(project, scenario, mode)
    }
}

async fn run(request: RunRequest) -> Result<RunResult, RunError> {
    run_scenario(
        request,
        RunOptions::new(HttpTransport::new().expect("client builds")),
        CancellationToken::new(),
        |_| {},
    )
    .await
}

async fn report(request: RunRequest) -> RunResult {
    run(request).await.expect("the run starts")
}

fn step<'r>(run: &'r RunResult, handle: &str) -> &'r StepResult {
    run.steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no result of `{handle}` in {run:#?}"))
}

fn handles(run: &RunResult) -> Vec<&str> {
    run.steps.iter().map(|step| step.handle.as_str()).collect()
}

/// Everything a step reports but how long it took.
fn untimed(step: &StepResult) -> StepResult {
    StepResult {
        duration_ms: 0,
        ..step.clone()
    }
}

fn blocked_by(scenario: &Scenario, handle: &str) -> Option<NotRunCause> {
    Some(NotRunCause::BlockedBy {
        step: id(scenario, handle),
    })
}

// Criterion 1 --------------------------------------------------------------------

/// Through *Weight 0 kg*: the start, three requests and their checks, as the
/// full run reports them, and nothing after.
#[tokio::test]
async fn req04a_criterion_1_through_a_request_runs_its_checks() {
    let (project, scenario) = labels();
    let whole = report(full(&project, &scenario, RunMode::Simulation)).await;
    let part = report(through(
        &project,
        &scenario,
        RunMode::Simulation,
        "weight_0",
    ))
    .await;
    assert_eq!(part.steps.len(), 15, "{:?}", handles(&part));
    assert!(part.steps.iter().all(|step| step.phase == Phase::Main));
    for (part, whole) in part.steps.iter().zip(&whole.steps) {
        assert_eq!(untimed(part), untimed(whole));
    }
    assert_eq!(handles(&part)[10], "weight_0");
    assert_eq!(handles(&part)[14], "weight_0_text");
    assert_eq!(part.status, RunStatus::Passed);
    assert!(part.diagnostic);
}

/// Every node of a scenario with an `if`: the run through it is a prefix of
/// the full run that ends before a step, and the whole Cleanup follows.
#[tokio::test]
async fn req04a_criterion_1_through_any_step_is_a_prefix_of_the_full_run() {
    let (project, scenario, _) = workflow();
    let whole = report(full(&project, &scenario, RunMode::Simulation)).await;
    let main: Vec<_> = whole
        .steps
        .iter()
        .filter(|step| step.phase == Phase::Main)
        .collect();
    for (at, target) in main.iter().enumerate() {
        let part = report(through(
            &project,
            &scenario,
            RunMode::Simulation,
            &target.handle,
        ))
        .await;
        let steps: Vec<_> = part
            .steps
            .iter()
            .filter(|step| step.phase == Phase::Main)
            .collect();
        let end = main[at + 1..]
            .iter()
            .position(|step| !step.kind.is_data())
            .map_or(main.len(), |next| at + 1 + next);
        assert_eq!(steps.len(), end, "through {}", target.handle);
        for (part, whole) in steps.iter().zip(&main) {
            assert_eq!(untimed(part), untimed(whole), "through {}", target.handle);
        }
        assert_eq!(
            part.steps.len() - steps.len(),
            scenario.cleanup.nodes.len(),
            "Cleanup runs after {}",
            target.handle
        );
    }
}

const WITH_CLEANUP: &str = r#"scenario "REQ-04a cleanup" {
  on_error stop
  begin = start
  first = POST "/first" {
    sample 201 { id: "f-1" }
  }
  wrong = assert first.status == 200
  second = POST "/second" {
    sample 201 { id: "s-1" }
  }
  third = POST "/third" {
    sample 201 { id: "t-1" }
  }
  cleanup {
    drop_first = DELETE "/first/{{first.body.id}}" {
      sample 204 {}
    }
    drop_third = DELETE "/third/{{third.body.id}}" {
      sample 204 {}
    }
  }
}
"#;

/// Cleanup runs whole after the part, also after a failure under the policy
/// *Stop*; a Cleanup step that reads a step the part left out is skipped and
/// names it.
#[tokio::test]
async fn req04a_criterion_1_cleanup_runs_after_the_part() {
    let (project, scenario) = scenario(WITH_CLEANUP);
    for (handle, main) in [
        ("first", &["begin", "first", "wrong"][..]),
        ("second", &["begin", "first", "wrong", "second"][..]),
    ] {
        let part = report(through(&project, &scenario, RunMode::Simulation, handle)).await;
        assert_eq!(
            handles(&part),
            [main, &["drop_first", "drop_third"]].concat()
        );
        assert_eq!(step(&part, "wrong").status, StepStatus::Failed);
        if handle == "second" {
            assert_eq!(
                step(&part, "second").cause,
                Some(NotRunCause::Stopped {
                    by: id(&scenario, "wrong")
                })
            );
        }
        let first = step(&part, "drop_first");
        assert_eq!(first.phase, Phase::Cleanup);
        assert_eq!(first.status, StepStatus::Passed);
        let third = step(&part, "drop_third");
        assert_eq!(third.status, StepStatus::Skipped);
        assert_eq!(third.cause, blocked_by(&scenario, "third"));
    }
}

// Criterion 2 --------------------------------------------------------------------

const BRANCHES: &str = r#"scenario "REQ-04a branches" {
  on_error stop
  begin = start
  first = GET "/first" {
    sample 200 { id: "f-1" }
  }
  found = if first.status == 200 {
    taken = GET "/taken/{{first.body.id}}" {
      sample 200 { ok: true }
    }
    taken_ok = assert taken.status == 200
    first_there = assert first.body.id == "f-1"
  } else {
    other = GET "/other" {
      sample 200 { ok: true }
    }
    other_ok = assert other.status == 200
    first_gone = assert first.status == 404
  }
  last = GET "/last" {
    sample 200 { ok: true }
  }
}
"#;

/// Through a request in a branch not taken: it and its check are skipped as
/// in a full run.
#[tokio::test]
async fn req04a_criterion_2_through_a_branch_not_taken() {
    let (project, scenario) = scenario(BRANCHES);
    let whole = report(full(&project, &scenario, RunMode::Simulation)).await;
    let part = report(through(&project, &scenario, RunMode::Simulation, "other")).await;
    for handle in ["other", "other_ok"] {
        let result = step(&part, handle);
        assert_eq!(result.status, StepStatus::Skipped);
        assert_eq!(result.cause, Some(NotRunCause::OutsidePath));
        assert_eq!(untimed(result), untimed(step(&whole, handle)));
    }
    assert!(!handles(&part).contains(&"last"), "{:?}", handles(&part));
    assert_eq!(part.status, RunStatus::Passed);
}

// Criterion 3 --------------------------------------------------------------------

/// One request and the check that reads it; every other value is the earlier
/// run's.
#[tokio::test]
async fn req04a_criterion_3_alone_runs_the_request_and_its_checks() {
    let (project, scenario, previous) = workflow();
    let run = report(alone(
        &project,
        &scenario,
        RunMode::Simulation,
        "dodaj_produkty_do_koszyka",
        Some(&previous),
    ))
    .await;
    assert_eq!(
        handles(&run),
        ["dodaj_produkty_do_koszyka", "porownaj_sume_z_pobrana_cena"]
    );
    assert!(run.steps.iter().all(|step| step.phase == Phase::Main));
    assert_eq!(
        run.steps
            .iter()
            .filter(|step| step.request.is_some())
            .count(),
        1
    );
    assert!(
        run.steps
            .iter()
            .all(|step| step.status == StepStatus::Passed),
        "{run:#?}"
    );
    // The values are the earlier run's, as a single step reads them.
    let single = report(RunRequest {
        single_node_id: Some(id(&scenario, "dodaj_produkty_do_koszyka")),
        previous_run: Some(previous.clone()),
        ..full(&project, &scenario, RunMode::Simulation)
    })
    .await;
    assert_eq!(untimed(&run.steps[0]), untimed(&single.steps[0]));
    assert_eq!(run.notice, single.notice);
    assert!(run.diagnostic);
    assert_eq!(run.status, RunStatus::Passed);
}

/// A request in a branch the earlier run did not take is sent with the checks
/// that stand beside it.
#[tokio::test]
async fn req04a_criterion_3_alone_in_a_branch_not_taken() {
    let (project, scenario) = scenario(BRANCHES);
    let previous = report(full(&project, &scenario, RunMode::Simulation)).await;
    for previous in [Some(&previous), None] {
        let run = report(alone(
            &project,
            &scenario,
            RunMode::Simulation,
            "other",
            previous,
        ))
        .await;
        assert_eq!(handles(&run), ["other", "other_ok"]);
        assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    }
}

/// A check in a branch the request does not stand in follows what the earlier
/// run decided, so the two arms are never both judged.
#[tokio::test]
async fn req04a_criterion_3_a_check_in_a_branch_not_taken_stays_out() {
    let (project, scenario) = scenario(BRANCHES);
    let previous = report(full(&project, &scenario, RunMode::Simulation)).await;
    let run = report(alone(
        &project,
        &scenario,
        RunMode::Simulation,
        "first",
        Some(&previous),
    ))
    .await;
    assert_eq!(handles(&run), ["first", "first_there", "first_gone"]);
    assert_eq!(step(&run, "first_there").status, StepStatus::Passed);
    let gone = step(&run, "first_gone");
    assert_eq!(gone.status, StepStatus::Skipped);
    assert_eq!(gone.cause, Some(NotRunCause::OutsidePath));
    assert_eq!(run.status, RunStatus::Passed);

    // With nothing decided, both are held back by the `if`.
    let run = report(alone(
        &project,
        &scenario,
        RunMode::Simulation,
        "first",
        None,
    ))
    .await;
    assert_eq!(step(&run, "first").status, StepStatus::Passed);
    for handle in ["first_there", "first_gone"] {
        assert_eq!(step(&run, handle).status, StepStatus::Blocked);
        assert_eq!(step(&run, handle).cause, blocked_by(&scenario, "found"));
    }
}

const CHECKS: &str = r#"scenario "REQ-04a checks" {
  on_error stop
  begin = start
  first = GET "/first" {
    sample 200 { id: "f-1" }
  }
  second = GET "/second" {
    sample 200 { id: "f-1", name: "n" }
  }
  wrong = assert second.status == 404
  same = assert second.body.id == first.body.id
  named = assert second.body.name == "n"
  later = GET "/later/{{second.body.id}}" {
    sample 200 { ok: true }
  }
  later_same = assert later.body.ok == true
}
"#;

/// The checks of a request alone are all evaluated: no step comes after them,
/// so the policy *Stop* has nothing to stop.
#[tokio::test]
async fn req04a_criterion_3_a_failed_check_does_not_hold_the_others_back() {
    let (project, scenario) = scenario(CHECKS);
    let previous = report(full(&project, &scenario, RunMode::Simulation)).await;
    let run = report(alone(
        &project,
        &scenario,
        RunMode::Simulation,
        "second",
        Some(&previous),
    ))
    .await;
    assert_eq!(handles(&run), ["second", "wrong", "same", "named"]);
    let statuses: Vec<_> = run.steps.iter().map(|step| step.status).collect();
    assert_eq!(
        statuses,
        [
            StepStatus::Passed,
            StepStatus::Failed,
            StepStatus::Passed,
            StepStatus::Passed
        ]
    );
    assert_eq!(run.status, RunStatus::Failed);
}

/// The checks read the response of this run, not the earlier one's.
#[tokio::test]
async fn req04a_criterion_3_checks_read_the_new_response() {
    let (project, scenario) = scenario(CHECKS);
    let mut previous = report(full(&project, &scenario, RunMode::Simulation)).await;
    for step in &mut previous.steps {
        if step.handle == "second" {
            step.outputs
                .insert("body".into(), json!({ "id": "stale", "name": "stale" }));
        }
    }
    let run = report(alone(
        &project,
        &scenario,
        RunMode::Simulation,
        "second",
        Some(&previous),
    ))
    .await;
    assert_eq!(step(&run, "named").status, StepStatus::Passed);
    assert_eq!(step(&run, "same").status, StepStatus::Passed);
}

const CONDITIONS: &str = r#"scenario "REQ-04a values" {
  on_error stop
  begin = start
  first = GET "/first" {
    sample 200 { id: "f-1" }
  }
  ready = condition all {
    first.status == 200,
  } with first.body
  second = GET "/second/{{ready.value.id}}" {
    sample 200 { ok: true }
  }
  second_ok = condition all {
    second.body.ok == true,
  } with second.body
  third = assert second_ok.value.ok == true
}
"#;

/// A request that reads a *Data condition* reads it from the earlier run; its
/// own conditions are its checks, and a check of another check is not.
#[tokio::test]
async fn req04a_criterion_3_conditions() {
    let (project, scenario) = scenario(CONDITIONS);
    let previous = report(full(&project, &scenario, RunMode::Simulation)).await;
    let request = |previous: &RunResult| {
        alone(
            &project,
            &scenario,
            RunMode::Simulation,
            "second",
            Some(previous),
        )
    };
    let run = report(request(&previous)).await;
    assert_eq!(handles(&run), ["second", "second_ok"]);
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    // One that was false guards the request, as in a single step.
    let mut unmet = previous.clone();
    for step in &mut unmet.steps {
        if step.handle == "ready" {
            step.status = StepStatus::Blocked;
            step.outputs.insert("passed".into(), json!(false));
        }
    }
    let run = report(request(&unmet)).await;
    let second = step(&run, "second");
    assert!(second.request.is_none());
    assert_eq!(
        second.cause,
        Some(NotRunCause::ConditionUnmet {
            condition: id(&scenario, "ready")
        })
    );
    assert_eq!(
        step(&run, "second_ok").cause,
        blocked_by(&scenario, "second")
    );
}

const COMPONENT: &str = r#"component partial_maker "Maker" {
  made = POST "/made" {
    sample 201 { id: "m-1" }
  }
  again = POST "/again" {
    sample 201 { id: "m-2" }
  }
  wrong = assert again.status == 200
  last = POST "/last" {
    sample 201 { id: "m-3" }
  }
  export id: string = made.body.id
}
"#;

const CALLS: &str = r#"scenario "REQ-04a calls" {
  on_error stop
  begin = start
  maker = partial_maker@1()
  read = GET "/read/{{maker}}" {
    sample 200 { ok: true }
  }
  read_ok = assert read.body.ok == true
}
"#;

/// A component call inside a part keeps the rules of every run, *Stop*
/// included; a value it made is read from the earlier run by its existence.
#[tokio::test]
async fn req04a_criterion_3_component_calls() {
    let mut project = empty_project();
    project.scenarios.clear();
    project.environment.base_url = "http://127.0.0.1:9".into();
    let lowered = |project: &Project, text: &str| {
        let result = lower(&parse(text), &LowerContext::folder(project));
        assert!(!result.has_errors(), "{:#?}", result.diagnostics);
        result.document.expect("a document")
    };
    let Document::Component(mut component) = lowered(&project, COMPONENT) else {
        panic!("not a component");
    };
    component.revision = 1;
    project.components.push(component);
    let Document::Scenario(scenario) = lowered(&project, CALLS) else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    let mode = RunMode::Simulation;

    let whole = report(full(&project, &scenario, mode)).await;
    let inner = |run: &RunResult| -> Vec<StepStatus> {
        let children = step(run, "maker").children.clone().expect("inner steps");
        children.iter().map(|step| step.status).collect()
    };
    for part in [
        through(&project, &scenario, mode, "maker"),
        alone(&project, &scenario, mode, "maker", None),
    ] {
        let part = report(part).await;
        assert_eq!(inner(&part), inner(&whole));
        assert!(inner(&part).contains(&StepStatus::Skipped));
    }
    let run = report(alone(&project, &scenario, mode, "read", Some(&whole))).await;
    assert_eq!(handles(&run), ["read", "read_ok"]);
    let url = &step(&run, "read").request.as_ref().expect("sent").url;
    assert!(url.ends_with("/read/m-1"), "{url}");
}

// Criterion 4 --------------------------------------------------------------------

/// The check's other subject is missing from the earlier run: the request is
/// sent, the check names the step.
#[tokio::test]
async fn req04a_criterion_4_a_check_without_its_other_subject() {
    let (project, scenario, mut previous) = workflow();
    previous
        .steps
        .retain(|step| step.handle != "pobierz_ceny_produktow");
    let run = report(alone(
        &project,
        &scenario,
        RunMode::Simulation,
        "dodaj_produkty_do_koszyka",
        Some(&previous),
    ))
    .await;
    let sent = step(&run, "dodaj_produkty_do_koszyka");
    assert_eq!(sent.status, StepStatus::Passed);
    assert!(sent.request.is_some());
    let check = step(&run, "porownaj_sume_z_pobrana_cena");
    assert_eq!(check.status, StepStatus::Blocked);
    assert_eq!(check.cause, blocked_by(&scenario, "pobierz_ceny_produktow"));
}

/// The same with no earlier run at all, under either policy: the order of the
/// lines does not hold the request back. The other checks are evaluated.
#[tokio::test]
async fn req04a_criterion_4_with_no_earlier_run() {
    for text in [
        CHECKS.to_owned(),
        CHECKS.replace("on_error stop", "on_error continue"),
    ] {
        let (project, scenario) = scenario(&text);
        let run = report(alone(
            &project,
            &scenario,
            RunMode::Simulation,
            "second",
            None,
        ))
        .await;
        assert_eq!(step(&run, "second").status, StepStatus::Passed);
        assert_eq!(step(&run, "same").cause, blocked_by(&scenario, "first"));
        assert_eq!(step(&run, "named").status, StepStatus::Passed);
        assert_eq!(
            run.notice.as_deref(),
            Some("Single step: no historical inputs.")
        );
    }
}

/// A value the request itself reads is missing: nothing is sent, and its
/// checks name the request.
#[tokio::test]
async fn req04a_criterion_4_a_request_without_its_value_is_not_sent() {
    let (project, scenario) = scenario(CHECKS);
    let run = report(alone(
        &project,
        &scenario,
        RunMode::Simulation,
        "later",
        None,
    ))
    .await;
    let later = step(&run, "later");
    assert_eq!(later.status, StepStatus::Blocked);
    assert!(later.request.is_none());
    assert_eq!(later.cause, blocked_by(&scenario, "second"));
    assert_eq!(
        step(&run, "later_same").cause,
        blocked_by(&scenario, "later")
    );
}

// Criterion 5 --------------------------------------------------------------------

#[tokio::test]
async fn req04a_criterion_5_the_earlier_run_is_validated() {
    let (project, scenario, previous) = workflow();
    let refused = |change: &dyn Fn(&mut RunResult)| {
        let mut previous = previous.clone();
        change(&mut previous);
        run(alone(
            &project,
            &scenario,
            RunMode::Simulation,
            "dodaj_produkty_do_koszyka",
            Some(&previous),
        ))
    };
    for result in [
        refused(&|run| run.scenario_id = "another".into()).await,
        refused(&|run| run.mode = RunMode::Live).await,
        refused(&|run| run.status = RunStatus::Running).await,
        refused(&|run| run.finished_at = None).await,
    ] {
        assert!(
            matches!(result, Err(RunError::InvalidPreviousRun(_))),
            "{result:?}"
        );
    }
}

#[tokio::test]
async fn req04a_criterion_5_conflicts_and_unknown_nodes() {
    let (project, scenario, previous) = workflow();
    let node = id(&scenario, "dodaj_produkty_do_koszyka");
    let partials = [
        Partial::Through {
            node_id: node.clone(),
        },
        Partial::Alone {
            node_id: node.clone(),
            previous: Some(Box::new(previous.clone())),
        },
    ];
    for partial in partials {
        let request = RunRequest {
            partial: Some(partial),
            ..full(&project, &scenario, RunMode::Simulation)
        };
        for conflicting in [
            RunRequest {
                target_node_id: Some(node.clone()),
                ..request.clone()
            },
            RunRequest {
                single_node_id: Some(node.clone()),
                ..request.clone()
            },
        ] {
            let result = run(conflicting).await;
            assert!(matches!(result, Err(RunError::ConflictingModes)));
        }
        let result = run(RunRequest {
            previous_run: Some(previous.clone()),
            ..request
        })
        .await;
        assert!(matches!(
            result,
            Err(RunError::PreviousRunWithoutSingleNode)
        ));
    }
    // A Cleanup step is not a node of the main graph.
    for unknown in ["nowhere", scenario.cleanup.nodes[0].id.as_str()] {
        for partial in [
            Partial::Through {
                node_id: unknown.into(),
            },
            Partial::Alone {
                node_id: unknown.into(),
                previous: None,
            },
        ] {
            let result = run(RunRequest {
                partial: Some(partial),
                ..full(&project, &scenario, RunMode::Simulation)
            })
            .await;
            assert!(matches!(result, Err(RunError::NodeNotFound(id)) if id == unknown));
        }
    }
}

/// The report is diagnostic from its first snapshot, through the service too.
#[test]
fn req04a_criterion_5_the_report_is_diagnostic() {
    let (project, scenario) = labels();
    let service = RunnerService::new(Arc::new(|| {})).expect("the service starts");
    for request in [
        through(&project, &scenario, RunMode::Simulation, "weight_0"),
        alone(&project, &scenario, RunMode::Simulation, "weight_0", None),
    ] {
        let handle = service.start(request).expect("the run starts");
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !handle.is_finished() {
            assert!(std::time::Instant::now() < deadline, "the run ends");
            std::thread::sleep(Duration::from_millis(5));
        }
        let run = handle.latest().expect("a report");
        assert!(run.diagnostic);
        assert_eq!(run.status, RunStatus::Passed);
    }
    service.shutdown(Duration::from_secs(5));
}

// Criterion 6 --------------------------------------------------------------------

/// One line per call, the state of a request included.
#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl Recorder {
    fn note(&self, line: String) -> Result<(), JournalSinkError> {
        self.0.lock().expect("the log").push(line);
        Ok(())
    }

    fn calls(&self) -> Vec<String> {
        self.0.lock().expect("the log").clone()
    }
}

impl JournalSink for Recorder {
    fn begin_run(&self, _: &JournalRun) -> Result<(), JournalSinkError> {
        self.note("begin".into())
    }

    fn record(&self, _: &str, entry: &JournalEntry) -> Result<(), JournalSinkError> {
        self.note(format!("{} {:?}", entry.method, entry.state))
    }

    fn cleanup_started(&self, _: &str) -> Result<(), JournalSinkError> {
        self.note("cleanup".into())
    }

    fn finish_run(&self, _: &str) -> Result<(), JournalSinkError> {
        self.note("finish".into())
    }
}

struct Server {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Server {
    /// Answers every request with 201 and an id, or never answers.
    async fn start(answer: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
        let base_url = format!("http://{}", listener.local_addr().expect("an address"));
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        let task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0; 4096];
                let read = stream.read(&mut buffer).await.unwrap_or(0);
                let text = String::from_utf8_lossy(&buffer[..read]);
                let line: Vec<&str> = text.split_whitespace().take(2).collect();
                log.lock().expect("the log").push(line.join(" "));
                if !answer {
                    std::future::pending::<()>().await;
                }
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 10\r\nConnection: close\r\n\r\n{\"id\":\"1\"}",
                    )
                    .await;
            }
        });
        Self {
            base_url,
            received,
            task,
        }
    }

    fn requests(&self) -> Vec<String> {
        self.received.lock().expect("the log").clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn journalled(request: RunRequest, cancel: CancellationToken) -> (RunResult, Vec<String>) {
    let journal = Arc::new(Recorder::default());
    let options = RunOptions::new(HttpTransport::new().expect("client builds"))
        .with_journal(Arc::clone(&journal) as _);
    let run = run_scenario(request, options, cancel, |_| {})
        .await
        .expect("the run starts");
    (run, journal.calls())
}

#[tokio::test]
async fn req04a_criterion_6_through_is_journalled_as_a_full_run() {
    let server = Server::start(true).await;
    let (mut project, scenario) =
        scenario(&WITH_CLEANUP.replace("first.status == 200", "first.status == 201"));
    project.environment.base_url = server.base_url.clone();
    let (run, calls) = journalled(
        through(&project, &scenario, RunMode::Live, "first"),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(server.requests(), ["POST /first", "DELETE /first/1"]);
    assert_eq!(
        calls,
        [
            "begin",
            "POST InFlight",
            "POST Completed",
            "cleanup",
            "DELETE InFlight",
            "DELETE Completed",
            "finish"
        ]
    );
}

#[tokio::test]
async fn req04a_criterion_6_alone_is_journalled_and_sends_one_request() {
    let server = Server::start(true).await;
    let (mut project, scenario) = scenario(WITH_CLEANUP);
    project.environment.base_url = server.base_url.clone();
    let (run, calls) = journalled(
        alone(&project, &scenario, RunMode::Live, "first", None),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(handles(&run), ["first", "wrong"]);
    assert_eq!(server.requests(), ["POST /first"]);
    assert_eq!(
        calls,
        ["begin", "POST InFlight", "POST Completed", "finish"]
    );
}

/// Cancelled while its request is out: the journal is resolved and ended, the
/// checks did not run, and nothing else is sent.
#[tokio::test]
async fn req04a_criterion_6_alone_cancelled() {
    let server = Server::start(false).await;
    let (mut project, scenario) = scenario(WITH_CLEANUP);
    project.environment.base_url = server.base_url.clone();
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let received = Arc::clone(&server.received);
    tokio::spawn(async move {
        while received.lock().expect("the log").is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        token.cancel();
    });
    let (run, calls) = journalled(
        alone(&project, &scenario, RunMode::Live, "first", None),
        cancel,
    )
    .await;
    assert_eq!(run.status, RunStatus::Cancelled);
    assert_eq!(step(&run, "first").status, StepStatus::Cancelled);
    assert_eq!(step(&run, "wrong").cause, Some(NotRunCause::Cancelled));
    assert_eq!(handles(&run), ["first", "wrong"]);
    assert_eq!(server.requests(), ["POST /first"]);
    assert_eq!(
        calls,
        ["begin", "POST InFlight", "POST Uncertain", "finish"]
    );
}
