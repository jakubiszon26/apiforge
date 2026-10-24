//! What the editor knows about the data behind a name.
//!
//! Completion after `handle.` needs the shape of a response, which the document
//! does not state. Three sources know something about it, in this order: the data
//! contract a request declares, the sample response a simulation would use, and
//! the output of the last run. The contract is what the team agreed on, the sample
//! is what the author wrote down, and the run is what the API actually sent; where
//! they disagree, the agreement wins and the detail line says where the shape came
//! from.
//!
//! Nothing here reaches the network, and nothing here reads a secret: a run's
//! outputs arrive already redacted.

use apiforge_core::{
    format::{Node, Project},
    graph::output_ports,
    model::{NodeKind, ValueType},
    path,
    run::StepResult,
};
use apiforge_script::complete::{ComponentOffer, Handle, Port, Scope, ShapePath, ShapeProvider};
use serde_json::Value;

/// Segments offered for one path. A deeply nested object would otherwise fill the
/// popup with one branch.
const MAX_SEGMENTS: usize = 40;

/// The shapes of one board.
pub struct BoardShapes<'a> {
    pub project: &'a Project,
    pub nodes: &'a [Node],
    /// Steps of the last run of this board, if one is shown.
    pub steps: &'a [StepResult],
}

impl ShapeProvider for BoardShapes<'_> {
    fn segments(&self, handle: &str, port: &str, prefix: &str) -> Vec<ShapePath> {
        let Some(node) = self.nodes.iter().find(|node| node.handle == handle) else {
            return Vec::new();
        };
        for (sample, source) in self.samples(node, port) {
            let Some(found) = at_path(&sample, prefix) else {
                continue;
            };
            let segments = segments_of(&found, &source);
            if !segments.is_empty() {
                return segments;
            }
        }
        Vec::new()
    }
}

impl BoardShapes<'_> {
    /// The values that may describe a port, best source first.
    fn samples(&self, node: &Node, port: &str) -> Vec<(Value, String)> {
        let mut found = Vec::new();
        if let Some(contract) = node
            .config
            .contract_id
            .as_ref()
            .and_then(|id| self.project.contracts.iter().find(|found| &found.id == id))
            && port == "body"
        {
            found.push((contract.sample.clone(), "contract".to_owned()));
        }
        if port == "body"
            && node.kind == NodeKind::Request
            && let apiforge_core::format::OptionalJson::Present(sample) =
                &node.config.sample_response
        {
            found.push((sample.clone(), "sample".to_owned()));
        }
        if let Some(step) = self.steps.iter().find(|step| step.node_id == node.id)
            && let Some(value) = step.outputs.get(port)
        {
            found.push((value.clone(), "last run".to_owned()));
        }
        found
    }
}

/// The value at a dotted path, where `[]` stands for any index.
fn at_path(value: &Value, prefix: &str) -> Option<Value> {
    if prefix.is_empty() {
        return Some(value.clone());
    }
    let mut current = value.clone();
    for segment in prefix.split('.') {
        current = match segment {
            // An index the completion engine could not read: the first item of a
            // list describes the rest of it well enough to continue.
            "[]" => current.as_array()?.first()?.clone(),
            key => path::resolve(&current, key)?.into_owned(),
        };
    }
    Some(current)
}

/// The segments that may follow a value: an object's keys, or a list's index.
fn segments_of(value: &Value, source: &str) -> Vec<ShapePath> {
    match value {
        Value::Object(fields) => fields
            .iter()
            .take(MAX_SEGMENTS)
            .map(|(key, value)| ShapePath {
                segment: key.clone(),
                value_type: type_of(value),
                source: Some(source.to_owned()),
            })
            .collect(),
        Value::Array(items) => {
            let first = items.first().map_or(ValueType::Any, type_of);
            vec![ShapePath {
                segment: "[0]".to_owned(),
                value_type: first,
                source: Some(source.to_owned()),
            }]
        }
        _ => Vec::new(),
    }
}

fn type_of(value: &Value) -> ValueType {
    match value {
        Value::String(_) => ValueType::String,
        Value::Number(_) => ValueType::Number,
        Value::Bool(_) => ValueType::Boolean,
        Value::Object(_) => ValueType::Object,
        Value::Array(_) => ValueType::Array,
        Value::Null => ValueType::Any,
    }
}

