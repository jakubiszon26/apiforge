//! RUN-03 (#389): checks and values run before the next step.
//!
//! When a step has finished, every check and value that is ready is computed
//! before the next request, component call or `if` starts (FLOW-00, rule 6).
//! The runs here are simulated, or live against a loopback server that
//! records what reaches it.

use apiforge_core::{
    demos::{demo_projects, shop_project},
    format::{Graph, Project, Scenario},
    graph::dependency_index,
    model::NodeKind,
    run::{NotRunCause, Phase, RunMode, RunResult, RunStatus, StepResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use std::{
    collections::HashSet,
    fs,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

/// A request, a check of it and a value of it, the request's next step
/// written between them. The check expects `expected`.
fn cart_then_payment(policy: &str, expected: u16) -> String {
    format!(
        r#"scenario "Checks before the next step" {{
  on_error {policy}
  begin = start
  @after(begin)
  cart = POST "/carts" {{
    sample 201 {{ id: "c-1", total: 0 }}
  }}
  @after(cart)
  paid = POST "/payments" {{
    sample 201 {{ id: "p-1" }}
  }}
  created = assert cart.status == {expected}
  total = cart.body.total
}}
"#
    )
}

/// The same flow in the cleanup.
const CLEANUP: &str = r#"scenario "Checks before the next cleanup step" {
  begin = start
  @after(begin)
  cart = POST "/carts" {
    sample 201 { id: "c-1", total: 0 }
  }
  cleanup {
    removed = DELETE "/carts/{{cart.body.id}}" {
      sample 204 {}
    }
    @after(removed)
    again = GET "/carts/{{cart.body.id}}" {
      sample 404 {}
    }
    gone = assert removed.status == 204
  }
}
"#;

/// The same flow inside a component, which a scenario calls.
fn cart_component(expected: u16) -> String {
    format!(
        r#"component cart_flow "Cart flow" {{
  begin = start
  @after(begin)
  cart = POST "/carts" {{
    sample 201 {{ id: "c-1", total: 0 }}
  }}
  @after(cart)
  paid = POST "/payments" {{
    sample 201 {{ id: "p-1" }}
  }}
  created = assert cart.status == {expected}
  export id: string = cart.body.id
}}
"#
    )
}

const CALLER: &str = r#"scenario "Calls the cart flow" {
  on_error stop
  begin = start
  @after(begin)
  flow = cart_flow@1()
}
"#;

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

/// The caller of `cart_flow`, published as revision 1 in the shop project.
fn with_component(expected: u16) -> (Project, Scenario) {
    let mut project = shop_project();
    let Document::Component(mut component) = lowered(&project, &cart_component(expected)) else {
        panic!("not a component");
    };
    component.revision = 1;
    project.components.push(component);
    scenario_in(project, CALLER)
}

async fn run(project: Project, scenario: &Scenario, mode: RunMode) -> RunResult {
    run_scenario(
        RunRequest::new(project, scenario.id.clone(), mode),
        RunOptions::new(HttpTransport::new().expect("client builds")),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("the run starts")
}

fn handles(steps: &[StepResult], phase: Phase) -> Vec<&str> {
    steps
        .iter()
        .filter(|step| step.phase == phase)
        .map(|step| step.handle.as_str())
        .collect()
}

fn step<'a>(steps: &'a [StepResult], handle: &str) -> &'a StepResult {
    steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no result for `{handle}`: {steps:#?}"))
}

/// A loopback API that answers every request 201 with a cart and records the
/// method and path of each.
struct Api {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Api {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let log = Arc::clone(&log);
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let mut chunk = [0; 1024];
                    while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                        let count = stream.read(&mut chunk).await.unwrap();
                        if count == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&chunk[..count]);
                    }
                    let text = String::from_utf8_lossy(&bytes);
                    let line: Vec<&str> = text
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .split_whitespace()
                        .take(2)
                        .collect();
                    log.lock().unwrap().push(line.join(" "));
                    let body = r#"{"id":"c-1","total":0}"#;
                    let response = format!(
                        "HTTP/1.1 201 Created\r\ncontent-type: application/json\r\n\
                         content-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self {
            base_url,
            received,
            task,
        }
    }

    fn received(&self) -> Vec<String> {
        self.received.lock().unwrap().clone()
    }
}

