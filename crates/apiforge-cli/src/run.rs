//! `apiforge run`: executes one scenario of a workspace and reports it.
//!
//! The command runs the document, never a file of source: `apply` is how text
//! becomes a document. A simulation sends nothing and is the default; `--live`
//! sends real requests, holds the workspace's writer lock and journals what it
//! sends, exactly as the application does, so an interrupted run leaves the same
//! record behind.
//!
//! Secrets are read from the environment or from a file, never from the command
//! line, where they would survive in shell history and in the process list.

use apiforge_core::format::OptionalJson;
use apiforge_core::run::{RunMode, RunResult, RunStatus, StepResult, StepStatus};
use apiforge_mcp::executor::{self, CancellationToken, DEADLINE, EndedBy};
use apiforge_runner::RunRequest;
use apiforge_storage::{JournalStore, WorkspaceStore};
use std::{ffi::OsString, path::PathBuf};

pub const USAGE: &str = "apiforge run (--workspace <doc.json> | --folder <directory>) --scenario <name|id|relative.afs> [--live] [--secrets <file.json>] [--json]";

pub struct Args {
    pub path: PathBuf,
    pub folder: bool,
    pub scenario: String,
    pub live: bool,
    pub secrets: Option<PathBuf>,
    pub json: bool,
}

pub fn arguments(args: &[OsString]) -> Option<Args> {
    if args.first()?.to_str()? != "run" {
        return None;
    }
    let (mut path, mut scenario, mut secrets) = (None, None, None);
    let (mut live, mut json, mut folder) = (false, false, false);
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.to_str()? {
            "--workspace" if path.is_none() => path = Some(PathBuf::from(rest.next()?)),
            "--folder" if path.is_none() => {
                path = Some(PathBuf::from(rest.next()?));
                folder = true;
            }
            "--scenario" if scenario.is_none() => {
                scenario = Some(rest.next()?.to_str()?.to_owned());
            }
            "--secrets" if secrets.is_none() => secrets = Some(PathBuf::from(rest.next()?)),
            "--live" if !live => live = true,
            "--json" if !json => json = true,
            _ => return None,
        }
    }
    Some(Args {
        path: path?,
        folder,
        scenario: scenario?,
        live,
        secrets,
        json,
    })
}

pub fn run(args: &[OsString]) -> u8 {
    let Some(args) = arguments(args) else {
        eprintln!("{USAGE}");
        return 2;
    };
    match execute(args) {
        Ok(status) => u8::from(status != RunStatus::Passed),
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}

fn execute(args: Args) -> Result<RunStatus, String> {
    if args.folder {
        return super::folder::execute_run(args);
    }
    // A live run writes a journal, which only the window holding the workspace
    // lock may do. A simulation changes nothing and reads.
    let store = if args.live {
        WorkspaceStore::open_exclusive(&args.path)
    } else {
        WorkspaceStore::open_read_only(&args.path)
    }
    .map_err(|error| error.to_string())?;
    let loaded = store
        .load()
        .map_err(|_| "Cannot load document; check its format and recovery state".to_owned())?
        .ok_or("Workspace/project file does not exist")?;
    if loaded.recovered.is_some() {
        return Err(
            "Backup recovery must be explicitly completed before using this document".into(),
        );
    }
    let workspace = loaded.workspace;
    let project = workspace
        .projects
        .iter()
        .find(|project| project.id == workspace.active_id)
        .ok_or("Active project does not exist")?;
    let index = super::document::scenario_index(project, &args.scenario)?;
    let scenario = &project.scenarios[index];
    let secrets = executor::secrets(args.secrets.as_deref())?;

    let journal = if args.live {
        Some(JournalStore::open(store).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let request = RunRequest {
        project: project.clone(),
        scenario_id: scenario.id.clone(),
        target: apiforge_runner::RunTarget::Scenario,
        mode: if args.live {
            RunMode::Live
        } else {
            RunMode::Simulation
        },
        target_node_id: None,
        single_node_id: None,
        previous_run: None,
        secrets,
    };
    execute_request(request, journal, args.json)
}

/// Runs `request` with the shared executor and prints what it returns.
pub(super) fn execute_request(
    request: RunRequest,
    journal: Option<apiforge_storage::LoadedJournal>,
    json: bool,
) -> Result<RunStatus, String> {
    // What an earlier run left unfinished is said before this one adds to it.
    if let Some(journal) = &journal {
        report_interrupted(&journal.interrupted);
        if journal.unreadable {
            eprintln!("An unreadable interrupted-run journal was preserved for review.");
        }
    }
    let outcome = executor::execute(request, journal, CancellationToken::new(), DEADLINE)?;
    if outcome.ended_by == EndedBy::TimedOut {
        return Err("The run did not finish within ten minutes".into());
    }
    let report = outcome.report;
    if json {
        let text = serde_json::to_string_pretty(&report).map_err(|_| "Cannot print the report")?;
        println!("{text}");
    } else {
        print_report(&report);
    }
    Ok(report.status)
}

/// Says what an earlier run left unfinished, before this one adds to it.
fn report_interrupted(runs: &[apiforge_core::journal::JournalRun]) {
    for run in runs {
        eprintln!(
            "A live run of \"{}\" from {} did not finish. These requests may have changed the API:",
            run.scenario_name, run.started_at
        );
        for entry in run.effects() {
            eprintln!("  {} {} — {}", entry.method, entry.url, entry.state.label());
        }
    }
}

/// One line per step, with a component's inner steps indented beneath it.
///
/// A component that failed says to look at its inner steps, so they have to be
/// here: without them the run names the component and nothing that explains it.
fn print_steps(steps: &[StepResult], depth: usize) {
    for step in steps {
        let mark = match step.status {
            StepStatus::Passed => "ok",
            StepStatus::Failed => "failed",
            StepStatus::Error => "error",
            StepStatus::Blocked => "blocked",
            StepStatus::Skipped => "skipped",
            StepStatus::Cancelled => "cancelled",
        };
        let indent = "  ".repeat(depth);
        let message = step.message.as_deref().unwrap_or_default();
        println!(
            "{mark:>9}  {indent}{}{}{message}",
            step.display_name(),
            if message.is_empty() { "" } else { " — " }
        );
        // What a comparison wanted and what it got: the report carries both, and
        // a failure that does not say them cannot be acted on.
        if let (OptionalJson::Present(expected), OptionalJson::Present(actual)) =
            (&step.expected, &step.actual)
            && step.status != StepStatus::Passed
        {
            println!("{:>9}  {indent}  expected {}", "", compact(expected));
            println!("{:>9}  {indent}  actual   {}", "", compact(actual));
        }
        if let Some(children) = &step.children {
            print_steps(children, depth + 1);
        }
    }
}

/// A value on one line, shortened so a large body cannot bury the verdict.
fn compact(value: &serde_json::Value) -> String {
    const LIMIT: usize = 120;
    let text = match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    match text.char_indices().nth(LIMIT) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text,
    }
}

/// One line per step, then the verdict.
fn print_report(report: &RunResult) {
    print_steps(&report.steps, 0);
    if let Some(notice) = &report.notice {
        println!("{notice}");
    }
    if let Some(error) = &report.error {
        eprintln!("{error}");
    }
    println!("{}", report.status.label());
}
