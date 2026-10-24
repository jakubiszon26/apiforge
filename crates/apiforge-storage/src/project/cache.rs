//! Verified, self-contained last-valid snapshots. They are always candidates for
//! a labelled preview/explicit run decision, never an alternative source loader.
use super::{schema, transaction, *};
use apiforge_core::{
    format::{Graph, Project},
    model::NodeKind,
};
use apiforge_script::{Document, LowerContext};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashSet},
    path::PathBuf,
};

pub(super) const CACHE_PATH: &str = ".apiforge/local/last-valid.json";
const CACHE_VERSION: u32 = 1;
const RESOLUTION_POLICY_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CacheProvenance {
    pub session_epoch: String,
    pub source_version: u64,
    pub saved_source_hash: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CacheEntry {
    project_id: String,
    key: DocumentKey,
    source: String,
    source_hash: String,
    provenance: CacheProvenance,
    model_version: u32,
    cache_version: u32,
    context_digest: String,
    semantic_digest: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    snapshot_digest: String,
    identity_base: IdentityBase,
    /// No scenarios, runtime secrets or implicitly resolved latest definitions.
    context: Project,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CacheFile {
    format_version: u32,
    entries: Vec<CacheEntry>,
}

/// The wire DTO is private. Construction and loading both verify the source,
/// model, closed dependency set, configuration, identities and all digests.
#[derive(Clone)]
pub struct LastValidSnapshot {
    entry: CacheEntry,
}

impl LastValidSnapshot {
    pub fn capture(
        key: DocumentKey,
        source: &str,
        provenance: CacheProvenance,
        document: IdentityBase,
        context: &Project,
    ) -> Result<Self> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(ProjectError::Limit);
        }
        let context = closed_public_context(&document, context)?;
        let mut entry = CacheEntry {
            project_id: context.id.clone(),
            key,
            source: source.into(),
            source_hash: transaction::hash(source.as_bytes()),
            provenance,
            model_version: apiforge_core::format::FORMAT_VERSION,
            cache_version: CACHE_VERSION,
            context_digest: String::new(),
            semantic_digest: String::new(),
            snapshot_digest: String::new(),
            identity_base: document,
            context,
        };
        validate_models(&entry)?;
        entry.context_digest = context_digest(&entry.context)?;
        entry.semantic_digest = semantic_digest(&entry.identity_base, &entry.context)?;
        entry.snapshot_digest = snapshot_digest(&entry)?;
        verify_entry(&entry)?;
        Ok(Self { entry })
    }

    pub fn project_id(&self) -> &str {
        &self.entry.project_id
    }
    pub fn key(&self) -> &DocumentKey {
        &self.entry.key
    }
    pub fn source(&self) -> &str {
        &self.entry.source
    }
    pub fn source_hash(&self) -> &str {
        &self.entry.source_hash
    }
    pub fn provenance(&self) -> &CacheProvenance {
        &self.entry.provenance
    }
    pub fn document(&self) -> &IdentityBase {
        &self.entry.identity_base
    }
    pub fn context(&self) -> &Project {
        &self.entry.context
    }
    pub fn context_digest(&self) -> &str {
        &self.entry.context_digest
    }
    pub fn snapshot_digest(&self) -> &str {
        &self.entry.snapshot_digest
    }
    /// Equality does not make this cached model current. Valid live source must
    /// still be lowered by the project session with its own analysis ticket.
    pub fn matches_source(&self, source: &str) -> bool {
        transaction::hash(source.as_bytes()) == self.entry.source_hash
    }
    pub fn matches_context(&self, context: &Project) -> bool {
        closed_public_context(&self.entry.identity_base, context)
            .and_then(|context| context_digest(&context))
            .is_ok_and(|digest| digest == self.entry.context_digest)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheNotice {
    Invalid,
    Unsupported,
    Foreign,
    TooLarge,
    Unavailable,
}

#[derive(Clone)]
pub struct LastValidRead {
    root: PathBuf,
    project_id: String,
    manifest_state: FileState,
    cache_state: Option<FileState>,
    cache_bytes: Option<Vec<u8>>,
    snapshots: Vec<LastValidSnapshot>,
    notice: Option<CacheNotice>,
    preserved_copy: Option<ProjectPath>,
}

impl LastValidRead {
    pub fn snapshots(&self) -> &[LastValidSnapshot] {
        &self.snapshots
    }
    pub fn notice(&self) -> Option<CacheNotice> {
        self.notice
    }
    pub fn preserved_copy(&self) -> Option<&ProjectPath> {
        self.preserved_copy.as_ref()
    }
}

impl ProjectStore {
    /// Invalid/foreign/future caches are preserved and ignored with a notice.
    /// Shared head/registry errors still block the read; they are not cache loss.
    pub fn load_last_valid_cache(&self, project_id: &str) -> Result<LastValidRead> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        self.load_cache_locked(project_id)
    }

    fn load_cache_locked(&self, project_id: &str) -> Result<LastValidRead> {
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        let cache_path = ProjectPath::new(CACHE_PATH)?;
        let before = self.stable_read(|| Ok(vec![manifest_path.clone()]))?;
        let manifest = manifest_in(&before, project_id)?;
        let mut read = LastValidRead {
            root: self.root().into(),
            project_id: project_id.into(),
            manifest_state: before
                .state(&manifest_path)
                .ok_or(ProjectError::InvalidMetadata)?,
            cache_state: None,
            cache_bytes: None,
            snapshots: Vec::new(),
            notice: None,
            preserved_copy: None,
        };
        let snapshot =
            match self.stable_read(|| Ok(vec![manifest_path.clone(), cache_path.clone()])) {
                Ok(snapshot) => snapshot,
                Err(
                    error @ (ProjectError::Limit
                    | ProjectError::Io { .. }
                    | ProjectError::UnsafePath { .. }),
                ) => {
                    let after = self.stable_read(|| Ok(vec![manifest_path.clone()]))?;
                    manifest_in(&after, project_id)?;
                    if after.generation != before.generation
                        || after.state(&manifest_path) != before.state(&manifest_path)
                    {
                        return Err(ProjectError::Busy);
                    }
                    read.notice = Some(if matches!(error, ProjectError::Limit) {
                        CacheNotice::TooLarge
                    } else {
                        CacheNotice::Unavailable
                    });
                    return Ok(read);
                }
                Err(error) => return Err(error),
            };
        let observed_manifest = manifest_in(&snapshot, project_id)?;
        if observed_manifest != manifest {
            return Err(ProjectError::Busy);
        }
        read.cache_state = snapshot.state(&cache_path);
        let Some(bytes) = snapshot.files[&cache_path].as_deref() else {
            return Ok(read);
        };
        read.cache_bytes = Some(bytes.to_vec());
        match decode_cache(bytes, &manifest) {
            Ok(snapshots) => read.snapshots = snapshots,
            Err(ProjectError::UnsupportedVersion) => read.notice = Some(CacheNotice::Unsupported),
            Err(ProjectError::Limit) => read.notice = Some(CacheNotice::TooLarge),
            Err(ProjectError::ForeignRecovery) => read.notice = Some(CacheNotice::Foreign),
            Err(_) => read.notice = Some(CacheNotice::Invalid),
        }
        Ok(read)
    }

    /// Cache failure is separate from source/recovery saving. New managed cache
    /// bytes never save another document, rewrite source or advance generation.
    pub fn save_last_valid_cache(
        &self,
        preview: &LastValidRead,
        snapshots: &[LastValidSnapshot],
    ) -> Result<LastValidRead> {
        self.save_cache(preview, snapshots, false)
    }

    /// Explicit rebuild keeps the previous uninterpreted bytes in metadata
    /// history. Oversized/unreadable originals need manual export first.
    pub fn replace_last_valid_cache(
        &self,
        preview: &LastValidRead,
        snapshots: &[LastValidSnapshot],
    ) -> Result<LastValidRead> {
        self.save_cache(preview, snapshots, true)
    }

    fn save_cache(
        &self,
        preview: &LastValidRead,
        snapshots: &[LastValidSnapshot],
        replace: bool,
    ) -> Result<LastValidRead> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        if preview.root != self.root() {
            return Err(ProjectError::InvalidCache);
        }
        if preview.notice.is_some() && (!replace || preview.cache_state.is_none()) {
            return Err(ProjectError::CacheReplacementRequired);
        }
        let current = self.load_cache_locked(&preview.project_id)?;
        if current.manifest_state != preview.manifest_state
            || current.cache_state != preview.cache_state
        {
            return Err(ProjectError::Conflict {
                path: self.root().join(CACHE_PATH),
            });
        }
        if current.notice.is_some() && !replace {
            return Err(ProjectError::CacheReplacementRequired);
        }
        let file = CacheFile {
            format_version: CACHE_VERSION,
            entries: snapshots
                .iter()
                .map(|snapshot| snapshot.entry.clone())
                .collect(),
        };
        let bytes = super::codec::encode(&file, MAX_JSON_BYTES)?;
        let manifest_snapshot = self.stable_read(|| Ok(vec![ProjectPath::new(MANIFEST_PATH)?]))?;
        let manifest = manifest_in(&manifest_snapshot, &preview.project_id)?;
        decode_cache(&bytes, &manifest)?;
        self.check_secrets(&bytes)?;
        let preserved_copy = if replace {
            match current.cache_bytes.as_deref() {
                Some(bytes) => {
                    self.preserve_local_metadata(&preview.project_id, CACHE_PATH, bytes)?
                }
                None => None,
            }
        } else {
            None
        };
        // Validation can be substantial. Recheck the exact baseline after it.
        let checked = self.load_cache_locked(&preview.project_id)?;
        if checked.manifest_state != preview.manifest_state
            || checked.cache_state != preview.cache_state
        {
            return Err(ProjectError::Conflict {
                path: self.root().join(CACHE_PATH),
            });
        }
        self.atomic(&super::path::resolve(self.root(), CACHE_PATH)?, &bytes)?;
        let mut saved = self.load_cache_locked(&preview.project_id)?;
        if saved.manifest_state != preview.manifest_state
            || saved.cache_state != Some(FileState::of(&bytes))
            || saved.notice.is_some()
        {
            return Err(ProjectError::Conflict {
                path: self.root().join(CACHE_PATH),
            });
        }
        saved.preserved_copy = preserved_copy;
        Ok(saved)
    }
}

