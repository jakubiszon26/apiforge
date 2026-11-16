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
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, SeededRandom, run_scenario};
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

// LANG-11 (#357) --------------------------------------------------------------

/// A `then` branch of three requests in a chain, and `later`, written before
/// the `if`, ordered after the branch's last statement and the other branch.
fn after_the_branch(taken: bool) -> String {
    format!(
        r#"scenario "After the branch" {{
  param taken = {taken}
  @after(third, choice.else)
  later = GET "/later" {{
    sample 200 {{}}
  }}
  choice = if param.taken == true {{
    made = POST "/made" {{
      sample 201 {{}}
    }}
    @after(made)
    second = GET "/second" {{
      sample 200 {{}}
    }}
    @after(second)
    third = GET "/third" {{
      sample 200 {{}}
    }}
  }}
}}
"#
    )
}

fn position(run: &RunResult, handle: &str) -> usize {
    run.steps
        .iter()
        .position(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no result for `{handle}`: {run:#?}"))
}

/// Criterion 1: `@after(last_in_then, x.else)` waits for the statements of the
/// `then` branch when it is taken, though the line comes first in the file.
#[tokio::test]
async fn lang11_criterion_1_after_the_last_statement_waits_for_the_branch() {
    let (project, scenario) = lowered(&after_the_branch(true));
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    let order = ["choice", "made", "second", "third", "later"].map(|handle| position(&run, handle));
    assert!(order.is_sorted(), "{order:?}: {run:#?}");
    for handle in ["made", "second", "third", "later"] {
        assert_eq!(
            by_handle(&run, handle).status,
            StepStatus::Passed,
            "{handle}"
        );
    }
}

/// Criterion 1: with the `else` branch taken, the statements of `then` are
/// skipped and `later` still runs, through `choice.else`.
#[tokio::test]
async fn lang11_criterion_1_after_the_other_branch_runs_when_it_is_taken() {
    let (project, scenario) = lowered(&after_the_branch(false));
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    for handle in ["made", "second", "third"] {
        assert_eq!(
            by_handle(&run, handle).status,
            StepStatus::Skipped,
            "{handle}"
        );
    }
    assert_eq!(by_handle(&run, "later").status, StepStatus::Passed);
    assert!(position(&run, "choice") < position(&run, "later"));
}

// LANG-10 (#354) --------------------------------------------------------------

/// A value read by two requests, and a call in each request's body.
const SENT_IDS: &str = r#"scenario "Sent ids" {
  order = uuid()
  @after(order)
  first = POST "/orders" {
    body { orderId: order, lineId: uuid() }
    sample 201 {}
  }
  @after(first)
  second = POST "/orders" {
    body { orderId: order, lineId: uuid() }
    sample 201 {}
  }
}
"#;

/// A component whose export is a new id, called between two values.
const TAGGER: &str = r#"component Tagger "Tagger" {
  export tag: string = uuid()
}
"#;

const NESTED_IDS: &str = r#"scenario "Nested ids" {
  before = uuid()
  @after(before)
  tagged = Tagger@1()
  @after(tagged)
  after = uuid()
}
"#;

/// Two values that are never sent, for a live run with no API.
const LIVE_IDS: &str = r#"scenario "Live ids" {
  one = uuid()
  @after(one)
  two = uuid()
}
"#;

/// The `n`-th value `uuid()` gives in a simulation.
fn placeholder(n: u64) -> Value {
    Value::String(format!("00000000-0000-4000-8000-{n:012}"))
}

/// `xxxxxxxx-xxxx-4xxx-[89ab]xxx-xxxxxxxxxxxx` in lowercase hexadecimal.
fn is_uuid_v4(value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
        })
        && bytes[14] == b'4'
        && b"89ab".contains(&bytes[19])
}

/// The body a step's request carried, as the report shows it.
fn sent_body(run: &RunResult, handle: &str) -> Value {
    let step = serde_json::to_value(by_handle(run, handle)).expect("a step serialises");
    step["request"]["body"].clone()
}

/// The single output of a value step, or the named output of another step.
fn output(run: &RunResult, handle: &str, port: Option<&str>) -> Value {
    let outputs = &by_handle(run, handle).outputs;
    match port {
        Some(port) => outputs[port].clone(),
        None => outputs.values().next().cloned().expect("an output"),
    }
}

/// `NESTED_IDS` in the demo shop project, with `Tagger` published as revision 1.
fn nested_ids() -> (Project, Scenario) {
    let mut project = shop_project();
    let result = lower(&parse(TAGGER), &LowerContext::folder(&project));
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let Some(Document::Component(mut tagger)) = result.document else {
        panic!("not a component");
    };
    tagger.id = "tagger".into();
    tagger.revision = 1;
    project.components.push(tagger);
    let result = lower(&parse(NESTED_IDS), &LowerContext::folder(&project));
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let Some(Document::Scenario(scenario)) = result.document else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    (project, scenario)
}

