//! Component publication (ADR 0010). A working component is the editable
//! document; a published revision is immutable canonical source with explicit
//! call revisions, plus its own identity/layout checkpoint.
//!
//! One transaction saves the working text, writes the next unused revision,
//! records it in the registry with its checkpoint and advances the working
//! document's base revision. An existing published file is never overwritten,
//! and the new revision only becomes resolvable once that transaction commits.

use super::{EventKind, Origin, ProjectSession, working_revision};
use crate::{
    Error, Result,
    analysis::{self, Target, identity_base},
    context,
    document::AnalysisState,
};
use apiforge_script::Document;
use apiforge_storage::project::{
    DiagramRecord, DocumentKey, DocumentKind, FileState, ProjectChange, ProjectPath, RevisionEntry,
};

/// What a publication created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    pub component_id: String,
    pub revision: u32,
    pub generation: u64,
}

impl ProjectSession {
    /// The revision this document's next publication would create, or why it
    /// cannot be published. Asking never changes anything.
    pub fn publishable(&self, document_id: &str) -> Result<u32> {
        let index = self.index(document_id)?;
        let document = &self.documents[index];
        if document.entry.kind != DocumentKind::Component {
            return Err(Error::NotAComponent);
        }
        if self.store.is_read_only() {
            return Err(Error::Storage(
                apiforge_storage::project::ProjectError::ReadOnly,
            ));
        }
        if document.conflict.is_some() {
            return Err(Error::ExternalConflict);
        }
        if document.unavailable.is_some() || document.analysis == AnalysisState::Invalid {
            return Err(Error::InvalidModel);
        }
        self.require_no_held_draft(document_id)?;
        if !document.fields.is_empty() {
            return Err(Error::ModelLocked);
        }
        if self.diagram.is_none() {
            return Err(apiforge_storage::project::ProjectError::InvalidCheckpoint.into());
        }
        if self.manifest.revisions.iter().any(|revision| {
            revision.component_id == document.entry.entity_id && revision.revision == u32::MAX
        }) {
            return Err(Error::Unrepresentable(
                "the component revision limit was reached",
            ));
        }
        Ok(working_revision(&self.manifest, &document.entry))
    }

