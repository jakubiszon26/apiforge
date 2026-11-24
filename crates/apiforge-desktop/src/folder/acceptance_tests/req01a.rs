//! REQ-01a (#457): *Steps*, a scenario read as a numbered list of its steps
//! with their checks. Decided by the owner on 2026-11-24 (REQ-00: 1, 5, 8 =
//! A) with the rulings of the same day. On the `labels` reference scenario
//! (FIX-01) and on `rooms/double-booking`. The rules of SENDS that no
//! reference scenario shows are unit tests of `steps/sends.rs`.
use super::*;
use crate::testing::{LABELS, MINIMUM, assert_disjoint, assert_within_window, capture};
use apiforge_core::format::{Graph, Node, Project};
use apiforge_core::graph::sequence;
use apiforge_core::model::NodeKind;
use egui_kittest::kittest::NodeT;
use std::path::{Path, PathBuf};

const STEPS: &str = "Steps";
const CANVAS: &str = "Canvas";
const ROOMS: &str = "scenarios/double-booking.afs";
const CLEANUP: &str = "CLEANUP · runs after the test, also when a check fails";
const DIFFERS: &str = "SENDS · differs from the common request";

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(path)
}

fn labels_text() -> String {
    fs::read_to_string(fixture("labels.afs")).unwrap()
}

/// `labels` with `weight_31`'s response naming the field `mass`: its check
/// `weight_31_field` fails.
fn failing() -> String {
    let text = labels_text();
    let failing = text.replacen(
        r#"field: "weight", message: "Weight must be at most 30 kg.""#,
        r#"field: "mass", message: "Weight must be at most 30 kg.""#,
        1,
    );
    assert_ne!(failing, text);
    failing
}

/// `text` as the only document of a project, at `path`, open in Diagram.
fn opened(size: egui::Vec2, path: &str, text: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Label validation".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = apiforge_project::ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, path, text)
        .unwrap();
    drop(session);
    let mut harness = crate::testing::folder_harness_in(directory.path(), size, Mode::Diagram);
    app(&mut harness).review_open(path, None);
    settle(&mut harness);
    (directory, harness, id)
}

fn labels(size: egui::Vec2, text: &str) -> (tempfile::TempDir, FolderHarness, String) {
    opened(size, LABELS, text)
}

/// `rooms/double-booking` open in Diagram, `edit` made to its text.
fn rooms_with(
    size: egui::Vec2,
    edit: impl FnOnce(String) -> String,
) -> (tempfile::TempDir, FolderHarness, String) {
    fn copy_tree(from: &Path, to: &Path) {
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                fs::create_dir_all(&target).unwrap();
                copy_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let project = tempfile::tempdir().unwrap();
    copy_tree(&fixture("rooms"), project.path());
    let path = project.path().join(ROOMS);
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, edit(text)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(ROOMS, None);
    settle(&mut harness);
    let id = folder(&harness).workbench.active().unwrap().to_owned();
    (project, harness, id)
}

fn rooms(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    rooms_with(size, |text| text)
}

fn steps(harness: &mut FolderHarness) {
    harness.get_by_label(STEPS).click();
    settle(harness);
}

fn canvas(harness: &mut FolderHarness) {
    harness.get_by_label(CANVAS).click();
    settle(harness);
}

/// Replaces the document's text, as typing it does.
fn edit_source(harness: &mut FolderHarness, id: &str, text: &str) {
    app(harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(id, text, CODE)
        .unwrap();
    app(harness).analyser.typed(id, Instant::now());
    settle(harness);
}

/// Puts the caret into the document's editor at `at`.
fn put_caret(harness: &mut FolderHarness, id: &str, at: usize) {
    let id = editor_id(id);
    harness.ctx.memory_mut(|memory| memory.request_focus(id));
    app(harness).code.select(at, at);
    harness.run_steps(3);
    settle(harness);
}

fn click_at(harness: &mut FolderHarness, at: egui::Pos2) {
    harness.hover_at(at);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    settle(harness);
}

/// A finished simulation of the active scenario, with Run results closed.
fn ran(harness: &mut FolderHarness) -> RunResult {
    choose(harness, "Run simulation");
    let result = finish_run(harness);
    choose(harness, "Run results");
    result
}

fn graphs(harness: &FolderHarness, id: &str) -> (Graph, Graph, std::sync::Arc<Project>) {
    let snapshot = folder(harness).snapshot(id).unwrap();
    let model = snapshot.model.as_ref().unwrap();
    (
        graph_of(model, false).clone(),
        graph_of(model, true).clone(),
        folder(harness).context_project(),
    )
}

fn node<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
}

/// One row of the list: its accessible name and place.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    rect: egui::Rect,
    selected: bool,
}

/// The rows of the list, top to bottom.
fn rows(harness: &FolderHarness) -> Vec<Row> {
    buttons(harness, |name| {
        name.starts_with("Step ") || name.starts_with("Cleanup step ")
    })
}

/// The buttons whose names `keep` accepts, top to bottom.
fn buttons(harness: &FolderHarness, keep: impl Fn(&str) -> bool) -> Vec<Row> {
    let mut rows: Vec<Row> = harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter_map(|found| {
            let name = found.accesskit_node().label()?.to_string();
            keep(&name).then(|| Row {
                rect: found.rect(),
                selected: found.accesskit_node().is_selected() == Some(true),
                name,
            })
        })
        .collect();
    rows.sort_by(|a, b| a.rect.top().total_cmp(&b.rect.top()));
    rows
}

fn names(harness: &FolderHarness) -> Vec<String> {
    rows(harness).into_iter().map(|row| row.name).collect()
}

/// The row whose name contains `part`, the only one.
fn row(harness: &FolderHarness, part: &str) -> Row {
    let mut found: Vec<Row> = rows(harness)
        .into_iter()
        .filter(|row| row.name.contains(part))
        .collect();
    assert_eq!(found.len(), 1, "{part:?} in {:#?}", names(harness));
    found.remove(0)
}

/// The checks listed inside `row`, top to bottom, by their whole names.
fn checks_in(harness: &FolderHarness, row: &Row) -> Vec<String> {
    buttons(harness, |name| {
        name.contains(" of ") && !name.starts_with("Step ") && !name.starts_with("Cleanup step ")
    })
    .into_iter()
    .filter(|found| row.rect.contains_rect(found.rect))
    .map(|found| found.name)
    .collect()
}

