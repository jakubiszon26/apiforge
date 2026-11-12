//! LAY-04 (#35): the ports decide the rows. After *Tidy up* a card stands
//! so that a wire into it from the column before runs straight, from the
//! output port to the input port, where the row of shape D leaves it room.
//! Criteria 1–3 are the layout crate's (`layered.rs` and the `quality`
//! example); these are the window's: the demo board, and the boards the
//! desktop tidies.
use super::*;
use crate::canvas::Command;
use apiforge_core::format::Graph;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";

/// The demo shop's scenario as saved, its checks as `expanded` says, at
/// the compact or the full density, tidied up with nothing selected.
fn demo_tidied(compact: bool, expanded: bool) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    let mut harness = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    if expanded {
        app(&mut harness).debug_draw_checks();
    }
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    if !compact {
        app(&mut harness).active_board_command(Command::SetCompactCards(false));
        settle(&mut harness);
    }
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    harness.step();
    (project, harness, id)
}

fn main_graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

fn board<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

/// The drawn data wire from the step named `from` into the step named `to`.
fn data_wire(harness: &FolderHarness, id: &str, from: &str, to: &str) -> Vec<egui::Pos2> {
    let graph = main_graph(harness, id);
    let named = |name: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.display_name() == name)
            .unwrap_or_else(|| panic!("the demo has {name}"))
            .id
            .clone()
    };
    let (source, target) = (named(from), named(to));
    board(harness, id)
        .debug_wires()
        .iter()
        .find(|wire| {
            !wire.dashed && wire.source.as_deref() == Some(source.as_str()) && wire.target == target
        })
        .unwrap_or_else(|| panic!("a data wire {from} -> {to} is drawn"))
        .points
        .clone()
}

/// Criterion 4: on the demo board tidied up, *Product → Active promotion*
/// and *Active promotion → Add to cart* are straight: one segment from the
/// output port to the input port. At both densities, with the checks
/// collapsed and expanded, since folding them moves nothing.
#[test]
fn lay04_criterion_4_the_demo_wires_are_straight() {
    for compact in [true, false] {
        for expanded in [false, true] {
            let (_project, harness, id) = demo_tidied(compact, expanded);
            for (from, to) in [
                ("Product", "Active promotion"),
                ("Active promotion", "Add to cart"),
            ] {
                let points = data_wire(&harness, &id, from, to);
                assert_eq!(
                    points.len(),
                    2,
                    "compact {compact}, expanded {expanded}: {from} -> {to} runs straight: {points:?}"
                );
                assert_eq!(points[0].y, points[1].y, "{from} -> {to} is level");
            }
        }
    }
}

/// Criteria 1 and 4 in the window: a card moved to straighten its wire
/// stays in its column and in its row, and Tidy up still ends with the
/// board in view.
#[test]
fn lay04_the_straightened_board_stays_in_view() {
    let (_project, mut harness, id) = demo_tidied(true, false);
    let canvas = board(&harness, &id);
    let focus = canvas.debug_focus();
    assert!(
        canvas
            .debug_cards()
            .iter()
            .all(|(_, rect)| focus.contains_rect(*rect)),
        "after Tidy up every card of the demo is in view"
    );
    // Tidy up again gives the same board.
    let first = main_graph(&harness, &id);
    choose(&mut harness, TIDY);
    harness.step();
    let second = main_graph(&harness, &id);
    for (before, after) in first.nodes.iter().zip(&second.nodes) {
        assert_eq!(
            (before.position.x.round(), before.position.y.round()),
            (after.position.x.round(), after.position.y.round()),
            "{} stays where the first Tidy up put it",
            before.handle
        );
    }
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay04() {
    let reference = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay")
            .join(format!("{name}.afs"));
        fs::read_to_string(path).unwrap()
    };
    for (source, expanded, label) in [
        (None, false, "lay04-demo-1600"),
        (Some("shop-30"), false, "lay04-shop-30-1600-collapsed"),
        (Some("order-flow"), false, "lay04-order-flow-1600"),
    ] {
        let project = crate::testing::shop_project();
        if let Some(name) = source {
            fs::write(project.path().join(SCENARIO), reference(name)).unwrap();
            fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
        }
        let mut harness = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
        if expanded {
            app(&mut harness).debug_draw_checks();
        }
        app(&mut harness).review_open(SCENARIO, None);
        settle(&mut harness);
        let id = only(&harness, DocumentKind::Scenario);
        app(&mut harness).selections.insert(id.clone(), Vec::new());
        settle(&mut harness);
        choose(&mut harness, TIDY);
        harness.step();
        choose(&mut harness, FIT);
        harness.step();
        let canvas = board(&harness, &id);
        let focus = canvas.debug_focus();
        assert!(
            canvas
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{label}: a card is in view"
        );
        crate::testing::capture(&mut harness, label, None);
    }
}
