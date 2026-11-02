//! UX-00: the design review views of the shared mock-up for UX-01..05
//! (docs/tickets/UX-00.md). Throwaway branch `mock/ux-00`: the switches in
//! `crate::mock_ux00` change the real header, Run options, Run results,
//! editor and panel for these views only.
use super::*;
use crate::mock_ux00::{self as mock, Drag, Flags, RunMode, SimulationColour, Status, Unsaved};
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_disjoint, assert_within_window, capture, folder,
    folder_harness, settle,
};
use egui_kittest::kittest::Queryable;

const SIZES: [(egui::Vec2, &str); 2] = [(MINIMUM, "960x540"), (WIDE, "1600x1000")];

fn app(harness: &mut FolderHarness) -> &mut FolderApp {
    harness.state_mut().as_mut().unwrap()
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    // The pointer leaves for the header's empty top margin, so no control
    // is captured hovered.
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
}

/// The demo shop at `size`, in `mode`, with *Live allowed* ticked (MCP-09)
/// so its badge shares the header with the run controls.
fn shop_window(
    size: egui::Vec2,
    mode: &str,
    configuration: &std::path::Path,
    base_url: Option<String>,
) -> (tempfile::TempDir, FolderHarness) {
    let project = crate::testing::shop_project();
    if let Some(base_url) = base_url {
        let manifest = project.path().join("apiforge.project.json");
        let mut config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        config["environment"]["baseUrl"] = serde_json::json!(base_url);
        std::fs::write(manifest, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    }
    let mut harness = folder_harness(project.path(), size);
    {
        let app = app(&mut harness);
        app.use_configuration_directory(configuration);
        let id = app.session.as_ref().unwrap().manifest().id.clone();
        app.consent.set(&id, true);
        app.say_consent();
    }
    settle(&mut harness);
    click(&mut harness, mode);
    (project, harness)
}

fn of_kind(harness: &FolderHarness, kind: DocumentKind) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().kind == kind)
        .unwrap()
        .id()
        .to_owned()
}

/// An unsaved, valid change to the scenario's code.
fn edit_scenario(harness: &mut FolderHarness) {
    let id = of_kind(harness, DocumentKind::Scenario);
    let app = app(harness);
    let session = app.session.as_mut().unwrap();
    let text = session.document(&id).unwrap().text().to_owned();
    let edited = text.replacen("Check the promotion,", "Check the active promotion,", 1);
    assert_ne!(edited, text);
    session.edit_source(&id, &edited, CODE).unwrap();
    app.analyser.typed(&id, Instant::now());
    settle(harness);
}

/// Runs the active tab with the header's run button and waits for its report.
fn run(harness: &mut FolderHarness, label: &str) {
    click(harness, label);
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
}

fn header_fits(harness: &FolderHarness, extra: &[&str]) {
    let mut labels = vec!["Code", "Diagram", "Split", "Save", "Save all"];
    if harness.query_by_label("Cancel run").is_none() {
        labels.push("Run options");
    }
    labels.extend_from_slice(extra);
    assert_within_window(harness, &labels);
    assert_disjoint(harness, &labels);
}

