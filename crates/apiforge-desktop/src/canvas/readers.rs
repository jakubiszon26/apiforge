//! Chips at the reader (LAY-23): a value many steps read draws a quiet chip
//! `← cartId` beside each reader's input instead of a long wire.
//!
//! An output port is drawn with chips when [`MIN_READERS`] or more steps of
//! the document read it, or when it is a variable's (LAY-25) and one of its
//! readers is not beside it: in the next column of the source's row, or in
//! the source's own column. Then every reader of the port has a chip, the
//! one beside it too: a value shown as a chip is read only through chips,
//! and a port never shows a count and a line (LAY-32). The source's port
//! says how many steps read it; pointing at the count lists the fields read
//! and who reads each (LAY-18). The wire comes back while its chip or its
//! input is pointed at and while its reader or its source is selected.
//!
//! It is a rendering rule only. Nothing here is saved, no card's box
//! changes, the router's other wires do not depend on it: the wires that
//! are chips are simply not given to the router, and a wire brought back is
//! routed apart, so the wires already drawn keep their routes. Tidy up
//! leaves the chips and the counts room in the gaps between columns, and
//! they are laid clear of each other, of the cards and of the lines drawn
//! (LAY-37). The rule is decided on the cards where they rest, so a card
//! being dragged turns no wire into a chip or back until it is dropped.

use super::{
    Camera,
    layout::{self, Card},
    paint,
    routing::{BoardWire, Start},
    shape::{self, on_spine},
};
use crate::theme;
use apiforge_core::{
    format::{Binding, Graph, Project},
    graph::{self, WireKind, wires as document_wires},
    run::StepResult,
};
use eframe::egui::{FontId, Galley, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
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

/// The room a chip or a count takes in the gap beside its card, at most;
/// Tidy up leaves it there clear of the lines (LAY-37).
pub(super) const GAP: f32 = 96.0;

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
    /// What the chip says where it has room: the variable's name, or the
    /// step and the last field it reads (`login.token`), its index kept
    /// (`rooms…[1].id`, LAY-39). The first of `forms`.
    pub text: String,
    /// Every text the chip may say, widest first; where none fits, the
    /// arrow alone. A step's read: the step and its last field, then the
    /// step (`login.token`, `login`); through a list element, the step, the
    /// last index and the field after it, then the step and the index, then
    /// the index alone (`rooms…[1].id`, `rooms…[1]`, `…[1]`, LAY-39). A
    /// variable: its name, then the number of its card's badge (`7`,
    /// LAY-07), so that the chip still points at it (LAY-25).
    pub forms: Vec<String>,
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
    let groups = shape::groups(cards);
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
            // A card hung under its source reads it through a chip: no
            // line runs inside the group (LAY-31).
            let hung = || {
                port_wires
                    .iter()
                    .any(|wire| shape::hung_under(cards, &groups, *card, wire.target))
            };
            count >= MIN_READERS || (saved && far()) || hung()
        })
        .flat_map(|(_, port_wires)| port_wires.into_iter().map(|wire| wire.id.clone()))
        .collect()
}

/// How a count names its readers: `1 reader`, `3 readers` (LAY-31: one
/// card hung under its source reads it through a chip).
fn noun(readers: usize) -> &'static str {
    if readers == 1 { "reader" } else { "readers" }
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

/// The last index of a path, `[n]`, with what stands before it and after
/// it: `items[1].id` → (`items`, `[1]`, `.id`). A quoted key (`["[2]"]`) is
/// not an index.
fn last_index(path: &str) -> Option<(&str, &str, &str)> {
    let bytes = path.as_bytes();
    let mut found = None;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => {
                // A quoted key, to its closing quote.
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
            }
            b'[' => {
                let digits = bytes[at + 1..]
                    .iter()
                    .take_while(|byte| byte.is_ascii_digit())
                    .count();
                if digits > 0 && bytes.get(at + 1 + digits) == Some(&b']') {
                    found = Some((at, at + digits + 2));
                    at += digits + 1;
                }
            }
            _ => {}
        }
        at += 1;
    }
    found.map(|(start, end)| (&path[..start], &path[start..end], &path[end..]))
}