/// The names in scope, owned so that a [`Scope`] can borrow them for a frame.
#[derive(Default)]
pub struct Names {
    pub handles: Vec<Handle>,
    pub environment: Vec<String>,
    pub parameters: Vec<String>,
    pub inputs: Vec<String>,
    pub components: Vec<ComponentOffer>,
}

impl Names {
    /// What can be referenced from a board.
    ///
    /// `request` is the request being edited, whose named arguments are reachable
    /// as `input.name`; inside a component those are its inputs instead.
    pub fn of_board(
        project: &Project,
        nodes: &[Node],
        parameters: &[String],
        request: Option<&Node>,
    ) -> Self {
        let handles = nodes
            .iter()
            .map(|node| {
                let mut handle = Handle::new(node.handle.clone(), kind_label(node.kind));
                handle.variable =
                    node.kind == NodeKind::Value && node.config.variable.unwrap_or(false);
                handle.ports = output_ports(node, project)
                    .into_iter()
                    .map(|port| Port::new(port.name, port.value_type))
                    .collect();
                handle
            })
            .collect();
        let environment = project
            .environment
            .variables
            .keys()
            .cloned()
            .chain(project.environment.secret_names.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let inputs = request
            .map(|request| {
                request
                    .inputs
                    .keys()
                    .filter(|name| apiforge_core::graph::is_request_argument(name))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let components = project
            .components
            .iter()
            .map(|component| ComponentOffer {
                handle: component.handle.clone(),
                name: component.name.clone(),
                inputs: component
                    .inputs
                    .iter()
                    .map(|input| input.name.clone())
                    .collect(),
            })
            .collect();
        Self {
            handles,
            environment,
            parameters: parameters.to_vec(),
            inputs,
            components,
        }
    }

    /// Borrows the names for one frame, with the shapes the host can offer.
    pub fn scope<'a>(&'a self, shapes: &'a dyn ShapeProvider) -> Scope<'a> {
        Scope {
            handles: &self.handles,
            environment: &self.environment,
            parameters: &self.parameters,
            inputs: &self.inputs,
            components: &self.components,
            shapes,
        }
    }
}

fn kind_label(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Start => "start",
        NodeKind::Request => "request",
        NodeKind::Value => "value",
        NodeKind::Assert => "assert",
        NodeKind::Condition => "condition",
        NodeKind::If => "if",
        NodeKind::Component => "component",
        NodeKind::Export => "export",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project() -> Project {
        apiforge_core::demos::shop_project()
    }

    /// A request block, built directly so the test says what it depends on.
    fn request(handle: &str, contract: Option<&str>, sample: Option<Value>) -> Node {
        let mut node: Node = serde_json::from_value(json!({
            "id": format!("node-{handle}"),
            "kind": "request",
            "handle": handle,
            "position": {"x": 0, "y": 0},
            "inputs": {"url": {"kind": "literal", "value": "/products"}},
            "config": {"method": "GET"}
        }))
        .expect("a request node");
        node.config.contract_id = contract.map(str::to_owned);
        if let Some(sample) = sample {
            node.config.sample_response = apiforge_core::format::OptionalJson::Present(sample);
        }
        node
    }

    #[test]
    fn a_contract_describes_a_body_before_a_sample_does() {
        let mut project = project();
        project.contracts.push(apiforge_core::format::Contract {
            id: "contract-1".into(),
            name: "Product".into(),
            sample: json!({"id": "p-1", "price": 1.5}),
        });
        let nodes = [request(
            "products",
            Some("contract-1"),
            Some(json!({"other": true})),
        )];
        let shapes = BoardShapes {
            project: &project,
            nodes: &nodes,
            steps: &[],
        };
        let found = shapes.segments("products", "body", "");
        let names: Vec<&str> = found.iter().map(|path| path.segment.as_str()).collect();
        assert_eq!(names, ["id", "price"]);
        assert_eq!(found[0].source.as_deref(), Some("contract"));
        assert_eq!(found[1].value_type, ValueType::Number);
    }

    #[test]
    fn a_sample_describes_a_body_when_no_contract_does() {
        let project = project();
        let nodes = [request(
            "products",
            None,
            Some(json!({"items": [{"id": "p-1", "price": 2}]})),
        )];
        let shapes = BoardShapes {
            project: &project,
            nodes: &nodes,
            steps: &[],
        };
        assert_eq!(
            shapes
                .segments("products", "body", "")
                .into_iter()
                .map(|path| path.segment)
                .collect::<Vec<_>>(),
            ["items"]
        );
        // A list offers its first item, which describes the rest.
        let items = shapes.segments("products", "body", "items");
        assert_eq!(items[0].segment, "[0]");
        assert_eq!(items[0].value_type, ValueType::Object);
        // And the path continues through it.
        let fields = shapes.segments("products", "body", "items.[]");
        assert_eq!(
            fields
                .iter()
                .map(|path| path.segment.clone())
                .collect::<Vec<_>>(),
            ["id", "price"]
        );
        assert_eq!(fields[0].source.as_deref(), Some("sample"));
    }

    #[test]
    fn the_last_run_describes_what_nothing_else_does() {
        let project = project();
        let node = request("products", None, None);
        let mut step = StepResult::new(&node, apiforge_core::run::Phase::Main);
        step.outputs.insert(
            "headers".into(),
            json!({"content-type": "application/json"}),
        );
        let nodes = [node];
        let steps = [step];
        let shapes = BoardShapes {
            project: &project,
            nodes: &nodes,
            steps: &steps,
        };
        let found = shapes.segments("products", "headers", "");
        assert_eq!(found[0].segment, "content-type");
        assert_eq!(found[0].source.as_deref(), Some("last run"));
    }

    #[test]
    fn a_shape_that_says_nothing_falls_through_to_the_next_source() {
        let project = project();
        // The sample has no `items`, the run does: the run answers.
        let node = request("products", None, Some(json!({"other": 1})));
        let mut step = StepResult::new(&node, apiforge_core::run::Phase::Main);
        step.outputs
            .insert("body".into(), json!({"items": [{"id": "p-1"}]}));
        let nodes = [node];
        let steps = [step];
        let shapes = BoardShapes {
            project: &project,
            nodes: &nodes,
            steps: &steps,
        };
        let found = shapes.segments("products", "body", "items.[]");
        assert_eq!(found[0].segment, "id");
        assert_eq!(found[0].source.as_deref(), Some("last run"));
    }

    #[test]
    fn an_unknown_name_or_path_offers_nothing() {
        let project = project();
        let shapes = BoardShapes {
            project: &project,
            nodes: &[],
            steps: &[],
        };
        assert!(shapes.segments("absent", "body", "").is_empty());
        assert!(shapes.segments("absent", "body", "deep.path").is_empty());
        let nodes = [request("products", None, Some(json!({"a": 1})))];
        let shapes = BoardShapes {
            project: &project,
            nodes: &nodes,
            steps: &[],
        };
        assert!(shapes.segments("products", "body", "nowhere").is_empty());
        // A scalar has no segments under it.
        assert!(shapes.segments("products", "body", "a").is_empty());
    }

    #[test]
    fn the_names_of_a_board_include_its_blocks_and_environment() {
        let project = project();
        let scenario = &project.scenarios[0];
        let parameters: Vec<String> = scenario.parameters.keys().cloned().collect();
        let names = Names::of_board(&project, &scenario.graph.nodes, &parameters, None);
        assert_eq!(names.handles.len(), scenario.graph.nodes.len());
        assert!(names.handles.iter().all(|handle| !handle.name.is_empty()));
        assert!(
            names.environment.iter().all(|name| !name.contains(' ')),
            "environment names only"
        );
        assert_eq!(names.components.len(), project.components.len());
        // The scope borrows the names without copying them again.
        let scope = names.scope(&apiforge_script::complete::NoShapes);
        assert_eq!(scope.handles.len(), names.handles.len());
    }

    #[test]
    fn a_requests_own_arguments_are_its_inputs() {
        let mut project = project();
        let mut node = request("products", None, None);
        node.inputs.insert(
            "arg:productId".into(),
            apiforge_core::format::Binding::Literal { value: json!(1) },
        );
        let names = Names::of_board(&project, std::slice::from_ref(&node), &[], Some(&node));
        for input in &names.inputs {
            assert!(apiforge_core::graph::is_request_argument(input));
        }
        // Without a request there are no inputs to offer.
        project.components.clear();
        let names = Names::of_board(&project, &[node], &[], None);
        assert!(names.inputs.is_empty());
        assert!(names.components.is_empty());
    }
}
