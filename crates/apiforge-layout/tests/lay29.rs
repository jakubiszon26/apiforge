//! LAY-29 (#382): a wire never runs under its own card.
//!
//! Two things. An ordering line leaves the socket its card draws for it, on
//! the bottom edge, and goes round the card to the step it orders (owner
//! decision A of 2026-11-16; before, it began on the top edge, above the
//! socket, where nothing is drawn). And the router never lets a wire under a
//! card it joins: its own two cards are in its way on every attempt, a port
//! that lies inside the wire's other card is left through its stub, and a
//! wire with no way round has no route. `under_own` is the measure; the
//! reference boards are the desktop's own (`examples/support/boards.txt`).

use apiforge_layout::{
    GAP, Point, Port, Rect, Route, Routing, Side, Size, Wire, crossing, packed, route, under_own,
    violations,
};
use std::time::{Duration, Instant};

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

/// Whether a point of the route lies on the top edge of `card`.
fn touches_top(route: &Route, card: &Rect) -> bool {
    route.segments().any(|(from, to)| {
        from.y.min(to.y) <= card.min.y + 0.01
            && from.y.max(to.y) >= card.min.y - 0.01
            && from.x.min(to.x) <= card.max.x + 0.01
            && from.x.max(to.x) >= card.min.x - 0.01
    })
}

/// Criterion 1: five steps ordered after *Start*, in a row and one under
/// it, as Tidy up puts them. Every ordering line of *Start* leaves its
/// *Next* socket on the bottom edge downwards, goes round and enters its
/// step from above; none touches the top edge of *Start* or lies inside a
/// card it joins.
#[test]
fn lay29_criterion_1_the_lines_of_start_leave_its_socket_and_go_round() {
    let mut cards = vec![card(0.0, 0.0, 280.0, 132.0)];
    cards.extend((1..5).map(|column| card(column as f32 * 380.0, 0.0, 280.0, 110.0)));
    cards.push(card(0.0, 312.0, 280.0, 110.0));
    let wires: Vec<Wire> = (1..=5)
        .map(|step| wire((0, Side::Bottom, 140.0), (step, Side::Top, 140.0)))
        .collect();
    let routes = route(&cards, &wires, &Routing::default());
    for (index, line) in routes.iter().enumerate() {
        assert!(line.complete, "wire {index} has a route");
        let (start, next) = (line.points[0], line.points[1]);
        assert_eq!(start, Point::new(140.0, 132.0), "wire {index}: at Next");
        assert!(
            (next.x - start.x).abs() < 0.01 && next.y >= start.y + GAP - 0.01,
            "wire {index} leaves downwards for a stub: {:?}",
            line.points
        );
        assert!(
            !touches_top(line, &cards[0]),
            "wire {index} keeps off the top edge of Start: {:?}",
            line.points
        );
        let last = line.points.len() - 1;
        assert!(
            line.points[last - 1].y < line.points[last].y,
            "wire {index} enters its step from above: {:?}",
            line.points
        );
    }
    assert_eq!(under_own(&routes, &wires, &cards), []);
    assert_eq!(violations(&routes, &wires, &cards), []);
    assert_eq!(packed(&routes, &cards, GAP), [], "the gap holds");
    assert_eq!(crossings(&routes), 0, "{routes:?}");
}

/// Criterion 2: on every reference board each line begins at a socket its
/// card draws (none leaves a top edge, which only receives) and no piece
/// of a line lies inside a card it joins.
#[test]
fn lay29_criterion_2_every_line_begins_at_a_socket_and_none_is_under_its_card() {
    for board in reference() {
        let from_top = board
            .wires
            .iter()
            .filter(|wire| wire.from.side == Side::Top)
            .count();
        assert_eq!(from_top, 0, "{}: lines leaving a top edge", board.name);
        let routes = route(&board.cards, &board.wires, &Routing::default());
        assert_eq!(
            under_own(&routes, &board.wires, &board.cards),
            [],
            "{}: lines under a card they join",
            board.name
        );
        assert_eq!(
            routes.iter().filter(|route| !route.complete).count(),
            0,
            "{}: every wire has a route",
            board.name
        );
    }
}

