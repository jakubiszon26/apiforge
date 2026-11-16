//! LAY-33 (#393): a scenario opened for the first time has the shape Tidy up
//! gives it. A clean demo shop holds the LAY-01 fixture `shop-30.afs` with no
//! stored layout, *Compact cards* on and checks collapsed, and the scenario
//! reaches the window by each path a person has: registered before the
//! window opens the folder, and registered through *Register in project* in
//! an open window. Either way Tidy up right after opening moves no card.
//!
//! A scenario whose layout a session without a window saved (MCP, the
//! command line) is laid out by Tidy up on its first open in the window
//! (owner decision of 2026-11-16), and the window saves that layout; a
//! layout the window made or a person changed is kept.
use super::*;
use crate::canvas::Command;
use crate::testing::{MINIMUM, folder_harness_in};
use apiforge_core::format::Position;
use std::collections::BTreeMap;

const SHOP_30: &str = "shop-30.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";

fn fixture() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-30.afs");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// Registers `shop-30.afs` in the clean demo shop at `root` from a session
/// without a window, which places and saves its layout with the uniform
/// placer of MCP and the command line.
fn register_without_a_window(root: &std::path::Path) {
    fs::write(root.join(SHOP_30), fixture()).unwrap();
    let mut session = apiforge_project::ProjectSession::open(root).unwrap();
    session
        .register_existing(
            &[apiforge_project::Registration {
                path: SHOP_30.into(),
                kind: DocumentKind::Scenario,
            }],
            apiforge_project::Origin::SESSION,
        )
        .unwrap();
}

/// The main board of `shop-30.afs` as the saved files hold it, read by a
/// session without a window.
fn saved_positions(root: &std::path::Path) -> BTreeMap<String, (i64, i64)> {
    let session = apiforge_project::ProjectSession::open(root).unwrap();
    let document = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == SHOP_30)
        .expect("shop-30.afs is registered");
    rounded(graph_of(document.model().expect("a valid model"), false))
}

fn rounded(graph: &Graph) -> BTreeMap<String, (i64, i64)> {
    graph
        .nodes
        .iter()
        .map(|node| {
            (
                node.handle.clone(),
                (
                    node.position.x.round() as i64,
                    node.position.y.round() as i64,
                ),
            )
        })
        .collect()
}

/// The id of the registered `shop-30.afs`.
fn shop_30(harness: &FolderHarness) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == SHOP_30)
        .expect("shop-30.afs is registered")
        .id()
        .to_owned()
}

fn model(harness: &FolderHarness, id: &str) -> Document {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .model()
        .expect("a valid model")
        .clone()
}

/// Every block of the main board by handle, at its rounded position.
fn positions(harness: &FolderHarness, id: &str) -> BTreeMap<String, (i64, i64)> {
    rounded(graph_of(&model(harness, id), false))
}

/// The window on `root` with `shop-30.afs` shown on its board.
fn open_shop_30(root: &std::path::Path, size: egui::Vec2) -> (FolderHarness, String) {
    let mut harness = folder_harness_in(root, size, Mode::Diagram);
    let id = shop_30(&harness);
    app(&mut harness).review_open(SHOP_30, None);
    settle(&mut harness);
    (harness, id)
}

/// The preferences the checkpoint had: *Compact cards* on, checks collapsed,
/// and the board drawing at that density with those lines.
fn assert_checkpoint_preferences(harness: &FolderHarness) {
    let window = folder(harness);
    assert!(window.preferences.compact_cards, "Compact cards is on");
    assert!(window.preferences.checks_collapsed, "checks are collapsed");
    assert_eq!(window.density(), crate::canvas::CardDensity::Compact);
    assert_eq!(
        window.card_density.get(),
        crate::canvas::CardDensity::Compact
    );
    assert!(
        window.card_density.lines().folded,
        "the placer folds checks"
    );
    assert_eq!(window.card_density.lines().wires, window.preferences.wires);
}

/// The scenario's board, drawn, with a card whole in view; then Tidy up.
fn assert_tidy_up_moves_nothing(harness: &mut FolderHarness, id: &str, path: &str) {
    assert_eq!(folder(harness).workbench.active(), Some(id));
    let board = folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn");
    let focus = board.debug_focus();
    assert!(
        board
            .debug_cards()
            .iter()
            .any(|(_, rect)| focus.contains_rect(*rect)),
        "{path}: no card stands whole in the board {focus:?}"
    );
    let opened = positions(harness, id);
    assert!(opened.len() > 25, "{path}: shop-30 is on the board");
    app(harness).active_board_command(Command::TidyUp(Vec::new()));
    settle(harness);
    let tidied = positions(harness, id);
    let moved: Vec<String> = opened
        .iter()
        .filter(|(handle, at)| {
            let to = tidied[*handle];
            (to.0 - at.0).abs() > 1 || (to.1 - at.1).abs() > 1
        })
        .map(|(handle, at)| format!("{handle} {at:?} -> {:?}", tidied[handle]))
        .collect();
    assert!(
        moved.is_empty(),
        "{path}: Tidy up right after opening moves {} cards: {moved:#?}",
        moved.len()
    );
}

