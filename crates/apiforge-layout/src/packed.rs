//! The gap every line keeps (LAY-03), as a measure.
//!
//! A board is easy to follow when no two lines run close together. [`packed`]
//! lists every place where a board breaks that: two parallel segments nearer
//! than the gap, side by side or end to end, and a segment nearer than the gap
//! to a card. The router keeps the gap ([`crate::route`]), Tidy up makes the
//! room for it, and the `quality` example and the tests count what is left.
//!
//! Two things are not breaches. The first and the last stretch of a line, as
//! long as the gap, are where its ports are: a line has to touch its own
//! card. And wires of one port share the straight piece out of that port,
//! which they leave one by one like the teeth of a comb; the same holds for
//! the piece into a port, and for a rail into a step and the rail out of it,
//! which use one port. Ports nearer each other than the gap, the operands of
//! a condition, count as one port in this: the straight pieces into them are
//! as near each other as the ports are, and part where they turn.

use crate::geometry::{Point, Rect};
use crate::route::Route;

/// The gap, in board pixels: between two lines, and between a line and a card.
pub const GAP: f32 = 20.0;

/// What a measure forgives: rounding, not distance.
const TOLERANCE: f32 = 0.5;

/// One place where the gap is not kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breach {
    /// The route whose segment it is.
    pub route: usize,
    /// What the segment is too near to.
    pub near: Near,
    /// Whether the segment runs horizontally.
    pub horizontal: bool,
    /// Where: the stretch the two share, between their lines; or the part of
    /// the segment within the gap of the card.
    pub area: Rect,
}

/// What a breaching segment is too near to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Near {
    /// A segment of another route, or of the same route folded on itself.
    Route(usize),
    /// A card or chip, as an index into the rectangles.
    Card(usize),
}

/// A segment of a route with its stretch at the ports taken off.
#[derive(Clone, Copy)]
struct Piece {
    route: usize,
    /// Its place among the route's segments.
    index: usize,
    horizontal: bool,
    /// Its line: `y` of a horizontal segment, `x` of a vertical one.
    line: f32,
    low: f32,
    high: f32,
    /// The route's first segment, with the port it leaves; its last, with the
    /// port it enters.
    leaves: Option<Point>,
    enters: Option<Point>,
}

/// The axis-aligned segments of the complete routes, port stretches removed.
fn pieces(routes: &[Route], gap: f32) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for (route, drawn) in routes.iter().enumerate() {
        let points = &drawn.points;
        if !drawn.complete || points.len() < 2 {
            continue;
        }
        let last = points.len() - 2;
        for (index, pair) in points.windows(2).enumerate() {
            let (from, to) = (pair[0], pair[1]);
            let horizontal = (from.y - to.y).abs() < 0.01;
            let vertical = (from.x - to.x).abs() < 0.01;
            if horizontal == vertical {
                // A point, or a line drawn directly between two ports.
                continue;
            }
            let along = |point: Point| if horizontal { point.x } else { point.y };
            let (mut start, mut end) = (along(from), along(to));
            let direction = (end - start).signum();
            let length = (end - start).abs();
            if index == 0 {
                start += direction * gap.min(length);
            }
            if index == last {
                end -= direction * gap.min((end - start).abs());
            }
            if (end - start) * direction <= 0.01 {
                continue;
            }
            pieces.push(Piece {
                route,
                index,
                horizontal,
                line: if horizontal { from.y } else { from.x },
                low: start.min(end),
                high: start.max(end),
                leaves: (index == 0).then_some(points[0]),
                enters: (index == last).then_some(points[points.len() - 1]),
            });
        }
    }
    pieces
}

fn area(horizontal: bool, along: (f32, f32), across: (f32, f32)) -> Rect {
    let (low, high) = (along.0.min(along.1), along.0.max(along.1));
    let (near, far) = (across.0.min(across.1), across.0.max(across.1));
    if horizontal {
        Rect::new(Point::new(low, near), Point::new(high, far))
    } else {
        Rect::new(Point::new(near, low), Point::new(far, high))
    }
}

