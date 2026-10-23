//! Mapping of version-one documents onto the current document model.

use crate::v1;
use apiforge_core::{format as current, model::NodeKind};

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

fn scenario(source: &v1::Scenario) -> current::Scenario {
    current::Scenario {
        id: source.id.clone(),
        name: source.name.clone(),
        description: source.description.clone(),
        graph: graph(&source.graph),
        cleanup: graph(&source.cleanup),
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
        graph: graph(&source.graph),
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

fn graph(source: &v1::Graph) -> current::Graph {
    current::Graph {
        nodes: source.nodes.iter().map(node).collect(),
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

fn node(source: &v1::Node) -> current::Node {
    current::Node {
        id: source.id.clone(),
        kind: kind(source.kind),
        name: source.name.clone(),
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
