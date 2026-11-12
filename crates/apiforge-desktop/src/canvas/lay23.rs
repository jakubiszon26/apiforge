//! LAY-23 mock-up (#333). Throwaway code of the branch `mock/lay-23-fan-out`,
//! never merged: three ways to thin the wires of a value many steps read.
//!
//! - **A, chips at the reader.** A wire of a fanned-out port is not drawn;
//!   its reader shows a small chip `← cartId` at the input instead. The
//!   wire comes back while its source or its reader is selected.
//! - **B, bus.** A fanned-out port draws one trunk: a riser in the column
//!   gap beside the source, one trunk in the row gap above each row with
//!   readers, and a short drop to every reader.
//! - **C, data wires on demand.** Only ordering wires are drawn; the data
//!   wires of the selected cards come back.
//!
//! Nothing here is saved: the plan is a pure function of the board's wires,
//! its cards and the selection.

use super::Camera;
use super::layout::{Card, Chip};
use super::routing::{BoardWire, Start};
use super::shape::on_spine;
use crate::theme;
use apiforge_core::format::{Binding, Graph};
use eframe::egui::{FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use std::collections::{HashMap, HashSet};

/// The free space Tidy up leaves between two columns.
const GAP: f32 = 96.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Today's board: one wire per reader.
    #[default]
    Off,
    Chips,
    Bus,
    OnDemand,
}

/// The threshold rule under test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub mode: Mode,
    /// A port read by at least this many inputs is fanned out.
    pub min_readers: usize,
    /// A reader beside its source (the next column of its row, or its own
    /// column) keeps its wire.
    pub keep_near: bool,
    /// Every wire that is not near becomes a chip, whatever the count (the
    /// distance-only rule).
    pub every_far_wire: bool,
    /// Which ordering wires are left out, for the measurements of the
    /// other ways to cut lines.
    pub order: Order,
    /// Several wires from one port into one card are drawn once.
    pub merge_parallel: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    All,
    /// Without an `@after` whose target reads its source's data anyway.
    WithoutImplied,
    /// Without an ordering wire into the next step of the spine.
    WithoutNext,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Off,
            min_readers: 3,
            keep_near: true,
            every_far_wire: false,
            order: Order::All,
            merge_parallel: false,
        }
    }
}

/// A chip at a reader's input (direction A).
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    pub anchor: Pos2,
    /// Its place among the chips of one input, counted upwards.
    pub slot: usize,
    /// `login.token`, or the variable's name.
    pub text: String,
    /// The source step alone, for a zoom where the whole text has no room.
    pub short: String,
    /// Its wire is drawn as well: the source or the reader is selected.
    pub shown: bool,
    /// The mark at the source's port instead: how many steps read it.
    pub out: bool,
}

impl Tag {
    /// The room the chip takes on the board, for Fit: estimated at the
    /// smallest legible text.
    pub fn bounds(&self) -> Rect {
        let width = 9.0 * (self.text.chars().count() as f32 + 2.0) + 24.0;
        if self.out {
            return Rect::from_min_max(
                pos2(self.anchor.x, self.anchor.y - 12.0),
                pos2(self.anchor.x + width.min(GAP), self.anchor.y + 12.0),
            );
        }
        Rect::from_min_max(
            pos2(
                self.anchor.x - width,
                self.anchor.y - 12.0 - 24.0 * self.slot as f32,
            ),
            pos2(self.anchor.x, self.anchor.y + 12.0),
        )
    }
}

/// One fanned-out port drawn as a bus (direction B): the whole path of each
/// reader, from the port to the input. The paths share the riser and the
/// trunks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bus {
    pub paths: Vec<Vec<Pos2>>,
    pub junctions: Vec<Pos2>,
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// The wires the router still routes.
    pub routed: Vec<BoardWire>,
    /// Wires drawn because of the selection, picked out like a selected wire.
    pub shown: HashSet<String>,
    pub tags: Vec<Tag>,
    pub buses: Vec<Bus>,
    /// Data wires not drawn at all right now.
    pub hidden: usize,
}

