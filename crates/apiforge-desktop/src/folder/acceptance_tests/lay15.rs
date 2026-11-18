//! LAY-15 (#299): off-screen relations. While a step is selected, or a
//! trace is kept, a chip on the board's edge names each related card out of
//! view by its role, with an arrow for the direction; more than two on one
//! edge are one chip that unfolds. A click on an edge chip goes to its card
//! at the same zoom, and *Back* returns to where the first jump started:
//! the camera, the selection, the kept trace and the Inspector, its scroll
//! and its open sections (decided 2026-11-18). On `booking-day` of the room
//! project, tidied, chips on, checks folded, at 76 %, 1280 × 800, `second`
//! in view and `rooms` not.
use super::lay16::{press, release};
use super::lay23::{board, id_of};
use super::*;
use crate::canvas::{Command, DrawnEdge, Side};
use crate::testing::MINIMUM;
use egui::{Pos2, Rect, Vec2, pos2, vec2};
use std::collections::BTreeSet;

const SCENARIO: &str = "scenarios/booking-day.afs";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";
const SIZE: Vec2 = vec2(1280.0, 800.0);
const ZOOM: f32 = 0.76;
/// The statement of `second`'s room chip.
const ROOM_STATEMENT: &str = "rooms.body.items[1].id → second.body.roomId";
/// The edge chip of `rooms` as a source of `second`, and its button.
const ROOMS: &str = "Source: rooms.body.items[1].id ↑";
const ROOMS_JUMP: &str = "Source: rooms.body.items[1].id (jump)";
const TOKEN: &str = "Source: token ↑";
const SLOT: &str = "Source: slot · 2 fields ↑";
/// The three sources of `second` folded into one chip, and its button.
const SOURCES: &str = "↑ 3 sources";
const SOURCES_UNFOLD: &str = "↑ 3 sources (unfold)";
const BACK: &str = "Back to second";
const BODY_SOURCE: &str = "Body value source";

/// A copy of `fixtures/scenarios/lay/rooms/` in a temporary directory.
fn rooms_project() -> tempfile::TempDir {
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
    let directory = tempfile::tempdir().unwrap();
    copy(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/rooms"),
        directory.path(),
    );
    directory
}

fn document(harness: &FolderHarness) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == SCENARIO)
        .expect("booking-day is in the project")
        .id()
        .to_owned()
}

/// `booking-day` at `size`, tidied with nothing selected, at 76 % with
/// `second` in view, its top 120 px under the board's top, and `rooms`
/// out of view, two rows up.
fn opened(size: Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = rooms_project();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = document(&harness);
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    harness.step();
    show(&mut harness, &id, "second", ZOOM);
    let top = board(&harness, &id).debug_board().top();
    let card = card(&harness, &id, "second");
    pan(&mut harness, &id, vec2(0.0, top + 120.0 - card.top()));
    assert!(
        in_view(&harness, &id, "second"),
        "{size:?}: second is in view"
    );
    assert!(
        !in_view(&harness, &id, "rooms"),
        "{size:?}: rooms is out of view"
    );
    (project, harness, id)
}

/// The camera centred on `handle` at `zoom`, as a person scrolls there.
fn show(harness: &mut FolderHarness, id: &str, handle: &str, zoom: f32) {
    let node = id_of(harness, id, handle);
    app(harness)
        .canvases
        .get_mut(id)
        .expect("the board is drawn")
        .debug_show(&node, zoom);
    settle(harness);
    harness.step();
}

/// Moves the camera by `offset`, as a person drags the board.
fn pan(harness: &mut FolderHarness, id: &str, offset: Vec2) {
    app(harness)
        .canvases
        .get_mut(id)
        .expect("the board is drawn")
        .debug_pan(offset);
    settle(harness);
    harness.step();
}

/// The card of `handle` on screen.
fn card(harness: &FolderHarness, id: &str, handle: &str) -> Rect {
    board(harness, id)
        .debug_card_rect(&id_of(harness, id, handle))
        .unwrap_or_else(|| panic!("{handle} is on the board"))
}

/// The header row of `handle`'s card on screen.
fn header(harness: &FolderHarness, id: &str, handle: &str) -> Rect {
    board(harness, id)
        .debug_header(&id_of(harness, id, handle))
        .unwrap_or_else(|| panic!("{handle} is on the board"))
}

