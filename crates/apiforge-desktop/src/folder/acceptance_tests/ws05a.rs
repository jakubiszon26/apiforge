//! WS-05a (#574): the header above the board reads the scenario's title,
//! then the file name and the kind, and a double click on the title edits
//! it there. On a project of three scenarios kept here (WS-05b uses it), one
//! with a long title and a long file name, and on the demo shop's component.
use super::lay16::click_at;
use super::*;
use crate::testing::{MINIMUM, assert_disjoint, hover_until};
use apiforge_project::{FixedPlacer, Holder, HolderRole};
use egui::{Rect, Vec2, accesskit::Role, vec2};
use egui_kittest::kittest::NodeT;
use std::sync::Arc;

pub(super) const REFUSED: &str = "scenarios/double-booking.afs";
pub(super) const REFUSED_TITLE: &str = "Double booking is refused";
pub(super) const DAY: &str = "scenarios/booking-day.afs";
pub(super) const DAY_TITLE: &str = "A day of bookings";
pub(super) const LONG: &str = "scenarios/double-booking-notice-to-the-organiser.afs";
pub(super) const LONG_TITLE: &str = "The organiser is told when a double booking is refused";
/// The field's accessible name.
const FIELD: &str = "Title";
/// The least width the name of the header is cut to (KTLO-12).
const LEAST: f32 = 80.0;

/// A scenario as an author writes it: a comment before the first line and
/// inside, and spacing the printer would not write.
pub(super) fn scenario(title: &str) -> String {
    format!(
        r#"// Kept above the first line.
scenario "{title}" {{
  begin = start

  // Belongs to nothing.

  rooms   = GET "/rooms" {{
    query {{ floor: 2 }}
    sample 200 {{ items: [{{ id: "room-a", name: "Room A", seats: 8 }}] }}
  }}
  first = POST "/bookings" {{
    body {{ roomId: rooms.body.items[0].id, start: "2026-12-01T10:00:00Z" }}
    sample 201 {{ id: "bk-1", roomId: "room-a" }}
  }}
  booked = assert first.status == 201
}}
"#
    )
}

pub(super) fn rooms() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Room booking".into();
    project.environment.base_url = "https://rooms.example.test".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    for (path, title) in [
        (DAY, DAY_TITLE),
        (REFUSED, REFUSED_TITLE),
        (LONG, LONG_TITLE),
    ] {
        session
            .create_document(DocumentKind::Scenario, path, &scenario(title))
            .unwrap();
    }
    directory
}

fn id_at(harness: &FolderHarness, path: &str) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("the project has {path}"))
        .id()
        .to_owned()
}

fn open_at(root: &std::path::Path, path: &str, size: Vec2) -> (FolderHarness, String) {
    let mut harness = crate::testing::folder_harness_in(root, size, Mode::Diagram);
    app(&mut harness).review_open(path, None);
    settle(&mut harness);
    let id = id_at(&harness, path);
    (harness, id)
}

fn open(path: &str, size: Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = rooms();
    let (harness, id) = open_at(project.path(), path, size);
    (project, harness, id)
}

/// The text label with exactly `text` on the board's header row.
fn on_header(harness: &FolderHarness, text: &str) -> Option<Rect> {
    let row = harness.get_by_label("Add step").rect();
    let board = harness.get_by_label("Diagram board").rect();
    harness
        .query_all_by_role_and_label(Role::Label, text)
        .map(|node| node.rect())
        .find(|found| {
            (found.center().y - row.center().y).abs() <= 4.0 && found.left() >= board.left()
        })
}

fn editing(harness: &FolderHarness) -> bool {
    harness.query_by_label(FIELD).is_some()
}

fn double_click(harness: &mut FolderHarness, title: &str) {
    // Past the clicks before it, which would make it a third click.
    harness.run_steps(12);
    let at = on_header(harness, title)
        .unwrap_or_else(|| panic!("the header shows {title:?}"))
        .left_center()
        + vec2(8.0, 0.0);
    click_at(harness, at, egui::PointerButton::Primary, 2);
}

fn open_field(harness: &mut FolderHarness, title: &str) {
    double_click(harness, title);
    assert!(editing(harness), "a double click opens the field");
    let field = harness.get_by_label(FIELD);
    assert!(field.is_focused(), "the field has the keyboard");
    assert_eq!(
        field.accesskit_node().value().as_deref(),
        Some(title),
        "the field holds the whole title"
    );
}

