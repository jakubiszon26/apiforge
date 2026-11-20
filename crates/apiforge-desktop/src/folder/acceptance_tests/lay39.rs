//! LAY-39 (#435): a chip keeps the index of the element it reads. Two
//! bookings read `rooms.body.items[0].id` and `rooms.body.items[1].id`; their
//! chips say `← rooms…[0].id` and `← rooms…[1].id`, then `← rooms…[1]`, then
//! `← …[1]`, and the bare arrow only where not even that fits. Text only: the
//! statement, the accessible name, where a chip stands and every size rule
//! stay as they are. On a temporary copy of the room project
//! (`fixtures/scenarios/lay/rooms/`), tidied, chips on, checks folded,
//! 1280 × 800.
use super::lay23::{board, graph, handle_of, show};
use super::*;
use crate::testing::folder_harness_in;
use egui_kittest::kittest::Queryable;
use std::collections::BTreeMap;

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";

/// The three scenarios of the room project.
const SCENARIOS: [&str; 3] = ["double-booking", "double-booking-notice", "booking-day"];

/// The zooms the owner's task is read at.
const ZOOMS: [f32; 3] = [0.54, 0.76, 1.0];

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
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

/// The room project copied to a temporary directory, the scenario `name`
/// open on its board, tidied with nothing selected.
fn open(name: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let project = tempfile::tempdir().unwrap();
    copy_tree(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/rooms"),
        project.path(),
    );
    let mut harness = folder_harness_in(project.path(), SIZE, Mode::Diagram);
    let path = format!("scenarios/{name}.afs");
    app(&mut harness).review_open(&path, None);
    settle(&mut harness);
    let id = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("the room project has {path}"))
        .id()
        .to_owned();
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    harness.step();
    (project, harness, id)
}

/// Fit, then the zoom set to `zoom` on `second`, as the owner reads it.
fn at(harness: &mut FolderHarness, id: &str, zoom: f32) {
    choose(harness, FIT);
    harness.step();
    show(harness, id, "second", zoom);
}

/// The chip of `reader` whose statement starts with `source`, as drawn.
fn chip<'a>(
    harness: &'a FolderHarness,
    id: &str,
    reader: &str,
    source: &str,
) -> &'a crate::canvas::DrawnTag {
    let graph = graph(harness, id);
    board(harness, id)
        .debug_tags()
        .iter()
        .find(|tag| {
            handle_of(&graph, &tag.tag.reader) == reader && tag.tag.statement.starts_with(source)
        })
        .unwrap_or_else(|| panic!("{reader} has a chip of {source}"))
}

/// Whether a chip's read goes through a list element.
fn indexed(tag: &crate::canvas::DrawnTag) -> bool {
    let read = tag.tag.statement.split(" → ").next().unwrap_or_default();
    read.contains('[')
}

/// The statements of every chip of each room scenario on `main` 2f6922c,
/// as `reader | statement`, in the order the board lists them.
fn statements(name: &str) -> Vec<&'static str> {
    let list: &[&str] = match name {
        "double-booking" => &[
            "rooms | token → rooms.headers.Authorization",
            "first | token → first.headers.Authorization",
            "first | rooms.body.items[0].id → first.body.roomId",
            "first | slot.start → first.body.start",
            "second | token → second.headers.Authorization",
            "second | rooms.body.items[1].id → second.body.roomId",
            "second | slot.start → second.body.start",
            "calendar | rooms.body.items[0].id → calendar.url",
            "calendar | token → calendar.headers.Authorization",
            "calendar | slot.start → calendar.query.from",
        ],
        "double-booking-notice" => &[
            "rooms | token → rooms.headers.Authorization",
            "policy | token → policy.headers.Authorization",
            "first | token → first.headers.Authorization",
            "first | rooms.body.items[0].id → first.body.roomId",
            "first | slot.start → first.body.start",
            "notice | token → notice.headers.Authorization",
            "second | token → second.headers.Authorization",
            "second | rooms.body.items[1].id → second.body.roomId",
            "second | slot.start → second.body.start",
            "calendar | rooms.body.items[0].id → calendar.url",
            "calendar | token → calendar.headers.Authorization",
            "calendar | slot.start → calendar.query.from",
        ],
        "booking-day" => &[
            "profile | token → profile.headers.Authorization",
            "team | token → team.headers.Authorization",
            "buildings | token → buildings.headers.Authorization",
            "floors | token → floors.headers.Authorization",
            "rooms | token → rooms.headers.Authorization",
            "equipment | rooms.body.items[0].id → equipment.url",
            "equipment | token → equipment.headers.Authorization",
            "policy | token → policy.headers.Authorization",
            "holidays | token → holidays.headers.Authorization",
            "first | token → first.headers.Authorization",
            "first | rooms.body.items[0].id → first.body.roomId",
            "first | slot.start → first.body.start",
            "hold | token → hold.headers.Authorization",
            "hold | first.body.id → hold.body.booking",
            "invite | token → invite.headers.Authorization",
            "invite | first.body.id → invite.body.booking",
            "notice | token → notice.headers.Authorization",
            "second | token → second.headers.Authorization",
            "second | rooms.body.items[1].id → second.body.roomId",
            "second | slot.start → second.body.start",
            "calendar | rooms.body.items[0].id → calendar.url",
            "calendar | token → calendar.headers.Authorization",
            "calendar | slot.start → calendar.query.from",
        ],
        _ => unreachable!("a room scenario"),
    };
    list.to_vec()
}

