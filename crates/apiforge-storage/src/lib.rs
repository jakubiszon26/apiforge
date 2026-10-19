//! Durable local storage of the workspace and atomic JSON file writes.
//!
//! The workspace is stored as a version-one workspace document, the same format as a
//! workspace export, so a saved file can always be opened by the importer. Each save
//! writes a temporary file in the target directory, flushes it to disk and atomically
//! replaces the previous file, which is first kept as a backup. A crash therefore
//! leaves either the old or the new document, never a truncated one.
//!
//! Secret values (variables listed in `environment.secretNames`) are never written.

use apiforge_core::{
    ImportError, ImportLimits,
    format::{Project, Workspace},
    import_document,
};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

const WORKSPACE_FILE: &str = "workspace.json";
const BACKUP_SUFFIX: &str = ".bak";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("no data directory is available for this user")]
    NoDataDirectory,
    #[error("cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{path} is not a valid workspace: {source}")]
    Invalid {
        path: PathBuf,
        #[source]
        source: ImportError,
    },
    #[error("cannot serialise the workspace: {0}")]
    Serialise(#[from] serde_json::Error),
}

impl StorageError {
    fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}

/// A workspace read from storage.
#[derive(Debug)]
pub struct Loaded {
    pub workspace: Workspace,
    /// Set when the main file was unreadable and the backup was used instead; the
    /// main file is left untouched for inspection until the next successful save.
    pub recovered: Option<String>,
}

/// The workspace file of one user.
#[derive(Clone, Debug)]
pub struct WorkspaceStore {
    path: PathBuf,
}

impl WorkspaceStore {
    /// `<data dir>/apiforge/workspace.json`, e.g. `~/.local/share/apiforge` on Linux
    /// and `%APPDATA%\apiforge` on Windows.
    pub fn default_location() -> Result<Self, StorageError> {
        let directory = dirs::data_dir().ok_or(StorageError::NoDataDirectory)?;
        Ok(Self::at(directory.join("apiforge").join(WORKSPACE_FILE)))
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn backup_path(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_owned();
        name.push(BACKUP_SUFFIX);
        self.path.with_file_name(name)
    }

    /// Reads the workspace, or `None` when nothing has been saved yet.
    ///
    /// When the main file is corrupt, the backup is used and the reason is reported
    /// in [`Loaded::recovered`]. When both are unusable, the main file's error is
    /// returned and no file is modified.
    pub fn load(&self) -> Result<Option<Loaded>, StorageError> {
        let main = match read_workspace(&self.path) {
            Ok(Some(workspace)) => {
                return Ok(Some(Loaded {
                    workspace,
                    recovered: None,
                }));
            }
            Ok(None) => None,
            Err(error) => Some(error),
        };
        match (main, read_workspace(&self.backup_path())) {
            (None, Ok(None)) => Ok(None),
            (main, Ok(Some(workspace))) => Ok(Some(Loaded {
                workspace,
                recovered: Some(main.map_or_else(
                    || "The workspace file was missing; its backup was restored.".to_owned(),
                    |error| format!("{error}. The last backup was restored."),
                )),
            })),
            (Some(error), _) => Err(error),
            (None, Err(error)) => Err(error),
        }
    }

    /// Saves the workspace atomically, keeping the previous file as a backup.
    ///
    /// The document is validated first, so storage never holds a workspace the
    /// importer would reject.
    pub fn save(&self, workspace: &Workspace) -> Result<(), StorageError> {
        let public = public_workspace(workspace);
        apiforge_core::validate_workspace(&public).map_err(|source| StorageError::Invalid {
            path: self.path.clone(),
            source,
        })?;
        let bytes = serde_json::to_vec_pretty(&public)?;
        if self.path.exists() {
            // Only a readable document becomes the backup; a corrupt main file must
            // not overwrite the last good copy.
            if read_workspace(&self.path).is_ok_and(|workspace| workspace.is_some()) {
                fs::copy(&self.path, self.backup_path())
                    .map_err(|source| StorageError::io(&self.backup_path(), source))?;
            }
        }
        write_atomic(&self.path, &bytes)
    }
}

/// A copy without secret variable values, safe to write to disk or export.
pub fn public_project(project: &Project) -> Project {
    let mut project = project.clone();
    let secrets = project.environment.secret_names.clone();
    project
        .environment
        .variables
        .retain(|name, _| !secrets.contains(name));
    project
}

fn public_workspace(workspace: &Workspace) -> Workspace {
    Workspace {
        version: workspace.version,
        active_id: workspace.active_id.clone(),
        projects: workspace.projects.iter().map(public_project).collect(),
    }
}

fn read_workspace(path: &Path) -> Result<Option<Workspace>, StorageError> {
    let limits = ImportLimits::default();
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(StorageError::io(path, error)),
    };
    let mut bytes = Vec::new();
    // Read one byte past the limit so an oversized file is reported, not truncated.
    file.take(limits.max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| StorageError::io(path, source))?;
    let document = import_document(&bytes, &limits).map_err(|source| StorageError::Invalid {
        path: path.to_owned(),
        source,
    })?;
    Ok(Some(document.into_workspace()))
}

