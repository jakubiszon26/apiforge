//! MOCK (LAY-14, #298): the views of the mock-up. Throwaway.
use super::lay23::{Checks, board, graph, handle_of, id_of, show, tidied};
use super::*;
use crate::canvas::MockTrace;
use egui::{Pos2, Rect, vec2};

fn board_mut<'a>(harness: &'a mut FolderHarness, id: &str) -> &'a mut crate::canvas::Canvas {
    app(harness)
        .canvases
        .get_mut(id)
        .expect("the board is drawn")
}

/// The drawn wire from `source` into `target`: its id and a point on its
/// longest run, on screen.
fn wire(harness: &FolderHarness, id: &str, source: &str, target: &str) -> (String, Pos2) {
    let graph = graph(harness, id);
    let canvas = board(harness, id);
    let wire = canvas
        .debug_wires()
        .iter()
        .find(|wire| {
            !wire.dashed
                && wire
                    .source
                    .as_deref()
                    .map(|s| handle_of(&graph, s))
                    .as_deref()
                    == Some(source)
                && handle_of(&graph, &wire.target) == target
        })
        .unwrap_or_else(|| panic!("no wire {source} -> {target}"));
    let (a, b) = wire
        .points
        .windows(2)
        .map(|pair| (pair[0], pair[1]))
        .max_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1)))
        .unwrap();
    let at = a + (b - a) * 0.4;
    (
        wire.id.clone(),
        canvas.debug_to_screen(Rect::from_min_max(at, at)).min,
    )
}

/// The wires of the chip whose statement is `statement`, and its box.
fn chip(harness: &FolderHarness, id: &str, statement: &str) -> (Vec<String>, Rect) {
    let tag = board(harness, id)
        .debug_tags()
        .iter()
        .find(|tag| tag.tag.statement == statement)
        .unwrap_or_else(|| panic!("no chip {statement}"));
    (tag.tag.wires.clone(), tag.rect)
}

fn mock(harness: &mut FolderHarness, id: &str, trace: Option<MockTrace>) {
    board_mut(harness, id).mock = trace;
    harness.step();
    harness.step();
}

/// A capture that leaves the pointer where it is.
fn save(harness: &mut FolderHarness, name: &str) {
    harness.step();
    let image = harness.render().unwrap();
    let directory = crate::testing::capture_dir();
    fs::create_dir_all(&directory).unwrap();
    image.save(directory.join(format!("{name}.png"))).unwrap();
}

fn point(harness: &mut FolderHarness, at: Pos2) {
    harness.event(egui::Event::PointerMoved(at));
    harness.step();
    harness.step();
}

fn click(harness: &mut FolderHarness, at: Pos2) {
    super::lay16::press(harness, at);
    super::lay16::release(harness, at);
    settle(harness);
    harness.step();
}

fn away(harness: &mut FolderHarness) {
    harness.remove_cursor();
    harness.step();
    harness.step();
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay14() {
    for (size, suffix) in [(WIDE, "1600"), (crate::testing::MINIMUM, "960")] {
        // 1. A drawn wire: `valid_cart.value.id`, read by checkout and order.
        let (_project, mut harness, id) = tidied("shop-30", size, Checks::Collapsed);
        show(&mut harness, &id, "checkout", 0.6);
        let (wire_id, at) = wire(&harness, &id, "valid_cart", "checkout");
        let pointer = MockTrace {
            at: Some(at),
            pointer: true,
            ..Default::default()
        };
        // Main today: pointing at the wire.
        point(&mut harness, at);
        mock(&mut harness, &id, Some(pointer.clone()));
        save(&mut harness, &format!("lay14-wire-point-before-{suffix}"));
        // Proposed A: the box at the pointer.
        mock(
            &mut harness,
            &id,
            Some(MockTrace {
                wires: vec![wire_id.clone()],
                ..pointer.clone()
            }),
        );
        save(&mut harness, &format!("lay14-wire-point-a-box-{suffix}"));
        // Option B: the pill at the midpoint.
        mock(
            &mut harness,
            &id,
            Some(MockTrace {
                wires: vec![wire_id.clone()],
                pill: true,
                ..pointer.clone()
            }),
        );
        save(&mut harness, &format!("lay14-wire-point-b-pill-{suffix}"));
        // Pointing dims too (question 2, option B).
        mock(
            &mut harness,
            &id,
            Some(MockTrace {
                wires: vec![wire_id.clone()],
                dim: true,
                ..pointer.clone()
            }),
        );
        save(&mut harness, &format!("lay14-wire-point-dim-{suffix}"));
        // Main today: the wire clicked, the pointer gone.
        mock(&mut harness, &id, None);
        click(&mut harness, at);
        assert_eq!(
            board(&harness, &id).debug_selected_wires(),
            [wire_id.clone()]
        );
        away(&mut harness);
        save(&mut harness, &format!("lay14-wire-pin-before-{suffix}"));
        // Proposed: it keeps its statement and rings, the rest dims.
        let pinned = MockTrace {
            wires: vec![wire_id.clone()],
            at: Some(at),
            ..Default::default()
        };
        mock(
            &mut harness,
            &id,
            Some(MockTrace {
                dim: true,
                ..pinned.clone()
            }),
        );
        save(&mut harness, &format!("lay14-wire-pin-a-dim-{suffix}"));
        mock(&mut harness, &id, Some(pinned));
        save(&mut harness, &format!("lay14-wire-pin-c-no-dim-{suffix}"));
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
        mock(&mut harness, &id, None);

        // 2. The input row of a drawn wire: checkout's body.
        let checkout = id_of(&harness, &id, "checkout");
        if let Some(row) = board(&harness, &id).debug_input_row(&checkout, "body") {
            let at = row.left_center() + vec2(row.width() * 0.45, 2.0);
            point(&mut harness, at);
            mock(
                &mut harness,
                &id,
                Some(MockTrace {
                    at: Some(at),
                    pointer: true,
                    ..Default::default()
                }),
            );
            save(&mut harness, &format!("lay14-row-point-before-{suffix}"));
            let (chip_wires, _) = chip(&harness, &id, "expected_total → checkout.body.total");
            let mut wires = vec![wire_id.clone()];
            wires.extend(chip_wires);
            away(&mut harness);
            mock(
                &mut harness,
                &id,
                Some(MockTrace {
                    wires,
                    row: Some((checkout.clone(), "body".into())),
                    pointer: true,
                    ..Default::default()
                }),
            );
            save(&mut harness, &format!("lay14-row-point-after-{suffix}"));
            mock(&mut harness, &id, None);
        } else {
            eprintln!("NO ROW checkout.body at {suffix}");
        }

        // 3. A chip clicked: `expected_total → checkout.body.total`.
        let (chip_wires, chip_box) = chip(&harness, &id, "expected_total → checkout.body.total");
        click(&mut harness, chip_box.center());
        away(&mut harness);
        save(&mut harness, &format!("lay14-chip-click-before-{suffix}"));
        mock(
            &mut harness,
            &id,
            Some(MockTrace {
                wires: chip_wires,
                chip: true,
                dim: true,
                ..Default::default()
            }),
        );
        save(&mut harness, &format!("lay14-chip-click-after-{suffix}"));
    }
}
