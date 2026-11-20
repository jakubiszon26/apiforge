//! RUN-04 (#418): a stopped run says "stopped" on every step it did not
//! reach, and *Start* is the first thing a run does.
//!
//! Under the error policy *Stop*, a step skipped only because something
//! before it did not run reads "Stopped after an error." and names the step
//! that failed; a step on a branch that was not taken stays "outside the
//! active execution path". A ready *Start* is taken before a value that reads
//! no step. The runs here are simulated in the demo shop project.

use apiforge_core::{
    demos::shop_project,
    format::{Project, Scenario},
    run::{NotRunCause, Phase, RunMode, RunResult, RunStatus, StepResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use std::{fs, path::Path};
use tokio_util::sync::CancellationToken;

const OUTSIDE_PATH: &str = "Step outside the active execution path.";
const STOPPED: &str = "Stopped after an error.";

fn lowered(project: &Project, text: &str) -> Document {
    let result = lower(&parse(text), &LowerContext::folder(project));
    assert!(!result.has_errors(), "{:#?}", result.diagnostics);
    result.document.expect("a document")
}

/// The scenario lowered in `project`, which it joins.
fn scenario_in(mut project: Project, text: &str) -> (Project, Scenario) {
    let Document::Scenario(scenario) = lowered(&project, text) else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    (project, scenario)
}

fn scenario(text: &str) -> (Project, Scenario) {
    scenario_in(shop_project(), text)
}

fn shop_30() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/shop-30.afs");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// `shop-30` with the sample total of `checkout` broken.
fn broken_checkout() -> String {
    let text = shop_30();
    let broken = text.replace(
        r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#,
        r#"sample 201 { orderId: "o-1", status: "created", total: 170 }"#,
    );
    assert_ne!(broken, text);
    broken
}

/// `shop-30` with the sample amount of `refund` broken: it fails after the
/// branch has been decided.
fn broken_refund() -> String {
    let text = shop_30();
    let broken = text.replace(
        "sample 200 { refunded: true, amount: 172.5 }",
        "sample 200 { refunded: true, amount: 170 }",
    );
    assert_ne!(broken, text);
    broken
}

async fn simulate(project: Project, scenario: &Scenario) -> RunResult {
    run_scenario(
        RunRequest::new(project, scenario.id.clone(), RunMode::Simulation),
        RunOptions::new(HttpTransport::new().expect("client builds")),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("the run starts")
}

fn step<'a>(steps: &'a [StepResult], handle: &str) -> &'a StepResult {
    steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no result for `{handle}`: {steps:#?}"))
}

fn id(steps: &[StepResult], handle: &str) -> String {
    step(steps, handle).node_id.clone()
}

fn handles(steps: &[StepResult], phase: Phase) -> Vec<&str> {
    steps
        .iter()
        .filter(|step| step.phase == phase)
        .map(|step| step.handle.as_str())
        .collect()
}

/// `handle` was skipped, reads "Stopped after an error." and names `by`.
fn assert_stopped(steps: &[StepResult], handle: &str, by: &str) {
    let found = step(steps, handle);
    assert_eq!(found.status, StepStatus::Skipped, "{handle}: {found:#?}");
    assert_eq!(found.message.as_deref(), Some(STOPPED), "{handle}");
    assert_eq!(
        found.cause,
        Some(NotRunCause::Stopped { by: id(steps, by) }),
        "{handle}"
    );
}

/// `handle` was skipped as outside the active execution path.
fn assert_outside(steps: &[StepResult], handle: &str) {
    let found = step(steps, handle);
    assert_eq!(found.status, StepStatus::Skipped, "{handle}: {found:#?}");
    assert_eq!(found.message.as_deref(), Some(OUTSIDE_PATH), "{handle}");
    assert_eq!(found.cause, Some(NotRunCause::OutsidePath), "{handle}");
}

/// A step carries "Step outside the active execution path." if and only if
/// its cause is `OutsidePath`, in the main phase and in a component's steps.
fn assert_outside_only_when_outside(steps: &[StepResult]) {
    for step in steps.iter().filter(|step| step.phase == Phase::Main) {
        assert_eq!(
            step.message.as_deref() == Some(OUTSIDE_PATH),
            step.cause == Some(NotRunCause::OutsidePath),
            "`{}`: {:?} {:?}",
            step.handle,
            step.message,
            step.cause
        );
        if let Some(children) = &step.children {
            assert_outside_only_when_outside(children);
        }
    }
}

// Criterion 1 ------------------------------------------------------------------

const CHAIN: &str = r#"scenario "A chain stopped by a check" {
  begin = start
  a = GET "/a" {
    sample 200 { ok: true }
  }
  broken = assert a.body.ok == false
  b = GET "/b" {
    sample 200 {}
  }
  c = GET "/c" {
    sample 200 {}
  }
  d = GET "/d" {
    sample 200 {}
  }
}
"#;

/// Criterion 1: every step of the chain after the failed check is stopped by
/// it, not only the first.
#[tokio::test]
async fn run04_criterion_1_a_chain_after_a_failed_check_is_stopped() {
    let (project, scenario) = scenario(CHAIN);
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed, "{run:#?}");
    assert_eq!(step(&run.steps, "broken").status, StepStatus::Failed);
    for handle in ["b", "c", "d"] {
        assert_stopped(&run.steps, handle, "broken");
    }
    assert_outside_only_when_outside(&run.steps);
}