fn manifest_in(snapshot: &ProjectSnapshot, project_id: &str) -> Result<ProjectManifest> {
    let path = ProjectPath::new(MANIFEST_PATH)?;
    let manifest = ProjectManifest::from_bytes(
        snapshot
            .files
            .get(&path)
            .and_then(|bytes| bytes.as_deref())
            .ok_or(ProjectError::InvalidMetadata)?,
    )?;
    if manifest.id != project_id {
        return Err(ProjectError::ForeignRecovery);
    }
    Ok(manifest)
}

fn decode_cache(bytes: &[u8], manifest: &ProjectManifest) -> Result<Vec<LastValidSnapshot>> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(ProjectError::Limit);
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Version {
        format_version: u32,
    }
    let version: Version =
        apiforge_core::strict_json::from_slice(bytes).map_err(|_| ProjectError::InvalidCache)?;
    if version.format_version != CACHE_VERSION {
        return Err(ProjectError::UnsupportedVersion);
    }
    let cache: CacheFile =
        apiforge_core::strict_json::from_slice(bytes).map_err(|_| ProjectError::InvalidCache)?;
    let mut keys = HashSet::new();
    let mut snapshots = Vec::new();
    for entry in cache.entries {
        verify_entry(&entry)?;
        if !keys.insert(entry.key.clone()) {
            return Err(ProjectError::InvalidCache);
        }
        if entry.project_id != manifest.id || !matches_registry(&entry, manifest) {
            return Err(ProjectError::ForeignRecovery);
        }
        snapshots.push(LastValidSnapshot { entry });
    }
    Ok(snapshots)
}

