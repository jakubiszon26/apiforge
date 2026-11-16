//! The order of lines in a corridor (LAY-28).
//!
//! The router gives each wire the nearest free track, in the order it routes
//! them, so lines that share a corridor lie in it in an order nobody chose,
//! and a line that leaves the others crosses the ones between it and its
//! way out. [`untangle`] puts that right after the routes are known: it
//! moves a piece of a line to another track of its corridor, or lets two
//! lines change tracks, whenever the board has fewer crossings for it.
//!
//! A move keeps everything the router promised. The piece stays between the
//! two pieces it joins, which only get longer or shorter, so a line keeps its
//! bends and its ports; no piece comes onto a card, nearer than the gap to a
//! card or to another line ([`crate::packed`]), or into the mouth of another
//! port. Each move takes at least one crossing off the board, so the pass
//! ends, and it tries the moves in the order of the wires, so it gives the
//! same board every time.

use crate::geometry::{Point, Rect};
use crate::packed::{Breach, involving};
use crate::route::{Mouth, Route, Wire, intersection};

/// How many tracks away a piece looks for a better place.
const TRACKS: i32 = 12;

/// How often the pass goes over the board at most. Every reference board is
/// done in three rounds.
const ROUNDS: usize = 8;

/// How many of a piece's moves are held to the rules before it is left
/// where it is: the best ones, by the crossings they save.
const TRIED: usize = 6;

/// The most moves one pass weighs. A readable board needs a small part of
/// it; a board of hundreds of wires wired at random stops here instead of
/// making the person wait.
const BUDGET: usize = 60_000;

/// The measures of the board the moves keep.
pub(crate) struct Rules<'a> {
    pub cards: &'a [Rect],
    pub mouths: &'a [Mouth],
    pub gap: f32,
    pub stub: f32,
}

/// Reorders the lines of `routes` in their corridors so that they cross
/// less. Only the first `wires.len()` routes move; the rest are lines that
/// stay as they are, which the others still count their crossings with.
pub(crate) fn untangle(routes: &mut [Route], wires: &[Wire], rules: &Rules<'_>) {
    if rules.gap < 1.0 || wires.len() < 2 {
        return;
    }
    let mut board = Board {
        bounds: routes.iter().map(|route| bounds(&route.points)).collect(),
        routes,
        wires,
        rules,
        weighed: 0,
        level: Vec::new(),
        upright: Vec::new(),
        corridors: vec![Vec::new(); wires.len()],
        sights: (0..wires.len()).map(|_| Vec::new()).collect(),
    };
    board.index();
    for route in 0..wires.len() {
        board.survey(route);
    }
    for _ in 0..ROUNDS {
        let mut moved = false;
        for route in 0..wires.len() {
            let mut piece = 1;
            // A piece between two others: the first and the last piece of a
            // line belong to its ports.
            while piece + 1 < board.pieces(route) {
                if board.weighed > BUDGET {
                    return;
                }
                moved |= board.improve(route, piece);
                piece += 1;
            }
        }
        if !moved {
            break;
        }
    }
}

struct Board<'a> {
    routes: &'a mut [Route],
    bounds: Vec<Rect>,
    wires: &'a [Wire],
    rules: &'a Rules<'a>,
    weighed: usize,
    /// The horizontal and the vertical pieces of every line, by their lines:
    /// as the board is, not as a move being weighed would make it.
    level: Vec<Laid>,
    upright: Vec<Laid>,
    /// The corridor of every piece of every line that may move.
    corridors: Vec<Vec<Option<(f32, f32)>>>,
    /// What each piece crosses, found when it is first asked for and
    /// forgotten when a line moves.
    sights: Vec<Vec<Option<Sight>>>,
}

/// What a piece and the two it joins cross, for any line the piece could
/// lie on; see [`Board::sight`].
struct Sight {
    /// The low and the high ends of the pieces crossing it, each in order.
    lows: Vec<f32>,
    highs: Vec<f32>,
    /// For each piece it joins: the line of that piece's far end, and the
    /// lines of the pieces passing the corner in order.
    ends: [(f32, Vec<f32>); 2],
}

