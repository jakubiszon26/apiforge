//! Mock of REQ-20 (#622): the mark on the card of a request whose field
//! holds unfinished text, as a badge and as a line, in both densities, and
//! the notices of Run and Save. Throwaway.
use super::diag06a::{graph, id_of, rooms};
use super::diag07::board;
use super::*;
use crate::testing::{MINIMUM, WIDE};

const RUN: &str = "buildings has unfinished text in its address. The run used the last text that could be read: /buildings-v2?x={";
const SAVE: &str = "Saved without the unfinished text of buildings (address). The file holds /buildings-v2?x={";

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_req20_mock() {
    for (option, name) in [(0u8, "today"), (1, "v1"), (2, "v2")] {
        for (full, density) in [(false, "compact"), (true, "full")] {
            crate::canvas::MOCK_REQ20.with(|mock| mock.set(option));
            let (_project, mut harness, id) = rooms("scenarios/booking-day.afs", |_| {});
            harness.set_size(WIDE);
            settle(&mut harness);
            if full {
                app(&mut harness).switch_density(crate::canvas::CardDensity::Full);
                for _ in 0..40 {
                    harness.step();
                }
                choose(&mut harness, "Tidy up");
                for _ in 0..40 {
                    harness.step();
                }
            }
            harness.step();
            let card = id_of(&graph(&harness, &id), "buildings");
            // As the board opens.
            let rect = board(&harness, &id).debug_card_rect(&card).expect("drawn");
            let zoom = board(&harness, &id).debug_camera().1;
            crate::testing::capture(
                &mut harness,
                &format!("req20-{density}-open-{name}"),
                Some(egui::Rect::from_center_size(rect.center(), egui::vec2(420.0, 240.0))),
            );
            std::fs::write(
                crate::testing::capture_dir().join(format!("req20-{density}-open-{name}.txt")),
                format!("{zoom}"),
            )
            .unwrap();
            // Readable.
            app(&mut harness).canvases.get_mut(&id).unwrap().debug_show(&card, if full { 0.6 } else { 1.0 });
            for _ in 0..20 {
                harness.step();
            }
            harness.step();
            let rect = board(&harness, &id).debug_card_rect(&card).expect("drawn");
            crate::testing::capture(
                &mut harness,
                &format!("req20-{density}-100-{name}"),
                Some(egui::Rect::from_center_size(
                    rect.center(),
                    egui::vec2(420.0, (rect.height() + 70.0).max(300.0)),
                )),
            );
            if option != 0 && !full {
                for (text, kind) in [(RUN, "run"), (SAVE, "save")] {
                    if option != 1 {
                        continue;
                    }
                    app(&mut harness).canvases.get_mut(&id).unwrap().set_notice(text);
                    harness.step();
                    harness.step();
                    let area = board(&harness, &id).debug_board();
                    crate::testing::capture(
                        &mut harness,
                        &format!("req20-notice-{kind}"),
                        Some(egui::Rect::from_min_size(
                            egui::pos2(area.center().x - 400.0, area.top()),
                            egui::vec2(800.0, 90.0),
                        )),
                    );
                }
            }
        }
    }
    // Full windows of both variants as the board opens.
    for (option, name) in [(1u8, "v1"), (2, "v2")] {
        for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
            crate::canvas::MOCK_REQ20.with(|mock| mock.set(option));
            let (_project, mut harness, _id) = rooms("scenarios/booking-day.afs", |_| {});
            harness.set_size(size);
            settle(&mut harness);
            harness.step();
            crate::testing::capture(&mut harness, &format!("req20-window-{name}-{suffix}"), None);
        }
    }
    crate::canvas::MOCK_REQ20.with(|mock| mock.set(0));
}
