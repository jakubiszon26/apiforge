//! Native document entry point. No network request is executed by opening a project.
mod capture;
mod document_io;
mod graph_view;
mod inspector;
mod theme;
mod viewer;

use apiforge_core::ImportLimits;
use std::{
    env,
    ffi::OsString,
    path::PathBuf,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, PartialEq)]
enum Arguments {
    Window(Option<PathBuf>, bool),
    Check(PathBuf),
    Screenshot { output: PathBuf, project: PathBuf },
    Help,
}
fn arguments(values: impl IntoIterator<Item = OsString>) -> Result<Arguments, &'static str> {
    let mut values = values.into_iter();
    let first = values.next();
    let result = match first.as_ref().and_then(|value| value.to_str()) {
        None if first.is_none() => Arguments::Window(None, false),
        Some("--help" | "-h") => Arguments::Help,
        Some("--project") => Arguments::Window(
            Some(values.next().ok_or("Missing file path")?.into()),
            false,
        ),
        Some("--smoke-test") => {
            Arguments::Window(Some(values.next().ok_or("Missing file path")?.into()), true)
        }
        Some("--screenshot") => {
            let output = values.next().ok_or("Missing screenshot PNG path")?.into();
            if values.next().as_deref() != Some(std::ffi::OsStr::new("--project")) {
                return Err("Screenshot requires --project FILE");
            }
            let project = values.next().ok_or("Missing project file")?.into();
            Arguments::Screenshot { output, project }
        }
        Some("--check") => Arguments::Check(values.next().ok_or("Missing file path")?.into()),
        _ => return Err("Unknown argument; use --help"),
    };
    if values.next().is_some() {
        return Err("Unexpected extra argument");
    }
    Ok(result)
}
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
            println!(
                "API Forge\n\n  apiforge-desktop [--project FILE]\n  apiforge-desktop --check FILE\n  apiforge-desktop --smoke-test FILE\n  apiforge-desktop --screenshot OUTPUT.png --project FILE\n\nImports project/workspace JSON v1 without executing requests."
            );
            ExitCode::SUCCESS
        }
        Arguments::Check(path) => match document_io::read_document(&path, &ImportLimits::default())
        {
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
        Arguments::Window(path, smoke_test) => run_window(path, smoke_test, None),
        Arguments::Screenshot { output, project } => run_window(Some(project), false, Some(output)),
    }
}
fn run_window(path: Option<PathBuf>, smoke_test: bool, screenshot: Option<PathBuf>) -> ExitCode {
    let completed = (smoke_test || screenshot.is_some()).then(|| Arc::new(AtomicBool::new(false)));
    let observed = completed.clone();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("API Forge")
            .with_inner_size([1600.0, 1000.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    match eframe::run_native(
        "API Forge",
        options,
        Box::new(move |context| {
            Ok(Box::new(viewer::Desktop::new(
                context.egui_ctx.clone(),
                path,
                completed,
                screenshot,
            )))
        }),
    ) {
        Ok(())
            if observed
                .as_ref()
                .is_none_or(|status| status.load(Ordering::Acquire)) =>
        {
            ExitCode::SUCCESS
        }
        Ok(()) => ExitCode::FAILURE,
        Err(_) => {
            eprintln!(
                "Cannot initialize the native window. Check GPU drivers and the display session."
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
        assert_eq!(parse(&[]), Ok(Arguments::Window(None, false)));
        assert_eq!(
            parse(&["--check", "a b.json"]),
            Ok(Arguments::Check("a b.json".into()))
        );
        assert!(parse(&["--check"]).is_err());
        assert!(parse(&["--project", "x.json", "--check", "y.json"]).is_err());
        assert!(parse(&["arbitrary.json"]).is_err());
    }
}