impl Plan {
    /// What the plan adds to the board's bounds.
    pub fn bounds(&self) -> Vec<Rect> {
        self.tags
            .iter()
            .map(Tag::bounds)
            .chain(self.buses.iter().filter_map(|bus| {
                bus.paths
                    .iter()
                    .flatten()
                    .map(|point| Rect::from_min_max(*point, *point))
                    .reduce(Rect::union)
                    .map(|rect| rect.expand(8.0))
            }))
            .collect()
    }
}

fn source_rect(wire: &BoardWire, cards: &[Card], chips: &[Chip]) -> Option<Rect> {
    match &wire.start {
        Start::Output { card, .. } => cards.get(*card).map(Card::rect),
        Start::Chip(index) => chips.get(*index).map(Chip::rect),
    }
}

fn source_anchor(wire: &BoardWire, cards: &[Card], chips: &[Chip]) -> Option<Pos2> {
    match &wire.start {
        Start::Output { card, port } => cards.get(*card)?.output_anchor(port),
        Start::Chip(index) => chips.get(*index).map(Chip::output_anchor),
    }
}

/// What a chip at the reader says: the variable's name, or the step and
/// the last field of what is read; and the source step alone.
fn source_text(graph: &Graph, target: &str, wire: &BoardWire) -> (String, String) {
    let Some(node) = graph.nodes.iter().find(|node| node.id == target) else {
        return Default::default();
    };
    let mut text = (String::new(), String::new());
    if let Some(binding) = node.inputs.get(&wire.input) {
        binding.for_each_leaf(&mut |leaf, path| {
            if path != wire.operand_path.as_slice() || !text.0.is_empty() {
                return;
            }
            if let Binding::Reference {
                node_id,
                port,
                path,
            } = leaf
                && let Some(source) = graph.nodes.iter().find(|node| &node.id == node_id)
            {
                let full = if source.is_variable() {
                    source.handle.clone()
                } else if path.is_empty() {
                    format!("{}.{port}", source.handle)
                } else {
                    let last = path.rsplit('.').next().unwrap_or(path);
                    format!("{}.{last}", source.handle)
                };
                text = (full, source.handle.clone());
            }
        });
    }
    text
}

