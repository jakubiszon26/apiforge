//! Project text files: files such as `.gitignore` or `README.md`, edited as
//! exact text beside the project's definitions.
//!
//! A text file is not a definition. It has no entity, no model, no analysis and
//! no checkpoint, it is never lowered and it cannot be run. Its identity is its
//! project-relative path, so a registry entry is neither needed nor written;
//! the explorer's own moves carry an open file's path and unsaved work with
//! them, while a change made outside API Forge is a change of that path's file.
//!
//! Managed project data is out of reach: `ProjectPath` refuses `.git` and every
//! `.apiforge` path a caller could otherwise address, and a registered `.afs`
//! document opens as a document instead. Saving writes the exact bytes in the
//! editor through one ordinary project transaction, so an external change is
//! detected rather than overwritten.

use super::{EventKind, Origin, ProjectSession};
use crate::{Error, Result, session::ExternalConflict};
use apiforge_storage::project::{
    FileState, MAX_SOURCE_BYTES, ProjectChange, ProjectPath, RecoveryText, TextSelection,
};

/// One open project text file.
#[derive(Clone, Debug)]
pub struct TextFileSession {
    pub(crate) path: ProjectPath,
    pub(crate) text: String,
    pub(crate) version: u64,
    /// The disk state this session last read or wrote: the save baseline.
    pub(crate) saved: FileState,
    /// The text of that baseline; `None` when the file is not on disk.
    pub(crate) saved_text: Option<String>,
    pub(crate) selection: TextSelection,
    pub(crate) conflict: Option<ExternalConflict>,
    /// Restored unsaved work whose baseline is not the current file: it needs
    /// an explicit keep or discard and blocks saving until then.
    pub(crate) held: bool,
}

impl TextFileSession {
    pub fn path(&self) -> &str {
        self.path.as_str()
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn selection(&self) -> &TextSelection {
        &self.selection
    }
    pub fn conflict(&self) -> Option<ExternalConflict> {
        self.conflict
    }
    pub fn is_held(&self) -> bool {
        self.held
    }
    /// The file is not on disk: deleted outside API Forge, or kept only as
    /// unsaved work that the next save recreates.
    pub fn is_missing(&self) -> bool {
        self.saved == FileState::Missing
    }
    pub fn is_dirty(&self) -> bool {
        self.saved_text.as_deref() != Some(self.text.as_str())
    }
    pub fn needs_recovery(&self) -> bool {
        self.is_dirty() || self.held
    }

    fn recovery(&self) -> RecoveryText {
        RecoveryText {
            path: self.path.clone(),
            base_hash: match &self.saved {
                FileState::File { sha256, .. } => Some(sha256.clone()),
                _ => None,
            },
            text: self.text.clone(),
            selection: if self.text.is_char_boundary(self.selection.anchor)
                && self.text.is_char_boundary(self.selection.caret)
            {
                self.selection.clone()
            } else {
                TextSelection {
                    anchor: 0,
                    caret: 0,
                }
            },
        }
    }
}

impl ProjectSession {
    pub fn text_files(&self) -> &[TextFileSession] {
        &self.texts
    }

    pub fn text_file(&self, path: &str) -> Option<&TextFileSession> {
        self.texts.iter().find(|file| file.path.as_str() == path)
    }

    fn text_index(&self, path: &str) -> Result<usize> {
        self.texts
            .iter()
            .position(|file| file.path.as_str() == path)
            .ok_or(Error::UnknownTextFile)
    }

