//! Graph semantics shared by the editor and the runner: ports, wires, dependencies
//! and new nodes.
//!
//! Data wires are never stored. They are derived from reference leaves in node
//! inputs; `Graph::edges` holds only ordering (control) edges.

use crate::{
    format::{
        Binding, Component, Graph, Node, NodeConfig, OptionalJson, OutputPort, Port, Position,
        Project,
    },
    ids::new_id,
    model::{Branch, BuildType, HttpMethod, NodeKind, Predicate, ValueType},
    operator::Operator,
};
use indexmap::{IndexMap, IndexSet};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Pseudo-ports of control flow. They never appear in documents as data ports.
pub const ORDER_PORT: &str = "__order";
pub const TRUE_PORT: &str = "__true";
pub const FALSE_PORT: &str = "__false";

/// Operator a formula node uses when its configuration names none.
pub const DEFAULT_FORMULA_OPERATOR: Operator = Operator::Multiply;

impl NodeKind {
    pub const ALL: [Self; 10] = [
        Self::Trigger,
        Self::If,
        Self::Variable,
        Self::Request,
        Self::Select,
        Self::Build,
        Self::Formula,
        Self::Assert,
        Self::Condition,
        Self::Component,
    ];

    /// Default node name and library title.
    pub fn label(self) -> &'static str {
        match self {
            Self::Trigger => "Start",
            Self::If => "IF / ELSE",
            Self::Variable => "Variable",
            Self::Request => "HTTP request",
            Self::Select => "Select data",
            Self::Build => "Build data",
            Self::Formula => "Operator",
            Self::Assert => "Comparison",
            Self::Condition => "Data condition",
            Self::Component => "Component",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Trigger => "Runs when you start the test",
            Self::If => "Choose a branch: yes or no",
            Self::Variable => "A named value to use in fields",
            Self::Request => "Any REST API endpoint",
            Self::Select => "Pick a field from a response",
            Self::Build => "Prepare an object or a list",
            Self::Formula => "Maths, logic, text and data",
            Self::Assert => "Check the received result",
            Self::Condition => "Check scenario conditions",
            Self::Component => "Your own set of steps",
        }
    }

    /// Handle of a new block before de-duplication, e.g. `request` or `check_2`.
    pub fn handle_base(self) -> &'static str {
        match self {
            Self::Trigger => "begin",
            Self::If => "branch",
            Self::Variable => "variable",
            Self::Request => "request",
            Self::Select => "selection",
            Self::Build => "data",
            Self::Formula => "result",
            Self::Assert => "check",
            Self::Condition => "guard",
            Self::Component => "component_step",
        }
    }

    /// Nodes that only compute values. Reaching a consumer from Start also activates
    /// these producers, but never a request or component.
    pub fn is_pure_value(self) -> bool {
        matches!(
            self,
            Self::Variable | Self::Select | Self::Build | Self::Formula
        )
    }

    /// Kinds whose `actual`/`expected` inputs are compared by a predicate.
    pub fn is_check(self) -> bool {
        matches!(self, Self::Assert | Self::Condition | Self::If)
    }
}

impl Port {
    pub fn new(name: impl Into<String>, value_type: ValueType) -> Self {
        Self {
            name: name.into(),
            value_type,
            label: None,
            contract_id: None,
            required: None,
            default_value: OptionalJson::Missing,
            exposed: None,
        }
    }

    pub fn labelled(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }

    pub fn is_required(&self) -> bool {
        self.required.unwrap_or(false)
    }

    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn default(&self) -> Option<&Value> {
        match &self.default_value {
            OptionalJson::Present(value) => Some(value),
            OptionalJson::Missing => None,
        }
    }
}

impl From<&OutputPort> for Port {
    fn from(output: &OutputPort) -> Self {
        Self {
            name: output.name.clone(),
            value_type: output.value_type,
            label: output.label.clone(),
            contract_id: output.contract_id.clone(),
            required: output.required,
            default_value: output.default_value.clone(),
            exposed: output.exposed,
        }
    }
}

/// The component revision an instance node is pinned to.
pub fn component_definition<'a>(project: &'a Project, node: &Node) -> Option<&'a Component> {
    let (id, revision) = (node.config.component_id.as_deref()?, node.config.revision?);
    project
        .components
        .iter()
        .find(|component| component.id == id && component.revision == revision)
}

