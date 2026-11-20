//! LANG-13 (#386): a run follows the order the steps are written in.
//!
//! The runs here are simulated: nothing is sent.

use apiforge_core::{
    demos::shop_project,
    format::{Project, Scenario},
    model::{Branch, NodeKind},
    run::{Phase, RunMode, RunResult, RunStatus, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

/// The demo shop with the room booking's published `sign_in`, read in its
/// stored spelling: every component a fixture calls.
fn project() -> Project {
    let mut project = shop_project();
    let text = std::fs::read_to_string(
        repository().join("fixtures/scenarios/lay/rooms/.apiforge/revisions/sign-in/1.afs"),
    )
    .unwrap();
    let lowered = lower(&parse(&text), &LowerContext::folder(&project).stored());
    let Some(Document::Component(sign_in)) = lowered.document else {
        panic!("a component");
    };
    project.components.push(sign_in);
    project
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The scenario lowered as a folder source in `project`, which it joins.
fn scenario_in(mut project: Project, text: &str) -> (Project, Scenario) {
    let lowered = lower(&parse(text), &LowerContext::folder(&project));
    assert!(!lowered.has_errors(), "{text}\n{:#?}", lowered.diagnostics);
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    (project, scenario)
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

/// The handles of the steps that do something, in the order the run took
/// them, in `phase`.
fn taken(run: &RunResult, phase: Phase) -> Vec<String> {
    run.steps
        .iter()
        .filter(|step| step.phase == phase)
        .filter(|step| {
            matches!(
                step.kind,
                NodeKind::Request | NodeKind::Component | NodeKind::If
            )
        })
        .map(|step| step.handle.clone())
        .collect()
}

fn status(run: &RunResult, handle: &str) -> StepStatus {
    run.steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no step `{handle}`"))
        .status
}

/// Requests none of which reads another, a check written after a later
/// request, a branch not taken and a step after the `if`.
const WRITTEN: &str = r#"scenario "The order written" {
  on_error continue
  begin = start
  zeta = GET "/zeta" {
    sample 200 { id: 1 }
  }
  alpha = GET "/alpha" {
    sample 200 { id: 2 }
  }
  zeta_ok = assert zeta.status == 200
  chosen = if alpha.status == 404 {
    missing = GET "/missing" {
      sample 200 {}
    }
    after_missing = GET "/after-missing" {
      sample 200 {}
    }
  } else {
    found = GET "/found" {
      sample 200 {}
    }
  }
  mid = GET "/mid" {
    sample 200 {}
  }
  cleanup {
    second = DELETE "/second" {
      sample 204 {}
    }
    first = DELETE "/first" {
      sample 204 {}
    }
  }
}
"#;

#[tokio::test]
async fn lang13_criterion_1_the_run_follows_the_text() {
    let (project, scenario) = scenario_in(project(), WRITTEN);
    // Every statement in braces keeps the edge of its braces, which RUN-01
    // and an undecided `if` read.
    let chosen = scenario
        .graph
        .nodes
        .iter()
        .find(|node| node.handle == "chosen")
        .unwrap()
        .id
        .clone();
    for (handle, branch) in [
        ("missing", Branch::True),
        ("after_missing", Branch::True),
        ("found", Branch::False),
    ] {
        let id = &scenario
            .graph
            .nodes
            .iter()
            .find(|node| node.handle == handle)
            .unwrap()
            .id;
        assert!(
            scenario.graph.edges.iter().any(|edge| edge.source == chosen
                && edge.target == *id
                && edge.branch == Some(branch)),
            "{handle} keeps its branch edge"
        );
    }
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{:#?}", run.steps);
    assert_eq!(
        taken(&run, Phase::Main),
        [
            "zeta",
            "alpha",
            "chosen",
            "missing",
            "after_missing",
            "found",
            "mid"
        ]
    );
    for handle in ["zeta", "alpha", "found", "mid"] {
        assert_eq!(status(&run, handle), StepStatus::Passed, "{handle}");
    }
    for handle in ["missing", "after_missing"] {
        assert_eq!(status(&run, handle), StepStatus::Skipped, "{handle}");
    }
    assert_eq!(taken(&run, Phase::Cleanup), ["second", "first"]);
    // The check of `zeta` ran before the next step, `alpha` (RUN-03).
    let position = |handle: &str| run.steps.iter().position(|s| s.handle == handle).unwrap();
    assert!(position("zeta_ok") < position("alpha"));
}

/// The order recorded from `main`'s reading of every fixture in the
/// acceptance commit, by path.
fn recorded() -> BTreeMap<String, (String, String)> {
    include_str!("../../apiforge-script/tests/fixtures/lang13-step-order.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (path, order) = line.split_once(": ").expect("path: order");
            let (main, cleanup) = order.split_once(" |").expect("main | cleanup");
            (
                path.to_owned(),
                (main.trim().to_owned(), cleanup.trim().to_owned()),
            )
        })
        .collect()
}

#[tokio::test]
async fn lang13_criterion_11_fixtures_run_in_the_recorded_order() {
    let mut checked = 0;
    for (path, (main, cleanup)) in recorded() {
        let text = std::fs::read_to_string(repository().join(&path)).unwrap();
        if !text.starts_with("scenario") {
            continue;
        }
        let (project, scenario) = scenario_in(project(), &text);
        let run = simulate(project, &scenario).await;
        assert_eq!(taken(&run, Phase::Main).join(" "), main, "{path}");
        assert_eq!(taken(&run, Phase::Cleanup).join(" "), cleanup, "{path}");
        checked += 1;
    }
    assert!(checked >= 10, "every fixture scenario is run: {checked}");
}

// Criterion 14 ---------------------------------------------------------------

/// A scenario with a `start` whose checks and values read no step: a check
/// of a parameter, a value of nothing and a check written above the first
/// request.
const STANDALONE: &str = r#"scenario "Standalone checks" {
  param limit = 3
  early = assert param.limit == 3
  begin = start
  limit_ok = assert param.limit > 0
  constant = 42
  first = GET "/first" {
    sample 200 { id: 1 }
  }
}
"#;

#[tokio::test]
async fn lang13_criterion_14_checks_and_values_that_read_no_step_run() {
    let (project, scenario) = scenario_in(project(), STANDALONE);
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{:#?}", run.steps);
    for handle in ["early", "limit_ok", "constant", "first"] {
        assert_eq!(status(&run, handle), StepStatus::Passed, "{handle}");
    }
    let constant = run.steps.iter().find(|s| s.handle == "constant").unwrap();
    assert_eq!(constant.outputs["value"], serde_json::json!(42));
}

/// A component whose exports read its input and the environment, with and
/// without a `start` of its own, called by a scenario.
fn component(start: &str) -> String {
    format!(
        "component echo_id \"Echo id\" {{\n  input id: number = 7\n{start}  probe = GET \"/probe\" {{\n    sample 200 {{}}\n  }}\n  export given: number = param.id\n  export base: string = env.baseUrl\n}}\n"
    )
}

#[tokio::test]
async fn lang13_criterion_14_a_components_exports_of_its_input_and_env_have_their_value() {
    for start in ["", "  begin = start\n"] {
        let mut project = project();
        let lowered = lower(&parse(&component(start)), &LowerContext::folder(&project));
        assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
        let Some(Document::Component(mut echo)) = lowered.document else {
            panic!("a component");
        };
        echo.id = "echo-id".into();
        echo.revision = 1;
        project.components.push(echo);
        let caller = "scenario \"Calls\" {\n  begin = start\n  call = echo_id@1(id: 9)\n}\n";
        let (project, scenario) = scenario_in(project, caller);
        let base = project.environment.base_url.clone();
        let run = simulate(project, &scenario).await;
        assert_eq!(run.status, RunStatus::Passed, "{start:?}: {:#?}", run.steps);
        let call = run.steps.iter().find(|s| s.handle == "call").unwrap();
        assert_eq!(call.outputs["given"], serde_json::json!(9), "{start:?}");
        assert_eq!(call.outputs["base"], serde_json::json!(base), "{start:?}");
    }
}