    /// Opens a project file as text, or returns the one already open. A
    /// registered document is not a text file; nor is a directory, a file that
    /// is not UTF-8, or one over the 1 MiB limit.
    pub fn open_text_file(&mut self, path: &str) -> Result<&TextFileSession> {
        let path = ProjectPath::new(path)?;
        if let Ok(index) = self.text_index(path.as_str()) {
            return Ok(&self.texts[index]);
        }
        if self
            .documents
            .iter()
            .any(|document| document.entry.path == path)
        {
            return Err(Error::RegisteredDocument);
        }
        // `ProjectPath` permits the managed files it owns, such as the shared
        // checkpoint. Those are not project text: editing them raw would need
        // the validated reload policy that Stage II does not have.
        if path.as_str().split('/').any(|part| part == ".apiforge") {
            return Err(Error::ManagedData);
        }
        match self.store.file_state(&path)? {
            FileState::Directory => {
                return Err(Error::NotText(self.store.root().join(path.as_str())));
            }
            FileState::File { bytes, .. } if bytes > MAX_SOURCE_BYTES as u64 => {
                return Err(Error::SourceTooLarge);
            }
            _ => {}
        }
        let bytes = self
            .store
            .read(std::slice::from_ref(&path))?
            .files
            .remove(&path)
            .flatten();
        let text = match &bytes {
            Some(bytes) => Some(
                String::from_utf8(bytes.clone())
                    .map_err(|_| Error::NotText(self.store.root().join(path.as_str())))?,
            ),
            None => None,
        };
        // The file may have been replaced since the state was taken.
        let saved = bytes.as_deref().map_or(FileState::Missing, FileState::of);
        self.texts.push(TextFileSession {
            text: text.clone().unwrap_or_default(),
            version: 1,
            saved,
            saved_text: text,
            selection: TextSelection {
                anchor: 0,
                caret: 0,
            },
            conflict: None,
            held: false,
            path: path.clone(),
        });
        self.emit(
            Origin::SESSION,
            None,
            EventKind::TextFileOpened {
                path: path.as_str().to_owned(),
            },
        );
        self.restore_text_draft(path.as_str());
        Ok(self.text_file(path.as_str()).expect("just opened"))
    }

    /// Replaces an open text file's content with exactly this text.
    pub fn edit_text(&mut self, path: &str, text: &str, origin: Origin) -> Result<u64> {
        let index = self.text_index(path)?;
        if self.texts[index].held {
            return Err(Error::DraftDecisionRequired);
        }
        if text.len() > MAX_SOURCE_BYTES {
            return Err(Error::SourceTooLarge);
        }
        if self.texts[index].text == text {
            return Ok(self.texts[index].version);
        }
        self.texts[index].text = text.to_owned();
        self.texts[index].version += 1;
        let version = self.texts[index].version;
        self.emit(
            origin,
            None,
            EventKind::TextFileChanged {
                path: path.to_owned(),
                version,
            },
        );
        Ok(version)
    }

    pub fn set_text_selection(&mut self, path: &str, selection: TextSelection) -> Result<()> {
        let index = self.text_index(path)?;
        self.texts[index].selection = selection;
        Ok(())
    }

    /// Writes exactly the text in the editor. The expected disk state is the
    /// baseline this session read or last wrote, so a foreign change is
    /// reported instead of overwritten.
    pub fn save_text_file(&mut self, path: &str) -> Result<Option<u64>> {
        let index = self.text_index(path)?;
        if self.texts[index].held {
            return Err(Error::DraftDecisionRequired);
        }
        if self.texts[index].conflict.is_some() {
            return Err(Error::ExternalConflict);
        }
        self.require_current_registry()?;
        if !self.texts[index].is_dirty() {
            return Ok(None);
        }
        let file = &self.texts[index];
        let bytes = file.text.as_bytes().to_vec();
        let generation = self.store.commit(vec![ProjectChange::Write {
            path: file.path.clone(),
            expected: file.saved.clone(),
            bytes: bytes.clone(),
        }])?;
        self.refresh_local_baselines();
        let file = &mut self.texts[index];
        file.saved = FileState::of(&bytes);
        file.saved_text = Some(file.text.clone());
        self.emit(
            Origin::SESSION,
            None,
            EventKind::TextFileSaved {
                path: path.to_owned(),
                generation,
            },
        );
        // The saved draft is obsolete; an explicit recovery write reports any
        // failure, as it does for documents.
        let _ = self.save_recovery();
        Ok(Some(generation))
    }