    /// Publishes the working component as its next revision. Settles pending
    /// analysis first, so the revision describes the text being published.
    ///
    /// A recovered draft publishes its own source as a revision after the
    /// current maximum; it neither rebases nor replaces the primary working
    /// document, and other working documents keep their own base revisions.
    pub fn publish(&mut self, document_id: &str, origin: Origin) -> Result<Published> {
        let index = self.index(document_id)?;
        self.require_no_held_draft(document_id)?;
        self.require_current_registry()?;
        // Validate a stable saved dependency set before preparing new shared
        // data. Local last-valid cache is never publication authority.
        let (_, snapshot) = self.store.load()?;
        let manifest_path = ProjectPath::new(apiforge_storage::project::MANIFEST_PATH)?;
        if snapshot.state(&manifest_path) != Some(self.manifest_saved.clone()) {
            return Err(Error::RegistryChanged);
        }
        self.publishable(document_id)?;
        if self.documents[index].analysis == AnalysisState::Pending {
            self.analyse_now(index)?;
        }
        // Settling can invalidate what was valid a moment ago.
        self.publishable(document_id)?;
        let Some(Document::Component(component)) = self.documents[index].model.clone() else {
            return Err(Error::InvalidModel);
        };
        // Canonical printing drops comments that belong to no statement, so the
        // caller decides before the first publication that would lose them.
        self.require_comment_decision(index)?;

        // A handle is part of a published component's identity: an instance
        // resolves it by name. Changing it after the first publication would
        // silently repoint existing calls, so Stage II rejects it.
        if let Some(published) = self
            .manifest
            .revisions
            .iter()
            .find(|entry| entry.component_id == component.id)
            && published.handle != component.handle
        {
            return Err(Error::HandleChanged {
                published: published.handle.clone(),
            });
        }

        let revision = working_revision(&self.manifest, &self.documents[index].entry);
        debug_assert_eq!(
            component.revision, revision,
            "the model carries its next revision"
        );
        let path = ProjectPath::new(format!(
            ".apiforge/revisions/{}/{revision}.afs",
            component.id
        ))
        .map_err(|_| Error::Unrepresentable("a component ID is not a portable file name"))?;
        if self.store.file_state(&path)? != FileState::Missing {
            return Err(Error::RevisionExists {
                component_id: component.id.clone(),
                revision,
            });
        }

        let document = Document::Component(component.clone());
        let source = apiforge_script::print_folder_document(&self.context, &document)
            .map_err(|_| Error::Unprintable)?;
        // The loader derives every revision from its own source, so a revision
        // that would not lower there must not be written at all.
        let target = Target {
            kind: DocumentKind::Component,
            entity_id: component.id.clone(),
            revision,
        };
        let derived = analysis::derive(
            &target,
            &source,
            &self.context,
            None,
            None,
            self.placer.as_ref(),
        );
        match derived.1 {
            Some((Document::Component(printed), _)) if printed.handle == component.handle => {}
            _ => {
                return Err(Error::InvalidRevision {
                    component_id: component.id.clone(),
                    revision,
                });
            }
        }

        let mut manifest = self.manifest.clone();
        manifest.revisions.push(RevisionEntry {
            component_id: component.id.clone(),
            handle: component.handle.clone(),
            revision,
            path: path.clone(),
            source_hash: hash(&source),
        });
        manifest.documents[self.entry_position(index)].base_revision = Some(revision);
        manifest.validate()?;

        let mut diagram = self.diagram.clone();
        if let Some(file) = diagram.as_mut() {
            let key = DocumentKey::Revision {
                component_id: component.id.clone(),
                revision,
            };
            file.records.retain(|record| record.key != key);
            file.records.push(DiagramRecord::capture(
                key,
                &source,
                identity_base(&document),
                &self.context,
            )?);
            // The working document's own checkpoint describes its own text.
            let working = DocumentKey::Working {
                document_id: document_id.to_owned(),
            };
            file.records.retain(|record| record.key != working);
            file.records.push(DiagramRecord::capture(
                working,
                &self.documents[index].text,
                identity_base(&Document::Component(component.clone())),
                &self.context,
            )?);
        }

        let mut changes = Vec::new();
        for directory in [
            ".apiforge/revisions".to_owned(),
            format!(".apiforge/revisions/{}", component.id),
        ] {
            let path = ProjectPath::new(directory)?;
            if self.store.file_state(&path)? == FileState::Missing {
                changes.push(ProjectChange::CreateDirectory { path });
            }
        }
        let working_bytes = self.documents[index].text.as_bytes().to_vec();
        // Include an unchanged source too: its expected state must participate
        // in transaction preflight when an external editor changed it.
        changes.push(ProjectChange::Write {
            path: self.documents[index].entry.path.clone(),
            expected: self.documents[index].saved.clone(),
            bytes: working_bytes.clone(),
        });
        changes.push(ProjectChange::Write {
            path: path.clone(),
            expected: FileState::Missing,
            bytes: source.into_bytes(),
        });
        let diagram_bytes = diagram.as_ref().map(|file| file.to_bytes()).transpose()?;
        if let Some(bytes) = &diagram_bytes {
            changes.push(ProjectChange::Write {
                path: ProjectPath::new(super::DIAGRAM_PATH)?,
                expected: self.diagram_saved.clone(),
                bytes: bytes.clone(),
            });
        }
        let manifest_bytes = manifest.to_public_bytes()?;
        changes.push(ProjectChange::Write {
            path: ProjectPath::new(apiforge_storage::project::MANIFEST_PATH)?,
            expected: self.manifest_saved.clone(),
            bytes: manifest_bytes.clone(),
        });

        let generation = self.store.commit(changes)?;
        self.refresh_local_baselines();
        self.documents[index].saved = FileState::of(&working_bytes);
        self.documents[index].layout_dirty = false;
        self.manifest = manifest;
        self.documents[index].entry.base_revision = Some(revision);
        self.manifest_saved = FileState::of(&manifest_bytes);
        self.manifest_dirty = false;
        if let Some(bytes) = diagram_bytes {
            self.diagram_saved = FileState::of(&bytes);
            self.diagram = diagram;
        }

        // Only now does the revision exist for resolution. Every document is
        // analysed again: a call that resolved to the previous maximum may
        // resolve differently, and outstanding results are stale.
        self.published.push(component.clone());
        context::sort_components(&mut self.published);
        let mut project = context::base_project(&self.manifest);
        project.components = self.published.clone();
        self.context = std::sync::Arc::new(project);
        self.context_generation += 1;
        for index in 0..self.documents.len() {
            if self.documents[index].unavailable.is_none() {
                self.documents[index].analysis = AnalysisState::Pending;
            }
        }
        let version = self.documents[index].source_version;
        self.emit(
            origin,
            Some(document_id),
            EventKind::Saved {
                source_version: version,
                generation,
            },
        );
        self.emit(
            origin,
            Some(document_id),
            EventKind::Published {
                component_id: component.id.clone(),
                revision,
            },
        );
        let context_generation = self.context_generation;
        self.emit(
            Origin::SESSION,
            None,
            EventKind::ContextChanged {
                generation: context_generation,
            },
        );
        // The published text is saved, so its draft is obsolete.
        let _ = self.save_recovery();
        Ok(Published {
            component_id: component.id,
            revision,
            generation,
        })
    }

    fn entry_position(&self, index: usize) -> usize {
        let document_id = &self.documents[index].entry.document_id;
        self.manifest
            .documents
            .iter()
            .position(|entry| &entry.document_id == document_id)
            .expect("a registered document")
    }
}

fn hash(text: &str) -> String {
    match FileState::of(text.as_bytes()) {
        FileState::File { sha256, .. } => sha256,
        _ => unreachable!("bytes always have a file state"),
    }
}
