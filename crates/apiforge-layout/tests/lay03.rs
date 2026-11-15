//! LAY-03 (#34): lines are always far enough apart to trace.
//!
//! The router keeps a gap of 20 px between any two parallel segments, side by
//! side or end to end, and between a segment and a card; wires of one port
//! share only the straight piece out of it. `packed` is the measure. These
//! boards are made by hand or are two reference boards as Tidy up placed them
//! before this ticket, with 96 px between columns and rows and 24 px under a
//! card: the router has to keep the gap on them without Tidy up's help.

use apiforge_layout::{
    GAP, Near, Point, Port, Rect, Route, Routing, Side, Size, Wire, packed, route, violations,
};

fn card(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_size(Point::new(x, y), Size::new(width, height))
}

fn wire(from: (usize, Side, f32), to: (usize, Side, f32)) -> Wire {
    Wire::new(
        Port::new(from.0, from.1, from.2),
        Port::new(to.0, to.1, to.2),
    )
}

/// Routes the board and holds it to the rule and to ADR 0006.
fn routed(cards: &[Rect], wires: &[Wire]) -> Vec<Route> {
    routed_with(cards, wires, 0)
}

/// [`routed`], on a board whose cards leave `allowed` places with no room
/// for the gap.
fn routed_with(cards: &[Rect], wires: &[Wire], allowed: usize) -> Vec<Route> {
    let routes = route(cards, wires, &Routing::default());
    assert!(
        routes.iter().all(|route| route.complete),
        "every wire finds a route"
    );
    assert_eq!(violations(&routes, wires, cards), [], "no line on a card");
    let breaches = packed(&routes, cards, GAP);
    let line = |index: usize| -> String {
        routes[index]
            .points
            .iter()
            .map(|point| format!("({:.0}, {:.0})", point.x, point.y))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let told: Vec<String> = breaches
        .iter()
        .map(|breach| match breach.near {
            Near::Route(other) => format!(
                "wire {} [{}] and wire {other} [{}]",
                breach.route,
                line(breach.route),
                line(other)
            ),
            Near::Card(card) => {
                format!(
                    "wire {} [{}] and card {card}",
                    breach.route,
                    line(breach.route)
                )
            }
        })
        .collect();
    assert!(
        breaches.len() <= allowed,
        "{} breaches of the gap:\n{}",
        breaches.len(),
        told.join("\n")
    );
    assert_eq!(
        routes,
        route(cards, wires, &Routing::default()),
        "twice the same"
    );
    routes
}

/// A board in the format of `examples/support/boards.txt`.
fn board(text: &str) -> (Vec<Rect>, Vec<Wire>) {
    let side = |word: &str| match word {
        "left" => Side::Left,
        "right" => Side::Right,
        "top" => Side::Top,
        "bottom" => Side::Bottom,
        other => panic!("not a side: {other}"),
    };
    let (mut cards, mut wires) = (Vec::new(), Vec::new());
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let number = |index: usize| -> f32 { words[index].parse().expect("a number") };
        match words.first().copied() {
            Some("card") => cards.push(card(number(1), number(2), number(3), number(4))),
            Some("wire") => wires.push(wire(
                (number(1) as usize, side(words[2]), number(3)),
                (number(4) as usize, side(words[5]), number(6)),
            )),
            _ => {}
        }
    }
    (cards, wires)
}

/// Situation 3 and 4 of the finding: several wires that have to share the
/// corridor between two columns run 20 px apart there.
#[test]
fn lay03_wires_that_share_a_corridor_keep_the_gap() {
    // Three sources on the left, their readers on the right in the opposite
    // order: every wire travels up or down the corridor between them.
    let cards = [
        card(0.0, 0.0, 280.0, 120.0),
        card(0.0, 220.0, 280.0, 120.0),
        card(0.0, 440.0, 280.0, 120.0),
        card(500.0, 0.0, 280.0, 120.0),
        card(500.0, 220.0, 280.0, 120.0),
        card(500.0, 440.0, 280.0, 120.0),
    ];
    let wires = [
        wire((0, Side::Right, 60.0), (5, Side::Left, 60.0)),
        wire((1, Side::Right, 60.0), (3, Side::Left, 60.0)),
        wire((2, Side::Right, 60.0), (4, Side::Left, 60.0)),
        wire((0, Side::Right, 90.0), (4, Side::Left, 90.0)),
    ];
    routed(&cards, &wires);
}

