//! Local protection of unsaved work and the last-valid cache (ADR 0011/0012).
//! Recovery restores a draft automatically only when the saved source is still
//! the draft's base; any other draft waits for an explicit keep or discard.
use super::{ProjectSession, Result};
use crate::{
    Error,
    analysis::{self, document_of, identity_base},
    document::{AnalysisState, Unavailable},
};
use apiforge_storage::project::{
    CacheNotice, CacheProvenance, DiagramRecord, DocumentKey, FieldRecovery, FileState,
    LastValidSnapshot, ProjectError, RecoveryDocument, RecoveryRead, SourceRecovery, TextSelection,
};

#[derive(Default)]
pub(super) struct RecoveryState {
    /// Baseline for the next write. `None` when the files are unreadable,
    /// unsupported or foreign: they are preserved and never overwritten.
    pub(super) read: Option<RecoveryRead>,
    /// Drafts that are not applied, preserved verbatim in every later write.
    pub(super) held: Vec<RecoveryDocument>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftStatus {
    /// The saved source changed after the draft was taken.
    Stale,
    /// The draft's document is no longer registered.
    Orphan,
}

impl ProjectSession {
    pub(super) fn load_recovery(&mut self) -> Result<()> {
        self.recovery = RecoveryState::default();
        let Ok(read) = self.store.load_source_recovery(&self.manifest.id) else {
            return Ok(());
        };
        let drafts = if read.is_backup_candidate() {
            Vec::new()
        } else {
            read.recovery()
                .map(|recovery| recovery.documents.clone())
                .unwrap_or_default()
        };
        self.recovery.read = Some(read);
        for draft in drafts {
            self.adopt_draft(draft)?;
        }
        Ok(())
    }

    fn draft_target(&self, draft: &RecoveryDocument) -> Option<usize> {
        self.documents.iter().position(|document| {
            document.entry.document_id == draft.document_id
                && document.entry.entity_id == draft.entity_id
                && document.entry.kind == draft.kind
                // A missing file can be recreated from a kept draft; other
                // unavailable files cannot be edited as text.
                && matches!(document.unavailable, None | Some(Unavailable::Missing))
        })
    }

    /// Installs a draft's text, view state and unsaved layout. The layout is a
    /// candidate identity base; the text alone decides the semantics.
    fn apply_draft(&mut self, index: usize, draft: RecoveryDocument) -> Result<()> {
        let target = self.target(&self.documents[index].entry);
        let candidate = draft
            .checkpoint
            .as_ref()
            .filter(|record| record.validate_in_context(&self.context).is_ok())
            .map(|record| {
                let mut model = document_of(&record.identity_base);
                analysis::normalise(&mut model, &target);
                model
            });
        let document = &mut self.documents[index];
        // Over a missing file the draft becomes unsaved text; saving recreates it.
        document.unavailable = None;
        document.selection = draft.selection;
        document.fields = draft.fields;
        if document.text != draft.text {
            document.text = draft.text;
            document.source_version += 1;
        }
        if let Some(candidate) = candidate {
            document.layout_dirty = true;
            if let Some(previous) = document.model.replace(candidate) {
                document.history.record(previous);
            }
        }
        document.analysis = AnalysisState::Pending;
        let version = document.source_version;
        let id = document.entry.document_id.clone();
        self.emit(
            super::Origin::SESSION,
            Some(&id),
            super::EventKind::SourceChanged {
                source_version: version,
            },
        );
        self.analyse_now(index)
    }

    /// Own trash operations rewrite recovery under the writer lock. Adopt the
    /// result as the next baseline; unusable files stay unusable.
    pub(super) fn adopt_recovery_baseline(&mut self) {
        if self
            .recovery
            .read
            .as_ref()
            .is_some_and(|read| !read.is_backup_candidate())
            && let Ok(read) = self.store.load_source_recovery(&self.manifest.id)
        {
            self.recovery.read = Some(read);
        }
    }