fn press(harness: &mut FolderHarness, modifiers: egui::Modifiers, key: egui::Key) {
    harness.key_press_modifiers(modifiers, key);
    settle(harness);
    harness.step();
}

/// The field opened on `title` and `text` typed over the selected title.
fn type_title(harness: &mut FolderHarness, title: &str, text: &str) {
    open_field(harness, title);
    if text.is_empty() {
        harness.key_press(egui::Key::Backspace);
    } else {
        harness.get_by_label(FIELD).type_text(text);
    }
    settle(harness);
    harness.step();
    assert_eq!(
        harness
            .get_by_label(FIELD)
            .accesskit_node()
            .value()
            .unwrap_or_default(),
        text,
        "what is typed replaces the selected title"
    );
}

fn enter(harness: &mut FolderHarness) {
    press(harness, egui::Modifiers::NONE, egui::Key::Enter);
}

fn undo(harness: &mut FolderHarness) {
    press(harness, egui::Modifiers::COMMAND, egui::Key::Z);
}

fn dirty(harness: &FolderHarness, id: &str) -> bool {
    folder(harness).snapshot(id).unwrap().dirty
}

fn title_of(harness: &FolderHarness, id: &str) -> String {
    match &**folder(harness)
        .snapshot(id)
        .unwrap()
        .model
        .as_ref()
        .unwrap()
    {
        Document::Scenario(scenario) => scenario.name.clone(),
        Document::Component(component) => component.name.clone(),
    }
}

/// `text` typed in place of the document's code, as the Code view does, and
/// analysed; not saved.
fn write_code(harness: &mut FolderHarness, id: &str, text: &str) {
    let session = app(harness).session.as_mut().unwrap();
    session.edit_source(id, text, CODE).unwrap();
    session.settle(id).unwrap();
    settle(harness);
    harness.step();
}

/// `text` with an error: the last valid version stays on the board.
fn broken(text: &str) -> String {
    text.replace("assert first.status == 201", "assert first.status ==")
}

fn assert_nothing_written(harness: &mut FolderHarness, id: &str, before: &str, context: &str) {
    assert!(!editing(harness), "{context}: the field is closed");
    assert_eq!(source_of(harness, id), before, "{context}: the text");
    assert!(!dirty(harness, id), "{context}: not dirty");
    undo(harness);
    assert_eq!(source_of(harness, id), before, "{context}: no undo step");
    assert!(!dirty(harness, id), "{context}: no undo step");
}

/// Criterion 1: the title, then the file name, then the kind; a component
/// shows its name the same way.
#[test]
fn header_shows_title() {
    let (_project, harness, _) = open(REFUSED, WIDE);
    let parts = [REFUSED_TITLE, "double-booking.afs", "Scenario"];
    let rects = parts.map(|part| {
        on_header(&harness, part).unwrap_or_else(|| panic!("the header shows {part:?}"))
    });
    assert!(rects[0].right() <= rects[1].left() && rects[1].right() <= rects[2].left());
    assert!(rects[2].right() <= harness.get_by_label("Main").rect().left());

    let project = crate::testing::shop_project();
    let (harness, id) = open_at(project.path(), "components/product.afs", WIDE);
    let name = title_of(&harness, &id);
    let parts = [name.as_str(), "product.afs", "Component"];
    let rects = parts.map(|part| {
        on_header(&harness, part).unwrap_or_else(|| panic!("the header shows {part:?}"))
    });
    assert!(rects[0].right() <= rects[1].left() && rects[1].right() <= rects[2].left());
}

/// What of the header is shown: 0 all of it, 1 without the kind, 2 without
/// the file name as well, 3 with the title cut as well.
fn given_up(harness: &FolderHarness, title: &str, file: &str, full: f32, context: &str) -> u8 {
    let cut = on_header(harness, title)
        .unwrap_or_else(|| panic!("{context}: the title stays"))
        .width()
        < full - 0.5;
    let name = on_header(harness, file).is_some();
    let kind = on_header(harness, "Scenario").is_some();
    match (kind, name, cut) {
        (true, true, false) => 0,
        (false, true, false) => 1,
        (false, false, false) => 2,
        (false, false, true) => 3,
        other => panic!("{context}: (kind, file name, title cut) is {other:?}"),
    }
}