/// Wires of one port leave on one line and turn off it one by one: from the
/// turn onwards each reader has its own line.
#[test]
fn lay03_wires_of_one_port_part_like_a_comb() {
    let cards = [
        card(0.0, 200.0, 280.0, 120.0),
        card(600.0, 0.0, 280.0, 120.0),
        card(600.0, 200.0, 280.0, 120.0),
        card(600.0, 400.0, 280.0, 120.0),
        card(600.0, 600.0, 280.0, 120.0),
    ];
    let wires: Vec<Wire> = (1..=4)
        .map(|reader| wire((0, Side::Right, 60.0), (reader, Side::Left, 60.0)))
        .collect();
    let routes = routed(&cards, &wires);
    // Past the port's straight piece no two of them share a line: two
    // segments on one line are the first segment of each.
    for (index, a) in routes.iter().enumerate() {
        for b in &routes[index + 1..] {
            for (place, p) in a.segments().enumerate() {
                for (other, q) in b.segments().enumerate() {
                    let level = (p.0.y - p.1.y).abs() < 0.01
                        && (q.0.y - q.1.y).abs() < 0.01
                        && (p.0.y - q.0.y).abs() < 0.5;
                    let upright = (p.0.x - p.1.x).abs() < 0.01
                        && (q.0.x - q.1.x).abs() < 0.01
                        && (p.0.x - q.0.x).abs() < 0.5;
                    let overlap = if level {
                        p.0.x.max(p.1.x).min(q.0.x.max(q.1.x))
                            - p.0.x.min(p.1.x).max(q.0.x.min(q.1.x))
                    } else if upright {
                        p.0.y.max(p.1.y).min(q.0.y.max(q.1.y))
                            - p.0.y.min(p.1.y).max(q.0.y.min(q.1.y))
                    } else {
                        continue;
                    };
                    assert!(
                        overlap < 0.5 || (place == 0 && other == 0),
                        "{:?} and {:?} lie on each other",
                        a.points,
                        b.points
                    );
                }
            }
        }
    }
}

/// Situations 1 and 2: the rails of a row do not share a level above the
/// cards, and none runs on the card tops.
#[test]
fn lay03_the_rails_of_a_row_take_their_own_levels() {
    let cards: Vec<Rect> = (0..5)
        .map(|column| card(column as f32 * 376.0, 0.0, 280.0, 160.0))
        .chain((0..5).map(|column| card(column as f32 * 376.0, 400.0, 280.0, 160.0)))
        .collect();
    // Each step after the one before it, the row below after the last of the
    // row above, and two rails that jump a step.
    let mut wires: Vec<Wire> = (0..9)
        .map(|step| wire((step, Side::Top, 140.0), (step + 1, Side::Top, 140.0)))
        .collect();
    wires.push(wire((5, Side::Top, 140.0), (8, Side::Top, 140.0)));
    wires.push(wire((0, Side::Top, 140.0), (3, Side::Top, 140.0)));
    routed(&cards, &wires);
}

/// A corridor with room for one line: the second goes the long way round
/// rather than run beside the first.
#[test]
fn lay03_a_line_goes_the_long_way_rather_than_squeeze() {
    // 44 px between the two middle cards: one track, 20 px from either.
    let cards = [
        card(0.0, 0.0, 280.0, 100.0),
        card(0.0, 400.0, 280.0, 100.0),
        card(400.0, 0.0, 280.0, 228.0),
        card(400.0, 272.0, 280.0, 228.0),
        card(800.0, 0.0, 280.0, 100.0),
        card(800.0, 400.0, 280.0, 100.0),
    ];
    let wires = [
        wire((0, Side::Right, 50.0), (5, Side::Left, 50.0)),
        wire((1, Side::Right, 50.0), (4, Side::Left, 50.0)),
    ];
    routed(&cards, &wires);
}

