//! Saves the workspace on a background thread, so file I/O never blocks a frame.
//!
//! Changes are debounced: a burst of edits produces one save of the latest state.
//! At most one active save and one pending workspace are retained, even if storage
//! is slower than editing. Wake messages and flush requests never block the UI.
//! Dropping the [`Saver`] flushes a pending save before the thread ends.

use apiforge_core::format::Workspace;
use apiforge_storage::WorkspaceStore;
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const DEBOUNCE: Duration = Duration::from_millis(350);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    Saved,
    Pending,
    Failed(String),
}

impl SaveStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Saved => "Saved locally",
            Self::Pending => "Saving…",
            Self::Failed(_) => "Save failed",
        }
    }
}

struct SaveState {
    generation: u64,
    status: SaveStatus,
    pending: Option<(u64, Box<Workspace>)>,
    flush: bool,
    shutdown: bool,
}

pub struct Saver {
    sender: SyncSender<()>,
    status: Arc<Mutex<SaveState>>,
    worker: Option<JoinHandle<()>>,
}

impl Saver {
    /// Starts the save thread; `notify` is called after each save attempt, e.g. to
    /// request a repaint.
    pub fn spawn(
        store: WorkspaceStore,
        notify: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        Self::spawn_with_save(
            move |workspace| store.save(workspace).map_err(|error| error.to_string()),
            notify,
            DEBOUNCE,
        )
    }

