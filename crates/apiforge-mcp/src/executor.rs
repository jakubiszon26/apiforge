//! The shared run executor (MCP-00, "Runs, cancellation and the journal"):
//! starts a run on the runner, waits for it, and returns what happened as
//! data. `apiforge run` prints the returned report; `apiforge mcp` returns it
//! to the model. Nothing here prints.
use apiforge_core::{journal::JournalRun, run::RunResult};
use apiforge_runner::RunRequest;
use apiforge_storage::LoadedJournal;
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::Serialize;
use std::{path::Path, sync::Arc, time::Duration};

pub use tokio_util::sync::CancellationToken;

/// How long a run may take before the executor cancels it.
pub const DEADLINE: Duration = Duration::from_secs(600);

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
/// publishes.
pub fn execute_observed(
    request: RunRequest,
    journal: Option<LoadedJournal>,
    cancel: CancellationToken,
    deadline: Duration,
    notify: Notify,
) -> Result<RunOutcome, String> {
    let _ = (request, journal, cancel, deadline, notify);
    unimplemented!("MCP-03")
}

/// Session secrets, from a file and from the environment
/// (`APIFORGE_SECRET_<NAME>`). A name in both takes its value from the
/// environment, which belongs to this invocation.
pub fn secrets(path: Option<&Path>) -> Result<IndexMap<String, String>, String> {
    let _ = path;
    unimplemented!("MCP-03")
}
