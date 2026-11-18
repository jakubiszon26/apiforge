//! LAY-30 (#383): a step with exactly one check writes it on its folded
//! line (`9 · status == 409`, after a run with ✓ or ✕); two or more stay a
//! count. Checks are folded by default for everyone; *Checks* with an eye
//! on the board's bar shows or folds every check card, and an arrow at the
//! end of a step's checks line shows or folds that step's checks only, a
//! choice remembered per document in `.apiforge/local/`. Opening one step's
//! checks displaces nothing (W7). On a temporary copy of the room project
//! (`fixtures/scenarios/lay/rooms/`), 1280 × 800.
use super::lay23::{board, graph, handle_of, id_of, show};
use super::*;
use crate::preferences::{Preference, Preferences};
use crate::testing::folder_harness_in;
use apiforge_core::graph::sequence;
use egui_kittest::kittest::{NodeT, Queryable};
use std::collections::BTreeMap;
use std::path::Path;

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const DOUBLE: &str = "double-booking";
const DAY: &str = "booking-day";
/// The bar's switch of every check card.
const CHECKS: &str = "Checks";

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

/// The scenario `name` of `root` open on its board at `size`, nothing
/// selected; its document id.
fn open_in(root: &Path, name: &str, size: egui::Vec2) -> (FolderHarness, String) {
    let mut harness = folder_harness_in(root, size, Mode::Diagram);
    let id = reopen(&mut harness, name);
    (harness, id)
}

/// Opens the scenario `name` in `harness` and returns its document id.
fn reopen(harness: &mut FolderHarness, name: &str) -> String {
    let path = format!("scenarios/{name}.afs");
    app(harness).review_open(&path, None);
    settle(harness);
    let id = folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("the room project has {path}"))
        .id()
        .to_owned();
    app(harness).selections.insert(id.clone(), Vec::new());
    settle(harness);
    harness.step();
    id
}

/// The room project's scenario `name`, as it is, open at 1280 × 800.
fn open(name: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let project = room_project(name, |source| source);
    let (harness, id) = open_in(project.path(), name, SIZE);
    (project, harness, id)
}

/// The checks line of `handle`'s card in the last frame, whole.
fn line(harness: &FolderHarness, id: &str, handle: &str) -> String {
    board(harness, id)
        .debug_aggregate(&id_of(harness, id, handle))
        .unwrap_or_else(|| panic!("{handle} has no checks line"))
}

fn hidden(harness: &FolderHarness, id: &str, handle: &str) -> bool {
    board(harness, id)
        .debug_hidden()
        .contains(&id_of(harness, id, handle))
}

fn marked(harness: &FolderHarness, id: &str, handle: &str) -> bool {
    board(harness, id)
        .debug_marked()
        .contains(&id_of(harness, id, handle))
}

fn selection(harness: &FolderHarness, id: &str) -> Vec<String> {
    folder(harness)
        .selections
        .get(id)
        .cloned()
        .unwrap_or_default()
}

fn has(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label(label).is_some()
}

fn pressed(harness: &FolderHarness, label: &str) -> bool {
    match harness.get_by_label(label).accesskit_node().toggled() {
        Some(egui::accesskit::Toggled::True) => true,
        Some(egui::accesskit::Toggled::False) => false,
        other => panic!("{label} is a switch, announced {other:?}"),
    }
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    settle(harness);
    harness.step();
}

