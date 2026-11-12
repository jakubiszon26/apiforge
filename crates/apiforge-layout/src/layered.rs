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
    // Pinned blocks take no part in the result: they are where the user put them.
    let moving: Vec<usize> = (0..graph.blocks.len())
        .filter(|index| !graph.blocks[*index].pinned)
        .collect();
    if moving.is_empty() {
        return graph.blocks.iter().map(|block| block.position).collect();
    }
    let mut positions = layered_positions(graph, &edges, &layers, &moving);
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
/// in the same call are obstacles for the ones after them, in the order given, and
/// count as neighbours of the ones after them; a layer that no placed card has yet
/// takes its column from the first fresh card placed in it.
///
/// When nothing is placed yet, as for a scenario opened from code, there is no
/// board to fit into: the fresh cards are laid out as [`tidy`] lays them out.
pub fn place(graph: &Graph<'_>, fresh: &[usize]) -> Vec<Point> {
    let edges = graph.usable_edges();
    let layers = layer_assignment(graph.blocks.len(), &edges);
    // Where each layer sits now, taken from the cards that are already placed.
    let mut columns = existing_columns(graph, &layers, fresh);
    let layered = columns.iter().all(Option::is_none).then(|| {
        let moving: Vec<usize> = fresh
            .iter()
            .copied()
            .filter(|index| *index < graph.blocks.len())
            .collect();
        layered_positions(graph, &edges, &layers, &moving)
    });
    let mut obstacles: Vec<Rect> = graph
        .blocks
        .iter()
        .enumerate()
        .filter(|(index, _)| !fresh.contains(index))
        .map(|(_, block)| block.rect().inflate(graph.spacing.clearance))
        .collect();
    // The fresh cards already placed by this call, by block index.
    let mut settled: Vec<Option<Point>> = vec![None; graph.blocks.len()];
    let mut placed = Vec::with_capacity(fresh.len());
    for &index in fresh {
        let Some(block) = graph.blocks.get(index) else {
            placed.push(Point::default());
            continue;
        };
        let layer = layers.get(index).copied().unwrap_or(0);
        let start = match &layered {
            Some(positions) => positions[index],
            None => {
                let neighbours: Vec<Point> = edges
                    .iter()
                    .filter_map(|edge| match () {
                        () if edge.target == index => Some(edge.source),
                        () if edge.source == index => Some(edge.target),
                        () => None,
                    })
                    .filter_map(|neighbour| {
                        let other = graph.blocks.get(neighbour)?;
                        if fresh.contains(&neighbour) {
                            settled[neighbour]
                                .map(|position| Rect::from_size(position, other.size).center())
                        } else {
                            Some(other.rect().center())
                        }
                    })
                    .collect();
                start_position(block, layer, &layers, &columns, &neighbours, graph)
            }
        };
        let position = free_slot(start, block.size, &obstacles, graph.spacing.slot_step);
        obstacles.push(Rect::from_size(position, block.size).inflate(graph.spacing.clearance));
        settled[index] = Some(position);
        if layered.is_none()
            && let Some(column) = columns.get_mut(layer)
            && column.is_none()
        {
            *column = Some(position.x);
        }
        placed.push(position);
    }
    placed
}

