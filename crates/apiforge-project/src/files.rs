//! Explorer file operations (ADR 0011). Each operation is one storage
//! transaction outside text and model undo. Moves keep document IDs, deletion
//! goes through the local trash with every affected file and unsaved draft,
//! and nothing replaces an existing path.
use super::{EventKind, Origin, ProjectSession, document_from};
use crate::{Error, Result};
use apiforge_storage::project::{
    DiagramFile, FileState, MANIFEST_PATH, ProjectChange, ProjectError, ProjectManifest,
    ProjectPath, RecoveryDocument, RestoreDestination, TrashCommit, TrashIndex, TrashPreview,
};
use std::collections::BTreeSet;

/// What deleting explorer items would remove, for confirmation. Committing
/// requires that nothing it describes has changed since.
#[derive(Clone)]
pub struct DeletePreview {
    trash: TrashPreview,
    unsaved: Vec<String>,
    unsaved_texts: Vec<String>,
}

impl DeletePreview {
    /// Registered sources, by document.
    pub fn documents(&self) -> &[apiforge_storage::project::DocumentEntry] {
        self.trash.documents()
    }
    /// Other files, including those inside a deleted directory.
    pub fn files(&self) -> &[ProjectPath] {
        self.trash.files()
    }
    pub fn directories(&self) -> &[ProjectPath] {
        self.trash.directories()
    }
    /// Affected documents with unsaved work or a held draft. Their work moves to
    /// the trash with them and returns on restoration; it is not saved.
    pub fn unsaved_documents(&self) -> &[String] {
        &self.unsaved
    }
    /// Open text files with unsaved work among the deleted files. Their work
    /// stays in the window with a deletion conflict; the trash keeps the file.
    pub fn unsaved_texts(&self) -> &[String] {
        &self.unsaved_texts
    }
    /// Every loose file this deletion removes, including inside a directory.
    fn removed_files(&self) -> Vec<String> {
        let mut paths: Vec<String> = self
            .files()
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect();
        paths.extend(
            self.documents()
                .iter()
                .map(|entry| entry.path.as_str().to_owned()),
        );
        paths
    }
}

impl ProjectSession {
    /// Creates a directory and any missing parents.
    pub fn create_folder(&mut self, path: &str) -> Result<()> {
        let path = explorer_path(path)?;
        self.require_current_registry()?;
        if self.store.file_state(&path)? != FileState::Missing {
            return Err(occupied(self, &path));
        }
        let mut changes = self.parent_directories(&path)?;
        changes.push(ProjectChange::CreateDirectory { path });
        self.store.commit(changes)?;
        self.refresh_local_baselines();
        Ok(())
    }

    /// Renames or moves a file or directory. Registered documents keep their
    /// IDs, unsaved text and histories; declarations inside them are not
    /// renamed. A case-only rename passes through a unique temporary name, so
    /// it also works where names are compared without case.
    pub fn relocate(&mut self, from: &str, to: &str, origin: Origin) -> Result<Vec<String>> {
        let from = explorer_path(from)?;
        let to = explorer_path(to)?;
        if from == to {
            return Ok(Vec::new());
        }
        if to.as_str().starts_with(&format!("{}/", from.as_str())) {
            return Err(Error::Storage(ProjectError::InvalidPath));
        }
        let from_folded = from.as_str().to_lowercase();
        let to_folded = to.as_str().to_lowercase();
        // An absent file can still own unsaved text. Moving into that path (or
        // replacing its missing parent with a file) must not merge two editors.
        for open in self.texts.iter().map(|file| file.path()).chain(
            self.recovery
                .held_texts
                .iter()
                .map(|draft| draft.path.as_str()),
        ) {
            let path = open.to_lowercase();
            let affected = path == from_folded || path.starts_with(&format!("{from_folded}/"));
            if !affected
                && (path == to_folded
                    || path.starts_with(&format!("{to_folded}/"))
                    || to_folded.starts_with(&format!("{path}/")))
            {
                return Err(occupied(self, &to));
            }
        }
        // Validate the whole move first: a case-only rename takes two
        // transactions, and the first must not commit a move the second refuses.
        for entry in &self.manifest.documents {
            if let Some(rest) = within(&entry.path, &from) {
                ProjectPath::working_source(format!("{}{rest}", to.as_str()))
                    .map_err(|_| Error::RegisteredSourceExtension)?;
            }
        }
        if from.as_str().eq_ignore_ascii_case(to.as_str()) {
            let (parent, name) = split(&from);
            let extension = if name.ends_with(".afs") { ".afs" } else { "" };
            let temporary = ProjectPath::new(format!(
                "{parent}.rename-{}{extension}",
                uuid::Uuid::new_v4().simple()
            ))?;
            self.relocate_once(&from, &temporary, origin)?;
            return match self.relocate_once(&temporary, &to, origin) {
                Ok(ids) => Ok(ids),
                Err(error) => {
                    // Return to the original name; if that fails too, the
                    // temporary name is a valid, registered location.
                    let _ = self.relocate_once(&temporary, &from, origin);
                    Err(error)
                }
            };
        }
        self.relocate_once(&from, &to, origin)
    }

