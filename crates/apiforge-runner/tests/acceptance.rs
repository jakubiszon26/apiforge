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
    run::{NotRunCause, RunMode, RunResult, RunStatus, StepResult, StepStatus},
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
    let login = &node(&scenario, "login").id;
    let reads_status = |check: &&Node| {
        check
            .inputs
            .values()
            .flat_map(|binding| binding.references())
            .any(|reference| reference.node_id == login && reference.port == "status")
    };
    assert_eq!(checks(&scenario).filter(reads_status).count(), 1);
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
    let (project, scenario) = lowered(&broken_checkout());
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

// LAY-22 (#312) ---------------------------------------------------------------

/// `shop-30` with the sample total of `checkout` broken, as the task sheet uses.
fn broken_checkout() -> String {
    source("shop-30").replace(
        r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#,
        r#"sample 201 { orderId: "o-1", status: "created", total: 170 }"#,
    )
}

fn by_handle<'a>(run: &'a RunResult, handle: &str) -> &'a StepResult {
    run.steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no result for `{handle}`: {run:#?}"))
}

/// The messages of a step that did not run; they stay as they were.
const NOT_RUN_MESSAGES: [&str; 4] = [
    "Step outside the active execution path.",
    "Unavailable data or an unmet condition of an earlier step.",
    "Stopped after an error.",
    "Cancelled.",
];

/// Every result built for a step that did not run carries one cause, and every
/// step that ran carries none.
fn assert_causes_only_on_steps_that_did_not_run(run: &RunResult) {
    for step in &run.steps {
        let not_run = step
            .message
            .as_deref()
            .is_some_and(|message| NOT_RUN_MESSAGES.contains(&message));
        assert_eq!(
            step.cause.is_some(),
            not_run,
            "`{}`: {:?} {:?}",
            step.handle,
            step.message,
            step.cause
        );
    }
}

/// Criterion 1: the observer learns which step started before its result, only
/// for a step that executes, and the result clears it.
#[tokio::test]
async fn a_step_is_announced_before_its_result() {
    for text in [source("shop-30"), broken_checkout()] {
        let (project, scenario) = lowered(&text);
        let mut updates: Vec<(Option<String>, Vec<String>)> = Vec::new();
        let run = run_scenario(
            RunRequest::new(project, scenario.id.clone(), RunMode::Simulation),
            RunOptions::new(HttpTransport::new().expect("client builds")),
            CancellationToken::new(),
            |run: &RunResult| {
                let done = run.steps.iter().map(|step| step.node_id.clone()).collect();
                updates.push((run.running.clone(), done));
            },
        )
        .await
        .expect("the run starts");
        assert_eq!(run.running, None, "a finished run names no running step");
        assert_eq!(updates.last().map(|update| update.0.clone()), Some(None));
        let mut announced = Vec::new();
        for (index, (running, done)) in updates.iter().enumerate() {
            let Some(id) = running else { continue };
            assert!(!done.contains(id), "`{id}` is announced before its result");
            let (next_running, next_done) = &updates[index + 1];
            assert_eq!(next_running, &None, "the result of `{id}` clears it");
            assert_eq!(next_done.last(), Some(id), "the result that follows");
            assert_eq!(next_done.len(), done.len() + 1);
            announced.push(id.clone());
        }
        let executed: Vec<String> = run
            .steps
            .iter()
            .filter(|step| step.cause.is_none())
            .map(|step| step.node_id.clone())
            .collect();
        assert_eq!(announced, executed, "only the steps that execute");
        assert!(
            serde_json::to_string(&run)
                .expect("a report serialises")
                .find("running")
                .is_none(),
            "never in a report"
        );
    }
}

