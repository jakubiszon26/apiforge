//! Source analysis off the UI thread.
//!
//! Typing waits 250 ms per document, whether its tab is visible or not. Each
//! analysis runs on a worker with an immutable input; the result returns with
//! its ticket, and the session drops it when the source, the configuration or
//! the session itself has moved on.

use crate::buffers::APPLY_DELAY;
use apiforge_project::{Analysis, AnalysisTicket, ProjectSession};
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc::{Receiver, Sender, channel},
    time::{Duration, Instant},
};

pub struct Analyser {
    sender: Sender<Analysis>,
    results: Receiver<Analysis>,
    /// When each document was last typed into; analysis waits for a pause.
    typed: HashMap<String, Instant>,
    running: HashSet<AnalysisTicket>,
    /// Wakes the window when a worker finishes; `None` in headless tests.
    repaint: Option<egui::Context>,
}

impl Analyser {
    pub fn new(repaint: Option<egui::Context>) -> Self {
        let (sender, results) = channel();
        Self {
            sender,
            results,
            typed: HashMap::new(),
            running: HashSet::new(),
            repaint,
        }
    }

    pub fn typed(&mut self, document_id: &str, now: Instant) {
        self.typed.insert(document_id.to_owned(), now);
    }

    #[cfg(test)]
    pub fn is_busy(&self) -> bool {
        !self.running.is_empty()
    }

    /// Installs finished results, then starts every pending analysis whose
    /// pause has passed. Returns how long until the next one may start.
    pub fn poll(&mut self, session: &mut ProjectSession, now: Instant) -> Option<Duration> {
        for analysis in self.results.try_iter() {
            self.running.remove(&analysis.ticket);
            // The session refuses a late result (further typing, a configuration
            // change or a reload); the newer input is already pending.
            let _ = session.apply_analysis(analysis);
        }
        let mut next: Option<Duration> = None;
        // Tickets first: an input copies the source and model, which a large
        // document cannot afford on every frame of its pause.
        for ticket in session.pending_tickets() {
            if self.running.contains(&ticket) {
                continue;
            }
            let document_id = &ticket.document_id;
            let wait = self.typed.get(document_id).map_or(Duration::ZERO, |typed| {
                (*typed + APPLY_DELAY).saturating_duration_since(now)
            });
            if !wait.is_zero() {
                next = Some(next.map_or(wait, |next| next.min(wait)));
                continue;
            }
            let Ok(input) = session.analysis_for(document_id) else {
                continue;
            };
            self.typed.remove(document_id);
            self.running.insert(input.ticket.clone());
            let sender = self.sender.clone();
            let repaint = self.repaint.clone();
            std::thread::spawn(move || {
                // The window may have closed; then nobody wants the result.
                let _ = sender.send(input.run());
                if let Some(context) = repaint {
                    context.request_repaint();
                }
            });
        }
        next
    }

    /// Waits for every running analysis, for tests and before a decision that
    /// needs settled results.
    #[cfg(test)]
    pub fn finish(&mut self, session: &mut ProjectSession) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.is_busy() && Instant::now() < deadline {
            if let Ok(analysis) = self.results.recv_timeout(Duration::from_millis(10)) {
                self.running.remove(&analysis.ticket);
                let _ = session.apply_analysis(analysis);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_project::{AnalysisState, DocumentKind, Origin};

    const SCENARIO: &str = "scenario \"Main\" {\n  total = 2\n}\n";

    fn session() -> (tempfile::TempDir, ProjectSession, String, String) {
        let directory = tempfile::tempdir().unwrap();
        let mut session = ProjectSession::create(directory.path(), "Folder").unwrap();
        let visible = session
            .create_document(DocumentKind::Scenario, "visible.afs", SCENARIO)
            .unwrap();
        let hidden = session
            .create_document(DocumentKind::Scenario, "hidden.afs", SCENARIO)
            .unwrap();
        (directory, session, visible, hidden)
    }

    #[test]
    fn typing_waits_for_a_pause_and_hidden_documents_settle_too() {
        let (_directory, mut session, visible, hidden) = session();
        let mut analyser = Analyser::new(None);
        let start = Instant::now();
        for id in [&visible, &hidden] {
            session
                .edit_source(id, &SCENARIO.replace('2', "3"), Origin(1))
                .unwrap();
            analyser.typed(id, start);
        }
        let wait = analyser.poll(&mut session, start).unwrap();
        assert!(wait > Duration::ZERO && wait <= APPLY_DELAY);
        assert!(!analyser.is_busy(), "nothing starts during the pause");

        assert_eq!(analyser.poll(&mut session, start + APPLY_DELAY), None);
        analyser.finish(&mut session);
        for id in [&visible, &hidden] {
            assert_eq!(
                session.document(id).unwrap().analysis(),
                AnalysisState::Valid
            );
        }
    }

    #[test]
    fn a_result_for_older_text_is_dropped_and_the_new_text_analysed() {
        let (_directory, mut session, visible, _hidden) = session();
        let mut analyser = Analyser::new(None);
        let start = Instant::now();
        session
            .edit_source(&visible, "scenario \"Main\" {\n  total = \n}\n", Origin(1))
            .unwrap();
        analyser.poll(&mut session, start);
        // More typing before the worker's result arrives.
        session
            .edit_source(&visible, &SCENARIO.replace('2', "5"), Origin(1))
            .unwrap();
        analyser.finish(&mut session);
        let document = session.document(&visible).unwrap();
        assert_eq!(document.analysis(), AnalysisState::Pending, "stale result");
        assert!(document.diagnostics().is_empty());

        analyser.poll(&mut session, start);
        analyser.finish(&mut session);
        assert_eq!(
            session.document(&visible).unwrap().analysis(),
            AnalysisState::Valid
        );
    }
}
