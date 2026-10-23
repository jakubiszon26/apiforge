//! The live-run journal: what a run may have changed in the tested API.
//!
//! A live request can change the tested API, and a process that dies while a run is
//! in progress leaves no report behind. The journal is the only record of those
//! effects: the runner writes an entry *before* a request is sent and resolves it
//! once the outcome is known, so a file left behind by a crashed process says which
//! requests were certainly sent, which may have been sent, and whether the cleanup
//! phase ever started.
//!
//! The journal is not a run history. A run that finishes removes itself, so entries
//! on disk always describe an interruption. Field names are part of the file format
//! and stay stable.
//!
//! Only live runs are journalled; a simulation sends nothing and records nothing.

use crate::run::Phase;
use serde::{Deserialize, Serialize};

/// The file format of the journal. An unknown version is preserved, never rewritten.
pub const JOURNAL_VERSION: u32 = 1;

/// Entries one interrupted run may contribute. Older entries of the same run are
/// dropped first and counted in [`JournalRun::elided`].
pub const MAX_ENTRIES_PER_RUN: usize = 200;

/// Runs the journal keeps. The oldest interrupted run is dropped first.
pub const MAX_RUNS: usize = 16;

/// What is known about the effect of one live request on the tested API.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum EffectState {
    /// Written before the request was sent; the outcome is not known. A process
    /// that dies here leaves this state behind, which is the uncertain case.
    InFlight,
    /// A response arrived, so the request was performed. Its status is recorded.
    Completed,
    /// The request never left the process, for example because the host could not
    /// be resolved, so the API cannot have seen it.
    NotSent,
    /// Bytes were sent but no complete response arrived, for example after a
    /// timeout or a reset connection. The API may have performed the request.
    Uncertain,
}

impl EffectState {
    pub fn label(self) -> &'static str {
        match self {
            Self::InFlight => "Sending",
            Self::Completed => "Performed",
            Self::NotSent => "Not sent",
            Self::Uncertain => "Possibly performed",
        }
    }

    /// Whether the tested API may hold a change from this request. `InFlight` counts:
    /// a journal read after a crash cannot tell how far the request got.
    pub fn may_have_changed_the_api(self) -> bool {
        matches!(self, Self::InFlight | Self::Completed | Self::Uncertain)
    }
}

/// How far the cleanup phase of an interrupted run got.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum CleanupState {
    /// The main phase was still running; cleanup had not started.
    #[default]
    NotStarted,
    /// Cleanup had started. Its own entries say which requests it managed to send.
    Started,
}

/// One journalled live request.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalEntry {
    /// Position of the request within its run, counted from one.
    pub sequence: u64,
    pub node_id: String,
    pub handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub phase: Phase,
    pub method: String,
    /// Redacted: secret values and sensitive query values are masked, exactly as in
    /// a report. The journal is a file on disk and never holds a secret.
    pub url: String,
    /// RFC 3339 timestamp taken before the request was sent.
    pub started_at: String,
    pub state: EffectState,
    /// The response status, for a `Completed` entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// Why the request did not complete, for a `NotSent` or `Uncertain` entry.
    /// Redacted like `url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl JournalEntry {
    /// The label, or the handle of a node without one.
    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.handle)
    }
}

/// One live run, recorded while it executes and removed when it finishes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalRun {
    pub run_id: String,
    pub scenario_id: String,
    /// The scenario's display name when the run started, for a report after a crash
    /// that the document may no longer explain.
    pub scenario_name: String,
    /// RFC 3339 timestamp of the run's start.
    pub started_at: String,
    pub cleanup: CleanupState,
    pub entries: Vec<JournalEntry>,
    /// Entries dropped because the run exceeded [`MAX_ENTRIES_PER_RUN`].
    #[serde(default)]
    pub elided: u64,
}

impl JournalRun {
    /// A run with no requests recorded yet.
    pub fn new(
        run_id: impl Into<String>,
        scenario_id: impl Into<String>,
        scenario_name: impl Into<String>,
        started_at: impl Into<String>,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            scenario_id: scenario_id.into(),
            scenario_name: scenario_name.into(),
            started_at: started_at.into(),
            cleanup: CleanupState::NotStarted,
            entries: Vec::new(),
            elided: 0,
        }
    }

    /// Requests that may have changed the tested API, newest last.
    pub fn effects(&self) -> impl Iterator<Item = &JournalEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.state.may_have_changed_the_api())
    }

    /// Inserts an entry, or replaces the entry with the same sequence number.
    ///
    /// Beyond [`MAX_ENTRIES_PER_RUN`] the oldest entry is dropped and counted, so a
    /// long run cannot grow the file without bound. An entry that resolves a dropped
    /// sequence is ignored rather than re-added out of order.
    pub fn record(&mut self, entry: JournalEntry) {
        match self
            .entries
            .iter_mut()
            .find(|existing| existing.sequence == entry.sequence)
        {
            Some(existing) => *existing = entry,
            None => {
                if self
                    .entries
                    .first()
                    .is_some_and(|first| first.sequence > entry.sequence)
                {
                    return;
                }
                self.entries.push(entry);
                while self.entries.len() > MAX_ENTRIES_PER_RUN {
                    self.entries.remove(0);
                    self.elided += 1;
                }
            }
        }
    }
}