/// Criterion 1 on `shop-30` with the wrong checkout total: all eleven steps
/// not run are stopped by `checkout_total`, a branch of the `if` that did
/// not run among them (criterion 2); none is blocked by a neighbour.
#[tokio::test]
async fn run04_criterion_1_shop_30_is_stopped_by_checkout_total() {
    let (project, scenario) = scenario(&broken_checkout());
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed);
    let not_run: Vec<&StepResult> = run
        .steps
        .iter()
        .filter(|step| step.cause.is_some())
        .collect();
    assert_eq!(not_run.len(), 11, "{not_run:#?}");
    for found in not_run {
        assert_stopped(&run.steps, &found.handle, "checkout_total");
    }
    for handle in ["paid", "payment", "receipt", "not_created", "logout"] {
        assert_stopped(&run.steps, handle, "checkout_total");
    }
    assert_outside_only_when_outside(&run.steps);
}

// Criterion 2 ------------------------------------------------------------------

/// Criterion 2: in `shop-30` stopped by the refund's amount, the arm of the
/// `if` that was not taken stays outside the path.
#[tokio::test]
async fn run04_criterion_2_shop_30_keeps_the_branch_not_taken_outside() {
    let (project, scenario) = scenario(&broken_refund());
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed);
    assert_outside(&run.steps, "not_created");
    assert_stopped(&run.steps, "logout", "refund_amount");
    assert_outside_only_when_outside(&run.steps);
}

const ARM_NOT_TAKEN: &str = r#"scenario "A chain in the arm not taken" {
  begin = start
  first = GET "/a" {
    sample 200 {}
  }
  chosen = if first.status == 200 {
    yes = GET "/yes" {
      sample 200 {}
    }
    wrong = assert yes.status == 500
  } else {
    no = GET "/no" {
      sample 200 {}
    }
    no_more = GET "/no-more" {
      sample 200 {}
    }
  }
}
"#;

/// Criterion 2: a hand-written chain inside the arm not taken of an `if` that
/// ran keeps `OutsidePath` after a failure in the arm taken.
#[tokio::test]
async fn run04_criterion_2_a_chain_in_the_arm_not_taken_stays_outside() {
    let (project, scenario) = scenario(ARM_NOT_TAKEN);
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed, "{run:#?}");
    assert_eq!(step(&run.steps, "wrong").status, StepStatus::Failed);
    assert_outside(&run.steps, "no");
    assert_outside(&run.steps, "no_more");
    assert_outside_only_when_outside(&run.steps);
}

// Criterion 4 ------------------------------------------------------------------

const COMPONENT: &str = r#"component chain_flow "Chain flow" {
  begin = start
  cart = POST "/carts" {
    sample 201 { id: "c-1" }
  }
  created = assert cart.status == 200
  paid = POST "/payments" {
    sample 201 {}
  }
  confirmed = GET "/payments/p-1" {
    sample 200 {}
  }
  export id: string = cart.body.id
}
"#;

