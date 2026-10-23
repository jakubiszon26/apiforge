//! Layered layout: which column a card belongs in, in which order, and where.
//!
//! The flow runs left to right, so a layer is a column and the order within a layer
//! is top to bottom. The three phases are the usual ones (Sugiyama): longest-path
//! layering, crossing reduction by barycentre sweeps, and coordinate assignment by
//! Brandes–Köpf. Everything is deterministic: ties break on the index a caller gave
//! a block, never on iteration order of a hash container.
//!
//! Two entry points, as [ADR 0006](../../../docs/decisions/0006-layout-and-wire-routing.md)
//! describes them: [`tidy`] lays out everything and ignores pins, while [`place`]
//! finds a home for new cards and moves nothing else.

use crate::geometry::{Block, Edge, Point, Rect, Size, compare};
use std::collections::{BTreeSet, VecDeque};

/// Spacing of the layered layout, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    /// Free space between the widest card of a layer and the next layer.
    pub layer_gap: f32,
    /// Free space between cards within a layer.
    pub row_gap: f32,
    /// Step of the search for a free slot in incremental placement.
    pub slot_step: f32,
    /// Free space a placed card keeps from any other card.
    pub clearance: f32,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            layer_gap: 96.0,
            row_gap: 48.0,
            slot_step: 24.0,
            clearance: 24.0,
        }
    }
}

/// At most this many barycentre sweeps; the usual graph settles in a few.
const MAX_SWEEPS: usize = 24;

/// Cards and dependencies to lay out. Indices refer to `blocks`.
#[derive(Clone, Debug)]
pub struct Graph<'a> {
    pub blocks: &'a [Block],
    pub edges: &'a [Edge],
    pub spacing: Spacing,
}

impl<'a> Graph<'a> {
    pub fn new(blocks: &'a [Block], edges: &'a [Edge]) -> Self {
        Self {
            blocks,
            edges,
            spacing: Spacing::default(),
        }
    }

    #[must_use]
    pub fn with_spacing(mut self, spacing: Spacing) -> Self {
        self.spacing = spacing;
        self
    }

    /// Edges with both ends inside the graph and no self-loop, which is what every
    /// phase below assumes.
    fn usable_edges(&self) -> Vec<Edge> {
        self.edges
            .iter()
            .copied()
            .filter(|edge| {
                edge.source != edge.target
                    && edge.source < self.blocks.len()
                    && edge.target < self.blocks.len()
            })
            .collect()
    }
}

/// Lays out the cards and returns one position per block.
///
/// A pinned block keeps its coordinates and becomes an obstacle: that is how a
/// *tidy up* of a selection works, with everything outside the selection pinned.
/// The laid-out blocks are then moved to the nearest free slot where they would
/// otherwise land on a pinned one. With nothing pinned the whole board is laid out,
/// which is the ordinary case.
///
/// The result is anchored so that the top-left corner of the blocks being laid out
/// stays where it is now; a tidy-up does not send the board off to the origin.
pub fn tidy(graph: &Graph<'_>) -> Vec<Point> {
    if graph.blocks.is_empty() {
        return Vec::new();
    }
    let edges = graph.usable_edges();
    let layers = layer_assignment(graph.blocks.len(), &edges);
    let order = order_within_layers(graph.blocks.len(), &edges, &layers);
    let mut positions = coordinates(graph, &layers, &order);
    // Pinned blocks take no part in the result: they are where the user put them.
    let moving: Vec<usize> = (0..graph.blocks.len())
        .filter(|index| !graph.blocks[*index].pinned)
        .collect();
    if moving.is_empty() {
        return graph.blocks.iter().map(|block| block.position).collect();
    }
    let anchor = Rect::bounding(moving.iter().map(|index| graph.blocks[*index].rect()))
        .map(|bounds| bounds.min)
        .unwrap_or_default();
    let laid_out = Rect::bounding(
        moving
            .iter()
            .map(|index| Rect::from_size(positions[*index], graph.blocks[*index].size)),
    )
    .map(|bounds| bounds.min)
    .unwrap_or_default();
    let shift = Point::new(anchor.x - laid_out.x, anchor.y - laid_out.y);
    for index in &moving {
        positions[*index].x += shift.x;
        positions[*index].y += shift.y;
    }
    let mut obstacles: Vec<Rect> = graph
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| block.pinned)
        .map(|(index, block)| {
            positions[index] = block.position;
            block.rect().inflate(graph.spacing.clearance)
        })
        .collect();
    if !obstacles.is_empty() {
        // The layout knows nothing about the pinned cards, so a laid-out card can
        // land on one. Each then takes the nearest free slot, in document order so
        // that the result does not depend on the layer walk.
        for index in &moving {
            let block = &graph.blocks[*index];
            positions[*index] = free_slot(
                positions[*index],
                block.size,
                &obstacles,
                graph.spacing.slot_step,
            );
            obstacles.push(
                Rect::from_size(positions[*index], block.size).inflate(graph.spacing.clearance),
            );
        }
    }
    positions
}

