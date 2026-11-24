//! Mock of DIAG-23 (#612): where the cards stand after an add when the tidy
//! of the new board does not start where the kept cards are. Today, option
//! A and option B, on `rooms/booking-day`, for the comparison board.
use super::diag06a::{graph, id_of, rooms};
use super::diag07::board;
use super::lay16::click_at;
use super::*;
use crate::testing::{MINIMUM, WIDE};

fn add_after_profile(harness: &mut FolderHarness, id: &str, label: &str) {
    let model = graph(harness, id);
    app(harness)
        .selections
        .insert(id.to_owned(), vec![id_of(&model, "profile")]);
    settle(harness);
    harness.step();
    harness.get_by_label("Add step").click();
    settle(harness);
    harness.step();
    harness.event(egui::Event::Text(label.to_owned()));
    harness.step();
    harness.step();
    let menu = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("board-action-menu")))
        .expect("the menu is drawn");
    let entry = harness
        .query_all_by_label(label)
        .map(|node| node.rect())
        .filter(|rect| menu.contains_rect(*rect))
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap_or_else(|| panic!("the menu offers {label}"));
    click_at(harness, entry.center(), egui::PointerButton::Primary, 1);
    settle(harness);
    harness.step();
    assert_eq!(graph(harness, id).nodes.len(), model.nodes.len() + 1);
}

fn rects(harness: &FolderHarness, id: &str) -> serde_json::Value {
    let model = graph(harness, id);
    let cards: Vec<serde_json::Value> = board(harness, id)
        .debug_cards()
        .iter()
        .map(|(card, rect)| {
            let node = model.nodes.iter().find(|node| &node.id == card);
            serde_json::json!({
                "id": card,
                "handle": node.map(|node| node.handle.clone()),
                "by_hand": node.map(|node| !node.auto_placed),
                "world": node.map(|node| [node.position.x, node.position.y]),
                "rect": [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
            })
        })
        .collect();
    serde_json::Value::Array(cards)
}

fn overlaps(harness: &FolderHarness, id: &str) -> Vec<String> {
    let model = graph(harness, id);
    let name = |card: &str| {
        model
            .nodes
            .iter()
            .find(|node| node.id == card)
            .map_or_else(|| "chip".to_owned(), |node| node.handle.clone())
    };
    let cards = board(harness, id).debug_cards();
    let mut found = Vec::new();
    for (index, (one, a)) in cards.iter().enumerate() {
        for (other, b) in &cards[index + 1..] {
            if a.shrink(0.5).intersects(b.shrink(0.5)) {
                found.push(format!("{} on {}", name(one), name(other)));
            }
        }
    }
    found
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag23_mock() {
    // Placed by hand before the add: a step before the place and one after it.
    let hand = std::env::var("DIAG23_HAND").unwrap_or_else(|_| "token:30:-70,rooms:30:60".into());
    for (case, label, full) in [(1, "Sign in", false), (2, "HTTP request", true)] {
        for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
            for (option, name) in [(0u8, "today"), (1, "a"), (2, "b")] {
                crate::board_edit::MOCK_DIAG23.with(|mock| mock.set(0));
                let (_project, mut harness, id) = rooms("scenarios/booking-day.afs", |_| {});
                harness.set_size(size);
                settle(&mut harness);
                if full {
                    app(&mut harness).switch_density(crate::canvas::CardDensity::Full);
                    settle(&mut harness);
                }
                choose(&mut harness, "Tidy up");
                settle(&mut harness);
                harness.step();
                for placed in hand.split(',').filter(|placed| !placed.is_empty()) {
                    let mut parts = placed.split(':');
                    let card = parts.next().unwrap();
                    let dx: f64 = parts.next().unwrap().parse().unwrap();
                    let dy: f64 = parts.next().unwrap().parse().unwrap();
                    let moved = graph(&harness, &id)
                        .nodes
                        .iter()
                        .find(|node| node.handle == card)
                        .map(|node| {
                            (
                                node.id.clone(),
                                apiforge_core::format::Position {
                                    x: node.position.x + dx,
                                    y: node.position.y + dy,
                                },
                            )
                        })
                        .unwrap();
                    app(&mut harness)
                        .active_board_command(crate::canvas::Command::Move(vec![moved]));
                    settle(&mut harness);
                }
                // The first two rows, and the row above them that today's
                // placement uses, in view at one zoom for every option.
                let (zoom, corner) = match (case, suffix) {
                    (1, "960") => (0.32, egui::pos2(-30.0, -190.0)),
                    (1, _) => (0.62, egui::pos2(-30.0, -190.0)),
                    (_, "960") => (0.19, egui::pos2(-780.0, -440.0)),
                    _ => (0.44, egui::pos2(-457.0, -440.0)),
                };
                let profile = id_of(&graph(&harness, &id), "profile");
                app(&mut harness)
                    .canvases
                    .get_mut(&id)
                    .unwrap()
                    .debug_show(&profile, zoom);
                settle(&mut harness);
                harness.step();
                // `corner` of the board in the top-left corner of the view.
                let canvas = app(&mut harness).canvases.get_mut(&id).unwrap();
                let (pan, _) = canvas.debug_camera();
                let world = canvas.debug_world(&profile).unwrap().min;
                let drawn = canvas.debug_to_screen(egui::Rect::from_min_max(world, world)).min;
                let wanted = canvas.debug_board().min + (world - corner) * zoom;
                canvas.debug_set_camera((pan + (wanted - drawn), zoom));
                settle(&mut harness);
                harness.step();
                let camera = board(&harness, &id).debug_camera();
                let before = rects(&harness, &id);
                let before_overlaps = overlaps(&harness, &id);
                let stem = format!("diag23-case{case}-{name}-{suffix}");
                if option == 0 {
                    crate::testing::capture(
                        &mut harness,
                        &format!("diag23-case{case}-before-{suffix}"),
                        None,
                    );
                }
                crate::board_edit::MOCK_DIAG23.with(|mock| mock.set(option));
                add_after_profile(&mut harness, &id, label);
                crate::board_edit::MOCK_DIAG23.with(|mock| mock.set(0));
                app(&mut harness)
                    .selections
                    .insert(id.clone(), Vec::new());
                app(&mut harness)
                    .canvases
                    .get_mut(&id)
                    .unwrap()
                    .debug_set_camera(camera);
                settle(&mut harness);
                harness.step();
                harness.step();
                assert_eq!(board(&harness, &id).debug_camera(), camera);
                let area = board(&harness, &id).debug_board();
                let data = serde_json::json!({
                    "case": case, "option": name, "size": suffix,
                    "zoom": camera.1,
                    "board": [area.min.x, area.min.y, area.max.x, area.max.y],
                    "before": before,
                    "after": rects(&harness, &id),
                    "overlaps_before": before_overlaps,
                    "overlaps_after": overlaps(&harness, &id),
                });
                crate::testing::capture(&mut harness, &stem, None);
                std::fs::write(
                    crate::testing::capture_dir().join(format!("{stem}.json")),
                    serde_json::to_string_pretty(&data).unwrap(),
                )
                .unwrap();
            }
        }
    }
}
