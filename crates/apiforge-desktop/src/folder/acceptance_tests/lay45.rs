//! LAY-45 (#468): the ordering lines of a chain read left to right, row by
//! row. The ticket opens with a feasibility gate on its criteria 3, 8 and
//! 10: the reference boards routed side to side keep the gap, cross no more
//! than before and fit as before. These are the gate's tests; the
//! criteria's own tests follow when the gate holds.
use super::lay28::{board, drawn, tidied};
use super::*;
use crate::canvas::WireMode;
use apiforge_layout::{GAP, Route, crossing, groups, packed_outside_groups};

const REFERENCE: [&str; 5] = ["order-flow", "shop-30", "shop-50", "fan-out", "labels"];
const FIT: &str = "Fit the view";

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

/// What the window drew before this ticket, on `main` after LANG-13
/// (6a4a3da), at 1600 × 1000: each reference board in each *Wires* mode
/// with its checks folded and drawn, its crossings, and the zoom Fit gives
/// at 960 × 540 and at 1600 × 1000.
/// (board, wires, checks drawn, crossings, fit at 960, fit at 1600)
const BEFORE: [(&str, WireMode, bool, usize, f32, f32); 20] = [
    ("order-flow", WireMode::All, false, 17, 0.259, 0.596),
    ("order-flow", WireMode::All, true, 40, 0.254, 0.583),
    ("order-flow", WireMode::Chips, false, 3, 0.203, 0.480),
    ("order-flow", WireMode::Chips, true, 6, 0.201, 0.476),
    ("shop-30", WireMode::All, false, 93, 0.173, 0.438),
    ("shop-30", WireMode::All, true, 143, 0.162, 0.410),
    ("shop-30", WireMode::Chips, false, 0, 0.195, 0.490),
    ("shop-30", WireMode::Chips, true, 1, 0.179, 0.466),
    ("shop-50", WireMode::All, false, 289, 0.092, 0.232),
    ("shop-50", WireMode::All, true, 427, 0.086, 0.217),
    ("shop-50", WireMode::Chips, false, 2, 0.104, 0.271),
    ("shop-50", WireMode::Chips, true, 11, 0.097, 0.253),
    ("fan-out", WireMode::All, false, 65, 0.212, 0.538),
    ("fan-out", WireMode::All, true, 73, 0.209, 0.518),
    ("fan-out", WireMode::Chips, false, 0, 0.233, 0.549),
    ("fan-out", WireMode::Chips, true, 0, 0.189, 0.442),
    ("labels", WireMode::All, false, 0, 0.145, 0.367),
    ("labels", WireMode::All, true, 5, 0.130, 0.329),
    ("labels", WireMode::Chips, false, 0, 0.139, 0.360),
    ("labels", WireMode::Chips, true, 0, 0.124, 0.323),
];

/// One board as the window draws it after Tidy up: its crossings, whether
/// its lines keep the gap, and its Fit zoom at 960 × 540 and 1600 × 1000.
fn measure(name: &str, wires: WireMode, expanded: bool) -> (usize, bool, f32, f32) {
    let (_project, mut harness, id) = tidied(name, WIDE, expanded, wires);
    let (routes, cards) = drawn(&harness, &id);
    let found = crossings(&routes);
    let kept = packed_outside_groups(&routes, &cards, &groups(&cards), GAP).is_empty();
    choose(&mut harness, FIT);
    let wide = board(&harness, &id).debug_zoom();
    let (_project, mut small, id) = tidied(name, crate::testing::MINIMUM, expanded, wires);
    choose(&mut small, FIT);
    let narrow = board(&small, &id).debug_zoom();
    (found, kept, narrow, wide)
}

/// The gate's table: every reference board in both modes and both check
/// states, measured once and printed, for the pull request and #468.
fn table() -> Vec<(&'static str, WireMode, bool, usize, bool, f32, f32)> {
    let rows: Vec<_> = REFERENCE
        .iter()
        .flat_map(|name| {
            [WireMode::All, WireMode::Chips]
                .into_iter()
                .flat_map(move |wires| [false, true].map(|expanded| (*name, wires, expanded)))
        })
        .map(|(name, wires, expanded)| {
            let (found, kept, narrow, wide) = measure(name, wires, expanded);
            eprintln!(
                "LAY-45 {name} {wires:?} drawn={expanded} crossings={found} gap={kept} fit960={narrow:.3} fit1600={wide:.3}"
            );
            (name, wires, expanded, found, kept, narrow, wide)
        })
        .collect();
    rows
}

/// Criteria 3, 8 and 10, the feasibility gate: on every reference board, in
/// both modes and both check states, the lines keep the gap (`packed` 0),
/// cross no more than before, and Fit shows the board at a zoom not more
/// than 0.02 below the one before; `shop-30` with every wire has at most 76
/// crossings with its checks folded and 116 with them drawn (LAY-28's
/// limits).
#[test]
fn lay45_criterion_8_no_more_crossings_than_before() {
    for (name, wires, expanded, found, kept, narrow, wide) in table() {
        let label = format!("{name}, {wires:?}, checks drawn: {expanded}");
        let &(_, _, _, before, fit_narrow, fit_wide) = BEFORE
            .iter()
            .find(|row| row.0 == name && row.1 == wires && row.2 == expanded)
            .expect("a figure from before");
        let most = match (name, wires, expanded) {
            ("shop-30", WireMode::All, false) => 76,
            ("shop-30", WireMode::All, true) => 116,
            _ => before,
        };
        assert!(found <= most, "{label}: {found} crossings, at most {most}");
        assert!(kept, "{label}: a line within 20 px of another or a card");
        assert!(
            narrow >= fit_narrow - 0.02 && wide >= fit_wide - 0.02,
            "{label}: Fit at {narrow:.3} and {wide:.3}, {fit_narrow:.3} and {fit_wide:.3} before"
        );
    }
}
