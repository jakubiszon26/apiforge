//! Shape D (LAY-06): what Tidy up makes of a whole board, and the marks the
//! board draws on it.
//!
//! The spine is the steps a person follows: *Start*, requests, component
//! calls and `if`s, in the order the engine reaches them, left to right in
//! rows that wrap after five columns. Everything else is a satellite: a
//! value, a check, a *Data condition* or an export hangs in the column of
//! the latest spine step it reads, stacked below it in the engine's order.
//! A check folds into the same subject LAY-11 folds it into.
//!
//! Like [`super::collapse`], this reads the document alone, so the same
//! board comes out folded or not.

use super::collapse::collapsible;
use super::layout::Card;
use apiforge_core::{
    format::Graph,
    graph::{check_subjects, dependency_index, sequence},
    model::NodeKind,
};
use eframe::egui::{Rect, pos2, vec2};
use std::collections::HashMap;

/// Whether a step of this kind stands on the spine.
pub fn on_spine(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Start | NodeKind::Request | NodeKind::Component | NodeKind::If
    )
}

/// The spine and the satellites of a board, by node index.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// The spine, in the engine's order.
    pub spine: Vec<usize>,
    /// Each satellite with the index in `spine` of the step it hangs under,
    /// in the engine's order.
    pub satellites: Vec<(usize, usize)>,
}

/// The plan of shape D for `graph`, or `None` for a board without a spine
/// step. `later` is as in [`collapsible`].
///
/// A satellite hangs under the latest spine step it reads, data or order:
/// a check under the subject LAY-11 folds it into, any other block under
/// the latest spine step among its dependencies, or else under the column
/// of the latest satellite it reads, or else under the spine step the
/// engine reaches just before it.
pub fn plan(graph: &Graph, later: Option<&Graph>) -> Option<Plan> {
    let order = sequence(graph);
    let index_of: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();
    let spine: Vec<usize> = order
        .iter()
        .filter_map(|id| index_of.get(id).copied())
        .filter(|index| on_spine(graph.nodes[*index].kind))
        .collect();
    if spine.is_empty() {
        return None;
    }
    let column: HashMap<&str, usize> = spine
        .iter()
        .enumerate()
        .map(|(column, index)| (graph.nodes[*index].id.as_str(), column))
        .collect();
    let folded: HashMap<String, String> = collapsible(graph, later)
        .into_iter()
        .map(|candidate| (candidate.check, candidate.anchor))
        .collect();
    let dependencies = dependency_index(graph);
    let mut hung: HashMap<&str, usize> = HashMap::new();
    let mut satellites = Vec::new();
    // The spine step the engine reached last, for a block that reads none.
    let mut before = 0;
    for id in &order {
        if let Some(at) = column.get(id) {
            before = *at;
            continue;
        }
        let Some(&index) = index_of.get(id) else {
            continue;
        };
        let node = &graph.nodes[index];
        let subject = folded
            .get(*id)
            .and_then(|anchor| column.get(anchor.as_str()).copied())
            .or_else(|| {
                check_subjects(graph, node)
                    .subjects
                    .iter()
                    .filter_map(|subject| column.get(subject.step).copied())
                    .max()
            });
        let reads = dependencies.get(id);
        let under = subject
            .or_else(|| {
                reads
                    .into_iter()
                    .flatten()
                    .filter_map(|source| column.get(source).copied())
                    .max()
            })
            .or_else(|| {
                reads
                    .into_iter()
                    .flatten()
                    .filter_map(|source| hung.get(source).copied())
                    .max()
            })
            .unwrap_or(before);
        hung.insert(id, under);
        satellites.push((index, under));
    }
    Some(Plan { spine, satellites })
}

/// The continuation marks of a board: each spine step at the end of a row
/// with the spine step the engine reaches next, which starts the row below.
/// A pair is a row break when the next step stands left of the first and
/// below it, which is how Tidy up wraps the spine; on any other board the
/// pairs still name the next step wherever it went. Cards not drawn, such
/// as folded checks, are not on the spine anyway.
pub fn continuations(graph: &Graph, cards: &[Card]) -> Vec<(String, String)> {
    let card = |id: &str| cards.iter().find(|card| card.id == id);
    let spine: Vec<&Card> = sequence(graph)
        .into_iter()
        .filter_map(card)
        .filter(|card| on_spine(card.kind))
        .collect();
    spine
        .windows(2)
        .filter(|pair| {
            let (last, next) = (pair[0].rect(), pair[1].rect());
            next.right() <= last.left() && next.top() >= last.bottom()
        })
        .map(|pair| (pair[0].id.clone(), pair[1].id.clone()))
        .collect()
}

/// The free space between the last card of a row and its continuation
/// mark, the mark's height and its widest.
const MARK_GAP: f32 = 8.0;
const MARK_HEIGHT: f32 = 24.0;
const MARK_WIDTH: f32 = 120.0;

/// A continuation mark (LAY-06): beside the last spine card of a row, the
/// name of the step the engine reaches next, which starts the row below.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// The node id of the card at the end of the row.
    pub last: String,
    /// The node id of the next step in order.
    pub next: String,
    /// Where the mark is, in board coordinates: right of the last card,
    /// level with its header, never wider than the gap the board leaves.
    pub rect: Rect,
    pub text: String,
}

/// The marks of a board, one per pair of [`continuations`].
pub fn marks(graph: &Graph, cards: &[Card]) -> Vec<Mark> {
    let card = |id: &str| cards.iter().find(|card| card.id == id);
    continuations(graph, cards)
        .into_iter()
        .filter_map(|(last, next)| {
            let at = card(&last)?.rect();
            let text = format!("↳ {}", card(&next)?.name);
            let width = (8.0 * text.chars().count() as f32 + 24.0).clamp(64.0, MARK_WIDTH);
            Some(Mark {
                rect: Rect::from_min_size(
                    pos2(at.right() + MARK_GAP, at.top() + 20.0),
                    vec2(width, MARK_HEIGHT),
                ),
                last,
                next,
                text,
            })
        })
        .collect()
}