impl Sight {
    fn crossings(&self, line: f32) -> usize {
        // Of the pieces between the piece's ends, those that begin before
        // the line and do not end before it.
        let begun = self.lows.partition_point(|low| *low < line);
        let ended = self.highs.partition_point(|high| *high <= line);
        let joined: usize = self
            .ends
            .iter()
            .map(|(end, passing)| {
                let (low, high) = (end.min(line) + 0.01, end.max(line) - 0.01);
                passing
                    .iter()
                    .filter(|other| **other > low && **other < high)
                    .count()
            })
            .sum();
        begun - ended + joined
    }
}

/// A piece of a line on the board.
struct Laid {
    route: usize,
    /// `y` of a horizontal piece, `x` of a vertical one.
    line: f32,
    low: f32,
    high: f32,
    /// The part of it that keeps other lines at a distance: all of it but
    /// the stretch at a port.
    body: (f32, f32),
}

/// One way to change the board: a piece on a new line and, when two lines
/// change tracks, the other piece on the line of the first.
#[derive(Clone, Copy)]
struct Move {
    line: f32,
    other: Option<(usize, usize, f32)>,
    /// How many crossings the board loses.
    gain: usize,
    /// How far the pieces travel, to prefer the nearer of two equal moves.
    travel: f32,
}

fn bounds(points: &[Point]) -> Rect {
    Rect::bounding(points.iter().map(|point| Rect::new(*point, *point))).unwrap_or_default()
}

fn horizontal(from: Point, to: Point) -> bool {
    (from.y - to.y).abs() < 0.01 && (from.x - to.x).abs() >= 0.01
}

fn vertical(from: Point, to: Point) -> bool {
    (from.x - to.x).abs() < 0.01 && (from.y - to.y).abs() >= 0.01
}

