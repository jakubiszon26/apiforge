//! Mapping of version-one documents onto the current document model.

use crate::v1;
use apiforge_core::{
    format::{self as current, TemplatePart},
    graph::{
        BODY_INPUT, HEADERS_INPUT, QUERY_INPUT, URL_INPUT, VALUE_INPUT, condition_inputs,
        create_node, export_position,
    },
    handle::{handle_base, is_valid_handle, unique_handle},
    js::js_key_order,
    model::{NodeKind, Operator, ValueType},
    path::JsonPath,
    template,
};
use serde_json::Value;
use std::collections::HashSet;

pub fn workspace(source: &v1::Workspace) -> current::Workspace {
    current::Workspace {
        version: current::FORMAT_VERSION,
        active_id: source.active_id.clone(),
        projects: source.projects.iter().map(project).collect(),
    }
}

pub fn project(source: &v1::Project) -> current::Project {
    current::Project {
        version: current::FORMAT_VERSION,
        id: source.id.clone(),
        name: source.name.clone(),
        scenarios: source.scenarios.iter().map(scenario).collect(),
        components: {
            // Revisions of a component share a handle; other components get their own.
            let mut handles: Vec<(String, String)> = Vec::new();
            source
                .components
                .iter()
                .map(|source| {
                    let handle = match handles.iter().find(|(id, _)| id == &source.id) {
                        Some((_, handle)) => handle.clone(),
                        None => {
                            let handle = unique_handle(&handle_base(&source.name), |taken| {
                                handles.iter().any(|(_, handle)| handle == taken)
                            });
                            handles.push((source.id.clone(), handle.clone()));
                            handle
                        }
                    };
                    component(source, handle)
                })
                .collect()
        },
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

/// A component whose outputs become Export blocks named like the outputs, in a
/// column to the right of its graph. Inner blocks give way to the output names.
/// Output defaults, `required` and `exposed` had no effect on runs and are dropped.
fn component(source: &v1::Component, handle: String) -> current::Component {
    let mut handles: HashSet<String> = source
        .outputs
        .iter()
        .map(|output| output.name.clone())
        .collect();
    let mut graph = graph(&source.graph, &mut handles);
    let exports: Vec<current::Node> = source
        .outputs
        .iter()
        .enumerate()
        .map(|(index, output)| {
            assert!(
                is_valid_handle(&output.name),
                "output `{}` of component {} is not a valid handle",
                output.name,
                source.id
            );
            let mut export =
                create_node(NodeKind::Export, export_position(&graph.nodes, index), None);
            export.id = format!("{}-export-{}", source.id, output.name);
            export.handle = output.name.clone();
            export.label = output.label.clone();
            export.config.output_type =
                (output.value_type != ValueType::Any).then_some(output.value_type);
            export.config.contract_id = output.contract_id.clone();
            export.inputs.insert(
                VALUE_INPUT.to_owned(),
                binding(&output.binding).normalized(),
            );
            export
        })
        .collect();
    graph.nodes.extend(exports);
    current::Component {
        id: source.id.clone(),
        revision: source.revision,
        handle,
        name: source.name.clone(),
        description: source.description.clone(),
        inputs: source.inputs.iter().map(port).collect(),
        graph,
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
        None if source.kind == v1::NodeKind::Request => request(source),
        None if source.kind == v1::NodeKind::Condition => condition(source),
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
        auto_placed: false,
        // Version two writes every binding in canonical form.
        inputs: inputs
            .into_iter()
            .map(|(name, binding)| (name, binding.normalized()))
            .collect(),
        config,
    }
}

/// A Data condition with positional inputs: check *n* compares `actual<n>`, the
/// predicate's input narrowed to its path, with `expected<n>`. A condition without
/// a predicate list is one check of its `actual` and `expected` inputs.
fn condition(source: &v1::Node) -> (current::Bindings, current::NodeConfig) {
    let config = &source.config;
    let checks: Vec<(
        apiforge_core::model::Predicate,
        String,
        Option<String>,
        String,
    )> = match config.predicates.as_ref().filter(|list| !list.is_empty()) {
        Some(list) => list
            .iter()
            .map(|check| {
                (
                    check.predicate,
                    check.input.clone(),
                    check.path.clone(),
                    check.expected.clone(),
                )
            })
            .collect(),
        None => vec![(
            config.predicate.unwrap_or_default(),
            "actual".into(),
            None,
            "expected".into(),
        )],
    };
    let mut inputs = current::Bindings::new();
    // A condition with a predicate list passed its `actual` input on.
    if config
        .predicates
        .as_ref()
        .is_some_and(|list| !list.is_empty())
        && let Some(value) = source.inputs.get("actual")
    {
        inputs.insert(VALUE_INPUT.to_owned(), binding(value));
    }
    for (index, (_, actual, path, expected)) in checks.iter().enumerate() {
        let (actual_name, expected_name) = condition_inputs(index + 1);
        let value = source.inputs.get(actual).map(binding);
        if let Some(value) = select(source, value, path.clone().unwrap_or_default()) {
            inputs.insert(actual_name, value);
        }
        if let Some(value) = source.inputs.get(expected) {
            inputs.insert(expected_name, binding(value));
        }
    }
    let config = current::NodeConfig {
        predicates: Some(
            checks
                .iter()
                .map(|(predicate, ..)| current::ConditionPredicate {
                    predicate: *predicate,
                })
                .collect(),
        ),
        tolerance: config.tolerance,
        match_mode: config.match_mode,
        ..current::NodeConfig::default()
    };
    (inputs, config)
}

/// A request's inputs and configuration. URL, header and query templates become
/// bindings: `{{name}}` and `/:name` take the node input of that name, and a token
/// without an input names an environment variable, as the reference runner
/// resolved them. Literal bodies are expanded the same way.
fn request(source: &v1::Node) -> (current::Bindings, current::NodeConfig) {
    let mut inputs = current::Bindings::new();
    inputs.insert(
        URL_INPUT.to_owned(),
        text_binding(source, source.config.url.as_deref().unwrap_or(""), true),
    );
    for (name, map) in [
        (HEADERS_INPUT, &source.config.headers),
        (QUERY_INPUT, &source.config.query),
    ] {
        if let Some(map) = map.as_ref().filter(|map| !map.is_empty()) {
            let fields = map
                .iter()
                .map(|(key, value)| (key.clone(), text_binding(source, value, false)))
                .collect();
            inputs.insert(name.to_owned(), current::Binding::Object { fields });
        }
    }
    if let Some(body) = source.inputs.get(BODY_INPUT) {
        let body = match body {
            v1::Binding::Literal { value } => body_binding(source, value),
            other => binding(other),
        };
        inputs.insert(BODY_INPUT.to_owned(), body);
    }
    let config = current::NodeConfig {
        method: source.config.method,
        timeout_ms: source.config.timeout_ms,
        sample_response: optional_json(&source.config.sample_response),
        sample_status: source.config.sample_status,
        contract_id: source.config.contract_id.clone(),
        ..current::NodeConfig::default()
    };
    (inputs, config)
}

/// The binding a `{{name}}` token or `/:name` parameter stood for: the node input,
/// the default of a declared argument without one, or an environment variable.
fn token_binding(source: &v1::Node, key: &str) -> current::Binding {
    let declared_default = || {
        source
            .config
            .input_ports
            .iter()
            .chain(source.config.template_inputs.iter())
            .flatten()
            .find(|port| port.name == key)
            .and_then(|port| match &port.default_value {
                v1::OptionalJson::Present(value) => Some(current::Binding::literal(value.clone())),
                v1::OptionalJson::Missing => None,
            })
    };
    source
        .inputs
        .get(key)
        .map(binding)
        .or_else(declared_default)
        .unwrap_or_else(|| current::Binding::environment(key))
}

/// Text with `{{name}}` tokens and, in a URL, `/:name` parameters as a literal or
/// template.
fn text_binding(source: &v1::Node, text: &str, url: bool) -> current::Binding {
    let mut parts = Vec::new();
    let mut push_text = |parts: &mut Vec<TemplatePart>, text: &str| {
        if text.is_empty() {
            return;
        }
        match parts.last_mut() {
            Some(TemplatePart::Text { text: last }) => last.push_str(text),
            _ => parts.push(TemplatePart::Text {
                text: text.to_owned(),
            }),
        }
    };
    let mut last = 0;
    for token in template::tokens(text) {
        url_text(
            source,
            &text[last..token.range.start],
            url,
            &mut parts,
            &mut push_text,
        );
        parts.push(TemplatePart::Value {
            binding: token_binding(source, token.key),
        });
        last = token.range.end;
    }
    url_text(source, &text[last..], url, &mut parts, &mut push_text);
    if parts
        .iter()
        .all(|part| matches!(part, TemplatePart::Text { .. }))
    {
        return current::Binding::literal(Value::String(text.to_owned()));
    }
    current::Binding::Template { parts }
}

/// Literal text of a template; in a URL, `/:name` parameters naming an input
/// become values (which the runner percent-encodes, as it did for parameters).
fn url_text(
    source: &v1::Node,
    text: &str,
    url: bool,
    parts: &mut Vec<TemplatePart>,
    push_text: &mut impl FnMut(&mut Vec<TemplatePart>, &str),
) {
    if !url {
        push_text(parts, text);
        return;
    }
    let mut rest = text;
    while let Some(index) = rest.find("/:") {
        let after = &rest[index + 2..];
        let length = after
            .char_indices()
            .find(|(position, character)| {
                !(character.is_ascii_alphanumeric()
                    || *character == '_'
                    || (*position > 0 && *character == '-'))
            })
            .map_or(after.len(), |(position, _)| position);
        let name = &after[..length];
        let starts_well = name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
        if starts_well && source.inputs.contains_key(name) {
            push_text(parts, &rest[..index + 1]);
            parts.push(TemplatePart::Value {
                binding: token_binding(source, name),
            });
            rest = &after[length..];
        } else {
            push_text(parts, &rest[..index + 2]);
            rest = after;
        }
    }
    push_text(parts, rest);
}

/// A literal JSON body with its tokens: a string that is exactly one token takes
/// the value with its type, other strings with tokens become templates.
fn body_binding(source: &v1::Node, value: &Value) -> current::Binding {
    fn has_tokens(value: &Value) -> bool {
        match value {
            Value::String(text) => !template::tokens(text).is_empty(),
            Value::Array(items) => items.iter().any(has_tokens),
            Value::Object(fields) => fields.values().any(has_tokens),
            _ => false,
        }
    }
    if !has_tokens(value) {
        return current::Binding::literal(value.clone());
    }
    match value {
        Value::String(text) => match template::tokens(text).as_slice() {
            [token] if token.range == (0..text.len()) => token_binding(source, token.key),
            _ => text_binding(source, text, false),
        },
        Value::Array(items) => current::Binding::Array {
            items: items
                .iter()
                .map(|item| body_binding(source, item))
                .collect(),
        },
        Value::Object(fields) => current::Binding::Object {
            fields: fields
                .iter()
                .map(|(key, item)| (key.clone(), body_binding(source, item)))
                .collect(),
        },
        other => current::Binding::literal(other.clone()),
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
    select(
        source,
        value,
        source.config.path.clone().unwrap_or_default(),
    )
}

/// `value` narrowed to a field path, as a Select node or a condition predicate's
/// path selected it.
fn select(
    source: &v1::Node,
    value: Option<current::Binding>,
    path: String,
) -> Option<current::Binding> {
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
        v1::NodeKind::Trigger => NodeKind::Start,
        v1::NodeKind::If => NodeKind::If,
    }
}

fn config(source: &v1::NodeConfig) -> current::NodeConfig {
    current::NodeConfig {
        method: source.method,
        body_format: None,
        timeout_ms: source.timeout_ms,
        sample_response: optional_json(&source.sample_response),
        sample_status: source.sample_status,
        contract_id: source.contract_id.clone(),
        predicate: source.predicate,
        tolerance: source.tolerance,
        variable: None,
        component_id: source.component_id.clone(),
        revision: source.revision,
        output_type: None,
        predicates: None,
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
