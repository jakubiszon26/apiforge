//! LAY-08, criterion 6: `sequence` replays the runner's order without running
//! anything. The runner's own loop is the reference, so this test sits outside
//! the unit tests of `graph`, where a dependency on the runner would see another
//! build of this crate.

use apiforge_core::{
    demos::shop_project,
    format::{Graph, Project, Scenario},
    graph::sequence,
    run::{Phase, RunMode, RunResult},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use std::{fs, path::Path};
use tokio_util::sync::CancellationToken;

/// A reference scenario lowered in the demo shop project, whose components it
/// calls.
fn fixture(name: &str) -> (Project, Scenario) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()));
    let project = shop_project();
    let lowered = lower(&parse(&text), &LowerContext::folder(&project));
    assert!(!lowered.has_errors(), "{name}: {:#?}", lowered.diagnostics);
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("{name} is not a scenario");
    };
    (project, scenario)
}

async fn simulate(mut project: Project, scenario: &Scenario) -> RunResult {
    project.scenarios.push(scenario.clone());
    let request = RunRequest {
        project,
        scenario_id: scenario.id.clone(),
        target: apiforge_runner::RunTarget::Scenario,
        mode: RunMode::Simulation,
        target_node_id: None,
        single_node_id: None,
        previous_run: None,
        secrets: Default::default(),
    };
    let options = RunOptions::new(HttpTransport::new().expect("client builds"));
    run_scenario(request, options, CancellationToken::new(), |_| {})
        .await
        .expect("the request is valid")
}

fn executed(run: &RunResult, phase: Phase) -> Vec<&str> {
    run.steps
        .iter()
        .filter(|step| step.phase == phase)
        .map(|step| step.node_id.as_str())
        .collect()
}

fn reversed(graph: &mut Graph) {
    graph.nodes.reverse();
}

#[tokio::test]
async fn sequence_agrees_with_the_engine_on_the_fixtures() {
    for name in ["order-flow", "shop-30", "shop-50"] {
        let (project, mut scenario) = fixture(name);
        // The document as written, and turned around: a document in engine
        // order would pass with no scheduling at all.
        for turn in ["as written", "reversed"] {
            let run = simulate(project.clone(), &scenario).await;
            assert_eq!(
                sequence(&scenario.graph),
                executed(&run, Phase::Main),
                "{name}, main, {turn}"
            );
            assert_eq!(
                sequence(&scenario.cleanup),
                executed(&run, Phase::Cleanup),
                "{name}, cleanup, {turn}"
            );
            assert_eq!(
                sequence(&scenario.graph).len(),
                scenario.graph.nodes.len(),
                "{name}: every step has a place"
            );
            reversed(&mut scenario.graph);
            reversed(&mut scenario.cleanup);
        }
    }
}
