//! Orthogonal wire routing: wires that go around cards instead of through them.
//!
//! A wire leaves its port perpendicular to the card side for a short stub, then
//! travels along the orthogonal visibility graph over the inflated card rectangles
//! to the other stub. A\* picks the route, paying for length, bends and crossings,
//! and the later of two crossing wires draws a bridge.
//!
//! Every line keeps a gap (LAY-03): [`Routing::margin`] from each card, its own
//! two included, and [`Routing::channel_gap`] from each line routed before it,
//! side by side and end to end. Travel the same way within the gap of a segment
//! already routed is priced out of the search; crossing it stays free. Wires of
//! one port share the straight piece out of that port and leave it one by one.
//! The grid has a track every gap, so lines that share a corridor stack in it at
//! that distance, and a line with no room there goes the long way round.
//! [`crate::packed`] is how tests check it.
//!
//! The guarantee the board relies on: a final route never touches a card other than
//! its own two, and never passes under those (LAY-29): it meets them at its ports.
//! Where a port itself lies inside another card — cards can overlap, the wire's
//! other card included — the route may cross that card within its stub and no
//! further. Every step that could break the guarantee (nudging, smoothing) checks
//! its result and falls back.
//! [ADR 0006](../../../docs/decisions/0006-layout-and-wire-routing.md) is the
//! contract; `violations` and `under_own` are how tests check it.

use crate::geometry::{Point, Port, Rect, Side, compare};
use crate::order::{Rules, untangle};
use crate::packed::GAP;
use std::collections::BinaryHeap;

/// Routing parameters, in pixels and in cost units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Routing {
    /// How far a wire stays clear of a card it passes.
    pub margin: f32,
    /// Length of the stub leaving a port.
    pub stub: f32,
    /// Cost of one corner, as a length.
    pub bend_cost: f32,
    /// Cost of crossing a wire routed earlier, as a length.
    pub crossing_cost: f32,
    /// Smallest distance between two parallel segments of two wires.
    pub channel_gap: f32,
    /// `None` keeps the gaps: a wire gives one up only when no route keeps it.
    /// `Some(price)` lets a wire run within the gap of a card or of another
    /// wire, never through a card, for `price` times the length it runs there:
    /// a wire routed alone this way shows the corridor it wants, which is how
    /// Tidy up finds the gaps to widen.
    pub squeeze: Option<f32>,
    /// Whether the lines of a corridor are put in the order that crosses
    /// least, once every wire has its route (LAY-28, [`crate::order`]).
    /// Without it each line keeps the track the router gave it.
    pub ordered: bool,
}

impl Default for Routing {
    fn default() -> Self {
        Self {
            margin: GAP,
            stub: GAP,
            bend_cost: 40.0,
            crossing_cost: 120.0,
            channel_gap: GAP,
            squeeze: None,
            ordered: true,
        }
    }
}

impl Routing {
    /// The routing Tidy up asks a board's lines with, one wire at a time: a
    /// line may pass within the gap of a card, at a price, where that is the
    /// shorter way.
    pub fn sizing() -> Self {
        Self {
            squeeze: Some(SQUEEZE),
            ..Self::default()
        }
    }
}

/// What a pixel within the gap of a card costs while Tidy up asks a board's
/// lines what room they want, as a length: a line would rather pass between
/// two cards, 300 px for the price of 75, than go round a stack of them, and
/// Tidy up then makes that passage wide enough. Measured on the reference
/// boards: at this price they have a quarter fewer crossings than at 1.
const SQUEEZE: f32 = 0.25;

/// The same on the board as it is drawn: 20 px beside another line cost as
/// much as 4000 px of detour, more than the search window holds, so a line
/// runs within the gap of another only where the window has no other route.
/// A price instead of a wall keeps that case in the same search.
const KEPT: f32 = 200.0;

/// How far beyond the two ports the first search looks. A detour around a card or
/// two fits in this; a wire that needs more gets a wider window.
const LOCAL_SLACK: f32 = 480.0;

/// The largest grid one search builds. A board whose cards leave a wire no local
/// route can reach this; beyond it the wire is drawn as a plain corner rather than
/// made to wait. The cap is far above what a readable board needs.
const MAX_CELLS: usize = 160_000;

/// A wire to route, from an output port to an input port.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wire {
    pub from: Port,
    pub to: Port,
}

impl Wire {
    pub const fn new(from: Port, to: Port) -> Self {
        Self { from, to }
    }
}

/// Where one wire crosses another, for the bridge the later wire draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bridge {
    pub at: Point,
    /// Whether the crossing segment of this wire runs horizontally.
    pub horizontal: bool,
}

/// A routed wire: a polyline from port to port, plus its bridges.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<Point>,
    pub bridges: Vec<Bridge>,
    /// Whether a route around the cards was found. It is false only when the ports
    /// sit in a pocket that cards enclose completely, the wire's own two among
    /// them (LAY-29); the wire is then drawn as a plain corner so that it stays
    /// visible and orthogonal, and it may cross a card. Such a board needs
    /// *tidy up*, not a cleverer router.
    pub complete: bool,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            bridges: Vec::new(),
            complete: true,
        }
    }
}

impl Route {
    /// The route's segments as point pairs.
    pub fn segments(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        self.points.windows(2).map(|pair| (pair[0], pair[1]))
    }

    pub fn length(&self) -> f32 {
        self.segments()
            .map(|(from, to)| from.orthogonal_distance(to))
            .sum()
    }

    /// Corners, excluding the two ends.
    pub fn bends(&self) -> usize {
        self.points.len().saturating_sub(2)
    }
}

/// Routes every wire around the cards.
///
/// `cards` are the rectangles in the order the ports refer to them. A wire between
/// two top ports, an ordering wire on the rail above a row (LAY-06), is routed
/// first; then longer wires, so the long route picks the open channel and the short
/// ones adapt.
pub fn route(cards: &[Rect], wires: &[Wire], settings: &Routing) -> Vec<Route> {
    route_among(cards, wires, settings, &[])
}

/// [`route`], among lines that are already on the board and stay as they are:
/// `fixed` are their routes, as `route` returned them for the same cards. The
/// new wires keep the gap from them as from each other. The board uses it for
/// the wires it brings back while a card is selected.
pub fn route_among(
    cards: &[Rect],
    wires: &[Wire],
    settings: &Routing,
    fixed: &[Route],
) -> Vec<Route> {
    route_reserving(cards, wires, settings, fixed, &[])
}

