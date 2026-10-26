//! KTLO-12: one header row above the board, driven through named widgets
//! (ADR 0013); see docs/tickets/KTLO-12.md. Criterion 5, the accessibility
//! audit, is `stage_ii_screens_and_dialogs_name_every_control`, which audits
//! Diagram and Split at both window sizes. Criterion 6 is the design review
//! and the QA report.
use super::*;
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_disjoint, assert_within_window, folder, folder_harness,
    settle,
};
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};

/// The demo scenario's file name and kind, as the Diagram header shows them.
const NAME: &str = "promotion-and-cart-price.afs";
const KIND: &str = "Scenario";
/// The board's two tool buttons, after Add step.
const TOOLS: [&str; 2] = ["Tidy up", "Wire style"];
/// The top app bar, which this ticket does not change.
const APP_BAR: [&str; 8] = [
    "Project",
    "Code",
    "Diagram",
    "Split",
    "Save",
    "Save all",
    "Run options",
    "Run",
];
/// How far apart, in points, the centres of two widgets on one row may be.
const ROW: f32 = 4.0;

fn open(size: egui::Vec2, mode: &str) -> (tempfile::TempDir, FolderHarness) {
    let project = shop();
    let mut harness = folder_harness(project.path(), size);
    harness.get_by_label(mode).click();
    settle(&mut harness);
    (project, harness)
}

fn rect(harness: &FolderHarness, label: &str) -> egui::Rect {
    harness.get_by_label(label).rect()
}

fn on_row(row: egui::Rect, other: egui::Rect) -> bool {
    (row.center().y - other.center().y).abs() <= ROW
}

/// A plain text label with exactly `text` on the row of the Main tab.
fn row_label(harness: &FolderHarness, text: &str) -> Option<egui::Rect> {
    let main = rect(harness, "Main");
    harness
        .query_all_by_role_and_label(Role::Label, text)
        .map(|node| node.rect())
        .find(|found| on_row(main, *found))
}

/// The steps summary ("6 steps · All steps") on the row of the Main tab.
fn summary(harness: &FolderHarness) -> Option<egui::Rect> {
    let main = rect(harness, "Main");
    harness
        .query_all_by(|node| {
            node.role() == Role::Label
                && node.value().is_some_and(|value| {
                    value.starts_with(|first: char| first.is_ascii_digit())
                        && value.contains(" steps")
                })
        })
        .map(|node| node.rect())
        .find(|found| on_row(main, *found))
}

/// The row's widgets that every mode shows, left to right.
fn common_row(harness: &FolderHarness) -> Vec<(String, egui::Rect)> {
    ["Main", "Cleanup", "Add step"]
        .into_iter()
        .chain(TOOLS)
        .map(|label| (label.to_owned(), rect(harness, label)))
        .collect()
}

fn assert_left_to_right(order: &[(String, egui::Rect)], context: &str) {
    for pair in order.windows(2) {
        let ((first, a), (second, b)) = (&pair[0], &pair[1]);
        assert!(
            a.right() <= b.left() + 0.5,
            "{context}: {first:?} at {a:?} is not left of {second:?} at {b:?}"
        );
    }
}

fn assert_inside(harness: &FolderHarness, found: &[(String, egui::Rect)], context: &str) {
    let window = harness.ctx.content_rect();
    for (label, at) in found {
        assert!(
            window.contains_rect(*at),
            "{context}: {label:?} at {at:?} is outside the window {window:?}"
        );
    }
}

/// Criterion 1: in Diagram and in Split exactly one row lies between the tab
/// strip and the board, and it is on the board's own header band; the top app
/// bar is unchanged.
#[test]
fn one_row_lies_between_the_tab_strip_and_the_board() {
    for size in [MINIMUM, WIDE] {
        for mode in ["Diagram", "Split"] {
            let (_project, harness) = open(size, mode);
            let context = format!("{mode} at {size:?}");

            assert_within_window(&harness, &APP_BAR);
            let project = rect(&harness, "Project");
            for label in APP_BAR {
                assert!(
                    on_row(project, rect(&harness, label)),
                    "{context}: the app bar keeps {label:?} on its row"
                );
            }

            let tab = harness
                .query_all_by_label(NAME)
                .map(|node| node.rect())
                .min_by(|a, b| a.top().total_cmp(&b.top()))
                .expect("the tab strip names the scenario");
            let board = rect(&harness, "Diagram board");
            let main = rect(&harness, "Main");
            let mut row = common_row(&harness);
            if mode == "Diagram" {
                let name = row_label(&harness, NAME)
                    .unwrap_or_else(|| panic!("{context}: the file name is on the row of Main"));
                row.push((NAME.to_owned(), name));
            }
            for (label, at) in &row {
                assert!(
                    on_row(main, *at),
                    "{context}: {label:?} at {at:?} is not on the row of Main at {main:?}"
                );
                assert!(
                    at.top() >= board.top() && at.top() >= tab.bottom(),
                    "{context}: {label:?} at {at:?} is not on the board's band {board:?} \
                     below the tab strip {tab:?}"
                );
            }
            assert!(
                main.top() - board.top() < 16.0,
                "{context}: the row at {main:?} is the board's first band {board:?}"
            );
        }
    }
}

