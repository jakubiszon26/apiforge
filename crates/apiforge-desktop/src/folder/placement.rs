//! Placement of fresh blocks at the sizes this window actually draws.
//!
//! Card sizes belong to the renderer (ADR 0012), so the folder session takes a
//! placer from its host. The library's fixed placer assumes one uniform card
//! and is there for headless sessions and the CLI; the board draws taller,
//! content-dependent cards, so placing with the uniform size overlaps them.

use crate::canvas;
use apiforge_core::format::{Graph, Position, Project};
use apiforge_project::{Document, Placer};
use std::collections::HashSet;

/// Measures each card and chip as the board paints it, then runs the shared
/// incremental layout. Pure, so analysis can place on a worker thread.
pub struct CardPlacer;

impl Placer for CardPlacer {
    fn place(&self, document: &mut Document, fresh: &[String], context: &Project) {
        let fresh: HashSet<&str> = fresh.iter().map(String::as_str).collect();
        let parameters = super::parameters_of(document);
        let graphs = match document {
            Document::Scenario(scenario) => vec![&mut scenario.graph, &mut scenario.cleanup],
            Document::Component(component) => vec![&mut component.graph],
        };
        for graph in graphs {
            place_graph(graph, &fresh, context, &parameters);
        }
    }
}

fn place_graph(graph: &mut Graph, fresh: &HashSet<&str>, context: &Project, parameters: &[String]) {
    let indices: Vec<usize> = (0..graph.nodes.len())
        .filter(|index| fresh.contains(graph.nodes[*index].id.as_str()))
        .collect();
    if indices.is_empty() {
        return;
    }
    // Rows read the other blocks of the same graph, as the board does.
    let reference = graph.clone();
    let blocks: Vec<apiforge_layout::Block> = graph
        .nodes
        .iter()
        .map(|node| {
            let rect = canvas::Card::build(node, &reference, context).rect();
            let block = apiforge_layout::Block::new(
                apiforge_layout::Size::new(rect.width(), rect.height()),
                apiforge_layout::Point::new(node.position.x as f32, node.position.y as f32),
            );
            if fresh.contains(node.id.as_str()) {
                block
            } else {
                block.pinned()
            }
        })
        // Proxy chips the user put on the board are obstacles, not cards.
        .chain(reference.proxies.iter().map(|proxy| {
            let rect = canvas::Chip::build(proxy, &reference, context, parameters).rect();
            apiforge_layout::Block::new(
                apiforge_layout::Size::new(rect.width(), rect.height()),
                apiforge_layout::Point::new(rect.min.x, rect.min.y),
            )
            .pinned()
        }))
        .collect();
    let index_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
    let edges: Vec<apiforge_layout::Edge> = apiforge_core::graph::wires(graph)
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