/// [`route_among`], keeping the mouths of `sockets` free as well: ports no
/// wire of this call ends on, whose wires the board draws later. A line may
/// cross in front of one but does not run into it or turn there, so the
/// wire brought back into it has its 20 px (LAY-29: an ordering line now
/// passes the inputs of the next card on its way to the rail).
pub fn route_reserving(
    cards: &[Rect],
    wires: &[Wire],
    settings: &Routing,
    fixed: &[Route],
    sockets: &[Port],
) -> Vec<Route> {
    let mut routes = vec![Route::default(); wires.len()];
    let order = routing_order(cards, wires);

    let mut routed: Vec<Trace> = fixed
        .iter()
        .filter(|route| route.complete && route.points.len() >= 2)
        .map(|route| Trace::of(&route.points, settings.stub))
        .collect();
    let mouths: Vec<Mouth> = wires
        .iter()
        .flat_map(|wire| [(wire.from, false), (wire.to, false)])
        .chain(
            sockets
                .iter()
                .filter(|socket| {
                    // A socket a wire of this routing ends on is that
                    // wire's port, not a free one.
                    !wires
                        .iter()
                        .flat_map(|wire| [wire.from, wire.to])
                        .any(|port| {
                            port.block == socket.block
                                && port.side == socket.side
                                && (port.offset - socket.offset).abs() < 0.5
                        })
                })
                .map(|port| (*port, true)),
        )
        .filter_map(|(port, free)| {
            let card = cards.get(port.block)?;
            Some(Mouth {
                at: step(port.point_on(card), port.side, settings.stub),
                horizontal: port.side.is_horizontal(),
                block: port.block,
                free,
            })
        })
        .collect();
    let mut scratch = Scratch::default();
    for index in order {
        let wire = wires[index];
        let (Some(from_card), Some(to_card)) =
            (cards.get(wire.from.block), cards.get(wire.to.block))
        else {
            continue;
        };
        let foreign: Vec<Rect> = cards
            .iter()
            .enumerate()
            .filter(|(card, _)| *card != wire.from.block && *card != wire.to.block)
            .map(|(_, card)| *card)
            .collect();
        // The two cards the wire joins. A route never passes under them
        // (LAY-29): a card is painted over its wires, so a piece of a line
        // inside one is not seen and the line seems to come out of an edge
        // where there is no port. They are in the wire's way on every attempt.
        let own: Vec<Rect> = [wire.from.block, wire.to.block]
            .into_iter()
            .filter_map(|block| cards.get(block).copied())
            .collect();
        // The card at the other end, for each port: cards overlap, so a port
        // can lie inside the card its wire goes to.
        let apart = wire.from.block != wire.to.block;
        let (beyond_start, beyond_end) = (apart.then_some(*to_card), apart.then_some(*from_card));
        let start = wire.from.point_on(from_card);
        let end = wire.to.point_on(to_card);
        // Cards overlap, so a port can sit inside another card, the wire's
        // other card included. The stub then reaches past whatever buries it:
        // that crossing belongs to the stub, which is the one exception the
        // guarantee makes, and the route proper starts in open space.
        let attempt = |rung: Rung, scratch: &mut Scratch| {
            let Rung { margin, slack } = rung;
            // While Tidy up measures, a card keeps a line off itself only; the
            // clearance around it is room the line pays for.
            let (margin, zone) = match settings.squeeze {
                Some(_) => (0.0, margin),
                None => (margin, 0.0),
            };
            let mut obstacles: Vec<Rect> =
                foreign.iter().map(|card| card.inflate(margin)).collect();
            let mut zones: Vec<Rect> = if zone > 0.0 {
                foreign.iter().map(|card| card.inflate(zone)).collect()
            } else {
                Vec::new()
            };
            // A stub is measured before the wire's own cards join the obstacles: a
            // port sits on its card's edge, so a clearance around that card would
            // leave it nowhere to start.
            let stub_start = clear_stub(
                start,
                wire.from.side,
                &foreign,
                &obstacles,
                beyond_start.map(|card| (card, card.inflate(margin))),
                settings,
            );
            let stub_end = clear_stub(
                end,
                wire.to.side,
                &foreign,
                &obstacles,
                beyond_end.map(|card| (card, card.inflate(margin))),
                settings,
            );
            obstacles.extend(own.iter().map(|card| card.inflate(margin)));
            if zone > 0.0 {
                zones.extend(own.iter().map(|card| card.inflate(zone)));
            }
            // The search covers a window around the two ports. A card outside it
            // cannot be met by a route inside it, and a wire between neighbours
            // then searches a small graph instead of the whole board.
            let window = Rect::bounding([
                Rect::new(start, start),
                Rect::new(end, end),
                Rect::new(stub_start, stub_start),
                Rect::new(stub_end, stub_end),
            ])
            .unwrap_or(Rect::new(start, end))
            .inflate(slack);
            let path = search(
                &Problem {
                    start: stub_start,
                    end: stub_end,
                    from_side: wire.from.side,
                    to_side: wire.to.side,
                    obstacles: &obstacles,
                    zones: &zones,
                    routed: &routed,
                    price: settings.squeeze.unwrap_or(KEPT),
                    mouths: &mouths,
                    blocks: (wire.from.block, wire.to.block),
                    origin: start,
                    target: end,
                    settings,
                    window,
                },
                scratch,
            );
            (stub_start, stub_end, path)
        };
        // A detour needs room, so the window widens until a route appears. The
        // margin is a clearance, not the guarantee: where cards leave no room for
        // it, the last attempt lets the wire hug them instead of crossing them.
        let whole = Rect::bounding(cards.iter().copied())
            .map(|board| board.width().max(board.height()))
            .unwrap_or(0.0);
        let rung = |margin: f32, slack: f32| Rung { margin, slack };
        let mut attempted = attempt(rung(settings.margin, LOCAL_SLACK), &mut scratch);
        // No attempt lets the wire under a card it joins (LAY-29): where
        // cards leave no way round, the wire has no route and says so.
        for next in [
            rung(settings.margin, LOCAL_SLACK * 4.0),
            rung(settings.margin, whole),
            rung(0.0, whole),
        ] {
            if !attempted.2.is_empty() {
                break;
            }
            attempted = attempt(next, &mut scratch);
        }
        let (stub_start, stub_end, path) = attempted;
        let complete = !path.is_empty();
        let mut points = vec![start];
        if complete {
            points.extend(path);
        } else {
            // Enclosed ports: a plain corner keeps the wire visible and orthogonal.
            points.push(stub_start);
            points.push(Point::new(stub_end.x, stub_start.y));
            points.push(stub_end);
        }
        points.push(end);
        // The search walks the grid cell by cell, so collinear runs arrive split
        // into many small segments; nudging them separately would build a
        // staircase. The corners go first, the stub ends stay.
        let points = simplify(points);
        if complete {
            routed.push(Trace::of(&points, settings.stub));
        }
        routes[index] = Route {
            points,
            bridges: Vec::new(),
            complete,
        };
    }
    if settings.ordered && settings.squeeze.is_none() {
        // The lines are where the gaps allow; now their order in each
        // corridor (LAY-28). The lines that stay are counted, not moved.
        let mut board: Vec<Route> = routes;
        board.extend(fixed.iter().cloned());
        untangle(
            &mut board,
            wires,
            &Rules {
                cards,
                mouths: &mouths,
                gap: settings.channel_gap,
                stub: settings.stub,
            },
        );
        board.truncate(wires.len());
        routes = board;
    }
    bridges(&mut routes, wires, cards);
    routes
}

/// One attempt at a wire: the clearance from cards and how far the search
/// looks.
#[derive(Clone, Copy)]
struct Rung {
    margin: f32,
    slack: f32,
}

/// Where the stub of a port ends, kept free for that port's wires.
pub(crate) struct Mouth {
    pub(crate) at: Point,
    /// Whether the stub runs horizontally.
    pub(crate) horizontal: bool,
    /// The card the port is on.
    pub(crate) block: usize,
    /// A socket no wire of this routing ends on, kept for a wire the board
    /// draws later ([`route_reserving`]).
    pub(crate) free: bool,
}

/// A line already on the board, as the wires routed after it see it.
struct Trace {
    /// Every segment, for the cost of crossing it.
    segments: Vec<(Point, Point)>,
    /// The segments that keep others at a distance: all of it but the stretch
    /// at each port, which is as near its neighbours as the ports are.
    body: Vec<Body>,
    from: Point,
    to: Point,
}

#[derive(Clone, Copy)]
struct Body {
    horizontal: bool,
    /// `y` of a horizontal segment, `x` of a vertical one.
    line: f32,
    low: f32,
    high: f32,
    /// The straight piece out of the wire's first port, which wires of the same
    /// port share; and the one into its last port.
    exit: bool,
    entry: bool,
}