/// Whether `rect` is wholly in the board's free area: inside the board and
/// clear of the minimap.
fn free(harness: &FolderHarness, id: &str, rect: Rect) -> bool {
    let canvas = board(harness, id);
    canvas.debug_board().contains_rect(rect)
        && canvas
            .debug_minimap()
            .is_none_or(|minimap| !minimap.intersects(rect))
}

/// Whether the header row of `handle` is wholly in the board's free area.
fn in_view(harness: &FolderHarness, id: &str, handle: &str) -> bool {
    free(harness, id, header(harness, id, handle))
}

fn edge_chips(harness: &FolderHarness, id: &str) -> Vec<DrawnEdge> {
    board(harness, id).debug_edge_chips()
}

fn texts(harness: &FolderHarness, id: &str) -> BTreeSet<String> {
    edge_chips(harness, id)
        .into_iter()
        .map(|chip| chip.text)
        .collect()
}

fn selection(harness: &FolderHarness, id: &str) -> Vec<String> {
    folder(harness)
        .selections
        .get(id)
        .cloned()
        .unwrap_or_default()
}

fn kept(harness: &FolderHarness, id: &str) -> Vec<String> {
    let mut wires = board(harness, id).debug_selected_wires().to_vec();
    wires.sort();
    wires
}

fn statements(harness: &FolderHarness, id: &str) -> BTreeSet<String> {
    board(harness, id)
        .debug_statements()
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}

/// The document's text and its layout are unsaved.
fn unsaved(harness: &FolderHarness, id: &str) -> (bool, bool) {
    let document = folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap();
    (document.is_dirty(), document.is_layout_dirty())
}

/// A click: the pointer leaves afterwards.
fn click(harness: &mut FolderHarness, at: Pos2) {
    press(harness, at);
    release(harness, at);
    harness.step();
    harness.step();
    harness.remove_cursor();
    settle(harness);
    harness.step();
}

/// A click on the accessible button `label`: the pointer leaves
/// afterwards.
fn click_label(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    harness.step();
    harness.remove_cursor();
    settle(harness);
    harness.step();
    harness.step();
}

fn escape(harness: &mut FolderHarness) {
    harness.key_press(egui::Key::Escape);
    settle(harness);
    harness.step();
}

/// A click on the header of `handle`'s card: it selects the step and opens
/// its Inspector.
fn select(harness: &mut FolderHarness, id: &str, handle: &str) {
    let at = header(harness, id, handle).center() + vec2(30.0, 0.0);
    click(harness, at);
    assert_eq!(
        selection(harness, id),
        vec![id_of(harness, id, handle)],
        "the click selects {handle}"
    );
}

/// `second`'s chip for `rooms.body.items[1].id`, on screen.
fn room_chip(harness: &FolderHarness, id: &str) -> Rect {
    board(harness, id)
        .debug_tags()
        .iter()
        .find(|tag| tag.tag.statement == ROOM_STATEMENT)
        .map(|tag| tag.rect)
        .expect("second's body carries the chip of rooms")
}

/// A click on `second`'s room chip keeps its trace.
fn keep_room_trace(harness: &mut FolderHarness, id: &str) {
    let at = room_chip(harness, id).center();
    click(harness, at);
    assert!(
        !kept(harness, id).is_empty(),
        "the click keeps the wire of the chip"
    );
    assert_eq!(
        statements(harness, id),
        BTreeSet::from([ROOM_STATEMENT.to_owned()])
    );
}

fn back(harness: &FolderHarness, id: &str) -> Option<(String, Rect)> {
    board(harness, id).debug_back()
}

/// What the Inspector shows: its step, its scroll offset and the open
/// state of each of its sections.
fn inspector(harness: &FolderHarness) -> (Option<String>, f32, Vec<(String, bool)>) {
    let folder = folder(harness);
    (
        folder.debug_inspector_node(),
        folder.debug_inspector_offset(),
        folder.debug_inspector_sections(),
    )
}

