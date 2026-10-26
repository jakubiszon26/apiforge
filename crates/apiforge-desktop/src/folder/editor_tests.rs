//! KTLO-13: the code editor fills its panel without a bordered box, a long
//! line scrolls under line numbers that stay put, and keyboard focus shows as
//! a line in the gutter. Driven through named widgets (ADR 0013); what the
//! editor paints is read from the frame's shapes. See docs/tickets/KTLO-13.md.
//! The KTLO-18 tests at the end expand the Problems section over the code
//! (docs/tickets/KTLO-18.md); they share this file's `editor` and
//! `problems_top`.
use super::project_interaction_tests::replace_text;
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

/// Whether an accent-coloured line is painted in the gutter: left of the text
/// area and taller than a dot.
fn focus_line(harness: &FolderHarness) -> bool {
    let editor = editor(harness);
    let in_gutter = |rect: egui::Rect| {
        rect.right() <= editor.left() + 1.0
            && rect.left() >= editor.left() - 80.0
            && rect.height() >= 20.0
    };
    shapes(harness).into_iter().any(|(_, shape)| match shape {
        Shape::LineSegment { points, stroke } => {
            stroke.color == theme::ACCENT
                && stroke.width > 0.0
                && in_gutter(egui::Rect::from_two_pos(points[0], points[1]))
        }
        Shape::Rect(rect) => rect.fill == theme::ACCENT && in_gutter(rect.rect),
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
        let label = editor_label(&harness);
        assert_within_window(&harness, &[&label, "PROBLEMS", "Restore problems"]);
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