pub fn plan(
    config: &Config,
    graph: &Graph,
    cards: &[Card],
    chips: &[Chip],
    wires: Vec<BoardWire>,
    selection: &HashSet<&str>,
) -> Plan {
    let wires = thinned(config, graph, cards, wires);
    if config.mode == Mode::Off {
        return Plan {
            routed: wires,
            ..Plan::default()
        };
    }
    let selected = |wire: &BoardWire| {
        let source = match &wire.start {
            Start::Output { card, .. } => cards.get(*card).map(|card| card.id.as_str()),
            Start::Chip(index) => chips.get(*index).map(Chip::id),
        };
        source.is_some_and(|id| selection.contains(id))
            || cards
                .get(wire.target)
                .is_some_and(|card| selection.contains(card.id.as_str()))
    };
    let mut plan = Plan::default();
    if config.mode == Mode::OnDemand {
        for wire in wires {
            if wire.control {
                plan.routed.push(wire);
            } else if selected(&wire) {
                plan.shown.insert(wire.id.clone());
                plan.routed.push(wire);
            } else {
                plan.hidden += 1;
            }
        }
        return plan;
    }
    // The rows of the board: the tops of its spine cards.
    let mut rows: Vec<f32> = cards
        .iter()
        .filter(|card| on_spine(card.kind))
        .map(|card| card.rect().top())
        .collect();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    let row_of = |y: f32| -> usize {
        rows.iter()
            .rposition(|top| *top <= y + 1.0)
            .unwrap_or_default()
    };
    // A reader beside its source: in the next column of the same row, or
    // in the source's own column.
    let near = |from: Rect, to: Rect| {
        let pitch = super::layout::WIDTH + GAP;
        let same_row = row_of(from.top()) == row_of(to.top());
        let step = to.left() - from.left();
        same_row && (step.abs() < 1.0 || (step - pitch).abs() < 1.0)
    };
    // The readers of each port.
    let mut nets: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, wire) in wires.iter().enumerate() {
        if !wire.control {
            nets.entry(format!("{:?}", wire.start))
                .or_default()
                .push(index);
        }
    }
    let fanned = |index: usize| -> bool {
        let wire = &wires[index];
        if wire.control {
            return false;
        }
        let (Some(from), Some(to)) = (
            source_rect(wire, cards, chips),
            cards.get(wire.target).map(Card::rect),
        ) else {
            return false;
        };
        if config.keep_near && near(from, to) {
            return false;
        }
        config.every_far_wire || nets[&format!("{:?}", wire.start)].len() >= config.min_readers
    };
    let taken: Vec<bool> = (0..wires.len()).map(fanned).collect();
    match config.mode {
        Mode::Chips => {
            let mut tags: Vec<Tag> = Vec::new();
            for (index, wire) in wires.iter().enumerate() {
                if !taken[index] {
                    plan.routed.push(wire.clone());
                    continue;
                }
                let card = &cards[wire.target];
                let Some(anchor) = card.input_anchor(&wire.input, &wire.operand_path) else {
                    plan.routed.push(wire.clone());
                    continue;
                };
                let shown = selected(wire);
                if shown {
                    plan.shown.insert(wire.id.clone());
                    plan.routed.push(wire.clone());
                } else {
                    plan.hidden += 1;
                }
                let (text, short) = source_text(graph, &card.id, wire);
                // One chip per source at an input: a second read of the
                // same value adds nothing.
                if let Some(tag) = tags
                    .iter_mut()
                    .find(|tag| tag.anchor == anchor && tag.text == text)
                {
                    tag.shown |= shown;
                    continue;
                }
                let slot = tags.iter().filter(|tag| tag.anchor == anchor).count();
                tags.push(Tag {
                    anchor,
                    slot,
                    text,
                    short,
                    shown,
                    out: false,
                });
            }
            // The source says how many steps read the port whose wires
            // are chips now.
            let mut keys: Vec<&String> = nets.keys().collect();
            keys.sort();
            for key in keys {
                let members = &nets[key];
                if !members.iter().any(|index| taken[*index]) {
                    continue;
                }
                let first = &wires[members[0]];
                let Some(anchor) = source_anchor(first, cards, chips) else {
                    continue;
                };
                let readers: HashSet<usize> =
                    members.iter().map(|index| wires[*index].target).collect();
                tags.push(Tag {
                    anchor,
                    slot: 0,
                    text: format!("{} readers", readers.len()),
                    short: readers.len().to_string(),
                    shown: members.iter().any(|index| selected(&wires[*index]))
                        && members
                            .iter()
                            .all(|index| !taken[*index] || plan.shown.contains(&wires[*index].id)),
                    out: true,
                });
            }
            plan.tags = tags;
        }
        Mode::Bus => {
            let mut keys: Vec<&String> = nets.keys().collect();
            keys.sort();
            let mut lane = 0.0;
            let mut bused: HashSet<usize> = HashSet::new();
            for key in keys {
                let members: Vec<usize> = nets[key]
                    .iter()
                    .copied()
                    .filter(|index| taken[*index])
                    .collect();
                // A bus of one drop is a wire.
                if members.len() < 2 {
                    continue;
                }
                let first = &wires[members[0]];
                let (Some(from), Some(start)) = (
                    source_rect(first, cards, chips),
                    source_anchor(first, cards, chips),
                ) else {
                    continue;
                };
                let riser = from.right() + 24.0 + 8.0 * lane;
                let mut bus = Bus::default();
                // (row, drop x, end)
                let mut drops: Vec<(usize, f32, Pos2)> = Vec::new();
                for index in &members {
                    let wire = &wires[*index];
                    let card = &cards[wire.target];
                    let Some(end) = card.input_anchor(&wire.input, &wire.operand_path) else {
                        continue;
                    };
                    bused.insert(*index);
                    drops.push((
                        row_of(card.rect().top()),
                        card.rect().left() - 24.0 - 8.0 * lane,
                        end,
                    ));
                }
                let trunk = |row: usize| rows[row] - 36.0 - 8.0 * lane;
                for (row, x, end) in &drops {
                    let y = trunk(*row);
                    bus.paths.push(vec![
                        start,
                        pos2(riser, start.y),
                        pos2(riser, y),
                        pos2(*x, y),
                        pos2(*x, end.y),
                        *end,
                    ]);
                }
                // Junction dots: where a drop leaves a trunk that goes on, and
                // where a trunk leaves a riser that goes on.
                let mut by_row: HashMap<usize, Vec<f32>> = HashMap::new();
                for (row, x, _) in &drops {
                    by_row.entry(*row).or_default().push(*x);
                }
                let lowest = by_row.keys().copied().max().unwrap_or_default();
                let highest = by_row.keys().copied().min().unwrap_or_default();
                for (row, xs) in &by_row {
                    let y = trunk(*row);
                    let (left, right) = (
                        xs.iter().copied().fold(f32::INFINITY, f32::min).min(riser),
                        xs.iter()
                            .copied()
                            .fold(f32::NEG_INFINITY, f32::max)
                            .max(riser),
                    );
                    for x in xs {
                        if *x > left + 0.5 && *x < right - 0.5 {
                            bus.junctions.push(pos2(*x, y));
                        }
                    }
                    let goes_on = if y > start.y {
                        *row < lowest
                    } else {
                        *row > highest
                    };
                    let both_sides = left < riser - 0.5 && right > riser + 0.5;
                    if goes_on || both_sides {
                        bus.junctions.push(pos2(riser, y));
                    }
                }
                // The riser leaves the stub both ways when rows above and
                // below are fed.
                if by_row.keys().any(|row| trunk(*row) < start.y)
                    && by_row.keys().any(|row| trunk(*row) > start.y)
                {
                    bus.junctions.push(pos2(riser, start.y));
                }
                bus.junctions.dedup();
                plan.buses.push(bus);
                lane += 1.0;
            }
            for (index, wire) in wires.iter().enumerate() {
                if !bused.contains(&index) {
                    plan.routed.push(wire.clone());
                }
            }
        }
        Mode::Off | Mode::OnDemand => unreachable!(),
    }
    plan
}

