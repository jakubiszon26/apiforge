//! Transactional deletion/restoration of registered working documents. Published
//! revisions are deliberately outside this service. Trash has no automatic expiry.
use super::{
    recovery::{self, BACKUP_PATH, RECOVERY_PATH},
    schema, *,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const TRASH: &str = ".apiforge/local/trash";
const DIAGRAM: &str = ".apiforge/diagram.json";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrashEntry {
    pub document: DocumentEntry,
    pub registry_index: usize,
    pub source_state: FileState,
    pub checkpoint: Option<DiagramRecord>,
    /// Dirty source/fields/layout are preserved separately from saved source.
    pub recovery: Option<RecoveryDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrashIndex {
    pub format_version: u32,
    pub operation_id: String,
    pub project_id: String,
    pub deleted_at_unix_ms: u64,
    pub restored_at_unix_ms: Option<u64>,
    pub entries: Vec<TrashEntry>,
}

impl TrashIndex {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ProjectError::Limit);
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Version {
            format_version: u32,
        }
        let version: Version = apiforge_core::strict_json::from_slice(bytes)
            .map_err(|_| ProjectError::InvalidTrash)?;
        if version.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        let index: Self = apiforge_core::strict_json::from_slice(bytes)
            .map_err(|_| ProjectError::InvalidTrash)?;
        index.validate()?;
        Ok(index)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = super::codec::encode(self, MAX_JSON_BYTES)?;
        Self::from_bytes(&bytes)?;
        Ok(bytes)
    }

    fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        operation_path(&self.operation_id, "index.json")?;
        if !schema::valid_id(&self.project_id) || self.entries.is_empty() {
            return Err(ProjectError::InvalidTrash);
        }
        if self.entries.len() > MAX_TRANSACTION_OPERATIONS {
            return Err(ProjectError::Limit);
        }
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();
        for entry in &self.entries {
            let document = &entry.document;
            ProjectPath::working_source(document.path.as_str())?;
            if !schema::valid_id(&document.document_id)
                || !schema::valid_id(&document.entity_id)
                || !ids.insert(&document.document_id)
                || !paths.insert(document.path.as_str().to_lowercase())
                || document.base_revision == Some(0)
                || (document.kind == DocumentKind::Scenario
                    && (document.recovered || document.base_revision.is_some()))
            {
                return Err(ProjectError::InvalidTrash);
            }
            match &entry.source_state {
                FileState::Missing => {}
                FileState::File { bytes, sha256 }
                    if *bytes <= MAX_SOURCE_BYTES as u64 && schema::valid_hash(sha256) => {}
                _ => return Err(ProjectError::InvalidTrash),
            }
            if let Some(checkpoint) = &entry.checkpoint {
                checkpoint.validate_metadata()?;
                if !checkpoint.matches_document(document) {
                    return Err(ProjectError::InvalidTrash);
                }
            }
            if let Some(recovery) = &entry.recovery {
                validate_recovery_document(recovery, document)?;
            }
        }
        SourceRecovery {
            format_version: 1,
            project_id: self.project_id.clone(),
            documents: self
                .entries
                .iter()
                .filter_map(|entry| entry.recovery.clone())
                .collect(),
        }
        .to_bytes()?;
        Ok(())
    }
}

/// The UI can show exactly which files/drafts will be deleted before committing.
/// The full preview is opaque so a later write cannot quietly adopt newer bytes.
#[derive(Clone)]
pub struct TrashPreview {
    root: PathBuf,
    manifest: ProjectManifest,
    snapshot: ProjectSnapshot,
    entries: Vec<DocumentEntry>,
}

impl TrashPreview {
    pub fn documents(&self) -> &[DocumentEntry] {
        &self.entries
    }
}

#[derive(Clone)]
pub struct TrashRestorePreview {
    root: PathBuf,
    manifest: ProjectManifest,
    snapshot: ProjectSnapshot,
    index: TrashIndex,
}

impl TrashRestorePreview {
    pub fn index(&self) -> &TrashIndex {
        &self.index
    }
}

