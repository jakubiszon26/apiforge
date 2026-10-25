//! Exact source recovery is durable user work, not an executable model cache.
//! Reading returns a preview and an opaque baseline; it never applies a draft or
//! authorises replacing a changed source. All writes use the project writer lock.
use super::{schema, *};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

pub(super) const RECOVERY_PATH: &str = ".apiforge/local/recovery.json";
pub(super) const BACKUP_PATH: &str = ".apiforge/local/recovery.json.bak";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextSelection {
    /// UTF-8 byte offsets, both at character boundaries. Direction is retained.
    pub anchor: usize,
    pub caret: usize,
}

impl TextSelection {
    fn validate(&self, text: &str) -> Result<()> {
        if !text.is_char_boundary(self.anchor) || !text.is_char_boundary(self.caret) {
            return Err(ProjectError::InvalidRecovery);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldRecovery {
    /// Stable inspector widget identity, scoped to its owning document.
    pub widget: u64,
    pub base_hash: String,
    pub text: String,
    pub selection: TextSelection,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryDocument {
    pub document_id: String,
    pub entity_id: String,
    pub kind: DocumentKind,
    pub base_revision: Option<u32>,
    /// Last known registry path; document identity does not depend on this path.
    pub path: ProjectPath,
    /// Hash of exact disk source at the editing baseline; null means absent.
    pub base_hash: Option<String>,
    pub source_version: u64,
    pub text: String,
    pub selection: TextSelection,
    pub fields: Vec<FieldRecovery>,
    /// Last analysed identity/layout, possibly for an earlier valid source when
    /// current text is invalid. A session must verify provenance before reuse.
    pub checkpoint: Option<DiagramRecord>,
}

impl RecoveryDocument {
    /// A matching baseline alone does not authorise applying a restored draft.
    /// Sessions still check document/entity context and analyse its source.
    pub fn matches_base(&self, state: &FileState) -> bool {
        match (&self.base_hash, state) {
            (None, FileState::Missing) => true,
            (Some(expected), FileState::File { sha256, .. }) => expected == sha256,
            _ => false,
        }
    }

    fn validate(&self) -> Result<()> {
        ProjectPath::working_source(self.path.as_str())
            .map_err(|_| ProjectError::InvalidRecovery)?;
        if !schema::valid_id(&self.document_id)
            || !schema::valid_id(&self.entity_id)
            || self
                .base_hash
                .as_ref()
                .is_some_and(|hash| !schema::valid_hash(hash))
            || self.base_revision == Some(0)
            || (self.kind == DocumentKind::Scenario && self.base_revision.is_some())
        {
            return Err(ProjectError::InvalidRecovery);
        }
        validate_buffer(&self.text, &self.selection)?;
        let mut widgets = HashSet::new();
        for field in &self.fields {
            if !widgets.insert(field.widget) || !schema::valid_hash(&field.base_hash) {
                return Err(ProjectError::InvalidRecovery);
            }
            validate_buffer(&field.text, &field.selection)?;
        }
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint.validate_metadata()?;
            if checkpoint.key
                != (DocumentKey::Working {
                    document_id: self.document_id.clone(),
                })
                || checkpoint.identity_base.entity_id() != self.entity_id
                || checkpoint.identity_base.kind() != self.kind
            {
                return Err(ProjectError::InvalidRecovery);
            }
        }
        Ok(())
    }
}

/// An open project text file, such as `.gitignore`: exact unsaved text and the
/// disk baseline it was edited from. A text file is not a definition, so it has
/// no entity, kind, revision or checkpoint; its identity is its path.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryText {
    pub path: ProjectPath,
    /// Hash of exact disk bytes at the editing baseline; null means absent.
    pub base_hash: Option<String>,
    pub text: String,
    pub selection: TextSelection,
}

impl RecoveryText {
    /// As for a document, a matching baseline alone does not authorise applying
    /// the draft; the session still decides.
    pub fn matches_base(&self, state: &FileState) -> bool {
        match (&self.base_hash, state) {
            (None, FileState::Missing) => true,
            (Some(expected), FileState::File { sha256, .. }) => expected == sha256,
            _ => false,
        }
    }

    fn validate(&self) -> Result<()> {
        // The path type already refuses .git and managed .apiforge data; a text
        // file may not address the project's own control files either.
        if self.path.as_str().starts_with(".apiforge/")
            || self
                .base_hash
                .as_ref()
                .is_some_and(|hash| !schema::valid_hash(hash))
        {
            return Err(ProjectError::InvalidRecovery);
        }
        validate_buffer(&self.text, &self.selection)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRecovery {
    pub format_version: u32,
    pub project_id: String,
    pub documents: Vec<RecoveryDocument>,
    /// Open text files with unsaved work. Absent in a recovery file written
    /// before project text files could be edited.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<RecoveryText>,
}

impl SourceRecovery {
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
            .map_err(|_| ProjectError::InvalidRecovery)?;
        if version.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        let recovery: Self = apiforge_core::strict_json::from_slice(bytes)
            .map_err(|_| ProjectError::InvalidRecovery)?;
        recovery.validate()?;
        Ok(recovery)
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
        if !schema::valid_id(&self.project_id) {
            return Err(ProjectError::InvalidRecovery);
        }
        let mut documents = HashSet::new();
        let mut total_text_bytes = 0usize;
        for document in &self.documents {
            if !documents.insert(&document.document_id) {
                return Err(ProjectError::InvalidRecovery);
            }
            document.validate()?;
            total_text_bytes = total_text_bytes
                .checked_add(document.text.len())
                .ok_or(ProjectError::Limit)?;
            for field in &document.fields {
                total_text_bytes = total_text_bytes
                    .checked_add(field.text.len())
                    .ok_or(ProjectError::Limit)?;
            }
            if total_text_bytes > MAX_JSON_BYTES {
                return Err(ProjectError::Limit);
            }
        }
        let mut texts = HashSet::new();
        for text in &self.texts {
            if !texts.insert(&text.path) {
                return Err(ProjectError::InvalidRecovery);
            }
            text.validate()?;
            total_text_bytes = total_text_bytes
                .checked_add(text.text.len())
                .ok_or(ProjectError::Limit)?;
            if total_text_bytes > MAX_JSON_BYTES {
                return Err(ProjectError::Limit);
            }
        }
        Ok(())
    }
}

fn validate_buffer(text: &str, selection: &TextSelection) -> Result<()> {
    if text.len() > MAX_SOURCE_BYTES {
        return Err(ProjectError::Limit);
    }
    selection.validate(text)
}

#[derive(Clone)]
struct RecoveryBaseline {
    root: PathBuf,
    project_id: String,
    manifest: FileState,
    primary: FileState,
    backup: FileState,
}

/// The preview is immutable to callers. In particular a backup candidate cannot
/// become an ordinary save baseline because a view repainted or was reopened.
#[derive(Clone)]
pub struct RecoveryRead {
    baseline: RecoveryBaseline,
    recovery: Option<SourceRecovery>,
    from_backup: bool,
}

impl RecoveryRead {
    pub fn recovery(&self) -> Option<&SourceRecovery> {
        self.recovery.as_ref()
    }
    pub fn is_backup_candidate(&self) -> bool {
        self.from_backup
    }
}

impl ProjectStore {
    /// Read-only sessions may preview recovery. Neither previewing nor a matching
    /// base hash makes any source/model change or writes a file.
    pub fn load_source_recovery(&self, project_id: &str) -> Result<RecoveryRead> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        self.read_recovery(project_id)
    }

    pub fn save_source_recovery(
        &self,
        preview: &RecoveryRead,
        recovery: &SourceRecovery,
    ) -> Result<RecoveryRead> {
        self.save_recovery_with_hook(preview, recovery, |_| Ok(()))
    }

    /// Commits a file operation and the corresponding draft paths together.
    /// The supplied recovery observation must still match every durable baseline.
    pub fn commit_with_source_recovery(
        &self,
        mut changes: Vec<ProjectChange>,
        preview: &RecoveryRead,
        desired: &SourceRecovery,
    ) -> Result<RecoveryRead> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        self.validate_public_changes(&changes)?;
        if preview.from_backup {
            return Err(ProjectError::RecoveryDecisionRequired);
        }
        if desired.project_id != preview.baseline.project_id {
            return Err(ProjectError::ForeignRecovery);
        }
        let snapshot = self.checked_recovery_snapshot(&preview.baseline)?;
        changes.extend(recovery_changes(&snapshot, desired)?);
        self.commit_locked(changes, |_| Ok(()))?;
        self.read_recovery(&desired.project_id)
    }

    fn save_recovery_with_hook(
        &self,
        preview: &RecoveryRead,
        recovery: &SourceRecovery,
        mut boundary: impl FnMut(&str) -> Result<()>,
    ) -> Result<RecoveryRead> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        if preview.from_backup {
            return Err(ProjectError::RecoveryDecisionRequired);
        }
        if recovery.project_id != preview.baseline.project_id {
            return Err(ProjectError::ForeignRecovery);
        }
        let bytes = recovery.to_bytes()?;
        self.check_secrets(&bytes)?;
        let current = self.checked_recovery_snapshot(&preview.baseline)?;
        let primary = current.files[&ProjectPath::new(RECOVERY_PATH)?].as_deref();
        let backup = current.files[&ProjectPath::new(BACKUP_PATH)?].as_deref();
        // Preserve future or foreign backup data. A corrupt backup can be
        // replaced by a checked valid primary without losing the current draft.
        if let Some(backup) = backup {
            match decode_for_project(backup, &recovery.project_id) {
                Err(ProjectError::InvalidRecovery) => {}
                result => {
                    result?;
                }
            }
        }
        if let Some(primary) = primary {
            decode_for_project(primary, &recovery.project_id)?;
            self.check_secrets(primary)?;
            self.atomic(&super::path::resolve(self.root(), BACKUP_PATH)?, primary)?;
            boundary("backup")?;
        }
        // A hook or non-cooperating editor may have changed the main file during
        // the backup write. Never silently adopt that third state as a baseline.
        boundary("before-primary")?;
        let expected_backup = if primary.is_some() {
            &preview.baseline.primary
        } else {
            &preview.baseline.backup
        };
        self.check_recovery_primary(&preview.baseline, expected_backup)?;
        self.atomic(&super::path::resolve(self.root(), RECOVERY_PATH)?, &bytes)?;
        boundary("primary")?;
        self.ack_recovery_write(&preview.baseline, &bytes, expected_backup)
    }

