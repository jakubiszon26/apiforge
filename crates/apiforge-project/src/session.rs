use crate::{
    Error, Result,
    analysis::{self, Analysis, AnalysisInput, AnalysisTicket, Target},
    context,
    document::{AnalysisState, DocumentSession, Unavailable},
    layout::{FixedPlacer, Placer},
};
use apiforge_core::format::Project;
use apiforge_script::Document;
use apiforge_storage::project::{
    DiagramFile, DiagramRecord, DocumentEntry, DocumentKey, DocumentKind, FileState, MANIFEST_PATH,
    ProjectChange, ProjectManifest, ProjectPath, ProjectSnapshot, ProjectStore,
};
use std::{path::Path, sync::Arc};

#[path = "external.rs"]
mod external;
#[path = "files.rs"]
mod files;
#[path = "recovery.rs"]
mod recovery;
#[path = "register.rs"]
mod register;
pub use external::{
    ExternalChanges, ExternalCheck, ExternalConflict, ExternalOutcome, ObservedFile, RegistryState,
};
pub use files::DeletePreview;
pub use recovery::DraftStatus;
use recovery::RecoveryState;
pub use register::{
    Declaration, ImportCandidate, ImportItem, Registration, SourceSurvey, declaration,
    rename_handle,
};

const DIAGRAM_PATH: &str = ".apiforge/diagram.json";
const GITIGNORE_PATH: &str = ".gitignore";
const LOCAL_RULE: &str = "/.apiforge/local/";

/// Identifies the view or command source of a change, so that a view can
/// recognise its own edits instead of echoing them back as new commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Origin(pub u64);

impl Origin {
    pub const SESSION: Self = Self(0);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    DocumentRegistered,
    /// The document's registry path changed; its identity did not.
    DocumentMoved,
    /// The document was moved to the trash and is no longer registered.
    DocumentRemoved,
    /// The document's external conflict started or ended.
    ExternalConflict,
    /// The registry on disk changed, became invalid or was restored.
    RegistryChanged,
    SourceChanged {
        source_version: u64,
    },
    ModelChanged {
        source_version: u64,
    },
    AnalysisPending {
        source_version: u64,
    },
    AnalysisCompleted {
        source_version: u64,
        valid: bool,
    },
    Saved {
        source_version: u64,
        generation: u64,
    },
    ContextChanged {
        generation: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub sequence: u64,
    pub origin: Origin,
    pub document_id: Option<String>,
    pub kind: EventKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveOutcome {
    /// `None` when nothing differed from the saved state.
    pub generation: Option<u64>,
    pub source_version: u64,
    /// Whether the shared identity/layout checkpoint describes the saved source.
    pub checkpoint: bool,
}

/// The single owner of a folder project's documents. Commands are applied
/// serially; analysis can run elsewhere through `AnalysisInput::run`.
pub struct ProjectSession {
    store: ProjectStore,
    epoch: u64,
    manifest: ProjectManifest,
    manifest_saved: FileState,
    manifest_dirty: bool,
    /// `None` when the shared checkpoint file is unreadable or unsupported; it
    /// is preserved for explicit repair and no checkpoint is written over it.
    diagram: Option<DiagramFile>,
    diagram_saved: FileState,
    published: Vec<apiforge_core::format::Component>,
    context: Arc<Project>,
    context_generation: u64,
    documents: Vec<DocumentSession>,
    sequence: u64,
    events: Vec<Event>,
    placer: Arc<dyn Placer>,
    /// Identifies this session instance in analysis tickets and cache provenance.
    session_id: String,
    recovery: RecoveryState,
    cache: Option<apiforge_storage::project::LastValidRead>,
    /// The registry as last observed by an external check.
    registry: RegistryState,
}

impl std::fmt::Debug for ProjectSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectSession")
            .field("root", &self.store.root())
            .field("read_only", &self.store.is_read_only())
            .field("epoch", &self.epoch)
            .field("documents", &self.documents.len())
            .finish_non_exhaustive()
    }
}

impl ProjectSession {
    /// Writes a new, empty manifest. The folder must not contain a project.
    pub fn create(root: impl AsRef<Path>, name: &str) -> Result<Self> {
        Self::initialise(root, name, &[])
    }

