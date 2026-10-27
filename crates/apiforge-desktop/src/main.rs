//! API Forge desktop application.
//!
//! Opening a document never executes a request.

mod app;
mod board_edit;
mod buffers;
mod canvas;
mod capture;
mod code;
mod dialogs;
mod dnd;
mod editor;
mod files;
mod folder;
mod icons;
mod inspector;
mod labels;
mod library;
mod navigator;
mod persistence;
mod preferences;
mod report;
mod shapes;
#[cfg(test)]
mod testing;
mod theme;
mod widgets;

use apiforge_storage::WorkspaceStore;
use std::{
    env,
    ffi::OsString,
    path::PathBuf,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq)]
enum Arguments {
    /// A document opened for this session only, which is never written back.
    Window(PathBuf),
    /// A folder project, opened explicitly.
    Folder(PathBuf),
    /// Whatever the application data directory holds, migrating or creating a
    /// folder project as needed. This is what starting without arguments does.
    Managed,
    /// The saved JSON workspace, for a session that wants the previous
    /// application after folder projects became the default.
    Legacy,
    Check(PathBuf),
    SmokeTest(PathBuf),
    /// Captures the window; without a project the demo workspace is shown.
    Screenshot {
        output: PathBuf,
        project: Option<PathBuf>,
        folder: Option<PathBuf>,
        select: Option<String>,
        /// The folder window's mode; without it, the one the folder restores.
        mode: Option<folder::Mode>,
        /// The window's inner size in logical points, width and height.
        size: Option<(u32, u32)>,
    },
    Help,
}

fn arguments(values: impl IntoIterator<Item = OsString>) -> Result<Arguments, &'static str> {
    let mut values = values.into_iter();
    let first = values.next();
    let mut path = |message| values.next().map(PathBuf::from).ok_or(message);
    let result = match first.as_ref().map(|value| value.to_str()) {
        None => Arguments::Managed,
        Some(Some("--help" | "-h")) => Arguments::Help,
        Some(Some("--project")) => Arguments::Window(path("Missing file path")?),
        Some(Some("--folder")) => Arguments::Folder(path("Missing folder path")?),
        Some(Some("--managed")) => Arguments::Managed,
        Some(Some("--legacy")) => Arguments::Legacy,
        Some(Some("--smoke-test")) => Arguments::SmokeTest(path("Missing file path")?),
        Some(Some("--check")) => Arguments::Check(path("Missing file path")?),
        Some(Some("--screenshot")) => {
            let output = path("Missing screenshot PNG path")?;
            let (mut project, mut folder, mut select) = (None, None, None);
            let (mut mode, mut size) = (None, None);
            while let Some(flag) = values.next() {
                let value = values
                    .next()
                    .ok_or("Missing value after a screenshot option")?;
                match flag.to_str() {
                    Some("--project") => project = Some(PathBuf::from(value)),
                    Some("--folder") => folder = Some(PathBuf::from(value)),
                    Some("--select") => select = Some(value.to_string_lossy().into_owned()),
                    Some("--mode") => mode = Some(view_mode(value.to_str())?),
                    Some("--size") => size = Some(window_size(value.to_str())?),
                    _ => {
                        return Err(
                            "Unknown screenshot option; use --project FILE, --folder PATH, --select NODE, --mode MODE or --size WxH",
                        );
                    }
                }
            }
            if project.is_some() && folder.is_some() {
                return Err("Use either --project or --folder, not both");
            }
            if mode.is_some() && folder.is_none() {
                return Err(
                    "--mode applies only to a --folder capture; the workspace window has no modes",
                );
            }
            Arguments::Screenshot {
                output,
                project,
                folder,
                select,
                mode,
                size,
            }
        }
        _ => return Err("Unknown argument; use --help"),
    };
    if values.next().is_some() {
        return Err("Unexpected extra argument");
    }
    Ok(result)
}

