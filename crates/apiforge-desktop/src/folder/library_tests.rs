//! KTLO-05b: the block library panel beside the board in the folder window,
//! driven through named widgets (ADR 0013). Persistence (criterion 6) and the
//! Add-step popup sections (criterion 5, below 1440 px) are separate pull
//! requests; see docs/tickets/KTLO-05b.md.
use super::*;
use crate::testing::{MINIMUM, WIDE, assert_within_window, folder, folder_harness, settle};
use egui_kittest::kittest::Queryable;

/// A project whose one scenario has an empty cleanup board, so the board shows
/// its empty state when the Cleanup phase is in view.
fn empty_scenario() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    apiforge_project::prepare_project(directory.path(), &apiforge_core::demos::empty_project())
        .unwrap();
    directory
}

fn active_id(harness: &crate::testing::FolderHarness) -> String {
    folder(harness).workbench.active().unwrap().to_owned()
}

fn node_count(harness: &crate::testing::FolderHarness, id: &str) -> usize {
    graph_of(&model(folder(harness), id), false).nodes.len()
}

/// Criterion 1: Diagram shows a panel named "Block library" with search, the
/// Blocks / Variables / Operators sections and the project's components.
#[test]
fn diagram_shows_a_named_block_library_panel_with_its_sections() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    for label in [
        "Block library",
        "Blocks",
        "Variables",
        "Operators",
        "HTTP request",
    ] {
        assert!(
            harness.query_by_label(label).is_some(),
            "the library panel is missing {label:?}"
        );
    }
}

/// Criterion 2: clicking a block adds it at the board's visible centre,
/// selected; the source gains the statement and the tab becomes dirty. The
/// empty Cleanup board keeps the centre free, so the placement is exact.
#[test]
fn clicking_a_block_adds_it_at_the_visible_centre_selected_and_dirty() {
    let project = empty_scenario();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    let id = active_id(&harness);
    let before = graph_of(&model(folder(&harness), &id), true).nodes.len();
    let centre = folder(&harness).canvases.get(&id).unwrap().visible_center();

    harness.get_by_label("HTTP request").click();
    settle(&mut harness);

    let model = model(folder(&harness), &id);
    let graph = graph_of(&model, true);
    assert_eq!(graph.nodes.len(), before + 1, "one node is added");
    let added = graph.nodes.last().unwrap();
    let selection = folder(&harness)
        .selections
        .get(&id)
        .cloned()
        .unwrap_or_default();
    assert!(
        selection.contains(&added.id),
        "the new block is selected: {selection:?}"
    );
    assert!(
        (added.position.x - centre.x).abs() < 1.0 && (added.position.y - centre.y).abs() < 1.0,
        "the block is at the visible centre {centre:?}, not {:?}",
        added.position
    );
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&id)
            .unwrap()
            .needs_recovery(),
        "the tab is dirty after an edit"
    );
}

/// Criterion 4: an operator chip adds a Value block with that operator.
#[test]
fn an_operator_chip_adds_a_value_block() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    let id = active_id(&harness);
    let before = node_count(&harness, &id);

    harness.get_by_label("Operators").click();
    settle(&mut harness);
    harness.get_by_label("A = B").click();
    settle(&mut harness);

    assert_eq!(
        node_count(&harness, &id),
        before + 1,
        "the operator is added"
    );
}

/// Criterion 4: *Add variable* adds a variable to the document's graph.
#[test]
fn add_variable_adds_a_variable_node() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    let id = active_id(&harness);
    let before = node_count(&harness, &id);

    harness.get_by_label("Variables").click();
    settle(&mut harness);
    harness.get_by_label("Add variable").click();
    settle(&mut harness);

    assert_eq!(node_count(&harness, &id), before + 1, "a variable is added");
    let model = model(folder(&harness), &id);
    assert!(
        graph_of(&model, false).nodes.last().unwrap().is_variable(),
        "the added node is a variable"
    );
}

/// Criterion 5 (panel half): Split shows the library at 1440 px and wider and
/// not below it, where the Add-step popup takes over instead.
#[test]
fn split_shows_the_library_only_when_wide_enough() {
    let project = shop();

    let mut wide = folder_harness(project.path(), egui::vec2(1600.0, 1000.0));
    wide.get_by_label("Split").click();
    settle(&mut wide);
    assert!(
        wide.query_by_label("Block library").is_some(),
        "wide Split shows the library panel"
    );

    let mut narrow = folder_harness(project.path(), egui::vec2(1200.0, 800.0));
    narrow.get_by_label("Split").click();
    settle(&mut narrow);
    assert!(
        narrow.query_by_label("Block library").is_none(),
        "narrow Split has no room for the library panel"
    );
}

/// Criterion 8: at the minimum window every library control is inside it,
/// alongside the board and inspector.
#[test]
fn the_library_fits_the_minimum_window_in_diagram() {
    let project = shop();
    for size in [MINIMUM, WIDE] {
        let mut harness = folder_harness(project.path(), size);
        harness.get_by_label("Diagram").click();
        settle(&mut harness);
        assert_within_window(
            &harness,
            &["Block library", "Blocks", "Variables", "Operators"],
        );
    }
}

/// Criterion 9: the empty board's hint names the library only when it is in
/// view, otherwise it only offers the first step.
#[test]
fn the_empty_board_hint_depends_on_the_library_being_in_view() {
    let project = empty_scenario();

    // The Cleanup phase of the demo scenario starts empty, so the board shows
    // its empty-state hint.
    let mut diagram = folder_harness(project.path(), WIDE);
    diagram.get_by_label("Diagram").click();
    settle(&mut diagram);
    diagram.get_by_label("Cleanup").click();
    settle(&mut diagram);
    assert!(
        diagram
            .query_by_label("Drag a block from the library or add the first step.")
            .is_some(),
        "Diagram shows the library in view, so the hint mentions it"
    );

    let mut narrow = folder_harness(project.path(), egui::vec2(1200.0, 800.0));
    narrow.get_by_label("Split").click();
    settle(&mut narrow);
    narrow.get_by_label("Cleanup").click();
    settle(&mut narrow);
    assert!(
        narrow.query_by_label("Add the first step.").is_some(),
        "narrow Split has no library panel, so the hint only offers the first step"
    );
}
