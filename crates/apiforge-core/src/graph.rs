//! Graph semantics shared by the editor and the runner: ports, wires, dependencies
//! and new nodes.
//!
//! Data wires are never stored. They are derived from reference leaves in node
//! inputs; `Graph::edges` holds only ordering (control) edges.

use crate::{
    format::{
        Binding, Component, ConditionPredicate, Graph, Node, NodeConfig, OptionalJson, Port,
        Position, Project, ProxySource,
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
        Self::Start,
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
            Self::Start => "Start",
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
            Self::Start => "Runs when you start the test",
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
            Self::Start => "begin",
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

    /// Whether a value must be bound, as a built-in port declares it: only an
    /// explicit `required(true)`. Component inputs are read by
    /// [`Port::is_required_input`] instead, and a call's ports carry its result.
    pub fn is_required(&self) -> bool {
        self.required.unwrap_or(false)
    }

    /// Whether a declared component input is required (KTLO-32, the owner's
    /// decision on #62): `@required(true)` is, `@optional`/`@required(false)`
    /// is not, and an input with neither is unless it has a default, which a
    /// caller that leaves it out receives instead.
    pub fn is_required_input(&self) -> bool {
        self.required.unwrap_or(self.default().is_none())
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

/// A user-defined request argument, distinct from its HTTP fields.
pub fn is_request_argument(name: &str) -> bool {
    ![URL_INPUT, HEADERS_INPUT, QUERY_INPUT, BODY_INPUT].contains(&name)
}

/// The inputs a node declares.
pub fn input_ports(node: &Node, project: &Project) -> Vec<Port> {
    match node.kind {
        NodeKind::Start => vec![],
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
        // A call reads the component's inputs as declared (KTLO-32), so the
        // engine, the board, the inspector and Extract component agree.
        NodeKind::Component => component_definition(project, node)
            .map(|component| {
                component
                    .inputs
                    .iter()
                    .map(|input| Port {
                        required: Some(input.is_required_input()),
                        ..input.clone()
                    })
                    .collect()
            })
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
            ports.extend(
                node.inputs
                    .keys()
                    .filter(|name| is_request_argument(name))
                    .map(|name| Port::new(name, ValueType::Any).required(false)),
            );
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
        NodeKind::Start | NodeKind::Export => vec![],
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
        Binding::Argument { .. }
        | Binding::Reference { .. }
        | Binding::Parameter { .. }
        | Binding::Environment { .. } => ValueType::Any,
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

/// The steps in the order the engine reaches them, without executing anything.
///
/// The runner's loop takes the first step of the document whose dependencies
/// have finished, and every step finishes whatever its result: a skipped branch
/// or a stopped run still takes its turn. So the order is static. Dependencies
/// outside `graph` do not hold a step back, as a Cleanup step reading the main
/// graph. Steps of a cycle, which the engine refuses, keep their document order
/// at the end, so every step has a place.
pub fn sequence(graph: &Graph) -> Vec<&str> {
    let index = dependency_index(graph);
    let mut pending: Vec<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    let known: HashSet<&str> = pending.iter().copied().collect();
    let mut done: HashSet<&str> = HashSet::with_capacity(pending.len());
    let mut order = Vec::with_capacity(pending.len());
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

/// A step a check verifies: a request or a component call it reads directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subject<'a> {
    /// The node id of the step.
    pub step: &'a str,
    /// The check has another subject too, and counts under each.
    pub shared: bool,
    /// The check is a gate: a Data condition whose value another step reads.
    pub gate: bool,
}

/// What a check reads directly, by role.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckSubjects<'a> {
    /// The steps it verifies; none for a standalone check.
    pub subjects: Vec<Subject<'a>>,
    /// The Value blocks it reads: its sources, which it does not verify.
    pub values: Vec<&'a str>,
}

/// The subjects of a Comparison or a Data condition, and its value sources; of
/// any other node, nothing. Only references written in the check count: a
/// step read through a value is that value's source, not the check's subject.
pub fn check_subjects<'a>(graph: &'a Graph, check: &'a Node) -> CheckSubjects<'a> {
    check_subjects_in(std::slice::from_ref(&graph), check)
}

/// The same over several graphs at once, for a check that reads steps of
/// another graph: a Cleanup check reads the main graph's steps, and whether it
/// is a gate is decided by every graph that may read it.
pub fn check_subjects_in<'a>(graphs: &[&'a Graph], check: &'a Node) -> CheckSubjects<'a> {
    if !matches!(check.kind, NodeKind::Assert | NodeKind::Condition) {
        return CheckSubjects::default();
    }
    let nodes = || graphs.iter().flat_map(|graph| &graph.nodes);
    let mut steps: IndexSet<&str> = IndexSet::new();
    let mut values: IndexSet<&str> = IndexSet::new();
    for binding in check.inputs.values() {
        for reference in binding.references() {
            let source = nodes().find(|node| node.id == reference.node_id);
            match source.map(|node| node.kind) {
                Some(NodeKind::Request | NodeKind::Component) => {
                    steps.insert(reference.node_id);
                }
                Some(NodeKind::Value) => {
                    values.insert(reference.node_id);
                }
                _ => {}
            }
        }
    }
    let gate = check.kind == NodeKind::Condition
        && nodes().any(|node| {
            node.inputs
                .values()
                .any(|binding| binding.references_node(&check.id))
        });
    let shared = steps.len() > 1;
    CheckSubjects {
        subjects: steps
            .into_iter()
            .map(|step| Subject { step, shared, gate })
            .collect(),
        values: values.into_iter().collect(),
    }
}

/// One output field of a step that feeds an input of another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Feed<'a> {
    /// The node id of the other step: the producer or the reader.
    pub step: &'a str,
    /// The output port read, such as `body`.
    pub output: &'a str,
    /// The path inside that output; empty for the whole output.
    pub path: &'a str,
    /// The reader's input it feeds, such as `headers`.
    pub input: &'a str,
}

/// A check that verifies a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckOf<'a> {
    /// The node id of the check.
    pub check: &'a str,
    /// The check has another subject too.
    pub shared: bool,
    /// The check is a gate: a Data condition another step reads.
    pub gate: bool,
}

/// What a step is related to inside its graph.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Relations<'a> {
    /// The fields the step reads, one per reference, Data conditions excepted.
    pub producers: Vec<Feed<'a>>,
    /// The fields other steps read from it, one per reference; the checks
    /// that verify it are listed under `checks` instead.
    pub readers: Vec<Feed<'a>>,
    /// The checks whose subjects include the step: `check_subjects` inverted.
    pub checks: Vec<CheckOf<'a>>,
    /// The Data conditions whose value the step reads.
    pub gates: Vec<&'a str>,
    /// For a check, the steps it verifies; empty otherwise.
    pub subjects: Vec<Subject<'a>>,
    /// For a check, the Value blocks it reads; empty otherwise.
    pub values: Vec<&'a str>,
}