/// Finds a position for each card in `fresh`, leaving every other card where it is.
///
/// A new card goes into the layer its dependencies put it in, at the free slot
/// nearest its neighbours: the search starts at the average position of the cards it
/// is connected to and walks outwards in [`Spacing::slot_step`] steps, so a card
/// lands beside what it is wired to and never on top of another card. Cards placed
/// in the same call are obstacles for the ones after them, in the order given.
pub fn place(graph: &Graph<'_>, fresh: &[usize]) -> Vec<Point> {
    let edges = graph.usable_edges();
    let layers = layer_assignment(graph.blocks.len(), &edges);
    // Where each layer sits now, taken from the cards that are already placed.
    let columns = existing_columns(graph, &layers, fresh);
    let mut obstacles: Vec<Rect> = graph
        .blocks
        .iter()
        .enumerate()
        .filter(|(index, _)| !fresh.contains(index))
        .map(|(_, block)| block.rect().inflate(graph.spacing.clearance))
        .collect();
    let mut placed = Vec::with_capacity(fresh.len());
    for &index in fresh {
        let Some(block) = graph.blocks.get(index) else {
            placed.push(Point::default());
            continue;
        };
        let layer = layers.get(index).copied().unwrap_or(0);
        let neighbours: Vec<Point> = edges
            .iter()
            .filter_map(|edge| match () {
                () if edge.target == index => Some(edge.source),
                () if edge.source == index => Some(edge.target),
                () => None,
            })
            .filter(|neighbour| !fresh.contains(neighbour))
            .filter_map(|neighbour| graph.blocks.get(neighbour))
            .map(|neighbour| neighbour.rect().center())
            .collect();
        let start = start_position(block, layer, &layers, &columns, &neighbours, graph);
        let position = free_slot(start, block.size, &obstacles, graph.spacing.slot_step);
        obstacles.push(Rect::from_size(position, block.size).inflate(graph.spacing.clearance));
        placed.push(position);
    }
    placed
}

/// The left edge of every layer that already holds a placed card.
fn existing_columns(graph: &Graph<'_>, layers: &[usize], fresh: &[usize]) -> Vec<Option<f32>> {
    let depth = layers.iter().copied().max().map_or(0, |max| max + 1);
    let mut columns = vec![None; depth];
    for (index, block) in graph.blocks.iter().enumerate() {
        if fresh.contains(&index) {
            continue;
        }
        let layer = layers.get(index).copied().unwrap_or(0);
        let left = columns[layer].unwrap_or(f32::INFINITY);
        columns[layer] = Some(left.min(block.position.x));
    }
    columns
}

/// Where to start looking for a slot: the card's layer decides x, its neighbours y.
fn start_position(
    block: &Block,
    layer: usize,
    layers: &[usize],
    columns: &[Option<f32>],
    neighbours: &[Point],
    graph: &Graph<'_>,
) -> Point {
    let x = columns
        .get(layer)
        .copied()
        .flatten()
        .or_else(|| {
            // An empty layer sits after the last layer that has one, by the widest
            // card of that layer plus the gap.
            columns[..layer]
                .iter()
                .enumerate()
                .filter_map(|(index, left)| left.map(|left| (index, left)))
                .next_back()
                .map(|(index, left)| {
                    left + layer_width(graph, layers, index) + graph.spacing.layer_gap
                })
        })
        .unwrap_or(block.position.x);
    let y = if neighbours.is_empty() {
        block.position.y
    } else {
        let mean =
            neighbours.iter().map(|point| point.y).sum::<f32>() / neighbours.len().max(1) as f32;
        mean - block.size.height * 0.5
    };
    Point::new(x, y)
}

