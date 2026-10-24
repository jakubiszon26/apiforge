//! Native v2 Export behavior, independent of the converted v1 run corpus.
use apiforge_core::{
    format::{Binding, Component, Graph, Node, OrderEdge, Port, Position, Project},
    graph::create_node,
    model::{Branch, ErrorPolicy, NodeKind, ValueType},
    run::{RunMode, RunResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, SeededRandom, run_scenario};
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn node(id: &str, kind: NodeKind) -> Node {
    let mut node = create_node(kind, Position { x: 0.0, y: 0.0 }, None);
    node.id = id.into();
    node.handle = id.into();
    node
}

fn export(name: &str, binding: Binding) -> Node {
    let mut result = node(name, NodeKind::Export);
    result.inputs.insert("value".into(), binding);
    result
}

fn component(name: &str, nodes: Vec<Node>, edges: Vec<OrderEdge>) -> Component {
    Component {
        id: name.into(),
        handle: name.into(),
        name: name.into(),
        revision: 1,
        description: String::new(),
        inputs: vec![],
        graph: Graph {
            nodes,
            edges,
            proxies: Vec::new(),
        },
        action: None,
    }
}

fn instance(id: &str, definition: &Component) -> Node {
    let mut result = node(id, NodeKind::Component);
    result.config.component_id = Some(definition.id.clone());
    result.config.revision = Some(1);
    result
}

fn project(components: Vec<Component>, nodes: Vec<Node>) -> Project {
    let mut project = apiforge_core::demos::empty_project();
    project.components = components;
    project.scenarios[0].error_policy = ErrorPolicy::Continue;
    project.scenarios[0].graph = Graph {
        nodes,
        edges: vec![],
        proxies: Vec::new(),
    };
    project
}

async fn run(project: Project) -> RunResult {
    let id = project.scenarios[0].id.clone();
    run_scenario(
        RunRequest::new(project, id, RunMode::Simulation),
        RunOptions {
            limits: Default::default(),
            transport: HttpTransport::new().unwrap(),
            random: Box::new(SeededRandom::new(1)),
            journal: None,
        },
        CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn nested_exports_keep_null_distinct_from_missing_and_isolate_instances() {
    let mut inner = component(
        "inner",
        vec![export("payload", Binding::parameter("item"))],
        vec![],
    );
    inner
        .inputs
        .push(Port::new("item", ValueType::Object).required(true));
    let mut call = instance("inside", &inner);
    call.inputs
        .insert("item".into(), Binding::parameter("item"));
    let mut outer = component(
        "outer",
        vec![
            call,
            export(
                "number",
                Binding::reference("inside", "payload", "nested.value"),
            ),
            export("empty", Binding::reference("inside", "payload", "nothing")),
        ],
        vec![],
    );
    outer.inputs = inner.inputs.clone();
    let make = |id: &str, value: i32| {
        let mut call = instance(id, &outer);
        call.inputs.insert(
            "item".into(),
            Binding::from_json(json!({"nested":{"value":value},"nothing":null})),
        );
        call
    };
    let result = run(project(
        vec![inner, outer.clone()],
        vec![make("first", 7), make("second", 11)],
    ))
    .await;
    for (step, value) in result.steps.iter().zip([7, 11]) {
        assert_eq!(step.status, StepStatus::Passed, "{:?}", step.message);
        assert_eq!(step.outputs["number"], json!(value));
        assert_eq!(step.outputs.get("empty"), Some(&json!(null)));
        let nested = &step.children.as_ref().unwrap()[0];
        assert_eq!(nested.outputs["payload"]["nested"]["value"], json!(value));
        assert!(
            nested
                .children
                .as_ref()
                .unwrap()
                .iter()
                .all(|s| s.status == StepStatus::Passed)
        );
    }
    assert_eq!(result.steps.len(), 2);
}

#[tokio::test]
async fn a_skipped_export_has_no_output_but_an_active_null_export_does() {
    let mut gate = node("gate", NodeKind::If);
    gate.inputs
        .insert("actual".into(), Binding::literal(json!(false)));
    gate.inputs
        .insert("expected".into(), Binding::literal(json!(true)));
    let edge = |target: &str, branch| OrderEdge {
        id: target.into(),
        source: "gate".into(),
        target: target.into(),
        branch: Some(branch),
    };
    let definition = component(
        "choose",
        vec![
            gate,
            export("inactive", Binding::literal(json!("not published"))),
            export("active", Binding::literal(json!(null))),
        ],
        vec![
            edge("inactive", Branch::True),
            edge("active", Branch::False),
        ],
    );
    let result = run(project(
        vec![definition.clone()],
        vec![instance("call", &definition)],
    ))
    .await;
    let call = &result.steps[0];
    assert_eq!(call.status, StepStatus::Passed);
    assert!(!call.outputs.contains_key("inactive"));
    assert_eq!(call.outputs.get("active"), Some(&json!(null)));
    let children = call.children.as_ref().unwrap();
    assert_eq!(
        children
            .iter()
            .find(|s| s.node_id == "inactive")
            .unwrap()
            .status,
        StepStatus::Skipped
    );
}

#[tokio::test]
async fn a_failed_export_keeps_earlier_outputs_and_does_not_invent_a_value() {
    let definition = component(
        "partial",
        vec![
            export("available", Binding::literal(json!(42))),
            export(
                "broken",
                Binding::from_json(json!({"nested": null}))
                    .replace_at(
                        &["nested".into()],
                        Binding::parameter_field("item", "missing"),
                    )
                    .unwrap(),
            ),
        ],
        vec![OrderEdge {
            id: "sequence".into(),
            source: "available".into(),
            target: "broken".into(),
            branch: None,
        }],
    );
    let mut definition = definition;
    definition
        .inputs
        .push(Port::new("item", ValueType::Object).required(true));
    let mut call = instance("call", &definition);
    call.inputs
        .insert("item".into(), Binding::from_json(json!({})));
    let result = run(project(vec![definition], vec![call])).await;
    let call = &result.steps[0];
    assert_eq!(call.status, StepStatus::Error);
    assert_eq!(call.outputs["available"], json!(42));
    assert!(!call.outputs.contains_key("broken"));
    assert_eq!(call.children.as_ref().unwrap()[1].status, StepStatus::Error);
}
