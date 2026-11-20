//! LAY-42 (#439): changing a field's source is reached from the field and
//! applied only on *Apply*. A kept statement offers *Change source*, which
//! opens the Inspector at the field; a step, an output or a response field
//! chosen there is held, framed `… not applied yet`, with a summary of the
//! change above *Apply* and *Cancel*, and nothing is written until *Apply*.
//! On a temporary copy of the room project (`fixtures/scenarios/lay/rooms/`),
//! `double-booking`, 1280 × 800.
use super::lay23::{graph, id_of, select, show};
use super::*;
use apiforge_core::format::{Binding, Graph};
use egui_kittest::kittest::{NodeT, Queryable};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const SMALL: egui::Vec2 = egui::vec2(960.0, 540.0);

/// The relation the owner traces: the chip of `second` that reads `rooms`.
const STATEMENT: &str = "rooms.body.items[1].id → second.body.roomId";
const BUTTON: &str = "Change source of second.body.roomId";

/// `second`'s body in the fixture, and as *Apply* writes it.
const BODY: &str = "body { roomId: rooms.body.items[1].id, start: slot.start, end: slot.end }";
const APPLIED: &str = "body { roomId: first.body.roomId, start: slot.start, end: slot.end }";

/// The summary of criterion 4, sentence by sentence.
const SUMMARY: [&str; 4] = [
    "second · Body · roomId will read first.body.roomId (sample \"room-a\") instead of rooms.body.items[1].id (sample \"room-b\").",
    "Only this field changes.",
    "second still runs 8th, in booked → Yes, after first.",
    "rooms is still read by first and calendar.",
];

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

/// The room project copied fresh with each `edits` pair replaced once in
/// `double-booking.afs`, the scenario open on its board at `size`, the
/// library folded, nothing selected, the whole board readable at 56 %.
fn rooms(size: egui::Vec2, edits: &[(&str, &str)]) -> (tempfile::TempDir, FolderHarness, String) {
    let project = tempfile::tempdir().unwrap();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/rooms");
    copy(&fixture, project.path());
    let path = "scenarios/double-booking.afs";
    let file = project.path().join(path);
    let mut text = fs::read_to_string(&file).unwrap();
    for (from, to) in edits {
        assert!(text.contains(from), "{from:?} is in {path}");
        text = text.replacen(from, to, 1);
    }
    fs::write(&file, text).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(path, None);
    settle(&mut harness);
    app(&mut harness).workbench.panels.library_collapsed = true;
    settle(&mut harness);
    let id = folder(&harness).workbench.active().unwrap().to_owned();
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    whole(&mut harness, &id);
    (project, harness, id)
}

/// The whole board of `double-booking` readable in the window: 56 %.
fn whole(harness: &mut FolderHarness, id: &str) {
    // A smaller window shows `second` and its chips at the same zoom.
    if harness.ctx.content_rect().width() < SIZE.x {
        show(harness, id, "second", 0.56);
        return;
    }
    show(harness, id, "rooms", 0.56);
    app(harness)
        .canvases
        .get_mut(id)
        .expect("the board")
        .debug_pan(egui::vec2(140.0, -85.0));
    settle(harness);
    harness.step();
}

fn step(harness: &mut FolderHarness) {
    settle(harness);
    harness.step();
    harness.step();
}

/// The text a node carries: its name, or the text of a label.
fn said(node: &egui_kittest::kittest::AccessKitNode<'_>) -> String {
    node.label().or_else(|| node.value()).unwrap_or_default()
}

fn has_text(harness: &FolderHarness, text: &str) -> bool {
    harness
        .query_all_by(|node| said(node) == text)
        .next()
        .is_some()
}

/// The text of the document as the session holds it.
fn text(harness: &FolderHarness, id: &str) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .text()
        .to_owned()
}

fn can_undo(harness: &FolderHarness, id: &str) -> (bool, bool) {
    let session = folder(harness).session.as_ref().unwrap();
    let document = session.document(id).unwrap();
    (document.can_undo(), document.can_redo())
}

