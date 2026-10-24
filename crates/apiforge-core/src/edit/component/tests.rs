use super::*;
use crate::{
    edit::test_support::at,
    format::{Environment, Node, OrderEdge, Position, Scenario},
    graph::{create_node, create_value_node},
    model::{ErrorPolicy, NodeKind},
    operator::Operator,
};
use indexmap::IndexMap;
use serde_json::json;

/// A request whose `body` takes the binding given as `id`.
fn request(id: &str, inputs: &[(&str, Binding)]) -> Node {
    let mut node = create_node(NodeKind::Request, Position { x: 0.0, y: 0.0 }, None);
    node.id = id.into();
    node.handle = id.into();
    for (name, binding) in inputs {
        let name = if *name == "id" { "body" } else { name };
        node.inputs.insert(name.to_owned(), binding.clone());
    }
    node
}

/// The draft row of the first block's `body`, enabled as an argument.
fn body_row(draft: &mut ComponentDraft) -> &mut InputRow {
    let row = draft
        .inputs
        .iter_mut()
        .find(|row| row.input == "body")
        .expect("a body row");
    row.enabled = true;
    row
}

fn plain_request(id: &str) -> Node {
    request(id, &[("id", Binding::literal(json!("123")))])
}

fn project(graph: Graph, components: Vec<Component>) -> Project {
    Project {
        version: crate::format::FORMAT_VERSION,
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
                proxies: Vec::new(),
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

/// An Export block named `name` passing `binding` on.
fn output(name: &str, value_type: ValueType, binding: Binding) -> Node {
    let mut export = create_node(NodeKind::Export, Position { x: 0.0, y: 0.0 }, None);
    export.id = format!("export-{name}");
    export.handle = name.into();
    export.config.output_type = Some(value_type);
    export.inputs.insert("value".into(), binding);
    export
}

/// `component` with `exports` added to its graph.
fn with_outputs(mut component: Component, exports: Vec<Node>) -> Component {
    component.graph.nodes.extend(exports);
    component
}

/// The Export blocks of a component, in graph order.
fn exports(component: &Component) -> Vec<&Node> {
    component
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Export)
        .collect()
}

#[test]
fn saving_a_revision_uses_exports_edited_on_the_board_and_keeps_their_identity() {
    let initial = with_outputs(
        component(
            "reusable",
            1,
            Graph {
                nodes: vec![plain_request("read")],
                edges: vec![],
                proxies: Vec::new(),
            },
        ),
        vec![output(
            "result",
            ValueType::Any,
            Binding::reference("read", "body", ""),
        )],
    );
    let mut edited = initial.graph.clone();
    let existing = edited.nodes.last_mut().unwrap();
    existing.handle = "renamed_result".into();
    existing
        .inputs
        .insert("value".into(), Binding::literal(json!(42)));
    existing.note = Some("Export edited on the board".into());
    existing.position = Position { x: 830.0, y: 240.0 };
    existing.auto_placed = true;
    let mut extra = output("extra", ValueType::String, Binding::literal(json!("added")));
    extra.note = Some("A new public output".into());
    edited
        .edges
        .push(edge("export-order", "read", &existing.id));
    edited.nodes.push(extra);
    let ids = edited.nodes.iter().map(|n| n.id.as_str()).collect();
    let project = project(
        Graph {
            nodes: vec![],
            edges: vec![],
            proxies: Vec::new(),
        },
        vec![initial.clone()],
    );
    let draft = draft(&edited, &ids, &project, Some(&initial));
    assert_eq!(
        draft
            .outputs
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>(),
        ["renamed_result", "extra"]
    );
    let result = materialize_component(&edited, &ids, &draft, Some(&initial)).unwrap();
    assert_eq!(
        result.graph, edited,
        "saving must preserve export layout, notes, identities and ordering"
    );
    assert_eq!(initial.graph.nodes.last().unwrap().handle, "result");
}

#[test]
fn deleted_or_disabled_exports_are_not_resurrected_and_leave_no_dangling_edges() {
    let initial = with_outputs(
        component(
            "reusable",
            1,
            Graph {
                nodes: vec![plain_request("read")],
                edges: vec![],
                proxies: Vec::new(),
            },
        ),
        vec![
            output(
                "first",
                ValueType::Any,
                Binding::reference("read", "body", ""),
            ),
            output(
                "second",
                ValueType::Any,
                Binding::reference("read", "status", ""),
            ),
        ],
    );
    let project = project(
        Graph {
            nodes: vec![],
            edges: vec![],
            proxies: Vec::new(),
        },
        vec![initial.clone()],
    );
    for deleted_on_board in [false, true] {
        let mut graph = initial.graph.clone();
        if deleted_on_board {
            graph.nodes.retain(|node| node.handle != "first");
        } else {
            graph.edges.push(edge("to-first", "read", "export-first"));
        }
        graph.edges.push(edge("to-second", "read", "export-second"));
        let ids = graph.nodes.iter().map(|n| n.id.as_str()).collect();
        let mut draft = draft(&graph, &ids, &project, Some(&initial));
        if !deleted_on_board {
            draft.outputs[0].enabled = false;
        }
        let result = materialize_component(&graph, &ids, &draft, Some(&initial)).unwrap();
        assert_eq!(
            exports(&result)
                .iter()
                .map(|n| n.handle.as_str())
                .collect::<Vec<_>>(),
            ["second"]
        );
        assert_eq!(
            result.graph.edges,
            [edge("to-second", "read", "export-second")]
        );
        crate::validate_project(&Project {
            components: vec![result],
            ..project.clone()
        })
        .unwrap();
    }
}

#[test]
fn export_ordering_branches_and_explicit_public_arguments_survive_revision() {
    let mut branch = create_node(NodeKind::If, Position { x: 0.0, y: 0.0 }, None);
    branch.id = "branch".into();
    branch.handle = "choice".into();
    let mut yes = output("yes", ValueType::Boolean, Binding::literal(json!(true)));
    yes.note = Some("Only the true branch publishes this".into());
    let mut order = edge("true-order", "branch", &yes.id);
    order.branch = Some(crate::model::Branch::True);
    let initial = component(
        "reusable",
        1,
        Graph {
            nodes: vec![branch, yes],
            edges: vec![order],
            proxies: Vec::new(),
        },
    );
    let graph = &initial.graph;
    let ids = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let project = project(
        Graph {
            nodes: vec![],
            edges: vec![],
            proxies: Vec::new(),
        },
        vec![initial.clone()],
    );
    let mut draft = draft(graph, &ids, &project, Some(&initial));
    let output_value = draft
        .inputs
        .iter_mut()
        .find(|row| row.node_id == "export-yes")
        .unwrap();
    assert!(
        !output_value.enabled,
        "literal exports must not become arguments automatically"
    );
    output_value.enabled = true;
    output_value.name = "published".into();
    let result = materialize_component(graph, &ids, &draft, Some(&initial)).unwrap();
    assert_eq!(
        exports(&result)[0].inputs["value"],
        Binding::parameter("published")
    );
    assert_eq!(exports(&result)[0].note, exports(&initial)[0].note);
    assert_eq!(result.graph.edges, initial.graph.edges);
    crate::validate_project(&Project {
        components: vec![result],
        ..project
    })
    .unwrap();
}

fn component(id: &str, revision: u32, graph: Graph) -> Component {
    Component {
        id: id.into(),
        revision,
        handle: "read".into(),
        name: "Read".into(),
        description: String::new(),
        inputs: vec![],
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
    let mut select = create_value_node(
        Binding::reference("get", "body", "data.id"),
        Position { x: 0.0, y: 0.0 },
    );
    select.id = "select".into();
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
        proxies: Vec::new(),
    };
    let before = graph.clone();
    let ids = HashSet::from(["get", "select"]);
    let p = project(graph.clone(), vec![]);
    let mut d = draft(&graph, &ids, &p, None);
    let argument = d.inputs.iter_mut().find(|row| row.input == "body").unwrap();
    assert!(argument.mandatory && argument.enabled);
    assert_eq!(argument.label, "get → Body");
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
        result.graph.nodes[0].inputs["body"],
        Binding::parameter("recordId")
    );
    assert_eq!(
        result.graph.nodes[1].inputs["value"],
        Binding::reference("get", "body", "data.id")
    );
    assert_eq!(result.graph.edges, vec![edge("order", "get", "select")]);
    assert_eq!(
        exports(&result)[0].inputs["value"],
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
        proxies: Vec::new(),
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
    let initial = with_outputs(
        Component {
            name: "Read twice".into(),
            description: "Description".into(),
            inputs: vec![public],
            ..component("reusable", 3, graph.clone())
        },
        vec![record],
    );
    let graph = initial.graph.clone();
    let ids = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let p = project(graph.clone(), vec![initial.clone()]);
    let mut d = draft(&graph, &ids, &p, Some(&initial));
    assert_eq!(d.inputs.iter().filter(|row| row.enabled).count(), 1);
    assert_eq!(body_row(&mut d).targets.len(), 2);
    assert_eq!(body_row(&mut d).default_value, Some(Value::Null));
    body_row(&mut d).name = "targetId".into();
    let result = materialize_component(&graph, &ids, &d, Some(&initial)).unwrap();
    assert_eq!((result.id.as_str(), result.revision), ("reusable", 3));
    assert_eq!(
        result.inputs[0].default_value,
        OptionalJson::Present(Value::Null)
    );
    assert_eq!(result.inputs[0].label.as_deref(), Some("Record identifier"));
    assert_eq!(
        exports(&result)[0].label.as_deref(),
        Some("Selected record")
    );
    for node in result
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Request)
    {
        assert_eq!(node.inputs["body"], Binding::parameter("targetId"));
    }
    assert_eq!(
        exports(&result)[0].inputs["value"],
        Binding::reference("two", "body", "data.record")
    );
    assert_eq!(initial.graph.nodes[0].inputs["body"], parameter);
}

