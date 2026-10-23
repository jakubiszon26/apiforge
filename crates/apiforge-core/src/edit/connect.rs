//! Creating and removing wires between blocks.
use super::{EditError, unique_id};
use crate::{
    binding::data_edge_id,
    format::{Binding, Graph, OrderEdge, Project},
    graph::{FALSE_PORT, ORDER_PORT, TRUE_PORT, compatible, has_cycle, input_slots, output_ports},
    model::{Branch, NodeKind},
};
use std::collections::HashSet;

/// The branch an ordering edge leaving `port` takes, or `None` for a data port.
pub(super) fn control_branch(port: &str) -> Option<Option<Branch>> {
    match port {
        ORDER_PORT => Some(None),
        TRUE_PORT => Some(Some(Branch::True)),
        FALSE_PORT => Some(Some(Branch::False)),
        _ => None,
    }
}

/// A new ordering edge with an identifier unique among `graph`'s edges.
pub(super) fn order_edge(
    graph: &Graph,
    source: &str,
    target: &str,
    branch: Option<Branch>,
) -> OrderEdge {
    OrderEdge {
        id: unique_id("order", |id| graph.edges.iter().any(|edge| edge.id == id)),
        source: source.to_owned(),
        target: target.to_owned(),
        branch,
    }
}

/// Connects `source`'s output to a slot of `target` (see [`input_slots`]): an
/// input, or with a non-empty `target_path` an operand, field or item of a Value.
///
/// A control pseudo-port (`__order`, `__true`, `__false`) wired to the `__order`
/// input adds an ordering edge; IF blocks must use a branch port and other blocks
/// must not. Any other pair binds the slot to a reference to the output, provided
/// the ports are compatible and the slot has no reference yet. A connection that
/// would make the graph cyclic is rejected.
pub fn connect(
    graph: &Graph,
    project: &Project,
    source: &str,
    source_port: &str,
    target: &str,
    target_input: &str,
    target_path: &[String],
) -> Result<Graph, EditError> {
    if source == target {
        return Err(EditError::SelfConnection);
    }
    let from = graph.nodes.iter().find(|node| node.id == source);
    let to = graph.nodes.iter().find(|node| node.id == target);
    let (Some(from), Some(to)) = (from, to) else {
        return Err(EditError::NodeNotFound);
    };
    let mut next = graph.clone();
    match control_branch(source_port) {
        Some(branch) if target_input == ORDER_PORT => {
            if to.kind == NodeKind::Trigger {
                return Err(EditError::StartHasPredecessor);
            }
            if (from.kind == NodeKind::If) != branch.is_some() {
                return Err(EditError::BranchRequired);
            }
            if graph
                .edges
                .iter()
                .any(|edge| edge.source == source && edge.target == target && edge.branch == branch)
            {
                return Err(EditError::DuplicateConnection);
            }
            next.edges.push(order_edge(graph, source, target, branch));
        }
        _ => {
            let output = output_ports(from, project)
                .into_iter()
                .find(|port| port.name == source_port);
            let input = input_slots(to, project)
                .into_iter()
                .find(|slot| slot.input == target_input && slot.path == target_path);
            match (output, input) {
                (Some(output), Some(input)) if compatible(&output, &input.port) => {}
                _ => return Err(EditError::IncompatiblePorts),
            }
            let current = to.inputs.get(target_input);
            if current
                .and_then(|binding| binding.at(target_path))
                .is_some_and(|binding| !binding.references().is_empty())
            {
                return Err(EditError::InputAlreadyBound);
            }
            let reference = Binding::reference(source, source_port, "");
            let binding = match current {
                Some(current) => current
                    .replace_at(target_path, reference)
                    .ok_or(EditError::InvalidOperandPath)?,
                None if target_path.is_empty() => reference,
                None => return Err(EditError::InvalidOperandPath),
            };
            if let Some(node) = next.nodes.iter_mut().find(|node| node.id == target) {
                node.inputs.insert(target_input.to_owned(), binding);
            }
        }
    }
    if has_cycle(&next) {
        return Err(EditError::ConnectionCycle);
    }
    Ok(next)
}

