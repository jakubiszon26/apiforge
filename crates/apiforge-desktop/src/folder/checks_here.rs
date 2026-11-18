//! A step's own choice of its checks (LAY-30): the steps of each document
//! whose checks hang under them while the boards fold checks, or fold while
//! the boards draw them. Kept on this computer in
//! `.apiforge/local/board-checks.json`, outside Git and outside the project's
//! format; *Checks* on the bar forgets every choice.
use super::FolderApp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The record's name under `.apiforge/local/`.
const RECORD: &str = "board-checks.json";

/// The choices, by document id, each a set of step handles.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ChecksHere {
    version: u32,
    /// Whether the boards folded checks when the choices were made: a
    /// choice is the opposite of the boards' setting, so one made under the
    /// other setting is not one any more.
    board_folds: bool,
    documents: BTreeMap<String, BTreeSet<String>>,
}

impl ChecksHere {
    /// The steps of `document` with a choice of their own.
    pub(super) fn of(&self, document: &str) -> impl Iterator<Item = String> + '_ {
        self.documents.get(document).into_iter().flatten().cloned()
    }

    fn set(&mut self, document: &str, step: &str, chosen: bool) {
        let steps = self.documents.entry(document.to_owned()).or_default();
        if chosen {
            steps.insert(step.to_owned());
        } else {
            steps.remove(step);
        }
        if steps.is_empty() {
            self.documents.remove(document);
        }
    }
}

impl FolderApp {
    /// Reads the choices of the project just opened, which a step's switch
    /// made before the read finished keeps.
    pub(super) fn load_checks_here(&mut self) {
        self.checks_here = ChecksHere {
            version: 1,
            board_folds: self.preferences.checks_collapsed,
            documents: BTreeMap::new(),
        };
        let Some(session) = &self.session else {
            return;
        };
        let identity = session.storage_identity();
        let store = session.store().clone();
        self.storage_io(
            "Loading the checks of each step…",
            move || store.read_local(RECORD),
            move |app, result| {
                if !app.session_matches(&identity) {
                    return;
                }
                let Ok(Ok(Some(bytes))) = result else {
                    return;
                };
                let Ok(stored) = serde_json::from_slice::<ChecksHere>(&bytes) else {
                    return;
                };
                if stored.version != 1 || stored.board_folds != app.preferences.checks_collapsed {
                    return;
                }
                for (document, steps) in stored.documents {
                    app.checks_here.documents.entry(document).or_insert(steps);
                }
                let documents: Vec<String> = app.canvases.keys().cloned().collect();
                for document in documents {
                    app.show_checks_here(&document);
                }
            },
        );
    }

    /// Gives the board of `document` its steps' choices.
    pub(super) fn show_checks_here(&mut self, document: &str) {
        let steps: Vec<String> = self.checks_here.of(document).collect();
        if let Some(canvas) = self.canvases.get_mut(document) {
            canvas.set_here(steps);
        }
    }

    /// Sets the choice of `step` (a handle) in `document`, against the
    /// boards' setting, and keeps it.
    pub(super) fn choose_checks_here(&mut self, document: &str, step: &str, chosen: bool) {
        self.checks_here.set(document, step, chosen);
        self.show_checks_here(document);
        self.save_checks_here();
    }

    /// Whether `step` of `document` has a choice of its own.
    pub(super) fn checks_chosen(&self, document: &str, step: &str) -> bool {
        self.checks_here
            .documents
            .get(document)
            .is_some_and(|steps| steps.contains(step))
    }

    /// Forgets every step's choice: *Checks* on the bar sets every step.
    pub(super) fn forget_checks_here(&mut self, board_folds: bool) {
        self.checks_here.board_folds = board_folds;
        self.checks_here.documents.clear();
        let documents: Vec<String> = self.canvases.keys().cloned().collect();
        for document in documents {
            self.show_checks_here(&document);
        }
        self.save_checks_here();
    }

    /// Writes the choices to `.apiforge/local/`; a window that only reads
    /// the project keeps them for this session.
    fn save_checks_here(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        if session.is_read_only() {
            return;
        }
        let Ok(bytes) = serde_json::to_vec_pretty(&self.checks_here) else {
            return;
        };
        let store = session.store().clone();
        self.storage_io(
            "Saving the checks of each step…",
            move || store.write_local(RECORD, &bytes),
            |_, _| {},
        );
    }
}