/// Criterion 1, opened with the folder: `shop-30.afs` is registered in a
/// clean demo shop with no stored layout at all, then the window opens the
/// folder and shows it.
#[test]
fn lay33_criterion_1_opened_with_the_folder_has_the_tidy_up_shape() {
    for size in [WIDE, MINIMUM] {
        let project = crate::testing::shop_project();
        let root = project.path();
        register_without_a_window(root);
        // Registration out of the window placed it with the uniform placer;
        // removing the diagram leaves no stored position for any block.
        fs::remove_file(root.join(DIAGRAM)).unwrap();

        let (mut harness, id) = open_shop_30(root, size);
        assert_checkpoint_preferences(&harness);
        assert_tidy_up_moves_nothing(&mut harness, &id, "opened with the folder");
    }
}

/// Criterion 1, registered through *Register in project*: the window is
/// open on a clean demo shop, `shop-30.afs` appears in the folder and is
/// registered from the explorer, which opens it.
#[test]
fn lay33_criterion_1_registered_in_the_window_has_the_tidy_up_shape() {
    for size in [WIDE, MINIMUM] {
        let project = crate::testing::shop_project();
        let root = project.path();
        fs::remove_file(root.join(DIAGRAM)).unwrap();
        let mut harness = folder_harness_in(root, size, Mode::Diagram);
        assert_checkpoint_preferences(&harness);

        fs::write(root.join(SHOP_30), fixture()).unwrap();
        follow_disk(&mut harness);
        app(&mut harness).explorer_action(explorer::Action::Register(SHOP_30.into()));
        settle(&mut harness);
        let id = shop_30(&harness);
        assert_tidy_up_moves_nothing(&mut harness, &id, "registered in the window");
    }
}

/// Criterion 5: `shop-30.afs` registered by a session without a window keeps
/// the uniform placer's layout in `diagram.json`. Its first open in the
/// window lays it out as Tidy up does, leaves nothing unsaved, and saves
/// that layout, which a session without a window then reads.
#[test]
fn lay33_criterion_5_a_layout_saved_without_a_window_is_tidied_on_first_open() {
    for size in [WIDE, MINIMUM] {
        let project = crate::testing::shop_project();
        let root = project.path();
        register_without_a_window(root);
        let headless = saved_positions(root);

        let (mut harness, id) = open_shop_30(root, size);
        assert_checkpoint_preferences(&harness);
        let opened = positions(&harness, &id);
        assert_ne!(opened, headless, "the window lays the board out again");
        let document = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&id)
            .unwrap();
        assert!(
            !document.is_dirty() && !document.is_layout_dirty(),
            "opening leaves nothing unsaved"
        );
        assert_eq!(saved_positions(root), opened, "the layout is saved");
        assert_tidy_up_moves_nothing(&mut harness, &id, "saved without a window");
    }
}

/// Criterion 6: a layout the window saved is kept. The board tidied on
/// first open, with one card then moved by hand and saved, opens again
/// exactly as it was saved; so does a scenario registered in the window.
#[test]
fn lay33_criterion_6_a_layout_from_the_window_is_kept() {
    let project = crate::testing::shop_project();
    let root = project.path();
    register_without_a_window(root);
    let moved = {
        let (mut harness, id) = open_shop_30(root, WIDE);
        let node = graph_of(&model(&harness, &id), false).nodes[1].clone();
        app(&mut harness).active_board_command(Command::Move(vec![(
            node.id.clone(),
            Position {
                x: node.position.x + 400.0,
                y: node.position.y + 300.0,
            },
        )]));
        settle(&mut harness);
        app(&mut harness).save_all();
        settle(&mut harness);
        positions(&harness, &id)
    };
    assert_eq!(saved_positions(root), moved, "the moved card is saved");
    let (harness, id) = open_shop_30(root, WIDE);
    assert_eq!(positions(&harness, &id), moved, "a saved layout is kept");
    drop(harness);

    let project = crate::testing::shop_project();
    let root = project.path();
    let registered = {
        let mut harness = folder_harness_in(root, WIDE, Mode::Diagram);
        fs::write(root.join(SHOP_30), fixture()).unwrap();
        follow_disk(&mut harness);
        app(&mut harness).explorer_action(explorer::Action::Register(SHOP_30.into()));
        settle(&mut harness);
        let id = shop_30(&harness);
        positions(&harness, &id)
    };
    let (harness, id) = open_shop_30(root, WIDE);
    assert_eq!(
        positions(&harness, &id),
        registered,
        "a layout made in the window is kept"
    );
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay33_first_open() {
    for (size, label) in [(MINIMUM, "960"), (WIDE, "1600")] {
        let project = crate::testing::shop_project();
        let root = project.path();
        register_without_a_window(root);
        let (mut harness, id) = open_shop_30(root, size);
        let board = folder(&harness)
            .canvases
            .get(&id)
            .expect("the board is drawn");
        let focus = board.debug_focus();
        assert!(
            board
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{label}: a card is in view"
        );
        crate::testing::capture(&mut harness, &format!("lay33-shop-30-{label}"), None);
    }
}
