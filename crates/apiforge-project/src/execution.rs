//! Immutable run preparation and explicit last-valid consent (ADR 0012).
use super::*;
use apiforge_storage::project::{CacheProvenance, FieldRecovery, LastValidSnapshot};

/// A proposed run, including the exact snapshot the user is being asked about.
/// A caller must validate it again immediately before starting execution.
#[derive(Clone, Debug)]
pub struct PreparedRun {
    pub snapshot: LastValidSnapshot,
    pub last_valid: bool,
    pub unsaved: bool,
    ticket: crate::AnalysisTicket,
    fields: Vec<FieldRecovery>,
}

impl PreparedRun {
    pub fn document_id(&self) -> &str {
        &self.ticket.document_id
    }
    pub fn project(&self) -> Result<apiforge_core::format::Project> {
        let apiforge_storage::project::IdentityBase::Scenario(scenario) = self.snapshot.document()
        else {
            return Err(Error::InvalidModel);
        };
        let mut project = self.snapshot.context().clone();
        project.scenarios.push(scenario.clone());
        Ok(project)
    }
}

impl ProjectSession {
    /// Pure report invalidation; rendering never settles analysis or reads disk.
    pub fn run_is_stale(&self, prepared: &PreparedRun) -> bool {
        self.index(prepared.document_id()).map_or(true, |index| {
            let document = &self.documents[index];
            prepared.last_valid
                || !prepared.snapshot.matches_source(document.text())
                || self.ticket(index) != prepared.ticket
                || document.fields != prepared.fields
                || document.conflict.is_some()
                || self.registry != crate::RegistryState::Current
        })
    }

    /// An explicit portable export of all in-memory recovery and public settings.
    /// This does not acknowledge any source or local recovery as saved.
    pub fn export_unsaved(&self) -> Result<Vec<u8>> {
        let manifest: serde_json::Value = serde_json::from_slice(&self.manifest.to_public_bytes()?)
            .map_err(|_| Error::InvalidModel)?;
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "formatVersion": 1,
            "project": manifest,
            "recovery": self.recovery_contents(),
        }))
        .map_err(|_| Error::InvalidModel)?;
        if bytes.len() > apiforge_storage::project::MAX_TRANSACTION_BYTES {
            return Err(apiforge_storage::project::ProjectError::Limit.into());
        }
        Ok(bytes)
    }

    /// Settles code and freezes the selected scenario's complete public inputs.
    /// Unresolved fields require the same explicit older-snapshot decision as
    /// invalid source. Conflicts and held recovery never grant run permission.
    pub fn prepare_run(&mut self, document_id: &str) -> Result<PreparedRun> {
        self.require_current_registry()?;
        self.require_no_held_draft(document_id)?;
        self.settle(document_id)?;
        let index = self.index(document_id)?;
        let document = &self.documents[index];
        if document.entry.kind != DocumentKind::Scenario || document.conflict.is_some() {
            return Err(Error::ModelLocked);
        }
        let last_valid = document.analysis != AnalysisState::Valid || !document.fields.is_empty();
        let snapshot = if last_valid {
            document
                .valid_snapshot
                .clone()
                .or_else(|| {
                    let key = DocumentKey::Working {
                        document_id: document_id.into(),
                    };
                    self.cache
                        .as_ref()?
                        .snapshots()
                        .iter()
                        .find(|snapshot| snapshot.key() == &key)
                        .cloned()
                })
                .ok_or(Error::InvalidModel)?
        } else {
            self.current_snapshot(document_id)?
        };
        let document = &self.documents[index];
        Ok(PreparedRun {
            unsaved: !snapshot.matches_source(document.text()) || document.is_dirty() || last_valid,
            snapshot,
            last_valid,
            ticket: self.ticket(index),
            fields: document.fields.clone(),
        })
    }

    /// A decision never reloads or silently changes the proposed snapshot.
    /// Source, context, reload, field changes and conflicts invalidate it.
    pub fn validate_run(&mut self, prepared: &PreparedRun) -> Result<()> {
        let current = self.prepare_run(prepared.document_id())?;
        if current.ticket != prepared.ticket
            || current.fields != prepared.fields
            || current.snapshot.snapshot_digest() != prepared.snapshot.snapshot_digest()
            || current.last_valid != prepared.last_valid
        {
            return Err(Error::StaleAnalysis);
        }
        Ok(())
    }

    pub(super) fn seal_valid(&mut self, index: usize) {
        let document = &self.documents[index];
        if document.analysis != AnalysisState::Valid {
            return;
        }
        let Some(model) = &document.model else {
            return;
        };
        self.documents[index].valid_snapshot = LastValidSnapshot::capture(
            DocumentKey::Working {
                document_id: document.id().into(),
            },
            document.text(),
            CacheProvenance {
                session_epoch: self.session_id.clone(),
                source_version: document.source_version,
                saved_source_hash: match &document.saved {
                    FileState::File { sha256, .. } if !document.is_dirty() => Some(sha256.clone()),
                    _ => None,
                },
            },
            analysis::identity_base(model),
            &self.context,
        )
        .ok();
    }
}
