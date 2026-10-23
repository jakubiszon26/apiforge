//! Mapping of version-one documents onto the current document model.

use crate::v1;
use apiforge_core::{
    format::{self as current, TemplatePart},
    graph::VALUE_INPUT,
    handle::{handle_base, unique_handle},
    js::js_key_order,
    model::{NodeKind, Operator},
    path::JsonPath,
    template,
};
use serde_json::Value;
use std::collections::HashSet;

pub fn workspace(source: &v1::Workspace) -> current::Workspace {
    current::Workspace {
        version: source.version,
        active_id: source.active_id.clone(),
        projects: source.projects.iter().map(project).collect(),
    }
}

pub fn project(source: &v1::Project) -> current::Project {
    current::Project {
        version: source.version,
        id: source.id.clone(),
        name: source.name.clone(),
        scenarios: source.scenarios.iter().map(scenario).collect(),
        components: source.components.iter().map(component).collect(),
        contracts: source
            .contracts
            .iter()
            .map(|contract| current::Contract {
                id: contract.id.clone(),
                name: contract.name.clone(),
                sample: contract.sample.clone(),
            })
            .collect(),
        environment: current::Environment {
            base_url: source.environment.base_url.clone(),
            variables: source.environment.variables.clone(),
            secret_names: source.environment.secret_names.clone(),
        },
    }
}

/// The main and cleanup graphs of a scenario, which share one handle namespace.
pub fn scenario_graphs(main: &v1::Graph, cleanup: &v1::Graph) -> (current::Graph, current::Graph) {
    let mut handles = HashSet::new();
    let main = graph(main, &mut handles);
    (main, graph(cleanup, &mut handles))
}

fn scenario(source: &v1::Scenario) -> current::Scenario {
    let (graph, cleanup) = scenario_graphs(&source.graph, &source.cleanup);
    current::Scenario {
        id: source.id.clone(),
        name: source.name.clone(),
        description: source.description.clone(),
        graph,
        cleanup,
        parameters: source.parameters.clone(),
        error_policy: source.error_policy,
    }
}

fn component(source: &v1::Component) -> current::Component {
    current::Component {
        id: source.id.clone(),
        revision: source.revision,
        name: source.name.clone(),
        description: source.description.clone(),
        inputs: source.inputs.iter().map(port).collect(),
        outputs: source
            .outputs
            .iter()
            .map(|output| current::OutputPort {
                name: output.name.clone(),
                value_type: output.value_type,
                label: output.label.clone(),
                contract_id: output.contract_id.clone(),
                required: output.required,
                default_value: optional_json(&output.default_value),
                exposed: output.exposed,
                binding: binding(&output.binding),
            })
            .collect(),
        graph: graph(&source.graph, &mut HashSet::new()),
        action: source
            .action
            .as_ref()
            .map(|action| current::ComponentAction {
                input: action.input.clone(),
                value_type: action.value_type,
                contract_id: action.contract_id.clone(),
            }),
    }
}

fn graph(source: &v1::Graph, handles: &mut HashSet<String>) -> current::Graph {
    current::Graph {
        nodes: source
            .nodes
            .iter()
            .map(|source| node(source, handles))
            .collect(),
        edges: source
            .edges
            .iter()
            .map(|edge| current::OrderEdge {
                id: edge.id.clone(),
                source: edge.source.clone(),
                target: edge.target.clone(),
                branch: edge.branch,
            })
            .collect(),
    }
}

