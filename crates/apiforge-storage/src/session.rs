use crate::file::{self, Loaded, WorkspaceFile};
use apiforge_core::format::Workspace;
use ring::digest::{Context, SHA256};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use thiserror::Error;

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_CODE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Document(#[from] file::StorageError),
    #[error("This workspace is open in another API Forge window ({path}).")]
    Locked { path: PathBuf },
    #[error("This workspace is read-only. Changes cannot be saved.")]
    ReadOnly,
    #[error("The file changed outside this window: {path}. Reload it before saving.")]
    ExternallyModified { path: PathBuf },
    #[error("Confirm backup recovery before saving {path}.")]
    RecoveryRequired { path: PathBuf },
    #[error("No usable backup is available for {path}.")]
    NoRecovery { path: PathBuf },
    #[error("Cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("The file exceeds the {MAX_BYTES}-byte storage limit: {path}.")]
    TooLarge { path: PathBuf },
    #[error("Invalid code drafts: {0}")]
    InvalidDrafts(&'static str),
    #[error("Unsupported code draft version; existing drafts were preserved.")]
    UnsupportedDraftVersion,
}

fn io_error(path: &Path, source: io::Error) -> StorageError {
    StorageError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Only source text belongs in drafts; callers must not substitute secret values.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Drafts {
    pub version: u32,
    pub entries: Vec<CodeDraft>,
}

impl Default for Drafts {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodeDraft {
    pub project_id: String,
    pub target: DraftTarget,
    pub base_hash: String,
    pub text: String,
    /// UTF-8 byte offset, at a character boundary.
    pub cursor: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum DraftTarget {
    Scenario { id: String },
    Component { id: String, revision: u32 },
}

#[derive(Debug)]
pub struct LoadedDrafts {
    pub drafts: Drafts,
    pub recovered: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Fingerprint {
    bytes: u64,
    sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    main: Option<Fingerprint>,
    backup: Option<Fingerprint>,
}

#[derive(Debug, Default)]
struct Tracked {
    expected: Option<Snapshot>,
    recovery_required: bool,
}

#[derive(Debug, Default)]
struct State {
    workspace: Tracked,
    drafts: Tracked,
}

#[derive(Debug)]
struct Session {
    // Clones and the background saver share this handle. The OS releases its
    // advisory lock when the last clone is dropped, including after a crash.
    lock: Option<File>,
    state: Mutex<State>,
}

/// One workspace session. Never delete or replace its persistent lock file.
#[derive(Clone, Debug)]
pub struct WorkspaceStore {
    file: WorkspaceFile,
    session: Arc<Session>,
}

impl WorkspaceStore {
    /// Opens the default location; a second window gets a read-only session.
    pub fn default_location() -> Result<Self, StorageError> {
        let directory = dirs::data_dir().ok_or(file::StorageError::NoDataDirectory)?;
        let path = directory.join("apiforge/workspace.json");
        match Self::open_exclusive(&path) {
            Err(StorageError::Locked { .. }) => Self::open_read_only(path),
            result => result,
        }
    }

    pub fn open_exclusive(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let path = path.into();
        let directory = directory(&path);
        create_directory(directory)?;
        let lock_path = directory.join("workspace.lock");
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| io_error(&lock_path, source))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(StorageError::Locked { path: lock_path }),
            Err(TryLockError::Error(source)) => return Err(io_error(&lock_path, source)),
        }
        Ok(Self::new(path, Some(lock)))
    }

    pub fn open_read_only(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        Ok(Self::new(path.into(), None))
    }

    fn new(path: PathBuf, lock: Option<File>) -> Self {
        Self {
            file: WorkspaceFile::at(path),
            session: Arc::new(Session {
                lock,
                state: Mutex::new(State::default()),
            }),
        }
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    pub fn is_read_only(&self) -> bool {
        self.session.lock.is_none()
    }

    fn writable(&self) -> Result<(), StorageError> {
        if self.is_read_only() {
            Err(StorageError::ReadOnly)
        } else {
            Ok(())
        }
    }

    /// Returns a recovery candidate without applying it. Only `restore_backup`
    /// authorizes replacement of a corrupt or missing primary with that backup.
    /// An explicit reload also establishes the next save's expected fingerprints.
    pub fn load(&self) -> Result<Option<Loaded>, StorageError> {
        let mut state = self.session.state.lock().unwrap_or_else(|p| p.into_inner());
        self.load_workspace(&mut state.workspace)
    }

    fn load_workspace(&self, tracked: &mut Tracked) -> Result<Option<Loaded>, StorageError> {
        let before = snapshot(self.path())?;
        let loaded = self.file.load();
        check(self.path(), &before)?;
        tracked.recovery_required = loaded.as_ref().is_err()
            || loaded
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .is_some_and(|l| l.recovered.is_some());
        tracked.expected = Some(before);
        Ok(loaded?)
    }

    pub fn save(&self, workspace: &Workspace) -> Result<(), StorageError> {
        self.writable()?;
        let mut state = self.session.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.workspace.expected.is_none() {
            self.load_workspace(&mut state.workspace)?;
        }
        guard(self.path(), &state.workspace)?;
        self.file.save(workspace)?;
        state.workspace.expected = Some(snapshot(self.path())?);
        Ok(())
    }

    /// Restores the exact backup previewed by `load`. A changed backup or primary
    /// requires another reload and confirmation, including deletion/replacement.
    pub fn restore_backup(&self) -> Result<Workspace, StorageError> {
        self.writable()?;
        let mut state = self.session.state.lock().unwrap_or_else(|p| p.into_inner());
        let tracked = &mut state.workspace;
        let expected = tracked
            .expected
            .as_ref()
            .ok_or_else(|| StorageError::NoRecovery {
                path: self.path().to_owned(),
            })?;
        check(self.path(), expected)?;
        if !tracked.recovery_required {
            return Err(StorageError::NoRecovery {
                path: self.path().to_owned(),
            });
        }
        let loaded = self
            .file
            .load()?
            .filter(|l| l.recovered.is_some())
            .ok_or_else(|| StorageError::NoRecovery {
                path: self.path().to_owned(),
            })?;
        self.file.save(&loaded.workspace)?;
        tracked.expected = Some(snapshot(self.path())?);
        tracked.recovery_required = false;
        Ok(loaded.workspace)
    }

    pub fn archive_legacy_workspace(&self) -> Result<bool, StorageError> {
        self.writable()?;
        let mut state = self.session.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(expected) = &state.workspace.expected {
            check(self.path(), expected)?;
        }
        let archived = self.file.archive_legacy_workspace()?;
        if archived {
            state.workspace.expected = Some(snapshot(self.path())?);
            state.workspace.recovery_required = false;
        }
        Ok(archived)
    }

    fn drafts_path(&self) -> PathBuf {
        self.path().with_file_name("drafts.json")
    }

    pub fn load_drafts(&self) -> Result<Option<LoadedDrafts>, StorageError> {
        let mut state = self.session.state.lock().unwrap_or_else(|p| p.into_inner());
        self.read_drafts(&mut state.drafts)
    }

    fn read_drafts(&self, tracked: &mut Tracked) -> Result<Option<LoadedDrafts>, StorageError> {
        let path = self.drafts_path();
        let before = snapshot(&path)?;
        let result = match read_drafts(&path) {
            Ok(Some(drafts)) => Ok(Some(LoadedDrafts {
                drafts,
                recovered: false,
            })),
            Err(error @ StorageError::UnsupportedDraftVersion) => Err(error),
            main => match read_drafts(&backup_path(&path)) {
                Ok(Some(drafts)) => Ok(Some(LoadedDrafts {
                    drafts,
                    recovered: true,
                })),
                Ok(None) => main.map(|drafts| {
                    drafts.map(|drafts| LoadedDrafts {
                        drafts,
                        recovered: false,
                    })
                }),
                Err(error) => match main {
                    Err(main) => Err(main),
                    _ => Err(error),
                },
            },
        };
        check(&path, &before)?;
        tracked.recovery_required = result.as_ref().is_err()
            || result
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .is_some_and(|l| l.recovered);
        tracked.expected = Some(before);
        result
    }

    /// Uses the workspace lock, but its own change fingerprint and recovery gate.
    /// Known secret values supplied by the caller are never written into drafts.
    pub fn save_drafts(&self, drafts: &Drafts, secret_values: &[&str]) -> Result<(), StorageError> {
        self.writable()?;
        validate_drafts(drafts)?;
        if drafts.entries.iter().any(|draft| {
            secret_values
                .iter()
                .any(|secret| !secret.is_empty() && draft.text.contains(secret))
        }) {
            return Err(StorageError::InvalidDrafts(
                "source contains a secret value; use its environment name",
            ));
        }
        let path = self.drafts_path();
        let mut state = self.session.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.drafts.expected.is_none() {
            self.read_drafts(&mut state.drafts)?;
        }
        guard(&path, &state.drafts)?;
        write_drafts(&path, drafts)?;
        state.drafts.expected = Some(snapshot(&path)?);
        Ok(())
    }

    pub fn restore_drafts_backup(&self) -> Result<Drafts, StorageError> {
        self.writable()?;
        let path = self.drafts_path();
        let mut state = self.session.state.lock().unwrap_or_else(|p| p.into_inner());
        let tracked = &mut state.drafts;
        let expected = tracked
            .expected
            .as_ref()
            .ok_or_else(|| StorageError::NoRecovery { path: path.clone() })?;
        check(&path, expected)?;
        if !tracked.recovery_required {
            return Err(StorageError::NoRecovery { path });
        }
        if matches!(
            read_drafts(&path),
            Err(StorageError::UnsupportedDraftVersion)
        ) {
            return Err(StorageError::UnsupportedDraftVersion);
        }
        let drafts = read_drafts(&backup_path(&path))?
            .ok_or_else(|| StorageError::NoRecovery { path: path.clone() })?;
        // Preserve the good backup instead of rotating a corrupt primary over it.
        write_drafts_bytes(&path, &drafts)?;
        tracked.expected = Some(snapshot(&path)?);
        tracked.recovery_required = false;
        Ok(drafts)
    }
}

fn directory(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn create_directory(path: &Path) -> Result<(), StorageError> {
    let mut missing = Vec::new();
    let mut current = path;
    while !current.exists() {
        missing.push(current.to_owned());
        current = directory(current);
    }
    fs::create_dir_all(path).map_err(|source| io_error(path, source))?;
    #[cfg(unix)]
    for created in missing {
        for directory in [created.as_path(), directory(&created)] {
            File::open(directory)
                .and_then(|f| f.sync_all())
                .map_err(|source| io_error(directory, source))?;
        }
    }
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".bak");
    path.with_file_name(name)
}

fn fingerprint(path: &Path) -> Result<Option<Fingerprint>, StorageError> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(path, error)),
    };
    let mut context = Context::new(&SHA256);
    let mut bytes = 0;
    let mut buffer = [0; 16 * 1024];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|source| io_error(path, source))?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > MAX_BYTES as u64 {
            return Err(StorageError::TooLarge {
                path: path.to_owned(),
            });
        }
        context.update(&buffer[..n]);
    }
    let sha256 = context
        .finish()
        .as_ref()
        .try_into()
        .expect("SHA-256 digest length");
    Ok(Some(Fingerprint { bytes, sha256 }))
}

