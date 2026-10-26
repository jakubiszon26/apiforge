//! KTLO-20: the sleeping dog that keeps an empty board company, driven
//! through named widgets (ADR 0013, docs/gui-testing.md).
//!
//! The criteria are issue #46 and the mock-up approved there: the drawing
//! above the hint, the pair centred in the board, 220 points wide at
//! 960 × 540 and 280 at 1600 × 1000, in Diagram and on the board side of
//! Split whenever no tab is open, with the four-second loop off in the test
//! harness. What the accessibility tree cannot see — the drawing itself, the
//! letters' bold font and the loop on screen — is the design review and the
//! QA report.
use super::project_interaction_tests::{app, choose};
use super::*;
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_disjoint, assert_within_window, folder, folder_harness,
    settle, shop_project,
};
use egui_kittest::kittest::Queryable;

/// The drawing's accessible name. It is decoration, but a named one: a test
/// measures it, and a screen reader says what the empty board shows.
const DOG: &str = "Sleeping dog";
/// The hint the drawing sits above.
const HINT: &str = "Open a scenario or component to see its diagram.";
/// The board area, named whether a document fills it or not.
const BOARD: &str = "Diagram board";
/// The hint of the code side, which this ticket leaves alone.
const CODE_HINT: &str = "Open a scenario or component from the document list.";
/// The demo scenario, which the project opens by itself the first time.
const TAB: &str = "promotion-and-cart-price.afs";
/// The view box's 50:34, which the drawing's box keeps.
const ASPECT: f32 = 0.68;

fn rect(harness: &FolderHarness, label: &str) -> egui::Rect {
    harness.get_by_label(label).rect()
}

fn shown(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label(label).is_some()
}

/// The demo project in `mode` with its last tab closed, as the owner leaves it.
fn empty_board(size: egui::Vec2, mode: &str) -> (tempfile::TempDir, FolderHarness) {
    let project = shop_project();
    let mut harness = folder_harness(project.path(), size);
    harness.get_by_label(mode).click();
    settle(&mut harness);
    choose(&mut harness, &format!("Close {TAB}"));
    assert!(
        folder(&harness).workbench.active().is_none(),
        "closing the only tab leaves none open"
    );
    (project, harness)
}

/// Whether the window asked to be drawn again at once, as an animation does.
/// Two passes: a repaint asked for during one pass is reported by the next.
fn redraws_at_once(harness: &mut FolderHarness) -> bool {
    harness.step();
    harness.step();
    harness.ctx.requested_repaint_last_pass()
}

/// Whether the drawing's own loop asked for the next frame. The diagram view
/// asks for a repaint in one place only: the frame that draws the dog.
fn the_loop_redraws(harness: &mut FolderHarness) -> bool {
    harness.step();
    harness.step();
    harness
        .ctx
        .repaint_causes()
        .iter()
        .any(|cause| cause.file.ends_with("folder/diagram.rs"))
}

/// Criterion 1: closing the last tab shows the drawing in Diagram and on the
/// board side of Split; opening a file hides it again.
#[test]
fn closing_the_last_tab_shows_the_drawing_and_opening_a_file_hides_it() {
    for mode in ["Diagram", "Split"] {
        let project = shop_project();
        let mut harness = folder_harness(project.path(), WIDE);
        harness.get_by_label(mode).click();
        settle(&mut harness);
        assert!(
            !shown(&harness, DOG),
            "{mode} shows no drawing while a file is open"
        );
        choose(&mut harness, &format!("Close {TAB}"));
        assert!(
            shown(&harness, DOG),
            "{mode} shows the drawing once the last tab is closed"
        );
        // The document list is the way back to the file, and it is in Code.
        harness.get_by_label("Code").click();
        settle(&mut harness);
        choose(&mut harness, TAB);
        harness.get_by_label(mode).click();
        settle(&mut harness);
        assert_eq!(
            folder(&harness).workbench.tabs().len(),
            1,
            "the file is open again"
        );
        assert!(
            !shown(&harness, DOG),
            "{mode} hides the drawing when a file is open again"
        );
    }
}

/// Criterion 2: the drawing is 220 points wide at the minimum window and 280
/// at the default one, in the view box's proportions, in both modes.
#[test]
fn the_drawing_is_220_points_wide_at_960_by_540_and_280_at_1600_by_1000() {
    for (size, width) in [(MINIMUM, 220.0_f32), (WIDE, 280.0_f32)] {
        for mode in ["Diagram", "Split"] {
            let (_project, harness) = empty_board(size, mode);
            let dog = rect(&harness, DOG);
            assert!(
                (dog.width() - width).abs() <= 0.5,
                "{mode} at {size:?} draws the dog {} points wide, not {width}",
                dog.width()
            );
            assert!(
                (dog.height() - width * ASPECT).abs() <= 0.5,
                "{mode} at {size:?} draws the dog {} points high, not {}",
                dog.height(),
                width * ASPECT
            );
        }
    }
}

