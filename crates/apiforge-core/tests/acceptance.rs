//! LAY-08, criterion 6: `sequence` replays the runner's order without running
//! anything. The runner's own loop is the reference, so this test sits outside
//! the unit tests of `graph`, where a dependency on the runner would see another
//! build of this crate.

use apiforge_core::{
    demos::shop_project,
    format::{Graph, Node, Project, Scenario},
    graph::{relations, sequence},
    run::{NotRunCause, Phase, RunMode, RunResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use std::collections::BTreeSet;
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

// LAY-22 (#312) ---------------------------------------------------------------

fn handle_of<'a>(graph: &'a Graph, id: &str) -> &'a str {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| panic!("no node `{id}`"), |node| node.handle.as_str())
}

fn node_by_handle<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no step `{handle}`"))
}

/// Criterion 4: a step's producers and readers with the field and the input
/// between them, its checks and its gates; a check's subjects and values.
#[test]
fn relations_of_cart_on_shop_30() {
    let (_, scenario) = fixture("shop-30");
    let graph = &scenario.graph;
    let handles = |ids: Vec<&str>| -> BTreeSet<String> {
        ids.into_iter()
            .map(|id| handle_of(graph, id).to_owned())
            .collect()
    };
    let set = |names: &[&str]| -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    };

    let cart = relations(graph, node_by_handle(graph, "cart"));
    assert_eq!(
        handles(cart.producers.iter().map(|feed| feed.step).collect()),
        set(&["login", "first", "second"])
    );
    let login = &node_by_handle(graph, "login").id;
    assert!(
        cart.producers.iter().any(|feed| feed.step == login
            && feed.output == "body"
            && feed.path == "token"
            && feed.input == "headers"),
        "{:#?}",
        cart.producers
    );
    let checks = handles(cart.checks.iter().map(|check| check.check).collect());
    assert!(checks.contains("cart_total"), "{checks:?}");
    assert_eq!(
        checks,
        set(&["cart_created", "cart_total", "receipt_total"])
    );
    let shared: Vec<_> = cart
        .checks
        .iter()
        .filter(|check| check.shared)
        .map(|check| handle_of(graph, check.check))
        .collect();
    assert_eq!(shared, ["receipt_total"]);
    let readers = handles(cart.readers.iter().map(|feed| feed.step).collect());
    assert!(readers.contains("order"), "{readers:?}");
    assert!(
        readers.iter().all(|reader| !checks.contains(reader)),
        "checks are listed apart: {readers:?}"
    );
    let order = &node_by_handle(graph, "order").id;
    assert!(
        cart.readers
            .iter()
            .any(|feed| feed.step == order && feed.output == "body" && feed.input == "query"),
        "{:#?}",
        cart.readers
    );
    assert!(cart.gates.is_empty() && cart.subjects.is_empty() && cart.values.is_empty());

    // A check: its subjects and its value sources.
    let cart_total = relations(graph, node_by_handle(graph, "cart_total"));
    assert_eq!(
        handles(
            cart_total
                .subjects
                .iter()
                .map(|subject| subject.step)
                .collect()
        ),
        set(&["cart"])
    );
    assert_eq!(handles(cart_total.values.clone()), set(&["expected_total"]));

    // A gate: the Data condition a step reads, and the step it checks.
    let checkout = relations(graph, node_by_handle(graph, "checkout"));
    assert_eq!(handles(checkout.gates.clone()), set(&["valid_cart"]));
    let cart_view = relations(graph, node_by_handle(graph, "cart_view"));
    assert!(
        cart_view
            .checks
            .iter()
            .any(|check| handle_of(graph, check.check) == "valid_cart" && check.gate),
        "{:#?}",
        cart_view.checks
    );
}

/// Criterion 5: a report written before causes existed loads, and writing it
/// again adds neither a cause nor the running step.
#[test]
fn a_report_without_causes_still_loads() {
    // A whole report from the run corpus, with a skipped step written the way
    // the runner wrote one before this change.
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/runs/single-with-history.json");
    let text = fs::read_to_string(&path).expect("the recorded run");
    let fixture: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let mut recorded = fixture["request"]["previousRun"].clone();
    recorded["steps"]
        .as_array_mut()
        .expect("steps")
        .push(serde_json::json!({
            "nodeId": "reads-failed", "handle": "reads_failed", "kind": "value",
            "status": "skipped", "phase": "main", "inputs": {}, "outputs": {},
            "durationMs": 0, "message": "Stopped after an error."
        }));
    assert!(!recorded.to_string().contains("\"cause\""));
    let report: RunResult = serde_json::from_value(recorded).expect("an earlier report loads");
    assert!(
        report
            .steps
            .iter()
            .any(|step| step.status == StepStatus::Skipped)
    );
    assert!(report.steps.iter().all(|step| step.cause.is_none()));
    assert_eq!(report.running, None);
    let written = serde_json::to_string(&report).expect("the report serialises");
    assert!(!written.contains("\"cause\"") && !written.contains("\"running\""));

    // A cause written now reads back as written.
    let mut later = report.clone();
    later.steps[0].cause = Some(NotRunCause::Stopped {
        by: "create".into(),
    });
    let reloaded: RunResult =
        serde_json::from_str(&serde_json::to_string(&later).expect("serialises")).expect("loads");
    assert_eq!(reloaded, later);
}