/// Every step's place in the engine's order, by handle.
fn numbers(harness: &FolderHarness, id: &str) -> BTreeMap<String, usize> {
    let graph = graph(harness, id);
    sequence(&graph)
        .into_iter()
        .enumerate()
        .map(|(index, node)| (handle_of(&graph, node), index + 1))
        .collect()
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

/// `second` answered with 201: the API accepts the second booking.
fn accepted(source: String) -> String {
    let before = r#"sample 409 { error: { code: "room_unavailable", message: "The room is closed for maintenance." } }"#;
    assert!(source.contains(before), "the fixture's sample of second");
    source.replacen(before, r#"sample 201 { id: "bk-2" }"#, 1)
}

/// `double-booking` with `rooms` given one check and a gate `first` reads,
/// and `first` left with one check, which `calendar` shares.
fn variant(source: String) -> String {
    let rooms_end = "  slot = {";
    assert!(source.contains(rooms_end));
    let source = source.replacen(
        rooms_end,
        "  rooms_listed = assert rooms.status == 200\n  open_rooms = condition all {\n    nonempty rooms.body.items,\n  } with rooms.body\n  slot = {",
        1,
    );
    let room =
        "body { roomId: rooms.body.items[0].id, start: slot.start, end: slot.end }\n    sample 201";
    assert!(source.contains(room));
    let source = source.replacen(
        room,
        "body { roomId: open_rooms.value.items[0].id, start: slot.start, end: slot.end }\n    sample 201",
        1,
    );
    let other = "    not_booked = assert first.status == 201\n";
    assert!(source.contains(other));
    source.replacen(
        other,
        "    retry = GET \"/rooms\" {\n      sample 200 { items: [] }\n    }\n",
        1,
    )
}

/// Criterion 1: folded, before a run, on `double-booking`, a step with one
/// check writes it with its own name left out; two or more stay a count.
#[test]
fn lay30_criterion_1_one_check_is_written_on_the_folded_line() {
    let (_project, harness, id) = open(DOUBLE);
    assert!(board(&harness, &id).debug_collapsed(), "folded by default");
    assert_eq!(line(&harness, &id, "second"), "9 · status == 409");
    assert_eq!(
        line(&harness, &id, "calendar"),
        "11 · body.items[0].id == first.body.id"
    );
    assert_eq!(line(&harness, &id, "first"), "7, 11 · 2 checks");
}

/// Criterion 2: after a passing simulation `second` reads `9 · ✓ status ==
/// 409`; with the sample of `second` answering 201 it reads `9 · ✕ status
/// == 409`, its footer and border red, and `calendar` `11 · 1 skipped`.
#[test]
fn lay30_criterion_2_the_run_marks_the_written_check() {
    let (_project, mut harness, id) = open(DOUBLE);
    simulate(&mut harness);
    assert_eq!(line(&harness, &id, "second"), "9 · ✓ status == 409");
    assert!(!marked(&harness, &id, "second"));

    let project = room_project(DOUBLE, accepted);
    let (mut harness, id) = open_in(project.path(), DOUBLE, SIZE);
    simulate(&mut harness);
    assert_eq!(line(&harness, &id, "second"), "9 · ✕ status == 409");
    assert!(marked(&harness, &id, "second"), "second's footer is red");
    assert_eq!(
        board(&harness, &id).debug_signal(&id_of(&harness, &id, "second")),
        Some(crate::theme::DANGER),
        "second's border is red"
    );
    assert_eq!(line(&harness, &id, "calendar"), "11 · 1 skipped");
}

/// Criterion 3: a step with one check and a gate, or two checks, reads as
/// on `main`; a check of two steps is written on each, each leaving out its
/// own name.
#[test]
fn lay30_criterion_3_gates_and_counts_read_as_before() {
    let project = room_project(DOUBLE, variant);
    let (harness, id) = open_in(project.path(), DOUBLE, SIZE);
    let numbers = numbers(&harness, &id);
    assert_eq!(
        line(&harness, &id, "rooms"),
        format!("{} · 1 check · 1 gate", numbers["rooms_listed"]),
        "one check and a gate: a count"
    );
    assert!(!hidden(&harness, &id, "open_rooms"), "the gate is drawn");
    let shared = numbers["one_booking"];
    assert_eq!(
        line(&harness, &id, "calendar"),
        format!("{shared} · body.items[0].id == first.body.id")
    );
    assert_eq!(
        line(&harness, &id, "first"),
        format!("{shared} · calendar.body.items[0].id == body.id")
    );

    // Two checks: the count, as on `main`.
    let (_project, harness, id) = open(DOUBLE);
    assert_eq!(line(&harness, &id, "first"), "7, 11 · 2 checks");
}

/// The text the checks row of `handle` drew in the last frame, and where.
fn drawn_line(
    harness: &FolderHarness,
    id: &str,
    handle: &str,
    start: &str,
) -> (String, egui::Rect) {
    board(harness, id)
        .debug_texts(&id_of(harness, id, handle))
        .into_iter()
        .find(|text| text.kind == crate::canvas::TextKind::Result && text.text.starts_with(start))
        .map(|text| (text.text, text.rect))
        .unwrap_or_else(|| panic!("{handle} drew a checks line starting {start:?}"))
}

/// Criterion 4: a predicate wider than its row ends with `…` and never
/// runs under the arrow; pointing at the line shows the whole text.
#[test]
fn lay30_criterion_4_a_long_predicate_is_cut_before_the_arrow() {
    let (_project, mut harness, id) = open(DOUBLE);
    show(&mut harness, &id, "calendar", 0.56);
    let whole = line(&harness, &id, "calendar");
    let (text, rect) = drawn_line(&harness, &id, "calendar", "11 · ");
    assert!(text.ends_with('…'), "the row cuts {text:?}");
    assert!(
        whole.starts_with(text.trim_end_matches('…')),
        "{text:?} is the start of {whole:?}"
    );
    let arrow = harness.get_by_label("Expand checks of calendar").rect();
    assert!(
        rect.right() <= arrow.left(),
        "the text {rect:?} ends before the arrow {arrow:?}"
    );
    let row = harness.get_by_label("Checks of calendar").rect();
    let shown = crate::testing::hover_until(
        &mut harness,
        move |_| {
            Some(egui::Rect::from_center_size(
                row.left_center() + egui::vec2(20.0, 0.0),
                egui::vec2(4.0, 4.0),
            ))
        },
        |harness| harness.query_by_label(&whole).is_some(),
    );
    assert!(shown, "pointing at the line shows it whole");
}

/// Whether `rect` overlaps a node named in `names`, other than itself.
fn overlaps(harness: &FolderHarness, rect: egui::Rect, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter_map(|name| harness.query_by_label(name).map(|node| (name, node.rect())))
        .filter(|(_, other)| other.shrink(0.5).intersects(rect.shrink(0.5)))
        .map(|(name, _)| (*name).to_owned())
        .collect()
}

/// Criterion 5: the bar holds *Checks*, pressed while check cards are
/// drawn, inside the window at 960 × 540 and 1280 × 800 and over no
/// neighbour; the view menu has no *Collapse checks*. One click draws every
/// check card, a second folds them; the camera stays; the state survives a
/// restart; a new installation and an existing one both start folded.
#[test]
fn lay30_criterion_5_the_bar_switches_every_check() {
    for size in [crate::testing::MINIMUM, SIZE] {
        let project = room_project(DOUBLE, |source| source);
        let (harness, _id) = open_in(project.path(), DOUBLE, size);
        let bar = harness.get_by_label(CHECKS).rect();
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
        assert!(window.contains_rect(bar), "{size:?}: {bar:?} is inside");
        let neighbours = overlaps(
            &harness,
            bar,
            &[
                "Add step",
                "Tidy up",
                "Board view",
                "Main",
                "Cleanup",
                "Run simulation",
            ],
        );
        assert!(
            neighbours.is_empty(),
            "{size:?}: Checks covers {neighbours:?}"
        );
        assert!(!pressed(&harness, CHECKS), "folded: not pressed");
    }

    let (_project, mut harness, id) = open(DOUBLE);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    app(&mut harness).preferences = Preferences::at(Some(path.clone()));
    harness.get_by_label("Board view").click();
    settle(&mut harness);
    assert!(
        !has(&harness, "Collapse checks"),
        "the view menu no longer has it"
    );
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    harness.step();

    let camera = board(&harness, &id).debug_camera();
    click(&mut harness, CHECKS);
    assert!(
        pressed(&harness, CHECKS),
        "pressed while check cards are drawn"
    );
    assert!(
        board(&harness, &id).debug_hidden().is_empty(),
        "every check is drawn"
    );
    assert_eq!(
        board(&harness, &id).debug_camera(),
        camera,
        "the camera stays"
    );
    assert!(
        !Preferences::at(Some(path.clone())).checks_collapsed,
        "remembered"
    );
    assert!(
        !app(&mut harness).new_canvas().debug_collapsed(),
        "a restart draws them"
    );
    click(&mut harness, CHECKS);
    assert!(!pressed(&harness, CHECKS));
    for check in ["refused", "one_booking", "not_booked"] {
        assert!(hidden(&harness, &id, check), "{check} folds again");
    }
    assert_eq!(
        board(&harness, &id).debug_camera(),
        camera,
        "the camera stays"
    );
    assert!(Preferences::at(Some(path.clone())).checks_collapsed);

    // A new installation, and an existing one whose file was written before
    // LAY-30, both start folded.
    let fresh = directory.path().join("new.json");
    assert!(Preferences::at(Some(fresh)).checks_collapsed);
    let earlier = directory.path().join("earlier.json");
    fs::write(
        &earlier,
        br#"{"version": 1, "wireStyle": "orthogonal", "bridges": true, "codePanel": false, "compactCards": true, "wires": "chips"}"#,
    )
    .unwrap();
    assert!(Preferences::at(Some(earlier.clone())).checks_collapsed);
    let written = Preferences::at(Some(earlier.clone()));
    written.save(Preference::ChecksCollapsed);
    assert!(Preferences::at(Some(earlier)).checks_collapsed);
}

/// Criterion 6: the line ends with *Expand checks of second* / *Collapse
/// checks of second*: one click shows or folds that step's checks only, in
/// either state of the board, and selects nothing; a click on the rest of
/// the line opens the Inspector's list, which offers the same switch in
/// both directions.
#[test]
fn lay30_criterion_6_a_step_shows_or_folds_its_own_checks() {
    let (_project, mut harness, id) = open(DOUBLE);
    show(&mut harness, &id, "second", 1.0);
    assert!(hidden(&harness, &id, "refused"));
    click(&mut harness, "Expand checks of second");
    assert!(!hidden(&harness, &id, "refused"), "second's check is shown");
    assert!(
        hidden(&harness, &id, "one_booking"),
        "another step's stays folded"
    );
    assert!(hidden(&harness, &id, "not_booked"));
    assert!(
        selection(&harness, &id).is_empty(),
        "the arrow selects nothing"
    );
    assert_eq!(
        line(&harness, &id, "second"),
        "1 check",
        "shown: no predicate"
    );
    click(&mut harness, "Collapse checks of second");
    assert!(hidden(&harness, &id, "refused"));
    assert!(selection(&harness, &id).is_empty());

    // Every check drawn: the arrow folds this step's only.
    click(&mut harness, CHECKS);
    show(&mut harness, &id, "second", 1.0);
    click(&mut harness, "Collapse checks of second");
    assert!(hidden(&harness, &id, "refused"));
    assert!(!hidden(&harness, &id, "one_booking"));
    assert!(!hidden(&harness, &id, "not_booked"));
    assert!(selection(&harness, &id).is_empty());
    assert_eq!(line(&harness, &id, "second"), "9 · status == 409");

    // The rest of the line opens the Inspector's list, with the switch.
    click(&mut harness, "Checks of second");
    assert_eq!(
        selection(&harness, &id),
        vec![id_of(&harness, &id, "second")]
    );
    assert!(has(&harness, "Check refused"), "the list is open");
    assert!(pressed(&harness, "Collapse checks here"));
    click(&mut harness, "Collapse checks here");
    assert!(
        !hidden(&harness, &id, "refused"),
        "shown again from the list"
    );

    // And with the board folded, the other direction.
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    click(&mut harness, CHECKS);
    show(&mut harness, &id, "second", 1.0);
    click(&mut harness, "Checks of second");
    assert!(!pressed(&harness, "Expand checks here"));
    click(&mut harness, "Expand checks here");
    assert!(!hidden(&harness, &id, "refused"));
    assert!(pressed(&harness, "Expand checks here"));
}

/// Closes the window the way the application closes it.
fn close(mut harness: FolderHarness) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !app(&mut harness).prepare_close() {
        assert!(Instant::now() < deadline, "close did not finish");
        harness.step();
    }
}

/// Every file of the project outside `.apiforge/local/`, with its bytes.
fn tracked(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if name == ".apiforge/local" || name == ".git" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                into.insert(name, fs::read(&path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

fn git(root: &Path, arguments: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(arguments)
        .current_dir(root)
        .output()
        .ok()?;
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Criterion 7: a step's own choice survives closing the document and the
/// application; afterwards `git status` in the project is clean and the
/// `.afs` and `.apiforge/diagram.json` are byte for byte the same. The
/// board's switch forgets the single choices.
#[test]
fn lay30_criterion_7_a_step_choice_is_kept_on_this_computer() {
    let project = room_project(DOUBLE, |source| source);
    let root = project.path();
    fs::write(root.join(".gitignore"), "/.apiforge/local/\n").unwrap();
    let (mut harness, id) = open_in(root, DOUBLE, SIZE);
    // Laid out once, so that the board has its diagram file.
    choose(&mut harness, "Tidy up");
    settle(&mut harness);
    choose(&mut harness, "Save all");
    settle(&mut harness);
    let committed = git(root, &["init", "-q"])
        .and_then(|_| git(root, &["add", "-A"]))
        .and_then(|_| git(root, &["commit", "-q", "-m", "rooms"]))
        .is_some();
    let before = tracked(root);
    assert!(
        before.contains_key(".apiforge/diagram.json"),
        "the board was saved"
    );

    show(&mut harness, &id, "second", 1.0);
    click(&mut harness, "Expand checks of second");
    assert!(!hidden(&harness, &id, "refused"));
    assert!(
        fs::read_dir(root.join(".apiforge/local"))
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| fs::read_to_string(entry.path()).is_ok_and(|text| text.contains("second"))),
        "the choice is kept in .apiforge/local/"
    );

    // Closing the document and opening it again.
    click(&mut harness, &format!("Close {DOUBLE}.afs"));
    let id = reopen(&mut harness, DOUBLE);
    assert!(
        !hidden(&harness, &id, "refused"),
        "kept across the document"
    );
    close(harness);

    // And across the application.
    let (mut harness, id) = open_in(root, DOUBLE, SIZE);
    assert!(!hidden(&harness, &id, "refused"), "kept across a restart");
    assert_eq!(tracked(root), before, "no project file changed");
    if committed {
        assert_eq!(git(root, &["status", "--porcelain"]).unwrap(), "");
    }

    // The board's switch forgets the step's own choice, here and on disk.
    click(&mut harness, CHECKS);
    click(&mut harness, CHECKS);
    assert!(hidden(&harness, &id, "refused"), "the switch forgot it");
    close(harness);
    let (harness, id) = open_in(root, DOUBLE, SIZE);
    assert!(
        hidden(&harness, &id, "refused"),
        "forgotten after a restart too"
    );
}

/// What the last frame drew besides the cards, of every card but `except`:
/// each chip and count with its place on the board and its text.
fn chips_of_others(harness: &FolderHarness, id: &str, except: &[&str]) -> Vec<String> {
    let graph = graph(harness, id);
    let canvas = board(harness, id);
    let skip = |node: &str| except.contains(&handle_of(&graph, node).as_str());
    let tags = canvas
        .debug_tags()
        .iter()
        .filter(|tag| !skip(&tag.tag.reader))
        .map(|tag| {
            format!(
                "chip {} {} {:?} {:?}",
                handle_of(&graph, &tag.tag.reader),
                tag.tag.statement,
                tag.shown,
                tag.world
            )
        });
    let counts = canvas
        .debug_counts()
        .iter()
        .filter(|count| !skip(&count.count.source))
        .map(|count| format!("count {} {:?}", count.count.label, count.world));
    tags.chain(counts).collect()
}

/// Whether the segment `a`–`b` has a piece inside `rect`.
fn crosses(rect: egui::Rect, a: egui::Pos2, b: egui::Pos2) -> bool {
    let steps = 64;
    (0..=steps).any(|step| rect.contains(a.lerp(b, step as f32 / steps as f32)))
}

/// Criterion 8 (W7): on `booking-day` at 76 %, the arrow of `hold`, in a
/// middle row, opened: its check card overlaps no card; every chip and
/// count of every other card has the rectangle and the text it had before;
/// no line at rest runs through a chip. Folding it again gives the board
/// of before.
#[test]
fn lay30_criterion_8_opening_one_step_displaces_nothing() {
    let (_project, mut harness, id) = open(DAY);
    show(&mut harness, &id, "hold", 0.76);
    let cards_before = board(&harness, &id).debug_cards().to_vec();
    let chips_before = chips_of_others(&harness, &id, &["hold", "held"]);
    let all_before = chips_of_others(&harness, &id, &[]);
    assert!(!chips_before.is_empty());

    click(&mut harness, "Expand checks of hold");
    assert!(!hidden(&harness, &id, "held"), "held hangs under hold");
    assert!(selection(&harness, &id).is_empty());
    let held = id_of(&harness, &id, "held");
    let cards = board(&harness, &id).debug_cards().to_vec();
    let held_rect = cards
        .iter()
        .find(|(card, _)| *card == held)
        .map(|(_, rect)| *rect)
        .expect("held is drawn");
    for (card, rect) in &cards {
        if *card != held {
            assert!(
                !rect.shrink(0.5).intersects(held_rect),
                "held covers {}",
                handle_of(&graph(&harness, &id), card)
            );
        }
    }
    for (card, rect) in &cards_before {
        assert!(
            cards.contains(&(card.clone(), *rect)),
            "{} kept its place",
            handle_of(&graph(&harness, &id), card)
        );
    }
    assert_eq!(
        chips_of_others(&harness, &id, &["hold", "held"]),
        chips_before,
        "every other chip and count as before"
    );
    let canvas = board(&harness, &id);
    let boxes: Vec<egui::Rect> = canvas
        .debug_tags()
        .iter()
        .map(|tag| tag.world)
        .chain(canvas.debug_counts().iter().map(|count| count.world))
        .collect();
    for wire in canvas.debug_wires().iter().filter(|wire| !wire.revealed) {
        for pair in wire.points.windows(2) {
            for chip in &boxes {
                assert!(
                    !crosses(chip.shrink(1.0), pair[0], pair[1]),
                    "the line {} runs through the chip at {chip:?}",
                    wire.id
                );
            }
        }
    }

    click(&mut harness, "Collapse checks of hold");
    assert!(hidden(&harness, &id, "held"));
    assert_eq!(board(&harness, &id).debug_cards().to_vec(), cards_before);
    assert_eq!(chips_of_others(&harness, &id, &[]), all_before);
}

/// Criterion 9: a check folded by its step's switch that fails turns the
/// step's line and border red; a gate whose result is read later stays
/// drawn.
#[test]
fn lay30_criterion_9_a_folded_failure_is_red_and_a_gate_stays() {
    let project = room_project(DOUBLE, accepted);
    let (mut harness, id) = open_in(project.path(), DOUBLE, SIZE);
    click(&mut harness, CHECKS);
    show(&mut harness, &id, "second", 1.0);
    click(&mut harness, "Collapse checks of second");
    assert!(hidden(&harness, &id, "refused"));
    simulate(&mut harness);
    assert_eq!(line(&harness, &id, "second"), "9 · ✕ status == 409");
    assert!(marked(&harness, &id, "second"));
    assert_eq!(
        board(&harness, &id).debug_signal(&id_of(&harness, &id, "second")),
        Some(crate::theme::DANGER)
    );

    let project = room_project(DOUBLE, variant);
    let (mut harness, id) = open_in(project.path(), DOUBLE, SIZE);
    click(&mut harness, CHECKS);
    show(&mut harness, &id, "rooms", 1.0);
    click(&mut harness, "Collapse checks of rooms");
    assert!(hidden(&harness, &id, "rooms_listed"), "the check folds");
    assert!(!hidden(&harness, &id, "open_rooms"), "the gate stays drawn");
}

/// The captures of the pull request, at 1280 × 800 on the room project:
/// at rest, passed, failed and one step opened.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay30_checks_line() {
    let (_project, mut harness, id) = open(DOUBLE);
    choose(&mut harness, "Fit the view");
    harness.step();
    show(&mut harness, &id, "second", 0.56);
    crate::testing::capture(&mut harness, "lay30-line-at-rest", None);
    simulate(&mut harness);
    crate::testing::capture(&mut harness, "lay30-line-passed", None);
    click(&mut harness, "Expand checks of second");
    crate::testing::capture(&mut harness, "lay30-step-open", None);

    let project = room_project(DOUBLE, accepted);
    let (mut harness, id) = open_in(project.path(), DOUBLE, SIZE);
    choose(&mut harness, "Fit the view");
    harness.step();
    show(&mut harness, &id, "second", 0.56);
    simulate(&mut harness);
    crate::testing::capture(&mut harness, "lay30-line-failed", None);
}