    /// Applies a draft whose base is the saved source; holds any other.
    pub(super) fn adopt_draft(&mut self, draft: RecoveryDocument) -> Result<()> {
        match self.draft_target(&draft) {
            Some(index) if draft.matches_base(&self.documents[index].saved) => {
                self.apply_draft(index, draft)
            }
            _ => {
                self.recovery.held.push(draft);
                Ok(())
            }
        }
    }

    /// Own commits change the manifest that recovery and cache baselines bind.
    /// Adopt the new baseline only when their own files are unchanged, so that
    /// a foreign recovery or cache write still surfaces as a conflict.
    pub(super) fn refresh_local_baselines(&mut self) {
        if let Some(previous) = &self.recovery.read
            && let Ok(read) = self.store.load_source_recovery(&self.manifest.id)
            && read.is_backup_candidate() == previous.is_backup_candidate()
            && read.recovery() == previous.recovery()
        {
            self.recovery.read = Some(read);
        }
        if let Some(previous) = &self.cache
            && let Ok(read) = self.store.load_last_valid_cache(&self.manifest.id)
            && read.notice() == previous.notice()
            && same_snapshots(read.snapshots(), previous.snapshots())
        {
            self.cache = Some(read);
        }
    }

    pub(super) fn require_no_held_draft(&self, document_id: &str) -> Result<()> {
        if self
            .recovery
            .held
            .iter()
            .any(|draft| draft.document_id == document_id)
        {
            return Err(Error::DraftDecisionRequired);
        }
        Ok(())
    }

    /// Drafts waiting for a decision. Their documents cannot be edited or saved
    /// until the draft is kept or discarded.
    pub fn recovered_drafts(&self) -> Vec<(DraftStatus, &RecoveryDocument)> {
        self.recovery
            .held
            .iter()
            .map(|draft| {
                let status = if self.draft_target(draft).is_some() {
                    DraftStatus::Stale
                } else {
                    DraftStatus::Orphan
                };
                (status, draft)
            })
            .collect()
    }

    /// Keeps a stale draft over the changed saved source. The document becomes
    /// dirty against the current file; saving it later replaces that file.
    pub fn keep_draft(&mut self, document_id: &str) -> Result<()> {
        let position = self
            .recovery
            .held
            .iter()
            .position(|draft| draft.document_id == document_id)
            .ok_or(Error::UnknownDocument)?;
        let index = self
            .draft_target(&self.recovery.held[position])
            .ok_or(Error::UnknownDocument)?;
        let draft = self.recovery.held.remove(position);
        self.apply_draft(index, draft)
    }

    /// Forgets a held draft; the next recovery write no longer contains it.
    pub fn discard_draft(&mut self, document_id: &str) -> Result<()> {
        let before = self.recovery.held.len();
        self.recovery
            .held
            .retain(|draft| draft.document_id != document_id);
        if self.recovery.held.len() == before {
            return Err(Error::UnknownDocument);
        }
        Ok(())
    }

    /// The primary recovery file is unusable and a checked backup exists.
    pub fn recovery_decision_required(&self) -> bool {
        self.recovery
            .read
            .as_ref()
            .is_some_and(RecoveryRead::is_backup_candidate)
    }

    /// Unreadable, unsupported or foreign recovery files block recovery writes.
    pub fn recovery_unavailable(&self) -> bool {
        self.recovery.read.is_none()
    }

    /// Explicitly restores the previewed backup, then processes its drafts.
    pub fn restore_recovery_backup(&mut self) -> Result<()> {
        let read = self
            .recovery
            .read
            .as_ref()
            .ok_or(Error::RecoveryUnavailable)?;
        self.store.restore_source_recovery_backup(read)?;
        self.load_recovery()
    }

    pub fn set_selection(&mut self, document_id: &str, selection: TextSelection) -> Result<()> {
        let index = self.index(document_id)?;
        self.documents[index].selection = selection;
        Ok(())
    }

    pub fn set_field_drafts(
        &mut self,
        document_id: &str,
        fields: Vec<FieldRecovery>,
    ) -> Result<()> {
        let index = self.index(document_id)?;
        self.documents[index].fields = fields;
        Ok(())
    }

