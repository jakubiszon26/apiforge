//! Changes made outside the session (ADR 0012). A worker scans the folder and
//! reads changed files; the session then applies the result without I/O for
//! sources. Clean documents follow the disk, documents with unsaved work get a
//! conflict and are never merged or overwritten, and an external rename is
//! followed only when a removed and an added source have a unique hash match.
use super::{EventKind, Origin, ProjectSession};
use crate::{Error, Result, document::AnalysisState, document::Unavailable};
use apiforge_storage::project::{
    FileState, MANIFEST_PATH, MAX_SOURCE_BYTES, ProjectChange, ProjectError, ProjectManifest,
    ProjectPath, ProjectStore, TreeScan,
};
use std::collections::HashMap;

/// Why a document's unsaved work cannot follow the disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalConflict {
    /// The saved file changed outside the session.
    Changed,
    /// The saved file was deleted outside the session.
    Deleted,
}

/// The registry as last observed on disk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RegistryState {
    #[default]
    Current,
    /// Valid, but changed outside the session while it had unsaved work or
    /// configuration. Writes wait for an explicit project reload.
    Changed,
    /// Unreadable or invalid. Writes and execution wait until it is repaired.
    Invalid,
    Missing,
}

/// The file content a check observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservedFile {
    Missing,
    Bytes(Vec<u8>),
    /// A source over the 1 MiB limit, by its state only.
    TooLarge(FileState),
    /// A registry that cannot be read within its limit.
    Unreadable,
}

/// Immutable input for a worker. Holds no session reference.
#[derive(Clone)]
pub struct ExternalCheck {
    store: ProjectStore,
    epoch: u64,
    manifest: FileState,
    documents: Vec<(String, ProjectPath, FileState)>,
    /// Open text files, by path: their identity, unlike a document's.
    texts: Vec<(ProjectPath, FileState)>,
}

/// What a check found. Applying it to a session of a different epoch, or to
/// documents saved since the check began, ignores the stale parts.
#[derive(Clone, Debug)]
pub struct ExternalChanges {
    epoch: u64,
    manifest_base: FileState,
    pub scan: TreeScan,
    manifest: Option<ObservedFile>,
    documents: Vec<(String, FileState, ObservedFile)>,
    texts: Vec<(String, FileState, Option<Vec<u8>>)>,
}

/// What applying a check changed, for notification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExternalOutcome {
    pub reloaded: Vec<String>,
    pub conflicts: Vec<String>,
    pub renamed: Vec<String>,
    pub missing: Vec<String>,
    /// Paths of open text files whose unsaved work cannot follow the disk.
    pub text_conflicts: Vec<String>,
    pub registry: Option<RegistryState>,
    /// The project was reloaded because its registry changed.
    pub project_reloaded: bool,
}