/// The layered layout of the whole graph, shifted so that the top-left corner of
/// the `moving` blocks stays where it is now.
fn layered_positions(
    graph: &Graph<'_>,
    edges: &[Edge],
    layers: &[usize],
    moving: &[usize],
) -> Vec<Point> {
    let order = order_within_layers(graph.blocks.len(), edges, layers);
    let mut positions = coordinates(graph, layers, &order);
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
    for index in moving {
        positions[*index].x += shift.x;
        positions[*index].y += shift.y;
    }
    positions
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
                // A reversed edge runs against its ports, so it has none.
                Edge {
                    source: edge.target,
                    target: edge.source,
                    source_offset: edge.target_offset,
                    target_offset: edge.source_offset,
                    ports: false,
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
    /// The edge of each chain, back edges reversed, for its ports.
    edges: Vec<Edge>,
}

impl Order {
    fn position(&self, layer: usize, slot: Slot) -> Option<usize> {
        self.layers
            .get(layer)?
            .iter()
            .position(|candidate| *candidate == slot)
    }

    /// The port of chain `chain` at its link `at`: the source port at the
    /// first link and the target port at the last, when the edge has ports;
    /// a dummy in between has none.
    fn port(&self, chain: usize, at: usize) -> Option<f32> {
        let edge = self.edges.get(chain)?;
        let length = self.chains.get(chain)?.len();
        match () {
            () if !edge.ports => None,
            () if at == 0 => Some(edge.source_offset),
            () if at + 1 == length => Some(edge.target_offset),
            () => None,
        }
    }
}

/// Barycentre sweeps with the port offsets as a tie-breaker.
fn order_within_layers(count: usize, edges: &[Edge], layers: &[usize]) -> Order {
    let forward = acyclic(count, edges);
    let depth = layers.iter().copied().max().map_or(0, |max| max + 1);
    let mut order = Order {
        layers: vec![Vec::new(); depth],
        chains: vec![Vec::new(); forward.len()],
        edges: forward.clone(),
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

    // The rows of a slot's neighbours in the adjacent layer, each with the
    // port the wire leaves or enters that neighbour by, when it has one.
    let neighbours =
        |order: &Order, layer: usize, slot: Slot, previous: bool| -> Vec<(f32, Option<f32>)> {
            let mut rows = Vec::new();
            for (number, chain) in order.chains.iter().enumerate() {
                let Some(at) = chain
                    .iter()
                    .position(|(level, link)| *level == layer && *link == slot)
                else {
                    continue;
                };
                let other = if previous {
                    at.checked_sub(1)
                } else {
                    (at + 1 < chain.len()).then_some(at + 1)
                };
                if let Some(other) = other
                    && let Some(row) = order.position(chain[other].0, chain[other].1)
                {
                    rows.push((row as f32, order.port(number, other)));
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
            let mut keyed: Vec<(f32, Option<f32>, usize, Slot)> = order.layers[layer]
                .iter()
                .enumerate()
                .map(|(row, slot)| {
                    let rows = neighbours(&order, layer, *slot, downwards);
                    let barycentre = if rows.is_empty() {
                        row as f32
                    } else {
                        rows.iter().map(|(row, _)| row).sum::<f32>() / rows.len() as f32
                    };
                    // Two slots fed by the same neighbours tie; the ports they
                    // are wired to there break the tie (LAY-04), so that two
                    // wires leaving one card do not cross right after it.
                    let ports: Vec<f32> = rows.iter().filter_map(|(_, port)| *port).collect();
                    let port =
                        (!ports.is_empty()).then(|| ports.iter().sum::<f32>() / ports.len() as f32);
                    (barycentre, port, row, *slot)
                })
                .collect();
            keyed.sort_by(|left, right| {
                compare(left.0, right.0)
                    .then_with(|| by_port(left.1, right.1))
                    .then_with(|| left.2.cmp(&right.2))
            });
            order.layers[layer] = keyed.into_iter().map(|(_, _, _, slot)| slot).collect();
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

/// Two ports on one card in their order down its side; a missing one ties.
fn by_port(left: Option<f32>, right: Option<f32>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(a), Some(b)) => compare(a, b),
        _ => std::cmp::Ordering::Equal,
    }
}

/// Edge crossings between adjacent layers, counted on the chain links so that a long
/// edge crosses once per layer it actually passes through.
///
/// Two links that share an end are ordered there by their ports (LAY-04): two
/// wires leaving one card cross when the upper port feeds the lower target.
fn crossings(order: &Order) -> usize {
    struct Link {
        layer: usize,
        from: usize,
        to: usize,
        from_port: Option<f32>,
        to_port: Option<f32>,
    }
    let mut links: Vec<Link> = Vec::new();
    for (number, chain) in order.chains.iter().enumerate() {
        for (at, pair) in chain.windows(2).enumerate() {
            let [(from_layer, from_slot), (_, to_slot)] = [pair[0], pair[1]];
            if let (Some(from), Some(to)) = (
                order.position(from_layer, from_slot),
                order.position(from_layer + 1, to_slot),
            ) {
                links.push(Link {
                    layer: from_layer,
                    from,
                    to,
                    from_port: order.port(number, at),
                    to_port: order.port(number, at + 1),
                });
            }
        }
    }
    let mut total = 0;
    for (index, left) in links.iter().enumerate() {
        for right in &links[index + 1..] {
            if left.layer != right.layer {
                continue;
            }
            let start = left
                .from
                .cmp(&right.from)
                .then_with(|| by_port(left.from_port, right.from_port));
            let end = left
                .to
                .cmp(&right.to)
                .then_with(|| by_port(left.to_port, right.to_port));
            if start.is_ne() && end.is_ne() && start != end {
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
/// at least the row gap. The ports decide the rows (LAY-04): a card sits where a
/// wire from the previous column meets its input port level with the output port
/// feeding it, so that the wire runs straight.
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

    // Layer by layer from the left: the first column stacks, and each later
    // one is placed by `place_layer`, from the cards feeding it.
    for (layer, slots) in order.layers.iter().enumerate() {
        let blocks: Vec<usize> = slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Block(index) => Some(*index),
                Slot::Dummy(_) => None,
            })
            .collect();
        let tops = if layer == 0 {
            stacked(graph, &blocks)
        } else {
            place_layer(graph, order, &positions, layer, &blocks)
        };
        for (index, top) in blocks.iter().zip(tops) {
            positions[*index] = Point::new(columns[layer], top);
        }
    }
    straighten_backwards(graph, order, &mut positions);
    // Centre each card in its column so that layers of mixed widths stay readable.
    for (index, block) in graph.blocks.iter().enumerate() {
        let layer = layers[index];
        positions[index].x = columns[layer] + (widths[layer] - block.size.width) * 0.5;
    }
    positions
}

/// The wires between neighbouring columns that carry ports, as (source,
/// target, source port, target port).
fn short_wires(order: &Order) -> Vec<(usize, usize, f32, f32)> {
    order
        .chains
        .iter()
        .zip(&order.edges)
        .filter(|(chain, edge)| chain.len() == 2 && edge.ports)
        .map(|(_, edge)| {
            (
                edge.source,
                edge.target,
                edge.source_offset,
                edge.target_offset,
            )
        })
        .collect()
}

/// The second pass of LAY-04, right to left: a card none of whose wires to a
/// neighbouring column runs straight moves, within the room its column
/// leaves it, to where a wire to the next column does, if that wire's target
/// has no straight wire in yet. The first column, which nothing feeds, gains
/// the most.
fn straighten_backwards(graph: &Graph<'_>, order: &Order, positions: &mut [Point]) {
    let wires = short_wires(order);
    let level = |positions: &[Point], wire: &(usize, usize, f32, f32)| {
        ((positions[wire.0].y + wire.2) - (positions[wire.1].y + wire.3)).abs() < 0.5
    };
    let gap = graph.spacing.row_gap;
    for slots in order.layers.iter().rev().skip(1) {
        let blocks: Vec<usize> = slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Block(index) => Some(*index),
                Slot::Dummy(_) => None,
            })
            .collect();
        for (row, &index) in blocks.iter().enumerate() {
            if wires
                .iter()
                .any(|wire| (wire.0 == index || wire.1 == index) && level(positions, wire))
            {
                continue;
            }
            let lower = row.checked_sub(1).map_or(f32::NEG_INFINITY, |above| {
                let above = blocks[above];
                positions[above].y + graph.blocks[above].size.height + gap
            });
            let upper = blocks.get(row + 1).map_or(f32::INFINITY, |below| {
                positions[*below].y - gap - graph.blocks[index].size.height
            });
            let wanted = wires
                .iter()
                .filter(|wire| {
                    wire.0 == index
                        && !wires
                            .iter()
                            .any(|other| other.1 == wire.1 && level(positions, other))
                })
                .map(|wire| positions[wire.1].y + wire.3 - wire.2)
                .filter(|top| *top >= lower && *top <= upper)
                .min_by(|left, right| {
                    compare(
                        (left - positions[index].y).abs(),
                        (right - positions[index].y).abs(),
                    )
                });
            if let Some(top) = wanted {
                positions[index].y = top;
            }
        }
    }
}

/// The tops of `blocks` stacked from zero, one row gap apart.
fn stacked(graph: &Graph<'_>, blocks: &[usize]) -> Vec<f32> {
    let mut top = 0.0;
    blocks
        .iter()
        .map(|index| {
            let at = top;
            top += graph.blocks[*index].size.height + graph.spacing.row_gap;
            at
        })
        .collect()
}

/// What a card of a later column asks for (LAY-04): the rows that make a wire
/// from the previous column with ports straight, and the row level with the
/// centre of its feeders.
#[derive(Clone, Debug, Default)]
struct Wanted {
    /// The tops at which a wire from the previous column runs straight.
    straight: Vec<f32>,
    /// The top that puts the card's centre at the mean of its feeders'
    /// centres, `None` for a card nothing feeds.
    centre: Option<f32>,
}

impl Wanted {
    fn of(
        graph: &Graph<'_>,
        order: &Order,
        positions: &[Point],
        layer: usize,
        index: usize,
    ) -> Self {
        let height = graph.blocks[index].size.height;
        let mut straight = Vec::new();
        let mut centres = Vec::new();
        for (chain, edge) in order.chains.iter().zip(&order.edges) {
            let (Some(&(level, last)), Some(&(_, Slot::Block(source)))) =
                (chain.last(), chain.first())
            else {
                continue;
            };
            if level != layer || last != Slot::Block(index) || source == index {
                continue;
            }
            let top = positions[source].y;
            centres.push(top + graph.blocks[source].size.height * 0.5 - height * 0.5);
            if edge.ports && chain.len() == 2 {
                straight.push(top + edge.source_offset - edge.target_offset);
            }
        }
        let centre =
            (!centres.is_empty()).then(|| centres.iter().sum::<f32>() / centres.len() as f32);
        Self { straight, centre }
    }

    /// How many wires the best row makes straight.
    fn strength(&self) -> usize {
        self.straight
            .iter()
            .map(|value| agreeing(&self.straight, *value))
            .max()
            .unwrap_or(0)
    }

    /// The top to take between `lower` and `upper`: of the straight rows
    /// that fit there the one the most of them agree with, then the one
    /// nearest the centre row, then the higher; without one that fits, the
    /// centre row, held between the bounds.
    fn top(&self, lower: f32, upper: f32) -> Option<f32> {
        let fits = |value: f32| value >= lower - 0.5 && value <= upper + 0.5;
        let mean = self.centre.unwrap_or(0.0);
        let best = self
            .straight
            .iter()
            .copied()
            .filter(|value| fits(*value))
            .min_by(|left, right| {
                agreeing(&self.straight, *right)
                    .cmp(&agreeing(&self.straight, *left))
                    .then_with(|| compare((left - mean).abs(), (right - mean).abs()))
                    .then_with(|| compare(*left, *right))
            });
        best.or(self.centre)
            .map(|value| value.clamp(lower, upper.max(lower)))
    }
}

/// How many of `values` are within half a pixel of `value`.
fn agreeing(values: &[f32], value: f32) -> usize {
    values
        .iter()
        .filter(|other| (**other - value).abs() < 0.5)
        .count()
}

/// The tops of the cards of a later column, in the order found above (LAY-04).
///
/// The ports decide the rows: a card whose wire from the previous column can
/// run straight is placed first, the card that straightens the most wires
/// before the others, each where its wire is straight as far as the cards
/// placed before it leave room, keeping the order and the row gap with
/// space for the cards still to come between them. The others follow, at
/// the row their feeders ask for within the room left.
fn place_layer(
    graph: &Graph<'_>,
    order: &Order,
    positions: &[Point],
    layer: usize,
    blocks: &[usize],
) -> Vec<f32> {
    let gap = graph.spacing.row_gap;
    let height = |row: usize| graph.blocks[blocks[row]].size.height;
    let wanted: Vec<Wanted> = blocks
        .iter()
        .map(|index| Wanted::of(graph, order, positions, layer, *index))
        .collect();
    let mut turn: Vec<usize> = (0..blocks.len()).collect();
    turn.sort_by(|left, right| {
        wanted[*right]
            .strength()
            .cmp(&wanted[*left].strength())
            .then_with(|| left.cmp(right))
    });
    let mut tops: Vec<Option<f32>> = vec![None; blocks.len()];
    for row in turn {
        // The room between the nearest placed card above and below, with the
        // unplaced cards between them stacked tight.
        let lower = (0..row)
            .rev()
            .find_map(|above| tops[above].map(|top| (above, top)))
            .map_or(f32::NEG_INFINITY, |(above, top)| {
                top + (above..row)
                    .map(|between| height(between) + gap)
                    .sum::<f32>()
            });
        let upper = (row + 1..blocks.len())
            .find_map(|below| tops[below].map(|top| (below, top)))
            .map_or(f32::INFINITY, |(below, top)| {
                top - (row..below)
                    .map(|between| height(between) + gap)
                    .sum::<f32>()
            });
        let top = wanted[row]
            .top(lower, upper)
            .unwrap_or(if lower.is_finite() {
                lower
            } else if upper.is_finite() {
                upper
            } else {
                0.0
            });
        tops[row] = Some(top);
    }
    tops.into_iter().map(|top| top.unwrap_or(0.0)).collect()
}

/// The geometry of shape D (LAY-06): a spine in rows, satellites under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rows {
    /// The widest a row may be. The spine wraps to a new row when the next
    /// column would not fit: five columns of 280 px at the default.
    pub width: f32,
    /// The width of a column, the widest card it holds.
    pub column_width: f32,
    /// Free space between two columns, where the data wires run.
    pub column_gap: f32,
    /// Free space between a card and the satellite hung under it.
    pub stack_gap: f32,
    /// Free space between the lowest card of a row and the next row, which
    /// holds the rail the next row's ordering wires run on.
    pub row_gap: f32,
    /// The height a board may reach, as a share of its width, by rows grown
    /// to run a wire between two spine steps straight (LAY-04): the shape of
    /// the view a fit fills, so that growing never lowers the fit zoom. The
    /// default is the board's view in the folder window at 1600 × 1000,
    /// 1134 × 760 px.
    pub aspect: f32,
}

impl Default for Rows {
    fn default() -> Self {
        let column_width = 280.0;
        let column_gap = 96.0;
        Self {
            width: 5.0 * column_width + 4.0 * column_gap,
            column_width,
            column_gap,
            stack_gap: 24.0,
            row_gap: 96.0,
            aspect: 760.0 / 1134.0,
        }
    }
}

impl Rows {
    /// How many spine columns a row holds, at least one.
    pub fn columns(&self) -> usize {
        let pitch = self.column_width + self.column_gap;
        if !(pitch > 0.0 && self.width.is_finite()) {
            return 1;
        }
        (((self.width + self.column_gap) / pitch).floor() as usize).max(1)
    }
}

/// A board in shape D: the spine in the order given, left to right in rows
/// that wrap after [`Rows::columns`], and each satellite in the column of the
/// spine block it hangs under, stacked below it in the order given.
#[derive(Clone, Debug)]
pub struct Hanging<'a> {
    pub blocks: &'a [Block],
    /// The spine, in the engine's order.
    pub spine: &'a [usize],
    /// Each satellite with the index in `spine` of the block it hangs under,
    /// in the order they stack.
    pub satellites: &'a [(usize, usize)],
    /// The wires (LAY-04). A spine block moves down its row so that a wire
    /// with ports into it from the column before runs straight; one at
    /// either end of a wire without ports, an ordering wire on the rail
    /// above the row, stays at the row's top.
    pub edges: &'a [Edge],
    pub rows: Rows,
}

/// One row of a board in shape D.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The spine blocks of the row, left to right.
    pub spine: Vec<usize>,
    /// The top of the row, where its spine blocks start.
    pub top: f32,
    /// The tallest stack of the row from the row's top: a spine block and
    /// its satellites, and how far the ports dropped them (LAY-04).
    pub height: f32,
}

