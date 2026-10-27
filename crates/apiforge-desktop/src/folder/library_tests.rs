//! KTLO-05b: the block library panel beside the board in the folder window,
//! driven through named widgets (ADR 0013). Persistence of its width
//! (criterion 6) is KTLO-05c; the Add-step popup sections (criterion 5, below
//! 1440 px) are a separate pull request; see docs/tickets/KTLO-05b.md. The
//! KTLO-17 tests at the end fold the panel to a rail (docs/tickets/KTLO-17.md).
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

/// The right edge of the block library panel: where its named resize handle is.
fn library_edge(harness: &FolderHarness) -> egui::Pos2 {
    harness.get_by_label("Resize block library").rect().center()
}

/// KTLO-05c criterion 4: the library panel's width, changed through its resize
/// handle, comes back when the project is closed and opened again.
#[test]
fn the_library_width_comes_back_after_reopening_the_project() {
    let project = shop();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    // Grab the handle on the panel's side of the edge: the board, drawn after
    // the panel, takes the pointer on the other side.
    let start = library_edge(&harness) - egui::vec2(2.0, 0.0);
    let end = start + egui::vec2(60.0, 0.0);

    harness.hover_at(start);
    harness.step();
    harness.drag_at(start);
    harness.step();
    for step in 1..=6 {
        harness.hover_at(start + (end - start) * (step as f32 / 6.0));
        harness.step();
    }
    harness.drop_at(end);
    settle(&mut harness);
    let resized = library_edge(&harness).x;
    assert!(
        (resized - end.x).abs() < 2.0,
        "the handle resizes the panel: {} -> {resized}",
        start.x
    );

    // Close the window the way the application closes it, then open again.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !harness.state_mut().as_mut().unwrap().prepare_close() {
        assert!(std::time::Instant::now() < deadline, "close did not finish");
        harness.step();
    }
    drop(harness);

    let mut reopened = folder_harness(project.path(), WIDE);
    reopened.get_by_label("Diagram").click();
    settle(&mut reopened);
    let restored = library_edge(&reopened).x;
    assert!(
        (restored - resized).abs() < 1.0,
        "the library keeps its width: its edge was at {resized}, now {restored}"
    );
}

// ----- KTLO-17: the library collapses to a rail ---------------------------------

/// The *Add step* popup's heading when it is opened from the toolbar.
const POPUP: &str = "Add a step to the scenario";
/// The narrowest Split window with the library panel beside the board.
const WIDE_SPLIT: egui::Vec2 = egui::vec2(SPLIT_INSPECTOR_WIDTH, 900.0);
/// The widest Split window without it.
const NARROW_SPLIT: egui::Vec2 = egui::vec2(SPLIT_INSPECTOR_WIDTH - 1.0, 900.0);
/// The rail's widest allowed width (criterion 1).
const RAIL: f32 = 40.0;

/// The folder window on `project` at `size`, switched to `mode`.
fn window(project: &tempfile::TempDir, size: egui::Vec2, mode: &str) -> FolderHarness {
    let mut harness = folder_harness(project.path(), size);
    harness.get_by_label(mode).click();
    settle(&mut harness);
    harness
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    settle(harness);
}

fn ctrl_b(harness: &mut FolderHarness) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::B);
    settle(harness);
}

/// The library panel is shown: its caption, search and chevron are there and
/// the rail is not.
fn assert_expanded(harness: &FolderHarness, context: &str) {
    for label in ["Block library", "Search blocks", "Collapse block library"] {
        assert!(
            harness.query_by_label(label).is_some(),
            "{context}: the expanded panel has {label:?}"
        );
    }
    assert!(
        harness.query_by_label("Show block library").is_none(),
        "{context}: no rail beside the expanded panel"
    );
}

/// The library is folded to the rail: its button is there, the panel is not.
fn assert_collapsed(harness: &FolderHarness, context: &str) {
    assert!(
        harness.query_by_label("Show block library").is_some(),
        "{context}: the rail holds *Show block library*"
    );
    for label in ["Block library", "Search blocks", "Collapse block library"] {
        assert!(
            harness.query_by_label(label).is_none(),
            "{context}: the collapsed library has no {label:?}"
        );
    }
}