/// The check whose line is named `name`.
fn check_of<'a>(graph: &'a Graph, name: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| name.contains(&format!(" · {} of ", node.handle)))
        .unwrap_or_else(|| panic!("a check of {name}"))
}

/// The texts inside `row`, with their places, top to bottom.
fn texts(harness: &FolderHarness, row: &Row) -> Vec<(String, egui::Rect)> {
    let mut texts: Vec<(String, egui::Rect)> = harness
        .query_all_by_role(egui::accesskit::Role::Label)
        .filter(|found| row.rect.expand(0.5).contains_rect(found.rect()))
        .filter_map(|found| {
            let node = found.accesskit_node();
            Some((node.value().or_else(|| node.label())?, found.rect()))
        })
        .collect();
    texts.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    texts
}

/// The lines under the caption that starts with `caption` in `row`.
fn column(harness: &FolderHarness, row: &Row, caption: &str) -> Vec<String> {
    let texts = texts(harness, row);
    let (_, at) = texts
        .iter()
        .find(|(text, _)| text.starts_with(caption))
        .unwrap_or_else(|| panic!("{caption:?} in {}: {texts:?}", row.name));
    texts
        .iter()
        .filter(|(_, rect)| {
            (rect.left() - at.left()).abs() < 1.0 && rect.top() >= at.bottom() - 0.5
        })
        .map(|(text, _)| text.clone())
        .collect()
}

/// Every text painted inside `area`: its galley and where it stands.
fn painted(
    harness: &FolderHarness,
    area: egui::Rect,
) -> Vec<(std::sync::Arc<egui::Galley>, egui::Rect)> {
    fn walk(
        shape: &egui::Shape,
        clip: egui::Rect,
        out: &mut Vec<(std::sync::Arc<egui::Galley>, egui::Rect)>,
    ) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, clip, out)),
            egui::Shape::Text(text) => {
                let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                // A text the list's edge cuts is cut by scrolling.
                if clip.contains_rect(rect) {
                    out.push((text.galley.clone(), rect));
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, clipped.clip_rect.intersect(area), &mut out);
    }
    out
}

/// The place of the list: below the header, from the first row down.
fn list_area(harness: &FolderHarness) -> egui::Rect {
    // Under the tabs' row with the switch and the header of *Steps*.
    let switch = harness.get_by_label(STEPS).rect();
    let window = harness.ctx.content_rect();
    egui::Rect::from_min_max(
        egui::pos2(rows(harness)[0].rect.left() - 100.0, switch.bottom() + 52.0),
        window.max,
    )
}

/// Criterion 9 at one moment: no two painted texts overlap, and a cut text
/// ends where its column does.
fn assert_texts_fit(harness: &FolderHarness, context: &str) {
    let texts = painted(harness, list_area(harness));
    assert!(texts.len() > 10, "{context}: the list is drawn");
    for (index, (first, a)) in texts.iter().enumerate() {
        for (second, b) in &texts[index + 1..] {
            let over = a.shrink(0.75).intersect(b.shrink(0.75));
            assert!(
                !over.is_positive(),
                "{context}: {:?} at {a:?} overlaps {:?} at {b:?}",
                first.text(),
                second.text()
            );
        }
        if first.elided {
            let room = first.job.wrap.max_width;
            assert!(
                room - a.width() < 8.5,
                "{context}: {:?} is cut at {} of {room}",
                first.text(),
                a.width()
            );
        }
    }
}

fn scroll_to(harness: &mut FolderHarness, part: &str) {
    let name = row(harness, part).name;
    harness.get_by_label(&name).scroll_to_me();
    settle(harness);
}

/// Criterion 1: *Steps | Canvas* is on the header above the board, the first
/// open is *Canvas*, and the choice is kept per project, in the local view
/// state alone and only while it is *Steps*.
#[test]
fn req01a_criterion_1_the_choice_is_kept_with_the_projects_view() {
    let (project, mut harness, _) = labels(WIDE, &labels_text());
    let state = project.path().join(".apiforge/local/workspace.json");
    let saved = |harness: &mut FolderHarness| -> serde_json::Value {
        // A view is written a second after its change.
        let deadline = Instant::now() + Duration::from_secs(15);
        while folder(harness).changed_at.is_some() {
            assert!(Instant::now() < deadline, "the view state was not written");
            harness.step();
            std::thread::sleep(Duration::from_millis(10));
        }
        settle(harness);
        serde_json::from_slice(&fs::read(&state).unwrap()).unwrap()
    };
    assert!(
        harness.query_by_label("Diagram board").is_some(),
        "the first open is Canvas"
    );
    let manifest = fs::read(project.path().join("apiforge.project.json")).unwrap();
    steps(&mut harness);
    assert!(harness.query_by_label("Diagram board").is_none());
    assert_eq!(rows(&harness).len(), 16, "the list");
    assert_eq!(saved(&mut harness)["panels"]["steps"], true);
    assert_eq!(
        fs::read(project.path().join("apiforge.project.json")).unwrap(),
        manifest,
        "the project's own files are as they were"
    );

    // Code and Split are as without it, and Diagram comes back to the list.
    choose(&mut harness, "Split");
    assert!(harness.query_by_label("Diagram board").is_some());
    assert!(harness.query_by_label(STEPS).is_none());
    choose(&mut harness, "Diagram");
    assert_eq!(rows(&harness).len(), 16);
    saved(&mut harness);
    drop(harness);

    let mut harness = crate::testing::first_open_harness(project.path(), WIDE, Some(Mode::Diagram));
    settle(&mut harness);
    assert_eq!(rows(&harness).len(), 16, "reopened on the list");
    canvas(&mut harness);
    assert!(harness.query_by_label("Diagram board").is_some());
    assert!(saved(&mut harness)["panels"].get("steps").is_none());
}

