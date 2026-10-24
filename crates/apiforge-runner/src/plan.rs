//! Dependency indexes and Start reachability of one graph, built once per execution.

use apiforge_core::{format::Graph, graph::dependency_index, model::NodeKind};
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
