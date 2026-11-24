use crate::{
    Error, Result,
    analysis::{self, Analysis, AnalysisInput, AnalysisTicket, Restored, Target},
    context,
    document::{AnalysisState, DocumentSession, Step, Unavailable},
    language,
    layout::{FixedPlacer, Placer},
};
use apiforge_core::{ImportError, format::Project};
use apiforge_script::{Diagnostic, Document, Severity};
use apiforge_storage::project::{
    DiagramFile, DiagramRecord, DocumentEntry, DocumentKey, DocumentKind, FileState, Holder,
    MANIFEST_PATH, ProjectChange, ProjectError, ProjectManifest, ProjectPath, ProjectSnapshot,
    ProjectStore,
};
use std::{collections::HashSet, path::Path, sync::Arc};

#[path = "comments.rs"]
mod comments;
#[path = "conflicts.rs"]
mod conflicts;
pub use conflicts::{ConflictDecision, ConflictPreview, ConflictTarget};
#[path = "storage_work.rs"]
mod storage_work;
pub use storage_work::{SessionIdentity, StorageCompletion, StorageMode, StorageSnapshot};
#[path = "execution.rs"]
mod execution;
#[path = "external.rs"]
mod external;
pub use execution::PreparedRun;
#[path = "files.rs"]
mod files;
#[path = "new_document.rs"]
mod new_document;
pub use new_document::{NewDocumentCheck, RegistrationProblem};
#[path = "publish.rs"]
mod publish;
#[path = "recovery.rs"]
mod recovery;
#[path = "register.rs"]
mod register;
#[path = "text.rs"]
mod text;
pub use external::{
    CommitChange, CommitChangeKind, ExternalChanges, ExternalCheck, ExternalConflict,
    ExternalOutcome, ObservedFile, RegistryState,
};
pub use files::DeletePreview;
pub use publish::Published;
pub use recovery::DraftStatus;
use recovery::RecoveryState;
pub use register::{
    Declaration, ImportCandidate, ImportItem, Registration, SourceSurvey, declaration,
    rename_handle,
};
pub use text::TextFileSession;

pub(crate) const DIAGRAM_PATH: &str = ".apiforge/diagram.json";
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
    /// A working component became an immutable published revision.
    Published {
        component_id: String,
        revision: u32,
    },
    /// A project text file, identified by its path rather than a document ID.
    TextFileOpened {
        path: String,
    },
    TextFileChanged {
        path: String,
        version: u64,
    },
    TextFileSaved {
        path: String,
        generation: u64,
    },
    TextFileClosed {
        path: String,
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
    /// The context generations a publication produced, with the component
    /// published: a run that calls none of them stays fresh across them
    /// (UX-25, plan decision 16).
    publications: Vec<(u64, String)>,
    documents: Vec<DocumentSession>,
    /// Project files open as plain text; they are not definitions.
    texts: Vec<TextFileSession>,
    sequence: u64,
    events: Vec<Event>,
    placer: Arc<dyn Placer>,
    /// Identifies this session instance in analysis tickets and cache provenance.
    session_id: String,
    recovery: RecoveryState,
    cache: Option<apiforge_storage::project::LastValidRead>,
    /// The registry as last observed by an external check.
    registry: RegistryState,
    /// The head generation of the stable read this session was loaded from.
    loaded_generation: u64,
    /// Every registered revision file as loaded, which an observer
    /// fingerprints to follow a publication (MCP-08a).
    revisions_saved: Vec<(ProjectPath, FileState)>,
}

