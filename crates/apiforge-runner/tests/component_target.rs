//! KTLO-23a: a component runs on its own, as a scenario does — its own steps,
//! its exports as steps, its input defaults, the pinned revisions it calls, and
//! a live run journalled under its name (docs/tickets/KTLO-23.md).

use apiforge_core::{
    format::{Component, Project},
    journal::{Journal, JournalEntry, JournalRun, JournalSink, JournalSinkError},
    model::NodeKind,
    run::{RunMode, RunResult, RunStatus, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, RunTarget, run_scenario};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;

/// The shop fixture: `get-product` (one input, `id`, defaulting to 123) and
/// `product-cart` (`product` without a default, `quantity` defaulting to 1).
fn shop() -> Project {
    serde_json::from_slice(include_bytes!("../../../fixtures/projects/shop.json")).unwrap()
}

fn component<'a>(project: &'a Project, id: &str) -> &'a Component {
    project
        .components
        .iter()
        .find(|component| component.id == id)
        .unwrap()
}

fn request(
    project: Project,
    id: &str,
    inputs: IndexMap<String, Value>,
    mode: RunMode,
) -> RunRequest {
    RunRequest {
        target: RunTarget::Component {
            revision: 1,
            inputs,
        },
        ..RunRequest::component(project, id, 1, mode)
    }
}

async fn run(request: RunRequest, options: RunOptions) -> RunResult {
    run_scenario(request, options, CancellationToken::new(), |_| {})
        .await
        .unwrap()
}

fn options() -> RunOptions {
    RunOptions::new(HttpTransport::new().unwrap())
}

/// Criterion 2: every input has a default, so the component runs as written;
/// the report lists the component's own nodes, and its `export` is a step
/// whose value is the output.
#[tokio::test]
async fn ktlo23_a_component_runs_on_its_own_with_its_input_defaults() {
    let project = shop();
    let product = component(&project, "get-product").clone();
    let report = run(
        RunRequest::component(project, "get-product", 1, RunMode::Simulation),
        options(),
    )
    .await;
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    assert_eq!(report.scenario_id, "get-product");
    let ids: Vec<&str> = report
        .steps
        .iter()
        .map(|step| step.node_id.as_str())
        .collect();
    let nodes: Vec<&str> = product
        .graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(ids, nodes, "the steps are the component's own blocks");
    assert_eq!(
        report.snapshot.as_ref().unwrap().graph,
        product.graph,
        "the report's board is the component's"
    );
    let fetch = &report.steps[0];
    assert!(
        fetch
            .request
            .as_ref()
            .unwrap()
            .url
            .ends_with("/products/123"),
        "the default of `id` is used"
    );
    let export = report
        .steps
        .iter()
        .find(|step| step.kind == NodeKind::Export)
        .expect("an export step");
    assert_eq!(export.status, StepStatus::Passed);
    assert_eq!(
        export.outputs.get("value"),
        fetch.outputs.get("body"),
        "the export's value is the output"
    );
}

/// Decision 1 (the values the Run inputs dialog will give): a value replaces
/// the default.
#[tokio::test]
async fn ktlo23_a_given_input_replaces_its_default() {
    let inputs = IndexMap::from([("id".to_owned(), json!(124))]);
    let report = run(
        request(shop(), "get-product", inputs, RunMode::Simulation),
        options(),
    )
    .await;
    assert_eq!(report.status, RunStatus::Passed);
    assert!(
        report.steps[0]
            .request
            .as_ref()
            .unwrap()
            .url
            .ends_with("/products/124")
    );
}

/// Criterion 7: until KTLO-23b asks for it, an input without a default and
/// without a value fails the step that reads it.
#[tokio::test]
async fn ktlo23_an_input_without_a_default_fails_its_step_as_missing() {
    let report = run(
        RunRequest::component(shop(), "product-cart", 1, RunMode::Simulation),
        options(),
    )
    .await;
    assert_eq!(report.status, RunStatus::Failed);
    let reading = report
        .steps
        .iter()
        .find(|step| step.node_id == "id")
        .expect("the step that reads `product`");
    assert_eq!(reading.status, StepStatus::Error);
    assert_eq!(
        reading.message.as_deref(),
        Some("Missing required input: product.")
    );
    assert!(
        report
            .steps
            .iter()
            .filter(|step| step.node_id != "id")
            .all(|step| step.status != StepStatus::Passed),
        "nothing after it passes: {:?}",
        report.steps
    );
}