/// Scrolls the Inspector until *Body value source* stands near its top,
/// and opens it.
fn open_body_source(harness: &mut FolderHarness) {
    let close = harness.get_by_label("Close Inspector").rect();
    let inside = pos2(close.center().x - 120.0, close.bottom() + 260.0);
    for _ in 0..20 {
        let header = harness.get_by_label(BODY_SOURCE).rect();
        if header.top() < close.bottom() + 160.0 {
            break;
        }
        harness.hover_at(inside);
        harness.step();
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -40.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        settle(harness);
    }
    harness.remove_cursor();
    harness.step();
    click_label(harness, BODY_SOURCE);
}

/// No edge chip, *Back* or kept statement overlaps the selected card, each
/// other, the minimap or the zoom bar, and every edge chip is inside the
/// board.
fn assert_clear(harness: &FolderHarness, id: &str, context: &str) {
    let canvas = board(harness, id);
    let inside = canvas.debug_board();
    let mut boxes: Vec<(String, Rect)> = Vec::new();
    for chip in canvas.debug_edge_chips() {
        assert!(
            inside.contains_rect(chip.rect),
            "{context}: {:?} at {:?} is inside the board {inside:?}",
            chip.text,
            chip.rect
        );
        boxes.push((chip.text, chip.rect));
    }
    if let Some((text, rect)) = canvas.debug_back() {
        assert!(
            inside.contains_rect(rect),
            "{context}: Back is inside the board"
        );
        boxes.push((text, rect));
    }
    boxes.extend(canvas.debug_statements());
    let mut avoid: Vec<(String, Rect)> = selection(harness, id)
        .iter()
        .filter_map(|card| Some((format!("the card {card}"), canvas.debug_card_rect(card)?)))
        .collect();
    if let Some(minimap) = canvas.debug_minimap() {
        avoid.push(("the minimap".into(), minimap));
    }
    avoid.push(("the zoom bar".into(), canvas.debug_zoom_bar()));
    for (index, (text, rect)) in boxes.iter().enumerate() {
        for (other, other_rect) in boxes.iter().skip(index + 1) {
            assert!(
                !rect.shrink(0.5).intersects(*other_rect),
                "{context}: {text:?} at {rect:?} and {other:?} at {other_rect:?} overlap"
            );
        }
        for (other, other_rect) in &avoid {
            assert!(
                !rect.shrink(0.5).intersects(*other_rect),
                "{context}: {text:?} at {rect:?} covers {other} at {other_rect:?}"
            );
        }
    }
}

/// Criterion 1: a click on `second`'s room chip shows exactly one edge
/// chip, on the top edge, reading `Source: rooms.body.items[1].id ↑`; Esc
/// removes it with the trace. A kept trace whose two ends are in view shows
/// none.
#[test]
fn lay15_criterion_1_a_kept_trace_has_its_edge_chip() {
    let (_project, mut harness, id) = opened(SIZE);
    assert!(edge_chips(&harness, &id).is_empty(), "none at rest");

    keep_room_trace(&mut harness, &id);
    let chips = edge_chips(&harness, &id);
    assert_eq!(chips.len(), 1, "one edge chip: {chips:?}");
    assert_eq!(chips[0].text, ROOMS);
    assert_eq!(chips[0].side, Side::Top);
    assert_eq!(chips[0].targets, vec![id_of(&harness, &id, "rooms")]);
    let top = board(&harness, &id).debug_board().top();
    assert!(
        chips[0].rect.top() - top < 24.0,
        "it stands on the top edge: {:?} under {top}",
        chips[0].rect
    );
    assert!(harness.query_by_label(ROOMS_JUMP).is_some(), "a button");

    escape(&mut harness);
    assert!(kept(&harness, &id).is_empty(), "Esc ends the trace");
    assert!(edge_chips(&harness, &id).is_empty(), "and its edge chip");

    // Both ends in view.
    choose(&mut harness, FIT);
    harness.step();
    assert!(in_view(&harness, &id, "rooms") && in_view(&harness, &id, "second"));
    keep_room_trace(&mut harness, &id);
    assert!(
        edge_chips(&harness, &id).is_empty(),
        "both ends in view: no edge chip: {:?}",
        edge_chips(&harness, &id)
    );
}

