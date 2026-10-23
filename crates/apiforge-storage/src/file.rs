//! Durable local storage of the workspace and atomic JSON file writes.
//!
//! The workspace is stored as a version-two workspace document, the same format as a
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

#[cfg(test)]
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
    /// Set for a backup candidate. The session requires explicit recovery before
    /// this candidate can replace the primary document.
    pub recovered: Option<String>,
}

/// The workspace file of one user.
#[derive(Clone, Debug)]
pub struct WorkspaceFile {
    path: PathBuf,
}

impl WorkspaceFile {
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
                    source:
                        ImportError::LegacyVersion
                        | ImportError::UnsupportedVersion
                        | ImportError::TooLarge { .. },
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
                    || "The workspace file is missing; a valid backup is available.".to_owned(),
                    |error| format!("{error}. A valid backup is available."),
                )),
            })),
            (Some(error), _) => Err(error),
            (None, Err(error)) => Err(error),
        }
    }

    /// Preserves a version-one workspace and its backup under `workspace.v1.json`
    /// and `workspace.v1.json.bak` (ADR 0004), allowing a fresh v2 workspace.
    /// Returns false for a missing or supported workspace, without changing it.
    ///
    /// The archive names must be unused. Hard links reserve them without replacing
    /// existing archives, and both copies are made durable before either original
    /// name is removed. On a filesystem without hard-link support this fails safely;
    /// the desktop then opens a temporary session with autosave disabled.
    pub fn archive_legacy_workspace(&self) -> Result<bool, StorageError> {
        match read_workspace(&self.path) {
            Err(StorageError::Invalid {
                source: ImportError::LegacyVersion,
                ..
            }) => {}
            Ok(_) => return Ok(false),
            Err(error) => return Err(error),
        }
        let archive = self.path.with_extension("v1.json");
        let backup = self.backup_path();
        let archive_backup = WorkspaceFile::at(&archive).backup_path();
        // Check the backup name before reserving anything. A failure must keep
        // the main file, its backup and any previous archive untouched.
        let has_backup = match fs::symlink_metadata(&backup) {
            Ok(_) => true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(StorageError::io(&backup, error)),
        };
        fs::hard_link(&self.path, &archive).map_err(|source| StorageError::io(&archive, source))?;
        if has_backup && let Err(source) = fs::hard_link(&backup, &archive_backup) {
            // Originals still exist; remove only the link created by this call.
            let _ = fs::remove_file(&archive);
            return Err(StorageError::io(&archive_backup, source));
        }
        let directory = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        // Windows requires a writable handle for FlushFileBuffers. Opening with
        // write access here does not truncate or otherwise change the bytes.
        File::options()
            .read(true)
            .write(true)
            .open(&archive)
            .and_then(|file| file.sync_all())
            .map_err(|source| StorageError::io(&archive, source))?;
        if has_backup {
            File::options()
                .read(true)
                .write(true)
                .open(&archive_backup)
                .and_then(|file| file.sync_all())
                .map_err(|source| StorageError::io(&archive_backup, source))?;
        }
        sync_directory(directory).map_err(|source| StorageError::io(directory, source))?;
        if has_backup {
            fs::remove_file(&backup).map_err(|source| StorageError::io(&backup, source))?;
        }
        fs::remove_file(&self.path).map_err(|source| StorageError::io(&self.path, source))?;
        sync_directory(directory).map_err(|source| StorageError::io(directory, source))?;
        Ok(true)
    }

    /// Saves the workspace atomically, keeping the previous file as a backup.
    ///
    /// The document is validated first, so storage never holds a workspace the
    /// importer would reject.
    pub fn save(&self, workspace: &Workspace) -> Result<(), StorageError> {
        self.save_envelope(workspace, false)
    }

    /// Keeps a standalone project file standalone, including its backup.
    pub fn save_project(&self, project: &Project) -> Result<(), StorageError> {
        self.save_envelope(
            &Workspace {
                version: project.version,
                active_id: project.id.clone(),
                projects: vec![project.clone()],
            },
            true,
        )
    }

    fn save_envelope(&self, workspace: &Workspace, standalone: bool) -> Result<(), StorageError> {
        let public = public_workspace(workspace);
        apiforge_core::validate_workspace(&public).map_err(|source| StorageError::Invalid {
            path: self.path.clone(),
            source,
        })?;
        let bytes = if standalone {
            serde_json::to_vec_pretty(&public.projects[0])?
        } else {
            serde_json::to_vec_pretty(&public)?
        };
        // Semantic validation alone does not enforce the importer's byte limit.
        // Never acknowledge a save that the next application launch cannot open.
        import_document(&bytes, &ImportLimits::default()).map_err(|source| {
            StorageError::Invalid {
                path: self.path.clone(),
                source,
            }
        })?;
        match read_document(&self.path) {
            // Only a readable document becomes the backup; a corrupt main file must
            // not overwrite the last good copy.
            Ok(Some(previous)) => {
                // Backups need the same atomic replacement and durability as the
                // main file; copying in place could destroy the only recovery copy.
                // Compact encoding avoids inflating an imported document beyond
                // the size limit while retaining it as a recovery workspace.
                let public = public_workspace(previous.workspace());
                let previous = if previous.is_standalone_project() {
                    serde_json::to_vec(&public.projects[0])?
                } else {
                    serde_json::to_vec(&public)?
                };
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
                    source:
                        ImportError::LegacyVersion
                        | ImportError::UnsupportedVersion
                        | ImportError::TooLarge { .. },
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
    Ok(read_document(path)?.map(|doc| doc.into_workspace()))
}

fn read_document(path: &Path) -> Result<Option<apiforge_core::ValidatedDocument>, StorageError> {
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
    Ok(Some(document))
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
        let store = WorkspaceFile::at(directory.path().join("nested").join(WORKSPACE_FILE));
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn save_round_trips_and_keeps_a_backup() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
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
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
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
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
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
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
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
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
        fs::write(store.path(), b"[]").unwrap();
        assert!(matches!(store.load(), Err(StorageError::Invalid { .. })));
        assert_eq!(fs::read(store.path()).unwrap(), b"[]");
    }

    #[test]
    fn failed_backup_replacement_preserves_the_main_document() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
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
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
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
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
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
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
        let original = workspace();
        store.save(&original).unwrap();
        store.save(&original).unwrap();
        let backup = fs::read(store.backup_path()).unwrap();
        let mut newer = original.clone();
        newer.version = apiforge_core::format::FORMAT_VERSION + 1;
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
    fn legacy_workspace_is_neither_replaced_by_backup_nor_overwritten_by_save() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
        let original = workspace();
        store.save(&original).unwrap();
        store.save(&original).unwrap();
        let backup = fs::read(store.backup_path()).unwrap();
        let legacy = include_bytes!("../../../tools/convert-v1/sources/workspace-v1.json");
        fs::write(store.path(), legacy).unwrap();
        assert!(matches!(
            store.load(),
            Err(StorageError::Invalid {
                source: ImportError::LegacyVersion,
                ..
            })
        ));
        assert!(matches!(
            store.save(&original),
            Err(StorageError::Invalid {
                source: ImportError::LegacyVersion,
                ..
            })
        ));
        assert_eq!(fs::read(store.path()).unwrap(), legacy);
        assert_eq!(fs::read(store.backup_path()).unwrap(), backup);
    }

    #[test]
    fn legacy_files_are_archived_byte_for_byte_before_a_fresh_save() {
        for with_backup in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
            let archive = directory.path().join("workspace.v1.json");
            let archive_backup = directory.path().join("workspace.v1.json.bak");
            let legacy = include_bytes!("../../../tools/convert-v1/sources/workspace-v1.json");
            fs::write(store.path(), legacy).unwrap();
            // Preserve even a corrupt old backup as evidence for recovery.
            let backup = b"{ old backup, incomplete";
            if with_backup {
                fs::write(store.backup_path(), backup).unwrap();
            }
            assert!(store.archive_legacy_workspace().unwrap());
            assert!(!store.path().exists());
            assert!(!store.backup_path().exists());
            assert_eq!(fs::read(&archive).unwrap(), legacy);
            if with_backup {
                assert_eq!(fs::read(&archive_backup).unwrap(), backup);
            } else {
                assert!(!archive_backup.exists());
            }
            let fresh = workspace();
            store.save(&fresh).unwrap();
            store.save(&fresh).unwrap();
            assert_eq!(store.load().unwrap().unwrap().workspace, fresh);
            assert_eq!(fs::read(&archive).unwrap(), legacy);
            if with_backup {
                assert_eq!(fs::read(&archive_backup).unwrap(), backup);
            }
        }
    }

    #[test]
    fn an_existing_legacy_archive_preserves_all_original_files() {
        for conflict in ["workspace.v1.json", "workspace.v1.json.bak"] {
            let directory = tempfile::tempdir().unwrap();
            let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
            let legacy = include_bytes!("../../../tools/convert-v1/sources/workspace-v1.json");
            fs::write(store.path(), legacy).unwrap();
            fs::write(store.backup_path(), b"original backup").unwrap();
            let existing = directory.path().join(conflict);
            fs::write(&existing, b"earlier archive").unwrap();
            assert!(store.archive_legacy_workspace().is_err());
            assert_eq!(fs::read(store.path()).unwrap(), legacy);
            assert_eq!(fs::read(store.backup_path()).unwrap(), b"original backup");
            assert_eq!(fs::read(existing).unwrap(), b"earlier archive");
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
        }
    }

    #[test]
    fn archiving_does_not_change_missing_current_newer_or_corrupt_workspaces() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
        assert!(!store.archive_legacy_workspace().unwrap());
        store.save(&workspace()).unwrap();
        let current = fs::read(store.path()).unwrap();
        assert!(!store.archive_legacy_workspace().unwrap());
        assert_eq!(fs::read(store.path()).unwrap(), current);
        for bytes in [b"{ truncated".as_slice(), br#"{"version":3}"#] {
            fs::write(store.path(), bytes).unwrap();
            assert!(store.archive_legacy_workspace().is_err());
            assert_eq!(fs::read(store.path()).unwrap(), bytes);
        }
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn oversized_main_is_preserved_instead_of_silently_rolling_back_to_backup() {
        let directory = tempfile::tempdir().unwrap();
        let store = WorkspaceFile::at(directory.path().join(WORKSPACE_FILE));
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