    /// Explicitly restore exactly the backup previously previewed by the caller.
    /// The verified backup remains intact instead of rotating corrupt primary
    /// bytes over it. Changed primary/backup/registry requires a new preview.
    pub fn restore_source_recovery_backup(&self, preview: &RecoveryRead) -> Result<RecoveryRead> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        if !preview.from_backup {
            return Err(ProjectError::NoRecoveryCandidate);
        }
        let current = self.checked_recovery_snapshot(&preview.baseline)?;
        let bytes = current.files[&ProjectPath::new(BACKUP_PATH)?]
            .as_deref()
            .ok_or(ProjectError::NoRecoveryCandidate)?;
        decode_for_project(bytes, &preview.baseline.project_id)?;
        self.check_secrets(bytes)?;
        self.atomic(&super::path::resolve(self.root(), RECOVERY_PATH)?, bytes)?;
        self.ack_recovery_write(&preview.baseline, bytes, &preview.baseline.backup)
    }

    fn ack_recovery_write(
        &self,
        baseline: &RecoveryBaseline,
        bytes: &[u8],
        backup: &FileState,
    ) -> Result<RecoveryRead> {
        let observed = self.read_recovery(&baseline.project_id)?;
        for (path, actual, expected) in [
            (
                MANIFEST_PATH,
                &observed.baseline.manifest,
                &baseline.manifest,
            ),
            (
                RECOVERY_PATH,
                &observed.baseline.primary,
                &FileState::of(bytes),
            ),
            (BACKUP_PATH, &observed.baseline.backup, backup),
        ] {
            if actual != expected {
                return Err(ProjectError::Conflict {
                    path: self.root().join(path),
                });
            }
        }
        Ok(observed)
    }

    fn recovery_snapshot(&self) -> Result<ProjectSnapshot> {
        self.stable_read(|| {
            Ok(vec![
                ProjectPath::new(MANIFEST_PATH)?,
                ProjectPath::new(RECOVERY_PATH)?,
                ProjectPath::new(BACKUP_PATH)?,
            ])
        })
    }

    fn read_recovery(&self, project_id: &str) -> Result<RecoveryRead> {
        let snapshot = self.recovery_snapshot()?;
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        let manifest = ProjectManifest::from_bytes(
            snapshot.files[&manifest_path]
                .as_deref()
                .ok_or(ProjectError::InvalidMetadata)?,
        )?;
        if manifest.id != project_id {
            return Err(ProjectError::ForeignRecovery);
        }
        let primary_path = ProjectPath::new(RECOVERY_PATH)?;
        let backup_path = ProjectPath::new(BACKUP_PATH)?;
        let primary = snapshot.files[&primary_path].as_deref();
        let backup = snapshot.files[&backup_path].as_deref();
        let (recovery, from_backup) =
            match primary.map(|bytes| decode_for_project(bytes, project_id)) {
                Some(Ok(recovery)) => (Some(recovery), false),
                Some(Err(ProjectError::InvalidRecovery)) | None => {
                    match backup.map(|bytes| decode_for_project(bytes, project_id)) {
                        Some(Ok(recovery)) => (Some(recovery), true),
                        Some(Err(error)) => return Err(error),
                        None if primary.is_some() => return Err(ProjectError::InvalidRecovery),
                        None => (None, false),
                    }
                }
                Some(Err(error)) => return Err(error),
            };
        Ok(RecoveryRead {
            baseline: RecoveryBaseline {
                root: self.root().to_owned(),
                project_id: project_id.to_owned(),
                manifest: snapshot
                    .state(&manifest_path)
                    .ok_or(ProjectError::InvalidMetadata)?,
                primary: snapshot
                    .state(&primary_path)
                    .ok_or(ProjectError::InvalidRecovery)?,
                backup: snapshot
                    .state(&backup_path)
                    .ok_or(ProjectError::InvalidRecovery)?,
            },
            recovery,
            from_backup,
        })
    }

    fn checked_recovery_snapshot(&self, baseline: &RecoveryBaseline) -> Result<ProjectSnapshot> {
        if baseline.root != self.root() {
            return Err(ProjectError::ForeignRecovery);
        }
        let snapshot = self.recovery_snapshot()?;
        for (path, expected) in [
            (MANIFEST_PATH, &baseline.manifest),
            (RECOVERY_PATH, &baseline.primary),
            (BACKUP_PATH, &baseline.backup),
        ] {
            if snapshot.state(&ProjectPath::new(path)?).as_ref() != Some(expected) {
                return Err(ProjectError::Conflict {
                    path: self.root().join(path),
                });
            }
        }
        Ok(snapshot)
    }

    fn check_recovery_primary(
        &self,
        baseline: &RecoveryBaseline,
        expected_backup: &FileState,
    ) -> Result<()> {
        let snapshot = self.recovery_snapshot()?;
        for (path, expected) in [
            (MANIFEST_PATH, &baseline.manifest),
            (RECOVERY_PATH, &baseline.primary),
            (BACKUP_PATH, expected_backup),
        ] {
            if snapshot.state(&ProjectPath::new(path)?).as_ref() != Some(expected) {
                return Err(ProjectError::Conflict {
                    path: self.root().join(path),
                });
            }
        }
        Ok(())
    }
}

