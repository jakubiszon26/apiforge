use super::*;
use crate::{
    edit::test_support::at,
    format::{Environment, Node, NodeConfig, OrderEdge, Position, Scenario},
    graph::create_node,
    model::{ErrorPolicy, HttpMethod, NodeKind},
    operator::Operator,
};
use indexmap::IndexMap;
use serde_json::json;

fn request(id: &str, inputs: &[(&str, Binding)]) -> Node {
    let mut port = Port::new("id", ValueType::Any).required(true);
    port.exposed = Some(true);
    Node {
        id: id.into(),
        kind: NodeKind::Request,
        handle: id.into(),
        label: None,
        note: None,
        position: Position { x: 0.0, y: 0.0 },
        inputs: inputs
            .iter()
            .map(|(name, binding)| ((*name).to_owned(), binding.clone()))
            .collect(),
        config: NodeConfig {
            method: Some(HttpMethod::Get),
            url: Some("/records/:id".into()),
            input_ports: Some(vec![port]),
            ..NodeConfig::default()
        },
    }
}

fn plain_request(id: &str) -> Node {
    request(id, &[("id", Binding::literal(json!("123")))])
}

fn project(graph: Graph, components: Vec<Component>) -> Project {
    Project {
        version: 1,
        id: "test-project".into(),
        name: "Test".into(),
        components,
        contracts: vec![],
        environment: Environment {
            base_url: "http://localhost:3000".into(),
            variables: IndexMap::new(),
            secret_names: vec![],
        },
        scenarios: vec![Scenario {
            id: "scenario".into(),
            name: "Scenario".into(),
            description: String::new(),
            graph,
            cleanup: Graph {
                nodes: vec![],
                edges: vec![],
            },
            parameters: IndexMap::new(),
            error_policy: ErrorPolicy::Continue,
        }],
    }
}

fn draft(
    graph: &Graph,
    ids: &HashSet<&str>,
    project: &Project,
    initial: Option<&Component>,
) -> ComponentDraft {
    ComponentDraft {
        name: "Reusable request".into(),
        description: String::new(),
        inputs: initialize_inputs(graph, ids, project, initial),
        outputs: initialize_outputs(graph, ids, project, initial),
        action: None,
        action_type: ValueType::Any,
        action_contract: None,
    }
}

fn output(name: &str, value_type: ValueType, binding: Binding) -> OutputPort {
    OutputPort {
        name: name.into(),
        value_type,
        label: None,
        contract_id: None,
        required: None,
        default_value: OptionalJson::Missing,
        exposed: None,
        binding,
    }
}

fn component(id: &str, revision: u32, graph: Graph) -> Component {
    Component {
        id: id.into(),
        revision,
        name: "Read".into(),
        description: String::new(),
        inputs: vec![],
        outputs: vec![],
        graph,
        action: None,
    }
}

fn edge(id: &str, source: &str, target: &str) -> OrderEdge {
    OrderEdge {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        branch: None,
    }
}

#[test]
fn extraction_maps_external_arguments_and_keeps_internal_dependencies() {
    let mut select = create_node(NodeKind::Select, Position { x: 0.0, y: 0.0 }, None);
    select.id = "select".into();
    select.config.path = Some("data.id".into());
    select
        .inputs
        .insert("value".into(), Binding::reference("get", "body", ""));
    let graph = Graph {
        nodes: vec![
            plain_request("source"),
            request(
                "get",
                &[("id", Binding::reference("source", "body", "data.id"))],
            ),
            select,
            request(
                "outside",
                &[("id", Binding::reference("select", "value", ""))],
            ),
        ],
        edges: vec![
            edge("order", "get", "select"),
            edge("outside-order", "select", "outside"),
        ],
    };
    let before = graph.clone();
    let ids = HashSet::from(["get", "select"]);
    let p = project(graph.clone(), vec![]);
    let mut d = draft(&graph, &ids, &p, None);
    let argument = d.inputs.iter_mut().find(|row| row.input == "id").unwrap();
    assert!(argument.mandatory && argument.enabled);
    assert_eq!(argument.label, "get → id");
    argument.name = "recordId".into();
    assert_eq!(
        d.outputs.iter().filter(|row| row.enabled).count(),
        1,
        "only the first output of the last block"
    );
    let result = materialize_component(&graph, &ids, &d, None).unwrap();
    assert!(result.id.starts_with("component-"));
    assert_eq!(result.revision, 1);
    assert_eq!(
        result.graph.nodes[0].inputs["id"],
        Binding::parameter("recordId")
    );
    assert_eq!(
        result.graph.nodes[1].inputs["value"],
        Binding::reference("get", "body", "")
    );
    assert_eq!(result.graph.edges, vec![edge("order", "get", "select")]);
    assert_eq!(
        result.outputs[0].binding,
        Binding::reference("select", "value", "")
    );
    assert_eq!(graph, before);
}