/// Criterion 5: `order-flow` and `fan-out` as Tidy up placed them before this
/// ticket, with 96 px between columns and 24 px under a card. Wires squeezed
/// through the 24 px and shared the 12 px edge of every card (107 breaches on
/// `order-flow`, 92 on `fan-out`); now each keeps the gap, by a longer route,
/// wherever the board has room. A few places have none: a port that feeds
/// four readers through a gap that holds three lines 20 px apart, two
/// satellites 24 px apart joined bottom to top by a wire with a jog, two
/// wires into one port with one track beside it. Tidy up widens those gaps.
#[test]
fn lay03_criterion_5_boards_tidied_before_keep_the_gap() {
    for (name, text, allowed) in [("order-flow", ORDER_FLOW, 5), ("fan-out", FAN_OUT, 3)] {
        let (cards, wires) = board(text);
        assert!(wires.len() >= 18, "{name}");
        routed_with(&cards, &wires, allowed);
    }
}

const ORDER_FLOW: &str = "\
card 0 0 280 132 begin
card 376 0 280 190 login
card 752 0 280 166 products
card 752 190 280 70 first
card 752 284 280 70 second
card 752 378 280 70 total
card 1128 0 280 190 cart
card 1504 0 280 214 order
card 0 544 280 94 created
card 376 544 280 166 receipt
card 0 662 280 96 failed
wire 0 top 140 1 top 140
wire 6 top 140 7 top 140
wire 8 top 70 9 top 140
wire 8 bottom 210 10 top 140
wire 1 right 98 2 left 74
wire 2 right 122 3 left 52
wire 2 right 122 4 left 52
wire 3 right 52 5 left 48
wire 4 right 52 5 left 56
wire 1 right 98 6 left 74
wire 3 right 52 6 left 98
wire 4 right 52 6 left 98
wire 1 right 98 7 left 74
wire 6 right 122 7 left 98
wire 5 right 52 7 left 98
wire 7 right 146 8 left 52
wire 7 right 122 9 left 51
wire 1 right 98 9 left 74
";

const FAN_OUT: &str = "\
card 0 0 280 132 begin
card 376 0 280 142 catalogue
card 376 166 280 70 first
card 752 0 280 142 product
card 1128 0 280 166 cart
card 1128 190 280 70 cartId
card 1128 284 280 70 itemId
card 1504 0 280 166 quantity
card 0 450 280 166 apply_coupon
card 376 450 280 166 note
card 752 450 280 142 stock
card 1128 450 280 166 shipping
card 1504 450 280 142 remove_coupon
card 0 832 280 142 view
card 376 832 280 190 checkout
card 376 1166 280 70 orderNumber
card 752 832 280 190 lookup
wire 0 top 140 1 top 140
wire 4 top 140 7 top 140
wire 7 top 140 8 top 140
wire 8 top 140 9 top 140
wire 9 top 140 10 top 140
wire 10 top 140 11 top 140
wire 11 top 140 12 top 140
wire 12 top 140 13 top 140
wire 13 top 140 14 top 140
wire 14 top 140 16 top 140
wire 1 right 98 2 left 52
wire 2 right 52 3 left 74
wire 3 right 98 4 left 74
wire 4 right 122 5 left 52
wire 4 right 122 6 left 52
wire 5 right 52 7 left 51
wire 6 right 52 7 left 51
wire 14 right 98 15 left 52
wire 15 right 52 16 left 51
";

/// Two ports of one card nearer than the gap (the two operands of a
/// condition): their wires arrive on two tracks, not end to end on one, and
/// are as near each other as the ports only on the straight pieces into them.
#[test]
fn lay03_two_ports_nearer_than_the_gap_get_two_tracks() {
    let cards = [
        card(0.0, 0.0, 280.0, 190.0),
        card(380.0, 400.0, 280.0, 96.0),
    ];
    let wires = [
        wire((0, Side::Right, 162.0), (1, Side::Left, 86.0)),
        wire((0, Side::Right, 138.0), (1, Side::Left, 92.0)),
    ];
    routed(&cards, &wires);
}
