//! KTLO-13: the code editor fills its panel without a bordered box, a long
//! line scrolls under line numbers that stay put, and keyboard focus shows as
//! a line in the gutter. Driven through named widgets (ADR 0013); what the
//! editor paints is read from the frame's shapes. See docs/tickets/KTLO-13.md.
//! The KTLO-18 tests at the end expand the Problems section over the code
//! (docs/tickets/KTLO-18.md); they share this file's `editor` and
//! `problems_top`. The KTLO-32 test after them reads the board's painted
//! text the same way (docs/tickets/KTLO-32.md). `mod ux03` at the end turns
//! the focus line grey and holds the error marks back while typing
//! (docs/tickets/UX-03.md). `mod ux20` after it gives undo on invalid source
//! a plain sentence and keeps the completion popup inside the code (#228).
use super::project_interaction_tests::{app, replace_text};
use super::*;
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_within_window, folder, folder_harness, settle,
};
use egui::epaint::{ClippedShape, Shape};
use egui_kittest::kittest::Queryable;

/// The font size the folder window's code view uses.
const CODE_FONT: f32 = 13.0;

/// The demo shop with a scenario description far wider than either window, so
/// its line in the source scrolls horizontally.
fn long_line_project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let mut demo = apiforge_core::demos::shop_project();
    demo.scenarios[0].description = "Check the promotion, the final price and an extra \
        field. The demo data can be run against the local demo API, which answers on the \
        address configured in the project environment, and reports every step it ran."
        .into();
    apiforge_project::prepare_project(directory.path(), &demo).unwrap();
    directory
}

fn show(harness: &mut FolderHarness, mode: &str) {
    harness.get_by_label(mode).click();
    settle(harness);
}

/// The text area of the active document: the text edit itself.
fn editor(harness: &FolderHarness) -> egui::Rect {
    harness.get_by_label_contains("Contents of").rect()
}

/// The top of the Problems bar below the code: the panel-coloured fill under
/// its caption. Only the code panel's own margin (6) lies between the bar and
/// the bottom of the code.
fn problems_top(harness: &FolderHarness) -> f32 {
    let caption = harness.get_by_label("PROBLEMS").rect();
    shapes(harness)
        .into_iter()
        .filter_map(|(_, shape)| match shape {
            Shape::Rect(rect) if rect.fill == theme::PANEL && rect.rect.contains_rect(caption) => {
                Some(rect.rect.top())
            }
            _ => None,
        })
        .reduce(f32::max)
        .expect("the Problems bar has a fill of its own")
}

fn shapes(harness: &FolderHarness) -> Vec<(egui::Rect, Shape)> {
    fn flatten(clip: egui::Rect, shape: &Shape, into: &mut Vec<(egui::Rect, Shape)>) {
        match shape {
            Shape::Vec(shapes) => {
                for shape in shapes {
                    flatten(clip, shape, into);
                }
            }
            shape => into.push((clip, shape.clone())),
        }
    }
    let mut all = Vec::new();
    for ClippedShape { clip_rect, shape } in &harness.output().shapes {
        flatten(*clip_rect, shape, &mut all);
    }
    all
}

fn near(a: egui::Rect, b: egui::Rect, tolerance: f32) -> bool {
    (a.min - b.min).length() <= tolerance && (a.max - b.max).length() <= tolerance
}

/// Line numbers painted beside the code: monospace numerals at the code's
/// size, between the editor's top and the Problems bar. Keyed by the number,
/// with where each was painted and the clip it was painted under.
fn line_numbers(harness: &FolderHarness) -> Vec<(String, egui::Rect, egui::Rect)> {
    let editor = editor(harness);
    let bottom = problems_top(harness);
    shapes(harness)
        .into_iter()
        .filter_map(|(clip, shape)| match shape {
            Shape::Text(text) => {
                let job = &text.galley.job;
                let numeral = !job.text.is_empty() && job.text.chars().all(|c| c.is_ascii_digit());
                let monospace = job.sections.first().is_some_and(|section| {
                    section.format.font_id == egui::FontId::monospace(CODE_FONT)
                });
                let rect = text.visual_bounding_rect();
                (numeral && monospace && rect.top() >= editor.top() && rect.bottom() <= bottom)
                    .then(|| (job.text.clone(), rect, clip))
            }
            _ => None,
        })
        .collect()
}

/// Whether the focus line is painted in the gutter: a grey line (UX-03, the
/// accent before it) left of the text area and taller than a dot.
fn focus_line(harness: &FolderHarness) -> bool {
    line_in_gutter(harness, theme::MUTED)
}

/// Whether a line of `colour` is painted in the gutter: left of the text area
/// and taller than a dot.
fn line_in_gutter(harness: &FolderHarness, colour: egui::Color32) -> bool {
    let editor = editor(harness);
    let in_gutter = |rect: egui::Rect| {
        rect.right() <= editor.left() + 1.0
            && rect.left() >= editor.left() - 80.0
            && rect.height() >= 20.0
    };
    shapes(harness).into_iter().any(|(_, shape)| match shape {
        Shape::LineSegment { points, stroke } => {
            stroke.color == colour
                && stroke.width > 0.0
                && in_gutter(egui::Rect::from_two_pos(points[0], points[1]))
        }
        Shape::Rect(rect) => rect.fill == colour && in_gutter(rect.rect),
        _ => false,
    })
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

fn active_source(harness: &FolderHarness) -> String {
    let app = folder(harness);
    let active = app.workbench.active().unwrap();
    app.session
        .as_ref()
        .unwrap()
        .document(active)
        .unwrap()
        .text()
        .to_owned()
}

/// Criterion 1: no frame stroke or fill of the editor's own, and the text area
/// reaches the bottom of the panel, in Code and Split at both sizes.
#[test]
fn the_editor_paints_no_box_and_reaches_the_bottom_of_its_panel() {
    for size in [MINIMUM, WIDE] {
        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), size);
        for mode in ["Code", "Split"] {
            show(&mut harness, mode);
            let editor = editor(&harness);
            let boxes: Vec<_> = shapes(&harness)
                .into_iter()
                .filter_map(|(_, shape)| match shape {
                    Shape::Rect(rect)
                        if near(rect.rect, editor, 3.0)
                            && (rect.fill.a() > 0
                                || (rect.stroke.width > 0.0 && rect.stroke.color.a() > 0)) =>
                    {
                        Some(rect)
                    }
                    _ => None,
                })
                .collect();
            assert!(
                boxes.is_empty(),
                "{mode} at {size:?}: the editor at {editor:?} paints a box of its own: {boxes:?}"
            );
            let bottom = problems_top(&harness);
            assert!(
                editor.bottom() >= bottom - 7.0,
                "{mode} at {size:?}: the text area ends at {} but the panel at {bottom}",
                editor.bottom()
            );
        }
    }
}