/// Criterion 1, as the owner decided on 2026-11-24: the switch stands at the
/// right of the file tabs' row, in the same place in *Steps* and in *Canvas*,
/// with a step selected or not, at every width; a long title never meets it.
#[test]
fn req01a_criterion_1_the_switch_stands_at_the_right_of_the_tabs() {
    // A file name of sixty characters stands for the title of WS-05a.
    let name = "a-parcel-label-is-refused-without-a-token-and-over-30-kg.afs";
    assert_eq!(name.chars().count(), 60);
    for size in [MINIMUM, egui::vec2(1280.0, 800.0), WIDE] {
        let (_project, mut harness, id) =
            opened(size, &format!("scenarios/{name}"), &labels_text());
        let places = |harness: &FolderHarness| {
            [STEPS, CANVAS].map(|words| harness.get_by_label(words).rect())
        };
        let tab = harness.get_by_label(&format!("Close {name}")).rect();
        let canvas_places = places(&harness);
        for place in canvas_places {
            assert!(
                (place.center().y - tab.center().y).abs() < 4.0,
                "the tabs' row"
            );
        }
        let window = harness.ctx.content_rect();
        assert!(
            window.right() - canvas_places[1].right() < 32.0,
            "at the right"
        );
        let title = |harness: &FolderHarness| {
            harness
                .query_all_by_role_and_label(egui::accesskit::Role::Label, name)
                .map(|found| found.rect())
                .max_by(|a, b| a.top().total_cmp(&b.top()))
                .expect("the header names the file")
        };
        let mut all = vec![STEPS, CANVAS];
        all.extend(["Main", "Cleanup", "Add step", "Tidy up", "Board view"]);
        assert_within_window(&harness, &all);
        assert_disjoint(&harness, &all);
        let board = harness.get_by_label("Diagram board").rect();
        assert!(canvas_places[0].bottom() <= board.top());
        assert!(
            canvas_places
                .iter()
                .all(|place| !title(&harness).intersects(*place))
        );
        steps(&mut harness);
        assert_eq!(places(&harness), canvas_places, "the same place in Steps");
        assert!(
            canvas_places
                .iter()
                .all(|place| !title(&harness).intersects(*place))
        );
        // A selected step opens the Inspector in Canvas.
        let first = node(&graphs(&harness, &id).0, "no_token").id.clone();
        app(&mut harness).selections.insert(id.clone(), vec![first]);
        settle(&mut harness);
        assert_eq!(places(&harness), canvas_places, "a step selected");
        canvas(&mut harness);
        assert!(harness.query_by_label("Resize inspector").is_some());
        assert_eq!(places(&harness), canvas_places, "the Inspector open");
        // The chosen one is selected for assistive technology.
        for (words, chosen) in [(STEPS, false), (CANVAS, true)] {
            let found = harness.get_by_role_and_label(egui::accesskit::Role::Button, words);
            let node = found.accesskit_node();
            assert_eq!(
                node.toggled() == Some(egui::accesskit::Toggled::True),
                chosen,
                "{words}"
            );
        }
        // The keyboard.
        harness.get_by_label(STEPS).focus();
        harness.step();
        harness.key_press(egui::Key::Enter);
        settle(&mut harness);
        assert!(harness.query_by_label("Diagram board").is_none());
    }
}

/// The part of each tab's name that is on screen, left to right.
fn tab_names(harness: &FolderHarness) -> Vec<(String, egui::Rect)> {
    let row = harness
        .query_all_by_label_contains("Close ")
        .map(|found| found.rect())
        .next()
        .expect("a tab")
        .y_range();
    let mut names = Vec::new();
    for clipped in &harness.output().shapes {
        if let egui::Shape::Text(text) = &clipped.shape {
            let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
            let seen = rect.intersect(clipped.clip_rect);
            if row.contains(rect.center().y)
                && seen.is_positive()
                && text.galley.text().ends_with(".afs")
            {
                names.push((text.galley.text().to_owned(), seen));
            }
        }
    }
    names.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
    names
}

/// Criterion 1, the owner's decision of 2026-11-24: with many files open the
/// tabs give way and scroll, the switch stays whole in its place and the
/// active tab stays in view; a component's tab and Split, which have no
/// switch, give the tabs their room back.
#[test]
fn req01a_criterion_1_many_tabs_give_way_to_the_switch() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _) = rooms(size);
        let place = harness.get_by_label(STEPS).rect();
        let session = app(&mut harness).session.as_mut().unwrap();
        let mut opened = Vec::new();
        for index in 0..16 {
            let path = format!("scenarios/another-test-{index}.afs");
            let text = format!("scenario \"T{index}\" {{\n  begin = start\n}}\n");
            session
                .create_document(DocumentKind::Scenario, &path, &text)
                .unwrap();
            opened.push(path);
        }
        for path in &opened {
            app(&mut harness).review_open(path, None);
            settle(&mut harness);
        }
        let last = "another-test-15.afs";
        for _ in 0..2 {
            let switch = harness.get_by_label(STEPS).rect();
            assert_eq!(switch, place, "the switch stays in its place");
            assert_within_window(&harness, &[STEPS, CANVAS]);
            let names = tab_names(&harness);
            for (name, seen) in &names {
                assert!(seen.right() < switch.left(), "{name} at {seen:?}");
            }
            let (_, seen) = names
                .iter()
                .find(|(name, _)| name == last)
                .expect("the active tab is in view");
            assert!(seen.width() > 80.0, "whole: {seen:?}");
            let close = harness.get_by_label(&format!("Close {last}")).rect();
            assert!(close.right() < switch.left(), "its close control too");
            steps(&mut harness);
        }
        // Split has no switch: the tabs reach the edge again.
        choose(&mut harness, "Split");
        assert!(harness.query_by_label(STEPS).is_none());
        let close = harness.get_by_label(&format!("Close {last}")).rect();
        assert!(close.right() > place.left(), "{close:?} in Split");
        // Neither has a component in Diagram.
        choose(&mut harness, "Diagram");
        let component = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().kind == DocumentKind::Component)
            .unwrap()
            .entry()
            .path
            .as_str()
            .to_owned();
        app(&mut harness).review_open(&component, None);
        settle(&mut harness);
        assert!(harness.query_by_label(STEPS).is_none());
        let close = harness
            .get_by_label(&format!("Close {}", file_name(&component)))
            .rect();
        assert!(close.right() > place.left(), "{close:?} of a component");
    }
}