/// What the chip of a step's read may say, widest first: the step and the
/// last field of `path` (`port` where the path is empty), then the step.
/// Through a list element the last index is kept (LAY-39): the step, `…`
/// where something between it and the index is left out (`port_named`, or
/// a path before the index), the index and what follows it; then the step
/// and the index; then the index alone.
fn step_forms(step: &str, port: &str, port_named: bool, path: &str) -> Vec<String> {
    let mut forms = match last_index(path) {
        Some((before, index, after)) => {
            let left_out = if port_named || !before.is_empty() {
                "…"
            } else {
                ""
            };
            vec![
                format!("{step}{left_out}{index}{after}"),
                format!("{step}{left_out}{index}"),
                format!("…{index}"),
            ]
        }
        None => {
            let last = path.rsplit('.').next().filter(|last| !last.is_empty());
            vec![format!("{step}.{}", last.unwrap_or(port)), step.to_owned()]
        }
    };
    forms.dedup();
    forms
}

/// What the chip of a variable may say: its name, else the number on its
/// card, never the bare arrow while a number fits (LAY-25).
fn variable_forms(handle: &str, number: Option<usize>) -> Vec<String> {
    let mut forms = vec![handle.to_owned()];
    forms.extend(number.map(|number| number.to_string()));
    forms
}

/// The reference written whole, as the code writes it, and its reader:
/// `rooms.body.items[1].id → second.body.roomId`. `port` is empty where the
/// code names the source without it.
fn statement(source: &str, port: &str, path: &str, target: &str) -> String {
    let whole = [source, port, path]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(".");
    format!("{whole} → {target}")
}

/// What a chip may say, widest first (see [`Tag::forms`]), and the
/// reference written whole. `number` is the source's place in the engine's
/// order, its badge.
pub(super) fn texts(
    graph: &Graph,
    project: &Project,
    wire: &BoardWire,
    reader: &str,
    source: &str,
    number: Option<usize>,
) -> (Vec<String>, String) {
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
    let target = std::iter::once(reader.handle.clone())
        .chain(std::iter::once(wire.input.clone()))
        .chain(fields(binding, &wire.operand_path))
        .collect::<Vec<_>>()
        .join(".");
    if source_node.is_variable() {
        return (
            variable_forms(&source_node.handle, number),
            statement(&source_node.handle, "", &field, &target),
        );
    }
    // As the code writes it: a step with one output port is named without
    // it (`cart.id`, not `cart.<port>.id`).
    let named = graph::output_ports(source_node, project).len() != 1;
    (
        step_forms(&source_node.handle, &port, named, &field),
        statement(
            &source_node.handle,
            if named { &port } else { "" },
            &field,
            &target,
        ),
    )
}

/// How many readers a line of a count's list names (LAY-18); the Inspector
/// lists them all.
const NAMED: usize = 6;

/// How long a value from the last run may be in a count's list (LAY-18).
const VALUE: usize = 28;

/// A field of an output port the document reads: as the code writes it
/// (`login.body.token`), its path in the port's value, and its readers in
/// the document's order, each with whether it is a check.
struct Field {
    name: String,
    path: String,
    readers: Vec<(String, bool)>,
}

/// The fields of `source`'s port `port` the document reads, most read first.
fn fields_read(graph: &Graph, project: &Project, source: &str, port: &str) -> Vec<Field> {
    let Some(source_node) = graph.nodes.iter().find(|node| node.id == source) else {
        return Vec::new();
    };
    // As a chip writes the reference: a variable, or a step with one output
    // port, is named without it.
    let named_port =
        if source_node.is_variable() || graph::output_ports(source_node, project).len() == 1 {
            ""
        } else {
            port
        };
    let mut fields: Vec<Field> = Vec::new();
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
        let path = match leaf {
            Some(Binding::Reference { path, .. }) => path.clone(),
            _ => String::new(),
        };
        let entry = (reader.handle.clone(), reader.kind.is_check());
        match fields.iter_mut().find(|field| field.path == path) {
            Some(field) => {
                if !field.readers.contains(&entry) {
                    field.readers.push(entry);
                }
            }
            None => fields.push(Field {
                name: [source_node.handle.as_str(), named_port, path.as_str()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("."),
                path,
                readers: vec![entry],
            }),
        }
    }
    fields.sort_by_key(|field| std::cmp::Reverse(field.readers.len()));
    fields
}

/// The value of `path` in `source`'s port `port` in the last run, as JSON
/// cut to [`VALUE`] characters; a secret is as the run report has it.
fn run_value(steps: &[StepResult], source: &str, port: &str, path: &str) -> Option<String> {
    let output = steps
        .iter()
        .find(|step| step.node_id == source)?
        .outputs
        .get(port)?;
    let value = if path.is_empty() {
        output.clone()
    } else {
        apiforge_core::path::resolve(output, path)?.into_owned()
    };
    let text = value.to_string();
    Some(if text.chars().count() > VALUE {
        format!("{}…", text.chars().take(VALUE).collect::<String>())
    } else {
        text
    })
}

