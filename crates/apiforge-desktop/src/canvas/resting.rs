//! The lines a board draws at rest (LAY-03): what Tidy up makes room for.
//!
//! Tidy up widens the gaps between cards until the lines of the board keep
//! their distance from each other. Which lines those are depends on what the
//! window shows: checks folded into their subjects are not drawn and neither
//! are their wires (LAY-11), a value read by three or more steps is a chip
//! at each far reader instead of a wire (LAY-23), and a rail into the next
//! step is an arrow at its badge (LAY-24). [`resting`] gives the
//! router's input for a graph as the board draws it with nothing selected,
//! pointed at or dragged, from the same parts the board itself is drawn from.

use super::layout::{Card, CardDensity, Chip};
use super::readers::{self, WireMode};
use super::routing::{self, WireStyle};
use super::{after, collapsible, folded_wires};
use apiforge_core::format::{Graph, Project};
use apiforge_layout as layout;
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

/// What the router is given for the board at rest: the rectangles of the
/// cards drawn, then of the proxy chips, and the wires as ports on them.
pub fn resting(board: &Drawn<'_>) -> (Vec<layout::Rect>, Vec<layout::Wire>) {
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
    if board.lines.wires == WireMode::Chips {
        // A rail into the next step is an arrow at the step's badge (LAY-24).
        let arrows: HashSet<String> = after::marks(graph, board.later, &cards, &wires)
            .into_iter()
            .flat_map(|mark| mark.wires)
            .collect();
        let chipped = readers::chipped(graph, &cards, &wires);
        if !chipped.is_empty() {
            let (tags, _) = readers::plan(graph, board.project, &cards, &wires, &chipped);
            let tagged: HashSet<&str> = tags
                .iter()
                .flat_map(|tag| tag.wires.iter().map(String::as_str))
                .collect();
            wires.retain(|wire| !tagged.contains(wire.id.as_str()));
        }
        wires.retain(|wire| !arrows.contains(&wire.id));
    }
    routing::geometry(&routing::Request {
        cards: &cards,
        chips: &chips,
        wires: &wires,
        moving: &HashSet::new(),
        style: WireStyle::Orthogonal,
        bridges: false,
        among: &[],
    })
}