/// Shape D: one position per block, and the rows. A block that is neither
/// on the spine nor a satellite keeps its position.
///
/// The ports decide where a stack starts in its row (LAY-04, [`drops`]):
/// the first stack of a row at the row's top, every other one as far down
/// as makes a wire from the column before into its spine block straight,
/// within the room the row has under it, or, for a wire from the spine block
/// before, as far as the board's [`Rows::aspect`] lets the rows grow. A row
/// is as tall as its tallest stack from the row's top, and the next row
/// starts [`Rows::row_gap`] below it. The result is anchored like
/// [`tidy`]'s: the top-left corner of the blocks laid out stays where it is.
pub fn hang(hanging: &Hanging<'_>) -> (Vec<Point>, Vec<Row>) {
    let blocks = hanging.blocks;
    let mut positions: Vec<Point> = blocks.iter().map(|block| block.position).collect();
    let spine: Vec<usize> = hanging
        .spine
        .iter()
        .copied()
        .filter(|index| *index < blocks.len())
        .collect();
    if spine.len() != hanging.spine.len() || spine.is_empty() {
        return (positions, Vec::new());
    }
    let mut stacks: Vec<Vec<usize>> = spine.iter().map(|index| vec![*index]).collect();
    for &(block, under) in hanging.satellites {
        if block < blocks.len()
            && let Some(stack) = stacks.get_mut(under)
            && !stack.contains(&block)
        {
            stack.push(block);
        }
    }
    let rows = hanging.rows;
    let per = rows.columns();
    let pitch = rows.column_width + rows.column_gap;
    // Each block's offset from the top of its stack, and each stack's height.
    let mut offsets = vec![0.0f32; blocks.len()];
    let heights: Vec<f32> = stacks
        .iter()
        .map(|stack| {
            let mut y = 0.0;
            for (place, &block) in stack.iter().enumerate() {
                if place > 0 {
                    y += rows.stack_gap;
                }
                offsets[block] = y;
                y += blocks[block].size.height;
            }
            y
        })
        .collect();
    // What the rows may grow by in all, for straight wires between spine
    // steps: up to the height the board's width allows at `aspect`.
    let tallest: Vec<f32> = heights
        .chunks(per)
        .map(|chunk| chunk.iter().copied().fold(0.0, f32::max))
        .collect();
    let natural = tallest.iter().sum::<f32>() + (tallest.len() - 1) as f32 * rows.row_gap;
    let wide = per.min(stacks.len()) as f32 * pitch - rows.column_gap;
    let mut budget = (wide * rows.aspect - natural).max(0.0);
    let mut laid = Vec::new();
    let mut top = 0.0;
    for (number, chunk) in stacks.chunks(per).enumerate() {
        let first = number * per;
        let heights = &heights[first..first + chunk.len()];
        let drops = drops(
            chunk,
            &offsets,
            heights,
            (tallest[number], budget),
            hanging.edges,
        );
        let height = heights
            .iter()
            .zip(&drops)
            .map(|(height, drop)| height + drop)
            .fold(0.0, f32::max);
        budget = (budget - (height - tallest[number])).max(0.0);
        for (column, stack) in chunk.iter().enumerate() {
            let left = column as f32 * pitch;
            for &block in stack {
                let size = blocks[block].size;
                positions[block] = Point::new(
                    left + (rows.column_width - size.width) * 0.5,
                    top + drops[column] + offsets[block],
                );
            }
        }
        laid.push(Row {
            spine: spine[first..first + chunk.len()].to_vec(),
            top,
            height,
        });
        top += height + rows.row_gap;
    }
    // The blocks laid out keep their top-left corner where it was.
    let moved: Vec<usize> = stacks.iter().flatten().copied().collect();
    let anchor = Rect::bounding(moved.iter().map(|index| blocks[*index].rect()))
        .map(|bounds| bounds.min)
        .unwrap_or_default();
    let now = Rect::bounding(
        moved
            .iter()
            .map(|index| Rect::from_size(positions[*index], blocks[*index].size)),
    )
    .map(|bounds| bounds.min)
    .unwrap_or_default();
    let shift = Point::new(anchor.x - now.x, anchor.y - now.y);
    for index in moved {
        positions[index].x += shift.x;
        positions[index].y += shift.y;
    }
    for row in &mut laid {
        row.top += shift.y;
    }
    (positions, laid)
}