#[derive(Clone, Debug)]
pub struct RestoreDestination {
    pub document_id: String,
    pub path: ProjectPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashCommit {
    pub operation_id: String,
    pub generation: u64,
}

impl ProjectStore {
    pub fn preview_trash(&self, document_ids: &[String]) -> Result<TrashPreview> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        let (manifest, snapshot) = self.load_locked(&recovery_paths()?)?;
        let ids: HashSet<_> = document_ids.iter().collect();
        if ids.is_empty() || ids.len() != document_ids.len() {
            return Err(ProjectError::InvalidTrash);
        }
        let entries: Vec<_> = manifest
            .documents
            .iter()
            .filter(|entry| ids.contains(&entry.document_id))
            .cloned()
            .collect();
        if entries.len() != ids.len() {
            return Err(ProjectError::InvalidTrash);
        }
        let diagram = diagram_from_snapshot(&snapshot)?;
        let recovery = recovery::recovery_from_snapshot(&snapshot, &manifest.id)?;
        for entry in &entries {
            working_checkpoint(&diagram, entry)?;
            if let Some(draft) = recovery
                .documents
                .iter()
                .find(|draft| draft.document_id == entry.document_id)
            {
                validate_recovery_document(draft, entry)?;
            }
        }
        Ok(TrashPreview {
            root: self.root().into(),
            manifest,
            snapshot,
            entries,
        })
    }

    /// One transaction captures saved bytes and current drafts before any original
    /// is removed. Supplied in-memory drafts supersede their persisted versions.
    pub fn trash_documents(
        &self,
        preview: &TrashPreview,
        dirty: &[RecoveryDocument],
    ) -> Result<TrashCommit> {
        self.trash_with_hook(preview, dirty, |_| Ok(()))
    }

