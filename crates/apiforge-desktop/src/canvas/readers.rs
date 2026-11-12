//! Chips at the reader (LAY-23): a value many steps read draws a quiet chip
//! `← cartId` beside each reader's input instead of a long wire.
//!
//! A data wire becomes a chip when its output port is read by
//! [`MIN_READERS`] or more steps of the document and its reader is not
//! beside the source: in the next column of the source's row, or in the
//! source's own column. The source's port says how many steps read it. The
//! wire comes back while its chip or its input is pointed at and while its
//! reader or its source is selected.
//!
//! It is a rendering rule only. Nothing here is saved, no card's box
//! changes, Tidy up and the router's other wires do not depend on it: the
//! wires that are chips are simply not given to the router, and a wire
//! brought back is routed apart, so the wires already drawn keep their
//! routes. The rule is decided on the cards where they rest, so a card
//! being dragged turns no wire into a chip or back until it is dropped.

use super::{
    Camera,
    layout::{self, Card, Chip},
    paint,
    routing::{BoardWire, Start},
    shape::on_spine,
};
use crate::theme;
use apiforge_core::{
    format::{Binding, Graph},
    graph::{WireKind, wires as document_wires},
};
use eframe::egui::{FontId, Painter, Pos2, Rect, Stroke, StrokeKind, galley::Galley, pos2, vec2};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

/// How the board draws the data wires of a value many steps read. A user
/// setting (*Wires* in the board's view menu), never part of the document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WireMode {
    /// One wire per reader, as before LAY-23.
    All,
    /// *Chips for shared values*: a chip at each far reader (decision 2 of
    /// 2026-11-12: the default).
    #[default]
    Chips,
}

/// A port read by at least this many steps draws chips (decision 3).
pub const MIN_READERS: usize = 3;

/// The free space Tidy up leaves between two columns (shape D), where a
/// chip and a count stand.
const GAP: f32 = 96.0;

/// The chip's text before the camera's zoom, and its smallest on screen.
const TEXT: f32 = 11.0;

/// A chip at a reader's input: the wire it stands for is not drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    /// The reader's node id, and its card.
    pub reader: String,
    pub card: usize,
    /// The source's node id and output port.
    pub source: String,
    pub port: String,
    pub input: String,
    pub path: Vec<String>,
    /// What the chip says: the variable's name, or the step and the last
    /// field it reads (`login.token`).
    pub text: String,
    /// The source step alone, for a zoom where the text has no room.
    pub short: String,
    /// The whole reference, `cartId → checkout.body.cartId`: the chip's
    /// accessible name and what pointing at it shows.
    pub statement: String,
    /// The wires the chip stands for: one, or several reads of one value at
    /// one input.
    pub wires: Vec<String>,
    /// The input's socket, in board coordinates.
    pub anchor: Pos2,
    /// Its place among the chips of one socket, counted upwards.
    pub slot: usize,
}

/// The mark at a source's port: how many steps read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Count {
    pub source: String,
    pub card: usize,
    pub port: String,
    pub readers: usize,
    /// `cartId → 9 readers`, its accessible name.
    pub label: String,
    /// Every wire of the port that is a chip.
    pub wires: Vec<String>,
    pub anchor: Pos2,
}

/// The steps that read each output port, by `(source, port)`, as the
/// document says: what is folded or drawn does not change it.
fn readers_by_port(graph: &Graph) -> HashMap<(String, String), BTreeSet<String>> {
    let mut readers: HashMap<(String, String), BTreeSet<String>> = HashMap::new();
    for wire in document_wires(graph) {
        if wire.kind != WireKind::Data {
            continue;
        }
        let Some(port) = wire.source_port else {
            continue;
        };
        readers
            .entry((wire.source, port))
            .or_default()
            .insert(wire.target);
    }
    readers
}

/// The tops of the board's rows: of its spine cards.
fn rows(cards: &[Card]) -> Vec<f32> {
    let mut rows: Vec<f32> = cards
        .iter()
        .filter(|card| on_spine(card.kind))
        .map(|card| card.rect().top())
        .collect();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    rows
}