/// What pointing at a count shows (LAY-18): one line per field of its port
/// the document reads, most read first, with the number of its readers and
/// the first [`NAMED`] of them, `login.body.token → 10 readers: catalogue,
/// …, +4 more`; a field only checks read counts checks. After a run, `steps`,
/// each line names the field's value before the arrow.
pub fn field_list(
    graph: &Graph,
    project: &Project,
    steps: Option<&[StepResult]>,
    count: &Count,
) -> String {
    fields_read(graph, project, &count.source, &count.port)
        .into_iter()
        .map(|field| {
            let checks = field.readers.iter().all(|(_, check)| *check);
            let unit = match (checks, field.readers.len()) {
                (true, 1) => "check",
                (true, _) => "checks",
                (false, 1) => "reader",
                (false, _) => "readers",
            };
            let mut names: Vec<&str> = field
                .readers
                .iter()
                .take(NAMED)
                .map(|(name, _)| name.as_str())
                .collect();
            let more = field.readers.len().saturating_sub(NAMED);
            let more = format!("+{more} more");
            if field.readers.len() > NAMED {
                names.push(&more);
            }
            let value = steps
                .and_then(|steps| run_value(steps, &count.source, &count.port, &field.path))
                .map_or_else(String::new, |value| format!(" = {value}"));
            format!(
                "{}{value} → {} {unit}: {}",
                field.name,
                field.readers.len(),
                names.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
        let (forms, statement) = texts(graph, project, wire, &reader.id, &source.id, number);
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
                text: forms.first().cloned().unwrap_or_default(),
                forms,
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
                label: format!("{name} → {number} {}", noun(number)),
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

/// The room on screen a chip or a count keeps from another chip or count,
/// from a card and from a line (LAY-37).
const CLEAR: f32 = 4.0;

/// How far a chip or a count may stand from its socket to keep that room,
/// in its own heights, and the step it is moved by.
const REACH: f32 = 4.0;
const NUDGE: f32 = 1.0;

/// What the chips and the counts keep clear of, on screen (LAY-37).
pub struct Clear<'a> {
    /// The cards and their badges.
    pub boxes: &'a [Rect],
    /// The lines drawn at rest.
    pub lines: &'a [Vec<Pos2>],
}

/// Whether the segment `a`–`b` passes through `rect`.
fn crosses(rect: Rect, a: Pos2, b: Pos2) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for (p, q) in [
        (-dx, a.x - rect.left()),
        (dx, rect.right() - a.x),
        (-dy, a.y - rect.top()),
        (dy, rect.bottom() - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else if p < 0.0 {
            low = low.max(q / p);
        } else {
            high = high.min(q / p);
        }
    }
    low <= high
}

/// The obstacles in `band`: boxes, and the segments of lines. Only what
/// is in it is known, so a place whose room reaches out of it is not free.
struct Near {
    band: Rect,
    boxes: Vec<Rect>,
    segments: Vec<[Pos2; 2]>,
}

impl Near {
    fn of(band: Rect, clear: &Clear<'_>, laid: &[Rect]) -> Self {
        Self {
            band,
            boxes: clear
                .boxes
                .iter()
                .chain(laid)
                .filter(|other| other.intersects(band))
                .copied()
                .collect(),
            segments: clear
                .lines
                .iter()
                .flat_map(|line| line.windows(2).map(|pair| [pair[0], pair[1]]))
                .filter(|[a, b]| Rect::from_two_pos(*a, *b).intersects(band))
                .collect(),
        }
    }

    fn free(&self, rect: Rect) -> bool {
        let around = rect.expand(CLEAR - 0.5);
        self.band.contains_rect(around)
            && !self.boxes.iter().any(|other| other.intersects(around))
            && !self.segments.iter().any(|[a, b]| crosses(around, *a, *b))
    }
}

/// Where a box of `size` keeps its room from what is `near`: the place
/// nearest its socket's height `want`, not above `lowest`, at one of the
/// `edges` it may stand at, the first edge preferred at a height; `None`
/// when there is none within [`REACH`]. `at` gives the box for an edge and
/// the height of its centre.
fn spot(
    at: impl Fn(f32, f32) -> Rect,
    edges: &[f32],
    want: f32,
    lowest: f32,
    near: &Near,
) -> Option<Rect> {
    let start = want.max(lowest);
    let first = *edges.first()?;
    let reach = REACH * at(first, start).height();
    let steps = (reach / NUDGE).ceil() as usize;
    let at = &at;
    (0..=steps)
        .flat_map(|step| {
            let by = step as f32 * NUDGE;
            [start + by, start - by]
                .into_iter()
                .take(if step == 0 { 1 } else { 2 })
        })
        .filter(|centre| *centre >= lowest)
        .flat_map(|centre| edges.iter().map(move |edge| at(*edge, centre)))
        .find(|rect| near.free(*rect))
}

/// Each of `texts` that fits `room` on screen, in order; the last always.
fn fitting(
    painter: &Painter,
    texts: &[String],
    font: &FontId,
    room: f32,
) -> Vec<(String, Arc<Galley>)> {
    let last = texts.len().saturating_sub(1);
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let galley = painter.layout_no_wrap(text.clone(), font.clone(), theme::MUTED);
            (index, text, galley)
        })
        .filter(|(index, _, galley)| *index == last || galley.size().x + 8.0 <= room)
        .map(|(_, text, galley)| (text.clone(), galley))
        .collect()
}

/// A chip or a count laid in `rect`, with its stub to `socket`: straight
/// where it is level with it, else bent down or up at `elbow` across.
fn stubbed(
    (shown, galley): (String, Arc<Galley>),
    rect: Rect,
    socket: Pos2,
    elbow: f32,
    font: f32,
) -> Laid {
    let centre = rect.center().y;
    let edge = if elbow < socket.x {
        rect.right()
    } else {
        rect.left()
    };
    let (stub, bend) = if (centre - socket.y).abs() < 0.5 {
        ([pos2(edge, socket.y), socket], None)
    } else {
        (
            [pos2(edge, centre), pos2(elbow, centre)],
            Some(pos2(elbow, socket.y)),
        )
    };
    Laid {
        galley,
        shown,
        rect,
        font,
        stub,
        bend,
        socket,
    }
}

/// The order chips or counts are laid in: by column, then down the board.
fn down_columns(anchors: impl Iterator<Item = Pos2>) -> Vec<usize> {
    let anchors: Vec<Pos2> = anchors.collect();
    let mut order: Vec<usize> = (0..anchors.len()).collect();
    order.sort_by(|a, b| {
        let (a, b) = (anchors[*a], anchors[*b]);
        a.x.round()
            .total_cmp(&b.x.round())
            .then(a.y.total_cmp(&b.y))
    });
    order
}

/// The chips of `tags` left of their sockets, in the gap between two
/// columns; their text keeps a legible size at any zoom and shortens through
/// its forms (the source step, a variable's badge number, a list element's
/// index), then to the arrow, where the gap has no room for it. The chips
/// of one column stand one under the other, [`CLEAR`] apart, and keep that
/// room from the cards and the lines in `clear` (LAY-37): a chip that would
/// come nearer moves up or down, its stub bent to its socket, else it
/// shortens.
pub fn lay_tags(painter: &Painter, tags: &[Tag], camera: Camera, clear: &Clear<'_>) -> Vec<Laid> {
    let font = paint::min_on_screen(TEXT, camera.zoom, paint::LINE_MIN);
    let font_id = FontId::proportional(font);
    let stub = stub_length(camera.zoom);
    let room = GAP * camera.zoom - stub - 2.0;
    let mut laid: Vec<Option<Laid>> = vec![None; tags.len()];
    let mut placed: Vec<Rect> = Vec::with_capacity(tags.len());
    let mut below: Option<(f32, f32)> = None;
    for index in down_columns(tags.iter().map(|tag| camera.to_screen(tag.anchor))) {
        let tag = &tags[index];
        let anchor = camera.to_screen(tag.anchor);
        let forms: Vec<String> = tag
            .forms
            .iter()
            .map(|form| format!("← {form}"))
            .chain(std::iter::once("←".to_owned()))
            .collect();
        let texts = fitting(painter, &forms, &font_id, room);
        // Under the chip above it in its column.
        let lowest = match below {
            Some((x, bottom)) if (x - anchor.x).abs() < 1.0 => bottom + CLEAR,
            _ => f32::MIN,
        };
        // Every place the search below may try, with its room: beyond a
        // few lines down a widened gap, and its reach up or down.
        let tall = texts[0].1.size().y + 3.0;
        let reach = tall * (REACH + 1.0) + CLEAR;
        let start = anchor.y.max(lowest + tall / 2.0);
        let band = Rect::from_min_max(
            pos2(
                anchor.x - stub - 4.0 * (room + CLEAR),
                start.min(anchor.y) - reach,
            ),
            pos2(anchor.x, start + reach),
        );
        let near = Near::of(band, clear, &placed);
        // Where lines run down the gap past the socket, the chip may stand
        // left of any of them, its stub across them: level with its socket
        // first, then nearest it.
        let mut rights: Vec<f32> = near
            .segments
            .iter()
            .filter(|[a, b]| (a.x - b.x).abs() < 0.5 && a.x < anchor.x - stub + CLEAR)
            .filter(|[a, b]| a.y.min(b.y) < anchor.y + tall && a.y.max(b.y) > anchor.y - tall)
            .map(|[a, _]| a.x - CLEAR)
            .collect();
        rights.sort_by(|a, b| b.total_cmp(a));
        rights.insert(0, anchor.x - stub);
        let at = |size: Vec2| {
            move |right: f32, centre: f32| {
                Rect::from_min_size(pos2(right - size.x, centre - size.y / 2.0), size)
            }
        };
        let chosen = texts.iter().find_map(|(shown, galley)| {
            let size = galley.size() + vec2(8.0, 3.0);
            let rect = spot(at(size), &rights, anchor.y, lowest + size.y / 2.0, &near)?;
            Some(((shown.clone(), galley.clone()), rect))
        });
        // Nowhere clear within reach: under the chip above, as before.
        let (text, rect) = chosen.unwrap_or_else(|| {
            let (shown, galley) = texts[0].clone();
            let size = galley.size() + vec2(8.0, 3.0);
            let centre = anchor.y.max(lowest + size.y / 2.0);
            ((shown, galley), at(size)(anchor.x - stub, centre))
        });
        placed.push(rect);
        below = Some((anchor.x, rect.bottom()));
        laid[index] = Some(stubbed(text, rect, anchor, rect.right() + stub / 2.0, font));
    }
    laid.into_iter().flatten().collect()
}

/// The counts at the sources' ports, right of them in the gap, laid as the
/// chips are: [`CLEAR`] from the cards, the marks and the lines in `clear`,
/// from the chips in `tags` and from each other (LAY-37). A count that would
/// come nearer moves up or down, its stub bent to its port, else it
/// shortens.
pub fn lay_counts(
    painter: &Painter,
    counts: &[&Count],
    camera: Camera,
    clear: &Clear<'_>,
    tags: &[Laid],
) -> Vec<Laid> {
    let font = paint::min_on_screen(TEXT, camera.zoom, paint::LINE_MIN);
    let font_id = FontId::proportional(font);
    let stub = stub_length(camera.zoom);
    let room = GAP * camera.zoom - stub - 2.0;
    let mut laid: Vec<Option<Laid>> = vec![None; counts.len()];
    let mut placed: Vec<Rect> = tags.iter().map(|tag| tag.rect).collect();
    for index in down_columns(counts.iter().map(|count| camera.to_screen(count.anchor))) {
        let count = counts[index];
        let anchor = camera.to_screen(count.anchor);
        let texts = fitting(
            painter,
            &[
                format!("→ {} {}", count.readers, noun(count.readers)),
                format!("→ {}", count.readers),
                "→".to_owned(),
            ],
            &font_id,
            room,
        );
        // Every place the search below may try, with its room.
        let tall = texts[0].1.size().y + 3.0;
        let reach = tall * (REACH + 1.0) + CLEAR;
        let band = Rect::from_min_max(
            pos2(anchor.x, anchor.y - reach),
            pos2(anchor.x + stub + 4.0 * (room + CLEAR), anchor.y + reach),
        );
        let near = Near::of(band, clear, &placed);
        // Where lines run down the gap past the port, the count may stand
        // right of any of them, its stub across them: level with its port
        // first, then nearest it.
        let mut lefts: Vec<f32> = near
            .segments
            .iter()
            .filter(|[a, b]| (a.x - b.x).abs() < 0.5 && a.x > anchor.x + stub - CLEAR)
            .filter(|[a, b]| a.y.min(b.y) < anchor.y + tall && a.y.max(b.y) > anchor.y - tall)
            .map(|[a, _]| a.x + CLEAR)
            .collect();
        lefts.sort_by(f32::total_cmp);
        lefts.insert(0, anchor.x + stub);
        let at = |size: Vec2| {
            move |left: f32, centre: f32| {
                Rect::from_min_size(pos2(left, centre - size.y / 2.0), size)
            }
        };
        let chosen = texts.iter().find_map(|(shown, galley)| {
            let size = galley.size() + vec2(8.0, 3.0);
            let rect = spot(at(size), &lefts, anchor.y, f32::MIN, &near)?;
            Some(((shown.clone(), galley.clone()), rect))
        });
        let (text, rect) = chosen.unwrap_or_else(|| {
            let (shown, galley) = texts[0].clone();
            let size = galley.size() + vec2(8.0, 3.0);
            ((shown, galley), at(size)(anchor.x + stub, anchor.y))
        });
        placed.push(rect);
        laid[index] = Some(stubbed(text, rect, anchor, rect.left() - stub / 2.0, font));
    }
    laid.into_iter().flatten().collect()
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
    )
    .swap_remove(0);
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

/// What a kept statement offers at the end of a line whose field the
/// Inspector can read from another step (LAY-42).
const CHANGE_SOURCE: &str = "Change source";

/// The side of the icon before [`CHANGE_SOURCE`].
const ACTION_ICON: f32 = 11.0;

/// The width *Change source* adds to a statement's box: a divider, the
/// icon, the words and their margins.
fn action_width(painter: &Painter) -> f32 {
    let words = painter.layout_no_wrap(
        CHANGE_SOURCE.to_owned(),
        FontId::proportional(theme::FONT_META),
        theme::ACCENT,
    );
    6.0 + 5.0 + ACTION_ICON + 4.0 + words.size().x + 8.0
}

/// The size of the box of a statement `text`, with *Change source* at the
/// end of its lines when `actions` (LAY-42).
pub fn statement_size(painter: &Painter, text: &str, actions: bool) -> Vec2 {
    let size = painter
        .layout_no_wrap(
            text.to_owned(),
            FontId::proportional(theme::FONT_META),
            theme::TEXT,
        )
        .size()
        + vec2(12.0, 6.0);
    if actions {
        size + vec2(action_width(painter), 0.0)
    } else {
        size
    }
}

/// Paints a kept statement as [`paint_statement`] does, each line marked in
/// `actions` ending with *Change source* (LAY-42). Returns the box and, by
/// line, the button's rect.
pub fn paint_statement_actions(
    painter: &Painter,
    near: Rect,
    text: &str,
    actions: &[bool],
    within: Rect,
) -> (Rect, Vec<Option<Rect>>) {
    let font = FontId::proportional(theme::FONT_META);
    let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), theme::TEXT);
    let text_width = galley.size().x;
    let size = statement_size(painter, text, true);
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
    let divider = rect.left() + 6.0 + text_width + 6.0;
    painter.line_segment(
        [
            pos2(divider, rect.top() + 3.0),
            pos2(divider, rect.bottom() - 3.0),
        ],
        Stroke::new(1.0, theme::BORDER),
    );
    let lines = actions.len().max(1);
    let row = (size.y - 6.0) / lines as f32;
    let buttons = actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            if !*action {
                return None;
            }
            let top = if index == 0 {
                rect.top() + 1.0
            } else {
                rect.top() + 3.0 + row * index as f32
            };
            let bottom = if index + 1 == lines {
                rect.bottom() - 1.0
            } else {
                rect.top() + 3.0 + row * (index + 1) as f32
            };
            let button =
                Rect::from_min_max(pos2(divider + 1.0, top), pos2(rect.right() - 1.0, bottom));
            painter.rect_filled(button, 3.0, theme::ACCENT_SOFT);
            let icon = Rect::from_min_size(
                pos2(divider + 5.0, button.center().y - ACTION_ICON / 2.0),
                Vec2::splat(ACTION_ICON),
            );
            crate::icons::Icon::Wire.paint(painter, icon, theme::ACCENT);
            let words =
                painter.layout_no_wrap(CHANGE_SOURCE.to_owned(), font.clone(), theme::ACCENT);
            let y = button.center().y - words.size().y / 2.0;
            painter.galley(pos2(icon.right() + 4.0, y), words, theme::ACCENT);
            Some(button)
        })
        .collect();
    (rect, buttons)
}

