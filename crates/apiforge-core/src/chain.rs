//! The chain of the written language (LANG-13, ADR 0005): the steps of a
//! block run in the order they are written. A *step* is a statement that
//! does something or routes the flow — *Start*, a request, a component call,
//! an `if`; checks and values are data statements and run when what they read
//! exists (RUN-03).
//!
//! The document stays a graph of ordering edges. In a written graph every
//! statement in braces carries the edge of its braces, each step follows the
//! step written before it in the same braces (after an `if`, the ends of its
//! branches, LANG-14) and there is no other ordering edge. Lowering builds
//! that graph from the text with [`chain`]; a board edit and a model written
//! as a source for the first time rebuild it with the same function, so the
//! model and the text say the same thing.
use crate::{
    branches::Branches,
    format::{Graph, Node, OrderEdge, Position},
    graph::{create_node, dependency_index, sequence},
    handle::unique_handle,
    model::{Branch, NodeKind},
};
use std::collections::{HashMap, HashSet, VecDeque};

/// A statement that does something or routes the flow: *Start*, a request,
/// a component call, an `if`.
pub fn is_step(kind: NodeKind) -> bool {
    !kind.is_data()
}

/// The braces each statement stands in, by node id: the `if` and its branch.
/// A statement written outside braces has no entry.
pub type Braces<'g> = HashMap<&'g str, (&'g str, Branch)>;

/// The braces of every statement of `graph`, as [`Branches`] reads them.
pub fn braces(graph: &Graph) -> Braces<'_> {
    let branches = Branches::new(graph);
    graph
        .nodes
        .iter()
        .filter_map(|node| Some((node.id.as_str(), branches.parent(&node.id)?)))
        .collect()
}

/// `graph` with its ordering edges rebuilt as the written language reads
/// them: every statement in braces keeps the edge of its `braces`, every step
/// follows the step before it in `graph.nodes` within the same braces, and
/// no other ordering edge is kept. An edge that already exists keeps its
/// identifier.
pub fn chain(graph: &Graph, braces: &Braces<'_>) -> Graph {
    let kinds: HashMap<&str, NodeKind> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.kind))
        .collect();
    let mut blocks: HashMap<Option<(&str, Branch)>, Vec<&str>> = HashMap::new();
    for node in &graph.nodes {
        if is_step(node.kind) {
            blocks
                .entry(braces.get(node.id.as_str()).copied())
                .or_default()
                .push(node.id.as_str());
        }
    }
    let mut wanted: Vec<(&str, &str, Option<Branch>)> = vec![];
    for node in &graph.nodes {
        let id = node.id.as_str();
        let own = braces.get(id).copied();
        if let Some((x, branch)) = own {
            wanted.push((x, id, Some(branch)));
        }
        if !is_step(node.kind) {
            continue;
        }
        let block = &blocks[&own];
        let index = block.iter().position(|&step| step == id).unwrap_or(0);
        if let Some(previous) = index.checked_sub(1).map(|index| block[index]) {
            for (source, branch) in ends(previous, &kinds, &blocks) {
                wanted.push((source, id, branch));
            }
        }
    }
    let existing: HashMap<(&str, &str, Option<Branch>), &str> = graph
        .edges
        .iter()
        .map(|edge| {
            (
                (edge.source.as_str(), edge.target.as_str(), edge.branch),
                edge.id.as_str(),
            )
        })
        .collect();
    let mut taken: HashSet<String> = HashSet::new();
    let mut seen = HashSet::new();
    let mut edges = Vec::with_capacity(wanted.len());
    let mut fresh = vec![];
    for key in wanted {
        if !seen.insert(key) {
            continue;
        }
        let (source, target, branch) = key;
        match existing.get(&key) {
            Some(&id) if taken.insert(id.to_owned()) => edges.push(OrderEdge {
                id: id.to_owned(),
                source: source.into(),
                target: target.into(),
                branch,
            }),
            _ => {
                fresh.push(edges.len());
                edges.push(OrderEdge {
                    id: String::new(),
                    source: source.into(),
                    target: target.into(),
                    branch,
                });
            }
        }
    }
    let mut number = 0;
    for index in fresh {
        let id = loop {
            let id = format!("order-{number}");
            number += 1;
            if !taken.contains(&id) && !existing.values().any(|&used| used == id) {
                break id;
            }
        };
        taken.insert(id.clone());
        edges[index].id = id;
    }
    Graph {
        nodes: graph.nodes.clone(),
        edges,
        proxies: graph.proxies.clone(),
    }
}