/// The newest revision of every component, in document order of first appearance.
pub fn latest_components(project: &Project) -> Vec<&Component> {
    let mut latest: IndexMap<&str, &Component> = IndexMap::new();
    for component in &project.components {
        let entry = latest.entry(&component.id).or_insert(component);
        if component.revision > entry.revision {
            *entry = component;
        }
    }
    latest.into_values().collect()
}

pub fn formula_operator(node: &Node) -> Operator {
    node.config.operation.unwrap_or(DEFAULT_FORMULA_OPERATOR)
}

pub fn predicate(node: &Node) -> Predicate {
    node.config.predicate.unwrap_or_default()
}

/// Declared inputs of a node, followed by text template aliases.
pub fn input_ports(node: &Node, project: &Project) -> Vec<Port> {
    let mut ports = builtin_input_ports(node, project);
    if let Some(templates) = &node.config.template_inputs {
        for port in templates {
            if !ports.iter().any(|existing| existing.name == port.name) {
                ports.push(port.clone());
            }
        }
    }
    ports
}

fn builtin_input_ports(node: &Node, project: &Project) -> Vec<Port> {
    match node.kind {
        NodeKind::Trigger => vec![],
        NodeKind::Variable => {
            let mut port = Port::new("value", ValueType::Any)
                .labelled("Variable value")
                .required(true);
            port.exposed = Some(true);
            vec![port]
        }
        NodeKind::Component => component_definition(project, node)
            .map(|component| component.inputs.clone())
            .unwrap_or_default(),
        NodeKind::Request => {
            let mut ports = node.config.input_ports.clone().unwrap_or_default();
            if !ports.iter().any(|port| port.name == "body") {
                ports.push(Port::new("body", ValueType::Any).required(false));
            }
            ports
        }
        NodeKind::Select => vec![
            Port::new("value", ValueType::Any)
                .labelled("Data")
                .required(true),
        ],
        NodeKind::Build => node
            .inputs
            .keys()
            .map(|name| Port::new(name.clone(), ValueType::Any).required(true))
            .collect(),
        NodeKind::Formula => formula_operator(node)
            .spec()
            .operands
            .iter()
            .map(|operand| {
                let mut port = Port::new(operand.name, operand.value_type)
                    .labelled(operand.label)
                    .required(operand.required);
                if let Some(default) = operand.default {
                    port.default_value = OptionalJson::Present(default.to_value());
                }
                port
            })
            .collect(),
        NodeKind::Assert | NodeKind::Condition | NodeKind::If => {
            let expected_required =
                !matches!(predicate(node), Predicate::Exists | Predicate::NotEmpty);
            let mut ports = vec![
                Port::new("actual", ValueType::Any)
                    .labelled("Actual value")
                    .required(true),
                Port::new("expected", ValueType::Any)
                    .labelled("Expected value")
                    .required(expected_required),
            ];
            for name in node.inputs.keys() {
                if !ports.iter().any(|port| &port.name == name) {
                    ports.push(Port::new(name.clone(), ValueType::Any));
                }
            }
            ports
        }
    }
}

pub fn output_ports(node: &Node, project: &Project) -> Vec<Port> {
    match node.kind {
        NodeKind::Trigger => vec![],
        NodeKind::Component => component_definition(project, node)
            .map(|component| component.outputs.iter().map(Port::from).collect())
            .unwrap_or_default(),
        NodeKind::Request => {
            let mut body = Port::new("body", ValueType::Any).labelled("Response");
            body.contract_id = node.config.contract_id.clone();
            vec![
                body,
                Port::new("status", ValueType::Number).labelled("HTTP status"),
                Port::new("headers", ValueType::Object).labelled("Headers"),
                Port::new("duration", ValueType::Number).labelled("Duration"),
            ]
        }
        NodeKind::Assert | NodeKind::Condition | NodeKind::If => vec![
            Port::new("value", ValueType::Any),
            Port::new("passed", ValueType::Boolean),
        ],
        NodeKind::Formula => vec![Port::new("value", formula_operator(node).spec().output)],
        NodeKind::Build => vec![Port::new(
            "value",
            match node.config.build_type.unwrap_or_default() {
                BuildType::Array => ValueType::Array,
                BuildType::Object => ValueType::Object,
            },
        )],
        NodeKind::Select | NodeKind::Variable => vec![Port::new("value", ValueType::Any)],
    }
}

