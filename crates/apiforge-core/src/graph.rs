//! Graph semantics shared by the editor and the runner: ports, wires, dependencies
//! and new nodes.
//!
//! Data wires are never stored. They are derived from reference leaves in node
//! inputs; `Graph::edges` holds only ordering (control) edges.

use crate::{
    format::{
        Binding, Component, ConditionPredicate, Graph, Node, NodeConfig, OptionalJson, Port,
        Position, Project,
    },
    ids::new_id,
    model::{Branch, HttpMethod, NodeKind, Predicate, ValueType},
    operator::{OperandSpec, Operator},
};
use indexmap::{IndexMap, IndexSet};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Pseudo-ports of control flow. They never appear in documents as data ports.
pub const ORDER_PORT: &str = "__order";
pub const TRUE_PORT: &str = "__true";
pub const FALSE_PORT: &str = "__false";

/// The input of a Value block holding its expression.
pub const VALUE_INPUT: &str = "value";
/// Inputs of a request: its URL template, header and query objects and body.
pub const URL_INPUT: &str = "url";
pub const HEADERS_INPUT: &str = "headers";
pub const QUERY_INPUT: &str = "query";
pub const BODY_INPUT: &str = "body";

impl NodeKind {
    pub const ALL: [Self; 8] = [
        Self::Trigger,
        Self::If,
        Self::Request,
        Self::Value,
        Self::Assert,
        Self::Condition,
        Self::Component,
        Self::Export,
    ];