/// The widest card of a layer, or zero for a layer without placed cards.
fn layer_width(graph: &Graph<'_>, layers: &[usize], layer: usize) -> f32 {
    graph
        .blocks
        .iter()
        .enumerate()
        .filter(|(index, _)| layers.get(*index).copied() == Some(layer))
        .map(|(_, block)| block.size.width)
        .fold(0.0, f32::max)
}

/// The first position at or below `start` whose card touches nothing, searched
/// alternately down and up so that a card stays near where it wanted to be.
fn free_slot(start: Point, size: Size, obstacles: &[Rect], step: f32) -> Point {
    let step = if step.is_finite() && step > 0.0 {
        step
    } else {
        24.0
    };
    let fits = |position: Point| {
        let rectangle = Rect::from_size(position, size);
        !obstacles
            .iter()
            .any(|obstacle| obstacle.overlaps(&rectangle))
    };
    if fits(start) {
        return start;
    }
    // 512 steps at 24 px reach past any board a user can read; beyond that the
    // original position is a better answer than an arbitrarily distant one.
    for distance in 1..=512 {
        let offset = step * distance as f32;
        for candidate in [
            Point::new(start.x, start.y + offset),
            Point::new(start.x, start.y - offset),
        ] {
            if fits(candidate) {
                return candidate;
            }
        }
    }
    start
}

/// Longest-path layering over a topological order, with back edges reversed.
///
/// A validated document is acyclic; a cycle in the input is broken defensively so
/// that the layout still terminates and stays deterministic.
fn layer_assignment(count: usize, edges: &[Edge]) -> Vec<usize> {
    let forward = acyclic(count, edges);
    let mut incoming = vec![0usize; count];
    for edge in &forward {
        incoming[edge.target] += 1;
    }
    let mut layers = vec![0usize; count];
    let mut ready: VecDeque<usize> = (0..count).filter(|index| incoming[*index] == 0).collect();
    let mut settled = 0;
    while let Some(index) = ready.pop_front() {
        settled += 1;
        for edge in forward.iter().filter(|edge| edge.source == index) {
            layers[edge.target] = layers[edge.target].max(layers[index] + 1);
            incoming[edge.target] -= 1;
            if incoming[edge.target] == 0 {
                ready.push_back(edge.target);
            }
        }
    }
    debug_assert_eq!(
        settled, count,
        "the edge set must be acyclic by construction"
    );
    layers
}

/// The edge set with every back edge of a depth-first traversal reversed.
fn acyclic(count: usize, edges: &[Edge]) -> Vec<Edge> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Open,
        Done,
    }
    let mut mark = vec![Mark::New; count];
    let mut back = vec![false; edges.len()];
    // An explicit stack keeps deep graphs off the call stack.
    for root in 0..count {
        if mark[root] != Mark::New {
            continue;
        }
        let mut stack = vec![(root, 0usize)];
        mark[root] = Mark::Open;
        while let Some((node, cursor)) = stack.pop() {
            let next = edges
                .iter()
                .enumerate()
                .skip(cursor)
                .find(|(_, edge)| edge.source == node);
            match next {
                Some((position, edge)) => {
                    stack.push((node, position + 1));
                    match mark[edge.target] {
                        Mark::New => {
                            mark[edge.target] = Mark::Open;
                            stack.push((edge.target, 0));
                        }
                        Mark::Open => back[position] = true,
                        Mark::Done => {}
                    }
                }
                None => mark[node] = Mark::Done,
            }
        }
    }
    edges
        .iter()
        .enumerate()
        .map(|(position, edge)| {
            if back[position] {
                Edge {
                    source: edge.target,
                    target: edge.source,
                    source_offset: edge.target_offset,
                    target_offset: edge.source_offset,
                }
            } else {
                *edge
            }
        })
        .collect()
}

/// A node of the ordering phase: a real card, or a dummy standing in for one layer
/// of an edge that spans several.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Slot {
    Block(usize),
    /// The edge this dummy belongs to, for crossing counts and tie-breaking.
    Dummy(usize),
}

