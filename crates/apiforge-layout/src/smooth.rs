//! The spline style: the same orthogonal route with rounded corners.
//!
//! Smoothing never changes where a wire goes, only how its corners look, so both
//! styles avoid cards equally well. A corner is rounded by at most half of each
//! adjacent segment, and a radius whose curve would reach into a card is halved
//! until it fits, down to the sharp corner the orthogonal style draws.
//!
//! A cubic curve stays inside the convex hull of its control points, so testing the
//! box around those four points is enough to know a corner is clear. The test is
//! conservative: it can shrink a radius that would in fact have fitted, which keeps
//! the guarantee of [ADR 0006](../../../docs/decisions/0006-layout-and-wire-routing.md)
//! on the safe side.

use crate::geometry::{Point, Rect};
use crate::route::Route;

/// How round a corner may be, in pixels.
pub const DEFAULT_RADIUS: f32 = 24.0;

/// Where a cubic control point sits for a corner that looks like a circular arc.
const ARC: f32 = 0.552_284_8;

/// One piece of a smoothed wire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    Line {
        from: Point,
        to: Point,
    },
    Curve {
        from: Point,
        control1: Point,
        control2: Point,
        to: Point,
    },
}

impl Piece {
    pub fn from(&self) -> Point {
        match self {
            Self::Line { from, .. } | Self::Curve { from, .. } => *from,
        }
    }

    pub fn to(&self) -> Point {
        match self {
            Self::Line { to, .. } | Self::Curve { to, .. } => *to,
        }
    }

    /// The box containing the piece: the segment itself, or the hull of a curve's
    /// control points, which contains the curve.
    pub fn bounds(&self) -> Rect {
        let points: &[Point] = match self {
            Self::Line { from, to } => &[*from, *to],
            Self::Curve {
                from,
                control1,
                control2,
                to,
            } => &[*from, *control1, *control2, *to],
        };
        Rect::bounding(points.iter().map(|point| Rect::new(*point, *point))).unwrap_or_default()
    }
}

/// Rounds the corners of a route, keeping every piece clear of `obstacles`.
///
/// `obstacles` are the cards the route must avoid — the caller leaves out the
/// route's own two cards, as the router does.
pub fn smooth(route: &Route, radius: f32, obstacles: &[Rect]) -> Vec<Piece> {
    let points = &route.points;
    if points.len() < 2 {
        return Vec::new();
    }
    let radius = if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        0.0
    };
    let mut pieces = Vec::with_capacity(points.len() * 2);
    // Where the straight part of each segment begins and ends once its corners have
    // taken their share; `cursor` walks along the route.
    let mut cursor = points[0];
    for index in 1..points.len() - 1 {
        let (previous, corner, next) = (points[index - 1], points[index], points[index + 1]);
        let incoming = corner.distance(previous);
        let outgoing = next.distance(corner);
        let mut corner_radius = radius.min(incoming * 0.5).min(outgoing * 0.5);
        // The curve must clear every card; a corner that cannot be rounded stays
        // sharp, which is what the orthogonal style draws anyway.
        let curve = loop {
            if corner_radius <= 0.5 {
                break None;
            }
            let candidate = rounded(previous, corner, next, corner_radius);
            if !obstacles
                .iter()
                .any(|obstacle| obstacle.overlaps(&candidate.bounds()))
            {
                break Some(candidate);
            }
            corner_radius *= 0.5;
        };
        match curve {
            Some(curve) => {
                if cursor.distance(curve.from()) > 0.01 {
                    pieces.push(Piece::Line {
                        from: cursor,
                        to: curve.from(),
                    });
                }
                cursor = curve.to();
                pieces.push(curve);
            }
            None => {
                if cursor.distance(corner) > 0.01 {
                    pieces.push(Piece::Line {
                        from: cursor,
                        to: corner,
                    });
                }
                cursor = corner;
            }
        }
    }
    let last = points[points.len() - 1];
    if cursor.distance(last) > 0.01 || pieces.is_empty() {
        pieces.push(Piece::Line {
            from: cursor,
            to: last,
        });
    }
    pieces
}

/// The rounded corner at `corner`, cutting `radius` off each adjacent segment.
fn rounded(previous: Point, corner: Point, next: Point, radius: f32) -> Piece {
    let along = |from: Point, to: Point, distance: f32| {
        let length = from.distance(to);
        if length <= f32::EPSILON {
            return from;
        }
        let ratio = (distance / length).clamp(0.0, 1.0);
        Point::new(
            from.x + (to.x - from.x) * ratio,
            from.y + (to.y - from.y) * ratio,
        )
    };
    let start = along(corner, previous, radius);
    let end = along(corner, next, radius);
    Piece::Curve {
        from: start,
        control1: along(start, corner, radius * ARC),
        control2: along(end, corner, radius * ARC),
        to: end,
    }
}

