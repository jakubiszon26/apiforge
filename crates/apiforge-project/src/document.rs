//! One registered document: exact source, its analysis, its model and the model
//! history. The text-edit history belongs to the view adapter (ADR 0012).
use apiforge_script::{Diagnostic, Document};
use apiforge_storage::project::{DocumentEntry, FieldRecovery, FileState, TextSelection};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisState {
    Pending,
    Valid,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelState {
    /// Derived from the current source.
    Current,
    /// An earlier model of this session; the source is pending or invalid.
    LastValid,
    Absent,
}

/// Saved data that cannot be edited as source text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    Missing,
    NotUtf8,
    /// Over the 1 MiB source limit.
    TooLarge,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct History {
    pub undo: Vec<Arc<Document>>,
    pub redo: Vec<Arc<Document>>,
    pub generation: u64,
}

const HISTORY_LIMIT: usize = 100;

impl History {
    pub fn record(&mut self, previous: Arc<Document>) {
        if self.undo.len() == HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(previous);
        self.redo.clear();
        self.generation += 1;
    }
}

#[derive(Clone, Debug)]
pub struct DocumentSession {
    pub(crate) entry: DocumentEntry,
    /// Changed only through `set_text`, which forgets `text_state`.
    text: String,
    /// The state `text` would have on disk, hashed once per text and kept
    /// because views ask whether a document is dirty several times a frame.
    text_state: OnceLock<FileState>,
    pub(crate) source_version: u64,
    /// The disk state this session last read or wrote: the save baseline.
    pub(crate) saved: FileState,
    pub(crate) unavailable: Option<Unavailable>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Diagnostics the analysis found beyond the ones it keeps.
    pub(crate) omitted_diagnostics: usize,
    pub(crate) analysis: AnalysisState,
    pub(crate) model: Option<Arc<Document>>,
    /// The source text `model` describes, when known. Differs from `text` while
    /// the current source is pending or invalid.
    pub(crate) model_source: Option<String>,
    /// Sealed while the model was current, including its original configuration.
    pub(crate) valid_snapshot: Option<apiforge_storage::project::LastValidSnapshot>,
    /// View state kept for recovery: directional selection and field drafts.
    pub(crate) selection: TextSelection,
    pub(crate) fields: Vec<FieldRecovery>,
    /// Layout differs from the saved checkpoint while semantics do not.
    pub(crate) layout_dirty: bool,
    pub(crate) history: History,
    /// The saved file changed or disappeared outside the session while this
    /// document had unsaved work.
    pub(crate) conflict: Option<crate::session::ExternalConflict>,
    /// The exact source version for which comment loss was accepted.
    pub(crate) comments_accepted: Option<u64>,
}

impl DocumentSession {
    pub fn id(&self) -> &str {
        &self.entry.document_id
    }
    pub fn entry(&self) -> &DocumentEntry {
        &self.entry
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn source_version(&self) -> u64 {
        self.source_version
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    /// How many diagnostics the analysis found beyond the
    /// `MAX_DIAGNOSTICS` that `diagnostics` keeps.
    pub fn omitted_diagnostics(&self) -> usize {
        self.omitted_diagnostics
    }
    /// SHA-256 of the saved bytes this session last read or wrote; `None`
    /// when the file is missing.
    pub fn saved_sha256(&self) -> Option<&str> {
        match &self.saved {
            FileState::File { sha256, .. } => Some(sha256),
            _ => None,
        }
    }
    pub fn analysis(&self) -> AnalysisState {
        self.analysis
    }
    pub fn unavailable(&self) -> Option<Unavailable> {
        self.unavailable
    }
    /// Source differs from the save baseline. Layout-only changes are separate.
    pub fn is_dirty(&self) -> bool {
        self.unavailable.is_none() && self.text_state() != &self.saved
    }
    pub fn selection(&self) -> &TextSelection {
        &self.selection
    }
    pub fn field_drafts(&self) -> &[FieldRecovery] {
        &self.fields
    }
    /// Unsaved work that local recovery must protect.
    pub fn needs_recovery(&self) -> bool {
        self.is_dirty() || self.layout_dirty || !self.fields.is_empty()
    }
    pub fn is_layout_dirty(&self) -> bool {
        self.layout_dirty
    }
    /// The latest valid model, with `model_state` telling whether it is current.
    pub fn model(&self) -> Option<&Document> {
        self.model.as_deref()
    }
    /// The same model, shared: a view can hold it without copying it, and tell
    /// by identity whether it has been replaced.
    pub fn shared_model(&self) -> Option<&Arc<Document>> {
        self.model.as_ref()
    }
    pub fn model_state(&self) -> ModelState {
        match (&self.model, self.analysis) {
            (None, _) => ModelState::Absent,
            (Some(_), AnalysisState::Valid) => ModelState::Current,
            (Some(_), _) => ModelState::LastValid,
        }
    }
    /// Graph mutations are allowed only on a model derived from current source.
    pub fn accepts_model_edits(&self) -> bool {
        self.unavailable.is_none()
            && self.conflict.is_none()
            && self.model_state() == ModelState::Current
    }
    pub fn can_undo(&self) -> bool {
        self.accepts_model_edits() && !self.history.undo.is_empty()
    }
    pub fn conflict(&self) -> Option<crate::session::ExternalConflict> {
        self.conflict
    }
    pub fn can_redo(&self) -> bool {
        self.accepts_model_edits() && !self.history.redo.is_empty()
    }
}

impl DocumentSession {
    /// Replaces the text. Callers advance `source_version` as their change
    /// requires.
    pub(crate) fn set_text(&mut self, text: String) {
        self.text = text;
        self.text_state = OnceLock::new();
    }

    /// The state the current text has once written.
    pub(crate) fn text_state(&self) -> &FileState {
        self.text_state
            .get_or_init(|| FileState::of(self.text.as_bytes()))
    }

    pub(crate) fn new(entry: DocumentEntry, text: String, saved: FileState) -> Self {
        Self {
            entry,
            text,
            text_state: OnceLock::new(),
            source_version: 1,
            saved,
            unavailable: None,
            diagnostics: Vec::new(),
            omitted_diagnostics: 0,
            analysis: AnalysisState::Pending,
            model: None,
            model_source: None,
            valid_snapshot: None,
            selection: TextSelection {
                anchor: 0,
                caret: 0,
            },
            fields: Vec::new(),
            layout_dirty: false,
            comments_accepted: None,
            history: History::default(),
            conflict: None,
        }
    }
}