    /// Returns the file to its saved content, reading the file again. An
    /// external change is refused: accept or export it deliberately instead.
    pub fn revert_text_file(&mut self, path: &str, origin: Origin) -> Result<()> {
        let index = self.text_index(path)?;
        let state = self.store.file_state(&self.texts[index].path)?;
        if state != self.texts[index].saved && self.texts[index].conflict.is_none() {
            return Err(Error::ExternalConflict);
        }
        self.reread_text(index, origin)
    }

    /// Replaces the editor's content with the file on disk, whatever it is now.
    fn reread_text(&mut self, index: usize, origin: Origin) -> Result<()> {
        let project_path = self.texts[index].path.clone();
        let path = project_path.as_str().to_owned();
        let bytes = self
            .store
            .read(std::slice::from_ref(&project_path))?
            .files
            .remove(&project_path)
            .flatten();
        let state = bytes.as_deref().map_or(FileState::Missing, FileState::of);
        let text = match bytes {
            Some(bytes) => Some(
                String::from_utf8(bytes)
                    .map_err(|_| Error::NotText(self.store.root().join(project_path.as_str())))?,
            ),
            None => None,
        };
        let file = &mut self.texts[index];
        file.saved = state;
        file.saved_text = text.clone();
        file.text = text.unwrap_or_default();
        file.version += 1;
        file.conflict = None;
        file.held = false;
        file.selection = TextSelection {
            anchor: 0,
            caret: 0,
        };
        let version = file.version;
        self.emit(
            origin,
            None,
            EventKind::TextFileChanged {
                path: path.clone(),
                version,
            },
        );
        self.drop_text_draft(&path);
        Ok(())
    }

    /// Closes a text file. Unsaved work must be saved or reverted first: a
    /// close that silently dropped it would lose user work.
    pub fn close_text_file(&mut self, path: &str) -> Result<()> {
        let index = self.text_index(path)?;
        if self.texts[index].needs_recovery() {
            return Err(Error::UnsavedTextFile);
        }
        self.texts.remove(index);
        self.emit(
            Origin::SESSION,
            None,
            EventKind::TextFileClosed {
                path: path.to_owned(),
            },
        );
        self.drop_text_draft(path);
        Ok(())
    }

    /// Keeps restored unsaved work: it becomes this session's current text.
    pub fn keep_text_draft(&mut self, path: &str) -> Result<()> {
        let index = self.text_index(path)?;
        self.texts[index].held = false;
        Ok(())
    }

    /// Discards restored unsaved work and returns to the file on disk.
    pub fn discard_text_draft(&mut self, path: &str) -> Result<()> {
        let index = self.text_index(path)?;
        if !self.texts[index].held {
            return Ok(());
        }
        self.texts[index].held = false;
        self.reread_text(index, Origin::SESSION)
    }

    /// Replaces unsaved work with the file on disk, resolving a conflict.
    pub fn accept_external_text(&mut self, path: &str, origin: Origin) -> Result<()> {
        let index = self.text_index(path)?;
        self.reread_text(index, origin)
    }

    /// Keeps the local text of a file deleted outside API Forge; the next save
    /// recreates it.
    pub fn keep_deleted_text(&mut self, path: &str) -> Result<()> {
        let index = self.text_index(path)?;
        let file = &mut self.texts[index];
        if file.conflict != Some(ExternalConflict::Deleted) {
            return Err(Error::UnknownTextFile);
        }
        file.conflict = None;
        file.saved = FileState::Missing;
        file.saved_text = None;
        Ok(())
    }

    pub(crate) fn text_recovery(&self) -> Vec<RecoveryText> {
        let mut texts = self.recovery.held_texts.clone();
        texts.extend(
            self.texts
                .iter()
                .filter(|file| file.needs_recovery())
                .map(TextFileSession::recovery),
        );
        texts
    }

    pub(crate) fn has_unsaved_text(&self) -> bool {
        !self.recovery.held_texts.is_empty()
            || self.texts.iter().any(TextFileSession::needs_recovery)
    }