impl ExternalOutcome {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl ExternalCheck {
    /// Scans the folder and reads every registered file whose state differs
    /// from the session's saved state, plus the registry when it changed.
    pub fn run(&self) -> Result<ExternalChanges> {
        let scan = self.store.scan_tree()?;
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        // A bounded scan can leave out existing files: absence from it is
        // confirmed directly before anything counts as removed.
        let state_of = |path: &ProjectPath| -> Result<Option<FileState>> {
            match scan.entry(path.as_str()) {
                Some(entry) => Ok(Some(entry.state().unwrap_or(FileState::Directory))),
                None => match self.store.file_state(path) {
                    Ok(state) => Ok(Some(state)),
                    // Over the payload limit: the next scan records its hash.
                    Err(ProjectError::Limit) => Ok(None),
                    Err(error) => Err(error.into()),
                },
            }
        };
        let manifest = match state_of(&manifest_path)? {
            Some(state) if state == self.manifest => None,
            Some(FileState::Missing) => Some(ObservedFile::Missing),
            _ => Some(
                match self.store.read(std::slice::from_ref(&manifest_path)) {
                    Ok(mut snapshot) => snapshot
                        .files
                        .remove(&manifest_path)
                        .flatten()
                        .map_or(ObservedFile::Missing, ObservedFile::Bytes),
                    Err(ProjectError::Limit) => ObservedFile::Unreadable,
                    Err(error) => return Err(error.into()),
                },
            ),
        };
        let mut documents = Vec::new();
        for (id, path, saved) in &self.documents {
            let Some(current) = state_of(path)? else {
                continue;
            };
            if &current == saved {
                continue;
            }
            let observed = match &current {
                FileState::File { bytes, .. } if *bytes > MAX_SOURCE_BYTES as u64 => {
                    ObservedFile::TooLarge(current.clone())
                }
                FileState::File { .. } => match self.store.read(std::slice::from_ref(path)) {
                    Ok(mut snapshot) => snapshot
                        .files
                        .remove(path)
                        .flatten()
                        .map_or(ObservedFile::Missing, ObservedFile::Bytes),
                    // Grew past the limit after the scan; the next check
                    // records its state.
                    Err(ProjectError::Limit) => continue,
                    Err(error) => return Err(error.into()),
                },
                // A directory or link where the source was: treat as removed.
                _ => ObservedFile::Missing,
            };
            documents.push((id.clone(), saved.clone(), observed));
        }
        let mut texts = Vec::new();
        // The tree scan hashes only sources and the registry, so an open text
        // file's state is read directly rather than taken from the scan.
        for (path, saved) in &self.texts {
            let current = match self.store.file_state(path) {
                Ok(state) => state,
                // Over the payload limit: the next check reports it again.
                Err(ProjectError::Limit) => continue,
                Err(error) => return Err(error.into()),
            };
            if &current == saved {
                continue;
            }
            let bytes = match &current {
                FileState::File { bytes, .. } if *bytes > MAX_SOURCE_BYTES as u64 => continue,
                FileState::File { .. } => match self.store.read(std::slice::from_ref(path)) {
                    Ok(mut snapshot) => snapshot.files.remove(path).flatten(),
                    Err(ProjectError::Limit) => continue,
                    Err(error) => return Err(error.into()),
                },
                // A directory or link where the file was: treat as removed.
                _ => None,
            };
            texts.push((path.as_str().to_owned(), saved.clone(), bytes));
        }
        Ok(ExternalChanges {
            epoch: self.epoch,
            manifest_base: self.manifest.clone(),
            scan,
            manifest,
            documents,
            texts,
        })
    }
}

impl ProjectSession {
    /// Input for a background check of external changes.
    pub fn external_check(&self) -> ExternalCheck {
        ExternalCheck {
            store: self.store.clone(),
            epoch: self.epoch,
            manifest: self.manifest_saved.clone(),
            documents: self
                .documents
                .iter()
                .map(|document| {
                    (
                        document.entry.document_id.clone(),
                        document.entry.path.clone(),
                        document.saved.clone(),
                    )
                })
                .collect(),
            texts: self.text_baselines(),
        }
    }

    pub fn registry_state(&self) -> RegistryState {
        self.registry
    }

    /// Applies a check. A changed valid registry reloads the project when
    /// nothing is unsaved; otherwise writes wait for `reload_project`.
    pub fn apply_external(&mut self, changes: ExternalChanges) -> Result<ExternalOutcome> {
        let mut outcome = ExternalOutcome::default();
        if changes.epoch != self.epoch || changes.manifest_base != self.manifest_saved {
            return Ok(outcome);
        }
        if let Some(observed) = &changes.manifest {
            let state = match observed {
                ObservedFile::Missing => RegistryState::Missing,
                ObservedFile::Unreadable | ObservedFile::TooLarge(_) => RegistryState::Invalid,
                ObservedFile::Bytes(bytes) => match ProjectManifest::from_bytes(bytes) {
                    Ok(manifest) if manifest.id == self.manifest.id => RegistryState::Changed,
                    _ => RegistryState::Invalid,
                },
            };
            if state == RegistryState::Changed && !self.has_unsaved_work() {
                self.reload()?;
                outcome.project_reloaded = true;
                outcome.registry = Some(RegistryState::Current);
                self.registry = RegistryState::Current;
                return Ok(outcome);
            }
            if state != self.registry {
                self.registry = state;
                outcome.registry = Some(state);
                self.emit(Origin::SESSION, None, EventKind::RegistryChanged);
            }
            return Ok(outcome);
        }
        if self.registry != RegistryState::Current {
            // The registry was restored to the saved bytes.
            self.registry = RegistryState::Current;
            outcome.registry = Some(RegistryState::Current);
            self.emit(Origin::SESSION, None, EventKind::RegistryChanged);
        }
        let registered: Vec<_> = self
            .documents
            .iter()
            .map(|document| document.entry.path.as_str().to_lowercase())
            .collect();
        // Unique hash matches between removed sources and unregistered ones.
        let mut added: HashMap<String, Vec<String>> = HashMap::new();
        for entry in &changes.scan.entries {
            if let Some(FileState::File { sha256, .. }) = entry.state()
                && entry.portable
                && entry.path.ends_with(".afs")
                && !registered.contains(&entry.path.to_lowercase())
            {
                added.entry(sha256).or_default().push(entry.path.clone());
            }
        }
        let mut removed: HashMap<String, usize> = HashMap::new();
        for (_, base, observed) in &changes.documents {
            if let (FileState::File { sha256, .. }, ObservedFile::Missing) = (base, observed) {
                *removed.entry(sha256.clone()).or_default() += 1;
            }
        }
        let mut renames = Vec::new();
        for (id, base, observed) in changes.documents {
            let Ok(index) = self.index(&id) else { continue };
            if self.documents[index].saved != base {
                // Saved by this session after the check began.
                continue;
            }
            if let (FileState::File { sha256, .. }, ObservedFile::Missing) = (&base, &observed)
                && removed.get(sha256) == Some(&1)
                && let Some([path]) = added.get(sha256).map(Vec::as_slice)
                && let Ok(path) = ProjectPath::working_source(path.as_str())
            {
                renames.push((id, path));
                continue;
            }
            let dirty = self.documents[index].needs_recovery();
            match (observed, dirty) {
                (ObservedFile::Missing, true) => {
                    self.conflict(index, ExternalConflict::Deleted);
                    outcome.conflicts.push(id);
                }
                (_, true) => {
                    self.conflict(index, ExternalConflict::Changed);
                    outcome.conflicts.push(id);
                }
                (ObservedFile::Missing, false) => {
                    self.follow_disk(index, ObservedFile::Missing, Origin::SESSION)?;
                    outcome.missing.push(id);
                }
                (observed, false) => {
                    self.follow_disk(index, observed, Origin::SESSION)?;
                    outcome.reloaded.push(id);
                }
            }
        }
        if !renames.is_empty() {
            outcome.renamed = self.follow_renames(renames)?;
        }
        outcome.text_conflicts = self.apply_external_texts(changes.texts);
        Ok(outcome)
    }

