//! Documents to canonical syntax. Exact metadata attributes cover fields whose
//! presence is semantic but whose normal spelling intentionally uses defaults.
use crate::{
    ast::*,
    format::print_file,
    lower::{Document, LowerContext, lower, validate_document_model},
    parse,
    reconcile::semantic,
};
use apiforge_core::{format::*, graph, model::*, operator::Operator, path::JsonPath};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub fn print_scenario(project: &Project, scenario: &Scenario) -> Result<String, Vec<Diagnostic>> {
    print_document(project, &Document::Scenario(scenario.clone()))
}
pub fn print_component(
    project: &Project,
    component: &Component,
) -> Result<String, Vec<Diagnostic>> {
    print_document(project, &Document::Component(component.clone()))
}
/// Prints a folder definition with the folder policy: component-only projects
/// are valid and every call must carry its explicit published revision.
pub fn print_folder_document(
    project: &Project,
    document: &Document,
) -> Result<String, Vec<Diagnostic>> {
    print_with(project, document, true)
}
fn print_document(project: &Project, document: &Document) -> Result<String, Vec<Diagnostic>> {
    print_with(project, document, false)
}
fn print_with(
    project: &Project,
    document: &Document,
    folder: bool,
) -> Result<String, Vec<Diagnostic>> {
    let valid = if folder {
        validate_document_model(document, &LowerContext::folder(project)).is_ok()
    } else {
        let mut validation = project.clone();
        match document {
            Document::Scenario(s) => validation.scenarios = vec![s.clone()],
            Document::Component(c) => {
                validation
                    .components
                    .retain(|old| old.id != c.id || old.revision != c.revision);
                validation.components.push(c.clone());
            }
        }
        apiforge_core::validate_project(&validation).is_ok()
    };
    if !valid {
        return Err(vec![Diagnostic::error(
            0..0,
            "Cannot print an invalid document",
        )]);
    }
    let (kind, description, parameters, inputs, action, main, cleanup) = match document {
        Document::Scenario(s) => (
            FileKind::Scenario {
                name: s.name.clone(),
            },
            s.description.clone(),
            s.parameters
                .iter()
                .map(|(k, v)| (k.clone(), literal(v)))
                .collect(),
            vec![],
            None,
            &s.graph,
            Some(&s.cleanup),
        ),
        Document::Component(c) => (
            FileKind::Component {
                handle: c.handle.clone(),
                name: c.name.clone(),
            },
            c.description.clone(),
            vec![],
            c.inputs.iter().map(input).collect(),
            c.action.as_ref(),
            &c.graph,
            None,
        ),
    };
    let all_nodes: HashMap<_, _> = main
        .nodes
        .iter()
        .chain(cleanup.into_iter().flat_map(|g| &g.nodes))
        .map(|n| (n.id.as_str(), n))
        .collect();
    let printer = Printer {
        project,
        nodes: all_nodes,
    };
    let file = File {
        kind,
        description: (!description.is_empty()).then_some(description),
        on_error: match document {
            Document::Scenario(s) => Some(
                if s.error_policy == ErrorPolicy::Stop {
                    "stop"
                } else {
                    "continue"
                }
                .into(),
            ),
            _ => None,
        },
        parameters,
        inputs,
        action: action.map(|a| a.input.clone()),
        action_type: action.map(|a| a.value_type.to_string()),
        action_contract: action.and_then(|a| a.contract_id.clone()),
        statements: printer.statements(main, None),
        cleanup: cleanup
            .filter(|g| !g.nodes.is_empty() || !g.edges.is_empty())
            .map(|g| printer.statements(g, None)),
        range: 0..0,
    };
    let source = print_file(&file);
    if source.len() > crate::MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::error(
            0..0,
            "Printed source would exceed the 1 MiB limit",
        )]);
    }
    // Printing is a checked projection: never return plausible text that silently
    // drops a field or cannot be lowered back to the same semantics.
    let tree = parse(&source);
    let context = if folder {
        LowerContext::folder(project)
    } else {
        LowerContext::document(project)
    };
    let context = if let Document::Component(component) = document {
        context.with_component(component)
    } else {
        context
    };
    let lowered = lower(&tree, &context);
    if lowered.has_errors() {
        return Err(lowered.diagnostics);
    }
    if lowered
        .document
        .as_ref()
        .is_none_or(|d| semantic(d, project) != semantic(document, project))
    {
        return Err(vec![Diagnostic::error(
            0..0,
            "Document cannot be represented without semantic loss",
        )]);
    }
    Ok(source)
}
fn expr(kind: ExprKind) -> Expr {
    Expr { kind, range: 0..0 }
}
fn literal(value: &Value) -> Expr {
    expr(match value {
        Value::Object(fields) => ExprKind::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), literal(v)))
                .collect(),
        ),
        Value::Array(items) => ExprKind::Array(items.iter().map(literal).collect()),
        _ => ExprKind::Literal(value.clone()),
    })
}
fn attribute(name: &str, arguments: Vec<Expr>) -> Attribute {
    Attribute {
        name: name.into(),
        arguments,
        range: 0..0,
    }
}
fn input(port: &Port) -> Input {
    let mut attributes = vec![];
    if let Some(label) = &port.label {
        attributes.push(attribute("label", vec![literal(&json!(label))]));
    }
    if let Some(contract) = &port.contract_id {
        attributes.push(attribute("contract", vec![literal(&json!(contract))]));
    }
    if let Some(required) = port.required {
        attributes.push(if required {
            attribute("required", vec![literal(&json!(true))])
        } else {
            attribute("optional", vec![])
        });
    }
    if let Some(exposed) = port.exposed {
        attributes.push(if exposed {
            attribute("exposed", vec![literal(&json!(true))])
        } else {
            attribute("hidden", vec![])
        });
    }
    Input {
        name: port.name.clone(),
        value_type: port.value_type.to_string(),
        default: match &port.default_value {
            OptionalJson::Present(v) => Some(literal(v)),
            _ => None,
        },
        attributes,
        range: 0..0,
    }
}
struct Printer<'a> {
    project: &'a Project,
    nodes: HashMap<&'a str, &'a Node>,
}
impl Printer<'_> {
    fn binding(&self, binding: &Binding) -> Expr {
        expr(match binding {
            Binding::Literal { value } => return literal(value),
            Binding::Object { fields } => ExprKind::Object(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), self.binding(v)))
                    .collect(),
            ),
            Binding::Array { items } => {
                ExprKind::Array(items.iter().map(|v| self.binding(v)).collect())
            }
            Binding::Template { parts } => ExprKind::Template(
                parts
                    .iter()
                    .map(|p| match p {
                        TemplatePart::Text { text } => Part::Text(text.clone()),
                        TemplatePart::Value { binding } => Part::Value(self.binding(binding)),
                    })
                    .collect(),
            ),
            Binding::Argument { name, path } => {
                return with_path(
                    field(expr(ExprKind::Name("input".into())), name, false),
                    path,
                );
            }
            Binding::Environment { name } => {
                return field(expr(ExprKind::Name("env".into())), name, false);
            }
            Binding::Parameter { name, path } => {
                return with_path(
                    field(expr(ExprKind::Name("param".into())), name, false),
                    path,
                );
            }
            Binding::Reference {
                node_id,
                port,
                path,
            } => {
                let source = self.nodes.get(node_id.as_str());
                let root = expr(ExprKind::Name(
                    source
                        .map_or(node_id.as_str(), |n| n.handle.as_str())
                        .into(),
                ));
                let root = if source
                    .is_some_and(|node| graph::output_ports(node, self.project).len() == 1)
                {
                    root
                } else {
                    field(root, port, false)
                };
                return with_path(root, path);
            }
            Binding::Expression { operator, inputs } => {
                // Infix is used only when it represents exactly the stored slots.
                if inputs.len() == 2
                    && inputs.contains_key("left")
                    && inputs.contains_key("right")
                    && let Some(symbol) = binary_symbol(*operator)
                {
                    ExprKind::Binary {
                        operator: symbol.into(),
                        left: Box::new(self.binding(&inputs["left"])),
                        right: Box::new(self.binding(&inputs["right"])),
                    }
                } else if *operator == Operator::Not
                    && inputs.len() == 1
                    && inputs.contains_key("left")
                {
                    ExprKind::Not(Box::new(self.binding(&inputs["left"])))
                } else {
                    ExprKind::Call {
                        name: format!(
                            "op.{}",
                            serde_json::to_value(operator).unwrap().as_str().unwrap()
                        ),
                        revision: None,
                        arguments: inputs
                            .iter()
                            .map(|(name, value)| Argument {
                                name: Some(name.clone()),
                                value: self.binding(value),
                            })
                            .collect(),
                    }
                }
            }
        })
    }
    fn slot(&self, node: &Node, name: &str) -> Expr {
        node.inputs
            .get(name)
            .map_or_else(|| expr(ExprKind::Missing), |v| self.binding(v))
    }
    fn check(&self, node: &Node, predicate: Predicate, actual: &str, expected: &str) -> Check {
        Check {
            actual: self.slot(node, actual),
            predicate: predicate_symbol(predicate).into(),
            expected: if matches!(predicate, Predicate::Exists | Predicate::NotEmpty) {
                None
            } else {
                Some(self.slot(node, expected))
            },
            tolerance: if predicate == Predicate::Approximately {
                Some(literal(&json!(node.config.tolerance.unwrap_or(0.0))))
            } else {
                None
            },
            range: 0..0,
        }
    }
    fn parent<'a>(&self, graph: &'a Graph, node: &Node) -> Option<(&'a str, Branch)> {
        let incoming: Vec<_> = graph.edges.iter().filter(|e| e.target == node.id).collect();
        match incoming.as_slice() {
            [edge]
                if self
                    .nodes
                    .get(edge.source.as_str())
                    .is_some_and(|n| n.kind == NodeKind::If) =>
            {
                edge.branch.map(|branch| (edge.source.as_str(), branch))
            }
            _ => None,
        }
    }
    /// The same rule as `parent`, for every node of a graph at once.
    fn parents<'g>(&self, graph: &'g Graph) -> HashMap<&'g str, (&'g str, Branch)> {
        let mut incoming: HashMap<&str, Vec<&OrderEdge>> = HashMap::new();
        for edge in &graph.edges {
            incoming.entry(edge.target.as_str()).or_default().push(edge);
        }
        incoming
            .into_iter()
            .filter_map(|(target, edges)| match edges.as_slice() {
                [edge]
                    if self
                        .nodes
                        .get(edge.source.as_str())
                        .is_some_and(|n| n.kind == NodeKind::If) =>
                {
                    edge.branch
                        .map(|branch| (target, (edge.source.as_str(), branch)))
                }
                _ => None,
            })
            .collect()
    }
    fn statements(&self, graph: &Graph, parent: Option<(&str, Branch)>) -> Vec<Statement> {
        // Each block is ordered on its own: a member depends on another when
        // anything nested in it does. A global order filtered per block is not
        // enough, because a nested statement may depend on a later outer one.
        // Lowering keeps text order, and this greedy choice reproduces an order
        // it produced, so print -> lower -> print is a fixed point. Dependency
        // cycles between blocks fall back to document order.
        let parents = self.parents(graph);
        let members: Vec<&Node> = graph
            .nodes
            .iter()
            .filter(|n| parents.get(n.id.as_str()).copied() == parent)
            .collect();
        let position: HashMap<&str, usize> = members
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), index))
            .collect();
        // The member whose block contains a node, possibly through nested IFs.
        let owner = |id| {
            let mut current: &str = id;
            for _ in 0..=graph.nodes.len() {
                if let Some(&index) = position.get(current) {
                    return Some(index);
                }
                current = parents.get(current)?.0;
            }
            None
        };
        let mut requires = vec![HashSet::new(); members.len()];
        for (id, dependencies) in graph::dependency_index(graph) {
            let Some(member) = owner(id) else { continue };
            for dependency in dependencies {
                if let Some(other) = owner(dependency)
                    && other != member
                {
                    requires[member].insert(other);
                }
            }
        }
        let mut done = vec![false; members.len()];
        let mut ordered = Vec::with_capacity(members.len());
        while ordered.len() < members.len() {
            let mut remaining = (0..members.len()).filter(|&index| !done[index]);
            let next = remaining
                .clone()
                .find(|&index| requires[index].iter().all(|&other| done[other]))
                .or_else(|| remaining.next())
                .expect("a member remains");
            done[next] = true;
            ordered.push(self.statement(members[next], graph));
        }
        ordered
    }
    fn statement(&self, node: &Node, graph: &Graph) -> Statement {
        let mut attributes = vec![];
        if let Some(label) = &node.label {
            attributes.push(attribute("label", vec![literal(&json!(label))]));
        }
        if let Some(note) = &node.note {
            // Comments are trimmed by attachment. Preserve exact whitespace with
            // an explicit string when comments alone cannot carry it.
            if note.split('\n').any(|line| line.trim() != line)
                || note.is_empty()
                || note.contains('\r')
            {
                attributes.push(attribute("note", vec![literal(&json!(note))]));
            }
        }
        let order: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.target == node.id && self.parent(graph, node).is_none())
            .map(|e| {
                let root = expr(ExprKind::Name(self.nodes[e.source.as_str()].handle.clone()));
                e.branch.map_or(root.clone(), |b| {
                    field(root, if b == Branch::True { "then" } else { "else" }, false)
                })
            })
            .collect();
        if !order.is_empty() {
            attributes.push(attribute("after", order));
        }
        let mut expected_config = NodeConfig::default();
        let mut represented: HashSet<String> = HashSet::new();
        let form = match node.kind {
            NodeKind::Start => Form::Start,
            NodeKind::Value => {
                if node.is_variable() {
                    expected_config.variable = Some(true);
                }
                represented.insert("value".into());
                Form::Value(self.slot(node, "value"))
            }
            NodeKind::Export => {
                expected_config.output_type =
                    Some(node.config.output_type.unwrap_or(ValueType::Any));
                represented.insert("value".into());
                Form::Export {
                    value_type: expected_config.output_type.unwrap().to_string(),
                    value: self.slot(node, "value"),
                }
            }
            NodeKind::Assert | NodeKind::If => {
                let predicate = node.config.predicate.unwrap_or_default();
                expected_config.predicate = Some(predicate);
                represented.insert("actual".into());
                if !matches!(predicate, Predicate::Exists | Predicate::NotEmpty) {
                    represented.insert("expected".into());
                }
                if predicate == Predicate::Approximately {
                    expected_config.tolerance = Some(node.config.tolerance.unwrap_or(0.0));
                }
                let check = self.check(node, predicate, "actual", "expected");
                if node.kind == NodeKind::If {
                    Form::If {
                        check,
                        then: self.statements(graph, Some((&node.id, Branch::True))),
                        otherwise: {
                            let nodes = self.statements(graph, Some((&node.id, Branch::False)));
                            (!nodes.is_empty()).then_some(nodes)
                        },
                    }
                } else {
                    Form::Assert(check)
                }
            }
            NodeKind::Condition => {
                let predicates = graph::condition_predicates(node);
                expected_config.predicates = Some(
                    predicates
                        .iter()
                        .map(|p| ConditionPredicate { predicate: *p })
                        .collect(),
                );
                expected_config.match_mode = Some(node.config.match_mode.unwrap_or(Match::All));
                let checks = predicates
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let (actual, expected) = graph::condition_inputs(i + 1);
                        represented.insert(actual.clone());
                        if !matches!(p, Predicate::Exists | Predicate::NotEmpty) {
                            represented.insert(expected.clone());
                        }
                        if *p == Predicate::Approximately {
                            expected_config.tolerance = Some(node.config.tolerance.unwrap_or(0.0));
                        }
                        self.check(node, *p, &actual, &expected)
                    })
                    .collect();
                represented.insert("value".into());
                Form::Condition {
                    mode: if expected_config.match_mode == Some(Match::Any) {
                        "any"
                    } else {
                        "all"
                    }
                    .into(),
                    checks,
                    value: node.inputs.get("value").map(|v| self.binding(v)),
                }
            }
            NodeKind::Component => {
                let definition = graph::component_definition(self.project, node);
                expected_config.component_id = node.config.component_id.clone();
                expected_config.revision = node.config.revision;
                // Call arguments name declared inputs only. Extra bindings, which
                // the document model permits, are carried by `@inputs`.
                let declared = |name: &String| {
                    definition.is_some_and(|c| c.inputs.iter().any(|p| p.name == *name))
                };
                represented.extend(node.inputs.keys().filter(|n| declared(n)).cloned());
                Form::Value(expr(ExprKind::Call {
                    name: definition.map_or("Unknown", |c| c.handle.as_str()).into(),
                    revision: node.config.revision,
                    arguments: node
                        .inputs
                        .iter()
                        .filter(|(name, _)| declared(name))
                        .map(|(name, v)| Argument {
                            name: Some(name.clone()),
                            value: self.binding(v),
                        })
                        .collect(),
                }))
            }
            NodeKind::Request => {
                expected_config.method = Some(node.config.method.unwrap_or(HttpMethod::Get));
                let mut items = vec![];
                represented.insert("url".into());
                for (name, binding) in node
                    .inputs
                    .iter()
                    .filter(|(name, _)| graph::is_request_argument(name))
                {
                    represented.insert(name.clone());
                    items.push(RequestItem::Argument {
                        name: name.clone(),
                        value: self.binding(binding),
                    });
                }
                for name in ["headers", "query", "body"] {
                    if let Some(binding) = node.inputs.get(name) {
                        represented.insert(name.into());
                        let value = self.binding(binding);
                        items.push(match name {
                            "headers" => RequestItem::Headers(value),
                            "query" => RequestItem::Query(value),
                            _ => {
                                expected_config.body_format =
                                    Some(node.config.body_format.unwrap_or_default());
                                RequestItem::Body {
                                    text: expected_config.body_format == Some(BodyFormat::Text),
                                    value,
                                }
                            }
                        });
                    }
                }
                if let Some(milliseconds) = node.config.timeout_ms {
                    expected_config.timeout_ms = Some(milliseconds);
                    items.push(RequestItem::Timeout { milliseconds });
                }
                if let Some(contract) = &node.config.contract_id
                    && self.project.contracts.iter().any(|c| c.id == *contract)
                {
                    expected_config.contract_id = Some(contract.clone());
                    items.push(RequestItem::Contract(contract.clone()));
                }
                if let OptionalJson::Present(value) = &node.config.sample_response {
                    let status = node.config.sample_status.unwrap_or(200);
                    expected_config.sample_response = OptionalJson::Present(value.clone());
                    expected_config.sample_status = Some(status);
                    items.push(RequestItem::Sample {
                        status,
                        value: literal(value),
                    });
                }
                Form::Request {
                    method: serde_json::to_value(expected_config.method)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .into(),
                    url: self.slot(node, "url"),
                    items,
                }
            }
        };
        if node.config != expected_config {
            let mut metadata = serde_json::to_value(&node.config).unwrap();
            let map = metadata.as_object_mut().unwrap();
            map.remove("componentId");
            map.remove("revision");
            attributes.push(attribute("config", vec![literal(&metadata)]));
        }
        if node.inputs.keys().any(|name| !represented.contains(name)) {
            attributes.push(attribute(
                "inputs",
                vec![expr(ExprKind::Object(
                    node.inputs
                        .iter()
                        .map(|(k, v)| (k.clone(), self.binding(v)))
                        .collect(),
                ))],
            ));
        }
        Statement {
            handle: node.handle.clone(),
            variable: node.is_variable(),
            attributes,
            note: node.note.clone(),
            form,
            range: 0..0,
        }
    }
}
fn field(base: Expr, key: &str, index: bool) -> Expr {
    expr(ExprKind::Field {
        base: Box::new(base),
        key: key.into(),
        index,
    })
}
fn with_path(mut root: Expr, path: &str) -> Expr {
    if let Ok(path) = JsonPath::parse(path) {
        for key in path.segments() {
            root = field(
                root,
                key,
                key.parse::<usize>().is_ok_and(|n| n.to_string() == *key),
            );
        }
    }
    root
}
fn binary_symbol(operator: Operator) -> Option<&'static str> {
    Some(match operator {
        Operator::Add => "+",
        Operator::Subtract => "-",
        Operator::Multiply => "*",
        Operator::Divide => "/",
        Operator::Modulo => "%",
        Operator::Equals => "==",
        Operator::NotEquals => "!=",
        Operator::Greater => ">",
        Operator::Less => "<",
        Operator::AtLeast => ">=",
        Operator::AtMost => "<=",
        Operator::And => "&&",
        Operator::Or => "||",
        _ => return None,
    })
}
fn predicate_symbol(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Equals => "==",
        Predicate::NotEquals => "!=",
        Predicate::Greater => ">",
        Predicate::Less => "<",
        Predicate::Exists => "exists",
        Predicate::NotEmpty => "nonempty",
        Predicate::Contains => "contains",
        Predicate::Type => "is",
        Predicate::Approximately => "~=",
    }
}
