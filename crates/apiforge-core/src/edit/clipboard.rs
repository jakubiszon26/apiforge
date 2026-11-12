//! Copying, pasting and duplicating selections of blocks.
//!
//! A copied selection pastes into any graph: the same board, the other graph
//! of a scenario, or a graph of another document (UX-34, the owner's decision
//! A of 2026-11-03). Clashing handles are numbered, references inside the
//! selection follow the copies, references to blocks the target also has are
//! kept, and a reference the target cannot satisfy leaves its slot
//! unconnected and is reported, so the caller can say what is missing.
use super::{EditError, unique_id};
use crate::{
    format::{Binding, Graph, Node, OrderEdge, Position, Project},
    handle::{unique_handle, unique_label},
    model::NodeKind,
    validation::output_exists,
};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Default distance of pasted copies from their originals.
pub const PASTE_OFFSET: Position = Position { x: 40.0, y: 50.0 };

/// A frozen copy of the selected blocks and the ordering edges between them.
///
/// It is an owned copy: later edits of the originals do not change it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fragment {
    pub graph: Graph,
    /// The handles of the blocks outside the selection that its bindings read,
    /// by identifier, so a paste into another graph can find those blocks by
    /// the name the code gives them.
    pub sources: BTreeMap<String, String>,
}

/// Where a fragment is pasted.
#[derive(Clone, Copy, Debug)]
pub struct PasteTarget<'a> {
    pub graph: &'a Graph,
    /// Other graphs whose blocks the pasted bindings may read: the main graph
    /// on a cleanup board.
    pub visible: &'a [&'a Graph],
    /// Other graphs whose handles and labels the copies must not repeat: the
    /// other graph of a scenario, which shares its handles.
    pub reserved: &'a [&'a Graph],
    /// The scenario's parameters or the component's inputs, when the paste
    /// should check them; `None` keeps every parameter reference.
    pub parameters: Option<&'a [String]>,
    pub project: &'a Project,
}

impl<'a> PasteTarget<'a> {
    /// The graph alone, with no other graph around it.
    pub fn graph(graph: &'a Graph, project: &'a Project) -> Self {
        Self {
            graph,
            visible: &[],
            reserved: &[],
            parameters: None,
            project,
        }
    }
}

/// What a pasted binding read that the target does not have.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Missing {
    /// A step, by its handle where it was copied.
    Step(String),
    /// A scenario parameter or a component input, by name.
    Input(String),
}

/// A reference a paste could not keep: its slot is left unconnected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved {
    /// The pasted block whose input held the reference.
    pub node_id: String,
    pub input: String,
    /// The slot inside the input that held the reference; empty when it was
    /// the whole input.
    pub path: Vec<String>,
    pub missing: Missing,
}

/// The board after a paste and the identifiers of the new blocks, in fragment order.
#[derive(Clone, Debug, PartialEq)]
pub struct Pasted {
    pub graph: Graph,
    pub ids: Vec<String>,
    /// References the target could not satisfy, each left unconnected.
    pub unresolved: Vec<Unresolved>,
}

/// A frozen fragment of the selected blocks and the ordering edges between them.
///
/// `reference` holds the blocks the selection may read besides `graph`'s own
/// (the main graph on a cleanup board); the handles of those it reads are
/// recorded so a paste elsewhere can find them by name.
pub fn copy_selection(graph: &Graph, reference: &Graph, selected: &HashSet<&str>) -> Fragment {
    let nodes: Vec<Node> = graph
        .nodes
        .iter()
        .filter(|node| selected.contains(node.id.as_str()))
        .cloned()
        .collect();
    let sources = nodes
        .iter()
        .flat_map(|node| node.inputs.values())
        .flat_map(Binding::references)
        .filter(|read| !selected.contains(read.node_id))
        .filter_map(|read| {
            graph
                .nodes
                .iter()
                .chain(&reference.nodes)
                .find(|node| node.id == read.node_id)
                .map(|node| (node.id.clone(), node.handle.clone()))
        })
        .collect();
    let edges = graph
        .edges
        .iter()
        .filter(|edge| {
            selected.contains(edge.source.as_str()) && selected.contains(edge.target.as_str())
        })
        .cloned()
        .collect();
    Fragment {
        graph: Graph {
            // A pasted block is a new block, which no copied proxy could point at.
            proxies: Vec::new(),
            nodes,
            edges,
        },
        sources,
    }
}