/// Writes `bytes` to `path` through a synced temporary file in the same directory
/// and an atomic rename, creating the directory if needed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(directory).map_err(|source| StorageError::io(directory, source))?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)
        .map_err(|source| StorageError::io(directory, source))?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|source| StorageError::io(temporary.path(), source))?;
    temporary
        .persist(path)
        .map_err(|error| StorageError::io(path, error.error))?;
    sync_directory(directory);
    Ok(())
}

/// Makes the rename durable where the platform supports syncing directories.
fn sync_directory(directory: &Path) {
    #[cfg(unix)]
    if let Ok(handle) = File::open(directory) {
        // Best effort: the data itself is already synced, and a failure here only
        // widens the window in which a power loss could revert to the old file.
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = directory;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> Workspace {
        import_document(
            include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .expect("fixture imports")
        .into_workspace()
    }

    #[test]
    fn missing_storage_loads_as_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join("nested").join(WORKSPACE_FILE));
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn save_round_trips_and_keeps_a_backup() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let mut original = workspace();
        store.save(&original).unwrap();
        assert_eq!(store.load().unwrap().unwrap().workspace, original);
        assert!(!store.backup_path().exists());

        original.projects[0].name = "Renamed".into();
        store.save(&original).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.workspace.projects[0].name, "Renamed");
        assert!(loaded.recovered.is_none());
        assert!(store.backup_path().exists());
    }

    #[test]
    fn secret_values_are_never_written() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let mut workspace = workspace();
        let environment = &mut workspace.projects[0].environment;
        environment.secret_names.push("token".into());
        environment
            .variables
            .insert("token".into(), "canary-secret-value".into());
        store.save(&workspace).unwrap();
        let text = fs::read_to_string(store.path()).unwrap();
        assert!(!text.contains("canary-secret-value"));
        assert!(text.contains("\"token\""), "the secret name is kept");
    }

    #[test]
    fn a_corrupt_file_falls_back_to_the_backup_without_overwriting_it() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let workspace = workspace();
        store.save(&workspace).unwrap();
        store.save(&workspace).unwrap();
        fs::write(store.path(), b"{ truncated").unwrap();

        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.workspace, workspace);
        assert!(loaded.recovered.unwrap().contains("backup"));
        // Saving over the corrupt file must not replace the good backup with it.
        store.save(&workspace).unwrap();
        assert!(read_workspace(&store.backup_path()).unwrap().is_some());
    }

    #[test]
    fn invalid_workspaces_are_rejected_before_writing() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let mut workspace = workspace();
        workspace.active_id = "does-not-exist".into();
        assert!(matches!(
            store.save(&workspace),
            Err(StorageError::Invalid { .. })
        ));
        assert!(!store.path().exists());
    }

    #[test]
    fn unreadable_storage_without_backup_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        fs::write(store.path(), b"[]").unwrap();
        assert!(matches!(store.load(), Err(StorageError::Invalid { .. })));
        assert_eq!(fs::read(store.path()).unwrap(), b"[]");
    }
}