/// Handles derive from version-one names (`Cena po promocji` → `cena_po_promocji`),
/// numbered on collision; the name stays as the label. Start is always `begin`.
fn node(source: &v1::Node, handles: &mut HashSet<String>) -> current::Node {
    let base = match source.kind {
        v1::NodeKind::Trigger => "begin".to_owned(),
        _ => handle_base(&source.name),
    };
    let handle = unique_handle(&base, |handle| handles.contains(handle));
    handles.insert(handle.clone());
    let (inputs, config) = match value_binding(source) {
        Some(value) => {
            let config = current::NodeConfig {
                variable: (source.kind == v1::NodeKind::Variable).then_some(true),
                ..current::NodeConfig::default()
            };
            let inputs = value
                .map(|value| current::Bindings::from([(VALUE_INPUT.to_owned(), value)]))
                .unwrap_or_default();
            (inputs, config)
        }
        None => (bindings(&source.inputs), config(&source.config)),
    };
    current::Node {
        id: source.id.clone(),
        // The default name of Start is no label: new Start blocks have none.
        label: (source.name != handle
            && !(source.kind == v1::NodeKind::Trigger && source.name == "Start"))
            .then(|| source.name.clone()),
        handle,
        note: None,
        kind: kind(source.kind),
        position: current::Position {
            x: source.position.x,
            y: source.position.y,
        },
        inputs,
        config,
    }
}

/// The expression of a Value replacing a Variable, Select, Build or Formula node:
/// `Some(None)` for a value without an expression, `None` for other kinds.
fn value_binding(source: &v1::Node) -> Option<Option<current::Binding>> {
    let input = |name: &str| source.inputs.get(name).map(binding);
    match source.kind {
        v1::NodeKind::Variable => Some(input("value")),
        v1::NodeKind::Select => Some(selection(source)),
        v1::NodeKind::Build => {
            // A JavaScript object orders integer keys first; the reference built
            // objects and lists in that order.
            let entries = js_key_order(
                source
                    .inputs
                    .iter()
                    .map(|(name, value)| (name, binding(value))),
            );
            Some(Some(match source.config.build_type.unwrap_or_default() {
                v1::BuildType::Object => current::Binding::Object {
                    fields: entries
                        .into_iter()
                        .map(|(name, value)| (name.clone(), value))
                        .collect(),
                },
                v1::BuildType::Array => current::Binding::Array {
                    items: entries.into_iter().map(|(_, value)| value).collect(),
                },
            }))
        }
        v1::NodeKind::Formula => {
            let operator = source.config.operation.unwrap_or(Operator::Multiply);
            // Operands keep their catalogue order; missing ones take the default
            // the formula's port supplied.
            let inputs = operator
                .spec()
                .operands
                .iter()
                .filter_map(|operand| {
                    let value = input(operand.name).or_else(|| {
                        operand
                            .default
                            .map(|default| current::Binding::literal(default.to_value()))
                    })?;
                    Some((operand.name.to_owned(), value))
                })
                .collect();
            Some(Some(current::Binding::Expression { operator, inputs }))
        }
        _ => None,
    }
}

/// A Select node as one binding. A static path on a reference or parameter extends
/// its path; any other selection becomes the `field` operator, whose path
/// is a template when it contains `{{name}}` tokens. Tokens name the node's
/// inputs, or else environment variables, as the reference runner resolved them.
fn selection(source: &v1::Node) -> Option<current::Binding> {
    let value = source.inputs.get("value").map(binding);
    let path = source.config.path.clone().unwrap_or_default();
    let tokens = template::tokens(&path);
    if tokens.is_empty() {
        if path.is_empty() {
            return value;
        }
        let join = |base: &str| {
            let (mut joined, field) = (JsonPath::parse(base).ok()?, JsonPath::parse(&path).ok()?);
            for segment in field.segments() {
                joined.push_key(segment.clone());
            }
            Some(joined.to_string())
        };
        match &value {
            Some(current::Binding::Reference {
                node_id,
                port,
                path: base,
            }) => {
                if let Some(joined) = join(base) {
                    return Some(current::Binding::reference(node_id, port, joined));
                }
            }
            Some(current::Binding::Parameter { name, path: base }) => {
                if let Some(joined) = join(base) {
                    return Some(current::Binding::parameter_field(name, joined));
                }
            }
            _ => {}
        }
    }
    let mut parts = Vec::new();
    let mut last = 0;
    for token in &tokens {
        if token.range.start > last {
            parts.push(TemplatePart::Text {
                text: path[last..token.range.start].to_owned(),
            });
        }
        let value = source
            .inputs
            .get(token.key)
            .map(binding)
            .unwrap_or_else(|| current::Binding::environment(token.key));
        parts.push(TemplatePart::Value { binding: value });
        last = token.range.end;
    }
    if last < path.len() {
        parts.push(TemplatePart::Text {
            text: path[last..].to_owned(),
        });
    }
    let path = if tokens.is_empty() {
        current::Binding::literal(Value::String(path))
    } else {
        current::Binding::Template { parts }
    };
    Some(current::Binding::Expression {
        operator: Operator::Field,
        inputs: current::Bindings::from([
            (
                "left".to_owned(),
                value.unwrap_or_else(|| current::Binding::literal(Value::Null)),
            ),
            ("right".to_owned(), path),
        ]),
    })
}