/// Where the rail starts: the folder window's left edge in Diagram (inset by
/// the margin of the harness's own central panel), the code panel's edge (its
/// named resize handle) in Split.
fn rail_start(harness: &FolderHarness, mode: &str) -> f32 {
    match mode {
        "Split" => harness.get_by_label("Resize code").rect().center().x,
        _ => {
            let margin = egui::Frame::central_panel(&harness.ctx.global_style()).inner_margin;
            harness.ctx.content_rect().left() + margin.leftf()
        }
    }
}

/// Asserts that the rail is at most [`RAIL`] wide, holds its button and that
/// the board starts at its right edge.
fn assert_rail_beside_the_board(harness: &FolderHarness, mode: &str, context: &str) {
    let start = rail_start(harness, mode);
    let button = harness.get_by_label("Show block library").rect();
    let board = harness.get_by_label("Diagram board").rect();
    assert!(
        button.left() >= start - 1.0 && button.right() <= board.left() + 1.0,
        "{context}: the rail's button {button:?} lies between {start} and the board {board:?}"
    );
    assert!(
        board.left() - start <= RAIL,
        "{context}: the rail is at most {RAIL} px wide: the board starts {} px after {start}",
        board.left() - start
    );
}

/// Drags the library's resize handle by `dx`, as KTLO-05c's test does.
fn drag_library_edge(harness: &mut FolderHarness, dx: f32) {
    let start = library_edge(harness) - egui::vec2(2.0, 0.0);
    let end = start + egui::vec2(dx, 0.0);
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

/// KTLO-17 criterion 1: in Diagram the caption row holds *Collapse block
/// library*; clicking it replaces the panel by a rail at most 40 px wide with
/// *Show block library*, and the board takes the freed width.
#[test]
fn ktlo17_collapsing_the_library_leaves_a_rail_and_widens_the_board() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let context = format!("Diagram at {size:?}");
        let mut harness = window(&project, size, "Diagram");
        assert_expanded(&harness, &context);
        let caption = harness.get_by_label("Block library").rect();
        let chevron = harness.get_by_label("Collapse block library").rect();
        let edge = library_edge(&harness).x;
        assert!(
            (chevron.center().y - caption.center().y).abs() < 8.0
                && chevron.left() > caption.right()
                && chevron.right() <= edge,
            "{context}: the chevron {chevron:?} is at the right of the caption row {caption:?}, \
             inside the panel ending at {edge}"
        );
        let before = harness.get_by_label("Diagram board").rect();

        click(&mut harness, "Collapse block library");

        assert_collapsed(&harness, &context);
        assert_rail_beside_the_board(&harness, "Diagram", &context);
        let after = harness.get_by_label("Diagram board").rect();
        assert!(
            after.left() < before.left() - 150.0 && (after.right() - before.right()).abs() < 1.0,
            "{context}: the board takes the freed width: {before:?} -> {after:?}"
        );
        assert_within_window(&harness, &["Show block library"]);
    }
}

/// KTLO-17 criterion 2: *Show block library* and Ctrl+B, in either state,
/// bring the panel back at the width it had before collapsing.
#[test]
fn ktlo17_the_library_comes_back_at_its_width_by_button_and_by_ctrl_b() {
    let project = shop();
    let mut harness = window(&project, WIDE, "Diagram");
    drag_library_edge(&mut harness, 60.0);
    let width = library_edge(&harness).x;

    click(&mut harness, "Collapse block library");
    assert_collapsed(&harness, "after the chevron");
    click(&mut harness, "Show block library");
    assert_expanded(&harness, "after the rail's button");
    let restored = library_edge(&harness).x;
    assert!(
        (restored - width).abs() < 1.0,
        "the button restores the width: the edge was at {width}, now {restored}"
    );

    ctrl_b(&mut harness);
    assert_collapsed(&harness, "after Ctrl+B on the panel");
    ctrl_b(&mut harness);
    assert_expanded(&harness, "after Ctrl+B on the rail");
    let restored = library_edge(&harness).x;
    assert!(
        (restored - width).abs() < 1.0,
        "Ctrl+B restores the width: the edge was at {width}, now {restored}"
    );
}

