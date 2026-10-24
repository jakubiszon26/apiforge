//! The board's wires: routed polylines, and the cache that keeps them.
//!
//! The geometry comes from `apiforge-layout`; this module only translates cards and
//! wires into its rectangles and ports and keeps the results between frames. A route
//! depends on nothing but the card rectangles and the two ports, so an unchanged
//! board reuses its routes and a drag reroutes the wires of the moved cards only.
//!
//! While a drag exceeds its frame budget the moved wires are drawn as direct curves,
//! as [ADR 0006](../../../../docs/decisions/0006-layout-and-wire-routing.md)
//! prescribes, and the first idle frame routes the whole board again.

use super::layout::{Card, Chip};
use apiforge_core::model::Branch;
use apiforge_layout as layout;
use eframe::egui::{Pos2, Rect, pos2};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

/// How wires are drawn. A user setting, not part of the document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WireStyle {
    /// Right angles, with a bridge where two wires cross.
    #[default]
    Orthogonal,
    /// The same route with rounded corners.
    Spline,
}

/// What a frame may spend on rerouting the wires of dragged cards.
const DRAG_BUDGET: Duration = Duration::from_millis(8);

/// A wire ready to draw.
#[derive(Clone, Debug)]
pub struct Routed {
    pub id: String,
    /// The route in board coordinates. Empty when the wire has no route at all,
    /// for example because a card is missing.
    pub points: Vec<Pos2>,
    /// The route with its corners rounded, which is what the board draws.
    pub pieces: Vec<layout::Piece>,
    /// Where this wire crosses one routed before it.
    pub bridges: Vec<(Pos2, bool)>,
    pub control: bool,
    pub branch: Option<Branch>,
    /// False while the wire is drawn as a direct curve: the board was mid-drag and
    /// over budget, or the cards enclose the ports so that no route exists.
    pub routed: bool,
}

impl Routed {
    /// A wire the board draws as a direct curve between its ports.
    fn direct(id: String, start: Pos2, end: Pos2, control: bool, branch: Option<Branch>) -> Self {
        Self {
            id,
            points: vec![start, end],
            pieces: Vec::new(),
            bridges: Vec::new(),
            control,
            branch,
            routed: false,
        }
    }
}

/// Where a wire starts: an output of a card, or a proxy chip standing for a
/// variable, parameter or environment value.
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
    Output { card: usize, port: String },
    Chip(usize),
}

/// A wire as the board draws it. The document says which consumer reads what; the
/// board decides where the wire starts, which is the only thing a proxy changes.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardWire {
    pub id: String,
    pub start: Start,
    /// The consumer, as an index into the cards.
    pub target: usize,
    pub input: String,
    pub operand_path: Vec<String>,
    /// An ordering wire, drawn dashed between control ports.
    pub control: bool,
    pub branch: Option<Branch>,
}

/// What the board asks for.
pub struct Request<'a> {
    pub cards: &'a [Card],
    /// Proxy chips: wire starts, and obstacles like any card.
    pub chips: &'a [Chip],
    pub wires: &'a [BoardWire],
    /// Cards and chips the user is dragging; their wires are the only ones rerouted.
    pub moving: &'a HashSet<String>,
    pub style: WireStyle,
    pub bridges: bool,
}

impl Request<'_> {
    /// Rectangles are indexed by card and then by chip, so one index addresses
    /// everything a wire can start from or has to avoid.
    fn block_id(&self, index: usize) -> &str {
        match self.cards.get(index) {
            Some(card) => card.id.as_str(),
            None => self.chips[index - self.cards.len()].id(),
        }
    }
}

/// Keeps the routes of one board between frames.
#[derive(Default)]
pub struct Router {
    routes: Vec<Routed>,
    /// The geometry the routes were computed from, or `None` when they are stale.
    key: Option<Key>,
}

/// Everything a set of routes depends on.
#[derive(Clone, PartialEq)]
struct Key {
    blocks: Vec<(String, [i32; 4])>,
    wires: Vec<(String, [i32; 4])>,
    style: WireStyle,
    bridges: bool,
}