/// UX-02: *Run simulation* and *Run live* on the header's button, beside
/// MCP-09's *Live allowed*, after a run so the SIMULATION / LIVE API badge in
/// Run results is in view; *Run options* open with the mode; the violet
/// variant; a component's tab, whose header also has *Publish*.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux02_run_mode() {
    for (size, suffix) in SIZES {
        let configuration = tempfile::tempdir().unwrap();
        mock::set(Flags {
            run_mode: RunMode::Simulation,
            ..Flags::default()
        });
        let (_project, mut harness) = shop_window(size, "Diagram", configuration.path(), None);
        run(&mut harness, "Run simulation");
        app(&mut harness).notice = None;
        settle(&mut harness);
        header_fits(&harness, &["Run simulation", "Live allowed"]);
        capture(&mut harness, &format!("ux02-run-simulation-{suffix}"), None);
        click(&mut harness, "Run options");
        capture(&mut harness, &format!("ux02-run-options-{suffix}"), None);
        // The mode is chosen in the menu.
        click(&mut harness, "Live");
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
        assert_eq!(mock::get().run_mode, RunMode::Live);
        header_fits(&harness, &["Run live", "Live allowed"]);
        capture(&mut harness, &format!("ux02-run-live-{suffix}"), None);
        if size == MINIMUM {
            click(&mut harness, "Run options");
            capture(&mut harness, "ux02-run-options-live-960x540", None);
            harness.key_press(egui::Key::Escape);
            settle(&mut harness);
            mock::set(Flags {
                run_mode: RunMode::Simulation,
                colour: SimulationColour::Violet,
                ..Flags::default()
            });
            settle(&mut harness);
            capture(&mut harness, "ux02-run-simulation-violet-960x540", None);
            // *Run options* as a chevron of the Run button: the project's
            // name gets back the room the wider button takes.
            mock::set(Flags {
                run_mode: RunMode::Simulation,
                chevron_options: true,
                ..Flags::default()
            });
            settle(&mut harness);
            header_fits(&harness, &["Run simulation", "Live allowed"]);
            capture(&mut harness, "ux02-run-simulation-chevron-960x540", None);
            click(&mut harness, "Run options");
            capture(&mut harness, "ux02-run-options-chevron-960x540", None);
            harness.key_press(egui::Key::Escape);
            settle(&mut harness);
            mock::set(Flags {
                run_mode: RunMode::Simulation,
                ..Flags::default()
            });
            let component = of_kind(&harness, DocumentKind::Component);
            app(&mut harness).activate_tab(&component);
            settle(&mut harness);
            header_fits(&harness, &["Run simulation", "Live allowed", "Publish"]);
            capture(&mut harness, "ux02-component-tab-960x540", None);
        }
    }
    mock::set(Flags::default());
}

/// UX-04: *Running unsaved code* in the header instead of the band, while
/// the run runs: beside *Cancel run* and, as the other placement, after the
/// project name; the band as on `main` for comparison; a component's tab at
/// 960 px, the fullest header. The API accepts and never answers, so the
/// run is still running when captured.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux04_unsaved_notice() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming() {
            held.push(stream);
        }
    });
    for (size, suffix) in SIZES {
        let configuration = tempfile::tempdir().unwrap();
        mock::set(Flags {
            run_mode: RunMode::Live,
            ..Flags::default()
        });
        let (_project, mut harness) = shop_window(
            size,
            "Split",
            configuration.path(),
            Some(format!("http://{address}")),
        );
        edit_scenario(&mut harness);
        click(&mut harness, "Run live");
        if harness.query_by_label("Run this version").is_some() {
            click(&mut harness, "Run this version");
        }
        assert!(folder(&harness).runs.handle.is_some(), "the run is running");
        assert!(
            folder(&harness)
                .notice
                .as_ref()
                .is_some_and(|notice| notice.text.starts_with("Running unsaved code")),
            "the run is of unsaved code: {:?}",
            folder(&harness).notice.as_ref().map(|notice| &notice.text)
        );
        let header = egui::Rect::from_min_max(
            egui::Pos2::ZERO,
            egui::pos2(size.x, harness.get_by_label("Code").rect().bottom() + 9.0),
        );
        capture(&mut harness, &format!("ux04-now-in-band-{suffix}"), None);
        for (unsaved, name) in [
            (Unsaved::BesideRun, "beside-cancel"),
            (Unsaved::BesideTitle, "beside-title"),
        ] {
            mock::set(Flags {
                run_mode: RunMode::Live,
                unsaved,
                ..Flags::default()
            });
            settle(&mut harness);
            header_fits(
                &harness,
                &["Cancel run", "Live allowed", "Running unsaved code"],
            );
            capture(&mut harness, &format!("ux04-{name}-{suffix}"), None);
            capture(
                &mut harness,
                &format!("ux04-{name}-header-{suffix}"),
                Some(header),
            );
        }
        if size == MINIMUM {
            mock::set(Flags {
                run_mode: RunMode::Live,
                unsaved: Unsaved::BesideRun,
                ..Flags::default()
            });
            let component = of_kind(&harness, DocumentKind::Component);
            app(&mut harness).activate_tab(&component);
            settle(&mut harness);
            header_fits(
                &harness,
                &[
                    "Cancel run",
                    "Live allowed",
                    "Publish",
                    "Running unsaved code",
                ],
            );
            capture(&mut harness, "ux04-component-tab-960x540", None);
        }
        harness.get_by_label("Cancel run").click();
        harness.step();
    }
    mock::set(Flags::default());
}