/// Criterion 2: on `labels` the rows and their numbers are the engine's
/// order, the written order and the board's badges.
#[test]
fn req01a_criterion_2_rows_are_numbered_as_the_board() {
    let (_project, mut harness, id) = labels(WIDE, &labels_text());
    let (main, _, _) = graphs(&harness, &id);
    let order = sequence(&main);
    let written: Vec<&str> = main.nodes.iter().map(|node| node.id.as_str()).collect();
    assert_eq!(order, written, "on labels the order is as written");
    let badges: std::collections::HashMap<String, String> = order
        .iter()
        .filter_map(|step| {
            let badge = folder(&harness).canvases[&id].debug_badge(step)?;
            Some(((*step).to_owned(), badge.text))
        })
        .collect();
    steps(&mut harness);
    let mut listed: Vec<String> = Vec::new();
    for row in rows(&harness) {
        let (number, rest) = row
            .name
            .strip_prefix("Step ")
            .and_then(|rest| rest.split_once(" · "))
            .unwrap();
        let number: usize = number.parse().unwrap();
        let step = main
            .nodes
            .iter()
            .find(|node| node.id == order[number - 1])
            .unwrap();
        assert_eq!(rest, format!("POST {}", step.display_name()));
        assert_eq!(badges.get(&step.id), Some(&number.to_string()), "{rest}");
        listed.push(step.id.clone());
        for check in checks_in(&harness, &row) {
            listed.push(check_of(&main, &check).id.clone());
        }
    }
    let without_start: Vec<&str> = order
        .iter()
        .copied()
        .filter(|id| {
            main.nodes
                .iter()
                .any(|node| node.id == *id && node.kind != NodeKind::Start)
        })
        .collect();
    assert_eq!(listed, without_start);
    assert!(harness.query_by_label("16 steps · 33 checks").is_some());
}

/// Criterion 3: on `double-booking` every step is a row, the branches stand
/// under *then* and *else*, and `not_booked` is under *else* and keeps the
/// name of the step it reads.
#[test]
fn req01a_criterion_3_every_step_is_a_row_in_its_branch() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _) = rooms(size);
        steps(&mut harness);
        assert_eq!(
            names(&harness),
            [
                concat!(
                    "Step 2 · value slot = { start: \"2026-12-01T10:00:00Z\",",
                    " end: \"2026-12-01T11:00:00Z\" }"
                ),
                "Step 3 · call token",
                "Step 4 · GET rooms",
                "Step 5 · POST first",
                "Step 6 · if first.status == 201",
                "Step 8 · POST second",
                "Step 10 · GET calendar",
                "Step 11 · calendar.body.items[0].id == first.body.id · one_booking",
                "Step 7 · first.status == 201 · not_booked",
                "Cleanup step 1 · DELETE cancelled",
            ]
        );
        // A value, a call, an `if` and a lone check are one line.
        let height = |part: &str| row(&harness, part).rect.height();
        for part in [
            "value slot",
            "call token",
            "if first",
            "one_booking",
            "not_booked",
        ] {
            assert!(height(part) < 34.0, "{part} is one line");
        }
        // The branches are indented under the `if`.
        let left = |part: &str| row(&harness, part).rect.left();
        assert_eq!(left("if first"), left("POST first"));
        for part in ["second", "GET calendar", "one_booking", "not_booked"] {
            assert!(left(part) > left("if first") + 30.0, "{part} is indented");
        }
        // The checks once, under the request they are written after.
        assert_eq!(
            checks_in(&harness, &row(&harness, "second")),
            ["status == 409 · refused of second"]
        );
        for part in ["rooms", "POST first", "GET calendar"] {
            assert!(
                checks_in(&harness, &row(&harness, part)).is_empty(),
                "{part}"
            );
        }
        scroll_to(&mut harness, "second");
        for words in ["then", "else"] {
            scroll_to(
                &mut harness,
                if words == "then" {
                    "second"
                } else {
                    "not_booked"
                },
            );
            let shown: Vec<String> = painted(&harness, list_area(&harness))
                .into_iter()
                .map(|(galley, _)| galley.text().to_owned())
                .filter(|text| text == words)
                .collect();
            assert_eq!(shown.len(), 1, "{words} is written once");
        }
    }
}

/// Criterion 4: on `labels` a card lists what differs from the common
/// request; the caption is one line at both sizes and its hover is the
/// common request in full.
#[test]
fn req01a_criterion_4_sends_lists_what_differs() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _) = labels(size, &labels_text());
        steps(&mut harness);
        for (name, expected) in [
            ("No token", "no header Authorization"),
            ("Weight 31 kg", "weight: 31"),
            ("Weight 0 kg", "weight: 0"),
            (
                "Unknown parcel",
                "path /parcels/{{param.missing_parcel_id}}/labels",
            ),
            ("Postcode of symbols only", "postcode: \"!@#$%\""),
            ("Postcode null", "postcode: null"),
            ("No postcode field", "no postcode"),
            ("Unknown service", "service: param.unknown_service"),
        ] {
            let row = shown(&mut harness, &format!("POST {name}"));
            assert_eq!(column(&harness, &row, "SENDS"), [expected], "{name}");
            assert_eq!(
                column(&harness, &row, "READS")[0],
                if name == "Unknown parcel" {
                    "param.missing_parcel_id"
                } else {
                    "param.parcel_id"
                }
            );
            // The caption is whole and on one line.
            assert!(
                texts(&harness, &row)
                    .iter()
                    .any(|(text, _)| text == DIFFERS),
                "{name}: the caption is whole at {size:?}"
            );
        }
        let captions: Vec<_> = painted(&harness, list_area(&harness))
            .into_iter()
            .filter(|(galley, _)| galley.text() == DIFFERS)
            .collect();
        assert!(!captions.is_empty());
        for (galley, _) in captions {
            assert!(!galley.elided && galley.rows.len() == 1);
        }
        // The hover.
        let first = shown(&mut harness, "POST No token");
        let caption = texts(&harness, &first)
            .into_iter()
            .find(|(text, _)| text == DIFFERS)
            .unwrap()
            .1;
        assert!(crate::testing::hover_until(
            &mut harness,
            |_| Some(caption),
            |harness| {
                harness
                    .query_by_label_contains("POST /parcels/{{param.parcel_id}}/labels")
                    .is_some_and(|found| {
                        let text = found.accesskit_node().value().unwrap_or_default();
                        [
                            "weight: 2",
                            "postcode: \"00-950\"",
                            "header Authorization: \"Bearer sample-token\"",
                        ]
                        .iter()
                        .all(|part| text.contains(part))
                    })
            }
        ));
    }
}

/// A group of two with more than four places, and a tie of four.
const GROUPS: &str = "scenario \"Groups\" {
  begin = start
  one = POST \"/orders\" {
    headers { \"Content-Type\": \"application/json\", Authorization: \"Bearer x\" }
    body { a: 1, b: 2, c: 3, d: 4 }
    sample 200 {}
  }
  two = POST \"/orders\" { sample 200 {} }
  tie_a = PUT \"/orders/1\" { body { kind: \"a\", size: 1 } sample 200 {} }
  tie_b = PUT \"/orders/1\" { body { kind: \"a\", size: 1 } sample 200 {} }
  tie_c = PUT \"/orders/1\" { body { kind: \"b\", size: 1 } sample 200 {} }
  tie_d = PUT \"/orders/1\" { body { kind: \"b\", size: 2 } sample 200 {} }
}
";