impl Trace {
    fn of(points: &[Point], stub: f32) -> Self {
        let segments: Vec<(Point, Point)> =
            points.windows(2).map(|pair| (pair[0], pair[1])).collect();
        let last = segments.len().saturating_sub(1);
        let mut body = Vec::new();
        for (index, (from, to)) in segments.iter().enumerate() {
            let horizontal = (from.y - to.y).abs() < 0.01;
            if horizontal == ((from.x - to.x).abs() < 0.01) {
                continue;
            }
            let (mut start, mut end) = if horizontal {
                (from.x, to.x)
            } else {
                (from.y, to.y)
            };
            let direction = (end - start).signum();
            if index == 0 {
                start += direction * stub.min((end - start).abs());
            }
            if index == last {
                end -= direction * stub.min((end - start).abs());
            }
            if (end - start) * direction <= 0.01 {
                continue;
            }
            body.push(Body {
                horizontal,
                line: if horizontal { from.y } else { from.x },
                low: start.min(end),
                high: start.max(end),
                exit: index == 0,
                entry: index == last,
            });
        }
        Self {
            segments,
            body,
            from: points.first().copied().unwrap_or_default(),
            to: points.last().copied().unwrap_or_default(),
        }
    }
}

/// The end of the stub leaving `point`.
///
/// Two cases, and they pull in opposite directions. A port *inside* another card —
/// cards overlap — needs a stub long enough to leave it, because that crossing is
/// the one exception the guarantee makes. A port merely *beside* a card has no room
/// for a full stub, so the stub shrinks to the free distance, possibly to nothing,
/// and the search leaves along the card instead of through it.
///
/// `beyond` is the card at the wire's other end and its obstacle: a port
/// inside that card leaves it through its stub too (LAY-29). A port merely
/// beside it keeps its stub; the search then deals with the card.
fn clear_stub(
    point: Point,
    side: Side,
    cards: &[Rect],
    obstacles: &[Rect],
    beyond: Option<(Rect, Rect)>,
    settings: &Routing,
) -> Point {
    // A port within the clearance of a card is not inside it. Only a card that
    // really contains the port earns the long stub.
    let beyond = beyond
        .filter(|(card, _)| card.contains_strictly(point))
        .map(|(_, obstacle)| obstacle);
    let buried = beyond.is_some() || cards.iter().any(|card| card.contains_strictly(point));
    if !buried {
        let free = obstacles
            .iter()
            .filter_map(|obstacle| entry_distance(point, side, obstacle))
            .fold(settings.stub, f32::min);
        return step(point, side, free.max(0.0));
    }
    let mut distance = settings.stub;
    // Four rounds leave a pile of cards; each round clears the ones found so far.
    for _ in 0..4 {
        let end = step(point, side, distance);
        let Some(beyond) = obstacles
            .iter()
            .chain(&beyond)
            .filter(|obstacle| obstacle.contains(end))
            .map(|obstacle| far_side(point, side, obstacle) + 1.0)
            .fold(None, |most: Option<f32>, far| {
                Some(most.map_or(far, |most| most.max(far)))
            })
        else {
            return end;
        };
        if beyond <= distance {
            return end;
        }
        distance = beyond;
    }
    step(point, side, distance)
}

/// How far along `side` the far edge of `obstacle` lies, seen from `point`.
fn far_side(point: Point, side: Side, obstacle: &Rect) -> f32 {
    match side {
        Side::Left => point.x - obstacle.min.x,
        Side::Right => obstacle.max.x - point.x,
        Side::Top => point.y - obstacle.min.y,
        Side::Bottom => obstacle.max.y - point.y,
    }
}

/// How far a ray leaving `point` along `side` travels before entering `obstacle`,
/// or `None` when it never does.
fn entry_distance(point: Point, side: Side, obstacle: &Rect) -> Option<f32> {
    let (across, along, near, far) = match side {
        Side::Left => (point.y, point.x, obstacle.max.x, obstacle.min.x),
        Side::Right => (point.y, point.x, obstacle.min.x, obstacle.max.x),
        Side::Top => (point.x, point.y, obstacle.max.y, obstacle.min.y),
        Side::Bottom => (point.x, point.y, obstacle.min.y, obstacle.max.y),
    };
    let (low, high) = if side.is_horizontal() {
        (obstacle.min.y, obstacle.max.y)
    } else {
        (obstacle.min.x, obstacle.max.x)
    };
    if across <= low || across >= high {
        return None;
    }
    let forward = if matches!(side, Side::Left | Side::Top) {
        -1.0
    } else {
        1.0
    };
    let entry = (near - along) * forward;
    let exit = (far - along) * forward;
    if exit <= 0.0 {
        None
    } else {
        Some(entry.max(0.0))
    }
}

/// Moves a point `distance` along the outward direction of a side.
fn step(point: Point, side: Side, distance: f32) -> Point {
    let direction = side.outward();
    Point::new(
        point.x + direction.x * distance,
        point.y + direction.y * distance,
    )
}

/// Coordinates the visibility graph may use, as sorted lines.
///
/// `required` coordinates — the stub ends — are kept exactly and nothing else is
/// allowed within half a pixel of them, so a route's first and last points are the
/// stub ends themselves rather than a line that happens to be near them. A segment
/// of a few tenths of a pixel would otherwise appear diagonal.
fn lines(
    required: impl IntoIterator<Item = f32>,
    others: impl IntoIterator<Item = f32>,
) -> Vec<f32> {
    let finite = |values: Vec<f32>| -> Vec<f32> {
        let mut values: Vec<f32> = values
            .into_iter()
            .filter(|value| value.is_finite())
            .collect();
        values.sort_by(|left, right| compare(*left, *right));
        values.dedup_by(|left, right| (*left - *right).abs() < 0.5);
        values
    };
    let mut kept: Vec<f32> = required
        .into_iter()
        .filter(|value| value.is_finite())
        .collect();
    kept.sort_by(|left, right| compare(*left, *right));
    // Two stub ends a fraction of a pixel apart stay two lines: merging them is
    // what turns a stub into a diagonal.
    kept.dedup_by(|left, right| (*left - *right).abs() < 1e-4);
    for value in finite(others.into_iter().collect()) {
        if kept.iter().all(|line| (line - value).abs() >= 0.5) {
            kept.push(value);
        }
    }
    kept.sort_by(|left, right| compare(*left, *right));
    kept
}

