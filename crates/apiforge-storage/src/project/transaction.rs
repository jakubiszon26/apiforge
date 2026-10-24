use super::{path, schema, *};
use crate::file::{sync_directory, write_atomic};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, TryLockError},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const HEAD: &str = ".apiforge/local/head.json";
const TRANSACTIONS: &str = ".apiforge/local/transactions";

/// Includes absence and directories so that creates and deletes detect races.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum FileState {
    Missing,
    File { bytes: u64, sha256: String },
    Directory,
}

impl FileState {
    pub fn of(bytes: &[u8]) -> Self {
        Self::File {
            bytes: bytes.len() as u64,
            sha256: hash(bytes),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ProjectChange {
    Write {
        path: ProjectPath,
        expected: FileState,
        bytes: Vec<u8>,
    },
    RemoveFile {
        path: ProjectPath,
        expected: FileState,
    },
    CreateDirectory {
        path: ProjectPath,
    },
    RemoveDirectory {
        path: ProjectPath,
    },
}

/// All bytes came from one stable generation. Missing requested files remain
/// explicit; callers must not silently replace them with cached definitions.
#[derive(Clone, Debug)]
pub struct ProjectSnapshot {
    pub generation: u64,
    pub files: BTreeMap<ProjectPath, Option<Vec<u8>>>,
}

impl ProjectSnapshot {
    pub fn state(&self, path: &ProjectPath) -> Option<FileState> {
        self.files
            .get(path)
            .map(|bytes| bytes.as_deref().map_or(FileState::Missing, FileState::of))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Head {
    format_version: u32,
    generation: u64,
    active_transaction_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Descriptor {
    format_version: u32,
    transaction_id: String,
    base_generation: u64,
    target_generation: u64,
    operations: Vec<Operation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Operation {
    path: ProjectPath,
    expected: FileState,
    target: FileState,
    payload: Option<usize>,
}

pub(super) struct Owner {
    lock: Option<File>,
    pub(super) worker: Mutex<()>,
    secrets: Mutex<Vec<String>>,
}

/// Cloneable ownership retains the OS writer lock until the final worker or
/// session drops it. Mutations and reads within an owner are serialised.
#[derive(Clone)]
pub struct ProjectStore {
    root: PathBuf,
    pub(super) owner: Arc<Owner>,
}

impl ProjectStore {
    /// A second writer falls back to read-only, without creating local data.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        match Self::open_exclusive(root.as_ref()) {
            Err(ProjectError::Locked) => Self::open_read_only(root),
            result => result,
        }
    }

    pub fn open_read_only(root: impl AsRef<Path>) -> Result<Self> {
        Self::new(root.as_ref(), None)
    }

    pub fn open_exclusive(root: impl AsRef<Path>) -> Result<Self> {
        let store = Self::new(root.as_ref(), None)?;
        for relative in [".apiforge", ".apiforge/local"] {
            let path = path::resolve(&store.root, relative)?;
            if !path.exists() {
                create_directory(&path)?;
            }
        }
        let lock_path = path::resolve(&store.root, ".apiforge/local/project.lock")?;
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| ProjectError::io(&lock_path, error))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(ProjectError::Locked),
            Err(TryLockError::Error(error)) => return Err(ProjectError::io(&lock_path, error)),
        }
        // An earlier opener may have failed while flushing newly-created local
        // infrastructure. Establish its durability even when it already exists.
        for relative in [".apiforge", ".apiforge/local"] {
            let path = path::resolve(&store.root, relative)?;
            sync_directory(&path).map_err(|error| ProjectError::io(&path, error))?;
            sync_parent(&path)?;
        }
        sync_parent(&lock_path)?;
        let transactions = path::resolve(&store.root, TRANSACTIONS)?;
        if !transactions.exists() {
            create_directory(&transactions)?;
        }
        sync_directory(&transactions).map_err(|error| ProjectError::io(&transactions, error))?;
        sync_parent(&transactions)?;
        Self::new(&store.root, Some(lock))
    }

    fn new(root: &Path, lock: Option<File>) -> Result<Self> {
        let root = std::path::absolute(root).map_err(|error| ProjectError::io(root, error))?;
        path::check_boundary(&root)?;
        if !root.is_dir() {
            return Err(ProjectError::InvalidPath);
        }
        // Retain the checked, absolute spelling so repeated checks also catch a
        // root renamed/replaced by a symlink after opening.
        Ok(Self {
            root,
            owner: Arc::new(Owner {
                lock,
                worker: Mutex::new(()),
                secrets: Mutex::new(Vec::new()),
            }),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn is_read_only(&self) -> bool {
        self.owner.lock.is_none()
    }

    /// Session-only values, never serialised in descriptors or error messages.
    pub fn set_known_secrets(&self, values: impl IntoIterator<Item = String>) {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        *self.owner.secrets.lock().unwrap_or_else(|p| p.into_inner()) = values
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect();
    }

    pub(super) fn check_secrets(&self, bytes: &[u8]) -> Result<()> {
        for value in self
            .owner
            .secrets
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
        {
            // JSON encodes control characters and quotes. Protect both the raw
            // bytes in source and the escaped representation in JSON snapshots.
            let encoded = serde_json::to_string(value).map_err(|_| ProjectError::Secret)?;
            if bytes
                .windows(value.len())
                .any(|window| window == value.as_bytes())
                || bytes
                    .windows(encoded.len() - 2)
                    .any(|window| window == &encoded.as_bytes()[1..encoded.len() - 1])
            {
                return Err(ProjectError::Secret);
            }
        }
        Ok(())
    }

    /// Stable raw reads for storage clients. Definitions are loaded with `load`,
    /// which discovers all registered sources inside the same read protocol.
    pub fn read(&self, paths: &[ProjectPath]) -> Result<ProjectSnapshot> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        self.stable_read(|| Ok(paths.to_vec()))
    }

    /// Reads the registry, every working/published source and the optional diagram
    /// checkpoint. Invalid source is data, not a storage failure. Missing or
    /// mismatched immutable published source is always an integrity error.
    pub fn load(&self) -> Result<(ProjectManifest, ProjectSnapshot)> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        let snapshot = self.stable_read(|| {
            let bytes = self
                .read_file(MANIFEST_PATH)?
                .ok_or(ProjectError::InvalidMetadata)?;
            let manifest = ProjectManifest::from_bytes(&bytes)?;
            let mut paths = vec![
                manifest_path.clone(),
                ProjectPath::new(".apiforge/diagram.json")?,
            ];
            paths.extend(manifest.documents.iter().map(|entry| entry.path.clone()));
            paths.extend(manifest.revisions.iter().map(|entry| entry.path.clone()));
            Ok(paths)
        })?;
        let manifest = ProjectManifest::from_bytes(
            snapshot.files[&manifest_path]
                .as_deref()
                .ok_or(ProjectError::InvalidMetadata)?,
        )?;
        // The registry may have changed between discovery and the first read.
        // Fail rather than accept a registry whose dependency set was not read.
        for entry in &manifest.documents {
            if !snapshot.files.contains_key(&entry.path) {
                return Err(ProjectError::Busy);
            }
        }
        for entry in &manifest.revisions {
            let bytes = snapshot
                .files
                .get(&entry.path)
                .and_then(|bytes| bytes.as_deref())
                .ok_or(ProjectError::InvalidMetadata)?;
            if hash(bytes) != entry.source_hash {
                return Err(ProjectError::InvalidMetadata);
            }
        }
        Ok((manifest, snapshot))
    }

    pub(super) fn stable_read(
        &self,
        discover: impl Fn() -> Result<Vec<ProjectPath>>,
    ) -> Result<ProjectSnapshot> {
        self.stable_read_with_hook(discover, |_, _| Ok(()))
    }

    fn stable_read_with_hook(
        &self,
        discover: impl Fn() -> Result<Vec<ProjectPath>>,
        mut boundary: impl FnMut(&str, usize) -> Result<()>,
    ) -> Result<ProjectSnapshot> {
        for attempt in 0..3 {
            let before = self.head()?;
            if before.active_transaction_id.is_some() {
                return Err(ProjectError::RecoveryRequired);
            }
            // Discovery is fingerprinted as well as content reads. This avoids
            // accepting a different registry between dependency discovery/read.
            let registry_before = self.read_file(MANIFEST_PATH)?;
            let paths = discover()?;
            let mut files = BTreeMap::new();
            for path in &paths {
                files.insert(path.clone(), self.read_file(path.as_str())?);
            }
            boundary("read", attempt)?;
            let mut unchanged =
                self.head()? == before && self.read_file(MANIFEST_PATH)? == registry_before;
            for (path, bytes) in &files {
                unchanged &= self.read_file(path.as_str())? == *bytes;
            }
            boundary("recheck", attempt)?;
            // Head must also remain stable across the fingerprint recheck.
            unchanged &= self.head()? == before;
            if unchanged {
                return Ok(ProjectSnapshot {
                    generation: before.generation,
                    files,
                });
            }
        }
        Err(ProjectError::Busy)
    }

    /// Executes one logical operation. Expected fingerprints are supplied by the
    /// caller's last observation; a queued plan never silently adopts new bytes.
    pub fn commit(&self, changes: Vec<ProjectChange>) -> Result<u64> {
        self.commit_with_hook(changes, |_| Ok(()))
    }

    fn commit_with_hook(
        &self,
        changes: Vec<ProjectChange>,
        mut boundary: impl FnMut(&str) -> Result<()>,
    ) -> Result<u64> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        let head = self.head()?;
        if head.active_transaction_id.is_some() {
            return Err(ProjectError::RecoveryRequired);
        }
        let manifest_before = self.read_file(MANIFEST_PATH)?;
        let (descriptor, payloads) = self.plan(&head, changes)?;
        self.validate_manifest_plan(&descriptor, &payloads)?;
        self.preflight_directories(&descriptor)?;
        for operation in &descriptor.operations {
            if self.state(operation.path.as_str())? != operation.expected {
                return Err(self.conflict(&operation.path));
            }
        }
        boundary("preflight")?;
        let directory = self.transaction_path(&descriptor.transaction_id)?;
        create_directory(&directory)?;
        boundary("stage-directory")?;
        for (index, bytes) in &payloads {
            self.atomic(&directory.join(index.to_string()), bytes)?;
            boundary("payload")?;
        }
        self.atomic(
            &directory.join("transaction.json"),
            &json_bytes(&descriptor)?,
        )?;
        boundary("descriptor")?;
        // Recheck all targets after staging and before durable activation.
        for operation in &descriptor.operations {
            if self.state(operation.path.as_str())? != operation.expected {
                return Err(self.conflict(&operation.path));
            }
        }
        if self.head()? != head || self.read_file(MANIFEST_PATH)? != manifest_before {
            return Err(ProjectError::Busy);
        }
        self.write_head(&Head {
            active_transaction_id: Some(descriptor.transaction_id.clone()),
            ..head
        })?;
        boundary("activation")?;
        self.replay(&descriptor, &payloads, &mut boundary)?;
        // Cleanup is not part of acknowledgement. Only this transaction's own
        // staging directory may be removed, and never replay unreferenced intent.
        let _ = fs::remove_dir_all(&directory);
        let _ = sync_parent(&directory);
        Ok(descriptor.target_generation)
    }

    /// Explicit roll-forward. Never overwrites a third (foreign) file state.
    /// A read-only opener cannot perform this operation or consume mixed files.
    pub fn recover(&self) -> Result<Option<u64>> {
        self.recover_with_hook(|_| Ok(()))
    }

    fn recover_with_hook(
        &self,
        mut boundary: impl FnMut(&str) -> Result<()>,
    ) -> Result<Option<u64>> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        let head = self.head()?;
        let Some(id) = &head.active_transaction_id else {
            return Ok(None);
        };
        let directory = self.transaction_path(id)?;
        let descriptor: Descriptor = parse_json(
            &read_bounded(&directory.join("transaction.json"), MAX_JSON_BYTES)?
                .ok_or(ProjectError::InvalidTransaction)?,
        )?;
        if descriptor.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        if descriptor.transaction_id != *id
            || descriptor.base_generation != head.generation
            || head.generation.checked_add(1) != Some(descriptor.target_generation)
        {
            return Err(ProjectError::InvalidTransaction);
        }
        let mut payloads = BTreeMap::new();
        // Reject oversized or duplicate plans before reading their staged files.
        if descriptor.operations.len() > MAX_TRANSACTION_OPERATIONS {
            return Err(ProjectError::Limit);
        }
        let mut total = 0usize;
        for (operation_index, operation) in descriptor.operations.iter().enumerate() {
            if let Some(index) = operation.payload {
                if index != operation_index {
                    return Err(ProjectError::InvalidTransaction);
                }
                let path = directory.join(index.to_string());
                path::check_boundary(&path)?;
                let bytes = read_bounded(&path, payload_limit(operation.path.as_str()))?
                    .ok_or(ProjectError::InvalidTransaction)?;
                total = total.checked_add(bytes.len()).ok_or(ProjectError::Limit)?;
                if total > MAX_TRANSACTION_BYTES {
                    return Err(ProjectError::Limit);
                }
                self.check_secrets(&bytes)?;
                payloads.insert(index, bytes);
            }
        }
        self.validate_descriptor(&descriptor, &payloads)?;
        if let Some(operation) = descriptor
            .operations
            .iter()
            .find(|operation| operation.path.as_str() == MANIFEST_PATH)
        {
            let bytes = operation
                .payload
                .and_then(|index| payloads.get(&index))
                .ok_or(ProjectError::InvalidTransaction)?;
            ProjectManifest::from_bytes(bytes)?;
        }
        self.replay(&descriptor, &payloads, &mut boundary)?;
        let _ = fs::remove_dir_all(&directory);
        let _ = sync_parent(&directory);
        Ok(Some(descriptor.target_generation))
    }

    fn replay(
        &self,
        descriptor: &Descriptor,
        payloads: &BTreeMap<usize, Vec<u8>>,
        boundary: &mut impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        for operation in &descriptor.operations {
            let path = path::resolve(&self.root, operation.path.as_str())?;
            let current = self.state(operation.path.as_str())?;
            if current != operation.target {
                if current != operation.expected {
                    return Err(self.conflict(&operation.path));
                }
                match &operation.target {
                    FileState::File { .. } => self.atomic(
                        &path,
                        &payloads[&operation.payload.ok_or(ProjectError::InvalidTransaction)?],
                    )?,
                    FileState::Directory => create_directory(&path)?,
                    FileState::Missing => {
                        match operation.expected {
                            FileState::Directory => fs::remove_dir(&path),
                            _ => fs::remove_file(&path),
                        }
                        .map_err(|error| ProjectError::io(&path, error))?;
                        sync_parent(&path)?;
                    }
                }
            } else {
                // A previous attempt may have failed after replacement but before
                // directory sync. Idempotent replay must still establish durability.
                if matches!(current, FileState::File { .. }) {
                    File::options()
                        .read(true)
                        .write(true)
                        .open(&path)
                        .and_then(|file| file.sync_all())
                        .map_err(|error| ProjectError::io(&path, error))?;
                }
                if current == FileState::Directory {
                    sync_directory(&path).map_err(|error| ProjectError::io(&path, error))?;
                }
                if current == FileState::Missing {
                    // A later operation may already have removed this parent
                    // before the previous replay crashed. Flush the surviving
                    // ancestor rather than requiring a deleted directory.
                    sync_surviving_parent(&path)?;
                } else {
                    sync_parent(&path)?;
                }
            }
            boundary("operation")?;
        }
        self.write_head(&Head {
            format_version: 1,
            generation: descriptor.target_generation,
            active_transaction_id: None,
        })?;
        boundary("commit")?;
        Ok(())
    }

    fn plan(
        &self,
        head: &Head,
        changes: Vec<ProjectChange>,
    ) -> Result<(Descriptor, BTreeMap<usize, Vec<u8>>)> {
        let mut payloads = BTreeMap::new();
        let mut operations = Vec::new();
        for (index, change) in changes.into_iter().enumerate() {
            let operation = match change {
                ProjectChange::Write {
                    path,
                    expected,
                    bytes,
                } => {
                    self.check_secrets(&bytes)?;
                    let target = FileState::of(&bytes);
                    payloads.insert(index, bytes);
                    Operation {
                        path,
                        expected,
                        target,
                        payload: Some(index),
                    }
                }
                ProjectChange::RemoveFile { path, expected } => Operation {
                    path,
                    expected,
                    target: FileState::Missing,
                    payload: None,
                },
                ProjectChange::CreateDirectory { path } => Operation {
                    path,
                    expected: FileState::Missing,
                    target: FileState::Directory,
                    payload: None,
                },
                ProjectChange::RemoveDirectory { path } => Operation {
                    path,
                    expected: FileState::Directory,
                    target: FileState::Missing,
                    payload: None,
                },
            };
            operations.push(operation);
        }
        // Full random UUIDs; create_dir reserves the name without replacing any
        // existing staging directory if a collision occurs.
        let transaction_id = uuid::Uuid::new_v4().to_string();
        let descriptor = Descriptor {
            format_version: 1,
            transaction_id,
            base_generation: head.generation,
            target_generation: head.generation.checked_add(1).ok_or(ProjectError::Limit)?,
            operations,
        };
        self.validate_descriptor(&descriptor, &payloads)?;
        Ok((descriptor, payloads))
    }

    fn validate_descriptor(
        &self,
        descriptor: &Descriptor,
        payloads: &BTreeMap<usize, Vec<u8>>,
    ) -> Result<()> {
        if descriptor.operations.is_empty()
            || descriptor.operations.len() > MAX_TRANSACTION_OPERATIONS
        {
            return Err(ProjectError::Limit);
        }
        let mut paths = HashSet::new();
        let mut used_payloads = HashSet::new();
        let mut total = 0;
        for (index, operation) in descriptor.operations.iter().enumerate() {
            // Recovery has checked-backup and deliberate-restore rules of its
            // own. It cannot be replaced through the generic transaction API.
            if matches!(
                operation.path.as_str(),
                super::recovery::RECOVERY_PATH | super::recovery::BACKUP_PATH
            ) {
                return Err(ProjectError::InvalidTransaction);
            }
            if !paths.insert(operation.path.as_str().to_lowercase()) {
                return Err(ProjectError::InvalidTransaction);
            }
            for state in [&operation.expected, &operation.target] {
                if let FileState::File { bytes, sha256 } = state
                    && (!schema::valid_hash(sha256)
                        || *bytes > payload_limit(operation.path.as_str()) as u64)
                {
                    return Err(ProjectError::InvalidTransaction);
                }
            }
            if operation.path.is_revision()
                && !(operation.expected == FileState::Missing
                    && operation.target != FileState::Missing)
            {
                return Err(ProjectError::ImmutableRevision);
            }
            match (&operation.expected, &operation.target, operation.payload) {
                (
                    FileState::Missing | FileState::File { .. },
                    FileState::File { .. },
                    Some(payload),
                ) if payload == index => {
                    let bytes = payloads
                        .get(&payload)
                        .ok_or(ProjectError::InvalidTransaction)?;
                    if FileState::of(bytes) != operation.target {
                        return Err(ProjectError::InvalidTransaction);
                    }
                    if bytes.len() > payload_limit(operation.path.as_str()) {
                        return Err(ProjectError::Limit);
                    }
                    used_payloads.insert(payload);
                    total += bytes.len();
                }
                (FileState::Missing, FileState::Directory, None)
                | (FileState::Directory | FileState::File { .. }, FileState::Missing, None) => {}
                _ => return Err(ProjectError::InvalidTransaction),
            }
        }
        if total > MAX_TRANSACTION_BYTES || used_payloads.len() != payloads.len() {
            return Err(ProjectError::Limit);
        }
        self.check_secrets(&json_bytes(descriptor)?)?;
        Ok(())
    }

    fn validate_manifest_plan(
        &self,
        descriptor: &Descriptor,
        payloads: &BTreeMap<usize, Vec<u8>>,
    ) -> Result<()> {
        if let Some(bytes) = self.read_file(MANIFEST_PATH)? {
            ProjectManifest::from_bytes(&bytes)?;
        }
        let operation = descriptor
            .operations
            .iter()
            .find(|operation| operation.path.as_str() == MANIFEST_PATH);
        if let Some(operation) = operation {
            let payload = operation
                .payload
                .and_then(|index| payloads.get(&index))
                .ok_or(ProjectError::InvalidMetadata)?;
            let manifest = ProjectManifest::from_bytes(payload)?;
            if manifest
                .environment
                .secret_names
                .iter()
                .any(|name| manifest.environment.variables.contains_key(name))
            {
                return Err(ProjectError::Secret);
            }
        } else if self.read_file(MANIFEST_PATH)?.is_none() {
            return Err(ProjectError::InvalidMetadata);
        }
        Ok(())
    }

    /// Directories are explicit and ordered. Catch missing parents and nonempty
    /// removals before activation, taking earlier operations into account.
    fn preflight_directories(&self, descriptor: &Descriptor) -> Result<()> {
        let mut virtual_states = BTreeMap::<String, FileState>::new();
        for operation in &descriptor.operations {
            let parent = operation
                .path
                .as_str()
                .rsplit_once('/')
                .map(|(parent, _)| parent);
            if let Some(parent) = parent {
                let state = match virtual_states.get(parent) {
                    Some(state) => state.clone(),
                    None => self.state(parent)?,
                };
                if state != FileState::Directory {
                    return Err(ProjectError::InvalidTransaction);
                }
            }
            if operation.expected == FileState::Directory && operation.target == FileState::Missing
            {
                let path = path::resolve(&self.root, operation.path.as_str())?;
                let entries =
                    fs::read_dir(&path).map_err(|error| ProjectError::io(&path, error))?;
                for entry in entries {
                    let entry = entry.map_err(|error| ProjectError::io(&path, error))?;
                    let child = format!(
                        "{}/{}",
                        operation.path.as_str(),
                        entry.file_name().to_string_lossy()
                    );
                    if virtual_states.get(&child) != Some(&FileState::Missing) {
                        return Err(ProjectError::InvalidTransaction);
                    }
                }
                let prefix = format!("{}/", operation.path.as_str());
                if virtual_states
                    .iter()
                    .any(|(path, state)| path.starts_with(&prefix) && *state != FileState::Missing)
                {
                    return Err(ProjectError::InvalidTransaction);
                }
            }
            virtual_states.insert(operation.path.as_str().to_owned(), operation.target.clone());
        }
        Ok(())
    }

    pub(super) fn writable(&self) -> Result<()> {
        if self.is_read_only() {
            Err(ProjectError::ReadOnly)
        } else {
            Ok(())
        }
    }
    fn conflict(&self, path: &ProjectPath) -> ProjectError {
        ProjectError::Conflict {
            path: self.root.join(path.as_str()),
        }
    }

    fn state(&self, relative: &str) -> Result<FileState> {
        let path = path::resolve(&self.root, relative)?;
        if path.is_dir() {
            return Ok(FileState::Directory);
        }
        Ok(read_bounded(&path, payload_limit(relative))?
            .as_deref()
            .map_or(FileState::Missing, FileState::of))
    }

    fn read_file(&self, relative: &str) -> Result<Option<Vec<u8>>> {
        let path = path::resolve(&self.root, relative)?;
        read_bounded(&path, payload_limit(relative))
    }

    fn head(&self) -> Result<Head> {
        let Some(bytes) = self.read_file(HEAD)? else {
            return Ok(Head {
                format_version: 1,
                ..Head::default()
            });
        };
        let head: Head = parse_json(&bytes)?;
        if head.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        if let Some(id) = &head.active_transaction_id {
            self.transaction_path(id)?;
        }
        Ok(head)
    }

    fn write_head(&self, head: &Head) -> Result<()> {
        self.atomic(&path::resolve(&self.root, HEAD)?, &json_bytes(head)?)
    }
    fn transaction_path(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty()
            || id.len() > 100
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(ProjectError::InvalidTransaction);
        }
        path::resolve(&self.root, &format!("{TRANSACTIONS}/{id}"))
    }

    pub(super) fn atomic(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        path::check_boundary(path)?;
        // All managed directories are explicit operations (or writer-local
        // infrastructure), never implicitly created by write_atomic.
        if !path.parent().is_some_and(Path::is_dir) {
            return Err(ProjectError::InvalidPath);
        }
        write_atomic(path, bytes).map_err(|error| match error {
            crate::file::StorageError::Io { path, source } => ProjectError::Io { path, source },
            _ => ProjectError::InvalidTransaction,
        })
    }
}

fn payload_limit(path: &str) -> usize {
    if path.ends_with(".afs") {
        MAX_SOURCE_BYTES
    } else {
        MAX_JSON_BYTES
    }
}
fn hash(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    super::codec::encode(value, MAX_JSON_BYTES)
}
fn parse_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    apiforge_core::strict_json::from_slice(bytes).map_err(|_| ProjectError::InvalidTransaction)
}
fn read_bounded(path: &Path, limit: usize) -> Result<Option<Vec<u8>>> {
    path::check_boundary(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(ProjectError::UnsafePath {
                    path: path.to_owned(),
                });
            }
            if metadata.len() > limit as u64 {
                return Err(ProjectError::Limit);
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ProjectError::io(path, error)),
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ProjectError::io(path, error)),
    };
    if !file
        .metadata()
        .map_err(|error| ProjectError::io(path, error))?
        .is_file()
    {
        return Err(ProjectError::UnsafePath {
            path: path.to_owned(),
        });
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ProjectError::io(path, error))?;
    if bytes.len() > limit {
        return Err(ProjectError::Limit);
    }
    Ok(Some(bytes))
}
fn sync_parent(path: &Path) -> Result<()> {
    let parent = path.parent().ok_or(ProjectError::InvalidPath)?;
    sync_directory(parent).map_err(|error| ProjectError::io(parent, error))
}
fn sync_surviving_parent(path: &Path) -> Result<()> {
    let mut parent = path.parent().ok_or(ProjectError::InvalidPath)?;
    while !parent.is_dir() {
        parent = parent.parent().ok_or(ProjectError::InvalidPath)?;
    }
    path::check_boundary(parent)?;
    sync_directory(parent).map_err(|error| ProjectError::io(parent, error))
}
fn create_directory(path: &Path) -> Result<()> {
    path::check_boundary(path)?;
    fs::create_dir(path).map_err(|error| ProjectError::io(path, error))?;
    sync_directory(path).map_err(|error| ProjectError::io(path, error))?;
    sync_parent(path)
}

#[cfg(test)]
#[path = "transaction_tests.rs"]
mod tests;