/// Criterion 4: a component that calls another runs the revision its call
/// pins, not a later one, and the called steps are the call's children.
#[tokio::test]
async fn ktlo23_a_called_component_runs_its_pinned_revision() {
    let mut project = shop();
    let mut later = component(&project, "get-product").clone();
    later.revision = 2;
    later.graph.nodes[0].config.sample_response =
        apiforge_core::format::OptionalJson::Present(json!({ "id": 999 }));
    project.components.push(later);
    let mut outer = component(&project, "get-product").clone();
    outer.id = "outer".into();
    outer.handle = "outer".into();
    outer.name = "Outer".into();
    outer.inputs.clear();
    outer.graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "call", "kind": "component", "handle": "call",
             "position": {"x": 0, "y": 0}, "inputs": {"id": {"kind": "literal", "value": 124}},
             "config": {"componentId": "get-product", "revision": 1}},
            {"id": "out", "kind": "export", "handle": "product",
             "position": {"x": 0, "y": 0},
             "inputs": {"value": {"kind": "reference", "nodeId": "call", "port": "product", "path": ""}},
             "config": {"type": "object"}}
        ],
        "edges": []
    }))
    .unwrap();
    project.components.push(outer);
    let report = run(
        RunRequest::component(project, "outer", 1, RunMode::Simulation),
        options(),
    )
    .await;
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    let call = &report.steps[0];
    let children = call.children.as_deref().expect("the call's own steps");
    assert_eq!(children[0].node_id, "fetch");
    assert!(
        children[0]
            .request
            .as_ref()
            .unwrap()
            .url
            .ends_with("/products/124")
    );
    assert_eq!(
        report.steps[1].outputs["value"]["id"],
        json!(123),
        "revision 1's sample, not revision 2's"
    );
}

/// A run asks for a component that exists at that revision.
#[tokio::test]
async fn ktlo23_an_unknown_component_or_input_does_not_start() {
    let unknown = RunRequest::component(shop(), "get-product", 7, RunMode::Simulation);
    assert!(apiforge_runner::validate(&unknown).is_err());
    let inputs = IndexMap::from([("nope".to_owned(), json!(1))]);
    let extra = request(shop(), "get-product", inputs, RunMode::Simulation);
    assert!(apiforge_runner::validate(&extra).is_err());
}

/// A journal in memory, with one line per call.
#[derive(Default)]
struct Recorder {
    journal: Mutex<Journal>,
    calls: Mutex<Vec<String>>,
}

impl JournalSink for Recorder {
    fn begin_run(&self, run: &JournalRun) -> Result<(), JournalSinkError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("begin {} {}", run.scenario_id, run.scenario_name));
        self.journal.lock().unwrap().begin(run.clone());
        Ok(())
    }

    fn record(&self, run_id: &str, entry: &JournalEntry) -> Result<(), JournalSinkError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("record {} {:?}", entry.method, entry.state));
        if let Some(run) = self.journal.lock().unwrap().run_mut(run_id) {
            run.record(entry.clone());
        }
        Ok(())
    }

    fn cleanup_started(&self, _: &str) -> Result<(), JournalSinkError> {
        self.calls.lock().unwrap().push("cleanup".into());
        Ok(())
    }

    fn finish_run(&self, run_id: &str) -> Result<(), JournalSinkError> {
        self.calls.lock().unwrap().push("finish".into());
        self.journal.lock().unwrap().finish(run_id);
        Ok(())
    }
}

/// A loopback API that answers every request with a product, or never answers.
async fn server(answer: bool) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buffer = [0; 4096];
                let _ = stream.read(&mut buffer).await;
                if !answer {
                    std::future::pending::<()>().await;
                }
                let body = json!({"id": 123, "name": "Live"}).to_string();
                let _ = stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (base_url, task)
}

/// Criterion 8: a live component run sends its requests and is journalled
/// under the component's name; a component has no cleanup phase. A process
/// that dies mid-run leaves the run, named so, for the next start to list.
#[tokio::test]
async fn ktlo23_a_live_component_run_is_journalled_under_its_name() {
    let (base_url, task) = server(true).await;
    let mut project = shop();
    project.environment.base_url = base_url;
    let name = component(&project, "get-product").name.clone();
    let journal = Arc::new(Recorder::default());
    let report = run(
        RunRequest::component(project.clone(), "get-product", 1, RunMode::Live),
        options().with_journal(journal.clone() as _),
    )
    .await;
    task.abort();
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    assert_eq!(report.steps[1].outputs["value"]["name"], json!("Live"));
    assert_eq!(
        *journal.calls.lock().unwrap(),
        [
            format!("begin get-product {name}"),
            "record GET InFlight".to_owned(),
            "record GET Completed".to_owned(),
            "finish".to_owned(),
        ]
    );

    let (base_url, task) = server(false).await;
    project.environment.base_url = base_url;
    let journal = Arc::new(Recorder::default());
    let abandoned = tokio::time::timeout(
        Duration::from_millis(300),
        run_scenario(
            RunRequest::component(project, "get-product", 1, RunMode::Live),
            options().with_journal(journal.clone() as _),
            CancellationToken::new(),
            |_| {},
        ),
    )
    .await;
    task.abort();
    assert!(abandoned.is_err(), "the stalled run cannot finish");
    let left = journal.journal.lock().unwrap().clone();
    assert_eq!(left.runs.len(), 1);
    assert_eq!(left.runs[0].scenario_id, "get-product");
    assert_eq!(left.runs[0].scenario_name, name);
}

