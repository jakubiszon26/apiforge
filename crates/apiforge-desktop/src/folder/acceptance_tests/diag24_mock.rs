//! Mock of DIAG-24: the menu of a value, a check and an `if`, and where a
//! request taken from it lands. Today, option A and option B, on
//! `rooms/booking-day`, for the comparison board. Throwaway.
use super::diag06a::{graph, id_of, rooms};
use super::diag07::board;
use super::lay16::click_at;
use super::*;
use crate::testing::WIDE;

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag24_mock() {
    for (handle, port) in [
        ("profile", "body"),
        ("slot", "value"),
        ("big_enough", "value"),
        ("booked", "__true"),
    ] {
        for (option, name) in [(0u8, "today"), (1, "a"), (2, "b")] {
            crate::canvas::MOCK_DIAG24.with(|mock| mock.set(option));
            let (_project, mut harness, id) = rooms("scenarios/booking-day.afs", |_| {});
            harness.set_size(WIDE);
            settle(&mut harness);
            harness.step();
            let model = graph(&harness, &id);
            let card = id_of(&model, handle);
            let camera = board(&harness, &id).debug_camera();
            let area = board(&harness, &id).debug_board();
            let at = area.min + egui::vec2(500.0, 20.0);
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .unwrap()
                .debug_menu_from(at, (card, port.to_owned()), handle == "profile");
            harness.step();
            harness.step();
            let menu = harness
                .ctx
                .memory(|memory| memory.area_rect(egui::Id::new("board-action-menu")))
                .expect("the menu is drawn");
            crate::testing::capture(
                &mut harness,
                &format!("diag24-menu-{handle}-{name}"),
                Some(menu.expand(6.0)),
            );
            let entry = harness
                .query_all_by_label("HTTP request")
                .map(|node| node.rect())
                .filter(|rect| menu.contains_rect(*rect))
                .min_by(|a, b| a.top().total_cmp(&b.top()));
            let Some(entry) = entry else {
                crate::canvas::MOCK_DIAG24.with(|mock| mock.set(0));
                continue;
            };
            click_at(&mut harness, entry.center(), egui::PointerButton::Primary, 1);
            settle(&mut harness);
            harness.step();
            crate::canvas::MOCK_DIAG24.with(|mock| mock.set(0));
            let after = graph(&harness, &id);
            let new = after
                .nodes
                .iter()
                .find(|node| !model.nodes.iter().any(|old| old.id == node.id))
                .expect("the new block")
                .clone();
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .unwrap()
                .debug_set_camera(camera);
            settle(&mut harness);
            harness.step();
            harness.step();
            let text = source_of(&harness, &id);
            let line = text
                .lines()
                .position(|line| line.trim_start().starts_with(&format!("{} =", new.handle)))
                .map_or(0, |index| index + 1);
            let order = apiforge_core::graph::sequence(&after);
            let number = order.iter().position(|id| *id == new.id).map_or(0, |i| i + 1);
            std::fs::write(
                crate::testing::capture_dir().join(format!("diag24-place-{handle}-{name}.txt")),
                format!(
                    "line {line} of {} · number {number} of {}\n{}",
                    text.lines().count(),
                    order.len(),
                    text.lines()
                        .skip(line.saturating_sub(3))
                        .take(8)
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            )
            .unwrap();
            crate::testing::capture(
                &mut harness,
                &format!("diag24-place-{handle}-{name}"),
                Some(area),
            );
        }
    }
}
