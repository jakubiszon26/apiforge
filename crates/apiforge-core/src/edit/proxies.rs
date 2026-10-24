//! Proxies: the places a variable is also shown on the board.
//!
//! A proxy is layout and nothing else. It changes where a consumer's wire starts,
//! never what the consumer is bound to, so these operations touch no binding and
//! two graphs that differ only in their proxies run identically
//! ([ADR 0004](../../../../docs/decisions/0004-document-format-v2.md)).

use super::{EditError, unique_id};
use crate::{
    format::{Binding, Graph, Position, Proxy, ProxySource},
    graph::VALUE_INPUT,
};
use std::collections::HashSet;

/// The proxy a dragged source stands for, or `None` for something no proxy can
/// hold: a field of a response, a request's own argument or a computed value.
pub fn proxy_source(binding: &Binding) -> Option<ProxySource> {
    match binding {
        Binding::Reference { node_id, path, .. } if path.is_empty() => {
            Some(ProxySource::Variable {
                node_id: node_id.clone(),
            })
        }
        Binding::Parameter { name, path } if path.is_empty() => {
            Some(ProxySource::Parameter { name: name.clone() })
        }
        Binding::Environment { name } => Some(ProxySource::Environment { name: name.clone() }),
        _ => None,
    }
}

/// What a consumer bound to this proxy reads. A proxy never introduces a binding
/// of its own: this is the same binding the Variables panel carries.
pub fn proxy_binding(source: &ProxySource) -> Binding {
    match source {
        ProxySource::Variable { node_id } => Binding::reference(node_id, VALUE_INPUT, ""),
        ProxySource::Parameter { name } => Binding::parameter(name),
        ProxySource::Environment { name } => Binding::environment(name),
    }
}

/// Places another copy of a variable, parameter or environment value on the board.
///
/// A variable may be placed as often as it is useful: each consumer takes its wire
/// from the nearest place. Only a Value marked as a variable can be placed, because
/// nothing else is listed with the variables.
pub fn add_proxy(
    graph: &Graph,
    source: ProxySource,
    position: Position,
) -> Result<Graph, EditError> {
    if let ProxySource::Variable { node_id } = &source
        && !graph
            .nodes
            .iter()
            .any(|node| &node.id == node_id && node.is_variable())
    {
        return Err(EditError::NotAVariable);
    }
    let mut next = graph.clone();
    next.proxies.push(Proxy {
        id: unique_id("proxy", |id| {
            graph.proxies.iter().any(|proxy| proxy.id == id)
        }),
        source,
        position,
    });
    Ok(next)
}

/// Puts proxies at new positions; unknown identifiers are ignored, as a move of
/// blocks ignores them.
pub fn move_proxies(graph: &Graph, moved: &[(String, Position)]) -> Graph {
    let mut next = graph.clone();
    for (id, position) in moved {
        if let Some(proxy) = next.proxies.iter_mut().find(|proxy| &proxy.id == id) {
            proxy.position = position.clone();
        }
    }
    next
}

/// Removes proxies by identifier. The variable itself stays: a proxy is only a
/// place it is shown.
pub fn remove_proxies(graph: &Graph, ids: &HashSet<&str>) -> Graph {
    let mut next = graph.clone();
    next.proxies
        .retain(|proxy| !ids.contains(proxy.id.as_str()));
    next
}