/// Criterion 3: the hint, centred alone today, sits under the drawing, and the
/// pair is centred in the board area at both window sizes and in both modes.
#[test]
fn the_hint_sits_under_the_drawing_and_the_pair_is_centred_in_the_board() {
    for size in [MINIMUM, WIDE] {
        for mode in ["Diagram", "Split"] {
            let (_project, harness) = empty_board(size, mode);
            let dog = rect(&harness, DOG);
            let hint = rect(&harness, HINT);
            let board = rect(&harness, BOARD);
            assert!(
                hint.top() >= dog.bottom() - 0.5,
                "{mode} at {size:?}: the hint at {hint:?} is not under the drawing at {dog:?}"
            );
            assert!(
                hint.top() - dog.bottom() <= 24.0,
                "{mode} at {size:?}: the hint is {} points below the drawing",
                hint.top() - dog.bottom()
            );
            assert!(
                (dog.center().x - hint.center().x).abs() <= 1.0,
                "{mode} at {size:?}: the drawing at {dog:?} and the hint at {hint:?} \
                 do not share a horizontal centre"
            );
            let pair = dog.union(hint);
            assert!(
                (pair.center().x - board.center().x).abs() <= 1.0,
                "{mode} at {size:?}: the pair at {pair:?} is not centred across the board {board:?}"
            );
            assert!(
                (pair.center().y - board.center().y).abs() <= 1.0,
                "{mode} at {size:?}: the pair at {pair:?} is not centred down the board {board:?}"
            );
            assert_within_window(&harness, &[DOG, HINT]);
            assert_disjoint(&harness, &[DOG, HINT]);
        }
    }
}

/// Criterion 4: the code side of Split keeps its own hint, to the left of the
/// board the drawing is on.
#[test]
fn the_code_side_of_split_keeps_its_own_hint() {
    for size in [MINIMUM, WIDE] {
        let (_project, harness) = empty_board(size, "Split");
        let code = rect(&harness, CODE_HINT);
        let dog = rect(&harness, DOG);
        assert!(
            code.right() <= dog.left(),
            "at {size:?} the code hint at {code:?} is not left of the drawing at {dog:?}"
        );
        assert_within_window(&harness, &[CODE_HINT, DOG, HINT]);
    }
}

/// Criterion 5: the board's other hints are unchanged — a text file's hint
/// keeps the board to itself, justified across it as it is today. Its board
/// is a few points shorter than an empty one, whose tab strip holds no tab,
/// so what is measured is the hint itself.
#[test]
fn the_text_file_hint_keeps_the_board_to_itself() {
    let project = shop_project();
    std::fs::write(project.path().join("README.md"), "# Shop\n").unwrap();
    let mut harness = folder_harness(project.path(), WIDE);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    app(&mut harness).open_text("README.md");
    settle(&mut harness);
    let hint = rect(
        &harness,
        "README.md is a text file, not a scenario or component, so it has no diagram.",
    );
    assert!(
        !shown(&harness, DOG),
        "a text file's hint keeps the board to itself"
    );
    let window = harness.ctx.content_rect();
    assert!(
        hint.height() > 200.0,
        "the text-file hint at {hint:?} is no longer justified across its board"
    );
    assert!(
        (hint.center().x - window.center().x).abs() <= 1.0,
        "the text-file hint at {hint:?} is no longer centred in the window {window:?}"
    );
}

/// Criterion 3 of the ticket: a tab open on a file whose code has errors, or
/// on one missing from the folder, shows the hint it shows today and no
/// drawing.
#[test]
fn a_file_with_errors_or_a_missing_one_keeps_its_own_hint() {
    for (break_it, expected) in [
        (true, "The code has errors"),
        (false, "missing from the project folder"),
    ] {
        let project = shop_project();
        let source = project
            .path()
            .join("scenarios")
            .join("promotion-and-cart-price.afs");
        if break_it {
            std::fs::write(&source, "scenario \"Broken\" {\n").unwrap();
        } else {
            std::fs::remove_file(&source).unwrap();
        }
        let mut harness = folder_harness(project.path(), WIDE);
        harness.get_by_label("Diagram").click();
        settle(&mut harness);
        assert!(
            harness.query_by_label_contains(expected).is_some(),
            "the board shows {expected:?}"
        );
        assert!(
            !shown(&harness, DOG),
            "{expected:?} keeps the board to itself"
        );
    }
}

/// Criterion 6: the four-second loop is off in the test harness, so a board
/// that shows the drawing settles as every other board does.
#[test]
fn an_empty_board_asks_for_no_redraw_in_the_test_harness() {
    for mode in ["Diagram", "Split"] {
        let (_project, mut harness) = empty_board(MINIMUM, mode);
        assert!(shown(&harness, DOG), "{mode} shows the drawing");
        assert!(
            !the_loop_redraws(&mut harness),
            "{mode} keeps the loop going in the test harness"
        );
        assert!(
            !redraws_at_once(&mut harness),
            "{mode} draws an empty board again every frame"
        );
    }
}

/// Criterion 6, the other half: the loop runs while an empty board is on
/// screen and nowhere else. It is off in the harness, so this test turns it on
/// for the window it watches; it never settles such a window.
#[test]
fn the_loop_runs_only_while_an_empty_board_is_visible() {
    for mode in ["Diagram", "Split"] {
        let (_project, mut harness) = empty_board(MINIMUM, mode);
        assert!(
            !folder(&harness).animation,
            "the harness holds the drawing at its rest frame by default"
        );
        app(&mut harness).animation = true;
        assert!(
            the_loop_redraws(&mut harness),
            "{mode} does not keep the drawing moving on an empty board"
        );
        assert!(
            redraws_at_once(&mut harness),
            "{mode} does not ask for the next frame of the loop"
        );
    }
    // Code shows no board, so there is nothing to animate.
    let (_project, mut harness) = empty_board(MINIMUM, "Code");
    app(&mut harness).animation = true;
    assert!(
        !the_loop_redraws(&mut harness),
        "Code keeps the loop going with no board on screen"
    );
    // Neither does a board with a file open, which shows no drawing. Such a
    // board asks for frames of its own, so this watches the loop's own ask.
    for mode in ["Diagram", "Split"] {
        let project = shop_project();
        let mut harness = folder_harness(project.path(), MINIMUM);
        harness.get_by_label(mode).click();
        settle(&mut harness);
        app(&mut harness).animation = true;
        assert!(
            !the_loop_redraws(&mut harness),
            "{mode} keeps the loop going with a file open"
        );
    }
}