/// The producers and readers of `node`, each with the output field and the
/// input between them, its checks and its gates; for a check also its
/// subjects and value sources. Only references inside `graph` count, and
/// ordering edges relate nothing here.
pub fn relations<'a>(graph: &'a Graph, node: &'a Node) -> Relations<'a> {
    let kind_of = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|other| other.id == id)
            .map(|other| other.kind)
    };
    let mut relations = Relations::default();
    let mut gates: IndexSet<&str> = IndexSet::new();
    for (input, binding) in &node.inputs {
        for reference in binding.references() {
            match kind_of(reference.node_id) {
                None => {}
                Some(NodeKind::Condition) => {
                    gates.insert(reference.node_id);
                }
                Some(_) => relations.producers.push(Feed {
                    step: reference.node_id,
                    output: reference.port,
                    path: reference.path,
                    input,
                }),
            }
        }
    }
    relations.gates = gates.into_iter().collect();
    let checks: Vec<CheckOf<'a>> = graph
        .nodes
        .iter()
        .filter_map(|other| {
            let subjects = check_subjects(graph, other);
            let subject = subjects
                .subjects
                .iter()
                .find(|subject| subject.step == node.id)?;
            Some(CheckOf {
                check: &other.id,
                shared: subject.shared,
                gate: subject.gate,
            })
        })
        .collect();
    for other in &graph.nodes {
        if checks.iter().any(|check| check.check == other.id) {
            continue;
        }
        for (input, binding) in &other.inputs {
            for reference in binding.references() {
                if reference.node_id == node.id {
                    relations.readers.push(Feed {
                        step: &other.id,
                        output: reference.port,
                        path: reference.path,
                        input,
                    });
                }
            }
        }
    }
    relations.checks = checks;
    let own = check_subjects(graph, node);
    relations.subjects = own.subjects;
    relations.values = own.values;
    relations
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