impl Drop for Api {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Criterion 1, simulated: `created` and `total` are written after `paid`,
/// and run before it.
#[tokio::test]
async fn run03_criterion_1_the_simulated_run_evaluates_the_check_first() {
    let (project, scenario) = scenario(&cart_then_payment("continue", 201));
    let run = run(project, &scenario, RunMode::Simulation).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(
        handles(&run.steps, Phase::Main),
        ["begin", "cart", "created", "total", "paid"]
    );
}

/// Criterion 1, live: the same order against a real API.
#[tokio::test]
async fn run03_criterion_1_the_live_run_evaluates_the_check_first() {
    let api = Api::start().await;
    let (mut project, scenario) = scenario(&cart_then_payment("continue", 201));
    project.environment.base_url = api.base_url.clone();
    let run = run(project, &scenario, RunMode::Live).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(
        handles(&run.steps, Phase::Main),
        ["begin", "cart", "created", "total", "paid"]
    );
    assert_eq!(api.received(), ["POST /carts", "POST /payments"]);
}

/// Criterion 2: under `on_error stop` the failed check stops the run before
/// `paid`, which is not run and sends nothing.
fn assert_stopped_before_payment(steps: &[StepResult]) {
    let created = step(steps, "created");
    assert_eq!(created.status, StepStatus::Failed, "{steps:#?}");
    let paid = step(steps, "paid");
    assert_eq!(paid.status, StepStatus::Skipped, "{steps:#?}");
    assert_eq!(
        paid.cause,
        Some(NotRunCause::Stopped {
            by: created.node_id.clone()
        })
    );
    assert!(paid.request.is_none(), "nothing is sent: {paid:#?}");
}

#[tokio::test]
async fn run03_criterion_2_a_failed_check_stops_the_simulated_run_before_the_next_request() {
    let (project, scenario) = scenario(&cart_then_payment("stop", 200));
    let run = run(project, &scenario, RunMode::Simulation).await;
    assert_eq!(run.status, RunStatus::Failed, "{run:#?}");
    assert_stopped_before_payment(&run.steps);
}

#[tokio::test]
async fn run03_criterion_2_a_failed_check_stops_the_live_run_before_the_next_request() {
    let api = Api::start().await;
    let (mut project, scenario) = scenario(&cart_then_payment("stop", 200));
    project.environment.base_url = api.base_url.clone();
    let run = run(project, &scenario, RunMode::Live).await;
    assert_eq!(run.status, RunStatus::Failed, "{run:#?}");
    assert_stopped_before_payment(&run.steps);
    assert_eq!(api.received(), ["POST /carts"], "the payment is never sent");
}

/// Criterion 4: in the cleanup, `gone` runs before `again`.
#[tokio::test]
async fn run03_criterion_4_the_cleanup_evaluates_the_check_first() {
    let (project, scenario) = scenario(CLEANUP);
    let run = run(project, &scenario, RunMode::Simulation).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(
        handles(&run.steps, Phase::Cleanup),
        ["removed", "gone", "again"]
    );
}

fn children(run: &RunResult) -> &[StepResult] {
    step(&run.steps, "flow")
        .children
        .as_deref()
        .expect("the call reports its steps")
}

/// Criterion 4: inside a component, the check and the export run before
/// `paid`.
#[tokio::test]
async fn run03_criterion_4_a_component_evaluates_the_check_first() {
    let (project, scenario) = with_component(201);
    let run = run(project, &scenario, RunMode::Simulation).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(
        handles(children(&run), Phase::Main),
        ["begin", "cart", "created", "id", "paid"]
    );
}

/// Criterion 4: under the caller's `on_error stop`, a failed check inside the
/// component stops it before `paid`, live too.
#[tokio::test]
async fn run03_criterion_4_a_failed_check_stops_a_component_before_the_next_request() {
    let (project, scenario) = with_component(200);
    let simulated = run(project.clone(), &scenario, RunMode::Simulation).await;
    assert_stopped_before_payment(children(&simulated));

    let api = Api::start().await;
    let mut project = project;
    project.environment.base_url = api.base_url.clone();
    let live = run(project, &scenario, RunMode::Live).await;
    assert_stopped_before_payment(children(&live));
    assert_eq!(api.received(), ["POST /carts"], "the payment is never sent");
}

/// Criterion 5: `@after(check)` is still accepted and still orders.
#[tokio::test]
async fn run03_criterion_5_after_a_check_is_still_accepted() {
    let text = cart_then_payment("continue", 201)
        .replace("  @after(cart)\n  paid", "  @after(created)\n  paid");
    assert!(text.contains("@after(created)"));
    let (project, scenario) = scenario(&text);
    let run = run(project, &scenario, RunMode::Simulation).await;
    assert_eq!(run.status, RunStatus::Passed, "{run:#?}");
    assert_eq!(
        handles(&run.steps, Phase::Main),
        ["begin", "cart", "created", "total", "paid"]
    );
}

// Criterion 3 ----------------------------------------------------------------

fn is_step(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Start | NodeKind::Request | NodeKind::Component | NodeKind::If
    )
}