/// The order of the slots in each layer, after crossing reduction.
#[derive(Clone, Debug, Default)]
struct Order {
    layers: Vec<Vec<Slot>>,
    /// Chains of slots, one per edge, from its source layer to its target layer.
    chains: Vec<Vec<(usize, Slot)>>,
}

impl Order {
    fn position(&self, layer: usize, slot: Slot) -> Option<usize> {
        self.layers
            .get(layer)?
            .iter()
            .position(|candidate| *candidate == slot)
    }
}

/// Barycentre sweeps with the port offsets as a tie-breaker.
fn order_within_layers(count: usize, edges: &[Edge], layers: &[usize]) -> Order {
    let forward = acyclic(count, edges);
    let depth = layers.iter().copied().max().map_or(0, |max| max + 1);
    let mut order = Order {
        layers: vec![Vec::new(); depth],
        chains: vec![Vec::new(); forward.len()],
    };
    for (index, layer) in layers.iter().enumerate() {
        order.layers[*layer].push(Slot::Block(index));
    }
    // One dummy per intermediate layer keeps a long edge from being counted as a
    // crossing of every layer it passes.
    for (position, edge) in forward.iter().enumerate() {
        let (from, to) = (layers[edge.source], layers[edge.target]);
        let mut chain = vec![(from, Slot::Block(edge.source))];
        for layer in (from + 1)..to {
            order.layers[layer].push(Slot::Dummy(position));
            chain.push((layer, Slot::Dummy(position)));
        }
        if to > from {
            chain.push((to, Slot::Block(edge.target)));
        }
        order.chains[position] = chain;
    }

    let neighbours = |order: &Order, layer: usize, slot: Slot, previous: bool| -> Vec<f32> {
        let mut rows = Vec::new();
        for chain in &order.chains {
            let Some(at) = chain
                .iter()
                .position(|(level, link)| *level == layer && *link == slot)
            else {
                continue;
            };
            let other = if previous {
                at.checked_sub(1).map(|index| chain[index])
            } else {
                chain.get(at + 1).copied()
            };
            if let Some((level, link)) = other
                && let Some(row) = order.position(level, link)
            {
                rows.push(row as f32);
            }
        }
        rows
    };

    let mut best = order.clone();
    let mut fewest = crossings(&order);
    for sweep in 0..MAX_SWEEPS {
        let downwards = sweep % 2 == 0;
        let range: Vec<usize> = if downwards {
            (1..depth).collect()
        } else {
            (0..depth.saturating_sub(1)).rev().collect()
        };
        for layer in range {
            let mut keyed: Vec<(f32, usize, Slot)> = order.layers[layer]
                .iter()
                .enumerate()
                .map(|(row, slot)| {
                    let rows = neighbours(&order, layer, *slot, downwards);
                    let barycentre = if rows.is_empty() {
                        row as f32
                    } else {
                        rows.iter().sum::<f32>() / rows.len() as f32
                    };
                    (barycentre, row, *slot)
                })
                .collect();
            keyed
                .sort_by(|left, right| compare(left.0, right.0).then_with(|| left.1.cmp(&right.1)));
            order.layers[layer] = keyed.into_iter().map(|(_, _, slot)| slot).collect();
        }
        let count = crossings(&order);
        if count < fewest {
            fewest = count;
            best = order.clone();
        }
        if fewest == 0 {
            break;
        }
    }
    best
}

/// Edge crossings between adjacent layers, counted on the chain links so that a long
/// edge crosses once per layer it actually passes through.
fn crossings(order: &Order) -> usize {
    let mut links: Vec<(usize, usize, usize)> = Vec::new();
    for chain in &order.chains {
        for pair in chain.windows(2) {
            let [(from_layer, from_slot), (_, to_slot)] = [pair[0], pair[1]];
            if let (Some(from), Some(to)) = (
                order.position(from_layer, from_slot),
                order.position(from_layer + 1, to_slot),
            ) {
                links.push((from_layer, from, to));
            }
        }
    }
    let mut total = 0;
    for (index, left) in links.iter().enumerate() {
        for right in &links[index + 1..] {
            if left.0 == right.0
                && ((left.1 < right.1 && left.2 > right.2)
                    || (left.1 > right.1 && left.2 < right.2))
            {
                total += 1;
            }
        }
    }
    total
}

