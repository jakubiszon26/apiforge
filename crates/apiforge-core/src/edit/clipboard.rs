//! Copying, pasting and duplicating selections of blocks.
//!
//! A copied selection pastes into any graph: the same board, the other graph
//! of a scenario, or a graph of another document (UX-34, the owner's decision
//! A of 2026-11-03). Clashing handles are numbered, references inside the
//! selection follow the copies, references to blocks the target also has are
//! kept, and a reference the target cannot satisfy leaves its slot
//! unconnected and is reported, so the caller can say what is missing.
//!
//! The folder window copies, pastes and duplicates statements of the
//! written language (DIAG-06f): [`copy_statements`] takes an `if` with
//! everything in its braces, [`paste_statements`] writes the fragment after
//! a step, in its braces, and keeps a reference only where it still finds
//! its step above the place, and [`duplicate_statements`] writes each copy
//! directly after its original. A paste that would leave a statement reading
//! what is not there is refused and changes nothing. The legacy window keeps
//! [`copy_selection`], [`paste_selection`] and [`duplicate_selection`].
use super::{
    EditError,
    place::{Cycle, cycle_at, goes_with, index_after, inside},
    unique_id,
};
use crate::{
    chain::{Braces, braces, chain, is_step},
    format::{Binding, Graph, Node, OrderEdge, Position, Project},
    graph::has_cycle,
    handle::{unique_handle, unique_label},
    model::{Branch, NodeKind},
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
    /// The braces each copied statement stands in, by identifier, when the
    /// `if` of those braces is copied too (DIAG-06f): the `if` and its
    /// branch. A copied statement without an entry is written in the braces
    /// of the place it is pasted to.
    pub braces: BTreeMap<String, (String, Branch)>,
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
    /// The graph is a component's, which a refusal names (DIAG-06f): *not
    /// in this component* rather than *not in this scenario*.
    pub component: bool,
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
            component: false,
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
        braces: BTreeMap::new(),
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

/// The folder window's *Copy* (DIAG-06f): the selected statements and, for
/// a selected `if`, every statement in its braces at any depth, also when
/// only the `if` is selected. A statement selected without its `if` is
/// copied as a plain statement, which a paste writes in the braces of the
/// place. `reference` is read as by [`copy_selection`].
pub fn copy_statements(graph: &Graph, reference: &Graph, selected: &HashSet<&str>) -> Fragment {
    let written = braces(graph);
    let taken: HashSet<&str> = graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .filter(|id| selected.contains(id) || selected.iter().any(|x| inside(&written, id, x)))
        .collect();
    fragment(graph, reference, &taken, &written)
}

/// The fragment of the statements `taken`, with the braces of those whose
/// `if` is taken too.
fn fragment(
    graph: &Graph,
    reference: &Graph,
    taken: &HashSet<&str>,
    written: &Braces<'_>,
) -> Fragment {
    let mut fragment = copy_selection(graph, reference, taken);
    fragment.braces = fragment
        .graph
        .nodes
        .iter()
        .filter_map(|node| {
            let &(x, branch) = written.get(node.id.as_str())?;
            taken
                .contains(x)
                .then(|| (node.id.clone(), (x.to_owned(), branch)))
        })
        .collect();
    fragment
}

/// The folder window's *Paste* (DIAG-06f): the copies of `fragment` written
/// directly after the step `after`, its braces and the checks and values
/// written under it, in the braces it stands in, as *Add step* puts a step
/// ([`super::add_step_after`]); last in the graph when `after` is `None`.
/// Statements copied with their `if` stay in its braces; the others stand in
/// the braces of the place, in the order they were copied. The ordering
/// edges are one chain per braces ([`crate::chain::chain`]).
///
/// A reference outside the fragment is kept when the target has a statement
/// of that identity or handle with that output that stands above the place;
/// otherwise the paste is refused, naming every statement that would read
/// what is not there, and nothing is pasted. Handles and labels are numbered
/// as by [`paste_selection`]; the copies keep their positions.
pub fn paste_statements(
    target: &PasteTarget<'_>,
    fragment: &Fragment,
    after: Option<&str>,
) -> Result<Pasted, EditError> {
    let graph = target.graph;
    let written = braces(graph);
    let nodes: Vec<&Node> = graph.nodes.iter().collect();
    let (index, place) = match after {
        Some(anchor) => {
            if !graph.nodes.iter().any(|node| node.id == anchor) {
                return Err(EditError::NodeNotFound);
            }
            let place = written.get(anchor).copied();
            (index_after(&nodes, &written, anchor, place), place)
        }
        None => (nodes.len(), None),
    };
    let landing = Landing {
        index,
        braces: place,
        roots: fragment
            .graph
            .nodes
            .iter()
            .filter(|node| !fragment.braces.contains_key(&node.id))
            .map(|node| node.id.as_str())
            .collect(),
    };
    land(target, fragment, &[landing])
}

/// The folder window's *Duplicate* (DIAG-06f): the selection copied as by
/// [`copy_statements`], each selected step with the checks, values and
/// variables written directly under it that read it and no other step, and
/// pasted into the same graph taken per braces: in each braces the copies
/// follow the last selected step of those braces (or the last selected
/// statement, when no step of them is selected), its braces and what is
/// written under it, in their order. Refused as [`paste_statements`] is.
pub fn duplicate_statements(
    target: &PasteTarget<'_>,
    selected: &HashSet<&str>,
) -> Result<Pasted, EditError> {
    let graph = target.graph;
    let written = braces(graph);
    let chosen: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| selected.contains(node.id.as_str()))
        .collect();
    let mut taken: HashSet<&str> = HashSet::new();
    for node in &chosen {
        taken.extend(goes_with(graph, &written, node).into_keys());
    }
    let reference = target.visible.first().copied().unwrap_or(graph);
    let fragment = fragment(graph, reference, &taken, &written);
    // The statements copied without their `if`, by the braces they stand
    // in, each braces in the order it is first written.
    let mut groups: Vec<(Option<Brace<'_>>, Vec<&str>)> = vec![];
    for node in &fragment.graph.nodes {
        if fragment.braces.contains_key(&node.id) {
            continue;
        }
        let own = written.get(node.id.as_str()).copied();
        match groups.iter_mut().find(|(braces, _)| *braces == own) {
            Some((_, roots)) => roots.push(node.id.as_str()),
            None => groups.push((own, vec![node.id.as_str()])),
        }
    }
    let nodes: Vec<&Node> = graph.nodes.iter().collect();
    let mut landings: Vec<Landing<'_>> = groups
        .into_iter()
        .map(|(own, roots)| {
            let selected_here = |node: &&&Node| {
                roots.contains(&node.id.as_str()) && selected.contains(node.id.as_str())
            };
            let step = chosen
                .iter()
                .filter(selected_here)
                .rfind(|node| is_step(node.kind));
            let index = match step {
                Some(step) => index_after(&nodes, &written, &step.id, own),
                None => chosen
                    .iter()
                    .rfind(selected_here)
                    .and_then(|last| nodes.iter().position(|node| node.id == last.id))
                    .map_or(nodes.len(), |position| position + 1),
            };
            Landing {
                index,
                braces: own,
                roots,
            }
        })
        .collect();
    landings.sort_by_key(|landing| landing.index);
    land(target, &fragment, &landings)
}

/// The braces a statement stands in: the `if` and its branch.
type Brace<'a> = (&'a str, Branch);