/// Drops the proxies of everything that is no longer a variable of this graph.
///
/// Deleting a variable or clearing its variable mark leaves its proxies standing
/// for nothing. This completes such an edit rather than being one itself, so it
/// changes the graph in place: the proxies disappear within the edit that removed
/// their source, and one undo brings the variable and its proxies back together.
pub fn prune_proxies(graph: &mut Graph) {
    let variables: HashSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.is_variable())
        .map(|node| node.id.as_str())
        .collect();
    graph.proxies.retain(|proxy| match &proxy.source {
        ProxySource::Variable { node_id } => variables.contains(node_id.as_str()),
        ProxySource::Environment { .. } | ProxySource::Parameter { .. } => true,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        format::Node,
        graph::{create_node, delete_nodes},
        model::NodeKind,
    };

    fn at(x: f64, y: f64) -> Position {
        Position { x, y }
    }

    fn block(id: &str, kind: NodeKind) -> Node {
        let mut node = create_node(kind, at(0.0, 0.0), None);
        node.id = id.to_owned();
        node
    }

    fn variable(id: &str) -> Node {
        let mut node = block(id, NodeKind::Value);
        node.config.variable = Some(true);
        node
    }

    fn board() -> Graph {
        Graph {
            nodes: vec![variable("token"), block("call", NodeKind::Request)],
            edges: vec![],
            proxies: Vec::new(),
        }
    }

    #[test]
    fn a_variable_can_be_placed_several_times_and_only_a_variable_can_be_placed() {
        let source = ProxySource::Variable {
            node_id: "token".into(),
        };
        let once = add_proxy(&board(), source.clone(), at(10.0, 20.0)).unwrap();
        let twice = add_proxy(&once, source, at(300.0, 20.0)).unwrap();
        assert_eq!(twice.proxies.len(), 2);
        assert_ne!(twice.proxies[0].id, twice.proxies[1].id);
        // Placing one changes no binding at all.
        assert_eq!(twice.nodes, board().nodes);

        for missing in [
            ProxySource::Variable {
                node_id: "call".into(),
            },
            ProxySource::Variable {
                node_id: "absent".into(),
            },
        ] {
            let error = add_proxy(&board(), missing, at(0.0, 0.0)).unwrap_err();
            assert!(matches!(error, EditError::NotAVariable));
        }
    }

    #[test]
    fn a_parameter_or_environment_value_needs_no_block_to_stand_on() {
        let graph = add_proxy(
            &board(),
            ProxySource::Parameter {
                name: "quantity".into(),
            },
            at(0.0, 0.0),
        )
        .and_then(|graph| {
            add_proxy(
                &graph,
                ProxySource::Environment {
                    name: "baseUrl".into(),
                },
                at(0.0, 60.0),
            )
        })
        .unwrap();
        assert_eq!(graph.proxies.len(), 2);
        assert_eq!(
            proxy_binding(&graph.proxies[0].source),
            Binding::parameter("quantity")
        );
        assert_eq!(
            proxy_binding(&graph.proxies[1].source),
            Binding::environment("baseUrl")
        );
    }

    #[test]
    fn moving_and_removing_a_proxy_leaves_the_rest_of_the_board_alone() {
        let graph = add_proxy(
            &board(),
            ProxySource::Variable {
                node_id: "token".into(),
            },
            at(10.0, 10.0),
        )
        .unwrap();
        let id = graph.proxies[0].id.clone();
        let moved = move_proxies(&graph, &[(id.clone(), at(120.0, 240.0))]);
        assert_eq!(moved.proxies[0].position, at(120.0, 240.0));
        let ignored = move_proxies(&graph, &[("other".into(), at(0.0, 0.0))]);
        assert_eq!(ignored, graph);
        let removed = remove_proxies(&moved, &HashSet::from([id.as_str()]));
        assert!(removed.proxies.is_empty());
        assert_eq!(removed.nodes, graph.nodes);
    }

    #[test]
    fn a_proxy_goes_with_the_variable_it_stands_for() {
        let placed = add_proxy(
            &board(),
            ProxySource::Variable {
                node_id: "token".into(),
            },
            at(10.0, 10.0),
        )
        .unwrap();
        let deleted = delete_nodes(&placed, &HashSet::from(["token"]));
        assert!(deleted.proxies.is_empty());

        // Clearing the variable mark says the same thing as deleting the block:
        // the value is no longer listed with the variables.
        let mut plain = placed.clone();
        plain.nodes[0].config.variable = None;
        prune_proxies(&mut plain);
        assert!(plain.proxies.is_empty());
        let mut kept = placed.clone();
        prune_proxies(&mut kept);
        assert_eq!(kept, placed);
    }

    #[test]
    fn only_a_whole_variable_parameter_or_environment_value_can_be_placed() {
        assert_eq!(
            proxy_source(&Binding::reference("token", VALUE_INPUT, "")),
            Some(ProxySource::Variable {
                node_id: "token".into()
            })
        );
        assert_eq!(
            proxy_source(&Binding::parameter("quantity")),
            Some(ProxySource::Parameter {
                name: "quantity".into()
            })
        );
        // A field of a response, or of a parameter, is not a variable.
        assert_eq!(
            proxy_source(&Binding::reference("call", "body", "items[0].id")),
            None
        );
        assert_eq!(
            proxy_source(&Binding::parameter_field("quantity", "unit")),
            None
        );
        assert_eq!(proxy_source(&Binding::literal(serde_json::json!(1))), None);
    }
}