    /// Default node name and library title.
    pub fn label(self) -> &'static str {
        match self {
            Self::Trigger => "Start",
            Self::If => "IF / ELSE",
            Self::Request => "HTTP request",
            Self::Value => "Value",
            Self::Assert => "Comparison",
            Self::Condition => "Data condition",
            Self::Component => "Component",
            Self::Export => "Export",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Trigger => "Runs when you start the test",
            Self::If => "Choose a branch: yes or no",
            Self::Request => "Any REST API endpoint",
            Self::Value => "Data, a selected field or a calculation",
            Self::Assert => "Check the received result",
            Self::Condition => "Check scenario conditions",
            Self::Component => "Your own set of steps",
            Self::Export => "An output of this component",
        }
    }

    /// Handle of a new block before de-duplication, e.g. `request` or `check_2`.
    pub fn handle_base(self) -> &'static str {
        match self {
            Self::Trigger => "begin",
            Self::If => "branch",
            Self::Request => "request",
            Self::Value => "value",
            Self::Assert => "check",
            Self::Condition => "guard",
            Self::Component => "component_step",
            Self::Export => "output",
        }
    }

    /// Nodes that only compute values. Reaching a consumer from Start also activates
    /// these producers, but never a request or component.
    pub fn is_pure_value(self) -> bool {
        self == Self::Value
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

/// The outputs of a component: one per Export node of its graph, in graph order,
/// named by the Export's handle.
pub fn component_outputs(component: &Component) -> Vec<Port> {
    component
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Export)
        .map(|node| {
            let mut port = Port::new(
                node.handle.clone(),
                node.config.output_type.unwrap_or(ValueType::Any),
            );
            port.label = node.label.clone();
            port.contract_id = node.config.contract_id.clone();
            port
        })
        .collect()
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

pub fn predicate(node: &Node) -> Predicate {
    node.config.predicate.unwrap_or_default()
}

/// The inputs check `index` (from 1) of a Data condition compares. A condition's
/// optional `value` input is the data it passes on; without it, `actual1`.
pub fn condition_inputs(index: usize) -> (String, String) {
    (format!("actual{index}"), format!("expected{index}"))
}

/// The checks of a Data condition; at least one.
pub fn condition_predicates(node: &Node) -> Vec<Predicate> {
    match node.config.predicates.as_deref() {
        Some(predicates) if !predicates.is_empty() => {
            predicates.iter().map(|check| check.predicate).collect()
        }
        _ => vec![Predicate::Equals],
    }
}

/// The inputs a node declares.
pub fn input_ports(node: &Node, project: &Project) -> Vec<Port> {
    match node.kind {
        NodeKind::Trigger => vec![],
        NodeKind::Export => {
            let mut port = Port::new(
                VALUE_INPUT,
                node.config.output_type.unwrap_or(ValueType::Any),
            )
            .labelled("Output value");
            port.contract_id = node.config.contract_id.clone();
            port.exposed = Some(true);
            vec![port]
        }
        NodeKind::Value => {
            // A missing result is not an error: dependants report their own input.
            let mut port = Port::new(VALUE_INPUT, ValueType::Any)
                .labelled("Value")
                .required(false);
            port.exposed = Some(true);
            vec![port]
        }
        NodeKind::Component => component_definition(project, node)
            .map(|component| component.inputs.clone())
            .unwrap_or_default(),
        NodeKind::Request => {
            let mut ports = vec![
                Port::new(URL_INPUT, ValueType::String)
                    .labelled("URL")
                    .required(true),
            ];
            // Headers and query appear once the request has some.
            for (name, label) in [(HEADERS_INPUT, "Headers"), (QUERY_INPUT, "Query")] {
                if node.inputs.contains_key(name) {
                    ports.push(Port::new(name, ValueType::Object).labelled(label));
                }
            }
            ports.push(Port::new(BODY_INPUT, ValueType::Any).labelled("Body"));
            ports
        }
        NodeKind::Assert | NodeKind::If => vec![
            Port::new("actual", ValueType::Any)
                .labelled("Actual value")
                .required(true),
            Port::new("expected", ValueType::Any)
                .labelled("Expected value")
                .required(predicate(node).takes_expected()),
        ],
        NodeKind::Condition => {
            let checks = condition_predicates(node).into_iter().enumerate().flat_map(
                |(index, predicate)| {
                    let (actual, expected) = condition_inputs(index + 1);
                    [
                        Port::new(actual, ValueType::Any)
                            .labelled(format!("Value {}", index + 1))
                            .required(true),
                        Port::new(expected, ValueType::Any)
                            .labelled(format!("Expected {}", index + 1))
                            .required(predicate.takes_expected()),
                    ]
                },
            );
            // The data the condition guards; its output when the checks hold.
            let guarded = node
                .inputs
                .contains_key(VALUE_INPUT)
                .then(|| Port::new(VALUE_INPUT, ValueType::Any).labelled("Data passed on"));
            guarded.into_iter().chain(checks).collect()
        }
    }
}

pub fn output_ports(node: &Node, project: &Project) -> Vec<Port> {
    match node.kind {
        NodeKind::Trigger | NodeKind::Export => vec![],
        NodeKind::Component => component_definition(project, node)
            .map(component_outputs)
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
        NodeKind::Value => vec![Port::new(
            "value",
            node.inputs
                .get(VALUE_INPUT)
                .map_or(ValueType::Any, binding_type),
        )],
    }
}

/// The type a binding is known to produce without running it.
pub fn binding_type(binding: &Binding) -> ValueType {
    match binding {
        Binding::Expression { operator, .. } => operator.spec().output,
        Binding::Literal { value } => match value {
            Value::Bool(_) => ValueType::Boolean,
            Value::Number(_) => ValueType::Number,
            Value::String(_) => ValueType::String,
            Value::Array(_) => ValueType::Array,
            Value::Object(_) => ValueType::Object,
            Value::Null => ValueType::Any,
        },
        Binding::Object { .. } => ValueType::Object,
        Binding::Array { .. } => ValueType::Array,
        Binding::Template { .. } => ValueType::String,
        Binding::Reference { .. } | Binding::Parameter { .. } | Binding::Environment { .. } => {
            ValueType::Any
        }
    }
}

/// The port describing one operand of an operator.
pub fn operand_port(operand: &OperandSpec) -> Port {
    let mut port = Port::new(operand.name, operand.value_type)
        .labelled(operand.label)
        .required(operand.required);
    if let Some(default) = operand.default {
        port.default_value = OptionalJson::Present(default.to_value());
    }
    port
}

/// A place in a node's inputs that a wire or an inline field can fill: a whole
/// input, or a direct child of a Value's expression, object or list.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub input: String,
    /// Path inside the input's binding (see [`Binding::at`]); empty for the input.
    pub path: Vec<String>,
    pub port: Port,
}

