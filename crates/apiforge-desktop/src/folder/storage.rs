//! One serial storage worker per folder window. Callbacks stay on the UI thread;
//! owned inputs/results cross the channel, and snapshots are taken at dequeue.
use super::*;
use apiforge_project::{SessionIdentity, StorageCompletion, StorageMode};
use std::{
    any::Any,
    collections::VecDeque,
    sync::mpsc::{self, Sender},
};

type Value = Box<dyn Any + Send>;
type Reply = Result<Value, ()>;
type Work = Box<dyn FnOnce() -> Value + Send>;
type Apply = Box<dyn FnOnce(&mut FolderApp, Reply)>;
type Start = Box<dyn FnOnce(&mut FolderApp) -> Option<(Work, Apply)>>;

struct Queued {
    label: &'static str,
    blocks_edits: bool,
    session_work: bool,
    start: Start,
}

struct Active {
    label: &'static str,
    blocks_edits: bool,
    session_work: bool,
    apply: Apply,
    started: Instant,
}

pub(super) struct Storage {
    queue: VecDeque<Queued>,
    active: Option<Active>,
    sender: Sender<Work>,
    results: Receiver<Reply>,
    unavailable: bool,
}

impl Storage {
    pub fn new(context: egui::Context) -> Self {
        let (sender, requests) = mpsc::channel::<Work>();
        let (completed, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(work) = requests.recv() {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).map_err(|_| ());
                if completed.send(result).is_err() {
                    break;
                }
                context.request_repaint();
            }
        });
        Self {
            queue: VecDeque::new(),
            active: None,
            sender,
            results,
            unavailable: false,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.active.is_some() || !self.queue.is_empty()
    }

    pub fn blocks_edits(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.blocks_edits)
    }

    pub fn owns_session(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.session_work)
    }

    pub fn label(&self) -> Option<&'static str> {
        self.active
            .as_ref()
            .filter(|active| active.started.elapsed() >= Duration::from_millis(150))
            .map(|active| active.label)
    }

    fn finish(&mut self) -> Option<(Apply, Reply)> {
        self.active.as_ref()?;
        let result = if self.unavailable {
            Err(())
        } else {
            match self.results.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => Err(()),
            }
        };
        let active = self.active.take()?;
        Some((active.apply, result))
    }

    fn launch(
        &mut self,
        label: &'static str,
        blocks_edits: bool,
        session_work: bool,
        work: Work,
        apply: Apply,
    ) {
        self.active = Some(Active {
            label,
            blocks_edits,
            session_work,
            apply,
            started: Instant::now(),
        });
        if self.sender.send(work).is_err() {
            self.unavailable = true;
        }
    }
}

impl FolderApp {
    pub(super) fn restore_prompt(&mut self, mut prompt: Prompt, message: String) {
        match &mut prompt {
            Prompt::NewDocument { error, .. }
            | Prompt::Rename { error, .. }
            | Prompt::MoveTo { error, .. }
            | Prompt::NewFolder { error, .. }
            | Prompt::Delete { error, .. }
            | Prompt::Import { error, .. }
            | Prompt::Trash { error, .. }
            | Prompt::Register { error, .. } => *error = Some(message),
            _ => {
                self.notify(message, true);
                return;
            }
        }
        self.prompt = Some(prompt);
    }