/// Adds independent copies of `fragment` to the target graph.
///
/// Copies get fresh identifiers, handles numbered where the target or its
/// reserved graphs use them (`product_2`), labels numbered where another block
/// shows them (`Add to cart 2`), and positions moved by `offset`. References
/// between copied blocks, including nested operator leaves, follow the copies.
/// A reference to a block outside the fragment is kept when the target can
/// read a block of that identity or handle with that output; otherwise, like a
/// parameter the target lacks, its slot is left unconnected and reported in
/// [`Pasted::unresolved`]. A board can hold only one Start.
pub fn paste_selection(
    target: &PasteTarget<'_>,
    fragment: &Fragment,
    offset: &Position,
) -> Result<Pasted, EditError> {
    let graph = target.graph;
    let has_start = |graph: &Graph| graph.nodes.iter().any(|node| node.kind == NodeKind::Start);
    if has_start(&fragment.graph) && has_start(graph) {
        return Err(EditError::PasteSecondStart);
    }
    let mut mapping: HashMap<&str, String> = HashMap::new();
    for node in &fragment.graph.nodes {
        let id = unique_id("node", |id| {
            graph.nodes.iter().any(|node| node.id == id)
                || mapping.values().any(|taken| taken == id)
        });
        mapping.insert(&node.id, id);
    }
    let readable: Vec<&Node> = graph
        .nodes
        .iter()
        .chain(target.visible.iter().flat_map(|graph| &graph.nodes))
        .collect();
    // The block a reference outside the fragment reads in the target: the same
    // block when it is there, otherwise one the code names the same way.
    let resolve = |node_id: &str, port: &str| -> Result<String, Missing> {
        let handle = fragment.sources.get(node_id);
        let reads = |node: &&&Node| output_exists(node, port, target.project);
        readable
            .iter()
            .filter(reads)
            .find(|node| node.id == node_id)
            .or_else(|| {
                handle.and_then(|handle| {
                    readable
                        .iter()
                        .filter(reads)
                        .find(|node| &node.handle == handle)
                })
            })
            .map(|node| node.id.clone())
            .ok_or_else(|| Missing::Step(handle.map_or(node_id, String::as_str).to_owned()))
    };
    let others = || {
        graph
            .nodes
            .iter()
            .chain(target.reserved.iter().flat_map(|graph| &graph.nodes))
            .chain(target.visible.iter().flat_map(|graph| &graph.nodes))
    };
    let mut handles: HashSet<String> = others().map(|node| node.handle.clone()).collect();
    let mut labels: HashSet<String> = others()
        .map(|node| node.display_name().to_owned())
        .collect();

    let mut next = graph.clone();
    let mut ids = Vec::with_capacity(fragment.graph.nodes.len());
    let mut unresolved = Vec::new();
    for node in &fragment.graph.nodes {
        let mut copy = node.clone();
        copy.id = mapping[node.id.as_str()].clone();
        copy.handle = unique_handle(&node.handle, |handle| handles.contains(handle));
        handles.insert(copy.handle.clone());
        if let Some(label) = &node.label {
            let label = unique_label(label, |label| labels.contains(label));
            copy.label = Some(label);
        }
        labels.insert(copy.display_name().to_owned());
        copy.position = Position {
            x: node.position.x + offset.x,
            y: node.position.y + offset.y,
        };
        let mut inputs = copy.inputs.clone();
        for (input, binding) in &copy.inputs {
            let mut missing = Vec::new();
            let remapped = binding.map_leaves(&mut |leaf, slot| match leaf {
                Binding::Reference {
                    node_id,
                    port,
                    path,
                } => {
                    let source = match mapping.get(node_id.as_str()) {
                        Some(copied) => Ok(copied.clone()),
                        None => resolve(node_id, port),
                    };
                    match source {
                        Ok(source) => Some(Binding::reference(source, port, path)),
                        Err(lost) => {
                            missing.push((lost, slot.to_vec()));
                            None
                        }
                    }
                }
                Binding::Parameter { name, .. }
                    if target
                        .parameters
                        .is_some_and(|parameters| !parameters.contains(name)) =>
                {
                    missing.push((Missing::Input(name.clone()), slot.to_vec()));
                    None
                }
                other => Some(other.clone()),
            });
            match remapped {
                Some(remapped) => {
                    inputs.insert(input.clone(), remapped);
                }
                None => {
                    inputs.shift_remove(input);
                }
            }
            unresolved.extend(missing.into_iter().map(|(missing, path)| Unresolved {
                node_id: copy.id.clone(),
                input: input.clone(),
                path,
                missing,
            }));
        }
        copy.inputs = inputs;
        ids.push(copy.id.clone());
        next.nodes.push(copy);
    }
    for edge in &fragment.graph.edges {
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
    Ok(Pasted {
        graph: next,
        ids,
        unresolved,
    })
}

/// Copies the selection and pastes it into the same graph at the default
/// offset.
pub fn duplicate_selection(
    graph: &Graph,
    project: &Project,
    selected: &HashSet<&str>,
) -> Result<Pasted, EditError> {
    paste_selection(
        &PasteTarget::graph(graph, project),
        &copy_selection(graph, graph, selected),
        &PASTE_OFFSET,
    )
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

    fn graph(nodes: Vec<Node>) -> Graph {
        Graph {
            nodes,
            edges: vec![],
            proxies: Vec::new(),
        }
    }

    fn named(kind: NodeKind, handle: &str, label: Option<&str>) -> Node {
        let mut node = node(kind);
        node.handle = handle.into();
        node.label = label.map(Into::into);
        node
    }

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
            &project,
            &HashSet::from([f.formula.as_str(), f.assertion.as_str()]),
        )
        .unwrap();
        assert_eq!(copy.graph.nodes.len(), 5);
        assert!(copy.unresolved.is_empty());
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
        assert_eq!(formula.label, original.label, "no label to number");
        assert!(
            copy.graph
                .edges
                .iter()
                .any(|edge| edge.source == formula.id && edge.target == assertion.id)
        );
    }

    #[test]
    fn the_clipboard_is_frozen_and_every_paste_is_independent() {
        let project = empty_project();
        let mut source = node(NodeKind::Component);
        source.config.component_id = Some("custom".into());
        source.config.revision = Some(7);
        source.inputs.insert(
            "filter".into(),
            Binding::literal(json!({"active": true, "fields": ["name", "price"]})),
        );
        let original = source.clone();
        let mut board = graph(vec![source]);
        let fragment = copy_selection(&board, &board, &HashSet::from([original.id.as_str()]));
        board.nodes[0].config.revision = Some(8);
        board.nodes[0]
            .inputs
            .insert("filter".into(), Binding::literal(json!({"active": false})));

        let first = paste_selection(
            &PasteTarget::graph(&board, &project),
            &fragment,
            &PASTE_OFFSET,
        )
        .unwrap();
        let copied = first.graph.nodes.last().unwrap();
        assert_eq!(copied.config, original.config);
        assert_eq!(copied.inputs, original.inputs);
        let offset = Position { x: 80.0, y: 100.0 };
        let second = paste_selection(
            &PasteTarget::graph(&first.graph, &project),
            &fragment,
            &offset,
        )
        .unwrap();
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
        assert_eq!(fragment.graph.nodes[0], original);
    }

    #[test]
    fn pasting_a_group_rewires_nested_data_and_branches() {
        let project = empty_project();
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
        let mut source = graph(vec![external.clone(), branch.clone(), target.clone()]);
        source.edges.push(OrderEdge {
            id: "else".into(),
            source: branch.id.clone(),
            target: target.id.clone(),
            branch: Some(Branch::False),
        });
        let fragment = copy_selection(
            &source,
            &source,
            &HashSet::from([branch.id.as_str(), target.id.as_str()]),
        );
        // Deleting the originals does not invalidate the references within the fragment.
        let board = graph(vec![external.clone()]);
        let result = paste_selection(
            &PasteTarget::graph(&board, &project),
            &fragment,
            &PASTE_OFFSET,
        )
        .unwrap();
        assert!(result.unresolved.is_empty());
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

        // A graph without the external block leaves its operand unconnected
        // and says which step is missing.
        let empty = graph(vec![]);
        let result = paste_selection(
            &PasteTarget::graph(&empty, &project),
            &fragment,
            &PASTE_OFFSET,
        )
        .unwrap();
        let copied_target = &result.graph.nodes[1];
        assert_eq!(
            result.unresolved,
            [Unresolved {
                node_id: copied_target.id.clone(),
                input: "body".into(),
                path: vec!["right".into()],
                missing: Missing::Step(external.handle.clone()),
            }]
        );
        let input = &copied_target.inputs["body"];
        assert_eq!(input.at(&["right".into()]), None, "unconnected");
        assert!(
            input.at(&["left".into()]).is_some(),
            "the copied wire stays"
        );
        // A graph the board can read, as cleanup reads main, keeps it.
        let main = graph(vec![external.clone()]);
        let cleanup = PasteTarget {
            visible: &[&main],
            ..PasteTarget::graph(&empty, &project)
        };
        let result = paste_selection(&cleanup, &fragment, &PASTE_OFFSET).unwrap();
        assert!(result.unresolved.is_empty());
    }

    /// UX-34: a paste into the other graph of a scenario numbers the handles
    /// and labels that graph already uses.
    #[test]
    fn pasting_beside_a_sibling_graph_numbers_its_handles_and_labels() {
        let project = empty_project();
        let price = named(NodeKind::Value, "price", Some("Price"));
        let main = graph(vec![price.clone()]);
        let fragment = copy_selection(&main, &main, &HashSet::from([price.id.as_str()]));
        let cleanup = graph(vec![]);
        let target = PasteTarget {
            reserved: &[&main],
            ..PasteTarget::graph(&cleanup, &project)
        };
        let pasted = paste_selection(&target, &fragment, &PASTE_OFFSET).unwrap();
        let copy = &pasted.graph.nodes[0];
        assert_eq!(copy.handle, "price_2");
        assert_eq!(copy.label.as_deref(), Some("Price 2"));
        // Pasted again beside both, the next numbers.
        let twice = graph(vec![copy.clone()]);
        let target = PasteTarget {
            reserved: &[&main],
            ..PasteTarget::graph(&twice, &project)
        };
        let again = paste_selection(&target, &fragment, &PASTE_OFFSET).unwrap();
        let copy = &again.graph.nodes[1];
        assert_eq!(copy.handle, "price_3");
        assert_eq!(copy.label.as_deref(), Some("Price 3"));
    }

    /// UX-34: in another document a reference finds the step the code names
    /// the same way; a step or parameter that is not there is reported.
    #[test]
    fn references_follow_handles_into_another_document() {
        let project = empty_project();
        let product = named(NodeKind::Value, "product", Some("Product"));
        let mut check = named(NodeKind::Assert, "check", Some("Check"));
        check.inputs.insert(
            "actual".into(),
            Binding::reference(&product.id, "value", "price"),
        );
        check
            .inputs
            .insert("expected".into(), Binding::parameter("quantity"));
        let source = graph(vec![product.clone(), check.clone()]);
        let fragment = copy_selection(&source, &source, &HashSet::from([check.id.as_str()]));
        assert_eq!(
            fragment.sources,
            BTreeMap::from([(product.id.clone(), "product".to_owned())])
        );

        let other = named(NodeKind::Value, "product", None);
        let document = graph(vec![other.clone()]);
        let parameters = ["quantity".to_owned()];
        let target = PasteTarget {
            parameters: Some(&parameters),
            ..PasteTarget::graph(&document, &project)
        };
        let pasted = paste_selection(&target, &fragment, &PASTE_OFFSET).unwrap();
        assert!(pasted.unresolved.is_empty());
        let copy = &pasted.graph.nodes[1];
        assert_eq!(
            copy.inputs["actual"],
            Binding::reference(&other.id, "value", "price")
        );
        assert_eq!(copy.handle, "check");
        assert_eq!(copy.label.as_deref(), Some("Check"));

        let empty = graph(vec![]);
        let target = PasteTarget {
            parameters: Some(&[]),
            ..PasteTarget::graph(&empty, &project)
        };
        let pasted = paste_selection(&target, &fragment, &PASTE_OFFSET).unwrap();
        let copy = &pasted.graph.nodes[0];
        assert!(copy.inputs.is_empty(), "{:?}", copy.inputs);
        let missing: Vec<_> = pasted
            .unresolved
            .iter()
            .map(|unresolved| (unresolved.input.as_str(), unresolved.missing.clone()))
            .collect();
        assert_eq!(
            missing,
            [
                ("actual", Missing::Step("product".into())),
                ("expected", Missing::Input("quantity".into())),
            ]
        );
    }

    /// A block with the copied identity but without the output read gives
    /// way to the block the code names the same way.
    #[test]
    fn a_reference_falls_back_to_the_handle_when_the_identity_cannot_serve() {
        let project = empty_project();
        let product = named(NodeKind::Value, "product", None);
        let mut check = named(NodeKind::Assert, "check", None);
        check.inputs.insert(
            "actual".into(),
            Binding::reference(&product.id, "value", ""),
        );
        let source = graph(vec![product.clone(), check.clone()]);
        let fragment = copy_selection(&source, &source, &HashSet::from([check.id.as_str()]));
        let mut start = named(NodeKind::Start, "begin", None);
        start.id.clone_from(&product.id);
        let other = named(NodeKind::Value, "product", None);
        let document = graph(vec![start, other.clone()]);
        let pasted = paste_selection(
            &PasteTarget::graph(&document, &project),
            &fragment,
            &PASTE_OFFSET,
        )
        .unwrap();
        assert!(pasted.unresolved.is_empty());
        assert_eq!(
            pasted.graph.nodes[2].inputs["actual"],
            Binding::reference(&other.id, "value", "")
        );
    }

    #[test]
    fn pasting_start_cannot_create_a_second_trigger() {
        let start = node(NodeKind::Start);
        let board = graph(vec![start.clone()]);
        let error = duplicate_selection(
            &board,
            &empty_project(),
            &HashSet::from([start.id.as_str()]),
        )
        .unwrap_err();
        assert!(matches!(error, EditError::PasteSecondStart));
        assert!(error.to_string().contains("already has a Start"));
    }

    #[test]
    fn duplicating_nested_references_keeps_their_dependencies() {
        let project = at("blocks.json");
        let graph = &project.scenarios[0].graph;
        assert_eq!(dependencies(graph, "name-check"), ["response"]);
        let duplicate =
            duplicate_selection(graph, &project, &HashSet::from(["response", "name-check"]))
                .unwrap();
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
