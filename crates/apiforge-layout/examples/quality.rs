//! Says how readable the boards are, before and after a layout ticket
//! ([LAY-01](../../../docs/tickets/LAY-01.md)).
//!
//! ```text
//! cargo run -p apiforge-layout --example quality --release --locked
//! cargo run -p apiforge-layout --example quality --release --locked -- --dump <seed> <cards>
//! ```
//!
//! It prints two tables: generated scenario-like boards of 6, 12, 25 and 50 cards,
//! with and without an ordering chain, and the reference scenarios of
//! `fixtures/scenarios/lay/`, with every step a full card, as *Tidy up* leaves them
//! on `main`. The numbers are deterministic. `--dump` prints one generated board's
//! rectangles and routes, for reading a case by hand.
//!
//! The columns:
//!
//! - `wires`, `crossings` (the router's bridges), `bends`, `straight` (wires without
//!   a bend), `incomplete` (routes that found no way around the cards);
//! - `close`: pairs of parallel segments of different wires within 8 px of each
//!   other for at least 20 px; `coincident`: the same within 0.5 px, counted there
//!   and not under `close`;
//! - `long`: edges spanning more than one layer;
//! - `width`, `height`: the board's bounds; `fit_1600`, `fit_960`: the zoom that
//!   shows all of it at 1600 × 1000 and 960 × 540, not limited to the zoom the
//!   window allows;
//! - `card_hits`: pairs of cards that overlap; `chip_hits`: pairs with a proxy chip
//!   in them. A scenario read from code has no chips yet: they are placed by hand
//!   and live in the document, so `chips` says how many were measured;
//! - `clearance`: route segments closer than 12 px to a card or chip the wire does
//!   not end on, the router's `Routing::margin`.
//!
//! The card geometry is the desktop's (`canvas/layout.rs`), which this crate cannot
//! depend on. A test holds it against heights the window measured.

mod support;

use apiforge_core::{
    demos::shop_project,
    format::{Binding, Graph as Steps, Node, Project, ProxySource},
    graph::{
        FALSE_PORT, ORDER_PORT, TRUE_PORT, VALUE_INPUT, Wire as Link, WireKind, input_slots,
        output_ports, predicate, wires,
    },
    model::{Branch, NodeKind},
};
use apiforge_layout::{
    Block, Edge, Graph, Point, Port, Rect, Route, Routing, Side, Size, Wire, layers_of, route, tidy,
};
use apiforge_script::{Document, LowerContext, lower, parse};
use std::{fmt::Write, process::ExitCode};
use support::Random;

const WIDTH: f32 = 280.0;
const HEADER: f32 = 64.0;
const SUBTITLE: f32 = 24.0;
const INPUT_ROW: f32 = 52.0;
const OUTPUT_ROW: f32 = 28.0;
const FOOTER: f32 = 36.0;
const CONTROL: f32 = 28.0;
const CHIP_HEIGHT: f32 = 36.0;

/// Parallel segments this near each other, for this long, read as one line.
const CLOSE: f32 = 8.0;
const COINCIDENT: f32 = 0.5;
const ALONGSIDE: f32 = 20.0;
/// The router keeps exactly its margin; half a pixel forgives the sampling below.
const CLEARANCE: f32 = 11.5;

const SIZES: [usize; 4] = [6, 12, 25, 50];
const SEEDS: u64 = 10;

const SCENARIOS: [(&str, &str); 3] = [
    (
        "order-flow",
        include_str!("../../../fixtures/scenarios/lay/order-flow.afs"),
    ),
    (
        "shop-30",
        include_str!("../../../fixtures/scenarios/lay/shop-30.afs"),
    ),
    (
        "shop-50",
        include_str!("../../../fixtures/scenarios/lay/shop-50.afs"),
    ),
];

/// One input row of a card: its input and the path of its slot inside it.
type Row = (String, Vec<String>);

/// A card as the desktop measures it.
struct Card {
    subtitle: bool,
    inputs: Vec<Row>,
    outputs: Vec<String>,
    controls: Vec<&'static str>,
}