/// A\* over the orthogonal visibility graph between the two stub ends.
///
/// The graph is the sparse grid of the interesting lines: the stub ends, the sides
/// of each nearby obstacle, and a frame around the window, so a route around the
/// cards in the window always exists. A grid edge that a card covers is left out,
/// which is what keeps a route off the cards.
///
/// Two things keep this fast enough for a board of hundreds of cards. The grid
/// covers a window around the two ports rather than the whole board, so a short
/// wire searches a small graph and a card outside the window cannot be met anyway;
/// a caller widens the window when the search comes back empty. And the blocked
/// edges and the crossings of earlier wires are marked once per search by walking
/// the obstacles, instead of being recomputed for every edge the search considers.
fn search(problem: &Problem<'_>, scratch: &mut Scratch) -> Vec<Point> {
    let Problem {
        start,
        end,
        from_side,
        to_side,
        obstacles,
        zones,
        routed,
        price,
        mouths,
        blocks,
        origin,
        target,
        settings,
        window,
    } = *problem;
    let nearby: Vec<Rect> = obstacles
        .iter()
        .copied()
        .filter(|obstacle| obstacle.overlaps(&window))
        .collect();
    let frame = window.inflate(settings.margin + settings.stub);
    let near: Vec<Rect> = zones
        .iter()
        .copied()
        .filter(|zone| zone.overlaps(&window))
        .collect();
    // The grid: the sides of the cards, at their clearance, and a track every
    // gap, so that lines sharing a corridor find places a gap apart in it. A
    // board too large for the tracks is searched on the cards' sides alone.
    let grid = |tracks: bool| {
        let gap = settings.channel_gap;
        let track = move |low: f32, high: f32| {
            let count = if tracks && gap >= 1.0 {
                ((high - low) / gap).floor().max(0.0) as usize
            } else {
                0
            };
            let first = (low / gap).floor() + 1.0;
            (0..count).map(move |step| (first + step as f32) * gap)
        };
        let xs = lines(
            [start.x, end.x],
            [frame.min.x, frame.max.x]
                .into_iter()
                .chain(
                    nearby
                        .iter()
                        .chain(&near)
                        .flat_map(|card| [card.min.x, card.max.x]),
                )
                .chain(track(frame.min.x, frame.max.x))
                .filter(|line| *line > frame.min.x && *line < frame.max.x),
        );
        let ys = lines(
            [start.y, end.y],
            [frame.min.y, frame.max.y]
                .into_iter()
                .chain(
                    nearby
                        .iter()
                        .chain(&near)
                        .flat_map(|card| [card.min.y, card.max.y]),
                )
                .chain(track(frame.min.y, frame.max.y))
                .filter(|line| *line > frame.min.y && *line < frame.max.y),
        );
        (xs, ys)
    };
    let (mut xs, mut ys) = grid(true);
    if xs.len() * ys.len() > MAX_CELLS {
        (xs, ys) = grid(false);
    }
    let (width, height) = (xs.len(), ys.len());
    if width < 2 || height < 2 || width * height > MAX_CELLS {
        return Vec::new();
    }
    let Some(from) = cell_of(start, &xs, &ys) else {
        return Vec::new();
    };
    let Some(to) = cell_of(end, &xs, &ys) else {
        return Vec::new();
    };
    let at = |cell: usize| Point::new(xs[cell % width], ys[cell / width]);
    let cell = |x: usize, y: usize| y * width + x;

    // Which grid edges a card covers. A horizontal edge runs from `xs[x]` to
    // `xs[x + 1]` at `ys[y]`; a vertical one from `ys[y]` to `ys[y + 1]` at `xs[x]`.
    let mut blocked_h = vec![false; width.saturating_sub(1) * height];
    let mut blocked_v = vec![false; width * height.saturating_sub(1)];
    // How many earlier wires a grid edge crosses, which the cost pays for.
    let mut crossing_h = vec![0u16; blocked_h.len()];
    let mut crossing_v = vec![0u16; blocked_v.len()];
    // Lines strictly inside a span, as an index range.
    let inside = |lines: &[f32], low: f32, high: f32| {
        let first = lines.partition_point(|line| *line <= low);
        let last = lines.partition_point(|line| *line < high);
        first..last
    };
    // Edges that a span overlaps, as an index range. Edge `i` runs between lines
    // `i` and `i + 1`, so it is covered when `lines[i] < high` and `lines[i + 1] >
    // low`; there are `len - 1` edges.
    let covering = |lines: &[f32], low: f32, high: f32| {
        let first = lines.partition_point(|line| *line <= low).saturating_sub(1);
        let last = lines
            .partition_point(|line| *line < high)
            .min(lines.len().saturating_sub(1));
        first..last.max(first)
    };
    for obstacle in &nearby {
        for y in inside(&ys, obstacle.min.y, obstacle.max.y) {
            for x in covering(&xs, obstacle.min.x, obstacle.max.x) {
                blocked_h[y * (width - 1) + x] = true;
            }
        }
        for x in inside(&xs, obstacle.min.x, obstacle.max.x) {
            for y in covering(&ys, obstacle.min.y, obstacle.max.y) {
                blocked_v[y * width + x] = true;
            }
        }
    }
    // Edges within the gap of a card (while Tidy up measures) or of a line
    // routed earlier: open at a price.
    let mut tight_h = vec![false; blocked_h.len()];
    let mut tight_v = vec![false; blocked_v.len()];
    for zone in &near {
        for y in inside(&ys, zone.min.y, zone.max.y) {
            for x in covering(&xs, zone.min.x, zone.max.x) {
                tight_h[y * (width - 1) + x] = true;
            }
        }
        for x in inside(&xs, zone.min.x, zone.max.x) {
            for y in covering(&ys, zone.min.y, zone.max.y) {
                tight_v[y * width + x] = true;
            }
        }
    }
    let reach = settings.channel_gap - 0.01;
    let same = |a: Point, b: Point| a.orthogonal_distance(b) < 0.5;
    for wire in routed.iter().filter(|_| reach > 0.0) {
        for piece in &wire.body {
            // Only a line near the window can be met by a route in it.
            let (low, high) = (piece.low - reach, piece.high + reach);
            let outside = if piece.horizontal {
                piece.line + reach < frame.min.y
                    || piece.line - reach > frame.max.y
                    || high < frame.min.x
                    || low > frame.max.x
            } else {
                piece.line + reach < frame.min.x
                    || piece.line - reach > frame.max.x
                    || high < frame.min.y
                    || low > frame.max.y
            };
            if outside {
                continue;
            }
            // Wires of one port share the straight piece out of it, and into
            // it; so do wires of ports nearer than the gap, each on its own
            // port's line, or no line could reach a port among them.
            let coordinate = |point: Point| if piece.horizontal { point.y } else { point.x };
            let beside = |port: Point| {
                [origin, target]
                    .into_iter()
                    .filter(|own| own.distance(port) < settings.channel_gap - 0.5)
                    .map(coordinate)
                    .collect::<Vec<f32>>()
            };
            let mut open: Vec<f32> = Vec::new();
            if piece.exit {
                open.extend(beside(wire.from));
            }
            if piece.entry {
                open.extend(beside(wire.to));
            }
            let (across, along) = if piece.horizontal {
                (&ys, &xs)
            } else {
                (&xs, &ys)
            };
            for line in inside(across, piece.line - reach, piece.line + reach) {
                if open.iter().any(|own| (across[line] - own).abs() < 0.5) {
                    continue;
                }
                for edge in covering(along, piece.low - reach, piece.high + reach) {
                    if piece.horizontal {
                        tight_h[line * (width - 1) + edge] = true;
                    } else {
                        tight_v[edge * width + line] = true;
                    }
                }
            }
        }
    }
    // The mouth of every other port: a line may cross in front of it but not
    // run into it or turn there, or the port's own wire has no way out. A
    // wire's own two port lines stay open to it, past the mouths of the
    // ports beside its own. Past a socket kept free for a later wire only
    // the line of the wire's port on that socket's card stays open (LAY-29:
    // an ordering line leaves along the bottom of its card and would turn
    // in the mouth of an input of the next).
    for mouth in mouths.iter().filter(|_| reach > 0.0) {
        if same(mouth.at, start) || same(mouth.at, end) {
            continue;
        }
        let (across, along, line, at, own) = if mouth.horizontal {
            (&ys, &xs, mouth.at.y, mouth.at.x, [start.y, end.y])
        } else {
            (&xs, &ys, mouth.at.x, mouth.at.y, [start.x, end.x])
        };
        let (first, second) = (mouth.block == blocks.0, mouth.block == blocks.1);
        let beside = if mouth.free {
            [first, second]
        } else {
            [first || second; 2]
        };
        for line in inside(across, line - reach, line + reach) {
            if own
                .iter()
                .zip(beside)
                .any(|(own, beside)| beside && (across[line] - own).abs() < 0.5)
            {
                continue;
            }
            // In front of a free socket two lanes stay clear, not one:
            // its wire comes back among lines that had the first pick.
            let clear = if mouth.free { reach * 2.0 } else { reach };
            for edge in covering(along, at - clear, at + clear) {
                if mouth.horizontal {
                    tight_h[line * (width - 1) + edge] = true;
                } else {
                    tight_v[edge * width + line] = true;
                }
            }
        }
    }
    // The edge that reaches `value` across a set of lines: the one `value` lies
    // in, or the one that ends on it. Routes run on the grid's lines, so a
    // wire is crossed by the edge that arrives on its line.
    let reaching = |lines: &[f32], value: f32| {
        let after = lines.partition_point(|line| *line < value - 0.01);
        (after > 0 && after < lines.len()).then(|| after - 1)
    };
    for wire in routed.iter().filter(|_| settings.crossing_cost > 0.0) {
        for (a, b) in &wire.segments {
            // Only a wire inside the window can be crossed by a route in it.
            let span = Rect::new(
                Point::new(a.x.min(b.x), a.y.min(b.y)),
                Point::new(a.x.max(b.x), a.y.max(b.y)),
            );
            if !span.overlaps(&frame.inflate(1.0)) {
                continue;
            }
            let horizontal = (a.y - b.y).abs() < 0.01;
            if horizontal {
                // A horizontal wire crosses the vertical edges under it.
                let (low, high) = (a.x.min(b.x), a.x.max(b.x));
                if let Some(y) = reaching(&ys, a.y) {
                    for x in inside(&xs, low, high) {
                        crossing_v[y * width + x] = crossing_v[y * width + x].saturating_add(1);
                    }
                }
            } else {
                let (low, high) = (a.y.min(b.y), a.y.max(b.y));
                if let Some(x) = reaching(&xs, a.x) {
                    for y in inside(&ys, low, high) {
                        crossing_h[y * (width - 1) + x] =
                            crossing_h[y * (width - 1) + x].saturating_add(1);
                    }
                }
            }
        }
    }

    // The cost of reaching a cell depends on the direction of arrival, because a
    // bend is paid where the direction changes. Node `cell * 2` arrives
    // horizontally, `cell * 2 + 1` vertically.
    const HORIZONTAL: usize = 0;
    const VERTICAL: usize = 1;

    let from_cell = cell(from.0, from.1);
    let to_cell = cell(to.0, to.1);
    let goal = at(to_cell);
    let axis_of = |side: Side| {
        if side.is_horizontal() {
            HORIZONTAL
        } else {
            VERTICAL
        }
    };
    let exit_axis = axis_of(to_side);
    // A lower bound on the cost from a cell to the goal: the orthogonal distance,
    // plus a bend for every turn the route cannot avoid. It never overestimates, so
    // the first route A\* completes is the cheapest one.
    let remaining = |cell: usize, axis: usize| {
        let here = at(cell);
        let (dx, dy) = ((goal.x - here.x).abs(), (goal.y - here.y).abs());
        // A turn is unavoidable when the goal is off both axes, and when the one
        // axis it is off is not the one the route is travelling along.
        let across = dx > 0.5 && axis == VERTICAL || dy > 0.5 && axis == HORIZONTAL;
        let mut bends = f32::from(u8::from(dx > 0.5 && dy > 0.5 || across));
        // Arriving across the exit stub is one more bend.
        if dx > 0.5 || dy > 0.5 {
            let arrival = if dx > 0.5 { HORIZONTAL } else { VERTICAL };
            if arrival != exit_axis && bends < 1.5 {
                bends += 1.0;
            }
        } else if axis != exit_axis {
            bends += 1.0;
        }
        dx + dy + bends * settings.bend_cost
    };
    let nodes = width * height * 2;
    let best = &mut scratch.best;
    let came = &mut scratch.came;
    best.clear();
    best.resize(nodes, f32::INFINITY);
    came.clear();
    came.resize(nodes, usize::MAX);
    let queue = &mut scratch.queue;
    queue.clear();
    let entry = from_cell * 2 + axis_of(from_side);
    best[entry] = 0.0;
    queue.push(Visit {
        estimate: remaining(from_cell, axis_of(from_side)),
        node: entry,
    });
    let mut reached = None;
    while let Some(visit) = queue.pop() {
        let node = visit.node;
        let (here, axis) = (node / 2, node % 2);
        let cost = best[node];
        if visit.estimate > cost + remaining(here, axis) + 0.01 {
            continue;
        }
        if here == to_cell {
            // The cost of arriving across the exit stub is already paid below, so
            // the first arrival at the goal is the cheapest one.
            reached = Some(node);
            break;
        }
        let (x, y) = (here % width, here / width);
        let steps = [
            (x > 0).then(|| {
                (
                    cell(x - 1, y),
                    HORIZONTAL,
                    blocked_h[y * (width - 1) + x - 1],
                    crossing_h[y * (width - 1) + x - 1],
                    tight_h[y * (width - 1) + x - 1],
                )
            }),
            (x + 1 < width).then(|| {
                (
                    cell(x + 1, y),
                    HORIZONTAL,
                    blocked_h[y * (width - 1) + x],
                    crossing_h[y * (width - 1) + x],
                    tight_h[y * (width - 1) + x],
                )
            }),
            (y > 0).then(|| {
                (
                    cell(x, y - 1),
                    VERTICAL,
                    blocked_v[(y - 1) * width + x],
                    crossing_v[(y - 1) * width + x],
                    tight_v[(y - 1) * width + x],
                )
            }),
            (y + 1 < height).then(|| {
                (
                    cell(x, y + 1),
                    VERTICAL,
                    blocked_v[y * width + x],
                    crossing_v[y * width + x],
                    tight_v[y * width + x],
                )
            }),
        ];
        for (next, next_axis, blocked, crossings, tight) in steps.into_iter().flatten() {
            if blocked {
                continue;
            }
            let mut bend = if axis == next_axis {
                0.0
            } else {
                settings.bend_cost
            };
            if next == to_cell && next_axis != exit_axis {
                // The exit stub leaves along its own axis, so arriving across it
                // is a corner like any other.
                bend += settings.bend_cost;
            }
            let length = at(here).orthogonal_distance(at(next));
            let step = length
                + bend
                + f32::from(crossings) * settings.crossing_cost
                + if tight { length * price } else { 0.0 };
            let candidate = cost + step;
            let target = next * 2 + next_axis;
            if candidate + 0.01 >= best[target] {
                continue;
            }
            best[target] = candidate;
            came[target] = node;
            queue.push(Visit {
                estimate: candidate + at(next).orthogonal_distance(goal),
                node: target,
            });
        }
    }

    let Some(mut node) = reached else {
        return Vec::new();
    };
    let mut points = vec![at(node / 2)];
    while came[node] != usize::MAX {
        node = came[node];
        points.push(at(node / 2));
    }
    points.reverse();
    points
}