    pub fn create_with(
        root: impl AsRef<Path>,
        name: &str,
        placer: Arc<dyn Placer>,
    ) -> Result<Self> {
        Self::initialise_with(root, name, &[], placer)
    }

    /// Opens an existing project. A folder without a manifest is left untouched.
    /// A second session on the same folder is read-only.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(root, Arc::new(FixedPlacer))
    }

    pub fn open_with(root: impl AsRef<Path>, placer: Arc<dyn Placer>) -> Result<Self> {
        let root = root.as_ref();
        if !has_manifest(root)? {
            return Err(Error::NotAProject);
        }
        Self::load(ProjectStore::open(root)?, placer, 1)
    }

    /// The saved definitions only: no recovery drafts and no last-valid cache.
    /// Used to verify what a writer produced before it is published.
    pub(crate) fn open_without_recovery(root: &Path) -> Result<Self> {
        if !has_manifest(root)? {
            return Err(Error::NotAProject);
        }
        Self::load_saved(ProjectStore::open(root)?, Arc::new(FixedPlacer), 1)
    }

    /// Rereads every file and starts a new epoch, invalidating all tickets.
    /// Unsaved work is the caller's responsibility (recovery or a decision).
    pub fn reload(&mut self) -> Result<()> {
        let placer = self.placer.clone();
        let next = Self::load(self.store.clone(), placer, self.epoch + 1)?;
        let sequence = self.sequence;
        *self = next;
        self.sequence = sequence;
        self.emit(
            Origin::SESSION,
            None,
            EventKind::ContextChanged {
                generation: self.context_generation,
            },
        );
        Ok(())
    }

    fn load(store: ProjectStore, placer: Arc<dyn Placer>, epoch: u64) -> Result<Self> {
        let mut session = Self::load_saved(store, placer, epoch)?;
        session.load_last_valid();
        session.load_recovery()?;
        Ok(session)
    }

    fn load_saved(store: ProjectStore, placer: Arc<dyn Placer>, epoch: u64) -> Result<Self> {
        let diagram_path = ProjectPath::new(DIAGRAM_PATH)?;
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        let (manifest, snapshot) = store.load()?;
        let diagram = match snapshot.files.get(&diagram_path).and_then(Option::as_deref) {
            Some(bytes) => DiagramFile::from_bytes(bytes).ok(),
            None => Some(DiagramFile {
                format_version: 1,
                records: Vec::new(),
            }),
        };
        let context = context::resolve(&manifest, &snapshot, diagram.as_ref(), placer.as_ref())?;
        let mut session = Self {
            manifest_saved: snapshot.state(&manifest_path).unwrap_or(FileState::Missing),
            diagram_saved: snapshot.state(&diagram_path).unwrap_or(FileState::Missing),
            store,
            epoch,
            manifest: manifest.clone(),
            manifest_dirty: false,
            diagram,
            published: context.components.clone(),
            context: Arc::new(context),
            context_generation: 1,
            documents: Vec::new(),
            sequence: 0,
            events: Vec::new(),
            placer,
            session_id: uuid::Uuid::new_v4().to_string(),
            recovery: RecoveryState::default(),
            cache: None,
            registry: RegistryState::Current,
        };
        for entry in manifest.documents {
            session.documents.push(document_from(&snapshot, entry));
        }
        for index in 0..session.documents.len() {
            session.analyse_now(index)?;
        }
        Ok(session)
    }

    pub fn store(&self) -> &ProjectStore {
        &self.store
    }
    pub fn is_read_only(&self) -> bool {
        self.store.is_read_only()
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }
    pub fn is_manifest_dirty(&self) -> bool {
        self.manifest_dirty
    }
    /// Public configuration and published revisions, without any scenario.
    pub fn context(&self) -> &Arc<Project> {
        &self.context
    }
    pub fn context_generation(&self) -> u64 {
        self.context_generation
    }
    pub fn documents(&self) -> &[DocumentSession] {
        &self.documents
    }
    pub fn document(&self, document_id: &str) -> Option<&DocumentSession> {
        self.documents
            .iter()
            .find(|document| document.id() == document_id)
    }
    /// Shared checkpoints are unreadable/unsupported and remain untouched.
    pub fn checkpoints_unavailable(&self) -> bool {
        self.diagram.is_none()
    }

    /// Every event with a sequence greater than `after`. Several views can
    /// follow the same log and observe the same versions.
    pub fn events_since(&self, after: u64) -> &[Event] {
        let start = self.events.partition_point(|event| event.sequence <= after);
        &self.events[start..]
    }
    pub fn last_sequence(&self) -> u64 {
        self.sequence
    }

    /// Registers a new working document with its exact source.
    pub fn create_document(
        &mut self,
        kind: DocumentKind,
        path: &str,
        text: &str,
    ) -> Result<String> {
        let path = ProjectPath::working_source(path)?;
        self.require_current_registry()?;
        if text.len() > apiforge_storage::project::MAX_SOURCE_BYTES {
            return Err(Error::SourceTooLarge);
        }
        let mut manifest = self.manifest.clone();
        let entry = DocumentEntry {
            document_id: self.new_document_id(),
            kind,
            path: path.clone(),
            entity_id: uuid::Uuid::new_v4().to_string(),
            base_revision: None,
            recovered: false,
        };
        manifest.documents.push(entry.clone());
        let mut changes = self.parent_directories(&path)?;
        changes.push(ProjectChange::Write {
            path,
            expected: FileState::Missing,
            bytes: text.as_bytes().to_vec(),
        });
        changes.push(self.manifest_change(&manifest)?);
        self.store.commit(changes)?;
        self.refresh_local_baselines();
        self.manifest_saved = FileState::of(&manifest.to_public_bytes()?);
        self.manifest = manifest;
        self.manifest_dirty = false;
        self.documents.push(DocumentSession::new(
            entry.clone(),
            text.into(),
            FileState::of(text.as_bytes()),
        ));
        self.emit(
            Origin::SESSION,
            Some(&entry.document_id),
            EventKind::DocumentRegistered,
        );
        self.analyse_now(self.documents.len() - 1)?;
        Ok(entry.document_id)
    }

    /// Replaces a document's text. Analysis is deferred: the model is locked
    /// until a matching result is applied, and other documents stay editable.
    pub fn edit_source(&mut self, document_id: &str, text: &str, origin: Origin) -> Result<u64> {
        let index = self.index(document_id)?;
        self.require_no_held_draft(document_id)?;
        let document = &mut self.documents[index];
        if document.unavailable.is_some() {
            return Err(Error::ModelLocked);
        }
        if text.len() > apiforge_storage::project::MAX_SOURCE_BYTES {
            return Err(Error::SourceTooLarge);
        }
        if document.text == text {
            return Ok(document.source_version);
        }
        document.text = text.into();
        document.source_version += 1;
        document.analysis = AnalysisState::Pending;
        let version = document.source_version;
        self.emit(
            origin,
            Some(document_id),
            EventKind::SourceChanged {
                source_version: version,
            },
        );
        self.emit(
            origin,
            Some(document_id),
            EventKind::AnalysisPending {
                source_version: version,
            },
        );
        Ok(version)
    }

    /// Replaces a document's unsaved work with its saved source: text, unsaved
    /// layout and field drafts. Its model history starts again and identities
    /// come from the saved checkpoint. A file changed outside this session is a
    /// conflict; it is never adopted silently.
    pub fn revert(&mut self, document_id: &str, origin: Origin) -> Result<()> {
        let index = self.index(document_id)?;
        self.require_no_held_draft(document_id)?;
        if self.documents[index].unavailable.is_some() {
            return Err(Error::ModelLocked);
        }
        let path = self.documents[index].entry.path.clone();
        let mut snapshot = self.store.read(std::slice::from_ref(&path))?;
        let bytes = snapshot.files.remove(&path).flatten();
        let state = bytes.as_deref().map_or(FileState::Missing, FileState::of);
        if state != self.documents[index].saved {
            return Err(Error::Storage(
                apiforge_storage::project::ProjectError::Conflict {
                    path: self.store.root().join(path.as_str()),
                },
            ));
        }
        let text = bytes
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or(Error::ModelLocked)?;
        let document = &mut self.documents[index];
        document.text = text;
        document.source_version += 1;
        document.analysis = AnalysisState::Pending;
        document.diagnostics.clear();
        document.model = None;
        document.model_source = None;
        document.layout_dirty = false;
        document.fields.clear();
        document.history = Default::default();
        let version = document.source_version;
        self.emit(
            origin,
            Some(document_id),
            EventKind::SourceChanged {
                source_version: version,
            },
        );
        self.analyse_now(index)
    }

    /// Immutable input for every pending document, including hidden ones.
    pub fn pending_analyses(&self) -> Vec<AnalysisInput> {
        (0..self.documents.len())
            .filter(|index| self.documents[*index].analysis == AnalysisState::Pending)
            .map(|index| self.analysis_input(index))
            .collect()
    }

    pub fn analysis_for(&self, document_id: &str) -> Result<AnalysisInput> {
        Ok(self.analysis_input(self.index(document_id)?))
    }

    fn analysis_input(&self, index: usize) -> AnalysisInput {
        let document = &self.documents[index];
        let key = DocumentKey::Working {
            document_id: document.entry.document_id.clone(),
        };
        AnalysisInput {
            ticket: self.ticket(index),
            target: self.target(&document.entry),
            text: document.text.as_str().into(),
            context: self.context.clone(),
            base: document.model.clone(),
            checkpoint: self
                .diagram
                .as_ref()
                .and_then(|file| file.records.iter().find(|record| record.key == key))
                .filter(|record| record.matches_document(&document.entry))
                .cloned(),
            placer: self.placer.clone(),
        }
    }

    fn ticket(&self, index: usize) -> AnalysisTicket {
        AnalysisTicket {
            session_id: self.session_id.clone(),
            session_epoch: self.epoch,
            document_id: self.documents[index].entry.document_id.clone(),
            source_version: self.documents[index].source_version,
            context_generation: self.context_generation,
        }
    }

    /// Installs a result only when its whole ticket is current. A valid model
    /// replaces the previous one as a single model-history step.
    pub fn apply_analysis(&mut self, analysis: Analysis) -> Result<()> {
        let index = self.index(&analysis.ticket.document_id)?;
        if analysis.ticket != self.ticket(index)
            || self.documents[index].analysis != AnalysisState::Pending
            || self.documents[index].unavailable.is_some()
        {
            return Err(Error::StaleAnalysis);
        }
        let document = &mut self.documents[index];
        document.diagnostics = analysis.diagnostics;
        let version = document.source_version;
        let valid = analysis.model.is_some();
        match analysis.model {
            Some((model, _)) => {
                document.analysis = AnalysisState::Valid;
                document.model_source = Some(document.text.clone());
                if document.model.as_ref() != Some(&model) {
                    if let Some(previous) = document.model.replace(model) {
                        document.history.record(previous);
                    }
                    self.emit(
                        Origin::SESSION,
                        Some(&analysis.ticket.document_id),
                        EventKind::ModelChanged {
                            source_version: version,
                        },
                    );
                }
            }
            None => document.analysis = AnalysisState::Invalid,
        }
        self.emit(
            Origin::SESSION,
            Some(&analysis.ticket.document_id),
            EventKind::AnalysisCompleted {
                source_version: version,
                valid,
            },
        );
        Ok(())
    }

    /// Settles one document synchronously, as before saving or running.
    pub fn settle(&mut self, document_id: &str) -> Result<()> {
        let index = self.index(document_id)?;
        if self.documents[index].analysis == AnalysisState::Pending {
            self.analyse_now(index)?;
        }
        Ok(())
    }

    pub fn settle_all(&mut self) -> Result<()> {
        for index in 0..self.documents.len() {
            if self.documents[index].analysis == AnalysisState::Pending {
                self.analyse_now(index)?;
            }
        }
        Ok(())
    }

    fn analyse_now(&mut self, index: usize) -> Result<()> {
        if self.documents[index].unavailable.is_some() {
            return Ok(());
        }
        let analysis = self.analysis_input(index).run();
        self.apply_analysis(analysis)
    }

    /// A graphical edit. Semantic changes reprint canonical source and mark it
    /// dirty; a pure layout change leaves the source untouched.
    pub fn apply_model_edit(
        &mut self,
        document_id: &str,
        mut model: Document,
        origin: Origin,
    ) -> Result<()> {
        let index = self.index(document_id)?;
        self.require_no_held_draft(document_id)?;
        if !self.documents[index].accepts_model_edits() {
            return Err(Error::ModelLocked);
        }
        let target = self.target(&self.documents[index].entry);
        if analysis::kind(&model) != target.kind {
            return Err(Error::InvalidModel);
        }
        analysis::normalise(&mut model, &target);
        let previous = self.documents[index]
            .model
            .clone()
            .ok_or(Error::ModelLocked)?;
        if previous == model {
            return Ok(());
        }
        self.install_model(index, model, origin)?;
        self.documents[index].history.record(previous);
        Ok(())
    }

    pub fn undo(&mut self, document_id: &str, origin: Origin) -> Result<bool> {
        self.step_history(document_id, origin, true)
    }

    pub fn redo(&mut self, document_id: &str, origin: Origin) -> Result<bool> {
        self.step_history(document_id, origin, false)
    }

    fn step_history(&mut self, document_id: &str, origin: Origin, undo: bool) -> Result<bool> {
        let index = self.index(document_id)?;
        if !self.documents[index].accepts_model_edits() {
            return Err(Error::ModelLocked);
        }
        let history = &mut self.documents[index].history;
        let Some(model) = (if undo {
            history.undo.pop()
        } else {
            history.redo.pop()
        }) else {
            return Ok(false);
        };
        let current = self.documents[index]
            .model
            .clone()
            .ok_or(Error::ModelLocked)?;
        if let Err(error) = self.install_model(index, model.clone(), origin) {
            let history = &mut self.documents[index].history;
            if undo {
                history.undo.push(model);
            } else {
                history.redo.push(model);
            }
            return Err(error);
        }
        let history = &mut self.documents[index].history;
        if undo {
            history.redo.push(current);
        } else {
            history.undo.push(current);
        }
        Ok(true)
    }

    fn install_model(&mut self, index: usize, model: Document, origin: Origin) -> Result<()> {
        let context = apiforge_script::LowerContext::folder(&self.context);
        apiforge_script::validate_document_model(&model, &context)
            .map_err(|_| Error::InvalidModel)?;
        let document = &self.documents[index];
        let previous = document.model.as_ref().ok_or(Error::ModelLocked)?;
        let semantic = apiforge_script::semantic(previous, &self.context)
            != apiforge_script::semantic(&model, &self.context);
        let text = if semantic {
            Some(
                apiforge_script::print_folder_document(&self.context, &model)
                    .map_err(|_| Error::Unprintable)?,
            )
        } else {
            None
        };
        let document_id = document.entry.document_id.clone();
        let document = &mut self.documents[index];
        document.model = Some(model);
        match text {
            Some(text) => {
                document.model_source = Some(text.clone());
                document.text = text;
                document.source_version += 1;
                document.diagnostics.clear();
                let version = document.source_version;
                self.emit(
                    origin,
                    Some(&document_id),
                    EventKind::SourceChanged {
                        source_version: version,
                    },
                );
            }
            None => document.layout_dirty = true,
        }
        let version = self.documents[index].source_version;
        self.emit(
            origin,
            Some(&document_id),
            EventKind::ModelChanged {
                source_version: version,
            },
        );
        Ok(())
    }

    /// Changes shared configuration. Every document must be analysed again, and
    /// results computed against the previous configuration become stale.
    pub fn edit_configuration(
        &mut self,
        edit: impl FnOnce(&mut ProjectManifest),
        origin: Origin,
    ) -> Result<()> {
        let mut manifest = self.manifest.clone();
        edit(&mut manifest);
        manifest.validate()?;
        if manifest.documents != self.manifest.documents
            || manifest.revisions != self.manifest.revisions
            || manifest.id != self.manifest.id
        {
            return Err(Error::Storage(
                apiforge_storage::project::ProjectError::InvalidMetadata,
            ));
        }
        let mut project = context::base_project(&manifest);
        project.components = self.published.clone();
        self.manifest = manifest;
        self.manifest_dirty = true;
        self.context = Arc::new(project);
        self.context_generation += 1;
        for index in 0..self.documents.len() {
            if self.documents[index].unavailable.is_none() {
                self.documents[index].analysis = AnalysisState::Pending;
            }
        }
        self.emit(
            origin,
            None,
            EventKind::ContextChanged {
                generation: self.context_generation,
            },
        );
        Ok(())
    }

    /// Saves shared configuration without saving any document, including when
    /// the project is empty. Failure keeps the configuration dirty and editable.
    pub fn save_configuration(&mut self) -> Result<Option<u64>> {
        if !self.manifest_dirty {
            return Ok(None);
        }
        self.require_current_registry()?;
        let bytes = self.manifest.to_public_bytes()?;
        let generation = self
            .store
            .commit(vec![self.manifest_change(&self.manifest)?])?;
        self.manifest_saved = FileState::of(&bytes);
        self.manifest_dirty = false;
        self.refresh_local_baselines();
        Ok(Some(generation))
    }

    /// Saves exactly the document's current text, including invalid text, plus
    /// its checkpoint when the model describes that text, plus dirty project
    /// configuration. Never saves another document's source.
    pub fn save(&mut self, document_id: &str) -> Result<SaveOutcome> {
        let index = self.index(document_id)?;
        self.require_no_held_draft(document_id)?;
        self.require_current_registry()?;
        if self.documents[index].conflict.is_some() {
            return Err(Error::ExternalConflict);
        }
        if self.documents[index].unavailable.is_some() {
            return Err(Error::ModelLocked);
        }
        // Settle first, so that a valid saved source also gets its checkpoint.
        if self.documents[index].analysis == AnalysisState::Pending {
            self.analyse_now(index)?;
        }
        let document = &self.documents[index];
        let version = document.source_version;
        let bytes = document.text.as_bytes().to_vec();
        let mut changes = Vec::new();
        if FileState::of(&bytes) != document.saved {
            changes.push(ProjectChange::Write {
                path: document.entry.path.clone(),
                expected: document.saved.clone(),
                bytes: bytes.clone(),
            });
        }
        let mut diagram = self.diagram.clone();
        let mut checkpoint = false;
        if document.analysis == AnalysisState::Valid
            && let (Some(model), Some(file)) = (&document.model, diagram.as_mut())
        {
            let key = DocumentKey::Working {
                document_id: document_id.into(),
            };
            let record = DiagramRecord::capture(
                key.clone(),
                &document.text,
                analysis::identity_base(model),
                &self.context,
            )?;
            file.records.retain(|existing| existing.key != key);
            file.records.push(record);
            checkpoint = true;
        }
        let diagram_bytes = match &diagram {
            Some(file) if checkpoint => Some(file.to_bytes()?),
            _ => None,
        };
        if let Some(bytes) = &diagram_bytes
            && FileState::of(bytes) != self.diagram_saved
        {
            changes.push(ProjectChange::Write {
                path: ProjectPath::new(DIAGRAM_PATH)?,
                expected: self.diagram_saved.clone(),
                bytes: bytes.clone(),
            });
        }
        if self.manifest_dirty {
            changes.push(self.manifest_change(&self.manifest)?);
        }
        if changes.is_empty() {
            self.documents[index].layout_dirty = false;
            return Ok(SaveOutcome {
                generation: None,
                source_version: version,
                checkpoint,
            });
        }
        let generation = self.store.commit(changes)?;
        self.refresh_local_baselines();
        let document = &mut self.documents[index];
        document.saved = FileState::of(&bytes);
        document.layout_dirty = false;
        if let Some(bytes) = diagram_bytes {
            self.diagram_saved = FileState::of(&bytes);
            self.diagram = diagram;
        }
        if self.manifest_dirty {
            self.manifest_saved = FileState::of(&self.manifest.to_public_bytes()?);
            self.manifest_dirty = false;
        }
        self.emit(
            Origin::SESSION,
            Some(document_id),
            EventKind::Saved {
                source_version: version,
                generation,
            },
        );
        // The saved draft is obsolete. A failure here is reported by the next
        // explicit recovery write, which close and project switch require.
        let _ = self.save_recovery();
        Ok(SaveOutcome {
            generation: Some(generation),
            source_version: version,
            checkpoint,
        })
    }

    fn manifest_change(&self, manifest: &ProjectManifest) -> Result<ProjectChange> {
        Ok(ProjectChange::Write {
            path: ProjectPath::new(MANIFEST_PATH)?,
            expected: self.manifest_saved.clone(),
            bytes: manifest.to_public_bytes()?,
        })
    }

    fn parent_directories(&self, path: &ProjectPath) -> Result<Vec<ProjectChange>> {
        let mut changes = Vec::new();
        let mut parent = String::new();
        let parts: Vec<_> = path.as_str().split('/').collect();
        for part in &parts[..parts.len() - 1] {
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(part);
            let directory = ProjectPath::new(parent.clone())?;
            if !self.store.root().join(directory.as_str()).is_dir() {
                changes.push(ProjectChange::CreateDirectory { path: directory });
            }
        }
        Ok(changes)
    }

    fn target(&self, entry: &DocumentEntry) -> Target {
        Target {
            kind: entry.kind,
            entity_id: entry.entity_id.clone(),
            revision: working_revision(&self.manifest, entry),
        }
    }

    fn new_document_id(&self) -> String {
        loop {
            let id = uuid::Uuid::new_v4().to_string();
            if self.document(&id).is_none() {
                return id;
            }
        }
    }

    fn index(&self, document_id: &str) -> Result<usize> {
        self.documents
            .iter()
            .position(|document| document.id() == document_id)
            .ok_or(Error::UnknownDocument)
    }

    fn emit(&mut self, origin: Origin, document_id: Option<&str>, kind: EventKind) {
        self.sequence += 1;
        self.events.push(Event {
            sequence: self.sequence,
            origin,
            document_id: document_id.map(str::to_owned),
            kind,
        });
    }
}