impl Card {
    fn generated(inputs: usize, outputs: usize) -> Self {
        Self {
            subtitle: false,
            inputs: vec![Row::default(); inputs],
            outputs: vec![String::new(); outputs],
            controls: vec![ORDER_PORT],
        }
    }

    fn of(node: &Node, project: &Project) -> Self {
        let inputs = input_slots(node, project)
            .into_iter()
            .filter(|slot| {
                slot.input != "expected"
                    || !matches!(node.kind, NodeKind::Assert | NodeKind::If)
                    || predicate(node).takes_expected()
            })
            .map(|slot| (slot.input, slot.path))
            .collect();
        Self {
            subtitle: has_subtitle(node),
            inputs,
            outputs: output_ports(node, project)
                .into_iter()
                .map(|port| port.name)
                .collect(),
            controls: match node.kind {
                NodeKind::If => vec![TRUE_PORT, FALSE_PORT],
                _ => vec![ORDER_PORT],
            },
        }
    }

    fn header(&self) -> f32 {
        HEADER + if self.subtitle { SUBTITLE } else { 0.0 }
    }

    fn outputs_top(&self) -> f32 {
        self.header() + self.inputs.len() as f32 * INPUT_ROW
    }

    fn height(&self) -> f32 {
        self.outputs_top() + self.outputs.len() as f32 * OUTPUT_ROW + 4.0 + FOOTER + CONTROL
    }

    fn input_y(&self, index: usize) -> f32 {
        self.header() + index as f32 * INPUT_ROW + 30.0
    }

    fn output_y(&self, index: usize) -> f32 {
        self.outputs_top() + index as f32 * OUTPUT_ROW + OUTPUT_ROW / 2.0 + 2.0
    }

    /// Where a wire ending at `input` lands: the deepest row containing `path`.
    fn input_port(&self, block: usize, input: &str, path: &[String]) -> Option<Port> {
        if input == ORDER_PORT {
            return Some(Port::new(block, Side::Top, WIDTH / 2.0));
        }
        let index = self
            .inputs
            .iter()
            .enumerate()
            .filter(|(_, row)| row.0 == input && path.starts_with(&row.1))
            .max_by_key(|(_, row)| row.1.len())?
            .0;
        Some(Port::new(block, Side::Left, self.input_y(index)))
    }

    /// Where a wire leaves: a data output, or the middle of a control button.
    fn output_port(&self, block: usize, output: &str) -> Option<Port> {
        if let Some(index) = self.controls.iter().position(|port| *port == output) {
            let width = WIDTH / self.controls.len() as f32;
            let offset = (index as f32 + 0.5) * width;
            return Some(Port::new(block, Side::Bottom, offset));
        }
        let index = self.outputs.iter().position(|name| name == output)?;
        Some(Port::new(block, Side::Right, self.output_y(index)))
    }
}

/// Whether the desktop writes a line under a card's name.
fn has_subtitle(node: &Node) -> bool {
    match node.kind {
        NodeKind::Request | NodeKind::Condition | NodeKind::Assert | NodeKind::If => true,
        NodeKind::Value => match node.inputs.get(VALUE_INPUT) {
            Some(Binding::Reference { path, .. } | Binding::Parameter { path, .. })
                if !path.is_empty() =>
            {
                true
            }
            Some(
                Binding::Expression { .. }
                | Binding::Object { .. }
                | Binding::Array { .. }
                | Binding::Template { .. },
            ) => true,
            _ => node.is_variable(),
        },
        _ => false,
    }
}

/// What the layout and the router work on.
#[derive(Default)]
struct Board {
    blocks: Vec<Block>,
    /// Pinned obstacles after the cards: one index space for ports and obstacles.
    chips: Vec<Block>,
    edges: Vec<Edge>,
    wires: Vec<Wire>,
}