/// KTLO-32: the shop with a component `notes` whose only input `note` is a
/// string declared as `declare` says (`required` absent, `false` or `true`)
/// with `default`, whose export returns `param.note`, and a scenario `calls`
/// whose one step calls `notes@1` with `arguments` as its inputs.
fn notes(required: Option<bool>, default: Option<Value>, arguments: Value) -> Project {
    let mut project = shop();
    let mut note = json!({ "name": "note", "type": "string" });
    if let Some(required) = required {
        note["required"] = json!(required);
    }
    if let Some(default) = default {
        note["defaultValue"] = default;
    }
    project.components.push(
        serde_json::from_value(json!({
            "id": "notes", "revision": 1, "handle": "notes", "name": "Notes",
            "description": "", "inputs": [note],
            "graph": {
                "nodes": [
                    {"id": "out", "kind": "export", "handle": "note",
                     "position": {"x": 0, "y": 0},
                     "inputs": {"value": {"kind": "parameter", "name": "note"}},
                     "config": {"type": "string"}}
                ],
                "edges": []
            }
        }))
        .unwrap(),
    );
    project.scenarios.push(
        serde_json::from_value(json!({
            "id": "calls", "name": "Calls", "description": "",
            "graph": {
                "nodes": [
                    {"id": "call", "kind": "component", "handle": "notes",
                     "position": {"x": 0, "y": 0}, "inputs": arguments,
                     "config": {"componentId": "notes", "revision": 1}}
                ],
                "edges": []
            },
            "cleanup": {"nodes": [], "edges": []},
            "parameters": {},
            "errorPolicy": "stop"
        }))
        .unwrap(),
    );
    project
}

/// KTLO-32 criterion 2: run alone without `note`, an input with neither
/// attribute nor default fails the step that reads it as missing, as a
/// `@required(true)` one does; `@optional` runs on with the input absent
/// (KTLO-27), and a default is used.
#[tokio::test]
async fn ktlo32_an_undeclared_input_is_required_when_run_alone() {
    let alone = |project| RunRequest::component(project, "notes", 1, RunMode::Simulation);
    for required in [None, Some(true)] {
        let report = run(alone(notes(required, None, json!({}))), options()).await;
        assert_eq!(report.status, RunStatus::Failed, "required: {required:?}");
        assert_eq!(report.steps[0].node_id, "out");
        assert_eq!(report.steps[0].status, StepStatus::Error);
        assert_eq!(
            report.steps[0].message.as_deref(),
            Some("Missing required input: note."),
            "required: {required:?}"
        );
    }

    let report = run(alone(notes(Some(false), None, json!({}))), options()).await;
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.steps);
    assert_eq!(
        report.steps[0].outputs.get("value"),
        None,
        "absent, not null"
    );

    let report = run(
        alone(notes(None, Some(json!("from the default")), json!({}))),
        options(),
    )
    .await;
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.steps);
    assert_eq!(
        report.steps[0].outputs.get("value"),
        Some(&json!("from the default"))
    );

    let inputs = IndexMap::from([("note".to_owned(), json!("given"))]);
    let report = run(
        request(
            notes(None, None, json!({})),
            "notes",
            inputs,
            RunMode::Simulation,
        ),
        options(),
    )
    .await;
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.steps);
    assert_eq!(report.steps[0].outputs.get("value"), Some(&json!("given")));
}

/// KTLO-32 criterion 3: a scenario's `notes@1(…)` without the argument fails
/// the call step with *Missing required input: note.*, before the component
/// runs; with the argument the component runs and returns it.
#[tokio::test]
async fn ktlo32_an_undeclared_input_is_required_on_a_call() {
    let scenario = |project| RunRequest::new(project, "calls", RunMode::Simulation);
    let report = run(scenario(notes(None, None, json!({}))), options()).await;
    assert_eq!(report.status, RunStatus::Failed, "{:?}", report.steps);
    let call = &report.steps[0];
    assert_eq!(call.node_id, "call");
    assert_eq!(call.status, StepStatus::Error);
    assert_eq!(
        call.message.as_deref(),
        Some("Missing required input: note.")
    );
    assert!(
        call.children.as_deref().unwrap_or_default().is_empty(),
        "the component does not run: {:?}",
        call.children
    );

    let given = json!({ "note": { "kind": "literal", "value": "hello" } });
    let report = run(scenario(notes(None, None, given)), options()).await;
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.steps);
    assert_eq!(report.steps[0].outputs.get("note"), Some(&json!("hello")));
}