/// Criterion 2: the kind goes first, then the file name, whole or not at
/// all, then the title ends in an ellipsis, never below the least width.
#[test]
fn header_gives_way_in_order() {
    let file = "double-booking.afs";
    let (_project, mut harness, _) = open(REFUSED, WIDE);
    let full = on_header(&harness, REFUSED_TITLE).unwrap().width();
    let name = on_header(&harness, file).unwrap().width();
    let mut levels = Vec::new();
    for width in (76..=160).rev().map(|width| width as f32 * 10.0) {
        harness.set_size(vec2(width, 800.0));
        settle(&mut harness);
        harness.step();
        let context = format!("{width} px");
        let level = given_up(&harness, REFUSED_TITLE, file, full, &context);
        if let Some(file) = on_header(&harness, file) {
            assert!(file.width() >= name - 0.5, "{context}: the file name whole");
        }
        let title = on_header(&harness, REFUSED_TITLE).unwrap();
        // The text ends at the last letter that fits before the ellipsis.
        assert!(title.width() >= LEAST - 8.0, "{context}: {title:?}");
        assert!(
            levels.last().is_none_or(|last| *last <= level),
            "{context}: the row gives up less than when it was wider"
        );
        levels.push(level);
    }
    levels.dedup();
    assert_eq!(levels, [0, 1, 2, 3], "one thing at a time");

    for collapsed in [false, true] {
        let (_project, mut harness, _) = open(LONG, MINIMUM);
        app(&mut harness).workbench.panels.library_collapsed = collapsed;
        settle(&mut harness);
        harness.step();
        assert!(on_header(&harness, LONG_TITLE).is_some());
        assert_disjoint(
            &harness,
            &[
                LONG_TITLE,
                "Main",
                "Cleanup",
                "Add step",
                "Tidy up",
                "Checks",
                "Board view",
            ],
        );
        open_field(&mut harness, LONG_TITLE);
        assert_disjoint(
            &harness,
            &[
                FIELD,
                "Main",
                "Cleanup",
                "Add step",
                "Tidy up",
                "Checks",
                "Board view",
            ],
        );
    }
}

/// Criterion 3: hover reads the title, then the project-relative path, also
/// when neither is cut.
#[test]
fn header_hover() {
    for (path, title, size) in [(REFUSED, REFUSED_TITLE, WIDE), (LONG, LONG_TITLE, MINIMUM)] {
        let (_project, mut harness, _) = open(path, size);
        let labels = |harness: &FolderHarness, text: &str| {
            harness
                .query_all_by_role_and_label(Role::Label, text)
                .map(|node| node.rect())
                .collect::<Vec<_>>()
        };
        let (titles, paths) = (labels(&harness, title).len(), labels(&harness, path).len());
        assert!(
            hover_until(
                &mut harness,
                |harness| on_header(harness, title),
                |harness| {
                    labels(harness, title).len() == titles + 1
                        && labels(harness, path).len() == paths + 1
                },
            ),
            "{path}: hover reads the title and the path"
        );
        let below = |text: &str| {
            labels(&harness, text)
                .iter()
                .map(|rect| rect.top())
                .fold(f32::MIN, f32::max)
        };
        assert!(below(title) < below(path), "the title is above the path");
    }
}

/// Criterion 4: today's header without a title: a document that never had a
/// valid model shows the hint and no header, and an empty title leaves the
/// file name in bold, which nothing opens.
#[test]
fn header_without_a_title() {
    let project = rooms();
    fs::write(
        project.path().join(REFUSED),
        "scenario \"Broken\" {\n  x =\n",
    )
    .unwrap();
    let (harness, id) = open_at(project.path(), REFUSED, WIDE);
    assert!(folder(&harness).snapshot(&id).unwrap().model.is_none());
    assert!(harness.query_by_label("Broken").is_none());
    assert!(has_label(
        &harness,
        "The code has errors and there is no valid version to show. \
         Fix the code to see its diagram."
    ));

    for title in ["", "   "] {
        let project = rooms();
        fs::write(project.path().join(REFUSED), scenario(title)).unwrap();
        let (mut harness, id) = open_at(project.path(), REFUSED, WIDE);
        assert_eq!(title_of(&harness, &id), title);
        let name = on_header(&harness, "double-booking.afs").expect("the file name");
        let kind = on_header(&harness, "Scenario").expect("the kind");
        assert!(name.right() <= kind.left());
        // Bold: as wide as the title that says the same.
        let (_other, titled, _) = {
            let project = rooms();
            fs::write(project.path().join(REFUSED), scenario("double-booking.afs")).unwrap();
            let (harness, id) = open_at(project.path(), REFUSED, WIDE);
            (project, harness, id)
        };
        let bold = on_header(&titled, "double-booking.afs").unwrap();
        assert!(
            (name.width() - bold.width()).abs() < 0.5,
            "{name:?} {bold:?}"
        );
        let before = source_of(&harness, &id);
        double_click(&mut harness, "double-booking.afs");
        assert_nothing_written(&mut harness, &id, &before, "an empty title");
    }
}