fn empty_manifest(name: &str) -> ProjectManifest {
    ProjectManifest {
        format_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        environment: apiforge_core::format::Environment {
            base_url: String::new(),
            variables: Default::default(),
            secret_names: Vec::new(),
        },
        contracts: Vec::new(),
        documents: Vec::new(),
        revisions: Vec::new(),
    }
}

/// A working component carries the revision its next publication would get.
fn working_revision(manifest: &ProjectManifest, entry: &DocumentEntry) -> u32 {
    if entry.kind == DocumentKind::Scenario {
        return 0;
    }
    manifest
        .revisions
        .iter()
        .filter(|revision| revision.component_id == entry.entity_id)
        .map(|revision| revision.revision)
        .max()
        .unwrap_or(0)
        + 1
}

fn document_from(snapshot: &ProjectSnapshot, entry: DocumentEntry) -> DocumentSession {
    let bytes = snapshot.files.get(&entry.path).and_then(Option::as_deref);
    let (text, unavailable) = match bytes.map(std::str::from_utf8) {
        None => (String::new(), Some(Unavailable::Missing)),
        Some(Err(_)) => (String::new(), Some(Unavailable::NotUtf8)),
        Some(Ok(text)) => (text.to_owned(), None),
    };
    let mut document =
        DocumentSession::new(entry, text, bytes.map_or(FileState::Missing, FileState::of));
    if unavailable.is_some() {
        document.unavailable = unavailable;
        document.analysis = AnalysisState::Invalid;
    }
    document
}

