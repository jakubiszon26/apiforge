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
use apiforge_runner::{RunRequest, RunnerService};
use apiforge_storage::{JournalStore, WorkspaceStore};
use indexmap::IndexMap;
use std::{ffi::OsString, path::PathBuf, sync::Arc, time::Duration};

pub const USAGE: &str = "apiforge run --workspace <doc.json> --scenario <name|id> [--live] [--secrets <file.json>] [--json]";

/// The prefix of an environment variable holding one session secret.
const SECRET_PREFIX: &str = "APIFORGE_SECRET_";

/// How long the command waits for a run that never reports back.
const DEADLINE: Duration = Duration::from_secs(600);

pub struct Args {
    pub path: PathBuf,
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
    let (mut live, mut json) = (false, false);
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.to_str()? {
            "--workspace" if path.is_none() => path = Some(PathBuf::from(rest.next()?)),
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
    let secrets = secrets(args.secrets.as_deref())?;

    let mut service = RunnerService::new(Arc::new(|| {}))
        .map_err(|error| format!("Cannot start the runner: {error}"))?;
    if args.live {
        let journal = JournalStore::open(store)
            .map_err(|error| format!("{error} A live run must be able to record what it sends."))?;
        report_interrupted(&journal.interrupted);
        service = service.with_journal(Arc::new(journal.store));
    }
    let request = RunRequest {
        project: project.clone(),
        scenario_id: scenario.id.clone(),
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
    let handle = service.start(request).map_err(|error| error.to_string())?;
    let deadline = std::time::Instant::now() + DEADLINE;
    let mut report: Option<RunResult> = None;
    while !handle.is_finished() {
        if std::time::Instant::now() > deadline {
            handle.cancel();
            return Err("The run did not finish within ten minutes".into());
        }
        if let Some(latest) = handle.latest() {
            report = Some(latest);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if let Some(latest) = handle.latest() {
        report = Some(latest);
    }
    let report = report.ok_or("The run produced no report")?;
    service.shutdown(Duration::from_secs(10));
    if args.json {
        let text = serde_json::to_string_pretty(&report).map_err(|_| "Cannot print the report")?;
        println!("{text}");
    } else {
        print_report(&report);
    }
    Ok(report.status)
}

/// Session secrets, from a file and from the environment. A name in both takes
/// its value from the environment, which belongs to this invocation.
fn secrets(path: Option<&std::path::Path>) -> Result<IndexMap<String, String>, String> {
    let mut found = IndexMap::new();
    if let Some(path) = path {
        let bytes = std::fs::read(path).map_err(|error| format!("Cannot read secrets: {error}"))?;
        if bytes.len() > 1024 * 1024 {
            return Err("The secrets file is too large".into());
        }
        let parsed: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| "The secrets file is not JSON".to_owned())?;
        let object = parsed
            .as_object()
            .ok_or("The secrets file must be an object of names and text values")?;
        for (name, value) in object {
            let text = value
                .as_str()
                .ok_or("The secrets file must be an object of names and text values")?;
            found.insert(name.clone(), text.to_owned());
        }
    }
    for (key, value) in std::env::vars() {
        if let Some(name) = key.strip_prefix(SECRET_PREFIX)
            && !name.is_empty()
        {
            found.insert(name.to_owned(), value);
        }
    }
    Ok(found)
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
