//! Chips at the reader (LAY-23): a value many steps read draws a quiet chip
//! `← cartId` beside each reader's input instead of a long wire.
//!
//! An output port is drawn with chips when [`MIN_READERS`] or more steps of
//! the document read it, or when it is a variable's (LAY-25) and one of its
//! readers is not beside it: in the next column of the source's row, or in
//! the source's own column. Then every reader of the port has a chip, the
//! one beside it too: a value shown as a chip is read only through chips,
//! and a port never shows a count and a line (LAY-32). The source's port
//! says how many steps read it. The wire comes back while its chip or its
//! input is pointed at and while its reader or its source is selected.
//!
//! It is a rendering rule only. Nothing here is saved, no card's box
//! changes, Tidy up and the router's other wires do not depend on it: the
//! wires that are chips are simply not given to the router, and a wire
//! brought back is routed apart, so the wires already drawn keep their
//! routes. The rule is decided on the cards where they rest, so a card
//! being dragged turns no wire into a chip or back until it is dropped.

use super::{
    Camera,
    layout::{self, Card},
    paint,
    routing::{BoardWire, Start},
    shape::on_spine,
};
use crate::theme;
use apiforge_core::run::StepResult;
use apiforge_core::{
    format::{Binding, Graph, Project},
    graph::{self, WireKind, wires as document_wires},
};
use eframe::egui::{FontId, Galley, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
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
    /// The source step alone, for a zoom where the text has no room; for a
    /// variable, the number of its card's badge (`7`, LAY-07), so that the
    /// chip still points at it (LAY-25).
    pub short: String,
    /// The whole reference, `cartId → checkout.body.cartId`: the chip's
    /// accessible name and what pointing at it shows.
    pub statement: String,
    /// The wires the chip stands for: one, or several reads of one value at
    /// one input.
    pub wires: Vec<String>,
    /// The input's socket, in board coordinates.
    pub anchor: Pos2,
    /// Its place among the chips of its reader, counted down the card.
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

/// The tops of the board's rows and the middles of its columns: of its
/// spine cards. Tidy up widens the gap between two columns where lines need
/// it (LAY-03), so a column is told by its place in the order, not by a
/// distance.
fn rows(cards: &[Card]) -> (Vec<f32>, Vec<f32>) {
    let spine = || cards.iter().filter(|card| on_spine(card.kind));
    let mut rows: Vec<f32> = spine().map(|card| card.rect().top()).collect();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    let mut columns: Vec<f32> = spine().map(|card| card.rect().center().x).collect();
    columns.sort_by(f32::total_cmp);
    columns.dedup_by(|a, b| (*a - *b).abs() < layout::WIDTH / 2.0);
    (rows, columns)
}

fn column_of(columns: &[f32], x: f32) -> usize {
    columns
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
        .map_or(0, |(index, _)| index)
}

fn row_of(rows: &[f32], y: f32) -> usize {
    rows.iter()
        .rposition(|top| *top <= y + 1.0)
        .unwrap_or_default()
}

/// A reader beside its source: in the same row, in the source's column or
/// the next one.
fn beside((rows, columns): &(Vec<f32>, Vec<f32>), from: Rect, to: Rect) -> bool {
    let (here, there) = (
        column_of(columns, from.center().x),
        column_of(columns, to.center().x),
    );
    row_of(rows, from.top()) == row_of(rows, to.top()) && (there == here || there == here + 1)
}

/// Whether the card `to` is far from the card `from` on the board: not
/// beside it (LAY-25's drop asks first there).
pub fn far(cards: &[Card], from: &Card, to: &Card) -> bool {
    !beside(&rows(cards), from.rect(), to.rect())
}

/// The wires of `wires` that are chips, decided on `cards` where they rest:
/// every wire of a port drawn with chips (LAY-32). A wire from a proxy the
/// person placed stays a wire: the proxy is there to shorten it.
pub fn chipped(graph: &Graph, cards: &[Card], wires: &[BoardWire]) -> HashSet<String> {
    let readers = readers_by_port(graph);
    let rows = rows(cards);
    // The data wires of each output port, as the board draws them.
    let mut ports: HashMap<(usize, &str), Vec<&BoardWire>> = HashMap::new();
    for wire in wires.iter().filter(|wire| !wire.control) {
        if let Start::Output { card, port } = &wire.start {
            ports.entry((*card, port.as_str())).or_default().push(wire);
        }
    }
    ports
        .into_iter()
        .filter(|((card, port), port_wires)| {
            let Some(source) = cards.get(*card) else {
                return false;
            };
            let count = readers
                .get(&(source.id.clone(), (*port).to_owned()))
                .map_or(0, BTreeSet::len);
            // A variable read far is read through chips whatever the count:
            // saving a value is how a person says "do not wire this" (LAY-25).
            let saved = graph
                .nodes
                .iter()
                .any(|node| node.id == source.id && node.is_variable());
            let far = || {
                port_wires.iter().any(|wire| {
                    cards
                        .get(wire.target)
                        .is_some_and(|target| !beside(&rows, source.rect(), target.rect()))
                })
            };
            count >= MIN_READERS || (saved && far())
        })
        .flat_map(|(_, port_wires)| port_wires.into_iter().map(|wire| wire.id.clone()))
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
        at = binding.at(std::slice::from_ref(segment));
    }
    fields
}