/// Criterion 2: `second` selected shows `↑ 3 sources`; a click unfolds it
/// into its three sources, each named once; two sources on one edge are
/// two chips from the start; a card whose header is partly out of the free
/// area has a chip, a card wholly in it has none.
#[test]
fn lay15_criterion_2_a_selected_step_has_its_edge_chips() {
    let (_project, mut harness, id) = opened(SIZE);
    select(&mut harness, &id, "second");
    let chips = edge_chips(&harness, &id);
    assert_eq!(chips.len(), 1, "one chip for three sources: {chips:?}");
    assert_eq!(chips[0].text, SOURCES);
    assert_eq!(chips[0].side, Side::Top);
    let sources: BTreeSet<String> = ["token", "rooms", "slot"]
        .iter()
        .map(|handle| id_of(&harness, &id, handle))
        .collect();
    assert_eq!(
        chips[0].targets.iter().cloned().collect::<BTreeSet<_>>(),
        sources
    );

    click_label(&mut harness, SOURCES_UNFOLD);
    assert_eq!(
        texts(&harness, &id),
        BTreeSet::from([TOKEN.to_owned(), ROOMS.to_owned(), SLOT.to_owned()]),
        "unfolded into its members"
    );
    for chip in edge_chips(&harness, &id) {
        assert_eq!(chip.side, Side::Top, "{} on the top edge", chip.text);
        assert_eq!(chip.targets.len(), 1);
    }

    // Two out of view on one edge: two chips from the start, none folded
    // or unfolded. Selected again, its Inspector open, and zoomed out so
    // that `rooms` and `second` are both in view, `slot` and `token` not.
    escape(&mut harness);
    assert!(edge_chips(&harness, &id).is_empty(), "nothing selected");
    let second = id_of(&harness, &id, "second");
    app(&mut harness)
        .selections
        .insert(id.clone(), vec![second.clone()]);
    settle(&mut harness);
    harness.step();
    show(&mut harness, &id, "second", 0.5);
    let top = board(&harness, &id).debug_board().top();
    let rooms = header(&harness, &id, "rooms");
    pan(&mut harness, &id, vec2(0.0, top + 20.0 - rooms.top()));
    assert!(in_view(&harness, &id, "rooms"), "rooms in view");
    assert!(
        free(&harness, &id, card(&harness, &id, "second")),
        "second in view"
    );
    assert!(!in_view(&harness, &id, "slot") && !in_view(&harness, &id, "token"));
    assert_eq!(
        texts(&harness, &id),
        BTreeSet::from([TOKEN.to_owned(), SLOT.to_owned()]),
        "two sources on the top edge are two chips"
    );

    // `rooms` partly out: it has a chip again.
    let rooms = header(&harness, &id, "rooms");
    let half = rooms.height() / 2.0;
    pan(&mut harness, &id, vec2(0.0, top - half - rooms.top()));
    assert!(!in_view(&harness, &id, "rooms"), "rooms is partly out");
    let chips = edge_chips(&harness, &id);
    assert_eq!(chips.len(), 1, "three on the top edge again: {chips:?}");
    assert_eq!(chips[0].text, SOURCES);

    // Wholly in: none.
    pan(&mut harness, &id, vec2(0.0, half + 1.0));
    assert!(in_view(&harness, &id, "rooms"));
    assert_eq!(
        texts(&harness, &id),
        BTreeSet::from([TOKEN.to_owned(), SLOT.to_owned()])
    );
}