/// Removes wires by their board identifiers.
///
/// Ordering edges are matched by edge id; a data wire id (see
/// [`data_edge_id`]) removes only that reference leaf, keeping the operators
/// around it. An input whose root reference is removed disappears.
pub fn remove_connections(graph: &Graph, wire_ids: &HashSet<&str>) -> Graph {
    Graph {
        edges: graph
            .edges
            .iter()
            .filter(|edge| !wire_ids.contains(edge.id.as_str()))
            .cloned()
            .collect(),
        nodes: graph
            .nodes
            .iter()
            .map(|node| {
                let mut node = node.clone();
                node.inputs = node
                    .inputs
                    .iter()
                    .filter_map(|(name, binding)| {
                        let next = binding.map_leaves(&mut |leaf, path| {
                            let id = data_edge_id(&node.id, name, path);
                            (!wire_ids.contains(id.as_str())).then(|| leaf.clone())
                        })?;
                        Some((name.clone(), next))
                    })
                    .collect();
                node
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        edit::test_support::{at, empty_project, left, node, trio},
        graph::{WireKind, wires},
    };
    use serde_json::json;

    #[test]
    fn connections_carry_reference_data_and_occupied_inputs_are_kept() {
        let project = empty_project();
        let f = trio();
        let connected = connect(
            &f.graph,
            &project,
            &f.request,
            "status",
            &f.formula,
            "value",
            &left(),
        )
        .expect("compatible ports connect");
        assert_eq!(
            connected.nodes[1].inputs["value"].at(&left()),
            Some(&Binding::reference(&f.request, "status", ""))
        );
        assert!(connected.edges.is_empty());
        assert_eq!(wires(&connected).len(), 1);
        let error = connect(
            &connected,
            &project,
            &f.request,
            "duration",
            &f.formula,
            "value",
            &left(),
        )
        .unwrap_err();
        assert!(matches!(error, EditError::InputAlreadyBound));
        assert!(error.to_string().contains("already has a source"));
        assert_eq!(
            f.graph.nodes[1].inputs["value"].at(&left()),
            Some(&Binding::literal(json!(0)))
        );
    }

    #[test]
    fn incompatible_types_self_connections_and_mixed_cycles_are_rejected() {
        let project = empty_project();
        let f = trio();
        assert!(matches!(
            connect(
                &f.graph,
                &project,
                &f.request,
                "headers",
                &f.formula,
                "value",
                &left()
            ),
            Err(EditError::IncompatiblePorts)
        ));
        let forward = connect(
            &f.graph,
            &project,
            &f.request,
            "status",
            &f.formula,
            "value",
            &left(),
        )
        .unwrap();
        let error = connect(
            &forward,
            &project,
            &f.formula,
            ORDER_PORT,
            &f.request,
            ORDER_PORT,
            &[],
        )
        .unwrap_err();
        assert!(matches!(error, EditError::ConnectionCycle));
        assert!(error.to_string().contains("cycle"));
        assert!(matches!(
            connect(
                &f.graph,
                &project,
                &f.formula,
                "value",
                &f.formula,
                "value",
                &left()
            ),
            Err(EditError::SelfConnection)
        ));
        assert!(matches!(
            connect(
                &f.graph,
                &project,
                "missing",
                "value",
                &f.formula,
                "value",
                &left()
            ),
            Err(EditError::NodeNotFound)
        ));
    }

    #[test]
    fn control_connections_follow_start_and_branch_rules() {
        let project = empty_project();
        let start = node(NodeKind::Trigger);
        let branch = node(NodeKind::If);
        let request = node(NodeKind::Request);
        let graph = Graph {
            nodes: vec![start.clone(), branch.clone(), request.clone()],
            edges: vec![],
        };
        let connect_order = |graph: &Graph, source: &str, port: &str, target: &str| {
            connect(graph, &project, source, port, target, ORDER_PORT, &[])
        };
        assert!(matches!(
            connect_order(&graph, &request.id, ORDER_PORT, &start.id),
            Err(EditError::StartHasPredecessor)
        ));
        assert!(matches!(
            connect_order(&graph, &branch.id, ORDER_PORT, &request.id),
            Err(EditError::BranchRequired)
        ));
        assert!(matches!(
            connect_order(&graph, &start.id, TRUE_PORT, &request.id),
            Err(EditError::BranchRequired)
        ));
        let started = connect_order(&graph, &start.id, ORDER_PORT, &branch.id).unwrap();
        assert_eq!(started.edges[0].branch, None);
        assert!(started.edges[0].id.starts_with("order-"));
        let yes = connect_order(&started, &branch.id, TRUE_PORT, &request.id).unwrap();
        assert_eq!(yes.edges[1].branch, Some(Branch::True));
        assert!(matches!(
            connect_order(&yes, &branch.id, TRUE_PORT, &request.id),
            Err(EditError::DuplicateConnection)
        ));
        let both = connect_order(&yes, &branch.id, FALSE_PORT, &request.id).unwrap();
        assert_eq!(both.edges[2].branch, Some(Branch::False));
        // A control port wired to a data input is not a valid data connection.
        assert!(matches!(
            connect(
                &graph,
                &project,
                &start.id,
                ORDER_PORT,
                &request.id,
                "body",
                &[]
            ),
            Err(EditError::IncompatiblePorts)
        ));
    }

    #[test]
    fn removing_a_data_wire_clears_only_that_operand() {
        let project = empty_project();
        let f = trio();
        let first = connect(
            &f.graph,
            &project,
            &f.request,
            "status",
            &f.formula,
            "value",
            &left(),
        )
        .unwrap();
        let right = ["right".to_owned()];
        let second = connect(
            &first, &project, &f.request, "duration", &f.formula, "value", &right,
        )
        .unwrap();
        let id = format!("data:{}:value:left", f.formula);
        let result = remove_connections(&second, &HashSet::from([id.as_str()]));
        assert_eq!(result.nodes[1].inputs["value"].at(&left()), None);
        assert!(matches!(
            result.nodes[1].inputs["value"].at(&right),
            Some(Binding::Reference { .. })
        ));
    }

    #[test]
    fn nested_wires_and_order_edges_are_removed_precisely() {
        let project = at("blocks.json");
        let graph = &project.scenarios[0].graph;
        let edges: Vec<_> = wires(graph)
            .into_iter()
            .filter(|wire| wire.target == "name-check")
            .collect();
        assert_eq!(edges.len(), 2);
        assert_ne!(edges[0].id, edges[1].id);
        let disconnected = remove_connections(graph, &HashSet::from([edges[0].id.as_str()]));
        assert_eq!(
            wires(&disconnected)
                .iter()
                .filter(|wire| wire.target == "name-check")
                .count(),
            1
        );
        assert!(disconnected.nodes[1].inputs["actual"].is_expression());

        let workflow = at("workflow.json");
        let graph = &workflow.scenarios[0].graph;
        let order = graph.edges[0].id.clone();
        let removed = remove_connections(graph, &HashSet::from([order.as_str()]));
        assert_eq!(removed.edges.len(), graph.edges.len() - 1);
        assert_eq!(
            wires(&removed)
                .iter()
                .filter(|wire| wire.kind == WireKind::Data)
                .count(),
            wires(graph)
                .iter()
                .filter(|wire| wire.kind == WireKind::Data)
                .count()
        );
    }
}
