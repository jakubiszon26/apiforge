//! Identity matching is separate from syntax and semantic validation.
use crate::{Document, Lowered};
use apiforge_core::{format::*, ids::new_id};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug)]
pub struct Reconciled {
    pub document: Document,
    /// Nodes which need automatic layout. Existing positions are left intact.
    pub unpositioned: Vec<String>,
}

/// Refuses partial/error results. Matched nodes keep identity and layout;
/// ambiguous insertions and kind changes receive fresh identities.
pub fn reconcile(current: &Document, lowered: &Lowered) -> Result<Reconciled, &'static str> {
    if lowered.has_errors() {
        return Err("Cannot reconcile a document with errors");
    }
    let mut document = lowered.document.clone().ok_or("No document to reconcile")?;
    if !matches!(
        (current, &document),
        (Document::Scenario(_), Document::Scenario(_))
            | (Document::Component(_), Document::Component(_))
    ) {
        return Err("The source declaration does not match the document target");
    }
    let old = nodes(current);
    let next = nodes(&document);
    let old_handles: HashMap<_, _> = old.iter().map(|n| (n.handle.as_str(), *n)).collect();
    let next_handles: HashMap<_, _> = next.iter().map(|n| (n.handle.as_str(), *n)).collect();
    let removed: Vec<_> = old
        .iter()
        .filter(|n| !next_handles.contains_key(n.handle.as_str()))
        .copied()
        .collect();
    let added: Vec<_> = next
        .iter()
        .filter(|n| !old_handles.contains_key(n.handle.as_str()))
        .copied()
        .collect();
    let rename = match (removed.as_slice(), added.as_slice()) {
        ([a], [b]) if a.kind == b.kind => {
            let old_ids = handles(current);
            let new_ids = handles(&document);
            let remap = |name: &str| {
                if name == a.handle {
                    b.handle.clone()
                } else {
                    name.to_owned()
                }
            };
            if semantic_node(a, &old_ids, &remap, false)
                == semantic_node(b, &new_ids, &str::to_owned, false)
                && uses(current, &a.handle, &remap) == uses(&document, &b.handle, &str::to_owned)
            {
                Some((b.handle.clone(), *a))
            } else {
                None
            }
        }
        _ => None,
    };
    let mut identities = HashMap::new();
    let mut unpositioned = vec![];
    let mut preserved = HashMap::new();
    for node in next {
        let previous = old_handles
            .get(node.handle.as_str())
            .copied()
            .filter(|old| old.kind == node.kind)
            .or_else(|| {
                rename
                    .as_ref()
                    .filter(|(name, _)| name == &node.handle)
                    .map(|(_, node)| *node)
            });
        let id = previous.map_or_else(|| new_id("node"), |n| n.id.clone());
        if let Some(previous) = previous {
            preserved.insert(node.handle.clone(), previous);
        } else {
            unpositioned.push(id.clone());
        }
        identities.insert(node.id.clone(), id);
    }
    for (graph, old) in graphs_mut(&mut document).into_iter().zip(graphs(current)) {
        // Code carries no layout, so the proxies of the document being edited stay
        // where they are, except those whose variable the text removed or unmarked.
        graph.proxies = old.proxies.clone();
        for node in &mut graph.nodes {
            node.id = identities[&node.id].clone();
            if let Some(old) = preserved.get(&node.handle) {
                node.position = old.position.clone();
                node.auto_placed = old.auto_placed;
            }
            for binding in node.inputs.values_mut() {
                *binding = binding
                    .map_leaves(&mut |leaf, _| {
                        Some(match leaf {
                            Binding::Reference {
                                node_id,
                                port,
                                path,
                            } => Binding::reference(
                                identities.get(node_id).unwrap_or(node_id),
                                port,
                                path,
                            ),
                            _ => leaf.clone(),
                        })
                    })
                    .expect("every leaf retained");
            }
        }
        apiforge_core::edit::prune_proxies(graph);
        for edge in &mut graph.edges {
            edge.source = identities[&edge.source].clone();
            edge.target = identities[&edge.target].clone();
            edge.id = graphs(current)
                .into_iter()
                .flat_map(|g| &g.edges)
                .find(|old| {
                    old.source == edge.source
                        && old.target == edge.target
                        && old.branch == edge.branch
                })
                .map_or_else(|| new_id("order"), |e| e.id.clone());
        }
    }
    match (current, &mut document) {
        (Document::Scenario(old), Document::Scenario(new)) => new.id = old.id.clone(),
        (Document::Component(old), Document::Component(new)) => {
            if old.handle == new.handle || component_rename_equal(old, new) {
                new.id = old.id.clone();
                new.revision = old.revision;
            } else {
                new.id = new_id("component");
                new.revision = 1;
            }
        }
        _ => unreachable!(),
    }
    Ok(Reconciled {
        document,
        unpositioned,
    })
}

