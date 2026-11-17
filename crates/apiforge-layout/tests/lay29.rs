//! LAY-29 (#382): a wire never runs under its own card.
//!
//! The router never lets a wire under a card it joins: its own two cards
//! are in its way on every attempt, a port that lies inside the wire's
//! other card is left through its stub, and a wire with no way round has
//! no route. `under_own` is the measure; the reference boards are the
//! desktop's own (`examples/support/boards.txt`). Where an ordering line
//! leaves its card is not this ticket's: the owner narrowed it on
//! 2026-11-17 to the part that does not depend on the language.

use apiforge_layout::{
    GAP, Point, Port, Rect, Routing, Side, Size, Wire, packed, route, under_own, violations,
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

/// Criterion 2: on every reference board no piece of a line lies inside
/// a card it joins, every wire has a route, the gap of LAY-03 holds and no
/// line touches another card.
#[test]
fn lay29_criterion_2_on_the_reference_boards_no_line_is_under_its_card() {
    for board in reference() {
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
    }
}

/// Criterion 1, the port inside the other card: two cards lie on each
/// other and the port of one is inside the other. The wire leaves through
/// its stub, which is the only piece of it inside a card it joins.
#[test]
fn lay29_criterion_1_a_port_inside_the_other_card_is_left_through_its_stub() {
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

/// Criterion 1, cards that touch: the port at the bottom of one card
/// opens onto the top edge of the wire's other card. The wire runs along
/// the edge between them or has no route; it never passes under either.
#[test]
fn lay29_criterion_1_between_cards_that_touch_the_wire_is_not_under_them() {
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

/// Criterion 1, cards closer than a stub: the ports of a wire face the
/// other card 12 px and 9 px away, as `begin` and `login` lie when cards
/// are piled. Each stub stops between the two cards and the wire goes
/// round; it has a route, and no piece of it lies inside either card.
#[test]
fn lay29_criterion_1_cards_closer_than_a_stub_still_give_a_route() {
    // A port on the top of the lower card, under the upper one, wired to
    // the top of the upper card.
    let cards = [
        card(300.0, 200.0, 280.0, 110.0),
        card(380.0, 0.0, 280.0, 188.0),
    ];
    let wires = [wire((0, Side::Top, 140.0), (1, Side::Top, 140.0))];
    let routes = route(&cards, &wires, &Routing::default());
    assert!(routes[0].complete, "{:?}", routes[0].points);
    assert_eq!(under_own(&routes, &wires, &cards), []);
    assert_eq!(violations(&routes, &wires, &cards), []);
    // Two ports facing each other across 9 px.
    let cards = [
        card(400.0, 180.0, 210.0, 130.0),
        card(619.0, 190.0, 330.0, 170.0),
    ];
    let wires = [wire((0, Side::Right, 70.0), (1, Side::Left, 54.0))];
    let routes = route(&cards, &wires, &Routing::default());
    assert!(routes[0].complete, "{:?}", routes[0].points);
    assert_eq!(under_own(&routes, &wires, &cards), []);
    assert_eq!(violations(&routes, &wires, &cards), []);
}

/// Criterion 1, a card over one port only: the wire's two cards face each
/// other 10 px apart and a third card lies over the start port, ending 2 px
/// short of the other card. The route leaves that card through its stub
/// and touches no card past it: a card over one port is no room for the
/// route, only one over both is (ADR 0006).
#[test]
fn lay29_criterion_1_a_card_over_one_port_is_left_through_the_stub_only() {
    let cards = [
        card(0.0, 0.0, 100.0, 100.0),
        card(110.0, 0.0, 100.0, 100.0),
        card(50.0, 0.0, 58.0, 100.0),
    ];
    let wires = [wire((0, Side::Right, 50.0), (1, Side::Left, 80.0))];
    let routes = route(&cards, &wires, &Routing::default());
    assert!(routes[0].complete, "{:?}", routes[0].points);
    assert_eq!(
        violations(&routes, &wires, &cards),
        [],
        "{:?}",
        routes[0].points
    );
    assert_eq!(under_own(&routes, &wires, &cards), []);
}

/// Criterion 5: the budget of ADR 0006. The reference boards route within
/// it by a wide margin; the budget's own board is `bench`'s.
#[test]
fn lay29_criterion_5_the_reference_boards_route_in_time() {
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
