//! Portable identity/layout candidates, never a replacement for source semantics.
//! The project layer must check source/context/semantic digests against its own
//! lowering result before reusing a candidate. This module owns the storage DTOs.
use super::{schema, *};
use apiforge_core::format::{Component, FORMAT_VERSION, Project, Scenario};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum DocumentKey {
    Working {
        #[serde(rename = "documentId")]
        document_id: String,
    },
    Revision {
        #[serde(rename = "componentId")]
        component_id: String,
        revision: u32,
    },
}

/// A core model snapshot, including ordering edge IDs, positions, autoPlaced and
/// variable proxies. No independent pinned/layout field or graph implementation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    content = "document",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum IdentityBase {
    Scenario(Scenario),
    Component(Component),
}

impl IdentityBase {
    pub fn entity_id(&self) -> &str {
        match self {
            Self::Scenario(scenario) => &scenario.id,
            Self::Component(component) => &component.id,
        }
    }
    pub fn kind(&self) -> DocumentKind {
        match self {
            Self::Scenario(_) => DocumentKind::Scenario,
            Self::Component(_) => DocumentKind::Component,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagramRecord {
    pub key: DocumentKey,
    pub source_hash: String,
    pub context_digest: String,
    pub model_version: u32,
    pub cache_version: u32,
    pub semantic_digest: String,
    pub identity_base: IdentityBase,
}

impl DiagramRecord {
    /// Structural provenance only. Matching source/context/semantics must be
    /// verified against authoritative source by the project layer before use.
    pub fn validate_metadata(&self) -> Result<()> {
        if self.model_version != FORMAT_VERSION || self.cache_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        if !schema::valid_id(self.identity_base.entity_id())
            || [
                &self.source_hash,
                &self.context_digest,
                &self.semantic_digest,
            ]
            .iter()
            .any(|hash| !schema::valid_hash(hash))
        {
            return Err(ProjectError::InvalidCheckpoint);
        }
        match (&self.key, &self.identity_base) {
            (DocumentKey::Working { document_id }, _) if schema::valid_id(document_id) => {}
            (
                DocumentKey::Revision {
                    component_id,
                    revision,
                },
                IdentityBase::Component(component),
            ) if schema::valid_id(component_id)
                && *revision > 0
                && component_id == &component.id
                && *revision == component.revision => {}
            _ => return Err(ProjectError::InvalidCheckpoint),
        }
        Ok(())
    }

    pub fn matches_document(&self, entry: &DocumentEntry) -> bool {
        self.key
            == DocumentKey::Working {
                document_id: entry.document_id.clone(),
            }
            && self.identity_base.kind() == entry.kind
            && self.identity_base.entity_id() == entry.entity_id
    }

    /// Validate the full graph, bindings and identities against resolved public
    /// dependencies, using the shared core validator. This still does not prove
    /// the stored digests match source; only the project analyser can do that.
    pub fn validate_in_context(&self, context: &Project) -> Result<()> {
        self.validate_metadata()?;
        let mut project = context.clone();
        project.scenarios.clear();
        match &self.identity_base {
            IdentityBase::Scenario(scenario) => project.scenarios.push(scenario.clone()),
            IdentityBase::Component(component) => {
                project.components.retain(|existing| {
                    existing.id != component.id || existing.revision != component.revision
                });
                project.components.push(component.clone());
            }
        }
        apiforge_core::validate_authoring_project(&project)
            .map_err(|_| ProjectError::InvalidCheckpoint)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagramFile {
    pub format_version: u32,
    pub records: Vec<DiagramRecord>,
}

impl DiagramFile {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ProjectError::Limit);
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Version {
            format_version: u32,
        }
        let version: Version = apiforge_core::strict_json::from_slice(bytes)
            .map_err(|_| ProjectError::InvalidCheckpoint)?;
        if version.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        let diagram: Self = apiforge_core::strict_json::from_slice(bytes)
            .map_err(|_| ProjectError::InvalidCheckpoint)?;
        diagram.validate()?;
        Ok(diagram)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut ordered = self.clone();
        ordered
            .records
            .sort_by(|left, right| left.key.cmp(&right.key));
        let bytes = super::codec::encode(&ordered, MAX_JSON_BYTES)?;
        // serde_json encodes non-finite floating point values as null. Never
        // acknowledge metadata that its strict DTO cannot reopen.
        Self::from_bytes(&bytes)?;
        Ok(bytes)
    }

    fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        let mut keys = HashSet::new();
        for record in &self.records {
            record.validate_metadata()?;
            if !keys.insert(&record.key) {
                return Err(ProjectError::InvalidCheckpoint);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "diagram_tests.rs"]
pub(super) mod tests;
