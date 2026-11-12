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
//! `fixtures/scenarios/lay/` as the desktop tidies them, read from
//! `support/boards.txt`: each at the compact and the full density, with its checks
//! collapsed and expanded, every card at the size the board draws it and every wire
//! between the ports the board gives the router (LAY-06). The desktop's acceptance
//! test `lay06_criterion_10_quality_on_real_card_sizes` writes that file and fails
//! when it is out of date. The numbers are deterministic. `--dump` prints one
//! generated board's rectangles and routes, for reading a case by hand.
//!
//! The columns:
//!
//! - `wires`, `crossings` (the router's bridges), `bends`, `straight` (wires without
//!   a bend), `incomplete` (routes that found no way around the cards);
//! - `close`: pairs of parallel segments of different wires within 8 px of each
//!   other for at least 20 px; `coincident`: the same within 0.5 px, counted there
//!   and not under `close`;
//! - `long`: edges spanning more than one layer, for the generated boards;
//! - `width`, `height`: the board's bounds; `fit_1600`, `fit_960`: the zoom that
//!   shows all of it at 1600 × 1000 and 960 × 540, not limited to the zoom the
//!   window allows;
//! - `card_hits`: pairs of cards that overlap; `chip_hits`: pairs with a proxy chip
//!   in them. A scenario read from code has no chips yet: they are placed by hand
//!   and live in the document, so `chips` says how many were measured;
//! - `clearance`: route segments closer than 12 px to a card or chip the wire does
//!   not end on, the router's `Routing::margin`.
//!
//! The generated boards use the full card's geometry (`canvas/layout.rs`), which
//! this crate cannot depend on; the reference boards carry the desktop's own.

mod support;

use apiforge_layout::{
    Block, Edge, Graph, Point, Port, Rect, Route, Routing, Side, Size, Wire, layers_of, route, tidy,
};
use std::{fmt::Write, process::ExitCode};
use support::Random;

const WIDTH: f32 = 280.0;
const HEADER: f32 = 64.0;
const INPUT_ROW: f32 = 52.0;
const OUTPUT_ROW: f32 = 28.0;
const FOOTER: f32 = 36.0;
const CONTROL: f32 = 28.0;

/// Parallel segments this near each other, for this long, read as one line.
const CLOSE: f32 = 8.0;
const COINCIDENT: f32 = 0.5;
const ALONGSIDE: f32 = 20.0;
/// The router keeps exactly its margin; half a pixel forgives the sampling below.
const CLEARANCE: f32 = 11.5;

const SIZES: [usize; 4] = [6, 12, 25, 50];
const SEEDS: u64 = 10;

/// The reference scenarios as the desktop tidies them, written by the desktop.
const BOARDS: &str = include_str!("support/boards.txt");

/// A card as the desktop measures it at the full density.
struct Card {
    inputs: usize,
    outputs: usize,
}

impl Card {
    fn generated(inputs: usize, outputs: usize) -> Self {
        Self { inputs, outputs }
    }

    fn outputs_top(&self) -> f32 {
        HEADER + self.inputs as f32 * INPUT_ROW
    }

    fn height(&self) -> f32 {
        self.outputs_top() + self.outputs as f32 * OUTPUT_ROW + 4.0 + FOOTER + CONTROL
    }

    fn input_y(&self, index: usize) -> f32 {
        HEADER + index as f32 * INPUT_ROW + 30.0
    }

    fn output_y(&self, index: usize) -> f32 {
        self.outputs_top() + index as f32 * OUTPUT_ROW + OUTPUT_ROW / 2.0 + 2.0
    }
}