/// Criterion 5: the title follows the code once it is analysed, not saved.
#[test]
fn title_follows_the_code() {
    let (_project, mut harness, id) = open(REFUSED, WIDE);
    let text = source_of(&harness, &id).replace(REFUSED_TITLE, "Two bookings of one room");
    write_code(&mut harness, &id, &text);
    assert!(dirty(&harness, &id), "not saved");
    assert!(on_header(&harness, "Two bookings of one room").is_some());
    assert!(on_header(&harness, REFUSED_TITLE).is_none());
}

/// Criterion 6: Return writes the `scenario` or `component` line and nothing
/// else, as one step of Undo.
#[test]
fn title_is_edited_in_the_header() {
    let (_project, mut harness, id) = open(REFUSED, WIDE);
    let before = source_of(&harness, &id);
    let shown = on_header(&harness, REFUSED_TITLE).unwrap();
    type_title(&mut harness, REFUSED_TITLE, "A second booking is refused");
    let field = harness.get_by_label(FIELD).rect();
    assert!(
        field.width() >= shown.width() && field.width() <= shown.width() + 16.0,
        "the field is the title's width: {field:?} over {shown:?}"
    );
    assert_eq!(source_of(&harness, &id), before, "nothing until Return");
    enter(&mut harness);
    assert!(!editing(&harness));
    let after = before.replacen(
        "scenario \"Double booking is refused\" {",
        "scenario \"A second booking is refused\" {",
        1,
    );
    assert_ne!(after, before);
    assert_eq!(source_of(&harness, &id), after, "the one line");
    assert!(dirty(&harness, &id));
    assert!(on_header(&harness, "A second booking is refused").is_some());
    undo(&mut harness);
    assert_eq!(source_of(&harness, &id), before, "one step of Undo");
    assert!(!dirty(&harness, &id));
    assert!(on_header(&harness, REFUSED_TITLE).is_some());

    let project = crate::testing::shop_project();
    let (mut harness, id) = open_at(project.path(), "components/product.afs", WIDE);
    let before = source_of(&harness, &id);
    let name = title_of(&harness, &id);
    let line = before
        .lines()
        .find(|line| line.starts_with("component "))
        .unwrap();
    let written = line.replace(&format!("\"{name}\""), "\"One product\"");
    assert_ne!(line, written);
    type_title(&mut harness, &name, "One product");
    enter(&mut harness);
    assert_eq!(
        source_of(&harness, &id),
        before.replacen(line, &written, 1),
        "the one line"
    );
    assert!(dirty(&harness, &id));
    undo(&mut harness);
    assert_eq!(source_of(&harness, &id), before, "one step of Undo");
}

/// Criterion 6, the rest of the text: a comment after the brace, Windows line
/// ends and a scenario with no step stay as they are written.
#[test]
fn title_edit_changes_one_line() {
    for text in [
        scenario(REFUSED_TITLE).replace("refused\" {", "refused\" { // why"),
        scenario(REFUSED_TITLE).replace('\n', "\r\n"),
        format!("scenario \"{REFUSED_TITLE}\" {{\n}}\n"),
    ] {
        let project = rooms();
        fs::write(project.path().join(REFUSED), &text).unwrap();
        let (mut harness, id) = open_at(project.path(), REFUSED, WIDE);
        type_title(&mut harness, REFUSED_TITLE, "Refused");
        enter(&mut harness);
        assert_eq!(
            source_of(&harness, &id),
            text.replacen(REFUSED_TITLE, "Refused", 1)
        );
        undo(&mut harness);
        assert_eq!(source_of(&harness, &id), text);
    }
}