pub(super) fn decode_for_project(bytes: &[u8], project_id: &str) -> Result<SourceRecovery> {
    let recovery = SourceRecovery::from_bytes(bytes)?;
    if recovery.project_id != project_id {
        return Err(ProjectError::ForeignRecovery);
    }
    Ok(recovery)
}

/// File operations must preserve the same recovery/backup rules as ordinary
/// autosave, but include their post-images in the shared operation transaction.
pub(super) fn recovery_from_snapshot(
    snapshot: &ProjectSnapshot,
    project_id: &str,
) -> Result<SourceRecovery> {
    let primary = snapshot
        .files
        .get(&ProjectPath::new(RECOVERY_PATH)?)
        .ok_or(ProjectError::InvalidRecovery)?
        .as_deref();
    let backup = snapshot
        .files
        .get(&ProjectPath::new(BACKUP_PATH)?)
        .ok_or(ProjectError::InvalidRecovery)?
        .as_deref();
    if let Some(backup) = backup {
        match decode_for_project(backup, project_id) {
            Err(ProjectError::InvalidRecovery) if primary.is_some() => {}
            result => {
                result?;
            }
        }
    }
    match primary.map(|bytes| decode_for_project(bytes, project_id)) {
        Some(Ok(recovery)) => Ok(recovery),
        Some(Err(ProjectError::InvalidRecovery)) | None if backup.is_some() => {
            Err(ProjectError::RecoveryDecisionRequired)
        }
        Some(Err(error)) => Err(error),
        None => Ok(SourceRecovery {
            format_version: 1,
            project_id: project_id.into(),
            documents: Vec::new(),
            texts: Vec::new(),
        }),
    }
}

pub(super) fn recovery_changes(
    snapshot: &ProjectSnapshot,
    desired: &SourceRecovery,
) -> Result<Vec<ProjectChange>> {
    let current = recovery_from_snapshot(snapshot, &desired.project_id)?;
    if current == *desired {
        return Ok(Vec::new());
    }
    let mut changes = Vec::new();
    if let Some(bytes) = snapshot
        .files
        .get(&ProjectPath::new(RECOVERY_PATH)?)
        .and_then(|bytes| bytes.as_ref())
    {
        changes.push(ProjectChange::Write {
            path: ProjectPath::new(BACKUP_PATH)?,
            expected: snapshot
                .state(&ProjectPath::new(BACKUP_PATH)?)
                .ok_or(ProjectError::InvalidRecovery)?,
            bytes: bytes.clone(),
        });
    }
    changes.push(ProjectChange::Write {
        path: ProjectPath::new(RECOVERY_PATH)?,
        expected: snapshot
            .state(&ProjectPath::new(RECOVERY_PATH)?)
            .ok_or(ProjectError::InvalidRecovery)?,
        bytes: desired.to_bytes()?,
    });
    Ok(changes)
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
