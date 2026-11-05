//! LAY-01, criterion 5: the reference scenarios of the LAY sprint.
//!
//! `fixtures/scenarios/lay/` is what the later LAY tickets test against, so these
//! tests hold what those tickets read: the steps they name, the one shared check,
//! the one used gate. The scenarios call components of the demo shop project and
//! are lowered and run in it, as a person opens them there.
//!
//! **A step** is a node of the scenario's main graph: the start, every request,
//! value, component, check and condition, a branch and both of its arms. The
//! `cleanup` block is a second graph, run in its own phase, and is counted apart.
//! This is the count that gives `order-flow` its 14 steps and the round-3
//! `shop-30` its 34.

use apiforge_core::{
    demos::shop_project,
    format::{Node, Project, Scenario},
    graph::{WireKind, wires},
    model::{Branch, NodeKind},
    run::{RunMode, RunResult, RunStatus, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, format, lower, parse};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};
use tokio_util::sync::CancellationToken;

const FIXTURES: [&str; 3] = ["order-flow", "shop-30", "shop-50"];

fn source(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// The scenario lowered in the demo shop project, which it joins.
fn lowered(text: &str) -> (Project, Scenario) {
    let mut project = shop_project();
    let tree = parse(text);
    let result = lower(&tree, &LowerContext::folder(&project));
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let Some(Document::Scenario(scenario)) = result.document else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    (project, scenario)
}

fn fixture(name: &str) -> (Project, Scenario) {
    lowered(&source(name))
}

fn node<'a>(scenario: &'a Scenario, handle: &str) -> &'a Node {
    scenario
        .graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no step `{handle}`"))
}

/// The steps whose outputs `step` reads directly, by handle.
fn read_by<'a>(scenario: &'a Scenario, step: &Node) -> BTreeSet<&'a str> {
    step.inputs
        .values()
        .flat_map(|binding| binding.references())
        .filter_map(|reference| {
            scenario
                .graph
                .nodes
                .iter()
                .find(|node| node.id == reference.node_id)
        })
        .map(|node| node.handle.as_str())
        .collect()
}

/// `read_by`, keeping the requests.
fn requests_read_by<'a>(scenario: &'a Scenario, step: &Node) -> BTreeSet<&'a str> {
    read_by(scenario, step)
        .into_iter()
        .filter(|handle| node(scenario, handle).kind == NodeKind::Request)
        .collect()
}

fn checks(scenario: &Scenario) -> impl Iterator<Item = &Node> {
    scenario
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Assert)
}

/// The checks of a request: those that read it and no other request.
fn checks_of<'a>(scenario: &'a Scenario, request: &str) -> Vec<&'a str> {
    checks(scenario)
        .filter(|check| requests_read_by(scenario, check) == BTreeSet::from([request]))
        .map(|check| check.handle.as_str())
        .collect()
}

async fn simulate(project: Project, scenario: &Scenario) -> RunResult {
    let transport = HttpTransport::new().expect("client builds");
    run_scenario(
        RunRequest::new(project, scenario.id.clone(), RunMode::Simulation),
        RunOptions::new(transport),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("the run starts")
}

/// The report without what differs between any two runs.
fn normalised(run: &RunResult) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                for name in ["id", "startedAt", "finishedAt", "durationMs"] {
                    fields.remove(name);
                }
                fields.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(run).expect("a report serialises");
    strip(&mut value);
    value
}

#[test]
fn lay01_criterion_5_the_fixtures_parse_in_canonical_form() {
    for name in FIXTURES {
        let text = source(name);
        let (_, scenario) = lowered(&text);
        assert!(!scenario.name.is_empty(), "{name}");
        assert_eq!(format(&text).as_ref(), Ok(&text), "{name}");
    }
}

#[test]
fn lay01_criterion_5_the_step_counts() {
    let count = |name| {
        let (_, scenario) = fixture(name);
        (scenario.graph.nodes.len(), scenario.cleanup.nodes.len())
    };
    assert_eq!(count("order-flow"), (14, 2));
    assert_eq!(count("shop-30"), (30, 2));
    assert_eq!(count("shop-50"), (56, 2));
}

#[test]
fn lay01_criterion_5_shop_30_keeps_the_steps_the_later_tickets_name() {
    let (_, scenario) = fixture("shop-30");
    for handle in [
        "login",
        "catalogue",
        "first",
        "second",
        "cart",
        "cart_total",
        "cart_view",
        "valid_cart",
        "address",
        "checkout",
        "order",
        "paid",
        "payment",
        "receipt",
        "refund",
        "logout",
    ] {
        node(&scenario, handle);
    }
    // LAY-09 criterion 3 reads `2 checks` on `login`; LAY-18 criterion 1 reads
    // `status → 1 check` there.
    assert_eq!(checks_of(&scenario, "login").len(), 2);
    assert_eq!(
        checks(&scenario)
            .filter(|check| {
                check.inputs.values().any(|binding| {
                    binding
                        .references()
                        .iter()
                        .any(|reference| reference.node_id == node(&scenario, "login").id)
                        && serde_json::to_string(binding)
                            .expect("a binding serialises")
                            .contains("status")
                })
            })
            .count(),
        1
    );
    // LAY-09 criterion 4, LAY-11 and the UX task sheet: `checkout` has three checks.
    assert_eq!(checks_of(&scenario, "checkout").len(), 3);
    // LAY-10 criterion 1 rings these around `cart`.
    let cart = node(&scenario, "cart");
    for producer in ["login", "first", "second"] {
        assert!(read_by(&scenario, cart).contains(producer), "{producer}");
    }
    assert!(read_by(&scenario, node(&scenario, "cart_total")).contains("cart"));
    assert!(read_by(&scenario, node(&scenario, "order")).contains("cart"));
    // Task 3 of the sheet asks where `receipt` gets `order.body.id`; LAY-14
    // criterion 2 needs a row fed by two sources, which is that same URL.
    let receipt = read_by(&scenario, node(&scenario, "receipt"));
    assert!(receipt.contains("order") && receipt.contains("payment"));
    // Task 7 of the sheet: a field of `login` feeds `payment`.
    assert!(read_by(&scenario, node(&scenario, "payment")).contains("login"));
    // LAY-18 criterion 2: a request nothing reads.
    let logout = &node(&scenario, "logout").id;
    assert!(
        wires(&scenario.graph)
            .iter()
            .all(|wire| wire.kind != WireKind::Data || &wire.source != logout)
    );
}

