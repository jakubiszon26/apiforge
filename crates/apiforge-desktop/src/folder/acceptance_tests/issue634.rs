//! Issue #634: two tests failed on slow runners. A board first drawn while a
//! slow storage job shows its band under the tabs was fitted to the shorter
//! board the band leaves and stayed so once the band had gone, `shop-30` at
//! 960 × 540 at 0.88 of Fit's zoom. And a window whose three frames take
//! longer than the second between two checks of the folder never settled.
use super::*;
use crate::testing::{FolderHarness, MINIMUM, folder_harness_in, quiesce};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const BAND: &str = "Opening the project…";
const FIT: &str = "Fit the view";

fn canvas(harness: &FolderHarness) -> &crate::canvas::Canvas {
    let id = only(harness, DocumentKind::Scenario);
    folder(harness)
        .canvases
        .get(&id)
        .expect("the board is drawn")
}

/// `shop-30` with no positions, its window not yet on the board.
fn window(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-30.afs");
    let project = crate::testing::shop_project();
    fs::copy(fixture, project.path().join(SCENARIO)).unwrap();
    fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
    let harness = folder_harness_in(project.path(), size, Mode::Diagram);
    (project, harness)
}

/// The zoom of the board and, on screen, where its cards stand.
fn view(harness: &FolderHarness) -> (f32, Vec<(String, egui::Rect)>) {
    let board = canvas(harness);
    (board.debug_zoom(), board.debug_cards().to_vec())
}

fn assert_same_view(
    left: &(f32, Vec<(String, egui::Rect)>),
    right: &(f32, Vec<(String, egui::Rect)>),
    when: &str,
) {
    assert!(
        (left.0 - right.0).abs() < 1e-3,
        "{when}: the zooms {} and {}",
        left.0,
        right.0
    );
    assert_eq!(left.1.len(), right.1.len(), "{when}: the same cards");
    for (one, other) in left.1.iter().zip(&right.1) {
        assert!(
            one.0 == other.0 && (one.1.min - other.1.min).length() < 1.0,
            "{when}: the cards {one:?} and {other:?}"
        );
    }
}

/// A board first drawn under the storage band opens at Fit's zoom of the
/// board without it, however many frames the band lasts, and nothing on it
/// moves when the band goes.
#[test]
fn a_board_first_drawn_under_the_storage_band_opens_at_fits_zoom() {
    for size in [WIDE, MINIMUM] {
        for frames in [1, 2, 6] {
            let when = format!("{size:?}, {frames} frames");
            let (_project, mut harness) = window(size);
            app(&mut harness).storage.forced_label = Some(BAND);
            app(&mut harness).canvases.clear();
            app(&mut harness).review_open(SCENARIO, None);
            harness.run_steps(frames);
            assert_eq!(folder(&harness).debug_storage_band(), Some(BAND));
            let under = canvas(&harness).debug_viewport();
            let opened = view(&harness);
            app(&mut harness).storage.forced_label = None;
            settle(&mut harness);
            assert!(
                canvas(&harness).debug_viewport().top() < under.top() - 10.0,
                "{when}: the band stood above the board"
            );
            let first = view(&harness);
            if frames > 2 {
                assert_same_view(&opened, &first, &format!("{when}: the band goes"));
            }
            choose(&mut harness, FIT);
            assert_same_view(&first, &view(&harness), &format!("{when}: Fit"));
        }
    }
}

/// *Fit the view* under the band fits the board as it stands without it,
/// and a fitted board keeps its view while the band comes and goes, with a
/// card selected too.
#[test]
fn the_storage_band_leaves_a_fitted_board_as_it_is() {
    let (_project, mut harness) = window(MINIMUM);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    choose(&mut harness, FIT);
    quiesce(&mut harness);
    let fitted = view(&harness);

    app(&mut harness).storage.forced_label = Some(BAND);
    harness.run_steps(4);
    assert_same_view(&fitted, &view(&harness), "the band comes");
    harness.get_by_label(FIT).click();
    harness.run_steps(4);
    assert_eq!(folder(&harness).debug_storage_band(), Some(BAND));
    assert_same_view(&fitted, &view(&harness), "Fit under the band");
    app(&mut harness).storage.forced_label = None;
    settle(&mut harness);
    assert_same_view(&fitted, &view(&harness), "the band goes");

    let id = only(&harness, DocumentKind::Scenario);
    let step = canvas(&harness).debug_cards()[0].0.clone();
    app(&mut harness).selections.insert(id, vec![step]);
    quiesce(&mut harness);
    let selected = view(&harness);
    app(&mut harness).storage.forced_label = Some(BAND);
    harness.run_steps(4);
    app(&mut harness).storage.forced_label = None;
    settle(&mut harness);
    assert_same_view(&selected, &view(&harness), "with a card selected");
}

