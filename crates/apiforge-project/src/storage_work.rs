//! Storage workers receive an owned candidate; the live session remains the
//! authority for newer edits. Only explicit completion updates its baselines.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionIdentity {
    id: String,
    epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageMode {
    /// A preview/read does not replace any live session state.
    Read,
    /// Source saves/recovery/text opens preserve newer edits and their history.
    PreserveEdits,
    /// Registry/destructive commands require unchanged memory before adoption.
    Exclusive,
}

pub struct StorageSnapshot {
    base: ProjectSession,
}

pub struct StorageCompletion<T> {
    base: ProjectSession,
    candidate: ProjectSession,
    mode: StorageMode,
    value: T,
}

impl StorageSnapshot {
    /// Run on the serial storage worker. Capture the snapshot when a queued
    /// command starts, so it sees earlier committed storage completions.
    pub fn run<T>(
        self,
        mode: StorageMode,
        command: impl FnOnce(&mut ProjectSession) -> T,
    ) -> StorageCompletion<T> {
        let mut candidate = self.base.clone_for_storage();
        let value = command(&mut candidate);
        StorageCompletion {
            base: self.base,
            candidate,
            mode,
            value,
        }
    }
}

impl ProjectSession {
    pub fn storage_identity(&self) -> SessionIdentity {
        SessionIdentity {
            id: self.session_id.clone(),
            epoch: self.epoch,
        }
    }

    pub fn storage_snapshot(&self) -> StorageSnapshot {
        StorageSnapshot {
            base: self.clone_for_storage(),
        }
    }

    /// Adopt only the versions the worker actually observed/saved. A successful
    /// save of version N cannot replace or mark version N+1 clean. All paths,
    /// fingerprints and lock ownership still come from the existing store APIs.
    pub fn complete_storage<T>(&mut self, completion: StorageCompletion<T>) -> Result<T> {
        let StorageCompletion {
            base,
            mut candidate,
            mode,
            value,
        } = completion;
        if self.storage_identity() != base.storage_identity() {
            return Err(Error::StaleStorage);
        }
        if mode == StorageMode::Read {
            return Ok(value);
        }
        if self.manifest_saved != base.manifest_saved
            || self.diagram_saved != base.diagram_saved
            || self.documents.iter().any(|live| {
                base.document(live.id())
                    .is_none_or(|before| live.saved != before.saved)
            })
            || self.texts.iter().any(|live| {
                base.text_file(live.path())
                    .is_none_or(|before| live.saved != before.saved)
            })
        {
            return Err(Error::StaleStorage);
        }
        if mode == StorageMode::Exclusive {
            if !self.same_storage_inputs(&base) {
                return Err(Error::StaleStorage);
            }
        } else {
            // A save or text open does not replace document membership/context.
            if candidate.storage_identity() != base.storage_identity()
                || candidate.manifest.documents != base.manifest.documents
                || candidate.context_generation != base.context_generation
            {
                return Err(Error::StaleStorage);
            }
            let context_changed = self.context_generation != base.context_generation;
            for current in &self.documents {
                let before = base.document(current.id()).ok_or(Error::StaleStorage)?;
                let after = candidate
                    .documents
                    .iter_mut()
                    .find(|doc| doc.id() == current.id())
                    .ok_or(Error::StaleStorage)?;
                let newer = changed_document(current, before) || context_changed;
                if newer {
                    let saved = after.saved.clone();
                    let saved_layout = after.model.clone();
                    let mut live = current.clone();
                    // Source edits can arrive while an older pending source is
                    // being analysed/saved. Its sealed model is a useful older
                    // preview, never a current model for the newer source.
                    if live.model == before.model && !context_changed {
                        live.model = after.model.clone();
                        live.model_source = after.model_source.clone();
                        live.valid_snapshot = after.valid_snapshot.clone();
                        live.history = after.history.clone();
                    }
                    live.saved = saved;
                    live.entry = after.entry.clone();
                    live.layout_dirty = live.model != saved_layout || after.layout_dirty;
                    *after = live;
                } else {
                    after.selection = current.selection.clone();
                    after.comments_accepted = current.comments_accepted;
                }
            }
            for current in &self.texts {
                let Some(before) = base.text_file(current.path()) else {
                    return Err(Error::StaleStorage);
                };
                let after = candidate
                    .texts
                    .iter_mut()
                    .find(|file| file.path == current.path)
                    .ok_or(Error::StaleStorage)?;
                if current.version != before.version || current.held != before.held {
                    let saved = after.saved.clone();
                    let saved_text = after.saved_text.clone();
                    *after = current.clone();
                    after.saved = saved;
                    after.saved_text = saved_text;
                } else {
                    after.selection = current.selection.clone();
                }
            }
            if self.manifest != base.manifest || context_changed {
                candidate.manifest_dirty = if candidate.manifest_saved == base.manifest_saved {
                    self.manifest_dirty
                } else {
                    self.manifest != candidate.manifest
                };
                candidate.manifest = self.manifest.clone();
                candidate.context = self.context.clone();
                candidate.context_generation = self.context_generation;
            }
            // Keep/discard decisions entered during an older recovery save
            // remain authoritative. Only its durable read baseline is adopted.
            candidate.recovery.held = self.recovery.held.clone();
            candidate.recovery.held_texts = self.recovery.held_texts.clone();
        }
        let produced = candidate
            .events
            .iter()
            .filter(|event| event.sequence > base.sequence)
            .cloned()
            .collect::<Vec<_>>();
        candidate.events = std::mem::take(&mut self.events);
        candidate.sequence = self.sequence;
        for event in produced {
            let obsolete_source = mode == StorageMode::PreserveEdits
                && match &event.kind {
                    EventKind::SourceChanged { .. }
                    | EventKind::ModelChanged { .. }
                    | EventKind::AnalysisPending { .. }
                    | EventKind::AnalysisCompleted { .. } => {
                        event.document_id.as_ref().is_some_and(|id| {
                            self.document(id).zip(base.document(id)).is_some_and(
                                |(live, before)| {
                                    changed_document(live, before)
                                        || self.context_generation != base.context_generation
                                },
                            )
                        })
                    }
                    EventKind::TextFileChanged { path, .. } => self
                        .text_file(path)
                        .zip(base.text_file(path))
                        .is_some_and(|(live, before)| live.version != before.version),
                    _ => false,
                };
            if !obsolete_source {
                candidate.emit(event.origin, event.document_id.as_deref(), event.kind);
            }
        }
        *self = candidate;
        Ok(value)
    }

