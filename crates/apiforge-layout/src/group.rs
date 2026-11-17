//! A step and the cards hung under it are one group (LAY-31).
//!
//! Tidy up hangs a step's checks, and the values and data conditions that
//! read it last, in a stack under it, [`STACK_GAP`] apart (LAY-06). Such a
//! stack is one group: lines of other cards go round it, never through its
//! gaps, and the lines of the group itself may run nearer its own cards than
//! the gap every line keeps (LAY-03), because a stack gap has no room for
//! that. The group is told by where the cards lie, so the rule is the same
//! for a tidied board and for one placed by hand: two cards one above the
//! other, at most the stack gap apart, are in one group.

use crate::geometry::{Point, Rect};
use crate::layered::STACK_GAP;
use crate::packed::{Breach, Near, packed};
use crate::route::Route;
use std::collections::BTreeMap;

/// What a measure forgives: rounding, not distance.
const TOLERANCE: f32 = 0.5;

/// The group of each card, as the index of its first card: cards one above
/// the other, sharing more than half of the narrower one's width, at most
/// [`STACK_GAP`] apart, are one group, and so is a chain of them. A card no
/// other card is stacked with is a group of its own.
pub fn groups(cards: &[Rect]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..cards.len()).collect();
    fn root(parent: &mut [usize], mut card: usize) -> usize {
        while parent[card] != card {
            parent[card] = parent[parent[card]];
            card = parent[card];
        }
        card
    }
    for upper in 0..cards.len() {
        for lower in 0..cards.len() {
            if upper != lower && stacked(&cards[upper], &cards[lower]) {
                let (a, b) = (root(&mut parent, upper), root(&mut parent, lower));
                let (low, high) = (a.min(b), a.max(b));
                parent[high] = low;
            }
        }
    }
    (0..cards.len())
        .map(|card| root(&mut parent, card))
        .collect()
}

/// Whether `lower` hangs under `upper`: below it, at most the stack gap
/// away, and sharing more than half of the narrower card's width.
fn stacked(upper: &Rect, lower: &Rect) -> bool {
    let shared = upper.max.x.min(lower.max.x) - upper.min.x.max(lower.min.x);
    let gap = lower.min.y - upper.max.y;
    shared > 0.5 * upper.width().min(lower.width())
        && (-TOLERANCE..=STACK_GAP + TOLERANCE).contains(&gap)
}

/// The groups of more than one card, by their first card: what they are
/// for the router and the measures.
#[derive(Clone, Debug, Default)]
pub(crate) struct Groups {
    /// Each card's group, `None` for a card alone.
    of: Vec<Option<usize>>,
    /// The box of each group: its cards and the gaps between them.
    boxes: BTreeMap<usize, Rect>,
}

impl Groups {
    /// `groups` as [`groups`] gives them for `cards`; anything else, an
    /// empty slice included, is read as every card alone.
    pub(crate) fn new(cards: &[Rect], groups: &[usize]) -> Self {
        if groups.len() != cards.len() {
            return Self::default();
        }
        let mut members: BTreeMap<usize, Vec<Rect>> = BTreeMap::new();
        for (card, group) in groups.iter().enumerate() {
            members.entry(*group).or_default().push(cards[card]);
        }
        members.retain(|_, cards| cards.len() > 1);
        let boxes = members
            .into_iter()
            .filter_map(|(group, cards)| Some((group, Rect::bounding(cards)?)))
            .collect::<BTreeMap<_, _>>();
        Self {
            of: groups
                .iter()
                .map(|group| boxes.contains_key(group).then_some(*group))
                .collect(),
            boxes,
        }
    }

    /// The group of a card, when it is in one with other cards.
    pub(crate) fn of(&self, card: usize) -> Option<usize> {
        self.of.get(card).copied().flatten()
    }

    /// The groups and their boxes.
    pub(crate) fn boxes(&self) -> impl Iterator<Item = (usize, Rect)> + '_ {
        self.boxes.iter().map(|(group, area)| (*group, *area))
    }

    fn area(&self, group: usize) -> Option<Rect> {
        self.boxes.get(&group).copied()
    }

    /// The groups of the cards a route's two ends lie on.
    fn ends(&self, route: &Route, cards: &[Rect]) -> Vec<usize> {
        let (Some(first), Some(last)) = (route.points.first(), route.points.last()) else {
            return Vec::new();
        };
        let on = |point: &Point| {
            cards
                .iter()
                .enumerate()
                .filter(|(_, card)| card.inflate(TOLERANCE).contains(*point))
                .filter_map(|(card, _)| self.of(card))
                .collect::<Vec<_>>()
        };
        let mut ends = on(first);
        ends.extend(on(last));
        ends.sort_unstable();
        ends.dedup();
        ends
    }
}

/// [`packed`] as the rule stands since LAY-31: a line keeps the gap from
/// every card and line outside its group, and inside the group it may not.
/// A route's group is that of a card one of its ends lies on, which is how
/// the router sees it too. Not counted: a breach beside a card of the
/// route's own group, and a breach between two lines of one group within
/// the gap around that group's box. `groups` are as [`groups`] gives them
/// for `cards`.
pub fn packed_outside_groups(
    routes: &[Route],
    cards: &[Rect],
    groups: &[usize],
    gap: f32,
) -> Vec<Breach> {
    let found = packed(routes, cards, gap);
    let known = Groups::new(cards, groups);
    if known.boxes.is_empty() {
        return found;
    }
    let ends: Vec<Vec<usize>> = routes
        .iter()
        .map(|route| known.ends(route, cards))
        .collect();
    found
        .into_iter()
        .filter(|breach| {
            let own = &ends[breach.route];
            match breach.near {
                Near::Card(card) => !known.of(card).is_some_and(|group| own.contains(&group)),
                Near::Route(other) => !ends[other]
                    .iter()
                    .filter(|group| own.contains(group))
                    .filter_map(|group| known.area(*group))
                    .any(|area| area.inflate(gap).contains(breach.area.center())),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    fn card(x: f32, y: f32, height: f32) -> Rect {
        Rect::from_size(Point::new(x, y), Size::new(280.0, height))
    }

    #[test]
    fn a_chain_of_hung_cards_is_one_group_named_by_its_first_card() {
        let cards = [
            card(0.0, 300.0, 70.0),
            card(0.0, 0.0, 100.0),
            card(0.0, 124.0, 70.0),
            card(0.0, 218.0, 58.0),
            card(400.0, 124.0, 70.0),
        ];
        assert_eq!(groups(&cards), [0, 0, 0, 0, 4]);
    }

    #[test]
    fn overlapping_or_distant_cards_are_not_a_group() {
        let cards = [
            card(0.0, 0.0, 100.0),
            card(0.0, 90.0, 100.0),
            card(0.0, 215.0, 100.0),
        ];
        assert_eq!(groups(&cards), [0, 1, 2]);
    }
}