/// A scenario-like board: a chain of ordering edges with a few branches, and data
/// wires from recent predecessors into the inputs.
fn generated(seed: u64, count: usize, order: bool) -> Board {
    let mut random = Random(seed * 2 + 1);
    let cards: Vec<Card> = (0..count)
        .map(|_| Card::generated(1 + random.below(3), 1 + random.below(2)))
        .collect();
    let mut board = Board {
        blocks: cards
            .iter()
            .enumerate()
            .map(|(index, card)| {
                let start = Point::new(index as f32 * 10.0, index as f32 * 7.0);
                Block::new(Size::new(WIDTH, card.height()), start)
            })
            .collect(),
        ..Board::default()
    };
    for target in 1..count {
        // Ordering: usually from the previous card, sometimes a branch from two back.
        let source = if random.chance(80) || target < 2 {
            target - 1
        } else {
            target - 2
        };
        if order {
            board.edges.push(Edge::new(source, target));
            board.wires.push(Wire::new(
                Port::new(source, Side::Bottom, WIDTH / 2.0),
                Port::new(target, Side::Top, WIDTH / 2.0),
            ));
        }
        // Data: each input is fed by one of the last four cards, with 70 % chance.
        for input in 0..cards[target].inputs.len() {
            if !random.chance(70) {
                continue;
            }
            let source = target - (1 + random.below(4.min(target)));
            let output = random.below(cards[source].outputs.len());
            let (from, to) = (cards[source].output_y(output), cards[target].input_y(input));
            board
                .edges
                .push(Edge::new(source, target).with_offsets(from, to));
            board.wires.push(Wire::new(
                Port::new(source, Side::Right, from),
                Port::new(target, Side::Left, to),
            ));
        }
    }
    board
}

/// The output and the input a link connects, as the desktop's board reads it.
fn ports(link: &Link) -> (&str, &str) {
    match link.kind {
        WireKind::Order => (
            match link.branch {
                Some(Branch::True) => TRUE_PORT,
                Some(Branch::False) => FALSE_PORT,
                None => ORDER_PORT,
            },
            ORDER_PORT,
        ),
        WireKind::Data => (
            link.source_port.as_deref().unwrap_or_default(),
            link.target_input.as_deref().unwrap_or_default(),
        ),
    }
}

/// A proxy's width follows its label, as on the desktop.
fn chip_label<'a>(source: &'a ProxySource, steps: &'a Steps) -> &'a str {
    match source {
        ProxySource::Variable { node_id } => steps
            .nodes
            .iter()
            .find(|node| &node.id == node_id)
            .map_or(node_id, Node::display_name),
        ProxySource::Parameter { name } | ProxySource::Environment { name } => name,
    }
}

/// The board of a scenario's main graph, in the demo shop project it calls into.
fn reference(steps: &Steps, project: &Project) -> Board {
    let cards: Vec<Card> = steps
        .nodes
        .iter()
        .map(|node| Card::of(node, project))
        .collect();
    let index_of = |id: &str| steps.nodes.iter().position(|node| node.id == id);
    let links = wires(steps);
    Board {
        blocks: steps
            .nodes
            .iter()
            .zip(&cards)
            .map(|(node, card)| {
                let position = Point::new(node.position.x as f32, node.position.y as f32);
                Block::new(Size::new(WIDTH, card.height()), position)
            })
            .collect(),
        chips: steps
            .proxies
            .iter()
            .map(|proxy| {
                let label = chip_label(&proxy.source, steps).chars().count();
                let width = (7.2 * label as f32 + 64.0).clamp(112.0, 240.0);
                let position = Point::new(proxy.position.x as f32, proxy.position.y as f32);
                Block::new(Size::new(width, CHIP_HEIGHT), position).pinned()
            })
            .collect(),
        edges: links
            .iter()
            .filter_map(|link| Some(Edge::new(index_of(&link.source)?, index_of(&link.target)?)))
            .collect(),
        wires: links
            .iter()
            .filter_map(|link| {
                let (source, target) = (index_of(&link.source)?, index_of(&link.target)?);
                let (output, input) = ports(link);
                Some(Wire::new(
                    cards[source].output_port(source, output)?,
                    cards[target].input_port(target, input, &link.operand_path)?,
                ))
            })
            .collect(),
    }
}

