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
    /// The components the run calls, directly or through another component:
    /// publishing one of them makes its results stale, publishing any other
    /// leaves them fresh (UX-25, plan decision 16).
    calls: std::collections::BTreeSet<String>,
}

impl PreparedRun {
    pub fn document_id(&self) -> &str {
        &self.ticket.document_id
    }
    /// The frozen project the run executes: the snapshot's public context
    /// with the scenario, or the component at its working revision (KTLO-23).
    pub fn project(&self) -> Result<apiforge_core::format::Project> {
        use apiforge_storage::project::IdentityBase;
        let mut project = self.snapshot.context().clone();
        match self.snapshot.document() {
            IdentityBase::Scenario(scenario) => project.scenarios.push(scenario.clone()),
            // Its next revision, which no published revision uses.
            IdentityBase::Component(component) => project.components.push(component.clone()),
        }
        Ok(project)
    }
}

impl ProjectSession {
    /// Pure report invalidation; rendering never settles analysis or reads disk.
    pub fn run_is_stale(&self, prepared: &PreparedRun) -> bool {
        self.index(prepared.document_id()).map_or(true, |index| {
            let document = &self.documents[index];
            let ticket = self.ticket(index);
            prepared.last_valid
                || !prepared.snapshot.matches_source(document.text())
                || ticket.session_id != prepared.ticket.session_id
                || ticket.session_epoch != prepared.ticket.session_epoch
                || ticket.source_version != prepared.ticket.source_version
                || self.context_changed_for(prepared)
                || document.fields != prepared.fields
                || document.conflict.is_some()
                || self.registry != crate::RegistryState::Current
        })
    }

    /// Whether the run is stale for a reason other than its document's text
    /// and the edits that led to it: an earlier version ran, the session or
    /// the project's context changed, fields are unresolved, the document is
    /// in conflict or the registry changed. A caller that knows which texts
    /// it still accepts for these results compares the text itself (UX-01:
    /// checks added from the results).
    pub fn run_is_stale_beyond_text(&self, prepared: &PreparedRun) -> bool {
        self.index(prepared.document_id()).map_or(true, |index| {
            let document = &self.documents[index];
            let ticket = self.ticket(index);
            prepared.last_valid
                || ticket.session_id != prepared.ticket.session_id
                || ticket.session_epoch != prepared.ticket.session_epoch
                || self.context_changed_for(prepared)
                || document.fields != prepared.fields
                || document.conflict.is_some()
                || self.registry != crate::RegistryState::Current
        })
    }

    /// Whether the project's context changed for this run since it was
    /// prepared: a change other than a publication, or the publication of a
    /// component the run calls. Publishing a component the run does not call
    /// changes nothing the run used (UX-25, plan decision 16).
    fn context_changed_for(&self, prepared: &PreparedRun) -> bool {
        let from = prepared.ticket.context_generation;
        if self.context_generation < from {
            return true;
        }
        (from + 1..=self.context_generation).any(|generation| {
            !self.publications.iter().any(|(published, component)| {
                *published == generation && !prepared.calls.contains(component)
            })
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
        // A project in an older language does not run until it is fixed.
        if let Some(problem) = self.language_problem() {
            return Err(problem);
        }
        self.require_current_registry()?;
        self.require_no_held_draft(document_id)?;
        self.settle(document_id)?;
        let index = self.index(document_id)?;
        let document = &self.documents[index];
        if !matches!(
            document.entry.kind,
            DocumentKind::Scenario | DocumentKind::Component
        ) || document.conflict.is_some()
        {
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
            calls: calls(&snapshot),
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

/// The components a snapshot's definition calls, directly or through the
/// revisions its calls pin, by component ID: a publication of any revision
/// of one of them makes the run stale.
fn calls(snapshot: &LastValidSnapshot) -> std::collections::BTreeSet<String> {
    pinned_calls(snapshot_graphs(snapshot), snapshot.context())
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

/// The graphs of a snapshot's own definition.
fn snapshot_graphs(snapshot: &LastValidSnapshot) -> Vec<&apiforge_core::format::Graph> {
    use apiforge_storage::project::IdentityBase;
    match snapshot.document() {
        IdentityBase::Scenario(scenario) => vec![&scenario.graph, &scenario.cleanup],
        IdentityBase::Component(component) => vec![&component.graph],
    }
}

/// Every call of `graphs`, and of the revisions in `context` those calls
/// pin, transitively: (component ID, pinned revision), each once.
fn pinned_calls<'a>(
    mut graphs: Vec<&'a apiforge_core::format::Graph>,
    context: &'a apiforge_core::format::Project,
) -> std::collections::BTreeSet<(String, Option<u32>)> {
    use apiforge_core::model::NodeKind;
    let mut visited = std::collections::BTreeSet::new();
    while let Some(graph) = graphs.pop() {
        for node in &graph.nodes {
            if node.kind != NodeKind::Component {
                continue;
            }
            let Some(id) = &node.config.component_id else {
                continue;
            };
            let revision = node.config.revision;
            if visited.insert((id.clone(), revision)) {
                graphs.extend(
                    context
                        .components
                        .iter()
                        .filter(|component| {
                            &component.id == id && Some(component.revision) == revision
                        })
                        .map(|component| &component.graph),
                );
            }
        }
    }
    visited
}

/// The published revisions in `context` that `graphs` call, directly or
/// through the revisions their calls pin: (component ID, revision). A call
/// that resolves to no revision of the context adds nothing.
fn called_revisions(
    graphs: Vec<&apiforge_core::format::Graph>,
    context: &apiforge_core::format::Project,
) -> std::collections::BTreeSet<(String, u32)> {
    pinned_calls(graphs, context)
        .into_iter()
        .filter_map(|(id, revision)| {
            let revision = revision?;
            context
                .components
                .iter()
                .any(|component| component.id == id && component.revision == revision)
                .then_some((id, revision))
        })
        .collect()
}

impl PreparedRun {
    /// The published revisions the run calls, directly or through the
    /// revisions its calls pin, as (component ID, revision) (MCP-21). A
    /// component run on its own does not call its own revisions.
    pub fn called_revisions(&self) -> std::collections::BTreeSet<(String, u32)> {
        called_revisions(snapshot_graphs(&self.snapshot), self.snapshot.context())
    }
}

impl ProjectSession {
    /// The published revisions `text` calls in the project's context,
    /// directly or through the revisions its calls pin, as (component ID,
    /// revision) (MCP-21). A text with errors still gives the calls that
    /// resolve; a call that resolves to no published revision adds nothing.
    pub fn called_revisions(&self, text: &str) -> std::collections::BTreeSet<(String, u32)> {
        if text.len() > apiforge_storage::project::MAX_SOURCE_BYTES {
            return std::collections::BTreeSet::new();
        }
        let tree = apiforge_script::parse(text);
        let lowered =
            apiforge_script::lower(&tree, &apiforge_script::LowerContext::folder(&self.context));
        let graphs = match &lowered.document {
            Some(apiforge_script::Document::Scenario(scenario)) => {
                vec![&scenario.graph, &scenario.cleanup]
            }
            Some(apiforge_script::Document::Component(component)) => vec![&component.graph],
            None => Vec::new(),
        };
        called_revisions(graphs, &self.context)
    }
}