fn matches_registry(entry: &CacheEntry, manifest: &ProjectManifest) -> bool {
    match &entry.key {
        DocumentKey::Working { document_id } => manifest.documents.iter().any(|document| {
            &document.document_id == document_id
                && document.entity_id == entry.identity_base.entity_id()
                && document.kind == entry.identity_base.kind()
        }),
        DocumentKey::Revision {
            component_id,
            revision,
        } => manifest.revisions.iter().any(|published| {
            &published.component_id == component_id
                && published.revision == *revision
                && published.source_hash == entry.source_hash
        }),
    }
}

fn verify_entry(entry: &CacheEntry) -> Result<()> {
    if entry.cache_version != CACHE_VERSION
        || entry.model_version != apiforge_core::format::FORMAT_VERSION
    {
        return Err(ProjectError::UnsupportedVersion);
    }
    if entry.source.len() > MAX_SOURCE_BYTES {
        return Err(ProjectError::Limit);
    }
    if !schema::valid_id(&entry.project_id)
        || entry.project_id != entry.context.id
        || !schema::valid_id(&entry.provenance.session_epoch)
        || entry
            .provenance
            .saved_source_hash
            .as_ref()
            .is_some_and(|hash| !schema::valid_hash(hash))
        || [
            &entry.source_hash,
            &entry.context_digest,
            &entry.semantic_digest,
            &entry.snapshot_digest,
        ]
        .iter()
        .any(|hash| !schema::valid_hash(hash))
        || transaction::hash(entry.source.as_bytes()) != entry.source_hash
    {
        return Err(ProjectError::InvalidCache);
    }
    DiagramRecord {
        key: entry.key.clone(),
        source_hash: entry.source_hash.clone(),
        context_digest: entry.context_digest.clone(),
        model_version: entry.model_version,
        cache_version: entry.cache_version,
        semantic_digest: entry.semantic_digest.clone(),
        identity_base: entry.identity_base.clone(),
    }
    .validate_metadata()?;
    validate_models(entry)?;
    if closed_public_context(&entry.identity_base, &entry.context)? != entry.context
        || context_digest(&entry.context)? != entry.context_digest
        || semantic_digest(&entry.identity_base, &entry.context)? != entry.semantic_digest
        || snapshot_digest(entry)? != entry.snapshot_digest
    {
        return Err(ProjectError::InvalidCache);
    }
    let tree = apiforge_script::parse(&entry.source);
    let context = LowerContext::folder(&entry.context);
    let context = match &entry.identity_base {
        IdentityBase::Component(component) => context.with_component(component),
        _ => context,
    };
    let lowered = apiforge_script::lower(&tree, &context);
    let model = lowered
        .document
        .as_ref()
        .ok_or(ProjectError::InvalidCache)?;
    if lowered.has_errors()
        || apiforge_script::semantic(model, &entry.context)
            != apiforge_script::semantic(&script_document(&entry.identity_base), &entry.context)
    {
        return Err(ProjectError::InvalidCache);
    }
    Ok(())
}