fn scenario(text: &str, project: &Project) -> Result<Steps, String> {
    let lowered = lower(&parse(text), &LowerContext::folder(project));
    match lowered.document {
        Some(Document::Scenario(scenario)) if !lowered.has_errors() => Ok(scenario.graph),
        _ => Err(format!("{:#?}", lowered.diagnostics)),
    }
}

/// The board tidied and routed.
struct Laid {
    cards: Vec<Rect>,
    chips: Vec<Rect>,
    routes: Vec<Route>,
    long: usize,
}

fn lay(board: &Board) -> Laid {
    let blocks: Vec<Block> = board.blocks.iter().chain(&board.chips).copied().collect();
    let graph = Graph::new(&blocks, &board.edges);
    let layers = layers_of(&graph);
    let rectangles: Vec<Rect> = tidy(&graph)
        .iter()
        .zip(&blocks)
        .map(|(position, block)| Rect::from_size(*position, block.size))
        .collect();
    let routes = route(&rectangles, &board.wires, &Routing::default());
    let (cards, chips) = rectangles.split_at(board.blocks.len());
    Laid {
        cards: cards.to_vec(),
        chips: chips.to_vec(),
        routes,
        long: board
            .edges
            .iter()
            .filter(|edge| layers[edge.target] > layers[edge.source] + 1)
            .count(),
    }
}

/// How far apart two parallel segments run, and for how long side by side.
/// `None` when they are not parallel or one of them is not axis-aligned.
fn alongside(a: (Point, Point), b: (Point, Point)) -> Option<(f32, f32)> {
    let horizontal = |(from, to): (Point, Point)| (from.y - to.y).abs() < 0.01;
    let vertical = |(from, to): (Point, Point)| (from.x - to.x).abs() < 0.01;
    let overlap = |a1: f32, a2: f32, b1: f32, b2: f32| {
        a1.max(a2).min(b1.max(b2)) - a1.min(a2).max(b1.min(b2))
    };
    if horizontal(a) && horizontal(b) {
        Some(((a.0.y - b.0.y).abs(), overlap(a.0.x, a.1.x, b.0.x, b.1.x)))
    } else if vertical(a) && vertical(b) {
        Some(((a.0.x - b.0.x).abs(), overlap(a.0.y, a.1.y, b.0.y, b.1.y)))
    } else {
        None
    }
}

/// Close and coincident pairs of segments, each pair from two different wires.
fn close_pairs(routes: &[Route]) -> (usize, usize) {
    let (mut close, mut coincident) = (0, 0);
    for (index, a) in routes.iter().enumerate() {
        for b in &routes[index + 1..] {
            for first in a.segments() {
                for second in b.segments() {
                    match alongside(first, second) {
                        Some((gap, length)) if length >= ALONGSIDE && gap < COINCIDENT => {
                            coincident += 1;
                        }
                        Some((gap, length)) if length >= ALONGSIDE && gap <= CLOSE => close += 1,
                        _ => {}
                    }
                }
            }
        }
    }
    (close, coincident)
}

fn overlap(a: &Rect, b: &Rect) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

/// Pairs of overlapping cards, and of overlapping things with a chip among them.
fn collisions(cards: &[Rect], chips: &[Rect]) -> (usize, usize) {
    let pairs = |rectangles: &[Rect], others: &[Rect]| {
        rectangles
            .iter()
            .map(|a| others.iter().filter(|b| overlap(a, b)).count())
            .sum::<usize>()
    };
    let card_hits = (0..cards.len())
        .map(|index| pairs(&cards[index..=index], &cards[index + 1..]))
        .sum();
    let chip_hits = (0..chips.len())
        .map(|index| pairs(&chips[index..=index], &chips[index + 1..]))
        .sum::<usize>()
        + pairs(chips, cards);
    (card_hits, chip_hits)
}

