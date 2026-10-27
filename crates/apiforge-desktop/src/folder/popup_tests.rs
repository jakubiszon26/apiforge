//! KTLO-05d: in Split below 1440 px, where the block library panel is hidden,
//! the board's *Add step* popup offers the panel's Variables and Operators
//! sections (KTLO-05a criterion 5, second half), driven through named widgets.
use super::project_interaction_tests::{choose, reveal};
use super::*;
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_within_window, capture, folder, folder_harness, settle,
};
use apiforge_core::{
    graph::{VALUE_INPUT, create_operator_node},
    model::NodeKind,
    operator::Operator,
};
use egui_kittest::kittest::Queryable;

/// The popup's heading when it is opened from the toolbar.
const POPUP: &str = "Add a step to the scenario";
/// The widest Split window without the library panel.
const NARROW_SPLIT: egui::Vec2 = egui::vec2(SPLIT_INSPECTOR_WIDTH - 1.0, 900.0);

/// A project whose one scenario has an empty cleanup board, so a block added
/// at the visible centre keeps its exact position.
fn empty_scenario() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    apiforge_project::prepare_project(directory.path(), &apiforge_core::demos::empty_project())
        .unwrap();
    directory
}

fn active_id(harness: &FolderHarness) -> String {
    folder(harness).workbench.active().unwrap().to_owned()
}

/// The folder window on `project` at `size`, switched to `mode`.
fn window(project: &tempfile::TempDir, size: egui::Vec2, mode: &str) -> FolderHarness {
    let mut harness = folder_harness(project.path(), size);
    harness.get_by_label(mode).click();
    settle(&mut harness);
    harness
}

/// Opens the *Add step* popup from the board's toolbar.
fn open_popup(harness: &mut FolderHarness) {
    harness.get_by_label("Add step").click();
    settle(harness);
    assert!(harness.query_by_label(POPUP).is_some(), "the popup opens");
}

/// Asserts that the last block of the board is selected, that the tab is dirty
/// and that the source gained its statement, as after a click in the panel.
fn assert_added_like_the_panel(
    harness: &FolderHarness,
    cleanup: bool,
) -> apiforge_core::format::Node {
    let id = active_id(harness);
    let model = model(folder(harness), &id);
    let added = graph_of(&model, cleanup).nodes.last().unwrap().clone();
    let selection = folder(harness)
        .selections
        .get(&id)
        .cloned()
        .unwrap_or_default();
    assert!(
        selection.contains(&added.id),
        "the new block is selected: {selection:?}"
    );
    let document = folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(&id)
        .unwrap();
    assert!(document.needs_recovery(), "the tab is dirty after an edit");
    assert!(
        document.text().contains(&format!("{} =", added.handle)),
        "the source gains the block's statement:\n{}",
        document.text()
    );
    added
}

/// Criterion 1: below 1440 px in Split, an operator chip in the popup adds a
/// Value block with that operator at the board's visible centre, selected,
/// with the source updated and the tab dirty, exactly as the panel's chip does.
#[test]
fn narrow_split_popup_operator_chip_adds_like_the_panel() {
    let project = empty_scenario();
    let mut harness = window(&project, NARROW_SPLIT, "Split");
    assert!(
        harness.query_by_label("Block library").is_none(),
        "no library panel below 1440 px"
    );
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    let id = active_id(&harness);
    let before = graph_of(&model(folder(&harness), &id), true).nodes.len();
    let centre = folder(&harness).canvases.get(&id).unwrap().visible_center();

    open_popup(&mut harness);
    choose(&mut harness, "A = B");

    let count = graph_of(&model(folder(&harness), &id), true).nodes.len();
    assert_eq!(count, before + 1, "the operator is added");
    let added = assert_added_like_the_panel(&harness, true);
    let operator = Operator::ALL
        .into_iter()
        .find(|operator| operator.label() == "A = B")
        .unwrap();
    let expected = create_operator_node(operator, added.position.clone());
    assert_eq!(added.kind, NodeKind::Value, "an operator is a Value block");
    assert_eq!(
        added.inputs.get(VALUE_INPUT),
        expected.inputs.get(VALUE_INPUT),
        "the Value block computes the chip's operator"
    );
    assert!(
        (added.position.x - centre.x).abs() < 1.0 && (added.position.y - centre.y).abs() < 1.0,
        "the block is at the visible centre {centre:?}, not {:?}",
        added.position
    );
}

/// Criterion 1: below 1440 px in Split, *Add variable* in the popup adds a
/// variable as the panel's does, and the popup then lists it among the
/// document's variables.
#[test]
fn narrow_split_popup_adds_and_lists_variables() {
    let project = shop();
    let mut harness = window(&project, NARROW_SPLIT, "Split");
    let id = active_id(&harness);
    let before = graph_of(&model(folder(&harness), &id), false).nodes.len();

    open_popup(&mut harness);
    choose(&mut harness, "Add variable");

    let count = graph_of(&model(folder(&harness), &id), false).nodes.len();
    assert_eq!(count, before + 1, "a variable is added");
    let added = assert_added_like_the_panel(&harness, false);
    assert!(added.is_variable(), "the added node is a variable");

    open_popup(&mut harness);
    let chip = format!("{} · variable", added.display_name());
    assert!(
        harness.query_by_label(&chip).is_some(),
        "the popup lists the document's variable {chip:?}"
    );
}

/// Criterion 2: at 1440 px in Split, and in Diagram, the library panel is in
/// view, so the popup keeps only its steps.
#[test]
fn the_popup_is_unchanged_when_the_library_panel_is_in_view() {
    let project = shop();
    for (size, mode) in [
        (egui::vec2(SPLIT_INSPECTOR_WIDTH, 900.0), "Split"),
        (WIDE, "Split"),
        (MINIMUM, "Diagram"),
    ] {
        let mut harness = window(&project, size, mode);
        assert!(
            harness.query_by_label("Search blocks").is_some(),
            "{mode} at {size:?} shows the panel"
        );
        open_popup(&mut harness);
        for label in ["Add variable", "A = B", "VARIABLES", "OPERATORS"] {
            assert!(
                harness.query_by_label(label).is_none(),
                "{mode} at {size:?}: the popup has no {label:?}"
            );
        }
    }
}

/// Criterion 1 at the minimum window: the popup and every part of its new
/// sections fit inside the window once scrolled to.
#[test]
fn the_popup_sections_fit_the_minimum_window() {
    let project = shop();
    let mut harness = window(&project, MINIMUM, "Split");
    open_popup(&mut harness);
    assert_within_window(&harness, &[POPUP]);
    for label in ["VARIABLES", "Add variable", "OPERATORS", "A = B"] {
        reveal(&mut harness, label);
        assert_within_window(&harness, &[POPUP, label]);
    }
}

/// Design review: the popup open in Split at both standard windows.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_add_step_popup_in_split() {
    let project = shop();
    for (size, name) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
        let mut harness = window(&project, size, "Split");
        open_popup(&mut harness);
        capture(&mut harness, &format!("add-step-popup-split-{name}"), None);
        if harness.query_by_label("OPERATORS").is_some() {
            reveal(&mut harness, "OPERATORS");
            capture(
                &mut harness,
                &format!("add-step-popup-split-{name}-scrolled"),
                None,
            );
        }
    }
}