    /// The recovery content this session would write now.
    pub fn recovery_contents(&self) -> SourceRecovery {
        let mut documents = self.recovery.held.clone();
        for document in self.documents.iter().filter(|d| d.needs_recovery()) {
            let selection = if document.text.is_char_boundary(document.selection.anchor)
                && document.text.is_char_boundary(document.selection.caret)
            {
                document.selection.clone()
            } else {
                TextSelection {
                    anchor: 0,
                    caret: 0,
                }
            };
            let source = match document.analysis {
                AnalysisState::Valid => Some(&document.text),
                _ => document.model_source.as_ref(),
            };
            let checkpoint = document
                .model
                .as_ref()
                .zip(source)
                .and_then(|(model, source)| {
                    DiagramRecord::capture(
                        DocumentKey::Working {
                            document_id: document.entry.document_id.clone(),
                        },
                        source,
                        identity_base(model),
                        &self.context,
                    )
                    .ok()
                });
            documents.push(RecoveryDocument {
                document_id: document.entry.document_id.clone(),
                entity_id: document.entry.entity_id.clone(),
                kind: document.entry.kind,
                base_revision: document.entry.base_revision,
                path: document.entry.path.clone(),
                base_hash: sha256(&document.saved),
                source_version: document.source_version,
                text: document.text.clone(),
                selection,
                fields: document.fields.clone(),
                checkpoint,
            });
        }
        SourceRecovery {
            format_version: 1,
            project_id: self.manifest.id.clone(),
            documents,
        }
    }

    /// Writes recovery when it differs from what is on disk. Closing a project
    /// or switching to another requires this to succeed (or an explicit
    /// export/discard decision). Never saves `.afs` files.
    pub fn save_recovery(&mut self) -> Result<bool> {
        if self.store.is_read_only() {
            return Err(Error::Storage(ProjectError::ReadOnly));
        }
        let read = self
            .recovery
            .read
            .as_ref()
            .ok_or(Error::RecoveryUnavailable)?;
        if read.is_backup_candidate() {
            return Err(Error::Storage(ProjectError::RecoveryDecisionRequired));
        }
        let desired = self.recovery_contents();
        let unchanged = match read.recovery() {
            Some(current) => *current == desired,
            None => desired.documents.is_empty(),
        };
        if unchanged {
            return Ok(false);
        }
        let read = self.store.save_source_recovery(read, &desired)?;
        self.recovery.read = Some(read);
        Ok(true)
    }

    /// Closes the session once unsaved work is protected: recovery must be
    /// written successfully. A read-only session, or one whose recovery files
    /// are unusable, returns itself so the caller can offer export or discard.
    /// Configuration is not part of source recovery and must be saved first.
    pub fn close(mut self) -> std::result::Result<(), (Box<Self>, Error)> {
        if self.manifest_dirty {
            return Err((Box::new(self), Error::UnsavedConfiguration));
        }
        let unsaved = !self.recovery.held.is_empty()
            || self
                .documents
                .iter()
                .any(|document| document.needs_recovery());
        // Undoing an edit or discarding the final held draft can leave the
        // session clean while the on-disk recovery still contains obsolete work.
        // Synchronise that recovery before acknowledging close. Clean readers
        // and unusable/backup candidates must still be left untouched.
        let recovery_needs_clearing = !self.store.is_read_only()
            && self.recovery.read.as_ref().is_some_and(|read| {
                !read.is_backup_candidate()
                    && read
                        .recovery()
                        .is_some_and(|recovery| !recovery.documents.is_empty())
            });
        if !unsaved && !recovery_needs_clearing {
            return Ok(());
        }
        match self.save_recovery() {
            Ok(_) => Ok(()),
            Err(error) => Err((Box::new(self), error)),
        }
    }

    /// Explicitly abandons every unsaved change and held draft, then closes.
    /// A writable session also removes them from recovery, so they do not
    /// return on the next open.
    /// A failed recovery write returns the session unchanged, so the caller can
    /// keep editing or retry.
    pub fn discard_unsaved_and_close(self) -> std::result::Result<(), (Box<Self>, Error)> {
        let Some(read) = &self.recovery.read else {
            return Ok(());
        };
        if self.store.is_read_only() || read.is_backup_candidate() {
            return Ok(());
        }
        let empty = SourceRecovery {
            format_version: 1,
            project_id: self.manifest.id.clone(),
            documents: Vec::new(),
        };
        if read.recovery().is_some_and(|current| *current != empty)
            && let Err(error) = self.store.save_source_recovery(read, &empty)
        {
            return Err((Box::new(self), error.into()));
        }
        Ok(())
    }

