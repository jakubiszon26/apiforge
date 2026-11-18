//! Mock-ups for the close of the LAY sprint (#296): the folded checks line
//! that writes its one predicate, with the switch on the bar and the arrow
//! on a step (LAY-30 re-scoped, #383); the line a card carries at the
//! Overview zoom; the staged source editor reached from a kept statement.
//! Throwaway code on a mock branch: the views render the room project
//! (`fixtures/scenarios/lay/rooms/`) at 1280 × 800. Not for merging.
use super::lay23::{id_of, select, show};
use super::*;
use std::time::{Duration, Instant};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn copy(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The room project copied fresh, `scenario` opened in Diagram as a first
/// open shows it; `edit` replaces a piece of its text before it is opened.
fn rooms(
    scenario: &str,
    edit: Option<(&str, &str)>,
    library: bool,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = tempfile::tempdir().unwrap();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/rooms");
    copy(&fixture, project.path());
    let path = format!("scenarios/{scenario}.afs");
    if let Some((from, to)) = edit {
        let file = project.path().join(&path);
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains(from), "{from:?} is in {path}");
        fs::write(&file, text.replacen(from, to, 1)).unwrap();
    }
    let mut harness = crate::testing::folder_harness_in(project.path(), SIZE, Mode::Diagram);
    app(&mut harness).review_open(&path, None);
    settle(&mut harness);
    app(&mut harness).workbench.panels.library_collapsed = !library;
    settle(&mut harness);
    let id = folder(&harness).workbench.active().unwrap().to_owned();
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

fn simulate(harness: &mut FolderHarness) {
    choose(harness, "Run simulation");
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
    harness.step();
    // Run results folded to their bar: the views are of the board.
    choose(harness, "Run results");
    settle(harness);
    harness.step();
}

fn canvas<'a>(harness: &'a mut FolderHarness, id: &str) -> &'a mut crate::canvas::Canvas {
    app(harness).canvases.get_mut(id).expect("the board")
}

/// The board at `zoom`, `handle` placed `offset` from the middle of the view.
fn place(harness: &mut FolderHarness, id: &str, handle: &str, zoom: f32, offset: egui::Vec2) {
    show(harness, id, handle, zoom);
    canvas(harness, id).debug_pan(offset);
    settle(harness);
    harness.step();
}

/// The whole board of `double-booking` readable in the window, library
/// folded: 56 %.
fn whole(harness: &mut FolderHarness, id: &str) {
    place(harness, id, "rooms", 0.56, egui::vec2(140.0, -85.0));
}

/// The API accepts the second booking: `refused` fails.
const ACCEPTED: (&str, &str) = (
    r#"sample 409 { error: { code: "room_unavailable", message: "The room is closed for maintenance." } }"#,
    r#"sample 201 { id: "bk-2", roomId: "room-b", start: "2026-12-01T10:00:00Z", end: "2026-12-01T11:00:00Z" }"#,
);

/// (a) The folded checks line: one check is written, two are counted; at
/// rest, passed, failed; the switch on the bar; a step's arrow, open.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_laymock_checks() {
    let (_project, mut harness, id) = rooms("double-booking", None, false);
    whole(&mut harness, &id);
    crate::testing::capture(&mut harness, "lay30-line-at-rest", None);
    simulate(&mut harness);
    whole(&mut harness, &id);
    crate::testing::capture(&mut harness, "lay30-line-passed", None);
    // A step's own arrow, open: the check hangs under its step.
    let second = id_of(&harness, &id, "second");
    canvas(&mut harness, &id).expand_here(&second, true);
    settle(&mut harness);
    harness.step();
    crate::testing::capture(&mut harness, "lay30-step-open", None);

    let (_project, mut harness, id) = rooms("double-booking", Some(ACCEPTED), false);
    simulate(&mut harness);
    whole(&mut harness, &id);
    crate::testing::capture(&mut harness, "lay30-line-failed", None);
}

/// (b) The Overview line: what `double-booking` shows when it is first
/// opened at 1280 × 800, the library open and folded; `booking-day` too.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_laymock_overview() {
    for (scenario, library, name) in [
        ("double-booking", true, "lay41-overview-first-open"),
        ("double-booking", false, "lay41-overview-library-folded"),
        ("booking-day", true, "lay41-overview-booking-day"),
    ] {
        let (_project, mut harness, _) = rooms(scenario, None, library);
        choose(&mut harness, "Fit the view");
        settle(&mut harness);
        harness.step();
        crate::testing::capture(&mut harness, name, None);
    }
}

/// (c) The staged source editor: *Change source* on the statement a chip
/// click keeps, then the Inspector at the field with the choice held and
/// its summary above *Apply* and *Cancel*.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_laymock_source() {
    let (_project, mut harness, id) = rooms("double-booking", None, false);
    whole(&mut harness, &id);
    harness
        .get_by_label("rooms.body.items[1].id → second.body.roomId")
        .click();
    settle(&mut harness);
    harness.step();
    crate::testing::capture(&mut harness, "lay42-change-source-on-statement", None);

    select(&mut harness, &id, &["second"]);
    crate::inspector::binding::MOCK_STAGED.store(true, std::sync::atomic::Ordering::Relaxed);
    for _ in 0..6 {
        settle(&mut harness);
        harness.step();
    }
    crate::testing::capture(&mut harness, "lay42-staged-source", None);
    crate::inspector::binding::MOCK_STAGED.store(false, std::sync::atomic::Ordering::Relaxed);
}