/// KTLO-17 criterion 3: in Split at 1440 px and wider the chevron, the rail and
/// Ctrl+B behave as in Diagram, with the rail between the code and the board;
/// below 1440 px there is neither panel nor rail, and the popup is the library.
#[test]
fn ktlo17_split_collapses_when_wide_and_is_unchanged_when_narrow() {
    for size in [WIDE_SPLIT, WIDE] {
        let project = shop();
        let context = format!("Split at {size:?}");
        let mut harness = window(&project, size, "Split");
        assert_expanded(&harness, &context);
        let width = library_edge(&harness).x;
        click(&mut harness, "Collapse block library");
        assert_collapsed(&harness, &context);
        assert_rail_beside_the_board(&harness, "Split", &context);
        click(&mut harness, "Show block library");
        assert_expanded(&harness, &context);
        assert!(
            (library_edge(&harness).x - width).abs() < 1.0,
            "{context}: the panel comes back at its width"
        );
        ctrl_b(&mut harness);
        assert_collapsed(&harness, &format!("{context}, Ctrl+B"));
        ctrl_b(&mut harness);
        assert_expanded(&harness, &format!("{context}, Ctrl+B again"));
    }
    for size in [NARROW_SPLIT, MINIMUM] {
        let project = shop();
        let context = format!("Split at {size:?}");
        let mut harness = window(&project, size, "Split");
        for pressed in [false, true] {
            for label in [
                "Block library",
                "Collapse block library",
                "Show block library",
            ] {
                assert!(
                    harness.query_by_label(label).is_none(),
                    "{context} (Ctrl+B pressed: {pressed}): no {label:?}"
                );
            }
            ctrl_b(&mut harness);
        }
        click(&mut harness, "Add step");
        assert!(harness.query_by_label(POPUP).is_some(), "the popup opens");
        for label in ["VARIABLES", "Add variable", "OPERATORS", "A = B"] {
            assert!(
                harness.query_by_label(label).is_some(),
                "{context}: the popup still lists {label:?} (KTLO-05d)"
            );
        }
    }
}

/// KTLO-17 criterion 4: while the library is collapsed the *Add step* popup
/// lists Variables and Operators; once it is shown again the popup keeps only
/// its steps.
#[test]
fn ktlo17_the_popup_carries_the_library_while_it_is_collapsed() {
    let sections = ["VARIABLES", "Add variable", "OPERATORS", "A = B"];
    for (size, mode) in [(MINIMUM, "Diagram"), (WIDE, "Diagram"), (WIDE, "Split")] {
        let project = shop();
        let context = format!("{mode} at {size:?}");
        let mut harness = window(&project, size, mode);
        click(&mut harness, "Collapse block library");
        click(&mut harness, "Add step");
        assert!(harness.query_by_label(POPUP).is_some(), "the popup opens");
        for label in sections {
            assert!(
                harness.query_by_label(label).is_some(),
                "{context}, collapsed: the popup lists {label:?}"
            );
        }
        harness.key_press(Key::Escape);
        settle(&mut harness);
        assert!(harness.query_by_label(POPUP).is_none(), "Escape closes it");

        click(&mut harness, "Show block library");
        click(&mut harness, "Add step");
        assert!(harness.query_by_label(POPUP).is_some(), "the popup opens");
        for label in sections {
            assert!(
                harness.query_by_label(label).is_none(),
                "{context}, expanded: the popup has no {label:?}"
            );
        }
    }
}

/// KTLO-17 criterion 5: the empty board's hint offers only the first step while
/// the library is collapsed, and names the library while it is shown.
#[test]
fn ktlo17_the_empty_board_hint_follows_the_collapsed_library() {
    const DRAG: &str = "Drag a block from the library or add the first step.";
    const FIRST: &str = "Add the first step.";
    for (size, mode) in [(WIDE, "Diagram"), (MINIMUM, "Diagram"), (WIDE, "Split")] {
        let project = empty_scenario();
        let context = format!("{mode} at {size:?}");
        let mut harness = window(&project, size, mode);
        click(&mut harness, "Cleanup");
        assert!(
            harness.query_by_label(DRAG).is_some(),
            "{context}: expanded"
        );
        click(&mut harness, "Collapse block library");
        assert!(
            harness.query_by_label(FIRST).is_some() && harness.query_by_label(DRAG).is_none(),
            "{context}: collapsed, the hint only offers the first step"
        );
        click(&mut harness, "Show block library");
        assert!(
            harness.query_by_label(DRAG).is_some() && harness.query_by_label(FIRST).is_none(),
            "{context}: expanded again, the hint names the library"
        );
    }
}

