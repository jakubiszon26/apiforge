//! Adding blocks: from the library, as response assertions and as variables.
use super::{
    EditError,
    connect::{control_branch, order_edge},
    unique_id,
};
use crate::{
    block::Block,
    format::{Binding, Component, Graph, Node, Position, Project},
    graph::{VALUE_INPUT, condition_inputs, create_node, has_cycle, output_ports, position_after},
    handle::{handle_base, unique_handle},
    js,
    model::NodeKind,
};
use serde_json::Value;

/// An output of an existing block that a new block is connected from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputRef<'a> {
    pub node_id: &'a str,
    /// A data output, or a control pseudo-port such as `__order` or `__true`.
    pub port: &'a str,
}

/// The board after adding a block, and the new block's identifier.
#[derive(Clone, Debug, PartialEq)]
pub struct AddedNode {
    pub graph: Graph,
    pub node_id: String,
}

/// The slot (input and path) a new node takes a dropped data connection on, if any.
fn default_slot(node: &Node) -> Option<(String, Vec<String>)> {
    let whole = |input: &str| Some((input.to_owned(), Vec::new()));
    match node.kind {
        NodeKind::If | NodeKind::Assert => whole("actual"),
        NodeKind::Condition => whole(&condition_inputs(1).0),
        NodeKind::Request => whole("body"),
        NodeKind::Export => whole(VALUE_INPUT),
        NodeKind::Value => match node.inputs.get(VALUE_INPUT) {
            Some(Binding::Object { fields }) => fields
                .keys()
                .next()
                .map(|field| (VALUE_INPUT.to_owned(), vec![field.clone()])),
            _ => whole(VALUE_INPUT),
        },
        NodeKind::Start | NodeKind::Component => None,
    }
}

/// Adds a new block of `kind`, optionally connected from an existing output.
///
/// - From a control pseudo-port the block gets an ordering edge (taking the IF
///   branch of `__true`/`__false`).
/// - From a data output the block's default input is bound to it: `actual` for
///   checks, `value` for values, `body` for requests and the action input of a
///   component. A block without such an input gets an ordering edge instead.
/// - Without a source, a request or component gets an ordering edge from Start
///   when Start has no outgoing edge yet, so the first step is visibly connected.
///
/// Handles are numbered when taken (`request_2`). A board can hold only one Start.
pub fn add_node(
    graph: &Graph,
    block: Block,
    position: Position,
    component: Option<&Component>,
    source: Option<OutputRef<'_>>,
) -> Result<AddedNode, EditError> {
    let kind = block.kind();
    let start = graph.nodes.iter().find(|node| node.kind == NodeKind::Start);
    if kind == NodeKind::Start && start.is_some() {
        return Err(EditError::SecondStart);
    }
    if let Some(source) = source
        && !graph.nodes.iter().any(|node| node.id == source.node_id)
    {
        return Err(EditError::NodeNotFound);
    }
    let mut node = block.create(position, component);
    node.id = unique_id("node", |id| graph.nodes.iter().any(|node| node.id == id));
    node.handle = unique_handle(&node.handle, |handle| {
        graph.nodes.iter().any(|node| node.handle == handle)
    });

    let mut next = graph.clone();
    match source {
        Some(source) => match control_branch(source.port) {
            Some(branch) => {
                let edge = order_edge(&next, source.node_id, &node.id, branch);
                next.edges.push(edge);
            }
            None => {
                let slot = component
                    .and_then(|component| component.action.as_ref())
                    .map(|action| (action.input.clone(), Vec::new()))
                    .or_else(|| default_slot(&node));
                match slot {
                    Some((input, path)) => {
                        let reference = Binding::reference(source.node_id, source.port, "");
                        let bound = match node.inputs.get(&input) {
                            Some(current) => current
                                .replace_at(&path, reference)
                                .ok_or(EditError::InvalidOperandPath)?,
                            None => reference,
                        };
                        node.inputs.insert(input, bound);
                    }
                    None => {
                        let edge = order_edge(&next, source.node_id, &node.id, None);
                        next.edges.push(edge);
                    }
                }
            }
        },
        None => {
            if let Some(start) = start
                && matches!(kind, NodeKind::Request | NodeKind::Component)
                && !graph.edges.iter().any(|edge| edge.source == start.id)
            {
                let edge = order_edge(&next, &start.id, &node.id, None);
                next.edges.push(edge);
            }
        }
    }
    let node_id = node.id.clone();
    next.nodes.push(node);
    Ok(AddedNode {
        graph: next,
        node_id,
    })
}

