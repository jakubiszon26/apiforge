//! External changes off the UI thread.
//!
//! Once a second, and whenever the window regains focus or a file operation
//! finishes, a worker scans the folder and reads changed files. The session
//! applies the result: clean documents follow the disk, unsaved work gets a
//! conflict. The latest scan also feeds the explorer.
//!
//! A read-only window also asks, with each check, who holds the project
//! (MCP-07): the holder's description while a writer lives, or that the lock
//! is free, which is when the window takes the project back.

use apiforge_project::{ExternalChanges, ProjectError, ProjectSession};
use apiforge_storage::project::{Holder, TreeScan};
use eframe::egui;
use std::{
    sync::mpsc::{Receiver, Sender, channel},
    time::{Duration, Instant},
};

pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

type Result = std::result::Result<ExternalChanges, apiforge_project::Error>;
/// What a worker reports: the folder's changes, then, for a read-only
/// session, who holds the project (`None` when the lock is free). The probe
/// follows the check rather than delaying it: confirming a live writer takes
/// a moment the explorer need not wait for.
enum Message {
    Check(Result),
    Probe(std::result::Result<Option<Holder>, ProjectError>),
}

pub struct Watcher {
    sender: Sender<Message>,
    results: Receiver<Message>,
    running: bool,
    /// When the next check may start; `None` starts one on the next poll.
    due: Option<Instant>,
    focused: bool,
    /// Wakes the window when a worker finishes; `None` in headless tests.
    repaint: Option<egui::Context>,
    pub scan: Option<TreeScan>,
    /// Why the folder could not be checked, until a check succeeds.
    pub problem: Option<String>,
    pub recovery_required: bool,
    /// The latest answer to who holds the project, until the window takes it.
    probe: Option<Option<Holder>>,
    /// How long between two checks: [`POLL_INTERVAL`], longer in tests that
    /// prove a change was followed without waiting for a check.
    interval: Duration,
    /// No check starts before this: the window has decided to hand the
    /// project over and waits for the agent's first save (MCP-08b).
    held_until: Option<Instant>,
    /// When each check started, for tests of when the window reads.
    #[cfg(test)]
    pub started: Vec<Instant>,
}

impl Watcher {
    pub fn new(repaint: Option<egui::Context>) -> Self {
        let (sender, results) = channel();
        Self {
            sender,
            results,
            running: false,
            due: None,
            focused: true,
            repaint,
            scan: None,
            problem: None,
            recovery_required: false,
            probe: None,
            interval: POLL_INTERVAL,
            held_until: None,
            #[cfg(test)]
            started: Vec::new(),
        }
    }