impl Board<'_> {
    fn movable(&self, route: usize) -> bool {
        route < self.wires.len() && self.routes[route].complete
    }

    /// How many pieces the line has; none when it may not be moved.
    fn pieces(&self, route: usize) -> usize {
        if self.movable(route) {
            self.routes[route].points.len().saturating_sub(1)
        } else {
            0
        }
    }

    /// The piece's direction and line, when it lies between two pieces at
    /// right angles to it.
    fn lane(&self, route: usize, piece: usize) -> Option<(bool, f32)> {
        let points = &self.routes[route].points;
        if piece == 0 || piece + 2 >= points.len() {
            return None;
        }
        let (before, from, to, after) = (
            points[piece - 1],
            points[piece],
            points[piece + 1],
            points[piece + 2],
        );
        if horizontal(from, to) && vertical(before, from) && vertical(to, after) {
            Some((true, from.y))
        } else if vertical(from, to) && horizontal(before, from) && horizontal(to, after) {
            Some((false, from.x))
        } else {
            None
        }
    }

    /// Whether the piece may lie on `line`: the two pieces it joins keep
    /// their direction and a length of the gap, or what they have when that
    /// is less, and the line is in the piece's corridor.
    fn reaches(&self, route: usize, piece: usize, horizontal: bool, line: f32) -> bool {
        let points = &self.routes[route].points;
        let across = |point: Point| if horizontal { point.y } else { point.x };
        let now = across(points[piece]);
        [across(points[piece - 1]), across(points[piece + 2])]
            .into_iter()
            .all(|end| {
                let least = (now - end).abs().min(self.rules.gap);
                (line - end).signum() == (now - end).signum() && (line - end).abs() >= least - 0.01
            })
            && self.corridors[route][piece].is_some_and(|(low, high)| line >= low && line <= high)
    }

    /// The corridor the piece lies in: the lines it may take without coming
    /// within the gap of a card, between the nearest card on either side of
    /// it. `None` for a piece that is within the gap of a card already,
    /// where the board has no room: it stays.
    fn corridor(&self, route: usize, piece: usize) -> Option<(f32, f32)> {
        let (horizontal, _) = self.lane(route, piece)?;
        let points = &self.routes[route].points;
        let (from, to) = (points[piece], points[piece + 1]);
        let (line, low, high) = if horizontal {
            (from.y, from.x.min(to.x), from.x.max(to.x))
        } else {
            (from.x, from.y.min(to.y), from.y.max(to.y))
        };
        // What `packed` forgives: half a pixel.
        let clear = self.rules.gap - 0.5;
        let mut corridor = (f32::NEG_INFINITY, f32::INFINITY);
        for card in self.rules.cards {
            let (near, far, start, end) = if horizontal {
                (card.min.y, card.max.y, card.min.x, card.max.x)
            } else {
                (card.min.x, card.max.x, card.min.y, card.max.y)
            };
            if end + clear <= low || start - clear >= high {
                continue;
            }
            if far + clear <= line {
                corridor.0 = corridor.0.max(far + clear);
            } else if near - clear >= line {
                corridor.1 = corridor.1.min(near - clear);
            } else {
                return None;
            }
        }
        Some(corridor)
    }

    /// Whether the piece would have its track to itself on `line`: no piece
    /// of another line, `except` aside, runs within the gap of it there. A
    /// first look that saves weighing a move the rules would refuse; they
    /// are still asked.
    fn free(
        &self,
        route: usize,
        piece: usize,
        flat: bool,
        line: f32,
        except: Option<usize>,
    ) -> bool {
        let points = &self.routes[route].points;
        let (from, to) = (points[piece], points[piece + 1]);
        let (low, high) = if flat {
            (from.x.min(to.x), from.x.max(to.x))
        } else {
            (from.y.min(to.y), from.y.max(to.y))
        };
        let limit = self.rules.gap - 0.5;
        let beside = if flat { &self.level } else { &self.upright };
        let first = beside.partition_point(|other| other.line <= line - limit);
        beside[first..]
            .iter()
            .take_while(|other| other.line < line + limit)
            .all(|other| {
                other.route == route
                    || Some(other.route) == except
                    || other.body.1 - other.body.0 <= 0.01
                    || low.max(other.body.0) - high.min(other.body.1) >= limit
            })
    }

    /// Finds the corridors of a line's pieces again, after it moved.
    fn survey(&mut self, route: usize) {
        self.corridors[route] = (0..self.pieces(route))
            .map(|piece| self.corridor(route, piece))
            .collect();
    }

    fn set(&mut self, route: usize, piece: usize, horizontal: bool, line: f32) {
        let points = &mut self.routes[route].points;
        for point in &mut points[piece..=piece + 1] {
            if horizontal {
                point.y = line;
            } else {
                point.x = line;
            }
        }
        self.bounds[route] = bounds(points);
    }

    /// What the piece and the two it joins cross on the board as it is,
    /// for any line the piece could lie on.
    fn sight(&self, route: usize, piece: usize, flat: bool) -> Sight {
        let points = &self.routes[route].points;
        let (before, from, to, after) = (
            points[piece - 1],
            points[piece],
            points[piece + 1],
            points[piece + 2],
        );
        let across = |point: Point| if flat { point.y } else { point.x };
        let along = |point: Point| if flat { point.x } else { point.y };
        let (crossing, beside) = if flat {
            (&self.upright, &self.level)
        } else {
            (&self.level, &self.upright)
        };
        // The piece is crossed by the pieces at right angles to it between
        // its ends, wherever it lies between theirs.
        let (low, high) = (along(from).min(along(to)), along(from).max(along(to)));
        let first = crossing.partition_point(|other| other.line <= low + 0.01);
        let (mut lows, mut highs) = (Vec::new(), Vec::new());
        for other in crossing[first..]
            .iter()
            .take_while(|other| other.line < high - 0.01)
            .filter(|other| other.route != route)
        {
            lows.push(other.low + 0.01);
            highs.push(other.high - 0.01);
        }
        lows.sort_by(f32::total_cmp);
        highs.sort_by(f32::total_cmp);
        // A piece it joins is crossed by the pieces that run the piece's own
        // way past the corner, between the corner and that piece's far end.
        // Only as far as the piece can go.
        let far = TRACKS as f32 * self.rules.gap + 1.0;
        let passing = |corner: f32, end: f32| -> Vec<f32> {
            let (nearest, furthest) = (end.min(across(from) - far), end.max(across(from) + far));
            let first = beside.partition_point(|other| other.line < nearest);
            beside[first..]
                .iter()
                .take_while(|other| other.line <= furthest)
                .filter(|other| {
                    other.route != route && corner > other.low + 0.01 && corner < other.high - 0.01
                })
                .map(|other| other.line)
                .collect()
        };
        Sight {
            lows,
            highs,
            ends: [
                (across(before), passing(along(from), across(before))),
                (across(after), passing(along(to), across(after))),
            ],
        }
    }

    /// The crossings of the piece and the two it joins, with every other
    /// line as the board is, were the piece on `line`.
    fn tally(&mut self, route: usize, piece: usize, flat: bool, line: f32) -> usize {
        if self.sights[route].len() <= piece {
            self.sights[route].resize_with(piece + 1, || None);
        }
        if self.sights[route][piece].is_none() {
            self.sights[route][piece] = Some(self.sight(route, piece, flat));
        }
        self.sights[route][piece]
            .as_ref()
            .map_or(0, |sight| sight.crossings(line))
    }

    /// The crossings of the piece and the two it joins with one other line.
    fn against(&self, route: usize, piece: usize, other: usize) -> usize {
        let points = &self.routes[route].points;
        let mut count = 0;
        for pair in points[piece - 1..=piece + 2].windows(2) {
            for (from, to) in self.routes[other].segments() {
                count += usize::from(intersection(pair[0], pair[1], from, to).is_some());
            }
        }
        count
    }

    /// Lists the pieces of every line again, after a move.
    fn index(&mut self) {
        for sights in &mut self.sights {
            sights.clear();
        }
        self.level.clear();
        self.upright.clear();
        let gap = self.rules.gap;
        for (route, drawn) in self.routes.iter().enumerate() {
            let last = drawn.points.len().saturating_sub(2);
            for (index, (from, to)) in drawn.segments().enumerate() {
                let flat = horizontal(from, to);
                if !flat && !vertical(from, to) {
                    continue;
                }
                let (line, start, end) = if flat {
                    (from.y, from.x, to.x)
                } else {
                    (from.x, from.y, to.y)
                };
                let (low, high) = (start.min(end), start.max(end));
                // As `packed` judges it: without the gap's length at a port.
                let leaves = if index == 0 { gap } else { 0.0 };
                let enters = if index == last { gap } else { 0.0 };
                let body = if start < end {
                    (low + leaves, high - enters)
                } else {
                    (low + enters, high - leaves)
                };
                let laid = Laid {
                    route,
                    line,
                    low,
                    high,
                    body,
                };
                if flat {
                    self.level.push(laid);
                } else {
                    self.upright.push(laid);
                }
            }
        }
        self.level.sort_by(|a, b| a.line.total_cmp(&b.line));
        self.upright.sort_by(|a, b| a.line.total_cmp(&b.line));
    }

    /// The crossings of two lines with each other.
    fn between(&self, a: usize, b: usize) -> usize {
        if !touches(&self.bounds[a], &self.bounds[b]) {
            return 0;
        }
        let mut count = 0;
        for (a1, a2) in self.routes[a].segments() {
            for (b1, b2) in self.routes[b].segments() {
                count += usize::from(intersection(a1, a2, b1, b2).is_some());
            }
        }
        count
    }

    /// What a move may not make worse, for one line: the mouths of other
    /// ports that the piece and the two it joins run into.
    fn trespasses(&self, route: usize, piece: usize) -> usize {
        let points = &self.routes[route].points;
        let wire = &self.wires[route];
        let last = points.len() - 2;
        let reach = self.rules.gap - 0.01;
        // The stub ends of the line's own two ports, and the lines they are on.
        let ahead = |from: Point, to: Point| {
            let length = from.orthogonal_distance(to);
            if length < 0.01 {
                return from;
            }
            let part = self.rules.stub.min(length) / length;
            Point::new(
                from.x + (to.x - from.x) * part,
                from.y + (to.y - from.y) * part,
            )
        };
        let own = [
            ahead(points[0], points[1]),
            ahead(points[last + 1], points[last]),
        ];
        let mut count = 0;
        for index in piece - 1..=piece + 1 {
            let (from, to) = (points[index], points[index + 1]);
            let span = Rect::new(
                Point::new(from.x.min(to.x), from.y.min(to.y)),
                Point::new(from.x.max(to.x), from.y.max(to.y)),
            );
            let flat = horizontal(from, to);
            if !flat && !vertical(from, to) {
                continue;
            }
            let (line, low, high) = if flat {
                (from.y, span.min.x, span.max.x)
            } else {
                (from.x, span.min.y, span.max.y)
            };
            for mouth in self.rules.mouths {
                let (across, along) = if flat {
                    (mouth.at.y, mouth.at.x)
                } else {
                    (mouth.at.x, mouth.at.y)
                };
                if mouth.horizontal != flat
                    || (line - across).abs() >= reach
                    || low >= along + reach
                    || high <= along - reach
                    || own
                        .iter()
                        .any(|end| end.orthogonal_distance(mouth.at) < 0.5)
                {
                    continue;
                }
                // A line passes the mouths beside its own ports on its own
                // ports' lines, as the router lets it.
                let lines = if flat {
                    [own[0].y, own[1].y]
                } else {
                    [own[0].x, own[1].x]
                };
                let neighbour = mouth.block == wire.from.block || mouth.block == wire.to.block;
                count +=
                    usize::from(!neighbour || lines.iter().all(|own| (own - line).abs() >= 0.5));
            }
        }
        count
    }

    /// The breaches of the gap that the pieces of a move are in, each with
    /// the two it joins, among the lines within `reach` of them.
    fn breaches(&self, moved: &[(usize, usize)], reach: f32) -> Vec<Breach> {
        let around = Rect::bounding(
            moved
                .iter()
                .map(|(route, piece)| bounds(&self.routes[*route].points[piece - 1..=piece + 2])),
        )
        .unwrap_or_default()
        .inflate(reach);
        involving(
            self.routes,
            self.rules.cards,
            self.rules.gap,
            |route, index| {
                moved
                    .iter()
                    .any(|(of, piece)| *of == route && index + 1 >= *piece && index <= piece + 1)
            },
            |route| touches(&around, &self.bounds[route]),
        )
    }

    /// Tries the piece on the other tracks of its corridor, and against the
    /// pieces of other lines there, and makes the best move that takes
    /// crossings off the board.
    fn improve(&mut self, route: usize, piece: usize) -> bool {
        let Some((flat, line)) = self.lane(route, piece) else {
            return false;
        };
        let here = self.tally(route, piece, flat, line);
        if here == 0 {
            // Nothing to gain: this piece and the two it joins cross nothing.
            return false;
        }
        let gap = self.rules.gap;
        let far = TRACKS as f32 * gap;
        let mut moves: Vec<Move> = Vec::new();
        // Another track.
        let nearest = (line / gap).round() as i32;
        for track in nearest - TRACKS..=nearest + TRACKS {
            let target = track as f32 * gap;
            if (target - line).abs() < 0.5
                || !self.reaches(route, piece, flat, target)
                || !self.free(route, piece, flat, target, None)
            {
                continue;
            }
            self.weighed += 1;
            let there = self.tally(route, piece, flat, target);
            if there < here {
                moves.push(Move {
                    line: target,
                    other: None,
                    gain: here - there,
                    travel: (target - line).abs(),
                });
            }
        }
        // The track of another line, which takes this one.
        let span = bounds(&self.routes[route].points[piece..=piece + 1]).inflate(far);
        for other in 0..self.wires.len() {
            if other == route || !touches(&span, &self.bounds[other]) {
                continue;
            }
            for theirs in 1..self.pieces(other).saturating_sub(1) {
                let Some((their_flat, their_line)) = self.lane(other, theirs) else {
                    continue;
                };
                if their_flat != flat
                    || (their_line - line).abs() < 0.5
                    || (their_line - line).abs() > far
                    || !self.reaches(route, piece, flat, their_line)
                    || !self.reaches(other, theirs, flat, line)
                    || !self.free(route, piece, flat, their_line, Some(other))
                    || !self.free(other, theirs, flat, line, Some(route))
                {
                    continue;
                }
                self.weighed += 1;
                // Each line against the rest of the board as it is, less the
                // other of the two, which moves as well; then the two
                // against each other.
                let before = here + self.tally(other, theirs, flat, their_line)
                    - self.against(route, piece, other)
                    - self.against(other, theirs, route)
                    + self.between(route, other);
                let mut after = self.tally(route, piece, flat, their_line)
                    + self.tally(other, theirs, flat, line);
                self.set(route, piece, flat, their_line);
                after -= self.against(route, piece, other);
                self.set(route, piece, flat, line);
                self.set(other, theirs, flat, line);
                after -= self.against(other, theirs, route);
                self.set(route, piece, flat, their_line);
                after += self.between(route, other);
                self.set(route, piece, flat, line);
                self.set(other, theirs, flat, their_line);
                if after < before {
                    moves.push(Move {
                        line: their_line,
                        other: Some((other, theirs, their_line)),
                        gain: before - after,
                        travel: (their_line - line).abs() * 2.0,
                    });
                }
            }
        }
        // The most crossings first, then the shortest way; the order they
        // were found in settles the rest.
        moves.sort_by(|left, right| {
            right
                .gain
                .cmp(&left.gain)
                .then_with(|| left.travel.total_cmp(&right.travel))
        });
        for candidate in moves.into_iter().take(TRIED) {
            let mut moved = vec![(route, piece)];
            moved.extend(candidate.other.map(|(other, theirs, _)| (other, theirs)));
            let trespasses = |board: &Self| -> usize {
                moved
                    .iter()
                    .map(|(route, piece)| board.trespasses(*route, *piece))
                    .sum()
            };
            // Wide enough for the pieces where they are and where they go.
            let reach = (candidate.line - line).abs() + gap;
            let (in_mouths, packed) = (trespasses(self), self.breaches(&moved, reach));
            self.set(route, piece, flat, candidate.line);
            if let Some((other, theirs, _)) = candidate.other {
                self.set(other, theirs, flat, line);
            }
            // On a board with no room for the gap somewhere, a move may leave
            // a breach where it was; it makes none.
            let kept = |breach: &Breach| {
                packed.iter().any(|old| {
                    old.route == breach.route
                        && old.near == breach.near
                        && old.horizontal == breach.horizontal
                        && within(&breach.area, &old.area)
                        && apart(breach) >= apart(old) - 0.5
                })
            };
            if trespasses(self) <= in_mouths && self.breaches(&moved, reach).iter().all(kept) {
                self.index();
                for (route, _) in moved {
                    self.survey(route);
                }
                return true;
            }
            self.set(route, piece, flat, line);
            if let Some((other, theirs, their_line)) = candidate.other {
                self.set(other, theirs, flat, their_line);
            }
        }
        false
    }
}

/// Whether two rectangles meet, their borders included: a line has no area.
fn touches(a: &Rect, b: &Rect) -> bool {
    a.min.x <= b.max.x && b.min.x <= a.max.x && a.min.y <= b.max.y && b.min.y <= a.max.y
}

/// Whether `inner` lies in `outer`, give or take a rounding.
fn within(inner: &Rect, outer: &Rect) -> bool {
    inner.min.x >= outer.min.x - 0.5
        && inner.min.y >= outer.min.y - 0.5
        && inner.max.x <= outer.max.x + 0.5
        && inner.max.y <= outer.max.y + 0.5
}

/// How far apart the two of a breach are: nothing for a line at a card.
fn apart(breach: &Breach) -> f32 {
    if breach.horizontal {
        breach.area.height()
    } else {
        breach.area.width()
    }
}
