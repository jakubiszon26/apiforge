//! API Forge desktop application.
//!
//! Opening a document never executes a request.

mod app;
mod canvas;
mod capture;
mod dialogs;
mod dnd;
mod editor;
mod files;
mod icons;
mod inspector;
mod labels;
mod library;
mod navigator;
mod persistence;
mod preferences;
mod report;
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
    /// The user's workspace, or a document opened for this session only.
    Window(Option<PathBuf>),
    Check(PathBuf),
    SmokeTest(PathBuf),
    /// Captures the window; without a project the demo workspace is shown.
    Screenshot {
        output: PathBuf,
        project: Option<PathBuf>,
        select: Option<String>,
    },
    Help,
}

fn arguments(values: impl IntoIterator<Item = OsString>) -> Result<Arguments, &'static str> {
    let mut values = values.into_iter();
    let first = values.next();
    let mut path = |message| values.next().map(PathBuf::from).ok_or(message);
    let result = match first.as_ref().map(|value| value.to_str()) {
        None => Arguments::Window(None),
        Some(Some("--help" | "-h")) => Arguments::Help,
        Some(Some("--project")) => Arguments::Window(Some(path("Missing file path")?)),
        Some(Some("--smoke-test")) => Arguments::SmokeTest(path("Missing file path")?),
        Some(Some("--check")) => Arguments::Check(path("Missing file path")?),
        Some(Some("--screenshot")) => {
            let output = path("Missing screenshot PNG path")?;
            let (mut project, mut select) = (None, None);
            while let Some(flag) = values.next() {
                let value = values
                    .next()
                    .ok_or("Missing value after a screenshot option")?;
                match flag.to_str() {
                    Some("--project") => project = Some(PathBuf::from(value)),
                    Some("--select") => select = Some(value.to_string_lossy().into_owned()),
                    _ => {
                        return Err(
                            "Unknown screenshot option; use --project FILE or --select NODE",
                        );
                    }
                }
            }
            Arguments::Screenshot {
                output,
                project,
                select,
            }
        }
        _ => return Err("Unknown argument; use --help"),
    };
    if values.next().is_some() {
        return Err("Unexpected extra argument");
    }
    Ok(result)
}

const HELP: &str = "API Forge

  apiforge-desktop                      open your workspace (saved automatically)
  apiforge-desktop --project FILE       open a project or workspace for this session only
  apiforge-desktop --check FILE         validate a document without opening a window
  apiforge-desktop --smoke-test FILE    open a window, draw a few frames and exit
  apiforge-desktop --screenshot OUT.png [--project FILE] [--select NODE]

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
                    "Valid v1 document: {} project(s)",
                    document.workspace().projects.len()
                );
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        },
        Arguments::Window(None) => match WorkspaceStore::default_location() {
            Ok(store) => run_window(app::Source::Storage(store), Probe::None),
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Arguments::Window(Some(path)) => session(&path, Probe::None),
        Arguments::SmokeTest(path) => session(&path, Probe::Smoke),
        Arguments::Screenshot {
            output,
            project,
            select,
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
                app::Source::Session(workspace),
                Probe::Screenshot(output, select),
            )
        }
    }
}

fn session(path: &std::path::Path, probe: Probe) -> ExitCode {
    match files::read_document(path) {
        Ok(document) => run_window(app::Source::Session(document.into_workspace()), probe),
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
    /// Capture the window, optionally with a node (by id or name) selected.
    Screenshot(PathBuf, Option<String>),
}

struct Window {
    app: app::App,
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
            if let Some(error) = self.app.close_save_error() {
                self.app.abort_close_after_save_error(error);
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

/// Minimum logical window size: 1920×1080 at 200 % scaling (G6).
const MIN_WINDOW_SIZE: [f32; 2] = [960.0, 540.0];

fn run_window(source: app::Source, probe: Probe) -> ExitCode {
    let automated = !matches!(probe, Probe::None);
    let completed = Arc::new(AtomicBool::new(!automated));
    let observed = Arc::clone(&completed);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("API Forge")
            .with_app_id("apiforge")
            .with_inner_size([1600.0, 1000.0])
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
                Probe::Screenshot(path, select) => {
                    (Some(capture::Capture::new(path)), false, select)
                }
            };
            let mut app = app::App::new(&context.egui_ctx, source);
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
        assert_eq!(parse(&[]), Ok(Arguments::Window(None)));
        assert_eq!(
            parse(&["--check", "a b.json"]),
            Ok(Arguments::Check("a b.json".into()))
        );
        assert!(parse(&["--check"]).is_err());
        assert!(parse(&["--project", "x.json", "--check", "y.json"]).is_err());
        assert!(parse(&["arbitrary.json"]).is_err());
    }
}