/// KTLO-17 criterion 6 (window half; the file half is in `apiforge-storage`):
/// a project that never collapsed the library opens expanded; one closed with
/// the library collapsed reopens with the rail, in Diagram and in Split.
#[test]
fn ktlo17_the_collapsed_library_comes_back_after_reopening_the_project() {
    let project = shop();
    let mut harness = window(&project, WIDE, "Diagram");
    assert_expanded(&harness, "a view state without the flag");
    click(&mut harness, "Collapse block library");
    assert_collapsed(&harness, "before closing");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !harness.state_mut().as_mut().unwrap().prepare_close() {
        assert!(std::time::Instant::now() < deadline, "close did not finish");
        harness.step();
    }
    drop(harness);

    let mut reopened = folder_harness(project.path(), WIDE);
    settle(&mut reopened);
    assert_eq!(folder(&reopened).workbench.mode, Mode::Diagram);
    assert_collapsed(&reopened, "Diagram after reopening");
    assert_rail_beside_the_board(&reopened, "Diagram", "Diagram after reopening");
    click(&mut reopened, "Split");
    assert_collapsed(&reopened, "Split after reopening");
}

/// KTLO-17 criterion 7 (layout half; the audit is in `accessibility_tests`):
/// the chevron, then the rail, the board toolbar and the inspector are inside
/// the window at 960 × 540 in Diagram, and at 1600 × 1000 in Diagram and Split.
#[test]
fn ktlo17_the_rail_and_its_neighbours_fit_the_window() {
    let toolbar = ["Main", "Cleanup", "Add step", "Zoom in", "Zoom out"];
    let inspector = ["Close Inspector", "Resize inspector"];
    for (size, mode) in [(MINIMUM, "Diagram"), (WIDE, "Diagram"), (WIDE, "Split")] {
        let project = shop();
        let mut harness = window(&project, size, mode);
        // The inspector opens for one selected block.
        click(&mut harness, "Expected price (Value)");
        let mut labels = vec!["Collapse block library", "Block library"];
        labels.extend(toolbar);
        labels.extend(inspector);
        assert_within_window(&harness, &labels);
        click(&mut harness, "Collapse block library");
        let mut labels = vec!["Show block library"];
        labels.extend(toolbar);
        labels.extend(inspector);
        assert_within_window(&harness, &labels);
    }
}

// ----- KTLO-11: the left panel switches between Library and Files ----------------
//
// docs/tickets/KTLO-11.md; owner decisions of 2026-10-26 on #14: Diagram only
// (1 = A), the chosen tab remembered per project (2 = A), underline tabs like
// *Main | Cleanup*.

