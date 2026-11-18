//! LAY-41 (#437): at the Overview zoom (from 25 % to below 45 %) a card
//! carries one line under its name saying what the step is — a request's
//! method and path, a component call's handle and revision, the condition
//! of an `if`, a check or a *Data condition*, a value where its card has
//! room — in the code face at 9 px on screen, cut with `…` at the card's
//! edge, the checks pill under it. No card's box, route, chip or count and
//! no Fit moves. On a temporary copy of the room project
//! (`fixtures/scenarios/lay/rooms/`), first open, Fit, 1280 × 800.
use super::lay23::{board, graph, handle_of, id_of, show};
use super::*;
use crate::canvas::{DrawnText, TextKind};
use crate::testing::folder_harness_in;
use apiforge_core::model::NodeKind;
use std::path::Path;
use std::time::{Duration, Instant};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const SMALL: egui::Vec2 = egui::vec2(960.0, 540.0);
const DOUBLE: &str = "double-booking";
const NOTICE: &str = "double-booking-notice";
const DAY: &str = "booking-day";
/// The line's size on screen at every Overview zoom.
const LINE: f32 = 9.0;

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// A temporary copy of the room project, `edit` applied to the text of
/// the scenario `name` first.
fn room_project(name: &str, edit: impl FnOnce(String) -> String) -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/rooms"),
        project.path(),
    );
    let path = project.path().join(format!("scenarios/{name}.afs"));
    let source = fs::read_to_string(&path).unwrap();
    fs::write(&path, edit(source)).unwrap();
    project
}

/// `path` of the project at `root` opened at `size` as it first opens,
/// the library open or folded, nothing selected, fitted; its document id.
fn open_path(root: &Path, path: &str, size: egui::Vec2, library: bool) -> (FolderHarness, String) {
    let mut harness = folder_harness_in(root, size, Mode::Diagram);
    app(&mut harness).workbench.panels.library_collapsed = !library;
    app(&mut harness).review_open(path, None);
    settle(&mut harness);
    let id = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("the project has {path}"))
        .id()
        .to_owned();
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    fit(&mut harness);
    (harness, id)
}

fn fit(harness: &mut FolderHarness) {
    choose(harness, "Fit the view");
    settle(harness);
    harness.step();
}

/// The room project's scenario `name`, as it is, first opened at `size`.
fn open(name: &str, size: egui::Vec2, library: bool) -> (tempfile::TempDir, FolderHarness, String) {
    let project = room_project(name, |source| source);
    let (harness, id) = open_path(
        project.path(),
        &format!("scenarios/{name}.afs"),
        size,
        library,
    );
    (project, harness, id)
}

/// `shop-30` on the demo shop's board, as a new install first opens it.
fn open_shop_30(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    let path = "scenarios/promotion-and-cart-price.afs";
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/shop-30.afs"),
    )
    .unwrap();
    fs::write(project.path().join(path), source).unwrap();
    let (harness, id) = open_path(project.path(), path, size, true);
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
}