/// The row whose name contains `part`, scrolled into view.
fn shown(harness: &mut FolderHarness, part: &str) -> Row {
    scroll_to(harness, part);
    row(harness, part)
}

/// The place of `text` in `row`.
fn text_at(harness: &FolderHarness, row: &Row, text: &str) -> egui::Rect {
    texts(harness, row)
        .into_iter()
        .find(|(found, _)| found == text)
        .unwrap_or_else(|| panic!("{text:?} in {}", row.name))
        .1
}

/// Criterion 4, rules 4 and 5: a group of two lists its own places, and more
/// than four lines are three and a count whose hover lists the rest.
#[test]
fn req01a_criterion_4_more_than_four_lines_are_cut() {
    let (_project, mut harness, _) = opened(WIDE, "scenarios/two.afs", GROUPS);
    steps(&mut harness);
    let one = shown(&mut harness, "POST one");
    assert_eq!(
        column(&harness, &one, "SENDS"),
        ["a: 1", "b: 2", "c: 3", "+ 2 more"]
    );
    assert!(
        texts(&harness, &one)
            .iter()
            .any(|(text, _)| text == "SENDS")
    );
    let two = shown(&mut harness, "POST two");
    assert_eq!(column(&harness, &two, "SENDS"), ["no body"]);
    // A tie has no common value: each lists its own.
    for (handle, expected) in [
        ("tie_a", vec!["kind: \"a\""]),
        ("tie_d", vec!["kind: \"b\"", "size: 2"]),
    ] {
        let row = shown(&mut harness, &format!("PUT {handle}"));
        assert_eq!(column(&harness, &row, "SENDS"), expected);
    }
    let more = texts(&harness, &one)
        .into_iter()
        .find(|(text, _)| text == "+ 2 more")
        .unwrap()
        .1;
    assert!(crate::testing::hover_until(
        &mut harness,
        |_| Some(more),
        |harness| harness
            .query_by_label("d: 4\nheader Authorization: \"Bearer x\"")
            .is_some()
    ));
}

/// Criterion 5: SETS replaces SENDS for a step a later step reads, one line
/// per field and reader; a call says it on its line.
#[test]
fn req01a_criterion_5_sets_replaces_sends() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _) = rooms(size);
        steps(&mut harness);
        let listed = shown(&mut harness, "GET rooms");
        assert_eq!(
            column(&harness, &listed, "SETS"),
            [
                "body.items[0].id → first",
                "body.items[1].id → second",
                "body.items[0].id → calendar",
            ]
        );
        assert!(
            texts(&harness, &listed)
                .iter()
                .all(|(text, _)| !text.starts_with("SENDS"))
        );
        // Cleanup is later too.
        let first = shown(&mut harness, "POST first");
        assert_eq!(
            column(&harness, &first, "SETS"),
            ["status → booked", "body.id → cancelled"]
        );
        let call = shown(&mut harness, "call token");
        let call = texts(&harness, &call);
        assert!(
            call.iter()
                .any(|(text, _)| text == "SETS token · read by 5"),
            "{call:?}"
        );
        let second = shown(&mut harness, "POST second");
        assert_eq!(
            column(&harness, &second, "SENDS"),
            [
                "roomId: rooms.body.items[1].id",
                "start: slot.start",
                "end: slot.end",
                "header Authorization: \"Bearer {{token}}\"",
            ]
        );
    }
}

/// Criterion 6: Cleanup is a section after Main under its heading, numbered
/// as its board; a row that reads a Main step names it.
#[test]
fn req01a_criterion_6_cleanup_follows_main() {
    let (_project, mut harness, id) = rooms(WIDE);
    let (_, cleanup, _) = graphs(&harness, &id);
    steps(&mut harness);
    let heading = harness.get_by_label(CLEANUP).rect();
    let all = rows(&harness);
    let (after, before): (Vec<Row>, Vec<Row>) = all
        .into_iter()
        .partition(|row| row.name.starts_with("Cleanup step "));
    assert!(before.iter().all(|row| row.rect.bottom() <= heading.top()));
    assert!(after.iter().all(|row| row.rect.top() >= heading.bottom()));
    let number = sequence(&cleanup)
        .iter()
        .position(|step| *step == node(&cleanup, "cancelled").id)
        .unwrap()
        + 1;
    assert_eq!(after.len(), 1);
    assert_eq!(
        after[0].name,
        format!("Cleanup step {number} · DELETE cancelled")
    );
    let after = [shown(&mut harness, "DELETE cancelled")];
    assert_eq!(
        column(&harness, &after[0], "READS"),
        ["first.body.id", "token"]
    );
    assert_eq!(
        checks_in(&harness, &after[0]),
        ["status == 204 · gone of cancelled"]
    );
    // No heading without Cleanup's steps.
    let (_labels, mut harness, _) = labels(WIDE, &labels_text());
    steps(&mut harness);
    assert!(harness.query_by_label(CLEANUP).is_none());
}