/// Criterion 2: a line longer than the panel scrolls horizontally; with the
/// caret at its end the line numbers are still at the panel's left edge.
#[test]
fn a_long_line_scrolls_under_line_numbers_that_stay_at_the_left_edge() {
    for size in [MINIMUM, WIDE] {
        let project = long_line_project();
        let mut harness = folder_harness(project.path(), size);
        show(&mut harness, "Code");
        let source = active_source(&harness);
        let (index, long) = source
            .lines()
            .enumerate()
            .max_by_key(|(_, line)| line.len())
            .unwrap();
        let end_of_long = source
            .lines()
            .take(index)
            .map(|line| line.len() + 1)
            .sum::<usize>()
            + long.len();
        let before = editor(&harness);
        let numbers_before = line_numbers(&harness);
        assert!(
            numbers_before.iter().any(|(number, _, _)| number == "1"),
            "at {size:?}: no line numbers beside the code: {numbers_before:?}"
        );
        let row = harness
            .ctx
            .fonts_mut(|fonts| fonts.row_height(&egui::FontId::monospace(CODE_FONT)));
        let at = before.left_top() + egui::vec2(120.0, 2.0 + row * (index as f32 + 0.5));
        click_at(&mut harness, at);
        harness.key_press(egui::Key::End);
        settle(&mut harness);

        let caret = folder(&harness)
            .workbench
            .active_tab()
            .unwrap()
            .selection
            .caret;
        assert_eq!(
            caret,
            end_of_long,
            "at {size:?}: the caret is not at the end of line {}",
            index + 1
        );
        let after = editor(&harness);
        assert!(
            after.left() < before.left() - 100.0,
            "at {size:?}: the long line did not scroll: the text area moved from {before:?} to {after:?}"
        );
        let numbers_after = line_numbers(&harness);
        for (number, was, _) in &numbers_before {
            let (_, now, clip) = numbers_after
                .iter()
                .find(|(other, _, _)| other == number)
                .unwrap_or_else(|| panic!("at {size:?}: line number {number} is gone"));
            assert!(
                (now.left() - was.left()).abs() <= 1.0,
                "at {size:?}: line number {number} moved from {was:?} to {now:?}"
            );
            assert!(
                clip.contains_rect(*now),
                "at {size:?}: line number {number} at {now:?} is outside the panel {clip:?}"
            );
        }
    }
}

/// Criterion 3: a focus line is visible in the gutter while the editor has
/// keyboard focus, and not otherwise.
#[test]
fn a_focus_line_shows_in_the_gutter_only_while_the_editor_has_focus() {
    let project = crate::testing::shop_project();
    let mut harness = folder_harness(project.path(), MINIMUM);
    show(&mut harness, "Code");
    let rect = editor(&harness);
    click_at(&mut harness, rect.left_top() + egui::vec2(60.0, 10.0));
    assert!(
        harness.get_by_label_contains("Contents of").is_focused(),
        "a click in the editor gives it keyboard focus"
    );
    assert!(
        focus_line(&harness),
        "no focus line in the gutter while the editor has focus"
    );

    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    assert!(
        !harness.get_by_label_contains("Contents of").is_focused(),
        "Escape leaves the editor"
    );
    assert!(
        !focus_line(&harness),
        "a focus line stays in the gutter without focus"
    );
}

/// The gutter covers the left of the panel, so a caret scrolled back to the
/// start of a long line must bring the whole line start out from under it.
#[test]
fn home_on_a_scrolled_long_line_shows_its_start_beside_the_gutter() {
    let project = long_line_project();
    let mut harness = folder_harness(project.path(), MINIMUM);
    show(&mut harness, "Code");
    let source = active_source(&harness);
    let index = source
        .lines()
        .enumerate()
        .max_by_key(|(_, line)| line.len())
        .unwrap()
        .0;
    let before = editor(&harness);
    let row = harness
        .ctx
        .fonts_mut(|fonts| fonts.row_height(&egui::FontId::monospace(CODE_FONT)));
    click_at(
        &mut harness,
        before.left_top() + egui::vec2(120.0, 2.0 + row * (index as f32 + 0.5)),
    );
    harness.key_press(egui::Key::End);
    settle(&mut harness);
    assert!(
        editor(&harness).left() < before.left() - 100.0,
        "the line scrolled"
    );
    harness.key_press(egui::Key::Home);
    settle(&mut harness);
    let after = editor(&harness);
    assert!(
        (after.left() - before.left()).abs() <= 1.0,
        "the text area is at {after:?} after Home, not back at {before:?}"
    );
}

// ---- KTLO-18: the Problems section expands over the code ----

/// The accessible name of the active document's editor, which is its
/// registry path, as the cross-package acceptance tests derive it.
fn editor_label(harness: &FolderHarness) -> String {
    let app = folder(harness);
    let id = app.workbench.active().expect("an open document");
    format!(
        "Contents of {}",
        app.session
            .as_ref()
            .unwrap()
            .document(id)
            .unwrap()
            .entry()
            .path
            .as_str()
    )
}

/// The strip of code left visible above the Problems bar.
fn code_strip(harness: &FolderHarness) -> f32 {
    problems_top(harness) - editor(harness).top()
}

/// The height the view state holds for the Problems section.
fn remembered(harness: &FolderHarness) -> f32 {
    folder(harness).workbench.panels.diagnostics
}

fn row_height(harness: &mut FolderHarness) -> f32 {
    harness
        .ctx
        .fonts_mut(|fonts| fonts.row_height(&egui::FontId::monospace(CODE_FONT)))
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    settle(harness);
}

/// Drags the Problems resize handle by `dy`, as KTLO-05c's test drags the
/// library's edge.
fn drag_problems_edge(harness: &mut FolderHarness, dy: f32) {
    let start = harness.get_by_label("Resize problems").rect().center();
    let end = start + egui::vec2(0.0, dy);
    harness.hover_at(start);
    harness.step();
    harness.drag_at(start);
    harness.step();
    for step in 1..=6 {
        harness.hover_at(start + (end - start) * (step as f32 / 6.0));
        harness.step();
    }
    harness.drop_at(end);
    settle(harness);
}

