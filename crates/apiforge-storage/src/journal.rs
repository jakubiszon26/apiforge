//! The durable side of the live-run journal.
//!
//! `journal.json` lives beside the workspace and is covered by the same directory
//! lock, so only the window that may write the workspace may write the journal. Its
//! content is not a user document: nobody edits it, it is rewritten in full by every
//! record and it is removed as soon as a run reports its own outcome. There is no
//! backup and no external-change detection; what matters is that every record is on
//! disk before the request it describes is sent, which [`write_atomic`] guarantees by
//! flushing the file and its directory before it returns.
//!
//! A file that is still there when a session opens the workspace exclusively was left
//! by a process that died mid-run: the lock rules out a second live writer.

use crate::{
    file::{self, write_atomic},
    session::{MAX_BYTES, StorageError, WorkspaceStore},
};
use apiforge_core::journal::{
    CleanupState, JOURNAL_VERSION, Journal, JournalEntry, JournalRun, JournalSink, JournalSinkError,
};
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

/// The journal of one workspace session, and the writer the runner uses.
pub struct JournalStore {
    // The clone keeps the session's lock alive for as long as the journal is written.
    store: Arc<dyn WriterSession>,
    path: PathBuf,
    journal: Mutex<Journal>,
}

/// A shared writer owner keeps its OS lock alive and serialises project I/O.
/// Journal persistence does not depend on a particular workspace container.
trait WriterSession: Send + Sync {
    fn access(
        &self,
        action: &mut dyn FnMut(&Path) -> Result<(), StorageError>,
    ) -> Result<(), StorageError>;
}

impl std::fmt::Debug for JournalStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JournalStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl WriterSession for WorkspaceStore {
    fn access(
        &self,
        action: &mut dyn FnMut(&Path) -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        if self.is_read_only() {
            return Err(StorageError::ReadOnly);
        }
        action(&journal_path(self.path()))
    }
}

impl WriterSession for crate::project::ProjectStore {
    fn access(
        &self,
        action: &mut dyn FnMut(&Path) -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        self.journal_access(action)
    }
}

/// An opened journal together with what the previous session left behind.
#[derive(Debug)]
pub struct LoadedJournal {
    pub store: JournalStore,
    /// Runs that were executing when a previous process ended. Their effects on the
    /// tested API are unconfirmed.
    pub interrupted: Vec<JournalRun>,
    /// A journal file existed but could not be read; it was preserved beside the
    /// workspace as `journal.unreadable.json` and this session starts empty.
    pub unreadable: bool,
}

impl JournalStore {
    /// Opens the journal of a writable session.
    ///
    /// Fails for a read-only session, which must not write files another window owns,
    /// and for a journal written by an unknown version, which is preserved untouched.
    pub fn open(store: WorkspaceStore) -> Result<LoadedJournal, StorageError> {
        Self::open_owner(Arc::new(store))
    }

    pub fn open_project(
        store: crate::project::ProjectStore,
    ) -> Result<LoadedJournal, StorageError> {
        Self::open_owner(Arc::new(store))
    }

    fn open_owner(store: Arc<dyn WriterSession>) -> Result<LoadedJournal, StorageError> {
        let mut loaded = None;
        let mut location = PathBuf::new();
        store.access(&mut |path| {
            location = path.to_owned();
            loaded = Some(Self::read_or_preserve(path)?);
            Ok(())
        })?;
        let (journal, unreadable) = loaded.expect("the owner completed journal access");
        Ok(LoadedJournal {
            interrupted: journal.runs.clone(),
            unreadable,
            store: Self {
                store,
                path: location,
                journal: Mutex::new(journal),
            },
        })
    }

