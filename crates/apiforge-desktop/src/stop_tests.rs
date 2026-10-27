//! KTLO-03 criterion 5: the *Migration stopped* window at both screen sizes.
//! The window is a native window of its own size; on a screen it is centred,
//! as the window system centres it, so the harness draws it the same way.
use super::*;
use crate::Front;
use crate::testing::legacy_workspace::{REFUSED_PROJECT_NAME, REFUSED_SCENARIO_NAME};
use crate::testing::{MINIMUM, WIDE, assert_disjoint, assert_within_window};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const OPEN: &str = "Open the previous workspace";
const QUIT: &str = "Quit";

fn refusal() -> Refusal {
    Refusal {
        project: Some(REFUSED_PROJECT_NAME.into()),
        scenario: Some(REFUSED_SCENARIO_NAME.into()),
        reason: "a scenario cannot be written as code without loss".into(),
    }
}

/// The previous workspace in a session, so a test never opens the data
/// directory of this computer.
fn previous() -> Result<app::Source, String> {
    Ok(app::Source::Session(app::default_workspace()))
}

/// Where the window lies on a screen of `size`.
fn window_on(size: egui::Vec2) -> egui::Rect {
    egui::Rect::from_center_size((size / 2.0).to_pos2(), WINDOW_SIZE)
}

fn stop_harness(size: egui::Vec2) -> Harness<'static, Front> {
    // The theme's fonts take effect from the frame after it is applied, and
    // the builder draws one frame at once; the application applies it before
    // its first frame.
    let mut themed = false;
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        move |ui, front: &mut Front| {
            if !themed {
                crate::theme::apply(ui.ctx());
                themed = true;
                return;
            }
            match front {
                Front::Stopped(_) => {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(window_on(size)), |ui| {
                        front.ui(ui)
                    });
                }
                _ => front.ui(ui),
            }
        },
        Front::Stopped(Box::new(Stopped::new(refusal(), previous))),
    );
    harness.run();
    harness
}

fn has_text(harness: &Harness<'static, Front>, text: &str) -> bool {
    harness.query_by_label_contains(text).is_some()
}

#[test]
fn ktlo03_the_stop_window_says_what_was_refused_and_offers_two_ways_out() {
    for size in [MINIMUM, WIDE] {
        let harness = stop_harness(size);
        assert!(has_text(&harness, "Migration stopped"), "{size:?}: title");
        for text in [
            REFUSED_PROJECT_NAME,
            REFUSED_SCENARIO_NAME,
            "a scenario cannot be written as code without loss",
            "left untouched",
            "nothing was activated",
        ] {
            assert!(has_text(&harness, text), "{size:?}: {text:?} is shown");
        }
        let open = harness.get_by_role_and_label(egui::accesskit::Role::Button, OPEN);
        let quit = harness.get_by_role_and_label(egui::accesskit::Role::Button, QUIT);
        assert_within_window(&harness, &[OPEN, QUIT]);
        assert_disjoint(&harness, &[OPEN, QUIT]);
        // Inside the window's 24-point margins, the buttons on its last row.
        let inner = window_on(size).shrink(24.0);
        for button in [open.rect(), quit.rect()] {
            assert!(
                inner.expand(0.5).contains_rect(button),
                "{size:?}: {button:?} inside {inner:?}"
            );
        }
        assert!(
            quit.rect().left() >= open.rect().right(),
            "{size:?}: Quit is the last button"
        );
        // On the last row: 24 points above the window's bottom edge, as in the
        // mock-up (found in the release-binary capture, KTLO-03 QA).
        for button in [open.rect(), quit.rect()] {
            assert!(
                (button.bottom() - inner.bottom()).abs() <= 1.0,
                "{size:?}: {button:?} ends on the margin {}",
                inner.bottom()
            );
        }
    }
}

#[test]
fn ktlo03_open_the_previous_workspace_opens_the_workspace_window() {
    for size in [MINIMUM, WIDE] {
        let mut harness = stop_harness(size);
        harness.get_by_label(OPEN).click();
        harness.step();
        let commands = harness.output().viewport_output[&egui::ViewportId::ROOT]
            .commands
            .clone();
        harness.run();
        assert!(
            matches!(harness.state(), Front::Workspace(_)),
            "{size:?}: the workspace window replaced the stop window"
        );
        assert!(harness.state().has_document(), "{size:?}");
        assert!(
            !has_text(&harness, "Migration stopped"),
            "{size:?}: the stop window is gone"
        );
        assert!(
            commands.contains(&egui::ViewportCommand::Resizable(true))
                && commands.iter().any(|command| matches!(
                    command,
                    egui::ViewportCommand::InnerSize(size) if *size == egui::vec2(1600.0, 1000.0)
                )),
            "{size:?}: the window grows to the workspace window's size: {commands:?}"
        );
    }
}

#[test]
fn ktlo03_quit_closes_the_window_and_opens_nothing() {
    for size in [MINIMUM, WIDE] {
        let mut harness = stop_harness(size);
        harness.get_by_label(QUIT).click();
        harness.step();
        let commands = &harness.output().viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(
            commands.contains(&egui::ViewportCommand::Close),
            "{size:?}: {commands:?}"
        );
        assert!(
            matches!(harness.state(), Front::Stopped(_)),
            "{size:?}: nothing else opens"
        );
    }
}