/// A bus: the riser and the trunks heavier than a wire, the drops as a wire.
pub fn paint_bus(painter: &Painter, bus: &Bus, camera: Camera) {
    let heavy = Stroke::new(2.6, theme::CONTROL_WIRE);
    let light = Stroke::new(1.6, theme::DATA_WIRE);
    for path in &bus.paths {
        let line: Vec<Pos2> = path.iter().map(|point| camera.to_screen(*point)).collect();
        if line.len() < 6 {
            continue;
        }
        painter.add(Shape::line(line[..4].to_vec(), heavy));
        painter.add(Shape::line(line[3..].to_vec(), light));
        let end = line[line.len() - 1];
        let direction = (end - line[line.len() - 2]).normalized();
        super::paint::arrowhead(painter, end, direction, theme::DATA_WIRE, camera.zoom);
    }
    let radius = (4.0 * camera.zoom).clamp(2.5, 5.0);
    for junction in &bus.junctions {
        painter.circle_filled(camera.to_screen(*junction), radius, theme::CONTROL_WIRE);
    }
}

/// The chip of a reader: a small pill left of the input, in the gap between
/// two columns, with a stub into the socket. Its text keeps a legible size
/// at any zoom; where the gap has no room for it, the chip says the source
/// step alone, and below that only the arrow.
pub fn paint_tag(painter: &Painter, tag: &Tag, camera: Camera) {
    let anchor = camera.to_screen(tag.anchor);
    let font = FontId::proportional((11.0 * camera.zoom).max(super::paint::LINE_MIN));
    let (border, ink) = if tag.shown {
        (theme::ACCENT, theme::TEXT)
    } else {
        (theme::CONTROL_WIRE, theme::MUTED)
    };
    let stub = (10.0 * camera.zoom).max(4.0);
    let room = GAP * camera.zoom - stub - 2.0;
    let arrow = if tag.out { "→" } else { "←" };
    let galley = [
        format!("{arrow} {}", tag.text),
        format!("{arrow} {}", tag.short),
        arrow.to_owned(),
    ]
    .into_iter()
    .map(|text| painter.layout_no_wrap(text, font.clone(), ink))
    .find(|galley| galley.size().x + 8.0 <= room)
    .unwrap_or_else(|| painter.layout_no_wrap(arrow.to_owned(), font.clone(), ink));
    let size = galley.size() + vec2(8.0, 3.0);
    let centre = anchor.y - tag.slot as f32 * (size.y + 1.0);
    let left = if tag.out {
        anchor.x + stub
    } else {
        anchor.x - stub - size.x
    };
    let rect = Rect::from_min_size(pos2(left, centre - size.y / 2.0), size);
    let line = Stroke::new(1.4, border);
    if tag.out {
        painter.line_segment([anchor, pos2(rect.left(), anchor.y)], line);
    } else if tag.slot == 0 {
        painter.line_segment([pos2(rect.right(), anchor.y), anchor], line);
    } else {
        // A stacked chip joins the stub of the one below it.
        let x = rect.right() + stub / 2.0;
        painter.line_segment([pos2(rect.right(), centre), pos2(x, centre)], line);
        painter.line_segment([pos2(x, centre), pos2(x, anchor.y)], line);
    }
    painter.rect(
        rect,
        size.y / 2.0,
        if tag.shown {
            theme::ACCENT_SOFT
        } else {
            theme::WHITE
        },
        Stroke::new(1.0, border),
        eframe::egui::StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(4.0, 1.5), galley, ink);
}