impl Slot {
    /// Whether a reference at `path` inside `input` belongs to this slot.
    pub fn contains(&self, input: &str, path: &[String]) -> bool {
        self.input == input && path.starts_with(&self.path)
    }
}

/// The slots of a node, in display order.
///
/// A Value whose expression is an operator, object or list shows one slot per
/// operand, field or item, so each can be wired and edited on its own. Every other
/// node shows one slot per input port.
pub fn input_slots(node: &Node, project: &Project) -> Vec<Slot> {
    let whole = |port: Port| Slot {
        input: port.name.clone(),
        path: Vec::new(),
        port,
    };
    if node.kind != NodeKind::Value {
        return input_ports(node, project).into_iter().map(whole).collect();
    }
    let slot = |segment: String, port: Port| Slot {
        input: VALUE_INPUT.to_owned(),
        path: vec![segment],
        port,
    };
    match node.inputs.get(VALUE_INPUT) {
        Some(Binding::Expression { operator, .. }) => operator
            .spec()
            .operands
            .iter()
            .map(|operand| slot(operand.name.to_owned(), operand_port(operand)))
            .collect(),
        Some(Binding::Object { fields }) => fields
            .keys()
            .map(|name| slot(name.clone(), Port::new(name.clone(), ValueType::Any)))
            .collect(),
        Some(Binding::Array { items }) => (0..items.len())
            .map(|index| {
                let port = Port::new(index.to_string(), ValueType::Any)
                    .labelled(format!("Item {}", index + 1));
                slot(index.to_string(), port)
            })
            .collect(),
        _ => input_ports(node, project).into_iter().map(whole).collect(),
    }
}