fn row_of(rows: &[f32], y: f32) -> usize {
    rows.iter()
        .rposition(|top| *top <= y + 1.0)
        .unwrap_or_default()
}

/// A reader beside its source: in the same row, in the source's column or
/// the next one.
fn beside(rows: &[f32], from: Rect, to: Rect) -> bool {
    let pitch = layout::WIDTH + GAP;
    let step = to.left() - from.left();
    let half = layout::WIDTH / 2.0;
    row_of(rows, from.top()) == row_of(rows, to.top())
        && (step.abs() < half || (step - pitch).abs() < half)
}

/// The wires of `wires` that are chips, decided on `cards` where they rest.
/// A wire from a proxy the person placed stays a wire: the proxy is there
/// to shorten it.
pub fn chipped(graph: &Graph, cards: &[Card], wires: &[BoardWire]) -> HashSet<String> {
    let readers = readers_by_port(graph);
    let rows = rows(cards);
    wires
        .iter()
        .filter(|wire| !wire.control)
        .filter_map(|wire| {
            let Start::Output { card, port } = &wire.start else {
                return None;
            };
            let (source, target) = (cards.get(*card)?, cards.get(wire.target)?);
            let count = readers
                .get(&(source.id.clone(), port.clone()))
                .map_or(0, BTreeSet::len);
            (count >= MIN_READERS && !beside(&rows, source.rect(), target.rect()))
                .then(|| wire.id.clone())
        })
        .collect()
}

/// The fields a reference sits in inside an input: the keys of the objects
/// on its path, not an operand's or a template part's position.
fn fields(binding: Option<&Binding>, path: &[String]) -> Vec<String> {
    let mut fields = Vec::new();
    let mut at = binding;
    for segment in path {
        let Some(binding) = at else { break };
        if matches!(binding, Binding::Object { .. }) {
            fields.push(segment.clone());
        }
        at = binding.child(segment);
    }
    fields
}