/// A node of the search: an index into the candidate x and y lines.
type Cell = (usize, usize);

/// What one wire's search is given.
struct Problem<'a> {
    start: Point,
    end: Point,
    from_side: Side,
    to_side: Side,
    /// Cards to avoid, already inflated by the clearance in use.
    obstacles: &'a [Rect],
    /// The clearance around the cards where it is room a line pays for
    /// instead of an obstacle; empty unless Tidy up is measuring.
    zones: &'a [Rect],
    /// The wires routed before this one: crossing one costs, and running
    /// within the gap of one costs `price` per pixel.
    routed: &'a [Trace],
    price: f32,
    /// Where the stub of every port on the board ends, and the two cards
    /// this wire joins.
    mouths: &'a [Mouth],
    blocks: (usize, usize),
    /// The two ports, which tell the wires that share one.
    origin: Point,
    target: Point,
    settings: &'a Routing,
    /// The part of the board the search may use.
    window: Rect,
}

/// Buffers the search reuses between wires, so that routing a board allocates a
/// handful of times rather than once per wire.
#[derive(Default)]
struct Scratch {
    best: Vec<f32>,
    came: Vec<usize>,
    queue: BinaryHeap<Visit>,
}

/// One entry of the search frontier: a node and the cost estimate through it.
#[derive(Clone, Copy, PartialEq)]
struct Visit {
    estimate: f32,
    node: usize,
}

impl Eq for Visit {}