    fn trash_with_hook(
        &self,
        preview: &TrashPreview,
        dirty: &[RecoveryDocument],
        boundary: impl FnMut(&str) -> Result<()>,
    ) -> Result<TrashCommit> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        self.check_preview(&preview.root, &preview.snapshot)?;
        let ids: HashSet<_> = preview
            .entries
            .iter()
            .map(|entry| entry.document_id.as_str())
            .collect();
        let mut live = BTreeMap::new();
        for document in dirty {
            if !ids.contains(document.document_id.as_str())
                || live
                    .insert(document.document_id.as_str(), document)
                    .is_some()
            {
                return Err(ProjectError::InvalidTrash);
            }
        }
        let mut diagram = diagram_from_snapshot(&preview.snapshot)?;
        let mut recovery =
            recovery::recovery_from_snapshot(&preview.snapshot, &preview.manifest.id)?;
        let operation_id = uuid::Uuid::new_v4().to_string();
        let mut index = TrashIndex {
            format_version: 1,
            operation_id: operation_id.clone(),
            project_id: preview.manifest.id.clone(),
            deleted_at_unix_ms: timestamp()?,
            restored_at_unix_ms: None,
            entries: Vec::new(),
        };
        let mut changes = Vec::new();
        if self.state(TRASH)? == FileState::Missing {
            changes.push(ProjectChange::CreateDirectory {
                path: ProjectPath::new(TRASH)?,
            });
        }
        changes.push(ProjectChange::CreateDirectory {
            path: ProjectPath::new(format!("{TRASH}/{operation_id}"))?,
        });
        for document in &preview.entries {
            let source = preview
                .snapshot
                .files
                .get(&document.path)
                .ok_or(ProjectError::InvalidTrash)?;
            let checkpoint = working_checkpoint(&diagram, document)?;
            let draft = live
                .get(document.document_id.as_str())
                .map(|document| (*document).clone())
                .or_else(|| {
                    recovery
                        .documents
                        .iter()
                        .find(|entry| entry.document_id == document.document_id)
                        .cloned()
                });
            if let Some(draft) = &draft {
                validate_recovery_document(draft, document)?;
            }
            let entry_index = index.entries.len();
            index.entries.push(TrashEntry {
                document: document.clone(),
                registry_index: preview
                    .manifest
                    .documents
                    .iter()
                    .position(|entry| entry.document_id == document.document_id)
                    .ok_or(ProjectError::InvalidTrash)?,
                source_state: source.as_deref().map_or(FileState::Missing, FileState::of),
                checkpoint,
                recovery: draft,
            });
            if let Some(bytes) = source {
                changes.push(write(
                    operation_path(&operation_id, &format!("{entry_index}.afs"))?,
                    FileState::Missing,
                    bytes.clone(),
                ));
            }
        }
        changes.push(write(
            operation_path(&operation_id, "index.json")?,
            FileState::Missing,
            index.to_bytes()?,
        ));
        let diagram_before = diagram.clone();
        diagram.records.retain(|record| !matches!(&record.key, DocumentKey::Working { document_id } if ids.contains(document_id.as_str())));
        if diagram != diagram_before {
            changes.push(write_from_snapshot(
                &preview.snapshot,
                DIAGRAM,
                diagram.to_bytes()?,
            )?);
        }
        recovery
            .documents
            .retain(|document| !ids.contains(document.document_id.as_str()));
        changes.extend(recovery::recovery_changes(&preview.snapshot, &recovery)?);
        for entry in &index.entries {
            if matches!(entry.source_state, FileState::File { .. }) {
                changes.push(ProjectChange::RemoveFile {
                    path: entry.document.path.clone(),
                    expected: entry.source_state.clone(),
                });
            }
        }
        let mut manifest = preview.manifest.clone();
        manifest
            .documents
            .retain(|entry| !ids.contains(entry.document_id.as_str()));
        changes.push(write_from_snapshot(
            &preview.snapshot,
            MANIFEST_PATH,
            manifest.to_public_bytes()?,
        )?);
        let generation = self.commit_locked(changes, boundary)?;
        Ok(TrashCommit {
            operation_id,
            generation,
        })
    }

    pub fn preview_trash_restore(&self, operation_id: &str) -> Result<TrashRestorePreview> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        let index_path = operation_path(operation_id, "index.json")?;
        let snapshot = self.stable_read(|| {
            // Discover payloads under the same stable-read protocol as the index.
            let snapshot = self.stable_read(|| Ok(vec![index_path.clone()]))?;
            let index = decode_index(&snapshot, operation_id)?;
            let mut paths = vec![
                ProjectPath::new(MANIFEST_PATH)?,
                ProjectPath::new(DIAGRAM)?,
                index_path.clone(),
            ];
            paths.extend(recovery_paths()?);
            for (index, entry) in index.entries.iter().enumerate() {
                if matches!(entry.source_state, FileState::File { .. }) {
                    paths.push(operation_path(operation_id, &format!("{index}.afs"))?);
                }
            }
            Ok(paths)
        })?;
        let index = decode_index(&snapshot, operation_id)?;
        let manifest = ProjectManifest::from_bytes(
            bytes(&snapshot, MANIFEST_PATH)?.ok_or(ProjectError::InvalidMetadata)?,
        )?;
        if index.project_id != manifest.id || index.restored_at_unix_ms.is_some() {
            return Err(ProjectError::InvalidTrash);
        }
        verify_payloads(&snapshot, &index)?;
        diagram_from_snapshot(&snapshot)?;
        recovery::recovery_from_snapshot(&snapshot, &manifest.id)?;
        Ok(TrashRestorePreview {
            root: self.root().into(),
            manifest,
            snapshot,
            index,
        })
    }

    /// Restore saved source exactly, preserving dirty work in local recovery.
    /// Occupied paths always require a different explicit destination.
    pub fn restore_trash(
        &self,
        preview: &TrashRestorePreview,
        destinations: &[RestoreDestination],
    ) -> Result<TrashCommit> {
        self.restore_with_hook(preview, destinations, |_| Ok(()))
    }

    fn restore_with_hook(
        &self,
        preview: &TrashRestorePreview,
        destinations: &[RestoreDestination],
        boundary: impl FnMut(&str) -> Result<()>,
    ) -> Result<TrashCommit> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        self.check_preview(&preview.root, &preview.snapshot)?;
        let mut overrides = BTreeMap::new();
        for destination in destinations {
            ProjectPath::working_source(destination.path.as_str())?;
            if !preview
                .index
                .entries
                .iter()
                .any(|entry| entry.document.document_id == destination.document_id)
                || overrides
                    .insert(&destination.document_id, &destination.path)
                    .is_some()
            {
                return Err(ProjectError::InvalidTrash);
            }
        }
        let mut manifest = preview.manifest.clone();
        let mut diagram = diagram_from_snapshot(&preview.snapshot)?;
        let diagram_before = diagram.clone();
        let mut recovery = recovery::recovery_from_snapshot(&preview.snapshot, &manifest.id)?;
        let mut changes = Vec::new();
        let mut directories = HashSet::new();
        let mut restored_entries = Vec::new();
        for (index, entry) in preview.index.entries.iter().enumerate() {
            let mut document = entry.document.clone();
            if let Some(path) = overrides.get(&document.document_id) {
                document.path = (*path).clone();
            }
            if self.state(document.path.as_str())? != FileState::Missing {
                return Err(ProjectError::Conflict {
                    path: self.root().join(document.path.as_str()),
                });
            }
            self.create_restore_parents(&document.path, &mut directories, &mut changes)?;
            if matches!(entry.source_state, FileState::File { .. }) {
                let payload = operation_path(&preview.index.operation_id, &format!("{index}.afs"))?;
                let source = preview
                    .snapshot
                    .files
                    .get(&payload)
                    .and_then(|source| source.as_ref())
                    .ok_or(ProjectError::InvalidTrash)?;
                changes.push(write(
                    document.path.clone(),
                    FileState::Missing,
                    source.clone(),
                ));
            }
            if let Some(checkpoint) = &entry.checkpoint {
                match diagram
                    .records
                    .iter()
                    .find(|record| record.key == checkpoint.key)
                {
                    Some(existing) if existing != checkpoint => {
                        return Err(ProjectError::Conflict {
                            path: self.root().join(DIAGRAM),
                        });
                    }
                    Some(_) => {}
                    None => diagram.records.push(checkpoint.clone()),
                }
            }
            if let Some(draft) = &entry.recovery {
                let mut draft = draft.clone();
                draft.path = document.path.clone();
                match recovery
                    .documents
                    .iter()
                    .find(|existing| existing.document_id == draft.document_id)
                {
                    Some(existing) if existing != &draft => {
                        return Err(ProjectError::Conflict {
                            path: self.root().join(RECOVERY_PATH),
                        });
                    }
                    Some(_) => {}
                    None => recovery.documents.push(draft),
                }
            }
            restored_entries.push((entry.registry_index, document));
        }
        restored_entries.sort_by_key(|(index, _)| *index);
        for (index, document) in restored_entries {
            manifest
                .documents
                .insert(index.min(manifest.documents.len()), document);
        }
        let manifest_bytes = manifest.to_public_bytes()?;
        if diagram != diagram_before {
            changes.push(write_from_snapshot(
                &preview.snapshot,
                DIAGRAM,
                diagram.to_bytes()?,
            )?);
        }
        changes.extend(recovery::recovery_changes(&preview.snapshot, &recovery)?);
        changes.push(write_from_snapshot(
            &preview.snapshot,
            MANIFEST_PATH,
            manifest_bytes,
        )?);
        let mut index = preview.index.clone();
        index.restored_at_unix_ms = Some(timestamp()?);
        changes.push(write_from_snapshot(
            &preview.snapshot,
            operation_path(&index.operation_id, "index.json")?.as_str(),
            index.to_bytes()?,
        )?);
        let generation = self.commit_locked(changes, boundary)?;
        Ok(TrashCommit {
            operation_id: index.operation_id,
            generation,
        })
    }

    fn check_preview(&self, root: &std::path::Path, expected: &ProjectSnapshot) -> Result<()> {
        if root != self.root() {
            return Err(ProjectError::InvalidTrash);
        }
        let observed = self.stable_read(|| Ok(expected.files.keys().cloned().collect()))?;
        if observed.generation != expected.generation {
            return Err(ProjectError::Busy);
        }
        for (path, expected_bytes) in &expected.files {
            if observed.files.get(path) != Some(expected_bytes) {
                return Err(ProjectError::Conflict {
                    path: self.root().join(path.as_str()),
                });
            }
        }
        Ok(())
    }

    fn create_restore_parents(
        &self,
        path: &ProjectPath,
        created: &mut HashSet<String>,
        changes: &mut Vec<ProjectChange>,
    ) -> Result<()> {
        let mut parent = String::new();
        let parts: Vec<_> = path.as_str().split('/').collect();
        for part in &parts[..parts.len() - 1] {
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(part);
            if created.contains(&parent) {
                continue;
            }
            match self.state(&parent)? {
                FileState::Directory => {}
                FileState::Missing => {
                    created.insert(parent.clone());
                    changes.push(ProjectChange::CreateDirectory {
                        path: ProjectPath::new(parent.clone())?,
                    });
                }
                _ => {
                    return Err(ProjectError::Conflict {
                        path: self.root().join(&parent),
                    });
                }
            }
        }
        Ok(())
    }
}