/// Criterion 2: Diagram shows file name, kind, Main, Cleanup, steps summary,
/// Add step and the tools, left to right; Split shows Main, Cleanup, then the
/// same right-hand group, with no file name or kind on the row.
#[test]
fn the_row_shows_its_parts_left_to_right() {
    let (_project, diagram) = open(WIDE, "Diagram");
    let mut order = vec![
        (
            NAME.to_owned(),
            row_label(&diagram, NAME).expect("Diagram shows the file name on the row"),
        ),
        (
            KIND.to_owned(),
            row_label(&diagram, KIND).expect("Diagram shows the kind on the row"),
        ),
    ];
    let mut common = common_row(&diagram);
    let summary_at = summary(&diagram).expect("Diagram shows the steps summary on the row");
    common.insert(2, ("steps summary".to_owned(), summary_at));
    order.extend(common);
    assert_left_to_right(&order, "Diagram");
    assert_disjoint(
        &diagram,
        &["Main", "Cleanup", "Add step", TOOLS[0], TOOLS[1]],
    );

    let (_project, split) = open(WIDE, "Split");
    assert!(
        row_label(&split, NAME).is_none() && row_label(&split, KIND).is_none(),
        "Split leaves the file name and kind to the tab strip"
    );
    let mut order = common_row(&split);
    let summary_at = summary(&split).expect("Split at 1600 px shows the steps summary");
    order.insert(2, ("steps summary".to_owned(), summary_at));
    assert_left_to_right(&order, "Split");
}

/// Criterion 3: Main and Cleanup keep their names and switch the board's
/// phase; Add step and the tools keep their names and Add step opens its menu.
#[test]
fn main_cleanup_add_step_and_the_tools_keep_their_names_and_behaviour() {
    for mode in ["Diagram", "Split"] {
        let (_project, mut harness) = open(WIDE, mode);
        let id = folder(&harness).workbench.active().unwrap().to_owned();
        for label in ["Main", "Cleanup", "Add step"].into_iter().chain(TOOLS) {
            assert_eq!(
                harness.query_all_by_label(label).count(),
                1,
                "{mode}: exactly one control is named {label:?}"
            );
        }
        harness.get_by_label("Cleanup").click();
        settle(&mut harness);
        assert!(
            folder(&harness).cleanup(&id),
            "{mode}: Cleanup shows cleanup"
        );
        harness.get_by_label("Main").click();
        settle(&mut harness);
        assert!(!folder(&harness).cleanup(&id), "{mode}: Main shows main");

        let before = harness.root().children_recursive().count();
        harness.get_by_label("Add step").click();
        settle(&mut harness);
        assert!(
            harness.root().children_recursive().count() > before,
            "{mode}: Add step opens its menu"
        );
    }
}

/// The names of the controls Tab visits, in order, from the current focus.
fn tab_order(harness: &mut FolderHarness, presses: usize) -> Vec<String> {
    let mut visited = Vec::new();
    for _ in 0..presses {
        harness.key_press(egui::Key::Tab);
        settle(harness);
        let name = harness
            .query_by(|node| node.is_focused() && node.role() != Role::Window)
            .map(|node| {
                let node = node.accesskit_node();
                node.label()
                    .or_else(|| node.value())
                    .unwrap_or_else(|| "<unnamed>".to_owned())
            })
            .unwrap_or_else(|| "<window>".to_owned());
        visited.push(name);
    }
    visited
}

fn assert_visits_in_order(visited: &[String], names: &[&str], context: &str) {
    let mut at = 0;
    for name in names {
        at += visited[at..]
            .iter()
            .position(|visit| visit == name)
            .unwrap_or_else(|| panic!("{context}: {name} after {at} in {visited:#?}"));
    }
}

/// The row's stops follow one another, before anything on the board itself.
fn assert_row_stops_together(visited: &[String], row: &[&str], context: &str) {
    let first = visited
        .iter()
        .position(|visit| visit == row[0])
        .unwrap_or_else(|| panic!("{context}: Tab never reaches {} in {visited:#?}", row[0]));
    let stops: Vec<&str> = visited[first..]
        .iter()
        .take(row.len())
        .map(String::as_str)
        .collect();
    assert_eq!(stops, row, "{context}: the row's stops in {visited:#?}");
}

/// Criterion 3: the keyboard visits the file name, Main, Cleanup, Add step and
/// the tools in that order, in both modes, and the row's stops come before
/// the board's own. In Split the code editor lies
/// between the tab strip and the board and Tab indents inside it, so the walk
/// past it starts from the next stop after the editor, its resize handle.
#[test]
fn tab_runs_from_the_name_through_main_and_cleanup_to_add_step_and_the_tools() {
    let board = ["Main", "Cleanup", "Add step", TOOLS[0], TOOLS[1]];
    for size in [MINIMUM, WIDE] {
        let (_project, mut diagram) = open(size, "Diagram");
        let visited = tab_order(&mut diagram, 40);
        let context = format!("Diagram at {size:?}");
        assert_visits_in_order(&visited, &[NAME, "Main"], &context);
        assert_row_stops_together(&visited, &board, &context);

        let (_project, mut split) = open(size, "Split");
        let visited = tab_order(&mut split, 12);
        let context = format!("Split at {size:?}");
        assert_visits_in_order(
            &visited,
            &[NAME, "Contents of scenarios/promotion-and-cart-price.afs"],
            &context,
        );
        split.get_by_label("Resize code").focus();
        settle(&mut split);
        let visited = tab_order(&mut split, 40);
        assert_row_stops_together(&visited, &board, &context);
    }
}