impl Ord for Visit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // A min-heap: the smallest estimate first, the node as a tie-break.
        // `total_cmp` is an integer comparison; the heap does thousands of these
        // per wire, which is where a router spends its time.
        other
            .estimate
            .total_cmp(&self.estimate)
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The cell of the sparse grid a point sits on.
///
/// The nearest line wins, not the first one near enough: two stub ends can be a
/// fraction of a pixel apart, and taking the first would put one of them on the
/// other's line and bend its stub.
fn cell_of(point: Point, xs: &[f32], ys: &[f32]) -> Option<Cell> {
    let nearest = |lines: &[f32], value: f32| {
        lines
            .iter()
            .enumerate()
            .min_by(|left, right| compare((left.1 - value).abs(), (right.1 - value).abs()))
            .filter(|(_, line)| (*line - value).abs() < 0.5)
            .map(|(index, _)| index)
    };
    Some((nearest(xs, point.x)?, nearest(ys, point.y)?))
}

/// Removes points that do not turn, so that a route has no redundant corners.
fn simplify(points: Vec<Point>) -> Vec<Point> {
    simplify_protecting(points, &[])
}

/// [`simplify`], but keeping the listed points even where they do not turn.
fn simplify_protecting(points: Vec<Point>, protect: &[Point]) -> Vec<Point> {
    let mut result: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if result
            .last()
            .is_some_and(|last| last.orthogonal_distance(point) < 0.01)
        {
            continue;
        }
        if let [.., before, between] = result.as_slice() {
            let straight_x =
                (before.x - between.x).abs() < 0.01 && (between.x - point.x).abs() < 0.01;
            let straight_y =
                (before.y - between.y).abs() < 0.01 && (between.y - point.y).abs() < 0.01;
            let onwards = if straight_x {
                (between.y - before.y).signum() == (point.y - between.y).signum()
            } else {
                (between.x - before.x).signum() == (point.x - between.x).signum()
            };
            let kept = protect
                .iter()
                .any(|point| point.orthogonal_distance(*between) < 0.01);
            // A collinear turn that doubles back is an excursion: dropping it
            // leaves a segment inside the one that was already there, so it cannot
            // meet a card the route did not already pass. That holds for a stub
            // end too, which is why the excursion outranks its protection.
            if (straight_x || straight_y) && (!onwards || !kept) {
                result.pop();
            }
        }
        result.push(point);
    }
    result
}

/// Where two axis-aligned segments cross, if they cross at a single interior point.
pub fn intersection(a1: Point, a2: Point, b1: Point, b2: Point) -> Option<Point> {
    let horizontal = |p: Point, q: Point| (p.y - q.y).abs() < 0.01;
    let vertical = |p: Point, q: Point| (p.x - q.x).abs() < 0.01;
    let (h1, h2, v1, v2) = if horizontal(a1, a2) && vertical(b1, b2) {
        (a1, a2, b1, b2)
    } else if vertical(a1, a2) && horizontal(b1, b2) {
        (b1, b2, a1, a2)
    } else {
        return None;
    };
    let (left, right) = (h1.x.min(h2.x), h1.x.max(h2.x));
    let (top, bottom) = (v1.y.min(v2.y), v1.y.max(v2.y));
    let crosses =
        v1.x > left + 0.01 && v1.x < right - 0.01 && h1.y > top + 0.01 && h1.y < bottom - 0.01;
    crosses.then(|| Point::new(v1.x, h1.y))
}

/// The order wires are routed in. A wire between two top ports runs on the
/// rail above the cards (LAY-06) and goes first, so that a wire routed later
/// crosses it once, where it drops into its card, rather than twice under
/// it, between the rail and the cards. Then the shorter wires first: a wire
/// between neighbours has one good route and takes it, and a long one has
/// many to choose from among the lines already there (LAY-03: measured on
/// the reference boards, a third fewer crossings than the long ones first).
fn routing_order(cards: &[Rect], wires: &[Wire]) -> Vec<usize> {
    let span = |wire: &Wire| -> f32 {
        let (Some(from), Some(to)) = (cards.get(wire.from.block), cards.get(wire.to.block)) else {
            return 0.0;
        };
        wire.from
            .point_on(from)
            .orthogonal_distance(wire.to.point_on(to))
    };
    let rail = |wire: &Wire| wire.from.side == Side::Top && wire.to.side == Side::Top;
    let mut order: Vec<usize> = (0..wires.len()).collect();
    order.sort_by(|left, right| {
        rail(&wires[*right])
            .cmp(&rail(&wires[*left]))
            .then_with(|| compare(span(&wires[*left]), span(&wires[*right])))
            .then_with(|| left.cmp(right))
    });
    order
}

/// Marks every crossing on the wire routed later, which is the one that draws it.
fn bridges(routes: &mut [Route], wires: &[Wire], cards: &[Rect]) {
    // The routing order decides who draws: the wire routed later.
    let order = routing_order(cards, wires);
    let mut found: Vec<Vec<Bridge>> = vec![Vec::new(); routes.len()];
    for (rank, &index) in order.iter().enumerate() {
        for &earlier in &order[..rank] {
            for (a1, a2) in routes[index].segments() {
                for (b1, b2) in routes[earlier].segments() {
                    if let Some(at) = intersection(a1, a2, b1, b2) {
                        found[index].push(Bridge {
                            at,
                            horizontal: (a1.y - a2.y).abs() < 0.01,
                        });
                    }
                }
            }
        }
    }
    for (route, bridges) in routes.iter_mut().zip(found) {
        route.bridges = bridges;
    }
}

/// Cards a route touches although it should not, as `(wire, card)` pairs.
///
/// A route may touch only its own two cards, and a port that lies inside a third
/// card may be left only through the route's first or last segment — the stub. Tests
/// and debug assertions use this; it is the written form of the ADR's guarantee.
pub fn violations(routes: &[Route], wires: &[Wire], cards: &[Rect]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (index, route) in routes.iter().enumerate() {
        let Some(wire) = wires.get(index) else {
            continue;
        };
        if !route.complete {
            // A wire whose ports are enclosed by cards has no route to judge; the
            // board itself is the problem and *tidy up* is the answer.
            continue;
        }
        let segments: Vec<(Point, Point)> = route.segments().collect();
        let Some(first) = segments.first() else {
            continue;
        };
        // Whether a card other than the port's own lies over the port; the
        // card at the wire's other end counts (LAY-29).
        let under = |point: Point, own: usize| {
            cards
                .iter()
                .enumerate()
                .any(|(card, rectangle)| card != own && rectangle.contains_strictly(point))
        };
        // A port inside another card is left through its stub, which is the route's
        // first or last segment, however much it has to cross to get out.
        let buried_start = under(first.0, wire.from.block);
        let buried_end = under(segments[segments.len() - 1].1, wire.to.block);
        for (card, rectangle) in cards.iter().enumerate() {
            if card == wire.from.block || card == wire.to.block {
                continue;
            }
            for (position, (from, to)) in segments.iter().enumerate() {
                if (position == 0 && buried_start) || (position + 1 == segments.len() && buried_end)
                {
                    continue;
                }
                let touched = Rect::new(
                    Point::new(from.x.min(to.x), from.y.min(to.y)),
                    Point::new(from.x.max(to.x), from.y.max(to.y)),
                );
                if rectangle.overlaps(&touched) {
                    found.push((index, card));
                    break;
                }
            }
        }
    }
    found
}