fn has_manifest(root: &Path) -> Result<bool> {
    let probe = ProjectStore::open_read_only(root)?;
    let path = ProjectPath::new(MANIFEST_PATH)?;
    Ok(probe.read(std::slice::from_ref(&path))?.files[&path].is_some())
}

/// Keeps local session data out of Git: appends the rule to an existing
/// `.gitignore` without rewriting it, or creates one.
fn ignore_local_data(store: &ProjectStore) -> Result<Vec<ProjectChange>> {
    let path = ProjectPath::new(GITIGNORE_PATH)?;
    let current = store.read(std::slice::from_ref(&path))?.files[&path].clone();
    let Some(bytes) = current else {
        return Ok(vec![ProjectChange::Write {
            path,
            expected: FileState::Missing,
            bytes: format!("{LOCAL_RULE}\n").into_bytes(),
        }]);
    };
    let covered = String::from_utf8_lossy(&bytes).lines().any(|line| {
        matches!(
            line.trim(),
            "/.apiforge/local/" | "/.apiforge/local" | ".apiforge/local/" | ".apiforge/local"
        )
    });
    if covered {
        return Ok(Vec::new());
    }
    let mut next = bytes.clone();
    if !next.is_empty() && !next.ends_with(b"\n") {
        next.push(b'\n');
    }
    next.extend_from_slice(format!("{LOCAL_RULE}\n").as_bytes());
    Ok(vec![ProjectChange::Write {
        path,
        expected: FileState::of(&bytes),
        bytes: next,
    }])
}
