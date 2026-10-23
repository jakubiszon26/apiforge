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
        "id": "source", "kind": "request", "handle": "source",
        "position": {"x": 0, "y": 0},
        "inputs": {"url": {"kind": "literal", "value": "/source"}},
        "config": {"method": "GET",
                   "sampleResponse": {"id": 7, "name": "Zażółć", "tags": ["a", "b"]}}
    })];
    all.extend(nodes.as_array().cloned().unwrap_or_default());
    serde_json::from_value(json!({
        "version": 2, "id": "p", "name": "p", "components": [], "contracts": [],
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
    json!({"id": id, "kind": "value", "handle": id, "position": {"x": 0, "y": 0},
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
        journal: None,
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
async fn requests_send_encoded_url_values_text_fields_and_bodies() {
    let text = |text: &str| json!({"kind": "text", "text": text});
    let value = |binding: Value| json!({"kind": "value", "binding": binding});
    let request = |id: &str, format: &str, body: Value| {
        json!({
            "id": id, "kind": "request", "handle": id, "position": {"x": 0, "y": 0},
            "inputs": {
                "url": {"kind": "template", "parts": [
                    text("/items/"), value(reference("name")), text("?fixed=a b")
                ]},
                "headers": {"kind": "object", "fields": {
                    "X-Tags": reference("tags"),
                    "X-Id": {"kind": "template", "parts": [text("id-"), value(reference("id"))]}
                }},
                "query": {"kind": "object", "fields": {"n": reference("id")}},
                "body": body
            },
            "config": {"method": "POST", "bodyFormat": format, "sampleResponse": {}}
        })
    };
    let run = run(project(json!([
        request("json", "json", reference("tags")),
        request("text", "text", reference("id")),
    ])))
    .await;
    let snapshot = |id: &str| step(&run, id).request.clone().expect("request snapshot");
    let json = snapshot("json");
    assert_eq!(
        json.url,
        "https://api.example.test/items/Za%C5%BC%C3%B3%C5%82%C4%87?fixed=a+b&n=7"
    );
    assert_eq!(json.headers["X-Tags"], r#"["a","b"]"#);
    assert_eq!(json.headers["X-Id"], "id-7");
    assert_eq!(json.headers["Content-Type"], "application/json");
    assert_eq!(
        json.body,
        apiforge_core::format::OptionalJson::Present(json!(["a", "b"]))
    );
    let text = snapshot("text");
    assert_eq!(text.headers["Content-Type"], "text/plain");
    assert_eq!(
        text.body,
        apiforge_core::format::OptionalJson::Present(json!("7"))
    );
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

#[tokio::test]
async fn named_request_arguments_feed_all_http_fields_in_dependency_order() {
    let arg = |name: &str, path: &str| json!({"kind": "argument", "name": name, "path": path});
    let template = |prefix: &str, value: Value| {
        json!({"kind": "template", "parts": [
            {"kind": "text", "text": prefix}, {"kind": "value", "binding": value}
        ]})
    };
    let mut document = project(json!([{
        "id": "custom", "kind": "request", "handle": "custom", "position": {"x": 0, "y": 0},
        "inputs": {
            "url": template("/products/", arg("name", "")),
            "headers": {"kind": "object", "fields": {"X-Product": template("id-", arg("productId", ""))}},
            "query": {"kind": "object", "fields": {"product": arg("productId", ""), "active": arg("active", "")}},
            "body": {"kind": "object", "fields": {
                "id": arg("productId", ""), "active": arg("active", ""), "nullable": arg("nullable", ""), "zero": arg("zero", "")
            }},
            "productId": arg("product", "id"),
            "name": arg("product", "name"),
            "active": {"kind": "literal", "value": false},
            "nullable": {"kind": "literal", "value": null},
            "zero": {"kind": "literal", "value": 0},
            "product": reference("")
        },
        "config": {"method": "POST", "sampleResponse": {}}
    }]));
    // Save/import retains the local sources and custom input names.
    document = apiforge_core::import_document(
        &serde_json::to_vec(&document).unwrap(),
        &Default::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .remove(0);
    let result = run(document).await;
    let custom = step(&result, "custom");
    assert_eq!(custom.status, StepStatus::Passed, "{:?}", custom.message);
    let request = custom.request.as_ref().unwrap();
    assert_eq!(
        request.url,
        "https://api.example.test/products/Za%C5%BC%C3%B3%C5%82%C4%87?product=7&active=false"
    );
    assert_eq!(request.headers["X-Product"], "id-7");
    assert_eq!(
        request.body,
        apiforge_core::format::OptionalJson::Present(
            json!({"id": 7, "active": false, "nullable": null, "zero": 0})
        )
    );
}

#[tokio::test]
async fn local_argument_secrets_are_redacted_in_reports() {
    let mut document = project(json!([{
        "id": "custom", "kind": "request", "handle": "custom", "position": {"x": 0, "y": 0},
        "inputs": {
            "url": {"kind": "literal", "value": "/"},
            "token": {"kind": "environment", "name": "token"},
            "body": {"kind": "argument", "name": "token"}
        }, "config": {"method": "POST", "sampleResponse": {}}
    }]));
    document.environment.secret_names.push("token".into());
    let mut request = RunRequest::new(document, "main", RunMode::Simulation);
    request
        .secrets
        .insert("token".into(), "canary-private-argument".into());
    let result = run_scenario(
        request,
        RunOptions::new(HttpTransport::new().unwrap()),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(
        step(&result, "custom").status,
        StepStatus::Passed,
        "{:?}",
        step(&result, "custom").message
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("canary-private-argument")
    );
}

#[test]
fn local_argument_cycles_unknown_names_and_escaping_scope_are_rejected() {
    for inputs in [
        json!({"url": {"kind": "argument", "name": "absent"}}),
        json!({"url": {"kind": "literal", "value": "/"}, "a": {"kind": "argument", "name": "a"}}),
        json!({"url": {"kind": "literal", "value": "/"}, "a": {"kind": "argument", "name": "b"}, "b": {"kind": "argument", "name": "a"}}),
        json!({"url": {"kind": "argument", "name": "body"}, "body": {"kind": "literal", "value": 1}}),
    ] {
        let mut doc = project(json!([]));
        doc.scenarios[0].graph.nodes[0].inputs = serde_json::from_value(inputs).unwrap();
        assert!(apiforge_core::validate_project(&doc).is_err());
    }
    let doc = project(json!([variable(
        "invalid",
        json!({"kind": "argument", "name": "x"})
    )]));
    assert!(apiforge_core::validate_project(&doc).is_err());
}

#[tokio::test]
async fn body_editor_templates_send_typed_inputs_and_interpolated_text() {
    use apiforge_core::{body_text, format::OptionalJson, model::BodyFormat, template_text::Scope};
    let mut document = project(json!([{
        "id": "custom", "kind": "request", "handle": "custom", "position": {"x": 0, "y": 0},
        "inputs": {
            "url": {"kind": "literal", "value": "/body"},
            "product": reference(""),
            "zero": {"kind": "literal", "value": 0},
            "active": {"kind": "literal", "value": false},
            "nullable": {"kind": "literal", "value": null}
        }, "config": {"method": "POST", "sampleResponse": {}}
    }]));
    let node = &document.scenarios[0].graph.nodes[1];
    let scope = Scope {
        nodes: &document.scenarios[0].graph.nodes,
        project: &document,
        request: Some(node),
    };
    let body = body_text::parse_json_body(r#"{"product": {{input.product}}, "id": "{{input.product.id}}", "zero": "{{input.zero}}", "active": {{input.active}}, "nullable": {{input.nullable}}, "label": "id-{{input.product.id}}"}"#, scope).unwrap();
    document.scenarios[0].graph.nodes[1]
        .inputs
        .insert("body".into(), body);
    let result = run(document.clone()).await;
    let custom = step(&result, "custom");
    assert_eq!(custom.status, StepStatus::Passed, "{:?}", custom.message);
    assert_eq!(
        custom.request.as_ref().unwrap().body,
        OptionalJson::Present(json!({
            "product": {"id": 7, "name": "Zażółć", "tags": ["a", "b"]}, "id": 7, "zero": 0, "active": false, "nullable": null, "label": "id-7"
        }))
    );
    let node = &document.scenarios[0].graph.nodes[1];
    let scope = Scope {
        nodes: &document.scenarios[0].graph.nodes,
        project: &document,
        request: Some(node),
    };
    let text =
        body_text::parse_text_body("<product>{{input.product.id}}</product>", scope).unwrap();
    let node = &mut document.scenarios[0].graph.nodes[1];
    node.inputs.insert("body".into(), text);
    node.config.body_format = Some(BodyFormat::Text);
    let result = run(document).await;
    let custom = step(&result, "custom");
    assert_eq!(custom.status, StepStatus::Passed, "{:?}", custom.message);
    assert_eq!(
        custom.request.as_ref().unwrap().body,
        OptionalJson::Present(json!("<product>7</product>"))
    );
}