/// Coordinates from the layering and the order.
///
/// Layers are columns: each starts after the widest card of the previous one plus
/// the layer gap. Within a layer the cards keep the order found above, separated by
/// the row gap, and each is then pulled towards the average row of its neighbours
/// (the balanced step of Brandes–Köpf) without ever reordering or overlapping.
fn coordinates(graph: &Graph<'_>, layers: &[usize], order: &Order) -> Vec<Point> {
    let depth = order.layers.len();
    let mut positions = vec![Point::default(); graph.blocks.len()];
    let mut widths = vec![0.0f32; depth];
    for (index, block) in graph.blocks.iter().enumerate() {
        let layer = layers[index];
        widths[layer] = widths[layer].max(block.size.width);
    }
    let mut left = 0.0;
    let mut columns = Vec::with_capacity(depth);
    for width in &widths {
        columns.push(left);
        left += width + graph.spacing.layer_gap;
    }

    // Rows first, in order, then one balancing pass per layer from the second
    // column onwards: a card moves to the average row of the cards feeding it, as
    // far as the gap to its neighbours above and below allows.
    let mut rows: Vec<Vec<usize>> = vec![Vec::new(); depth];
    for (layer, slots) in order.layers.iter().enumerate() {
        for slot in slots {
            if let Slot::Block(index) = slot {
                rows[layer].push(*index);
            }
        }
    }
    for (layer, blocks) in rows.iter().enumerate() {
        let mut top = 0.0;
        for index in blocks {
            positions[*index] = Point::new(columns[layer], top);
            top += graph.blocks[*index].size.height + graph.spacing.row_gap;
        }
    }
    for (layer, blocks) in rows.iter().enumerate().skip(1) {
        for (row, &index) in blocks.iter().enumerate() {
            let sources: Vec<f32> = order
                .chains
                .iter()
                .filter_map(|chain| {
                    let (level, slot) = *chain.last()?;
                    (level == layer && slot == Slot::Block(index))
                        .then(|| chain.first().copied())
                        .flatten()
                })
                .filter_map(|(_, slot)| match slot {
                    Slot::Block(source) => {
                        Some(positions[source].y + graph.blocks[source].size.height * 0.5)
                    }
                    Slot::Dummy(_) => None,
                })
                .collect();
            if sources.is_empty() {
                continue;
            }
            let wanted = sources.iter().sum::<f32>() / sources.len() as f32
                - graph.blocks[index].size.height * 0.5;
            let lower = match row.checked_sub(1).and_then(|above| blocks.get(above)) {
                Some(above) => {
                    positions[*above].y + graph.blocks[*above].size.height + graph.spacing.row_gap
                }
                None => f32::NEG_INFINITY,
            };
            let upper = match blocks.get(row + 1) {
                Some(below) => {
                    positions[*below].y - graph.spacing.row_gap - graph.blocks[index].size.height
                }
                None => f32::INFINITY,
            };
            positions[index].y = wanted.clamp(lower.min(upper), upper.max(lower));
        }
    }
    // Centre each card in its column so that layers of mixed widths stay readable.
    for (index, block) in graph.blocks.iter().enumerate() {
        let layer = layers[index];
        positions[index].x = columns[layer] + (widths[layer] - block.size.width) * 0.5;
    }
    positions
}

/// Layers of a graph, for callers that need the columns without the coordinates.
pub fn layers_of(graph: &Graph<'_>) -> Vec<usize> {
    layer_assignment(graph.blocks.len(), &graph.usable_edges())
}

/// The rows of each layer after crossing reduction, as block indices.
pub fn rows_of(graph: &Graph<'_>) -> Vec<Vec<usize>> {
    let edges = graph.usable_edges();
    let layers = layer_assignment(graph.blocks.len(), &edges);
    let order = order_within_layers(graph.blocks.len(), &edges, &layers);
    order
        .layers
        .iter()
        .map(|slots| {
            slots
                .iter()
                .filter_map(|slot| match slot {
                    Slot::Block(index) => Some(*index),
                    Slot::Dummy(_) => None,
                })
                .collect()
        })
        .collect()
}

