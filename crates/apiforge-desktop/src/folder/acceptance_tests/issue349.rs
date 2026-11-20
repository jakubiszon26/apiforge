//! #349: the inputs of a *Data condition* are spaced so their wires keep
//! the 20 px gap. Owner decision A of 2026-11-16: the sockets on one side of
//! a compact card are 20 px apart and the card is as much taller as that
//! takes, about 28 px for three inputs. On the reference scenarios of
//! `fixtures/scenarios/lay/`, with every wire and check drawn.
use super::lay28::{drawn, graph, tidied};
use super::*;
use crate::canvas::WireMode;
use apiforge_core::model::NodeKind;
use apiforge_layout::GAP;

/// A satellite's box with one socket on its line: the header, the line and
/// the result (a value has no result), as LAY-21 measured them.
const VALUE: f32 = 70.0;
const CHECK: f32 = 96.0;

/// Criterion 1: on every reference board the lines into one card end 20 px
/// apart or more, and so do the lines out of it that leave different
/// sockets: no two sockets on one side of a card are nearer than the gap.
#[test]
fn issue349_criterion_1_sockets_of_a_card_are_a_gap_apart() {
    for name in ["order-flow", "shop-30", "shop-50", "fan-out"] {
        let (_project, harness, id) = tidied(name, WIDE, true, WireMode::All);
        let (routes, cards) = drawn(&harness, &id);
        let mut judged = 0;
        for card in &cards {
            // The ends of the lines on the card's left and right sides.
            for side in [card.min.x, card.max.x] {
                let mut sockets: Vec<f32> = routes
                    .iter()
                    .flat_map(|route| [route.points[0], route.points[route.points.len() - 1]])
                    .filter(|end| {
                        (end.x - side).abs() < 0.5 && end.y > card.min.y && end.y < card.max.y
                    })
                    .map(|end| end.y)
                    .collect();
                sockets.sort_by(f32::total_cmp);
                sockets.dedup_by(|a, b| (*a - *b).abs() < 0.5);
                for pair in sockets.windows(2) {
                    judged += 1;
                    assert!(
                        pair[1] - pair[0] >= GAP - 0.01,
                        "{name}: two sockets {} px apart on the card at ({}, {})",
                        pair[1] - pair[0],
                        card.min.x,
                        card.min.y
                    );
                }
            }
        }
        assert!(judged > 0, "{name}: cards with two sockets on a side");
    }
}

/// Criterion 2: a card grows by what its sockets take and no more. A value,
/// a check or a *Data condition* with one wire into its line is as tall as
/// before; with two it is 8 px taller and with three, 28 px: `checkout_ok`
/// of `fan-out`, 96 px before, is 124 px.
#[test]
fn issue349_criterion_2_a_card_grows_by_what_its_sockets_take() {
    let mut grown = [0; 4];
    for name in ["shop-30", "fan-out"] {
        let (_project, harness, id) = tidied(name, WIDE, true, WireMode::All);
        let canvas = super::lay28::board(&harness, &id);
        // The data lines: since LAY-45 a branch of an `if` enters a card at
        // its order socket on the left, which is no input.
        let routes: Vec<Vec<egui::Pos2>> = canvas
            .debug_wires()
            .iter()
            .filter(|wire| !wire.dashed && !wire.points.is_empty())
            .map(|wire| wire.points.clone())
            .collect();
        for node in graph(&harness, &id).nodes {
            let base = match node.kind {
                NodeKind::Value => VALUE,
                NodeKind::Assert | NodeKind::Condition => CHECK,
                _ => continue,
            };
            let world = canvas.debug_world(&node.id).expect("the card is drawn");
            // The wires into the card's line, on its left side.
            let wired = routes
                .iter()
                .filter(|route| {
                    let end = route[route.len() - 1];
                    (end.x - world.left()).abs() < 0.5
                        && end.y > world.top()
                        && end.y < world.bottom()
                })
                .count();
            let more = match wired {
                0 | 1 => 0.0,
                sockets => (sockets - 1) as f32 * GAP + 12.0 - 24.0,
            };
            assert_eq!(
                world.height(),
                base + more,
                "{name}: {} with {wired} wired inputs",
                node.handle
            );
            grown[wired.min(3)] += 1;
            if node.handle == "checkout_ok" {
                assert_eq!((wired, world.height()), (3, 124.0), "checkout_ok");
            }
        }
    }
    assert!(
        grown[1] > 0 && grown[2] > 0 && grown[3] > 0,
        "cards with one, two and three wired inputs were measured: {grown:?}"
    );
}