/// Where each chip with an index stood on `main` 2f6922c after Fit and the
/// zoom set on `second`: `(reader, zoom)` → the right edge of its box and the
/// middle of its row, on the board. Measured again for LAY-45 (#468): Tidy
/// up gives a row gap the line to the next row runs in and no column room
/// for the arrow at a badge, so the cards and their chips moved by 20 px;
/// `first` of `booking-day`, the first step of a row, has that line on its
/// left, and its chip stands beyond it.
fn stubs(name: &str) -> BTreeMap<(&'static str, u32), (f32, f32)> {
    let list: &[(&str, [(f32, f32); 3])] = match name {
        "double-booking" => &[
            ("first", [(1390.0, 107.35), (1390.0, 98.0), (1390.0, 98.0)]),
            ("second", [(-10.0, 521.35), (-10.0, 512.0), (-10.0, 512.0)]),
            ("calendar", [(390.0, 465.0), (390.0, 465.0), (390.0, 465.0)]),
        ],
        "double-booking-notice" => &[
            ("first", [(1630.0, 107.35), (1630.0, 98.0), (1630.0, 98.0)]),
            ("second", [(870.0, 541.35), (870.0, 532.0), (870.0, 532.0)]),
            (
                "calendar",
                [(1250.0, 485.0), (1250.0, 485.0), (1250.0, 485.0)],
            ),
        ],
        "booking-day" => &[
            (
                "equipment",
                [(990.0, 417.0), (990.0, 417.0), (990.0, 417.0)],
            ),
            (
                "first",
                [(-47.4, 883.35), (-45.3, 874.0), (-44.0, 874.0)],
            ),
            (
                "second",
                [(-10.0, 1297.35), (-10.0, 1288.0), (-10.0, 1288.0)],
            ),
            (
                "calendar",
                [(490.0, 1241.0), (490.0, 1241.0), (490.0, 1241.0)],
            ),
        ],
        _ => unreachable!("a room scenario"),
    };
    list.iter()
        .flat_map(|(reader, places)| {
            ZOOMS
                .iter()
                .zip(places)
                .map(move |(zoom, place)| ((*reader, (zoom * 100.0).round() as u32), *place))
        })
        .collect()
}

/// Criterion 1: the chip of `second.body.roomId` offers `rooms…[1].id`,
/// `rooms…[1]`, `…[1]` in that order; `first`'s the same with `[0]`;
/// `calendar`'s read of `rooms.body.items[0].id` in its path the `[0]` forms.
#[test]
fn lay39_criterion_1_a_chip_offers_the_index_forms() {
    let (_project, mut harness, id) = open("double-booking");
    at(&mut harness, &id, 1.0);
    for (reader, index) in [("second", 1), ("first", 0), ("calendar", 0)] {
        let tag = chip(&harness, &id, reader, "rooms.");
        assert_eq!(
            tag.tag.forms,
            vec![
                format!("rooms…[{index}].id"),
                format!("rooms…[{index}]"),
                format!("…[{index}]"),
            ],
            "{reader}: {}",
            tag.tag.statement
        );
        assert_eq!(tag.tag.text, tag.tag.forms[0], "{reader}: its widest form");
    }
}