/// KTLO-18 criterion 1: the caption row holds *Expand problems*; clicking it
/// gives the section the panel's height less a strip of code, and the editor
/// stays visible above it. The strip is the rounded value the ticket names:
/// four whole lines plus the code panel's margin, never more than 96 px.
#[test]
fn expanding_the_problems_section_keeps_a_strip_of_code_above_it() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let mut harness = folder_harness(project.path(), size);
        let row = row_height(&mut harness);
        let before = code_strip(&harness);

        click(&mut harness, "Expand problems");
        let after = code_strip(&harness);
        assert!(
            after < before,
            "at {size:?}: the Problems section did not grow: the code strip went from {before} to {after}"
        );
        assert!(
            after >= row * 4.0 && after <= 96.0 + 1.0,
            "at {size:?}: the code strip is {after} px, not four whole lines within 96 px (one line is {row} px)"
        );
        let editor = editor(&harness);
        assert!(
            editor.height() > 0.0,
            "at {size:?}: the editor is gone while the problems are expanded"
        );
        // The editor's own rect is all of its text laid out, which the demo's
        // long lines take past the right of a 960-wide window, expanded or not.
        // What the strip shows is what the code panel paints: the first lines,
        // numbered, whole, and inside the window.
        let window = harness.ctx.content_rect();
        let shown: Vec<String> = line_numbers(&harness)
            .into_iter()
            .filter(|(_, rect, clip)| rect.bottom() <= clip.bottom() && window.contains_rect(*rect))
            .map(|(number, ..)| number)
            .collect();
        assert!(
            ["1", "2", "3", "4"]
                .iter()
                .all(|line| shown.iter().any(|number| number == line)),
            "at {size:?}: the strip shows lines {shown:?}, not the first four"
        );
        assert_within_window(&harness, &["PROBLEMS", "Restore problems"]);
    }
}

/// KTLO-18 criterion 2: the button is then named *Restore problems*, and
/// clicking it returns the height the section had, which is also the height
/// the view state keeps.
#[test]
fn restoring_the_problems_section_returns_the_height_it_had() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    let strip = code_strip(&harness);
    let height = remembered(&harness);

    click(&mut harness, "Expand problems");
    assert!(
        harness.query_by_label("Expand problems").is_none(),
        "the button is still named *Expand problems* while expanded"
    );
    click(&mut harness, "Restore problems");
    assert!(
        harness.query_by_label("Restore problems").is_none(),
        "the button is still named *Restore problems* after restoring"
    );
    assert!(
        (code_strip(&harness) - strip).abs() <= 1.0,
        "the code strip is {} px, not back at {strip} px",
        code_strip(&harness)
    );
    assert!(
        (remembered(&harness) - height).abs() <= 1.0,
        "the view state holds {} px, not the {height} px the section had",
        remembered(&harness)
    );
}

/// KTLO-18 criterion 2, second half: an expanded section is never written to
/// the view state, so a window closed while expanded reopens as it was.
#[test]
fn the_expanded_height_is_never_written_to_the_view_state() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    let height = remembered(&harness);
    click(&mut harness, "Expand problems");
    assert!(
        (remembered(&harness) - height).abs() <= 1.0,
        "the expanded height {} replaced the remembered {height}",
        remembered(&harness)
    );
}

/// KTLO-18 criterion 4: the resize handle still works while expanded, and the
/// next *Restore problems* returns the height from before the expansion, not
/// the dragged one.
#[test]
fn dragging_while_expanded_works_and_restore_returns_the_earlier_height() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    let strip = code_strip(&harness);

    click(&mut harness, "Expand problems");
    let expanded = code_strip(&harness);
    drag_problems_edge(&mut harness, 40.0);
    let dragged = code_strip(&harness);
    assert!(
        (dragged - expanded).abs() > 4.0,
        "the handle did not move the expanded section: {expanded} then {dragged}"
    );
    click(&mut harness, "Restore problems");
    assert!(
        (code_strip(&harness) - strip).abs() <= 1.0,
        "after a drag while expanded, restoring gave {} px instead of {strip} px",
        code_strip(&harness)
    );
}

/// KTLO-18 criterion 5: decision 2 is "Code and Split", and the code panel is
/// one widget, so the button behaves the same on the split's code side.
#[test]
fn the_problems_section_expands_in_split_too() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let mut harness = folder_harness(project.path(), size);
        show(&mut harness, "Split");
        let row = row_height(&mut harness);
        let before = code_strip(&harness);

        click(&mut harness, "Expand problems");
        let after = code_strip(&harness);
        assert!(
            after < before && after >= row * 4.0 && after <= 96.0 + 1.0,
            "Split at {size:?}: the code strip is {after} px, not four whole lines within 96 px (from {before})"
        );
        click(&mut harness, "Restore problems");
        assert!(
            (code_strip(&harness) - before).abs() <= 1.0,
            "Split at {size:?}: restoring gave {} px instead of {before} px",
            code_strip(&harness)
        );
    }
}

/// KTLO-18 criterion 3: a problem row still reveals its line in the editor
/// while the section is expanded.
#[test]
fn clicking_a_problem_while_expanded_still_reveals_its_line() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    // Three lines that do not parse, so the list has rows to click.
    let label = editor_label(&harness);
    replace_text(&mut harness, &label, "scenario \"Main\" {\n  ???\n}\n");
    let id = folder(&harness).workbench.active().unwrap().to_owned();
    let diagnostics = folder(&harness).snapshot(&id).unwrap().diagnostics.len();
    assert!(diagnostics > 0, "the broken source produced no problems");

    let range = folder(&harness).snapshot(&id).unwrap().diagnostics[0]
        .range
        .clone();

    click(&mut harness, "Expand problems");
    let row = harness
        .get_by_label_contains("Line 2, column")
        .rect()
        .center();
    click_at(&mut harness, row);
    settle(&mut harness);
    let selection = folder(&harness)
        .workbench
        .active_tab()
        .unwrap()
        .selection
        .clone();
    assert_eq!(
        (selection.anchor, selection.caret),
        (range.start, range.end),
        "clicking the problem did not select its range"
    );
    assert!(
        harness
            .ctx
            .memory(|memory| memory.has_focus(editor_id(&id))),
        "clicking the problem did not focus the editor"
    );
}

/// KTLO-32: a project whose one scenario holds only a call to `notes@1`, a
/// component whose only input `note` is a string declared with neither
/// `@optional`/`@required` nor a default.
fn undeclared_input_project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let mut demo = apiforge_core::demos::empty_project();
    demo.components.push(
        serde_json::from_value(serde_json::json!({
            "id": "notes", "revision": 1, "handle": "notes", "name": "Notes",
            "description": "", "inputs": [{ "name": "note", "type": "string" }],
            "graph": {
                "nodes": [
                    {"id": "out", "kind": "export", "handle": "note",
                     "position": {"x": 80, "y": 80},
                     "inputs": {"value": {"kind": "parameter", "name": "note"}},
                     "config": {"type": "string"}}
                ],
                "edges": []
            }
        }))
        .unwrap(),
    );
    demo.scenarios[0].graph = serde_json::from_value(serde_json::json!({
        "nodes": [
            {"id": "call", "kind": "component", "handle": "write_a_note",
             "label": "Write a note", "position": {"x": 80, "y": 80}, "inputs": {},
             "config": {"componentId": "notes", "revision": 1}}
        ],
        "edges": []
    }))
    .unwrap();
    apiforge_project::prepare_project(directory.path(), &demo).unwrap();
    directory
}