#[test]
fn a_direct_parameter_output_follows_the_argument_rename() {
    let graph = Graph {
        nodes: vec![plain_request("request")],
        edges: vec![],
        proxies: Vec::new(),
    };
    let mut context = Port::new("context", ValueType::Object).required(false);
    context.default_value = OptionalJson::Present(json!({"source": "manual"}));
    let initial = with_outputs(
        Component {
            inputs: vec![context],
            ..component("reusable", 1, graph.clone())
        },
        vec![
            output("context", ValueType::Object, Binding::parameter("context")),
            output(
                "body",
                ValueType::Any,
                Binding::reference("request", "body", ""),
            ),
        ],
    );
    let graph = initial.graph.clone();
    let ids = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let p = project(graph.clone(), vec![initial.clone()]);
    let mut d = draft(&graph, &ids, &p, Some(&initial));
    let row = d
        .inputs
        .iter_mut()
        .find(|row| row.name == "context")
        .unwrap();
    assert_eq!(
        row.targets.len(),
        1,
        "the parameter is used directly by an Export"
    );
    assert_eq!(row.targets[0].node_id, "export-context");
    row.name = "metadata".into();
    let result = materialize_component(&graph, &ids, &d, Some(&initial)).unwrap();
    assert_eq!(
        exports(&result)[0].inputs["value"],
        Binding::parameter("metadata")
    );
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
        proxies: Vec::new(),
    };
    let ids = HashSet::from(["one", "two"]);
    let p = project(graph.clone(), vec![]);
    let mut d = draft(&graph, &ids, &p, None);
    for row in d.inputs.iter_mut().filter(|row| row.input == "body") {
        row.enabled = true;
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
        proxies: Vec::new(),
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
        proxies: Vec::new(),
    };
    let none = HashSet::new();
    let p = project(graph.clone(), vec![]);
    let mut d = draft(&graph, &HashSet::from(["one"]), &p, None);
    d.name = " ".into();
    d.action = Some("missing".into());
    body_row(&mut d).name = " 1d".into();
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
        proxies: Vec::new(),
    };
    let ids = HashSet::from(["one"]);
    let p = project(graph.clone(), vec![]);
    let mut d = draft(&graph, &ids, &p, None);
    body_row(&mut d).name = " recordId ".into();
    body_row(&mut d).contract_id = Some(String::new());
    d.action = Some(body_row(&mut d).key.clone());
    d.action_type = ValueType::Object;
    d.action_contract = Some("Product".into());
    assert!(
        validate_draft(&graph, &ids, &d)
            .contains(&DraftError::InvalidName(DraftSection::Arguments))
    );
    body_row(&mut d).name = "recordId".into();
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
        proxies: Vec::new(),
    };
    let initial = with_outputs(
        Component {
            ..component("component", 2, graph.clone())
        },
        vec![
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
    );
    let graph = initial.graph.clone();
    let ids = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let p = project(graph.clone(), vec![initial.clone()]);
    let d = draft(&graph, &ids, &p, Some(&initial));
    let result = materialize_component(&graph, &ids, &d, Some(&initial)).unwrap();
    let bindings = |component: &Component| -> Vec<Binding> {
        exports(component)
            .iter()
            .map(|export| export.inputs["value"].clone())
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
        proxies: Vec::new(),
    };
    let initial = with_outputs(
        Component {
            ..component("recursive", 1, graph.clone())
        },
        vec![output(
            "value",
            ValueType::Any,
            Binding::reference("self", "value", ""),
        )],
    );
    let empty = Graph {
        nodes: vec![],
        edges: vec![],
        proxies: Vec::new(),
    };
    let p = project(empty, vec![initial.clone()]);
    let graph = initial.graph.clone();
    let ids = graph.nodes.iter().map(|n| n.id.as_str()).collect();
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
        proxies: Vec::new(),
    };
    crate::validate_project(&project).unwrap();
}

#[test]
fn extraction_uses_the_nested_slot_type_instead_of_the_outer_result() {
    let mut project = at("blocks.json");
    let source = project.scenarios[0].graph.nodes[0].clone();
    let length = Binding::expression(Operator::Length)
        .replace_at(
            &["left".into()],
            Binding::reference(&source.id, "body", "name"),
        )
        .unwrap();
    let calculation = create_value_node(
        Binding::Expression {
            operator: Operator::Multiply,
            inputs: IndexMap::from([
                ("left".into(), length),
                ("right".into(), Binding::literal(json!(2))),
            ]),
        },
        Position { x: 0.0, y: 0.0 },
    );
    let graph = Graph {
        nodes: vec![source.clone(), calculation.clone()],
        edges: vec![],
        proxies: Vec::new(),
    };
    let ids = HashSet::from([calculation.id.as_str()]);
    let rows = initialize_inputs(&graph, &ids, &project, None);
    let reference = rows
        .iter()
        .find(|row| matches!(row.original, Some(Binding::Reference { .. })))
        .unwrap();
    assert_eq!(reference.value_type, ValueType::Any);
    assert_eq!(reference.name, "value_left_left");
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
        proxies: Vec::new(),
    };
    crate::validate_project(&project).unwrap();
}