/// Criterion 3: the gap of LAY-03 and the guarantee of ADR 0006 hold on
/// every reference board, and the price the owner accepted is not
/// exceeded: `shop-30` in the default view has at most 6 crossings (4
/// before), with every wire and check drawn at most 106 (105).
#[test]
fn lay29_criterion_3_the_gap_holds_and_the_price_is_the_one_accepted() {
    let mut found: Vec<(String, usize)> = Vec::new();
    for board in reference() {
        let routes = route(&board.cards, &board.wires, &Routing::default());
        assert_eq!(
            packed(&routes, &board.cards, GAP),
            [],
            "{}: the gap holds",
            board.name
        );
        assert_eq!(
            violations(&routes, &board.wires, &board.cards),
            [],
            "{}",
            board.name
        );
        found.push((board.name, crossings(&routes)));
    }
    let of = |name: &str| -> usize {
        found
            .iter()
            .find(|(board, _)| board == name)
            .unwrap_or_else(|| panic!("a board {name}"))
            .1
    };
    for (name, most) in [
        ("shop-30 compact collapsed 30 chips", 6),
        ("shop-30 compact expanded 30 all", 106),
    ] {
        assert!(of(name) <= most, "{name}: {} crossings", of(name));
    }
}

/// Criterion 4, the port inside the other card: two cards lie on each
/// other and the port of one is inside the other. The wire leaves through
/// its stub, which is the only piece of it inside a card it joins.
#[test]
fn lay29_criterion_4_a_port_inside_the_other_card_is_left_through_its_stub() {
    let cards = [
        card(0.0, 0.0, 280.0, 120.0),
        card(100.0, 100.0, 280.0, 120.0),
    ];
    let wires = [wire((0, Side::Bottom, 140.0), (1, Side::Left, 60.0))];
    let routes = route(&cards, &wires, &Routing::default());
    let line = &routes[0];
    assert!(line.complete, "{:?}", line.points);
    let (start, next) = (line.points[0], line.points[1]);
    assert_eq!(start, Point::new(140.0, 120.0));
    assert!(
        (next.x - start.x).abs() < 0.01 && next.y > cards[1].max.y,
        "the stub leaves the card that buries the port: {:?}",
        line.points
    );
    // Past the stub no piece lies inside either card.
    for (from, to) in line.segments().skip(1) {
        for rectangle in &cards {
            let inside = from.x.min(to.x) < rectangle.max.x - 0.01
                && from.x.max(to.x) > rectangle.min.x + 0.01
                && from.y.min(to.y) < rectangle.max.y - 0.01
                && from.y.max(to.y) > rectangle.min.y + 0.01;
            assert!(!inside, "{from:?} -> {to:?} is under {rectangle:?}");
        }
    }
    assert_eq!(under_own(&routes, &wires, &cards), []);
}

/// Criterion 4, cards that touch: the port at the bottom of one card
/// opens onto the top edge of the wire's other card. The wire runs along
/// the edge between them or has no route; it never passes under either.
#[test]
fn lay29_criterion_4_between_cards_that_touch_the_wire_is_not_under_them() {
    let cards = [
        card(100.0, 0.0, 280.0, 120.0),
        card(0.0, 120.0, 480.0, 120.0),
    ];
    let wires = [wire((0, Side::Bottom, 140.0), (1, Side::Bottom, 240.0))];
    let routes = route(&cards, &wires, &Routing::default());
    assert_eq!(under_own(&routes, &wires, &cards), []);
    for line in routes.iter().filter(|line| line.complete) {
        for (from, to) in line.segments() {
            for rectangle in &cards {
                let inside = from.x.min(to.x) < rectangle.max.x - 0.01
                    && from.x.max(to.x) > rectangle.min.x + 0.01
                    && from.y.min(to.y) < rectangle.max.y - 0.01
                    && from.y.max(to.y) > rectangle.min.y + 0.01;
                assert!(!inside, "{from:?} -> {to:?} is under {rectangle:?}");
            }
        }
    }
}

/// Criterion 7: the budget of ADR 0006. The reference boards route within
/// it by a wide margin; the budget's own board is `bench`'s.
#[test]
fn lay29_criterion_7_the_reference_boards_route_in_time() {
    let board = reference()
        .into_iter()
        .find(|board| board.name == "shop-50 compact expanded 56 all")
        .expect("the board");
    let started = Instant::now();
    let routes = route(&board.cards, &board.wires, &Routing::default());
    let taken = started.elapsed();
    assert_eq!(routes.len(), board.wires.len());
    // A debug build is some twenty times slower than the release build the
    // budget is measured on (35 ms there).
    assert!(taken < Duration::from_secs(20), "{taken:?}");
}