#[test]
fn one_parameter_feeds_several_blocks_and_null_defaults_survive_revision() {
    let parameter = Binding::parameter("recordId");
    let graph = Graph {
        nodes: vec![
            request("one", &[("id", parameter.clone())]),
            request("two", &[("id", parameter.clone())]),
        ],
        edges: vec![],
    };
    let mut public = Port::new("recordId", ValueType::Any)
        .labelled("Record identifier")
        .required(true);
    public.exposed = Some(true);
    public.default_value = OptionalJson::Present(Value::Null);
    let mut record = output(
        "record",
        ValueType::Object,
        Binding::reference("two", "body", "data.record"),
    );
    record.label = Some("Selected record".into());
    let initial = Component {
        name: "Read twice".into(),
        description: "Description".into(),
        inputs: vec![public],
        outputs: vec![record],
        ..component("reusable", 3, graph.clone())
    };
    let ids = HashSet::from(["one", "two"]);
    let p = project(graph.clone(), vec![initial.clone()]);
    let mut d = draft(&graph, &ids, &p, Some(&initial));
    assert_eq!(d.inputs.iter().filter(|row| row.enabled).count(), 1);
    assert_eq!(d.inputs[0].targets.len(), 2);
    assert_eq!(d.inputs[0].default_value, Some(Value::Null));
    d.inputs[0].name = "targetId".into();
    let result = materialize_component(&graph, &ids, &d, Some(&initial)).unwrap();
    assert_eq!((result.id.as_str(), result.revision), ("reusable", 3));
    assert_eq!(
        result.inputs[0].default_value,
        OptionalJson::Present(Value::Null)
    );
    assert_eq!(result.inputs[0].label.as_deref(), Some("Record identifier"));
    assert_eq!(result.outputs[0].label.as_deref(), Some("Selected record"));
    for node in &result.graph.nodes {
        assert_eq!(node.inputs["id"], Binding::parameter("targetId"));
    }
    assert_eq!(
        result.outputs[0].binding,
        Binding::reference("two", "body", "data.record")
    );
    assert_eq!(initial.graph.nodes[0].inputs["id"], parameter);
}

#[test]
fn a_direct_parameter_output_follows_the_argument_rename() {
    let graph = Graph {
        nodes: vec![plain_request("request")],
        edges: vec![],
    };
    let mut context = Port::new("context", ValueType::Object).required(false);
    context.default_value = OptionalJson::Present(json!({"source": "manual"}));
    let initial = Component {
        inputs: vec![context],
        outputs: vec![
            output("context", ValueType::Object, Binding::parameter("context")),
            output(
                "body",
                ValueType::Any,
                Binding::reference("request", "body", ""),
            ),
        ],
        ..component("reusable", 1, graph.clone())
    };
    let ids = HashSet::from(["request"]);
    let p = project(graph.clone(), vec![initial.clone()]);
    let mut d = draft(&graph, &ids, &p, Some(&initial));
    let row = d
        .inputs
        .iter_mut()
        .find(|row| row.name == "context")
        .unwrap();
    assert_eq!(row.key, "public:context");
    assert_eq!(row.label, "Public argument → context");
    row.name = "metadata".into();
    let result = materialize_component(&graph, &ids, &d, Some(&initial)).unwrap();
    assert_eq!(result.outputs[0].binding, Binding::parameter("metadata"));
    let metadata = result.inputs.iter().find(|input| input.name == "metadata");
    assert_eq!(
        metadata.map(|input| &input.default_value),
        Some(&OptionalJson::Present(json!({"source": "manual"})))
    );
}

#[test]
fn duplicate_names_and_disabled_boundary_arguments_are_rejected() {
    let graph = Graph {
        nodes: vec![plain_request("one"), plain_request("two")],
        edges: vec![],
    };
    let ids = HashSet::from(["one", "two"]);
    let p = project(graph.clone(), vec![]);
    let mut d = draft(&graph, &ids, &p, None);
    for row in d.inputs.iter_mut().filter(|row| row.enabled) {
        row.name = "id".into();
    }
    assert!(
        validate_draft(&graph, &ids, &d)
            .contains(&DraftError::DuplicateName(DraftSection::Arguments))
    );
    let error = materialize_component(&graph, &ids, &d, None).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Arguments: names must be unique.")
    );

    let boundary = Graph {
        nodes: vec![
            plain_request("source"),
            request(
                "target",
                &[("id", Binding::reference("source", "body", ""))],
            ),
        ],
        edges: vec![],
    };
    let ids = HashSet::from(["target"]);
    let p = project(boundary.clone(), vec![]);
    let mut d = draft(&boundary, &ids, &p, None);
    d.inputs
        .iter_mut()
        .find(|row| row.mandatory)
        .unwrap()
        .enabled = false;
    let error = materialize_component(&boundary, &ids, &d, None).unwrap_err();
    assert!(error.to_string().contains("must be public arguments"));
}

