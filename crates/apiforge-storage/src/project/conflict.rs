//! Deliberate completion of a transaction after a foreign file edit. Approvals
//! never change original intent: exact observed states and durable conflict copies
//! are bound to that intent in a separate, versioned receipt.
use super::{
    path, schema,
    transaction::{self, ActiveTransaction, Head},
    *,
};
use crate::file::sync_directory;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

const ARCHIVES: &str = ".apiforge/local/conflicts";
const RECEIPT: &str = "resolutions.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryCandidateStatus {
    Expected,
    Applied,
    Approved,
    Conflict,
    /// A link, size limit or access failure prevents safe observation. Intended
    /// bytes can still be inspected/exported; completion requires manual recovery.
    Unavailable,
}

#[derive(Clone)]
pub struct RecoveryCandidate {
    path: ProjectPath,
    expected: FileState,
    target: FileState,
    current: Option<FileState>,
    current_bytes: Option<Vec<u8>>,
    intended_bytes: Option<Vec<u8>>,
    status: RecoveryCandidateStatus,
}

impl RecoveryCandidate {
    pub fn path(&self) -> &ProjectPath {
        &self.path
    }
    pub fn expected(&self) -> &FileState {
        &self.expected
    }
    pub fn target(&self) -> &FileState {
        &self.target
    }
    pub fn current(&self) -> Option<&FileState> {
        self.current.as_ref()
    }
    pub fn current_bytes(&self) -> Option<&[u8]> {
        self.current_bytes.as_deref()
    }
    pub fn intended_bytes(&self) -> Option<&[u8]> {
        self.intended_bytes.as_deref()
    }
    pub fn status(&self) -> RecoveryCandidateStatus {
        self.status
    }
}

/// Durable archive metadata. A null copy path records an observed deletion.
/// Names are derived from transaction/approval/operation IDs, never user paths.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConflictArchive {
    pub approval_id: String,
    pub operation_index: usize,
    pub path: ProjectPath,
    pub accepted: FileState,
    pub copy_path: Option<ProjectPath>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryCompletion {
    pub generation: u64,
    pub archives: Vec<ConflictArchive>,
}

#[derive(Clone)]
pub struct TransactionRecoveryPreview {
    root: PathBuf,
    head: Head,
    descriptor_state: FileState,
    receipt_state: FileState,
    candidates: Vec<RecoveryCandidate>,
    archives: Vec<ConflictArchive>,
}