/// The wires the board drew: id, source and reader.
fn wires(harness: &FolderHarness, id: &str) -> Vec<(String, Option<String>, String)> {
    let mut wires: Vec<_> = super::lay23::board(harness, id)
        .debug_wires()
        .iter()
        .map(|drawn| (drawn.id.clone(), drawn.source.clone(), drawn.target.clone()))
        .collect();
    wires.sort();
    wires
}

/// What criterion 3 compares: the text, its unsaved mark, the board's wires
/// and the undo history. The board draws the selected step's wires, so the
/// wires are compared while the same step is selected.
#[derive(Debug, PartialEq)]
struct Untouched {
    document: Document,
    wires: Vec<(String, Option<String>, String)>,
}

fn untouched(harness: &FolderHarness, id: &str) -> Untouched {
    Untouched {
        document: document(harness, id),
        wires: wires(harness, id),
    }
}

/// What criteria 2 and 7 compare: the text, its unsaved mark and the undo
/// history.
#[derive(Debug, PartialEq)]
struct Document {
    text: String,
    dirty: bool,
    history: (bool, bool),
}

fn document(harness: &FolderHarness, id: &str) -> Document {
    Document {
        text: text(harness, id),
        dirty: folder(harness).debug_dirty(id),
        history: can_undo(harness, id),
    }
}

/// A click at `at`, the pointer left there.
fn click_at(harness: &mut FolderHarness, at: egui::Pos2) {
    harness.hover_at(at);
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
}

/// A click on the chip whose statement holds `statement`, or where no chip
/// stands for it on its line: the relation is kept. `statement` ends with the
/// field read, `<reader>.<input>.<field>`.
fn keep_chip(harness: &mut FolderHarness, statement: &str) {
    let chip = harness
        .query_all_by(|node| {
            node.role() == egui::accesskit::Role::Button
                && said(node).lines().any(|line| line == statement)
        })
        .next()
        .map(|node| node.rect());
    let at = chip.map(|chip| chip.center()).unwrap_or_else(|| {
        let id = folder(harness).workbench.active().unwrap().to_owned();
        let target = statement.rsplit(" → ").next().unwrap();
        let mut parts = target.splitn(3, '.');
        let (reader, input, field) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap_or_default(),
        );
        let path: Vec<String> = field
            .split('.')
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect();
        let wire = apiforge_core::binding::data_edge_id(&id_of(harness, &id, reader), input, &path);
        super::lay14::point_on(harness, &id, &wire)
    });
    click_at(harness, at);
    step(harness);
}

/// A click on the room chip of `second`: its statement is kept.
fn keep(harness: &mut FolderHarness) {
    keep_chip(harness, STATEMENT);
}

/// *Change source* on the kept statement.
fn change_source(harness: &mut FolderHarness) {
    keep(harness);
    harness.get_by_label(BUTTON).click();
    step(harness);
    step(harness);
}