    fn relocate_once(
        &mut self,
        from: &ProjectPath,
        to: &ProjectPath,
        origin: Origin,
    ) -> Result<Vec<String>> {
        if self.store.is_read_only() {
            return Err(Error::Storage(ProjectError::ReadOnly));
        }
        self.require_current_registry()?;
        if self.store.file_state(to)? != FileState::Missing {
            return Err(occupied(self, to));
        }
        let (files, directories) = match self.store.file_state(from)? {
            FileState::Missing => return Err(occupied(self, from)),
            FileState::File { .. } => (vec![from.clone()], Vec::new()),
            FileState::Directory => self.store.list_directory(from)?,
        };
        let moved = |path: &ProjectPath| -> Result<ProjectPath> {
            Ok(ProjectPath::new(format!(
                "{}{}",
                to.as_str(),
                &path.as_str()[from.as_str().len()..]
            ))?)
        };
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        let mut paths = files.clone();
        paths.push(manifest_path.clone());
        let snapshot = self.store.read(&paths)?;
        // Registered sources inside the moved tree, including ones whose file
        // is missing: their registry path follows the move.
        let mut registry = self.saved_manifest(&snapshot.files[&manifest_path])?;
        let mut documents = Vec::new();
        for entry in &mut registry.documents {
            if &entry.path == from
                || entry
                    .path
                    .as_str()
                    .starts_with(&format!("{}/", from.as_str()))
            {
                entry.path = ProjectPath::working_source(moved(&entry.path)?.as_str().to_owned())
                    .map_err(|_| Error::RegisteredSourceExtension)?;
                documents.push((entry.document_id.clone(), entry.path.clone()));
            }
        }
        let mut changes = self.parent_directories(to)?;
        if !directories.is_empty() || files.first() != Some(from) {
            changes.push(ProjectChange::CreateDirectory { path: to.clone() });
            for directory in &directories {
                changes.push(ProjectChange::CreateDirectory {
                    path: moved(directory)?,
                });
            }
        }
        for file in &files {
            let bytes = snapshot.files[file]
                .clone()
                .ok_or_else(|| occupied(self, file))?;
            changes.push(ProjectChange::Write {
                path: moved(file)?,
                expected: FileState::Missing,
                bytes,
            });
        }
        for file in &files {
            changes.push(ProjectChange::RemoveFile {
                path: file.clone(),
                expected: snapshot.state(file).unwrap_or(FileState::Missing),
            });
        }
        let mut removed: Vec<_> = directories.iter().collect();
        removed.sort_by_key(|path| std::cmp::Reverse(path.as_str().matches('/').count()));
        for directory in removed {
            changes.push(ProjectChange::RemoveDirectory {
                path: directory.clone(),
            });
        }
        if files.first() != Some(from) {
            changes.push(ProjectChange::RemoveDirectory { path: from.clone() });
        }
        if !documents.is_empty() {
            changes.push(ProjectChange::Write {
                path: manifest_path,
                expected: self.manifest_saved.clone(),
                bytes: registry.to_public_bytes()?,
            });
        }
        let mut recovery = self.recovery_contents();
        for draft in &mut recovery.documents {
            if within(&draft.path, from).is_some() {
                draft.path = moved(&draft.path)?;
            }
        }
        for draft in &mut recovery.texts {
            if within(&draft.path, from).is_some() {
                draft.path = moved(&draft.path)?;
            }
        }
        let baseline = self
            .recovery
            .read
            .as_ref()
            .ok_or(Error::RecoveryUnavailable)?;
        self.recovery.read = Some(
            self.store
                .commit_with_source_recovery(changes, baseline, &recovery)?,
        );
        if !documents.is_empty() {
            self.manifest_saved = FileState::of(&registry.to_public_bytes()?);
            self.manifest.documents = registry.documents;
        }
        let mut text_paths: Vec<_> = self
            .texts
            .iter()
            .map(|file| file.path.clone())
            .chain(
                self.recovery
                    .held_texts
                    .iter()
                    .map(|draft| draft.path.clone()),
            )
            .filter(|path| within(path, from).is_some())
            .collect();
        text_paths.sort();
        text_paths.dedup();
        for file in text_paths {
            self.text_file_moved(file.as_str(), moved(&file)?.as_str());
        }
        let mut ids = Vec::new();
        for (document_id, path) in documents {
            if let Ok(index) = self.index(&document_id) {
                self.documents[index].entry.path = path.clone();
            }
            for draft in &mut self.recovery.held {
                if draft.document_id == document_id {
                    draft.path = path.clone();
                }
            }
            self.emit(origin, Some(&document_id), EventKind::DocumentMoved);
            ids.push(document_id);
        }
        self.refresh_local_baselines();
        Ok(ids)
    }

