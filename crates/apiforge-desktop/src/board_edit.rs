//! Board commands as graph edits.
//!
//! Each function takes the graph being edited with what surrounds it and returns
//! the next graph, plus the selection that should follow, or a message for the
//! board. Nothing here owns a document, so the workspace window and folder
//! projects apply the same edits through their own histories.

use crate::canvas;
use apiforge_core::{
    block::Block,
    edit,
    format::{Binding, Graph, Node, Position, Project},
    graph::delete_nodes,
};
use std::collections::HashSet;

/// The graph on the board and everything its cards are drawn from.
pub struct Board<'a> {
    pub graph: &'a Graph,
    /// Nodes that bindings may reference (main graph nodes on the cleanup board).
    pub reference: &'a Graph,
    pub project: &'a Project,
    pub parameters: &'a [String],
}

/// A graph edit and the selection that should follow it.
pub struct Edit {
    pub graph: Graph,
    pub selection: Option<Vec<String>>,
}

impl From<Graph> for Edit {
    fn from(graph: Graph) -> Self {
        Self {
            graph,
            selection: None,
        }
    }
}

impl Board<'_> {
    /// Final positions after a drag. A block placed by hand is pinned: automatic
    /// layout treats it as an obstacle until the next tidy up. A proxy is always
    /// where it was put.
    pub fn moved(&self, positions: Vec<(String, Position)>) -> Graph {
        let mut graph = self.graph.clone();
        for (id, position) in positions {
            if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == id) {
                node.position = position;
                node.auto_placed = false;
            } else if let Some(proxy) = graph.proxies.iter_mut().find(|proxy| proxy.id == id) {
                proxy.position = position;
            }
        }
        graph
    }

    /// Lays out the selection, or the whole board when nothing is selected.
    ///
    /// It clears the pins of everything it moves: after a tidy up the layout owns
    /// those positions again. Blocks outside the selection are pinned for the
    /// layout, so the tidied blocks keep clear of them. `None` when fewer than
    /// two blocks would move.
    pub fn tidy_up(&self, selection: &[String]) -> Option<Graph> {
        let moving = |node: &Node| selection.is_empty() || selection.contains(&node.id);
        if self.graph.nodes.iter().filter(|node| moving(node)).count() < 2 {
            return None;
        }
        let blocks: Vec<apiforge_layout::Block> = self
            .graph
            .nodes
            .iter()
            .map(|node| {
                let block = self.card_block(node);
                if moving(node) { block } else { block.pinned() }
            })
            // The chips come after the blocks, so a block keeps its index and the
            // laid-out positions still line up with the nodes.
            .chain(self.chip_obstacles(self.graph))
            .collect();
        let placed = apiforge_layout::tidy(&apiforge_layout::Graph::new(
            &blocks,
            &layout_edges(self.graph),
        ));
        let mut tidied = self.graph.clone();
        for (node, position) in tidied.nodes.iter_mut().zip(placed) {
            if !moving(node) {
                continue;
            }
            node.position = Position {
                x: f64::from(position.x),
                y: f64::from(position.y),
            };
            node.auto_placed = true;
        }
        Some(tidied)
    }

    pub fn add_node(
        &self,
        block: Block,
        component: Option<(String, u32)>,
        position: Position,
        source: Option<(String, String)>,
    ) -> Result<Edit, String> {
        let definition = component.as_ref().and_then(|(id, revision)| {
            self.project
                .components
                .iter()
                .find(|candidate| &candidate.id == id && candidate.revision == *revision)
        });
        let source = source
            .as_ref()
            .map(|(node_id, port)| edit::OutputRef { node_id, port });
        let added = edit::add_node(self.graph, block, position, definition, source)
            .map_err(|error| error.to_string())?;
        Ok(Edit {
            graph: self.free_slot_for(added.graph, &added.node_id),
            selection: Some(vec![added.node_id]),
        })
    }

    /// Adds a node and selects it, keeping it clear of occupied positions.
    pub fn push_node(&self, node: Node) -> Edit {
        let id = node.id.clone();
        let mut graph = self.graph.clone();
        graph.nodes.push(node);
        Edit {
            graph: self.free_slot_for(graph, &id),
            selection: Some(vec![id]),
        }
    }

    /// Sets a slot of a node: a whole input, or the binding at `path` inside it.
    /// `Ok(None)` when the node does not exist.
    pub fn set_input(
        &self,
        node: &str,
        input: &str,
        path: &[String],
        binding: Binding,
    ) -> Result<Option<Graph>, String> {
        let mut graph = self.graph.clone();
        let Some(target) = graph
            .nodes
            .iter_mut()
            .find(|candidate| candidate.id == node)
        else {
            return Ok(None);
        };
        let next = match target.inputs.get(input) {
            Some(current) => current.replace_at(path, binding),
            None => path.is_empty().then_some(binding),
        };
        let next = next.ok_or_else(|| edit::EditError::InvalidOperandPath.to_string())?;
        target.inputs.insert(input.to_owned(), next);
        Ok(Some(graph))
    }

    /// Deletes blocks, proxies and wires. One selection can hold blocks and
    /// proxies; deleting a variable takes its proxies with it.
    pub fn delete(&self, ids: &[String], wires: &[String]) -> Graph {
        let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let wires: HashSet<&str> = wires.iter().map(String::as_str).collect();
        edit::remove_proxies(
            &edit::remove_connections(&delete_nodes(self.graph, &ids), &wires),
            &ids,
        )
    }

    /// Places a proxy and selects it, which shows at once what it stands for and
    /// what now reads it from here.
    pub fn add_proxy(
        &self,
        source: apiforge_core::format::ProxySource,
        position: Position,
    ) -> Result<Edit, String> {
        let graph =
            edit::add_proxy(self.graph, source, position).map_err(|error| error.to_string())?;
        let placed = graph.proxies.last().map(|proxy| proxy.id.clone());
        Ok(Edit {
            graph,
            selection: Some(placed.into_iter().collect()),
        })
    }

    pub fn connect(
        &self,
        source: &str,
        source_port: &str,
        target: &str,
        target_input: &str,
        target_path: &[String],
    ) -> Result<Graph, String> {
        edit::connect(
            self.graph,
            self.project,
            source,
            source_port,
            target,
            target_input,
            target_path,
        )
        .map_err(|error| error.to_string())
    }

    /// Copies the selection and pastes it at an offset, keeping references to
    /// nodes outside this graph (main-graph outputs read by cleanup) exact.
    pub fn paste(&self, fragment: &Graph, offset: &Position) -> Result<Edit, String> {
        let external: Vec<&str> = self
            .reference
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect();
        let pasted = edit::paste_selection(self.graph, fragment, offset, &external)
            .map_err(|error| error.to_string())?;
        Ok(Edit {
            graph: pasted.graph,
            selection: Some(pasted.ids),
        })
    }

    /// Moves a freshly added block to the nearest free slot when the position it
    /// was given is already occupied, and leaves everything else where it is.
    ///
    /// A block that keeps its requested position stays pinned, because that
    /// position is the one the user pointed at; a block the layout had to move is
    /// marked as automatically placed, so a later tidy up owns it.
    pub fn free_slot_for(&self, mut graph: Graph, node_id: &str) -> Graph {
        let Some(fresh) = graph.nodes.iter().position(|node| node.id == node_id) else {
            return graph;
        };
        let blocks: Vec<apiforge_layout::Block> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let block = self.card_block(node);
                if index == fresh {
                    block
                } else {
                    block.pinned()
                }
            })
            .chain(self.chip_obstacles(&graph))
            .collect();
        let placed = apiforge_layout::place(
            &apiforge_layout::Graph::new(&blocks, &layout_edges(&graph)),
            &[fresh],
        );
        if let Some(position) = placed.first() {
            let node = &mut graph.nodes[fresh];
            let moved = (position.x - node.position.x as f32).abs() > 0.5
                || (position.y - node.position.y as f32).abs() > 0.5;
            if moved {
                node.position = Position {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                };
                node.auto_placed = true;
            }
        }
        graph
    }

    fn card_block(&self, node: &Node) -> apiforge_layout::Block {
        let rect = canvas::Card::build(node, self.reference, self.project).rect();
        apiforge_layout::Block::new(
            apiforge_layout::Size::new(rect.width(), rect.height()),
            apiforge_layout::Point::new(node.position.x as f32, node.position.y as f32),
        )
    }

    /// The proxies of a graph as pinned obstacles, so that automatic placement
    /// keeps clear of the chips the user put on the board.
    fn chip_obstacles(&self, graph: &Graph) -> Vec<apiforge_layout::Block> {
        graph
            .proxies
            .iter()
            .map(|proxy| {
                let rect = canvas::Chip::build(proxy, graph, self.project, self.parameters).rect();
                apiforge_layout::Block::new(
                    apiforge_layout::Size::new(rect.width(), rect.height()),
                    apiforge_layout::Point::new(rect.min.x, rect.min.y),
                )
                .pinned()
            })
            .collect()
    }
}

fn layout_edges(graph: &Graph) -> Vec<apiforge_layout::Edge> {
    let index_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
    apiforge_core::graph::wires(graph)
        .iter()
        .filter_map(|wire| {
            Some(apiforge_layout::Edge::new(
                index_of(&wire.source)?,
                index_of(&wire.target)?,
            ))
        })
        .collect()
}