/// Where a step ends: the step itself, or for an `if` the last steps of its
/// branches, the branch itself when its braces hold no step.
fn ends<'g>(
    step: &'g str,
    kinds: &HashMap<&str, NodeKind>,
    blocks: &HashMap<Option<(&'g str, Branch)>, Vec<&'g str>>,
) -> Vec<(&'g str, Option<Branch>)> {
    if kinds.get(step) != Some(&NodeKind::If) {
        return vec![(step, None)];
    }
    let mut result = vec![];
    for branch in [Branch::True, Branch::False] {
        match blocks
            .get(&Some((step, branch)))
            .and_then(|block| block.last())
        {
            Some(last) => result.extend(ends(last, kinds, blocks)),
            None => result.push((step, Some(branch))),
        }
    }
    let mut seen = HashSet::new();
    result.retain(|end| seen.insert(*end));
    result
}

/// Why a model cannot be written in the written language without changing
/// what runs. Each names the step, by node id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unchained {
    /// The step runs nowhere today: nothing orders it after *Start*.
    RunsNowhere(String),
    /// The step's ordering is not a chain with `if`s: it follows a branch
    /// it does not stand in, or a step of other braces.
    NotAChain(String),
}

impl Unchained {
    /// The node id of the step.
    pub fn step(&self) -> &str {
        match self {
            Self::RunsNowhere(step) | Self::NotAChain(step) => step,
        }
    }
}

/// `graph` written as one chain per braces in the order its steps run today
/// ([`sequence`]), its nodes in that order: what the written language says
/// for a model that was not written in it (a demo, a legacy workspace). The
/// braces and the order of the steps do not change. A step that runs nowhere,
/// or whose ordering is not a chain with `if`s, is refused by name.
pub fn chained(graph: &Graph) -> Result<Graph, Unchained> {
    let kinds: HashMap<&str, NodeKind> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.kind))
        .collect();
    let branches = Branches::new(graph);
    let active = active(graph);
    let order = sequence(graph);
    let mut braces = braces(graph);
    for &id in &order {
        let kind = kinds[id];
        if !is_step(kind) || kind == NodeKind::Start {
            continue;
        }
        if active.as_ref().is_some_and(|active| !active.contains(id)) {
            return Err(Unchained::RunsNowhere(id.to_owned()));
        }
        let sources = graph
            .edges
            .iter()
            .filter(|edge| edge.target == id)
            .map(|edge| (edge.source.as_str(), edge.branch))
            .collect();
        let sources = branches.shorten(sources);
        // A step ordered only after steps of one pair of braces stands in
        // them: a model drawn before LANG-14 gave such a step no edge of its
        // branch, and it runs on that branch only. An ordering after a check
        // or a value only orders (FLOW-00, F5) and is dropped.
        if !braces.contains_key(id)
            && let Some(&(first, None)) = sources.first()
            && let Some(&inner) = braces.get(first)
            && sources.iter().all(|&(source, branch)| {
                branch.is_none()
                    && kinds.get(source).is_some_and(|kind| is_step(*kind))
                    && braces.get(source) == Some(&inner)
            })
        {
            braces.insert(id, inner);
        }
        let own = braces.get(id).copied();
        let expressible = sources.into_iter().all(|(source, branch)| match branch {
            Some(branch) => own == Some((source, branch)),
            None => {
                kinds.get(source).is_none_or(|kind| kind.is_data())
                    || braces.get(source).copied() == own
            }
        });
        if !expressible {
            return Err(Unchained::NotAChain(id.to_owned()));
        }
    }
    let position: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(index, &id)| (id, index))
        .collect();
    let mut ordered = graph.clone();
    ordered.nodes.sort_by_key(|node| position[node.id.as_str()]);
    let result = chain(&ordered, &braces);
    let steps = |graph: &Graph| -> Vec<String> {
        sequence(graph)
            .into_iter()
            .filter(|id| kinds.get(id).is_some_and(|&kind| is_step(kind)))
            .map(str::to_owned)
            .collect()
    };
    let (before, after) = (steps(graph), steps(&result));
    if let Some((step, _)) = before.iter().zip(&after).find(|(a, b)| a != b) {
        return Err(Unchained::NotAChain(step.clone()));
    }
    Ok(result)
}