/// What a chip says, its shorter form, and the reference written whole.
/// `number` is the source's place in the engine's order, its badge.
pub(super) fn texts(
    graph: &Graph,
    project: &Project,
    wire: &BoardWire,
    reader: &str,
    source: &str,
    number: Option<usize>,
) -> (String, String, String) {
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
    let (text, short, whole) = if source_node.is_variable() {
        let whole = if field.is_empty() {
            source_node.handle.clone()
        } else {
            format!("{}.{field}", source_node.handle)
        };
        // Its name, else the number on its card: never the bare arrow
        // while a number fits (LAY-25).
        let short = number.map_or_else(|| source_node.handle.clone(), |n| n.to_string());
        (source_node.handle.clone(), short, whole)
    } else {
        let last = field.rsplit('.').next().filter(|last| !last.is_empty());
        let text = format!("{}.{}", source_node.handle, last.unwrap_or(&port));
        // As the code writes it: a step with one output port is named
        // without it (`cart.id`, not `cart.<port>.id`).
        let port = if graph::output_ports(source_node, project).len() == 1 {
            ""
        } else {
            port.as_str()
        };
        let whole = [source_node.handle.as_str(), port, field.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(".");
        (text, source_node.handle.clone(), whole)
    };
    let target = std::iter::once(reader.handle.clone())
        .chain(std::iter::once(wire.input.clone()))
        .chain(fields(binding, &wire.operand_path))
        .collect::<Vec<_>>()
        .join(".");
    (text, short, format!("{whole} → {target}"))
}

/// The chips and counts of the wires in `chipped`, at the cards as they are
/// drawn now. A wire whose input a compact card does not show has neither:
/// it is not drawn either way.
pub fn plan(
    graph: &Graph,
    project: &Project,
    cards: &[Card],
    wires: &[BoardWire],
    chipped: &HashSet<String>,
) -> (Vec<Tag>, Vec<Count>) {
    let readers = readers_by_port(graph);
    let numbers: HashMap<&str, usize> = graph::sequence(graph)
        .into_iter()
        .enumerate()
        .map(|(index, id)| (id, index + 1))
        .collect();
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
        let number = numbers.get(source.id.as_str()).copied();
        let (text, short, statement) = texts(graph, project, wire, &reader.id, &source.id, number);
        // One chip per value at a socket: a second read of it adds nothing.
        if let Some(tag) = tags
            .iter_mut()
            .find(|tag| tag.anchor == anchor && tag.source == source.id && tag.port == *port)
        {
            tag.wires.push(wire.id.clone());
        } else {
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
                slot: 0,
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
            let node = graph.nodes.iter().find(|node| node.id == source.id);
            let name = match node {
                Some(node) if node.is_variable() => node.handle.clone(),
                Some(node) => format!("{}.{port}", node.handle),
                None => format!("{}.{port}", source.name),
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
    // The chips of one card stand in order down its left edge.
    tags.sort_by(|a, b| a.card.cmp(&b.card).then(a.anchor.y.total_cmp(&b.anchor.y)));
    let mut previous = None;
    let mut slot = 0;
    for tag in &mut tags {
        slot = if previous == Some(tag.card) {
            slot + 1
        } else {
            0
        };
        tag.slot = slot;
        previous = Some(tag.card);
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
                pos2(tag.anchor.x - GAP, tag.anchor.y - ROW / 2.0),
                pos2(tag.anchor.x, tag.anchor.y + ROW * (tag.slot as f32 + 0.5)),
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
    /// The text the chip shows at this zoom, and its size on screen.
    #[cfg_attr(not(test), allow(dead_code))]
    pub shown: String,
    pub rect: Rect,
    #[cfg_attr(not(test), allow(dead_code))]
    pub font: f32,
    /// The stub from the box to the socket.
    pub stub: [Pos2; 2],
    /// Where a stacked chip's stub bends down to the socket.
    pub bend: Option<Pos2>,
    /// The socket or port the stub ends at, on screen.
    pub socket: Pos2,
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

/// The chips of `tags` left of their sockets, in the gap between two
/// columns; their text keeps a legible size at any zoom and shortens to the
/// source step (a variable's badge number), then to the arrow, where the gap has no room for it. The
/// chips of one card stand one under the other: a chip that would cover the
/// one above it moves down and its stub bends to its socket.
pub fn lay_tags(painter: &Painter, tags: &[Tag], camera: Camera) -> Vec<Laid> {
    let font = paint::min_on_screen(TEXT, camera.zoom, paint::LINE_MIN);
    let font_id = FontId::proportional(font);
    let stub = stub_length(camera.zoom);
    let room = GAP * camera.zoom - stub - 2.0;
    let mut laid: Vec<Laid> = Vec::with_capacity(tags.len());
    let mut below: Option<(usize, f32)> = None;
    for tag in tags {
        let anchor = camera.to_screen(tag.anchor);
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
        let free = match below {
            Some((card, bottom)) if card == tag.card => bottom + 2.0,
            _ => f32::MIN,
        };
        let centre = anchor.y.max(free + size.y / 2.0);
        let rect = Rect::from_min_size(pos2(anchor.x - stub - size.x, centre - size.y / 2.0), size);
        below = Some((tag.card, rect.bottom()));
        let (stub, bend) = if (centre - anchor.y).abs() < 0.5 {
            ([pos2(rect.right(), anchor.y), anchor], None)
        } else {
            let x = rect.right() + stub / 2.0;
            (
                [pos2(rect.right(), centre), pos2(x, centre)],
                Some(pos2(x, anchor.y)),
            )
        };
        laid.push(Laid {
            galley,
            shown,
            rect,
            font,
            stub,
            bend,
            socket: anchor,
        });
    }
    laid
}

/// The count at a source's port, right of it in the gap. Where it would
/// cover something in `avoid` (a continuation mark, on screen) it stands
/// under it and its stub bends up to the port.
pub fn lay_count(
    painter: &Painter,
    count: &Count,
    camera: Camera,
    avoid: &[Rect],
    value: Option<(String, String)>,
) -> Laid {
    let anchor = camera.to_screen(count.anchor);
    let font = paint::min_on_screen(TEXT, camera.zoom, paint::LINE_MIN);
    let font_id = FontId::proportional(font);
    let stub = stub_length(camera.zoom);
    let room = GAP * camera.zoom - stub - 2.0;
    let (shown, galley) = fitting(
        painter,
        &value
            .iter()
            .flat_map(|(name, value)| {
                [
                    format!("→ {} · {name} {value}", count.readers),
                    format!("→ {} · {value}", count.readers),
                ]
            })
            .chain([
                format!("→ {} readers", count.readers),
                format!("→ {}", count.readers),
                "→".to_owned(),
            ])
            .collect::<Vec<_>>(),
        &font_id,
        room,
    );
    let size = galley.size() + vec2(8.0, 3.0);
    let mut rect = Rect::from_min_size(pos2(anchor.x + stub, anchor.y - size.y / 2.0), size);
    for _ in 0..avoid.len() {
        let Some(over) = avoid.iter().find(|other| other.intersects(rect)) else {
            break;
        };
        rect = rect.translate(vec2(0.0, over.bottom() + 2.0 - rect.top()));
    }
    let (stub, bend) = if (rect.center().y - anchor.y).abs() < 0.5 {
        ([anchor, pos2(rect.left(), anchor.y)], None)
    } else {
        let x = anchor.x + stub / 2.0;
        (
            [pos2(rect.left(), rect.center().y), pos2(x, rect.center().y)],
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
        socket: anchor,
    }
}

/// The mark `+ var` under a count (LAY-25): the port has values worth a
/// variable. It stands in the gap under the count, at the count's size, and
/// moves down past whatever in `avoid` it would cover (a chip, a count,
/// another mark, the row of another output of the card, on screen); where
/// the gap has no room for its text it says `+`, and with no room for that
/// it is not drawn.
pub fn lay_mark(painter: &Painter, count: &Laid, camera: Camera, avoid: &[Rect]) -> Option<Laid> {
    let font_id = FontId::proportional(count.font);
    let stub = stub_length(camera.zoom);
    let room = GAP * camera.zoom - stub - 2.0;
    let (shown, galley) = fitting(
        painter,
        &["+ var".to_owned(), "+".to_owned()],
        &font_id,
        room,
    );
    let size = galley.size() + vec2(8.0, 3.0);
    if size.x > room {
        return None;
    }
    let mut rect = Rect::from_min_size(pos2(count.rect.left(), count.rect.bottom() + 3.0), size);
    for _ in 0..=avoid.len() {
        let Some(over) = avoid.iter().find(|other| other.intersects(rect)) else {
            break;
        };
        rect = rect.translate(vec2(0.0, over.bottom() + 2.0 - rect.top()));
    }
    Some(Laid {
        galley,
        shown,
        rect,
        font: count.font,
        stub: [rect.left_center(), rect.left_center()],
        bend: None,
        socket: rect.left_center(),
    })
}

/// The mark `+ var`: an action, so in the accent; filled while pointed at
/// or while its menu is open.
pub fn paint_mark(painter: &Painter, laid: &Laid, lit: bool) {
    let fill = if lit {
        theme::ACCENT_SOFT
    } else {
        theme::WHITE
    };
    painter.rect(
        laid.rect,
        laid.rect.height() / 2.0,
        fill,
        Stroke::new(1.0, theme::ACCENT),
        StrokeKind::Inside,
    );
    painter.galley(
        laid.rect.min + vec2(4.0, 1.5),
        laid.galley.clone(),
        theme::ACCENT,
    );
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
        painter.line_segment([bend, laid.socket], line);
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

/// The statement of what is pointed at or kept, in a box above `near`, or
/// under it where there is no room above, kept whole inside `within`
/// (LAY-14). Returns the box.
pub fn paint_statement(painter: &Painter, near: Rect, text: &str, within: Rect) -> Rect {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        FontId::proportional(theme::FONT_META),
        theme::TEXT,
    );
    let size = galley.size() + vec2(12.0, 6.0);
    let mut at = pos2(near.left(), near.top() - size.y - 4.0);
    if at.y < within.top() {
        at.y = near.bottom() + 4.0;
    }
    at.x = at.x.min(within.right() - size.x).max(within.left());
    at.y = at.y.min(within.bottom() - size.y).max(within.top());
    let rect = Rect::from_min_size(at, size);
    painter.rect(
        rect,
        4.0,
        theme::WHITE,
        Stroke::new(1.0, theme::ACCENT),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(6.0, 3.0), galley, theme::TEXT);
    rect
}

/// MOCK (LAY-18): what the board shows at a reader count. Throwaway.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Mock18 {
    /// Main as it is.
    #[default]
    Off,
    /// Option A: pointing at a count lists the fields read and who reads
    /// each; a click keeps the list with the wires.
    List,
    /// Option B: one count per field in the gap.
    PerField,
    /// Question 3, placement 1: the list, each field with the last run's
    /// value.
    ListValues,
    /// Question 3, placement 2: the last run's value of the most read
    /// field beside the count.
    CountValue,
}

/// MOCK (LAY-18): the fields of `port` the document reads, as the code
/// writes them, each with its readers in the document's order and whether
/// the reader is a check.
pub fn fields_read(
    graph: &Graph,
    project: &Project,
    source: &str,
    port: &str,
) -> Vec<(String, Vec<(String, bool)>)> {
    let Some(source_node) = graph.nodes.iter().find(|node| node.id == source) else {
        return Vec::new();
    };
    let shown_port =
        if source_node.is_variable() || graph::output_ports(source_node, project).len() == 1 {
            ""
        } else {
            port
        };
    let mut fields: Vec<(String, Vec<(String, bool)>)> = Vec::new();
    for wire in document_wires(graph) {
        if wire.kind != WireKind::Data
            || wire.source != source
            || wire.source_port.as_deref() != Some(port)
        {
            continue;
        }
        let Some(reader) = graph.nodes.iter().find(|node| node.id == wire.target) else {
            continue;
        };
        let leaf = wire
            .target_input
            .as_ref()
            .and_then(|input| reader.inputs.get(input))
            .and_then(|binding| binding.at(&wire.operand_path));
        let field = match leaf {
            Some(Binding::Reference { path, .. }) => path.clone(),
            _ => String::new(),
        };
        let name = [source_node.handle.as_str(), shown_port, field.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(".");
        let entry = (reader.handle.clone(), reader.kind.is_check());
        match fields.iter_mut().find(|(field, _)| *field == name) {
            Some((_, readers)) => {
                if !readers.contains(&entry) {
                    readers.push(entry);
                }
            }
            None => fields.push((name, vec![entry])),
        }
    }
    fields.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    fields
}

/// MOCK (LAY-18, option A): the list a count shows, one line per field.
pub fn field_list(
    graph: &Graph,
    project: &Project,
    source: &str,
    port: &str,
    steps: Option<&[StepResult]>,
) -> String {
    const NAMED: usize = 6;
    fields_read(graph, project, source, port)
        .into_iter()
        .map(|(field, readers)| {
            let field = match run_value(graph, steps, source, port, &field, 28) {
                Some(value) => format!("{field} = {value}"),
                None => field,
            };
            let checks = readers.iter().all(|(_, check)| *check);
            let unit = match (checks, readers.len()) {
                (true, 1) => "check",
                (true, _) => "checks",
                (false, 1) => "reader",
                (false, _) => "readers",
            };
            let mut names: Vec<String> = readers
                .iter()
                .take(NAMED)
                .map(|(name, _)| name.clone())
                .collect();
            if readers.len() > NAMED {
                names.push(format!("+{} more", readers.len() - NAMED));
            }
            format!("{field} → {} {unit}: {}", readers.len(), names.join(", "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// MOCK (LAY-18, option B): one count per field, stacked down the gap from
/// the port.
pub fn lay_fields(
    painter: &Painter,
    graph: &Graph,
    project: &Project,
    count: &Count,
    camera: Camera,
) -> Vec<Laid> {
    let anchor = camera.to_screen(count.anchor);
    let font = paint::min_on_screen(TEXT, camera.zoom, paint::LINE_MIN);
    let font_id = FontId::proportional(font);
    let stub = stub_length(camera.zoom);
    let room = GAP * camera.zoom - stub - 2.0;
    let mut laid = Vec::new();
    let mut top = f32::MIN;
    for (field, readers) in fields_read(graph, project, &count.source, &count.port) {
        let last = field.rsplit('.').next().unwrap_or(&field).to_owned();
        let (shown, galley) = fitting(
            painter,
            &[
                format!("{last} → {}", readers.len()),
                format!("→ {}", readers.len()),
            ],
            &font_id,
            room,
        );
        let size = galley.size() + vec2(8.0, 3.0);
        let centre = anchor.y.max(top + size.y / 2.0);
        let rect = Rect::from_min_size(pos2(anchor.x + stub, centre - size.y / 2.0), size);
        top = rect.bottom() + 2.0;
        let (stub_line, bend) = if (centre - anchor.y).abs() < 0.5 {
            ([anchor, pos2(rect.left(), anchor.y)], None)
        } else {
            let x = anchor.x + stub / 2.0;
            (
                [pos2(rect.left(), centre), pos2(x, centre)],
                Some(pos2(x, anchor.y)),
            )
        };
        laid.push(Laid {
            galley,
            shown,
            rect,
            font,
            stub: stub_line,
            bend,
            socket: anchor,
        });
    }
    laid
}

/// MOCK (LAY-18, question 3): the last run's value of `field` (as
/// [`fields_read`] names it) of `port`, written short.
pub fn run_value(
    graph: &Graph,
    steps: Option<&[StepResult]>,
    source: &str,
    port: &str,
    field: &str,
    most: usize,
) -> Option<String> {
    let step = steps?.iter().find(|step| step.node_id == source)?;
    let mut value = step.outputs.get(port)?;
    let handle = &graph.nodes.iter().find(|node| node.id == source)?.handle;
    let rest = field.strip_prefix(handle.as_str())?;
    let rest = rest.strip_prefix('.').unwrap_or(rest);
    let rest = rest.strip_prefix(port).unwrap_or(rest);
    for key in rest.split(['.', '[', ']']).filter(|key| !key.is_empty()) {
        value = match key.parse::<usize>() {
            Ok(index) if value.is_array() => value.get(index)?,
            _ => value.get(key)?,
        };
    }
    let text = value.to_string();
    Some(if text.chars().count() > most {
        format!("{}…", text.chars().take(most).collect::<String>())
    } else {
        text
    })
}

/// MOCK (LAY-18, question 3): the most read field of a count and its value.
pub fn count_value(
    graph: &Graph,
    project: &Project,
    count: &Count,
    steps: Option<&[StepResult]>,
) -> Option<(String, String)> {
    let (field, _) = fields_read(graph, project, &count.source, &count.port)
        .into_iter()
        .next()?;
    let value = run_value(graph, steps, &count.source, &count.port, &field, 8)?;
    Some((field.rsplit('.').next()?.to_owned(), value))
}