/// UX-01: one run, then a check added for two different fields of one step
/// (`productId` and `total` of *Add to cart*) from Run results without
/// running again; the other fields still offer *Check*. At 960 × 540 Run
/// results is maximised, as the fields do not show in its normal height.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux01_two_checks() {
    fn check(harness: &mut FolderHarness, field: &str) {
        let y = harness.get_by_label(field).rect().center().y;
        let button = harness
            .get_all_by_label("Check")
            .min_by(|a, b| {
                let a = (a.rect().center().y - y).abs();
                let b = (b.rect().center().y - y).abs();
                a.total_cmp(&b)
            })
            .unwrap();
        button.click();
        harness.hover_at(egui::pos2(480.0, 3.0));
        settle(harness);
    }
    for (size, suffix) in SIZES {
        let configuration = tempfile::tempdir().unwrap();
        mock::set(Flags {
            run_mode: RunMode::Simulation,
            checks_kept: true,
            ..Flags::default()
        });
        mock::clear_checks();
        let (_project, mut harness) = shop_window(size, "Split", configuration.path(), None);
        run(&mut harness, "Run simulation");
        app(&mut harness).notice = None;
        let id = of_kind(&harness, DocumentKind::Scenario);
        app(&mut harness)
            .selections
            .insert(id.clone(), vec!["cart".into()]);
        settle(&mut harness);
        if size == MINIMUM {
            click(&mut harness, "Maximise run results");
        } else {
            // The panel at its tallest normal height, so the fields show.
            app(&mut harness).runs.panel.debug_set_height(560.0);
            settle(&mut harness);
        }
        capture(&mut harness, &format!("ux01-before-{suffix}"), None);
        check(&mut harness, "productId");
        check(&mut harness, "total");
        assert_eq!(
            mock::checks(),
            2,
            "two checks from one run: {:?}",
            mock::checked()
        );
        assert!(mock::is_checked("cart", "cart", "productId"));
        assert!(mock::is_checked("cart", "cart", "total"));
        capture(&mut harness, &format!("ux01-two-checks-{suffix}"), None);
        if size == MINIMUM {
            click(&mut harness, "Restore run results");
            capture(&mut harness, "ux01-two-checks-restored-960x540", None);
        }
    }
    mock::set(Flags::default());
}

/// UX-03: the editor while a line is being typed, the code invalid for the
/// moment, the caret in it: as on `main`, with the focus line at the
/// gutter's edge and the PROBLEMS status in red; without the focus line;
/// with the status saying the save state only, never red; with neither.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux03_editor_status() {
    for (size, suffix) in SIZES {
        mock::set(Flags::default());
        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), size);
        click(&mut harness, "Code");
        // The caret at the end of line 2, then a statement half typed.
        let at = harness
            .get_by_label_contains("Contents of")
            .rect()
            .left_top()
            + egui::vec2(120.0, 24.0);
        harness.hover_at(at);
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
        harness.key_press(egui::Key::End);
        harness.key_press(egui::Key::Enter);
        harness.event(egui::Event::Text("total = cart.body.".into()));
        settle(&mut harness);
        for (status, no_focus_line, name) in [
            (Status::Current, false, "now"),
            (Status::Current, true, "no-focus-line"),
            (Status::SaveStateOnly, false, "quiet-status"),
            (Status::SaveStateOnly, true, "neither"),
        ] {
            mock::set(Flags {
                status,
                no_focus_line,
                ..Flags::default()
            });
            settle(&mut harness);
            capture(&mut harness, &format!("ux03-{name}-{suffix}"), None);
        }
    }
    mock::set(Flags::default());
}

/// UX-05: Run results dragged by its edge towards the top of the workspace:
/// mid-drag, with the drop zone the release fills, and released, filling
/// the workspace with its grip at the top to drag it back down.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux05_drag_to_workspace() {
    for (size, suffix) in SIZES {
        let configuration = tempfile::tempdir().unwrap();
        mock::set(Flags {
            run_mode: RunMode::Simulation,
            ..Flags::default()
        });
        let (_project, mut harness) = shop_window(size, "Split", configuration.path(), None);
        run(&mut harness, "Run simulation");
        app(&mut harness).notice = None;
        mock::set(Flags {
            run_mode: RunMode::Simulation,
            drag: Drag::Mid,
            ..Flags::default()
        });
        settle(&mut harness);
        capture(&mut harness, &format!("ux05-mid-drag-{suffix}"), None);
        mock::set(Flags {
            run_mode: RunMode::Simulation,
            drag: Drag::Full,
            ..Flags::default()
        });
        settle(&mut harness);
        click(&mut harness, "Maximise run results");
        capture(&mut harness, &format!("ux05-released-{suffix}"), None);
    }
    mock::set(Flags::default());
}