fn distance(point: Point, rectangle: &Rect) -> f32 {
    let dx = (rectangle.min.x - point.x)
        .max(point.x - rectangle.max.x)
        .max(0.0);
    let dy = (rectangle.min.y - point.y)
        .max(point.y - rectangle.max.y)
        .max(0.0);
    dx.hypot(dy)
}

/// Route segments closer than the clearance to a rectangle their wire does not end
/// on, sampled every 2 px. A route that found no way around the cards is not judged.
fn clearance_violations(routes: &[Route], wires: &[Wire], rectangles: &[Rect]) -> usize {
    let mut violations = 0;
    for (route, wire) in routes.iter().zip(wires).filter(|(route, _)| route.complete) {
        let others: Vec<&Rect> = rectangles
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != wire.from.block && *index != wire.to.block)
            .map(|(_, rectangle)| rectangle)
            .collect();
        for (from, to) in route.segments() {
            let samples = (from.distance(to) / 2.0).ceil().max(1.0) as usize;
            let near = (0..=samples).any(|step| {
                let part = step as f32 / samples as f32;
                let point = Point::new(
                    from.x + (to.x - from.x) * part,
                    from.y + (to.y - from.y) * part,
                );
                others
                    .iter()
                    .any(|rectangle| distance(point, rectangle) < CLEARANCE)
            });
            violations += usize::from(near);
        }
    }
    violations
}

/// What a generated board and a reference scenario are both measured by.
#[derive(Clone, Copy, Default)]
struct Measures {
    wires: f64,
    crossings: f64,
    bends: f64,
    straight: f64,
    close: f64,
    coincident: f64,
    long: f64,
    incomplete: f64,
}

impl Measures {
    fn of(laid: &Laid) -> Self {
        let routes = &laid.routes;
        let (close, coincident) = close_pairs(routes);
        Self {
            wires: routes.len() as f64,
            crossings: routes.iter().map(|r| r.bridges.len()).sum::<usize>() as f64,
            bends: routes.iter().map(Route::bends).sum::<usize>() as f64,
            straight: routes.iter().filter(|r| r.bends() == 0).count() as f64,
            close: close as f64,
            coincident: coincident as f64,
            long: laid.long as f64,
            incomplete: routes.iter().filter(|r| !r.complete).count() as f64,
        }
    }

    fn add(&mut self, other: &Self) {
        self.wires += other.wires;
        self.crossings += other.crossings;
        self.bends += other.bends;
        self.straight += other.straight;
        self.close += other.close;
        self.coincident += other.coincident;
        self.long += other.long;
        self.incomplete += other.incomplete;
    }

    fn cells(&self, scale: f64, decimals: usize) -> String {
        [
            (self.wires, 6),
            (self.crossings, 9),
            (self.bends, 6),
            (self.straight, 8),
            (self.close, 6),
            (self.coincident, 10),
            (self.long, 5),
            (self.incomplete, 10),
        ]
        .map(|(value, width)| format!("{:>width$.decimals$}", value / scale))
        .join(" ")
    }
}

const MEASURES: &str = " wires crossings  bends straight  close coincident  long incomplete";

/// What a fit fills in the folder window in Diagram at each standard size, with the
/// explorer open and the Inspector closed (`BoardOverlays::focus`, measured there).
const FIT_1600: Size = Size::new(1134.0, 760.0);
const FIT_960: Size = Size::new(494.0, 300.0);

fn fit(area: Size, width: f32, height: f32) -> f32 {
    (area.width / width).min(area.height / height).min(1.0)
}

fn generated_table(text: &mut String) {
    let _ = writeln!(
        text,
        "generated boards, tidied: the mean of {SEEDS} seeds\n\ncards order{MEASURES}"
    );
    for count in SIZES {
        for order in [true, false] {
            let mut total = Measures::default();
            for seed in 1..=SEEDS {
                total.add(&Measures::of(&lay(&generated(seed, count, order))));
            }
            let _ = writeln!(
                text,
                "{count:>5} {:>5} {}",
                if order { "yes" } else { "no" },
                total.cells(SEEDS as f64, 1)
            );
        }
    }
}