    fn same_storage_inputs(&self, base: &Self) -> bool {
        self.context_generation == base.context_generation
            && self.manifest == base.manifest
            && self.documents.len() == base.documents.len()
            && self.texts.len() == base.texts.len()
            && self.recovery.held == base.recovery.held
            && self.recovery.held_texts == base.recovery.held_texts
            && self
                .documents
                .iter()
                .zip(&base.documents)
                .all(|(live, before)| live.entry == before.entry && !changed_document(live, before))
            && self.texts.iter().zip(&base.texts).all(|(live, before)| {
                live.path == before.path
                    && live.version == before.version
                    && live.held == before.held
            })
    }

    fn clone_for_storage(&self) -> Self {
        Self {
            store: self.store.clone(),
            epoch: self.epoch,
            manifest: self.manifest.clone(),
            manifest_saved: self.manifest_saved.clone(),
            manifest_dirty: self.manifest_dirty,
            diagram: self.diagram.clone(),
            diagram_saved: self.diagram_saved.clone(),
            published: self.published.clone(),
            context: self.context.clone(),
            context_generation: self.context_generation,
            documents: self.documents.clone(),
            texts: self.texts.clone(),
            sequence: self.sequence,
            events: Vec::new(),
            placer: self.placer.clone(),
            session_id: self.session_id.clone(),
            recovery: self.recovery.clone(),
            cache: self.cache.clone(),
            registry: self.registry,
        }
    }
}

fn changed_document(live: &DocumentSession, before: &DocumentSession) -> bool {
    live.source_version != before.source_version
        || live.fields != before.fields
        || live.model != before.model
        || live.history.generation != before.history.generation
        || live.conflict != before.conflict
}
