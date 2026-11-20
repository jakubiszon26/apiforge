//! The lines a board draws at rest (LAY-03): what Tidy up makes room for.
//!
//! Tidy up widens the gaps between cards until the lines of the board keep
//! their distance from each other. Which lines those are depends on what the
//! window shows: checks folded into their subjects are not drawn and neither
//! are their wires (LAY-11), and a value read by three or more steps is a
//! chip at each far reader instead of a wire (LAY-23); the chain of steps is
//! drawn in both modes (LAY-45). [`resting`] gives the
//! router's input for a graph as the board draws it with nothing selected,
//! pointed at or dragged, from the same parts the board itself is drawn from,
//! and the chips and counts that stand in the gaps, which Tidy up makes room
//! for too (LAY-37).

use super::layout::{Card, CardDensity, Chip};
use super::readers::{self, WireMode};
use super::routing::{self, WireStyle};
use super::{collapsible, folded_wires};
use apiforge_core::format::{Graph, Project};
use apiforge_layout as layout;
use eframe::egui;
use std::collections::{HashMap, HashSet};

/// Which lines a board draws: the *Wires* setting and *Collapse checks*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lines {
    pub wires: WireMode,
    /// Whether the checks are folded into their subjects.
    pub folded: bool,
}

impl Default for Lines {
    /// Every wire and every check: the most a board can draw.
    fn default() -> Self {
        Self {
            wires: WireMode::All,
            folded: false,
        }
    }
}

/// The board of one graph, for [`resting`].
pub struct Drawn<'a> {
    pub graph: &'a Graph,
    /// Nodes that bindings may reference.
    pub reference: &'a Graph,
    /// The graph that runs after this one, as [`collapsible`] takes it.
    pub later: Option<&'a Graph>,
    pub project: &'a Project,
    pub parameters: &'a [String],
    pub density: CardDensity,
    pub lines: Lines,
}

/// The room a count or a chip takes beside its card, across a gap between
/// two columns: its text shortens to fit it at any zoom (LAY-23).
pub const LANE: f32 = readers::GAP;

/// A chip at a reader's input or a count at a source's port on a board at
/// rest (LAY-23): what stands in the gap left or right of a card, which
/// Tidy up makes room for (LAY-37).
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    /// The step it stands beside, by node id.
    pub node: String,
    /// The socket or the port, on the board.
    pub anchor: egui::Pos2,
    /// A chip, left of the card; else a count, right of it.
    pub input: bool,
}

/// What the router is given for the board at rest: the rectangles of the
/// cards drawn, then of the proxy chips, and the wires as ports on them; and
/// the chips and counts the board draws instead of some wires.
pub fn resting(board: &Drawn<'_>) -> ((Vec<layout::Rect>, Vec<layout::Wire>), Vec<Lane>) {
    let rest = at_rest(board);
    let lanes = rest
        .tags
        .iter()
        .map(|tag| Lane {
            node: tag.reader.clone(),
            anchor: tag.anchor,
            input: true,
        })
        .chain(rest.counts.iter().map(|count| Lane {
            node: count.source.clone(),
            anchor: count.anchor,
            input: false,
        }))
        .collect();
    (geometry(&rest), lanes)
}

fn geometry(rest: &Rest) -> (Vec<layout::Rect>, Vec<layout::Wire>) {
    routing::geometry(&routing::Request {
        cards: &rest.cards,
        chips: &rest.chips,
        wires: &rest.wires,
        moving: &HashSet::new(),
        style: WireStyle::Orthogonal,
        bridges: false,
        among: &[],
    })
}

/// A board at rest: its cards and proxy chips, the wires drawn, and the
/// chips and counts that stand for the others.
struct Rest {
    cards: Vec<Card>,
    chips: Vec<Chip>,
    wires: Vec<routing::BoardWire>,
    tags: Vec<readers::Tag>,
    counts: Vec<readers::Count>,
}

fn at_rest(board: &Drawn<'_>) -> Rest {
    let graph = board.graph;
    let mut cards: Vec<Card> = graph
        .nodes
        .iter()
        .map(|node| Card::build(node, board.reference, board.project, board.density))
        .collect();
    let chips: Vec<Chip> = graph
        .proxies
        .iter()
        .map(|proxy| Chip::build(proxy, graph, board.project, board.parameters))
        .collect();
    let mut folded: HashMap<String, (String, String)> = HashMap::new();
    if board.lines.folded {
        let name = |id: &str| {
            cards
                .iter()
                .find(|card| card.id == id)
                .map(|card| &card.name)
        };
        for candidate in collapsible(graph, board.later) {
            if let (Some(check), Some(_)) = (name(&candidate.check), name(&candidate.anchor)) {
                folded.insert(candidate.check, (candidate.anchor, check.clone()));
            }
        }
        cards.retain(|card| !folded.contains_key(&card.id));
    }
    let (mut wires, _) = folded_wires(graph, &cards, &chips, &folded);
    let (mut tags, mut counts) = (Vec::new(), Vec::new());
    if board.lines.wires == WireMode::Chips {
        let chipped = readers::chipped(graph, &cards, &wires);
        if !chipped.is_empty() {
            (tags, counts) = readers::plan(graph, board.project, &cards, &wires, &chipped);
            let tagged: HashSet<&str> = tags
                .iter()
                .flat_map(|tag| tag.wires.iter().map(String::as_str))
                .collect();
            wires.retain(|wire| !tagged.contains(wire.id.as_str()));
        }
    }
    Rest {
        cards,
        chips,
        wires,
        tags,
        counts,
    }
}