/// A camera the person moved, kept while the band shows and restored under
/// it, is the one the board has without the band.
#[test]
fn a_kept_camera_restored_under_the_storage_band_is_itself() {
    let (_project, mut harness) = window(MINIMUM);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    choose(&mut harness, "Zoom in");
    quiesce(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    let moved = view(&harness);
    let kept = canvas(&harness).camera().expect("the person zoomed");

    app(&mut harness).storage.forced_label = Some(BAND);
    harness.run_steps(4);
    assert_eq!(canvas(&harness).camera(), Some(kept), "under the band");
    let mut fresh = folder(&harness).new_canvas();
    fresh.restore_camera(&context_key(&id, false), kept);
    app(&mut harness).canvases.insert(id, fresh);
    harness.run_steps(4);
    assert_same_view(&moved, &view(&harness), "restored under the band");
    app(&mut harness).storage.forced_label = None;
    settle(&mut harness);
    assert_eq!(canvas(&harness).camera(), Some(kept), "the band gone");
    assert_same_view(&moved, &view(&harness), "the band gone");
}

/// Blocks pasted out of view on a moved board are centred in the board of the
/// frame that shows them, the frame in which the band comes or goes too.
#[test]
fn a_paste_in_the_frame_the_band_comes_or_goes_is_centred() {
    for comes in [true, false] {
        let (_project, mut harness) = window(WIDE);
        app(&mut harness).review_open(SCENARIO, None);
        settle(&mut harness);
        quiesce(&mut harness);
        let id = only(&harness, DocumentKind::Scenario);
        app(&mut harness).storage.forced_label = (!comes).then_some(BAND);
        harness.run_steps(4);
        let step = canvas(&harness).debug_cards()[0].0.clone();
        let board = app(&mut harness).canvases.get_mut(&id).unwrap();
        board.restore_camera(&context_key(&id, false), (9000.0, 9000.0, 1.0));
        board.show_pasted(vec![step.clone()]);
        app(&mut harness).storage.forced_label = comes.then_some(BAND);
        harness.step();
        assert_eq!(folder(&harness).debug_storage_band().is_some(), comes);
        let centre = canvas(&harness).debug_card_rect(&step).unwrap().center();
        let focus = canvas(&harness).debug_focus().center();
        assert!(
            (centre - focus).length() < 1.0,
            "the band comes ({comes}): the pasted card at {centre:?}, the focus at {focus:?}"
        );
    }
}

/// A panel beside the board resized under the band pans nothing for the
/// selected card, whose top the person left above the focus: the panel hides
/// nothing of it.
#[test]
fn a_panel_resized_under_the_band_leaves_the_selected_card() {
    let (_project, mut harness) = window(WIDE);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    let step = canvas(&harness).debug_cards()[0].0.clone();
    app(&mut harness)
        .selections
        .insert(id.clone(), vec![step.clone()]);
    quiesce(&mut harness);
    app(&mut harness).storage.forced_label = Some(BAND);
    harness.run_steps(4);
    let focus = canvas(&harness).debug_focus();
    let (x, y, zoom) = canvas(&harness).camera().unwrap_or_else(|| {
        harness.get_by_label("Zoom in").click();
        harness.run_steps(4);
        canvas(&harness).camera().expect("the person zoomed")
    });
    let card = canvas(&harness).debug_card_rect(&step).unwrap();
    let to = egui::vec2(
        focus.center().x - card.center().x,
        focus.top() - 10.0 - card.top(),
    );
    app(&mut harness)
        .canvases
        .get_mut(&id)
        .unwrap()
        .restore_camera(&context_key(&id, false), (x + to.x, y + to.y, zoom));
    harness.run_steps(4);
    let before = canvas(&harness).debug_card_rect(&step).unwrap();
    let viewport = canvas(&harness).debug_viewport();
    assert!(
        (before.top() - (focus.top() - 10.0)).abs() < 1.0,
        "{before:?}"
    );

    harness.get_by_label("Collapse block library").click();
    harness.run_steps(6);
    assert_ne!(canvas(&harness).debug_viewport(), viewport, "the panel");
    assert_eq!(folder(&harness).debug_storage_band(), Some(BAND));
    let after = canvas(&harness).debug_card_rect(&step).unwrap();
    assert!(
        (after.min - before.min).length() < 1.0,
        "{before:?} {after:?}"
    );
}

/// A board first drawn under the band with its top card selected is fitted
/// as without the band.
#[test]
fn a_board_first_drawn_under_the_band_with_a_selection_is_fitted() {
    let (_project, mut harness) = window(MINIMUM);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    let step = canvas(&harness)
        .debug_cards()
        .iter()
        .min_by(|one, other| one.1.top().total_cmp(&other.1.top()))
        .unwrap()
        .0
        .clone();
    app(&mut harness).selections.insert(id.clone(), vec![step]);
    quiesce(&mut harness);
    choose(&mut harness, FIT);
    quiesce(&mut harness);
    let fitted = view(&harness);
    app(&mut harness).storage.forced_label = Some(BAND);
    app(&mut harness).canvases.remove(&id);
    harness.run_steps(6);
    assert_same_view(&fitted, &view(&harness), "under the band");
    app(&mut harness).storage.forced_label = None;
    settle(&mut harness);
    assert_same_view(&fitted, &view(&harness), "the band gone");
}

/// Following the once-a-second check of the folder takes three frames. A
/// window whose three frames take a second, as the large response of
/// `diag12_criterion_7` on a slow runner, settles all the same.
#[test]
fn a_window_of_slow_frames_settles() {
    let project = crate::testing::shop_project();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(MINIMUM)
        .build_ui_state(
            |ui, app: &mut Option<FolderApp>| {
                if let Some(app) = app {
                    std::thread::sleep(std::time::Duration::from_millis(350));
                    app.ui(ui);
                }
            },
            None,
        );
    let window = FolderApp::open(&harness.ctx.clone(), project.path().to_owned());
    *harness.state_mut() = Some(window);
    settle(&mut harness);
    let checks = app(&mut harness).watcher.started.len();
    settle(&mut harness);
    assert!(
        app(&mut harness).watcher.started.len() <= checks + 1,
        "a settling makes at most one check of its own accord"
    );
    harness.run_steps(6);
    assert!(
        app(&mut harness).watcher.started.len() > checks,
        "a window not settling checks as ever"
    );
}
