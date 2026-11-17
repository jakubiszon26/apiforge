//! LAY-31 (#384): a step and its checks are one group.
//!
//! Cards one above the other at most the stack gap apart are one group
//! (`groups`). A line of another card does not enter a group's box, and a
//! line of the group may run nearer than the gap to the group's own cards;
//! `packed_outside_groups` is LAY-03's measure with that exception.

use apiforge_layout::{
    GAP, Point, Port, Rect, Route, Routing, STACK_GAP, Side, Size, Wire, groups,
    packed_outside_groups, route, route_grouped, violations,
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

/// A step at 380 with two checks hung under it, a card left of it and one
/// right of it.
fn board() -> Vec<Rect> {
    vec![
        card(380.0, 0.0, 280.0, 120.0),
        card(380.0, 120.0 + STACK_GAP, 280.0, 70.0),
        card(380.0, 120.0 + STACK_GAP + 70.0 + STACK_GAP, 280.0, 70.0),
        card(0.0, 0.0, 280.0, 400.0),
        card(760.0, 0.0, 280.0, 400.0),
    ]
}

fn inside(route: &Route, area: Rect) -> bool {
    route.segments().any(|(from, to)| {
        let (low_x, high_x) = (from.x.min(to.x), from.x.max(to.x));
        let (low_y, high_y) = (from.y.min(to.y), from.y.max(to.y));
        low_x < area.max.x - 0.5
            && high_x > area.min.x + 0.5
            && low_y < area.max.y - 0.5
            && high_y > area.min.y + 0.5
    })
}

#[test]
fn lay31_cards_at_most_the_stack_gap_apart_are_one_group() {
    assert_eq!(STACK_GAP, 24.0);
    let cards = board();
    let found = groups(&cards);
    assert_eq!(found[0], found[1]);
    assert_eq!(found[1], found[2]);
    assert_ne!(found[0], found[3]);
    assert_ne!(found[0], found[4]);
    assert_ne!(found[3], found[4]);
    // Farther apart, or side by side: no group.
    let apart = vec![card(0.0, 0.0, 280.0, 100.0), card(0.0, 130.0, 280.0, 100.0)];
    let found = groups(&apart);
    assert_ne!(found[0], found[1]);
    let beside = vec![
        card(0.0, 0.0, 280.0, 100.0),
        card(300.0, 110.0, 280.0, 100.0),
    ];
    let found = groups(&beside);
    assert_ne!(found[0], found[1]);
}

/// Criterion 2 on a board of its own: a line between the cards left and
/// right of a group, level with the group's gaps, goes round the group,
/// while Tidy up asks (lines may pass between cards at a price) and as the
/// board draws it.
#[test]
fn lay31_a_line_of_other_cards_goes_round_a_group() {
    let cards = board();
    let found = groups(&cards);
    let area = Rect::bounding(cards[..3].iter().copied()).unwrap();
    let wires = [
        wire((3, Side::Right, 132.0), (4, Side::Left, 132.0)),
        wire((3, Side::Right, 228.0), (4, Side::Left, 228.0)),
    ];
    for settings in [Routing::sizing(), Routing::default()] {
        for one in &wires {
            let routes = route_grouped(&cards, &found, &[*one], &settings, &[]);
            assert!(routes[0].complete);
            assert!(
                !inside(&routes[0], area),
                "{:?} enters the group",
                routes[0].points
            );
        }
    }
    let routes = route_grouped(&cards, &found, &wires, &Routing::default(), &[]);
    assert_eq!(violations(&routes, &wires, &cards), []);
    assert_eq!(packed_outside_groups(&routes, &cards, &found, GAP), []);
}

/// Criterion 4 on a board of its own: a line from a step to a check hung
/// under it runs inside the group, near its cards, and under none of them;
/// LAY-03's measure does not count that, and still counts what a line does
/// outside its group.
#[test]
fn lay31_a_line_of_the_group_runs_inside_it() {
    let cards = board();
    let found = groups(&cards);
    let wires = [wire((0, Side::Right, 98.0), (2, Side::Left, 40.0))];
    let routes = route_grouped(&cards, &found, &wires, &Routing::default(), &[]);
    assert!(routes[0].complete, "{:?}", routes[0].points);
    assert_eq!(violations(&routes, &wires, &cards), []);
    assert_eq!(packed_outside_groups(&routes, &cards, &found, GAP), []);
    let area = Rect::bounding(cards[..3].iter().copied())
        .unwrap()
        .inflate(GAP);
    assert!(
        routes[0].points.iter().all(|point| area.contains(*point)),
        "it stays by its group: {:?}",
        routes[0].points
    );
    // Without groups the same line keeps the gap from both checks and
    // the measure counts nothing; a breach outside a group is counted.
    let plain = route(&cards, &wires, &Routing::default());
    assert!(plain[0].complete);
    let near = Route {
        points: vec![
            Point::new(280.0, 300.0),
            Point::new(370.0, 300.0),
            Point::new(370.0, 500.0),
        ],
        ..Route::default()
    };
    assert!(!packed_outside_groups(&[near], &cards, &found, GAP).is_empty());
}