    /// Checks every `interval` from now on instead of once a second: a test
    /// seam (MCP-08b), so that only a rescan can explain a prompt follow.
    #[cfg(test)]
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
        self.due = Some(Instant::now() + interval);
    }

    /// Starts no check before `until`, whatever asks for one, until
    /// [`Self::release_hold`]: a window handing the project over must not
    /// read the files the agent's first commit replaces, which Windows can
    /// refuse the commit (MCP-08b). The check in flight, if any, is the
    /// caller's to wait for.
    pub fn hold(&mut self, until: Instant) {
        self.held_until = Some(until);
    }

    /// Checks may start again, at once if one is due.
    pub fn release_hold(&mut self) {
        self.held_until = None;
    }

    /// Checks again as soon as possible, for example after a file operation.
    pub fn rescan(&mut self) {
        self.due = None;
    }

    /// Notes window focus; regaining it checks again at once.
    pub fn focus(&mut self, focused: bool) {
        if focused && !self.focused {
            self.rescan();
        }
        self.focused = focused;
    }

    pub fn is_busy(&self) -> bool {
        self.running
    }

    pub fn stop(&mut self) {
        self.focused = false;
    }

    pub fn resume(&mut self) {
        self.focused = true;
        self.due = None;
    }

    /// Close waits for the current reader to release its owner, without applying
    /// its obsolete result or starting another check.
    pub fn drain_stopped(&mut self) {
        for _ in self.results.try_iter() {
            self.running = false;
        }
    }

    /// The latest answer to who holds the project, once: `Some(None)` when
    /// the lock was free, `Some(Some(holder))` while a writer holds it.
    pub fn take_probe(&mut self) -> Option<Option<Holder>> {
        self.probe.take()
    }

    /// Applies a finished check, then starts the next one when it is due.
    /// Returns what changed and how long until the next check.
    pub fn poll(
        &mut self,
        session: &mut ProjectSession,
        now: Instant,
    ) -> (Option<ExternalChanges>, Duration) {
        let mut outcome = None;
        for message in self.results.try_iter() {
            let result = match message {
                Message::Check(result) => result,
                // An error leaves the last answer standing.
                Message::Probe(probe) => {
                    if let Ok(probe) = probe {
                        self.probe = Some(probe);
                    }
                    continue;
                }
            };
            self.running = false;
            match result {
                Ok(changes) => {
                    // A readable generation is not permission to resume a
                    // session that still needs an explicit reload decision.
                    self.scan = Some(changes.scan.clone());
                    self.problem = None;
                    outcome = Some(changes);
                }
                // A transaction in progress (this or another writer); the next
                // check sees its result.
                Err(apiforge_project::Error::Storage(ProjectError::Busy)) => {}
                Err(apiforge_project::Error::Storage(ProjectError::RecoveryRequired)) => {
                    self.recovery_required = true;
                }
                Err(error) => self.problem = Some(error.to_string()),
            }
        }
        if let Some(until) = self.held_until {
            if now < until {
                return (
                    outcome,
                    until.saturating_duration_since(now).min(self.interval),
                );
            }
            self.held_until = None;
        }
        let due = self.due.unwrap_or(now);
        if self.running || now < due {
            return (
                outcome,
                due.saturating_duration_since(now)
                    .max(POLL_INTERVAL / 4)
                    .min(self.interval),
            );
        }
        self.running = true;
        self.due = Some(now + self.interval);
        #[cfg(test)]
        self.started.push(now);
        let check = session.external_check();
        let observed = session.is_read_only().then(|| session.store().clone());
        let sender = self.sender.clone();
        let repaint = self.repaint.clone();
        std::thread::spawn(move || {
            // The window may have closed; then nobody wants the result.
            let _ = sender.send(Message::Check(check.run()));
            if let Some(context) = &repaint {
                context.request_repaint();
            }
            if let Some(store) = observed {
                let _ = sender.send(Message::Probe(store.holder()));
                if let Some(context) = &repaint {
                    context.request_repaint();
                }
            }
        });
        (outcome, self.interval.min(POLL_INTERVAL))
    }

    /// Checks the folder now and applies the result, for tests. A check
    /// already running may have read the folder before the caller's last
    /// write, so its answer is awaited and set aside; a check that met a
    /// transaction in progress is repeated, as the next tick would.
    #[cfg(test)]
    pub fn finish(
        &mut self,
        session: &mut ProjectSession,
    ) -> Option<apiforge_project::ExternalOutcome> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.running && Instant::now() < deadline {
            if let Ok(Message::Check(_)) = self.results.recv_timeout(Duration::from_millis(5)) {
                self.running = false;
            }
        }
        while Instant::now() < deadline {
            match session.external_check().run() {
                Ok(changes) => {
                    self.scan = Some(changes.scan.clone());
                    return session
                        .apply_external(changes)
                        .ok()
                        .filter(|outcome| !outcome.is_empty());
                }
                Err(apiforge_project::Error::Storage(ProjectError::Busy)) => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return None,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_project::{DocumentKind, ProjectSession};

    const SCENARIO: &str = "scenario \"Main\" {\n  total = 2\n}\n";

    #[test]
    fn checks_once_a_second_and_at_once_after_a_rescan() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = ProjectSession::create(directory.path(), "Folder").unwrap();
        let id = session
            .create_document(DocumentKind::Scenario, "main.afs", SCENARIO)
            .unwrap();
        let mut watcher = Watcher::new(None);
        let start = Instant::now();
        watcher.poll(&mut session, start);
        assert!(watcher.is_busy());
        assert_eq!(watcher.finish(&mut session), None);
        assert!(watcher.scan.as_ref().unwrap().entry("main.afs").is_some());

        std::fs::write(
            directory.path().join("main.afs"),
            SCENARIO.replace('2', "7"),
        )
        .unwrap();
        watcher.poll(&mut session, start + Duration::from_millis(500));
        assert!(!watcher.is_busy(), "not due yet");
        watcher.focus(false);
        watcher.focus(true);
        watcher.poll(&mut session, start + Duration::from_millis(600));
        assert!(watcher.is_busy(), "regaining focus checks at once");
        let outcome = watcher.finish(&mut session).unwrap();
        assert_eq!(outcome.reloaded, std::slice::from_ref(&id));
        assert!(session.document(&id).unwrap().text().contains('7'));
    }

    /// KTLO-15: a check that read the folder before a write and answers only
    /// after it must not stand in for a check of that write. On a loaded
    /// runner the once-a-second check can be that slow.
    #[test]
    fn finishing_checks_writes_made_while_a_slow_check_was_running() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = ProjectSession::create(directory.path(), "Folder").unwrap();
        let id = session
            .create_document(DocumentKind::Scenario, "main.afs", SCENARIO)
            .unwrap();
        let mut watcher = Watcher::new(None);

        // The check reads the folder now; its answer arrives later.
        let stale = session.external_check().run();
        watcher.running = true;
        let sender = watcher.sender.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            let _ = sender.send(Message::Check(stale));
        });

        std::fs::write(
            directory.path().join("main.afs"),
            SCENARIO.replace('2', "7"),
        )
        .unwrap();
        let outcome = watcher.finish(&mut session);
        assert_eq!(
            outcome.map(|outcome| outcome.reloaded),
            Some(vec![id.clone()]),
            "the write made while the slow check ran is followed"
        );
        assert!(session.document(&id).unwrap().text().contains('7'));
    }
}
