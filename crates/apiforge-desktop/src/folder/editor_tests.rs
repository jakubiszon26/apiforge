//! KTLO-13: the code editor fills its panel without a bordered box, a long
//! line scrolls under line numbers that stay put, and keyboard focus shows as
//! a line in the gutter. Driven through named widgets (ADR 0013); what the
//! editor paints is read from the frame's shapes. See docs/tickets/KTLO-13.md.
use super::*;
use crate::testing::{FolderHarness, MINIMUM, WIDE, folder, folder_harness, settle};
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

/// The top of the Problems bar's caption, just below the code panel: the bar's
/// inner margin (8) and the code panel's (6) lie between them.
fn problems_top(harness: &FolderHarness) -> f32 {
    harness.get_by_label("PROBLEMS").rect().top()
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
                editor.bottom() >= bottom - 16.0,
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