async fn live(project: Project, scenario: &Scenario, options: RunOptions) -> RunResult {
    run_scenario(
        RunRequest::new(project, scenario.id.clone(), RunMode::Live),
        options,
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("the run starts")
}

/// Criterion 3: a value block read by two requests sends the same id twice;
/// a call in each request sends a different one.
#[tokio::test]
async fn lang10_criterion_3_a_value_read_twice_sends_one_id_and_two_calls_two() {
    let (project, scenario) = lowered(SENT_IDS);
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    let (first, second) = (sent_body(&run, "first"), sent_body(&run, "second"));
    assert_eq!(first["orderId"], second["orderId"], "{first} {second}");
    assert_ne!(first["lineId"], second["lineId"], "{first} {second}");
    assert_eq!(output(&run, "order", None), first["orderId"]);
}

/// Criterion 4 (owner decision 1 A): a simulation gives the numbered
/// placeholders in evaluation order, the same in every simulation.
#[tokio::test]
async fn lang10_criterion_4_a_simulation_numbers_the_ids_in_order() {
    let (project, scenario) = lowered(SENT_IDS);
    let run = simulate(project.clone(), &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(output(&run, "order", None), placeholder(1));
    assert_eq!(sent_body(&run, "first")["lineId"], placeholder(2));
    assert_eq!(sent_body(&run, "second")["lineId"], placeholder(3));
    for n in 1..=3 {
        assert!(is_uuid_v4(&placeholder(n)), "{}", placeholder(n));
    }
    let again = simulate(project, &scenario).await;
    assert_eq!(normalised(&again), normalised(&run));
}

/// Criterion 4: a component called in the run shares the count.
#[tokio::test]
async fn lang10_criterion_4_a_nested_component_call_shares_the_count() {
    let (project, scenario) = nested_ids();
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(output(&run, "before", None), placeholder(1));
    assert_eq!(output(&run, "tagged", Some("tag")), placeholder(2));
    assert_eq!(output(&run, "after", None), placeholder(3));
}

/// Criterion 4: a live run gives random version 4 ids, not the placeholders;
/// with a fixed seed the same ones.
#[tokio::test]
async fn lang10_criterion_4_a_live_run_gives_random_ids() {
    let (project, scenario) = lowered(LIVE_IDS);
    let transport = || HttpTransport::new().expect("client builds");
    let run = live(project.clone(), &scenario, RunOptions::new(transport())).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    let (one, two) = (output(&run, "one", None), output(&run, "two", None));
    assert!(is_uuid_v4(&one) && is_uuid_v4(&two), "{one} {two}");
    assert_ne!(one, two);
    for value in [&one, &two] {
        assert!(
            !value.as_str().unwrap().starts_with("00000000-"),
            "{value} is a placeholder"
        );
    }

    let seeded = || {
        let mut options = RunOptions::new(transport());
        options.random = Box::new(SeededRandom::new(7));
        options
    };
    let first = live(project.clone(), &scenario, seeded()).await;
    let second = live(project, &scenario, seeded()).await;
    assert_eq!(output(&first, "one", None), output(&second, "one", None));
    assert_eq!(output(&first, "two", None), output(&second, "two", None));
}

// LANG-14 (#390) --------------------------------------------------------------

/// `later`, written first, follows the whole `if`; `then` is a chain of two
/// requests with the given sample lines, and `otherwise` the `else` braces.
fn after_the_whole_if(taken: bool, made: &str, second: &str, otherwise: &str) -> String {
    format!(
        r#"scenario "After the whole if" {{
  on_error continue
  param taken = {taken}
  @after(choice)
  later = GET "/later" {{
    sample 200 {{}}
  }}
  begin = start
  @after(begin)
  choice = if param.taken == true {{
    made = POST "/made" {{
      {made}
    }}
    @after(made)
    second = GET "/second" {{
      {second}
    }}
  }}{otherwise}
}}
"#
    )
}

const MADE: &str = "sample 201 {}";
const SECOND: &str = "sample 200 {}";
const OTHER: &str = r#" else {
    other = GET "/other" {
      sample 200 {}
    }
  }"#;

/// Criterion 1: after the last step of `then` when it is taken.
#[tokio::test]
async fn lang14_criterion_1_after_the_whole_if_waits_for_then() {
    for otherwise in ["", OTHER] {
        let (project, scenario) = lowered(&after_the_whole_if(true, MADE, SECOND, otherwise));
        let run = simulate(project, &scenario).await;
        assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
        let order = ["choice", "made", "second", "later"].map(|handle| position(&run, handle));
        assert!(order.is_sorted(), "{order:?}: {run:#?}");
        assert_eq!(by_handle(&run, "later").status, StepStatus::Passed);
    }
}

/// Criterion 1: after the last step of `else`, or after the condition when
/// the branch taken holds no step.
#[tokio::test]
async fn lang14_criterion_1_after_the_whole_if_waits_for_else_or_runs_at_once() {
    let (project, scenario) = lowered(&after_the_whole_if(false, MADE, SECOND, OTHER));
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert!(position(&run, "other") < position(&run, "later"));
    assert_eq!(by_handle(&run, "later").status, StepStatus::Passed);

    let (project, scenario) = lowered(&after_the_whole_if(false, MADE, SECOND, ""));
    let run = simulate(project, &scenario).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    for handle in ["made", "second"] {
        assert_eq!(
            by_handle(&run, handle).status,
            StepStatus::Skipped,
            "{handle}"
        );
    }
    assert!(position(&run, "choice") < position(&run, "later"));
    assert_eq!(by_handle(&run, "later").status, StepStatus::Passed);
}

/// Criterion 2: the step after the `if` does not run when the branch taken
/// ended in a step that errored or was blocked.
#[tokio::test]
async fn lang14_criterion_2_after_a_broken_branch_the_step_does_not_run() {
    let id = |run: &RunResult, handle: &str| by_handle(run, handle).node_id.clone();
    // Without a sample `second` errors; without one for `made`, `second` is blocked.
    for (made, second, last) in [
        (MADE, "", StepStatus::Error),
        ("", SECOND, StepStatus::Blocked),
    ] {
        let (project, scenario) = lowered(&after_the_whole_if(true, made, second, OTHER));
        let run = simulate(project, &scenario).await;
        assert_eq!(by_handle(&run, "second").status, last, "{run:#?}");
        assert_eq!(by_handle(&run, "later").status, StepStatus::Blocked);
        assert_eq!(
            by_handle(&run, "later").cause,
            Some(NotRunCause::BlockedBy {
                step: id(&run, "second")
            })
        );
    }
}
