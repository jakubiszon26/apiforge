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
    /// returned and no file is modified. Unsupported versions and oversized main
    /// files are preserved as errors, since falling back could discard newer data.
    pub fn load(&self) -> Result<Option<Loaded>, StorageError> {
        let main = match read_workspace(&self.path) {
            Ok(Some(workspace)) => {
                return Ok(Some(Loaded {
                    workspace,
                    recovered: None,
                }));
            }
            Ok(None) => None,
            // A newer or oversized document is not known to be corrupt. Using
            // an older backup could silently overwrite data we cannot interpret.
            Err(
                error @ StorageError::Invalid {
                    source: ImportError::UnsupportedVersion | ImportError::TooLarge { .. },
                    ..
                },
            ) => return Err(error),
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
        // Semantic validation alone does not enforce the importer's byte limit.
        // Never acknowledge a save that the next application launch cannot open.
        import_document(&bytes, &ImportLimits::default()).map_err(|source| {
            StorageError::Invalid {
                path: self.path.clone(),
                source,
            }
        })?;
        match read_workspace(&self.path) {
            // Only a readable document becomes the backup; a corrupt main file must
            // not overwrite the last good copy.
            Ok(Some(previous)) => {
                // Backups need the same atomic replacement and durability as the
                // main file; copying in place could destroy the only recovery copy.
                // Compact encoding avoids inflating an imported document beyond
                // the size limit while retaining it as a recovery workspace.
                let previous = serde_json::to_vec(&public_workspace(&previous))?;
                import_document(&previous, &ImportLimits::default()).map_err(|source| {
                    StorageError::Invalid {
                        path: self.backup_path(),
                        source,
                    }
                })?;
                write_atomic(&self.backup_path(), &previous)?;
            }
            Err(
                error @ StorageError::Invalid {
                    source: ImportError::UnsupportedVersion | ImportError::TooLarge { .. },
                    ..
                },
            ) => return Err(error),
            Ok(None) | Err(StorageError::Invalid { .. }) => {}
            // A corrupt document may be replaced after recovery, but an I/O
            // failure is not proof of corruption. Preserve the existing file.
            Err(error) => return Err(error),
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
    write_atomic_with_sync(path, bytes, sync_directory)
}

fn write_atomic_with_sync(
    path: &Path,
    bytes: &[u8],
    mut sync_directory: impl FnMut(&Path) -> io::Result<()>,
) -> Result<(), StorageError> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // A first save may create the application data directory. Flushing only that
    // directory would leave its name in the parent uncommitted after power loss.
    let mut created_parents = Vec::new();
    let mut candidate = Some(directory);
    while let Some(path) = candidate {
        match fs::metadata(path) {
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let parent = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty());
                created_parents.push(parent.unwrap_or(Path::new(".")).to_owned());
                candidate = parent;
            }
            Err(source) => return Err(StorageError::io(path, source)),
        }
    }
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
    // A failed directory flush means the replacement may not survive power loss.
    // The new document can already be visible, but it must not be reported saved.
    sync_directory(directory).map_err(|source| StorageError::io(directory, source))?;
    for parent in created_parents {
        sync_directory(&parent).map_err(|source| StorageError::io(&parent, source))?;
    }
    Ok(())
}

