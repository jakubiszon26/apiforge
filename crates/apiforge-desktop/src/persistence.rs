//! Saves the workspace on a background thread, so file I/O never blocks a frame.
//!
//! Changes are debounced: a burst of edits produces one save of the latest state.
//! Dropping the [`Saver`] flushes a pending save before the thread ends.

use apiforge_core::format::Workspace;
use apiforge_storage::WorkspaceStore;
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError, Sender},
    },
    thread::JoinHandle,
    time::Duration,
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

enum Message {
    Save(Box<Workspace>),
    Shutdown,
}

pub struct Saver {
    sender: Sender<Message>,
    status: Arc<Mutex<SaveStatus>>,
    worker: Option<JoinHandle<()>>,
}

impl Saver {
    /// Starts the save thread; `notify` is called after each save attempt, e.g. to
    /// request a repaint.
    pub fn spawn(
        store: WorkspaceStore,
        notify: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<Message>();
        let status = Arc::new(Mutex::new(SaveStatus::Saved));
        let shared = Arc::clone(&status);
        let worker = std::thread::Builder::new()
            .name("workspace-save".into())
            .spawn(move || {
                let mut pending: Option<Box<Workspace>> = None;
                loop {
                    let message = if pending.is_some() {
                        receiver.recv_timeout(DEBOUNCE)
                    } else {
                        receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
                    };
                    let shutdown = match message {
                        Ok(Message::Save(workspace)) => {
                            pending = Some(workspace);
                            continue;
                        }
                        Err(RecvTimeoutError::Timeout) => false,
                        Ok(Message::Shutdown) | Err(RecvTimeoutError::Disconnected) => true,
                    };
                    if let Some(workspace) = pending.take() {
                        let result = match store.save(&workspace) {
                            Ok(()) => SaveStatus::Saved,
                            Err(error) => SaveStatus::Failed(error.to_string()),
                        };
                        *shared.lock().unwrap_or_else(|poison| poison.into_inner()) = result;
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
        *self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = SaveStatus::Pending;
        if self
            .sender
            .send(Message::Save(Box::new(workspace.clone())))
            .is_err()
        {
            *self
                .status
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) =
                SaveStatus::Failed("The save thread stopped.".into());
        }
    }

    pub fn status(&self) -> SaveStatus {
        self.status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
}

impl Drop for Saver {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Shutdown);
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
}