    /// Replaces unsaved work with the file on disk, resolving a conflict.
    pub fn accept_external(&mut self, document_id: &str, origin: Origin) -> Result<()> {
        let index = self.index(document_id)?;
        self.require_no_held_draft(document_id)?;
        let path = self.documents[index].entry.path.clone();
        let observed = match self.store.read(std::slice::from_ref(&path)) {
            Ok(mut snapshot) => snapshot
                .files
                .remove(&path)
                .flatten()
                .map_or(ObservedFile::Missing, ObservedFile::Bytes),
            Err(ProjectError::Limit) => {
                ObservedFile::TooLarge(self.store.file_state(&path).unwrap_or(FileState::Missing))
            }
            Err(error) => return Err(error.into()),
        };
        let document = &mut self.documents[index];
        document.layout_dirty = false;
        document.fields.clear();
        document.history = Default::default();
        self.follow_disk(index, observed, origin)
    }

    /// Keeps unsaved work over a deleted file: the next save recreates it.
    /// A changed file is never overwritten this way.
    pub fn keep_deleted(&mut self, document_id: &str) -> Result<()> {
        let index = self.index(document_id)?;
        if self.documents[index].conflict != Some(ExternalConflict::Deleted) {
            return Err(Error::ExternalConflict);
        }
        let document = &mut self.documents[index];
        document.saved = FileState::Missing;
        document.conflict = None;
        let id = document.entry.document_id.clone();
        self.emit(Origin::SESSION, Some(&id), EventKind::ExternalConflict);
        Ok(())
    }

    /// Rereads the whole project after an external registry change, keeping
    /// unsaved work through recovery. Unsaved configuration must be saved or
    /// discarded first.
    pub fn reload_project(&mut self) -> Result<()> {
        if self.manifest_dirty {
            return Err(Error::UnsavedConfiguration);
        }
        // Recovery is bound to the old registry, so carry the work across in
        // memory and protect it against the new registry afterwards.
        let drafts = self.recovery_contents().documents;
        self.reload()?;
        self.registry = RegistryState::Current;
        for draft in drafts {
            self.recovery
                .held
                .retain(|held| held.document_id != draft.document_id);
            self.adopt_draft(draft)?;
        }
        if self.has_unsaved_work() && !self.store.is_read_only() {
            let _ = self.save_recovery();
        }
        Ok(())
    }