/// Criterion 7: no result before a run; after one every row and check has
/// its own, marked as Run results marks it, a failed check with its actual
/// value.
#[test]
fn req01a_criterion_7_results_come_with_a_run() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = labels(size, &failing());
        let (main, _, _) = graphs(&harness, &id);
        steps(&mut harness);
        for row in rows(&harness) {
            for word in ["Passed", "failed", "Failed", "Not run"] {
                assert!(!row.name.contains(word), "{}", row.name);
            }
            assert!(
                checks_in(&harness, &row)
                    .iter()
                    .all(|check| !check.contains("Passed"))
            );
        }
        let marks = |harness: &FolderHarness| {
            painted(harness, list_area(harness))
                .iter()
                .filter(|(galley, _)| ["✓", "✕", "−"].contains(&galley.text()))
                .count()
        };
        assert_eq!(marks(&harness), 0);
        let result = ran(&mut harness);
        assert!(marks(&harness) > 5);
        for row in rows(&harness) {
            let expected = if row.name.contains("Weight 31 kg") {
                "1 of 4 failed"
            } else {
                "Passed"
            };
            assert!(
                row.name.ends_with(&format!(" · {expected}")),
                "{}",
                row.name
            );
            for check in checks_in(&harness, &row) {
                let check_id = &check_of(&main, &check).id;
                let step = result
                    .steps
                    .iter()
                    .find(|step| step.node_id == *check_id)
                    .unwrap();
                assert!(
                    check.contains(&format!(" · {} · ", step.status.label())),
                    "{check}"
                );
            }
        }
        let weight = row(&harness, "Weight 31 kg");
        assert!(
            checks_in(&harness, &weight).contains(
                &concat!(
                    "error.field == \"weight\" · Failed · actual \"mass\"",
                    " · weight_31_field of Weight 31 kg"
                )
                .to_owned()
            ),
            "{:?}",
            checks_in(&harness, &weight)
        );
        // The status the request received, and the failed value, are drawn.
        scroll_to(&mut harness, "Weight 31 kg");
        let shown: Vec<String> = painted(&harness, list_area(&harness))
            .iter()
            .map(|(galley, _)| galley.text().to_owned())
            .collect();
        for text in ["422", "1 of 4 failed", "— actual \"mass\"", "✕"] {
            assert!(shown.iter().any(|shown| shown == text), "{text}: {shown:?}");
        }
        assert_texts_fit(&harness, "after a run");
        // An edit that makes the run stale takes the results away.
        let active = folder(&harness).workbench.active().unwrap().to_owned();
        let source = source_of(&harness, &active).replacen("weight: 31", "weight: 32", 1);
        edit_source(&mut harness, &active, &source);
        assert!(names(&harness).iter().all(|name| !name.ends_with("Passed")));
    }
}

/// Criterion 7: a step in a branch not taken reads *Not run* with the reason
/// of `not_run.rs`.
#[test]
fn req01a_criterion_7_a_branch_not_taken_is_not_run() {
    let (_project, mut harness, _) = rooms(WIDE);
    steps(&mut harness);
    ran(&mut harness);
    assert_eq!(
        row(&harness, "not_booked").name,
        "Step 7 · first.status == 201 · not_booked · Not run · Skipped · outside path"
    );
    assert!(row(&harness, "if first").name.ends_with(" · Passed"));

    let (_project, mut harness, _) = rooms_with(WIDE, |text| {
        let refused = text.replacen("sample 201 { id: \"bk-1\"", "sample 500 { id: \"bk-1\"", 1);
        assert_ne!(refused, text);
        refused
    });
    steps(&mut harness);
    ran(&mut harness);
    for part in ["POST second", "GET calendar", "one_booking"] {
        let name = row(&harness, part).name;
        assert!(
            name.ends_with(" · Not run · Skipped · outside path"),
            "{name}"
        );
    }
    let second = shown(&mut harness, "POST second");
    assert_eq!(
        checks_in(&harness, &second),
        ["status == 409 · Not run · Skipped · outside path · refused of second"]
    );
    assert!(
        texts(&harness, &second)
            .iter()
            .any(|(text, _)| text == "Skipped · outside path")
    );
    assert!(
        row(&harness, "not_booked").name.ends_with(" · Failed"),
        "{:?}",
        names(&harness)
    );
}

/// The selected rows.
fn selected(harness: &FolderHarness) -> Vec<String> {
    rows(harness)
        .into_iter()
        .filter(|row| row.selected)
        .map(|row| row.name)
        .collect()
}

/// A component's board is as without the choice: it has no list.
#[test]
fn req01a_a_component_keeps_its_board() {
    let (_project, mut harness, _) = rooms(WIDE);
    steps(&mut harness);
    let component = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Component)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();
    app(&mut harness).review_open(&component, None);
    settle(&mut harness);
    assert!(harness.query_by_label("Diagram board").is_some());
    assert!(harness.query_by_label(STEPS).is_none());
    app(&mut harness).review_open(ROOMS, None);
    settle(&mut harness);
    assert_eq!(rows(&harness).len(), 10, "the choice holds");
}

/// Criterion 8: a row selects its step in Canvas, Run results and Code.
#[test]
fn req01a_criterion_8_a_row_selects_its_step() {
    let (_project, mut harness, id) = rooms(WIDE);
    let (main, cleanup, _) = graphs(&harness, &id);
    ran(&mut harness);
    steps(&mut harness);
    for (part, graph, handle) in [
        ("POST second", &main, "second"),
        ("if first", &main, "booked"),
        ("value slot", &main, "slot"),
        ("DELETE cancelled", &cleanup, "cancelled"),
        ("GET rooms", &main, "rooms"),
    ] {
        let name = row(&harness, part).name;
        harness.get_by_label(&name).scroll_to_me();
        settle(&mut harness);
        let rect = row(&harness, part).rect;
        // At the row's first line, beside its texts.
        click_at(&mut harness, rect.left_top() + egui::vec2(4.0, 6.0));
        assert_eq!(
            folder(&harness).selections[&id],
            [node(graph, handle).id.clone()]
        );
        assert_eq!(selected(&harness).len(), 1, "{part}");
        assert!(selected(&harness)[0].contains(part));
        assert_eq!(
            folder(&harness).cleanup(&id),
            handle == "cancelled",
            "the board that holds it"
        );
        // Run results.
        choose(&mut harness, "Run results");
        let chosen = buttons(&harness, |name| name.starts_with(&format!("{handle} · ")))
            .into_iter()
            .find(|row| row.selected);
        assert!(chosen.is_some(), "Run results select {handle}");
        choose(&mut harness, "Run results");
        // Canvas.
        canvas(&mut harness);
        assert_eq!(
            folder(&harness).selections[&id],
            [node(graph, handle).id.clone()]
        );
        assert!(
            harness.query_all_by_label("Name in code").count() > 0,
            "the Inspector shows {handle}"
        );
        steps(&mut harness);
        // Code.
        choose(&mut harness, "Code");
        assert_eq!(folder(&harness).revealed.as_deref(), Some(handle));
        choose(&mut harness, "Diagram");
    }
    // A check of a request selects as its row of Run results does.
    let check = "status == 204 · Passed · gone of cancelled";
    choose(&mut harness, check);
    assert!(folder(&harness).cleanup(&id));
    assert!(selected(&harness)[0].contains("cancelled"));
    // Escape clears it.
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    assert!(selected(&harness).is_empty());
}

