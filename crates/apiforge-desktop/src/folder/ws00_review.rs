//! WS-00: the design review views of the workspace mock-up (#240). Throwaway
//! branch `mock/ws-00`: the switches in `crate::mock_ws00` change the real
//! sidebar, Inspector and *Add step* popup for these views only.
use super::*;
use crate::mock_ws00::{self as mock, Flags, Sidebar};
use crate::testing::{FolderHarness, MINIMUM, WIDE, capture, folder, folder_harness, settle};
use egui_kittest::kittest::Queryable;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

fn app(harness: &mut FolderHarness) -> &mut FolderApp {
    harness.state_mut().as_mut().unwrap()
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
}

/// The demo shop at `size` with the scenario open in `mode`, under `flags`.
fn window(size: egui::Vec2, mode: &str, flags: Flags) -> (tempfile::TempDir, FolderHarness) {
    mock::set(flags);
    let project = crate::testing::shop_project();
    let mut harness = folder_harness(project.path(), size);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    click(&mut harness, mode);
    (project, harness)
}

/// Selects the card named `name` on the active board, so the Inspector opens.
fn select(harness: &mut FolderHarness, name: &str) {
    let app = app(harness);
    let document_id = app.workbench.active().unwrap().to_owned();
    let model = app.snapshot(&document_id).unwrap().model.unwrap();
    let node = graph_of(&model, false)
        .nodes
        .iter()
        .find(|node| node.display_name() == name)
        .unwrap_or_else(|| panic!("no card {name}"))
        .id
        .clone();
    app.selections.insert(document_id, vec![node]);
    settle(harness);
}

/// Runs the scenario in simulation, so Run results opens at its default height.
fn run(harness: &mut FolderHarness) {
    click(harness, "Run simulation");
    let deadline = Instant::now() + Duration::from_secs(30);
    while harness.query_by_label("Cancel run").is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
}

fn board_width(harness: &FolderHarness) -> f32 {
    harness.get_by_label("Diagram board").rect().width()
}

fn shot(harness: &mut FolderHarness, name: &str) {
    settle(harness);
    let board = harness
        .query_by_label("Diagram board")
        .map(|node| node.rect().size());
    eprintln!("WS00 {name}: board {board:?}");
    capture(harness, name, None);
}

/// Q1: one sidebar with *Files* | *Library* in Code (A: Library disabled,
/// B: Library hidden) and in Split, and folded to the rail in Split.
#[test]
#[ignore = "design-review view"]
fn review_ws00_sidebar() {
    let (_project, mut harness) = window(
        WIDE,
        "Code",
        Flags {
            sidebar: Sidebar::LibraryDisabledInCode,
            ..Flags::default()
        },
    );
    click(&mut harness, "Library");
    shot(&mut harness, "ws00-sidebar-code-a");
    let (_project, mut harness) = window(
        WIDE,
        "Code",
        Flags {
            sidebar: Sidebar::LibraryHiddenInCode,
            ..Flags::default()
        },
    );
    shot(&mut harness, "ws00-sidebar-code-b");
    let (_project, mut harness) = window(
        WIDE,
        "Split",
        Flags {
            sidebar: Sidebar::LibraryDisabledInCode,
            ..Flags::default()
        },
    );
    click(&mut harness, "Files");
    shot(&mut harness, "ws00-sidebar-split");
    click(&mut harness, "Collapse project files");
    shot(&mut harness, "ws00-sidebar-rail-split");
}

/// Q2: Split at 1600 × 1000 with the Inspector open, as today and with the
/// sidebar folded to its rail while the Inspector is open.
#[test]
#[ignore = "design-review view"]
fn review_ws00_split_inspector() {
    for (name, flags) in [
        ("ws00-split-inspector-today", Flags::default()),
        (
            "ws00-split-inspector-proposed",
            Flags {
                sidebar: Sidebar::LibraryDisabledInCode,
                fold_for_inspector: true,
                ..Flags::default()
            },
        ),
    ] {
        let (_project, mut harness) = window(WIDE, "Split", flags);
        select(&mut harness, "Add to cart");
        click(&mut harness, "Fit the view");
        shot(&mut harness, name);
        eprintln!("WS00 {name}: board width {}", board_width(&harness));
    }
}

/// Q3: Diagram at 960 × 540 with Run results and the Inspector open.
#[test]
#[ignore = "design-review view"]
fn review_ws00_minimum_window() {
    for (name, flags) in [
        ("ws00-min-today", Flags::default()),
        (
            "ws00-min-a",
            Flags {
                fold_for_board: true,
                ..Flags::default()
            },
        ),
        (
            "ws00-min-b",
            Flags {
                narrow_inspector: true,
                ..Flags::default()
            },
        ),
        (
            "ws00-min-c",
            Flags {
                fold_for_board: true,
                narrow_inspector: true,
                ..Flags::default()
            },
        ),
    ] {
        let (_project, mut harness) = window(MINIMUM, "Diagram", flags);
        run(&mut harness);
        select(&mut harness, "Add to cart");
        click(&mut harness, "Fit the view");
        shot(&mut harness, name);
        eprintln!("WS00 {name}: board width {}", board_width(&harness));
    }
}

/// Q4: the *Add step* popup in Split at 960 × 540, as today and capped at the
/// board's height minus 24 px with its list scrolling.
#[test]
#[ignore = "design-review view"]
fn review_ws00_add_step_popup() {
    for (name, flags) in [
        ("ws00-addstep-540-today", Flags::default()),
        (
            "ws00-addstep-540-proposed",
            Flags {
                capped_popup: true,
                ..Flags::default()
            },
        ),
    ] {
        let (_project, mut harness) = window(MINIMUM, "Split", flags);
        click(&mut harness, "Add step");
        assert!(folder(&harness).is_analysed());
        shot(&mut harness, name);
    }
}