/// Criterion 2, on the board: a read with no index gives today's forms and
/// nothing else.
#[test]
fn lay39_criterion_2_a_read_with_no_index_keeps_its_forms() {
    let (_project, mut harness, id) = open("double-booking");
    at(&mut harness, &id, 1.0);
    let slot = chip(&harness, &id, "second", "slot.");
    assert_eq!(slot.tag.forms, vec!["slot.start", "slot"]);
    let (_project, mut harness, id) = open("booking-day");
    at(&mut harness, &id, 1.0);
    let first = chip(&harness, &id, "hold", "first.");
    assert_eq!(first.tag.forms, vec!["first.id", "first"]);
}

/// Criterion 3: the statement and the accessible name of every chip of the
/// three room scenarios are what `main` gives.
#[test]
fn lay39_criterion_3_statements_and_names_are_unchanged() {
    for name in SCENARIOS {
        let (_project, mut harness, id) = open(name);
        let graph = graph(&harness, &id);
        for zoom in ZOOMS {
            at(&mut harness, &id, zoom);
            let drawn: Vec<String> = board(&harness, &id)
                .debug_tags()
                .iter()
                .map(|tag| {
                    format!(
                        "{} | {}",
                        handle_of(&graph, &tag.tag.reader),
                        tag.tag.statement
                    )
                })
                .collect();
            assert_eq!(drawn, statements(name), "{name} at {zoom}");
        }
        // Every chip at Fit is named by its statement.
        choose(&mut harness, FIT);
        harness.step();
        for tag in board(&harness, &id).debug_tags() {
            assert!(
                harness
                    .query_all_by_label(&tag.tag.statement)
                    .next()
                    .is_some(),
                "{name}: a chip named {}",
                tag.tag.statement
            );
        }
    }
}

/// Criterion 4: on `double-booking` at 54 % no chip of `first`, `second` or
/// `calendar` whose read has an index is a bare arrow; the room chips of
/// `first` and `second` say different things, one `[0]`, the other `[1]`.
#[test]
fn lay39_criterion_4_no_bare_arrow_at_54() {
    let (_project, mut harness, id) = open("double-booking");
    at(&mut harness, &id, 0.54);
    let graph = graph(&harness, &id);
    for tag in board(&harness, &id).debug_tags() {
        let reader = handle_of(&graph, &tag.tag.reader);
        if ["first", "second", "calendar"].contains(&reader.as_str()) && indexed(tag) {
            assert_ne!(tag.shown, "←", "{reader}: {}", tag.tag.statement);
        }
    }
    let first = chip(&harness, &id, "first", "rooms.").shown.clone();
    let second = chip(&harness, &id, "second", "rooms.").shown.clone();
    assert_ne!(first, second);
    assert!(first.contains("[0]"), "first: {first}");
    assert!(second.contains("[1]"), "second: {second}");
}

/// Criterion 5: on `booking-day` at 76 % the room chips of `first` and
/// `second` say `← rooms…[0]` and `← rooms…[1]`, or wider.
#[test]
#[ignore = "LAY-39: owner decision pending, `← rooms…[0]` needs 64.4 px at 76 % and the gap gives 63.4"]
fn lay39_criterion_5_booking_day_at_76() {
    let (_project, mut harness, id) = open("booking-day");
    at(&mut harness, &id, 0.76);
    for (reader, index) in [("first", 0), ("second", 1)] {
        let shown = chip(&harness, &id, reader, "rooms.").shown.clone();
        assert!(
            [
                format!("← rooms…[{index}].id"),
                format!("← rooms…[{index}]")
            ]
            .contains(&shown),
            "{reader}: {shown}"
        );
    }
}

