//! KTLO-05b: the block library panel beside the board in the folder window,
//! driven through named widgets (ADR 0013). Persistence (criterion 6) and the
//! Add-step popup sections (criterion 5, below 1440 px) are separate pull
//! requests; see docs/tickets/KTLO-05b.md.
use super::project_interaction_tests::type_into;
use super::*;
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_within_window, folder, folder_harness, settle,
};
use apiforge_core::{
    graph::{VALUE_INPUT, create_operator_node},
    model::NodeKind,
    operator::Operator,
};
use egui_kittest::kittest::{NodeT, Queryable};

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
        "Search blocks",
        "HTTP request",
        "PROJECT COMPONENTS",
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
    let document = folder(&harness)
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
    let model = model(folder(&harness), &id);
    let added = graph_of(&model, false).nodes.last().unwrap().clone();
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
    let added = graph_of(&model, false).nodes.last().unwrap();
    assert!(added.is_variable(), "the added node is a variable");
    let chip = format!("{} · variable", added.display_name());
    assert!(
        harness.query_by_label(&chip).is_some(),
        "Variables lists the document's variable {chip:?}"
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

    for (width, shown) in [
        (SPLIT_INSPECTOR_WIDTH, true),
        (SPLIT_INSPECTOR_WIDTH - 1.0, false),
    ] {
        let mut harness = folder_harness(project.path(), egui::vec2(width, 900.0));
        harness.get_by_label("Split").click();
        settle(&mut harness);
        assert_eq!(
            harness.query_by_label("Block library").is_some(),
            shown,
            "Split at {width} px"
        );
    }
}

/// Criterion 8: at the minimum window every library control is inside it,
/// alongside the board toolbar and the inspector, in Diagram and in Split.
#[test]
fn the_library_fits_the_minimum_window_in_diagram() {
    let project = shop();
    for size in [MINIMUM, WIDE] {
        let mut harness = folder_harness(project.path(), size);
        harness.get_by_label("Diagram").click();
        settle(&mut harness);
        // The inspector opens for one selected block.
        harness.get_by_label("Expected price (Value)").click();
        settle(&mut harness);
        assert_within_window(
            &harness,
            &[
                "Block library",
                "Search blocks",
                "Blocks",
                "Variables",
                "Operators",
                "HTTP request",
                "Resize block library",
                "Main",
                "Cleanup",
                "Add step",
                "Zoom in",
                "Zoom out",
                "Close Inspector",
                "Resize inspector",
            ],
        );
        harness.get_by_label("Split").click();
        settle(&mut harness);
        let library = size.x >= SPLIT_INSPECTOR_WIDTH;
        assert_eq!(
            harness.query_by_label("Block library").is_some(),
            library,
            "Split at {size:?}"
        );
        let mut labels = vec!["Main", "Cleanup", "Add step", "Zoom in", "Zoom out"];
        if library {
            labels.extend(["Block library", "Search blocks", "HTTP request"]);
        }
        assert_within_window(&harness, &labels);
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

/// The accessible name of the active document's editor.
fn editor(harness: &FolderHarness) -> String {
    let id = active_id(harness);
    format!(
        "Contents of {}",
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .document(&id)
            .unwrap()
            .entry()
            .path
            .as_str()
    )
}

/// Criterion 3: dragging a block onto the board adds it at the drop point.
#[test]
fn dragging_a_block_onto_the_board_adds_it_at_the_drop_point() {
    let project = empty_scenario();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    let id = active_id(&harness);
    let before = graph_of(&model(folder(&harness), &id), true).nodes.len();
    let start = harness.get_by_label("HTTP request").rect().center();
    let board = harness.get_by_label("Diagram board").rect();
    let drop = board.center() + egui::vec2(-120.0, 90.0);
    let expected = folder(&harness)
        .canvases
        .get(&id)
        .unwrap()
        .debug_board_position(drop);

    harness.hover_at(start);
    harness.step();
    harness.drag_at(start);
    harness.step();
    for step in 1..=8 {
        harness.hover_at(start + (drop - start) * (step as f32 / 8.0));
        harness.step();
    }
    harness.drop_at(drop);
    settle(&mut harness);

    let model = model(folder(&harness), &id);
    let graph = graph_of(&model, true);
    assert_eq!(graph.nodes.len(), before + 1, "one node is added");
    let added = graph.nodes.last().unwrap();
    assert!(
        (added.position.x - expected.x).abs() < 1.0 && (added.position.y - expected.y).abs() < 1.0,
        "the block is at the drop point {expected:?}, not {:?}",
        added.position
    );
}

/// Criterion 7: a read-only board (here an invalid source) disables the panel.
#[test]
fn an_invalid_source_disables_the_library() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Split").click();
    settle(&mut harness);
    assert!(
        !harness
            .get_by_label("HTTP request")
            .accesskit_node()
            .is_disabled(),
        "an editable board accepts blocks"
    );
    let editor = editor(&harness);
    type_into(&mut harness, &editor, "scenario {{{\n");
    settle(&mut harness);
    assert!(
        harness
            .get_by_label("HTTP request")
            .accesskit_node()
            .is_disabled(),
        "the library is disabled while the board is read-only"
    );
}

/// Review of KTLO-05b: Cleanup steps may use the main graph's results, so the
/// Cleanup board's library lists the main graph's variables too, as the legacy
/// window does.
#[test]
fn cleanup_lists_the_main_graphs_variables() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    harness.get_by_label("Variables").click();
    settle(&mut harness);
    harness.get_by_label("Add variable").click();
    settle(&mut harness);
    let id = active_id(&harness);
    let model = model(folder(&harness), &id);
    let chip = format!(
        "{} · variable",
        graph_of(&model, false).nodes.last().unwrap().display_name()
    );

    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    assert!(
        harness.query_by_label(&chip).is_some(),
        "Cleanup lists the main graph's variable {chip:?}"
    );
}

/// Review of KTLO-05b: an operator group's header says whether it is open.
#[test]
fn operator_group_headers_expose_their_expanded_state() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    harness.get_by_label("Operators").click();
    settle(&mut harness);
    let expanded = |harness: &FolderHarness| {
        harness
            .get_by_label("Maths")
            .accesskit_node()
            .data()
            .is_expanded()
    };
    let before = expanded(&harness);
    assert!(before.is_some(), "the header reports its state");
    harness.get_by_label("Maths").click();
    settle(&mut harness);
    assert_eq!(
        expanded(&harness),
        before.map(|open| !open),
        "the state follows a click"
    );
}