/// Criterion 3: a click on the edge chip of criterion 1 puts `rooms` whole
/// inside the free area, with the Inspector open too, at 76 % exactly; the
/// selection, the kept trace, its statement and the Inspector's step stay;
/// the document is not marked unsaved.
#[test]
fn lay15_criterion_3_the_jump_keeps_the_zoom() {
    // The kept trace.
    let (_project, mut harness, id) = opened(SIZE);
    keep_room_trace(&mut harness, &id);
    let before = (
        selection(&harness, &id),
        kept(&harness, &id),
        statements(&harness, &id),
        unsaved(&harness, &id),
    );
    click_label(&mut harness, ROOMS_JUMP);
    assert!(
        free(&harness, &id, card(&harness, &id, "rooms")),
        "rooms is whole in the free area: {:?}",
        card(&harness, &id, "rooms")
    );
    assert_eq!(board(&harness, &id).debug_zoom(), ZOOM, "the same zoom");
    assert_eq!(
        (
            selection(&harness, &id),
            kept(&harness, &id),
            statements(&harness, &id),
            unsaved(&harness, &id),
        ),
        before,
        "the selection, the trace, its statement and the document stay"
    );

    // The selection, with its Inspector open.
    let (_project, mut harness, id) = opened(SIZE);
    select(&mut harness, &id, "second");
    let second = id_of(&harness, &id, "second");
    assert_eq!(inspector(&harness).0, Some(second.clone()));
    let before = unsaved(&harness, &id);
    click_label(&mut harness, SOURCES_UNFOLD);
    click_label(&mut harness, ROOMS_JUMP);
    assert!(
        free(&harness, &id, card(&harness, &id, "rooms")),
        "rooms is whole in the free area beside the Inspector: {:?}",
        card(&harness, &id, "rooms")
    );
    assert_eq!(board(&harness, &id).debug_zoom(), ZOOM);
    assert_eq!(selection(&harness, &id), vec![second.clone()]);
    assert_eq!(inspector(&harness).0, Some(second), "the Inspector stays");
    assert_eq!(unsaved(&harness, &id), before, "nothing is unsaved");
}

/// Criterion 4: after the jump `Back to second` is shown; a click on it
/// makes the camera what it was before the jump, with the selection and the
/// kept trace as they were, and *Back* goes.
#[test]
fn lay15_criterion_4_back_restores_the_board() {
    let (_project, mut harness, id) = opened(SIZE);
    assert_eq!(back(&harness, &id), None, "no Back before a jump");
    keep_room_trace(&mut harness, &id);
    let camera = board(&harness, &id).debug_camera();
    let before = (
        selection(&harness, &id),
        kept(&harness, &id),
        statements(&harness, &id),
    );
    click_label(&mut harness, ROOMS_JUMP);
    assert_ne!(board(&harness, &id).debug_camera(), camera, "it jumped");
    let (text, _) = back(&harness, &id).expect("Back after a jump");
    assert_eq!(text, format!("← {BACK}"));

    click_label(&mut harness, BACK);
    assert_eq!(board(&harness, &id).debug_camera(), camera, "the camera");
    assert_eq!(
        (
            selection(&harness, &id),
            kept(&harness, &id),
            statements(&harness, &id),
        ),
        before,
        "the selection and the kept trace"
    );
    assert_eq!(back(&harness, &id), None, "Back goes");
    assert!(harness.query_by_label(BACK).is_none());
}

/// Criterion 5: `second` selected, its Inspector scrolled to *Body value
/// source* and that section opened; a jump to `rooms` and `rooms` selected
/// there; *Back* selects `second` again, its Inspector at the same scroll
/// offset with the same sections open and closed.
#[test]
fn lay15_criterion_5_back_restores_the_inspector() {
    let (_project, mut harness, id) = opened(SIZE);
    select(&mut harness, &id, "second");
    open_body_source(&mut harness);
    let second = id_of(&harness, &id, "second");
    let before = inspector(&harness);
    assert_eq!(before.0, Some(second.clone()));
    assert!(before.1 > 0.0, "the Inspector is scrolled: {before:?}");
    assert!(
        before
            .2
            .contains(&("body-binding".to_owned(), true)),
        "Body value source is open: {before:?}"
    );
    assert!(
        before.2.iter().any(|(_, open)| !open),
        "some section is closed: {before:?}"
    );

    click_label(&mut harness, SOURCES_UNFOLD);
    click_label(&mut harness, ROOMS_JUMP);
    select(&mut harness, &id, "rooms");
    let rooms = id_of(&harness, &id, "rooms");
    assert_eq!(inspector(&harness).0, Some(rooms), "the Inspector shows rooms");

    click_label(&mut harness, BACK);
    assert_eq!(selection(&harness, &id), vec![second]);
    let after = inspector(&harness);
    assert_eq!(after.0, before.0, "the Inspector shows second");
    assert!(
        (after.1 - before.1).abs() < 0.5,
        "at the same scroll offset: {} for {}",
        after.1,
        before.1
    );
    assert_eq!(after.2, before.2, "the same sections open and closed");
    assert!(harness.query_by_label(BODY_SOURCE).is_some());
}

