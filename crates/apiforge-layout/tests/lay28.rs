//! LAY-28 (#348): lines in one corridor are ordered so they cross less.
//!
//! The router gives each line the nearest free track, in the order it routes
//! them. Once every wire has its route, the lines of a corridor change tracks
//! wherever the board has fewer crossings for it, and nothing else about them
//! changes: their ports, their bends, the gap of LAY-03 and the guarantee of
//! ADR 0006. `Routing::ordered` switches the order off, which is how these
//! tests see what it does; the reference boards are the desktop's own
//! (`examples/support/boards.txt`).
//!
//! The last test is #349's: with the inputs of one card a gap apart, no two
//! lines of a reference board run within 8 px of each other.

use apiforge_layout::{
    GAP, Point, Port, Rect, Route, Routing, Side, Size, Wire, crossing, packed, route, route_among,
    violations,
};

const BOARDS: &str = include_str!("../examples/support/boards.txt");

fn card(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_size(Point::new(x, y), Size::new(width, height))
}

fn wire(from: (usize, Side, f32), to: (usize, Side, f32)) -> Wire {
    Wire::new(
        Port::new(from.0, from.1, from.2),
        Port::new(to.0, to.1, to.2),
    )
}

fn unordered() -> Routing {
    Routing {
        ordered: false,
        ..Routing::default()
    }
}

/// How often two of the lines cross.
fn crossings(routes: &[Route]) -> usize {
    let mut count = 0;
    for (index, a) in routes.iter().enumerate() {
        for b in &routes[index + 1..] {
            for (a1, a2) in a.segments() {
                for (b1, b2) in b.segments() {
                    count += usize::from(crossing(a1, a2, b1, b2).is_some());
                }
            }
        }
    }
    count
}

/// Pairs of parallel segments of two lines that run within `near` px of each
/// other for 20 px or more, as the `quality` example counts `close` and
/// `coincident`.
fn alongside(routes: &[Route], near: f32) -> usize {
    let mut count = 0;
    for (index, a) in routes.iter().enumerate() {
        for b in &routes[index + 1..] {
            for (a1, a2) in a.segments() {
                for (b1, b2) in b.segments() {
                    let overlap = |a1: f32, a2: f32, b1: f32, b2: f32| {
                        a1.max(a2).min(b1.max(b2)) - a1.min(a2).max(b1.min(b2))
                    };
                    let flat = |p: Point, q: Point| (p.y - q.y).abs() < 0.01;
                    let upright = |p: Point, q: Point| (p.x - q.x).abs() < 0.01;
                    let (apart, length) = if flat(a1, a2) && flat(b1, b2) {
                        ((a1.y - b1.y).abs(), overlap(a1.x, a2.x, b1.x, b2.x))
                    } else if upright(a1, a2) && upright(b1, b2) {
                        ((a1.x - b1.x).abs(), overlap(a1.y, a2.y, b1.y, b2.y))
                    } else {
                        continue;
                    };
                    count += usize::from(apart <= near && length >= 20.0);
                }
            }
        }
    }
    count
}

/// A board of `examples/support/boards.txt`: its name, its cards and chips,
/// and its wires.
struct Board {
    name: String,
    cards: Vec<Rect>,
    wires: Vec<Wire>,
}

fn reference() -> Vec<Board> {
    let side = |word: &str| match word {
        "left" => Side::Left,
        "right" => Side::Right,
        "top" => Side::Top,
        "bottom" => Side::Bottom,
        other => panic!("not a side: {other}"),
    };
    let mut boards: Vec<Board> = Vec::new();
    for line in BOARDS.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let number = |index: usize| -> f32 { words[index].parse().expect("a number") };
        match words.first().copied() {
            Some("board") => boards.push(Board {
                name: words[1..].join(" "),
                cards: Vec::new(),
                wires: Vec::new(),
            }),
            Some("card" | "chip") => boards.last_mut().expect("a board").cards.push(card(
                number(1),
                number(2),
                number(3),
                number(4),
            )),
            Some("wire") => boards.last_mut().expect("a board").wires.push(wire(
                (number(1) as usize, side(words[2]), number(3)),
                (number(4) as usize, side(words[5]), number(6)),
            )),
            _ => {}
        }
    }
    assert_eq!(boards.len(), 24, "the reference boards");
    boards
}

/// A small deterministic generator; the layout crate carries no dependencies.
struct Random(u64);

impl Random {
    fn below(&mut self, limit: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % limit as u64) as usize
    }
}