fn recovery_paths() -> Result<Vec<ProjectPath>> {
    Ok(vec![
        ProjectPath::new(RECOVERY_PATH)?,
        ProjectPath::new(BACKUP_PATH)?,
    ])
}
fn bytes<'a>(snapshot: &'a ProjectSnapshot, path: &str) -> Result<Option<&'a [u8]>> {
    Ok(snapshot
        .files
        .get(&ProjectPath::new(path)?)
        .ok_or(ProjectError::InvalidTrash)?
        .as_deref())
}
fn diagram_from_snapshot(snapshot: &ProjectSnapshot) -> Result<DiagramFile> {
    bytes(snapshot, DIAGRAM)?.map_or(
        Ok(DiagramFile {
            format_version: 1,
            records: Vec::new(),
        }),
        DiagramFile::from_bytes,
    )
}
fn working_checkpoint(
    diagram: &DiagramFile,
    document: &DocumentEntry,
) -> Result<Option<DiagramRecord>> {
    let key = DocumentKey::Working {
        document_id: document.document_id.clone(),
    };
    let record = diagram.records.iter().find(|record| record.key == key);
    if record.is_some_and(|record| !record.matches_document(document)) {
        return Err(ProjectError::InvalidCheckpoint);
    }
    Ok(record.cloned())
}
fn operation_path(operation_id: &str, name: &str) -> Result<ProjectPath> {
    if uuid::Uuid::parse_str(operation_id).is_err() || operation_id.len() != 36 {
        return Err(ProjectError::InvalidTrash);
    }
    ProjectPath::new(format!("{TRASH}/{operation_id}/{name}"))
}
fn decode_index(snapshot: &ProjectSnapshot, operation_id: &str) -> Result<TrashIndex> {
    let path = operation_path(operation_id, "index.json")?;
    let index =
        TrashIndex::from_bytes(bytes(snapshot, path.as_str())?.ok_or(ProjectError::InvalidTrash)?)?;
    if index.operation_id != operation_id {
        return Err(ProjectError::InvalidTrash);
    }
    Ok(index)
}
fn verify_payloads(snapshot: &ProjectSnapshot, index: &TrashIndex) -> Result<()> {
    for (number, entry) in index.entries.iter().enumerate() {
        if matches!(entry.source_state, FileState::File { .. }) {
            let path = operation_path(&index.operation_id, &format!("{number}.afs"))?;
            if snapshot.state(&path) != Some(entry.source_state.clone()) {
                return Err(ProjectError::InvalidTrash);
            }
        }
    }
    Ok(())
}
fn validate_recovery_document(recovery: &RecoveryDocument, entry: &DocumentEntry) -> Result<()> {
    if recovery.document_id != entry.document_id
        || recovery.entity_id != entry.entity_id
        || recovery.kind != entry.kind
        || recovery.base_revision != entry.base_revision
        || recovery.path != entry.path
    {
        return Err(ProjectError::InvalidTrash);
    }
    SourceRecovery {
        format_version: 1,
        project_id: "validation".into(),
        documents: vec![recovery.clone()],
    }
    .to_bytes()?;
    Ok(())
}
fn write(path: ProjectPath, expected: FileState, bytes: Vec<u8>) -> ProjectChange {
    ProjectChange::Write {
        path,
        expected,
        bytes,
    }
}
fn write_from_snapshot(
    snapshot: &ProjectSnapshot,
    path: &str,
    bytes: Vec<u8>,
) -> Result<ProjectChange> {
    let path = ProjectPath::new(path)?;
    Ok(write(
        path.clone(),
        snapshot.state(&path).ok_or(ProjectError::InvalidTrash)?,
        bytes,
    ))
}
fn timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProjectError::InvalidTrash)?
        .as_millis()
        .try_into()
        .map_err(|_| ProjectError::Limit)
}

#[cfg(test)]
#[path = "trash_tests.rs"]
mod tests;