/// Criterion 6: from `second` to `rooms`, `rooms` selected, from its edge
/// chip on to a reader: *Back* still reads `Back to second` and returns
/// there. Removing `second` removes *Back*.
#[test]
fn lay15_criterion_6_back_returns_to_the_first_origin() {
    let (_project, mut harness, id) = opened(SIZE);
    select(&mut harness, &id, "second");
    let second = id_of(&harness, &id, "second");
    let camera = board(&harness, &id).debug_camera();
    click_label(&mut harness, SOURCES_UNFOLD);
    click_label(&mut harness, ROOMS_JUMP);
    select(&mut harness, &id, "rooms");
    let readers: Vec<DrawnEdge> = edge_chips(&harness, &id);
    assert!(!readers.is_empty(), "rooms has readers out of view");
    let chip = readers
        .iter()
        .find(|chip| chip.targets.len() == 1 && chip.text.starts_with("Reader: "))
        .cloned()
        .or_else(|| {
            let group = readers.iter().find(|chip| chip.targets.len() > 1)?.clone();
            click_label(&mut harness, &group.name);
            edge_chips(&harness, &id)
                .into_iter()
                .find(|chip| chip.targets.len() == 1 && chip.text.starts_with("Reader: "))
        })
        .expect("an edge chip of a reader of rooms");
    let after_first = board(&harness, &id).debug_camera();
    click_label(&mut harness, &chip.name);
    assert_ne!(
        board(&harness, &id).debug_camera(),
        after_first,
        "the second jump"
    );
    assert_eq!(
        back(&harness, &id).map(|(text, _)| text),
        Some(format!("← {BACK}")),
        "Back names the first origin"
    );
    click_label(&mut harness, BACK);
    assert_eq!(board(&harness, &id).debug_camera(), camera);
    assert_eq!(selection(&harness, &id), vec![second.clone()]);
    assert_eq!(back(&harness, &id), None);

    // Removing the step Back returns to removes Back.
    click_label(&mut harness, SOURCES_UNFOLD);
    click_label(&mut harness, ROOMS_JUMP);
    assert!(back(&harness, &id).is_some());
    app(&mut harness).active_board_command(Command::Delete {
        ids: vec![second],
        wires: Vec::new(),
    });
    settle(&mut harness);
    harness.step();
    assert_eq!(back(&harness, &id), None, "Back goes with second");
    assert!(harness.query_by_label(BACK).is_none());
}

/// Criterion 7: at 1280 × 800 and at 960 × 540 with the Inspector open, no
/// edge chip, *Back* or kept statement overlaps the selected card, each
/// other, the minimap or the zoom bar; every edge chip is inside the board.
#[test]
fn lay15_criterion_7_chips_cover_nothing() {
    for size in [SIZE, MINIMUM] {
        let (_project, mut harness, id) = opened(size);
        select(&mut harness, &id, "second");
        assert!(!edge_chips(&harness, &id).is_empty(), "{size:?}: chips");
        assert_clear(&harness, &id, &format!("{size:?} selected"));
        click_label(&mut harness, SOURCES_UNFOLD);
        assert_eq!(edge_chips(&harness, &id).len(), 3, "{size:?}: unfolded");
        assert_clear(&harness, &id, &format!("{size:?} unfolded"));
        click_label(&mut harness, ROOMS_JUMP);
        assert!(back(&harness, &id).is_some(), "{size:?}: Back");
        assert_clear(&harness, &id, &format!("{size:?} after the jump"));
        click_label(&mut harness, BACK);
        assert_clear(&harness, &id, &format!("{size:?} back"));

        escape(&mut harness);
        keep_room_trace(&mut harness, &id);
        assert_clear(&harness, &id, &format!("{size:?} kept"));
        click_label(&mut harness, ROOMS_JUMP);
        assert!(back(&harness, &id).is_some(), "{size:?}: Back");
        assert!(!statements(&harness, &id).is_empty());
        assert_clear(&harness, &id, &format!("{size:?} kept, after the jump"));
    }
}