/// Rows of cards with room for lines between them, wired forwards at random:
/// from a right side to a left side, and a few rails from top to top.
fn generated(seed: u64) -> (Vec<Rect>, Vec<Wire>) {
    let mut random = Random(seed * 2 + 1);
    let (columns, rows) = (5, 3);
    let cards: Vec<Rect> = (0..columns * rows)
        .map(|index| {
            let (column, row) = (index % columns, index / columns);
            card(column as f32 * 420.0, row as f32 * 320.0, 280.0, 160.0)
        })
        .collect();
    let mut wires = Vec::new();
    for target in 1..cards.len() {
        for input in 0..1 + random.below(3) {
            let source = random.below(target);
            wires.push(wire(
                (source, Side::Right, 30.0 + 24.0 * random.below(5) as f32),
                (target, Side::Left, 30.0 + 40.0 * input as f32),
            ));
        }
        if random.below(3) == 0 {
            let source = random.below(target);
            wires.push(wire((source, Side::Top, 140.0), (target, Side::Top, 140.0)));
        }
    }
    (cards, wires)
}

/// Criterion 1: lines that share a corridor and leave it one after another
/// do not cross each other. Three values of one card go up the corridor to
/// the three inputs of a card in the row above: each line turns into the
/// corridor and out of it inside the one before.
#[test]
fn lay28_criterion_1_lines_up_a_corridor_do_not_cross() {
    let mut cards: Vec<Rect> = (0..4)
        .map(|column| card(column as f32 * 380.0, 0.0, 280.0, 120.0))
        .collect();
    cards.extend((0..4).map(|column| card(column as f32 * 380.0, 300.0, 280.0, 120.0)));
    let wires = [
        wire((4, Side::Right, 30.0), (1, Side::Left, 30.0)),
        wire((4, Side::Right, 60.0), (1, Side::Left, 60.0)),
        wire((4, Side::Right, 90.0), (1, Side::Left, 90.0)),
    ];
    let before = route(&cards, &wires, &unordered());
    assert!(
        crossings(&before) > 0,
        "the board is one the router alone leaves crossed"
    );
    let routes = route(&cards, &wires, &Routing::default());
    assert_eq!(crossings(&routes), 0, "{routes:?}");
    assert_eq!(packed(&routes, &cards, GAP), [], "the gap holds");
    assert_eq!(violations(&routes, &wires, &cards), []);
    // In the corridor the lines lie in the order of their ports: the lowest
    // port turns first and comes out last.
    let track = |route: &Route| route.points[1].x;
    assert!(
        track(&routes[0]) < track(&routes[1]) && track(&routes[1]) < track(&routes[2]),
        "{routes:?}"
    );
}

/// Criterion 2: the order changes tracks and nothing else. On every
/// reference board and on generated ones: each line keeps its ports and its
/// bends, no line comes onto a card, the gap is broken nowhere it was kept,
/// no two lines come to lie on each other, the board has no more crossings
/// than without the order, and twice gives the same.
#[test]
fn lay28_criterion_2_the_order_keeps_what_the_router_promised() {
    let mut boards: Vec<(String, Vec<Rect>, Vec<Wire>)> = reference()
        .into_iter()
        .map(|board| (board.name, board.cards, board.wires))
        .collect();
    boards.extend((1..=12).map(|seed| {
        let (cards, wires) = generated(seed);
        (format!("generated {seed}"), cards, wires)
    }));
    let mut saved = 0;
    for (name, cards, wires) in &boards {
        let before = route(cards, wires, &unordered());
        let routes = route(cards, wires, &Routing::default());
        assert_eq!(routes.len(), before.len());
        for (index, (old, new)) in before.iter().zip(&routes).enumerate() {
            assert_eq!(
                old.points.first(),
                new.points.first(),
                "{name}: wire {index}"
            );
            assert_eq!(old.points.last(), new.points.last(), "{name}: wire {index}");
            assert_eq!(old.bends(), new.bends(), "{name}: wire {index}");
            assert_eq!(old.complete, new.complete, "{name}: wire {index}");
        }
        assert_eq!(
            violations(&routes, wires, cards),
            violations(&before, wires, cards),
            "{name}: no line on a card"
        );
        let (kept, broken) = (
            packed(&before, cards, GAP).len(),
            packed(&routes, cards, GAP).len(),
        );
        assert!(
            broken <= kept,
            "{name}: {broken} breaches of the gap, {kept} before"
        );
        assert!(
            alongside(&routes, 0.5) <= alongside(&before, 0.5),
            "{name}: no two lines come to lie on each other"
        );
        let (was, is) = (crossings(&before), crossings(&routes));
        assert!(is <= was, "{name}: {is} crossings, {was} before");
        saved += was - is;
        assert_eq!(
            routes,
            route(cards, wires, &Routing::default()),
            "{name}: twice the same"
        );
    }
    assert!(saved > 0, "the order saves crossings somewhere");
}