/// The order before RUN-03: the first ready node of the document, whatever
/// its kind. Dependencies outside `graph` hold nothing back.
fn previous_order(graph: &Graph) -> Vec<&str> {
    let index = dependency_index(graph);
    let mut pending: Vec<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    let known: HashSet<&str> = pending.iter().copied().collect();
    let mut done: HashSet<&str> = HashSet::new();
    let mut order = Vec::new();
    while let Some(ready) = pending.iter().position(|id| {
        index[id]
            .iter()
            .all(|source| !known.contains(source) || done.contains(source))
    }) {
        let id = pending.remove(ready);
        done.insert(id);
        order.push(id);
    }
    order.extend(pending);
    order
}

/// Compares the steps a graph ran with its order before RUN-03: the same
/// steps in the same order, every data statement after no more steps than
/// before. Returns the data statements that moved earlier, by handle.
fn compare(name: &str, graph: &Graph, ran: &[&StepResult], project: &Project) -> Vec<String> {
    let kind = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.kind)
            .unwrap_or_else(|| panic!("{name}: no node {id}"))
    };
    let before = previous_order(graph);
    let after: Vec<&str> = ran.iter().map(|step| step.node_id.as_str()).collect();
    assert_eq!(after.len(), before.len(), "{name}: every node has a result");
    let steps = |order: &[&str]| -> Vec<String> {
        order
            .iter()
            .filter(|id| is_step(kind(id)))
            .map(|id| (*id).to_owned())
            .collect()
    };
    assert_eq!(steps(&after), steps(&before), "{name}: the order of steps");
    let steps_before = |order: &[&str], id: &str| {
        order
            .iter()
            .take_while(|other| **other != id)
            .filter(|other| is_step(kind(other)))
            .count()
    };
    let mut moved = Vec::new();
    for node in graph.nodes.iter().filter(|node| !is_step(node.kind)) {
        let (old, new) = (
            steps_before(&before, &node.id),
            steps_before(&after, &node.id),
        );
        assert!(new <= old, "{name}: `{}` runs later", node.handle);
        if new < old {
            moved.push(node.handle.clone());
        }
    }
    // The steps of a call are compared with the component's graph.
    for step in ran.iter().filter(|step| step.kind == NodeKind::Component) {
        let Some(children) = &step.children else {
            continue;
        };
        let node = graph.nodes.iter().find(|node| node.id == step.node_id);
        let Some(component) =
            node.and_then(|node| apiforge_core::graph::component_definition(project, node))
        else {
            continue;
        };
        let inner: Vec<&StepResult> = children.iter().collect();
        let name = format!("{name} › {}", component.handle);
        for handle in compare(&name, &component.graph, &inner, project) {
            moved.push(format!("{}.{handle}", component.handle));
        }
    }
    moved
}

/// Every scenario of the repository: the LAY references and the canonical
/// script in the shop project, and the scenarios of the demo projects.
fn repository_scenarios() -> Vec<(String, Project, Scenario)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut scenarios = Vec::new();
    let mut files: Vec<_> = fs::read_dir(root.join("fixtures/scenarios/lay"))
        .expect("the LAY fixtures")
        .map(|entry| entry.expect("a readable directory").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "afs"))
        .collect();
    files.sort();
    files.push(root.join("fixtures/scripts/checkout.afs"));
    for path in files {
        let text = fs::read_to_string(&path).expect("a readable fixture");
        let (project, scenario) = scenario(&text);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        scenarios.push((name, project, scenario));
    }
    for project in demo_projects() {
        for scenario in project.scenarios.clone() {
            let name = format!("{} › {}", project.name, scenario.name);
            scenarios.push((name, project.clone(), scenario));
        }
    }
    scenarios
}

/// Criterion 3: the steps of every repository scenario, its components and
/// its cleanup keep their order; only data statements move earlier. The
/// list of what moved is printed for the pull request.
#[tokio::test]
async fn run03_criterion_3_every_repository_scenario_keeps_the_order_of_its_steps() {
    let scenarios = repository_scenarios();
    assert!(scenarios.len() >= 10, "{}", scenarios.len());
    for (name, project, scenario) in scenarios {
        let run = run(project.clone(), &scenario, RunMode::Simulation).await;
        let mut moved = Vec::new();
        for (phase, graph) in [
            (Phase::Main, &scenario.graph),
            (Phase::Cleanup, &scenario.cleanup),
        ] {
            let ran: Vec<&StepResult> = run
                .steps
                .iter()
                .filter(|step| step.phase == phase)
                .collect();
            moved.extend(compare(
                &format!("{name}, {phase:?}"),
                graph,
                &ran,
                &project,
            ));
        }
        println!(
            "{name}: {}",
            if moved.is_empty() {
                "-".to_owned()
            } else {
                moved.join(", ")
            }
        );
    }
}