    /// Discards unsaved configuration, for example to follow a changed registry.
    pub fn discard_configuration(&mut self) -> Result<()> {
        if !self.manifest_dirty {
            return Ok(());
        }
        let path = ProjectPath::new(MANIFEST_PATH)?;
        let snapshot = self.store.read(std::slice::from_ref(&path))?;
        let saved = ProjectManifest::from_bytes(
            snapshot.files[&path]
                .as_deref()
                .ok_or(ProjectError::InvalidMetadata)?,
        )?;
        let changed = snapshot.state(&path) != Some(self.manifest_saved.clone());
        // This explicit discard precedes reload of a changed registry. It does
        // not author a configuration against the old save baseline or approve
        // changed dependencies; the registry guard still blocks writes/runs
        // until reload validates the complete saved project.
        let mut manifest = self.manifest.clone();
        manifest.name = saved.name;
        manifest.environment = saved.environment;
        manifest.contracts = saved.contracts;
        manifest.validate()?;
        let mut project = crate::context::base_project(&manifest);
        project.components = self.published.clone();
        self.manifest = manifest;
        self.context = std::sync::Arc::new(project);
        self.manifest_dirty = false;
        if changed {
            self.registry = RegistryState::Changed;
        }
        self.invalidate_runtime_inputs(Origin::SESSION);
        Ok(())
    }

    pub(super) fn require_current_registry(&self) -> Result<()> {
        match self.registry {
            RegistryState::Current => Ok(()),
            _ => Err(Error::RegistryChanged),
        }
    }

    fn has_unsaved_work(&self) -> bool {
        self.manifest_dirty
            || !self.recovery.held.is_empty()
            || self
                .documents
                .iter()
                .any(|document| document.needs_recovery())
            || self.has_unsaved_text()
    }

    fn conflict(&mut self, index: usize, conflict: ExternalConflict) {
        if self.documents[index].conflict == Some(conflict) {
            return;
        }
        self.documents[index].conflict = Some(conflict);
        let id = self.documents[index].entry.document_id.clone();
        self.emit(Origin::SESSION, Some(&id), EventKind::ExternalConflict);
    }

    /// Adopts the observed file as the saved and current source.
    fn follow_disk(&mut self, index: usize, observed: ObservedFile, origin: Origin) -> Result<()> {
        let document = &mut self.documents[index];
        document.conflict = None;
        document.diagnostics.clear();
        match observed {
            ObservedFile::Bytes(bytes) => {
                document.saved = FileState::of(&bytes);
                match String::from_utf8(bytes) {
                    Ok(text) => {
                        document.unavailable = None;
                        document.text = text;
                        document.analysis = AnalysisState::Pending;
                    }
                    Err(_) => {
                        document.unavailable = Some(Unavailable::NotUtf8);
                        document.text.clear();
                        document.analysis = AnalysisState::Invalid;
                    }
                }
            }
            ObservedFile::TooLarge(state) => {
                document.saved = state;
                document.unavailable = Some(Unavailable::TooLarge);
                document.text.clear();
                document.analysis = AnalysisState::Invalid;
            }
            ObservedFile::Missing | ObservedFile::Unreadable => {
                document.saved = FileState::Missing;
                document.unavailable = Some(Unavailable::Missing);
                document.text.clear();
                document.analysis = AnalysisState::Invalid;
            }
        }
        document.source_version += 1;
        let version = document.source_version;
        let id = document.entry.document_id.clone();
        self.emit(
            origin,
            Some(&id),
            EventKind::SourceChanged {
                source_version: version,
            },
        );
        self.analyse_now(index)
    }

    /// Records externally renamed sources under their new paths, keeping IDs.
    fn follow_renames(&mut self, renames: Vec<(String, ProjectPath)>) -> Result<Vec<String>> {
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        let snapshot = self.store.read(std::slice::from_ref(&manifest_path))?;
        let mut registry = self.saved_manifest(&snapshot.files[&manifest_path])?;
        for (id, path) in &renames {
            if let Some(entry) = registry
                .documents
                .iter_mut()
                .find(|entry| &entry.document_id == id)
            {
                entry.path = path.clone();
            }
        }
        let bytes = registry.to_public_bytes()?;
        if self.store.is_read_only() {
            // A reader cannot record the rename; it follows the writer's
            // registry once that changes.
            return Ok(Vec::new());
        }
        self.store.commit(vec![ProjectChange::Write {
            path: manifest_path,
            expected: self.manifest_saved.clone(),
            bytes: bytes.clone(),
        }])?;
        self.manifest_saved = FileState::of(&bytes);
        self.manifest.documents = registry.documents;
        self.refresh_local_baselines();
        let mut ids = Vec::new();
        for (id, path) in renames {
            let index = self.index(&id)?;
            self.documents[index].entry.path = path.clone();
            for draft in &mut self.recovery.held {
                if draft.document_id == id {
                    draft.path = path.clone();
                }
            }
            self.emit(Origin::SESSION, Some(&id), EventKind::DocumentMoved);
            ids.push(id);
        }
        if self.has_unsaved_work() {
            let _ = self.save_recovery();
        }
        Ok(ids)
    }
}