/// Whether any two of the laid-out cards overlap, for tests and assertions.
pub fn any_overlap(positions: &[Point], blocks: &[Block]) -> bool {
    let rectangles: Vec<Rect> = positions
        .iter()
        .zip(blocks)
        .map(|(position, block)| Rect::from_size(*position, block.size))
        .collect();
    rectangles.iter().enumerate().any(|(index, left)| {
        rectangles[index + 1..]
            .iter()
            .any(|right| left.overlaps(right))
    })
}

/// Distinct x coordinates of the laid-out cards, as a stable set for tests.
pub fn columns_of(positions: &[Point]) -> BTreeSet<i64> {
    positions
        .iter()
        .map(|position| position.x.round() as i64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(count: usize) -> Vec<Block> {
        (0..count)
            .map(|index| {
                Block::new(
                    Size::new(280.0, 120.0),
                    Point::new(0.0, index as f32 * 10.0),
                )
            })
            .collect()
    }

    #[test]
    fn a_chain_becomes_one_card_per_column_in_order() {
        let blocks = blocks(4);
        let edges = [Edge::new(0, 1), Edge::new(1, 2), Edge::new(2, 3)];
        let graph = Graph::new(&blocks, &edges);
        assert_eq!(layers_of(&graph), [0, 1, 2, 3]);
        let positions = tidy(&graph);
        assert!(!any_overlap(&positions, &blocks));
        assert_eq!(columns_of(&positions).len(), 4);
        for pair in positions.windows(2) {
            assert!(pair[1].x > pair[0].x, "the flow runs left to right");
        }
    }

    #[test]
    fn independent_cards_share_a_column_without_overlapping() {
        let blocks = blocks(3);
        let graph = Graph::new(&blocks, &[]);
        let positions = tidy(&graph);
        assert_eq!(columns_of(&positions).len(), 1);
        assert!(!any_overlap(&positions, &blocks));
        let mut ys: Vec<f32> = positions.iter().map(|position| position.y).collect();
        ys.sort_by(|a, b| compare(*a, *b));
        for pair in ys.windows(2) {
            assert!(pair[1] - pair[0] >= 120.0, "cards keep the row gap");
        }
    }

    #[test]
    fn the_layout_is_deterministic() {
        let blocks = blocks(8);
        let edges = [
            Edge::new(0, 2),
            Edge::new(1, 2),
            Edge::new(2, 3),
            Edge::new(2, 4),
            Edge::new(3, 5),
            Edge::new(4, 5),
            Edge::new(5, 6),
            Edge::new(0, 7),
        ];
        let graph = Graph::new(&blocks, &edges);
        let first = tidy(&graph);
        let second = tidy(&graph);
        assert_eq!(first, second);
    }

    #[test]
    fn crossing_reduction_orders_a_crossed_pair() {
        // Two sources feeding two targets in the crossed order; after the sweeps
        // the targets follow their sources, so nothing crosses.
        let blocks = blocks(4);
        let edges = [Edge::new(0, 3), Edge::new(1, 2)];
        let graph = Graph::new(&blocks, &edges);
        let rows = rows_of(&graph);
        assert_eq!(rows.len(), 2);
        let order = &rows[1];
        let first_source_row = rows[0].iter().position(|index| *index == 0).unwrap();
        let first_target_row = order.iter().position(|index| *index == 3).unwrap();
        let second_target_row = order.iter().position(|index| *index == 2).unwrap();
        assert_eq!(
            first_source_row == 0,
            first_target_row < second_target_row,
            "targets follow the order of their sources"
        );
    }

    #[test]
    fn a_cycle_is_broken_instead_of_hanging() {
        let blocks = blocks(3);
        let edges = [Edge::new(0, 1), Edge::new(1, 2), Edge::new(2, 0)];
        let graph = Graph::new(&blocks, &edges);
        let layers = layers_of(&graph);
        assert_eq!(layers.len(), 3);
        let positions = tidy(&graph);
        assert!(!any_overlap(&positions, &blocks));
    }

    #[test]
    fn tidy_keeps_the_board_where_it_was() {
        let mut blocks = blocks(3);
        for (index, block) in blocks.iter_mut().enumerate() {
            block.position = Point::new(1200.0 + index as f32, 800.0);
        }
        let edges = [Edge::new(0, 1), Edge::new(1, 2)];
        let positions = tidy(&Graph::new(&blocks, &edges));
        let bounds = Rect::bounding(
            positions
                .iter()
                .zip(&blocks)
                .map(|(position, block)| Rect::from_size(*position, block.size)),
        )
        .unwrap();
        assert_eq!(bounds.min, Point::new(1200.0, 800.0));
    }

    #[test]
    fn a_new_card_lands_beside_its_neighbour_and_moves_nothing() {
        let mut blocks = blocks(3);
        blocks[0].position = Point::new(0.0, 0.0);
        blocks[0].pinned = true;
        blocks[1].position = Point::new(400.0, 0.0);
        blocks[1].pinned = true;
        // The third card is new: it depends on the second, so it belongs in a new
        // layer, and nothing else may move.
        let edges = [Edge::new(0, 1), Edge::new(1, 2)];
        let graph = Graph::new(&blocks, &edges);
        let placed = place(&graph, &[2]);
        assert_eq!(placed.len(), 1);
        assert!(placed[0].x > 400.0, "the new card goes into its own layer");
        let mut all = vec![blocks[0].position, blocks[1].position, placed[0]];
        assert!(!any_overlap(&all, &blocks));
        all.truncate(2);
        assert_eq!(all, [Point::new(0.0, 0.0), Point::new(400.0, 0.0)]);
    }

    #[test]
    fn a_new_card_in_an_occupied_layer_takes_a_free_slot() {
        let mut blocks = blocks(3);
        blocks[0].position = Point::new(0.0, 0.0);
        blocks[1].position = Point::new(400.0, 0.0);
        blocks[2].position = Point::new(400.0, 0.0);
        // Both targets of the same source share a layer; the new one must not land
        // on the card that is already there.
        let edges = [Edge::new(0, 1), Edge::new(0, 2)];
        let graph = Graph::new(&blocks, &edges);
        let placed = place(&graph, &[2]);
        let occupied = Rect::from_size(blocks[1].position, blocks[1].size);
        assert!(!Rect::from_size(placed[0], blocks[2].size).overlaps(&occupied));
        assert_eq!(placed[0].x, 400.0, "the layer's column is kept");
    }

    #[test]
    fn placement_of_several_new_cards_keeps_them_apart() {
        let mut blocks = blocks(4);
        blocks[0].position = Point::new(0.0, 0.0);
        for block in &mut blocks[1..] {
            block.position = Point::new(400.0, 0.0);
        }
        let edges = [Edge::new(0, 1), Edge::new(0, 2), Edge::new(0, 3)];
        let graph = Graph::new(&blocks, &edges);
        let placed = place(&graph, &[1, 2, 3]);
        let rectangles: Vec<Rect> = placed
            .iter()
            .zip(&blocks[1..])
            .map(|(position, block)| Rect::from_size(*position, block.size))
            .collect();
        for (index, left) in rectangles.iter().enumerate() {
            for right in &rectangles[index + 1..] {
                assert!(!left.overlaps(right), "{left:?} and {right:?} overlap");
            }
        }
    }

    #[test]
    fn an_empty_graph_lays_out_to_nothing() {
        assert!(tidy(&Graph::new(&[], &[])).is_empty());
        assert!(place(&Graph::new(&[], &[]), &[]).is_empty());
    }

    #[test]
    fn edges_outside_the_graph_and_self_loops_are_ignored() {
        let blocks = blocks(2);
        let edges = [Edge::new(0, 0), Edge::new(0, 9), Edge::new(0, 1)];
        let graph = Graph::new(&blocks, &edges);
        assert_eq!(layers_of(&graph), [0, 1]);
    }

    #[test]
    fn a_wide_card_decides_the_gap_to_the_next_layer() {
        let mut blocks = blocks(3);
        blocks[1].size = Size::new(600.0, 120.0);
        let edges = [Edge::new(0, 1), Edge::new(1, 2)];
        let positions = tidy(&Graph::new(&blocks, &edges));
        let spacing = Spacing::default();
        assert!(
            positions[2].x - positions[1].x >= 600.0 + spacing.layer_gap - 1.0,
            "{positions:?}"
        );
    }
}