impl std::fmt::Debug for ProjectSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectSession")
            .field("root", &self.store.root())
            .field("read_only", &self.store.is_read_only())
            .field("epoch", &self.epoch)
            .field("documents", &self.documents.len())
            .field("texts", &self.texts.len())
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
        Self::open_with_as(root, placer, &Holder::window())
    }

    /// [`Self::open_with`] for a writer that describes itself as `holder`.
    pub fn open_with_as(
        root: impl AsRef<Path>,
        placer: Arc<dyn Placer>,
        holder: &Holder,
    ) -> Result<Self> {
        let root = root.as_ref();
        if !has_manifest(root)? {
            return Err(Error::NotAProject);
        }
        let store = ProjectStore::open_as(root, holder)?;
        // Beside a living holder this session observes: it shows what the
        // holder committed, never the holder's unsaved drafts or its cache
        // (MCP-08a, planning decision 1). A probe that fails counts as a
        // living holder: never the holder's drafts by mistake.
        if store.is_read_only() && store.has_live_writer().unwrap_or(true) {
            return Self::load_saved(store, placer, 1);
        }
        Self::load(store, placer, 1)
    }

    /// Stable committed definitions only, without local recovery or cache and
    /// without creating project-local files. Shared with CLI checks and runs.
    pub fn open_saved_read_only(root: impl AsRef<Path>) -> Result<Self> {
        if !has_manifest(root.as_ref())? {
            return Err(Error::NotAProject);
        }
        Self::load_saved(
            ProjectStore::open_read_only(root)?,
            Arc::new(FixedPlacer),
            1,
        )
    }

    /// As `open_saved_read_only`, but a registered source over the 1 MiB
    /// limit does not fail the load: that document is `Unavailable::TooLarge`
    /// and the others are served (MCP-03).
    pub fn open_saved_tolerant(root: impl AsRef<Path>) -> Result<Self> {
        if !has_manifest(root.as_ref())? {
            return Err(Error::NotAProject);
        }
        Self::load_tolerant(ProjectStore::open_read_only(root)?)
    }

    /// One stable read of `store` in which a registered source over the 1 MiB
    /// limit is `Unavailable::TooLarge` instead of a failed load.
    fn load_tolerant(store: ProjectStore) -> Result<Self> {
        let (manifest, snapshot, too_large) = match store.load() {
            Ok((manifest, snapshot)) => (manifest, snapshot, HashSet::new()),
            Err(ProjectError::Limit) => load_tolerating_large_sources(&store)?,
            Err(error) => return Err(error.into()),
        };
        Self::from_snapshot(
            store,
            Arc::new(FixedPlacer),
            1,
            manifest,
            snapshot,
            &too_large,
        )
    }

    /// A session over the saved definitions of a store the caller already
    /// holds, such as the writer lock an agent session keeps across calls
    /// (MCP-04): no local recovery and no last-valid cache. A registered
    /// source over the 1 MiB limit is `Unavailable::TooLarge`, as in
    /// [`Self::open_saved_tolerant`].
    pub fn from_store_saved(store: ProjectStore) -> Result<Self> {
        if !has_manifest(store.root())? {
            return Err(Error::NotAProject);
        }
        Self::load_tolerant(store)
    }

    /// Stable saved definitions under the writer lock. Never falls back to a
    /// read-only session when another writer owns the project.
    pub fn open_saved_exclusive(root: impl AsRef<Path>) -> Result<Self> {
        Self::open_saved_exclusive_as(root, &Holder::window())
    }

    /// [`Self::open_saved_exclusive`] for a writer that describes itself as
    /// `holder`, such as a command-line subcommand.
    pub fn open_saved_exclusive_as(root: impl AsRef<Path>, holder: &Holder) -> Result<Self> {
        if !has_manifest(root.as_ref())? {
            return Err(Error::NotAProject);
        }
        Self::load_saved(
            ProjectStore::open_exclusive_as(root, holder)?,
            Arc::new(FixedPlacer),
            1,
        )
    }

    /// A verified, closed snapshot of a current scenario and its exact public
    /// dependencies. Another invalid scenario cannot block this one.
    pub fn current_snapshot(
        &mut self,
        document_id: &str,
    ) -> Result<apiforge_storage::project::LastValidSnapshot> {
        if let Some(problem) = self.language_problem() {
            return Err(problem);
        }
        self.require_no_held_draft(document_id)?;
        self.require_current_registry()?;
        self.settle(document_id)?;
        let document = &self.documents[self.index(document_id)?];
        // A scenario or, run on its own, a component (KTLO-23).
        if !matches!(
            document.entry.kind,
            DocumentKind::Scenario | DocumentKind::Component
        ) || document.analysis != AnalysisState::Valid
            || document.conflict.is_some()
            || !document.fields.is_empty()
        {
            return Err(Error::ModelLocked);
        }
        let model = document.model.as_ref().ok_or(Error::ModelLocked)?;
        Ok(apiforge_storage::project::LastValidSnapshot::capture(
            DocumentKey::Working {
                document_id: document_id.into(),
            },
            document.text(),
            apiforge_storage::project::CacheProvenance {
                session_epoch: self.session_id.clone(),
                source_version: document.source_version,
                saved_source_hash: match &document.saved {
                    FileState::File { sha256, .. } if !document.is_dirty() => Some(sha256.clone()),
                    _ => None,
                },
            },
            analysis::identity_base(model),
            &self.context,
        )?)
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
        self.events.clear();
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
        let (manifest, snapshot) = store.load()?;
        Self::from_snapshot(store, placer, epoch, manifest, snapshot, &HashSet::new())
    }

    /// A session over one stable read. A registered source in `too_large` was
    /// left out of the read: it is `Unavailable::TooLarge`.
    fn from_snapshot(
        store: ProjectStore,
        placer: Arc<dyn Placer>,
        epoch: u64,
        manifest: ProjectManifest,
        snapshot: ProjectSnapshot,
        too_large: &HashSet<ProjectPath>,
    ) -> Result<Self> {
        language::refuse_newer(&manifest)?;
        let diagram_path = ProjectPath::new(DIAGRAM_PATH)?;
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
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
            publications: Vec::new(),
            documents: Vec::new(),
            texts: Vec::new(),
            sequence: 0,
            events: Vec::new(),
            placer,
            session_id: uuid::Uuid::new_v4().to_string(),
            recovery: RecoveryState::default(),
            cache: None,
            registry: RegistryState::Current,
            loaded_generation: snapshot.generation,
            revisions_saved: manifest
                .revisions
                .iter()
                .map(|entry| {
                    (
                        entry.path.clone(),
                        snapshot.state(&entry.path).unwrap_or(FileState::Missing),
                    )
                })
                .collect(),
        };
        for entry in manifest.documents {
            let large = too_large.contains(&entry.path);
            let mut document = document_from(&snapshot, entry);
            if large {
                document.unavailable = Some(Unavailable::TooLarge);
            }
            session.documents.push(document);
        }
        for index in 0..session.documents.len() {
            session.analyse_now(index)?;
        }
        Ok(session)
    }

    pub fn store(&self) -> &ProjectStore {
        &self.store
    }
    /// The head generation of the stable read this session was loaded from
    /// (0 before the project's first commit on this machine). Commits made by
    /// the session itself do not advance it; `reload` does.
    pub fn loaded_generation(&self) -> u64 {
        self.loaded_generation
    }
    pub fn is_read_only(&self) -> bool {
        self.store.is_read_only()
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    /// The problem of a project written in an older language than this
    /// build's (LANG-15): `check` reports it, and a run is refused with it
    /// until `apiforge fix` has raised the project's language.
    pub fn language_problem(&self) -> Option<Error> {
        language::older(&self.manifest, self.store.root())
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
        // A valid source seals its checkpoint as its analysis is applied.
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
        if document.text() == text {
            return Ok(document.source_version);
        }
        document.set_text(text.into());
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

    /// Applies one diagnostic's suggested fix to the source, as an ordinary
    /// edit, so it joins the document's history and can be undone. The
    /// explicit pin for an unqualified component call is such a fix.
    ///
    /// The diagnostic is named by its position, and a document whose analysis
    /// has not settled has none to offer: a range taken from an earlier text
    /// could otherwise cut this one apart.
    pub fn apply_fix(
        &mut self,
        document_id: &str,
        diagnostic: usize,
        origin: Origin,
    ) -> Result<u64> {
        let index = self.index(document_id)?;
        if self.documents[index].analysis == AnalysisState::Pending {
            return Err(Error::StaleAnalysis);
        }
        let fix = self.documents[index]
            .diagnostics
            .get(diagnostic)
            .and_then(|diagnostic| diagnostic.fix.clone())
            .ok_or(Error::NoFix)?;
        let mut text = self.documents[index].text().to_owned();
        if fix.range.end > text.len()
            || !text.is_char_boundary(fix.range.start)
            || !text.is_char_boundary(fix.range.end)
        {
            return Err(Error::StaleAnalysis);
        }
        text.replace_range(fix.range, &fix.replacement);
        self.edit_source(document_id, &text, origin)
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
        document.set_text(text);
        document.source_version += 1;
        document.analysis = AnalysisState::Pending;
        document.diagnostics.clear();
        document.omitted_diagnostics = 0;
        document.model = None;
        document.model_source = None;
        document.layout_dirty = false;
        document.fields.clear();
        document.history.clear();
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

    /// The ticket of every pending document: what `pending_analyses` would
    /// return, without copying any source or model into an input.
    pub fn pending_tickets(&self) -> Vec<AnalysisTicket> {
        (0..self.documents.len())
            .filter(|index| self.documents[*index].analysis == AnalysisState::Pending)
            .map(|index| self.ticket(index))
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
            text: document.text().into(),
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

    /// Installs a result only when its whole ticket is current. A valid
    /// result whose text differs from the text the history stands at is one
    /// step of the history, whether or not the model changed (DIAG-06g): a
    /// comment or a blank line typed in Code is undone as itself.
    pub fn apply_analysis(&mut self, analysis: Analysis) -> Result<()> {
        let index = self.index(&analysis.ticket.document_id)?;
        if analysis.ticket != self.ticket(index)
            || self.documents[index].analysis != AnalysisState::Pending
            || self.documents[index].unavailable.is_some()
        {
            return Err(Error::StaleAnalysis);
        }
        let arranged = analysis.restored() == Some(Restored::Arranged);
        let document = &mut self.documents[index];
        document.diagnostics = analysis.diagnostics;
        document.omitted_diagnostics = analysis.omitted_diagnostics;
        let version = document.source_version;
        let valid = analysis.model.is_some();
        match analysis.model {
            Some((model, _)) => {
                document.analysis = AnalysisState::Valid;
                document.model_source = Some(document.text().to_owned());
                let present = document.history.present.replace(document.text().to_owned());
                if let (Some(text), Some(previous)) = (present, &document.model)
                    && text != document.text()
                {
                    let model = previous.clone();
                    document.history.record(Step { text, model });
                }
                if document.model.as_deref() != Some(&model) {
                    document.model = Some(Arc::new(model));
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
        self.seal_valid(index);
        if valid {
            self.seal_first_checkpoint(index);
        }
        if arranged {
            self.seal_arranged(index);
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

    /// A graphical edit. A semantic change rewrites the source of what it
    /// changes and nothing else (LAY-34) and marks it dirty; a pure layout
    /// change leaves the source untouched.
    pub fn apply_model_edit(
        &mut self,
        document_id: &str,
        model: Document,
        origin: Origin,
    ) -> Result<()> {
        self.apply_edit(document_id, model, origin, Written::Rebuilt)
    }

    /// A move on the board (DIAG-06a): `model` is the document with a step
    /// put at a place by [`apiforge_core::edit::place`], `moved` the blocks
    /// it moved. Their lines are moved in the text and nothing else changes;
    /// a move that cannot be written so is refused with
    /// [`Error::MoveNotWritten`] and never printed whole. One step of Undo.
    ///
    /// The model already holds the chain the move made, so LANG-13's rebuild
    /// of the chain ([`kept_chain`]) is not applied to it.
    pub fn apply_move(
        &mut self,
        document_id: &str,
        model: Document,
        moved: &[String],
        origin: Origin,
    ) -> Result<()> {
        self.apply_edit(document_id, model, origin, Written::Moved(moved))
    }

    /// A paste or a duplicate on the board (DIAG-06f): `model` is the
    /// document with the copies written in their places by
    /// [`apiforge_core::edit::paste_statements`] or
    /// [`apiforge_core::edit::duplicate_statements`], with the chain they
    /// made. The new statements are written into the text where the model
    /// writes them and nothing else changes; the chain is not rebuilt. One
    /// step of Undo.
    pub fn apply_pasted(
        &mut self,
        document_id: &str,
        model: Document,
        origin: Origin,
    ) -> Result<()> {
        self.apply_edit(document_id, model, origin, Written::Placed)
    }

    fn apply_edit(
        &mut self,
        document_id: &str,
        mut model: Document,
        origin: Origin,
        written: Written<'_>,
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
        let moved = match written {
            Written::Moved(moved) => Some(moved),
            Written::Rebuilt | Written::Placed => None,
        };
        let model = match written {
            Written::Rebuilt => kept_chain(&previous, model),
            Written::Placed => with_start(model),
            Written::Moved(_) => model,
        };
        if *previous == model {
            return Ok(());
        }
        // A move that changes no step's place, nor any statement's braces,
        // changes nothing: the order of the statements is the text's, which
        // it would leave as it is (DIAG-06d).
        if moved.is_some()
            && apiforge_script::semantic(&previous, &self.context)
                == apiforge_script::semantic(&model, &self.context)
        {
            return Ok(());
        }
        let text = self.documents[index].text().to_owned();
        self.install_model(index, model, origin, moved)?;
        let document = &mut self.documents[index];
        document.history.record(Step {
            text,
            model: previous,
        });
        document.history.present = Some(document.text().to_owned());
        Ok(())
    }

    pub fn undo(&mut self, document_id: &str, origin: Origin) -> Result<bool> {
        self.step_history(document_id, origin, true)
    }

    pub fn redo(&mut self, document_id: &str, origin: Origin) -> Result<bool> {
        self.step_history(document_id, origin, false)
    }

    /// Steps the history back or forward: the step's text is installed
    /// exactly as it was, with the model read from it (DIAG-06g). A Code edit
    /// not yet settled is settled first, so that it is a step of its own and
    /// is what the Undo takes back.
    fn step_history(&mut self, document_id: &str, origin: Origin, undo: bool) -> Result<bool> {
        let index = self.index(document_id)?;
        if self.documents[index].unavailable.is_none()
            && self.documents[index].conflict.is_none()
            && self.documents[index].analysis == AnalysisState::Pending
        {
            self.analyse_now(index)?;
        }
        if !self.documents[index].accepts_model_edits() {
            return Err(Error::ModelLocked);
        }
        let history = &mut self.documents[index].history;
        let Some(step) = (if undo {
            history.undo.pop()
        } else {
            history.redo.pop()
        }) else {
            return Ok(false);
        };
        let document = &self.documents[index];
        let current = Step {
            text: document.text().to_owned(),
            model: document.model.clone().ok_or(Error::ModelLocked)?,
        };
        if let Err(error) = self.install_step(index, &step, origin) {
            let history = &mut self.documents[index].history;
            if undo {
                history.undo.push(step);
            } else {
                history.redo.push(step);
            }
            return Err(error);
        }
        let history = &mut self.documents[index].history;
        history.generation += 1;
        if undo {
            history.redo.push(current);
        } else {
            history.undo.push(current);
        }
        Ok(true)
    }

    /// Installs a step of the history: its text byte for byte, and the model
    /// read from that text with the step's identities and positions
    /// (KTLO-26). A text that no longer reads as valid in the project's
    /// current context is refused and nothing changes.
    fn install_step(&mut self, index: usize, step: &Step, origin: Origin) -> Result<()> {
        let target = self.target(&self.documents[index].entry);
        let (diagnostics, omitted, model) = analysis::derive_counted(
            &target,
            &step.text,
            &self.context,
            Some(&step.model),
            None,
            self.placer.as_ref(),
        );
        let Some((model, _)) = model else {
            return Err(refused_text(&diagnostics));
        };
        let document = &mut self.documents[index];
        let document_id = document.entry.document_id.clone();
        document.model = Some(Arc::new(model));
        document.model_source = Some(step.text.clone());
        document.history.present = Some(step.text.clone());
        document.diagnostics = diagnostics;
        document.omitted_diagnostics = omitted;
        if document.text() == step.text {
            document.layout_dirty = true;
        } else {
            document.set_text(step.text.clone());
            document.source_version += 1;
            let version = document.source_version;
            self.emit(
                origin,
                Some(&document_id),
                EventKind::SourceChanged {
                    source_version: version,
                },
            );
        }
        self.seal_valid(index);
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

    fn install_model(
        &mut self,
        index: usize,
        model: Document,
        origin: Origin,
        moved: Option<&[String]>,
    ) -> Result<()> {
        let context = apiforge_script::LowerContext::folder(&self.context);
        apiforge_script::validate_document_model(&model, &context)
            .map_err(|error| refused_model(&model, &self.context, error))?;
        let document = &self.documents[index];
        let previous = document.model.as_ref().ok_or(Error::ModelLocked)?;
        let semantic = apiforge_script::semantic(previous, &self.context)
            != apiforge_script::semantic(&model, &self.context);
        let (model, written) = if semantic {
            // The edit changes the text of what it changes and nothing else
            // (LAY-34). Only printing the whole file drops comments that
            // belong to no statement, so only then is the person asked first
            // (owner, 2026-11-17).
            let in_place = match moved {
                Some(moved) => apiforge_script::edit_in_place_moving(
                    &self.context,
                    document.text(),
                    previous,
                    &model,
                    moved,
                ),
                None => {
                    apiforge_script::edit_in_place(&self.context, document.text(), previous, &model)
                }
            };
            let text = match in_place {
                Some(text) => text,
                // A move is written line by line or not at all (DIAG-06a).
                None if moved.is_some() => return Err(Error::MoveNotWritten),
                None => {
                    self.require_comment_decision(index)?;
                    apiforge_script::print_folder_document(&self.context, &model)
                        .map_err(|_| Error::Unprintable)?
                }
            };
            // Keep the model the printed text reads back, with the edited
            // model's identities and positions: a board edit may store blocks
            // or inputs in an order the source cannot state, and the
            // checkpoint sealed from the model must describe the source
            // (KTLO-26). What the reading reports is what the text's analysis
            // would (#578): nothing analyses it again after the edit.
            let target = self.target(&document.entry);
            match analysis::derive_counted(
                &target,
                &text,
                &self.context,
                Some(&model),
                None,
                self.placer.as_ref(),
            ) {
                (diagnostics, omitted, Some((read_back, _))) => {
                    (read_back, Some((text, diagnostics, omitted)))
                }
                (_, _, None) => (model, Some((text, Vec::new(), 0))),
            }
        } else {
            (model, None)
        };
        let document_id = document.entry.document_id.clone();
        let document = &mut self.documents[index];
        document.model = Some(Arc::new(model));
        match written {
            Some((text, diagnostics, omitted)) => {
                document.model_source = Some(text.clone());
                document.set_text(text);
                document.source_version += 1;
                document.diagnostics = diagnostics;
                document.omitted_diagnostics = omitted;
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
        self.seal_valid(index);
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
        if manifest == self.manifest {
            return Ok(());
        }
        // Validate immutable revision sources against the proposed settings,
        // including sources that use a display name instead of a stable
        // contract ID. A failed edit never changes memory or the save baseline.
        let (_, snapshot) = self.store.load()?;
        if snapshot.state(&ProjectPath::new(MANIFEST_PATH)?) != Some(self.manifest_saved.clone()) {
            return Err(Error::RegistryChanged);
        }
        let project = context::resolve(
            &manifest,
            &snapshot,
            self.diagram.as_ref(),
            self.placer.as_ref(),
        )?;
        self.published = project.components.clone();
        self.manifest = manifest;
        self.manifest_dirty = true;
        self.context = Arc::new(project);
        self.invalidate_runtime_inputs(origin);
        Ok(())
    }

    /// Session-only execution inputs changed (for example secret values).
    /// Invalidate analysis/run tickets without making public settings dirty.
    pub fn invalidate_runtime_inputs(&mut self, origin: Origin) {
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
        let bytes = document.text().as_bytes().to_vec();
        let mut changes = Vec::new();
        if FileState::of(&bytes) != document.saved {
            changes.push(ProjectChange::Write {
                path: document.entry.path.clone(),
                expected: document.saved.clone(),
                bytes: bytes.clone(),
            });
        }
        let diagram = self.checkpoint(index)?;
        let checkpoint = diagram.is_some();
        let diagram_bytes = diagram.as_ref().map(DiagramFile::to_bytes).transpose()?;
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

    /// The shared diagram file with this document's identity/layout record
    /// merged in, or `None` when the document has no valid model to describe.
    fn checkpoint(&self, index: usize) -> Result<Option<DiagramFile>> {
        let document = &self.documents[index];
        if document.analysis != AnalysisState::Valid {
            return Ok(None);
        }
        let (Some(model), Some(file)) = (&document.model, &self.diagram) else {
            return Ok(None);
        };
        let key = DocumentKey::Working {
            document_id: document.id().into(),
        };
        let record = DiagramRecord::capture(
            key.clone(),
            document.text(),
            analysis::identity_base(model),
            &self.context,
        )?;
        let mut file = file.clone();
        file.records.retain(|existing| existing.key != key);
        file.records.push(record);
        Ok(Some(file))
    }

    /// Seals the identities and layout of a document at its first successful
    /// analysis of saved source (owner decision of 2026-10-26, KTLO-04): a
    /// created, initialised, registered or imported document as soon as it is
    /// registered, an invalid one when its saved source first analyses
    /// cleanly. Without a record every later load would allocate fresh
    /// identities for its blocks. An existing record is never replaced here;
    /// saves keep their own route. Unsaved text is not sealed, because shared
    /// checkpoints describe saved valid source (ADR 0010), and a session that
    /// only reads the project writes nothing.
    fn seal_first_checkpoint(&mut self, index: usize) {
        let document = &self.documents[index];
        if self.store.is_read_only() || document.is_dirty() || document.conflict.is_some() {
            return;
        }
        let key = DocumentKey::Working {
            document_id: document.id().into(),
        };
        let Some(file) = &self.diagram else { return };
        if file.records.iter().any(|record| record.key == key) {
            return;
        }
        self.seal_new_checkpoint(index);
    }

    /// Saves the layout the host's placer made again for a document restored
    /// from its checkpoint (LAY-33), on the conditions of the first seal, so
    /// that the next open reads it and the document is left with nothing to
    /// save.
    fn seal_arranged(&mut self, index: usize) {
        let document = &self.documents[index];
        if self.store.is_read_only() || document.is_dirty() || document.conflict.is_some() {
            return;
        }
        self.seal_new_checkpoint(index);
    }

    /// Writes the checkpoint of a document whose text matches its saved file
    /// exactly, so nothing marks it dirty and no save is owed. ADR 0010 forbids
    /// failing valid source over a missing checkpoint, so a write that cannot
    /// be made is left to the next save rather than failing the registration,
    /// import or analysis that already succeeded.
    fn seal_new_checkpoint(&mut self, index: usize) {
        let Ok(Some(file)) = self.checkpoint(index) else {
            return;
        };
        let Ok(bytes) = file.to_bytes() else { return };
        if FileState::of(&bytes) == self.diagram_saved {
            return;
        }
        let Ok(path) = ProjectPath::new(DIAGRAM_PATH) else {
            return;
        };
        if self
            .store
            .commit(vec![ProjectChange::Write {
                path,
                expected: self.diagram_saved.clone(),
                bytes: bytes.clone(),
            }])
            .is_err()
        {
            return;
        }
        self.refresh_local_baselines();
        self.diagram_saved = FileState::of(&bytes);
        self.diagram = Some(file);
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
            stored: false,
            older: (crate::language::support(
                self.manifest.language(),
                crate::language::LANGUAGE_VERSION,
            ) == crate::language::Support::Older)
                .then(|| self.store.root().to_owned()),
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
    language::new_manifest(
        uuid::Uuid::new_v4().to_string(),
        name.into(),
        apiforge_core::format::Environment {
            base_url: String::new(),
            variables: Default::default(),
            secret_names: Vec::new(),
        },
        Vec::new(),
    )
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
        .saturating_add(1)
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

/// The registry, the checkpoint, the published revisions and every registered
/// source within the 1 MiB limit, in one stable read; the registered sources
/// over the limit are returned instead of read. A published revision or a
/// registry over its limit still fails the load.
fn load_tolerating_large_sources(
    store: &ProjectStore,
) -> Result<(ProjectManifest, ProjectSnapshot, HashSet<ProjectPath>)> {
    let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
    let first = store.read(std::slice::from_ref(&manifest_path))?;
    let registry = first.files[&manifest_path]
        .clone()
        .ok_or(ProjectError::InvalidMetadata)?;
    let manifest = ProjectManifest::from_bytes(&registry)?;
    let mut too_large = HashSet::new();
    for entry in &manifest.documents {
        match store.read(std::slice::from_ref(&entry.path)) {
            Ok(_) => {}
            Err(ProjectError::Limit) => {
                too_large.insert(entry.path.clone());
            }
            Err(error) => return Err(error.into()),
        }
    }
    if too_large.is_empty() {
        return Err(ProjectError::Limit.into());
    }
    let mut paths = vec![manifest_path.clone(), ProjectPath::new(DIAGRAM_PATH)?];
    paths.extend(
        manifest
            .documents
            .iter()
            .map(|entry| entry.path.clone())
            .filter(|path| !too_large.contains(path)),
    );
    paths.extend(manifest.revisions.iter().map(|entry| entry.path.clone()));
    let snapshot = store.read(&paths)?;
    // The registry the sources were chosen by is the one read with them.
    if snapshot.files[&manifest_path].as_deref() != Some(registry.as_slice()) {
        return Err(ProjectError::Busy.into());
    }
    for entry in &manifest.revisions {
        let state = snapshot.state(&entry.path);
        if !matches!(state, Some(FileState::File { sha256, .. }) if sha256 == entry.source_hash) {
            return Err(ProjectError::InvalidMetadata.into());
        }
    }
    Ok((manifest, snapshot, too_large))
}

/// Whether `root` has a manifest. One of a project written in a newer
/// language is refused here, before a store opens for writing (LANG-15);
/// one that cannot be read is the load's to report.
fn has_manifest(root: &Path) -> Result<bool> {
    let probe = ProjectStore::open_read_only(root)?;
    let path = ProjectPath::new(MANIFEST_PATH)?;
    let Some(bytes) = probe
        .read(std::slice::from_ref(&path))?
        .files
        .remove(&path)
        .flatten()
    else {
        return Ok(false);
    };
    if let Ok(manifest) = ProjectManifest::from_bytes(&bytes) {
        language::refuse_newer(&manifest)?;
    }
    Ok(true)
}

/// Keeps local session data out of Git: appends the rule to an existing
/// `.gitignore` without rewriting it, or creates one.
pub(crate) fn ignore_local_data(store: &ProjectStore) -> Result<Vec<ProjectChange>> {
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

/// How a board edit says the chain of the written language.
#[derive(Clone, Copy)]
enum Written<'a> {
    /// The edit changed the graph only: the chain is rebuilt
    /// ([`kept_chain`]).
    Rebuilt,
    /// A paste or a duplicate (DIAG-06f): the model holds the chain the edit
    /// made, and its new statements are written where it writes them.
    Placed,
    /// A move (DIAG-06a): the model holds the chain the move made, and the
    /// lines of the blocks it moved are moved.
    Moved(&'a [String]),
}

/// `model` with a *Start* written first in a scenario's main flow that has
/// a step and none, as rule 1 of the written language requires; what the
/// edit wrote is kept.
fn with_start(mut model: Document) -> Document {
    if let Document::Scenario(scenario) = &mut model {
        scenario.graph = apiforge_core::chain::with_start(&scenario.graph, &[&scenario.cleanup]);
    }
    model
}

/// A board edit keeps the chain of the written language (LANG-13): each
/// graph of the edited model is rebuilt by [`apiforge_core::chain::edited`],
/// and the first step placed in a scenario's main flow without a *Start*
/// gets one written above it, as rule 1 and the fix of a file require. A
/// component needs no *Start*.
fn kept_chain(previous: &Document, mut model: Document) -> Document {
    use apiforge_core::chain::{edited, with_start};
    match (previous, &mut model) {
        (Document::Scenario(before), Document::Scenario(after)) => {
            after.cleanup = edited(&before.cleanup, &after.cleanup);
            after.graph = with_start(&edited(&before.graph, &after.graph), &[&after.cleanup]);
        }
        (Document::Component(before), Document::Component(after)) => {
            after.graph = edited(&before.graph, &after.graph);
        }
        _ => {}
    }
    model
}

/// The refusal of an edited model (#565): the validation's message, and the
/// step its path points at. The path itself is never shown.
fn refused_model(model: &Document, context: &Project, error: ImportError) -> Error {
    match error {
        ImportError::InvalidDocument { path, message } => Error::EditRefused {
            reason: message.to_owned(),
            step: refused_step(model, context, &path),
        },
        error => Error::EditRefused {
            reason: error.to_string(),
            step: None,
        },
    }
}

/// The handle of the step a validation path points at, when the path is one
/// of the edited document's own nodes.
fn refused_step(model: &Document, context: &Project, path: &str) -> Option<String> {
    let path = path.strip_prefix("project.")?;
    let (graph, path) = match model {
        Document::Scenario(scenario) => {
            let path = path.strip_prefix("scenarios[0].")?;
            match path.strip_prefix("graph.") {
                Some(path) => (&scenario.graph, path),
                None => (&scenario.cleanup, path.strip_prefix("cleanup.")?),
            }
        }
        // Validated after every published revision of the project.
        Document::Component(component) => {
            let own = format!("components[{}].", context.components.len());
            (&component.graph, path.strip_prefix(&own)?)
        }
    };
    let (index, _) = path.strip_prefix("nodes[")?.split_once(']')?;
    let node = graph.nodes.get(index.parse::<usize>().ok()?)?;
    Some(node.handle.clone())
}

/// The refusal of a text that no longer makes a valid definition (#565): its
/// first error, in the words of the text's check.
fn refused_text(diagnostics: &[Diagnostic]) -> Error {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)
        .map_or(Error::InvalidModel, |diagnostic| Error::EditRefused {
            reason: diagnostic.message.clone(),
            step: None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Origin;
    use apiforge_script::{FAR_READ, SyntaxTree};

    /// Places as [`FixedPlacer`] and warns of far reads as the window does,
    /// after `.0` copies of the first, which fill the list.
    struct FarReads(usize);

    impl Placer for FarReads {
        fn place(&self, document: &mut Document, fresh: &[String], context: &Project) {
            FixedPlacer.place(document, fresh, context);
        }
        fn advise(
            &self,
            tree: &SyntaxTree,
            document: &Document,
            context: &Project,
        ) -> Vec<Diagnostic> {
            let far = apiforge_script::far_read_warnings(tree, document, context);
            let filler = far.iter().cycle().take(self.0).cloned();
            filler.chain(far.iter().cloned()).collect()
        }
    }

    const TYPE: &str = "The value's type does not match this input";
    /// One warning of the language, on `"b"`, and two far reads of `a`.
    const SOURCE: &str = "scenario \"S\" {\n  begin = start\n  a = GET \"/a\"\n  b = GET \"/b\"\n  c = GET \"/c/{{a.body.id}}\" {\n    body { again: a.body.id }\n  }\n  sum = c.status + \"b\"\n}\n";

    fn opened(filler: usize) -> (tempfile::TempDir, ProjectSession, String) {
        let directory = tempfile::tempdir().unwrap();
        let placer = Arc::new(FarReads(filler));
        let mut session = ProjectSession::create_with(directory.path(), "S", placer).unwrap();
        let id = session
            .create_document(DocumentKind::Scenario, "scenarios/s.afs", SOURCE)
            .unwrap();
        session.settle(&id).unwrap();
        (directory, session, id)
    }

    /// The document with the method of `handle` set to `method`.
    fn with_method(session: &ProjectSession, id: &str, handle: &str, method: &str) -> Document {
        let mut model = session.document(id).unwrap().model().unwrap().clone();
        let Document::Scenario(scenario) = &mut model else {
            panic!("a scenario")
        };
        let node = scenario
            .graph
            .nodes
            .iter_mut()
            .find(|node| node.handle == handle)
            .unwrap();
        node.config.method = Some(serde_json::from_value(method.into()).unwrap());
        model
    }

    /// The warnings as their message and the text each covers.
    fn covered(session: &ProjectSession, id: &str) -> Vec<(String, String)> {
        let document = session.document(id).unwrap();
        document
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.message.clone(),
                    document.text()[diagnostic.range.clone()].to_owned(),
                )
            })
            .collect()
    }

    /// What an analysis of the text the session holds reports.
    fn fresh(session: &ProjectSession, id: &str) -> (Vec<Diagnostic>, usize) {
        let analysis = session.analysis_for(id).unwrap().run();
        assert!(analysis.is_valid());
        (analysis.diagnostics, analysis.omitted_diagnostics)
    }

    fn reported(session: &ProjectSession, id: &str) -> (Vec<Diagnostic>, usize) {
        let document = session.document(id).unwrap();
        (
            document.diagnostics().to_vec(),
            document.omitted_diagnostics(),
        )
    }

    /// #578: a board edit keeps the warnings of the text it writes, with
    /// their ranges in that text; so do its Undo and Redo.
    #[test]
    fn a_model_edit_keeps_the_warnings_of_its_text() {
        let (_directory, mut session, id) = opened(0);
        let expected = [
            (TYPE.to_owned(), "\"b\"".to_owned()),
            (FAR_READ.to_owned(), "a.body.id".to_owned()),
            (FAR_READ.to_owned(), "a.body.id".to_owned()),
        ];
        assert_eq!(covered(&session, &id), expected);
        let before = reported(&session, &id);

        // Above every warning, and longer: every range moves.
        let model = with_method(&session, &id, "b", "DELETE");
        session
            .apply_model_edit(&id, model, Origin::SESSION)
            .unwrap();
        let edited = session.document(&id).unwrap().text().to_owned();
        assert_eq!(edited, SOURCE.replace("GET \"/b\"", "DELETE \"/b\""));
        assert_eq!(covered(&session, &id), expected);
        assert_ne!(reported(&session, &id), before, "the ranges moved");
        assert_eq!(reported(&session, &id), fresh(&session, &id));
        let after = reported(&session, &id);

        assert!(session.undo(&id, Origin::SESSION).unwrap());
        assert_eq!(reported(&session, &id), before, "Undo");
        assert!(session.redo(&id, Origin::SESSION).unwrap());
        assert_eq!(reported(&session, &id), after, "Redo");

        // A change of layout alone leaves the text, and so its warnings.
        let mut model = session.document(&id).unwrap().model().unwrap().clone();
        let Document::Scenario(scenario) = &mut model else {
            panic!("a scenario")
        };
        scenario.graph.nodes[0].position.x += 40.0;
        session
            .apply_model_edit(&id, model, Origin::SESSION)
            .unwrap();
        assert_eq!(session.document(&id).unwrap().text(), edited);
        assert_eq!(reported(&session, &id), after);
    }

    /// #578: what the edit does not touch stays, what it removes goes.
    #[test]
    fn a_model_edit_drops_a_warning_whose_text_it_removes() {
        let (_directory, mut session, id) = opened(0);
        let mut model = session.document(&id).unwrap().model().unwrap().clone();
        let Document::Scenario(scenario) = &mut model else {
            panic!("a scenario")
        };
        scenario.graph.nodes.retain(|node| node.handle != "sum");
        session
            .apply_model_edit(&id, model, Origin::SESSION)
            .unwrap();
        let far = (FAR_READ.to_owned(), "a.body.id".to_owned());
        assert_eq!(covered(&session, &id), [far.clone(), far]);
        assert_eq!(reported(&session, &id), fresh(&session, &id));
    }

    /// #578: the number of warnings beyond the limit is the new text's too.
    #[test]
    fn a_model_edit_keeps_the_count_of_omitted_warnings() {
        let (_directory, mut session, id) = opened(apiforge_script::MAX_DIAGNOSTICS);
        let before = reported(&session, &id);
        assert_eq!(before.0.len(), apiforge_script::MAX_DIAGNOSTICS);
        assert_eq!(before.1, 3, "one of the language and two far reads more");
        let model = with_method(&session, &id, "b", "DELETE");
        session
            .apply_model_edit(&id, model, Origin::SESSION)
            .unwrap();
        assert_eq!(session.document(&id).unwrap().omitted_diagnostics(), 3);
        assert_eq!(reported(&session, &id), fresh(&session, &id));
    }

    /// The text and the model the session holds, with what it reports.
    fn held(session: &ProjectSession, id: &str) -> (String, Document, (Vec<Diagnostic>, usize)) {
        let document = session.document(id).unwrap();
        (
            document.text().to_owned(),
            document.model().unwrap().clone(),
            reported(session, id),
        )
    }

    /// The sentence and the fields of the refusal of `edit` on a document
    /// with `source`, which is left as it was.
    fn refused(
        kind: DocumentKind,
        source: &str,
        edit: impl FnOnce(&mut Document),
    ) -> (String, String) {
        let directory = tempfile::tempdir().unwrap();
        let mut session =
            ProjectSession::create_with(directory.path(), "S", Arc::new(FixedPlacer)).unwrap();
        let path = match kind {
            DocumentKind::Scenario => "scenarios/s.afs",
            DocumentKind::Component => "components/s.afs",
        };
        let id = session.create_document(kind, path, source).unwrap();
        session.settle(&id).unwrap();
        if kind == DocumentKind::Component {
            // Its step is named beside a published revision of its own too.
            session.publish(&id, Origin::SESSION).unwrap();
            session.settle(&id).unwrap();
        }
        let before = held(&session, &id);
        let mut model = before.1.clone();
        edit(&mut model);
        let error = session
            .apply_model_edit(&id, model, Origin::SESSION)
            .unwrap_err();
        assert_eq!(held(&session, &id), before, "nothing changes");
        (error.to_string(), format!("{error:?}"))
    }

    fn main(model: &mut Document) -> &mut apiforge_core::format::Graph {
        match model {
            Document::Scenario(scenario) => &mut scenario.graph,
            Document::Component(component) => &mut component.graph,
        }
    }

    const PAY: &str = "scenario \"S\" {\n  begin = start\n  order = GET \"/order\"\n  pay = POST \"/pay\"\n  cleanup {\n    undo = DELETE \"/pay\"\n  }\n}\n";

    /// #565, criterion 1: the sentence, with the step its fault names.
    #[test]
    fn a_refused_model_edit_names_its_reason() {
        let literal = || apiforge_core::format::Binding::literal("x".into());
        assert_eq!(
            refused(DocumentKind::Scenario, PAY, |model| {
                main(model).nodes[2].handle = "order".into();
            }),
            (
                "This edit was not applied: duplicate handle. Nothing was changed.".to_owned(),
                "EditRefused { reason: \"Duplicate handle\", step: None }".to_owned()
            ),
            "no step"
        );
        assert_eq!(
            refused(DocumentKind::Scenario, PAY, |model| {
                main(model).nodes[2].inputs.clear();
            }),
            (
                "This edit was not applied: a request needs a URL (step \"pay\"). Nothing was changed."
                    .to_owned(),
                "EditRefused { reason: \"A request needs a URL\", step: Some(\"pay\") }".to_owned()
            ),
            "a step"
        );
        assert_eq!(
            refused(DocumentKind::Scenario, PAY, |model| {
                main(model).nodes[0].inputs.insert("x".into(), literal());
            })
            .0,
            "This edit was not applied: Start cannot have inputs (step \"begin\"). Nothing was changed.",
            "a kept capital"
        );
        // A word written in capitals keeps them, and has no step of its own
        // when the fault is not a step's.
        assert_eq!(
            refused(DocumentKind::Scenario, PAY, |model| {
                let mut deep = serde_json::Value::Null;
                for _ in 0..41 {
                    deep = serde_json::Value::Array(vec![deep]);
                }
                let Document::Scenario(scenario) = model else {
                    panic!("a scenario")
                };
                scenario.parameters.insert("deep".into(), deep);
            })
            .0,
            "This edit was not applied: JSON exceeds maximum nesting depth. Nothing was changed."
        );
        assert_eq!(
            refused(DocumentKind::Scenario, PAY, |model| {
                let Document::Scenario(scenario) = model else {
                    panic!("a scenario")
                };
                scenario.cleanup.nodes[0].inputs.clear();
            })
            .0,
            "This edit was not applied: a request needs a URL (step \"undo\"). Nothing was changed.",
            "a step of the cleanup"
        );
        assert_eq!(
            refused(
                DocumentKind::Component,
                "component fetch \"Fetch\" {\n  first = GET \"/a\"\n  second = GET \"/b\"\n}\n",
                |model| main(model).nodes[1].inputs.clear()
            )
            .0,
            "This edit was not applied: a request needs a URL (step \"second\"). Nothing was changed.",
            "a step of a component"
        );
    }

    /// #565, criterion 2: a step of the history whose text no longer reads
    /// as valid is refused with the first error of that text.
    #[test]
    fn a_refused_step_names_its_reason() {
        let directory = tempfile::tempdir().unwrap();
        let mut session =
            ProjectSession::create_with(directory.path(), "S", Arc::new(FixedPlacer)).unwrap();
        session
            .edit_configuration(
                |manifest| {
                    manifest.contracts = vec![apiforge_core::format::Contract {
                        id: "order".into(),
                        name: "Order".into(),
                        sample: serde_json::json!({ "id": 1 }),
                    }]
                },
                Origin::SESSION,
            )
            .unwrap();
        let without = "scenario \"S\" {\n  begin = start\n  a = GET \"/a\"\n}\n";
        let with = without.replace("\"/a\"", "\"/a\" {\n    contract \"Order\"\n  }");
        let id = session
            .create_document(DocumentKind::Scenario, "scenarios/s.afs", &with)
            .unwrap();
        session.settle(&id).unwrap();
        session.edit_source(&id, without, Origin::SESSION).unwrap();
        session.settle(&id).unwrap();
        session
            .edit_configuration(|manifest| manifest.contracts.clear(), Origin::SESSION)
            .unwrap();
        session.settle(&id).unwrap();
        let before = held(&session, &id);

        let error = session.undo(&id, Origin::SESSION).unwrap_err();
        assert_eq!(
            error.to_string(),
            "This edit was not applied: unknown or ambiguous contract. Nothing was changed."
        );
        assert_eq!(
            format!("{error:?}"),
            "EditRefused { reason: \"Unknown or ambiguous contract\", step: None }"
        );
        assert_eq!(held(&session, &id), before, "nothing changes");
    }
}
