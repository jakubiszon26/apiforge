//! Shared harnesses for interface tests, design review captures and the
//! headless inspection host (ADR 0013, docs/gui-testing.md).
use crate::app::{App, Source, default_workspace};
use crate::folder::{FolderApp, Mode};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The minimum supported window.
pub const MINIMUM: egui::Vec2 = egui::vec2(960.0, 540.0);
/// The default window of the native application.
pub const WIDE: egui::Vec2 = egui::vec2(1600.0, 1000.0);

const SETTLE_TIMEOUT: Duration = Duration::from_secs(15);

pub type FolderHarness = Harness<'static, Option<FolderApp>>;
pub type WorkspaceHarness = Harness<'static, Option<App>>;

/// The demo shop as a folder project: one scenario and three components.
pub fn shop_project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    apiforge_project::prepare_project(directory.path(), &apiforge_core::demos::shop_project())
        .unwrap();
    directory
}

/// Opens the folder window at `root` and waits until opening has finished.
pub fn folder_harness(root: &Path, size: egui::Vec2) -> FolderHarness {
    folder_harness_scaled(root, size, 1.0)
}

pub fn folder_harness_scaled(root: &Path, size: egui::Vec2, scale: f32) -> FolderHarness {
    folder_harness_with(root, size, scale, None)
}

/// The folder window asked for a mode at startup, as `--screenshot --mode` asks.
pub fn folder_harness_in(root: &Path, size: egui::Vec2, mode: Mode) -> FolderHarness {
    folder_harness_with(root, size, 1.0, Some(mode))
}

fn folder_harness_with(
    root: &Path,
    size: egui::Vec2,
    scale: f32,
    mode: Option<Mode>,
) -> FolderHarness {
    // The builder draws one frame before the application exists.
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(scale)
        .build_ui_state(
            |ui, app: &mut Option<FolderApp>| {
                if let Some(app) = app {
                    app.ui(ui);
                }
            },
            None,
        );
    // The application keeps its context, so it must be the harness's own.
    let context = harness.ctx.clone();
    let mut app = FolderApp::open(&context, root.to_owned());
    if let Some(mode) = mode {
        app.request_mode(mode);
    }
    *harness.state_mut() = Some(app);
    settle(&mut harness);
    harness
}

/// The folder window started from an application data directory, as managed
/// startup starts it, and settled once it has resolved to a project.
pub fn managed_harness(data: &Path, size: egui::Vec2) -> FolderHarness {
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        |ui, app: &mut Option<FolderApp>| {
            if let Some(app) = app {
                app.ui(ui);
            }
        },
        None,
    );
    let context = harness.ctx.clone();
    *harness.state_mut() = Some(FolderApp::start_managed(&context, data.to_owned()));
    settle(&mut harness);
    harness
}

pub fn folder(harness: &FolderHarness) -> &FolderApp {
    harness.state().as_ref().unwrap()
}

/// Steps until no storage work or analysis is in flight and the interface stops
/// requesting immediate repaints. Background work runs in real time, so this
/// waits for it.
pub fn settle(harness: &mut FolderHarness) {
    assert!(settles(harness), "the folder window did not settle");
}