/// What the board drew, for the mock-up's measurements.
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    /// Routed wires: their points, and whether they order.
    pub routed: Vec<(Vec<Pos2>, bool)>,
    pub buses: Vec<Vec<Vec<Pos2>>>,
    pub tags: Vec<(Pos2, String)>,
    pub hidden: usize,
}

/// The other ways to cut lines, measured beside the three directions.
fn thinned(
    config: &Config,
    graph: &Graph,
    cards: &[Card],
    wires: Vec<BoardWire>,
) -> Vec<BoardWire> {
    use apiforge_core::graph::{WireKind, wires as document_wires};
    let mut reads: HashMap<String, Vec<String>> = HashMap::new();
    for wire in document_wires(graph) {
        if wire.kind == WireKind::Data {
            reads.entry(wire.target).or_default().push(wire.source);
        }
    }
    let depends = |target: &str, source: &str| {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut open = vec![target];
        while let Some(id) = open.pop() {
            for next in reads.get(id).into_iter().flatten() {
                if next == source {
                    return true;
                }
                if seen.insert(next) {
                    open.push(next);
                }
            }
        }
        false
    };
    let plan = super::shape::plan(graph, None).unwrap_or_default();
    let column = |id: &str| -> Option<usize> {
        let index = graph.nodes.iter().position(|node| node.id == id)?;
        plan.spine.iter().position(|at| *at == index).or_else(|| {
            plan.satellites
                .iter()
                .find(|(at, _)| *at == index)
                .map(|(_, under)| *under)
        })
    };
    let mut seen: HashSet<(String, usize)> = HashSet::new();
    wires
        .into_iter()
        .filter(|wire| {
            if wire.control {
                let target = &cards[wire.target].id;
                let source = graph
                    .edges
                    .iter()
                    .find(|edge| edge.id == wire.id)
                    .map(|edge| edge.source.clone());
                let Some(source) = source else { return true };
                return match config.order {
                    Order::All => true,
                    Order::WithoutImplied => !depends(target, &source),
                    Order::WithoutNext => {
                        let on_spine = plan.spine.iter().any(|at| graph.nodes[*at].id == *target);
                        !(on_spine
                            && wire.branch.is_none()
                            && matches!(
                                (column(&source), column(target)),
                                (Some(from), Some(to)) if to == from + 1
                            ))
                    }
                };
            }
            !config.merge_parallel || seen.insert((format!("{:?}", wire.start), wire.target))
        })
        .collect()
}