fn reference_table(text: &mut String) -> Result<(), String> {
    let _ = writeln!(
        text,
        "\nreference scenarios, tidied, every step a full card\n\n\
         scenario   steps{MEASURES} width height fit_1600 fit_960 chips card_hits chip_hits clearance"
    );
    let project = shop_project();
    for (name, source) in SCENARIOS {
        let steps = scenario(source, &project).map_err(|error| format!("{name}: {error}"))?;
        let board = reference(&steps, &project);
        let laid = lay(&board);
        let everything: Vec<Rect> = laid.cards.iter().chain(&laid.chips).copied().collect();
        let left = everything.iter().map(|r| r.min.x).fold(f32::MAX, f32::min);
        let top = everything.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
        let right = everything.iter().map(|r| r.max.x).fold(f32::MIN, f32::max);
        let bottom = everything.iter().map(|r| r.max.y).fold(f32::MIN, f32::max);
        let (width, height) = (right - left, bottom - top);
        let (card_hits, chip_hits) = collisions(&laid.cards, &laid.chips);
        let _ = writeln!(
            text,
            "{name:<10} {:>5} {} {width:>5.0} {height:>6.0} {:>8.3} {:>7.3} {:>5} {card_hits:>9} {chip_hits:>9} {:>9}",
            steps.nodes.len(),
            Measures::of(&laid).cells(1.0, 0),
            fit(FIT_1600, width, height),
            fit(FIT_960, width, height),
            laid.chips.len(),
            clearance_violations(&laid.routes, &board.wires, &everything),
        );
    }
    Ok(())
}

fn dump(seed: u64, count: usize) -> String {
    let board = generated(seed, count, true);
    let laid = lay(&board);
    let mut text = String::new();
    for (index, card) in laid.cards.iter().enumerate() {
        let _ = writeln!(
            text,
            "card {index}: ({:.0}, {:.0}) {:.0} x {:.0}",
            card.min.x,
            card.min.y,
            card.width(),
            card.height()
        );
    }
    for (wire, routed) in board.wires.iter().zip(&laid.routes) {
        let points: Vec<String> = routed
            .points
            .iter()
            .map(|point| format!("({:.0}, {:.0})", point.x, point.y))
            .collect();
        let _ = writeln!(
            text,
            "wire {} -> {} {:?} bends {} bridges {}: {}",
            wire.from.block,
            wire.to.block,
            wire.from.side,
            routed.bends(),
            routed.bridges.len(),
            points.join(" ")
        );
    }
    text
}