/// `second` answered with 201: the API accepts the second booking and
/// its folded check `refused` fails.
fn accepted(source: String) -> String {
    let before = r#"sample 409 { error: { code: "room_unavailable", message: "The room is closed for maintenance." } }"#;
    assert!(source.contains(before), "the fixture's sample of second");
    source.replacen(before, r#"sample 201 { id: "bk-2" }"#, 1)
}

/// Switches the line on or off for the frames that follow, then draws one.
fn line_on(harness: &mut FolderHarness, on: bool) {
    crate::canvas::debug_overview_line(on);
    settle(harness);
    harness.step();
}

fn texts(harness: &FolderHarness, id: &str, handle: &str) -> Vec<DrawnText> {
    board(harness, id).debug_texts(&id_of(harness, id, handle))
}

/// The Overview line `handle`'s card drew in the last frame.
fn line_of(harness: &FolderHarness, id: &str, handle: &str) -> Option<DrawnText> {
    texts(harness, id, handle)
        .into_iter()
        .find(|text| text.kind == TextKind::Overview)
}

fn zoom(harness: &FolderHarness, id: &str) -> f32 {
    board(harness, id).debug_zoom()
}

fn overview(zoom: f32) -> bool {
    (0.25..0.45).contains(&zoom)
}

/// Every card drawn in the last frame, by handle, with its rectangle on
/// screen and what it wrote.
fn cards(harness: &FolderHarness, id: &str) -> Vec<(String, egui::Rect, Vec<DrawnText>)> {
    let graph = graph(harness, id);
    let board = board(harness, id);
    board
        .debug_cards()
        .iter()
        .filter(|(card, _)| board.debug_is_drawn(card))
        .map(|(card, rect)| (handle_of(&graph, card), *rect, board.debug_texts(card)))
        .collect()
}

/// Criterion 1: `double-booking` first opened at Fit, the library open:
/// the requests say their method and path, the call its component and
/// revision, the `if` its condition.
#[test]
fn lay41_criterion_1_double_booking_says_what_each_step_is() {
    let (_project, harness, id) = open(DOUBLE, SIZE, true);
    let zoom = zoom(&harness, &id);
    assert!(overview(zoom), "Fit is an Overview zoom, here {zoom}");
    for (handle, start) in [
        ("rooms", "GET /rooms"),
        ("first", "POST /bookings"),
        ("second", "POST /bookings"),
        ("calendar", "GET /rooms/"),
        ("token", "sign_in · r1"),
        ("booked", "first.status"),
    ] {
        let line =
            line_of(&harness, &id, handle).unwrap_or_else(|| panic!("{handle} draws a line"));
        assert!(
            line.text.starts_with(start),
            "{handle}'s line {:?} starts with {start:?}",
            line.text
        );
    }
}

/// Criterion 2: the line is 9 px on screen at 25 %, 36 % and 44 %; from
/// 45 % up the rows are written instead, and below 25 % there is none.
#[test]
fn lay41_criterion_2_nine_pixels_at_every_overview_zoom() {
    let (_project, mut harness, id) = open(DOUBLE, SIZE, true);
    for zoom in [0.25, 0.36, 0.44] {
        for handle in ["first", "token", "booked"] {
            show(&mut harness, &id, handle, zoom);
            let line = line_of(&harness, &id, handle)
                .unwrap_or_else(|| panic!("{handle} draws a line at {zoom}"));
            assert_eq!(line.size, LINE, "{handle}'s line at {zoom}");
        }
    }
    for zoom in [0.45, 0.6, 0.8, 0.24, 0.1] {
        for handle in ["first", "token", "booked"] {
            show(&mut harness, &id, handle, zoom);
            assert!(
                line_of(&harness, &id, handle).is_none(),
                "{handle} draws no Overview line at {zoom}"
            );
        }
    }
    // From 45 % the request's rows say its method and path.
    show(&mut harness, &id, "first", 0.45);
    assert!(
        texts(&harness, &id, "first")
            .iter()
            .any(|text| text.kind == TextKind::Subtitle && text.text.starts_with("POST")),
        "first's subtitle row at 45 %"
    );
}

/// The checks of criterion 3 on every card of the last frame: the line in
/// its card, under its name, above its pill and count, clear of the badge
/// and the sockets on the card's edges.
fn line_in_place(harness: &FolderHarness, id: &str) -> usize {
    let zoom = zoom(harness, id);
    let socket = (4.5 * zoom).max(1.25);
    let mut lines = 0;
    for (handle, card, texts) in cards(harness, id) {
        let Some(line) = texts.iter().find(|text| text.kind == TextKind::Overview) else {
            continue;
        };
        lines += 1;
        assert!(
            card.expand(0.5).contains_rect(line.rect),
            "{handle}: the line {:?} in its card {card:?}",
            line.rect
        );
        assert!(
            line.rect.left() >= card.left() + socket && line.rect.right() <= card.right() - socket,
            "{handle}: the line {:?} clear of the sockets on {card:?}",
            line.rect
        );
        let name = texts
            .iter()
            .find(|text| text.kind == TextKind::Name)
            .unwrap_or_else(|| panic!("{handle} has a name over its line"));
        assert!(
            name.rect.bottom() <= line.rect.top() + 0.5,
            "{handle}: the line under the name"
        );
        for result in texts
            .iter()
            .filter(|text| text.kind == TextKind::Result && text.rect.top() >= name.rect.bottom())
        {
            assert!(
                result.rect.top() >= line.rect.bottom() - 0.5,
                "{handle}: {:?} under the line",
                result.text
            );
        }
        if let Some(badge) = board(harness, id).debug_badge(&id_of(harness, id, &handle)) {
            assert!(
                !badge.rect.intersects(line.rect),
                "{handle}: the line clear of the badge"
            );
        }
    }
    lines
}

/// What every card wrote, but the line: kind and text.
fn written(harness: &FolderHarness, id: &str) -> Vec<(String, Vec<(TextKind, String)>)> {
    cards(harness, id)
        .into_iter()
        .map(|(handle, _, texts)| {
            let texts = texts
                .into_iter()
                .filter(|text| text.kind != TextKind::Overview)
                .map(|text| (text.kind, text.text))
                .collect();
            (handle, texts)
        })
        .collect()
}

/// Criterion 3: the line lies in its card, under the name and above the
/// pill, clear of the badge and the sockets; a wider text ends with `…`; a
/// card too low for it draws none and loses nothing it shows today — at
/// rest and after a run whose failure counts the steps it kept from
/// running.
#[test]
fn lay41_criterion_3_the_line_in_its_place() {
    for (library, edit) in [(true, false), (false, false), (true, true)] {
        let project = room_project(
            DOUBLE,
            |source| {
                if edit { accepted(source) } else { source }
            },
        );
        let (mut harness, id) = open_path(
            project.path(),
            "scenarios/double-booking.afs",
            SIZE,
            library,
        );
        if edit {
            simulate(&mut harness);
            fit(&mut harness);
        }
        let zoom = zoom(&harness, &id);
        assert!(overview(zoom), "Fit is an Overview zoom, here {zoom}");
        assert!(
            line_in_place(&harness, &id) >= 6,
            "the lines of criterion 1"
        );
        let with = written(&harness, &id);
        let rects: Vec<(String, Vec<DrawnText>)> = cards(&harness, &id)
            .into_iter()
            .map(|(handle, _, texts)| (handle, texts))
            .collect();
        line_on(&mut harness, false);
        assert_eq!(
            written(&harness, &id),
            with,
            "every card writes what it wrote without the line (library open: {library}, run: {edit})"
        );
        // A card with no line has every text where it had it.
        let without: Vec<(String, Vec<DrawnText>)> = cards(&harness, &id)
            .into_iter()
            .map(|(handle, _, texts)| (handle, texts))
            .collect();
        for ((handle, on), (_, off)) in rects.iter().zip(&without) {
            if on.iter().all(|text| text.kind != TextKind::Overview) {
                let at = |texts: &[DrawnText]| -> Vec<(String, egui::Rect)> {
                    texts
                        .iter()
                        .map(|text| (text.text.clone(), text.rect))
                        .collect()
                };
                assert_eq!(at(on), at(off), "{handle} without a line is as it was");
            }
        }
        line_on(&mut harness, true);
        if library && !edit {
            // `slot`, a one-row value card at 36 %, is too low for it.
            assert!(
                line_of(&harness, &id, "slot").is_none(),
                "slot has no room at {zoom}"
            );
            // `calendar`'s address is wider than its card.
            let line = line_of(&harness, &id, "calendar").expect("calendar's line");
            let whole = "GET /rooms/{{rooms.body.items[0].id}}/bookings";
            assert!(line.text.ends_with('…'), "{:?} is cut", line.text);
            assert!(
                whole.starts_with(line.text.trim_end_matches('…')),
                "{:?} starts {whole:?}",
                line.text
            );
        }
        if !library && !edit {
            // Folded, the board fits at 45 % on the label and stays
            // Overview: `slot` now holds its value.
            let line = line_of(&harness, &id, "slot").expect("slot's line with the library folded");
            assert!(line.text.starts_with("{ start: "), "{:?}", line.text);
        }
    }
    crate::canvas::debug_overview_line(true);
}

/// The board's geometry in the last frame: the camera, every card's box
/// on screen, every route, chip and count on the board.
fn geometry(harness: &FolderHarness, id: &str) -> String {
    let board = board(harness, id);
    let mut lines = vec![format!("camera {:?}", board.debug_camera())];
    lines.extend(
        board
            .debug_cards()
            .iter()
            .map(|(card, rect)| format!("card {card} {rect:?}")),
    );
    lines.extend(
        board
            .debug_wires()
            .iter()
            .map(|wire| format!("wire {} {:?}", wire.id, wire.points)),
    );
    lines.extend(board.debug_tags().iter().map(|tag| {
        format!(
            "chip {} {} {:?} {:?}",
            tag.tag.reader, tag.tag.statement, tag.rect, tag.world
        )
    }));
    lines.extend(
        board
            .debug_counts()
            .iter()
            .map(|count| format!("count {} {:?}", count.count.label, count.world)),
    );
    lines.join("\n")
}

/// Criterion 4: on the three room scenarios and `shop-30`, at 1280 × 800
/// and 960 × 540, the line switched on moves no card, route, chip or
/// count, and Fit stays where it was.
#[test]
fn lay41_criterion_4_the_line_moves_nothing() {
    for size in [SIZE, SMALL] {
        let mut boards = Vec::new();
        for name in [DOUBLE, NOTICE, DAY] {
            boards.push((name.to_owned(), open(name, size, true)));
        }
        boards.push(("shop-30".to_owned(), open_shop_30(size)));
        for (name, (_project, mut harness, id)) in boards {
            line_on(&mut harness, false);
            fit(&mut harness);
            let without = geometry(&harness, &id);
            line_on(&mut harness, true);
            fit(&mut harness);
            let with = geometry(&harness, &id);
            assert_eq!(with, without, "{name} at {size:?}");
            if overview(zoom(&harness, &id)) {
                assert!(
                    line_in_place(&harness, &id) > 0,
                    "{name} at {size:?} draws lines"
                );
            }
        }
    }
}

/// Criterion 5: LAY-09 holds — a name at least 11 px at Fit, no text
/// smaller when zooming in across 45 %, a failed card red at 35 % with its
/// line.
#[test]
fn lay41_criterion_5_lay09_holds() {
    let (_project, mut harness, id) = open(DOUBLE, SIZE, true);
    for (handle, _, texts) in cards(&harness, &id) {
        for name in texts.iter().filter(|text| text.kind == TextKind::Name) {
            assert!(name.size >= 11.0, "{handle}'s name {} px", name.size);
        }
    }
    for handle in ["first", "token", "booked", "slot"] {
        show(&mut harness, &id, handle, 0.44);
        let before = texts(&harness, &id, handle);
        show(&mut harness, &id, handle, 0.46);
        let after = texts(&harness, &id, handle);
        let smallest = after.iter().map(|text| text.size).fold(f32::MAX, f32::min);
        for text in &before {
            if text.kind == TextKind::Overview || text.kind == TextKind::Name {
                assert!(
                    smallest >= text.size.min(LINE),
                    "{handle}: {:?} {} px at 44 %, a text of {smallest} px at 46 %",
                    text.text,
                    text.size
                );
            }
        }
        let name = |texts: &[DrawnText]| {
            texts
                .iter()
                .find(|text| text.kind == TextKind::Name)
                .map(|text| text.size)
        };
        assert!(name(&after) >= name(&before), "{handle}'s name grows");
    }

    let project = room_project(DOUBLE, accepted);
    let (mut harness, id) = open_path(project.path(), "scenarios/double-booking.afs", SIZE, true);
    simulate(&mut harness);
    show(&mut harness, &id, "second", 0.35);
    assert_eq!(
        board(&harness, &id).debug_signal(&id_of(&harness, &id, "second")),
        Some(crate::theme::DANGER),
        "second is red at 35 %"
    );
    assert!(line_of(&harness, &id, "second").is_some(), "with its line");
}

/// The address of each request of `source` as written: handle, method and
/// path.
fn addresses(source: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        let Some((handle, rest)) = line.split_once(" = ") else {
            continue;
        };
        for method in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
            if let Some(path) = rest.strip_prefix(&format!("{method} \"")) {
                let path = path.split('"').next().unwrap();
                found.push((handle.to_owned(), format!("{method} {path}")));
            }
        }
    }
    found
}