/// Wires that run under a card they join, as `(wire, card)` pairs (LAY-29).
///
/// A card is painted over its wires, so a piece of a line inside the card it
/// leaves or enters is not seen: the line then seems to come out of an edge
/// where there is no port. A line may lie on the edge of its card, where its
/// port is; it is under the card when a piece of it lies inside. A wire
/// with no route to judge (`complete` false) is left out, as in
/// [`violations`].
pub fn under_own(routes: &[Route], wires: &[Wire], cards: &[Rect]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (index, (route, wire)) in routes.iter().zip(wires).enumerate() {
        if !route.complete {
            continue;
        }
        let mut own = vec![wire.from.block];
        if wire.to.block != wire.from.block {
            own.push(wire.to.block);
        }
        for card in own {
            let Some(rectangle) = cards.get(card) else {
                continue;
            };
            // A port inside the wire's other card is left through its stub,
            // the first or the last piece, as one inside a foreign card is.
            let last = route.points.len().saturating_sub(2);
            let start_buried = card == wire.to.block
                && route
                    .points
                    .first()
                    .is_some_and(|point| rectangle.contains_strictly(*point));
            let end_buried = card == wire.from.block
                && route
                    .points
                    .last()
                    .is_some_and(|point| rectangle.contains_strictly(*point));
            let inside = route.segments().enumerate().any(|(piece, (from, to))| {
                if (piece == 0 && start_buried) || (piece == last && end_buried) {
                    return false;
                }
                from.x.min(to.x) < rectangle.max.x - 0.01
                    && from.x.max(to.x) > rectangle.min.x + 0.01
                    && from.y.min(to.y) < rectangle.max.y - 0.01
                    && from.y.max(to.y) > rectangle.min.y + 0.01
            });
            if inside {
                found.push((index, card));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    fn card(x: f32, y: f32) -> Rect {
        Rect::from_size(Point::new(x, y), Size::new(280.0, 120.0))
    }

    #[test]
    fn a_clear_wire_runs_straight() {
        let cards = [card(0.0, 0.0), card(500.0, 0.0)];
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(1, Side::Left, 60.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        assert_eq!(routes[0].points.first(), Some(&Point::new(280.0, 60.0)));
        assert_eq!(routes[0].points.last(), Some(&Point::new(500.0, 60.0)));
        assert_eq!(routes[0].bends(), 0, "{:?}", routes[0].points);
        assert!(violations(&routes, &wires, &cards).is_empty());
    }

    /// A card is painted over its own wires, so a route that crosses one is
    /// invisible and the port it left looks unconnected. The ordering wire of two
    /// cards side by side is the case that shows it: its ports face away from each
    /// other, and the short way between them is straight through the source.
    #[test]
    fn a_wire_leaves_its_port_in_the_open_instead_of_under_its_own_card() {
        let cards = [card(40.0, 100.0), card(370.0, 100.0)];
        let wires = [Wire::new(
            Port::new(0, Side::Bottom, 140.0),
            Port::new(1, Side::Top, 140.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        let points = &routes[0].points;
        let (start, next) = (points[0], points[1]);
        assert_eq!(start, Point::new(180.0, 220.0), "{points:?}");
        assert!(
            (next.x - start.x).abs() < 0.01 && next.y > start.y,
            "the wire leaves the bottom port downwards: {points:?}"
        );
        // No segment may pass through either card it joins.
        for (from, to) in routes[0].segments() {
            for rectangle in &cards {
                let crosses = from.x.min(to.x) < rectangle.max.x
                    && rectangle.min.x < from.x.max(to.x)
                    && from.y.min(to.y) < rectangle.max.y
                    && rectangle.min.y < from.y.max(to.y);
                assert!(!crosses, "{from:?} -> {to:?} crosses {rectangle:?}");
            }
        }
        assert!(violations(&routes, &wires, &cards).is_empty());
    }

    #[test]
    fn a_card_in_the_way_is_routed_around() {
        let cards = [card(0.0, 0.0), card(800.0, 0.0), card(400.0, -20.0)];
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(1, Side::Left, 60.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        assert!(routes[0].bends() >= 2, "{:?}", routes[0].points);
        assert_eq!(violations(&routes, &wires, &cards), []);
    }

    #[test]
    fn every_route_is_orthogonal_and_ends_on_its_ports() {
        let cards = [
            card(0.0, 0.0),
            card(400.0, 200.0),
            card(800.0, 0.0),
            card(400.0, -200.0),
        ];
        let wires = [
            Wire::new(
                Port::new(0, Side::Right, 60.0),
                Port::new(2, Side::Left, 60.0),
            ),
            Wire::new(
                Port::new(0, Side::Bottom, 140.0),
                Port::new(1, Side::Top, 140.0),
            ),
            Wire::new(
                Port::new(1, Side::Right, 60.0),
                Port::new(2, Side::Left, 90.0),
            ),
            Wire::new(
                Port::new(3, Side::Right, 60.0),
                Port::new(2, Side::Left, 30.0),
            ),
        ];
        let routes = route(&cards, &wires, &Routing::default());
        for (index, routed) in routes.iter().enumerate() {
            assert_eq!(
                routed.points.first(),
                Some(&wires[index].from.point_on(&cards[wires[index].from.block]))
            );
            assert_eq!(
                routed.points.last(),
                Some(&wires[index].to.point_on(&cards[wires[index].to.block]))
            );
            for (from, to) in routed.segments() {
                let horizontal = (from.y - to.y).abs() < 0.01;
                let vertical = (from.x - to.x).abs() < 0.01;
                assert!(horizontal || vertical, "{from:?} -> {to:?} is diagonal");
            }
        }
        assert_eq!(violations(&routes, &wires, &cards), []);
    }

    #[test]
    fn routing_is_deterministic() {
        let cards = [card(0.0, 0.0), card(500.0, 300.0), card(250.0, 150.0)];
        let wires = [
            Wire::new(
                Port::new(0, Side::Right, 60.0),
                Port::new(1, Side::Left, 60.0),
            ),
            Wire::new(
                Port::new(0, Side::Bottom, 140.0),
                Port::new(1, Side::Top, 140.0),
            ),
        ];
        let settings = Routing::default();
        assert_eq!(
            route(&cards, &wires, &settings),
            route(&cards, &wires, &settings)
        );
    }

    #[test]
    fn crossing_wires_get_a_bridge_on_the_wire_routed_later() {
        // A wall with one gap forces both wires through it. They connect opposite
        // corners, so inside the gap they must cross; nothing can route around.
        let wall = |top: f32, height: f32| {
            Rect::from_size(Point::new(450.0, top), Size::new(280.0, height))
        };
        let cards = [
            card(0.0, 0.0),
            card(0.0, 600.0),
            card(900.0, 0.0),
            card(900.0, 600.0),
            wall(-3000.0, 3200.0),
            wall(360.0, 3000.0),
        ];
        let wires = [
            Wire::new(
                Port::new(0, Side::Right, 60.0),
                Port::new(3, Side::Left, 60.0),
            ),
            Wire::new(
                Port::new(1, Side::Right, 60.0),
                Port::new(2, Side::Left, 60.0),
            ),
        ];
        let routes = route(&cards, &wires, &Routing::default());
        assert_eq!(violations(&routes, &wires, &cards), []);
        let crossings: usize = routes.iter().map(|route| route.bridges.len()).sum();
        assert_eq!(
            crossings,
            1,
            "exactly one wire draws the crossing: {:?}",
            routes.iter().map(|route| &route.points).collect::<Vec<_>>()
        );
        for (index, routed) in routes.iter().enumerate() {
            for bridge in &routed.bridges {
                assert!(
                    routed
                        .segments()
                        .any(|(from, to)| on_segment(bridge.at, from, to)),
                    "bridge {bridge:?} is not on wire {index}"
                );
            }
        }
    }

    fn on_segment(point: Point, from: Point, to: Point) -> bool {
        let within =
            |value: f32, a: f32, b: f32| value >= a.min(b) - 0.01 && value <= a.max(b) + 0.01;
        within(point.x, from.x, to.x) && within(point.y, from.y, to.y)
    }

    #[test]
    fn parallel_wires_in_one_channel_are_spaced_apart() {
        // Three wires from one column to another, all wanting the same channel.
        let cards = [
            card(0.0, 0.0),
            card(0.0, 200.0),
            card(0.0, 400.0),
            card(900.0, 600.0),
            card(450.0, -200.0),
        ];
        let wires = [
            Wire::new(
                Port::new(0, Side::Right, 60.0),
                Port::new(3, Side::Left, 30.0),
            ),
            Wire::new(
                Port::new(1, Side::Right, 60.0),
                Port::new(3, Side::Left, 60.0),
            ),
            Wire::new(
                Port::new(2, Side::Right, 60.0),
                Port::new(3, Side::Left, 90.0),
            ),
        ];
        let routes = route(&cards, &wires, &Routing::default());
        assert_eq!(violations(&routes, &wires, &cards), []);
        // No two interior segments may sit on exactly the same line.
        let mut lines: Vec<(bool, i64)> = Vec::new();
        for routed in &routes {
            for segment in 1..routed.points.len().saturating_sub(2) {
                let (from, to) = (routed.points[segment], routed.points[segment + 1]);
                let horizontal = (from.y - to.y).abs() < 0.01;
                lines.push((
                    horizontal,
                    if horizontal {
                        from.y.round()
                    } else {
                        from.x.round()
                    } as i64,
                ));
            }
        }
        let unique: std::collections::BTreeSet<_> = lines.iter().copied().collect();
        assert_eq!(unique.len(), lines.len(), "{lines:?}");
    }

    #[test]
    fn a_port_inside_another_card_is_left_through_its_stub_only() {
        // Overlapping cards: the target's port sits inside the third card.
        let cards = [card(0.0, 0.0), card(600.0, 0.0), card(590.0, -10.0)];
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(1, Side::Left, 60.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        assert!(!routes[0].points.is_empty());
        assert_eq!(violations(&routes, &wires, &cards), []);
    }

    #[test]
    fn order_ports_leave_downwards_and_arrive_from_above() {
        let cards = [card(0.0, 0.0), card(0.0, 400.0)];
        let wires = [Wire::new(
            Port::new(0, Side::Bottom, 140.0),
            Port::new(1, Side::Top, 140.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        assert_eq!(routes[0].bends(), 0, "{:?}", routes[0].points);
        assert_eq!(routes[0].points.first(), Some(&Point::new(140.0, 120.0)));
        assert_eq!(routes[0].points.last(), Some(&Point::new(140.0, 400.0)));
    }

    /// LAY-06: an ordering wire between the tops of two cards of a row runs
    /// on the rail above them, and a data wire dropping into the row from
    /// above crosses it once, where it comes down, not twice under it.
    #[test]
    fn a_wire_between_two_tops_runs_on_the_rail_and_is_crossed_once() {
        let cards = [card(0.0, 300.0), card(400.0, 300.0), card(800.0, 0.0)];
        let wires = [
            Wire::new(
                Port::new(2, Side::Right, 60.0),
                Port::new(1, Side::Left, 60.0),
            ),
            Wire::new(
                Port::new(0, Side::Top, 140.0),
                Port::new(1, Side::Top, 140.0),
            ),
        ];
        let routes = route(&cards, &wires, &Routing::default());
        assert_eq!(violations(&routes, &wires, &cards), []);
        let rail = &routes[1];
        assert!(
            rail.points.iter().all(|point| point.y <= 300.0),
            "the rail stays above the row: {:?}",
            rail.points
        );
        assert_eq!(rail.bends(), 2, "up, along and down: {:?}", rail.points);
        let crossings: usize = routes.iter().map(|route| route.bridges.len()).sum();
        assert_eq!(crossings, 1, "{routes:?}");
    }

    #[test]
    fn a_wire_backwards_through_a_wall_of_cards_still_avoids_them() {
        // The target is left of the source, with a column of cards between them.
        let mut cards = vec![card(900.0, 0.0), card(0.0, 0.0)];
        for row in 0..4 {
            cards.push(card(450.0, row as f32 * 160.0 - 160.0));
        }
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(1, Side::Left, 60.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        assert!(routes[0].points.len() > 2);
        assert_eq!(violations(&routes, &wires, &cards), []);
    }

    #[test]
    fn a_port_with_no_room_for_the_clearance_hugs_the_card_instead_of_crossing_it() {
        // The third card leaves a 2 px gap in front of the port, far less than the
        // clearance. The wire must still go around it.
        let cards = [
            card(0.0, 0.0),
            card(900.0, 0.0),
            Rect::from_size(Point::new(282.0, -200.0), Size::new(280.0, 400.0)),
        ];
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(1, Side::Left, 60.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        assert!(routes[0].complete, "{:?}", routes[0].points);
        assert_eq!(violations(&routes, &wires, &cards), []);
    }

    #[test]
    fn a_wire_whose_ports_are_walled_in_is_marked_incomplete() {
        // A closed ring of cards around the source: no route exists at all.
        let wall = |x: f32, y: f32, width: f32, height: f32| {
            Rect::from_size(Point::new(x, y), Size::new(width, height))
        };
        let cards = [
            card(0.0, 0.0),
            card(1200.0, 0.0),
            // The walls overlap at the corners, so the ring has no seam to slip
            // through: two cards that only touch share a border a wire may run on.
            wall(-100.0, -200.0, 700.0, 180.0),
            wall(-100.0, 150.0, 700.0, 180.0),
            wall(-200.0, -200.0, 180.0, 530.0),
            wall(380.0, -200.0, 180.0, 530.0),
        ];
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(1, Side::Left, 60.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        assert!(!routes[0].complete);
        // The wire is still drawn, orthogonally, so the user sees the connection.
        for (from, to) in routes[0].segments() {
            assert!((from.x - to.x).abs() < 0.01 || (from.y - to.y).abs() < 0.01);
        }
        // An incomplete route is not judged against the guarantee.
        assert_eq!(violations(&routes, &wires, &cards), []);
    }

    #[test]
    fn a_route_never_doubles_back_onto_a_shorter_segment() {
        let cards = [card(0.0, 0.0), card(600.0, 0.0), card(300.0, -10.0)];
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(1, Side::Left, 60.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        // Consecutive segments on one axis must continue in the same direction.
        for pair in routes[0].points.windows(3) {
            let [a, b, c] = [pair[0], pair[1], pair[2]];
            let straight_x = (a.x - b.x).abs() < 0.01 && (b.x - c.x).abs() < 0.01;
            let straight_y = (a.y - b.y).abs() < 0.01 && (b.y - c.y).abs() < 0.01;
            assert!(
                !straight_x && !straight_y,
                "redundant corner in {:?}",
                routes[0].points
            );
        }
    }

    #[test]
    fn a_wire_to_a_missing_card_is_empty_rather_than_a_panic() {
        let cards = [card(0.0, 0.0)];
        let wires = [Wire::new(
            Port::new(0, Side::Right, 60.0),
            Port::new(9, Side::Left, 60.0),
        )];
        let routes = route(&cards, &wires, &Routing::default());
        assert_eq!(routes[0], Route::default());
    }

    #[test]
    fn intersections_ignore_touching_and_parallel_segments() {
        let a = (Point::new(0.0, 0.0), Point::new(100.0, 0.0));
        // Parallel.
        assert_eq!(
            intersection(a.0, a.1, Point::new(0.0, 50.0), Point::new(100.0, 50.0)),
            None
        );
        // Touching at an end, which is a corner rather than a crossing.
        assert_eq!(
            intersection(a.0, a.1, Point::new(100.0, 0.0), Point::new(100.0, 50.0)),
            None
        );
        assert_eq!(
            intersection(a.0, a.1, Point::new(50.0, -20.0), Point::new(50.0, 20.0)),
            Some(Point::new(50.0, 0.0))
        );
    }
}