/// Whether `text` is painted on the board, the area of *Diagram board*.
fn painted_on_board(harness: &FolderHarness, text: &str) -> bool {
    let board = harness.get_by_label("Diagram board").rect();
    shapes(harness).into_iter().any(|(_, shape)| match shape {
        Shape::Text(shape) => {
            shape.galley.job.text == text && board.contains_rect(shape.visual_bounding_rect())
        }
        _ => false,
    })
}

/// KTLO-32 criterion 4: with the call card selected, its row for the
/// undeclared input `note` is the missing marker (`note · Missing input` on
/// the compact card, LAY-21)
/// and the inspector labels it `note *` with *Set this required argument.*;
/// binding a value in the code clears the marker and the message. At both
/// window sizes.
#[test]
fn ktlo32_an_undeclared_input_is_marked_required_on_a_call() {
    for size in [MINIMUM, WIDE] {
        let project = undeclared_input_project();
        let mut harness = folder_harness(project.path(), size);
        let id = document(folder(&harness), "scenarios/new-scenario.afs");
        app(&mut harness).activate_tab(&id);
        settle(&mut harness);
        show(&mut harness, "Diagram");
        harness.get_by_label_contains("Write a note (").click();
        settle(&mut harness);
        assert_eq!(
            folder(&harness).selections.get(&id).cloned(),
            Some(vec!["call".to_owned()]),
            "{size:?}: the call card is selected"
        );

        // The compact call card (LAY-21) names the input on its missing line.
        assert!(
            painted_on_board(&harness, "note · Missing input"),
            "{size:?}: the row's name and the missing marker"
        );
        assert!(
            !painted_on_board(&harness, "Optional"),
            "{size:?}: not the optional one"
        );
        assert!(
            harness.query_by_label("note *").is_some(),
            "{size:?}: the inspector's label"
        );
        assert!(
            harness
                .query_by_label("Set this required argument.")
                .is_some(),
            "{size:?}: the inspector's message"
        );

        let source = active_source(&harness);
        let bound = source.replacen("notes@1()", "notes@1(note: \"hello\")", 1);
        assert_ne!(bound, source, "the call is in the source:\n{source}");
        app(&mut harness)
            .session
            .as_mut()
            .unwrap()
            .edit_source(&id, &bound, CODE)
            .unwrap();
        settle(&mut harness);
        assert!(
            !painted_on_board(&harness, "note · Missing input")
                && painted_on_board(&harness, "note · hello"),
            "{size:?}: a bound input is not missing"
        );
        assert!(
            harness
                .query_by_label("Set this required argument.")
                .is_none(),
            "{size:?}: nor asked for"
        );
        assert!(
            harness.query_by_label("note *").is_some(),
            "{size:?}: still required"
        );
    }
}

// ---- UX-03: a grey focus line; error marks after a pause in typing ----

mod ux03 {
    use super::*;
    use crate::folder::code_view::{MARKS_DELAY, marks_due};
    use crate::testing::capture;
    use egui_kittest::kittest::NodeT;
    use std::time::{Duration, Instant};

    /// The statement the mock-up shows half typed: `cart` is no block of
    /// the scenario and the line does not end, so the code is invalid.
    const HALF_TYPED: &str = "total = cart.body.";

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    fn active_id(harness: &FolderHarness) -> String {
        folder(harness).workbench.active().unwrap().to_owned()
    }

    fn snapshot(harness: &FolderHarness) -> Snapshot {
        let id = active_id(harness);
        folder(harness).snapshot(&id).unwrap()
    }

    fn marked(harness: &FolderHarness) -> Vec<usize> {
        folder(harness).marked_lines(&active_id(harness))
    }

