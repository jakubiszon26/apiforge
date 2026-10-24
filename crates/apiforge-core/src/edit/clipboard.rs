//! Copying, pasting and duplicating selections of blocks.
use super::{EditError, unique_id};
use crate::{
    format::{Binding, Graph, OrderEdge, Position},
    handle::unique_handle,
    model::NodeKind,
};
use std::collections::{HashMap, HashSet};

/// Default distance of pasted copies from their originals.
pub const PASTE_OFFSET: Position = Position { x: 40.0, y: 50.0 };

/// The board after a paste and the identifiers of the new blocks, in fragment order.
#[derive(Clone, Debug, PartialEq)]
pub struct Pasted {
    pub graph: Graph,
    pub ids: Vec<String>,
}

/// A frozen fragment of the selected blocks and the ordering edges between them.
///
/// The fragment is an owned copy: later edits of the originals do not change it.
pub fn copy_selection(graph: &Graph, selected: &HashSet<&str>) -> Graph {
    Graph {
        // A pasted block is a new block, which no copied proxy could point at.
        proxies: Vec::new(),
        nodes: graph
            .nodes
            .iter()
            .filter(|node| selected.contains(node.id.as_str()))
            .cloned()
            .collect(),
        edges: graph
            .edges
            .iter()
            .filter(|edge| {
                selected.contains(edge.source.as_str()) && selected.contains(edge.target.as_str())
            })
            .cloned()
            .collect(),
    }
}

/// Adds independent copies of `fragment` to `graph`.
///
/// Copies get fresh identifiers, names suffixed with " — copy" and positions moved
/// by `offset`. References between copied blocks, including nested operator
/// leaves, follow the copies; other references keep their producer, which must
/// exist on the board or be listed in `external_ids` (e.g. a main-graph block
/// visible from cleanup). A board can hold only one Start.
pub fn paste_selection(
    graph: &Graph,
    fragment: &Graph,
    offset: &Position,
    external_ids: &[&str],
) -> Result<Pasted, EditError> {
    let has_start = |graph: &Graph| graph.nodes.iter().any(|node| node.kind == NodeKind::Start);
    if has_start(fragment) && has_start(graph) {
        return Err(EditError::PasteSecondStart);
    }
    let mut mapping: HashMap<&str, String> = HashMap::new();
    for node in &fragment.nodes {
        let id = unique_id("node", |id| {
            graph.nodes.iter().any(|node| node.id == id)
                || mapping.values().any(|taken| taken == id)
        });
        mapping.insert(&node.id, id);
    }
    let available = |id: &str| {
        mapping.contains_key(id)
            || external_ids.contains(&id)
            || graph.nodes.iter().any(|node| node.id == id)
    };
    let missing_source = fragment.nodes.iter().any(|node| {
        node.inputs
            .values()
            .flat_map(Binding::references)
            .any(|reference| !available(reference.node_id))
    });
    if missing_source {
        return Err(EditError::PasteMissingSource);
    }

    let mut next = graph.clone();
    let mut ids = Vec::with_capacity(fragment.nodes.len());
    for node in &fragment.nodes {
        let mut copy = node.clone();
        copy.id = mapping[node.id.as_str()].clone();
        copy.handle = unique_handle(&node.handle, |handle| {
            next.nodes.iter().any(|node| node.handle == handle)
        });
        copy.position = Position {
            x: node.position.x + offset.x,
            y: node.position.y + offset.y,
        };
        for binding in copy.inputs.values_mut() {
            let remapped = binding.map_leaves(&mut |leaf, _| {
                Some(match leaf {
                    Binding::Reference {
                        node_id,
                        port,
                        path,
                    } => Binding::reference(
                        mapping.get(node_id.as_str()).unwrap_or(node_id),
                        port,
                        path,
                    ),
                    other => other.clone(),
                })
            });
            if let Some(remapped) = remapped {
                *binding = remapped;
            }
        }
        ids.push(copy.id.clone());
        next.nodes.push(copy);
    }
    for edge in &fragment.edges {
        // Fragments from `copy_selection` only hold internal edges; skip any other.
        let (Some(source), Some(target)) = (
            mapping.get(edge.source.as_str()),
            mapping.get(edge.target.as_str()),
        ) else {
            continue;
        };
        let id = unique_id("order", |id| next.edges.iter().any(|edge| edge.id == id));
        next.edges.push(OrderEdge {
            id,
            source: source.clone(),
            target: target.clone(),
            branch: edge.branch,
        });
    }
    Ok(Pasted { graph: next, ids })
}