/// Criterion 7: Escape, a click elsewhere, and Return on an unchanged or an
/// empty text write nothing.
#[test]
fn title_edit_is_abandoned() {
    let (_project, mut harness, id) = open(REFUSED, WIDE);
    let before = source_of(&harness, &id);

    type_title(&mut harness, REFUSED_TITLE, "Gone with Escape");
    press(&mut harness, egui::Modifiers::NONE, egui::Key::Escape);
    assert_nothing_written(&mut harness, &id, &before, "Escape");

    type_title(&mut harness, REFUSED_TITLE, "Gone with a click");
    let board = harness.get_by_label("Diagram board").rect();
    click_at(
        &mut harness,
        board.left_bottom() + vec2(120.0, -120.0),
        egui::PointerButton::Primary,
        1,
    );
    assert_nothing_written(&mut harness, &id, &before, "a click elsewhere");

    open_field(&mut harness, REFUSED_TITLE);
    enter(&mut harness);
    assert_nothing_written(&mut harness, &id, &before, "Return, unchanged");

    for text in ["", "   "] {
        type_title(&mut harness, REFUSED_TITLE, text);
        enter(&mut harness);
        assert_nothing_written(&mut harness, &id, &before, "Return, empty");
    }

    // A field left open goes with its view and with its tab.
    type_title(&mut harness, REFUSED_TITLE, "Gone with the view");
    choose(&mut harness, "Code");
    choose(&mut harness, "Diagram");
    assert_nothing_written(&mut harness, &id, &before, "another view");
    type_title(&mut harness, REFUSED_TITLE, "Gone with the tab");
    app(&mut harness).review_open(DAY, None);
    settle(&mut harness);
    assert!(!editing(&harness), "the field is not another tab's");
    assert!(on_header(&harness, DAY_TITLE).is_some());
    app(&mut harness).review_open(REFUSED, None);
    settle(&mut harness);
    assert_nothing_written(&mut harness, &id, &before, "another tab");
}

/// Criterion 8: what is typed is read back by the analysis as it is.
#[test]
fn title_edit_escapes() {
    let (_project, mut harness, id) = open(REFUSED, WIDE);
    let mut shown = REFUSED_TITLE.to_owned();
    for title in [
        r#"The "second" booking"#,
        r"rooms\first\n",
        "Zażółć gęślą jaźń — 予約",
        "{{rooms}} in braces",
        r#"\"{{"#,
    ] {
        type_title(&mut harness, &shown, title);
        enter(&mut harness);
        assert_eq!(title_of(&harness, &id), title);
        // Read from the text again, by the analysis.
        let text = source_of(&harness, &id);
        write_code(&mut harness, &id, &format!("{text}\n"));
        assert_eq!(
            folder(&harness).snapshot(&id).unwrap().model_state,
            ModelState::Current,
            "{text}"
        );
        assert_eq!(title_of(&harness, &id), title, "{text}");
        assert!(on_header(&harness, title).is_some());
        shown = title.to_owned();
    }
}

/// Criterion 9: nothing opens on a read-only or observed board, nor while
/// the code has errors; the title is still shown.
#[test]
fn title_edit_needs_an_editable_board() {
    // The code has errors: the last valid version's title.
    let (_project, mut harness, id) = open(REFUSED, WIDE);
    let text = source_of(&harness, &id);
    write_code(&mut harness, &id, &broken(&text));
    assert_eq!(
        folder(&harness).snapshot(&id).unwrap().model_state,
        ModelState::LastValid
    );
    double_click(&mut harness, REFUSED_TITLE);
    assert!(!editing(&harness), "the code has errors");

    // Read-only: the file changed on disk under unsaved work.
    let (project, mut harness, id) = open(REFUSED, WIDE);
    let text = source_of(&harness, &id);
    write_code(&mut harness, &id, &text.replace("floor: 2", "floor: 3"));
    fs::write(
        project.path().join(REFUSED),
        text.replace("floor: 2", "floor: 4"),
    )
    .unwrap();
    follow_disk(&mut harness);
    let snapshot = folder(&harness).snapshot(&id).unwrap();
    assert!(!snapshot.editable && snapshot.model.is_some(), "read-only");
    double_click(&mut harness, REFUSED_TITLE);
    assert!(!editing(&harness), "the board is read-only");

    // Observed: another process holds the project.
    let project = rooms();
    let agent = Holder::new(HolderRole::Agent, Some("Claude Code".into()));
    let _owner =
        ProjectSession::open_with_as(project.path(), Arc::new(FixedPlacer), &agent).unwrap();
    let (mut harness, _) = open_at(project.path(), REFUSED, WIDE);
    assert!(has_label(&harness, "Observing"));
    double_click(&mut harness, REFUSED_TITLE);
    assert!(!editing(&harness), "the board is observed");
}