    fn spawn_with_save(
        save: impl Fn(&Workspace) -> Result<(), String> + Send + 'static,
        notify: impl Fn() + Send + 'static,
        debounce: Duration,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<()>(1);
        let status = Arc::new(Mutex::new(SaveState {
            generation: 0,
            status: SaveStatus::Saved,
            pending: None,
            flush: false,
            shutdown: false,
        }));
        let shared = Arc::clone(&status);
        let worker = std::thread::Builder::new()
            .name("workspace-save".into())
            .spawn(move || {
                let mut deadline: Option<Instant> = None;
                let mut seen_generation = 0;
                loop {
                    let wake = match deadline {
                        Some(deadline) => receiver
                            .recv_timeout(deadline.saturating_duration_since(Instant::now())),
                        None => receiver.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    let (pending, shutdown) = {
                        let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                        if matches!(wake, Err(RecvTimeoutError::Disconnected)) {
                            state.shutdown = true;
                        }
                        // Wakes carry no document. Multiple edits coalesce into a
                        // single latest-value slot even when disk I/O is blocked.
                        if let Some((generation, _)) = state.pending.as_ref()
                            && *generation != seen_generation
                        {
                            seen_generation = *generation;
                            deadline = Some(Instant::now() + debounce);
                        }
                        let ready = state.flush
                            || state.shutdown
                            || deadline.is_some_and(|deadline| Instant::now() >= deadline);
                        let pending = if ready { state.pending.take() } else { None };
                        if ready {
                            deadline = None;
                            state.flush = false;
                        }
                        (pending, state.shutdown && state.pending.is_none())
                    };
                    if let Some((generation, workspace)) = pending {
                        let result = match save(&workspace) {
                            Ok(()) => SaveStatus::Saved,
                            Err(error) => SaveStatus::Failed(error),
                        };
                        {
                            let mut state =
                                shared.lock().unwrap_or_else(|poison| poison.into_inner());
                            // A newer edit may have arrived while I/O was running.
                            // Its Pending status belongs to that newer generation.
                            if state.generation == generation {
                                state.status = result;
                            }
                        }
                        notify();
                    }
                    if shutdown {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender,
            status,
            worker: Some(worker),
        })
    }

    /// Schedules saving this state of the workspace.
    pub fn save(&self, workspace: &Workspace) {
        let workspace = Box::new(workspace.clone());
        let mut state = self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.generation = state.generation.wrapping_add(1);
        state.status = SaveStatus::Pending;
        state.pending = Some((state.generation, workspace));
        self.wake(&mut state);
    }

    fn wake(&self, state: &mut SaveState) {
        match self.sender.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => {}
            Err(TrySendError::Disconnected(())) => {
                state.status = SaveStatus::Failed("The save thread stopped.".into());
            }
        }
    }

    pub fn status(&self) -> SaveStatus {
        self.status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .status
            .clone()
    }

    /// Requests immediate processing of pending edits without blocking the caller.
    /// Poll [`Self::status`] after repaint notification to observe the result. The
    /// worker remains available for further edits, e.g. after cancelling a close.
    pub fn flush(&self) {
        let mut state = self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.flush = true;
        self.wake(&mut state);
    }
}

impl Drop for Saver {
    fn drop(&mut self) {
        {
            let mut state = self
                .status
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.shutdown = true;
            self.wake(&mut state);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{ImportLimits, import_document};

    #[test]
    fn dropping_the_saver_flushes_the_latest_state() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join("workspace.json"));
        let mut workspace = import_document(
            include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        let saver = Saver::spawn(store.clone(), || {}).unwrap();
        saver.save(&workspace);
        workspace.projects[0].name = "Latest".into();
        saver.save(&workspace);
        drop(saver);
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.workspace.projects[0].name, "Latest");
    }

    #[test]
    fn an_old_save_completion_does_not_acknowledge_a_newer_edit() {
        let workspace = import_document(
            include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let saver = Saver::spawn_with_save(
            move |_| {
                started_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|_| "Timed out waiting for injected I/O".to_owned())?;
                Ok(())
            },
            move || {
                let _ = finished_tx.send(());
            },
            DEBOUNCE,
        )
        .unwrap();
        saver.save(&workspace);
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        saver.save(&workspace);
        release_tx.send(()).unwrap();
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let during_newer_save = saver.status();
        // Release the second save before joining, including if debounce has not
        // expired yet: shutdown must flush the remaining generation as usual.
        release_tx.send(()).unwrap();
        drop(saver);
        assert_eq!(during_newer_save, SaveStatus::Pending);
    }

    #[test]
    fn failed_save_is_visible_and_a_later_edit_can_be_saved() {
        let workspace = import_document(
            include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let (finished_tx, finished_rx) = mpsc::channel();
        let saver = Saver::spawn_with_save(
            move |_| {
                if attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    Err("Injected disk failure".into())
                } else {
                    Ok(())
                }
            },
            move || {
                let _ = finished_tx.send(());
            },
            DEBOUNCE,
        )
        .unwrap();
        saver.save(&workspace);
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            saver.status(),
            SaveStatus::Failed("Injected disk failure".into())
        );
        saver.save(&workspace);
        assert_eq!(saver.status(), SaveStatus::Pending);
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(saver.status(), SaveStatus::Saved);
    }

    #[test]
    fn flush_bypasses_debounce_reports_failure_and_keeps_the_worker_alive() {
        let workspace = import_document(
            include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let (finished_tx, finished_rx) = mpsc::channel();
        let saver = Saver::spawn_with_save(
            move |_| {
                if attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    Err("Injected flush failure".into())
                } else {
                    Ok(())
                }
            },
            move || {
                let _ = finished_tx.send(());
            },
            // No wall-clock race with the normal 350 ms debounce: this worker
            // cannot attempt either save within the test deadline without Flush.
            Duration::from_secs(60),
        )
        .unwrap();
        saver.save(&workspace);
        saver.flush();
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            saver.status(),
            SaveStatus::Failed("Injected flush failure".into())
        );
        saver.save(&workspace);
        saver.flush();
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(saver.status(), SaveStatus::Saved);
    }

    #[test]
    fn edits_during_blocked_io_coalesce_to_the_latest_workspace() {
        let mut workspace = import_document(
            include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        workspace.projects[0].name = "First active save".into();
        let written = Arc::new(Mutex::new(Vec::new()));
        let saved_names = Arc::clone(&written);
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let saver = Saver::spawn_with_save(
            move |workspace| {
                let first = {
                    let mut names = saved_names.lock().unwrap();
                    names.push(workspace.projects[0].name.clone());
                    names.len() == 1
                };
                if first {
                    started_tx.send(()).unwrap();
                    release_rx
                        .recv_timeout(Duration::from_secs(5))
                        .map_err(|_| "Timed out waiting for injected I/O".to_owned())?;
                }
                Ok(())
            },
            move || {
                let _ = finished_tx.send(());
            },
            Duration::from_secs(60),
        )
        .unwrap();
        saver.save(&workspace);
        saver.flush();
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        for index in 0..100 {
            workspace.projects[0].name = format!("Edit {index}");
            saver.save(&workspace);
        }
        saver.flush();
        release_tx.send(()).unwrap();
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(saver.status(), SaveStatus::Saved);
        drop(saver);
        assert_eq!(*written.lock().unwrap(), ["First active save", "Edit 99"]);
    }
}