/// Rounded coordinates: a sub-pixel difference cannot change a route.
fn quantise(rect: Rect) -> [i32; 4] {
    [
        rect.min.x.round() as i32,
        rect.min.y.round() as i32,
        rect.max.x.round() as i32,
        rect.max.y.round() as i32,
    ]
}

impl Router {
    /// Routes for the board, computed or reused.
    pub fn routes(&mut self, request: &Request<'_>) -> &[Routed] {
        let prepared = prepare(request);
        let key = Key {
            blocks: request
                .cards
                .iter()
                .map(|card| (card.id.clone(), quantise(card.rect())))
                .chain(
                    request
                        .chips
                        .iter()
                        .map(|chip| (chip.id().to_owned(), quantise(chip.rect()))),
                )
                .collect(),
            wires: prepared
                .iter()
                .map(|wire| {
                    (
                        wire.id.clone(),
                        [
                            wire.start.x.round() as i32,
                            wire.start.y.round() as i32,
                            wire.end.x.round() as i32,
                            wire.end.y.round() as i32,
                        ],
                    )
                })
                .collect(),
            style: request.style,
            bridges: request.bridges,
        };
        if request.moving.is_empty() {
            if self.key.as_ref() == Some(&key) {
                return &self.routes;
            }
            self.routes = route_all(request, &prepared, &key);
            self.key = Some(key);
            return &self.routes;
        }

        // A drag only moves cards, so the wires of the cards that stayed keep their
        // routes. Rerouting the rest within the frame budget keeps the board
        // responsive; whatever is left over is drawn directly for this frame.
        let same_wires = self
            .key
            .as_ref()
            .is_some_and(|previous| wire_ids(&previous.wires) == wire_ids(&key.wires));
        if !same_wires || self.routes.len() != prepared.len() {
            self.routes = route_all(request, &prepared, &key);
        } else {
            reroute_moving(request, &prepared, &mut self.routes);
        }
        // The board is mid-drag, so the cached routes no longer match the cards.
        // Dropping the key makes the next idle frame route everything again.
        self.key = None;
        &self.routes
    }
}

fn wire_ids(wires: &[(String, [i32; 4])]) -> Vec<&String> {
    wires.iter().map(|(id, _)| id).collect()
}

/// A wire translated into the router's terms.
struct Prepared {
    id: String,
    wire: layout::Wire,
    start: Pos2,
    end: Pos2,
    control: bool,
    branch: Option<Branch>,
}

/// Translates the board's cards, chips and wires into rectangles and ports.
fn prepare(request: &Request<'_>) -> Vec<Prepared> {
    request
        .wires
        .iter()
        .filter_map(|wire| {
            let target = request.cards.get(wire.target)?;
            let end = target.input_anchor(&wire.input, &wire.operand_path)?;
            let (source, start, source_rect) = match &wire.start {
                Start::Output { card, port } => {
                    let source = request.cards.get(*card)?;
                    (*card, source.output_anchor(port)?, source.rect())
                }
                Start::Chip(index) => {
                    let chip = request.chips.get(*index)?;
                    (
                        request.cards.len() + index,
                        chip.output_anchor(),
                        chip.rect(),
                    )
                }
            };
            Some(Prepared {
                id: wire.id.clone(),
                wire: layout::Wire::new(
                    port_at(source, start, source_rect),
                    port_at(wire.target, end, target.rect()),
                ),
                start,
                end,
                control: wire.control,
                branch: wire.branch,
            })
        })
        .collect()
}

/// The port a card anchor sits on, taken from which side of the card it touches.
fn port_at(block: usize, anchor: Pos2, card: Rect) -> layout::Port {
    let distances = [
        ((anchor.x - card.left()).abs(), layout::Side::Left),
        ((card.right() - anchor.x).abs(), layout::Side::Right),
        ((anchor.y - card.top()).abs(), layout::Side::Top),
        ((card.bottom() - anchor.y).abs(), layout::Side::Bottom),
    ];
    let side = distances
        .iter()
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map_or(layout::Side::Right, |(_, side)| *side);
    let offset = if side.is_horizontal() {
        anchor.y - card.top()
    } else {
        anchor.x - card.left()
    };
    layout::Port::new(block, side, offset)
}

