//! The two runs of a part of a scenario (ADR 0016).

use apiforge_core::{
    format::Graph,
    graph::{check_subjects, sequence},
    model::NodeKind,
    run::RunResult,
};
use indexmap::IndexSet;
use std::collections::HashMap;

/// A run of a part of the main graph.
#[derive(Clone, Debug, PartialEq)]
pub enum Partial {
    /// The scenario up to a node, with the checks and values computed before
    /// the next step, and then the whole Cleanup.
    Through { node_id: String },
    /// One request with the checks that read it, every other value read from
    /// `previous`, an earlier, already redacted report of the same scenario
    /// and mode. No other step and no Cleanup.
    Alone {
        node_id: String,
        previous: Option<Box<RunResult>>,
    },
}

impl Partial {
    pub fn node_id(&self) -> &str {
        match self {
            Self::Through { node_id } | Self::Alone { node_id, .. } => node_id,
        }
    }

    /// The nodes of the main graph this run executes.
    pub(crate) fn selection(&self, graph: &Graph) -> IndexSet<String> {
        match self {
            Self::Through { node_id } => through(graph, node_id),
            Self::Alone { node_id, .. } => alone(graph, node_id),
        }
    }
}

/// The nodes in the order the engine reaches them, up to `node_id`, and the
/// data statements it reaches before the next step: a prefix of [`sequence`],
/// so it holds everything its nodes depend on and the engine takes them in
/// the same order. Empty when the graph has no such node.
fn through(graph: &Graph, node_id: &str) -> IndexSet<String> {
    let kinds: HashMap<&str, NodeKind> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.kind))
        .collect();
    let order = sequence(graph);
    let Some(at) = order.iter().position(|id| *id == node_id) else {
        return IndexSet::new();
    };
    let end = order[at + 1..]
        .iter()
        .position(|id| !kinds[id].is_data())
        .map_or(order.len(), |next| at + 1 + next);
    order[..end].iter().map(|id| (*id).to_owned()).collect()
}

/// The node and the checks that read it ([`check_subjects`]).
fn alone(graph: &Graph, node_id: &str) -> IndexSet<String> {
    let checks = graph.nodes.iter().filter(|check| {
        check_subjects(graph, check)
            .subjects
            .iter()
            .any(|subject| subject.step == node_id)
    });
    std::iter::once(node_id)
        .chain(checks.map(|check| check.id.as_str()))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn reads(source: &str, port: &str) -> Value {
        json!({"kind": "reference", "nodeId": source, "port": port, "path": ""})
    }

    fn node(id: &str, kind: &str, inputs: Value) -> Value {
        json!({"id": id, "kind": kind, "handle": id, "position": {"x": 0, "y": 0},
            "inputs": inputs, "config": {}})
    }

    /// `a`, its value and check; `b`, checks of both and of the value; an
    /// `if`, and `c` after it. The checks stand first in the document.
    fn graph() -> Graph {
        let one = json!({"kind": "literal", "value": 1});
        let edge = |source: &str, target: &str, branch: Option<&str>| {
            let mut edge = json!({"id": format!("{source}-{target}"), "source": source,
                "target": target});
            if let Some(branch) = branch {
                edge["branch"] = json!(branch);
            }
            edge
        };
        serde_json::from_value(json!({
            "nodes": [
                node("both", "assert",
                    json!({"actual": reads("a", "status"), "expected": reads("b", "status")})),
                node("of-value", "assert",
                    json!({"actual": reads("value", "value"), "expected": one})),
                node("of-a", "assert", json!({"actual": reads("a", "status"), "expected": one})),
                node("start", "start", json!({})),
                node("a", "request", json!({"url": one})),
                node("value", "value", json!({"value": reads("a", "body")})),
                node("b", "request", json!({"url": one})),
                node("if", "if", json!({"actual": reads("b", "status"), "expected": one})),
                node("c", "request", json!({"url": one})),
            ],
            "edges": [edge("start", "a", None), edge("a", "b", None), edge("b", "if", None),
                edge("if", "c", Some("true"))]
        }))
        .expect("a graph")
    }

    #[test]
    fn through_is_a_prefix_up_to_the_next_step() {
        let graph = graph();
        let selected = |id| Vec::from_iter(through(&graph, id));
        assert_eq!(selected("start"), ["start"]);
        let a = ["start", "a", "of-a", "value", "of-value"];
        assert_eq!(selected("a"), a);
        // From a data statement on, the others before the next step run too.
        assert_eq!(selected("of-a"), a);
        assert_eq!(selected("b"), [&a[..], &["b", "both"]].concat());
        assert_eq!(selected("if"), [&a[..], &["b", "both", "if"]].concat());
        assert_eq!(selected("c").len(), graph.nodes.len());
        assert!(selected("nowhere").is_empty());
    }

    #[test]
    fn alone_is_the_request_and_the_checks_that_read_it() {
        let graph = graph();
        let selected = |id| Vec::from_iter(alone(&graph, id));
        // A check of a value made from the response reads the value.
        assert_eq!(selected("a"), ["a", "both", "of-a"]);
        assert_eq!(selected("b"), ["b", "both"]);
        assert_eq!(selected("c"), ["c"]);
    }
}