fn view_mode(value: Option<&str>) -> Result<folder::Mode, &'static str> {
    match value {
        Some("code") => Ok(folder::Mode::Code),
        Some("diagram") => Ok(folder::Mode::Diagram),
        Some("split") => Ok(folder::Mode::Split),
        _ => Err("--mode takes code, diagram or split"),
    }
}

/// `WIDTHxHEIGHT` in logical points, at least the minimum window.
fn window_size(value: Option<&str>) -> Result<(u32, u32), &'static str> {
    const MALFORMED: &str = "--size takes WIDTHxHEIGHT, for example 960x540";
    let (width, height) = value
        .and_then(|value| value.split_once('x'))
        .ok_or(MALFORMED)?;
    let number = |text: &str| {
        // `parse` would also take a leading `+`.
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(MALFORMED);
        }
        text.parse::<u32>().map_err(|_| MALFORMED)
    };
    let size = (number(width)?, number(height)?);
    if (size.0 as f32) < MIN_WINDOW_SIZE[0] || (size.1 as f32) < MIN_WINDOW_SIZE[1] {
        return Err("--size is below the minimum window of 960x540");
    }
    Ok(size)
}

/// Where managed startup begins, and whether it can.
///
/// Opening a folder project is the window's own asynchronous work, so almost
/// nothing happens here. The exception is a legacy workspace that cannot be
/// migrated yet: another window owns it, or it needs an explicit backup
/// decision. Only the workspace window can resolve either, and which window to
/// build has to be decided before one exists, so that case starts exactly as it
/// does today.
fn managed_opening() -> Result<Opening, String> {
    let data = dirs::data_dir()
        .ok_or("This system has no application data directory.")?
        .join("apiforge");
    match folder::managed_start(&data) {
        folder::Start::Managed => Ok(Opening::Managed(data)),
        folder::Start::Legacy(reason) => {
            if let Some(reason) = reason {
                eprintln!("Opening the workspace instead: {reason}");
            }
            let store = WorkspaceStore::default_location().map_err(|error| error.to_string())?;
            Ok(Opening::Workspace(app::Source::Storage(store)))
        }
        folder::Start::Refused(refusal) => {
            eprintln!("Opening the workspace instead: {}", refusal.reason);
            let store = WorkspaceStore::default_location().map_err(|error| error.to_string())?;
            Ok(Opening::Workspace(app::Source::Storage(store)))
        }
    }
}