/// Whether the control named `label` shows itself as the selected one.
fn is_selected(harness: &FolderHarness, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

/// The panel shows the block library: *Library* is the selected tab and the
/// block sections are there, the explorer is not.
fn assert_on_library(harness: &FolderHarness, context: &str) {
    assert!(
        is_selected(harness, "Library") && !is_selected(harness, "Files"),
        "{context}: *Library* is the selected tab"
    );
    for label in [
        "Search blocks",
        "Blocks",
        "HTTP request",
        "Collapse block library",
    ] {
        assert!(
            harness.query_by_label(label).is_some(),
            "{context}: *Library* shows {label:?}"
        );
    }
    assert!(
        harness.query_by_label("Project files").is_none(),
        "{context}: no explorer on *Library*"
    );
}

/// The panel shows the project's files: *Files* is the selected tab and the
/// explorer with its toolbar is there, the block sections are not.
fn assert_on_files(harness: &FolderHarness, context: &str) {
    assert!(
        is_selected(harness, "Files") && !is_selected(harness, "Library"),
        "{context}: *Files* is the selected tab"
    );
    for label in [
        "Project files",
        "New",
        "Trash",
        "Refresh",
        "Collapse project files",
    ] {
        assert!(
            harness.query_by_label(label).is_some(),
            "{context}: *Files* shows {label:?}"
        );
    }
    assert!(
        harness.query_by_label("Search blocks").is_none(),
        "{context}: no block library on *Files*"
    );
}

/// The folder window on `project` at `size` in Diagram with *Files* chosen.
fn files_window(project: &tempfile::TempDir, size: egui::Vec2) -> FolderHarness {
    let mut harness = window(project, size, "Diagram");
    click(&mut harness, "Files");
    harness
}

/// The names of the explorer's rows, top to bottom, as the tree shows them.
fn explorer_labels(harness: &FolderHarness) -> Vec<String> {
    let tree = harness.get_by_label("Project files").rect();
    harness
        .query_all_by(|node| {
            node.role() != egui::accesskit::Role::Tree
                && node.label().is_some()
                && node.bounding_box().is_some()
        })
        .filter(|node| tree.contains(node.rect().center()))
        .filter_map(|node| node.accesskit_node().label())
        .collect()
}

/// Clicks the explorer row named `name` (a tab or the board header may carry
/// the same name).
fn click_row(harness: &mut FolderHarness, name: &str) {
    let tree = harness.get_by_label("Project files").rect();
    harness
        .query_all_by_label(name)
        .find(|node| tree.contains(node.rect().center()))
        .unwrap_or_else(|| panic!("no explorer row {name:?}"))
        .click();
    settle(harness);
}

/// The project path of the active tab's file, document or text.
fn active_path(harness: &FolderHarness) -> String {
    let app = folder(harness);
    let tab = app.workbench.active().expect("an active tab");
    match app.text_of(tab) {
        Some(path) => path.to_owned(),
        None => app
            .session
            .as_ref()
            .unwrap()
            .document(tab)
            .unwrap()
            .entry()
            .path
            .as_str()
            .to_owned(),
    }
}

/// KTLO-11 criterion 1: in Diagram the panel's caption row holds *Library* and
/// *Files*; *Library* shows the block sections, *Files* the explorer with the
/// same rows as Code for the same project. Split keeps its library (1 = A).
#[test]
fn ktlo11_the_panel_has_library_and_files_tabs_with_the_rows_of_code() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let context = format!("Diagram at {size:?}");
        let mut harness = window(&project, size, "Code");
        let code_rows = explorer_labels(&harness);
        assert!(
            code_rows.iter().any(|row| row == "Demo shop"),
            "Code lists the project: {code_rows:?}"
        );

        click(&mut harness, "Diagram");
        assert_on_library(&harness, &context);
        let library = harness.get_by_label("Library").rect();
        let files = harness.get_by_label("Files").rect();
        let chevron = harness.get_by_label("Collapse block library").rect();
        assert!(
            (library.center().y - chevron.center().y).abs() < 8.0
                && (files.center().y - chevron.center().y).abs() < 8.0
                && library.right() <= files.left()
                && files.right() < chevron.left(),
            "{context}: *Library* {library:?} and *Files* {files:?} are on the caption row, \
             left of the chevron {chevron:?}"
        );
        let search = harness.get_by_label("Search blocks").rect();
        assert!(
            files.bottom() <= search.top(),
            "{context}: the tabs are above the library's search {search:?}"
        );

        click(&mut harness, "Files");
        assert_on_files(&harness, &context);
        assert_eq!(folder(&harness).workbench.mode, Mode::Diagram);
        assert_eq!(
            explorer_labels(&harness),
            code_rows,
            "{context}: *Files* lists the rows of Code"
        );
        let tree = harness.get_by_label("Project files").rect();
        let board = harness.get_by_label("Diagram board").rect();
        assert!(
            tree.right() <= board.left() + 1.0,
            "{context}: the explorer {tree:?} is left of the board {board:?}"
        );

        click(&mut harness, "Library");
        assert_on_library(&harness, &context);
    }
    // Decision 1 = A: Split's library panel has no tabs.
    let project = shop();
    let harness = window(&project, WIDE, "Split");
    assert!(harness.query_by_label("Block library").is_some());
    for label in ["Library", "Files"] {
        assert!(
            harness.query_by_label(label).is_none(),
            "Split has no {label:?} tab"
        );
    }
}