#[test]
fn lay01_criterion_5_shop_30_has_one_shared_check() {
    let (_, scenario) = fixture("shop-30");
    let shared: Vec<_> = checks(&scenario)
        .filter(|check| requests_read_by(&scenario, check).len() > 1)
        .collect();
    assert_eq!(
        shared
            .iter()
            .map(|check| check.handle.as_str())
            .collect::<Vec<_>>(),
        ["receipt_total"]
    );
    assert_eq!(
        requests_read_by(&scenario, shared[0]),
        BTreeSet::from(["cart", "receipt"])
    );
    // LAY-08 criterion 1: a check that reads a request and a value names both.
    assert_eq!(
        read_by(&scenario, node(&scenario, "cart_total")),
        BTreeSet::from(["cart", "expected_total"])
    );
    // LAY-08 criterion 4: a standalone check reads no step.
    assert!(
        checks(&scenario).any(|check| read_by(&scenario, check).is_empty()),
        "no standalone check"
    );
}

#[test]
fn lay01_criterion_5_shop_30_has_one_used_gate() {
    let (_, scenario) = fixture("shop-30");
    let conditions: Vec<_> = scenario
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Condition)
        .collect();
    assert_eq!(conditions.len(), 1);
    assert_eq!(conditions[0].handle, "valid_cart");
    let readers: BTreeSet<&str> = scenario
        .graph
        .nodes
        .iter()
        .filter(|node| read_by(&scenario, node).contains("valid_cart"))
        .map(|node| node.handle.as_str())
        .collect();
    // `checkout` reads it here and LAY-11 criterion 7 names `order`.
    assert_eq!(readers, BTreeSet::from(["checkout", "order"]));
}

#[test]
fn lay01_criterion_5_shop_30_keeps_its_shape() {
    let (_, scenario) = fixture("shop-30");
    let graph = &scenario.graph;
    // LAY-16 criterion 3: a query that is configured and has no wire.
    let catalogue = node(&scenario, "catalogue");
    assert!(
        serde_json::to_string(&catalogue.inputs)
            .expect("inputs serialise")
            .contains("limit")
    );
    assert!(wires(graph).iter().all(|wire| {
        wire.target != catalogue.id || wire.target_input.as_deref() != Some("query")
    }));
    // `@after` on a check, which LAY-11 criterion 7 draws from its subject.
    let (cart_total, address) = (node(&scenario, "cart_total"), node(&scenario, "address"));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == cart_total.id && edge.target == address.id && edge.branch.is_none()
    }));
    // The branch, with `receipt` inside it and an arm that never runs.
    let paid = node(&scenario, "paid");
    assert_eq!(paid.kind, NodeKind::If);
    let arm = |handle: &str, branch| {
        let target = &node(&scenario, handle).id;
        graph.edges.iter().any(|edge| {
            edge.source == paid.id && &edge.target == target && edge.branch == Some(branch)
        })
    };
    assert!(arm("payment", Branch::True));
    assert!(read_by(&scenario, node(&scenario, "receipt")).contains("payment"));
    assert!(arm("refund", Branch::True), "@after(paid.then)");
    assert!(arm("not_created", Branch::False));
    // The cleanup.
    let handles: Vec<_> = scenario
        .cleanup
        .nodes
        .iter()
        .map(|node| node.handle.as_str())
        .collect();
    assert_eq!(handles, ["removed", "gone"]);
}

#[tokio::test]
async fn lay01_criterion_5_the_fixtures_simulate_deterministically() {
    for name in FIXTURES {
        let (project, scenario) = fixture(name);
        let first = simulate(project.clone(), &scenario).await;
        let second = simulate(project, &scenario).await;
        assert_eq!(first.status, RunStatus::Passed, "{name}: {first:#?}");
        assert_eq!(
            first.steps.len(),
            scenario.graph.nodes.len() + scenario.cleanup.nodes.len(),
            "{name}: a row for every step"
        );
        assert_eq!(normalised(&first), normalised(&second), "{name}");
    }
}

/// The failed simulation of the UX task sheet (lay.md section 9), with the sample
/// total of `checkout` broken: one of its three checks fails and, as the error
/// policy is *Stop*, the run ends there.
#[tokio::test]
async fn lay01_criterion_5_a_broken_sample_fails_one_check_of_checkout() {
    let text = source("shop-30").replace(
        r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#,
        r#"sample 201 { orderId: "o-1", status: "created", total: 170 }"#,
    );
    let (project, scenario) = lowered(&text);
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed);
    let failed: Vec<_> = run
        .steps
        .iter()
        .filter(|step| step.status == StepStatus::Failed)
        .map(|step| step.handle.as_str())
        .collect();
    assert_eq!(failed, ["checkout_total"]);
    assert_eq!(checks_of(&scenario, "checkout").len(), 3);
}
