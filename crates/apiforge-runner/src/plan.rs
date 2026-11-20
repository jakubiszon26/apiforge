//! Dependency indexes, Start reachability and the braces of `if`s of one
//! graph, built once per execution.

use apiforge_core::{
    format::Graph,
    graph::dependency_index,
    model::{Branch, NodeKind},
};
use indexmap::IndexSet;
use std::collections::{HashMap, HashSet, VecDeque};

/// What the scheduler needs to know about a graph before executing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionPlan<'a> {
    /// Every node a node waits for: data references and ordering edges.
    pub dependencies: HashMap<&'a str, IndexSet<&'a str>>,
    /// Nodes on the active path from Start; `None` for a legacy graph without Start,
    /// where every root runs.
    pub reachable: Option<HashSet<&'a str>>,
    /// The braces each statement stands in directly: the `if` and its branch.
    pub braces: HashMap<&'a str, (&'a str, Branch)>,
}

impl<'a> ExecutionPlan<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        let dependencies = dependency_index(graph);
        let reachable = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Start)
            .map(|trigger| reachable_from(graph, &dependencies, &trigger.id));
        Self {
            dependencies,
            reachable,
            braces: braces(graph),
        }
    }

    /// Dependencies of a node; empty for an unknown node.
    pub fn dependencies_of(&self, node_id: &str) -> impl Iterator<Item = &'a str> + '_ {
        self.dependencies
            .get(node_id)
            .into_iter()
            .flat_map(|sources| sources.iter().copied())
    }

    /// Whether the node is on the active path; always true in a legacy graph.
    pub fn is_reachable(&self, node_id: &str) -> bool {
        self.reachable
            .as_ref()
            .is_none_or(|reachable| reachable.contains(node_id))
    }

    /// The `if`s whose braces hold the node, the innermost first.
    pub fn enclosing(&self, node_id: &str) -> impl Iterator<Item = (&'a str, Branch)> + '_ {
        let mut current = self.braces.get(node_id).copied();
        // A chain longer than the graph would be a cycle, which braces never form.
        let mut left = self.braces.len();
        std::iter::from_fn(move || {
            let found = current.filter(|_| left > 0)?;
            left -= 1;
            current = self.braces.get(found.0).copied();
            Some(found)
        })
    }
}

/// The braces each statement stands in, read from the ordering edges as the
/// script crate's `branches` reads them: a statement stands in the braces of
/// branch `b` of `x` when it follows `x.b` and everything else it follows
/// stands in the same braces, at any depth.
fn braces(graph: &Graph) -> HashMap<&str, (&str, Branch)> {
    let ifs: HashSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::If)
        .map(|node| node.id.as_str())
        .collect();
    let mut incoming: HashMap<&str, Vec<(&str, Option<Branch>)>> = HashMap::new();
    for edge in &graph.edges {
        incoming
            .entry(edge.target.as_str())
            .or_default()
            .push((edge.source.as_str(), edge.branch));
    }
    let mut parents: HashMap<&str, (&str, Branch)> = HashMap::new();
    let within = |parents: &HashMap<&str, (&str, Branch)>, id: &str, braces| {
        let mut current = id;
        for _ in 0..=graph.nodes.len() {
            match parents.get(current) {
                Some(&parent) if parent == braces => return true,
                Some(&(source, _)) => current = source,
                None => return false,
            }
        }
        false
    };
    // Sources come first, so their braces are known when a target asks.
    for id in topological(graph) {
        let Some(edges) = incoming.get(id) else {
            continue;
        };
        let parent = edges
            .iter()
            .filter_map(|&(source, branch)| Some((source, branch?)))
            .filter(|(source, _)| ifs.contains(source))
            .find(|&(braces, branch)| {
                edges.iter().all(|&(source, other)| {
                    (source, other) == (braces, Some(branch))
                        || within(&parents, source, (braces, branch))
                })
            });
        if let Some(parent) = parent {
            parents.insert(id, parent);
        }
    }
    parents
}