/// KTLO-11 criterion 2: on *Files*, a component and a scenario open in their
/// tab, the board shows them and the mode stays Diagram; a text file opens its
/// text tab, which has no diagram.
#[test]
fn ktlo11_files_opens_documents_and_text_without_leaving_diagram() {
    const NO_DIAGRAM: &str =
        "README.md is a text file, not a scenario or component, so it has no diagram.";
    for size in [MINIMUM, WIDE] {
        let project = shop();
        std::fs::write(project.path().join("README.md"), "# Shop\n").unwrap();
        let context = format!("Diagram at {size:?}");
        let mut harness = files_window(&project, size);
        let scenario = active_id(&harness);

        click_row(&mut harness, "components");
        click_row(&mut harness, "product.afs");
        assert_eq!(folder(&harness).workbench.mode, Mode::Diagram, "{context}");
        assert_eq!(active_path(&harness), "components/product.afs", "{context}");
        let component = active_id(&harness);
        assert_ne!(component, scenario);
        assert!(
            folder(&harness).canvases.contains_key(&component),
            "{context}: the board shows the component"
        );
        assert!(harness.query_by_label("Diagram board").is_some());
        assert_on_files(&harness, &format!("{context}, after a component"));

        click_row(&mut harness, "promotion-and-cart-price.afs");
        assert_eq!(folder(&harness).workbench.mode, Mode::Diagram, "{context}");
        assert_eq!(
            active_id(&harness),
            scenario,
            "{context}: the scenario's tab"
        );
        assert_eq!(
            folder(&harness).workbench.tabs().len(),
            2,
            "{context}: the scenario keeps its one tab"
        );

        click_row(&mut harness, "README.md");
        assert_eq!(folder(&harness).workbench.mode, Mode::Diagram, "{context}");
        assert_eq!(
            active_path(&harness),
            "README.md",
            "{context}: the text tab"
        );
        assert!(
            harness.query_by_label(NO_DIAGRAM).is_some(),
            "{context}: the Diagram area says the text file has no diagram"
        );
        assert_on_files(&harness, &format!("{context}, after a text file"));
    }
}

/// KTLO-11 criterion 3 (toolbar and Delete key; the click and keyboard tests of
/// the explorer in `tests.rs` run in both modes): *New*, *Trash*, *Refresh*
/// and Delete do on *Files* what they do in Code, and the explorer's keys do
/// not reach the board's selected block.
#[test]
fn ktlo11_the_explorer_on_files_does_what_it_does_in_code() {
    for size in [MINIMUM, WIDE] {
        for mode in ["Code", "Diagram"] {
            let project = shop();
            let context = format!("{mode} at {size:?}");
            let mut harness = window(&project, size, mode);
            let id = active_id(&harness);
            let blocks = node_count(&harness, &id);
            if mode == "Diagram" {
                // A selected block, which the board's own Delete would remove.
                click(&mut harness, "Expected price (Value)");
                assert!(
                    !folder(&harness)
                        .selections
                        .get(&id)
                        .cloned()
                        .unwrap_or_default()
                        .is_empty(),
                    "{context}: a block is selected"
                );
                click(&mut harness, "Files");
            }

            // + offers new items in the selected folder.
            click_row(&mut harness, "components");
            click(&mut harness, "New");
            click(&mut harness, "New scenario");
            assert!(
                matches!(
                    &folder(&harness).prompt,
                    Some(Prompt::NewDocument { kind: DocumentKind::Scenario, folder: Some(into), .. })
                        if into == "components"
                ),
                "{context}: *New scenario* asks for a name in components"
            );
            harness.key_press(Key::Escape);
            settle(&mut harness);
            assert!(
                folder(&harness).prompt.is_none(),
                "{context}: Escape cancels"
            );

            // The trash.
            click(&mut harness, "Trash");
            assert!(
                matches!(&folder(&harness).prompt, Some(Prompt::Trash { .. })),
                "{context}: *Trash* opens the trash"
            );
            harness.key_press(Key::Escape);
            settle(&mut harness);

            // Refresh lists a file written outside the window.
            std::fs::write(project.path().join("notes.txt"), "notes\n").unwrap();
            click(&mut harness, "Refresh");
            assert!(
                explorer_labels(&harness)
                    .iter()
                    .any(|row| row == "notes.txt"),
                "{context}: *Refresh* lists the new file"
            );

            // Delete on the project row does nothing; Backspace does nothing;
            // Delete on a file asks to delete that file.
            click_row(&mut harness, "Demo shop");
            harness.key_press(Key::Delete);
            harness.key_press(Key::Backspace);
            settle(&mut harness);
            assert!(
                folder(&harness).prompt.is_none(),
                "{context}: the project row is not deleted"
            );
            click_row(&mut harness, "product.afs");
            harness.key_press(Key::Delete);
            settle(&mut harness);
            assert!(
                matches!(
                    &folder(&harness).prompt,
                    Some(Prompt::Delete { paths, .. }) if paths == &["components/product.afs".to_owned()]
                ),
                "{context}: Delete asks to delete the selected file"
            );
            harness.key_press(Key::Escape);
            settle(&mut harness);
            assert_eq!(
                node_count(&harness, &id),
                blocks,
                "{context}: no block left the board"
            );
            assert!(project.path().join("components/product.afs").exists());
        }
    }
}

