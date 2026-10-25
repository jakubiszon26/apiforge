//! Explicit source/text conflict decisions bound to the displayed candidates.
use super::*;
use apiforge_storage::project::{FieldRecovery, TextSelection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictTarget {
    Document(String),
    Text(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictDecision {
    UseDisk,
    /// Only an observed deletion permits recreating the file on the next save.
    KeepDeleted,
}

#[derive(Clone, Debug, PartialEq)]
struct LocalVersion {
    version: u64,
    text: String,
    saved: FileState,
    fields: Vec<FieldRecovery>,
    model: Option<Document>,
    layout_dirty: bool,
    held: bool,
    conflict: ExternalConflict,
}

/// Candidates are immutable. A later local edit, context replacement or foreign
/// write requires another preview; accepting never rereads an unseen candidate.
#[derive(Clone, Debug)]
pub struct ConflictPreview {
    target: ConflictTarget,
    path: ProjectPath,
    session_id: String,
    epoch: u64,
    context_generation: u64,
    local: LocalVersion,
    disk: ProjectSnapshot,
}

impl ConflictPreview {
    pub fn target(&self) -> &ConflictTarget {
        &self.target
    }
    pub fn path(&self) -> &str {
        self.path.as_str()
    }
    pub fn local_bytes(&self) -> &[u8] {
        self.local.text.as_bytes()
    }
    pub fn disk_bytes(&self) -> Option<&[u8]> {
        self.disk.files[&self.path].as_deref()
    }
    pub fn is_deleted(&self) -> bool {
        self.disk_bytes().is_none()
    }
}

impl ProjectSession {
    pub fn preview_conflict(&self, target: ConflictTarget) -> Result<ConflictPreview> {
        let (path, local) = self.conflict_version(&target)?;
        let manifest = ProjectPath::new(MANIFEST_PATH)?;
        let disk = self.store.read(&[manifest.clone(), path.clone()])?;
        if disk.state(&manifest) != Some(self.manifest_saved.clone()) {
            return Err(Error::RegistryChanged);
        }
        Ok(ConflictPreview {
            target,
            path,
            session_id: self.session_id.clone(),
            epoch: self.epoch,
            context_generation: self.context_generation,
            local,
            disk,
        })
    }

    pub fn resolve_conflict(
        &mut self,
        preview: &ConflictPreview,
        decision: ConflictDecision,
        origin: Origin,
    ) -> Result<()> {
        if preview.session_id != self.session_id
            || preview.epoch != self.epoch
            || preview.context_generation != self.context_generation
            || self.conflict_version(&preview.target)?
                != (preview.path.clone(), preview.local.clone())
        {
            return Err(Error::StaleConflict);
        }
        let paths: Vec<_> = preview.disk.files.keys().cloned().collect();
        let current = self.store.read(&paths)?;
        if current.generation != preview.disk.generation || current.files != preview.disk.files {
            return Err(Error::StaleConflict);
        }
        if decision == ConflictDecision::KeepDeleted
            && (!preview.is_deleted() || preview.local.conflict != ExternalConflict::Deleted)
        {
            return Err(Error::ExternalConflict);
        }
        match &preview.target {
            ConflictTarget::Document(id) => {
                self.require_no_held_draft(id)?;
                let index = self.index(id)?;
                if decision == ConflictDecision::KeepDeleted {
                    self.documents[index].saved = FileState::Missing;
                    self.documents[index].conflict = None;
                    self.emit(origin, Some(id), EventKind::ExternalConflict);
                } else {
                    let document = &mut self.documents[index];
                    document.layout_dirty = false;
                    document.fields.clear();
                    document.history = Default::default();
                    document.selection = TextSelection {
                        anchor: 0,
                        caret: 0,
                    };
                    let observed = preview.disk_bytes().map_or(ObservedFile::Missing, |bytes| {
                        ObservedFile::Bytes(bytes.to_vec())
                    });
                    self.follow_disk(index, observed, origin)?;
                }
            }
            ConflictTarget::Text(path) => {
                let index = self.text_index(path)?;
                // Validate UTF-8 before discarding local work. Raw foreign bytes
                // remain exportable even when they cannot become editor text.
                let text = if decision == ConflictDecision::UseDisk {
                    preview
                        .disk_bytes()
                        .map(|bytes| {
                            String::from_utf8(bytes.to_vec())
                                .map_err(|_| Error::NotText(self.store.root().join(path)))
                        })
                        .transpose()?
                } else {
                    None
                };
                let file = &mut self.texts[index];
                if decision == ConflictDecision::UseDisk {
                    file.saved = preview
                        .disk
                        .state(&preview.path)
                        .ok_or(Error::StaleConflict)?;
                    // Accepting deletion is a clean, absent file; keeping a
                    // deletion below instead remains dirty for recreation.
                    file.saved_text = Some(text.clone().unwrap_or_default());
                    file.text = text.unwrap_or_default();
                    file.version += 1;
                    file.selection = TextSelection {
                        anchor: 0,
                        caret: 0,
                    };
                } else {
                    file.saved = FileState::Missing;
                    file.saved_text = None;
                }
                file.conflict = None;
                file.held = false;
                let version = file.version;
                self.emit(
                    origin,
                    None,
                    EventKind::TextFileChanged {
                        path: path.clone(),
                        version,
                    },
                );
                self.drop_text_draft(path);
            }
        }
        Ok(())
    }

    fn conflict_version(&self, target: &ConflictTarget) -> Result<(ProjectPath, LocalVersion)> {
        match target {
            ConflictTarget::Document(id) => {
                self.require_no_held_draft(id)?;
                let document = &self.documents[self.index(id)?];
                Ok((
                    document.entry.path.clone(),
                    LocalVersion {
                        version: document.source_version,
                        text: document.text().to_owned(),
                        saved: document.saved.clone(),
                        fields: document.fields.clone(),
                        model: document.model().cloned(),
                        layout_dirty: document.layout_dirty,
                        held: false,
                        conflict: document.conflict.ok_or(Error::ExternalConflict)?,
                    },
                ))
            }
            ConflictTarget::Text(path) => {
                let file = &self.texts[self.text_index(path)?];
                if file.held {
                    return Err(Error::DraftDecisionRequired);
                }
                Ok((
                    file.path.clone(),
                    LocalVersion {
                        version: file.version,
                        text: file.text.clone(),
                        saved: file.saved.clone(),
                        fields: Vec::new(),
                        model: None,
                        layout_dirty: false,
                        held: file.held,
                        conflict: file.conflict.ok_or(Error::ExternalConflict)?,
                    },
                ))
            }
        }
    }
}