#[cfg(test)]
mod tests {
    //! LAY-37 review findings: a chip or a count placed anywhere its search
    //! reaches keeps [`CLEAR`] from everything in `clear`, also beyond the
    //! band it was first looked for in; only the fallback place, where no
    //! clear place was found, may come nearer.
    use super::*;
    use eframe::egui::{Context, LayerId, RawInput, Vec2};

    const SOCKET: Pos2 = pos2(1000.0, 500.0);

    fn camera() -> Camera {
        Camera {
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }

    fn with_painter<T>(run: impl FnOnce(&Painter) -> T) -> T {
        let context = Context::default();
        let mut out = None;
        let mut run = Some(run);
        let mut output = context.run_ui(RawInput::default(), |ui| {
            if let Some(run) = run.take() {
                out = Some(run(&ui.ctx().layer_painter(LayerId::background())));
            }
        });
        output.textures_delta.clear();
        out.expect("a frame")
    }

    fn tag(anchor: Pos2) -> Tag {
        Tag {
            reader: "reader".into(),
            card: 0,
            source: "cart".into(),
            port: "body".into(),
            input: "query".into(),
            path: Vec::new(),
            text: "cart.id".into(),
            forms: vec!["cart.id".into(), "cart".into()],
            statement: "cart.id → reader.query".into(),
            wires: vec!["w".into()],
            anchor,
            slot: 0,
        }
    }

    fn count(anchor: Pos2) -> Count {
        Count {
            source: "source".into(),
            card: 0,
            port: "body".into(),
            readers: 3,
            label: "source.body → 3 readers".into(),
            wires: vec!["w".into()],
            anchor,
        }
    }

    /// The distance between a box and a segment, 0 where they meet.
    fn apart(rect: Rect, [a, b]: [Pos2; 2]) -> f32 {
        if crosses(rect, a, b) {
            return 0.0;
        }
        (0..=100)
            .map(|step| a.lerp(b, step as f32 / 100.0))
            .map(|at| rect.distance_to_pos(at))
            .fold(f32::INFINITY, f32::min)
    }

    /// Panics where `laid` comes nearer than [`CLEAR`] to something in
    /// `clear` without standing at `fallback`, the place it takes when no
    /// clear place is found.
    fn assert_clear(laid: &Laid, clear: &Clear<'_>, fallback: Rect, context: &str) {
        let at_fallback = (laid.rect.center() - fallback.center()).length() < 0.5;
        if at_fallback {
            return;
        }
        let near = CLEAR - 0.5 - 0.01;
        for other in clear.boxes {
            let gap = laid.rect.distance_to_pos(other.clamp(laid.rect.center()));
            let gap = gap.min(apart(*other, [laid.rect.left_top(), laid.rect.right_top()]));
            let gap = gap.min(apart(
                *other,
                [laid.rect.left_bottom(), laid.rect.right_bottom()],
            ));
            assert!(
                gap >= near,
                "{context}: {:?} is {gap} px from the box {other:?}",
                laid.rect
            );
        }
        for line in clear.lines {
            for pair in line.windows(2) {
                let gap = apart(laid.rect, [pair[0], pair[1]]);
                assert!(
                    gap >= near,
                    "{context}: {:?} is {gap} px from the line {pair:?}",
                    laid.rect
                );
            }
        }
    }

    /// Vertical lines down the gap past `SOCKET`, each just where the chip
    /// stood beside the one before, until the chip would stand beyond the
    /// band it was looked for in; then a line just past that place.
    fn walls(width: f32, side: f32) -> Vec<Vec<Pos2>> {
        let stub = stub_length(1.0);
        let room = GAP - stub - 2.0;
        let band = stub + 2.0 * (room + CLEAR);
        let line = |x: f32| vec![pos2(x, SOCKET.y - 300.0), pos2(x, SOCKET.y + 300.0)];
        let mut lines = Vec::new();
        // The near edge of the chip as it stands, its far edge from the
        // socket.
        let mut edge = stub;
        while edge + width < band {
            let x = edge + width / 2.0;
            lines.push(line(SOCKET.x + side * x));
            edge = x + CLEAR;
        }
        // Beyond the band: just past the far edge of the last place.
        lines.push(line(SOCKET.x + side * (edge + width + 1.0)));
        lines
    }

    /// Finding 1: a chip set left of lines that hug its card, beyond the
    /// band, keeps clear of a line there.
    #[test]
    fn a_chip_beyond_the_band_keeps_clear_of_a_line_there() {
        with_painter(|painter| {
            let tags = [tag(SOCKET)];
            let alone = lay_tags(
                painter,
                &tags,
                camera(),
                &Clear {
                    boxes: &[],
                    lines: &[],
                },
            );
            let fallback = alone[0].rect;
            let lines = walls(fallback.width(), -1.0);
            let clear = Clear {
                boxes: &[],
                lines: &lines,
            };
            let laid = lay_tags(painter, &tags, camera(), &clear);
            assert_clear(&laid[0], &clear, fallback, "a chip");
        });
    }

    /// Finding 1, for a count set right of lines that leave its port.
    #[test]
    fn a_count_beyond_the_band_keeps_clear_of_a_line_there() {
        with_painter(|painter| {
            let source = count(SOCKET);
            let counts = [&source];
            let none = Clear {
                boxes: &[],
                lines: &[],
            };
            let alone = lay_counts(painter, &counts, camera(), &none, &[]);
            let fallback = alone[0].rect;
            let lines = walls(fallback.width(), 1.0);
            let clear = Clear {
                boxes: &[],
                lines: &lines,
            };
            let laid = lay_counts(painter, &counts, camera(), &clear, &[]);
            assert_clear(&laid[0], &clear, fallback, "a count");
        });
    }

    /// LAY-39 criterion 1: a read through a list element offers the step,
    /// the last index and the field after it; then the step and the index;
    /// then the index alone.
    #[test]
    fn lay39_a_read_through_an_element_keeps_its_index() {
        assert_eq!(
            step_forms("rooms", "body", true, "items[1].id"),
            ["rooms…[1].id", "rooms…[1]", "…[1]"]
        );
        assert_eq!(
            step_forms("rooms", "body", true, "items[0].id"),
            ["rooms…[0].id", "rooms…[0]", "…[0]"]
        );
    }

    /// LAY-39 criterion 2: a path with no index gives today's forms and
    /// nothing else; a variable gives its name and its badge number; a path
    /// ending at an element gives `x…[n]`, then `…[n]`; with two indices the
    /// last is kept; nothing is left out, no `…` is written.
    #[test]
    fn lay39_the_forms_of_other_reads() {
        assert_eq!(
            step_forms("slot", "", false, "start"),
            ["slot.start", "slot"]
        );
        assert_eq!(
            step_forms("login", "body", true, "token"),
            ["login.token", "login"]
        );
        // No field: the port names it, as before.
        assert_eq!(
            step_forms("login", "status", true, ""),
            ["login.status", "login"]
        );
        assert_eq!(variable_forms("cartId", Some(7)), ["cartId", "7"]);
        assert_eq!(variable_forms("cartId", None), ["cartId"]);
        assert_eq!(
            step_forms("rooms", "body", true, "items[1]"),
            ["rooms…[1]", "…[1]"]
        );
        assert_eq!(
            step_forms("a", "body", true, "rows[2].cells[0].value"),
            ["a…[0].value", "a…[0]", "…[0]"]
        );
        assert_eq!(
            step_forms("list", "", false, "[3].name"),
            ["list[3].name", "list[3]", "…[3]"]
        );
        // A quoted key is not an index.
        assert_eq!(
            step_forms("map", "body", true, r#"["[2]"].id"#),
            ["map.id", "map"]
        );
    }

    /// LAY-39 criterion 3: the statement is the reference written whole,
    /// whatever the chip says.
    #[test]
    fn lay39_the_statement_is_the_reference_whole() {
        assert_eq!(
            statement("rooms", "body", "items[1].id", "second.body.roomId"),
            "rooms.body.items[1].id → second.body.roomId"
        );
        assert_eq!(
            statement("slot", "", "start", "second.body.start"),
            "slot.start → second.body.start"
        );
    }

    /// Finding 2: a chip pushed down by the chip above it to the end of its
    /// reach keeps clear of a card just below there.
    #[test]
    fn a_chip_at_the_end_of_its_reach_keeps_clear_of_what_is_below() {
        with_painter(|painter| {
            let tags = [tag(SOCKET), tag(SOCKET + vec2(0.0, 1.0))];
            let none = Clear {
                boxes: &[],
                lines: &[],
            };
            let stacked = lay_tags(painter, &tags, camera(), &none);
            let (above, fallback) = (stacked[0].rect, stacked[1].rect);
            let height = fallback.height();
            // Where the second chip may stand: from under the first to its
            // reach below that.
            let start = fallback.center().y;
            let last = start + (REACH * height / NUDGE).ceil() * NUDGE;
            let lane = (SOCKET.x - GAP, SOCKET.x - stub_length(1.0) + 1.0);
            // A card over every place but the last, clear of the first chip,
            // and one 2 px under the last.
            let blocked = Rect::from_min_max(
                pos2(lane.0, above.bottom() + CLEAR),
                pos2(lane.1, last - height / 2.0 - CLEAR),
            );
            let below = Rect::from_min_max(
                pos2(lane.0, last + height / 2.0 + 2.0),
                pos2(lane.1, last + height / 2.0 + 40.0),
            );
            let boxes = [blocked, below];
            let clear = Clear {
                boxes: &boxes,
                lines: &[],
            };
            let laid = lay_tags(painter, &tags, camera(), &clear);
            assert_clear(&laid[1], &clear, fallback, "the second chip");
        });
    }
}