const HELP: &str = "API Forge

  apiforge-desktop                      open your projects (saved automatically)
  apiforge-desktop --project FILE       open a project or workspace for this session only
  apiforge-desktop --folder PATH        open one folder project
  apiforge-desktop --managed            the same as no arguments, stated explicitly
  apiforge-desktop --legacy             open the previous JSON workspace instead
  apiforge-desktop --check FILE         validate a document without opening a window
  apiforge-desktop --smoke-test FILE    open a window, draw a few frames and exit
  apiforge-desktop --screenshot OUT.png [--project FILE | --folder PATH] [--select NODE]
                   [--mode code|diagram|split] [--size WxH]
                                        capture the window and exit; --mode needs --folder
                                        (default: the folder's own mode), --size defaults
                                        to 1600x1000 and is at least 960x540

Opening a document never executes requests.";

fn main() -> ExitCode {
    let arguments = match arguments(env::args_os().skip(1)) {
        Ok(arguments) => arguments,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match arguments {
        Arguments::Help => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Arguments::Check(path) => match files::read_document(&path) {
            Ok(document) => {
                println!(
                    "Valid v{} document: {} project(s)",
                    document.workspace().version,
                    document.workspace().projects.len()
                );
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        },
        Arguments::Legacy => match WorkspaceStore::default_location() {
            Ok(store) => run_window(Opening::Workspace(app::Source::Storage(store)), Probe::None),
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Arguments::Window(path) => session(&path, Probe::None),
        Arguments::Folder(path) => run_window(Opening::Folder(path, None), Probe::None),
        Arguments::Managed => match managed_opening() {
            Ok(opening) => run_window(opening, Probe::None),
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Arguments::SmokeTest(path) => session(&path, Probe::Smoke),
        Arguments::Screenshot {
            output,
            folder: Some(folder),
            select,
            mode,
            size,
            ..
        } => run_window(
            Opening::Folder(folder, mode),
            Probe::Screenshot(output, select, size),
        ),
        Arguments::Screenshot {
            output,
            project,
            select,
            size,
            ..
        } => {
            let workspace = match project {
                Some(project) => match files::read_document(&project) {
                    Ok(document) => document.into_workspace(),
                    Err(message) => {
                        eprintln!("{message}");
                        return ExitCode::FAILURE;
                    }
                },
                None => app::default_workspace(),
            };
            run_window(
                Opening::Workspace(app::Source::Session(workspace)),
                Probe::Screenshot(output, select, size),
            )
        }
    }
}

fn session(path: &std::path::Path, probe: Probe) -> ExitCode {
    match files::read_document(path) {
        Ok(document) => run_window(
            Opening::Workspace(app::Source::Session(document.into_workspace())),
            probe,
        ),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Automated checks that close the window by themselves.
enum Probe {
    None,
    Smoke,
    /// Capture the window, optionally with a node (by id or name) selected,
    /// at a window size other than the usual one.
    Screenshot(PathBuf, Option<String>, Option<(u32, u32)>),
}

/// What the window opens: the workspace, or a folder project.
enum Opening {
    Workspace(app::Source),
    /// A folder project, in the given mode instead of its own.
    Folder(PathBuf, Option<folder::Mode>),
    /// The application data directory, resolved to a project after the window
    /// exists so that migrating one does not delay it.
    Managed(PathBuf),
}

/// The workspace window, or the folder project window.
enum Front {
    Workspace(Box<app::App>),
    Folder(Box<folder::FolderApp>),
}

impl Front {
    fn ui(&mut self, ui: &mut eframe::egui::Ui) {
        match self {
            Self::Workspace(app) => app.ui(ui),
            Self::Folder(app) => app.ui(ui),
        }
    }

    fn has_document(&self) -> bool {
        match self {
            Self::Workspace(app) => app.has_document(),
            Self::Folder(app) => app.has_document(),
        }
    }

    fn prepare_close(&mut self) -> bool {
        match self {
            Self::Workspace(app) => app.prepare_close(),
            Self::Folder(app) => app.prepare_close(),
        }
    }

    fn close_error(&mut self) -> Option<String> {
        match self {
            Self::Workspace(app) => app.close_save_error(),
            Self::Folder(app) => app.close_error(),
        }
    }

    fn abort_close(&mut self, error: String) {
        match self {
            Self::Workspace(app) => app.abort_close_after_save_error(error),
            Self::Folder(app) => app.abort_close(error),
        }
    }

    /// Background saving and run cleanup keep the owning session alive.
    fn save_pending(&self) -> bool {
        match self {
            Self::Workspace(app) => app.save_pending(),
            Self::Folder(app) => app.save_pending(),
        }
    }

    fn select_node(&mut self, node: &str) {
        if let Self::Workspace(app) = self {
            app.select_node(node);
        }
    }
}

struct Window {
    app: Front,
    capture: Option<capture::Capture>,
    smoke: bool,
    frames: u8,
    completed: Arc<AtomicBool>,
    closing_since: Option<Instant>,
}

/// Cleanup has a bounded runner budget. Leave a little time for its final report
/// without allowing an unresponsive endpoint to keep the window open forever.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(12);

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        if ui.ctx().input(|input| input.viewport().close_requested()) {
            self.closing_since.get_or_insert_with(Instant::now);
        }
        if let Some(started) = self.closing_since {
            let ready = self.app.prepare_close();
            if let Some(error) = self.app.close_error() {
                self.app.abort_close(error);
                self.closing_since = None;
                ui.ctx()
                    .send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
            } else if ready || (started.elapsed() >= CLOSE_TIMEOUT && !self.app.save_pending()) {
                ui.ctx()
                    .send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            } else {
                ui.ctx()
                    .send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
        }
        ui.add_enabled_ui(self.closing_since.is_none(), |ui| self.app.ui(ui));
        if let Some(capture) = &mut self.capture {
            if let Some(result) = capture.update(ui.ctx(), self.app.has_document()) {
                if let Err(message) = &result {
                    eprintln!("{message}");
                }
                self.completed.store(result.is_ok(), Ordering::Release);
                ui.ctx()
                    .send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            }
        } else if self.smoke {
            self.frames += 1;
            if self.frames >= 3 {
                self.completed.store(true, Ordering::Release);
                ui.ctx()
                    .send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            } else {
                ui.ctx().request_repaint();
            }
        }
    }
}

/// The window's logical size when it opens.
const DEFAULT_WINDOW_SIZE: [f32; 2] = [1600.0, 1000.0];

/// Minimum logical window size: 1920×1080 at 200 % scaling (G6).
const MIN_WINDOW_SIZE: [f32; 2] = [960.0, 540.0];

fn run_window(opening: Opening, probe: Probe) -> ExitCode {
    let automated = !matches!(probe, Probe::None);
    let completed = Arc::new(AtomicBool::new(!automated));
    let size = match probe {
        Probe::Screenshot(_, _, Some((width, height))) => [width as f32, height as f32],
        _ => DEFAULT_WINDOW_SIZE,
    };
    let observed = Arc::clone(&completed);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("API Forge")
            .with_app_id("apiforge")
            .with_inner_size(size)
            .with_min_inner_size(MIN_WINDOW_SIZE),
        ..Default::default()
    };
    let started = eframe::run_native(
        "API Forge",
        options,
        Box::new(move |context| {
            let (capture, smoke, select) = match probe {
                Probe::None => (None, false, None),
                Probe::Smoke => (None, true, None),
                Probe::Screenshot(path, select, _) => {
                    (Some(capture::Capture::new(path)), false, select)
                }
            };
            let mut app = match opening {
                Opening::Workspace(source) => {
                    Front::Workspace(Box::new(app::App::new(&context.egui_ctx, source)))
                }
                Opening::Folder(root, mode) => {
                    let mut folder = folder::FolderApp::open(&context.egui_ctx, root);
                    if let Some(mode) = mode {
                        folder.request_mode(mode);
                    }
                    Front::Folder(Box::new(folder))
                }
                Opening::Managed(data) => Front::Folder(Box::new(
                    folder::FolderApp::start_managed(&context.egui_ctx, data),
                )),
            };
            if let Some(node) = select {
                app.select_node(&node);
            }
            Ok(Box::new(Window {
                app,
                capture,
                smoke,
                frames: 0,
                completed,
                closing_since: None,
            }))
        }),
    );
    match started {
        Ok(()) if observed.load(Ordering::Acquire) => ExitCode::SUCCESS,
        Ok(()) => ExitCode::FAILURE,
        Err(_) => {
            eprintln!(
                "Cannot initialise the native window. Check GPU drivers and the display session."
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Arguments, &'static str> {
        arguments(args.iter().map(OsString::from))
    }

    #[test]
    fn arguments_are_explicit_and_reject_ambiguous_input() {
        // Starting with no arguments is managed startup: folder projects are
        // the application, and the JSON workspace is what `--legacy` asks for.
        assert_eq!(parse(&[]), Ok(Arguments::Managed));
        assert_eq!(parse(&["--legacy"]), Ok(Arguments::Legacy));
        assert!(parse(&["--legacy", "somewhere"]).is_err());
        assert_eq!(
            parse(&["--check", "a b.json"]),
            Ok(Arguments::Check("a b.json".into()))
        );
        assert!(parse(&["--check"]).is_err());
        assert!(parse(&["--project", "x.json", "--check", "y.json"]).is_err());
        assert!(parse(&["arbitrary.json"]).is_err());
        assert_eq!(
            parse(&["--folder", "my project"]),
            Ok(Arguments::Folder("my project".into()))
        );
        assert!(parse(&["--folder"]).is_err());
        // Managed startup names no path: it is the application data directory
        // or nothing, and it is not the default.
        assert_eq!(parse(&["--managed"]), Ok(Arguments::Managed));
        assert!(parse(&["--managed", "somewhere"]).is_err());
        assert!(
            parse(&[
                "--screenshot",
                "a.png",
                "--project",
                "x.json",
                "--folder",
                "y"
            ])
            .is_err()
        );
    }

    fn screenshot(
        folder: Option<&str>,
        project: Option<&str>,
        mode: Option<folder::Mode>,
        size: Option<(u32, u32)>,
    ) -> Arguments {
        Arguments::Screenshot {
            output: "a.png".into(),
            project: project.map(PathBuf::from),
            folder: folder.map(PathBuf::from),
            select: None,
            mode,
            size,
        }
    }

    /// KTLO-08: QA captures of the folder window in each mode at both
    /// standard sizes.
    #[test]
    fn screenshot_takes_a_folder_mode_and_a_window_size() {
        use folder::Mode;
        for (word, mode) in [
            ("code", Mode::Code),
            ("diagram", Mode::Diagram),
            ("split", Mode::Split),
        ] {
            for (text, size) in [("960x540", (960, 540)), ("1600x1000", (1600, 1000))] {
                assert_eq!(
                    parse(&[
                        "--screenshot",
                        "a.png",
                        "--folder",
                        "p",
                        "--mode",
                        word,
                        "--size",
                        text
                    ]),
                    Ok(screenshot(Some("p"), None, Some(mode), Some(size)))
                );
            }
        }
        // Options come in any order, and each one stays optional.
        assert_eq!(
            parse(&[
                "--screenshot",
                "a.png",
                "--size",
                "960x540",
                "--mode",
                "split",
                "--folder",
                "p"
            ]),
            Ok(screenshot(
                Some("p"),
                None,
                Some(Mode::Split),
                Some((960, 540))
            ))
        );
        assert_eq!(
            parse(&[
                "--screenshot",
                "a.png",
                "--folder",
                "p",
                "--mode",
                "diagram"
            ]),
            Ok(screenshot(Some("p"), None, Some(Mode::Diagram), None))
        );
        // Without either option nothing changes: the folder's own mode, the
        // usual window.
        assert_eq!(
            parse(&["--screenshot", "a.png", "--folder", "p"]),
            Ok(screenshot(Some("p"), None, None, None))
        );
        // The legacy window has a size but no modes.
        assert_eq!(
            parse(&[
                "--screenshot",
                "a.png",
                "--project",
                "x.json",
                "--size",
                "960x540"
            ]),
            Ok(screenshot(None, Some("x.json"), None, Some((960, 540))))
        );
    }

    #[test]
    fn screenshot_rejects_a_mode_without_a_folder_and_a_malformed_size() {
        for legacy in [
            &[
                "--screenshot",
                "a.png",
                "--project",
                "x.json",
                "--mode",
                "code",
            ][..],
            &["--screenshot", "a.png", "--mode", "split"][..],
        ] {
            let error = parse(legacy).expect_err("the legacy window has no modes");
            assert!(
                error.contains("--mode") && error.contains("--folder"),
                "the error says --mode needs --folder: {error}"
            );
        }
        for mode in ["Code", "board", ""] {
            assert!(
                parse(&["--screenshot", "a.png", "--folder", "p", "--mode", mode]).is_err(),
                "{mode:?} is not a mode"
            );
        }
        assert!(parse(&["--screenshot", "a.png", "--folder", "p", "--mode"]).is_err());
        for size in [
            "960",
            "960x",
            "x540",
            "960x540x2",
            "wxh",
            "-960x540",
            "960 x 540",
            "0x0",
            // Below the window's minimum the capture would not have that size.
            "959x540",
            "960x539",
        ] {
            let error =
                parse(&["--screenshot", "a.png", "--folder", "p", "--size", size]).expect_err(size);
            assert!(error.contains("--size"), "the error names --size: {error}");
        }
    }
}