/// Criterion 10: the board's keys do not act while the field is open, and
/// Escape leaves the field and keeps the selection.
#[test]
fn title_edit_keeps_the_boards_keys() {
    let (_project, mut harness, id) = open(REFUSED, WIDE);
    let before = source_of(&harness, &id);
    let step = graph_of(
        folder(&harness)
            .snapshot(&id)
            .unwrap()
            .model
            .as_ref()
            .unwrap(),
        false,
    )
    .nodes
    .iter()
    .find(|node| node.handle == "first")
    .unwrap()
    .id
    .clone();
    let selected = vec![step];
    app(&mut harness)
        .selections
        .insert(id.clone(), selected.clone());
    settle(&mut harness);
    harness.step();

    open_field(&mut harness, REFUSED_TITLE);
    for (modifiers, key) in [
        (egui::Modifiers::ALT, egui::Key::ArrowUp),
        (egui::Modifiers::ALT, egui::Key::ArrowDown),
        (egui::Modifiers::NONE, egui::Key::Delete),
        (egui::Modifiers::NONE, egui::Key::Backspace),
        (egui::Modifiers::COMMAND, egui::Key::D),
    ] {
        press(&mut harness, modifiers, key);
        assert!(editing(&harness), "{key:?} leaves the field open");
        assert_eq!(source_of(&harness, &id), before, "{key:?}");
        assert_eq!(folder(&harness).selections.get(&id), Some(&selected));
    }
    press(&mut harness, egui::Modifiers::NONE, egui::Key::Escape);
    assert!(!editing(&harness), "Escape leaves the field");
    assert_eq!(
        folder(&harness).selections.get(&id),
        Some(&selected),
        "the selection stays"
    );
    assert_eq!(source_of(&harness, &id), before);
    press(&mut harness, egui::Modifiers::NONE, egui::Key::Escape);
    assert!(
        folder(&harness)
            .selections
            .get(&id)
            .is_none_or(|selection| selection.is_empty()),
        "then Escape is the board's again"
    );
}

/// Criterion 11: Split's header has no title, no file name and no kind.
#[test]
fn split_header_unchanged() {
    let (_project, mut harness, _) = open(REFUSED, WIDE);
    choose(&mut harness, "Split");
    for part in [REFUSED_TITLE, "double-booking.afs", "Scenario"] {
        assert!(on_header(&harness, part).is_none(), "{part}");
    }
    let board = harness.get_by_label("Diagram board").rect();
    let main = harness.get_by_label("Main").rect();
    assert!(
        main.left() - board.left() < 24.0,
        "Main is first on the row: {main:?} on {board:?}"
    );
}

/// Criterion 12: the header at both sizes: a short and a long title, the
/// edited state, a component, and a document that never parsed.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ws05_header() {
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        for (path, title, which) in [
            (REFUSED, REFUSED_TITLE, "short"),
            (LONG, LONG_TITLE, "long"),
        ] {
            let (_project, mut harness, _) = open(path, size);
            choose(&mut harness, "Files");
            crate::testing::capture(&mut harness, &format!("ws05a-{which}-{suffix}"), None);
            open_field(&mut harness, title);
            crate::testing::capture(&mut harness, &format!("ws05a-{which}-edit-{suffix}"), None);
        }
        let project = crate::testing::shop_project();
        let (mut harness, id) = open_at(project.path(), "components/product.afs", size);
        crate::testing::capture(&mut harness, &format!("ws05a-component-{suffix}"), None);
        let name = title_of(&harness, &id);
        open_field(&mut harness, &name);
        crate::testing::capture(
            &mut harness,
            &format!("ws05a-component-edit-{suffix}"),
            None,
        );

        let project = rooms();
        fs::write(
            project.path().join(REFUSED),
            "scenario \"Broken\" {\n  x =\n",
        )
        .unwrap();
        let (mut harness, _) = open_at(project.path(), REFUSED, size);
        crate::testing::capture(&mut harness, &format!("ws05a-unparsed-{suffix}"), None);

        let (_project, mut harness, id) = open(LONG, size);
        let text = source_of(&harness, &id);
        write_code(&mut harness, &id, &broken(&text));
        crate::testing::capture(&mut harness, &format!("ws05a-errors-{suffix}"), None);
    }
}