    /// Supplies verified last-valid models to documents whose saved source is
    /// invalid. Only an entry for the same document and the same closed
    /// configuration/dependency context is used, and only as a last-valid model.
    pub(super) fn load_last_valid(&mut self) {
        self.cache = self.store.load_last_valid_cache(&self.manifest.id).ok();
        let Some(cache) = &self.cache else {
            return;
        };
        for index in 0..self.documents.len() {
            let document = &self.documents[index];
            if document.model.is_some() || document.analysis != AnalysisState::Invalid {
                continue;
            }
            let key = DocumentKey::Working {
                document_id: document.entry.document_id.clone(),
            };
            let Some(snapshot) = cache
                .snapshots()
                .iter()
                .find(|snapshot| snapshot.key() == &key && snapshot.matches_context(&self.context))
            else {
                continue;
            };
            let target = self.target(&document.entry);
            let mut model = document_of(snapshot.document());
            analysis::normalise(&mut model, &target);
            let source = snapshot.source().to_owned();
            let document = &mut self.documents[index];
            document.model = Some(model);
            document.model_source = Some(source);
        }
    }

    pub fn cache_notice(&self) -> Option<CacheNotice> {
        match &self.cache {
            Some(cache) => cache.notice(),
            None => Some(CacheNotice::Unavailable),
        }
    }

    /// Persists a last-valid snapshot of every document with a known model
    /// source. A damaged or foreign cache is never replaced implicitly.
    pub fn save_last_valid(&mut self) -> Result<bool> {
        if self.store.is_read_only() {
            return Err(Error::Storage(ProjectError::ReadOnly));
        }
        let cache = self
            .cache
            .as_ref()
            .ok_or(Error::Storage(ProjectError::InvalidCache))?;
        if cache.notice().is_some() {
            return Err(Error::Storage(ProjectError::CacheReplacementRequired));
        }
        let mut snapshots = Vec::new();
        for document in &self.documents {
            let (Some(model), Some(source)) = (&document.model, &document.model_source) else {
                continue;
            };
            let saved = FileState::of(source.as_bytes()) == document.saved;
            let snapshot = LastValidSnapshot::capture(
                DocumentKey::Working {
                    document_id: document.entry.document_id.clone(),
                },
                source,
                CacheProvenance {
                    session_epoch: self.session_id.clone(),
                    source_version: document.source_version,
                    saved_source_hash: if saved { sha256(&document.saved) } else { None },
                },
                identity_base(model),
                &self.context,
            );
            if let Ok(snapshot) = snapshot {
                snapshots.push(snapshot);
            }
        }
        if same_snapshots(cache.snapshots(), &snapshots) {
            return Ok(false);
        }
        let saved = self.store.save_last_valid_cache(cache, &snapshots)?;
        self.cache = Some(saved);
        Ok(true)
    }
}

fn sha256(state: &FileState) -> Option<String> {
    match state {
        FileState::File { sha256, .. } => Some(sha256.clone()),
        _ => None,
    }
}

/// Equal content, ignoring provenance such as the session and source version.
fn same_snapshots(left: &[LastValidSnapshot], right: &[LastValidSnapshot]) -> bool {
    fn sorted(list: &[LastValidSnapshot]) -> Vec<&LastValidSnapshot> {
        let mut list: Vec<_> = list.iter().collect();
        list.sort_by(|a, b| a.key().cmp(b.key()));
        list
    }
    let (left, right) = (sorted(left), sorted(right));
    left.len() == right.len()
        && left.iter().zip(&right).all(|(a, b)| {
            a.key() == b.key()
                && a.source_hash() == b.source_hash()
                && a.context_digest() == b.context_digest()
                && a.document() == b.document()
        })
}