    /// The demo shop in Code with the marks' clock held at `start`.
    fn shop_in_code(size: egui::Vec2, start: Instant) -> (tempfile::TempDir, FolderHarness) {
        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), size);
        show(&mut harness, "Code");
        app(&mut harness).set_marks_clock(start);
        settle(&mut harness);
        (project, harness)
    }

    /// The caret at the end of line 2, the scenario's description, by a
    /// click in the editor, which then has keyboard focus.
    fn caret_after_line_2(harness: &mut FolderHarness) {
        let at = editor(harness).left_top() + egui::vec2(120.0, 24.0);
        click_at(harness, at);
        harness.key_press(egui::Key::End);
        settle(harness);
    }

    /// A new line 3 typed after the description, as in the mock-up:
    /// Enter, then [`HALF_TYPED`] in one go.
    fn type_half_statement(harness: &mut FolderHarness) {
        caret_after_line_2(harness);
        harness.key_press(egui::Key::Enter);
        harness.event(egui::Event::Text(HALF_TYPED.into()));
        settle_analysed(harness);
    }

    /// Settles until the analysis of the active document has a result: the
    /// analyser's own pause of 250 ms is real time and does not change.
    fn settle_analysed(harness: &mut FolderHarness) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            settle(harness);
            if snapshot(harness).analysis != AnalysisState::Pending {
                return;
            }
            assert!(Instant::now() < deadline, "the analysis never finished");
            std::thread::sleep(ms(10));
        }
    }

    /// The marks' clock stepped to `now`, then the window settled.
    fn at(harness: &mut FolderHarness, now: Instant) {
        app(harness).set_marks_clock(now);
        settle(harness);
    }

    /// The marks' clock stepped to `now`, a moment before the marks are
    /// due, and two frames drawn. The window asks for the next frame at
    /// once, as it should with the marks due within a frame (the harness's
    /// frame is 250 ms), so with the clock held it would never settle.
    fn just_before(harness: &mut FolderHarness, now: Instant) {
        app(harness).set_marks_clock(now);
        harness.step();
        harness.step();
    }

    /// The lines the analysis has errors on, counted from one, sorted.
    fn error_lines(harness: &FolderHarness) -> Vec<usize> {
        let snapshot = snapshot(harness);
        let mut lines: Vec<usize> = snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| line_column(&snapshot.text, diagnostic.range.start).0)
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// The byte ranges of the code the editor painted with a red underline.
    fn underlined(harness: &FolderHarness) -> Vec<std::ops::Range<usize>> {
        let source = active_source(harness);
        shapes(harness)
            .into_iter()
            .filter_map(|(_, shape)| match shape {
                Shape::Text(text) if text.galley.job.text == source => Some(text.galley),
                _ => None,
            })
            .flat_map(|galley| {
                galley
                    .job
                    .sections
                    .iter()
                    .filter(|section| {
                        section.format.underline.width > 0.0
                            && section.format.underline.color == theme::DANGER
                    })
                    .map(|section| section.byte_range.start.0..section.byte_range.end.0)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// No red number, dot or underline anywhere in the code.
    fn assert_no_marks(harness: &FolderHarness, when: &str) {
        assert_eq!(marked(harness), Vec::<usize>::new(), "{when}: marked lines");
        assert_eq!(
            underlined(harness),
            Vec::<std::ops::Range<usize>>::new(),
            "{when}: underlined code"
        );
    }

    /// The marks of the current analysis: every error's line marked and its
    /// range underlined.
    fn assert_marks_of_the_analysis(harness: &FolderHarness, when: &str) {
        let lines = error_lines(harness);
        assert!(!lines.is_empty(), "{when}: the code has errors to mark");
        let mut shown = marked(harness);
        shown.sort_unstable();
        assert_eq!(shown, lines, "{when}: the marked lines");
        let underlined = underlined(harness);
        for diagnostic in snapshot(harness)
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
        {
            // A diagnostic at the end of the text has an empty range; the
            // token it touches is underlined.
            let range =
                diagnostic.range.start..diagnostic.range.end.max(diagnostic.range.start + 1);
            assert!(
                underlined
                    .iter()
                    .any(|under| under.start < range.end && range.start < under.end),
                "{when}: {:?} at {range:?} is not underlined; underlined: {underlined:?}",
                diagnostic.message
            );
        }
    }

    /// The status beside PROBLEMS, which starts with `save_state`, and
    /// whether it is painted red.
    fn problems_status(harness: &FolderHarness, save_state: &str) -> (String, bool) {
        // A label's text is its accessible value.
        let node = harness.get_by_label_contains(save_state);
        let status = node
            .value()
            .or_else(|| node.accesskit_node().label())
            .unwrap_or_default();
        let red = shapes(harness).into_iter().any(|(_, shape)| match shape {
            Shape::Text(text) => {
                text.galley.job.text == status
                    && text
                        .galley
                        .job
                        .sections
                        .iter()
                        .all(|section| section.format.color == theme::DANGER)
            }
            _ => false,
        });
        (status, red)
    }

    /// Steps the window one frame at a time until it settles and the
    /// analysis has a result, checking after every frame that no mark is
    /// drawn.
    fn settle_without_marks(harness: &mut FolderHarness, when: &str) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            harness.step();
            assert_no_marks(harness, when);
            let immediate = harness
                .output()
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .is_some_and(|output| output.repaint_delay.is_zero());
            if !immediate
                && folder(harness).is_analysed()
                && snapshot(harness).analysis != AnalysisState::Pending
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{when}: the window never settled"
            );
            std::thread::sleep(ms(1));
        }
    }

    /// Criterion 1: the focus line at the gutter's edge is 2 px grey while
    /// the editor has keyboard focus, never the accent, and absent without
    /// focus.
    #[test]
    fn ux03_criterion_1_the_focus_line_is_grey() {
        let (_project, mut harness) = shop_in_code(MINIMUM, Instant::now());
        // On line 1, which is shorter than the panel, so the code does not
        // scroll from under the gutter.
        let at = editor(&harness).left_top() + egui::vec2(60.0, 10.0);
        click_at(&mut harness, at);
        assert!(
            harness.get_by_label_contains("Contents of").is_focused(),
            "a click in the editor gives it keyboard focus"
        );
        assert_eq!(
            folder(&harness).code.focus_stroke(),
            Some(egui::Stroke::new(2.0, theme::MUTED)),
            "the focus line the editor painted"
        );
        assert!(focus_line(&harness), "no grey line in the gutter");
        assert!(
            !line_in_gutter(&harness, theme::ACCENT) && !line_in_gutter(&harness, theme::DANGER),
            "a red line is still painted in the gutter"
        );

        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
        assert!(!harness.get_by_label_contains("Contents of").is_focused());
        assert_eq!(folder(&harness).code.focus_stroke(), None);
        assert!(!focus_line(&harness), "a focus line without focus");
    }

    /// Criterion 2: while typing pauses for less than 700 ms the editor marks
    /// nothing, although the analysis has finished and PROBLEMS counts the
    /// errors.
    #[test]
    fn ux03_criterion_2_no_marks_while_typing() {
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        type_half_statement(&mut harness);
        at(&mut harness, start + ms(300));
        assert_eq!(snapshot(&harness).analysis, AnalysisState::Invalid);
        assert!(error_lines(&harness).contains(&3), "line 3 has an error");
        let (status, red) = problems_status(&harness, "Unsaved changes");
        assert!(
            status.starts_with("Unsaved changes") && status.contains("error"),
            "PROBLEMS counts the errors: {status:?}"
        );
        assert!(red, "the status {status:?} is red");
        assert_no_marks(&harness, "300 ms after typing");
    }

    /// Criterion 2: an error corrected within the pause is never marked.
    #[test]
    fn ux03_criterion_2_an_error_corrected_within_the_pause_shows_no_mark() {
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        caret_after_line_2(&mut harness);
        harness.key_press(egui::Key::Enter);
        harness.event(egui::Event::Text(HALF_TYPED.into()));
        settle_without_marks(&mut harness, "while the error is typed");
        at(&mut harness, start + ms(300));
        assert_eq!(snapshot(&harness).analysis, AnalysisState::Invalid);
        assert_no_marks(&harness, "300 ms after typing the error");

        // `cart.body.` replaced by `2`: `total = 2` is a valid statement.
        for _ in 0.."cart.body.".len() {
            harness.key_press(egui::Key::Backspace);
        }
        harness.event(egui::Event::Text("2".into()));
        settle_without_marks(&mut harness, "while the error is corrected");
        assert!(
            active_source(&harness).contains("\n  total = 2\n"),
            "the line is corrected:\n{}",
            active_source(&harness)
        );
        assert_eq!(snapshot(&harness).analysis, AnalysisState::Valid);
        at(&mut harness, start + ms(600));
        assert_no_marks(&harness, "300 ms after the correction");
        just_before(&mut harness, start + ms(999));
        assert_no_marks(&harness, "699 ms after the correction");
        for after in [700, 2000] {
            at(&mut harness, start + ms(300 + after));
            assert_no_marks(&harness, &format!("{after} ms after the correction"));
        }
    }

    /// Criterion 2 and 5: the marks of the current analysis appear once
    /// typing has paused for 700 ms, not at 699.
    #[test]
    fn ux03_criterion_2_marks_follow_the_analysis_after_the_pause() {
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        type_half_statement(&mut harness);
        just_before(&mut harness, start + ms(699));
        assert_no_marks(&harness, "699 ms after typing");
        at(&mut harness, start + ms(700));
        assert_marks_of_the_analysis(&harness, "700 ms after typing");
        assert!(marked(&harness).contains(&3), "line 3 is marked");
    }

    /// Criterion 2: the frame of the keystroke itself shows no mark, though
    /// the marks were shown up to it: the edited text is never drawn with
    /// the earlier analysis's marks.
    #[test]
    fn ux03_criterion_2_the_keystroke_frame_shows_no_mark() {
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        type_half_statement(&mut harness);
        at(&mut harness, start + ms(700));
        assert_marks_of_the_analysis(&harness, "700 ms after typing");

        // Before `cart`: the marks would land one column off.
        for _ in 0.."cart.body.".len() {
            harness.key_press(egui::Key::ArrowLeft);
        }
        settle(&mut harness);
        assert_marks_of_the_analysis(&harness, "after moving the caret");
        harness.event(egui::Event::Text("x".into()));
        harness.step();
        assert!(
            active_source(&harness).contains("total = xcart.body."),
            "the key reached the text:\n{}",
            active_source(&harness)
        );
        assert_no_marks(&harness, "the keystroke's frame");
    }

    /// Criterion 2: once the pause has passed the marks are the current
    /// analysis's: while the typed text is still being analysed none is
    /// drawn, not the earlier analysis's at their old places.
    #[test]
    fn ux03_criterion_2_marks_wait_for_the_analysis_of_the_typed_text() {
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        let id = active_id(&harness);
        type_half_statement(&mut harness);
        at(&mut harness, start + ms(700));
        assert_marks_of_the_analysis(&harness, "700 ms after typing");

        // A second line typed at 700 ms, its analysis held back.
        harness.key_press(egui::Key::Enter);
        harness.event(egui::Event::Text("more = cart.".into()));
        harness.step();
        app(&mut harness)
            .analyser
            .typed(&id, Instant::now() + Duration::from_secs(3600));
        at(&mut harness, start + ms(1400));
        assert_eq!(snapshot(&harness).analysis, AnalysisState::Pending);
        assert!(
            !snapshot(&harness).diagnostics.is_empty(),
            "the earlier analysis's diagnostics are still held"
        );
        assert_no_marks(&harness, "700 ms after typing, the analysis pending");

        app(&mut harness)
            .analyser
            .typed(&id, Instant::now() - crate::buffers::APPLY_DELAY);
        settle_analysed(&mut harness);
        assert_marks_of_the_analysis(&harness, "the analysis done");
        assert!(marked(&harness).contains(&4), "the new line 4 is marked");
    }

    /// Criterion 2: the message on hover is a mark too: none over the error
    /// during the pause, the diagnostic's after it.
    #[test]
    fn ux03_criterion_2_the_hover_message_waits_for_the_pause() {
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        type_half_statement(&mut harness);
        at(&mut harness, start + ms(300));
        let message = "Unknown block handle";
        assert!(
            harness
                .query_by_label_contains(&format!("Line 3, column 11   {message}"))
                .is_some(),
            "the list names the error"
        );
        // Over `cart` on line 3: `  total = ` is ten columns.
        let (row, column) = harness.ctx.fonts_mut(|fonts| {
            let font = egui::FontId::monospace(CODE_FONT);
            (fonts.row_height(&font), fonts.glyph_width(&font, '0'))
        });
        let editor = editor(&harness);
        let over = editor.left_top() + egui::vec2(4.0 + column * 11.5, 2.0 + row * 2.5);
        harness.hover_at(over);
        harness.run_steps(6);
        assert!(
            harness.query_by_label(message).is_none(),
            "a hover message during the pause"
        );
        app(&mut harness).set_marks_clock(start + ms(700));
        harness.hover_at(over + egui::vec2(1.0, 0.0));
        harness.run_steps(6);
        assert!(
            harness.query_by_label(message).is_some(),
            "no hover message over the error after the pause"
        );
    }

    /// Criterion 3 (a regression test): the PROBLEMS status and list follow
    /// the analysis as before, with the marks' clock not moved at all:
    /// *Unsaved changes · N errors* in red, and a row for line 3. The
    /// analysis pause is still 250 ms.
    #[test]
    fn ux03_criterion_3_the_problems_status_is_unchanged() {
        assert_eq!(crate::buffers::APPLY_DELAY, ms(250));
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        let (status, red) = problems_status(&harness, "Saved");
        assert_eq!(status, "Saved");
        assert!(!red, "a saved, valid document's status is not red");

        type_half_statement(&mut harness);
        let errors = snapshot(&harness)
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count();
        let (status, red) = problems_status(&harness, "Unsaved changes");
        let expected = if errors == 1 {
            "Unsaved changes  ·  1 error".to_owned()
        } else {
            format!("Unsaved changes  ·  {errors} errors")
        };
        assert_eq!(status, expected);
        assert!(red, "the status {status:?} is red");
        assert!(
            harness.query_by_label_contains("Line 3, column").is_some(),
            "the list names line 3"
        );
    }

    /// Criterion 5: the rule itself.
    #[test]
    fn ux03_criterion_5_marks_due_at_the_delay() {
        assert_eq!(MARKS_DELAY, ms(700));
        let typed = Instant::now();
        assert!(marks_due(None, typed, MARKS_DELAY), "nothing typed");
        assert!(!marks_due(Some(typed), typed, MARKS_DELAY));
        assert!(!marks_due(Some(typed), typed + ms(699), MARKS_DELAY));
        assert!(marks_due(Some(typed), typed + ms(700), MARKS_DELAY));
        assert!(marks_due(Some(typed), typed + ms(5000), MARKS_DELAY));
    }

    /// Criterion 5: while the marks wait, the window asks to be drawn again
    /// when they are due, so they appear without a mouse move.
    #[test]
    fn ux03_criterion_5_a_repaint_is_requested_for_the_marks() {
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        let id = active_id(&harness);
        type_half_statement(&mut harness);
        at(&mut harness, start + ms(300));
        assert_eq!(folder(&harness).marks_wait(&id), Some(ms(400)));
        let delay = harness
            .output()
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .unwrap()
            .repaint_delay;
        assert!(
            delay <= ms(400),
            "the window asked to be drawn again after {delay:?}, later than the marks"
        );
        at(&mut harness, start + ms(700));
        assert_eq!(folder(&harness).marks_wait(&id), None);
    }

    /// Criterion 6: typing, paste, undo and redo restart the pause; moving
    /// the caret, selecting and scrolling do not.
    #[test]
    fn ux03_criterion_6_only_text_changes_restart_the_pause() {
        let start = Instant::now();
        let (_project, mut harness) = shop_in_code(MINIMUM, start);
        type_half_statement(&mut harness);
        // Long enough for the text edit to keep an undo point here.
        harness.run_steps(90);

        // Not text changes, 400 ms into the pause.
        at(&mut harness, start + ms(400));
        harness.key_press(egui::Key::ArrowLeft);
        harness.key_press(egui::Key::ArrowUp);
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::ArrowRight);
        settle(&mut harness);
        harness.hover_at(editor(&harness).left_top() + egui::vec2(120.0, 60.0));
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -40.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        settle(&mut harness);
        harness.key_press(egui::Key::End);
        settle(&mut harness);
        at(&mut harness, start + ms(700));
        assert_marks_of_the_analysis(&harness, "caret moves, a selection and a scroll");

        // Typing restarts a pause that is already running: a key at 700 ms,
        // another 400 ms later, the marks 700 ms after the second.
        let mut now = start + ms(700);
        harness.event(egui::Event::Text("y".into()));
        settle_analysed(&mut harness);
        at(&mut harness, now + ms(400));
        harness.event(egui::Event::Text("z".into()));
        settle_analysed(&mut harness);
        assert_eq!(snapshot(&harness).analysis, AnalysisState::Invalid);
        just_before(&mut harness, now + ms(400 + 699));
        assert_no_marks(&harness, "699 ms after typing within a pause");
        at(&mut harness, now + ms(400 + 700));
        assert_marks_of_the_analysis(&harness, "700 ms after typing within a pause");
        now += ms(400 + 700);
        harness.run_steps(90);

        // Each other text change restarts it: marks at 700 ms, not at 699.
        for (change, event) in [
            ("a paste", None),
            ("an undo", Some(egui::Modifiers::COMMAND)),
            (
                "a redo",
                Some(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT),
            ),
        ] {
            let before = active_source(&harness);
            match event {
                None => harness.event(egui::Event::Paste(" x".into())),
                Some(modifiers) => harness.key_press_modifiers(modifiers, egui::Key::Z),
            }
            settle_analysed(&mut harness);
            assert_ne!(active_source(&harness), before, "{change} changes the text");
            assert_eq!(
                snapshot(&harness).analysis,
                AnalysisState::Invalid,
                "{change} leaves the code invalid"
            );
            just_before(&mut harness, now + ms(699));
            assert_no_marks(&harness, &format!("699 ms after {change}"));
            at(&mut harness, now + ms(700));
            assert_marks_of_the_analysis(&harness, &format!("700 ms after {change}"));
            now += ms(700);
            app(&mut harness).set_marks_clock(now);
            harness.run_steps(90);
        }
    }

    /// Criterion 4: the design review view of the three states the QA report
    /// compares with `ux03-now`: the focused editor with its grey line; the
    /// statement half typed, the analysis done and no mark yet; the pause
    /// passed and the marks shown.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_ux03_editor_typing() {
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let start = Instant::now();
            let (_project, mut harness) = shop_in_code(size, start);
            caret_after_line_2(&mut harness);
            capture(&mut harness, &format!("ux03-focused-{suffix}"), None);
            harness.key_press(egui::Key::Enter);
            harness.event(egui::Event::Text(HALF_TYPED.into()));
            settle_analysed(&mut harness);
            at(&mut harness, start + ms(300));
            capture(&mut harness, &format!("ux03-typing-{suffix}"), None);
            at(&mut harness, start + ms(700));
            capture(&mut harness, &format!("ux03-marks-{suffix}"), None);
        }
    }
}