/// What the layout and the router work on.
#[derive(Default)]
struct Board {
    blocks: Vec<Block>,
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
        for input in 0..cards[target].inputs {
            if !random.chance(70) {
                continue;
            }
            let source = target - (1 + random.below(4.min(target)));
            let output = random.below(cards[source].outputs);
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

/// One reference scenario as the desktop tidied it: `board <scenario> <density>
/// <checks> <steps>`, then its `card`, `chip`, `wire` and `mark` lines, then `end`.
struct Tidied {
    name: String,
    density: String,
    checks: String,
    steps: usize,
    cards: Vec<Rect>,
    chips: Vec<Rect>,
    /// The continuation marks, which count in the board's size only.
    marks: Vec<Rect>,
    wires: Vec<Wire>,
}

fn side(word: &str) -> Result<Side, String> {
    match word {
        "left" => Ok(Side::Left),
        "right" => Ok(Side::Right),
        "top" => Ok(Side::Top),
        "bottom" => Ok(Side::Bottom),
        _ => Err(format!("not a side: {word}")),
    }
}

fn boards(text: &str) -> Result<Vec<Tidied>, String> {
    let number = |word: Option<&str>| -> Result<f32, String> {
        let word = word.ok_or("a number is missing")?;
        word.parse().map_err(|_| format!("not a number: {word}"))
    };
    let rect = |words: &mut std::str::SplitWhitespace<'_>| -> Result<Rect, String> {
        let (x, y) = (number(words.next())?, number(words.next())?);
        let (width, height) = (number(words.next())?, number(words.next())?);
        Ok(Rect::from_size(Point::new(x, y), Size::new(width, height)))
    };
    let mut result: Vec<Tidied> = Vec::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let mut words = line.split_whitespace();
        let kind = words.next().unwrap_or_default();
        if kind == "board" {
            let mut word = || {
                words
                    .next()
                    .map(str::to_owned)
                    .ok_or("a board line is short")
            };
            result.push(Tidied {
                name: word()?,
                density: word()?,
                checks: word()?,
                steps: number(Some(&word()?))? as usize,
                cards: Vec::new(),
                chips: Vec::new(),
                marks: Vec::new(),
                wires: Vec::new(),
            });
            continue;
        }
        if kind == "end" || kind.is_empty() {
            continue;
        }
        let board = result.last_mut().ok_or("a line before the first board")?;
        match kind {
            "card" => board.cards.push(rect(&mut words)?),
            "chip" => board.chips.push(rect(&mut words)?),
            "mark" => board.marks.push(rect(&mut words)?),
            "wire" => {
                let port = |words: &mut std::str::SplitWhitespace<'_>| -> Result<Port, String> {
                    let block = number(words.next())? as usize;
                    let side = side(words.next().ok_or("a side is missing")?)?;
                    Ok(Port::new(block, side, number(words.next())?))
                };
                let from = port(&mut words)?;
                board.wires.push(Wire::new(from, port(&mut words)?));
            }
            other => return Err(format!("not a board line: {other}")),
        }
    }
    Ok(result)
}

/// The board tidied and routed.
struct Laid {
    cards: Vec<Rect>,
    routes: Vec<Route>,
    long: usize,
}

