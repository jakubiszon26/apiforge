//! DIAG-06d (#493), criterion 4: what a move across braces does to a run is
//! what the moved text says, on both outcomes of the condition. A producer
//! moved into *Yes* while its reader stays after the `if`: the reader runs
//! when *Yes* is taken and is blocked, naming the producer, when *No* is
//! (RUN-01). A reader left outside a moved `if` likewise. The moves are
//! made by the board's operation (`edit::place`) and the scenarios are
//! simulated; nothing is sent.

use apiforge_core::{
    demos::shop_project,
    edit::{Place, place},
    format::{Graph, Project, Scenario},
    model::Branch,
    run::{NotRunCause, RunMode, RunResult, StepResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use std::time::Duration;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

fn lowered(project: &Project, text: &str) -> Scenario {
    let result = lower(&parse(text), &LowerContext::folder(project));
    assert!(!result.has_errors(), "{:#?}", result.diagnostics);
    match result.document.expect("a document") {
        Document::Scenario(scenario) => scenario,
        Document::Component(_) => panic!("not a scenario"),
    }
}

fn id(graph: &Graph, handle: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a statement `{handle}`"))
        .id
        .clone()
}

/// `text` with `step` put at the place `at` names, as the board moves it.
fn moved(
    text: &str,
    step: &str,
    at: impl FnOnce(&Graph) -> (String, Option<Branch>),
) -> (Project, Scenario) {
    let mut project = shop_project();
    let mut scenario = lowered(&project, text);
    let (anchor, branch) = at(&scenario.graph);
    let place_at = match branch {
        Some(branch) => Place::First(&anchor, branch),
        None => Place::After(&anchor),
    };
    let (graph, _) = place(
        &scenario.graph,
        &project,
        &id(&scenario.graph, step),
        place_at,
    )
    .expect("the move is allowed");
    scenario.graph = graph;
    project.scenarios.push(scenario.clone());
    (project, scenario)
}

async fn simulated(project: Project, scenario: &Scenario) -> RunResult {
    timeout(
        Duration::from_secs(20),
        run_scenario(
            RunRequest::new(project, scenario.id.clone(), RunMode::Simulation),
            RunOptions::new(HttpTransport::new().expect("client builds")),
            CancellationToken::new(),
            |_| {},
        ),
    )
    .await
    .expect("the run ends")
    .expect("the run starts")
}

fn step<'a>(steps: &'a [StepResult], handle: &str) -> &'a StepResult {
    steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no result for `{handle}`: {steps:#?}"))
}

fn assert_passed(run: &RunResult, handle: &str, outcome: &str) {
    let found = step(&run.steps, handle);
    assert_eq!(
        found.status,
        StepStatus::Passed,
        "{outcome}: `{handle}` runs: {found:#?}"
    );
}

fn assert_blocked_by(run: &RunResult, handle: &str, by: &str, outcome: &str) {
    let found = step(&run.steps, handle);
    assert_eq!(
        found.status,
        StepStatus::Blocked,
        "{outcome}: `{handle}`: {found:#?}"
    );
    assert_eq!(
        found.cause,
        Some(NotRunCause::BlockedBy {
            step: step(&run.steps, by).node_id.clone()
        }),
        "{outcome}: `{handle}` names `{by}`"
    );
}

/// The two outcomes of the condition `go`: *Yes* taken, then *No*.
const OUTCOMES: [(&str, &str); 2] = [("yes", "200"), ("no", "500")];

/// `p` is moved from before the `if` into its *Yes*; `r` after the `if`
/// reads it.
fn producer(status: &str) -> String {
    format!(
        r#"scenario "Producer" {{
  on_error continue
  begin = start
  a = GET "/a" {{
    sample {status} {{}}
  }}
  p = GET "/p" {{
    sample 200 {{ id: 1 }}
  }}
  go = if a.status == 200 {{
    inside = GET "/inside" {{
      sample 200 {{}}
    }}
  }}
  r = GET "/r/{{{{p.body.id}}}}" {{
    sample 200 {{}}
  }}
}}
"#
    )
}

#[tokio::test]
async fn diag06d_criterion_4_a_producer_moved_into_yes() {
    for (outcome, status) in OUTCOMES {
        let (project, scenario) = moved(&producer(status), "p", |graph| {
            (id(graph, "go"), Some(Branch::True))
        });
        let run = simulated(project, &scenario).await;
        if outcome == "yes" {
            assert_passed(&run, "p", outcome);
            assert_passed(&run, "r", outcome);
        } else {
            assert_eq!(
                step(&run.steps, "p").status,
                StepStatus::Skipped,
                "{outcome}: `p` is on the branch not taken"
            );
            assert_blocked_by(&run, "r", "p", outcome);
        }
    }
}

/// `go` with `p` in its braces is moved behind `r`, which reads `p` and
/// stays outside.
fn reader(status: &str) -> String {
    format!(
        r#"scenario "Reader" {{
  on_error continue
  begin = start
  a = GET "/a" {{
    sample {status} {{}}
  }}
  go = if a.status == 200 {{
    p = GET "/p" {{
      sample 200 {{ id: 1 }}
    }}
  }}
  b = GET "/b" {{
    sample 200 {{}}
  }}
  r = GET "/r/{{{{p.body.id}}}}" {{
    sample 200 {{}}
  }}
}}
"#
    )
}

#[tokio::test]
async fn diag06d_criterion_4_a_reader_outside_a_moved_if() {
    for (outcome, status) in OUTCOMES {
        let (project, scenario) = moved(&reader(status), "go", |graph| (id(graph, "b"), None));
        let run = simulated(project, &scenario).await;
        if outcome == "yes" {
            assert_passed(&run, "p", outcome);
            assert_passed(&run, "r", outcome);
        } else {
            assert_blocked_by(&run, "r", "p", outcome);
        }
    }
}
