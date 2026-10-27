//! Upgrade of the version-two JSON workspace into folder projects (Stage II
//! plan, "Upgrade migration"). Activation is all-or-nothing: one atomically
//! written catalogue lists every migrated project and is also the completion
//! marker. Original files are never modified. Not used by normal startup yet.
use crate::{Document, Error, ProjectSession, Result, context::base_project, prepare_project};
use apiforge_core::format::{Component, Project, Workspace};
use apiforge_storage::{
    CodeDraft, DraftTarget, WorkspaceStore,
    project::{
        DiagramFile, DiagramRecord, DocumentEntry, DocumentKey, DocumentKind, FieldRecovery,
        FileState, MANIFEST_PATH, ProjectChange, ProjectManifest, ProjectPath, ProjectStore,
        RecoveryDocument, SourceRecovery, TextSelection,
    },
    write_atomic,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error as ThisError;

pub const CATALOGUE_FILE: &str = "projects.json";
/// Managed projects live here, beside the catalogue, in the application data
/// directory; migration writes its destinations into it and startup creates a
/// new project there.
pub const PROJECTS_DIRECTORY: &str = "projects";
const MIGRATION_DIRECTORY: &str = "migration";
const INDEX_FILE: &str = "index.json";
/// Inside each prepared folder; identifies the migration that wrote it.
const MARKER: &str = ".apiforge/local/migration.json";
/// Legacy inputs beside the workspace, all covered by its directory lock.
const INPUTS: [&str; 5] = [
    "workspace.json",
    "workspace.json.bak",
    "drafts.json",
    "drafts.json.bak",
    "journal.json",
];

#[derive(Debug, ThisError)]
pub enum MigrationError {
    #[error("Migration requires the exclusive workspace session.")]
    ReadOnly,
    #[error("There is no workspace to migrate.")]
    NoWorkspace,
    #[error("The workspace needs explicit backup recovery before migration.")]
    WorkspaceRecoveryRequired,
    #[error("Code drafts are unreadable; recover or discard them before migration.")]
    DraftsUnreadable,
    #[error("The workspace changed during migration; nothing was activated.")]
    InputsChanged,
    #[error("A project folder already exists and belongs to something else: {path}")]
    Occupied { path: PathBuf },
    #[error(
        "Migrated project \u{201c}{project_name}\u{201d} ({project_id}) does not match the workspace: {reason}{}.",
        in_scenario(.scenario)
    )]
    Verification {
        project_id: String,
        /// The project's name, as the person knows it.
        project_name: String,
        /// The scenario that did not read back the same, when one did not.
        scenario: Option<String>,
        reason: &'static str,
    },
    #[error("A project ID cannot be used as a folder name: {project_id}")]
    UnportableId { project_id: String },
    #[error("Invalid migration metadata; it was preserved: {path}")]
    InvalidMetadata { path: PathBuf },
    #[error("Cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Workspace(#[from] apiforge_storage::StorageError),
}

fn in_scenario(scenario: &Option<String>) -> String {
    scenario
        .as_ref()
        .map(|name| format!(" (scenario \u{201c}{name}\u{201d})"))
        .unwrap_or_default()
}

fn io(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
    move |source| {
        Error::Migration(MigrationError::Io {
            path: path.to_owned(),
            source,
        })
    }
}

/// Every migrated project and the inputs it was derived from. Its presence
/// means the migration completed, and it is the record of which projects that
/// migration produced. It is not a registry of every folder project: one can
/// be opened from anywhere, and projects created later are not part of any
/// migration, so neither is written back here.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCatalogue {
    pub format_version: u32,
    pub migration_id: String,
    pub inputs: BTreeMap<String, Option<String>>,
    pub projects: Vec<CatalogueEntry>,
    pub active_project_id: Option<String>,
    /// Unresolved runs stay in the original journal: its records name no
    /// project, so they are reviewed globally rather than guessed per project.
    pub legacy_journal: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogueEntry {
    pub project_id: String,
    pub path: PathBuf,
}