fn snapshot(path: &Path) -> Result<Snapshot, StorageError> {
    Ok(Snapshot {
        main: fingerprint(path)?,
        backup: fingerprint(&backup_path(path))?,
    })
}

fn check(path: &Path, expected: &Snapshot) -> Result<(), StorageError> {
    if &snapshot(path)? != expected {
        Err(StorageError::ExternallyModified {
            path: path.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn guard(path: &Path, tracked: &Tracked) -> Result<(), StorageError> {
    if let Some(expected) = &tracked.expected {
        check(path, expected)?;
    }
    if tracked.recovery_required {
        Err(StorageError::RecoveryRequired {
            path: path.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_drafts(drafts: &Drafts) -> Result<(), StorageError> {
    if drafts.version != 1 {
        return Err(StorageError::UnsupportedDraftVersion);
    }
    let mut contexts = std::collections::HashSet::new();
    for draft in &drafts.entries {
        if draft.text.len() > MAX_CODE_BYTES || !draft.text.is_char_boundary(draft.cursor) {
            return Err(StorageError::InvalidDrafts("invalid text size or cursor"));
        }
        if draft.project_id.is_empty()
            || draft.base_hash.len() != 64
            || !draft.base_hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(StorageError::InvalidDrafts("invalid context or base hash"));
        }
        match &draft.target {
            DraftTarget::Scenario { id } if id.is_empty() => {
                return Err(StorageError::InvalidDrafts("missing scenario"));
            }
            DraftTarget::Component { id, revision } if id.is_empty() || *revision == 0 => {
                return Err(StorageError::InvalidDrafts("invalid component revision"));
            }
            _ => {}
        }
        if !contexts.insert((&draft.project_id, &draft.target)) {
            return Err(StorageError::InvalidDrafts("duplicate context"));
        }
    }
    Ok(())
}

fn read_drafts(path: &Path) -> Result<Option<Drafts>, StorageError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(path, error)),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| io_error(path, source))?;
    if bytes.len() > MAX_BYTES {
        return Err(StorageError::TooLarge {
            path: path.to_owned(),
        });
    }
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    let version: Version = serde_json::from_slice(&bytes)
        .map_err(|_| StorageError::InvalidDrafts("invalid JSON or version"))?;
    if version.version != 1 {
        return Err(StorageError::UnsupportedDraftVersion);
    }
    let drafts = serde_json::from_slice(&bytes)
        .map_err(|_| StorageError::InvalidDrafts("invalid JSON or unsupported fields"))?;
    validate_drafts(&drafts)?;
    Ok(Some(drafts))
}

fn write_drafts_bytes(path: &Path, drafts: &Drafts) -> Result<(), StorageError> {
    let bytes = serde_json::to_vec_pretty(drafts).map_err(file::StorageError::Serialise)?;
    if bytes.len() > MAX_BYTES {
        return Err(StorageError::TooLarge {
            path: path.to_owned(),
        });
    }
    Ok(file::write_atomic(path, &bytes)?)
}

fn write_drafts(path: &Path, drafts: &Drafts) -> Result<(), StorageError> {
    // Validate the new bytes before changing the existing backup.
    let bytes = serde_json::to_vec_pretty(drafts).map_err(file::StorageError::Serialise)?;
    if bytes.len() > MAX_BYTES {
        return Err(StorageError::TooLarge {
            path: path.to_owned(),
        });
    }
    if let Some(previous) = read_drafts(path)? {
        write_drafts_bytes(&backup_path(path), &previous)?;
    }
    Ok(file::write_atomic(path, &bytes)?)
}

#[cfg(test)]
mod tests;