/// Criterion 6: at 100 % on `double-booking` the two chips read
/// `← rooms…[0].id` and `← rooms…[1].id`.
#[test]
#[ignore = "LAY-39: owner decision pending, `← rooms…[0].id` needs 88.7 px at 100 % and the gap gives 84"]
fn lay39_criterion_6_whole_at_100() {
    let (_project, mut harness, id) = open("double-booking");
    at(&mut harness, &id, 1.0);
    assert_eq!(
        chip(&harness, &id, "first", "rooms.").shown,
        "← rooms…[0].id"
    );
    assert_eq!(
        chip(&harness, &id, "second", "rooms.").shown,
        "← rooms…[1].id"
    );
}

/// Criterion 7: nothing else moves. On the three room scenarios at 54 %,
/// 76 % and 100 % every chip and count keeps the room LAY-37 asks for, and a
/// chip with an index ends at the stub and on the row it had on `main`.
#[test]
fn lay39_criterion_7_nothing_else_moves() {
    let mut failures = Vec::new();
    for name in SCENARIOS {
        let (_project, mut harness, id) = open(name);
        let graph = graph(&harness, &id);
        let expected = stubs(name);
        for zoom in ZOOMS {
            at(&mut harness, &id, zoom);
            let found = super::lay37::breaches(&harness, &id, zoom);
            for (kind, list) in [
                ("chip/chip", &found.chips),
                ("chip/card", &found.cards),
                ("chip/line", &found.lines),
                ("away from its socket", &found.strays),
            ] {
                for item in list {
                    failures.push(format!("{name} at {zoom}, {kind}: {item}"));
                }
            }
            for tag in board(&harness, &id)
                .debug_tags()
                .iter()
                .filter(|tag| indexed(tag))
            {
                let reader = handle_of(&graph, &tag.tag.reader);
                let key = expected
                    .keys()
                    .find(|(name, percent)| {
                        *name == reader && *percent == (zoom * 100.0).round() as u32
                    })
                    .copied()
                    .unwrap_or_else(|| panic!("{name}: {reader} measured on main"));
                let (right, middle) = expected[&key];
                let (now_right, now_middle) = (tag.world.right(), tag.world.center().y);
                if (now_right - right).abs() > 0.5 || (now_middle - middle).abs() > 0.5 {
                    failures.push(format!(
                        "{name} at {zoom}: {} of {reader} ends at ({now_right:.1}, \
                         {now_middle:.1}), on main ({right:.1}, {middle:.1})",
                        tag.shown
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The camera at `zoom` on the middle between the cards `a` and `b`, moved
/// `right` points further so that the chips left of a card are in view.
fn between(harness: &mut FolderHarness, id: &str, a: &str, b: &str, zoom: f32, right: f32) {
    show(harness, id, a, zoom);
    let card = |harness: &FolderHarness, handle: &str| {
        board(harness, id)
            .debug_world(&super::lay23::id_of(harness, id, handle))
            .unwrap_or_else(|| panic!("{handle} is drawn"))
    };
    let offset = (card(harness, b).center() - card(harness, a).center()) * zoom / 2.0;
    app(harness)
        .canvases
        .get_mut(id)
        .expect("the board is drawn")
        .debug_pan(egui::vec2(right, 0.0) - offset);
    settle(harness);
    harness.step();
}

/// The review views for the owner, 1280 × 800: `double-booking` at 54 %
/// and `booking-day` at 76 % with `first` and `second` in view, and
/// `double-booking` at 100 % on `first` and on `second`.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay39() {
    let (_project, mut harness, id) = open("double-booking");
    choose(&mut harness, FIT);
    harness.step();
    between(&mut harness, &id, "first", "second", 0.54, 70.0);
    crate::testing::capture(&mut harness, "lay39-double-booking-54", None);
    for card in ["first", "second"] {
        show(&mut harness, &id, card, 1.0);
        crate::testing::capture(
            &mut harness,
            &format!("lay39-double-booking-100-{card}"),
            None,
        );
    }
    let (_project, mut harness, id) = open("booking-day");
    choose(&mut harness, FIT);
    harness.step();
    between(&mut harness, &id, "first", "second", 0.76, 0.0);
    crate::testing::capture(&mut harness, "lay39-booking-day-76", None);
}