/// Every place where `routes` do not keep `gap`: from each other and from
/// `cards`, the rectangles of the cards and chips of the board, the two a
/// route joins included. A route that found no way round the cards is not
/// judged.
pub fn packed(routes: &[Route], cards: &[Rect], gap: f32) -> Vec<Breach> {
    let pieces = pieces(routes, gap);
    let limit = gap - TOLERANCE;
    let mut breaches = Vec::new();
    let beside = |a: Option<Point>, b: Option<Point>| matches!((a, b), (Some(a), Some(b)) if a.distance(b) < limit);
    for (position, a) in pieces.iter().enumerate() {
        for b in &pieces[position + 1..] {
            if a.horizontal != b.horizontal {
                continue;
            }
            let across = (a.line - b.line).abs();
            // Negative while the two run side by side.
            let apart = a.low.max(b.low) - a.high.min(b.high);
            if across >= limit {
                continue;
            }
            if a.route == b.route {
                // A route folded on itself; a jog between two of its segments
                // is one line still.
                if apart > -TOLERANCE || a.index.abs_diff(b.index) < 2 {
                    continue;
                }
            } else {
                if apart >= limit {
                    continue;
                }
                // Wires of one port on the straight piece out of it, or into
                // it; and the pieces of two ports nearer than the gap, which
                // are as near each other as their ports.
                let ports = [
                    (a.leaves, b.leaves),
                    (a.enters, b.enters),
                    (a.leaves, b.enters),
                    (a.enters, b.leaves),
                ];
                if ports.into_iter().any(|(a, b)| beside(a, b)) {
                    continue;
                }
            }
            breaches.push(Breach {
                route: a.route,
                near: Near::Route(b.route),
                horizontal: a.horizontal,
                area: area(
                    a.horizontal,
                    (a.low.max(b.low), a.high.min(b.high)),
                    (a.line, b.line),
                ),
            });
        }
    }
    for piece in &pieces {
        for (card, rectangle) in cards.iter().enumerate() {
            let zone = rectangle.inflate(limit);
            let (near, far, low, high) = if piece.horizontal {
                (zone.min.y, zone.max.y, zone.min.x, zone.max.x)
            } else {
                (zone.min.x, zone.max.x, zone.min.y, zone.max.y)
            };
            if piece.line <= near || piece.line >= far {
                continue;
            }
            let (from, to) = (piece.low.max(low), piece.high.min(high));
            if to - from <= 0.01 {
                continue;
            }
            breaches.push(Breach {
                route: piece.route,
                near: Near::Card(card),
                horizontal: piece.horizontal,
                area: area(piece.horizontal, (from, to), (piece.line, piece.line)),
            });
        }
    }
    breaches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    fn line(points: &[(f32, f32)]) -> Route {
        Route {
            points: points.iter().map(|(x, y)| Point::new(*x, *y)).collect(),
            ..Route::default()
        }
    }

    fn count(routes: &[Route]) -> usize {
        packed(routes, &[], GAP).len()
    }

    #[test]
    fn two_lines_side_by_side_keep_the_gap() {
        let wire = line(&[(0.0, 0.0), (300.0, 0.0)]);
        let beside = |y: f32| line(&[(-50.0, y), (350.0, y)]);
        assert_eq!(count(&[wire.clone(), beside(20.0)]), 0);
        assert_eq!(count(&[wire.clone(), beside(19.0)]), 1);
        assert_eq!(count(&[wire.clone(), beside(0.0)]), 1);
        // At right angles two lines cross.
        assert_eq!(count(&[wire, line(&[(150.0, -90.0), (150.0, 90.0)])]), 0);
    }

    #[test]
    fn two_lines_end_to_end_keep_the_gap() {
        // Without the stretch at its ports the first line ends at 280, and
        // the second begins 20 px after its own port.
        let first = line(&[(0.0, 0.0), (300.0, 0.0)]);
        let second = |from: f32| line(&[(from, 0.0), (from + 300.0, 0.0)]);
        assert_eq!(count(&[first.clone(), second(270.0)]), 1);
        assert_eq!(count(&[first, second(280.0)]), 0);
    }

    #[test]
    fn the_stretch_at_a_port_is_not_judged() {
        // Two lines out of two cards part within their first 20 px; on one
        // track they would be end to end.
        let upper = line(&[(0.0, 0.0), (20.0, 0.0), (20.0, -100.0)]);
        let lower = line(&[(0.0, 30.0), (20.0, 30.0), (20.0, 130.0)]);
        assert_eq!(count(&[upper.clone(), lower]), 0);
        let lower = line(&[(0.0, 30.0), (20.0, 30.0), (20.0, 13.0), (90.0, 13.0)]);
        assert_eq!(count(&[upper, lower]), 1, "end to end on one track");
    }

    #[test]
    fn ports_nearer_than_the_gap_count_as_one() {
        // Two ports 8 px apart: the straight pieces out of them run 8 px
        // apart, and the lines part where they turn.
        let upper = line(&[(0.0, 0.0), (40.0, 0.0), (40.0, -100.0)]);
        let lower = line(&[(0.0, 8.0), (60.0, 8.0), (60.0, -100.0)]);
        assert_eq!(count(&[upper.clone(), lower]), 0);
        // After the turn they keep the gap like any two lines.
        let lower = line(&[(0.0, 8.0), (50.0, 8.0), (50.0, -100.0), (300.0, -100.0)]);
        assert_eq!(count(&[upper, lower]), 1);
    }

    #[test]
    fn wires_of_one_port_share_its_straight_piece_only() {
        let first = line(&[(0.0, 0.0), (20.0, 0.0), (20.0, -100.0)]);
        let second = line(&[(0.0, 0.0), (40.0, 0.0), (40.0, -100.0)]);
        let third = line(&[(0.0, 0.0), (300.0, 0.0)]);
        assert_eq!(count(&[first.clone(), second, third]), 0);
        // On one tooth they are one line again.
        let on_it = line(&[(0.0, 0.0), (20.0, 0.0), (20.0, -200.0)]);
        assert_eq!(count(&[first.clone(), on_it]), 1);
        // Another port's wire on the same piece is a breach.
        let other = line(&[(-50.0, 0.0), (350.0, 0.0)]);
        assert_eq!(count(&[line(&[(0.0, 0.0), (300.0, 0.0)]), other]), 1);
    }

    #[test]
    fn a_line_folded_on_itself_is_a_breach_and_a_jog_is_not() {
        let hairpin = line(&[(0.0, 0.0), (200.0, 0.0), (200.0, 8.0), (0.0, 8.0)]);
        assert_eq!(count(&[hairpin]), 1);
        let jog = line(&[(0.0, 0.0), (100.0, 0.0), (100.0, 8.0), (200.0, 8.0)]);
        assert_eq!(count(&[jog]), 0);
    }

    #[test]
    fn a_line_keeps_the_gap_from_every_card() {
        let card = Rect::from_size(Point::new(100.0, 0.0), Size::new(100.0, 100.0));
        let at = |y: f32| line(&[(0.0, y), (300.0, y)]);
        let count = |route: Route| packed(&[route], &[card], GAP).len();
        assert_eq!(count(at(120.0)), 0);
        assert_eq!(count(at(110.0)), 1);
        assert_eq!(count(at(-20.0)), 0);
        // Its own card, past the stretch at the port.
        assert_eq!(
            count(line(&[(200.0, 50.0), (220.0, 50.0), (220.0, 300.0)])),
            0
        );
        assert_eq!(
            count(line(&[(200.0, 50.0), (210.0, 50.0), (210.0, 300.0)])),
            1
        );
        // A route that found no way is not judged.
        let mut lost = at(50.0);
        lost.complete = false;
        assert_eq!(count(lost), 0);
    }
}