/// The slot that shows a reference at `path` inside `input`: the deepest slot
/// containing it.
pub fn slot_of<'a>(slots: &'a [Slot], input: &str, path: &[String]) -> Option<&'a Slot> {
    slots
        .iter()
        .filter(|slot| slot.contains(input, path))
        .max_by_key(|slot| slot.path.len())
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
                timeout_ms: Some(10_000.0),
                ..NodeConfig::default()
            };
            node.inputs
                .insert(URL_INPUT.into(), literal(json!("/endpoint")));
        }
        NodeKind::Value => {
            node.inputs.insert(VALUE_INPUT.into(), literal(json!("")));
        }
        NodeKind::Assert | NodeKind::If => {
            node.config.predicate = Some(Predicate::Equals);
            node.inputs.insert("actual".into(), literal(Value::Null));
            node.inputs.insert("expected".into(), literal(Value::Null));
        }
        NodeKind::Condition => {
            node.config.predicates = Some(vec![ConditionPredicate {
                predicate: Predicate::Equals,
            }]);
            let (actual, expected) = condition_inputs(1);
            node.inputs.insert(actual, literal(Value::Null));
            node.inputs.insert(expected, literal(Value::Null));
        }
        NodeKind::Trigger | NodeKind::Component | NodeKind::Export => {}
    }
    if let Some(component) = component {
        node.handle = component.handle.clone();
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

/// A Value block computing `binding`.
pub fn create_value_node(binding: Binding, position: Position) -> Node {
    let mut node = create_node(NodeKind::Value, position, None);
    node.inputs.insert(VALUE_INPUT.into(), binding);
    node
}

/// A Value block computing `operator`, with the operator's default operands.
pub fn create_operator_node(operator: Operator, position: Position) -> Node {
    let mut node = create_value_node(Binding::expression(operator), position);
    node.handle = crate::handle::handle_base(operator.label());
    node
}

/// Where the `index`-th Export block of a component goes: in a column to the right
/// of `nodes`.
pub fn export_position(nodes: &[Node], index: usize) -> Position {
    let x = nodes
        .iter()
        .map(|node| node.position.x)
        .reduce(f64::max)
        .map_or(100.0, |right| right + 340.0);
    Position {
        x,
        y: 80.0 + index as f64 * 140.0,
    }
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
        assert_eq!(
            request.inputs[URL_INPUT],
            Binding::literal(json!("/endpoint"))
        );
        assert_eq!(request.config.timeout_ms, Some(10_000.0));
        let check = create_node(NodeKind::If, Position { x: 0.0, y: 0.0 }, None);
        assert_eq!(check.inputs["actual"], Binding::literal(Value::Null));
        let operator = create_operator_node(Operator::Round, Position { x: 0.0, y: 0.0 });
        assert_eq!(operator.handle, "round");
        assert_eq!(
            operator.inputs[VALUE_INPUT].at(&["right".into()]),
            Some(&Binding::literal(json!(0)))
        );
    }

    #[test]
    fn ports_follow_node_kind_and_configuration() {
        let project = project();
        let mut check = node("c", NodeKind::Assert);
        check.config.predicate = Some(Predicate::Exists);
        let inputs = input_ports(&check, &project);
        assert_eq!(inputs[1].name, "expected");
        assert!(!inputs[1].is_required());

        let names = |node: &Node| -> Vec<String> {
            input_ports(node, &project)
                .into_iter()
                .map(|port| port.name)
                .collect()
        };
        let mut request = node("r", NodeKind::Request);
        assert_eq!(names(&request), ["url", "body"]);
        request.inputs.insert(
            QUERY_INPUT.into(),
            Binding::Object {
                fields: IndexMap::new(),
            },
        );
        assert_eq!(names(&request), ["url", "query", "body"]);

        let mut condition = create_node(NodeKind::Condition, Position { x: 0.0, y: 0.0 }, None);
        assert_eq!(names(&condition), ["actual1", "expected1"]);
        condition.config.predicates = Some(vec![
            ConditionPredicate {
                predicate: Predicate::Equals,
            },
            ConditionPredicate {
                predicate: Predicate::Exists,
            },
        ]);
        let ports = input_ports(&condition, &project);
        assert_eq!(ports[2].name, "actual2");
        assert!(!ports[3].is_required(), "exists takes no expected value");

        let value_type = |binding: Binding| {
            let value = create_value_node(binding, Position { x: 0.0, y: 0.0 });
            output_ports(&value, &project)[0].value_type
        };
        assert_eq!(
            value_type(Binding::Array { items: vec![] }),
            ValueType::Array
        );
        assert_eq!(
            value_type(Binding::expression(Operator::Equals)),
            ValueType::Boolean
        );
        assert_eq!(value_type(Binding::literal(json!("x"))), ValueType::String);
        assert_eq!(value_type(Binding::parameter("p")), ValueType::Any);
    }

    #[test]
    fn values_show_one_slot_per_operand_field_or_item() {
        let project = project();
        let slots = |binding: Binding| -> Vec<(String, Vec<String>)> {
            let value = create_value_node(binding, Position { x: 0.0, y: 0.0 });
            input_slots(&value, &project)
                .into_iter()
                .map(|slot| (slot.input, slot.path))
                .collect()
        };
        let path = |segment: &str| vec![segment.to_owned()];
        assert_eq!(
            slots(Binding::expression(Operator::Multiply)),
            [
                ("value".into(), path("left")),
                ("value".into(), path("right"))
            ]
        );
        let object = Binding::Object {
            fields: IndexMap::from([("id".to_owned(), Binding::literal(json!(1)))]),
        };
        assert_eq!(slots(object), [("value".into(), path("id"))]);
        assert_eq!(
            slots(Binding::reference("a", "body", "")),
            [("value".into(), vec![])]
        );
        let request = node("r", NodeKind::Request);
        let all = input_slots(&request, &project);
        assert!(all.iter().all(|slot| slot.path.is_empty()));
        let nested = Slot {
            input: "value".into(),
            path: path("left"),
            port: Port::new("left", ValueType::Any),
        };
        let whole = Slot {
            path: vec![],
            ..nested.clone()
        };
        let both = [whole, nested];
        let deep = ["left".to_owned(), "right".to_owned()];
        assert_eq!(slot_of(&both, "value", &deep).unwrap().path, path("left"));
        assert_eq!(
            slot_of(&both, "value", &[]).unwrap().path,
            Vec::<String>::new()
        );
        assert!(slot_of(&both, "other", &[]).is_none());
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
            nodes: vec![node("a", NodeKind::Value), node("b", NodeKind::Value)],
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
            nodes: vec![node("a", NodeKind::Request), node("b", NodeKind::Value)],
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
                node("b", NodeKind::Value),
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
