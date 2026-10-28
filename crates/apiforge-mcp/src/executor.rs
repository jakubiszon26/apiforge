//! The shared run executor (MCP-00, "Runs, cancellation and the journal"):
//! starts a run on the runner, waits for it, and returns what happened as
//! data. `apiforge run` prints the returned report; `apiforge mcp` returns it
//! to the model. Nothing here writes to the terminal.
use apiforge_core::{journal::JournalRun, run::RunResult};
use apiforge_runner::{RunRequest, RunnerService};
use apiforge_storage::LoadedJournal;
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::Serialize;
use std::{
    ffi::OsString,
    io::Read,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

pub use tokio_util::sync::CancellationToken;

/// How long a run may take before the executor cancels it.
pub const DEADLINE: Duration = Duration::from_secs(600);

/// The prefix of an environment variable holding one session secret.
const SECRET_PREFIX: &str = "APIFORGE_SECRET_";

/// How often the executor looks at the run.
const POLL: Duration = Duration::from_millis(20);

/// How long the runner gets to finish its cleanup once the run has ended.
const SHUTDOWN: Duration = Duration::from_secs(10);

/// Called by the runner after every published snapshot.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EndedBy {
    /// The main phase and the cleanup ran to their end.
    Completed,
    /// The caller cancelled; the main phase stopped and the cleanup ran.
    Cancelled,
    /// The deadline passed; the main phase stopped and the cleanup ran.
    TimedOut,
}

/// What a run produced.
#[derive(Clone, Debug)]
pub struct RunOutcome {
    /// The runner's final, redacted report.
    pub report: RunResult,
    /// Live runs an earlier process left unfinished, read before this run.
    pub interrupted_before: Vec<JournalRun>,
    /// A journal file existed but could not be read; it was preserved.
    pub journal_unreadable: bool,
    pub ended_by: EndedBy,
}

/// Runs `request` to its end, journalled when `journal` is given. Cancelling
/// `cancel` or passing `deadline` stops the main phase; the cleanup still runs
/// and the report says how far the run got.
pub fn execute(
    request: RunRequest,
    journal: Option<LoadedJournal>,
    cancel: CancellationToken,
    deadline: Duration,
) -> Result<RunOutcome, String> {
    execute_observed(request, journal, cancel, deadline, Arc::new(|| {}))
}

/// As `execute`, with `notify` called after every snapshot the runner
/// publishes. When the deadline passes, `cancel` is cancelled too, so a
/// `notify` that watches it sees the run stopping.
pub fn execute_observed(
    request: RunRequest,
    journal: Option<LoadedJournal>,
    cancel: CancellationToken,
    deadline: Duration,
    notify: Notify,
) -> Result<RunOutcome, String> {
    let mut service =
        RunnerService::new(notify).map_err(|error| format!("Cannot start the runner: {error}"))?;
    let (interrupted_before, journal_unreadable) = match journal {
        Some(journal) => {
            service = service.with_journal(Arc::new(journal.store));
            (journal.interrupted, journal.unreadable)
        }
        None => (Vec::new(), false),
    };
    let handle = service.start(request).map_err(|error| error.to_string())?;
    let deadline = Instant::now() + deadline;
    let mut ended_by = EndedBy::Completed;
    let mut report: Option<RunResult> = None;
    while !handle.is_finished() {
        if ended_by == EndedBy::Completed {
            if cancel.is_cancelled() {
                ended_by = EndedBy::Cancelled;
                handle.cancel();
            } else if Instant::now() >= deadline {
                ended_by = EndedBy::TimedOut;
                cancel.cancel();
                handle.cancel();
            }
        }
        if let Some(latest) = handle.latest() {
            report = Some(latest);
        }
        std::thread::sleep(POLL);
    }
    if let Some(latest) = handle.latest() {
        report = Some(latest);
    }
    let report = report.ok_or("The run produced no report")?;
    service.shutdown(SHUTDOWN);
    Ok(RunOutcome {
        report,
        interrupted_before,
        journal_unreadable,
        ended_by,
    })
}

/// The largest secrets file read.
pub const MAX_SECRETS_BYTES: u64 = 1024 * 1024;

/// Session secrets, from a file and from the environment
/// (`APIFORGE_SECRET_<NAME>`). A name in both takes its value from the
/// environment, which belongs to this invocation.
pub fn secrets(path: Option<&Path>) -> Result<IndexMap<String, String>, String> {
    secrets_from(path, std::env::vars_os())
}

/// As `secrets`, with the environment given. A variable whose name or value
/// is not Unicode is not a secret and is skipped: it cannot be named in a
/// document, and an unrelated one must not stop a run.
pub fn secrets_from(
    path: Option<&Path>,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<IndexMap<String, String>, String> {
    let mut found = IndexMap::new();
    if let Some(path) = path {
        let bytes = read_secrets_file(path)?;
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
    for (key, value) in environment {
        let (Some(key), Some(value)) = (key.to_str(), value.to_str()) else {
            continue;
        };
        if let Some(name) = key.strip_prefix(SECRET_PREFIX)
            && !name.is_empty()
        {
            found.insert(name.to_owned(), value.to_owned());
        }
    }
    Ok(found)
}

/// The secrets file, refused by its size before it is read, and read no
/// further than the limit in case the size was wrong (a pipe, a growing file).
fn read_secrets_file(path: &Path) -> Result<Vec<u8>, String> {
    let too_large = || "The secrets file is too large".to_owned();
    let file =
        std::fs::File::open(path).map_err(|error| format!("Cannot read secrets: {error}"))?;
    let length = file
        .metadata()
        .map_err(|error| format!("Cannot read secrets: {error}"))?
        .len();
    if length > MAX_SECRETS_BYTES {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take(MAX_SECRETS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read secrets: {error}"))?;
    if bytes.len() as u64 > MAX_SECRETS_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}
