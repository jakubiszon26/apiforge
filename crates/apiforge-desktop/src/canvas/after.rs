//! The arrow into the next step (LAY-24): in *Chips for shared values* an
//! `@after` whose target is the next step of the spine is not drawn as a
//! dashed rail. A short dashed arrow points into the step's badge instead,
//! and pointing at it says what the step waits for: `after quantity_ok`.
//!
//! "The next step": the target stands on the spine and the rail comes from
//! the column just before it, from the step there or from a check hung
//! under it, folded or drawn as a card; a branch wire is never one. The
//! first step of a row is the next step of the last one of the row above.
//! The columns are those of shape D ([`shape::plan`]), which reads the
//! document alone, so a card moved by hand keeps its arrow.
//!
//! It is a rendering rule only, like the chips of LAY-23: nothing is saved,
//! the rails are simply not given to the router, and a rail brought back
//! by pointing or selecting is routed apart, so the wires already drawn
//! keep their routes.

use super::layout::Card;
use super::routing::{BoardWire, Start};
use super::shape;
use crate::theme;
use apiforge_core::format::Graph;
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use std::collections::HashMap;

/// The arrow's length on screen at 100 %, and its shortest at any zoom.
const LENGTH: f32 = 22.0;
const SHORTEST: f32 = 14.0;
/// The free space between the arrow's tip and the badge.
const GAP: f32 = 2.0;
/// The arrowhead: its length and half its height, on screen.
const HEAD: f32 = 6.0;
const HALF: f32 = 4.0;
/// The smallest hover area, on screen.
const HIT: f32 = 16.0;
/// Half a step's badge on screen at 100 % and its least (LAY-07): the
/// arrow ends that far left of the card's corner where nothing moves the
/// badge.
const BADGE_HALF: f32 = 10.0;
const BADGE_HALF_LEAST: f32 = 7.5;

/// The rails into one step that the board does not draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// The step they enter, as an index into the cards.
    pub target: usize,
    /// The rails, in the order the step's `@after` writes them.
    pub wires: Vec<String>,
    /// For each rail, the card it is drawn from and the step its `@after`
    /// names: the same step, or a check of it.
    pub sources: Vec<(String, String)>,
    /// What pointing at it shows: `after quantity_ok`.
    pub text: String,
    /// Its accessible name: `apply_coupon after quantity_ok`.
    pub name: String,
}

/// The marks of a board: the ordering wires among `wires` that enter the
/// next step of the spine, by the step they enter.
pub fn marks(
    graph: &Graph,
    later: Option<&Graph>,
    cards: &[Card],
    wires: &[BoardWire],
) -> Vec<Mark> {
    let Some(plan) = shape::plan(graph, later) else {
        return Vec::new();
    };
    let mut column: HashMap<&str, usize> = HashMap::new();
    for (at, index) in plan.spine.iter().enumerate() {
        column.insert(graph.nodes[*index].id.as_str(), at);
    }
    for (index, under) in &plan.satellites {
        column.insert(graph.nodes[*index].id.as_str(), *under);
    }
    let handle = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map_or_else(|| id.to_owned(), |node| node.handle.clone())
    };
    let mut marks: Vec<Mark> = Vec::new();
    // The document's order of the edges is the order `@after` writes them.
    for edge in &graph.edges {
        let Some(wire) = wires
            .iter()
            .find(|wire| wire.id == edge.id && wire.control && wire.branch.is_none())
        else {
            continue;
        };
        let Start::Output { card: source, .. } = &wire.start else {
            continue;
        };
        let (source, target) = (&cards[*source], &cards[wire.target]);
        if !shape::on_spine(target.kind) {
            continue;
        }
        let (Some(from), Some(to)) = (
            column.get(source.id.as_str()),
            column.get(target.id.as_str()),
        ) else {
            continue;
        };
        if *to != from + 1 {
            continue;
        }
        let at = match marks.iter().position(|mark| mark.target == wire.target) {
            Some(at) => at,
            None => {
                marks.push(Mark {
                    target: wire.target,
                    wires: Vec::new(),
                    sources: Vec::new(),
                    text: String::new(),
                    name: String::new(),
                });
                marks.len() - 1
            }
        };
        marks[at].wires.push(wire.id.clone());
        marks[at]
            .sources
            .push((source.id.clone(), edge.source.clone()));
    }
    for mark in &mut marks {
        let names: Vec<String> = mark
            .sources
            .iter()
            .map(|(_, named)| handle(named))
            .collect();
        mark.text = format!("after {}", names.join(", "));
        mark.name = format!("{} {}", handle(&cards[mark.target].id), mark.text);
    }
    marks
}

/// Where the arrows reach on the board at `zoom`, for Fit to show them
/// whole: left of each marked card's top-left corner, level with it or
/// risen over a card of the column before where the gap is narrower than
/// the arrow. The arrow keeps its size on screen, so the smaller the zoom
/// the farther it reaches on the board.
pub fn bounds(marks: &[Mark], cards: &[Card], zoom: f32) -> Vec<Rect> {
    let zoom = zoom.max(f32::EPSILON);
    let left = (BADGE_HALF * zoom).max(BADGE_HALF_LEAST) + GAP + (LENGTH * zoom).max(SHORTEST);
    let (above, below) = (2.0 * HALF + 2.0 * GAP, HALF + GAP);
    marks
        .iter()
        .map(|mark| {
            let corner = cards[mark.target].rect().left_top();
            Rect::from_min_max(
                corner - vec2(left, above) / zoom,
                corner + vec2(0.0, below / zoom),
            )
        })
        .collect()
}