fn lay(board: &Board) -> Laid {
    let blocks = &board.blocks;
    let graph = Graph::new(blocks, &board.edges);
    let layers = layers_of(&graph);
    let rectangles: Vec<Rect> = tidy(&graph)
        .iter()
        .zip(blocks)
        .map(|(position, block)| Rect::from_size(*position, block.size))
        .collect();
    let routes = route(&rectangles, &board.wires, &Routing::default());
    Laid {
        cards: rectangles,
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
        "\nreference scenarios, tidied by the desktop, at the sizes it draws\n\n\
         scenario   density checks    steps cards wires crossings  bends straight  close coincident incomplete width height fit_1600 fit_960 chips card_hits chip_hits clearance"
    );
    for board in boards(BOARDS)? {
        let rectangles: Vec<Rect> = board.cards.iter().chain(&board.chips).copied().collect();
        let laid = Laid {
            cards: board.cards.clone(),
            routes: route(&rectangles, &board.wires, &Routing::default()),
            long: 0,
        };
        let measures = Measures::of(&laid);
        let everything: Vec<Rect> = rectangles.iter().chain(&board.marks).copied().collect();
        let left = everything.iter().map(|r| r.min.x).fold(f32::MAX, f32::min);
        let top = everything.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
        let right = everything.iter().map(|r| r.max.x).fold(f32::MIN, f32::max);
        let bottom = everything.iter().map(|r| r.max.y).fold(f32::MIN, f32::max);
        let (width, height) = (right - left, bottom - top);
        let (card_hits, chip_hits) = collisions(&board.cards, &board.chips);
        let _ = writeln!(
            text,
            "{:<10} {:<7} {:<9} {:>5} {:>5} {:>5} {:>9} {:>6} {:>8} {:>6} {:>10} {:>10} {width:>5.0} {height:>6.0} {:>8.3} {:>7.3} {:>5} {card_hits:>9} {chip_hits:>9} {:>9}",
            board.name,
            board.density,
            board.checks,
            board.steps,
            board.cards.len(),
            measures.wires,
            measures.crossings,
            measures.bends,
            measures.straight,
            measures.close,
            measures.coincident,
            measures.incomplete,
            fit(FIT_1600, width, height),
            fit(FIT_960, width, height),
            board.chips.len(),
            clearance_violations(&laid.routes, &board.wires, &rectangles),
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

    /// LAY-06 criterion 10: the reference boards are the desktop's, at both
    /// densities and with the checks collapsed and expanded, and a compact
    /// card is shorter than the same card full.
    #[test]
    fn the_reference_boards_are_the_desktops() {
        let boards = boards(BOARDS).expect("the board file reads");
        assert_eq!(
            boards.len(),
            12,
            "three scenarios, two densities, two states"
        );
        for board in &boards {
            assert!(
                !board.cards.is_empty() && !board.wires.is_empty(),
                "{}",
                board.name
            );
            for wire in &board.wires {
                let blocks = board.cards.len() + board.chips.len();
                assert!(wire.from.block < blocks && wire.to.block < blocks);
            }
        }
        let height = |density: &str| -> f32 {
            boards
                .iter()
                .find(|board| {
                    board.name == "shop-30"
                        && board.density == density
                        && board.checks == "expanded"
                })
                .expect("shop-30")
                .cards
                .iter()
                .map(Rect::height)
                .sum()
        };
        assert!(height("compact") < height("full"));
        let expanded = |name: &str| {
            boards
                .iter()
                .find(|board| board.name == name && board.checks == "expanded")
                .map(|board| board.cards.len())
        };
        let collapsed = |name: &str| {
            boards
                .iter()
                .find(|board| board.name == name && board.checks == "collapsed")
                .map(|board| board.cards.len())
        };
        assert!(
            collapsed("shop-30") < expanded("shop-30"),
            "collapsed folds checks"
        );
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

    /// LAY-04 criterion 3: on the generated boards without an ordering
    /// chain, the data-only rows of the table, at least 40 % of the wires
    /// run straight at every size, and the four rows together have no more
    /// crossings and coincident pairs than before LAY-04.
    #[test]
    fn lay04_criterion_3_straight_wires_on_data_only_boards() {
        // Before LAY-04: crossings and coincident pairs over the ten seeds.
        const BEFORE: [(usize, f64, f64); 4] = [
            (6, 20.0, 23.0),
            (12, 64.0, 61.0),
            (25, 181.0, 106.0),
            (50, 526.0, 328.0),
        ];
        let mut after = Measures::default();
        for (count, _, _) in BEFORE {
            let mut total = Measures::default();
            for seed in 1..=SEEDS {
                total.add(&Measures::of(&lay(&generated(seed, count, false))));
            }
            assert!(
                total.straight >= 0.40 * total.wires,
                "{count} cards: {} of {} wires straight",
                total.straight,
                total.wires
            );
            after.add(&total);
        }
        let crossings: f64 = BEFORE.iter().map(|row| row.1).sum();
        let coincident: f64 = BEFORE.iter().map(|row| row.2).sum();
        assert!(
            after.crossings <= crossings,
            "{} crossings, {crossings} before",
            after.crossings
        );
        assert!(
            after.coincident <= coincident,
            "{} coincident pairs, {coincident} before",
            after.coincident
        );
    }
}