const CALLER: &str = r#"scenario "Calls the chain flow" {
  on_error stop
  begin = start
  flow = chain_flow@1()
  next = GET "/next" {
    sample 200 {}
  }
  last = GET "/last" {
    sample 200 {}
  }
}
"#;

/// Criterion 4: inside a component the inner steps after the failed check are
/// stopped by it; the scenario's steps after the call are stopped by the call.
#[tokio::test]
async fn run04_criterion_4_inside_a_component() {
    let mut project = shop_project();
    let Document::Component(mut component) = lowered(&project, COMPONENT) else {
        panic!("not a component");
    };
    component.revision = 1;
    project.components.push(component);
    let (project, scenario) = scenario_in(project, CALLER);
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed, "{run:#?}");
    let children = step(&run.steps, "flow")
        .children
        .as_deref()
        .expect("the call's inner steps");
    assert_eq!(step(children, "created").status, StepStatus::Failed);
    for handle in ["paid", "confirmed"] {
        assert_stopped(children, handle, "created");
    }
    for handle in ["next", "last"] {
        assert_stopped(&run.steps, handle, "flow");
    }
    assert_outside_only_when_outside(&run.steps);
}

// Criterion 5 ------------------------------------------------------------------

/// A value written before *Start* that reads no step, read by a later request.
fn value_before_start(name: &str, value: &str) -> String {
    format!(
        r#"scenario "A value before Start" {{
  {name} = {value}
  begin = start
  first = GET "/a" {{
    sample 200 {{}}
  }}
  second = GET "/b" {{
    query {{ limit: {name} }}
    sample 200 {{}}
  }}
}}
"#
    )
}

/// Criterion 5: *Start* runs first; a value that fails after it stops every
/// later step, and none is "outside the active execution path".
#[tokio::test]
async fn run04_criterion_5_start_runs_before_a_failing_value() {
    let (project, scenario) = scenario(&value_before_start("bad", r#""1" + 1"#));
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed, "{run:#?}");
    assert_eq!(
        handles(&run.steps, Phase::Main),
        ["begin", "bad", "first", "second"]
    );
    assert_eq!(step(&run.steps, "begin").status, StepStatus::Passed);
    assert_eq!(step(&run.steps, "begin").cause, None);
    assert!(
        matches!(
            step(&run.steps, "bad").status,
            StepStatus::Failed | StepStatus::Error
        ),
        "{run:#?}"
    );
    for handle in ["first", "second"] {
        assert_stopped(&run.steps, handle, "bad");
    }
    assert!(
        run.steps
            .iter()
            .all(|step| step.message.as_deref() != Some(OUTSIDE_PATH)),
        "{run:#?}"
    );
    assert_outside_only_when_outside(&run.steps);
}

/// Criterion 5: a value that reads no step and passes runs right after *Start*.
#[tokio::test]
async fn run04_criterion_5_start_runs_before_a_value() {
    let (project, scenario) = scenario(&value_before_start("limit", "10"));
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(
        handles(&run.steps, Phase::Main),
        ["begin", "limit", "first", "second"]
    );
}

/// Criterion 5: a graph without a *Start* keeps the order of RUN-03, a ready
/// value before a ready step.
#[tokio::test]
async fn run04_criterion_5_a_graph_without_start_keeps_its_order() {
    // Since LANG-13 (#386) only a published revision has no *Start*: it
    // is read in its stored spelling.
    let text = r#"scenario "Without Start" {
  first = GET "/a" {
    sample 200 {}
  }
  limit = 10
  cleanup {
    removed = DELETE "/a" {
      sample 204 {}
    }
    tidy = 10
  }
}
"#;
    let mut project = shop_project();
    let result = lower(&parse(text), &LowerContext::folder(&project).stored());
    assert!(!result.has_errors(), "{:#?}", result.diagnostics);
    let Some(Document::Scenario(scenario)) = result.document else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(handles(&run.steps, Phase::Main), ["limit", "first"]);
    assert_eq!(handles(&run.steps, Phase::Cleanup), ["tidy", "removed"]);
}