/// Criterion 8: the owner's walk, by clicks alone: `second`'s room chip,
/// the edge chip, `rooms` in view with the traced line's other end ringed,
/// *Back*, the view of the start. No pan, no zoom.
#[test]
fn lay15_criterion_8_the_walk_by_clicks() {
    let (_project, mut harness, id) = opened(SIZE);
    let camera = board(&harness, &id).debug_camera();
    let rooms = id_of(&harness, &id, "rooms");

    // 1. The room chip.
    let at = room_chip(&harness, &id).center();
    click(&mut harness, at);
    // 2. The edge chip.
    let chip = edge_chips(&harness, &id)
        .into_iter()
        .find(|chip| chip.text == ROOMS)
        .expect("the edge chip of rooms");
    click(&mut harness, chip.rect.center());
    // 3. `rooms` in view, the other end of the line ringed.
    assert!(in_view(&harness, &id, "rooms"), "rooms in view");
    let inside = board(&harness, &id).debug_board();
    assert!(
        board(&harness, &id)
            .debug_ringed_ports()
            .iter()
            .any(|(card, at)| *card == rooms && inside.contains(*at)),
        "rooms' port is ringed: {:?}",
        board(&harness, &id).debug_ringed_ports()
    );
    assert_eq!(board(&harness, &id).debug_zoom(), ZOOM, "no zoom");
    // 4. Back.
    let (_, rect) = back(&harness, &id).expect("Back");
    click(&mut harness, rect.center());
    assert_eq!(
        board(&harness, &id).debug_camera(),
        camera,
        "the view of the start"
    );
    assert_eq!(
        statements(&harness, &id),
        BTreeSet::from([ROOM_STATEMENT.to_owned()]),
        "with the trace kept"
    );
}

/// Criterion 9: opening the Inspector does not pan the board away from the
/// selection while edge chips are shown (UX-18).
#[test]
fn lay15_criterion_9_the_inspector_keeps_the_place() {
    let (_project, mut harness, id) = opened(SIZE);
    let before = card(&harness, &id, "second");
    assert!(harness.query_by_label("Close Inspector").is_none());
    select(&mut harness, &id, "second");
    assert!(
        harness.query_by_label("Close Inspector").is_some(),
        "the Inspector is open"
    );
    assert!(!edge_chips(&harness, &id).is_empty(), "edge chips are shown");
    // The board pans no more than UX-18's least pan, which keeps second
    // whole beside the Inspector: never away from it.
    let after = card(&harness, &id, "second");
    let focus = board(&harness, &id).debug_focus();
    let least = (before.right() - focus.right()).max(0.0);
    assert!(
        (after.left() - before.left()).abs() <= least + 1.0
            && (after.top() - before.top()).abs() <= 1.0,
        "second moves no more than it must: {before:?} then {after:?} in {focus:?}"
    );
    assert!(free(&harness, &id, after), "second whole in the free area");
    for chip in edge_chips(&harness, &id) {
        assert!(
            board(&harness, &id).debug_board().contains_rect(chip.rect),
            "{} inside the board",
            chip.text
        );
    }
}

/// A capture of the frame as it is drawn.
fn save(harness: &mut FolderHarness, name: &str) {
    crate::testing::capture(harness, name, None);
}

/// The views of the QA section at 1280 × 800: the kept trace leaving the
/// view with its edge chip, the jump with *Back*, `second` selected with
/// its sources unfolded and its Inspector at *Body value source*, the
/// destination with `rooms` selected, and the return.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay15() {
    let (_project, mut harness, id) = opened(SIZE);
    keep_room_trace(&mut harness, &id);
    save(&mut harness, "lay15-trace-edge-chip-1280");
    click_label(&mut harness, ROOMS_JUMP);
    save(&mut harness, "lay15-trace-after-jump-1280");
    click_label(&mut harness, BACK);
    save(&mut harness, "lay15-trace-after-back-1280");
    escape(&mut harness);

    select(&mut harness, &id, "second");
    save(&mut harness, "lay15-selected-folded-1280");
    open_body_source(&mut harness);
    click_label(&mut harness, SOURCES_UNFOLD);
    save(&mut harness, "lay15-selected-unfolded-1280");
    click_label(&mut harness, ROOMS_JUMP);
    select(&mut harness, &id, "rooms");
    save(&mut harness, "lay15-selected-after-jump-1280");
    click_label(&mut harness, BACK);
    save(&mut harness, "lay15-selected-after-back-1280");
}