#[test]
fn every_draft_problem_is_reported() {
    let graph = Graph {
        nodes: vec![plain_request("one")],
        edges: vec![],
    };
    let none = HashSet::new();
    let p = project(graph.clone(), vec![]);
    let mut d = draft(&graph, &HashSet::from(["one"]), &p, None);
    d.name = " ".into();
    d.action = Some("missing".into());
    d.inputs[0].name = " 1d".into();
    d.outputs[0].name = String::new();
    assert_eq!(
        validate_draft(&graph, &none, &d),
        [
            DraftError::NoBlocks,
            DraftError::MissingName,
            DraftError::RemovedOutputSource,
            DraftError::InvalidName(DraftSection::Arguments),
            DraftError::EmptyName(DraftSection::Outputs),
            DraftError::InvalidName(DraftSection::Outputs),
            DraftError::UnknownAction,
        ]
    );
    d.outputs[0].enabled = false;
    assert!(validate_draft(&graph, &none, &d).contains(&DraftError::NoOutputs));
    let error = EditError::InvalidDraft(vec![DraftError::NoBlocks, DraftError::MissingName]);
    assert_eq!(
        error.to_string(),
        "Select at least one block.\nEnter a component name."
    );
}

#[test]
fn the_action_input_and_contracts_are_saved() {
    let graph = Graph {
        nodes: vec![plain_request("one")],
        edges: vec![],
    };
    let ids = HashSet::from(["one"]);
    let p = project(graph.clone(), vec![]);
    let mut d = draft(&graph, &ids, &p, None);
    d.inputs[0].name = " recordId ".into();
    d.inputs[0].contract_id = Some(String::new());
    d.action = Some(d.inputs[0].key.clone());
    d.action_type = ValueType::Object;
    d.action_contract = Some("Product".into());
    assert!(
        validate_draft(&graph, &ids, &d)
            .contains(&DraftError::InvalidName(DraftSection::Arguments))
    );
    d.inputs[0].name = "recordId".into();
    let result = materialize_component(&graph, &ids, &d, None).unwrap();
    assert_eq!(
        result.action,
        Some(ComponentAction {
            input: "recordId".into(),
            value_type: ValueType::Object,
            contract_id: Some("Product".into()),
        })
    );
    assert_eq!(result.inputs[0].contract_id, None);
    assert_eq!(
        result.inputs[0].default_value,
        OptionalJson::Present(json!("123"))
    );
}

#[test]
fn existing_output_aliases_keep_literal_environment_and_nested_bindings() {
    let graph = Graph {
        nodes: vec![plain_request("read")],
        edges: vec![],
    };
    let initial = Component {
        outputs: vec![
            output(
                "nested",
                ValueType::String,
                Binding::reference("read", "body", "items[0].label"),
            ),
            output("literal", ValueType::Number, Binding::literal(json!(0))),
            output(
                "environment",
                ValueType::String,
                Binding::environment("baseUrl"),
            ),
        ],
        ..component("component", 2, graph.clone())
    };
    let ids = HashSet::from(["read"]);
    let p = project(graph.clone(), vec![initial.clone()]);
    let d = draft(&graph, &ids, &p, Some(&initial));
    let result = materialize_component(&graph, &ids, &d, Some(&initial)).unwrap();
    let bindings = |component: &Component| -> Vec<Binding> {
        component
            .outputs
            .iter()
            .map(|output| output.binding.clone())
            .collect()
    };
    assert_eq!(bindings(&result), bindings(&initial));
}

#[test]
fn project_validation_rejects_a_recursive_definition() {
    let mut nested = create_node(NodeKind::Component, Position { x: 0.0, y: 0.0 }, None);
    nested.id = "self".into();
    nested.config.component_id = Some("recursive".into());
    nested.config.revision = Some(1);
    let graph = Graph {
        nodes: vec![nested],
        edges: vec![],
    };
    let initial = Component {
        outputs: vec![output(
            "value",
            ValueType::Any,
            Binding::reference("self", "value", ""),
        )],
        ..component("recursive", 1, graph.clone())
    };
    let empty = Graph {
        nodes: vec![],
        edges: vec![],
    };
    let p = project(empty, vec![initial.clone()]);
    let ids = HashSet::from(["self"]);
    let d = draft(&graph, &ids, &p, Some(&initial));
    let result = materialize_component(&graph, &ids, &d, Some(&initial)).unwrap();
    let recursive = Project {
        components: vec![result],
        ..p
    };
    let error = crate::validate_project(&recursive).unwrap_err();
    assert!(error.to_string().contains("Recursive"), "{error}");
}