/// Criterion 8, the keyboard: Tab reaches a row and Enter selects it.
#[test]
fn req01a_criterion_8_the_keyboard_selects_a_row() {
    let (_project, mut harness, id) = rooms(WIDE);
    let (main, _, _) = graphs(&harness, &id);
    steps(&mut harness);
    let name = row(&harness, "POST first").name;
    harness.get_by_label(&name).focus();
    harness.step();
    harness.key_press(egui::Key::Enter);
    settle(&mut harness);
    assert_eq!(
        folder(&harness).selections[&id],
        [node(&main, "first").id.clone()]
    );
}

/// Criterion 8, back: a step selected in Canvas, Run results or Code selects
/// its row, which is scrolled into view.
#[test]
fn req01a_criterion_8_a_step_selected_elsewhere_selects_its_row() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = labels(size, &labels_text());
        let (main, _, _) = graphs(&harness, &id);
        let step = main
            .nodes
            .iter()
            .rfind(|node| node.kind == NodeKind::Request)
            .unwrap();
        let in_view = |harness: &FolderHarness| {
            let area = list_area(harness);
            let found = rows(harness).into_iter().find(|row| row.selected).unwrap();
            assert!(found.name.contains(step.display_name()), "{}", found.name);
            assert!(
                area.intersect(found.rect).height() > found.rect.height().min(area.height()) - 60.0,
                "{:?} in {area:?}",
                found.rect
            );
        };
        // Canvas.
        app(&mut harness)
            .selections
            .insert(id.clone(), vec![step.id.clone()]);
        settle(&mut harness);
        steps(&mut harness);
        in_view(&harness);
        // Run results.
        app(&mut harness).selections.remove(&id);
        settle(&mut harness);
        choose(&mut harness, "Run simulation");
        finish_run(&mut harness);
        let first = rows(&harness)[0].name.clone();
        harness.get_by_label(&first).scroll_to_me();
        settle(&mut harness);
        let result = step_row_label(&harness, step.display_name()).unwrap();
        choose(&mut harness, &result);
        choose(&mut harness, "Run results");
        in_view(&harness);
        // Code.
        app(&mut harness).selections.remove(&id);
        choose(&mut harness, "Code");
        let text = source_of(&harness, &id);
        let at = text.find(&format!("{} = POST", step.handle)).unwrap();
        put_caret(&mut harness, &id, at);
        choose(&mut harness, "Diagram");
        in_view(&harness);
    }
}

/// Criterion 9: at both sizes nothing overlaps and no text is cut while its
/// column has room, along the whole list, before and after a run.
#[test]
fn req01a_criterion_9_texts_fit_their_columns() {
    let long = labels_text()
        .replacen(
            "@label(\"Weight 31 kg\")",
            "@label(\"A parcel of thirty-one kilograms is refused with the field and a message\")",
            1,
        )
        .replacen(
            "weight: 31,",
            "a_field_of_the_body_with_a_long_name_indeed: \"and a long value of it that goes on\", weight: 31,",
            1,
        );
    for size in [MINIMUM, WIDE] {
        for (name, (_project, mut harness, _)) in [
            ("labels", labels(size, &failing())),
            ("long names", labels(size, &long)),
            ("rooms", rooms(size)),
        ] {
            steps(&mut harness);
            for after in [false, true] {
                if after {
                    ran(&mut harness);
                }
                let context = format!("{name} at {size:?}, run {after}");
                for row in names(&harness) {
                    harness.get_by_label(&row).scroll_to_me();
                    settle(&mut harness);
                    assert_texts_fit(&harness, &context);
                    let found = rows(&harness).into_iter().find(|found| found.name == row);
                    assert!(
                        !texts(&harness, &found.unwrap()).is_empty(),
                        "{context}: the texts of {row} are inside its card"
                    );
                }
                // The cards do not overlap and stay inside the window's width.
                let all = rows(&harness);
                for pair in all.windows(2) {
                    assert!(pair[0].rect.bottom() <= pair[1].rect.top(), "{context}");
                }
                let window = harness.ctx.content_rect();
                for row in all {
                    assert!(row.rect.right() <= window.right() - 10.0, "{context}");
                }
            }
        }
    }
}

/// Criterion 9: a cut text is read in full on hover, a check's included.
#[test]
fn req01a_criterion_9_a_cut_text_has_its_hover() {
    let long = "a long value of the field that goes on and on and does not fit a column at all";
    let text = format!(
        "scenario \"Long\" {{
  begin = start
  one = POST \"/orders\" {{
    body {{ name: \"{long}\" }}
    sample 200 {{ name: \"n\" }}
  }}
  said = assert one.body.name == \"{long}\"
}}
"
    );
    let (_project, mut harness, _) = opened(MINIMUM, "scenarios/long.afs", &text);
    steps(&mut harness);
    let card = row(&harness, "POST one");
    let line = format!("name: \"{long}\"");
    let place = texts(&harness, &card)
        .into_iter()
        .find(|(text, _)| *text == line)
        .expect("the line's name is its whole text")
        .1;
    let cut: Vec<String> = painted(&harness, list_area(&harness))
        .into_iter()
        .filter(|(galley, _)| galley.elided)
        .map(|(galley, _)| galley.text().to_owned())
        .collect();
    assert!(cut.contains(&line), "{cut:?}");
    let count = harness.query_all_by_label(&line).count();
    assert!(crate::testing::hover_until(
        &mut harness,
        |_| Some(place),
        |harness| harness.query_all_by_label(&line).count() > count
    ));
    let check = format!("name == \"{long}\"");
    let name = format!("{check} · said of one");
    assert!(crate::testing::hover_until(
        &mut harness,
        |harness| Some(harness.get_by_label(&name).rect()),
        |harness| harness.query_by_label(&check).is_some()
    ));
}

/// The list is read while nothing can be edited: code with errors keeps the
/// last rows, under the board's own notice.
#[test]
fn req01a_code_with_errors_keeps_the_last_list() {
    let (_project, mut harness, id) = labels(WIDE, &labels_text());
    steps(&mut harness);
    edit_source(&mut harness, &id, "scenario \"broken\" {");
    assert_eq!(rows(&harness).len(), 16);
    let notice = harness.get_by_label("Show code").rect();
    assert!(notice.top() >= harness.get_by_label(STEPS).rect().bottom());
    assert!(notice.bottom() <= rows(&harness)[0].rect.top());
    let name = row(&harness, "POST Weight 31 kg").name;
    harness.get_by_label(&name).click();
    settle(&mut harness);
    assert_eq!(selected(&harness), [name]);
}

