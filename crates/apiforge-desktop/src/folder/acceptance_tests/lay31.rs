//! Mock-up of LAY-31 (#384): a step and its checks are one group. Throwaway;
//! `APIFORGE_LAY31` chooses what the layout does (unset: `main`).
use super::lay23::{Checks, board, graph, measures, show, tidied};
use super::*;
use apiforge_core::graph::{WireKind, wires as document_wires};
use std::collections::BTreeMap;

/// The gap between each check or value and the card it hangs under (its
/// latest subject), and the lines of other cards that cross that gap.
fn report(harness: &FolderHarness, id: &str, label: &str) {
    let graph = graph(harness, id);
    let canvas = board(harness, id);
    let cards: BTreeMap<String, egui::Rect> = graph
        .nodes
        .iter()
        .filter_map(|node| Some((node.id.clone(), canvas.debug_world(&node.id)?)))
        .collect();
    let handle = |node: &str| {
        graph
            .nodes
            .iter()
            .find(|candidate| candidate.id == node)
            .map_or_else(|| node.to_owned(), |node| node.handle.clone())
    };
    let wires = canvas.debug_wires();
    let (lines, crossings, pairs) = measures(harness, id);
    let bounds = cards
        .values()
        .fold(egui::Rect::NOTHING, |all, card| all.union(*card));
    errln!(
        "LAY-31 {label}: board {:.0} x {:.0}, {lines} lines, {crossings} crossings, {pairs} coincident pairs",
        bounds.width(),
        bounds.height()
    );
    // The groups: every card with the cards stacked under it in its column
    // until the next row.
    let mut columns: BTreeMap<i64, Vec<&apiforge_core::format::Node>> = BTreeMap::new();
    for node in &graph.nodes {
        if let Some(rect) = cards.get(&node.id) {
            columns.entry(rect.center().x.round() as i64).or_default().push(node);
        }
    }
    let mut worst = 0.0_f32;
    let mut total = 0.0_f32;
    let mut count = 0;
    let mut foreign_total = 0;
    let mut own_length = 0.0_f32;
    for stack in columns.values_mut() {
        stack.sort_by(|a, b| cards[&a.id].top().total_cmp(&cards[&b.id].top()));
        let mut group: Vec<&str> = Vec::new();
        for pair in stack.windows(2) {
            let (upper, lower) = (cards[&pair[0].id], cards[&pair[1].id]);
            if crate::canvas::shape::on_spine(pair[1].kind) {
                group.clear();
                continue;
            }
            if group.is_empty() {
                group.push(pair[0].id.as_str());
            }
            group.push(pair[1].id.as_str());
            let gap = egui::Rect::from_min_max(
                egui::pos2(upper.left().min(lower.left()), upper.bottom()),
                egui::pos2(upper.right().max(lower.right()), lower.top()),
            );
            let inside = |wire: &&crate::canvas::DrawnWire| {
                wire.points.windows(2).any(|pair| {
                    let segment = egui::Rect::from_two_pos(pair[0], pair[1]);
                    segment.left() < gap.right() - 1.0
                        && segment.right() > gap.left() + 1.0
                        && segment.top() < gap.bottom() - 0.5
                        && segment.bottom() > gap.top() + 0.5
                })
            };
            let member = |id: &str| group.contains(&id) || stack.iter().any(|node| node.id == id);
            let foreign = wires
                .iter()
                .filter(|wire| !wire.revealed)
                .filter(|wire| {
                    !member(wire.target.as_str())
                        && !wire.source.as_deref().is_some_and(member)
                })
                .filter(inside)
                .count();
            worst = worst.max(gap.height());
            total += gap.height();
            count += 1;
            foreign_total += foreign;
            errln!(
                "LAY-31 {label}: {:<18} under {:<18} gap {:>4.0}, foreign lines in the gap {foreign}",
                pair[1].handle,
                pair[0].handle,
                gap.height()
            );
        }
    }
    // The lines between a card and a card hung in its own column.
    for wire in wires.iter().filter(|wire| !wire.revealed && !wire.dashed) {
        let (Some(source), Some(target)) = (
            wire.source.as_ref().and_then(|id| cards.get(id)),
            cards.get(&wire.target),
        ) else {
            continue;
        };
        if (source.center().x - target.center().x).abs() < 1.0 && target.top() > source.top() {
            own_length += wire
                .points
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).length())
                .sum::<f32>();
        }
    }
    let _ = handle;
    errln!(
        "LAY-31 {label}: {count} hung cards, mean gap {:.0}, worst {worst:.0}, foreign lines in gaps {foreign_total}, lines from a card to one hung under it {own_length:.0} px, fit {:.0} %",
        total / count.max(1) as f32,
        canvas.debug_zoom() * 100.0
    );
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay31_mock() {
    let variant = std::env::var("APIFORGE_LAY31").unwrap_or_else(|_| "main".to_owned());
    for (size, suffix) in [(WIDE, "1600"), (crate::testing::MINIMUM, "960")] {
        let (_project, mut harness, id) = tidied("shop-30", size, Checks::Expanded);
        choose(&mut harness, "Fit the view");
        harness.step();
        if suffix == "1600" {
            report(&harness, &id, &variant);
        }
        crate::testing::capture(
            &mut harness,
            &format!("mock-{variant}-shop-30-fit-{suffix}"),
            None,
        );
        for (handle, zoom, name) in [
            ("cart_total", 0.6, "cart"),
            ("checkout_status", 0.6, "checkout"),
            ("logged_in", 1.0, "login"),
        ] {
            if suffix == "960" && name != "cart" {
                continue;
            }
            show(&mut harness, &id, handle, zoom);
            crate::testing::capture(
                &mut harness,
                &format!("mock-{variant}-{name}-{suffix}"),
                None,
            );
        }
    }
}