    /// Lists what deleting these explorer items would remove.
    pub fn preview_delete(&self, paths: &[&str]) -> Result<DeletePreview> {
        let paths = paths
            .iter()
            .map(|path| explorer_path(path))
            .collect::<Result<Vec<_>>>()?;
        let trash = self.store.preview_trash_paths(&paths)?;
        let unsaved = trash
            .documents()
            .iter()
            .filter(|entry| {
                self.document(&entry.document_id)
                    .is_some_and(|document| document.needs_recovery())
                    || self
                        .recovery
                        .held
                        .iter()
                        .any(|draft| draft.document_id == entry.document_id)
            })
            .map(|entry| entry.document_id.clone())
            .collect();
        let unsaved_texts = self
            .texts
            .iter()
            .filter(|file| {
                file.needs_recovery()
                    && (trash.files().iter().any(|path| path == &file.path)
                        || trash.directories().iter().any(|directory| {
                            file.path
                                .as_str()
                                .starts_with(&format!("{}/", directory.as_str()))
                        }))
            })
            .map(|file| file.path.as_str().to_owned())
            .collect();
        Ok(DeletePreview {
            trash,
            unsaved,
            unsaved_texts,
        })
    }

    /// Moves the previewed items to the local trash in one transaction, with
    /// the current unsaved work of every affected document.
    pub fn delete(&mut self, preview: &DeletePreview, origin: Origin) -> Result<TrashCommit> {
        let ids: BTreeSet<_> = preview
            .documents()
            .iter()
            .map(|entry| entry.document_id.clone())
            .collect();
        self.require_current_registry()?;
        self.check_saved_manifest()?;
        let mut drafts: Vec<RecoveryDocument> = Vec::new();
        for draft in self.recovery_contents().documents {
            if ids.contains(&draft.document_id)
                && !drafts
                    .iter()
                    .any(|existing| existing.document_id == draft.document_id)
            {
                drafts.push(draft);
            }
        }
        let commit = self.store.trash_documents(&preview.trash, &drafts)?;
        self.adopt_saved_diagram()?;
        if !ids.is_empty() {
            self.documents
                .retain(|document| !ids.contains(document.id()));
            self.recovery
                .held
                .retain(|draft| !ids.contains(&draft.document_id));
            self.manifest
                .documents
                .retain(|entry| !ids.contains(&entry.document_id));
            self.adopt_saved_manifest()?;
            for id in &ids {
                self.emit(origin, Some(id), EventKind::DocumentRemoved);
            }
        }
        self.text_files_removed(&preview.removed_files());
        self.adopt_recovery_baseline();
        self.refresh_local_baselines();
        Ok(commit)
    }

    /// Every readable trash operation, most recent first.
    pub fn trash_operations(&self) -> Result<Vec<TrashIndex>> {
        Ok(self.store.trash_operations()?)
    }