/// KTLO-11 criterion 4: the rail holds *Show block library* and *Show project
/// files*; each expands the panel on its tab, and the chosen tab is kept
/// through the chevron and Ctrl+B. Split's rail keeps its one button.
#[test]
fn ktlo11_the_rail_brings_the_panel_back_on_either_tab() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let context = format!("Diagram at {size:?}");
        let mut harness = files_window(&project, size);

        click(&mut harness, "Collapse project files");
        for label in ["Show block library", "Show project files"] {
            assert!(
                harness.query_by_label(label).is_some(),
                "{context}: the rail holds {label:?}"
            );
        }
        for label in ["Library", "Files", "Project files", "Search blocks"] {
            assert!(
                harness.query_by_label(label).is_none(),
                "{context}: the collapsed panel has no {label:?}"
            );
        }
        assert_rail_beside_the_board(&harness, "Diagram", &context);
        let files = harness.get_by_label("Show project files").rect();
        let board = harness.get_by_label("Diagram board").rect();
        assert!(
            files.right() <= board.left() + 1.0,
            "{context}: *Show project files* {files:?} is on the rail"
        );
        assert_within_window(&harness, &["Show block library", "Show project files"]);

        click(&mut harness, "Show block library");
        assert_on_library(&harness, &format!("{context}, from the rail"));
        click(&mut harness, "Collapse block library");
        click(&mut harness, "Show project files");
        assert_on_files(&harness, &format!("{context}, from the rail"));

        ctrl_b(&mut harness);
        assert!(
            harness.query_by_label("Show project files").is_some(),
            "{context}: Ctrl+B folds the panel"
        );
        ctrl_b(&mut harness);
        assert_on_files(&harness, &format!("{context}, Ctrl+B keeps the tab"));
    }
    let project = shop();
    let mut harness = window(&project, WIDE, "Split");
    click(&mut harness, "Collapse block library");
    assert!(harness.query_by_label("Show block library").is_some());
    assert!(
        harness.query_by_label("Show project files").is_none(),
        "Split's rail is unchanged (decision 1 = A)"
    );
}

/// KTLO-11 criterion 5: while *Files* is shown, the explorer's selection
/// follows the tab chosen in the tab strip.
#[test]
fn ktlo11_the_selection_follows_the_tab_strip_on_files() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let context = format!("Diagram at {size:?}");
        let mut harness = files_window(&project, size);
        click_row(&mut harness, "components");
        click_row(&mut harness, "product.afs");
        let scenario = "promotion-and-cart-price.afs";
        // The scenario's tab in the strip: the one beside its close button.
        let close = harness.get_by_label(&format!("Close {scenario}")).rect();
        harness
            .query_all_by_label(scenario)
            .find(|node| (node.rect().center().y - close.center().y).abs() < 4.0)
            .expect("the scenario's tab in the strip")
            .click();
        settle(&mut harness);

        assert_eq!(active_path(&harness), SCENARIO, "{context}");
        assert_eq!(
            folder(&harness).explorer.selected.as_deref(),
            Some(SCENARIO),
            "{context}: the explorer selects the active file"
        );
        let tree = harness.get_by_label("Project files").rect();
        let row = harness
            .query_all_by_label(scenario)
            .find(|node| tree.contains(node.rect().center()))
            .expect("the scenario's row");
        assert_eq!(
            row.accesskit_node().toggled(),
            Some(egui::accesskit::Toggled::True),
            "{context}: the scenario's row shows as selected"
        );
    }
}