/// A comparison of one output field with an expected value, placed next to `source`.
///
/// Its handle is derived from the field and may collide with existing handles; the
/// caller adds it with [`crate::handle::unique_handle`] or `dedupe_handles`.
///
/// The actual value stays a reference, so the check reads the field on every run;
/// the same binding is used for sample responses and run results.
pub fn field_assertion(source: &Node, port: &str, path: &str, value: &Value) -> Node {
    let position = Position {
        x: source.position.x + 340.0,
        y: source.position.y + 70.0,
    };
    let mut node = create_node(NodeKind::Assert, position, None);
    let subject = if path.is_empty() { port } else { path };
    node.handle = handle_base(&format!("check {subject}"));
    node.label = Some(format!("Check {subject}"));
    node.inputs.clear();
    node.inputs
        .insert("actual".into(), Binding::reference(&source.id, port, path));
    node.inputs
        .insert("expected".into(), Binding::literal(value.clone()));
    node
}

/// The board after a response assignment, and the variable's identifier.
#[derive(Clone, Debug, PartialEq)]
pub struct AssignedVariable {
    pub graph: Graph,
    pub variable_id: String,
}

/// Binds a variable to a response field: a new variable placed after every block,
/// or the existing variable `target_id`.
///
/// The variable keeps a reference, never a value observed in an earlier run. A
/// request cannot use a variable assigned from its own response.
pub fn assign_response_variable(
    graph: &Graph,
    project: &Project,
    source_id: &str,
    port: &str,
    path: &str,
    name: &str,
    target_id: Option<&str>,
) -> Result<AssignedVariable, EditError> {
    let source = graph
        .nodes
        .iter()
        .find(|node| node.id == source_id)
        .filter(|source| {
            output_ports(source, project)
                .iter()
                .any(|output| output.name == port)
        })
        .ok_or(EditError::ResponseSourceUnavailable)?;
    let name = js::trim(name);
    if name.is_empty() {
        return Err(EditError::VariableNameRequired);
    }
    let mut variable = match target_id {
        Some(target_id) => graph
            .nodes
            .iter()
            .find(|node| node.id == target_id)
            .cloned()
            .ok_or(EditError::VariableNotFound)?,
        None => {
            let mut node = create_node(
                NodeKind::Value,
                position_after(graph, source.position.y),
                None,
            );
            node.id = unique_id("node", |id| graph.nodes.iter().any(|node| node.id == id));
            node
        }
    };
    if variable.kind != NodeKind::Value {
        return Err(EditError::VariableNotFound);
    }
    variable.config.variable = Some(true);
    variable.label = Some(name.to_owned());
    if target_id.is_none() {
        variable.handle = unique_handle(&handle_base(name), |handle| {
            graph.nodes.iter().any(|node| node.handle == handle)
        });
    }
    variable
        .inputs
        .insert("value".into(), Binding::reference(&source.id, port, path));

    let mut next = graph.clone();
    let variable_id = variable.id.clone();
    match target_id {
        Some(target_id) => {
            for node in next.nodes.iter_mut().filter(|node| node.id == target_id) {
                *node = variable.clone();
            }
        }
        None => next.nodes.push(variable),
    }
    if has_cycle(&next) {
        return Err(EditError::AssignmentCycle);
    }
    Ok(AssignedVariable {
        graph: next,
        variable_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        edit::test_support::{at, empty_project, node},
        format::OrderEdge,
        graph::{FALSE_PORT, ORDER_PORT, input_ports, latest_components},
        model::{Branch, Predicate},
    };
    use serde_json::json;

    const AT: Position = Position { x: 0.0, y: 0.0 };

    #[test]
    fn new_requests_have_only_a_url() {
        let request = node(NodeKind::Request);
        assert_eq!(request.inputs.keys().collect::<Vec<_>>(), ["url"]);
        let names: Vec<_> = input_ports(&request, &empty_project())
            .into_iter()
            .map(|port| port.name)
            .collect();
        assert_eq!(names, ["url", "body"]);
        assert_eq!(
            request.inputs["url"],
            Binding::literal(serde_json::json!("/endpoint"))
        );
        assert!(request.config.sample_response.is_missing());
    }

    #[test]
    fn field_assertions_compare_a_referenced_field_with_a_literal() {
        for value in [
            json!("hello"),
            json!(0),
            json!(false),
            json!({}),
            json!([]),
            Value::Null,
        ] {
            let mut source = node(NodeKind::Request);
            source.config.sample_response =
                crate::format::OptionalJson::Present(json!({"arbitrary": value}));
            let check = field_assertion(&source, "body", "arbitrary", &value);
            assert_eq!(check.kind, NodeKind::Assert);
            assert_eq!(check.config.predicate, Some(Predicate::Equals));
            assert_eq!(check.label.as_deref(), Some("Check arbitrary"));
            assert_eq!(check.handle, "check_arbitrary");
            assert_eq!(
                check.inputs["actual"],
                Binding::reference(&source.id, "body", "arbitrary")
            );
            assert_eq!(check.inputs["expected"], Binding::literal(value));
            assert_eq!(check.position, Position { x: 440.0, y: 170.0 });
        }
        let source = node(NodeKind::Request);
        assert_eq!(
            field_assertion(&source, "status", "", &json!(200)).handle,
            "check_status"
        );
    }

    #[test]
    fn response_assignments_keep_paths_and_create_variables() {
        let project = empty_project();
        let mut source = node(NodeKind::Request);
        source.config.sample_response = crate::format::OptionalJson::Present(
            json!({"items": [{"price": 0, "flag": false, "optional": null}]}),
        );
        let mut graph = Graph {
            nodes: vec![source.clone()],
            edges: vec![],
        };
        for (name, path) in [
            ("capturedPrice", "items[0].price"),
            ("capturedFlag", "items[0].flag"),
            ("capturedNull", "items[0].optional"),
        ] {
            let result =
                assign_response_variable(&graph, &project, &source.id, "body", path, name, None)
                    .unwrap();
            graph = result.graph;
            let variable = graph
                .nodes
                .iter()
                .find(|node| node.id == result.variable_id)
                .unwrap();
            assert_eq!(variable.label.as_deref(), Some(name));
            assert_eq!(variable.handle, crate::handle::handle_base(name));
            assert_eq!(
                variable.inputs["value"],
                Binding::reference(&source.id, "body", path)
            );
        }
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.nodes[1].position, Position { x: 440.0, y: 100.0 });
        assert_eq!(graph.nodes[2].position.x, 780.0);
    }

    #[test]
    fn assigning_an_existing_variable_rejects_cycles_without_mutation() {
        let project = empty_project();
        let source = node(NodeKind::Request);
        let variable = node(NodeKind::Value);
        let mut graph = Graph {
            nodes: vec![source.clone(), variable.clone()],
            edges: vec![],
        };
        let before = graph.clone();
        let captured = assign_response_variable(
            &graph,
            &project,
            &source.id,
            "body",
            "data.price",
            " price ",
            Some(&variable.id),
        )
        .unwrap();
        assert_eq!(captured.graph.nodes.len(), 2);
        assert_eq!(captured.graph.nodes[1].label.as_deref(), Some("price"));
        assert_eq!(
            captured.graph.nodes[1].inputs["value"],
            Binding::reference(&source.id, "body", "data.price")
        );
        assert_eq!(graph, before);

        graph.nodes[0].inputs.insert(
            "argument".into(),
            Binding::reference(&variable.id, "value", ""),
        );
        let assign = |source: &str, port: &str, name: &str, target: Option<&str>| {
            assign_response_variable(&graph, &project, source, port, "data.price", name, target)
        };
        let error = assign(&source.id, "body", "price", Some(&variable.id)).unwrap_err();
        assert!(matches!(error, EditError::AssignmentCycle));
        assert!(error.to_string().contains("cycle"));
        assert_eq!(graph.nodes[1].inputs["value"], Binding::literal(json!("")));
        for (source, port) in [("missing", "body"), (source.id.as_str(), "missing")] {
            assert!(matches!(
                assign(source, port, "x", None),
                Err(EditError::ResponseSourceUnavailable)
            ));
        }
        assert!(matches!(
            assign(&source.id, "body", "  ", None),
            Err(EditError::VariableNameRequired)
        ));
        assert!(matches!(
            assign(&source.id, "body", "x", Some(&source.id)),
            Err(EditError::VariableNotFound)
        ));
    }

    fn start_graph() -> (Graph, String) {
        let start = node(NodeKind::Start);
        let id = start.id.clone();
        (
            Graph {
                nodes: vec![start],
                edges: vec![],
            },
            id,
        )
    }

    #[test]
    fn the_first_executable_block_is_connected_from_start() {
        let (graph, start) = start_graph();
        let added = add_node(&graph, Block::Request, AT, None, None).unwrap();
        assert_eq!(added.graph.nodes.len(), 2);
        assert_eq!(added.graph.edges.len(), 1);
        assert_eq!(added.graph.edges[0].source, start);
        assert_eq!(added.graph.edges[0].target, added.node_id);
        let second = add_node(&added.graph, Block::Request, AT, None, None).unwrap();
        assert_eq!(second.graph.edges.len(), 1, "Start already has a successor");
        let value = add_node(&graph, Block::Value, AT, None, None).unwrap();
        assert!(value.graph.edges.is_empty(), "only requests and components");
        assert!(matches!(
            add_node(&graph, Block::Start, AT, None, None),
            Err(EditError::SecondStart)
        ));
        assert!(
            add_node(&value.graph, Block::Request, AT, None, None).is_ok_and(|added| added
                .graph
                .edges
                .len()
                == 1)
        );
    }

    #[test]
    fn control_ports_add_ordering_edges_with_branches() {
        let (graph, start) = start_graph();
        let branch = add_node(
            &graph,
            Block::Branch,
            AT,
            None,
            Some(OutputRef {
                node_id: &start,
                port: ORDER_PORT,
            }),
        )
        .unwrap();
        let no = add_node(
            &branch.graph,
            Block::Request,
            AT,
            None,
            Some(OutputRef {
                node_id: &branch.node_id,
                port: FALSE_PORT,
            }),
        )
        .unwrap();
        let edge: &OrderEdge = no.graph.edges.last().unwrap();
        assert_eq!(
            (edge.source.as_str(), edge.target.as_str(), edge.branch),
            (
                branch.node_id.as_str(),
                no.node_id.as_str(),
                Some(Branch::False)
            )
        );
        assert_eq!(no.graph.edges.len(), 2);
    }

    #[test]
    fn data_ports_bind_the_default_input_of_the_new_block() {
        let request = node(NodeKind::Request);
        let graph = Graph {
            nodes: vec![request.clone()],
            edges: vec![],
        };
        let from = Some(OutputRef {
            node_id: &request.id,
            port: "status",
        });
        let fields = |field: &str| vec![field.to_owned()];
        for (block, input, path) in [
            (Block::Branch, "actual", vec![]),
            (Block::Comparison, "actual", vec![]),
            (Block::Condition, "actual1", vec![]),
            (Block::Value, "value", vec![]),
            (Block::Variable, "value", vec![]),
            (Block::Object, "value", fields("field")),
            (Block::Request, "body", vec![]),
        ] {
            let added = add_node(&graph, block, AT, None, from).unwrap();
            let node = added.graph.nodes.last().unwrap();
            assert_eq!(
                node.inputs[input].at(&path),
                Some(&Binding::reference(&request.id, "status", "")),
                "{block:?}"
            );
            assert!(added.graph.edges.is_empty());
        }
        let shop = at("shop.json");
        let latest = latest_components(&shop);
        let with_action = latest.iter().find(|c| c.action.is_some()).unwrap();
        let added = add_node(&graph, Block::Component, AT, Some(with_action), from).unwrap();
        let node = added.graph.nodes.last().unwrap();
        assert_eq!(node.handle, crate::handle::handle_base(&with_action.name));
        assert_eq!(
            node.inputs[with_action.action.as_ref().unwrap().input.as_str()],
            Binding::reference(&request.id, "status", "")
        );
        let without = latest.iter().find(|c| c.action.is_none()).unwrap();
        let added = add_node(&graph, Block::Component, AT, Some(without), from).unwrap();
        assert_eq!(added.graph.edges.len(), 1);
        assert_eq!(added.graph.edges[0].source, request.id);
        assert!(matches!(
            add_node(
                &graph,
                Block::Value,
                AT,
                None,
                Some(OutputRef {
                    node_id: "missing",
                    port: "value"
                })
            ),
            Err(EditError::NodeNotFound)
        ));
    }

    #[test]
    fn new_handles_are_numbered() {
        let (graph, _) = start_graph();
        let first = add_node(&graph, Block::Value, AT, None, None).unwrap();
        let second = add_node(&first.graph, Block::Value, AT, None, None).unwrap();
        let handles: Vec<_> = second.graph.nodes.iter().map(|node| &node.handle).collect();
        assert_eq!(handles, ["begin", "value", "value_2"]);
    }
}
