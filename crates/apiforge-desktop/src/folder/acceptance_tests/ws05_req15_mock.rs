//! Mock-ups of WS-05 (#574) and REQ-15 (#576): design review captures for
//! the comparison boards. Mock branch only.
use super::*;
use crate::mock;
use crate::testing::MINIMUM;
use egui_kittest::kittest::NodeT;
use std::sync::atomic::Ordering::Relaxed;

const REFUSED: &str = "scenarios/double-booking.afs";
const LONG: &str = "scenarios/double-booking-notice-to-the-organiser.afs";

fn scenario(title: &str, last: &str) -> String {
    format!(
        r#"scenario "{title}" {{
  begin = start
  rooms = GET "/rooms" {{
    query {{ floor: 2 }}
    sample 200 {{ items: [{{ id: "room-a", name: "Room A", seats: 8 }}] }}
  }}
  first = POST "/bookings" {{
    body {{ roomId: rooms.body.items[0].id, start: "2026-12-01T10:00:00Z" }}
    sample 201 {{ id: "bk-1", roomId: "room-a" }}
  }}
  booked = assert first.status == 201
{second}{last}}}
"#,
        second = if last.is_empty() { SECOND } else { "" }
    )
}

const SECOND: &str = r#"  second = POST "/bookings" {
    body { roomId: rooms.body.items[0].id, start: "2026-12-01T10:00:00Z" }
    sample 409 { error: { code: "room_taken" } }
  }
  refused = assert second.status == 409
"#;

const NO_SAMPLE: &str = r#"  calendar = GET "/rooms/{{rooms.body.items[0].id}}/bookings" {
    query { day: "2026-12-01" }
  }
  one_booking = assert calendar.body.items[0].id == first.body.id
"#;

fn rooms(size: egui::Vec2, open: &str, last: &str) -> (tempfile::TempDir, FolderHarness) {
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Room booking".into();
    project.environment.base_url = "https://rooms.example.test".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = apiforge_project::ProjectSession::open(directory.path()).unwrap();
    for (path, title) in [
        ("scenarios/booking-day.afs", "A day of bookings"),
        (REFUSED, "Double booking is refused"),
        (
            LONG,
            "The organiser is told when a double booking is refused",
        ),
    ] {
        session
            .create_document(
                DocumentKind::Scenario,
                path,
                &scenario(title, if path == REFUSED { last } else { "" }),
            )
            .unwrap();
    }
    drop(session);
    let mut harness = crate::testing::folder_harness_in(directory.path(), size, Mode::Diagram);
    app(&mut harness).review_open(open, None);
    settle(&mut harness);
    (directory, harness)
}

fn note(name: &str, text: String) {
    let path = crate::testing::capture_dir().join("rects.txt");
    let mut all = fs::read_to_string(&path).unwrap_or_default();
    all.push_str(&format!("{name} {text}\n"));
    fs::write(path, all).unwrap();
}

fn shot(harness: &mut FolderHarness, name: &str) {
    harness.run_steps(3);
    let edge = harness
        .query_by_label("Resize inspector")
        .map_or(0.0, |node| node.rect().left());
    note(name, format!("inspector {edge}"));
    crate::testing::capture(harness, name, None);
}

/// A capture that keeps the pointer where it is, for a hover text.
fn shot_hovered(harness: &mut FolderHarness, name: &str) {
    harness.run_steps(40);
    let image = harness.render().unwrap();
    let directory = crate::testing::capture_dir();
    fs::create_dir_all(&directory).unwrap();
    image.save(directory.join(format!("{name}.png"))).unwrap();
}

fn select(harness: &mut FolderHarness, handle: &str) {
    let id = folder(harness).workbench.active().unwrap().to_owned();
    let snapshot = folder(harness).snapshot(&id).unwrap();
    let step = graph_of(snapshot.model.as_ref().unwrap(), false)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone();
    app(harness).selections.insert(id, vec![step]);
    settle(harness);
}

#[test]
#[ignore = "WS-05 mock-up: design review capture; run with tools/gui/review.sh review_ws05"]
fn review_ws05_title() {
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        for (mode, name) in [(0, "today"), (1, "a"), (2, "b"), (3, "c")] {
            mock::TITLE.store(mode, Relaxed);
            for (open, which) in [(REFUSED, "short"), (LONG, "long")] {
                let (_project, mut harness) = rooms(size, open, "");
                harness.get_by_label("Files").click();
                settle(&mut harness);
                shot(&mut harness, &format!("ws05-{name}-{which}-{suffix}"));
                if mode == 1 && which == "short" {
                    let row = harness
                        .query_all_by_label("double-booking-notice-to-the-organiser.afs")
                        .find(|node| node.rect().left() < 240.0 && node.rect().top() > 130.0)
                        .expect("Files lists the long file")
                        .rect();
                    harness.hover_at(row.center());
                    shot_hovered(&mut harness, &format!("ws05-{name}-hover-{suffix}"));
                    harness.remove_cursor();
                    mock::EDIT.store(true, Relaxed);
                    shot(&mut harness, &format!("ws05-{name}-edit-{suffix}"));
                    mock::EDIT.store(false, Relaxed);
                }
            }
        }
        mock::TITLE.store(0, Relaxed);
    }
}

#[test]
#[ignore = "REQ-15 mock-up: design review capture; run with tools/gui/review.sh review_req15"]
fn review_req15_sample() {
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        for (mode, name) in [(0, "today"), (1, "a"), (2, "b")] {
            mock::SAMPLE.store(mode, Relaxed);
            let (_project, mut harness) = rooms(size, REFUSED, NO_SAMPLE);
            harness.get_by_label("Files").click();
            settle(&mut harness);
            // The Inspector of the request without a sample.
            select(&mut harness, "calendar");
            shot(&mut harness, &format!("req15-{name}-configuration-{suffix}"));
            // The refusal in Run results.
            choose(&mut harness, "Run simulation");
            let _ = finish_run(&mut harness);
            select(&mut harness, "calendar");
            super::inspector_tab(&harness, "Output data")
                .expect("the Inspector has Output data")
                .click();
            settle(&mut harness);
            shot(&mut harness, &format!("req15-{name}-outputs-{suffix}"));
            shot(&mut harness, &format!("req15-{name}-run-{suffix}"));
            let row = harness
                .query_all_by_label_contains("calendar")
                .filter(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
                .map(|node| {
                    (
                        node.accesskit_node().label().unwrap_or_default(),
                        node.rect(),
                    )
                })
                .collect::<Vec<_>>();
            note(&format!("req15-{name}-{suffix}"), format!("rows {row:?}"));
            if let Some((_, rect)) = row.iter().find(|(_, rect)| rect.top() > 200.0) {
                harness.hover_at(rect.center());
                harness.drag_at(rect.center());
                harness.drop_at(rect.center());
                settle(&mut harness);
            }
            shot(&mut harness, &format!("req15-{name}-refusal-{suffix}"));
            if mode != 0 {
                // After the action: the section open, in view.
                mock::OPENED.store(true, Relaxed);
                mock::SCROLL.store(4, Relaxed);
                super::inspector_tab(&harness, "Configuration")
                    .expect("the Inspector has Configuration")
                    .click();
                settle(&mut harness);
                harness.run_steps(5);
                shot(&mut harness, &format!("req15-{name}-opened-{suffix}"));
                mock::OPENED.store(false, Relaxed);
            }
        }
        mock::SAMPLE.store(0, Relaxed);
    }
}