/// Cards first, then chips: one index space for ports and obstacles.
fn rectangles(request: &Request<'_>) -> Vec<layout::Rect> {
    request
        .cards
        .iter()
        .map(Card::rect)
        .chain(request.chips.iter().map(Chip::rect))
        .map(|rect| {
            layout::Rect::new(
                layout::Point::new(rect.min.x, rect.min.y),
                layout::Point::new(rect.max.x, rect.max.y),
            )
        })
        .collect()
}

fn route_all(request: &Request<'_>, prepared: &[Prepared], key: &Key) -> Vec<Routed> {
    let cards = rectangles(request);
    let wires: Vec<layout::Wire> = prepared.iter().map(|wire| wire.wire).collect();
    let routes = layout::route(&cards, &wires, &layout::Routing::default());
    let _ = key;
    prepared
        .iter()
        .zip(routes)
        .map(|(wire, route)| finish(request, &cards, wire, route))
        .collect()
}

/// Reroutes the wires of the dragged cards in place, within the frame budget.
fn reroute_moving(request: &Request<'_>, prepared: &[Prepared], routes: &mut [Routed]) {
    let cards = rectangles(request);
    let started = Instant::now();
    let mut over_budget = false;
    for (index, wire) in prepared.iter().enumerate() {
        let touched = request
            .moving
            .contains(request.block_id(wire.wire.from.block))
            || request
                .moving
                .contains(request.block_id(wire.wire.to.block));
        if !touched {
            continue;
        }
        if over_budget || started.elapsed() > DRAG_BUDGET {
            over_budget = true;
            routes[index] = Routed::direct(
                wire.id.clone(),
                wire.start,
                wire.end,
                wire.control,
                wire.branch,
            );
            continue;
        }
        let routed = layout::route(&cards, &[wire.wire], &layout::Routing::default());
        routes[index] = match routed.into_iter().next() {
            Some(route) => finish(request, &cards, wire, route),
            None => Routed::direct(
                wire.id.clone(),
                wire.start,
                wire.end,
                wire.control,
                wire.branch,
            ),
        };
    }
}

/// Turns one layout route into a drawable wire.
fn finish(
    request: &Request<'_>,
    cards: &[layout::Rect],
    wire: &Prepared,
    route: layout::Route,
) -> Routed {
    if route.points.len() < 2 || !route.complete {
        // No route: the ports are enclosed, or a card went missing between frames.
        // A direct curve keeps the connection visible.
        return Routed::direct(
            wire.id.clone(),
            wire.start,
            wire.end,
            wire.control,
            wire.branch,
        );
    }
    let obstacles: Vec<layout::Rect> = cards
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != wire.wire.from.block && *index != wire.wire.to.block)
        .map(|(_, card)| card.inflate(layout::Routing::default().margin))
        .collect();
    let radius = match request.style {
        WireStyle::Orthogonal => 0.0,
        WireStyle::Spline => layout::DEFAULT_RADIUS,
    };
    Routed {
        id: wire.id.clone(),
        points: route
            .points
            .iter()
            .map(|point| pos2(point.x, point.y))
            .collect(),
        pieces: layout::smooth(&route, radius, &obstacles),
        bridges: if request.bridges {
            route
                .bridges
                .iter()
                .map(|bridge| (pos2(bridge.at.x, bridge.at.y), bridge.horizontal))
                .collect()
        } else {
            Vec::new()
        },
        control: wire.control,
        branch: wire.branch,
        routed: true,
    }
}

