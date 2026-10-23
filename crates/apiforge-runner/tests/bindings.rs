//! Evaluation of object, array and template bindings in simulation runs.

use apiforge_core::{
    format::Project,
    run::{RunMode, RunResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, SeededRandom, run_scenario};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

/// A one-scenario project whose graph holds a request with a sample and `nodes`.
fn project(nodes: Value) -> Project {
    let mut all = vec![json!({
        "id": "source", "kind": "request", "name": "Source",
        "position": {"x": 0, "y": 0}, "inputs": {},
        "config": {"method": "GET", "url": "/source",
                   "sampleResponse": {"id": 7, "name": "Zażółć", "tags": ["a", "b"]}}
    })];
    all.extend(nodes.as_array().cloned().unwrap_or_default());
    serde_json::from_value(json!({
        "version": 1, "id": "p", "name": "p", "components": [], "contracts": [],
        "environment": {"baseUrl": "https://api.example.test", "variables": {"region": "eu"}, "secretNames": []},
        "scenarios": [{
            "id": "main", "name": "main", "description": "", "parameters": {"count": 2},
            "errorPolicy": "continue", "graph": {"nodes": all, "edges": []},
            "cleanup": {"nodes": [], "edges": []}
        }]
    }))
    .expect("valid project")
}

fn variable(id: &str, value: Value) -> Value {
    json!({"id": id, "kind": "variable", "name": id, "position": {"x": 0, "y": 0},
           "inputs": {"value": value}, "config": {}})
}

fn reference(path: &str) -> Value {
    json!({"kind": "reference", "nodeId": "source", "port": "body", "path": path})
}

async fn run(project: Project) -> RunResult {
    let options = RunOptions {
        limits: Default::default(),
        transport: HttpTransport::new().expect("client builds"),
        random: Box::new(SeededRandom::new(1)),
    };
    run_scenario(
        RunRequest::new(project, "main", RunMode::Simulation),
        options,
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("run starts")
}

fn step<'a>(run: &'a RunResult, id: &str) -> &'a apiforge_core::run::StepResult {
    run.steps
        .iter()
        .find(|step| step.node_id == id)
        .unwrap_or_else(|| panic!("no step {id}"))
}

#[tokio::test]
async fn objects_and_arrays_evaluate_every_child() {
    let run = run(project(json!([variable(
        "built",
        json!({"kind": "object", "fields": {
            "id": reference("id"),
            "tags": {"kind": "array", "items": [reference("tags[1]"), {"kind": "literal", "value": null}]},
            "count": {"kind": "parameter", "name": "count"},
            "region": {"kind": "environment", "name": "region"}
        }})
    )])))
    .await;
    let built = step(&run, "built");
    assert_eq!(built.status, StepStatus::Passed, "{:?}", built.message);
    assert_eq!(
        built.outputs["value"],
        json!({"id": 7, "tags": ["b", null], "count": 2, "region": "eu"})
    );
}

#[tokio::test]
async fn a_missing_child_is_an_error_naming_it() {
    let run = run(project(json!([
        variable(
            "object",
            json!({"kind": "object", "fields": {"absent": reference("absent")}})
        ),
        variable(
            "array",
            json!({"kind": "array", "items": [reference("id"), reference("absent")]})
        ),
        variable(
            "template",
            json!({"kind": "template", "parts": [
                {"kind": "text", "text": "id "},
                {"kind": "value", "binding": reference("id")},
                {"kind": "value", "binding": reference("absent")}
            ]})
        ),
    ])))
    .await;
    for (id, message) in [
        ("object", "Missing value for field `absent`."),
        ("array", "Missing value for item 1."),
        ("template", "Missing value for template value 2."),
    ] {
        let result = step(&run, id);
        assert_eq!(result.status, StepStatus::Error, "{id}");
        assert_eq!(result.message.as_deref(), Some(message), "{id}");
    }
}

#[tokio::test]
async fn templates_convert_values_like_javascript_strings() {
    let run = run(project(json!([variable(
        "text",
        json!({"kind": "template", "parts": [
            {"kind": "text", "text": "#"},
            {"kind": "value", "binding": reference("id")},
            {"kind": "text", "text": " "},
            {"kind": "value", "binding": reference("name")},
            {"kind": "text", "text": " "},
            {"kind": "value", "binding": reference("tags")}
        ]})
    )])))
    .await;
    assert_eq!(
        step(&run, "text").outputs["value"],
        json!("#7 Zażółć [\"a\",\"b\"]")
    );
}