impl TransactionRecoveryPreview {
    pub fn transaction_id(&self) -> &str {
        self.head
            .active_transaction_id
            .as_deref()
            .unwrap_or_default()
    }
    pub fn generation(&self) -> u64 {
        self.head.generation
    }
    pub fn candidates(&self) -> &[RecoveryCandidate] {
        &self.candidates
    }
    pub fn archives(&self) -> &[ConflictArchive] {
        &self.archives
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResolutionFile {
    format_version: u32,
    transaction_id: String,
    descriptor_hash: String,
    entries: Vec<ConflictArchive>,
}

#[derive(PartialEq, Eq)]
struct TargetObservation {
    state: Option<FileState>,
    bytes: Option<Vec<u8>>,
}

pub(super) struct VerifiedApprovals {
    log: ResolutionFile,
    bytes: Option<Vec<u8>>,
}

impl VerifiedApprovals {
    pub(super) fn accepted_states(&self) -> BTreeMap<usize, FileState> {
        // Later explicit decisions supersede earlier ones for the same operation;
        // all historical copies remain referenced, verified and retained.
        self.log
            .entries
            .iter()
            .map(|entry| (entry.operation_index, entry.accepted.clone()))
            .collect()
    }
}

impl ProjectStore {
    /// Available to read-only owners. Returning candidates changes neither source
    /// nor consent; the byte slices support deliberate export by an adapter.
    pub fn preview_transaction_recovery(&self) -> Result<Option<TransactionRecoveryPreview>> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        self.transaction_preview_locked()
    }

    fn transaction_preview_locked(&self) -> Result<Option<TransactionRecoveryPreview>> {
        for _ in 0..3 {
            let Some(active) = self.read_active_transaction()? else {
                return Ok(None);
            };
            let approvals = self.read_conflict_approvals(&active)?;
            let observed = self.observe_transaction_targets(&active)?;
            if self.observe_transaction_targets(&active)? != observed {
                continue;
            }
            let fresh = self
                .read_active_transaction()?
                .ok_or(ProjectError::NoActiveTransaction)?;
            if fresh.head != active.head || fresh.descriptor_bytes != active.descriptor_bytes {
                continue;
            }
            if self.receipt_bytes(&active)? != approvals.bytes {
                continue;
            }
            let accepted = approvals.accepted_states();
            let candidates = active
                .descriptor
                .operations
                .iter()
                .enumerate()
                .zip(observed)
                .map(
                    |(
                        (index, operation),
                        TargetObservation {
                            state: current,
                            bytes: current_bytes,
                        },
                    )| {
                        let status = match current.as_ref() {
                            None => RecoveryCandidateStatus::Unavailable,
                            Some(state) if state == &operation.target => {
                                RecoveryCandidateStatus::Applied
                            }
                            Some(state) if state == &operation.expected => {
                                RecoveryCandidateStatus::Expected
                            }
                            Some(state) if accepted.get(&index) == Some(state) => {
                                RecoveryCandidateStatus::Approved
                            }
                            Some(_) => RecoveryCandidateStatus::Conflict,
                        };
                        RecoveryCandidate {
                            path: operation.path.clone(),
                            expected: operation.expected.clone(),
                            target: operation.target.clone(),
                            current,
                            current_bytes,
                            intended_bytes: operation
                                .payload
                                .and_then(|index| active.payloads.get(&index))
                                .cloned(),
                            status,
                        }
                    },
                )
                .collect();
            return Ok(Some(TransactionRecoveryPreview {
                root: self.root().into(),
                head: active.head,
                descriptor_state: FileState::of(&active.descriptor_bytes),
                receipt_state: byte_state(approvals.bytes.as_deref()),
                candidates,
                archives: approvals.log.entries,
            }));
        }
        Err(ProjectError::Busy)
    }

    /// This call is the explicit Finish intended operation decision. It approves
    /// exactly the previewed file states, never a fresh implicit version of them.
    /// The original descriptor and staged replacement bytes remain unchanged.
    pub fn finish_transaction_recovery(
        &self,
        preview: &TransactionRecoveryPreview,
    ) -> Result<RecoveryCompletion> {
        self.finish_with_hook(preview, |_| Ok(()))
    }

    fn finish_with_hook(
        &self,
        preview: &TransactionRecoveryPreview,
        mut boundary: impl FnMut(&str) -> Result<()>,
    ) -> Result<RecoveryCompletion> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        let (active, mut approvals) = self.check_recovery_preview(preview)?;
        self.check_transaction_secrets(&active)?;
        let mut additions = Vec::new();
        let approval_id = uuid::Uuid::new_v4().to_string();
        for (index, candidate) in preview.candidates.iter().enumerate() {
            if candidate.status == RecoveryCandidateStatus::Unavailable {
                return Err(ProjectError::UnsupportedRecoveryConflict);
            }
            if candidate.status != RecoveryCandidateStatus::Conflict {
                continue;
            }
            let current = candidate
                .current
                .as_ref()
                .ok_or(ProjectError::UnsupportedRecoveryConflict)?;
            // Recursive foreign directories and file/directory kind replacement
            // are deliberately not authorised by this file-conflict action.
            if current == &FileState::Directory
                || candidate.expected == FileState::Directory
                || candidate.target == FileState::Directory
            {
                return Err(ProjectError::UnsupportedRecoveryConflict);
            }
            if let Some(bytes) = &candidate.current_bytes {
                self.check_secrets(bytes)?;
            }
            additions.push(ConflictArchive {
                approval_id: approval_id.clone(),
                operation_index: index,
                path: candidate.path.clone(),
                accepted: current.clone(),
                copy_path: candidate
                    .current_bytes
                    .as_ref()
                    .map(|_| archive_path(&active, &approval_id, &format!("{index}.bin")))
                    .transpose()?,
            });
        }
        if !additions.is_empty() {
            let mut archive_index = approvals.log.clone();
            archive_index.entries = additions.clone();
            approvals.log.entries.extend(additions.clone());
            validate_resolution(&approvals.log, &active)?;
            let index_bytes = super::codec::encode(&archive_index, MAX_JSON_BYTES)?;
            let receipt_bytes = super::codec::encode(&approvals.log, MAX_JSON_BYTES)?;
            self.check_secrets(&index_bytes)?;
            self.check_secrets(&receipt_bytes)?;
            boundary("preflight")?;
            for relative in [
                ARCHIVES.to_owned(),
                format!("{ARCHIVES}/{}", active.descriptor.transaction_id),
            ] {
                self.ensure_archive_directory(&relative)?;
                boundary("archive-directory")?;
            }
            let attempt = path::resolve(
                self.root(),
                &format!(
                    "{ARCHIVES}/{}/{approval_id}",
                    active.descriptor.transaction_id
                ),
            )?;
            transaction::create_directory(&attempt)?;
            boundary("attempt-directory")?;
            for entry in &additions {
                if let Some(copy) = &entry.copy_path {
                    let bytes = preview.candidates[entry.operation_index]
                        .current_bytes
                        .as_deref()
                        .ok_or(ProjectError::InvalidResolution)?;
                    self.write_conflict_copy(&path::resolve(self.root(), copy.as_str())?, bytes)?;
                    boundary("conflict-copy")?;
                }
            }
            self.write_conflict_copy(&attempt.join("index.json"), &index_bytes)?;
            boundary("archive-index")?;
            // No target has changed yet. A foreign edit during preservation
            // invalidates the decision; orphan copies remain available for export.
            self.check_recovery_preview(preview)?;
            self.atomic(&active.directory.join(RECEIPT), &receipt_bytes)?;
            boundary("approval")?;
        }
        // Reread all receipts and archives before overwriting any target. A
        // crash after approval resumes via ordinary recover without reconfirming.
        let verified = self.read_conflict_approvals(&active)?;
        self.make_approvals_durable(&active, &verified)?;
        boundary("proofs-durable")?;
        self.verify_active_inputs(&active)?;
        let final_proofs = self.read_conflict_approvals(&active)?;
        if final_proofs.bytes != verified.bytes {
            return Err(ProjectError::Busy);
        }
        self.make_approvals_durable(&active, &final_proofs)?;
        self.replay(
            &active.descriptor,
            &active.payloads,
            &final_proofs.accepted_states(),
            &mut boundary,
        )?;
        self.cleanup_transaction(&active);
        Ok(RecoveryCompletion {
            generation: active.descriptor.target_generation,
            archives: final_proofs.log.entries,
        })
    }