/// Copies the selection and pastes it at the default offset.
pub fn duplicate_selection(graph: &Graph, selected: &HashSet<&str>) -> Result<Pasted, EditError> {
    paste_selection(graph, &copy_selection(graph, selected), &PASTE_OFFSET, &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        edit::{
            connect,
            test_support::{at, empty_project, left, node, trio},
        },
        graph::{ORDER_PORT, dependencies, wires},
        model::Branch,
        operator::Operator,
    };
    use serde_json::json;

    #[test]
    fn duplicate_remaps_internal_links_and_keeps_external_references() {
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
        let second = connect(
            &first,
            &project,
            &f.formula,
            "value",
            &f.assertion,
            "actual",
            &[],
        )
        .unwrap();
        let ordered = connect(
            &second,
            &project,
            &f.formula,
            ORDER_PORT,
            &f.assertion,
            ORDER_PORT,
            &[],
        )
        .unwrap();
        let copy = duplicate_selection(
            &ordered,
            &HashSet::from([f.formula.as_str(), f.assertion.as_str()]),
        )
        .unwrap();
        assert_eq!(copy.graph.nodes.len(), 5);
        let find = |id: &str| copy.graph.nodes.iter().find(|node| node.id == id).unwrap();
        let (formula, assertion) = (find(&copy.ids[0]), find(&copy.ids[1]));
        assert_eq!(
            formula.inputs["value"].at(&left()),
            Some(&Binding::reference(&f.request, "status", ""))
        );
        assert_eq!(
            assertion.inputs["actual"],
            Binding::reference(&formula.id, "value", "")
        );
        let original = ordered
            .nodes
            .iter()
            .find(|node| node.id == f.formula)
            .unwrap();
        assert_eq!(
            formula.handle,
            format!("{}_2", original.handle),
            "numbered handle"
        );
        assert_eq!(formula.label, original.label);
        assert!(
            copy.graph
                .edges
                .iter()
                .any(|edge| edge.source == formula.id && edge.target == assertion.id)
        );
    }

    #[test]
    fn the_clipboard_is_frozen_and_every_paste_is_independent() {
        let mut source = node(NodeKind::Component);
        source.config.component_id = Some("custom".into());
        source.config.revision = Some(7);
        source.inputs.insert(
            "filter".into(),
            Binding::literal(json!({"active": true, "fields": ["name", "price"]})),
        );
        let original = source.clone();
        let mut graph = Graph {
            nodes: vec![source],
            edges: vec![],
            proxies: Vec::new(),
        };
        let fragment = copy_selection(&graph, &HashSet::from([original.id.as_str()]));
        graph.nodes[0].config.revision = Some(8);
        graph.nodes[0]
            .inputs
            .insert("filter".into(), Binding::literal(json!({"active": false})));

        let first = paste_selection(&graph, &fragment, &PASTE_OFFSET, &[]).unwrap();
        let copied = first.graph.nodes.last().unwrap();
        assert_eq!(copied.config, original.config);
        assert_eq!(copied.inputs, original.inputs);
        let offset = Position { x: 80.0, y: 100.0 };
        let second = paste_selection(&first.graph, &fragment, &offset, &[]).unwrap();
        let again = second.graph.nodes.last().unwrap();
        assert_ne!(again.id, copied.id);
        assert_eq!(again.config, original.config);
        assert_eq!(
            again.position,
            Position {
                x: original.position.x + 80.0,
                y: original.position.y + 100.0
            }
        );
        assert_eq!(fragment.nodes[0], original);
    }

    #[test]
    fn pasting_a_group_rewires_nested_data_and_branches() {
        let external = node(NodeKind::Value);
        let branch = node(NodeKind::If);
        let mut target = node(NodeKind::Request);
        let body = Binding::expression(Operator::Add)
            .replace_at(
                &["left".into()],
                Binding::reference(&branch.id, "passed", ""),
            )
            .unwrap()
            .replace_at(
                &["right".into()],
                Binding::reference(&external.id, "value", ""),
            )
            .unwrap();
        target.inputs.insert("body".into(), body);
        let graph = Graph {
            nodes: vec![external.clone(), branch.clone(), target.clone()],
            edges: vec![OrderEdge {
                id: "else".into(),
                source: branch.id.clone(),
                target: target.id.clone(),
                branch: Some(Branch::False),
            }],
            proxies: Vec::new(),
        };
        let fragment = copy_selection(
            &graph,
            &HashSet::from([branch.id.as_str(), target.id.as_str()]),
        );
        // Deleting the originals does not invalidate the references within the fragment.
        let board = Graph {
            nodes: vec![external.clone()],
            edges: vec![],
            proxies: Vec::new(),
        };
        let result = paste_selection(&board, &fragment, &PASTE_OFFSET, &[]).unwrap();
        let (copied_branch, copied_target) = (&result.graph.nodes[1], &result.graph.nodes[2]);
        assert_eq!(
            result.graph.edges[0],
            OrderEdge {
                id: result.graph.edges[0].id.clone(),
                source: copied_branch.id.clone(),
                target: copied_target.id.clone(),
                branch: Some(Branch::False),
            }
        );
        let input = &copied_target.inputs["body"];
        assert_eq!(
            input.at(&["left".into()]),
            Some(&Binding::reference(&copied_branch.id, "passed", ""))
        );
        assert_eq!(
            input.at(&["right".into()]),
            Some(&Binding::reference(&external.id, "value", ""))
        );

        let empty = Graph {
            nodes: vec![],
            edges: vec![],
            proxies: Vec::new(),
        };
        let error = paste_selection(&empty, &fragment, &PASTE_OFFSET, &[]).unwrap_err();
        assert!(matches!(error, EditError::PasteMissingSource));
        assert!(error.to_string().contains("data source"));
        assert!(paste_selection(&empty, &fragment, &PASTE_OFFSET, &[&external.id]).is_ok());
    }

    #[test]
    fn pasting_start_cannot_create_a_second_trigger() {
        let start = node(NodeKind::Start);
        let graph = Graph {
            nodes: vec![start.clone()],
            edges: vec![],
            proxies: Vec::new(),
        };
        let error = duplicate_selection(&graph, &HashSet::from([start.id.as_str()])).unwrap_err();
        assert!(matches!(error, EditError::PasteSecondStart));
        assert!(error.to_string().contains("already has a Start"));
    }

    #[test]
    fn duplicating_nested_references_keeps_their_dependencies() {
        let project = at("blocks.json");
        let graph = &project.scenarios[0].graph;
        assert_eq!(dependencies(graph, "name-check"), ["response"]);
        let duplicate =
            duplicate_selection(graph, &HashSet::from(["response", "name-check"])).unwrap();
        assert_eq!(
            dependencies(&duplicate.graph, &duplicate.ids[1]),
            [duplicate.ids[0].as_str()]
        );
        assert_eq!(
            wires(&duplicate.graph).len(),
            wires(graph).len() + 2,
            "both nested wires of the copy are remapped"
        );
    }
}