impl ProjectCatalogue {
    /// `None` until a migration has completed in this data directory.
    pub fn load(data_directory: &Path) -> Result<Option<Self>> {
        read_json(&data_directory.join(CATALOGUE_FILE))
    }
}

/// Deserialisation alone does not make durable metadata safe to use as paths
/// or as evidence that a migration completed.
trait Metadata {
    fn is_valid(&self) -> bool;
}

impl Metadata for ProjectCatalogue {
    fn is_valid(&self) -> bool {
        let mut ids = HashSet::new();
        self.format_version == 1
            && valid_migration_id(&self.migration_id)
            && valid_inputs(&self.inputs)
            && self.projects.iter().all(|entry| {
                valid_project_id(&entry.project_id)
                    && ids.insert(entry.project_id.to_lowercase())
                    && !entry.path.as_os_str().is_empty()
                    && !entry
                        .path
                        .components()
                        .any(|part| matches!(part, std::path::Component::ParentDir))
            })
            && self
                .active_project_id
                .as_ref()
                .is_none_or(|id| self.projects.iter().any(|entry| entry.project_id == *id))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MigrationIndex {
    format_version: u32,
    migration_id: String,
    inputs: BTreeMap<String, Option<String>>,
    projects: Vec<IndexEntry>,
}

impl Metadata for MigrationIndex {
    fn is_valid(&self) -> bool {
        let mut ids = HashSet::new();
        self.format_version == 1
            && valid_migration_id(&self.migration_id)
            && valid_inputs(&self.inputs)
            && self.projects.iter().all(|entry| {
                valid_project_id(&entry.project_id)
                    && ids.insert(entry.project_id.to_lowercase())
                    && valid_hash(&entry.digest)
            })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IndexEntry {
    project_id: String,
    published: bool,
    /// Digest of the prepared shared files and recovery, excluding locks.
    digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Marker {
    migration_id: String,
    project_id: String,
}

impl Metadata for Marker {
    fn is_valid(&self) -> bool {
        valid_migration_id(&self.migration_id) && valid_project_id(&self.project_id)
    }
}

fn valid_migration_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id)
}

fn valid_project_id(id: &str) -> bool {
    !id.contains('/') && ProjectPath::new(id.to_owned()).is_ok()
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_inputs(inputs: &BTreeMap<String, Option<String>>) -> bool {
    inputs.len() == INPUTS.len()
        && INPUTS.iter().all(|name| {
            inputs
                .get(*name)
                .is_some_and(|hash| hash.as_deref().is_none_or(valid_hash))
        })
}

fn invalid_metadata(path: &Path) -> Error {
    MigrationError::InvalidMetadata {
        path: path.to_owned(),
    }
    .into()
}

#[derive(Debug)]
pub struct MigrationOutcome {
    pub catalogue: ProjectCatalogue,
    /// The catalogue already existed; nothing was done.
    pub already_complete: bool,
    /// A previous attempt of the same migration was continued.
    pub resumed: bool,
    /// Drafts whose project or definition no longer exists. They remain in the
    /// original drafts file, which migration never changes.
    pub unmigrated_drafts: Vec<CodeDraft>,
    /// The untouched original workspace.
    pub original: PathBuf,
}

/// Migrates the workspace of an exclusive legacy session into folders beside
/// it. Repeating the call after an interruption resumes the same migration.
pub fn migrate_workspace(legacy: &WorkspaceStore) -> Result<MigrationOutcome> {
    if legacy.is_read_only() {
        return Err(MigrationError::ReadOnly.into());
    }
    let data = legacy
        .path()
        .parent()
        .ok_or(MigrationError::NoWorkspace)?
        .to_owned();
    check_boundary(&data)?;
    if let Some(catalogue) = ProjectCatalogue::load(&data)? {
        return Ok(MigrationOutcome {
            catalogue,
            already_complete: true,
            resumed: false,
            unmigrated_drafts: Vec::new(),
            original: legacy.path().to_owned(),
        });
    }
    let inputs = input_hashes(&data)?;
    let loaded = legacy.load()?.ok_or(MigrationError::NoWorkspace)?;
    if loaded.recovered.is_some() {
        return Err(MigrationError::WorkspaceRecoveryRequired.into());
    }
    let drafts = match legacy.load_drafts() {
        Ok(None) => Vec::new(),
        Ok(Some(drafts)) if !drafts.recovered => drafts.drafts.entries,
        _ => return Err(MigrationError::DraftsUnreadable.into()),
    };
    let workspace = loaded.workspace;
    for project in &workspace.projects {
        if !valid_project_id(&project.id) {
            return Err(MigrationError::UnportableId {
                project_id: project.id.clone(),
            }
            .into());
        }
    }

    let migration = data.join(MIGRATION_DIRECTORY);
    let index_path = migration.join(INDEX_FILE);
    let previous: Option<MigrationIndex> = read_json(&index_path)?;
    let resumed = previous
        .as_ref()
        .is_some_and(|index| index.inputs == inputs);
    let mut index = match previous.filter(|index| index.inputs == inputs) {
        Some(index) => index,
        None => MigrationIndex {
            format_version: 1,
            migration_id: uuid::Uuid::new_v4().to_string(),
            inputs: inputs.clone(),
            projects: Vec::new(),
        },
    };
    if index.projects.iter().any(|entry| {
        !workspace
            .projects
            .iter()
            .any(|project| project.id == entry.project_id)
    }) {
        return Err(invalid_metadata(&index_path));
    }
    let staging = migration.join(&index.migration_id);
    let projects = data.join(PROJECTS_DIRECTORY);
    check_boundary(&staging)?;
    check_boundary(&projects)?;
    let mut unmigrated = Vec::new();

    for project in &workspace.projects {
        let destination = projects.join(&project.id);
        let folder = staging.join(&project.id);
        check_boundary(&destination)?;
        check_boundary(&folder)?;
        if let Some(position) = index
            .projects
            .iter()
            .position(|entry| entry.project_id == project.id)
            && (index.projects[position].published || destination.exists())
        {
            // rename may have succeeded before published=true reached disk.
            // The marker and recorded digest, not that flag alone, prove this
            // is our own unchanged publication. Foreign changes stay untouched.
            check_owned(
                &destination,
                &index.migration_id,
                &project.id,
                &index.projects[position].digest,
            )?;
            if !index.projects[position].published {
                // Finish durability of the interrupted rename before advancing
                // its index. The staging parent may already have been removed
                // after an earlier successful catalogue activation.
                sync_directory(&projects)?;
                if staging.exists() {
                    sync_directory(&staging)?;
                }
                index.projects[position].published = true;
                write_json(&index_path, &index)?;
            }
            continue;
        }
        if destination.exists() {
            return Err(MigrationError::Occupied { path: destination }.into());
        }
        if folder.exists() {
            fs::remove_dir_all(&folder).map_err(io(&folder))?;
        }
        fs::create_dir_all(&folder).map_err(io(&folder))?;
        let project_drafts: Vec<_> = drafts
            .iter()
            .filter(|draft| draft.project_id == project.id)
            .cloned()
            .collect();
        unmigrated.extend(prepare(&folder, project, &project_drafts)?);
        write_json(
            &folder.join(MARKER),
            &Marker {
                migration_id: index.migration_id.clone(),
                project_id: project.id.clone(),
            },
        )?;
        verify(&folder, project)?;
        let digest = tree_digest(&folder)?;
        index
            .projects
            .retain(|entry| entry.project_id != project.id);
        index.projects.push(IndexEntry {
            project_id: project.id.clone(),
            published: false,
            digest,
        });
        write_json(&index_path, &index)?;
    }
    let known: HashSet<_> = workspace.projects.iter().map(|p| p.id.as_str()).collect();
    unmigrated.extend(
        drafts
            .iter()
            .filter(|draft| !known.contains(draft.project_id.as_str()))
            .cloned(),
    );

    // Nothing is published unless every input is still what was migrated.
    if input_hashes(&data)? != inputs {
        return Err(MigrationError::InputsChanged.into());
    }
    fs::create_dir_all(&projects).map_err(io(&projects))?;
    for position in 0..index.projects.len() {
        if index.projects[position].published {
            continue;
        }
        let project_id = index.projects[position].project_id.clone();
        let folder = staging.join(&project_id);
        let destination = projects.join(&project_id);
        if tree_digest(&folder)? != index.projects[position].digest {
            let project_name = workspace
                .projects
                .iter()
                .find(|project| project.id == project_id)
                .map(|project| project.name.clone())
                .unwrap_or_default();
            return Err(MigrationError::Verification {
                project_id,
                project_name,
                scenario: None,
                reason: "the prepared folder changed before publication",
            }
            .into());
        }
        publish(&folder, &destination)?;
        index.projects[position].published = true;
        write_json(&index_path, &index)?;
    }

    let journal = data.join("journal.json");
    let catalogue = ProjectCatalogue {
        format_version: 1,
        migration_id: index.migration_id.clone(),
        inputs,
        projects: workspace
            .projects
            .iter()
            .map(|project| CatalogueEntry {
                project_id: project.id.clone(),
                path: projects.join(&project.id),
            })
            .collect(),
        active_project_id: active_project(&workspace),
        legacy_journal: journal.exists().then_some(journal),
    };
    write_json(&data.join(CATALOGUE_FILE), &catalogue)?;
    let _ = fs::remove_dir_all(&staging);
    Ok(MigrationOutcome {
        catalogue,
        already_complete: false,
        resumed,
        unmigrated_drafts: unmigrated,
        original: legacy.path().to_owned(),
    })
}

fn active_project(workspace: &Workspace) -> Option<String> {
    workspace
        .projects
        .iter()
        .any(|project| project.id == workspace.active_id)
        .then(|| workspace.active_id.clone())
}

/// Writes the folder and transfers drafts into local recovery. Returns the
/// drafts that name a definition the project no longer has.
fn prepare(folder: &Path, project: &Project, drafts: &[CodeDraft]) -> Result<Vec<CodeDraft>> {
    let mut manifest = prepare_project(folder, project)?;
    let store = ProjectStore::open_exclusive(folder)?;
    let mut context = base_project(&manifest);
    context.components = project.components.clone();
    let mut unmigrated = Vec::new();
    let mut documents: Vec<RecoveryDocument> = Vec::new();
    let mut changes = Vec::new();
    let mut records = Vec::new();

    // Code drafts first, so that field drafts can join their owner's entry.
    let (fields, code): (Vec<_>, Vec<_>) = drafts
        .iter()
        .partition(|draft| matches!(draft.target, DraftTarget::Field { .. }));
    for draft in code {
        let Some(entry) = draft_document(
            &mut manifest,
            &mut changes,
            &mut records,
            &context,
            &draft.target,
        )?
        else {
            unmigrated.push(draft.clone());
            continue;
        };
        documents.push(RecoveryDocument {
            document_id: entry.document_id.clone(),
            entity_id: entry.entity_id.clone(),
            kind: entry.kind,
            base_revision: entry.base_revision,
            path: entry.path.clone(),
            base_hash: Some(draft.base_hash.clone()),
            source_version: 1,
            text: draft.text.clone(),
            selection: TextSelection {
                anchor: draft.cursor,
                caret: draft.cursor,
            },
            fields: Vec::new(),
            checkpoint: None,
        });
    }
    for draft in fields {
        let DraftTarget::Field { owner, widget } = &draft.target else {
            continue;
        };
        let Some(entry) =
            draft_document(&mut manifest, &mut changes, &mut records, &context, owner)?
        else {
            unmigrated.push(draft.clone());
            continue;
        };
        let field = FieldRecovery {
            widget: *widget,
            base_hash: draft.base_hash.clone(),
            text: draft.text.clone(),
            selection: TextSelection {
                anchor: draft.cursor,
                caret: draft.cursor,
            },
        };
        if let Some(document) = documents
            .iter_mut()
            .find(|document| document.document_id == entry.document_id)
        {
            document.fields.push(field);
            continue;
        }
        // The owner has no code draft: recovery keeps its saved text as is.
        let text = definition_source(&context, &entry, project)?;
        documents.push(RecoveryDocument {
            document_id: entry.document_id.clone(),
            entity_id: entry.entity_id.clone(),
            kind: entry.kind,
            base_revision: entry.base_revision,
            path: entry.path.clone(),
            base_hash: sha256(text.as_bytes()),
            source_version: 1,
            text,
            selection: TextSelection {
                anchor: 0,
                caret: 0,
            },
            fields: vec![field],
            checkpoint: None,
        });
    }

    if !changes.is_empty() {
        let (_, snapshot) = store.load()?;
        let diagram_path = ProjectPath::new(".apiforge/diagram.json")?;
        let diagram_bytes =
            snapshot.files[&diagram_path]
                .as_deref()
                .ok_or(Error::Unrepresentable(
                    "the prepared checkpoints are missing",
                ))?;
        let mut diagram = DiagramFile::from_bytes(diagram_bytes)?;
        diagram.records.extend(records);
        changes.push(ProjectChange::Write {
            path: diagram_path.clone(),
            expected: FileState::of(diagram_bytes),
            bytes: diagram.to_bytes()?,
        });
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        changes.push(ProjectChange::Write {
            path: manifest_path.clone(),
            expected: snapshot.state(&manifest_path).unwrap_or(FileState::Missing),
            bytes: manifest.to_public_bytes()?,
        });
        store.commit(changes)?;
    }
    if !documents.is_empty() {
        let preview = store.load_source_recovery(&manifest.id)?;
        store.save_source_recovery(
            &preview,
            &SourceRecovery {
                format_version: 1,
                project_id: manifest.id.clone(),
                documents,
                // A legacy workspace has no project text files.
                texts: Vec::new(),
            },
        )?;
    }
    Ok(unmigrated)
}

/// The registered document a legacy draft belongs to. A draft of an older
/// component revision gets its own recovered working document, based on that
/// revision and never published or rebased automatically.
fn draft_document(
    manifest: &mut ProjectManifest,
    changes: &mut Vec<ProjectChange>,
    records: &mut Vec<DiagramRecord>,
    context: &Project,
    target: &DraftTarget,
) -> Result<Option<DocumentEntry>> {
    match target {
        DraftTarget::Scenario { id } => Ok(manifest
            .documents
            .iter()
            .find(|entry| entry.kind == DocumentKind::Scenario && &entry.entity_id == id)
            .cloned()),
        DraftTarget::Component { id, revision } => {
            let Some(component) = context
                .components
                .iter()
                .find(|component| &component.id == id && component.revision == *revision)
            else {
                return Ok(None);
            };
            if let Some(entry) = manifest
                .documents
                .iter()
                .find(|entry| &entry.entity_id == id && entry.base_revision == Some(*revision))
            {
                return Ok(Some(entry.clone()));
            }
            let entry = recovered_entry(manifest, component)?;
            let text = print(context, component)?;
            // Keep the revision's identities and layout, as for the primary copy.
            let next = context
                .components
                .iter()
                .filter(|c| &c.id == id)
                .map(|c| c.revision)
                .max()
                .unwrap_or(*revision)
                + 1;
            let mut model = component.clone();
            model.revision = next;
            records.push(DiagramRecord::capture(
                DocumentKey::Working {
                    document_id: entry.document_id.clone(),
                },
                &text,
                apiforge_storage::project::IdentityBase::Component(model),
                context,
            )?);
            changes.push(ProjectChange::Write {
                path: entry.path.clone(),
                expected: FileState::Missing,
                bytes: text.into_bytes(),
            });
            manifest.documents.push(entry.clone());
            Ok(Some(entry))
        }
        DraftTarget::Field { .. } => Ok(None),
    }
}

fn recovered_entry(manifest: &ProjectManifest, component: &Component) -> Result<DocumentEntry> {
    let taken: HashSet<_> = manifest
        .documents
        .iter()
        .map(|entry| entry.path.as_str().to_lowercase())
        .collect();
    let stem = format!(
        "components/{}-r{}-recovered",
        component.handle, component.revision
    );
    let path = (1..)
        .map(|suffix| match suffix {
            1 => format!("{stem}.afs"),
            _ => format!("{stem}-{suffix}.afs"),
        })
        .find(|path| !taken.contains(&path.to_lowercase()))
        .expect("an unused name exists");
    Ok(DocumentEntry {
        document_id: uuid::Uuid::new_v4().to_string(),
        kind: DocumentKind::Component,
        path: ProjectPath::working_source(path)?,
        entity_id: component.id.clone(),
        base_revision: Some(component.revision),
        recovered: true,
    })
}

fn definition_source(
    context: &Project,
    entry: &DocumentEntry,
    project: &Project,
) -> Result<String> {
    let document = match entry.kind {
        DocumentKind::Scenario => project
            .scenarios
            .iter()
            .find(|scenario| scenario.id == entry.entity_id)
            .map(|scenario| Document::Scenario(scenario.clone())),
        DocumentKind::Component => context
            .components
            .iter()
            .find(|component| {
                component.id == entry.entity_id && Some(component.revision) == entry.base_revision
            })
            .map(|component| Document::Component(component.clone())),
    }
    .ok_or(Error::UnknownDocument)?;
    apiforge_script::print_folder_document(context, &document).map_err(|_| Error::Unprintable)
}

fn print(context: &Project, component: &Component) -> Result<String> {
    apiforge_script::print_folder_document(context, &Document::Component(component.clone()))
        .map_err(|_| Error::Unprintable)
}

/// Reloads the prepared folder through the folder loader and compares it with
/// the legacy project: every scenario, every published revision and every
/// working component must keep semantics, identities, layout and proxies.
fn verify(folder: &Path, project: &Project) -> Result<()> {
    let failure_in = |scenario: Option<&str>, reason| {
        Error::Migration(MigrationError::Verification {
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            scenario: scenario.map(str::to_owned),
            reason,
        })
    };
    let failure = |reason| failure_in(None, reason);
    let session = ProjectSession::open_without_recovery(folder)?;
    let mut published = session.context().components.clone();
    let mut expected = project.components.clone();
    crate::context::sort_components(&mut published);
    crate::context::sort_components(&mut expected);
    if published != expected {
        return Err(failure("published component revisions differ"));
    }
    for scenario in &project.scenarios {
        let model = session
            .documents()
            .iter()
            .find(|document| {
                document.entry().kind == DocumentKind::Scenario
                    && document.entry().entity_id == scenario.id
            })
            .and_then(|document| document.model());
        if model != Some(&Document::Scenario(scenario.clone())) {
            return Err(failure_in(
                Some(&scenario.name),
                "a scenario differs after reloading",
            ));
        }
    }
    for document in session.documents() {
        let entry = document.entry();
        if entry.kind != DocumentKind::Component {
            continue;
        }
        let Some(Document::Component(working)) = document.model() else {
            return Err(failure("a working component did not load"));
        };
        let base = expected
            .iter()
            .find(|c| c.id == entry.entity_id && Some(c.revision) == entry.base_revision)
            .ok_or_else(|| failure("a working component has no base revision"))?;
        let mut normalised = base.clone();
        normalised.revision = working.revision;
        if working != &normalised {
            return Err(failure(
                "a working component differs from its base revision",
            ));
        }
    }
    if session.manifest().id != project.id
        || session.manifest().documents.len()
            < project.scenarios.len() + count_ids(&project.components)
    {
        return Err(failure("the registry is incomplete"));
    }
    Ok(())
}

fn count_ids(components: &[Component]) -> usize {
    components
        .iter()
        .map(|component| component.id.as_str())
        .collect::<HashSet<_>>()
        .len()
}

fn check_owned(
    destination: &Path,
    migration_id: &str,
    project_id: &str,
    digest: &str,
) -> Result<()> {
    let marker: Option<Marker> = read_json(&destination.join(MARKER))?;
    let owned = marker.is_some_and(|marker| {
        marker.migration_id == migration_id && marker.project_id == project_id
    });
    if !owned || tree_digest(destination)? != digest {
        return Err(MigrationError::Occupied {
            path: destination.to_owned(),
        }
        .into());
    }
    Ok(())
}

/// Moves a prepared folder into place. `rename` replaces an empty directory on
/// some platforms, so an existing destination is refused first.
fn publish(folder: &Path, destination: &Path) -> Result<()> {
    check_boundary(folder)?;
    check_boundary(destination)?;
    if fs::symlink_metadata(destination).is_ok() {
        return Err(MigrationError::Occupied {
            path: destination.to_owned(),
        }
        .into());
    }
    fs::rename(folder, destination).map_err(io(destination))?;
    for directory in [folder.parent(), destination.parent()]
        .into_iter()
        .flatten()
    {
        sync_directory(directory)?;
    }
    Ok(())
}

fn sync_directory(directory: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(io(directory))?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

fn input_hashes(data: &Path) -> Result<BTreeMap<String, Option<String>>> {
    INPUTS
        .iter()
        .map(|name| {
            let path = data.join(name);
            let hash = match fs::read(&path) {
                Ok(bytes) => sha256(&bytes),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(io(&path)(error)),
            };
            Ok(((*name).to_owned(), hash))
        })
        .collect()
}

/// Digest of every file except locks, transaction staging and the marker's
/// own directory metadata, so a published copy can be recognised later.
fn tree_digest(root: &Path) -> Result<String> {
    check_boundary(root)?;
    fn walk(root: &Path, directory: &Path, files: &mut Vec<(String, Vec<u8>)>) -> Result<()> {
        for entry in fs::read_dir(directory).map_err(io(directory))? {
            let entry = entry.map_err(io(directory))?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("inside the root")
                .to_string_lossy()
                .replace('\\', "/");
            if relative == ".apiforge/local/project.lock"
                || relative == ".apiforge/local/transactions"
                || relative.starts_with(".apiforge/local/transactions/")
            {
                continue;
            }
            let kind = entry.file_type().map_err(io(&path))?;
            if kind.is_dir() {
                walk(root, &path, files)?;
            } else if kind.is_file() {
                files.push((relative, fs::read(&path).map_err(io(&path))?));
            } else {
                return Err(invalid_metadata(&path));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort();
    let mut all = Vec::new();
    for (path, bytes) in files {
        all.extend_from_slice(path.as_bytes());
        all.push(0);
        all.extend_from_slice(sha256(&bytes).unwrap_or_default().as_bytes());
        all.push(b'\n');
    }
    Ok(sha256(&all).unwrap_or_default())
}

fn sha256(bytes: &[u8]) -> Option<String> {
    match FileState::of(bytes) {
        FileState::File { sha256, .. } => Some(sha256),
        _ => None,
    }
}

/// Reject links and parent traversal before raw migration filesystem operations.
/// Like folder storage, this guards existing ancestors as well as the leaf.
fn check_boundary(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for part in path.components() {
        if matches!(part, std::path::Component::ParentDir) {
            return Err(invalid_metadata(path));
        }
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                let link = metadata.file_type().is_symlink();
                #[cfg(windows)]
                let link = {
                    use std::os::windows::fs::MetadataExt;
                    link || metadata.file_attributes() & 0x400 != 0
                };
                if link {
                    return Err(invalid_metadata(&current));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(io(&current)(error)),
        }
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned + Metadata>(path: &Path) -> Result<Option<T>> {
    check_boundary(path)?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io(path)(error)),
    };
    let metadata: T =
        apiforge_core::strict_json::from_slice(&bytes).map_err(|_| invalid_metadata(path))?;
    if !metadata.is_valid() {
        return Err(invalid_metadata(path));
    }
    Ok(Some(metadata))
}

fn write_json(path: &Path, value: &(impl Serialize + Metadata)) -> Result<()> {
    check_boundary(path)?;
    if !value.is_valid() {
        return Err(invalid_metadata(path));
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| MigrationError::InvalidMetadata {
        path: path.to_owned(),
    })?;
    write_atomic(path, &bytes).map_err(|error| {
        Error::Migration(MigrationError::Workspace(
            apiforge_storage::StorageError::Document(error),
        ))
    })
}