    fn check_recovery_preview(
        &self,
        preview: &TransactionRecoveryPreview,
    ) -> Result<(ActiveTransaction, VerifiedApprovals)> {
        if preview.root != self.root() {
            return Err(ProjectError::NoActiveTransaction);
        }
        let active = self
            .read_active_transaction()?
            .ok_or(ProjectError::NoActiveTransaction)?;
        if active.head != preview.head
            || FileState::of(&active.descriptor_bytes) != preview.descriptor_state
        {
            return Err(ProjectError::Busy);
        }
        let approvals = self.read_conflict_approvals(&active)?;
        if byte_state(approvals.bytes.as_deref()) != preview.receipt_state {
            return Err(ProjectError::Busy);
        }
        let observed = self.observe_transaction_targets(&active)?;
        if observed.len() != preview.candidates.len() {
            return Err(ProjectError::InvalidResolution);
        }
        for (observation, candidate) in observed.iter().zip(&preview.candidates) {
            if observation.state != candidate.current {
                return Err(ProjectError::Conflict {
                    path: self.root().join(candidate.path.as_str()),
                });
            }
        }
        Ok((active, approvals))
    }

    fn observe_transaction_targets(
        &self,
        active: &ActiveTransaction,
    ) -> Result<Vec<TargetObservation>> {
        let mut observed = Vec::new();
        let mut total = 0usize;
        for operation in &active.descriptor.operations {
            let observation = (|| {
                let state = self.state(operation.path.as_str())?;
                let bytes = if matches!(state, FileState::File { .. }) {
                    let bytes = self
                        .read_file(operation.path.as_str())?
                        .ok_or(ProjectError::Busy)?;
                    if FileState::of(&bytes) != state {
                        return Err(ProjectError::Busy);
                    }
                    Some(bytes)
                } else {
                    None
                };
                Ok(TargetObservation {
                    state: Some(state),
                    bytes,
                })
            })();
            let value = match observation {
                Ok(value) => value,
                Err(ProjectError::Busy) => return Err(ProjectError::Busy),
                Err(_) => TargetObservation {
                    state: None,
                    bytes: None,
                },
            };
            total = total
                .checked_add(value.bytes.as_ref().map_or(0, Vec::len))
                .ok_or(ProjectError::Limit)?;
            if total > MAX_TRANSACTION_BYTES {
                return Err(ProjectError::Limit);
            }
            observed.push(value);
        }
        Ok(observed)
    }