/// An arrow laid out on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Laid {
    /// Its box: from the tail to the tip, as high as the arrowhead.
    pub rect: Rect,
    /// Where pointing at it shows its rail.
    pub hit: Rect,
}

impl Laid {
    fn tip(&self) -> Pos2 {
        self.rect.right_center()
    }
}

/// Lays out the arrow of a card whose badge is `badge` on screen: level
/// with the badge, ending just left of it. Where that box meets one of
/// `boxes` or a line of `lines` it rises, then sinks, a pixel at a time, as
/// the badge rises clear of a chip; with no clear place it stays level.
pub fn lay(
    card: Rect,
    badge: Option<Rect>,
    zoom: f32,
    boxes: &[Rect],
    lines: &[Vec<Pos2>],
) -> Laid {
    let length = (LENGTH * zoom).max(SHORTEST);
    let tip = match badge {
        Some(badge) => pos2(badge.left() - GAP, badge.center().y),
        None => pos2(card.left() - GAP, card.top()),
    };
    let at = |offset: f32| {
        let tip = tip + vec2(0.0, offset);
        let rect = Rect::from_min_max(
            pos2(tip.x - length, tip.y - HALF),
            pos2(tip.x, tip.y + HALF),
        );
        Laid {
            rect,
            hit: Rect::from_center_size(
                rect.center(),
                vec2(rect.width().max(HIT) + 2.0 * GAP, rect.height().max(HIT)),
            ),
        }
    };
    let clear = |laid: &Laid| {
        let rect = laid.rect.expand(0.5);
        boxes.iter().all(|other| !rect.intersects(*other))
            && lines.iter().all(|line| !crosses(line, rect))
    };
    let reach = (2.0 * HIT).max(24.0 * zoom);
    (0..=reach as usize)
        .flat_map(|step| [-(step as f32), step as f32])
        .map(at)
        .find(clear)
        .unwrap_or_else(|| at(0.0))
}

/// Whether the polyline `line` passes through `rect`.
pub fn crosses(line: &[Pos2], rect: Rect) -> bool {
    line.windows(2)
        .any(|pair| segment_crosses(pair[0], pair[1], rect))
}

/// Whether the segment from `a` to `b` meets `rect` (Liang–Barsky).
fn segment_crosses(a: Pos2, b: Pos2, rect: Rect) -> bool {
    let delta = b - a;
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for (p, q) in [
        (-delta.x, a.x - rect.left()),
        (delta.x, rect.right() - a.x),
        (-delta.y, a.y - rect.top()),
        (delta.y, rect.bottom() - a.y),
    ] {
        if p.abs() < f32::EPSILON {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                low = low.max(t);
            } else {
                high = high.min(t);
            }
            if low > high {
                return false;
            }
        }
    }
    true
}

/// The colour of an arrow: muted at rest, the accent while its rail is
/// drawn.
pub fn color(lit: bool) -> Color32 {
    if lit { theme::ACCENT } else { theme::MUTED }
}

/// Draws the arrow: a dashed line into an arrowhead at the badge.
pub fn paint(painter: &Painter, laid: &Laid, lit: bool) {
    let color = color(lit);
    let tip = laid.tip();
    let tail = pos2(laid.rect.left(), tip.y);
    let stroke = Stroke::new(1.6, color);
    let end = tip.x - HEAD;
    let (on, off) = (3.5, 2.5);
    let mut x = tail.x;
    while x < end {
        painter.line_segment([pos2(x, tip.y), pos2((x + on).min(end), tip.y)], stroke);
        x += on + off;
    }
    painter.add(Shape::convex_polygon(
        vec![tip, tip + vec2(-HEAD, -HALF), tip + vec2(-HEAD, HALF)],
        color,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_meets_a_box_it_passes_through() {
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        assert!(segment_crosses(pos2(-5.0, 5.0), pos2(15.0, 5.0), rect));
        assert!(segment_crosses(pos2(5.0, -5.0), pos2(5.0, 15.0), rect));
        assert!(!segment_crosses(pos2(-5.0, 12.0), pos2(15.0, 12.0), rect));
        assert!(!segment_crosses(pos2(12.0, -5.0), pos2(12.0, 15.0), rect));
        assert!(segment_crosses(pos2(2.0, 2.0), pos2(3.0, 3.0), rect));
    }

    #[test]
    fn the_arrow_rises_clear_of_a_chip() {
        let card = Rect::from_min_size(pos2(100.0, 100.0), vec2(240.0, 80.0));
        let badge = Rect::from_center_size(card.left_top(), vec2(20.0, 20.0));
        let level = lay(card, Some(badge), 1.0, &[], &[]);
        assert_eq!(level.rect.center().y, badge.center().y);
        assert!(level.rect.right() <= badge.left());
        assert!(level.rect.width() >= SHORTEST);
        assert!(level.hit.width() >= HIT && level.hit.height() >= HIT);
        let chip = Rect::from_min_max(pos2(60.0, 102.0), pos2(98.0, 116.0));
        let risen = lay(card, Some(badge), 1.0, &[chip], &[]);
        assert!(!risen.rect.intersects(chip));
        assert!(risen.rect.center().y < badge.center().y);
    }
}