// ---- UX-11: no completion in a comment; a click after a line's end ----

mod ux11 {
    use super::*;

    /// The demo shop in Code at the minimum size.
    fn shop_in_code() -> (tempfile::TempDir, FolderHarness) {
        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), MINIMUM);
        show(&mut harness, "Code");
        (project, harness)
    }

    /// The line of the active source holding `needle`, counted from zero.
    fn line_holding(harness: &FolderHarness, needle: &str) -> (usize, String) {
        active_source(harness)
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains(needle))
            .map(|(index, line)| (index, line.to_owned()))
            .unwrap_or_else(|| panic!("no line holds {needle:?}"))
    }

    /// The vertical middle of a line of the editor, counted from zero.
    fn row_middle(harness: &mut FolderHarness, index: usize) -> f32 {
        editor(harness).top() + 2.0 + row_height(harness) * (index as f32 + 0.5)
    }

    fn glyph_width(harness: &FolderHarness) -> f32 {
        harness
            .ctx
            .fonts_mut(|fonts| fonts.glyph_width(&egui::FontId::monospace(CODE_FONT), '0'))
    }

    fn type_text(harness: &mut FolderHarness, text: &str) {
        harness.event(egui::Event::Text(text.into()));
        settle(harness);
    }

    fn popup_open(harness: &FolderHarness) -> bool {
        folder(harness).code.popup_open()
    }

    fn focused(harness: &FolderHarness) -> bool {
        harness.get_by_label_contains("Contents of").is_focused()
    }

    /// Criterion 1: with the caret inside a `//` comment, a typed letter
    /// opens no completion popup; the same keystroke outside the comment
    /// does. Completion inside a string that is not a template is a
    /// follow-up, not part of this ticket.
    #[test]
    fn ux11_criterion_1_no_completion_inside_a_comment() {
        let (_project, mut harness) = shop_in_code();
        let (index, _) = line_holding(&harness, "on_error continue");
        let y = row_middle(&mut harness, index);
        let x = editor(&harness).left() + 40.0;
        click_at(&mut harness, egui::pos2(x, y));
        harness.key_press(egui::Key::End);
        settle(&mut harness);

        type_text(&mut harness, " // total = ");
        type_text(&mut harness, "p");
        assert!(
            active_source(&harness).contains("on_error continue // total = p"),
            "the letter was typed into the comment"
        );
        assert!(
            !popup_open(&harness),
            "a letter typed inside a comment opened the completion popup"
        );

        harness.key_press(egui::Key::Enter);
        settle(&mut harness);
        type_text(&mut harness, "total = ");
        type_text(&mut harness, "p");
        assert!(
            popup_open(&harness),
            "the same letter outside a comment opens the completion popup"
        );
    }

    /// The demo shop with a one-word description, so no line of the
    /// scenario is wider than the code panel at the wide size.
    fn short_lines_project() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let mut demo = apiforge_core::demos::shop_project();
        demo.scenarios[0].description = "Short.".into();
        apiforge_project::prepare_project(directory.path(), &demo).unwrap();
        directory
    }

    /// The line of the active source at `index`, counted from zero.
    fn line_at(harness: &FolderHarness, index: usize) -> String {
        active_source(harness)
            .lines()
            .nth(index)
            .unwrap()
            .to_owned()
    }

    /// Criterion 2: a click to the right of a short line's last character
    /// focuses the editor with the caret at the end of that line, and
    /// typing inserts there; in Code and Split, at both sizes, with every
    /// line narrower than the panel and with one wider than it.
    #[test]
    fn ux11_criterion_2_a_click_after_a_line_end_puts_the_caret_there() {
        for lines in ["short lines", "a long line"] {
            for size in [MINIMUM, WIDE] {
                for mode in ["Code", "Split"] {
                    let case = format!("{mode} at {size:?} with {lines}");
                    let project = if lines == "short lines" {
                        short_lines_project()
                    } else {
                        crate::testing::shop_project()
                    };
                    let mut harness = folder_harness(project.path(), size);
                    show(&mut harness, mode);
                    let (index, line) = line_holding(&harness, "on_error continue");
                    let y = row_middle(&mut harness, index);
                    let end =
                        editor(&harness).left() + 4.0 + glyph_width(&harness) * line.len() as f32;
                    click_at(&mut harness, egui::pos2(end + 80.0, y));
                    assert!(
                        focused(&harness),
                        "{case}: a click after the line's end focuses the editor"
                    );
                    type_text(&mut harness, "x");
                    assert_eq!(
                        line_at(&harness, index),
                        format!("{line}x"),
                        "{case}: the letter went to the line's end"
                    );
                }
            }
        }
    }

    /// Criterion 3: a click on the text still places the caret at the
    /// clicked character.
    #[test]
    fn ux11_criterion_3_a_click_on_the_text_places_the_caret_there() {
        let (_project, mut harness) = shop_in_code();
        let (index, line) = line_holding(&harness, "on_error continue");
        let column = line.find("on_error").unwrap() + 2;
        let y = row_middle(&mut harness, index);
        let x = editor(&harness).left() + 4.0 + glyph_width(&harness) * (column as f32 + 0.2);
        click_at(&mut harness, egui::pos2(x, y));
        assert!(focused(&harness), "a click on the text focuses the editor");

        type_text(&mut harness, "x");
        let mut expected = line.clone();
        expected.insert(column, 'x');
        assert_eq!(
            line_at(&harness, index),
            expected,
            "the letter went to the clicked character"
        );
    }
}