/// Where a part of a fragment is written: before the statement `index` of
/// the target (after the last when it is their number), in `braces`, the
/// copies of `roots` and of what their braces hold, in fragment order.
struct Landing<'a> {
    index: usize,
    braces: Option<Brace<'a>>,
    roots: Vec<&'a str>,
}

/// What a pasted statement would read that the target does not give.
enum Lack {
    /// No statement of that identity or handle.
    Step(String),
    /// A statement of that handle without that output.
    Output(String, String),
    /// A parameter or an input the target does not declare.
    Parameter(String),
}

/// Writes the copies of `fragment` at their landings, one chain per braces,
/// or refuses with the board's sentence.
fn land(
    target: &PasteTarget<'_>,
    fragment: &Fragment,
    landings: &[Landing<'_>],
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
    // The statement a reference outside the fragment reads in the target:
    // the same one when it is there, otherwise one the code names the same
    // way, with the output read.
    let resolve = |node_id: &str, port: &str| -> Option<String> {
        let reads = |node: &&&Node| output_exists(node, port, target.project);
        let handle = fragment.sources.get(node_id);
        readable
            .iter()
            .filter(reads)
            .find(|node| node.id == node_id)
            .or_else(|| {
                let handle = handle?;
                readable
                    .iter()
                    .filter(reads)
                    .find(|node| &node.handle == handle)
            })
            .map(|node| node.id.clone())
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

    let mut copies: Vec<Node> = Vec::with_capacity(fragment.graph.nodes.len());
    let mut lacks: Vec<(&str, Lack)> = vec![];
    for node in &fragment.graph.nodes {
        let mut copy = node.clone();
        copy.id = mapping[node.id.as_str()].clone();
        copy.handle = unique_handle(&node.handle, |handle| handles.contains(handle));
        handles.insert(copy.handle.clone());
        if let Some(label) = &node.label {
            copy.label = Some(unique_label(label, |label| labels.contains(label)));
        }
        labels.insert(copy.display_name().to_owned());
        for (input, binding) in &node.inputs {
            let remapped = binding.map_leaves(&mut |leaf, _| match leaf {
                Binding::Reference {
                    node_id,
                    port,
                    path,
                } => {
                    let source = mapping
                        .get(node_id.as_str())
                        .cloned()
                        .or_else(|| resolve(node_id, port));
                    match source {
                        Some(source) => Some(Binding::reference(source, port, path)),
                        None => {
                            let name = fragment
                                .sources
                                .get(node_id)
                                .map_or(node_id.as_str(), String::as_str);
                            let named = readable
                                .iter()
                                .any(|node| node.id == *node_id || node.handle == name);
                            lacks.push((
                                &node.handle,
                                if named {
                                    Lack::Output(name.to_owned(), port.clone())
                                } else {
                                    Lack::Step(name.to_owned())
                                },
                            ));
                            Some(leaf.clone())
                        }
                    }
                }
                Binding::Parameter { name, .. }
                    if target
                        .parameters
                        .is_some_and(|parameters| !parameters.contains(name)) =>
                {
                    lacks.push((&node.handle, Lack::Parameter(name.clone())));
                    Some(leaf.clone())
                }
                other => Some(other.clone()),
            });
            if let Some(remapped) = remapped {
                copy.inputs.insert(input.clone(), remapped);
            }
        }
        copies.push(copy);
    }
    if !lacks.is_empty() {
        return Err(EditError::PasteRefused(refusal(&lacks, target.component)));
    }

    // The statements of the result in written order, with their braces.
    let mut placed: HashMap<String, (String, Branch)> = braces(graph)
        .into_iter()
        .map(|(id, (x, branch))| (id.to_owned(), (x.to_owned(), branch)))
        .collect();
    // The statement copied without its `if` whose braces hold `id`.
    fn root_of<'f>(fragment: &'f Fragment, id: &'f str) -> &'f str {
        let mut current = id;
        for _ in 0..=fragment.graph.nodes.len() {
            match fragment.braces.get(current) {
                Some((x, _)) => current = x,
                None => break,
            }
        }
        current
    }
    let mut at: HashMap<usize, Vec<&Node>> = HashMap::new();
    for (node, copy) in fragment.graph.nodes.iter().zip(&copies) {
        // A *Start* is written first, where rule 1 of the language wants it.
        if copy.kind == NodeKind::Start {
            at.entry(0).or_default().push(copy);
            continue;
        }
        let root = root_of(fragment, &node.id);
        let (index, own) = landings
            .iter()
            .find(|landing| landing.roots.contains(&root))
            .map_or((graph.nodes.len(), None), |landing| {
                (landing.index, landing.braces)
            });
        match fragment.braces.get(&node.id) {
            Some((x, branch)) => {
                placed.insert(copy.id.clone(), (mapping[x.as_str()].clone(), *branch));
            }
            None => {
                if let Some((x, branch)) = own {
                    placed.insert(copy.id.clone(), (x.to_owned(), branch));
                }
            }
        }
        at.entry(index).or_default().push(copy);
    }
    let mut ordered = graph.clone();
    ordered.nodes = Vec::with_capacity(graph.nodes.len() + copies.len());
    for index in 0..=graph.nodes.len() {
        if let Some(landed) = at.get(&index) {
            ordered
                .nodes
                .extend(landed.iter().map(|copy| (*copy).clone()));
        }
        if let Some(node) = graph.nodes.get(index) {
            ordered.nodes.push(node.clone());
        }
    }
    let placed: Braces<'_> = placed
        .iter()
        .map(|(id, (x, branch))| (id.as_str(), (x.as_str(), *branch)))
        .collect();
    let result = chain(&ordered, &placed);
    if has_cycle(&result) {
        let pasted: HashSet<&str> = copies.iter().map(|copy| copy.id.as_str()).collect();
        let original: HashMap<&str, &str> = fragment
            .graph
            .nodes
            .iter()
            .zip(&copies)
            .map(|(node, copy)| (copy.id.as_str(), node.handle.as_str()))
            .collect();
        let message = match cycle_at(&result, target.project, |node| {
            pasted.contains(node.id.as_str())
        }) {
            Some(Cycle {
                reader,
                value,
                below: Some(below),
                ..
            }) => format!(
                "`{}` reads `{value}`, which runs after this place. Paste it below `{below}`.",
                original[reader.id.as_str()]
            ),
            _ => EditError::ConnectionCycle.to_string(),
        };
        return Err(EditError::PasteRefused(message));
    }
    let ids = ordered
        .nodes
        .iter()
        .filter(|node| mapping.values().any(|id| *id == node.id))
        .map(|node| node.id.clone())
        .collect();
    Ok(Pasted {
        graph: result,
        ids,
        unresolved: Vec::new(),
    })
}

/// The board's sentence for a paste that would leave statements reading
/// what is not there: the missing steps together, then each output, then
/// the parameters, each reader and each name once, in fragment order.
fn refusal(lacks: &[(&str, Lack)], component: bool) -> String {
    let here = if component { "component" } else { "scenario" };
    fn add<'a>(list: &mut Vec<&'a str>, item: &'a str) {
        if !list.contains(&item) {
            list.push(item);
        }
    }
    let mut sentences = vec![];
    let (mut readers, mut steps) = (vec![], vec![]);
    for (reader, lack) in lacks {
        if let Lack::Step(step) = lack {
            add(&mut readers, reader);
            add(&mut steps, step);
        }
    }
    if !steps.is_empty() {
        let verb = if readers.len() == 1 { "reads" } else { "read" };
        let (is, them) = if steps.len() == 1 {
            ("is", "it")
        } else {
            ("are", "them")
        };
        sentences.push(format!(
            "{} {verb} {}, which {is} not in this {here}. Copy {them} too.",
            listed(&readers),
            listed(&steps)
        ));
    }
    let mut outputs: Vec<String> = vec![];
    for (reader, lack) in lacks {
        if let Lack::Output(step, port) = lack {
            let sentence = format!(
                "`{reader}` reads `{step}.{port}`; `{step}` in this {here} has no `{port}`."
            );
            if !outputs.contains(&sentence) {
                outputs.push(sentence);
            }
        }
    }
    sentences.extend(outputs);
    let (mut readers, mut parameters) = (vec![], vec![]);
    for (reader, lack) in lacks {
        if let Lack::Parameter(name) = lack {
            add(&mut readers, reader);
            add(&mut parameters, name);
        }
    }
    if !parameters.is_empty() {
        let verb = if readers.len() == 1 { "reads" } else { "read" };
        let parameters: Vec<String> = parameters
            .iter()
            .map(|name| format!("param.{name}"))
            .collect();
        let parameters: Vec<&str> = parameters.iter().map(String::as_str).collect();
        sentences.push(format!(
            "{} {verb} {}, which this {here} does not declare.",
            listed(&readers),
            listed(&parameters)
        ));
    }
    sentences.join(" ")
}

/// `a`, `` `a` and `b` ``, `` `a`, `b` and `c` ``.
fn listed(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("`{name}`")).collect();
    match quoted.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
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

#[cfg(test)]
mod diag06f {
    //! DIAG-06f, criteria 1, 2, 4 and 5 on the operations. The documents are
    //! written in the language and lowered by `apiforge-script`, which links
    //! another build of this crate: they cross between the two builds as
    //! JSON. Every edit ends by printing the edited document, reading it back
    //! without an error and as the same document.
    use super::*;
    use crate::{
        chain::{braces, is_step},
        edit::add_step_after,
        format::Scenario,
        graph::sequence,
    };
    use serde::{Serialize, de::DeserializeOwned};

    /// A value of one build of this crate as the same type of the other.
    fn crossed<A: Serialize, B: DeserializeOwned>(value: &A) -> B {
        serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap()
    }

    fn lowered(project: &Project, text: &str) -> apiforge_script::Document {
        let theirs = crossed(project);
        let lowered = apiforge_script::lower(
            &apiforge_script::parse(text),
            &apiforge_script::LowerContext::folder(&theirs),
        );
        assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
        lowered.document.expect("a document")
    }

    fn scenario(project: &Project, text: &str) -> Scenario {
        match lowered(project, text) {
            apiforge_script::Document::Scenario(scenario) => crossed(&scenario),
            other => panic!("not a scenario: {other:?}"),
        }
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/scenarios/lay")
                .join(name),
        )
        .unwrap()
    }

    fn shop() -> (Project, Scenario) {
        let project = crate::demos::shop_project();
        let scenario = scenario(&project, &fixture("shop-30.afs"));
        (project, scenario)
    }

    fn labels() -> (Project, Scenario) {
        let project = crate::demos::shop_project();
        let scenario = scenario(&project, &fixture("labels.afs"));
        (project, scenario)
    }

    fn small(text: &str) -> (Project, Scenario) {
        let project = super::super::test_support::empty_project();
        let scenario = scenario(&project, text);
        (project, scenario)
    }

    fn node<'g>(graph: &'g Graph, handle: &str) -> &'g Node {
        graph
            .nodes
            .iter()
            .find(|node| node.handle == handle)
            .unwrap_or_else(|| panic!("a statement `{handle}`"))
    }

    fn id(graph: &Graph, handle: &str) -> String {
        node(graph, handle).id.clone()
    }

    fn ids<'g>(graph: &'g Graph, handles: &[&str]) -> HashSet<&'g str> {
        handles
            .iter()
            .map(|handle| node(graph, handle).id.as_str())
            .collect()
    }

    fn handles(graph: &Graph, ids: &[String]) -> Vec<String> {
        ids.iter()
            .map(|id| {
                graph
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
                    .unwrap()
                    .handle
                    .clone()
            })
            .collect()
    }

    /// The steps in the order they run.
    fn steps(graph: &Graph) -> Vec<String> {
        sequence(graph)
            .into_iter()
            .filter_map(|id| graph.nodes.iter().find(|node| node.id == id))
            .filter(|node| is_step(node.kind))
            .map(|node| node.handle.clone())
            .collect()
    }

    /// Every statement in written order.
    fn written(graph: &Graph) -> Vec<&str> {
        graph
            .nodes
            .iter()
            .map(|node| node.handle.as_str())
            .collect()
    }

    /// The braces the statement `handle` stands in, by the handle of its
    /// `if`.
    fn braces_of(graph: &Graph, handle: &str) -> Option<(String, Branch)> {
        let written = braces(graph);
        written.get(id(graph, handle).as_str()).map(|(x, branch)| {
            let x = graph.nodes.iter().find(|node| node.id == *x).unwrap();
            (x.handle.clone(), *branch)
        })
    }

    /// The scenario with `graph` as its main flow, printed, read back
    /// without an error and as itself.
    fn reads_back(project: &Project, scenario: &Scenario, graph: Graph) -> Scenario {
        let mut result = scenario.clone();
        result.graph = graph;
        let theirs = crossed(project);
        let document = apiforge_script::Document::Scenario(crossed(&result));
        let text = apiforge_script::print_folder_document(&theirs, &document)
            .expect("the edited document prints");
        let read = lowered(project, &text);
        assert_eq!(
            apiforge_script::semantic(&read, &theirs),
            apiforge_script::semantic(&document, &theirs),
            "the text reads back as the document:\n{text}"
        );
        result
    }

    fn target<'a>(
        graph: &'a Graph,
        project: &'a Project,
        parameters: &'a [String],
    ) -> PasteTarget<'a> {
        PasteTarget {
            parameters: Some(parameters),
            ..PasteTarget::graph(graph, project)
        }
    }

    fn parameters(scenario: &Scenario) -> Vec<String> {
        scenario
            .parameters
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// *Duplicate* of `selected` in the main flow, checked by printing.
    fn duplicated(
        project: &Project,
        scenario: &Scenario,
        selected: &[&str],
    ) -> (Scenario, Vec<String>) {
        let parameters = parameters(scenario);
        let pasted = duplicate_statements(
            &target(&scenario.graph, project, &parameters),
            &ids(&scenario.graph, selected),
        )
        .unwrap_or_else(|error| panic!("{selected:?} duplicates: {error}"));
        let result = reads_back(project, scenario, pasted.graph);
        let copies = handles(&result.graph, &pasted.ids);
        (result, copies)
    }

    /// `fragment` pasted into the main flow of `scenario` after the step
    /// *Add step* names for `selected`.
    fn pasted(
        project: &Project,
        scenario: &Scenario,
        fragment: &Fragment,
        selected: &[&str],
    ) -> Result<(Scenario, Vec<String>), String> {
        let parameters = parameters(scenario);
        let selection: Vec<String> = selected.iter().map(|h| id(&scenario.graph, h)).collect();
        let selection: Vec<&str> = selection.iter().map(String::as_str).collect();
        let after = add_step_after(&scenario.graph, &selection);
        let pasted = paste_statements(
            &target(&scenario.graph, project, &parameters),
            fragment,
            after,
        )
        .map_err(|error| error.to_string())?;
        let result = reads_back(project, scenario, pasted.graph);
        let copies = handles(&result.graph, &pasted.ids);
        Ok((result, copies))
    }

    fn copied(scenario: &Scenario, selected: &[&str]) -> Fragment {
        copy_statements(
            &scenario.graph,
            &scenario.graph,
            &ids(&scenario.graph, selected),
        )
    }

    // Criterion 1 -------------------------------------------------------------

    #[test]
    fn diag06f_criterion_1_one_step_is_duplicated_after_itself_with_its_checks() {
        let (project, shop) = shop();
        let (result, copies) = duplicated(&project, &shop, &["cart"]);
        // `cart_created` reads only `cart`; `cart_total` also reads
        // `expected_total` and is not the step's own.
        assert_eq!(copies, ["cart_2", "cart_created_2"]);
        let order = written(&result.graph);
        let at = |handle: &str| order.iter().position(|h| *h == handle).unwrap();
        assert_eq!(at("cart_2"), at("cart_total") + 1, "{order:?}");
        assert_eq!(at("cart_created_2"), at("cart_2") + 1, "{order:?}");
        assert_eq!(at("cart_view"), at("cart_created_2") + 1, "{order:?}");
        let order = steps(&result.graph);
        let at = |step: &str| order.iter().position(|s| s == step).unwrap();
        assert_eq!(at("cart_2"), at("cart") + 1, "{order:?}");
        assert_eq!(at("cart_view"), at("cart_2") + 1, "{order:?}");
        // The copy reads what the original reads, and its check reads it.
        let (original, copy) = (node(&result.graph, "cart"), node(&result.graph, "cart_2"));
        assert_eq!(original.inputs, copy.inputs);
        assert!(
            node(&result.graph, "cart_created_2").inputs["actual"].references_node(&copy.id),
            "the copied check reads the copy"
        );
    }

    #[test]
    fn diag06f_criterion_1_several_in_one_braces_follow_the_last_selected_step() {
        let (project, shop) = shop();
        let (result, copies) = duplicated(&project, &shop, &["login", "catalogue"]);
        assert_eq!(
            copies,
            [
                "login_2",
                "logged_in_2",
                "known_user_2",
                "catalogue_2",
                "listed_2",
                "first_2",
                "second_2"
            ]
        );
        let order = written(&result.graph);
        let at = |handle: &str| order.iter().position(|h| *h == handle).unwrap();
        assert_eq!(at("login_2"), at("second") + 1, "{order:?}");
        assert_eq!(at("product"), at("second_2") + 1, "{order:?}");
        let order = steps(&result.graph);
        assert_eq!(
            order[..5],
            ["begin", "login", "catalogue", "login_2", "catalogue_2"]
        );
        // A reference inside the selection follows the copies.
        let catalogue = node(&result.graph, "catalogue_2");
        let login = node(&result.graph, "login_2");
        assert!(
            catalogue
                .inputs
                .values()
                .any(|binding| binding.references_node(&login.id)),
            "{:?}",
            catalogue.inputs
        );
    }

    #[test]
    fn diag06f_criterion_1_a_selection_across_yes_and_no_is_taken_per_braces() {
        let (project, shop) = shop();
        let (result, copies) = duplicated(&project, &shop, &["receipt", "not_created"]);
        assert_eq!(copies, ["receipt_2", "not_created_2"]);
        let paid = Some(("paid".to_owned(), Branch::True));
        assert_eq!(braces_of(&result.graph, "receipt_2"), paid);
        assert_eq!(
            braces_of(&result.graph, "not_created_2"),
            Some(("paid".to_owned(), Branch::False))
        );
        let order = written(&result.graph);
        let at = |handle: &str| order.iter().position(|h| *h == handle).unwrap();
        // After `receipt` and `receipt_total`, written under it.
        assert_eq!(at("receipt_2"), at("receipt_total") + 1, "{order:?}");
        assert_eq!(at("not_created_2"), at("not_created") + 1, "{order:?}");
        assert_eq!(
            steps(&result.graph)[10..],
            [
                "paid",
                "payment",
                "receipt",
                "receipt_2",
                "refund",
                "logout"
            ]
        );
    }

    #[test]
    fn diag06f_criterion_1_a_statement_in_a_selected_if_is_copied_once() {
        let (project, shop) = shop();
        let (result, copies) = duplicated(&project, &shop, &["paid", "payment", "not_created"]);
        assert_eq!(
            copies,
            [
                "paid_2",
                "payment_2",
                "receipt_2",
                "receipt_total_2",
                "refund_2",
                "refund_amount_2",
                "logout_2",
                "not_created_2"
            ]
        );
        assert_eq!(result.graph.nodes.len(), shop.graph.nodes.len() + 8);
        assert_eq!(
            steps(&result.graph)[10..],
            [
                "paid",
                "payment",
                "receipt",
                "refund",
                "logout",
                "paid_2",
                "payment_2",
                "receipt_2",
                "refund_2",
                "logout_2"
            ]
        );
    }

    // Criterion 2 -------------------------------------------------------------

    const NESTED: &str = r#"scenario "Nested" {
  begin = start
  a = GET "/a" {
    sample 200 { ok: true, id: 1 }
  }
  x = if a.body.ok {
    b = GET "/b/{{a.body.id}}" {
      sample 200 { ok: true }
    }
    b_ok = assert b.status == 200
    y = if b.body.ok {
      c = GET "/c" {
        sample 200 {}
      }
    } else {
      d = GET "/d" {
        sample 200 {}
      }
    }
  } else {
    e = GET "/e" {
      sample 200 {}
    }
  }
  f = GET "/f" {
    sample 200 {}
  }
}
"#;

    #[test]
    fn fragment_takes_the_braces() {
        let (project, scenario) = small(NESTED);
        let fragment = copied(&scenario, &["x"]);
        let copied: Vec<&str> = fragment
            .graph
            .nodes
            .iter()
            .map(|node| node.handle.as_str())
            .collect();
        assert_eq!(copied, ["x", "b", "b_ok", "y", "c", "d", "e"]);
        let brace = |handle: &str| -> Option<(&str, Branch)> {
            let copy = node(&fragment.graph, handle);
            fragment.braces.get(&copy.id).map(|(x, branch)| {
                (
                    fragment
                        .graph
                        .nodes
                        .iter()
                        .find(|node| &node.id == x)
                        .unwrap()
                        .handle
                        .as_str(),
                    *branch,
                )
            })
        };
        assert_eq!(brace("x"), None, "the `if` itself is pasted at the place");
        assert_eq!(brace("b"), Some(("x", Branch::True)));
        assert_eq!(brace("y"), Some(("x", Branch::True)));
        assert_eq!(brace("c"), Some(("y", Branch::True)));
        assert_eq!(brace("d"), Some(("y", Branch::False)));
        assert_eq!(brace("e"), Some(("x", Branch::False)));

        // Pasted after `f`: the copy's branches hold what the original's hold.
        let (result, copies) = pasted(&project, &scenario, &fragment, &["f"]).unwrap();
        assert_eq!(copies, ["x_2", "b_2", "b_ok_2", "y_2", "c_2", "d_2", "e_2"]);
        assert_eq!(braces_of(&result.graph, "x_2"), None);
        for (handle, x, branch) in [
            ("b_2", "x_2", Branch::True),
            ("b_ok_2", "x_2", Branch::True),
            ("y_2", "x_2", Branch::True),
            ("c_2", "y_2", Branch::True),
            ("d_2", "y_2", Branch::False),
            ("e_2", "x_2", Branch::False),
        ] {
            assert_eq!(
                braces_of(&result.graph, handle),
                Some((x.to_owned(), branch)),
                "{handle}"
            );
        }
        assert_eq!(
            steps(&result.graph),
            [
                "begin", "a", "x", "b", "y", "c", "d", "e", "f", "x_2", "b_2", "y_2", "c_2", "d_2",
                "e_2"
            ]
        );
        // `b_2` reads `a`, which stands above the place.
        assert!(
            node(&result.graph, "b_2")
                .inputs
                .values()
                .any(|binding| binding.references_node(&id(&result.graph, "a")))
        );
    }

    #[test]
    fn fragment_without_its_if_is_plain_steps() {
        let (project, scenario) = small(NESTED);
        let fragment = copied(&scenario, &["b", "c"]);
        assert!(fragment.braces.is_empty(), "{:?}", fragment.braces);
        let (result, copies) = pasted(&project, &scenario, &fragment, &["f"]).unwrap();
        assert_eq!(copies, ["b_2", "c_2"]);
        assert_eq!(braces_of(&result.graph, "b_2"), None);
        assert_eq!(braces_of(&result.graph, "c_2"), None);
        assert_eq!(
            steps(&result.graph),
            [
                "begin", "a", "x", "b", "y", "c", "d", "e", "f", "b_2", "c_2"
            ]
        );
        // Into the braces of the place: after `b`, they stand in *Yes* of `x`.
        let (result, _) = pasted(&project, &scenario, &fragment, &["b"]).unwrap();
        assert_eq!(
            braces_of(&result.graph, "b_2"),
            Some(("x".to_owned(), Branch::True))
        );
        assert_eq!(
            braces_of(&result.graph, "c_2"),
            Some(("x".to_owned(), Branch::True))
        );
        let order = written(&result.graph);
        let at = |handle: &str| order.iter().position(|h| *h == handle).unwrap();
        assert_eq!(at("b_2"), at("b_ok") + 1, "{order:?}");
        assert_eq!(at("c_2"), at("b_2") + 1, "{order:?}");
    }

    // Criterion 4 -------------------------------------------------------------

    #[test]
    fn paste_keeps_a_reference_whose_step_stands_above_the_place() {
        let (project, shop) = shop();
        let fragment = copied(&shop, &["cart_view"]);
        let (result, copies) = pasted(&project, &shop, &fragment, &["address"]).unwrap();
        assert_eq!(copies, ["cart_view_2"]);
        assert_eq!(
            node(&result.graph, "cart_view").inputs,
            node(&result.graph, "cart_view_2").inputs,
            "it reads `cart` and `login`, as the original does"
        );
        let order = steps(&result.graph);
        let at = |step: &str| order.iter().position(|s| s == step).unwrap();
        assert_eq!(at("cart_view_2"), at("address") + 1, "{order:?}");
    }

    #[test]
    fn paste_keeps_a_reference_by_handle_in_another_document() {
        let (project, shop) = shop();
        let fragment = copied(&shop, &["cart_created"]);
        let (other_project, other) = small(
            r#"scenario "Other" {
  begin = start
  cart = POST "/carts" {
    sample 201 { id: 7 }
  }
}
"#,
        );
        let _ = project;
        let (result, copies) = pasted(&other_project, &other, &fragment, &[]).unwrap();
        assert_eq!(copies, ["cart_created"]);
        assert!(
            node(&result.graph, "cart_created").inputs["actual"]
                .references_node(&id(&result.graph, "cart")),
            "it reads the `cart` of this scenario"
        );
    }

    #[test]
    fn paste_keeps_a_reference_of_a_nested_slot() {
        // `cart_total` reads `cart` and `expected_total` inside its
        // comparison; pasted after `checkout`, both stand above it.
        let (project, shop) = shop();
        let fragment = copied(&shop, &["cart_total"]);
        let (result, _) = pasted(&project, &shop, &fragment, &["checkout"]).unwrap();
        assert_eq!(
            node(&result.graph, "cart_total").inputs,
            node(&result.graph, "cart_total_2").inputs
        );
    }

    // Criterion 5 -------------------------------------------------------------

    const OTHER: &str = r#"scenario "Other" {
  begin = start
  login = POST "/login" {
    sample 200 { token: "t" }
  }
  subtotal = GET "/subtotal" {
    sample 200 { value: 5 }
  }
}
"#;

    #[test]
    fn paste_refuses_a_step_that_is_not_there() {
        let (_, shop) = shop();
        let (project, other) = small(OTHER);
        // `cart_view` reads `login`, which the other scenario has, and
        // `cart`, which it lacks.
        let fragment = copied(&shop, &["cart_view"]);
        assert_eq!(
            pasted(&project, &other, &fragment, &[]).unwrap_err(),
            "`cart_view` reads `cart`, which is not in this scenario. Copy it too."
        );
        // Two readers of two missing steps.
        let fragment = copied(&shop, &["cart_view", "checkout"]);
        assert_eq!(
            pasted(&project, &other, &fragment, &[]).unwrap_err(),
            "`cart_view` and `checkout` read `cart`, `valid_cart` and `expected_total`, which \
             are not in this scenario. Copy them too."
        );
    }

    #[test]
    fn paste_refuses_a_step_of_that_name_without_that_output() {
        let (source_project, source) = small(
            r#"scenario "Source" {
  begin = start
  subtotal = 5
  total = assert subtotal == 5
}
"#,
        );
        let _ = source_project;
        let (project, other) = small(OTHER);
        // `subtotal` here is a request, which has no `value`.
        let fragment = copied(&source, &["total"]);
        assert_eq!(
            pasted(&project, &other, &fragment, &[]).unwrap_err(),
            "`total` reads `subtotal.value`; `subtotal` in this scenario has no `value`."
        );
    }

    #[test]
    fn paste_refuses_a_parameter_the_target_does_not_declare() {
        let (_, labels) = labels();
        let (project, shop) = shop();
        let fragment = copied(&labels, &["no_token"]);
        assert_eq!(
            pasted(&project, &shop, &fragment, &[]).unwrap_err(),
            "`no_token` reads `param.parcel_id`, which this scenario does not declare."
        );
        // A component's input is named so in a component.
        let parameters: Vec<String> = vec![];
        let component = PasteTarget {
            component: true,
            ..target(&shop.graph, &project, &parameters)
        };
        assert_eq!(
            paste_statements(&component, &fragment, None)
                .unwrap_err()
                .to_string(),
            "`no_token` reads `param.parcel_id`, which this component does not declare."
        );
    }

    #[test]
    fn paste_refuses_a_nested_binding_that_does_not_resolve() {
        let (_, shop) = shop();
        let (project, other) = small(
            r#"scenario "Other" {
  begin = start
  cart = POST "/carts" {
    sample 201 { total: 7 }
  }
}
"#,
        );
        // `cart_total` compares `cart.body.total` with `expected_total`.
        let fragment = copied(&shop, &["cart_total"]);
        assert_eq!(
            pasted(&project, &other, &fragment, &[]).unwrap_err(),
            "`cart_total` reads `expected_total`, which is not in this scenario. Copy it too."
        );
    }

    #[test]
    fn paste_refuses_a_step_below_the_place() {
        let (project, shop) = shop();
        // Pasted after `login`, `cart_view` would run before `cart`.
        let fragment = copied(&shop, &["cart_view"]);
        assert_eq!(
            pasted(&project, &shop, &fragment, &["login"]).unwrap_err(),
            "`cart_view` reads `cart.body.id`, which runs after this place. Paste it below \
             `cart`."
        );
    }

    #[test]
    fn paste_refuses_a_second_start() {
        let (project, shop) = shop();
        let fragment = copied(&shop, &["begin", "login"]);
        assert!(matches!(
            paste_statements(&target(&shop.graph, &project, &[]), &fragment, None),
            Err(EditError::PasteSecondStart)
        ));
    }
}