fn uses(document: &Document, target: &str, rename: &impl Fn(&str) -> String) -> Vec<String> {
    let names = handles(document);
    let mut uses = vec![];
    for graph in graphs(document) {
        for node in &graph.nodes {
            for (input, binding) in &node.inputs {
                for (path, leaf) in binding.leaves() {
                    if let Binding::Reference {
                        node_id,
                        port,
                        path: field,
                    } = leaf
                        && names.get(node_id).is_some_and(|h| *h == target)
                    {
                        uses.push(
                            json!([rename(&node.handle), input, path, port, field]).to_string(),
                        );
                    }
                }
            }
        }
        for edge in &graph.edges {
            let source = names[&edge.source];
            let destination = names[&edge.target];
            if source == target || destination == target {
                uses.push(json!([rename(source), rename(destination), edge.branch]).to_string());
            }
        }
    }
    uses.sort();
    uses
}
fn handles(document: &Document) -> HashMap<String, &str> {
    nodes(document)
        .into_iter()
        .map(|n| (n.id.clone(), n.handle.as_str()))
        .collect()
}
fn nodes(document: &Document) -> Vec<&Node> {
    graphs(document)
        .into_iter()
        .flat_map(|g| &g.nodes)
        .collect()
}
fn graphs(document: &Document) -> Vec<&Graph> {
    match document {
        Document::Scenario(s) => vec![&s.graph, &s.cleanup],
        Document::Component(c) => vec![&c.graph],
    }
}
fn graphs_mut(document: &mut Document) -> Vec<&mut Graph> {
    match document {
        Document::Scenario(s) => vec![&mut s.graph, &mut s.cleanup],
        Document::Component(c) => vec![&mut c.graph],
    }
}
fn semantic_node(
    node: &Node,
    names: &HashMap<String, &str>,
    rename: &impl Fn(&str) -> String,
    include_handle: bool,
) -> Value {
    let mut node = node.clone();
    for binding in node.inputs.values_mut() {
        *binding = binding
            .map_leaves(&mut |leaf, _| {
                Some(match leaf {
                    Binding::Reference {
                        node_id,
                        port,
                        path,
                    } => Binding::reference(
                        rename(names.get(node_id).copied().unwrap_or(node_id)),
                        port,
                        path,
                    ),
                    _ => leaf.clone(),
                })
            })
            .unwrap()
            .normalized();
    }
    let mut value = serde_json::to_value(node).expect("node serializes");
    let map = value.as_object_mut().unwrap();
    for key in ["id", "position", "autoPlaced"] {
        map.remove(key);
    }
    if include_handle {
        map.insert(
            "handle".into(),
            rename(map["handle"].as_str().unwrap()).into(),
        );
    } else {
        map.remove("handle");
    }
    value
}

/// ADR 0005's semantic projection, useful for round trips and change detection.
/// Component identities in instances are replaced by handle + pinned revision.
pub fn semantic(document: &Document, project: &Project) -> Value {
    let names = handles(document);
    let graph = |graph: &Graph| {
        let nodes: BTreeMap<_, _> = graph
            .nodes
            .iter()
            .map(|node| {
                let mut value = semantic_node(node, &names, &str::to_owned, true);
                if let Some(id) = &node.config.component_id
                    && let Some(component) = project.components.iter().find(|c| c.id == *id)
                {
                    value["config"]["componentId"] = component.handle.clone().into();
                }
                (node.handle.clone(), value)
            })
            .collect();
        let mut edges: Vec<_> = graph
            .edges
            .iter()
            .map(|e| json!([names[&e.source], names[&e.target], e.branch]).to_string())
            .collect();
        edges.sort();
        edges.dedup();
        json!({ "nodes": nodes, "edges": edges })
    };
    match document {
        Document::Scenario(s) => {
            json!({ "name": s.name, "description": s.description, "parameters": s.parameters, "errorPolicy": s.error_policy, "graph": graph(&s.graph), "cleanup": graph(&s.cleanup) })
        }
        Document::Component(c) => {
            json!({ "handle": c.handle, "name": c.name, "description": c.description, "inputs": c.inputs, "action": c.action, "graph": graph(&c.graph) })
        }
    }
}

fn component_rename_equal(old: &Component, new: &Component) -> bool {
    if old.name != new.name
        || old.description != new.description
        || old.inputs != new.inputs
        || old.action != new.action
    {
        return false;
    }
    let shape = |component: &Component| {
        let document = Document::Component(component.clone());
        let names = handles(&document);
        let nodes: BTreeMap<_, _> = component
            .graph
            .nodes
            .iter()
            .map(|n| {
                (
                    n.handle.clone(),
                    semantic_node(n, &names, &str::to_owned, true),
                )
            })
            .collect();
        let mut edges: Vec<_> = component
            .graph
            .edges
            .iter()
            .map(|e| json!([names[&e.source], names[&e.target], e.branch]).to_string())
            .collect();
        edges.sort();
        edges.dedup();
        json!([nodes, edges])
    };
    shape(old) == shape(new)
}