#[test]
fn saving_a_revision_keeps_existing_instances_pinned() {
    let shop = at("shop.json");
    let initial = shop
        .components
        .iter()
        .find(|component| component.id == "get-product")
        .unwrap();
    let ids: HashSet<&str> = initial.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut d = draft(&initial.graph, &ids, &shop, Some(initial));
    d.name = "Get product v2".into();
    let revised = materialize_component(&initial.graph, &ids, &d, Some(initial)).unwrap();
    let saved = add_component(&shop, &revised, Some("get-product")).unwrap();
    let latest = saved.components.last().unwrap();
    assert_eq!((latest.id.as_str(), latest.revision), ("get-product", 2));
    assert_eq!(saved.scenarios, shop.scenarios);
    assert!(matches!(
        add_component(&shop, &revised, Some("missing")),
        Err(EditError::ComponentNotFound)
    ));
    assert!(matches!(
        add_component(&shop, initial, None),
        Err(EditError::InvalidProject(_))
    ));
}

#[test]
fn extraction_maps_expression_leaves_and_keeps_the_operator_tree() {
    let mut project = at("blocks.json");
    let graph = project.scenarios[0].graph.clone();
    let ids = HashSet::from(["name-check"]);
    let rows = initialize_inputs(&graph, &ids, &project, None);
    let draft = ComponentDraft {
        name: "Name".into(),
        inputs: rows.clone(),
        outputs: initialize_outputs(&graph, &ids, &project, None),
        ..draft(&graph, &ids, &project, None)
    };
    let definition = materialize_component(&graph, &ids, &draft, None).unwrap();
    assert!(definition.graph.nodes[0].inputs["actual"].is_expression());
    assert_eq!(definition.inputs.len(), 2);
    let leaf = rows.iter().find(|row| row.enabled).unwrap();
    assert_eq!(
        leaf.label,
        "Nazwa jest niepustym tekstem → Actual value → A → A → A"
    );
    project.components.push(definition.clone());
    let mut instance = create_node(
        NodeKind::Component,
        Position { x: 400.0, y: 400.0 },
        Some(&definition),
    );
    for row in rows.iter().filter(|row| row.enabled) {
        instance
            .inputs
            .insert(row.name.clone(), row.original.clone().unwrap());
    }
    project.scenarios[0].graph = Graph {
        nodes: vec![graph.nodes[0].clone(), instance],
        edges: vec![],
    };
    crate::validate_project(&project).unwrap();
}

#[test]
fn extraction_uses_the_nested_slot_type_instead_of_the_outer_result() {
    let mut project = at("blocks.json");
    let source = project.scenarios[0].graph.nodes[0].clone();
    let mut calculation = create_node(NodeKind::Formula, Position { x: 0.0, y: 0.0 }, None);
    let length = Binding::expression(Operator::Length)
        .replace_at(
            &["left".into()],
            Binding::reference(&source.id, "body", "name"),
        )
        .unwrap();
    calculation.inputs = IndexMap::from([
        ("left".into(), length),
        ("right".into(), Binding::literal(json!(2))),
    ]);
    let graph = Graph {
        nodes: vec![source.clone(), calculation.clone()],
        edges: vec![],
    };
    let ids = HashSet::from([calculation.id.as_str()]);
    let rows = initialize_inputs(&graph, &ids, &project, None);
    let reference = rows
        .iter()
        .find(|row| matches!(row.original, Some(Binding::Reference { .. })))
        .unwrap();
    assert_eq!(reference.value_type, ValueType::Any);
    assert_eq!(reference.name, "left_left");
    let draft = ComponentDraft {
        name: "Length times two".into(),
        ..draft(&graph, &ids, &project, None)
    };
    let component = materialize_component(&graph, &ids, &draft, None).unwrap();
    project.components = vec![component.clone()];
    let mut instance = create_node(
        NodeKind::Component,
        Position { x: 0.0, y: 0.0 },
        Some(&component),
    );
    for row in rows.iter().filter(|row| row.enabled) {
        instance
            .inputs
            .insert(row.name.clone(), row.original.clone().unwrap());
    }
    project.scenarios[0].graph = Graph {
        nodes: vec![source, instance],
        edges: vec![],
    };
    crate::validate_project(&project).unwrap();
}
