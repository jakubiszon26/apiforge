//! Shape D (LAY-06): what Tidy up makes of a whole board.
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

/// The group of each card (LAY-31), as [`apiforge_layout::groups`] tells it
/// from where the cards lie: a step and the cards hung under it, at most the
/// stack gap apart, as the router sees them.
pub fn groups(cards: &[Card]) -> Vec<usize> {
    let rects: Vec<apiforge_layout::Rect> = cards
        .iter()
        .map(|card| {
            let rect = card.rect();
            apiforge_layout::Rect::new(
                apiforge_layout::Point::new(rect.min.x, rect.min.y),
                apiforge_layout::Point::new(rect.max.x, rect.max.y),
            )
        })
        .collect();
    apiforge_layout::groups(&rects)
}

/// Whether the card `reader` hangs under the card `source` (LAY-31): in its
/// group, below it, and not a spine step. `groups` are [`groups`] of `cards`.
pub fn hung_under(cards: &[Card], groups: &[usize], source: usize, reader: usize) -> bool {
    let (Some(from), Some(to)) = (cards.get(source), cards.get(reader)) else {
        return false;
    };
    source != reader
        && groups.get(source).is_some()
        && groups.get(source) == groups.get(reader)
        && !on_spine(to.kind)
        && to.rect().top() >= from.rect().bottom()
}