    fn read_or_preserve(path: &Path) -> Result<(Journal, bool), StorageError> {
        Ok(match read(path) {
            Ok(Some(journal)) => (journal, false),
            Ok(None) => (Journal::default(), false),
            Err(error @ StorageError::UnsupportedJournalVersion) => return Err(error),
            Err(_) => {
                preserve_unreadable(path)?;
                (Journal::default(), true)
            }
        })
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// What the file holds now.
    pub fn snapshot(&self) -> Journal {
        self.locked().clone()
    }

    /// Forgets runs the user has acknowledged. Unknown identifiers are ignored.
    pub fn discard(&self, run_ids: &[String]) -> Result<(), StorageError> {
        let mut journal = self.locked();
        let mut next = journal.clone();
        next.runs
            .retain(|run| !run_ids.iter().any(|id| id == &run.run_id));
        self.write(&next)?;
        *journal = next;
        Ok(())
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, Journal> {
        self.journal.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Writes the journal, or removes the file once no run is recorded.
    fn write(&self, journal: &Journal) -> Result<(), StorageError> {
        self.store.access(&mut |path| Self::write_at(path, journal))
    }

    fn write_at(path: &Path, journal: &Journal) -> Result<(), StorageError> {
        if journal.is_empty() {
            return remove(path);
        }
        let bytes = serde_json::to_vec_pretty(journal).map_err(file::StorageError::Serialise)?;
        if bytes.len() > MAX_BYTES {
            return Err(StorageError::TooLarge {
                path: path.to_owned(),
            });
        }
        write_atomic(path, &bytes).map_err(StorageError::Document)
    }

    fn update(
        &self,
        change: impl FnOnce(&mut Journal) -> Result<(), JournalSinkError>,
    ) -> Result<(), JournalSinkError> {
        let mut journal = self.locked();
        let mut next = journal.clone();
        change(&mut next)?;
        self.write(&next).map_err(sink_error)?;
        *journal = next;
        Ok(())
    }
}

impl JournalSink for JournalStore {
    fn begin_run(&self, run: &JournalRun) -> Result<(), JournalSinkError> {
        self.update(|journal| {
            journal.begin(run.clone());
            Ok(())
        })
    }

    fn record(&self, run_id: &str, entry: &JournalEntry) -> Result<(), JournalSinkError> {
        self.update(|journal| {
            journal
                .run_mut(run_id)
                .ok_or_else(|| JournalSinkError::new("the run is not journalled"))?
                .record(entry.clone());
            Ok(())
        })
    }

    fn cleanup_started(&self, run_id: &str) -> Result<(), JournalSinkError> {
        self.update(|journal| {
            journal
                .run_mut(run_id)
                .ok_or_else(|| JournalSinkError::new("the run is not journalled"))?
                .cleanup = CleanupState::Started;
            Ok(())
        })
    }

    fn finish_run(&self, run_id: &str) -> Result<(), JournalSinkError> {
        self.update(|journal| {
            journal.finish(run_id);
            Ok(())
        })
    }
}

/// A sink error names the problem; a storage path may be long but is not secret.
fn sink_error(error: StorageError) -> JournalSinkError {
    JournalSinkError::new(error.to_string())
}

fn journal_path(workspace: &Path) -> PathBuf {
    workspace.with_file_name("journal.json")
}

fn unreadable_path(journal: &Path) -> PathBuf {
    journal.with_file_name("journal.unreadable.json")
}

fn remove(path: &Path) -> Result<(), StorageError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(StorageError::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

/// Keeps the bytes of a journal that cannot be read, so the evidence of an
/// interrupted run is not destroyed by the next one. Only the newest is kept.
fn preserve_unreadable(path: &Path) -> Result<(), StorageError> {
    let bytes = read_bounded(path)?.unwrap_or_default();
    write_atomic(&unreadable_path(path), &bytes)?;
    remove(path)
}

fn read_bounded(path: &Path) -> Result<Option<Vec<u8>>, StorageError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(StorageError::Io {
                path: path.to_owned(),
                source,
            });
        }
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| StorageError::Io {
            path: path.to_owned(),
            source,
        })?;
    if bytes.len() > MAX_BYTES {
        return Err(StorageError::TooLarge {
            path: path.to_owned(),
        });
    }
    Ok(Some(bytes))
}

fn read(path: &Path) -> Result<Option<Journal>, StorageError> {
    let Some(bytes) = read_bounded(path)? else {
        return Ok(None);
    };
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    let version: Version = serde_json::from_slice(&bytes)
        .map_err(|_| StorageError::InvalidJournal("invalid JSON or version"))?;
    if version.version != JOURNAL_VERSION {
        return Err(StorageError::UnsupportedJournalVersion);
    }
    let journal: Journal = serde_json::from_slice(&bytes)
        .map_err(|_| StorageError::InvalidJournal("invalid JSON or unsupported fields"))?;
    validate(&journal)?;
    Ok(Some(journal))
}

fn validate(journal: &Journal) -> Result<(), StorageError> {
    let invalid = StorageError::InvalidJournal;
    let mut runs = std::collections::HashSet::new();
    for run in &journal.runs {
        if run.run_id.is_empty() || run.scenario_id.is_empty() {
            return Err(invalid("missing run or scenario identifier"));
        }
        if !runs.insert(&run.run_id) {
            return Err(invalid("duplicate run identifier"));
        }
        if run.entries.len() > apiforge_core::journal::MAX_ENTRIES_PER_RUN {
            return Err(invalid("too many entries"));
        }
        let mut sequences = std::collections::HashSet::new();
        for entry in &run.entries {
            if entry.sequence == 0 || entry.handle.is_empty() || entry.node_id.is_empty() {
                return Err(invalid("invalid entry"));
            }
            if !sequences.insert(entry.sequence) {
                return Err(invalid("duplicate entry sequence"));
            }
        }
    }
    if journal.runs.len() > apiforge_core::journal::MAX_RUNS {
        return Err(invalid("too many runs"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