/// Criterion 4: at 960 × 540 nothing on the row overlaps or is clipped, in
/// Diagram with the block library open and in Split. Diagram drops only the
/// kind; Split drops the steps summary.
#[test]
fn at_the_minimum_window_the_row_fits_without_overlap() {
    let (_project, wide) = open(WIDE, "Diagram");
    let full_name = row_label(&wide, NAME).unwrap().width();

    let (_project, diagram) = open(MINIMUM, "Diagram");
    assert!(
        diagram.query_by_label("Block library").is_some(),
        "the block library is open"
    );
    let name = row_label(&diagram, NAME).expect("the file name stays on the row");
    assert!(
        name.width() >= full_name - 0.5,
        "the file name is not shortened at 960 px: {} of {full_name}",
        name.width()
    );
    assert!(
        row_label(&diagram, KIND).is_none(),
        "the kind is the first part dropped"
    );
    let summary_at = summary(&diagram).expect("the steps summary still fits at 960 px");
    let mut row = vec![(NAME.to_owned(), name)];
    let mut common = common_row(&diagram);
    common.insert(2, ("steps summary".to_owned(), summary_at));
    row.extend(common);
    assert_inside(&diagram, &row, "Diagram at 960 × 540");
    assert_left_to_right(&row, "Diagram at 960 × 540");
    assert_within_window(&diagram, &["Block library", "Main", "Cleanup", "Add step"]);
    assert_within_window(&diagram, &TOOLS);

    let (_project, split) = open(MINIMUM, "Split");
    assert!(
        summary(&split).is_none(),
        "Split at 960 px leaves out the steps summary"
    );
    let row = common_row(&split);
    assert_inside(&split, &row, "Split at 960 × 540");
    assert_left_to_right(&row, "Split at 960 × 540");
    assert_disjoint(&split, &["Main", "Cleanup", "Add step", TOOLS[0], TOOLS[1]]);
}

/// How much of the Diagram row has been given up: 0 nothing, 1 the kind,
/// 2 the full file name as well, 3 the steps summary as well.
fn given_up(harness: &FolderHarness, full_name: f32, width: f32) -> u8 {
    let kind = row_label(harness, KIND).is_some();
    let name = row_label(harness, NAME)
        .unwrap_or_else(|| panic!("the file name stays on the row at {width} px"));
    let shortened = name.width() < full_name - 0.5;
    let summary = summary(harness).is_some();
    assert!(
        !(kind && (shortened || !summary)),
        "at {width} px the kind is dropped before anything else"
    );
    match (kind, shortened, summary) {
        (true, false, true) => 0,
        (false, false, true) => 1,
        (false, true, true) => 2,
        _ => 3,
    }
}

/// Criterion 4: as the board narrows, Diagram drops the kind first, then
/// shortens the name (its full text on hover), then drops the steps summary.
#[test]
fn a_narrowing_diagram_row_drops_the_kind_then_shortens_the_name_then_the_summary() {
    let (_project, wide) = open(WIDE, "Diagram");
    let full_name = row_label(&wide, NAME).unwrap().width();
    let mut previous = 0;
    // Each width opens a window; a sample keeps the suite's other timing
    // tests from being starved of CPU while still covering every transition.
    for width in [1600, 1360, 1120, 1040, 960, 920, 880, 840, 800, 760] {
        let width = width as f32;
        let (_project, harness) = open(egui::vec2(width, 800.0), "Diagram");
        let level = given_up(&harness, full_name, width);
        assert!(
            level >= previous,
            "at {width} px the row gives up less ({level}) than wider ({previous})"
        );
        previous = level;
        let expected = match width as u32 {
            1600 => Some(0),
            960 => Some(1),
            880 => Some(2),
            760 => Some(3),
            _ => None,
        };
        if let Some(expected) = expected {
            assert_eq!(level, expected, "what the row gives up at {width} px");
        }
    }

    let (_project, mut narrow) = open(egui::vec2(880.0, 800.0), "Diagram");
    let name = narrow
        .query_all_by_role_and_label(Role::Label, NAME)
        .count();
    let main = rect(&narrow, "Main");
    narrow
        .query_all_by_role_and_label(Role::Label, NAME)
        .find(|node| on_row(main, node.rect()))
        .expect("the shortened name is on the row")
        .hover();
    settle(&mut narrow);
    narrow.run_steps(60);
    assert!(
        narrow
            .query_all_by_role_and_label(Role::Label, NAME)
            .count()
            > name,
        "hovering the shortened name shows its full text"
    );
}