    pub(super) fn read_conflict_approvals(
        &self,
        active: &ActiveTransaction,
    ) -> Result<VerifiedApprovals> {
        let bytes = self.receipt_bytes(active)?;
        let log = match bytes.as_deref() {
            Some(bytes) => decode_resolution(bytes)?,
            None => ResolutionFile {
                format_version: 1,
                transaction_id: active.descriptor.transaction_id.clone(),
                descriptor_hash: transaction::hash(&active.descriptor_bytes),
                entries: Vec::new(),
            },
        };
        validate_resolution(&log, active)?;
        let mut attempts: BTreeMap<&str, Vec<ConflictArchive>> = BTreeMap::new();
        for entry in &log.entries {
            attempts
                .entry(&entry.approval_id)
                .or_default()
                .push(entry.clone());
        }
        for (approval, entries) in attempts {
            let index_path = archive_path(active, approval, "index.json")?;
            let path = path::resolve(self.root(), index_path.as_str())?;
            let index_bytes = transaction::read_bounded(&path, MAX_JSON_BYTES)?
                .ok_or(ProjectError::InvalidResolution)?;
            let index = decode_resolution(&index_bytes)?;
            validate_resolution(&index, active)?;
            if index.entries != entries {
                return Err(ProjectError::InvalidResolution);
            }
            for entry in &entries {
                if let Some(copy) = &entry.copy_path {
                    let path = path::resolve(self.root(), copy.as_str())?;
                    let bytes = transaction::read_bounded(
                        &path,
                        transaction::payload_limit(entry.path.as_str()),
                    )?
                    .ok_or(ProjectError::InvalidResolution)?;
                    if FileState::of(&bytes) != entry.accepted {
                        return Err(ProjectError::InvalidResolution);
                    }
                }
            }
            if transaction::read_bounded(&path, MAX_JSON_BYTES)?.as_deref()
                != Some(index_bytes.as_slice())
            {
                return Err(ProjectError::Busy);
            }
        }
        if self.receipt_bytes(active)? != bytes {
            return Err(ProjectError::Busy);
        }
        Ok(VerifiedApprovals { log, bytes })
    }

    fn receipt_bytes(&self, active: &ActiveTransaction) -> Result<Option<Vec<u8>>> {
        transaction::read_bounded(&active.directory.join(RECEIPT), MAX_JSON_BYTES)
    }

    /// An earlier attempt may have failed after a visible rename but before its
    /// directory flush. Rereading proof bytes alone is not a durability barrier.
    pub(super) fn make_approvals_durable(
        &self,
        active: &ActiveTransaction,
        approvals: &VerifiedApprovals,
    ) -> Result<()> {
        if approvals.bytes.is_none() {
            return Ok(());
        }
        let mut files = HashSet::from([active.directory.join(RECEIPT)]);
        for entry in &approvals.log.entries {
            files.insert(path::resolve(
                self.root(),
                archive_path(active, &entry.approval_id, "index.json")?.as_str(),
            )?);
            if let Some(copy) = &entry.copy_path {
                files.insert(path::resolve(self.root(), copy.as_str())?);
            }
        }
        let mut directories = HashSet::new();
        for file in files {
            path::check_boundary(&file)?;
            File::options()
                .read(true)
                .write(true)
                .open(&file)
                .and_then(|file| file.sync_all())
                .map_err(|error| ProjectError::io(&file, error))?;
            let mut parent = file.parent();
            while let Some(directory) =
                parent.filter(|directory| directory.starts_with(self.root()))
            {
                directories.insert(directory.to_owned());
                parent = directory.parent();
            }
        }
        // Children before ancestors, retaining durability of their directory names.
        let mut directories: Vec<_> = directories.into_iter().collect();
        directories.sort_by_key(|directory| std::cmp::Reverse(directory.components().count()));
        for directory in directories {
            path::check_boundary(&directory)?;
            sync_directory(&directory).map_err(|error| ProjectError::io(&directory, error))?;
        }
        Ok(())
    }