    /// Restores a trash operation. Documents return with their IDs; their
    /// unsaved work returns as unsaved (or as a held draft when the restored
    /// source differs from its base). An occupied path needs a destination.
    pub fn restore(
        &mut self,
        operation_id: &str,
        destinations: &[RestoreDestination],
    ) -> Result<Vec<String>> {
        self.require_current_registry()?;
        self.check_saved_manifest()?;
        let preview = self.store.preview_trash_restore(operation_id)?;
        let ids: Vec<_> = preview
            .index()
            .entries
            .iter()
            .map(|entry| entry.document.document_id.clone())
            .collect();
        if ids.iter().any(|id| self.document(id).is_some()) {
            return Err(Error::Storage(ProjectError::InvalidTrash));
        }
        self.store.restore_trash(&preview, destinations)?;
        // Before the restored documents are analysed, so that they reconcile
        // against the checkpoints the restore put back.
        self.adopt_saved_diagram()?;
        if ids.is_empty() {
            self.refresh_local_baselines();
            return Ok(ids);
        }
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        let saved = self.read_manifest()?;
        let entries: Vec<_> = saved
            .documents
            .iter()
            .filter(|entry| ids.contains(&entry.document_id))
            .cloned()
            .collect();
        let mut paths: Vec<_> = entries.iter().map(|entry| entry.path.clone()).collect();
        paths.push(manifest_path);
        let snapshot = self.store.read(&paths)?;
        // Keep unsaved configuration; only the registry's documents change.
        self.manifest.documents = saved.documents.clone();
        self.adopt_saved_manifest()?;
        self.adopt_recovery_baseline();
        self.refresh_local_baselines();
        let drafts: Vec<_> = self
            .recovery
            .read
            .as_ref()
            .and_then(|read| read.recovery())
            .map(|recovery| {
                recovery
                    .documents
                    .iter()
                    .filter(|draft| ids.contains(&draft.document_id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        for entry in entries {
            let id = entry.document_id.clone();
            self.documents.push(document_from(&snapshot, entry));
            self.emit(Origin::SESSION, Some(&id), EventKind::DocumentRegistered);
            let index = self.documents.len() - 1;
            self.analyse_now(index)?;
        }
        for draft in drafts {
            self.adopt_draft(draft)?;
        }
        Ok(ids)
    }

    /// The registry as saved, failing when it changed outside this session.
    pub(super) fn saved_manifest(&self, bytes: &Option<Vec<u8>>) -> Result<ProjectManifest> {
        let bytes = bytes.as_deref().ok_or(ProjectError::InvalidMetadata)?;
        if FileState::of(bytes) != self.manifest_saved {
            return Err(Error::Storage(ProjectError::Conflict {
                path: self.store.root().join(MANIFEST_PATH),
            }));
        }
        Ok(ProjectManifest::from_bytes(bytes)?)
    }

    pub(super) fn check_saved_manifest(&self) -> Result<()> {
        let path = ProjectPath::new(MANIFEST_PATH)?;
        let snapshot = self.store.read(std::slice::from_ref(&path))?;
        self.saved_manifest(&snapshot.files[&path]).map(|_| ())
    }

    pub(super) fn read_manifest(&self) -> Result<ProjectManifest> {
        let path = ProjectPath::new(MANIFEST_PATH)?;
        let snapshot = self.store.read(std::slice::from_ref(&path))?;
        let bytes = snapshot.files[&path]
            .as_deref()
            .ok_or(ProjectError::InvalidMetadata)?;
        Ok(ProjectManifest::from_bytes(bytes)?)
    }

    /// Adopts the registry this session just committed as the save baseline.
    fn adopt_saved_manifest(&mut self) -> Result<()> {
        let path = ProjectPath::new(MANIFEST_PATH)?;
        let snapshot = self.store.read(std::slice::from_ref(&path))?;
        self.manifest_saved = snapshot.state(&path).unwrap_or(FileState::Missing);
        Ok(())
    }

    /// Adopts the shared checkpoint file a trash transaction just rewrote.
    /// Trash operations carry the checkpoints of the documents they move, so a
    /// session holding its earlier copy would both reject its own next source
    /// save as a foreign change and reconcile a restored document against a
    /// record the restore had already put back.
    pub(super) fn adopt_saved_diagram(&mut self) -> Result<()> {
        let path = ProjectPath::new(super::DIAGRAM_PATH)?;
        let snapshot = self.store.read(std::slice::from_ref(&path))?;
        self.diagram_saved = snapshot.state(&path).unwrap_or(FileState::Missing);
        self.diagram = match snapshot.files.get(&path).and_then(Option::as_deref) {
            Some(bytes) => DiagramFile::from_bytes(bytes).ok(),
            None => Some(DiagramFile {
                format_version: 1,
                records: Vec::new(),
            }),
        };
        Ok(())
    }
}

/// A path the explorer may operate on: portable, outside the hidden control
/// directories and not the registry itself.
fn explorer_path(path: &str) -> Result<ProjectPath> {
    let path = ProjectPath::new(path)?;
    if path.as_str() == MANIFEST_PATH
        || path
            .as_str()
            .split('/')
            .any(|part| part.eq_ignore_ascii_case(".apiforge") || part.eq_ignore_ascii_case(".git"))
    {
        return Err(Error::Storage(ProjectError::InvalidPath));
    }
    Ok(path)
}

/// The rest of `path` below `root` (empty for `root` itself), if inside it.
fn within<'a>(path: &'a ProjectPath, root: &ProjectPath) -> Option<&'a str> {
    let rest = path.as_str().strip_prefix(root.as_str())?;
    (rest.is_empty() || rest.starts_with('/')).then_some(rest)
}

fn split(path: &ProjectPath) -> (String, &str) {
    match path.as_str().rsplit_once('/') {
        Some((parent, name)) => (format!("{parent}/"), name),
        None => (String::new(), path.as_str()),
    }
}

fn occupied(session: &ProjectSession, path: &ProjectPath) -> Error {
    Error::Storage(ProjectError::Conflict {
        path: session.store.root().join(path.as_str()),
    })
}