/// KTLO-11 criterion 6 (window half; the file half is in `apiforge-storage`):
/// a project closed on *Files* reopens on *Files*, also behind the rail; one
/// that never chose it opens on *Library*.
#[test]
fn ktlo11_the_chosen_tab_comes_back_after_reopening_the_project() {
    let close = |mut harness: FolderHarness| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !harness.state_mut().as_mut().unwrap().prepare_close() {
            assert!(std::time::Instant::now() < deadline, "close did not finish");
            harness.step();
        }
    };
    let project = shop();
    let mut harness = window(&project, WIDE, "Diagram");
    assert_on_library(&harness, "a view state without the field");
    click(&mut harness, "Files");
    close(harness);

    let mut reopened = folder_harness(project.path(), WIDE);
    settle(&mut reopened);
    assert_eq!(folder(&reopened).workbench.mode, Mode::Diagram);
    assert_on_files(&reopened, "reopened after *Files*");

    click(&mut reopened, "Collapse project files");
    close(reopened);
    let mut reopened = folder_harness(project.path(), WIDE);
    settle(&mut reopened);
    assert!(
        reopened.query_by_label("Show project files").is_some(),
        "reopened behind the rail"
    );
    click(&mut reopened, "Show project files");
    assert_on_files(&reopened, "the rail's *Files* after reopening");
    click(&mut reopened, "Library");
    close(reopened);

    let reopened = folder_harness(project.path(), WIDE);
    assert_on_library(&reopened, "reopened after *Library*");
}

/// KTLO-11 criterion 7 (layout half; the audit is in `accessibility_tests`):
/// at 960 × 540 every control of the panel on *Files*, the board toolbar and
/// the inspector are inside the window.
#[test]
fn ktlo11_the_files_tab_and_its_neighbours_fit_the_window() {
    let toolbar = ["Main", "Cleanup", "Add step", "Zoom in", "Zoom out"];
    let inspector = ["Close Inspector", "Resize inspector"];
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let mut harness = window(&project, size, "Diagram");
        // The inspector opens for one selected block.
        click(&mut harness, "Expected price (Value)");
        click(&mut harness, "Files");
        let mut labels = vec![
            "Library",
            "Files",
            "Collapse project files",
            "New",
            "Trash",
            "Refresh",
            "Resize project files",
        ];
        labels.extend(toolbar);
        labels.extend(inspector);
        assert_within_window(&harness, &labels);
        let tree = harness.get_by_label("Project files").rect();
        let window = harness.ctx.content_rect();
        assert!(
            tree.top() + 48.0 < window.bottom() && tree.left() >= window.left(),
            "{size:?}: the rows start inside the window: {tree:?}"
        );
        click(&mut harness, "Collapse project files");
        let mut labels = vec!["Show block library", "Show project files"];
        labels.extend(toolbar);
        labels.extend(inspector);
        assert_within_window(&harness, &labels);
    }
}

/// KTLO-11 *Changed*: in Diagram the panel widens to 420 px, one width for
/// both tabs; Split keeps the library's 320 px and does not shrink the
/// Diagram width it shows.
#[test]
fn ktlo11_the_panel_widens_to_420_in_diagram_only() {
    let project = shop();
    let mut harness = window(&project, WIDE, "Diagram");
    let start = rail_start(&harness, "Diagram");
    drag_library_edge(&mut harness, 300.0);
    let width = library_edge(&harness).x - start;
    assert!(
        (width - 420.0).abs() < 2.0,
        "Diagram's panel reaches 420 px, not more: {width}"
    );
    click(&mut harness, "Files");
    let files = harness
        .get_by_label("Resize project files")
        .rect()
        .center()
        .x
        - start;
    assert!(
        (files - width).abs() < 1.0,
        "one width for both tabs: {width} and {files}"
    );
    click(&mut harness, "Library");

    click(&mut harness, "Split");
    let split = library_edge(&harness).x - rail_start(&harness, "Split");
    assert!(
        split <= 321.0,
        "Split keeps the library's 320 px range: {split}"
    );
    click(&mut harness, "Diagram");
    let back = library_edge(&harness).x - start;
    assert!(
        (back - width).abs() < 1.0,
        "Diagram keeps its width after Split: {width} -> {back}"
    );
}