/// `graph` with a *Start* written first and chained before its first step,
/// when it holds a step and no *Start*: rule 1 of the written language. The
/// new block's identifier and handle are the first free ones, so the same
/// graph always gets the same *Start*.
pub fn with_start(graph: &Graph) -> Graph {
    if graph.nodes.iter().any(|node| node.kind == NodeKind::Start)
        || !graph.nodes.iter().any(|node| is_step(node.kind))
    {
        return graph.clone();
    }
    let first = graph
        .nodes
        .iter()
        .find(|node| is_step(node.kind))
        .map_or(Position { x: 0.0, y: 0.0 }, |node| node.position.clone());
    let left = graph
        .nodes
        .iter()
        .map(|node| node.position.x)
        .fold(first.x, f64::min);
    let mut start: Node = create_node(
        NodeKind::Start,
        Position {
            x: left - 200.0,
            y: first.y,
        },
        None,
    );
    let taken: HashSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    start.id = (1..)
        .map(|number| {
            if number == 1 {
                "start".to_owned()
            } else {
                format!("start-{number}")
            }
        })
        .find(|id| !taken.contains(id.as_str()))
        .expect("a free identifier");
    start.handle = unique_handle(NodeKind::Start.handle_base(), |handle| {
        graph.nodes.iter().any(|node| node.handle == handle)
    });
    let mut result = graph.clone();
    result.nodes.insert(0, start);
    let braces = braces(graph);
    chain(&result, &braces)
}

/// The nodes a run reaches from *Start*, as the runner reads them
/// (`apiforge-runner/src/plan.rs`): a node that follows or reads a reached
/// node is reached; the values a reached node reads are computed for it but
/// reach nothing further. `None` for a graph without *Start*, where every
/// statement runs.
pub fn active(graph: &Graph) -> Option<HashSet<&str>> {
    let start = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Start)?;
    let dependencies = dependency_index(graph);
    let mut successors: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in &graph.nodes {
        for &source in dependencies.get(node.id.as_str()).into_iter().flatten() {
            successors.entry(source).or_default().push(&node.id);
        }
    }
    let mut active = HashSet::from([start.id.as_str()]);
    let mut queue = VecDeque::from([start.id.as_str()]);
    while let Some(id) = queue.pop_front() {
        for &candidate in successors.get(id).into_iter().flatten() {
            if active.insert(candidate) {
                queue.push_back(candidate);
            }
        }
    }
    Some(active)
}

/// `after`, a board edit of `before`, with the chain kept: its ordering
/// edges rebuilt from the order of its steps in `graph.nodes` and the braces
/// they stand in, so the model says what its text says. A step added from
/// the library is written last and runs last, one added from a step's *Next*
/// is written after that step and runs after it, a removed step joins its
/// neighbours. An ordering edge the edit adds between two blocks that both
/// existed before, or takes away while both remain, is undone first: what
/// such a wire means is DIAG-06's (#391), and until then it changes nothing,
/// so a statement never leaves its braces by it.
pub fn edited(before: &Graph, after: &Graph) -> Graph {
    let existed: HashSet<&str> = before.nodes.iter().map(|node| node.id.as_str()).collect();
    let remains: HashSet<&str> = after.nodes.iter().map(|node| node.id.as_str()).collect();
    let key = |edge: &OrderEdge| (edge.source.clone(), edge.target.clone(), edge.branch);
    let had: HashSet<_> = before.edges.iter().map(key).collect();
    let has: HashSet<_> = after.edges.iter().map(key).collect();
    let mut result = after.clone();
    result.edges.retain(|edge| {
        had.contains(&key(edge))
            || !existed.contains(edge.source.as_str())
            || !existed.contains(edge.target.as_str())
    });
    for edge in &before.edges {
        if !has.contains(&key(edge))
            && remains.contains(edge.source.as_str())
            && remains.contains(edge.target.as_str())
        {
            result.edges.push(edge.clone());
        }
    }
    // A block that existed keeps the braces it was written in: read from
    // the edited graph, an edge from the empty branch of an `if` before it
    // would look like the edge of that branch. When its `if` is gone it
    // stands where the `if` stood.
    let written = braces(before);
    let read = braces(&result);
    let mut kept: Braces<'_> = HashMap::new();
    for node in &result.nodes {
        let id = node.id.as_str();
        let mut own = if existed.contains(id) {
            written.get(id).copied()
        } else {
            read.get(id).copied()
        };
        while let Some((x, _)) = own.filter(|(x, _)| !remains.contains(x)) {
            own = written.get(x).copied();
        }
        if let Some(own) = own {
            kept.insert(id, own);
        }
    }
    chain(&result, &kept)
}