/// Nodes with every ordering source before its target, ties in document
/// order. Nodes of a cycle are left out.
fn topological(graph: &Graph) -> Vec<&str> {
    let mut waiting: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), 0))
        .collect();
    let mut targets: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        if waiting.contains_key(edge.source.as_str())
            && let Some(count) = waiting.get_mut(edge.target.as_str())
        {
            *count += 1;
            targets
                .entry(edge.source.as_str())
                .or_default()
                .push(edge.target.as_str());
        }
    }
    let mut ready: VecDeque<&str> = graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .filter(|id| waiting[id] == 0)
        .collect();
    let mut order = Vec::with_capacity(graph.nodes.len());
    while let Some(id) = ready.pop_front() {
        order.push(id);
        for &target in targets.get(id).into_iter().flatten() {
            let count = waiting.get_mut(target).expect("a known target");
            *count -= 1;
            if *count == 0 {
                ready.push_back(target);
            }
        }
    }
    order
}

/// Walks successors from Start. Pure value producers a reached node depends on are
/// pulled in as arguments; requests and components must themselves be on the path,
/// so a reference never silently enables another side effect.
fn reachable_from<'a>(
    graph: &'a Graph,
    dependencies: &HashMap<&'a str, IndexSet<&'a str>>,
    start: &'a str,
) -> HashSet<&'a str> {
    let kinds: HashMap<&str, NodeKind> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.kind))
        .collect();
    let mut successors: HashMap<&str, Vec<&'a str>> = HashMap::new();
    for node in &graph.nodes {
        for source in dependencies.get(node.id.as_str()).into_iter().flatten() {
            successors.entry(source).or_default().push(&node.id);
        }
    }
    let mut reachable = HashSet::from([start]);
    let mut active = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(id) = queue.pop_front() {
        for candidate in successors.get(id).into_iter().flatten().copied() {
            reachable.insert(candidate);
            if active.insert(candidate) {
                queue.push_back(candidate);
            }
        }
        // Arguments are required values, not new control roots. Walk their pure
        // prerequisites without visiting their other consumers: a shared variable
        // must not silently enable a disconnected HTTP request.
        let mut arguments = vec![id];
        while let Some(argument) = arguments.pop() {
            for source in dependencies.get(argument).into_iter().flatten().copied() {
                if kinds.get(source).is_some_and(|kind| kind.is_pure_value())
                    && reachable.insert(source)
                {
                    arguments.push(source);
                }
            }
        }
    }
    reachable
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::format::{Binding, Node, NodeConfig, OrderEdge, Position};
    use indexmap::IndexMap;

    fn node(id: &str, kind: NodeKind) -> Node {
        Node {
            id: id.into(),
            kind,
            handle: id.into(),
            label: None,
            note: None,
            position: Position { x: 0.0, y: 0.0 },
            auto_placed: false,
            inputs: IndexMap::new(),
            config: NodeConfig::default(),
        }
    }

    fn with_input(mut node: Node, source: &str) -> Node {
        let name = format!("in{}", node.inputs.len());
        node.inputs
            .insert(name, Binding::reference(source, "value", ""));
        node
    }

    fn edge(source: &str, target: &str) -> OrderEdge {
        OrderEdge {
            id: format!("{source}-{target}"),
            source: source.into(),
            target: target.into(),
            branch: None,
        }
    }

    #[test]
    fn legacy_graphs_without_start_reach_everything() {
        let graph = Graph {
            nodes: vec![node("a", NodeKind::Request), node("b", NodeKind::Request)],
            edges: vec![edge("a", "b")],
            proxies: Vec::new(),
        };
        let plan = ExecutionPlan::new(&graph);
        assert_eq!(plan.reachable, None);
        assert!(plan.is_reachable("anything"));
        assert_eq!(plan.dependencies_of("b").collect::<Vec<_>>(), ["a"]);
        assert_eq!(plan.dependencies_of("missing").count(), 0);
    }

    #[test]
    fn start_reaches_successors_and_pulls_in_pure_arguments_only() {
        let graph = Graph {
            nodes: vec![
                node("start", NodeKind::Start),
                with_input(
                    with_input(
                        with_input(node("call", NodeKind::Request), "value"),
                        "other",
                    ),
                    "component",
                ),
                with_input(node("value", NodeKind::Value), "nested"),
                node("nested", NodeKind::Value),
                node("other", NodeKind::Request),
                node("component", NodeKind::Component),
                node("disconnected", NodeKind::Request),
                with_input(node("consumer", NodeKind::Assert), "call"),
            ],
            edges: vec![edge("start", "call")],
            proxies: Vec::new(),
        };
        let plan = ExecutionPlan::new(&graph);
        let mut reachable: Vec<_> = plan.reachable.clone().unwrap().into_iter().collect();
        reachable.sort_unstable();
        assert_eq!(reachable, ["call", "consumer", "nested", "start", "value"]);
        assert!(!plan.is_reachable("other"));
        assert!(!plan.is_reachable("component"));
        assert!(!plan.is_reachable("disconnected"));
    }

    #[test]
    fn a_shared_pure_argument_does_not_enable_its_other_consumers() {
        let graph = Graph {
            nodes: vec![
                node("start", NodeKind::Start),
                with_input(node("active", NodeKind::Request), "shared"),
                node("shared", NodeKind::Value),
                with_input(node("side-effect", NodeKind::Request), "shared"),
            ],
            edges: vec![edge("start", "active")],
            proxies: Vec::new(),
        };
        let plan = ExecutionPlan::new(&graph);
        assert!(plan.is_reachable("shared"));
        assert!(!plan.is_reachable("side-effect"));
    }

    #[test]
    fn an_explicitly_sequenced_value_still_activates_its_consumers() {
        let graph = Graph {
            nodes: vec![
                node("start", NodeKind::Start),
                node("value", NodeKind::Value),
                with_input(node("consumer", NodeKind::Request), "value"),
            ],
            edges: vec![edge("start", "value")],
            proxies: Vec::new(),
        };
        assert!(ExecutionPlan::new(&graph).is_reachable("consumer"));
    }

    fn branch(source: &str, target: &str, branch: Branch) -> OrderEdge {
        OrderEdge {
            branch: Some(branch),
            ..edge(source, target)
        }
    }

    #[test]
    fn braces_are_read_from_branch_edges_at_any_depth() {
        let graph = Graph {
            nodes: vec![
                node("x", NodeKind::If),
                node("first", NodeKind::Request),
                node("second", NodeKind::Request),
                node("inner", NodeKind::If),
                node("deep", NodeKind::Request),
                node("other", NodeKind::Request),
                node("outside", NodeKind::Request),
                node("after", NodeKind::Request),
            ],
            edges: vec![
                branch("x", "first", Branch::True),
                branch("x", "second", Branch::True),
                edge("first", "second"),
                branch("x", "inner", Branch::True),
                edge("second", "inner"),
                branch("inner", "deep", Branch::False),
                branch("x", "other", Branch::False),
                // Follows `x.then` and a statement outside the braces: not in them.
                branch("x", "outside", Branch::True),
                edge("after", "outside"),
                edge("deep", "after"),
                branch("x", "after", Branch::False),
            ],
            proxies: Vec::new(),
        };
        let plan = ExecutionPlan::new(&graph);
        let enclosing = |id: &str| plan.enclosing(id).collect::<Vec<_>>();
        assert_eq!(enclosing("first"), [("x", Branch::True)]);
        assert_eq!(enclosing("second"), [("x", Branch::True)]);
        assert_eq!(enclosing("inner"), [("x", Branch::True)]);
        assert_eq!(
            enclosing("deep"),
            [("inner", Branch::False), ("x", Branch::True)]
        );
        assert_eq!(enclosing("other"), [("x", Branch::False)]);
        assert_eq!(enclosing("outside"), []);
        assert_eq!(enclosing("after"), []);
        assert_eq!(enclosing("x"), []);
    }

    #[test]
    fn the_first_start_node_is_the_root() {
        let graph = Graph {
            nodes: vec![
                node("first", NodeKind::Start),
                node("second", NodeKind::Start),
                node("after-second", NodeKind::Request),
            ],
            edges: vec![edge("second", "after-second")],
            proxies: Vec::new(),
        };
        let plan = ExecutionPlan::new(&graph);
        assert!(plan.is_reachable("first"));
        assert!(!plan.is_reachable("second"));
        assert!(!plan.is_reachable("after-second"));
    }
}