/// What a chip says, its source step, and the reference written whole.
fn texts(graph: &Graph, wire: &BoardWire, reader: &str, source: &str) -> (String, String, String) {
    let reader = graph.nodes.iter().find(|node| node.id == reader);
    let source_node = graph.nodes.iter().find(|node| node.id == source);
    let (Some(reader), Some(source_node)) = (reader, source_node) else {
        return Default::default();
    };
    let binding = reader.inputs.get(&wire.input);
    let leaf = binding.and_then(|binding| binding.at(&wire.operand_path));
    let (port, field) = match leaf {
        Some(Binding::Reference { port, path, .. }) => (port.clone(), path.clone()),
        _ => (String::new(), String::new()),
    };
    let (text, whole) = if source_node.is_variable() {
        let whole = if field.is_empty() {
            source_node.handle.clone()
        } else {
            format!("{}.{field}", source_node.handle)
        };
        (source_node.handle.clone(), whole)
    } else {
        let last = field.rsplit('.').next().filter(|last| !last.is_empty());
        let text = format!("{}.{}", source_node.handle, last.unwrap_or(&port));
        let whole = [source_node.handle.as_str(), port.as_str(), field.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(".");
        (text, whole)
    };
    let target = std::iter::once(reader.handle.clone())
        .chain(std::iter::once(wire.input.clone()))
        .chain(fields(binding, &wire.operand_path))
        .collect::<Vec<_>>()
        .join(".");
    (
        text,
        source_node.handle.clone(),
        format!("{whole} → {target}"),
    )
}

/// The chips and counts of the wires in `chipped`, at the cards as they are
/// drawn now. A wire whose input a compact card does not show has neither:
/// it is not drawn either way.
pub fn plan(
    graph: &Graph,
    cards: &[Card],
    chips: &[Chip],
    wires: &[BoardWire],
    chipped: &HashSet<String>,
) -> (Vec<Tag>, Vec<Count>) {
    let _ = chips;
    let readers = readers_by_port(graph);
    let mut tags: Vec<Tag> = Vec::new();
    let mut counts: Vec<Count> = Vec::new();
    for wire in wires.iter().filter(|wire| chipped.contains(&wire.id)) {
        let Start::Output { card, port } = &wire.start else {
            continue;
        };
        let (Some(source), Some(reader)) = (cards.get(*card), cards.get(wire.target)) else {
            continue;
        };
        let Some(anchor) = reader.input_anchor(&wire.input, &wire.operand_path) else {
            continue;
        };
        let (text, short, statement) = texts(graph, wire, &reader.id, &source.id);
        // One chip per value at a socket: a second read of it adds nothing.
        if let Some(tag) = tags
            .iter_mut()
            .find(|tag| tag.anchor == anchor && tag.source == source.id && tag.port == *port)
        {
            tag.wires.push(wire.id.clone());
        } else {
            let slot = tags.iter().filter(|tag| tag.anchor == anchor).count();
            tags.push(Tag {
                reader: reader.id.clone(),
                card: wire.target,
                source: source.id.clone(),
                port: port.clone(),
                input: wire.input.clone(),
                path: wire.operand_path.clone(),
                text,
                short,
                statement,
                wires: vec![wire.id.clone()],
                anchor,
                slot,
            });
        }
        if let Some(count) = counts
            .iter_mut()
            .find(|count| count.source == source.id && count.port == *port)
        {
            count.wires.push(wire.id.clone());
        } else if let Some(at) = source.output_anchor(port) {
            let number = readers
                .get(&(source.id.clone(), port.clone()))
                .map_or(0, BTreeSet::len);
            let name = if graph
                .nodes
                .iter()
                .any(|node| node.id == source.id && node.is_variable())
            {
                source.name.clone()
            } else {
                format!("{}.{port}", source.name)
            };
            counts.push(Count {
                source: source.id.clone(),
                card: *card,
                port: port.clone(),
                readers: number,
                label: format!("{name} → {number} readers"),
                wires: vec![wire.id.clone()],
                anchor: at,
            });
        }
    }
    (tags, counts)
}

/// The room a chip and a count take on the board, for Fit: at most the gap
/// beside the card, and a few rows of text high.
pub fn bounds(tags: &[Tag], counts: &[Count]) -> Vec<Rect> {
    const ROW: f32 = 32.0;
    tags.iter()
        .map(|tag| {
            Rect::from_min_max(
                pos2(
                    tag.anchor.x - GAP,
                    tag.anchor.y - ROW * (tag.slot as f32 + 0.5),
                ),
                pos2(tag.anchor.x, tag.anchor.y + ROW / 2.0),
            )
        })
        .chain(counts.iter().map(|count| {
            Rect::from_min_max(
                pos2(count.anchor.x, count.anchor.y - ROW / 2.0),
                pos2(count.anchor.x + GAP, count.anchor.y + ROW / 2.0),
            )
        }))
        .collect()
}

/// A chip or a count laid out on screen at this zoom.
#[derive(Clone)]
pub struct Laid {
    pub galley: Arc<Galley>,
    /// The text the chip shows at this zoom.
    pub shown: String,
    pub rect: Rect,
    pub font: f32,
    /// The stub from the box to the socket.
    pub stub: [Pos2; 2],
    /// Where a stacked chip's stub bends down to the socket.
    pub bend: Option<Pos2>,
}

fn stub_length(zoom: f32) -> f32 {
    (10.0 * zoom).max(4.0)
}

/// The first of `texts` that fits `room` on screen, else the last.
fn fitting(painter: &Painter, texts: &[String], font: &FontId, room: f32) -> (String, Arc<Galley>) {
    let mut last = None;
    for text in texts {
        let galley = painter.layout_no_wrap(text.clone(), font.clone(), theme::MUTED);
        if galley.size().x + 8.0 <= room {
            return (text.clone(), galley);
        }
        last = Some((text.clone(), galley));
    }
    last.expect("a text to lay out")
}

/// A chip left of its socket, in the gap between two columns; its text
/// keeps a legible size at any zoom and shortens to the source step, then
/// to the arrow, where the gap has no room for it.
pub fn lay_tag(painter: &Painter, tag: &Tag, camera: Camera) -> Laid {
    let anchor = camera.to_screen(tag.anchor);
    let font = paint::min_on_screen(TEXT, camera.zoom, paint::LINE_MIN);
    let font_id = FontId::proportional(font);
    let stub = stub_length(camera.zoom);
    let room = GAP * camera.zoom - stub - 2.0;
    let (shown, galley) = fitting(
        painter,
        &[
            format!("← {}", tag.text),
            format!("← {}", tag.short),
            "←".to_owned(),
        ],
        &font_id,
        room,
    );
    let size = galley.size() + vec2(8.0, 3.0);
    let centre = anchor.y - tag.slot as f32 * (size.y + 2.0);
    let rect = Rect::from_min_size(pos2(anchor.x - stub - size.x, centre - size.y / 2.0), size);
    let (stub, bend) = if tag.slot == 0 {
        ([pos2(rect.right(), anchor.y), anchor], None)
    } else {
        let x = rect.right() + stub / 2.0;
        (
            [pos2(rect.right(), centre), pos2(x, centre)],
            Some(pos2(x, anchor.y)),
        )
    };
    Laid {
        galley,
        shown,
        rect,
        font,
        stub,
        bend,
    }
}

/// The count at a source's port, right of it in the gap.
pub fn lay_count(painter: &Painter, count: &Count, camera: Camera) -> Laid {
    let anchor = camera.to_screen(count.anchor);
    let font = paint::min_on_screen(TEXT, camera.zoom, paint::LINE_MIN);
    let font_id = FontId::proportional(font);
    let stub = stub_length(camera.zoom);
    let room = GAP * camera.zoom - stub - 2.0;
    let (shown, galley) = fitting(
        painter,
        &[
            format!("→ {} readers", count.readers),
            format!("→ {}", count.readers),
        ],
        &font_id,
        room,
    );
    let size = galley.size() + vec2(8.0, 3.0);
    let rect = Rect::from_min_size(pos2(anchor.x + stub, anchor.y - size.y / 2.0), size);
    Laid {
        galley,
        shown,
        rect,
        font,
        stub: [anchor, pos2(rect.left(), anchor.y)],
        bend: None,
    }
}

/// A chip or a count: a grey outline and muted text, quieter than a proxy
/// chip; in the accent while its wire is drawn.
pub fn paint(painter: &Painter, laid: &Laid, lit: bool) {
    let (border, ink, fill) = if lit {
        (theme::ACCENT, theme::TEXT, theme::ACCENT_SOFT)
    } else {
        (theme::CONTROL_WIRE, theme::MUTED, theme::WHITE)
    };
    let line = Stroke::new(1.2, border);
    painter.line_segment(laid.stub, line);
    if let Some(bend) = laid.bend {
        painter.line_segment([laid.stub[1], bend], line);
    }
    painter.rect(
        laid.rect,
        laid.rect.height() / 2.0,
        fill,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    painter.galley(laid.rect.min + vec2(4.0, 1.5), laid.galley.clone(), ink);
}

/// The statement of a pointed chip, in a box above it.
pub fn paint_statement(painter: &Painter, near: Rect, text: &str) {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        FontId::proportional(theme::FONT_META),
        theme::TEXT,
    );
    let size = galley.size() + vec2(12.0, 6.0);
    let rect = Rect::from_min_size(pos2(near.left(), near.top() - size.y - 4.0), size);
    painter.rect(
        rect,
        4.0,
        theme::WHITE,
        Stroke::new(1.0, theme::ACCENT),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(6.0, 3.0), galley, theme::TEXT);
}