    /// A draft of a file that is open: restored when its baseline is still the
    /// file on disk, otherwise held for an explicit decision.
    fn restore_text_draft(&mut self, path: &str) {
        let Some(position) = self
            .recovery
            .held_texts
            .iter()
            .position(|draft| draft.path.as_str() == path)
        else {
            return;
        };
        let draft = self.recovery.held_texts.remove(position);
        let Ok(index) = self.text_index(path) else {
            self.recovery.held_texts.insert(position, draft);
            return;
        };
        let file = &mut self.texts[index];
        let matches = draft.matches_base(&file.saved);
        file.text = draft.text;
        file.selection = draft.selection;
        file.held = !matches;
        file.version += 1;
        let version = file.version;
        self.emit(
            Origin::SESSION,
            None,
            EventKind::TextFileChanged {
                path: path.to_owned(),
                version,
            },
        );
    }

    fn drop_text_draft(&mut self, path: &str) {
        self.recovery
            .held_texts
            .retain(|draft| draft.path.as_str() != path);
    }

    /// The explorer moved a file that is open as text: the open editor follows
    /// it, keeping unsaved work. A read-only or failed move never gets here.
    pub(crate) fn text_file_moved(&mut self, from: &str, to: &str) {
        let Ok(path) = ProjectPath::new(to) else {
            return;
        };
        if let Ok(index) = self.text_index(from) {
            self.texts[index].path = path.clone();
        }
        for draft in &mut self.recovery.held_texts {
            if draft.path.as_str() == from {
                draft.path = path.clone();
            }
        }
    }

    /// A file open as text left the folder through API Forge itself. Clean
    /// files close; unsaved work stays with a deletion conflict.
    pub(crate) fn text_files_removed(&mut self, paths: &[String]) {
        self.texts.retain(|file| {
            !paths.iter().any(|path| path == file.path.as_str()) || file.needs_recovery()
        });
        for index in 0..self.texts.len() {
            if paths
                .iter()
                .any(|path| path == self.texts[index].path.as_str())
            {
                self.texts[index].conflict = Some(ExternalConflict::Deleted);
            }
        }
        for path in paths {
            self.drop_text_draft(path);
        }
    }

    /// Follows text files after an external check: a clean file takes the disk
    /// content, unsaved work gets a conflict and is never merged.
    pub(crate) fn apply_external_texts(
        &mut self,
        observed: Vec<(String, FileState, Option<Vec<u8>>)>,
    ) -> Vec<String> {
        let mut conflicts = Vec::new();
        for (path, base, bytes) in observed {
            let Ok(index) = self.text_index(&path) else {
                continue;
            };
            if self.texts[index].saved != base {
                // Saved by this session after the check began.
                continue;
            }
            let state = bytes.as_deref().map_or(FileState::Missing, FileState::of);
            if self.texts[index].needs_recovery() {
                let conflict = match state {
                    FileState::Missing => ExternalConflict::Deleted,
                    _ => ExternalConflict::Changed,
                };
                if self.texts[index].conflict != Some(conflict) {
                    self.texts[index].conflict = Some(conflict);
                    conflicts.push(path);
                }
                continue;
            }
            // Non-text content is not forced into the editor; the file keeps
            // its baseline and the next check reports it again.
            let text = match bytes {
                Some(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => Some(text),
                    Err(_) => continue,
                },
                None => None,
            };
            let file = &mut self.texts[index];
            file.saved = state;
            file.saved_text = text.clone();
            file.text = text.unwrap_or_default();
            file.version += 1;
            let version = file.version;
            self.emit(
                Origin::SESSION,
                None,
                EventKind::TextFileChanged {
                    path: path.clone(),
                    version,
                },
            );
        }
        conflicts
    }

    /// The text files an external check should read, with their baselines.
    pub(crate) fn text_baselines(&self) -> Vec<(ProjectPath, FileState)> {
        self.texts
            .iter()
            .map(|file| (file.path.clone(), file.saved.clone()))
            .collect()
    }
}
