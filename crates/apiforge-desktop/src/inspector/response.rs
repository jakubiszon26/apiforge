//! Response data offered for picking fields: the last run's output when available,
//! otherwise the step's saved sample.

use apiforge_core::{
    edit::{ResponseFields, collect_response_fields_bounded},
    format::{Binding, Node, OptionalJson, Project},
    graph::component_definition,
    model::NodeKind,
    path,
    run::{RunMode, RunResult},
};
use serde_json::Value;

/// Data of one output port and where it came from.
pub struct Source {
    pub value: Option<Value>,
    pub origin: String,
}

/// The value an output would have according to saved samples.
pub fn sample_output(node: &Node, port: &str, project: &Project) -> Option<Value> {
    sample_output_at(node, port, project, 0)
}

fn sample_output_at(node: &Node, port: &str, project: &Project, depth: usize) -> Option<Value> {
    if depth > 10 {
        return None;
    }
    match node.kind {
        NodeKind::Request => match port {
            "body" => match &node.config.sample_response {
                OptionalJson::Present(value) => Some(value.clone()),
                OptionalJson::Missing => node.config.contract_id.as_ref().and_then(|id| {
                    project
                        .contracts
                        .iter()
                        .find(|contract| &contract.id == id)
                        .map(|contract| contract.sample.clone())
                }),
            },
            "status" => Some(Value::from(node.config.sample_status.unwrap_or(200))),
            _ => None,
        },
        NodeKind::Component => {
            let component = component_definition(project, node)?;
            let export = component
                .graph
                .nodes
                .iter()
                .find(|node| node.kind == NodeKind::Export && node.handle == port)?;
            match export.inputs.get(apiforge_core::graph::VALUE_INPUT)? {
                Binding::Literal { value } => Some(value.clone()),
                Binding::Reference {
                    node_id,
                    port,
                    path: field,
                } => {
                    let source = component
                        .graph
                        .nodes
                        .iter()
                        .find(|node| &node.id == node_id)?;
                    let value = sample_output_at(source, port, project, depth + 1)?;
                    path::resolve(&value, field).map(|value| value.into_owned())
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Observed output of `node` in `run`, or its sample.
pub fn source(node: &Node, port: &str, project: &Project, run: Option<&RunResult>) -> Source {
    let observed = run.and_then(|run| {
        run.steps
            .iter()
            .find(|step| step.node_id == node.id)
            .and_then(|step| step.outputs.get(port))
            .map(|value| (value.clone(), run.mode))
    });
    match observed {
        Some((value, mode)) => Source {
            value: Some(value),
            origin: format!(
                "Last response · {}",
                match mode {
                    RunMode::Live => "API",
                    RunMode::Simulation => "simulation",
                }
            ),
        },
        None => Source {
            value: sample_output(node, port, project),
            origin: "Sample data".into(),
        },
    }
}

/// Fields of a source, with the whole value first.
pub fn fields(value: Option<&Value>) -> ResponseFields {
    collect_response_fields_bounded(value)
}

pub fn type_label(name: &str) -> &'static str {
    match name {
        "string" => "Text",
        "number" => "Number",
        "boolean" => "Yes / no",
        "object" => "Object",
        "array" => "List",
        "null" => "Null",
        _ => "Missing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn component_samples_follow_output_bindings() {
        let project = apiforge_core::demos::shop_project();
        let node = project.scenarios[0]
            .graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Component)
            .unwrap();
        let component = component_definition(&project, node).unwrap();
        let output = &apiforge_core::graph::component_outputs(component)[0].name;
        assert!(sample_output(node, output, &project).is_some());
        assert_eq!(type_label("array"), "List");
        assert_eq!(fields(Some(&json!({"a": 1}))).fields.len(), 2);
    }
}
