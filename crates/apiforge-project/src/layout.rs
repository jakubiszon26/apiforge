//! Placement of blocks that source analysis could not match to an existing
//! identity. Card sizes belong to the renderer, so hosts supply their own
//! placer; the fixed placer keeps headless sessions and the CLI deterministic.
use apiforge_core::format::{Graph, Position};
use apiforge_script::Document;
use std::collections::HashSet;

pub trait Placer: Send + Sync {
    /// Positions the nodes named in `fresh`, leaving every other node in place.
    fn place(&self, document: &mut Document, fresh: &[String]);
}

/// Uniform card sizes and the shared incremental layered layout.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedPlacer;

impl Placer for FixedPlacer {
    fn place(&self, document: &mut Document, fresh: &[String]) {
        let fresh: HashSet<_> = fresh.iter().map(String::as_str).collect();
        let graphs = match document {
            Document::Scenario(scenario) => vec![&mut scenario.graph, &mut scenario.cleanup],
            Document::Component(component) => vec![&mut component.graph],
        };
        for graph in graphs {
            place_graph(graph, &fresh);
        }
    }
}

fn place_graph(graph: &mut Graph, fresh: &HashSet<&str>) {
    let indices: Vec<usize> = (0..graph.nodes.len())
        .filter(|index| fresh.contains(graph.nodes[*index].id.as_str()))
        .collect();
    if indices.is_empty() {
        return;
    }
    let size = apiforge_layout::Size::new(240.0, 96.0);
    let blocks: Vec<_> = graph
        .nodes
        .iter()
        .map(|node| {
            let block = apiforge_layout::Block::new(
                size,
                apiforge_layout::Point::new(node.position.x as f32, node.position.y as f32),
            );
            if fresh.contains(node.id.as_str()) {
                block
            } else {
                block.pinned()
            }
        })
        .collect();
    let index_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
    let edges: Vec<_> = apiforge_core::graph::wires(graph)
        .iter()
        .filter_map(|wire| {
            Some(apiforge_layout::Edge::new(
                index_of(&wire.source)?,
                index_of(&wire.target)?,
            ))
        })
        .collect();
    let placed = apiforge_layout::place(&apiforge_layout::Graph::new(&blocks, &edges), &indices);
    for (index, position) in indices.into_iter().zip(placed) {
        let node = &mut graph.nodes[index];
        node.position = Position {
            x: f64::from(position.x),
            y: f64::from(position.y),
        };
        node.auto_placed = true;
    }
}