/// Makes the rename durable where the platform supports syncing directories.
fn sync_directory(directory: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(directory)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        // std does not expose a directory flush on Windows. The temporary file is
        // still flushed before replacement; power-loss durability is platform-bound.
        let _ = directory;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> Workspace {
        import_document(
            include_bytes!("../../../fixtures/workspace.json"),
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

    #[test]
    fn failed_backup_replacement_preserves_the_main_document() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let mut original = workspace();
        store.save(&original).unwrap();
        let saved = fs::read(store.path()).unwrap();
        // A directory cannot be replaced with a regular file, on either target OS.
        fs::create_dir(store.backup_path()).unwrap();
        original.projects[0].name = "Not committed".into();
        assert!(store.save(&original).is_err());
        assert_eq!(fs::read(store.path()).unwrap(), saved);
    }

    #[test]
    fn directory_flush_failure_is_not_acknowledged_as_a_successful_save() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        let result = write_atomic_with_sync(&path, b"new complete document", |_| {
            Err(io::Error::other("injected directory flush failure"))
        });
        assert!(matches!(result, Err(StorageError::Io { .. })));
        // Rename has happened before the flush fails: callers must treat the result
        // as unconfirmed, while the visible file must still be complete.
        assert_eq!(fs::read(path).unwrap(), b"new complete document");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_first_save_flushes_new_directory_entries_up_to_the_existing_parent() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("application").join("data");
        let mut flushed = Vec::new();
        write_atomic_with_sync(&nested.join("document.json"), b"complete", |path| {
            flushed.push(path.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            flushed,
            [
                nested,
                directory.path().join("application"),
                directory.path().to_owned()
            ]
        );
    }

    #[test]
    fn oversized_serialised_document_does_not_replace_a_reopenable_save() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let mut original = workspace();
        store.save(&original).unwrap();
        let saved = fs::read(store.path()).unwrap();
        // Each field is semantically valid, but their combined bytes exceed the
        // import limit. This used to save successfully and fail on the next launch.
        for index in 0..170 {
            original.projects[0]
                .environment
                .variables
                .insert(format!("large_{index}"), "x".repeat(100_000));
        }
        apiforge_core::validate_workspace(&original).unwrap();
        assert!(matches!(
            store.save(&original),
            Err(StorageError::Invalid { .. })
        ));
        assert_eq!(fs::read(store.path()).unwrap(), saved);
        assert!(!store.backup_path().exists());
    }

    #[test]
    fn a_missing_main_file_recovers_the_last_durable_backup() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let original = workspace();
        store.save(&original).unwrap();
        store.save(&original).unwrap();
        fs::remove_file(store.path()).unwrap();
        let recovered = store.load().unwrap().unwrap();
        assert_eq!(recovered.workspace, original);
        assert!(recovered.recovered.is_some());
    }

    #[test]
    fn newer_workspace_is_neither_replaced_by_backup_nor_overwritten_by_save() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let original = workspace();
        store.save(&original).unwrap();
        store.save(&original).unwrap();
        let backup = fs::read(store.backup_path()).unwrap();
        let mut newer = original.clone();
        newer.version = 2;
        let main = serde_json::to_vec(&newer).unwrap();
        fs::write(store.path(), &main).unwrap();
        assert!(matches!(
            store.load(),
            Err(StorageError::Invalid {
                source: ImportError::UnsupportedVersion,
                ..
            })
        ));
        assert!(matches!(
            store.save(&original),
            Err(StorageError::Invalid {
                source: ImportError::UnsupportedVersion,
                ..
            })
        ));
        assert_eq!(fs::read(store.path()).unwrap(), main);
        assert_eq!(fs::read(store.backup_path()).unwrap(), backup);
    }

    #[test]
    fn oversized_main_is_preserved_instead_of_silently_rolling_back_to_backup() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::at(directory.path().join(WORKSPACE_FILE));
        let original = workspace();
        store.save(&original).unwrap();
        store.save(&original).unwrap();
        let backup = fs::read(store.backup_path()).unwrap();
        let oversized = vec![b' '; ImportLimits::default().max_bytes + 1];
        fs::write(store.path(), &oversized).unwrap();
        assert!(matches!(
            store.load(),
            Err(StorageError::Invalid {
                source: ImportError::TooLarge { .. },
                ..
            })
        ));
        assert!(matches!(
            store.save(&original),
            Err(StorageError::Invalid {
                source: ImportError::TooLarge { .. },
                ..
            })
        ));
        assert_eq!(fs::read(store.path()).unwrap(), oversized);
        assert_eq!(fs::read(store.backup_path()).unwrap(), backup);
    }
}