/// Cards a smoothed route reaches into although it should not.
///
/// The rule is the one the router follows: only the route's own two cards may be
/// touched, and a card holding a port may be touched by the first or last piece.
pub fn violations(pieces: &[Piece], obstacles: &[Rect]) -> Vec<usize> {
    let mut found = Vec::new();
    for (card, obstacle) in obstacles.iter().enumerate() {
        if pieces
            .iter()
            .any(|piece| obstacle.overlaps(&piece.bounds()))
        {
            found.push(card);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Port, Side, Size};
    use crate::route::{Routing, Wire, route};

    fn route_of(points: &[Point]) -> Route {
        Route {
            points: points.to_vec(),
            ..Route::default()
        }
    }

    #[test]
    fn a_straight_wire_stays_one_line() {
        let straight = route_of(&[Point::new(0.0, 0.0), Point::new(100.0, 0.0)]);
        assert_eq!(
            smooth(&straight, DEFAULT_RADIUS, &[]),
            [Piece::Line {
                from: Point::new(0.0, 0.0),
                to: Point::new(100.0, 0.0),
            }]
        );
    }

    #[test]
    fn a_corner_becomes_a_curve_between_two_lines() {
        let bent = route_of(&[
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
        ]);
        let pieces = smooth(&bent, DEFAULT_RADIUS, &[]);
        assert_eq!(pieces.len(), 3);
        assert!(matches!(pieces[0], Piece::Line { .. }));
        assert!(matches!(pieces[1], Piece::Curve { .. }));
        assert!(matches!(pieces[2], Piece::Line { .. }));
        // The pieces join without a gap and keep the route's ends.
        assert_eq!(pieces[0].from(), Point::new(0.0, 0.0));
        assert_eq!(pieces[2].to(), Point::new(100.0, 100.0));
        for pair in pieces.windows(2) {
            assert!(pair[0].to().distance(pair[1].from()) < 0.01);
        }
        // The curve cuts the radius off both segments.
        assert_eq!(pieces[1].from(), Point::new(100.0 - DEFAULT_RADIUS, 0.0));
        assert_eq!(pieces[1].to(), Point::new(100.0, DEFAULT_RADIUS));
    }

    #[test]
    fn a_short_segment_limits_the_radius_to_its_half() {
        let tight = route_of(&[
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 100.0),
        ]);
        let pieces = smooth(&tight, DEFAULT_RADIUS, &[]);
        let curve = pieces
            .iter()
            .find(|piece| matches!(piece, Piece::Curve { .. }))
            .unwrap();
        assert_eq!(curve.from(), Point::new(5.0, 0.0));
    }

    #[test]
    fn a_corner_against_a_card_shrinks_or_stays_sharp() {
        // A card right at the corner leaves no room for any radius.
        let bent = route_of(&[
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
        ]);
        let card = Rect::from_size(Point::new(100.5, -40.0), Size::new(80.0, 80.0));
        let pieces = smooth(&bent, DEFAULT_RADIUS, &[card]);
        assert!(violations(&pieces, &[card]).is_empty(), "{pieces:?}");
        let rounded = pieces
            .iter()
            .filter(|piece| matches!(piece, Piece::Curve { .. }))
            .count();
        assert!(rounded <= 1);
        assert_eq!(pieces.first().map(Piece::from), Some(Point::new(0.0, 0.0)));
        assert_eq!(pieces.last().map(Piece::to), Some(Point::new(100.0, 100.0)));
    }

    #[test]
    fn smoothing_a_real_route_keeps_it_off_the_cards() {
        let card = |x: f32, y: f32| Rect::from_size(Point::new(x, y), Size::new(280.0, 120.0));
        let cards = [card(0.0, 0.0), card(800.0, 0.0), card(400.0, -20.0)];
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(1, Side::Left, 60.0),
        )];
        let routing = Routing::default();
        let routes = route(&cards, &wires, &routing);
        let obstacles = [cards[2].inflate(routing.margin)];
        let pieces = smooth(&routes[0], DEFAULT_RADIUS, &obstacles);
        assert!(pieces.len() > 1);
        assert!(violations(&pieces, &obstacles).is_empty(), "{pieces:?}");
    }

    #[test]
    fn an_empty_route_smooths_to_nothing() {
        assert!(smooth(&Route::default(), DEFAULT_RADIUS, &[]).is_empty());
        assert!(smooth(&route_of(&[Point::new(1.0, 1.0)]), DEFAULT_RADIUS, &[]).is_empty());
    }

    #[test]
    fn a_zero_radius_leaves_the_polyline_alone() {
        let bent = route_of(&[
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
        ]);
        let pieces = smooth(&bent, 0.0, &[]);
        assert_eq!(
            pieces,
            [
                Piece::Line {
                    from: Point::new(0.0, 0.0),
                    to: Point::new(100.0, 0.0)
                },
                Piece::Line {
                    from: Point::new(100.0, 0.0),
                    to: Point::new(100.0, 100.0)
                },
            ]
        );
    }
}