/// Brings the first node `matches` accepts into view in its list, then
/// clicks it.
fn click_listed(
    harness: &mut FolderHarness,
    matches: impl Fn(&egui_kittest::kittest::AccessKitNode<'_>) -> bool + Copy,
    what: &str,
) {
    harness
        .query_all_by(matches)
        .next()
        .unwrap_or_else(|| panic!("{what} is listed"))
        .scroll_to_me();
    step(harness);
    let rect = harness
        .query_all_by(matches)
        .next()
        .unwrap_or_else(|| panic!("{what} is listed"))
        .rect();
    click_at(harness, rect.center());
    step(harness);
}

/// Opens the combo box named `combo` and chooses `option` in it.
fn choose_in(harness: &mut FolderHarness, combo: &str, option: &str) {
    harness.get_by_label(combo).click();
    step(harness);
    // An option of the open list: a button, where the box is a combo box.
    click_listed(
        harness,
        |node| said(node) == option && node.role() == egui::accesskit::Role::Button,
        option,
    );
}

/// Chooses Step `first` and then Response field `roomId` in the `roomId`
/// editor.
fn choose_first_room(harness: &mut FolderHarness) {
    choose_in(harness, "roomId step", "first");
    pick_field(harness, "roomId", "roomId");
}

/// Opens the response field list of the `editor` field's editor and picks
/// `field`.
fn pick_field(harness: &mut FolderHarness, editor: &str, field: &str) {
    let below = harness.get_by_label(&format!("{editor} step")).rect().max.y;
    let button = harness
        .query_all_by(|node| {
            said(node).ends_with("▾") && node.role() == egui::accesskit::Role::Button
        })
        .filter(|node| node.rect().min.y >= below)
        .min_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
        .map(|node| said(&node.accesskit_node()))
        .expect("the response field button");
    click_listed(
        harness,
        |node| said(node) == button && node.role() == egui::accesskit::Role::Button,
        "the response field button",
    );
    let row = format!("{field} —");
    click_listed(harness, |node| said(node).starts_with(&row), field);
}

/// The value the combo box named `combo` shows.
fn shown(harness: &FolderHarness, combo: &str) -> String {
    harness.get_by_label(combo).value().unwrap_or_default()
}

/// The `roomId` editor's response field, as its button shows it.
fn field_shown(harness: &FolderHarness, field: &str) -> bool {
    harness
        .query_all_by(|node| said(node).trim_end_matches('▾').trim() == field)
        .next()
        .is_some()
}

/// The summary's sentences: the texts between the editor's controls (its
/// *Edit path manually* line) and *Apply*.
fn summary(harness: &FolderHarness) -> Vec<String> {
    let manual = staged_manual(harness);
    let apply = harness.get_by_label("Apply").rect();
    let mut lines: Vec<(f32, String)> = harness
        .query_all_by(|node| node.role() == egui::accesskit::Role::Label && !said(node).is_empty())
        .filter(|node| {
            let rect = node.rect();
            rect.min.y >= manual.max.y - 1.0
                && rect.max.y <= apply.min.y + 1.0
                && rect.min.x >= manual.min.x - 40.0
        })
        .map(|node| (node.rect().min.y, said(&node.accesskit_node())))
        .collect();
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines.dedup_by(|a, b| a.1 == b.1);
    lines.into_iter().map(|(_, text)| text).collect()
}

/// The *Edit path manually* line of the editor that holds a choice: the
/// one between its heading and *Apply*.
fn staged_manual(harness: &FolderHarness) -> egui::Rect {
    let heading = harness
        .query_all_by(|node| said(node).to_lowercase().contains("not applied yet"))
        .next()
        .expect("an editor holds a choice")
        .rect();
    let apply = harness.get_by_label("Apply").rect();
    harness
        .query_all_by_label("Edit path manually")
        .map(|node| node.rect())
        .find(|rect| rect.min.y > heading.max.y && rect.max.y < apply.min.y)
        .expect("the held editor's Edit path manually")
}

/// Criterion 8 over every summary a test read.
fn assert_plain_words(lines: &[String]) {
    for line in lines {
        let lower = line.to_lowercase();
        for word in ["@after", "wire", "edge"] {
            assert!(
                !lower.contains(word),
                "the summary never says {word:?}: {line:?}"
            );
        }
    }
}

/// Where the Inspector stands on screen: right of the board, under its tabs.
fn inspector(harness: &FolderHarness) -> egui::Rect {
    let tab = harness.get_by_label("Configuration").rect();
    let close = harness.get_by_label("Close Inspector").rect();
    let window = harness.ctx.content_rect();
    egui::Rect::from_min_max(
        egui::pos2(tab.min.x - 20.0, tab.max.y),
        egui::pos2(close.max.x + 20.0, window.max.y),
    )
}

/// `second`'s place in the run, the ordering into it and every binding but
/// its body's `roomId`.
fn rest(graph: &Graph, second: &str) -> (Option<usize>, Vec<String>, Vec<(String, String)>) {
    let place = apiforge_core::graph::sequence(graph)
        .iter()
        .position(|step| *step == second);
    let mut into: Vec<String> = graph
        .edges
        .iter()
        .filter(|edge| edge.target == second)
        .map(|edge| format!("{} {:?}", edge.source, edge.branch))
        .collect();
    into.sort();
    let mut bindings = Vec::new();
    for node in &graph.nodes {
        for (input, binding) in &node.inputs {
            let mut binding = binding.clone();
            if node.id == second
                && input == "body"
                && let Binding::Object { fields } = &mut binding
            {
                fields.shift_remove("roomId");
            }
            bindings.push((format!("{}.{input}", node.id), format!("{binding:?}")));
        }
    }
    (place, into, bindings)
}

#[test]
fn lay42_criterion_1_a_kept_statement_offers_change_source() {
    let (_project, mut harness, _id) = rooms(SIZE, &[]);
    // Pointed at only: the statement has no button.
    let chip = harness
        .query_all_by(|node| said(node).lines().any(|line| line == STATEMENT))
        .next()
        .expect("the room chip of second")
        .rect();
    harness.hover_at(chip.center());
    step(&mut harness);
    assert!(
        harness.query_by_label_contains("Change source").is_none(),
        "a statement only pointed at offers nothing"
    );
    harness.remove_cursor();
    step(&mut harness);
    keep(&mut harness);
    let button = harness.get_by_label(BUTTON);
    assert_eq!(
        button.accesskit_node().role(),
        egui::accesskit::Role::Button
    );
}

#[test]
fn lay42_criterion_2_change_source_opens_the_inspector_at_the_field() {
    let (_project, mut harness, id) = rooms(SIZE, &[]);
    let before = document(&harness, &id);
    change_source(&mut harness);
    let second = id_of(&harness, &id, "second");
    assert_eq!(
        folder(&harness).selections.get(&id),
        Some(&vec![second]),
        "second is selected"
    );
    assert!(
        has_text(&harness, "Body value source"),
        "Configuration is shown"
    );
    let editor = harness.get_by_label("roomId step").rect();
    let panel = inspector(&harness);
    assert!(
        panel.contains_rect(editor),
        "the roomId editor {editor:?} is inside the panel's view {panel:?}"
    );
    assert_eq!(shown(&harness, "roomId step"), "rooms");
    assert!(
        super::lay23::board(&harness, &id)
            .debug_statements()
            .iter()
            .any(|(text, _)| text == STATEMENT),
        "the kept trace stays"
    );
    assert_eq!(document(&harness, &id), before, "nothing is written");
}

#[test]
fn lay42_criterion_3_a_choice_writes_nothing() {
    let (_project, mut harness, id) = rooms(SIZE, &[]);
    change_source(&mut harness);
    let before = untouched(&harness, &id);
    choose_in(&mut harness, "roomId step", "first");
    assert_eq!(
        untouched(&harness, &id),
        before,
        "choosing a step writes nothing"
    );
    pick_field(&mut harness, "roomId", "roomId");
    assert_eq!(
        untouched(&harness, &id),
        before,
        "choosing a field writes nothing"
    );
    assert_eq!(shown(&harness, "roomId step"), "first");
    assert!(field_shown(&harness, "roomId"), "the editor shows roomId");
    assert!(
        harness
            .query_all_by(|node| said(node).to_lowercase().contains("not applied yet"))
            .next()
            .is_some(),
        "the editor is headed not applied yet"
    );
}

#[test]
fn lay42_criterion_4_and_8_the_summary_reads_the_change() {
    let (_project, mut harness, _id) = rooms(SIZE, &[]);
    change_source(&mut harness);
    choose_first_room(&mut harness);
    let lines = summary(&harness);
    assert_eq!(lines, SUMMARY, "the summary, sentence by sentence");
    assert_plain_words(&lines);
}

/// Criterion 5: the summary of a change on an edited copy, `handle`'s
/// `field` set to Step `step` · Response field `pick`, reached by its kept
/// statement `statement`.
fn summary_of(
    edits: &[(&str, &str)],
    statement: &str,
    reader: &str,
    field: &str,
    step_name: &str,
    pick: &str,
) -> Vec<String> {
    let (_project, mut harness, _id) = rooms(SIZE, edits);
    keep_chip(&mut harness, statement);
    let target = statement.rsplit(" → ").next().unwrap();
    harness
        .get_by_label(&format!("Change source of {target}"))
        .click();
    step(&mut harness);
    step(&mut harness);
    assert!(target.starts_with(reader));
    choose_in(&mut harness, &format!("{field} step"), step_name);
    pick_field(&mut harness, field, pick);
    let lines = summary(&harness);
    assert_plain_words(&lines);
    lines
}

#[test]
fn lay42_criterion_5_no_reader_left() {
    // `rooms` is read only by `second`.
    let lines = summary_of(
        &[
            ("roomId: rooms.body.items[0].id", "roomId: \"room-a\""),
            (
                "/rooms/{{rooms.body.items[0].id}}/bookings",
                "/rooms/room-a/bookings",
            ),
        ],
        STATEMENT,
        "second",
        "roomId",
        "first",
        "roomId",
    );
    assert!(
        lines.contains(&"rooms is no longer read by anything.".to_owned()),
        "{lines:#?}"
    );
}

#[test]
fn lay42_criterion_5_the_reader_keeps_another_read() {
    let lines = summary_of(
        &[(
            BODY,
            "body { roomId: rooms.body.items[1].id, start: rooms.body.items[1].name, end: slot.end }",
        )],
        STATEMENT,
        "second",
        "roomId",
        "first",
        "roomId",
    );
    assert!(
        lines.contains(&"second still reads rooms in start.".to_owned()),
        "{lines:#?}"
    );
}

#[test]
fn lay42_criterion_5_a_step_in_no_branch() {
    // `first` stands in no branch: its place has no "in …".
    let lines = summary_of(
        &[],
        "rooms.body.items[0].id → first.body.roomId",
        "first",
        "roomId",
        "rooms",
        "items[1].id",
    );
    let place = lines
        .iter()
        .find(|line| line.starts_with("first still runs"))
        .unwrap_or_else(|| panic!("{lines:#?}"));
    assert!(!place.contains(" in "), "{place}");
}

#[test]
fn lay42_criterion_5_no_sample() {
    let (_project, mut harness, _id) = rooms(SIZE, &[]);
    change_source(&mut harness);
    choose_in(&mut harness, "roomId step", "first");
    // A path the sample does not have, typed by hand.
    let manual = staged_manual(&harness);
    click_at(&mut harness, manual.center());
    step(&mut harness);
    harness.get_by_label("roomId field path").focus();
    step(&mut harness);
    harness
        .get_by_label("roomId field path")
        .type_text("room.number");
    step(&mut harness);
    let lines = summary(&harness);
    assert_plain_words(&lines);
    assert!(
        lines[0].contains("will read first.body.room.number (no sample) instead of"),
        "{lines:#?}"
    );
}

#[test]
fn lay42_criterion_5_a_source_in_the_other_branch() {
    // A step after the whole `if` reads `first`; `second` stands in
    // `booked → Yes`, so its value does not exist when `booked → No` runs.
    let lines = summary_of(
        &[(
            "  cleanup {",
            "  report = POST \"/reports\" {\n    body { booking: first.body.id }\n  }\n  cleanup {",
        )],
        "first.body.id → report.body.booking",
        "report",
        "booking",
        "second",
        "error",
    );
    assert!(
        lines.contains(&"This value does not exist when booked → No runs.".to_owned()),
        "{lines:#?}"
    );
}

#[test]
fn lay42_criterion_6_apply_writes_one_reference() {
    let (_project, mut harness, id) = rooms(SIZE, &[]);
    let original = text(&harness, &id);
    let second = id_of(&harness, &id, "second");
    let (place, into, bindings) = rest(&graph(&harness, &id), &second);
    change_source(&mut harness);
    choose_first_room(&mut harness);
    harness.get_by_label("Apply").click();
    step(&mut harness);
    let applied = text(&harness, &id);
    assert_eq!(
        applied,
        original.replacen(BODY, APPLIED, 1),
        "only the one line of second's body changes"
    );
    assert!(folder(&harness).debug_dirty(&id), "the document is unsaved");
    assert_eq!(
        rest(&graph(&harness, &id), &second),
        (place, into, bindings),
        "second's number, its branch and every other binding stay"
    );
    assert!(
        harness
            .query_all_by(|node| said(node).to_lowercase().contains("not applied yet"))
            .next()
            .is_none(),
        "the editor returns to its plain state"
    );
    assert_eq!(shown(&harness, "roomId step"), "first");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    step(&mut harness);
    assert_eq!(text(&harness, &id), original, "one undo step restores it");
}

/// Criterion 7: leaving the held choice by `leave` writes nothing, and the
/// editor opened again shows the old source.
fn leaves(leave: impl Fn(&mut FolderHarness, &str), how: &str) {
    let (_project, mut harness, id) = rooms(SIZE, &[]);
    let before = document(&harness, &id);
    change_source(&mut harness);
    choose_first_room(&mut harness);
    leave(&mut harness, &id);
    step(&mut harness);
    assert_eq!(document(&harness, &id), before, "{how}: nothing is written");
    // The editor, opened again.
    whole(&mut harness, &id);
    change_source(&mut harness);
    assert_eq!(shown(&harness, "roomId step"), "rooms", "{how}");
    assert!(field_shown(&harness, "items[1].id"), "{how}: items[1].id");
    assert!(
        harness
            .query_all_by(|node| said(node).to_lowercase().contains("not applied yet"))
            .next()
            .is_none(),
        "{how}: no choice is held"
    );
}

#[test]
fn lay42_criterion_7_cancel() {
    leaves(
        |harness, _| harness.get_by_label("Cancel").click(),
        "Cancel",
    );
}

#[test]
fn lay42_criterion_7_escape() {
    leaves(|harness, _| harness.key_press(egui::Key::Escape), "Esc");
}

#[test]
fn lay42_criterion_7_selecting_another_step() {
    leaves(
        |harness, id| select(harness, id, &["first"]),
        "selecting first",
    );
}

#[test]
fn lay42_criterion_7_closing_the_inspector() {
    leaves(
        |harness, _| harness.get_by_label("Close Inspector").click(),
        "closing the Inspector",
    );
}

#[test]
fn lay42_criterion_9_the_summary_fits_the_inspector() {
    for size in [SIZE, SMALL] {
        let (_project, mut harness, id) = rooms(size, &[]);
        keep(&mut harness);
        let board = super::lay23::board(&harness, &id).debug_board();
        let button = harness.get_by_label(BUTTON).rect();
        let (_, statement) = super::lay23::board(&harness, &id)
            .debug_statements()
            .into_iter()
            .find(|(text, _)| text == STATEMENT)
            .expect("the statement is kept");
        assert!(
            board.contains_rect(statement) && statement.contains_rect(button),
            "{size:?}: the statement {statement:?} with its button {button:?} stays whole inside the board {board:?}"
        );
        harness.get_by_label(BUTTON).click();
        step(&mut harness);
        step(&mut harness);
        choose_first_room(&mut harness);
        let panel = inspector(&harness);
        for label in ["Apply", "Cancel"] {
            let rect = harness.get_by_label(label).rect();
            assert!(
                panel.contains_rect(rect),
                "{size:?}: {label} {rect:?} is inside the Inspector {panel:?}"
            );
        }
        let lines = summary(&harness);
        assert_eq!(lines, SUMMARY, "{size:?}");
        for line in &lines {
            let rect = harness
                .query_all_by(|node| said(node) == *line)
                .next()
                .unwrap()
                .rect();
            assert!(
                rect.min.x >= panel.min.x && rect.max.x <= panel.max.x,
                "{size:?}: {line:?} {rect:?} fits the Inspector's width {panel:?}"
            );
        }
    }
}

/// The views of the QA report: *Change source* on the kept statement, the
/// editor holding `first` · `roomId` with its summary, after *Apply* and
/// after *Cancel*, at 1280 × 800.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay42() {
    let (_project, mut harness, _id) = rooms(SIZE, &[]);
    keep(&mut harness);
    crate::testing::capture(&mut harness, "lay42-qa-change-source-1280", None);
    harness.get_by_label(BUTTON).click();
    step(&mut harness);
    step(&mut harness);
    choose_first_room(&mut harness);
    crate::testing::capture(&mut harness, "lay42-qa-staged-1280", None);
    harness.get_by_label("Apply").click();
    step(&mut harness);
    crate::testing::capture(&mut harness, "lay42-qa-applied-1280", None);

    let (_project, mut harness, _id) = rooms(SIZE, &[]);
    change_source(&mut harness);
    choose_first_room(&mut harness);
    harness.get_by_label("Cancel").click();
    step(&mut harness);
    crate::testing::capture(&mut harness, "lay42-qa-cancelled-1280", None);
}