fn run(arguments: &[String]) -> Result<String, String> {
    match arguments {
        [] => {
            let mut text = String::new();
            generated_table(&mut text);
            reference_table(&mut text)?;
            Ok(text)
        }
        [flag, seed, cards] if flag == "--dump" => {
            let seed = seed.parse().map_err(|_| format!("not a seed: {seed}"))?;
            let cards = cards
                .parse()
                .map_err(|_| format!("not a number of cards: {cards}"))?;
            Ok(dump(seed, cards))
        }
        _ => Err("usage: quality [--dump <seed> <cards>]".to_owned()),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match run(&arguments) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(points: &[(f32, f32)]) -> Route {
        Route {
            points: points.iter().map(|(x, y)| Point::new(*x, *y)).collect(),
            ..Route::default()
        }
    }

    fn square(x: f32, y: f32, side: f32) -> Rect {
        Rect::from_size(Point::new(x, y), Size::new(side, side))
    }

    #[test]
    fn a_pair_is_close_within_8_px_for_20_px() {
        let wire = line(&[(0.0, 0.0), (100.0, 0.0)]);
        let pairs = |other: Route| close_pairs(&[wire.clone(), other]);
        assert_eq!(pairs(line(&[(0.0, 8.0), (20.0, 8.0)])), (1, 0));
        assert_eq!(pairs(line(&[(0.0, 0.4), (20.0, 0.4)])), (0, 1));
        assert_eq!(pairs(line(&[(0.0, 8.5), (100.0, 8.5)])), (0, 0));
        assert_eq!(pairs(line(&[(0.0, 8.0), (19.0, 8.0)])), (0, 0));
        // At right angles, and the same two along a vertical line.
        assert_eq!(pairs(line(&[(50.0, -50.0), (50.0, 50.0)])), (0, 0));
        let upright = line(&[(0.0, 0.0), (0.0, 100.0)]);
        assert_eq!(
            close_pairs(&[upright, line(&[(5.0, 40.0), (5.0, 90.0)])]),
            (1, 0)
        );
    }

    #[test]
    fn a_wire_is_never_close_to_itself() {
        let hairpin = line(&[(0.0, 0.0), (100.0, 0.0), (100.0, 4.0), (0.0, 4.0)]);
        assert_eq!(close_pairs(&[hairpin]), (0, 0));
    }

    #[test]
    fn collisions_tell_cards_from_chips() {
        let cards = [square(0.0, 0.0, 100.0), square(50.0, 50.0, 100.0)];
        assert_eq!(collisions(&cards, &[]), (1, 0));
        // Touching is not overlapping.
        assert_eq!(
            collisions(&[square(0.0, 0.0, 100.0), square(100.0, 0.0, 100.0)], &[]),
            (0, 0)
        );
        let chips = [square(90.0, 90.0, 20.0), square(95.0, 95.0, 20.0)];
        assert_eq!(collisions(&cards, &chips), (1, 5));
    }

    #[test]
    fn clearance_counts_segments_near_other_cards_only() {
        let rectangles = [
            square(0.0, 0.0, 100.0),
            square(300.0, 0.0, 100.0),
            square(150.0, 0.0, 100.0),
        ];
        let wire = Wire::new(
            Port::new(0, Side::Right, 50.0),
            Port::new(1, Side::Left, 50.0),
        );
        let at = |y: f32| line(&[(100.0, y), (300.0, y)]);
        let count = |routed: Route| clearance_violations(&[routed], &[wire], &rectangles);
        // 12 px under the third card is what the router keeps; 5 px is not.
        assert_eq!(count(at(112.0)), 0);
        assert_eq!(count(at(105.0)), 1);
        // A route that found no way is not judged.
        let mut lost = at(50.0);
        lost.complete = false;
        assert_eq!(count(lost), 0);
    }

    #[test]
    fn long_edges_span_more_than_one_layer() {
        let block = Block::new(Size::new(WIDTH, 100.0), Point::new(0.0, 0.0));
        let board = Board {
            blocks: vec![block; 3],
            edges: vec![Edge::new(0, 1), Edge::new(1, 2), Edge::new(0, 2)],
            ..Board::default()
        };
        assert_eq!(lay(&board).long, 1);
    }

    #[test]
    fn the_cards_measure_what_the_window_measured() {
        // `Card::height()` of the desktop, one step of each kind.
        let project = shop_project();
        let steps = scenario(SCENARIOS[1].1, &project).expect("the fixture lowers");
        let height = |handle: &str| {
            let node = steps
                .nodes
                .iter()
                .find(|node| node.handle == handle)
                .expect("the step");
            Card::of(node, &project).height()
        };
        for (handle, expected) in [
            ("begin", 132.0),
            ("login", 372.0),
            ("catalogue", 476.0),
            ("listed", 264.0),
            ("first", 236.0),
            ("expected_total", 288.0),
            ("product", 212.0),
            ("valid_cart", 472.0),
            ("paid", 316.0),
        ] {
            assert_eq!(height(handle), expected, "{handle}");
        }
    }

    #[test]
    fn a_fit_shows_the_whole_board_and_never_enlarges() {
        let area = Size::new(1000.0, 500.0);
        assert_eq!(fit(area, 2000.0, 500.0), 0.5);
        assert_eq!(fit(area, 500.0, 2000.0), 0.25);
        assert_eq!(fit(area, 100.0, 100.0), 1.0);
    }

    #[test]
    fn a_seed_names_one_board() {
        assert_ne!(dump(2, 12), dump(3, 12));
        assert_eq!(dump(2, 12), dump(2, 12));
    }
}