/// Criterion 3: the reference boards cross less. The board the window shows
/// for `shop-30` with every wire drawn, with and without its checks; the
/// boards with chips and the checks drawn; and all 24 together.
#[test]
fn lay28_criterion_3_the_reference_boards_cross_less() {
    let boards = reference();
    let measured: Vec<(String, usize, usize, usize)> = boards
        .iter()
        .map(|board| {
            let routes = route(&board.cards, &board.wires, &Routing::default());
            let before = route(&board.cards, &board.wires, &unordered());
            assert_eq!(
                packed(&routes, &board.cards, GAP),
                [],
                "{}: the gap holds",
                board.name
            );
            (
                board.name.clone(),
                crossings(&routes),
                crossings(&before),
                board.wires.len(),
            )
        })
        .collect();
    let of = |name: &str| -> usize {
        measured
            .iter()
            .find(|(board, ..)| board.starts_with(name))
            .unwrap_or_else(|| panic!("a board {name}"))
            .1
    };
    // What `quality` printed before this ticket, with the inputs of #349:
    // 82, 123, 17, 7, 38 and 130. The first three limits were 75, 105 and
    // 10 until LAY-29 (#382): an ordering line leaves its socket at the
    // bottom and goes round its card.
    for (name, most) in [
        ("shop-30 compact collapsed 30 all", 82),
        ("shop-30 compact expanded 30 all", 106),
        ("shop-30 compact expanded 30 chips", 12),
        ("fan-out compact expanded 26 chips", 5),
        ("shop-50 compact expanded 56 chips", 26),
        ("fan-out compact expanded 26 all", 90),
    ] {
        assert!(of(name) <= most, "{name}: {} crossings", of(name));
    }
    let (is, was): (usize, usize) = measured
        .iter()
        .fold((0, 0), |sum, (_, is, was, _)| (sum.0 + is, sum.1 + was));
    assert!(
        is * 100 <= was * 85,
        "all 24 boards: {is} crossings, {was} without the order"
    );
}

/// Criterion 4: lines brought back among lines that stay (a chip's wire, a
/// rail, while a card is selected) are ordered among themselves; the lines
/// that stay are counted and not moved, and the gap from them holds.
#[test]
fn lay28_criterion_4_lines_brought_back_leave_the_others_alone() {
    let mut cards: Vec<Rect> = (0..4)
        .map(|column| card(column as f32 * 380.0, 0.0, 280.0, 120.0))
        .collect();
    cards.extend((0..4).map(|column| card(column as f32 * 380.0, 300.0, 280.0, 120.0)));
    let resting = [wire((5, Side::Right, 60.0), (2, Side::Left, 60.0))];
    let fixed = route(&cards, &resting, &Routing::default());
    let back = [
        wire((4, Side::Right, 30.0), (1, Side::Left, 30.0)),
        wire((4, Side::Right, 60.0), (1, Side::Left, 60.0)),
        wire((4, Side::Right, 90.0), (1, Side::Left, 90.0)),
    ];
    let before = route_among(&cards, &back, &unordered(), &fixed);
    let routes = route_among(&cards, &back, &Routing::default(), &fixed);
    assert_eq!(routes.len(), back.len(), "only the lines brought back");
    let all = |new: &[Route]| -> Vec<Route> { fixed.iter().chain(new).cloned().collect() };
    assert!(
        crossings(&all(&routes)) < crossings(&all(&before)),
        "{} crossings, {} without the order",
        crossings(&all(&routes)),
        crossings(&all(&before))
    );
    assert_eq!(packed(&all(&routes), &cards, GAP), [], "the gap holds");
}

/// #349, criterion 3: with the sockets of one card a gap apart, no two lines
/// of a reference board run within 8 px of each other for 20 px, the pairs
/// `quality` prints as `close` (1 to 13 a board before); the pairs on one
/// line are the wires of one port, as before.
#[test]
fn issue349_criterion_3_no_two_lines_run_within_8_px() {
    for board in reference() {
        let routes = route(&board.cards, &board.wires, &Routing::default());
        let (close, coincident) = (alongside(&routes, 8.0), alongside(&routes, 0.5));
        assert_eq!(
            close - coincident,
            0,
            "{}: pairs of lines within 8 px of each other",
            board.name
        );
    }
}