fn validate_models(entry: &CacheEntry) -> Result<()> {
    if !entry.context.scenarios.is_empty()
        || entry
            .context
            .environment
            .secret_names
            .iter()
            .any(|name| entry.context.environment.variables.contains_key(name))
    {
        return Err(ProjectError::InvalidCache);
    }
    apiforge_core::validate_authoring_project(&entry.context)
        .map_err(|_| ProjectError::InvalidCache)?;
    let document = script_document(&entry.identity_base);
    apiforge_script::validate_document_model(&document, &LowerContext::folder(&entry.context))
        .map_err(|_| ProjectError::InvalidCache)
}

pub(super) fn closed_public_context(document: &IdentityBase, context: &Project) -> Result<Project> {
    let mut public = crate::public_project(context);
    public.scenarios.clear();
    let mut required = BTreeSet::new();
    let mut pending = component_calls(document_graphs(document))?;
    while let Some(key) = pending.pop() {
        if !required.insert(key.clone()) {
            continue;
        }
        if required.len() > 200 {
            return Err(ProjectError::Limit);
        }
        let mut definitions = context
            .components
            .iter()
            .filter(|component| component.id == key.0 && component.revision == key.1);
        let component = definitions.next().ok_or(ProjectError::InvalidCache)?;
        if definitions.next().is_some() {
            return Err(ProjectError::InvalidCache);
        }
        pending.extend(component_calls(vec![&component.graph])?);
    }
    public
        .components
        .retain(|component| required.contains(&(component.id.clone(), component.revision)));
    public
        .components
        .sort_by(|left, right| (&left.id, left.revision).cmp(&(&right.id, right.revision)));
    public
        .contracts
        .sort_by(|left, right| left.id.cmp(&right.id));
    public.environment.variables.sort_keys();
    public.environment.secret_names.sort();
    Ok(public)
}

fn component_calls(graphs: Vec<&Graph>) -> Result<Vec<(String, u32)>> {
    let mut calls = Vec::new();
    for graph in graphs {
        for node in &graph.nodes {
            if node.kind == NodeKind::Component {
                calls.push((
                    node.config
                        .component_id
                        .clone()
                        .ok_or(ProjectError::InvalidCache)?,
                    node.config
                        .revision
                        .filter(|revision| *revision > 0)
                        .ok_or(ProjectError::InvalidCache)?,
                ));
            }
        }
    }
    Ok(calls)
}
fn document_graphs(document: &IdentityBase) -> Vec<&Graph> {
    match document {
        IdentityBase::Scenario(scenario) => vec![&scenario.graph, &scenario.cleanup],
        IdentityBase::Component(component) => vec![&component.graph],
    }
}
fn script_document(document: &IdentityBase) -> Document {
    match document {
        IdentityBase::Scenario(scenario) => Document::Scenario(scenario.clone()),
        IdentityBase::Component(component) => Document::Component(component.clone()),
    }
}
pub(super) fn semantic_digest(document: &IdentityBase, context: &Project) -> Result<String> {
    digest_json(&apiforge_script::semantic(
        &script_document(document),
        context,
    ))
}
pub(super) fn context_digest(context: &Project) -> Result<String> {
    let components: Vec<_> = context.components.iter().map(|component| serde_json::json!({ "id":component.id, "revision":component.revision, "handle":component.handle,
        "definition":apiforge_script::semantic(&Document::Component(component.clone()), context) })).collect();
    digest_json(
        &serde_json::json!({"resolutionPolicyVersion":RESOLUTION_POLICY_VERSION,"modelVersion":context.version,"projectId":context.id,
        "environment":context.environment,"contracts":context.contracts,"components":components}),
    )
}
fn snapshot_digest(entry: &CacheEntry) -> Result<String> {
    let mut unsigned = entry.clone();
    unsigned.snapshot_digest.clear();
    digest_json(&unsigned)
}
fn digest_json(value: &impl Serialize) -> Result<String> {
    Ok(transaction::hash(&super::codec::encode(
        value,
        MAX_JSON_BYTES,
    )?))
}

#[cfg(test)]
#[path = "cache_tests.rs"]
pub(super) mod tests;
