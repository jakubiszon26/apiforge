//! Mapping of version-one documents onto the current document model.

use crate::v1;
use apiforge_core::{
    format as current,
    handle::{handle_base, unique_handle},
    model::NodeKind,
};
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
        inputs: bindings(&source.inputs),
        config: config(&source.config),
    }
}

fn kind(source: v1::NodeKind) -> NodeKind {
    match source {
        v1::NodeKind::Request => NodeKind::Request,
        v1::NodeKind::Select => NodeKind::Select,
        v1::NodeKind::Build => NodeKind::Build,
        v1::NodeKind::Formula => NodeKind::Formula,
        v1::NodeKind::Assert => NodeKind::Assert,
        v1::NodeKind::Condition => NodeKind::Condition,
        v1::NodeKind::Component => NodeKind::Component,
        v1::NodeKind::Variable => NodeKind::Variable,
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
        path: source.path.clone(),
        operation: source.operation,
        predicate: source.predicate,
        tolerance: source.tolerance,
        build_type: source.build_type.map(|build| match build {
            v1::BuildType::Object => apiforge_core::model::BuildType::Object,
            v1::BuildType::Array => apiforge_core::model::BuildType::Array,
        }),
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