/// Whether a value from `source` may feed `target`. A named contract on the target
/// must match exactly; otherwise base types must agree or one side accepts any.
pub fn compatible(source: &Port, target: &Port) -> bool {
    if let Some(contract) = &target.contract_id {
        return source.contract_id.as_ref() == Some(contract);
    }
    source.value_type == ValueType::Any
        || target.value_type == ValueType::Any
        || source.value_type == target.value_type
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireKind {
    Data,
    Order,
}

/// A wire as drawn on the board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wire {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: WireKind,
    pub branch: Option<Branch>,
    /// Output port of a data wire.
    pub source_port: Option<String>,
    /// Input of a data wire.
    pub target_input: Option<String>,
    /// Operand path of the reference inside the input's expression tree.
    pub operand_path: Vec<String>,
}

/// Ordering edges followed by data wires derived from every reference leaf.
pub fn wires(graph: &Graph) -> Vec<Wire> {
    let mut wires: Vec<Wire> = graph
        .edges
        .iter()
        .map(|edge| Wire {
            id: edge.id.clone(),
            source: edge.source.clone(),
            target: edge.target.clone(),
            kind: WireKind::Order,
            branch: edge.branch,
            source_port: None,
            target_input: None,
            operand_path: vec![],
        })
        .collect();
    for node in &graph.nodes {
        for (input, binding) in &node.inputs {
            binding.for_each_leaf(&mut |leaf, path| {
                if let Binding::Reference { node_id, port, .. } = leaf {
                    wires.push(Wire {
                        id: crate::binding::data_edge_id(&node.id, input, path),
                        source: node_id.clone(),
                        target: node.id.clone(),
                        kind: WireKind::Data,
                        branch: None,
                        source_port: Some(port.clone()),
                        target_input: Some(input.clone()),
                        operand_path: path.to_vec(),
                    });
                }
            });
        }
    }
    wires
}

/// Every node a node waits for, through ordering edges or data references, without
/// duplicates.
pub fn dependencies<'a>(graph: &'a Graph, node_id: &str) -> Vec<&'a str> {
    let mut result: IndexSet<&str> = IndexSet::new();
    for edge in graph.edges.iter().filter(|edge| edge.target == node_id) {
        result.insert(&edge.source);
    }
    if let Some(node) = graph.nodes.iter().find(|node| node.id == node_id) {
        for binding in node.inputs.values() {
            for reference in binding.references() {
                result.insert(reference.node_id);
            }
        }
    }
    result.into_iter().collect()
}

/// Dependency lists of every node, built once.
pub fn dependency_index(graph: &Graph) -> HashMap<&str, IndexSet<&str>> {
    let mut index: HashMap<&str, IndexSet<&str>> = graph
        .nodes
        .iter()
        .map(|node| {
            let references = node
                .inputs
                .values()
                .flat_map(Binding::references)
                .map(|reference| reference.node_id)
                .collect();
            (node.id.as_str(), references)
        })
        .collect();
    for edge in &graph.edges {
        if let Some(sources) = index.get_mut(edge.target.as_str()) {
            sources.insert(&edge.source);
        }
    }
    index
}

/// Whether ordering and data dependencies together form a cycle.
pub fn has_cycle(graph: &Graph) -> bool {
    let index = dependency_index(graph);
    let mut done: HashSet<&str> = HashSet::new();
    let mut visiting: HashSet<&str> = HashSet::new();
    fn visit<'a>(
        id: &'a str,
        index: &HashMap<&'a str, IndexSet<&'a str>>,
        visiting: &mut HashSet<&'a str>,
        done: &mut HashSet<&'a str>,
    ) -> bool {
        if done.contains(id) {
            return false;
        }
        if !visiting.insert(id) {
            return true;
        }
        let cyclic = index.get(id).is_some_and(|sources| {
            sources
                .iter()
                .any(|source| visit(source, index, visiting, done))
        });
        visiting.remove(id);
        done.insert(id);
        cyclic
    }
    graph
        .nodes
        .iter()
        .any(|node| visit(&node.id, &index, &mut visiting, &mut done))
}

/// The node and all its transitive dependencies within the graph.
pub fn ancestors_and_self(graph: &Graph, node_id: &str) -> IndexSet<String> {
    let index = dependency_index(graph);
    let mut selected = IndexSet::new();
    let mut stack = vec![node_id];
    while let Some(id) = stack.pop() {
        if selected.insert(id.to_owned())
            && let Some(sources) = index.get(id)
        {
            stack.extend(sources.iter().copied());
        }
    }
    selected
}

