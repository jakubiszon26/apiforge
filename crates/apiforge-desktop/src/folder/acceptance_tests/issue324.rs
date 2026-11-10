//! Issue #324: the storage band appearing under the tabs moves and shortens
//! the board's viewport for as long as a slow job runs. That change of the
//! board must not pan the camera to a selected card the person, or a
//! holder's save, had left partly outside the view: the camera stays where
//! the person was (MCP-08a), and keep-in-view pans only for what a change of
//! the board newly hides (UX-18).
use super::*;
use crate::testing::{folder_harness_in, quiesce};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const BAND: &str = "Checking external changes…";

fn canvas<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board was drawn")
}

#[test]
fn a_viewport_that_only_moves_does_not_pan_the_board() {
    let project = crate::testing::shop_project();
    let mut harness = folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let scenario = only(&harness, DocumentKind::Scenario);
    choose(&mut harness, "Expected price (Value)");
    quiesce(&mut harness);
    let selected = folder(&harness).selections[&scenario][0].clone();

    // The person scrolls the selected card partly out of view.
    let focus = canvas(&harness, &scenario).debug_focus();
    harness.hover_at(focus.center());
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(focus.width() * 0.6, 0.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    settle(&mut harness);
    harness.hover_at(egui::pos2(1.0, 1.0));
    quiesce(&mut harness);
    let card = canvas(&harness, &scenario)
        .debug_card_rect(&selected)
        .expect("the selected card is on the board");
    let focus = canvas(&harness, &scenario).debug_focus();
    assert!(
        !focus.contains_rect(card),
        "the card {card:?} is partly outside the focus {focus:?}"
    );
    let camera = canvas(&harness, &scenario).camera();
    assert!(camera.is_some(), "the person moved the camera");
    let viewport = canvas(&harness, &scenario).debug_viewport();

    // A slow job shows the band, then ends.
    app(&mut harness).storage.forced_label = Some(BAND);
    settle(&mut harness);
    assert_eq!(folder(&harness).debug_storage_band(), Some(BAND));
    let during = canvas(&harness, &scenario).debug_viewport();
    assert_ne!(during, viewport, "the band moved the board");
    app(&mut harness).storage.forced_label = None;
    settle(&mut harness);
    assert_eq!(canvas(&harness, &scenario).debug_viewport(), viewport);

    assert_eq!(
        canvas(&harness, &scenario).camera(),
        camera,
        "the camera stays where the person was"
    );
}