    pub(super) fn working_project<T: Send + 'static>(
        &mut self,
        label: &'static str,
        run: impl FnOnce(&mut ProjectSession) -> apiforge_project::Result<T> + Send + 'static,
        complete: impl FnOnce(&mut Self, apiforge_project::Result<T>) + 'static,
    ) {
        self.operation_sequence += 1;
        let ticket = self.operation_sequence;
        self.prompt = Some(Prompt::Working { ticket, label });
        self.storage_project(label, StorageMode::Exclusive, true, run, move |app, result| {
            if matches!(app.prompt, Some(Prompt::Working { ticket: current, .. }) if current == ticket) { app.prompt = None; }
            if matches!(result, Err(apiforge_project::Error::StaleStorage)) {
                app.watcher.rescan();
                app.notify("The project changed while this operation was running. Current work was kept; review the folder before retrying.", true);
                return;
            }
            complete(app, result);
        });
    }

    pub(super) fn working_io<T: Send + 'static>(
        &mut self,
        label: &'static str,
        run: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(&mut Self, Result<T, String>) + 'static,
    ) {
        self.operation_sequence += 1;
        let ticket = self.operation_sequence;
        let generation = self.open_generation;
        let identity = self.session.as_ref().map(ProjectSession::storage_identity);
        self.prompt = Some(Prompt::Working { ticket, label });
        self.storage.queue.push_back(Queued {
            label, blocks_edits: true, session_work: true,
            start: Box::new(move |_| {
                let work: Work = Box::new(move || Box::new(run()));
                let apply: Apply = Box::new(move |app, reply| {
                    if matches!(app.prompt, Some(Prompt::Working { ticket: current, .. }) if current == ticket) { app.prompt = None; }
                    if generation != app.open_generation || identity != app.session.as_ref().map(ProjectSession::storage_identity) { return; }
                    let result = reply.map_err(|_| "The storage worker stopped unexpectedly.".to_owned())
                        .and_then(|value| value.downcast::<T>().map(|value| *value).map_err(|_| "The storage result could not be read.".to_owned()));
                    complete(app, result);
                });
                Some((work, apply))
            }),
        });
        self.ui_context.request_repaint();
    }

    /// Polling never waits on a channel, joins a thread or locks project I/O.
    pub(super) fn poll_storage(&mut self) {
        if let Some((apply, result)) = self.storage.finish() {
            apply(self, result);
        }
        for _ in 0..16 {
            if self.storage.active.is_some() {
                break;
            }
            let Some(queued) = self.storage.queue.pop_front() else {
                break;
            };
            let Queued {
                label,
                blocks_edits,
                session_work,
                start,
            } = queued;
            if let Some((work, apply)) = start(self) {
                self.storage
                    .launch(label, blocks_edits, session_work, work, apply);
            }
        }
        if self.storage.is_busy() {
            self.ui_context
                .request_repaint_after(Duration::from_millis(16));
        }
    }

    pub(super) fn storage_project<T: Send + 'static>(
        &mut self,
        label: &'static str,
        mode: StorageMode,
        blocks_edits: bool,
        run: impl FnOnce(&mut ProjectSession) -> apiforge_project::Result<T> + Send + 'static,
        complete: impl FnOnce(&mut Self, apiforge_project::Result<T>) + 'static,
    ) {
        let identity = self.session.as_ref().map(ProjectSession::storage_identity);
        self.storage.queue.push_back(Queued {
            label,
            blocks_edits,
            session_work: true,
            start: Box::new(move |app| {
                let Some(session) = app.session.as_ref() else {
                    complete(app, Err(apiforge_project::Error::StaleStorage));
                    return None;
                };
                if Some(session.storage_identity()) != identity {
                    complete(app, Err(apiforge_project::Error::StaleStorage));
                    return None;
                }
                let snapshot = session.storage_snapshot();
                let work: Work = Box::new(move || Box::new(snapshot.run(mode, run)));
                let apply: Apply = Box::new(move |app, reply| {
                    let result = reply
                        .map_err(|_| apiforge_project::Error::StorageWorker)
                        .and_then(|value| {
                            value
                                .downcast::<StorageCompletion<apiforge_project::Result<T>>>()
                                .map_err(|_| apiforge_project::Error::StorageWorker)
                        })
                        .and_then(|completion| {
                            app.session
                                .as_mut()
                                .ok_or(apiforge_project::Error::StaleStorage)?
                                .complete_storage(*completion)
                        })
                        .and_then(|result| result);
                    complete(app, result);
                });
                Some((work, apply))
            }),
        });
        self.ui_context.request_repaint();
    }

    pub(super) fn storage_io<T: Send + 'static>(
        &mut self,
        label: &'static str,
        run: impl FnOnce() -> T + Send + 'static,
        complete: impl FnOnce(&mut Self, Result<T, String>) + 'static,
    ) {
        self.storage.queue.push_back(Queued {
            label,
            blocks_edits: false,
            session_work: false,
            start: Box::new(move |_| {
                let work: Work = Box::new(move || Box::new(run()));
                let apply: Apply = Box::new(move |app, reply| {
                    let result = reply
                        .map_err(|_| "The storage worker stopped unexpectedly.".to_owned())
                        .and_then(|value| {
                            value
                                .downcast::<T>()
                                .map(|value| *value)
                                .map_err(|_| "The storage result could not be read.".to_owned())
                        });
                    complete(app, result);
                });
                Some((work, apply))
            }),
        });
        self.ui_context.request_repaint();
    }

    pub(super) fn session_matches(&self, identity: &SessionIdentity) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.storage_identity() == *identity)
    }
}