/// Removes nodes, their ordering edges and every input that references them.
pub fn delete_nodes(graph: &Graph, ids: &HashSet<&str>) -> Graph {
    Graph {
        nodes: graph
            .nodes
            .iter()
            .filter(|node| !ids.contains(node.id.as_str()))
            .map(|node| {
                let mut node = node.clone();
                node.inputs.retain(|_, binding| {
                    !binding
                        .references()
                        .iter()
                        .any(|reference| ids.contains(reference.node_id))
                });
                node
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .filter(|edge| {
                !ids.contains(edge.source.as_str()) && !ids.contains(edge.target.as_str())
            })
            .cloned()
            .collect(),
    }
}

/// A new node with the default configuration of its kind.
///
/// A component node is pinned to `component`'s revision and starts with literal
/// defaults of its public arguments.
pub fn create_node(kind: NodeKind, position: Position, component: Option<&Component>) -> Node {
    let mut node = Node {
        id: new_id("node"),
        handle: kind.handle_base().to_owned(),
        label: None,
        note: None,
        kind,
        position,
        inputs: IndexMap::new(),
        config: NodeConfig::default(),
    };
    let literal = Binding::literal;
    match kind {
        NodeKind::Request => {
            node.config = NodeConfig {
                method: Some(HttpMethod::Get),
                url: Some("/endpoint".into()),
                headers: Some(IndexMap::new()),
                query: Some(IndexMap::new()),
                timeout_ms: Some(10_000.0),
                input_ports: Some(vec![]),
                ..NodeConfig::default()
            };
        }
        NodeKind::Variable => {
            node.inputs.insert("value".into(), literal(json!("")));
        }
        NodeKind::Select => {
            node.inputs.insert("value".into(), literal(json!({})));
            node.config.path = Some(String::new());
        }
        NodeKind::Build => {
            node.config.build_type = Some(BuildType::Object);
            node.inputs.insert("field".into(), literal(json!("")));
        }
        NodeKind::Formula => {
            node.config.operation = Some(DEFAULT_FORMULA_OPERATOR);
            node.inputs.insert("left".into(), literal(json!(0)));
            node.inputs.insert("right".into(), literal(json!(1)));
        }
        NodeKind::Assert | NodeKind::Condition | NodeKind::If => {
            node.config.predicate = Some(Predicate::Equals);
            node.inputs.insert("actual".into(), literal(Value::Null));
            node.inputs.insert("expected".into(), literal(Value::Null));
        }
        NodeKind::Trigger | NodeKind::Component => {}
    }
    if let Some(component) = component {
        node.handle = crate::handle::handle_base(&component.name);
        node.config = NodeConfig {
            component_id: Some(component.id.clone()),
            revision: Some(component.revision),
            ..NodeConfig::default()
        };
        for port in &component.inputs {
            if let Some(default) = port.default() {
                node.inputs
                    .insert(port.name.clone(), literal(default.clone()));
            }
        }
    }
    node
}

/// A formula node computing `operator`, with the operator's default operands.
pub fn create_operator_node(operator: Operator, position: Position) -> Node {
    let mut node = create_node(NodeKind::Formula, position, None);
    node.handle = crate::handle::handle_base(operator.label());
    node.config.operation = Some(operator);
    if let Binding::Expression { inputs, .. } = Binding::expression(operator) {
        node.inputs = inputs;
    }
    node
}

/// A position to the right of every node, clear of existing hit targets.
pub fn position_after(graph: &Graph, y: f64) -> Position {
    let x = graph
        .nodes
        .iter()
        .map(|node| node.position.x)
        .reduce(f64::max)
        .map_or(100.0, |right| right + 340.0);
    Position { x, y }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::OrderEdge;

    fn project() -> Project {
        let document = crate::import_document(
            include_bytes!("../../../fixtures/projects/blocks.json"),
            &crate::ImportLimits::default(),
        )
        .expect("fixture imports");
        document.workspace().projects[0].clone()
    }

    fn node(id: &str, kind: NodeKind) -> Node {
        let mut node = create_node(kind, Position { x: 0.0, y: 0.0 }, None);
        node.id = id.into();
        node
    }

    fn edge(source: &str, target: &str) -> OrderEdge {
        OrderEdge {
            id: format!("{source}->{target}"),
            source: source.into(),
            target: target.into(),
            branch: None,
        }
    }

    #[test]
    fn new_nodes_have_the_reference_defaults() {
        let request = create_node(NodeKind::Request, Position { x: 1.0, y: 2.0 }, None);
        assert!(request.id.starts_with("node-") && request.id.len() == 13);
        assert_eq!(request.config.url.as_deref(), Some("/endpoint"));
        assert_eq!(request.config.timeout_ms, Some(10_000.0));
        let check = create_node(NodeKind::If, Position { x: 0.0, y: 0.0 }, None);
        assert_eq!(check.inputs["actual"], Binding::literal(Value::Null));
        let operator = create_operator_node(Operator::Round, Position { x: 0.0, y: 0.0 });
        assert_eq!(operator.handle, "round");
        assert_eq!(operator.inputs["right"], Binding::literal(json!(0)));
    }

    #[test]
    fn ports_follow_node_kind_and_configuration() {
        let project = project();
        let mut check = node("c", NodeKind::Assert);
        check.config.predicate = Some(Predicate::Exists);
        let inputs = input_ports(&check, &project);
        assert_eq!(inputs[1].name, "expected");
        assert!(!inputs[1].is_required());

        let mut request = node("r", NodeKind::Request);
        request.config.template_inputs = Some(vec![Port::new("alias", ValueType::Any)]);
        let names: Vec<_> = input_ports(&request, &project)
            .into_iter()
            .map(|port| port.name)
            .collect();
        assert_eq!(names, ["body", "alias"]);

        let mut build = node("b", NodeKind::Build);
        build.config.build_type = Some(BuildType::Array);
        assert_eq!(
            output_ports(&build, &project)[0].value_type,
            ValueType::Array
        );
        let mut formula = node("f", NodeKind::Formula);
        formula.config.operation = Some(Operator::Equals);
        assert_eq!(
            output_ports(&formula, &project)[0].value_type,
            ValueType::Boolean
        );
    }

    #[test]
    fn wires_include_nested_references_with_distinct_ids() {
        let mut graph = Graph {
            nodes: vec![node("a", NodeKind::Request), node("b", NodeKind::Assert)],
            edges: vec![edge("a", "b")],
        };
        let expression = Binding::expression(Operator::Equals)
            .replace_at(&["left".into()], Binding::reference("a", "status", ""))
            .unwrap()
            .replace_at(&["right".into()], Binding::reference("a", "body", "x"))
            .unwrap();
        graph.nodes[1].inputs.insert("actual".into(), expression);
        let wires = wires(&graph);
        assert_eq!(wires.len(), 3);
        assert_eq!(wires[1].id, "data:b:actual:left");
        assert_eq!(wires[2].id, "data:b:actual:right");
        assert_eq!(dependencies(&graph, "b"), ["a"]);
    }

    #[test]
    fn cycles_combine_ordering_and_data_dependencies() {
        let mut graph = Graph {
            nodes: vec![node("a", NodeKind::Variable), node("b", NodeKind::Variable)],
            edges: vec![edge("a", "b")],
        };
        assert!(!has_cycle(&graph));
        graph.nodes[0]
            .inputs
            .insert("value".into(), Binding::reference("b", "value", ""));
        assert!(has_cycle(&graph));
    }

    #[test]
    fn deleting_nodes_removes_dependent_inputs_and_edges() {
        let mut graph = Graph {
            nodes: vec![node("a", NodeKind::Request), node("b", NodeKind::Variable)],
            edges: vec![edge("a", "b")],
        };
        graph.nodes[1]
            .inputs
            .insert("value".into(), Binding::reference("a", "body", ""));
        let deleted = delete_nodes(&graph, &HashSet::from(["a"]));
        assert_eq!(deleted.nodes.len(), 1);
        assert!(deleted.nodes[0].inputs.is_empty());
        assert!(deleted.edges.is_empty());
    }

    #[test]
    fn ancestors_follow_both_dependency_kinds() {
        let mut graph = Graph {
            nodes: vec![
                node("a", NodeKind::Request),
                node("b", NodeKind::Variable),
                node("c", NodeKind::Assert),
                node("d", NodeKind::Request),
            ],
            edges: vec![edge("a", "b")],
        };
        graph.nodes[2]
            .inputs
            .insert("actual".into(), Binding::reference("b", "value", ""));
        let selected = ancestors_and_self(&graph, "c");
        assert_eq!(selected.len(), 3);
        assert!(!selected.contains("d"));
    }

    #[test]
    fn contracts_restrict_compatibility() {
        let mut target = Port::new("x", ValueType::Object);
        assert!(compatible(&Port::new("y", ValueType::Any), &target));
        assert!(!compatible(&Port::new("y", ValueType::Number), &target));
        target.contract_id = Some("cart".into());
        assert!(!compatible(&Port::new("y", ValueType::Object), &target));
    }
}
