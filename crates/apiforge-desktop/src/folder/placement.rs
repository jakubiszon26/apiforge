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

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::format::{Binding, Proxy, ProxySource};
    use eframe::egui;

    const ORDER_FLOW: &str = include_str!("../../../../fixtures/scenarios/lay/order-flow.afs");

    /// The scenario as code lowers it: no positions, every block fresh.
    fn lowered(project: &Project) -> Document {
        let lowered = apiforge_script::lower(
            &apiforge_script::parse(ORDER_FLOW),
            &apiforge_script::LowerContext::folder(project),
        );
        lowered.document.expect("order-flow lowers")
    }

    /// The gap between two rectangles, zero when they touch or overlap.
    fn separation(a: egui::Rect, b: egui::Rect) -> f32 {
        let dx = (b.min.x - a.max.x).max(a.min.x - b.max.x).max(0.0);
        let dy = (b.min.y - a.max.y).max(a.min.y - b.max.y).max(0.0);
        dx.max(dy)
    }

    /// LAY-02 criterion 5: a chip that comes with blocks that are all fresh
    /// is placed beside the first block reading it, clear of every card.
    #[test]
    fn place_puts_a_chip_beside_its_first_reader() {
        let project = apiforge_core::demos::shop_project();
        let mut document = lowered(&project);
        let Document::Scenario(scenario) = &mut document else {
            panic!("order-flow is a scenario");
        };
        let first = scenario
            .graph
            .nodes
            .iter()
            .find(|node| node.handle == "first")
            .expect("the variable first")
            .id
            .clone();
        scenario.graph.proxies.push(Proxy {
            id: "chip".into(),
            source: ProxySource::Variable {
                node_id: first.clone(),
            },
            position: Position { x: 0.0, y: 5000.0 },
        });
        let fresh: Vec<String> = scenario
            .graph
            .nodes
            .iter()
            .chain(&scenario.cleanup.nodes)
            .map(|node| node.id.clone())
            .collect();
        CardPlacer.place(&mut document, &fresh, &project);
        let Document::Scenario(scenario) = &document else {
            unreachable!()
        };
        let graph = &scenario.graph;
        let reads_first = |binding: &Binding| {
            let mut reads = false;
            binding.for_each_leaf(&mut |leaf, _| {
                reads |= matches!(leaf, Binding::Reference { node_id, .. } if *node_id == first);
            });
            reads
        };
        let reader = graph
            .nodes
            .iter()
            .find(|node| node.inputs.values().any(reads_first))
            .expect("first has a reader");
        assert_eq!(reader.handle, "total", "the first reader in document order");
        let chip = canvas::Chip::build(&graph.proxies[0], graph, &project, &[]).rect();
        let card = |node| canvas::Card::build(node, graph, &project).rect();
        let beside = separation(chip, card(reader));
        assert!(
            beside <= 96.0,
            "the chip at {chip:?} is {beside} px from its reader at {:?}",
            card(reader)
        );
        for node in &graph.nodes {
            assert!(
                !chip.intersects(card(node)),
                "the chip overlaps {}",
                node.handle
            );
        }
    }
}