/// How far each stack of a row of shape D starts below the row's top (LAY-04).
///
/// A stack whose spine block is fed by a wire with ports from the stack
/// before it drops so that the wire runs straight: the drop of the stack
/// before plus the wire's misalignment, never above the row's top. Of
/// several such wires the one whose drop fits is taken, then the one the
/// most agree with, then the smallest drop. A drop fits when the stack stays
/// within the row as tall as it is, `tallest` at first; a wire from the spine
/// block before may also grow the row by up to `growth` in all. A stack keeps
/// to the row's top when nothing feeds it from the column before, when no
/// drop fits, and when its spine block is at either end of a wire without
/// ports, an ordering wire on the rail above the row (LAY-06), so that the
/// rail meets it there. The first stack of a row stays at the top.
/// `offsets` are the blocks' tops within their stacks.
fn drops(
    stacks: &[Vec<usize>],
    offsets: &[f32],
    heights: &[f32],
    (tallest, growth): (f32, f32),
    edges: &[Edge],
) -> Vec<f32> {
    let mut drops = vec![0.0f32; stacks.len()];
    let mut height = tallest;
    let railed = |block: usize| {
        edges
            .iter()
            .any(|edge| !edge.ports && (edge.source == block || edge.target == block))
    };
    for column in 1..stacks.len() {
        let spine = stacks[column][0];
        if railed(spine) {
            continue;
        }
        let before = &stacks[column - 1];
        // Each wire's misalignment, and whether it comes from the spine block.
        let wires: Vec<(f32, bool)> = edges
            .iter()
            .filter(|edge| edge.ports && edge.target == spine && before.contains(&edge.source))
            .map(|edge| {
                (
                    offsets[edge.source] + edge.source_offset - edge.target_offset,
                    edge.source == before[0],
                )
            })
            .collect();
        let misalignments: Vec<f32> = wires
            .iter()
            .map(|(misalignment, _)| *misalignment)
            .collect();
        let fits = |(misalignment, from_spine): (f32, bool)| {
            let drop = drops[column - 1] + misalignment;
            let limit = if from_spine { tallest + growth } else { height };
            drop >= -0.5 && drop + heights[column] <= limit.max(height) + 0.5
        };
        let Some(wire) = wires.iter().copied().min_by(|left, right| {
            fits(*right)
                .cmp(&fits(*left))
                .then_with(|| {
                    agreeing(&misalignments, right.0).cmp(&agreeing(&misalignments, left.0))
                })
                .then_with(|| compare(left.0.abs(), right.0.abs()))
                .then_with(|| compare(left.0, right.0))
        }) else {
            continue;
        };
        if fits(wire) {
            drops[column] = (drops[column - 1] + wire.0).max(0.0);
            height = height.max(drops[column] + heights[column]);
        }
    }
    drops
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

    /// LAY-02 criterion 1: a scenario opened from code has no positions, so
    /// every block is fresh. `place` then lays it out in the columns of
    /// `tidy`, and no two cards overlap or come closer than the row gap.
    #[test]
    fn place_with_every_block_fresh_gives_the_columns_of_tidy() {
        // The shape of a small scenario: a start, a request with its check, two
        // requests reading it, and a last step reading both; heights differ.
        let heights = [64.0, 220.0, 120.0, 300.0, 180.0, 120.0, 260.0];
        let blocks: Vec<Block> = heights
            .iter()
            .map(|height| Block::new(Size::new(280.0, *height), Point::default()))
            .collect();
        let edges = [
            Edge::new(0, 1),
            Edge::new(1, 2),
            Edge::new(1, 3),
            Edge::new(1, 4),
            Edge::new(3, 6),
            Edge::new(4, 6),
            Edge::new(6, 5),
        ];
        let graph = Graph::new(&blocks, &edges);
        let fresh: Vec<usize> = (0..blocks.len()).collect();
        let placed = place(&graph, &fresh);
        let tidied = tidy(&graph);
        assert_eq!(columns_of(&placed), columns_of(&tidied));
        let layers = layers_of(&graph);
        for (index, position) in placed.iter().enumerate() {
            assert_eq!(
                position.x.round(),
                tidied[index].x.round(),
                "block {index} is in its layer's column"
            );
            for (other, at) in placed.iter().enumerate().skip(index + 1) {
                if layers[other] != layers[index] {
                    continue;
                }
                let (upper, lower) = if position.y <= at.y {
                    (index, other)
                } else {
                    (other, index)
                };
                let gap = placed[lower].y - (placed[upper].y + blocks[upper].size.height);
                assert!(
                    gap >= Spacing::default().row_gap - 0.5,
                    "blocks {upper} and {lower} are {gap} apart: {placed:?}"
                );
            }
        }
        assert!(!any_overlap(&placed, &blocks));
        assert!(
            columns_of(&placed).len() > 1,
            "the flow is laid out, not stacked: {placed:?}"
        );
    }

    /// LAY-02 criterion 2: one fresh block among placed ones is placed as
    /// before: in the column after its source's, level with it.
    #[test]
    fn place_with_one_fresh_block_is_unchanged() {
        let mut blocks = blocks(3);
        blocks[0].position = Point::new(0.0, 0.0);
        blocks[0].pinned = true;
        blocks[1].position = Point::new(400.0, 200.0);
        blocks[1].pinned = true;
        blocks[2].position = Point::new(0.0, 0.0);
        let edges = [Edge::new(0, 1), Edge::new(1, 2)];
        let placed = place(&Graph::new(&blocks, &edges), &[2]);
        let spacing = Spacing::default();
        assert_eq!(
            placed,
            [Point::new(400.0 + 280.0 + spacing.layer_gap, 200.0)]
        );
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

    /// A spine of `count` 280 px blocks of `height`, in order, at the origin.
    fn spine_blocks(count: usize, height: f32) -> Vec<Block> {
        (0..count)
            .map(|_| Block::new(Size::new(280.0, height), Point::default()))
            .collect()
    }

    /// LAY-06 criterion 1: the spine runs left to right in the order given
    /// and wraps after the columns the row width holds, five at the default.
    #[test]
    fn lay06_the_spine_wraps_after_the_row_width() {
        let blocks = spine_blocks(12, 100.0);
        let spine: Vec<usize> = (0..12).rev().collect();
        let rows = Rows::default();
        assert_eq!(rows.columns(), 5);
        let (positions, laid) = hang(&Hanging {
            blocks: &blocks,
            spine: &spine,
            satellites: &[],
            edges: &[],
            rows,
        });
        assert_eq!(laid.len(), 3);
        assert_eq!(laid[0].spine, [11, 10, 9, 8, 7]);
        for (place, &block) in spine.iter().enumerate() {
            let (row, column) = (place / 5, place % 5);
            assert_eq!(positions[block].x, column as f32 * 376.0, "block {block}");
            assert_eq!(positions[block].y, laid[row].top, "block {block}");
        }
        // The row width is a parameter: three columns fit 1100 px.
        let narrow = Rows {
            width: 1100.0,
            ..Rows::default()
        };
        assert_eq!(narrow.columns(), 3);
        let (_, laid) = hang(&Hanging {
            blocks: &blocks,
            spine: &spine,
            satellites: &[],
            edges: &[],
            rows: narrow,
        });
        assert_eq!(laid.len(), 4);
    }

    /// LAY-06 criteria 1, 2 and 13: satellites stack under their spine
    /// block in the order given, and a row is as tall as its tallest stack.
    #[test]
    fn lay06_satellites_stack_and_a_row_is_its_tallest_stack() {
        let mut blocks = spine_blocks(7, 120.0);
        blocks[5].size.height = 70.0;
        blocks[6].size.height = 96.0;
        let spine = [0, 1, 2, 3, 4];
        let satellites = [(6, 1), (5, 1)];
        let rows = Rows {
            width: 3.0 * 280.0 + 2.0 * 96.0,
            ..Rows::default()
        };
        let (positions, laid) = hang(&Hanging {
            blocks: &blocks,
            spine: &spine,
            satellites: &satellites,
            edges: &[],
            rows,
        });
        assert_eq!(positions[6].x, positions[1].x);
        assert_eq!(positions[6].y, 120.0 + rows.stack_gap);
        assert_eq!(positions[5].y, positions[6].y + 96.0 + rows.stack_gap);
        let tallest = 120.0 + rows.stack_gap + 96.0 + rows.stack_gap + 70.0;
        assert_eq!(laid[0].height, tallest);
        assert_eq!(laid[1].top, tallest + rows.row_gap);
        assert_eq!(laid[1].height, 120.0, "a row without satellites");
        assert!(!any_overlap(&positions, &blocks));
    }

    /// LAY-06 criterion 6, and Tidy up's anchor: the same input gives the
    /// same board, kept where its top-left corner was.
    #[test]
    fn lay06_hanging_is_deterministic_and_stays_in_place() {
        let mut blocks = spine_blocks(8, 120.0);
        for (index, block) in blocks.iter_mut().enumerate() {
            block.position = Point::new(900.0 + index as f32 * 3.0, 400.0 + index as f32);
        }
        let spine = [0, 1, 2, 3, 4, 5];
        let satellites = [(7, 0), (6, 5)];
        let hanging = Hanging {
            blocks: &blocks,
            spine: &spine,
            satellites: &satellites,
            edges: &[],
            rows: Rows::default(),
        };
        let (first, _) = hang(&hanging);
        assert_eq!(first, hang(&hanging).0);
        let bounds = Rect::bounding(
            first
                .iter()
                .zip(&blocks)
                .map(|(position, block)| Rect::from_size(*position, block.size)),
        )
        .unwrap();
        assert_eq!(bounds.min, Point::new(900.0, 400.0));
        // An empty spine lays out nothing.
        let (kept, rows) = hang(&Hanging {
            spine: &[],
            ..hanging
        });
        assert!(rows.is_empty());
        assert_eq!(
            kept,
            blocks
                .iter()
                .map(|block| block.position)
                .collect::<Vec<_>>()
        );
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

    /// The wires of `edges` routed on the cards where `positions` put them,
    /// each from its port on the right side of its source to its port on
    /// the left side of its target.
    fn routed(blocks: &[Block], positions: &[Point], edges: &[Edge]) -> Vec<crate::Route> {
        use crate::{Port, Routing, Side, Wire, route};
        let rectangles: Vec<Rect> = positions
            .iter()
            .zip(blocks)
            .map(|(position, block)| Rect::from_size(*position, block.size))
            .collect();
        let wires: Vec<Wire> = edges
            .iter()
            .map(|edge| {
                Wire::new(
                    Port::new(edge.source, Side::Right, edge.source_offset),
                    Port::new(edge.target, Side::Left, edge.target_offset),
                )
            })
            .collect();
        route(&rectangles, &wires, &Routing::default())
    }

    /// LAY-04 criterion 1: a chain of five cards, each consuming the
    /// previous card's output into its first input, tidies so that every
    /// wire has zero bends. The cards differ in height and in where their
    /// ports are, as a board's do, and start anywhere.
    #[test]
    fn lay04_criterion_1_a_chain_of_five_runs_straight() {
        let heights = [132.0, 190.0, 166.0, 214.0, 142.0];
        let outputs = [98.0, 122.0, 98.0, 146.0, 98.0];
        let inputs = [52.0, 74.0, 51.0, 74.0, 52.0];
        let blocks: Vec<Block> = heights
            .iter()
            .enumerate()
            .map(|(index, height)| {
                Block::new(
                    Size::new(280.0, *height),
                    Point::new(index as f32 * 37.0, 300.0 - index as f32 * 53.0),
                )
            })
            .collect();
        let edges: Vec<Edge> = (0..4)
            .map(|index| {
                Edge::new(index, index + 1).with_offsets(outputs[index], inputs[index + 1])
            })
            .collect();
        let positions = tidy(&Graph::new(&blocks, &edges));
        assert!(!any_overlap(&positions, &blocks));
        for (edge, route) in edges.iter().zip(routed(&blocks, &positions, &edges)) {
            assert_eq!(
                route.bends(),
                0,
                "{} -> {} runs straight: {:?}",
                edge.source,
                edge.target,
                route.points
            );
        }
    }

    /// LAY-04 criterion 2: two wires leave one card from different ports.
    /// Their targets tie on the barycentre, and the ports break the tie: the
    /// target of the upper port stands above, so the wires do not cross
    /// right after the card.
    #[test]
    fn lay04_criterion_2_ports_break_ties() {
        let blocks = blocks(3);
        // Card 0 feeds card 1 from its lower port and card 2 from its upper.
        let edges = [
            Edge::new(0, 1).with_offsets(100.0, 60.0),
            Edge::new(0, 2).with_offsets(40.0, 60.0),
        ];
        let graph = Graph::new(&blocks, &edges);
        assert_eq!(rows_of(&graph)[1], [2, 1], "the upper port's target first");
        let positions = tidy(&graph);
        assert!(positions[2].y < positions[1].y, "{positions:?}");
        for route in routed(&blocks, &positions, &edges) {
            assert!(route.bridges.is_empty(), "no crossing: {:?}", route.points);
        }
    }

    /// A spine of five whose each step feeds the next from port 98 into port
    /// 74, so that each must stand 24 px below the one before to be straight.
    fn chain_of_five() -> (Vec<Block>, Vec<Edge>, [usize; 5]) {
        let blocks = spine_blocks(5, 120.0);
        let edges = (0..4)
            .map(|index| Edge::new(index, index + 1).with_offsets(98.0, 74.0))
            .collect();
        (blocks, edges, [0, 1, 2, 3, 4])
    }

    /// LAY-04 in shape D: where the row has room under its spine, a tall
    /// satellite here, every step drops to run the wire from the step before
    /// straight, the row's first step stays at its top and the row does not
    /// grow.
    #[test]
    fn lay04_a_row_of_shape_d_runs_its_spine_straight_in_its_room() {
        let (mut blocks, edges, spine) = chain_of_five();
        blocks.push(Block::new(Size::new(280.0, 400.0), Point::default()));
        let rows = Rows {
            aspect: 0.0,
            ..Rows::default()
        };
        let (positions, laid) = hang(&Hanging {
            blocks: &blocks,
            spine: &spine,
            satellites: &[(5, 0)],
            edges: &edges,
            rows,
        });
        assert_eq!(positions[0].y, laid[0].top);
        for edge in &edges {
            assert_eq!(
                positions[edge.source].y + edge.source_offset,
                positions[edge.target].y + edge.target_offset,
                "{} -> {} is straight",
                edge.source,
                edge.target
            );
        }
        assert_eq!(laid[0].height, 120.0 + rows.stack_gap + 400.0);
        assert!(!any_overlap(&positions, &blocks));
    }

    /// LAY-04 in shape D: a row grows for a straight wire between two spine
    /// steps only while the board stays within its aspect, so the fit zoom
    /// never drops; a wire from a satellite never grows it.
    #[test]
    fn lay04_a_row_grows_only_within_the_aspect() {
        let (blocks, edges, spine) = chain_of_five();
        let level = hang(&Hanging {
            blocks: &blocks,
            spine: &spine,
            satellites: &[],
            edges: &edges,
            rows: Rows {
                aspect: 0.0,
                ..Rows::default()
            },
        });
        assert!(
            level.0.iter().all(|position| position.y == 0.0),
            "no room, no growth: {:?}",
            level.0
        );
        let rows = Rows::default();
        let (positions, laid) = hang(&Hanging {
            blocks: &blocks,
            spine: &spine,
            satellites: &[],
            edges: &edges,
            rows,
        });
        for (step, position) in positions.iter().enumerate() {
            assert_eq!(position.y, step as f32 * 24.0, "step {step}");
        }
        assert_eq!(laid[0].height, 120.0 + 4.0 * 24.0);
        assert!(laid[0].height <= rows.width * rows.aspect);
        // A wire from a satellite into the next step does not grow the row.
        let mut blocks = spine_blocks(3, 120.0);
        blocks.push(Block::new(Size::new(280.0, 60.0), Point::default()));
        let edges = [Edge::new(3, 1).with_offsets(30.0, 74.0)];
        let (positions, laid) = hang(&Hanging {
            blocks: &blocks,
            spine: &[0, 1, 2],
            satellites: &[(3, 0)],
            edges: &edges,
            rows,
        });
        assert_eq!(laid[0].height, 120.0 + rows.stack_gap + 60.0);
        assert_eq!(positions[1].y, 0.0, "the drop would not fit: {positions:?}");
    }

    /// LAY-04 and LAY-06: a spine step at either end of an ordering wire,
    /// which runs on the rail above the row, stays at the row's top, and so
    /// does the step after it when its wire cannot be made straight.
    #[test]
    fn lay04_an_ordering_wire_keeps_its_ends_at_the_top() {
        let (blocks, mut edges, spine) = chain_of_five();
        edges.push(Edge::new(1, 3));
        let (positions, _) = hang(&Hanging {
            blocks: &blocks,
            spine: &spine,
            satellites: &[],
            edges: &edges,
            rows: Rows::default(),
        });
        assert_eq!(positions[1].y, 0.0, "{positions:?}");
        assert_eq!(positions[3].y, 0.0, "{positions:?}");
        assert_eq!(positions[2].y, 24.0, "{positions:?}");
        assert_eq!(positions[4].y, 24.0, "{positions:?}");
    }
}