/// Criterion 6: `booking-day` at Fit, every request card draws its method
/// and path, cut with `…` only where the card ends.
#[test]
fn lay41_criterion_6_booking_day_names_every_request() {
    let (project, harness, id) = open(DAY, SIZE, true);
    let zoom = zoom(&harness, &id);
    assert!(overview(zoom), "Fit is an Overview zoom, here {zoom}");
    let source = fs::read_to_string(project.path().join(format!("scenarios/{DAY}.afs"))).unwrap();
    let addresses = addresses(&source);
    let requests: Vec<String> = graph(&harness, &id)
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Request)
        .map(|node| node.handle.clone())
        .collect();
    // The board's fourteen requests; the cleanup's are on its own board.
    assert_eq!(requests.len(), 14, "booking-day's requests");
    for handle in &requests {
        let address = &addresses
            .iter()
            .find(|(written, _)| written == handle)
            .unwrap_or_else(|| panic!("{handle}'s address"))
            .1;
        let line = line_of(&harness, &id, handle)
            .unwrap_or_else(|| panic!("{handle} draws {address:?} at {zoom}"));
        let (method, _) = address.split_once(' ').unwrap();
        assert!(
            line.text.starts_with(&format!("{method} /")),
            "{handle}: {:?} names its method and path",
            line.text
        );
        if line.text.ends_with('…') {
            assert!(
                address.starts_with(line.text.trim_end_matches('…')),
                "{handle}: {:?} is the start of {address:?}",
                line.text
            );
        } else {
            assert_eq!(&line.text, address, "{handle}: whole");
        }
    }
}

/// The captures of the pull request, at 1280 × 800 on the room project as
/// the accepted mock-up shows it: `double-booking` first opened at Fit with
/// the library open and folded, and `booking-day` at Fit.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay41_overview_line() {
    for (scenario, library, name) in [
        (DOUBLE, true, "lay41-built-first-open"),
        (DOUBLE, false, "lay41-built-library-folded"),
        (DAY, true, "lay41-built-booking-day"),
    ] {
        let (_project, mut harness, _) = open(scenario, SIZE, library);
        crate::testing::capture(&mut harness, name, None);
    }
}
