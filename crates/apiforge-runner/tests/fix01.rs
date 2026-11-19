//! FIX-01 (#454): a reference scenario shaped like a validation test.
//!
//! `fixtures/scenarios/lay/labels.afs` sends sixteen requests to one endpoint
//! of a fictional parcel API, each followed by one to four checks, and no
//! request reads another. It calls no component, so it is lowered in a
//! folder project of its own, with nothing else in it.

use apiforge_core::{
    demos::empty_project,
    format::{Node, Project, Scenario},
    model::NodeKind,
    run::{Phase, RunMode, RunResult, RunStatus, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, format, lower, parse};
use std::{collections::BTreeMap, fs, path::Path};
use tokio_util::sync::CancellationToken;

fn source() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/labels.afs");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// The scenario lowered in an empty folder project, which it joins; no
/// diagnostic at all, not even a warning.
fn lowered() -> (Project, Scenario) {
    let mut project = empty_project();
    project.scenarios.clear();
    let result = lower(&parse(&source()), &LowerContext::folder(&project));
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let Some(Document::Scenario(scenario)) = result.document else {
        panic!("labels is not a scenario");
    };
    project.scenarios.push(scenario.clone());
    (project, scenario)
}

fn kinds<'a>(nodes: impl Iterator<Item = &'a NodeKind>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for kind in nodes {
        *counts.entry(format!("{kind:?}")).or_default() += 1;
    }
    counts
}

fn expected_kinds() -> BTreeMap<String, usize> {
    BTreeMap::from([
        ("Start".to_owned(), 1),
        ("Request".to_owned(), 16),
        ("Assert".to_owned(), 33),
    ])
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

/// Criterion 1: the fixture is in canonical form, as `apiforge fmt --check`
/// asks.
#[test]
fn fix01_criterion_1_labels_is_in_canonical_form() {
    let text = source();
    assert_eq!(format(&text).as_ref(), Ok(&text));
}

/// Criterion 2: it checks without a problem in a folder project and has 50
/// steps, 1 start, 16 requests and 33 checks, with no cleanup; no request
/// reads another.
#[test]
fn fix01_criterion_2_labels_checks_with_fifty_steps() {
    let (_, scenario) = lowered();
    assert_eq!(scenario.name, "Label validation");
    assert_eq!(scenario.graph.nodes.len(), 50);
    assert_eq!(
        kinds(scenario.graph.nodes.iter().map(|node| &node.kind)),
        expected_kinds()
    );
    assert!(scenario.cleanup.nodes.is_empty(), "no cleanup");
    let requests: Vec<&Node> = scenario
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Request)
        .collect();
    for request in &requests {
        for reference in request
            .inputs
            .values()
            .flat_map(|binding| binding.references())
        {
            assert!(
                requests.iter().all(|other| other.id != reference.node_id),
                "{} reads another request",
                request.handle
            );
        }
    }
}

/// Criterion 2: a simulated run passes with all 50 steps run and passed.
#[tokio::test]
async fn fix01_criterion_2_a_simulated_run_passes_with_fifty_steps() {
    let (project, scenario) = lowered();
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{:#?}", run.error);
    assert_eq!(run.steps.len(), 50);
    assert!(run.steps.iter().all(|step| step.phase == Phase::Main));
    assert_eq!(
        kinds(run.steps.iter().map(|step| &step.kind)),
        expected_kinds()
    );
    for step in &run.steps {
        assert_eq!(
            step.status,
            StepStatus::Passed,
            "{}: {:?}",
            step.handle,
            step.message
        );
    }
}