/// The node and every node that depends on it, directly or through others:
/// exactly the nodes whose `ancestors_and_self` contain it, found in one walk.
pub fn dependants_and_self(graph: &Graph, node_id: &str) -> IndexSet<String> {
    let mut dependants: HashMap<&str, Vec<&str>> = HashMap::new();
    for (target, sources) in dependency_index(graph) {
        for source in sources {
            dependants.entry(source).or_default().push(target);
        }
    }
    let mut selected = IndexSet::new();
    let mut stack = vec![node_id];
    while let Some(id) = stack.pop() {
        if selected.insert(id.to_owned())
            && let Some(targets) = dependants.get(id)
        {
            stack.extend(targets.iter().copied());
        }
    }
    selected
}

/// Removes nodes, their ordering edges and every input that references them.
pub fn delete_nodes(graph: &Graph, ids: &HashSet<&str>) -> Graph {
    Graph {
        // A variable's proxies are places to take that variable from, so they go
        // with it. Doing it here makes it part of the same edit, and one undo
        // brings the variable and its proxies back together.
        proxies: graph
            .proxies
            .iter()
            .filter(|proxy| match &proxy.source {
                ProxySource::Variable { node_id } => !ids.contains(node_id.as_str()),
                ProxySource::Environment { .. } | ProxySource::Parameter { .. } => true,
            })
            .cloned()
            .collect(),
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
        auto_placed: false,
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
        NodeKind::Start | NodeKind::Component | NodeKind::Export => {}
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
                    .insert(port.name.clone(), Binding::from_json(default.clone()));
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

    /// A check of `actual` against `expected`, both read from these sources.
    fn check(id: &str, kind: NodeKind, reads: &[(&str, &str)]) -> Node {
        let mut check = node(id, kind);
        check.inputs.clear();
        for (index, (source, port)) in reads.iter().enumerate() {
            check.inputs.insert(
                format!("in{index}"),
                Binding::reference(*source, *port, "total"),
            );
        }
        check
    }

    fn subject(step: &str, shared: bool, gate: bool) -> Subject<'_> {
        Subject { step, shared, gate }
    }

    /// LAY-08 criterion 1. A check names the steps it reads directly: a
    /// request or a component call is a subject, a value it is compared with
    /// is its value source (lay.md 1.3), and a gate is a Data condition whose
    /// value a later step reads.
    #[test]
    fn check_subjects_names_every_step_read_directly() {
        let mut through_a_value = node("first", NodeKind::Value);
        through_a_value
            .inputs
            .insert(VALUE_INPUT.into(), Binding::reference("cart", "body", ""));
        let mut checkout = node("checkout", NodeKind::Request);
        checkout.inputs.insert(
            BODY_INPUT.into(),
            Binding::reference("valid_cart", "value", "id"),
        );
        let mut standalone = node("standalone", NodeKind::Assert);
        standalone.inputs.clear();
        standalone
            .inputs
            .insert("actual".into(), Binding::literal(json!(1)));
        let graph = Graph {
            nodes: vec![
                node("begin", NodeKind::Start),
                node("cart", NodeKind::Request),
                node("receipt", NodeKind::Request),
                node("product", NodeKind::Component),
                node("expected_total", NodeKind::Value),
                through_a_value,
                check(
                    "cart_total",
                    NodeKind::Assert,
                    &[("cart", "body"), ("expected_total", "value")],
                ),
                check(
                    "receipt_total",
                    NodeKind::Assert,
                    &[("receipt", "body"), ("cart", "body")],
                ),
                check(
                    "twice",
                    NodeKind::Assert,
                    &[("cart", "body"), ("cart", "status")],
                ),
                check("priced", NodeKind::Assert, &[("product", "price")]),
                check("indirect", NodeKind::Assert, &[("first", "value")]),
                check("of_a_check", NodeKind::Assert, &[("cart_total", "passed")]),
                standalone,
                check("valid_cart", NodeKind::Condition, &[("cart", "body")]),
                check("unread", NodeKind::Condition, &[("cart", "body")]),
                check("ordered", NodeKind::Condition, &[("cart", "body")]),
                checkout,
            ],
            edges: vec![edge("ordered", "checkout")],
            proxies: Vec::new(),
        };
        let of = |id: &str| {
            let check = graph.nodes.iter().find(|node| node.id == id).unwrap();
            check_subjects(&graph, check)
        };

        let cart_total = of("cart_total");
        assert_eq!(cart_total.subjects, [subject("cart", false, false)]);
        assert_eq!(cart_total.values, ["expected_total"]);

        let shared = of("receipt_total");
        assert_eq!(
            shared.subjects,
            [
                subject("receipt", true, false),
                subject("cart", true, false)
            ],
            "a check with two subjects has both, each marked shared"
        );
        assert!(shared.values.is_empty());

        assert_eq!(
            of("twice").subjects,
            [subject("cart", false, false)],
            "two fields of one step are one subject"
        );
        assert_eq!(
            of("priced").subjects,
            [subject("product", false, false)],
            "a component call is a subject"
        );

        let indirect = of("indirect");
        assert!(
            indirect.subjects.is_empty(),
            "a step read through a value is not read directly"
        );
        assert_eq!(indirect.values, ["first"]);
        let of_a_check = of("of_a_check");
        assert!(of_a_check.subjects.is_empty() && of_a_check.values.is_empty());
        let standalone = of("standalone");
        assert!(standalone.subjects.is_empty() && standalone.values.is_empty());

        assert_eq!(
            of("valid_cart").subjects,
            [subject("cart", false, true)],
            "a Data condition read by a later step is a gate"
        );
        assert_eq!(
            of("unread").subjects,
            [subject("cart", false, false)],
            "a Data condition nothing reads is a check"
        );
        assert_eq!(
            of("ordered").subjects,
            [subject("cart", false, false)],
            "an ordering edge reads no value"
        );
    }

    /// LAY-08 criterion 6, the rule on its own: ready steps in document
    /// order, whatever comes first in the document.
    #[test]
    fn sequence_takes_the_first_ready_step_of_the_document() {
        let mut reader = node("reader", NodeKind::Value);
        reader
            .inputs
            .insert(VALUE_INPUT.into(), Binding::reference("source", "body", ""));
        let graph = Graph {
            nodes: vec![
                reader,
                node("ordered", NodeKind::Request),
                node("free", NodeKind::Request),
                node("source", NodeKind::Request),
            ],
            edges: vec![edge("source", "ordered")],
            proxies: Vec::new(),
        };
        assert_eq!(sequence(&graph), ["free", "source", "reader", "ordered"]);
    }

    /// Steps of a cycle cannot run; they keep their document order at the end,
    /// so every step has a place.
    #[test]
    fn sequence_leaves_a_cycle_at_the_end() {
        let graph = Graph {
            nodes: vec![
                node("one", NodeKind::Request),
                node("two", NodeKind::Request),
                node("free", NodeKind::Request),
            ],
            edges: vec![edge("one", "two"), edge("two", "one")],
            proxies: Vec::new(),
        };
        assert_eq!(sequence(&graph), ["free", "one", "two"]);
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

    /// KTLO-32 criterion 1: a component input declared with neither
    /// `@optional`/`@required(false)` nor a default is required wherever the
    /// component is called; `@optional` and `@required(true)` keep their
    /// reading, an input with a default keeps its own (it is never missing),
    /// and the built-in ports are untouched.
    #[test]
    fn an_undeclared_component_input_is_required() {
        assert!(Port::new("note", ValueType::String).is_required_input());
        assert!(
            Port::new("note", ValueType::String)
                .required(true)
                .is_required_input()
        );
        assert!(
            !Port::new("note", ValueType::String)
                .required(false)
                .is_required_input()
        );
        let defaulted = Port {
            default_value: OptionalJson::Present(json!("hello")),
            ..Port::new("greeting", ValueType::String)
        };
        assert!(!defaulted.is_required_input());

        let mut project = project();
        project.components.push(Component {
            id: "notes".into(),
            revision: 1,
            handle: "notes".into(),
            name: "Notes".into(),
            description: String::new(),
            inputs: vec![
                Port::new("note", ValueType::String),
                Port::new("extra", ValueType::String).required(false),
                Port::new("flag", ValueType::Boolean).required(true),
                defaulted,
            ],
            graph: Graph::default(),
            action: None,
        });
        let mut call = node("call", NodeKind::Component);
        call.config.component_id = Some("notes".into());
        call.config.revision = Some(1);
        let ports = input_ports(&call, &project);
        let required: Vec<(&str, Option<bool>)> = ports
            .iter()
            .map(|port| (port.name.as_str(), port.required))
            .collect();
        assert_eq!(
            required,
            [
                ("note", Some(true)),
                ("extra", Some(false)),
                ("flag", Some(true)),
                ("greeting", Some(false)),
            ]
        );
        assert!(ports[0].is_required(), "the board and the engine read it");

        let request = node("r", NodeKind::Request);
        let body = input_ports(&request, &project)
            .into_iter()
            .find(|port| port.name == BODY_INPUT)
            .unwrap();
        assert_eq!(body.required, None, "a request without a body still runs");
        assert!(!body.is_required());
        let export = input_ports(&node("e", NodeKind::Export), &project);
        assert!(
            !export[0].is_required(),
            "an export's value keeps its reading"
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
            proxies: Vec::new(),
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
            proxies: Vec::new(),
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
            proxies: Vec::new(),
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
            proxies: Vec::new(),
        };
        graph.nodes[2]
            .inputs
            .insert("actual".into(), Binding::reference("b", "value", ""));
        let selected = ancestors_and_self(&graph, "c");
        assert_eq!(selected.len(), 3);
        assert!(!selected.contains("d"));
    }

    #[test]
    fn dependants_are_the_nodes_whose_ancestors_contain_a_node() {
        let mut graph = Graph {
            nodes: vec![
                node("a", NodeKind::Request),
                node("b", NodeKind::Value),
                node("c", NodeKind::Assert),
                node("d", NodeKind::Request),
            ],
            edges: vec![edge("a", "b"), edge("d", "c")],
            proxies: Vec::new(),
        };
        graph.nodes[2]
            .inputs
            .insert("actual".into(), Binding::reference("b", "value", ""));
        for target in ["a", "b", "c", "d"] {
            let dependants = dependants_and_self(&graph, target);
            for candidate in ["a", "b", "c", "d"] {
                assert_eq!(
                    dependants.contains(candidate),
                    ancestors_and_self(&graph, candidate).contains(target),
                    "{candidate} depends on {target}"
                );
            }
        }
        assert_eq!(dependants_and_self(&graph, "a").len(), 3);
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