/// Criterion 2: one cause per branch of the engine that builds a result for a
/// step that does not run; the messages stay.
#[tokio::test]
async fn not_run_causes_are_an_enum_with_one_variant_per_branch() {
    let id = |run: &RunResult, handle: &str| by_handle(run, handle).node_id.clone();

    // Stopped, and a branch not taken.
    let (project, scenario) = lowered(
        r#"scenario "Causes stop" {
  begin = start
  @after(begin)
  first = GET "/a" {
    sample 200 { ok: true }
  }
  broken = assert first.body.ok == false
  later = GET "/b" {
    query { ok: first.body.ok }
    sample 200 {}
  }
}
"#,
    );
    let run = simulate(project, &scenario).await;
    assert_causes_only_on_steps_that_did_not_run(&run);
    assert_eq!(
        by_handle(&run, "later").cause,
        Some(NotRunCause::Stopped {
            by: id(&run, "broken")
        })
    );
    assert_eq!(
        by_handle(&run, "later").message.as_deref(),
        Some("Stopped after an error.")
    );

    let (project, scenario) = lowered(
        r#"scenario "Causes branch" {
  begin = start
  @after(begin)
  first = GET "/a" {
    sample 200 { ok: true }
  }
  chosen = if first.status == 200 {
    yes = GET "/yes" {
      sample 200 {}
    }
  } else {
    no = GET "/no" {
      sample 200 {}
    }
  }
}
"#,
    );
    let run = simulate(project.clone(), &scenario).await;
    assert_causes_only_on_steps_that_did_not_run(&run);
    assert_eq!(by_handle(&run, "yes").cause, None);
    assert_eq!(by_handle(&run, "no").cause, Some(NotRunCause::OutsidePath));

    // Cancelled before anything ran.
    let cancel = CancellationToken::new();
    cancel.cancel();
    let run = run_scenario(
        RunRequest::new(project, scenario.id.clone(), RunMode::Simulation),
        RunOptions::new(HttpTransport::new().expect("client builds")),
        cancel,
        |_| {},
    )
    .await
    .expect("the run starts");
    assert_causes_only_on_steps_that_did_not_run(&run);
    for handle in ["begin", "first", "chosen"] {
        assert_eq!(
            by_handle(&run, handle).cause,
            Some(NotRunCause::Cancelled),
            "{handle}"
        );
    }

    // A failed source, a false Data condition and a Cleanup input that is missing.
    let (project, scenario) = lowered(
        r#"scenario "Causes continue" {
  on_error continue
  begin = start
  @after(begin)
  missing = GET "/m" {
  }
  reader = GET "/r/{{missing.body.id}}" {
    sample 200 {}
  }
  @after(begin)
  first = GET "/a" {
    sample 200 { id: 1 }
  }
  valid = condition all {
    first.status == 500,
  } with first.body
  guarded = GET "/g/{{valid.value.id}}" {
    sample 200 {}
  }
  cleanup {
    removed = DELETE "/a/{{first.body.nothing}}" {
      sample 204 {}
    }
  }
}
"#,
    );
    let run = simulate(project, &scenario).await;
    assert_causes_only_on_steps_that_did_not_run(&run);
    assert_eq!(by_handle(&run, "missing").status, StepStatus::Error);
    assert_eq!(
        by_handle(&run, "reader").cause,
        Some(NotRunCause::BlockedBy {
            step: id(&run, "missing")
        })
    );
    assert_eq!(
        by_handle(&run, "valid").status,
        StepStatus::Blocked,
        "{run:#?}"
    );
    assert_eq!(by_handle(&run, "valid").cause, None, "the condition ran");
    assert_eq!(
        by_handle(&run, "guarded").cause,
        Some(NotRunCause::ConditionUnmet {
            condition: id(&run, "valid")
        })
    );
    assert_eq!(
        by_handle(&run, "removed").cause,
        Some(NotRunCause::CleanupInputMissing {
            step: id(&run, "first")
        })
    );
}

/// The step a not-run step's cause chain ends at, or the cause that ends it
/// without a step.
fn chain_end<'a>(run: &'a RunResult, step: &'a StepResult) -> Result<&'a str, NotRunCause> {
    let mut current = step;
    for _ in 0..=run.steps.len() {
        let Some(cause) = &current.cause else {
            return Ok(&current.handle);
        };
        let Some(next) = cause.step() else {
            return Err(cause.clone());
        };
        current = run
            .steps
            .iter()
            .find(|step| step.node_id == next)
            .unwrap_or_else(|| panic!("the cause of `{}` names `{next}`", current.handle));
    }
    panic!("the cause chain of `{}` loops", step.handle);
}

/// Criterion 3: after the failed simulation of `shop-30`, every not-run step's
/// cause chain reaches `checkout_total`; the arm of the branch that is not taken
/// is `OutsidePath` in a passing run and in a run that fails after the branch.
#[tokio::test]
async fn causes_on_shop_30() {
    let (project, scenario) = lowered(&broken_checkout());
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed);
    assert_causes_only_on_steps_that_did_not_run(&run);
    let not_run: Vec<_> = run
        .steps
        .iter()
        .filter(|step| step.cause.is_some())
        .collect();
    // Everything after `checkout_total` in the main phase; Cleanup runs.
    assert_eq!(not_run.len(), 11, "{not_run:#?}");
    for step in not_run {
        assert_eq!(
            chain_end(&run, step),
            Ok("checkout_total"),
            "`{}`: {:?}",
            step.handle,
            step.cause
        );
    }

    let (project, scenario) = fixture("shop-30");
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed);
    assert_causes_only_on_steps_that_did_not_run(&run);
    let not_run: Vec<_> = run
        .steps
        .iter()
        .filter(|step| step.cause.is_some())
        .map(|step| (step.handle.as_str(), step.cause.clone()))
        .collect();
    assert_eq!(not_run, [("not_created", Some(NotRunCause::OutsidePath))]);

    // A run that fails after the branch has been decided: the refund's amount.
    let text = source("shop-30").replace(
        "sample 200 { refunded: true, amount: 172.5 }",
        "sample 200 { refunded: true, amount: 170 }",
    );
    let (project, scenario) = lowered(&text);
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Failed);
    assert_causes_only_on_steps_that_did_not_run(&run);
    assert_eq!(
        by_handle(&run, "not_created").cause,
        Some(NotRunCause::OutsidePath)
    );
    assert_eq!(
        chain_end(&run, by_handle(&run, "logout")),
        Ok("refund_amount")
    );
}