/// Whether the folder window settles as [`settle`] waits for it to, within the
/// same time. A window that asks to be drawn again every frame never does.
pub fn settles(harness: &mut FolderHarness) -> bool {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        if folder(harness).is_analysed()
            && harness.try_run().is_ok()
            && folder(harness).is_analysed()
        {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        harness.step();
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The legacy workspace window with the default demo workspace in a session.
pub fn workspace_harness(size: egui::Vec2) -> WorkspaceHarness {
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        |ui, app: &mut Option<App>| {
            if let Some(app) = app {
                app.ui(ui);
            }
        },
        None,
    );
    let context = harness.ctx.clone();
    *harness.state_mut() = Some(App::new(&context, Source::Session(default_workspace())));
    harness.run();
    harness
}

fn window<State>(harness: &Harness<'_, State>) -> egui::Rect {
    harness.ctx.content_rect()
}

/// Each named widget is entirely inside the window.
pub fn assert_within_window<State>(harness: &Harness<'_, State>, labels: &[&str]) {
    let window = window(harness);
    for label in labels {
        let rect = harness.get_by_label(label).rect();
        assert!(
            window.contains_rect(rect),
            "{label:?} at {rect:?} is outside the window {window:?}"
        );
    }
}

/// No two of the named widgets overlap.
pub fn assert_disjoint<State>(harness: &Harness<'_, State>, labels: &[&str]) {
    let rects: Vec<_> = labels
        .iter()
        .map(|label| (*label, harness.get_by_label(label).rect()))
        .collect();
    for (index, (first, a)) in rects.iter().enumerate() {
        for (second, b) in &rects[index + 1..] {
            assert!(
                !a.intersects(*b) || a.intersect(*b).area() == 0.0,
                "{first:?} at {a:?} overlaps {second:?} at {b:?}"
            );
        }
    }
}

/// Where design review captures are written: `APIFORGE_CAPTURE_DIR`, otherwise
/// `gui-captures` in the Cargo target directory. Never inside the sources.
pub fn capture_dir() -> PathBuf {
    if let Some(directory) = std::env::var_os("APIFORGE_CAPTURE_DIR") {
        return directory.into();
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    target.join("gui-captures")
}

/// Renders the current frame and saves it, or only `region` (in points), as
/// `<name>.png` in [`capture_dir`]. Captures are for review, not assertions.
pub fn capture<State>(
    harness: &mut Harness<'_, State>,
    name: &str,
    region: Option<egui::Rect>,
) -> PathBuf {
    // Neither the pointer nor a hover highlight belongs in a review image.
    harness.remove_cursor();
    harness.step();
    let image = harness
        .render()
        .unwrap_or_else(|error| panic!("cannot render {name}: {error}"));
    let image = match region {
        Some(region) => {
            let scale = harness.ctx.pixels_per_point();
            let pixels = region.intersect(window(harness)) * scale;
            let x = pixels.min.x.floor().max(0.0) as u32;
            let y = pixels.min.y.floor().max(0.0) as u32;
            let width = (pixels.width().ceil() as u32).min(image.width() - x);
            let height = (pixels.height().ceil() as u32).min(image.height() - y);
            image::imageops::crop_imm(&image, x, y, width, height).to_image()
        }
        None => image,
    };
    let directory = capture_dir();
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("{name}.png"));
    image.save(&path).unwrap();
    path
}

/// The standard design review views. Run by `tools/gui/review.sh`.
mod review {
    use super::*;

    fn mode_views(harness: &mut FolderHarness, suffix: &str) {
        for mode in ["Code", "Diagram", "Split"] {
            harness.get_by_label(mode).click();
            settle(harness);
            capture(
                harness,
                &format!("folder-{}-{suffix}", mode.to_lowercase()),
                None,
            );
        }
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_folder_modes_at_the_minimum_window() {
        let project = shop_project();
        let mut harness = folder_harness(project.path(), MINIMUM);
        mode_views(&mut harness, "960x540");
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_folder_modes_at_the_default_window() {
        let project = shop_project();
        let mut harness = folder_harness(project.path(), WIDE);
        mode_views(&mut harness, "1600x1000");
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_folder_modes_at_double_scale() {
        let project = shop_project();
        let mut harness = folder_harness_scaled(project.path(), MINIMUM, 2.0);
        mode_views(&mut harness, "960x540@2x");
    }

    /// The code editor without a box (KTLO-13), with a line wider than the
    /// panel: Code and Split, then Code with the caret at the end of that
    /// line, so the line is scrolled and the gutter's focus line shows.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_code_editor() {
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = tempfile::tempdir().unwrap();
            let mut demo = apiforge_core::demos::shop_project();
            demo.scenarios[0].description = "Check the promotion, the final price and an \
                extra field. The demo data can be run against the local demo API, which \
                answers on the address configured in the project environment."
                .into();
            apiforge_project::prepare_project(directory.path(), &demo).unwrap();
            let mut harness = folder_harness(directory.path(), size);
            for mode in ["Split", "Code"] {
                harness.get_by_label(mode).click();
                settle(&mut harness);
                capture(
                    &mut harness,
                    &format!("editor-{}-{suffix}", mode.to_lowercase()),
                    None,
                );
            }
            // Line 2 holds the description; the caret goes to its end.
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
            settle(&mut harness);
            capture(
                &mut harness,
                &format!("editor-code-scrolled-{suffix}"),
                None,
            );
        }
    }

    /// The block library folded to its rail (KTLO-17) in Diagram and Split, and
    /// the expanded panel's caption row with its chevron, at both windows.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_library_rail() {
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let project = shop_project();
            let mut harness = folder_harness(project.path(), size);
            harness.get_by_label("Diagram").click();
            settle(&mut harness);
            let caption = harness.get_by_label("Block library").rect();
            let chevron = harness.get_by_label("Collapse block library").rect();
            let region = caption.union(chevron).expand2(egui::vec2(16.0, 12.0));
            capture(
                &mut harness,
                &format!("library-caption-{suffix}"),
                Some(region),
            );
            harness.get_by_label("Collapse block library").click();
            settle(&mut harness);
            for mode in ["Diagram", "Split"] {
                harness.get_by_label(mode).click();
                settle(&mut harness);
                capture(
                    &mut harness,
                    &format!("library-rail-{}-{suffix}", mode.to_lowercase()),
                    None,
                );
            }
        }
    }

    /// The sleeping dog on an empty board (KTLO-20): Diagram and the board
    /// side of Split with the last tab closed, at both window sizes and at
    /// double scale, which criterion 7 reviews by eye. The loop is off in the
    /// harness, so these show its rest frame.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_empty_board() {
        for (size, scale, suffix) in [
            (MINIMUM, 1.0, "960x540"),
            (WIDE, 1.0, "1600x1000"),
            (MINIMUM, 2.0, "960x540@2x"),
        ] {
            let project = shop_project();
            let mut harness = folder_harness_scaled(project.path(), size, scale);
            // The demo project opens its scenario by itself the first time.
            harness
                .get_by_label("Close promotion-and-cart-price.afs")
                .click();
            settle(&mut harness);
            for mode in ["Diagram", "Split"] {
                harness.get_by_label(mode).click();
                settle(&mut harness);
                capture(
                    &mut harness,
                    &format!("empty-{}-{suffix}", mode.to_lowercase()),
                    None,
                );
            }
        }
    }

    // ---- Throwaway mock-up for KTLO-23: a component runs from its tab ----

    const MOCK_COMPONENT: &str = "components/add-to-cart.afs";
    const MOCK_SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
    const MOCK_HEADER: [&str; 9] = [
        "Project",
        "Code",
        "Diagram",
        "Split",
        "Save",
        "Save all",
        "Publish",
        "Run options",
        "Run",
    ];

    fn mock_app(harness: &mut FolderHarness) -> &mut FolderApp {
        harness.state_mut().as_mut().unwrap()
    }

    /// The component tab active, in `mode`.
    fn mock_component_tab(size: egui::Vec2, mode: &str) -> (tempfile::TempDir, FolderHarness) {
        let project = shop_project();
        let mut harness = folder_harness(project.path(), size);
        mock_app(&mut harness).mock_open(MOCK_COMPONENT);
        settle(&mut harness);
        harness.get_by_label(mode).click();
        settle(&mut harness);
        (project, harness)
    }

    /// The header row's widgets, their edges and the gaps between them.
    fn mock_measure_header(harness: &FolderHarness, view: &str) -> String {
        let window = harness.ctx.content_rect();
        let title = harness
            .query_all_by_label("Demo shop")
            .map(|node| node.rect())
            .find(|rect| rect.top() < 50.0)
            .unwrap();
        let mut out = format!("{view}: window {:.0} px\n", window.width());
        out += &format!("  title  {:>6.1}..{:>6.1}\n", title.left(), title.right());
        let rects: Vec<(&str, egui::Rect)> = MOCK_HEADER
            .iter()
            .map(|label| (*label, harness.get_by_label(label).rect()))
            .collect();
        for (label, rect) in &rects {
            out += &format!(
                "  {label:<12} {:>6.1}..{:>6.1} (w {:>5.1}) inside {}\n",
                rect.left(),
                rect.right(),
                rect.width(),
                window.contains_rect(*rect)
            );
        }
        let code = rects[1].1;
        let run = rects[8].1;
        let save = rects[4].1;
        out += &format!(
            "  toolbar Code..Run {:.1} px (reserved 620); Save..Run {:.1} px; \
             title right {:.1} vs Code left {:.1}; Run right margin {:.1}\n",
            run.right() - code.left(),
            run.right() - save.left(),
            title.right(),
            code.left(),
            window.right() - run.right()
        );
        out
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mock_23_component_run() {
        let mut measured = String::new();
        // 1. The header with the component tab active, in every mode.
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            for mode in ["Code", "Split", "Diagram"] {
                let (_project, harness) = mock_component_tab(size, mode);
                measured += &mock_measure_header(&harness, &format!("{mode} {suffix}"));
                drop(harness);
            }
            let (_project, mut harness) = mock_component_tab(size, "Diagram");
            capture(
                &mut harness,
                &format!("mock-header-component-{suffix}"),
                None,
            );
            let header = MOCK_HEADER
                .iter()
                .map(|label| harness.get_by_label(label).rect())
                .reduce(|a, b| a.union(b))
                .unwrap()
                .expand2(egui::vec2(12.0, 8.0));
            capture(
                &mut harness,
                &format!("mock-header-component-row-{suffix}"),
                Some(header.with_min_x(0.0).with_max_x(size.x)),
            );
        }
        // For comparison: the scenario tab as on main today.
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let project = shop_project();
            let mut harness = folder_harness(project.path(), size);
            harness.get_by_label("Diagram").click();
            settle(&mut harness);
            let view = format!("scenario tab {suffix}");
            let window = harness.ctx.content_rect();
            let code = harness.get_by_label("Code").rect();
            let run = harness.get_by_label("Run").rect();
            measured += &format!(
                "{view}: toolbar Code..Run {:.1} px, Run right margin {:.1}\n",
                run.right() - code.left(),
                window.right() - run.right()
            );
        }

        // 1b. A long project name at 960 px: the title against Code, with the
        // reservation of main (620) and larger ones.
        let long = "Customer integration and contract verification workspace";
        for reserved in [620u32, 640, 650] {
            crate::folder::MOCK_RESERVED.store(reserved, std::sync::atomic::Ordering::Relaxed);
            for kind in ["component", "scenario"] {
                let directory = tempfile::tempdir().unwrap();
                let mut demo = apiforge_core::demos::shop_project();
                demo.name = long.into();
                apiforge_project::prepare_project(directory.path(), &demo).unwrap();
                let mut harness = folder_harness(directory.path(), MINIMUM);
                if kind == "component" {
                    mock_app(&mut harness).mock_open(MOCK_COMPONENT);
                    settle(&mut harness);
                }
                harness.get_by_label("Diagram").click();
                settle(&mut harness);
                let title = harness
                    .query_all_by_label(long)
                    .map(|node| node.rect())
                    .find(|rect| rect.top() < 50.0)
                    .unwrap();
                let code = harness.get_by_label("Code").rect();
                let run = harness.get_by_label("Run").rect();
                measured += &format!(
                    "long name, {kind} tab, reserved {reserved}: title {:.1}..{:.1} (w {:.1}), \
                     Code left {:.1}, gap {:.1}, Run right {:.1}\n",
                    title.left(),
                    title.right(),
                    title.width(),
                    code.left(),
                    code.left() - title.right(),
                    run.right()
                );
                let row = egui::Rect::from_min_max(
                    egui::pos2(0.0, 0.0),
                    egui::pos2(MINIMUM.x, run.bottom() + 8.0),
                );
                capture(
                    &mut harness,
                    &format!("mock-header-long-name-{kind}-{reserved}-960x540"),
                    Some(row),
                );
            }
        }
        crate::folder::MOCK_RESERVED.store(620, std::sync::atomic::Ordering::Relaxed);

        // 2. Run results after a component run, the export step selected.
        let (_project, mut harness) = mock_component_tab(WIDE, "Diagram");
        mock_app(&mut harness).mock_component_run(MOCK_COMPONENT, MOCK_SCENARIO);
        settle(&mut harness);
        capture(&mut harness, "mock-run-results-component-1600x1000", None);
        drop(harness);

        // 3. The Run inputs dialog over the component's board.
        for (size, suffix, typed) in [
            (MINIMUM, "960x540", vec![]),
            (WIDE, "1600x1000", vec![]),
            (
                WIDE,
                "1600x1000-filled",
                vec![("product", "{ \"id\": 123 }"), ("quantity", "2")],
            ),
        ] {
            let (_project, mut harness) = mock_component_tab(size, "Diagram");
            mock_app(&mut harness).mock_run_inputs(MOCK_COMPONENT, &typed);
            settle(&mut harness);
            capture(&mut harness, &format!("mock-run-inputs-{suffix}"), None);
        }

        std::fs::create_dir_all(capture_dir()).unwrap();
        std::fs::write(capture_dir().join("mock-23-measurements.txt"), &measured).unwrap();
        eprintln!("{measured}");
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_workspace_window() {
        let mut harness = workspace_harness(WIDE);
        capture(&mut harness, "workspace-1600x1000", None);
    }
}

/// Serves egui_inspection from a harness until killed or until
/// `APIFORGE_HOST_SECONDS` (default 3600) pass. Opens `APIFORGE_HOST_FOLDER`,
/// or the demo shop in a temporary directory. Started by `tools/gui/host.sh`.
#[test]
#[ignore = "interactive egui_mcp host; run with tools/gui/host.sh headless"]
fn inspection_host() {
    let demo = shop_project();
    let root = std::env::var_os("APIFORGE_HOST_FOLDER")
        .map(PathBuf::from)
        .unwrap_or_else(|| demo.path().to_owned());
    let seconds = std::env::var("APIFORGE_HOST_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3600);
    let mut harness = folder_harness(&root, WIDE);
    // A free loopback port, so parallel sessions never share a host.
    let address = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .unwrap()
        .to_string();
    harness
        .ctx
        .add_plugin(egui_inspection::InspectionPlugin::new(Some(
            "apiforge-desktop (headless)".into(),
        )));
    egui_inspection::serve(&harness.ctx, &address).unwrap();
    println!("HOST READY {address}");
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < deadline {
        harness.step();
        std::thread::sleep(Duration::from_millis(8));
    }
}