    fn ensure_archive_directory(&self, relative: &str) -> Result<()> {
        let path = path::resolve(self.root(), relative)?;
        match self.state(relative)? {
            FileState::Missing => transaction::create_directory(&path)?,
            FileState::Directory => {}
            _ => return Err(ProjectError::Conflict { path }),
        }
        sync_directory(&path).map_err(|error| ProjectError::io(&path, error))?;
        transaction::sync_parent(&path)
    }

    fn write_conflict_copy(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        path::check_boundary(path)?;
        self.check_secrets(bytes)?;
        let directory = path.parent().ok_or(ProjectError::InvalidPath)?;
        let mut temporary = tempfile::NamedTempFile::new_in(directory)
            .map_err(|error| ProjectError::io(directory, error))?;
        temporary
            .write_all(bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| ProjectError::io(temporary.path(), error))?;
        temporary
            .persist_noclobber(path)
            .map_err(|error| ProjectError::io(path, error.error))?;
        transaction::sync_parent(path)
    }
}

fn byte_state(bytes: Option<&[u8]>) -> FileState {
    bytes.map_or(FileState::Missing, FileState::of)
}

fn archive_path(active: &ActiveTransaction, approval: &str, name: &str) -> Result<ProjectPath> {
    if uuid::Uuid::parse_str(approval).is_err() || approval.len() != 36 {
        return Err(ProjectError::InvalidResolution);
    }
    ProjectPath::new(format!(
        "{ARCHIVES}/{}/{approval}/{name}",
        active.descriptor.transaction_id
    ))
}

fn decode_resolution(bytes: &[u8]) -> Result<ResolutionFile> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Version {
        format_version: u32,
    }
    let version: Version = apiforge_core::strict_json::from_slice(bytes)
        .map_err(|_| ProjectError::InvalidResolution)?;
    if version.format_version != 1 {
        return Err(ProjectError::UnsupportedVersion);
    }
    apiforge_core::strict_json::from_slice(bytes).map_err(|_| ProjectError::InvalidResolution)
}

fn validate_resolution(log: &ResolutionFile, active: &ActiveTransaction) -> Result<()> {
    if log.format_version != 1 {
        return Err(ProjectError::UnsupportedVersion);
    }
    if log.transaction_id != active.descriptor.transaction_id
        || log.descriptor_hash != transaction::hash(&active.descriptor_bytes)
    {
        return Err(ProjectError::InvalidResolution);
    }
    if log.entries.len() > MAX_TRANSACTION_OPERATIONS {
        return Err(ProjectError::Limit);
    }
    let mut keys = HashSet::new();
    let mut total = 0u64;
    for entry in &log.entries {
        let operation = active
            .descriptor
            .operations
            .get(entry.operation_index)
            .ok_or(ProjectError::InvalidResolution)?;
        if entry.path != operation.path
            || entry.accepted == operation.expected
            || entry.accepted == operation.target
            || operation.expected == FileState::Directory
            || operation.target == FileState::Directory
            || !keys.insert((&entry.approval_id, entry.operation_index))
        {
            return Err(ProjectError::InvalidResolution);
        }
        // Validate the ID even for a missing-state approval with no copy.
        archive_path(active, &entry.approval_id, "index.json")?;
        match &entry.accepted {
            FileState::Missing if entry.copy_path.is_none() => {}
            FileState::File { bytes, sha256 }
                if *bytes <= transaction::payload_limit(entry.path.as_str()) as u64
                    && schema::valid_hash(sha256) =>
            {
                let expected = archive_path(
                    active,
                    &entry.approval_id,
                    &format!("{}.bin", entry.operation_index),
                )?;
                if entry.copy_path.as_ref() != Some(&expected) {
                    return Err(ProjectError::InvalidResolution);
                }
                total = total.checked_add(*bytes).ok_or(ProjectError::Limit)?;
                if total > MAX_TRANSACTION_BYTES as u64 {
                    return Err(ProjectError::Limit);
                }
            }
            _ => return Err(ProjectError::InvalidResolution),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "conflict_tests.rs"]
mod tests;