// ---- UX-20: undo on invalid source says so plainly; the popup clears PROBLEMS ----

mod ux20 {
    use super::*;
    use std::time::{Duration, Instant};

    /// What Ctrl+Z on a board whose source is invalid or still being
    /// analysed says, instead of the session's own error (#153 part 1).
    const FIX_THE_CODE_FIRST: &str = "Fix the code first: the board follows the source.";

    /// The demo shop in Code at the minimum size.
    fn shop_in_code() -> (tempfile::TempDir, FolderHarness) {
        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), MINIMUM);
        show(&mut harness, "Code");
        (project, harness)
    }

    fn analysis(harness: &FolderHarness) -> AnalysisState {
        let app = folder(harness);
        app.snapshot(app.workbench.active().unwrap())
            .unwrap()
            .analysis
    }

    /// Settles until the analysis of the active document has a result: the
    /// analyser's own pause is real time.
    fn settle_analysed(harness: &mut FolderHarness) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            settle(harness);
            if analysis(harness) != AnalysisState::Pending {
                return;
            }
            assert!(Instant::now() < deadline, "the analysis never finished");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The source made invalid by typing a half statement on a new line 3.
    fn make_invalid(harness: &mut FolderHarness) {
        let at = editor(harness).left_top() + egui::vec2(120.0, 24.0);
        click_at(harness, at);
        harness.key_press(egui::Key::End);
        harness.key_press(egui::Key::Enter);
        harness.event(egui::Event::Text("total = cart.body.".into()));
        settle_analysed(harness);
        assert_eq!(analysis(harness), AnalysisState::Invalid);
    }

    fn undo(harness: &mut FolderHarness) {
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        settle(harness);
    }

    /// The notice is the plain sentence, and the session's own error is
    /// neither painted nor in the accessibility tree.
    fn assert_plain_notice(harness: &FolderHarness, when: &str) {
        let notice = folder(harness).notice().map(|notice| notice.text.clone());
        assert_eq!(
            notice.as_deref(),
            Some(FIX_THE_CODE_FIRST),
            "{when}: the notice"
        );
        let raw = apiforge_project::Error::ModelLocked.to_string();
        let painted = shapes(harness).into_iter().any(|(_, shape)| match shape {
            Shape::Text(text) => text.galley.job.text.contains(&raw),
            _ => false,
        });
        assert!(!painted, "{when}: the raw error {raw:?} is painted");
        assert!(
            harness.query_by_label_contains(&raw).is_none(),
            "{when}: the raw error is in the accessibility tree"
        );
    }

    /// Criterion 3: Ctrl+Z on the board with invalid source, and in Code
    /// without the editor's focus (decision 20 = A sends it to the model's
    /// history there too), shows the plain sentence and no raw error text.
    #[test]
    fn ux20_criterion_3_undo_on_invalid_source_says_so_plainly() {
        let (_project, mut harness) = shop_in_code();
        make_invalid(&mut harness);

        show(&mut harness, "Diagram");
        undo(&mut harness);
        assert_plain_notice(&harness, "Ctrl+Z on the board");

        app(&mut harness).notices.clear();
        show(&mut harness, "Code");
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
        assert!(
            !harness.get_by_label_contains("Contents of").is_focused(),
            "the editor gave up the keyboard"
        );
        undo(&mut harness);
        assert_plain_notice(&harness, "Ctrl+Z in Code without the editor's focus");
    }

    /// Criterion 4: completion opened on a new line at the bottom of the
    /// view, just above PROBLEMS, is drawn fully inside the editor's area,
    /// clear of the Problems header.
    #[test]
    fn ux20_criterion_4_the_popup_stays_inside_the_editor() {
        let (_project, mut harness) = shop_in_code();
        let row = row_height(&mut harness);
        let top = editor(&harness).top() + 2.0;
        let bottom = problems_top(&harness) - 6.0;
        // The last line wholly in view, then a new line under it, which
        // scrolls the code so the caret sits at the bottom of the view.
        let y = top + row * (((bottom - top) / row).floor() - 0.5);
        let x = editor(&harness).left() + 40.0;
        click_at(&mut harness, egui::pos2(x, y));
        harness.key_press(egui::Key::End);
        harness.key_press(egui::Key::Enter);
        harness.event(egui::Event::Text("total = p".into()));
        settle(&mut harness);
        assert!(folder(&harness).code.popup_open(), "completion opened");

        let popup = folder(&harness)
            .code
            .popup_rect()
            .expect("the popup was drawn");
        let header = harness.get_by_label("PROBLEMS").rect();
        let area = editor(&harness);
        assert!(
            popup.bottom() <= problems_top(&harness),
            "the popup {popup:?} reaches below the code into PROBLEMS at {}",
            problems_top(&harness)
        );
        assert!(
            !popup.intersects(header),
            "the popup {popup:?} covers the PROBLEMS header {header:?}"
        );
        assert!(
            popup.left() >= area.left() && popup.right() <= area.right(),
            "the popup {popup:?} is outside the editor {area:?}"
        );
        assert!(
            popup.top() >= top - 2.0,
            "the popup {popup:?} is above the editor"
        );
    }
}
