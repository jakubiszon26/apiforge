//! Shared folder registry. Definition semantics are validated by the project
//! loader; these checks cover the durable registry and its identity constraints.
use super::{MAX_JSON_BYTES, ProjectError, ProjectPath, Result};
use apiforge_core::format::{Contract, Environment};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const MANIFEST_PATH: &str = "apiforge.project.json";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectManifest {
    pub format_version: u32,
    pub id: String,
    pub name: String,
    pub environment: Environment,
    pub contracts: Vec<Contract>,
    pub documents: Vec<DocumentEntry>,
    pub revisions: Vec<RevisionEntry>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DocumentKind {
    Scenario,
    Component,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentEntry {
    pub document_id: String,
    pub kind: DocumentKind,
    pub path: ProjectPath,
    pub entity_id: String,
    #[serde(default)]
    pub base_revision: Option<u32>,
    pub recovered: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevisionEntry {
    pub component_id: String,
    pub handle: String,
    pub revision: u32,
    pub path: ProjectPath,
    pub source_hash: String,
}

impl ProjectManifest {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ProjectError::Limit);
        }
        // Inspect the version first, preserving unsupported files rather than
        // interpreting a future schema as corrupt version-one data.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Version {
            format_version: u32,
        }
        if apiforge_core::strict_json::from_slice::<Version>(bytes)
            .map_err(|_| ProjectError::InvalidMetadata)?
            .format_version
            != 1
        {
            return Err(ProjectError::UnsupportedVersion);
        }
        let manifest: Self = apiforge_core::strict_json::from_slice(bytes)
            .map_err(|_| ProjectError::InvalidMetadata)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Public configuration only. Runtime secret substitutions never belong in
    /// managed source, snapshots or this registry.
    pub fn to_public_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut public = self.clone();
        public
            .environment
            .variables
            .retain(|name, _| !self.environment.secret_names.contains(name));
        super::codec::encode(&public, MAX_JSON_BYTES)
    }

    pub fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        let invalid = || ProjectError::InvalidMetadata;
        if !valid_id(&self.id)
            || self.name.encode_utf16().count() > 100_000
            || self.contracts.len() > 200
        {
            return Err(invalid());
        }
        // Public configuration uses the same contract/environment rules as the
        // model, while an authoring registry legitimately has no scenarios yet.
        apiforge_core::validate_authoring_project(&apiforge_core::format::Project {
            version: apiforge_core::format::FORMAT_VERSION,
            id: self.id.clone(),
            name: self.name.clone(),
            environment: self.environment.clone(),
            contracts: self.contracts.clone(),
            scenarios: Vec::new(),
            components: Vec::new(),
        })
        .map_err(|_| ProjectError::InvalidMetadata)?;
        let mut paths = HashSet::new();
        let mut documents = HashSet::new();
        let mut primary = HashSet::new();
        let mut entities = HashMap::new();
        let mut scenarios = 0;
        for entry in &self.documents {
            ProjectPath::working_source(entry.path.as_str())?;
            if !valid_id(&entry.document_id)
                || !valid_id(&entry.entity_id)
                || !documents.insert(&entry.document_id)
                || !paths.insert(entry.path.as_str().to_lowercase())
                || (!entry.recovered && !primary.insert(&entry.entity_id))
                || entry.base_revision == Some(0)
                || (entry.kind == DocumentKind::Scenario
                    && (entry.recovered || entry.base_revision.is_some()))
            {
                return Err(invalid());
            }
            if let Some(kind) = entities.insert(&entry.entity_id, entry.kind)
                && (kind != entry.kind || kind == DocumentKind::Scenario)
            {
                return Err(invalid());
            }
            if entry.kind == DocumentKind::Scenario {
                scenarios += 1;
            }
        }
        let mut keys = HashSet::new();
        let mut handles = HashMap::new();
        let mut components = HashMap::new();
        for entry in &self.revisions {
            if !valid_id(&entry.component_id)
                || entry.revision == 0
                || !apiforge_core::handle::is_valid_handle(&entry.handle)
                || !entry.path.is_revision()
                || !entry.path.as_str().ends_with(".afs")
                || !valid_hash(&entry.source_hash)
                || !keys.insert((&entry.component_id, entry.revision))
                || !paths.insert(entry.path.as_str().to_lowercase())
                || handles
                    .insert(&entry.handle, &entry.component_id)
                    .is_some_and(|id| id != &entry.component_id)
                || components
                    .insert(&entry.component_id, &entry.handle)
                    .is_some_and(|handle| handle != &entry.handle)
                || entities
                    .get(&entry.component_id)
                    .is_some_and(|kind| *kind != DocumentKind::Component)
            {
                return Err(invalid());
            }
        }
        for entry in &self.documents {
            if let Some(revision) = entry.base_revision
                && !keys.contains(&(&entry.entity_id, revision))
            {
                return Err(invalid());
            }
        }
        let working_components = self
            .documents
            .iter()
            .filter(|entry| entry.kind == DocumentKind::Component && !entry.recovered)
            .count();
        if scenarios > 100 || keys.len() > 200 || working_components > 200 {
            return Err(ProjectError::Limit);
        }
        Ok(())
    }
}

pub(super) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.encode_utf16().count() <= 200
        && !["__proto__", "constructor", "prototype"].contains(&id)
}

pub(super) fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