/// The journal file: the live runs that have not reported their own outcome.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Journal {
    pub version: u32,
    pub runs: Vec<JournalRun>,
}

impl Default for Journal {
    fn default() -> Self {
        Self {
            version: JOURNAL_VERSION,
            runs: Vec::new(),
        }
    }
}

impl Journal {
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    pub fn run_mut(&mut self, run_id: &str) -> Option<&mut JournalRun> {
        self.runs.iter_mut().find(|run| run.run_id == run_id)
    }

    /// Starts a run, replacing an earlier record with the same identifier. Beyond
    /// [`MAX_RUNS`] the oldest run is dropped.
    pub fn begin(&mut self, run: JournalRun) {
        self.runs.retain(|existing| existing.run_id != run.run_id);
        self.runs.push(run);
        while self.runs.len() > MAX_RUNS {
            self.runs.remove(0);
        }
    }

    /// Removes a finished run. Its report is the record from then on.
    pub fn finish(&mut self, run_id: &str) {
        self.runs.retain(|run| run.run_id != run_id);
    }
}

/// A durable sink for the journal, implemented by the storage layer.
///
/// The runner calls these methods on its worker thread while a run executes, so an
/// implementation must be quick and must make each change durable before returning:
/// a record that is still in a buffer when the process dies records nothing.
///
/// A failure before a request is sent stops the run — an effect that cannot be
/// recorded is not performed. Implementations therefore report errors rather than
/// swallowing them.
pub trait JournalSink: Send + Sync {
    /// Records the start of a live run.
    fn begin_run(&self, run: &JournalRun) -> Result<(), JournalSinkError>;

    /// Inserts or replaces one entry of a run. An unknown run is an error.
    fn record(&self, run_id: &str, entry: &JournalEntry) -> Result<(), JournalSinkError>;

    /// Notes that the run's cleanup phase has started.
    fn cleanup_started(&self, run_id: &str) -> Result<(), JournalSinkError>;

    /// Removes a run that finished and reported its own outcome.
    fn finish_run(&self, run_id: &str) -> Result<(), JournalSinkError>;
}

/// Why a journal write failed. The message is shown in run reports, so it names the
/// problem without exposing a path's contents or a secret.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct JournalSinkError(String);

impl JournalSinkError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(sequence: u64, state: EffectState) -> JournalEntry {
        JournalEntry {
            sequence,
            node_id: format!("node-{sequence}"),
            handle: format!("step_{sequence}"),
            label: None,
            phase: Phase::Main,
            method: "POST".into(),
            url: "https://api.example.com/orders".into(),
            started_at: "2026-10-23T10:00:00.000Z".into(),
            state,
            status: None,
            detail: None,
        }
    }

    fn run() -> JournalRun {
        JournalRun::new("run-1", "scen-1", "Checkout", "2026-10-23T10:00:00.000Z")
    }

    #[test]
    fn resolving_an_entry_replaces_it_in_place() {
        let mut run = run();
        run.record(entry(1, EffectState::InFlight));
        run.record(entry(2, EffectState::InFlight));
        run.record(entry(1, EffectState::Completed));
        assert_eq!(run.entries.len(), 2);
        assert_eq!(run.entries[0].state, EffectState::Completed);
        assert_eq!(run.entries[1].state, EffectState::InFlight);
    }

    #[test]
    fn a_long_run_drops_its_oldest_entries_and_counts_them() {
        let mut run = run();
        for sequence in 1..=(MAX_ENTRIES_PER_RUN as u64 + 3) {
            run.record(entry(sequence, EffectState::Completed));
        }
        assert_eq!(run.entries.len(), MAX_ENTRIES_PER_RUN);
        assert_eq!(run.elided, 3);
        assert_eq!(run.entries[0].sequence, 4);
        // Resolving a dropped entry must not reinsert it out of order.
        run.record(entry(1, EffectState::Uncertain));
        assert_eq!(run.entries.len(), MAX_ENTRIES_PER_RUN);
        assert_eq!(run.entries[0].sequence, 4);
    }

    #[test]
    fn only_states_that_may_have_reached_the_api_are_effects() {
        let mut run = run();
        run.record(entry(1, EffectState::NotSent));
        run.record(entry(2, EffectState::InFlight));
        run.record(entry(3, EffectState::Completed));
        run.record(entry(4, EffectState::Uncertain));
        let sequences: Vec<u64> = run.effects().map(|entry| entry.sequence).collect();
        assert_eq!(sequences, [2, 3, 4]);
    }

    #[test]
    fn a_finished_run_leaves_the_journal() {
        let mut journal = Journal::default();
        journal.begin(run());
        assert!(!journal.is_empty());
        journal.begin(run());
        assert_eq!(journal.runs.len(), 1);
        journal.finish("run-1");
        assert!(journal.is_empty());
    }

    #[test]
    fn the_journal_keeps_a_bounded_number_of_runs() {
        let mut journal = Journal::default();
        for index in 0..(MAX_RUNS + 2) {
            journal.begin(JournalRun::new(
                format!("run-{index}"),
                "scen-1",
                "Checkout",
                "2026-10-23T10:00:00.000Z",
            ));
        }
        assert_eq!(journal.runs.len(), MAX_RUNS);
        assert_eq!(journal.runs[0].run_id, "run-2");
    }
}