fn kind(source: v1::NodeKind) -> NodeKind {
    match source {
        v1::NodeKind::Request => NodeKind::Request,
        v1::NodeKind::Select
        | v1::NodeKind::Build
        | v1::NodeKind::Formula
        | v1::NodeKind::Variable => NodeKind::Value,
        v1::NodeKind::Assert => NodeKind::Assert,
        v1::NodeKind::Condition => NodeKind::Condition,
        v1::NodeKind::Component => NodeKind::Component,
        v1::NodeKind::Trigger => NodeKind::Trigger,
        v1::NodeKind::If => NodeKind::If,
    }
}

fn config(source: &v1::NodeConfig) -> current::NodeConfig {
    current::NodeConfig {
        method: source.method,
        url: source.url.clone(),
        headers: source.headers.clone(),
        query: source.query.clone(),
        timeout_ms: source.timeout_ms,
        sample_response: optional_json(&source.sample_response),
        sample_status: source.sample_status,
        contract_id: source.contract_id.clone(),
        predicate: source.predicate,
        tolerance: source.tolerance,
        variable: None,
        component_id: source.component_id.clone(),
        revision: source.revision,
        input_ports: source
            .input_ports
            .as_ref()
            .map(|ports| ports.iter().map(port).collect()),
        template_inputs: source
            .template_inputs
            .as_ref()
            .map(|ports| ports.iter().map(port).collect()),
        predicates: source.predicates.as_ref().map(|predicates| {
            predicates
                .iter()
                .map(|predicate| current::ConditionPredicate {
                    input: predicate.input.clone(),
                    expected: predicate.expected.clone(),
                    predicate: predicate.predicate,
                    path: predicate.path.clone(),
                })
                .collect()
        }),
        match_mode: source.match_mode,
    }
}

fn port(source: &v1::Port) -> current::Port {
    current::Port {
        name: source.name.clone(),
        value_type: source.value_type,
        label: source.label.clone(),
        contract_id: source.contract_id.clone(),
        required: source.required,
        default_value: optional_json(&source.default_value),
        exposed: source.exposed,
    }
}

fn optional_json(source: &v1::OptionalJson) -> current::OptionalJson {
    match source {
        v1::OptionalJson::Missing => current::OptionalJson::Missing,
        v1::OptionalJson::Present(value) => current::OptionalJson::Present(value.clone()),
    }
}

fn bindings(source: &v1::Bindings) -> current::Bindings {
    source
        .iter()
        .map(|(name, value)| (name.clone(), binding(value)))
        .collect()
}

fn binding(source: &v1::Binding) -> current::Binding {
    match source {
        v1::Binding::Expression { operator, inputs } => current::Binding::Expression {
            operator: *operator,
            inputs: bindings(inputs),
        },
        v1::Binding::Literal { value } => current::Binding::literal(value.clone()),
        v1::Binding::Reference {
            node_id,
            port,
            path,
        } => current::Binding::reference(node_id, port, path),
        v1::Binding::Parameter { name } => current::Binding::parameter(name),
        v1::Binding::Environment { name } => current::Binding::environment(name),
    }
}
