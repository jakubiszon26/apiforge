//! External changes off the UI thread.
//!
//! Once a second, and whenever the window regains focus or a file operation
//! finishes, a worker scans the folder and reads changed files. The session
//! applies the result: clean documents follow the disk, unsaved work gets a
//! conflict. The latest scan also feeds the explorer.

use apiforge_project::{ExternalChanges, ProjectError, ProjectSession};
use apiforge_storage::project::TreeScan;
use eframe::egui;
use std::{
    sync::mpsc::{Receiver, Sender, channel},
    time::{Duration, Instant},
};

pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

type Result = std::result::Result<ExternalChanges, apiforge_project::Error>;

pub struct Watcher {
    sender: Sender<Result>,
    results: Receiver<Result>,
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
        }
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

    /// Applies a finished check, then starts the next one when it is due.
    /// Returns what changed and how long until the next check.
    pub fn poll(
        &mut self,
        session: &mut ProjectSession,
        now: Instant,
    ) -> (Option<ExternalChanges>, Duration) {
        let mut outcome = None;
        for result in self.results.try_iter() {
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
        let due = self.due.unwrap_or(now);
        if self.running || now < due {
            return (
                outcome,
                due.saturating_duration_since(now).max(POLL_INTERVAL / 4),
            );
        }
        self.running = true;
        self.due = Some(now + POLL_INTERVAL);
        let check = session.external_check();
        let sender = self.sender.clone();
        let repaint = self.repaint.clone();
        std::thread::spawn(move || {
            // The window may have closed; then nobody wants the result.
            let _ = sender.send(check.run());
            if let Some(context) = repaint {
                context.request_repaint();
            }
        });
        (outcome, POLL_INTERVAL)
    }

    /// Waits for the running check and applies it, for tests.
    #[cfg(test)]
    pub fn finish(
        &mut self,
        session: &mut ProjectSession,
    ) -> Option<apiforge_project::ExternalOutcome> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.running && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            if let Ok(result) = self.results.recv_timeout(Duration::from_millis(5)) {
                self.running = false;
                if let Ok(changes) = result {
                    self.scan = Some(changes.scan.clone());
                    return session
                        .apply_external(changes)
                        .ok()
                        .filter(|outcome| !outcome.is_empty());
                }
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
            let _ = sender.send(stale);
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