/// Review of the pull request: a project opened on *Steps* selects a check as
/// one opened on its board does, on the step it is folded into, so that the
/// board shown later marks it.
#[test]
fn req01a_criterion_8_a_check_selected_before_the_board_was_drawn() {
    let (project, mut harness, _) = rooms(WIDE);
    steps(&mut harness);
    let deadline = Instant::now() + Duration::from_secs(15);
    while folder(&harness).changed_at.is_some() {
        assert!(Instant::now() < deadline, "the view state was not written");
        harness.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    settle(&mut harness);
    drop(harness);

    let mut harness = crate::testing::first_open_harness(project.path(), WIDE, Some(Mode::Diagram));
    app(&mut harness).review_open(ROOMS, None);
    settle(&mut harness);
    let id = folder(&harness).workbench.active().unwrap().to_owned();
    let (main, _, _) = graphs(&harness, &id);
    assert_eq!(rows(&harness).len(), 10, "opened on the list");
    choose(&mut harness, "status == 409 · refused of second");
    let second = node(&main, "second").id.clone();
    assert_eq!(
        folder(&harness).selections[&id],
        std::slice::from_ref(&second)
    );
    canvas(&mut harness);
    assert!(folder(&harness).canvases[&id].debug_selected(&second));
}

/// Review of the pull request: a row out of sight is named and described for
/// assistive technology, and its texts are not laid out.
#[test]
fn req01a_rows_out_of_sight_are_described_and_not_drawn() {
    let (_project, mut harness, _) = labels(MINIMUM, &labels_text());
    steps(&mut harness);
    let all = rows(&harness);
    assert_eq!(all.len(), 16);
    let last = all.last().unwrap();
    assert!(last.rect.top() > harness.ctx.content_rect().bottom());
    assert!(
        !checks_in(&harness, last).is_empty(),
        "its checks are named"
    );
    assert!(texts(&harness, last).is_empty(), "its texts are not built");
    let found = harness.get_by_label(&last.name);
    let said = found.accesskit_node().description().unwrap();
    assert!(
        said.contains("reads param.parcel_id") && said.contains("sends service: null"),
        "{said}"
    );
}

/// What a frame of *Steps* costs at the most steps a scenario may have:
/// `cargo test -p apiforge-desktop --release --locked req01a_frame_budget
/// -- --ignored --nocapture`.
#[test]
#[ignore = "KTLO-31: timing, run in release as the development guide's budget step says"]
fn req01a_frame_budget() {
    let mut text = String::from("scenario \"Large\" {\n  begin = start\n");
    for step in 0..66 {
        text.push_str(&format!(
            "  r{step} = POST \"/orders\" {{ body {{ n: {step}, kind: \"a\" }} }}\n  \
             s{step} = assert r{step}.status == 200\n  t{step} = assert r{step}.body.n == {step}\n"
        ));
    }
    text.push_str("}\n");
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _) = opened(size, "scenarios/large.afs", &text);
        steps(&mut harness);
        assert_eq!(rows(&harness).len(), 66);
        let start = Instant::now();
        for _ in 0..100 {
            harness.step();
        }
        let frame = start.elapsed() / 100;
        eprintln!("{size:?}: {frame:?} a frame");
        assert!(frame < Duration::from_millis(16), "{size:?}: {frame:?}");
    }
}

/// Design review: the list at both sizes, before and after a run, its
/// hovers and the header of the board.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_req01a() {
    let shot = |harness: &mut FolderHarness, name: &str| {
        harness.run_steps(4);
        capture(harness, name, None);
    };
    for (size, at) in [(WIDE, "1600"), (MINIMUM, "960")] {
        let (_project, mut harness, _) = labels(size, &failing());
        shot(&mut harness, &format!("req01a-canvas-{at}"));
        steps(&mut harness);
        shot(&mut harness, &format!("req01a-labels-before-{at}"));
        ran(&mut harness);
        shot(&mut harness, &format!("req01a-labels-after-{at}"));

        let (_project, mut harness, _) = rooms(size);
        shot(&mut harness, &format!("req01a-rooms-canvas-{at}"));
        steps(&mut harness);
        shot(&mut harness, &format!("req01a-rooms-before-{at}"));
        ran(&mut harness);
        shot(&mut harness, &format!("req01a-rooms-after-{at}"));
        let name = row(&harness, "DELETE cancelled").name;
        choose(&mut harness, &name);
        shot(&mut harness, &format!("req01a-rooms-cleanup-{at}"));
        scroll_to(&mut harness, "POST second");
        shot(&mut harness, &format!("req01a-rooms-branch-{at}"));

        // The hovers, which a capture without the pointer does not show.
        let hovered = |harness: &mut FolderHarness, place: egui::Rect, shown: &str, name: &str| {
            assert!(crate::testing::hover_until(
                harness,
                |_| Some(place),
                |harness| harness.query_all_by_label_contains(shown).count() > 0,
            ));
            harness.run_steps(4);
            let directory = crate::testing::capture_dir();
            fs::create_dir_all(&directory).unwrap();
            let image = harness.render().unwrap();
            image.save(directory.join(format!("{name}.png"))).unwrap();
        };
        let (_project, mut harness, _) = labels(size, &labels_text());
        steps(&mut harness);
        let first = shown(&mut harness, "POST No token");
        let place = text_at(&harness, &first, DIFFERS);
        hovered(
            &mut harness,
            place,
            "The common request",
            &format!("req01a-caption-hover-{at}"),
        );
        let (_project, mut harness, _) = opened(size, "scenarios/two.afs", GROUPS);
        steps(&mut harness);
        let one = shown(&mut harness, "POST one");
        let place = text_at(&harness, &one, "+ 2 more");
        hovered(
            &mut harness,
            place,
            "d: 4\nheader",
            &format!("req01a-group-of-two-more-hover-{at}"),
        );
        scroll_to(&mut harness, "PUT tie_d");
        shot(&mut harness, &format!("req01a-tie-{at}"));
        let name = "scenarios/a-parcel-label-is-refused-without-a-token-and-over-30-kg.afs";
        let (_project, mut harness, _) = opened(size, name, &labels_text());
        shot(&mut harness, &format!("req01a-long-title-canvas-{at}"));
        steps(&mut harness);
        shot(&mut harness, &format!("req01a-long-title-steps-{at}"));
    }
}