/// The distance from a point to a segment, the usual projection.
pub fn distance_to_segment(point: Pos2, from: Pos2, to: Pos2) -> f32 {
    let line = to - from;
    let length = line.length_sq();
    if length <= f32::EPSILON {
        return point.distance(from);
    }
    let t = ((point - from).dot(line) / length).clamp(0.0, 1.0);
    point.distance(from + line * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    fn card(x: f32, y: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), vec2(280.0, 120.0))
    }

    /// Every board of the fixtures, routed through the desktop's own conversion:
    /// the guarantee has to hold for the cards the application really draws, not
    /// only for the rectangles the layout crate is tested with.
    #[test]
    fn fixture_boards_route_without_crossing_a_card() {
        let workspace = apiforge_core::import_document(
            include_bytes!("../../../../fixtures/workspace.json"),
            &apiforge_core::ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        let mut boards = 0;
        for project in &workspace.projects {
            for scenario in &project.scenarios {
                for graph in [&scenario.graph, &scenario.cleanup] {
                    let cards: Vec<Card> = graph
                        .nodes
                        .iter()
                        .map(|node| Card::build(node, graph, project))
                        .collect();
                    if cards.len() < 2 {
                        continue;
                    }
                    boards += 1;
                    let chips: Vec<Chip> = Vec::new();
                    let board_wires = super::super::board_wires(graph, &cards, &chips);
                    let mut router = Router::default();
                    let request = Request {
                        cards: &cards,
                        chips: &chips,
                        wires: &board_wires,
                        moving: &HashSet::new(),
                        style: WireStyle::Orthogonal,
                        bridges: true,
                    };
                    let first: Vec<(Vec<Pos2>, bool)> = router
                        .routes(&request)
                        .iter()
                        .map(|route| (route.points.clone(), route.routed))
                        .collect();
                    let prepared = prepare(&request);
                    let rectangles = rectangles(&request);
                    let wires: Vec<layout::Wire> = prepared.iter().map(|wire| wire.wire).collect();
                    let drawn: Vec<layout::Route> = first
                        .iter()
                        .map(|(points, routed)| layout::Route {
                            points: points
                                .iter()
                                .map(|point| layout::Point::new(point.x, point.y))
                                .collect(),
                            bridges: Vec::new(),
                            complete: *routed,
                        })
                        .collect();
                    assert_eq!(
                        layout::violations(&drawn, &wires, &rectangles),
                        [],
                        "{}: a wire of {} crosses a card",
                        project.name,
                        scenario.name
                    );
                    // The same board a second time gives the same routes, which is
                    // what makes the cache safe to trust.
                    let again: Vec<(Vec<Pos2>, bool)> = router
                        .routes(&request)
                        .iter()
                        .map(|route| (route.points.clone(), route.routed))
                        .collect();
                    assert_eq!(again, first);
                }
            }
        }
        assert!(boards > 0, "the fixtures have boards to route");
    }

    #[test]
    fn a_port_is_placed_on_the_side_it_touches() {
        let rect = card(100.0, 50.0);
        let right = port_at(0, pos2(380.0, 90.0), rect);
        assert_eq!(right.side, layout::Side::Right);
        assert_eq!(right.offset, 40.0);
        let left = port_at(1, pos2(100.0, 90.0), rect);
        assert_eq!(left.side, layout::Side::Left);
        let top = port_at(2, pos2(240.0, 50.0), rect);
        assert_eq!(top.side, layout::Side::Top);
        assert_eq!(top.offset, 140.0);
        let bottom = port_at(3, pos2(240.0, 170.0), rect);
        assert_eq!(bottom.side, layout::Side::Bottom);
    }

    #[test]
    fn distance_to_a_polyline_follows_its_segments() {
        let routed = Routed {
            id: "w".into(),
            points: vec![pos2(0.0, 0.0), pos2(100.0, 0.0), pos2(100.0, 100.0)],
            pieces: Vec::new(),
            bridges: Vec::new(),
            control: false,
            branch: None,
            routed: true,
        };
        let distance = |point: Pos2| {
            routed
                .points
                .windows(2)
                .map(|pair| distance_to_segment(point, pair[0], pair[1]))
                .fold(f32::INFINITY, f32::min)
        };
        assert_eq!(distance(pos2(50.0, 5.0)), 5.0);
        assert_eq!(distance(pos2(105.0, 50.0)), 5.0);
        // A point beside the corner, not between the two segments.
        assert!(distance(pos2(-10.0, 0.0)) - 10.0 < 0.01);
    }
}
