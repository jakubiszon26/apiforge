//! Writes a model-backed project as a new folder project in one transaction:
//! canonical scenario sources, working components from their latest revision,
//! immutable published revisions and checkpoints that keep every identity,
//! position and proxy. Migration builds on this; it never touches originals.
use crate::{Error, Result, analysis::identity_base, context::base_project};
use apiforge_core::format::{Component, Project};
use apiforge_script::Document;
use apiforge_storage::project::{
    DiagramFile, DiagramRecord, DocumentEntry, DocumentKey, DocumentKind, FileState, MANIFEST_PATH,
    ProjectChange, ProjectManifest, ProjectPath, ProjectStore, RevisionEntry,
};
use std::{collections::HashSet, path::Path};

/// The folder must not contain a project. Secret values are never written.
pub fn prepare_project(root: impl AsRef<Path>, project: &Project) -> Result<ProjectManifest> {
    let store = ProjectStore::open_exclusive(root)?;
    let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
    if store.read(std::slice::from_ref(&manifest_path))?.files[&manifest_path].is_some() {
        return Err(Error::AlreadyAProject);
    }
    let mut manifest = ProjectManifest {
        format_version: 1,
        id: project.id.clone(),
        name: project.name.clone(),
        environment: project.environment.clone(),
        contracts: project.contracts.clone(),
        documents: Vec::new(),
        revisions: Vec::new(),
    };
    let mut context = base_project(&manifest);
    context.components = project.components.clone();
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut records = Vec::new();
    let mut used = HashSet::new();

    for scenario in &project.scenarios {
        let path = unique_path("scenarios", &slug(&scenario.name, "scenario"), &mut used)?;
        let document = Document::Scenario(scenario.clone());
        let text = print(&context, &document)?;
        let entry = DocumentEntry {
            document_id: uuid::Uuid::new_v4().to_string(),
            kind: DocumentKind::Scenario,
            path: path.clone(),
            entity_id: scenario.id.clone(),
            base_revision: None,
            recovered: false,
        };
        records.push(DiagramRecord::capture(
            working_key(&entry),
            &text,
            identity_base(&document),
            &context,
        )?);
        manifest.documents.push(entry);
        files.push((path, text));
    }
    if !project.scenarios.is_empty() {
        directories.push("scenarios".to_owned());
    }

    let mut components: Vec<&Component> = project.components.iter().collect();
    components.sort_by(|left, right| (&left.id, left.revision).cmp(&(&right.id, right.revision)));
    if !components.is_empty() {
        directories.push(".apiforge/revisions".to_owned());
    }
    let mut published = HashSet::new();
    for component in &components {
        let directory = format!(".apiforge/revisions/{}", component.id);
        let path = ProjectPath::new(format!("{directory}/{}.afs", component.revision))
            .map_err(|_| Error::Unrepresentable("a component ID is not a portable file name"))?;
        if published.insert(component.id.clone()) {
            directories.push(directory);
        }
        let document = Document::Component((*component).clone());
        let text = print(&context, &document)?;
        records.push(DiagramRecord::capture(
            DocumentKey::Revision {
                component_id: component.id.clone(),
                revision: component.revision,
            },
            &text,
            identity_base(&document),
            &context,
        )?);
        manifest.revisions.push(RevisionEntry {
            component_id: component.id.clone(),
            handle: component.handle.clone(),
            revision: component.revision,
            path: path.clone(),
            source_hash: sha256(&text),
        });
        files.push((path, text));
    }

    // Each component gets a working document starting at its latest revision.
    // Its model carries the revision its next publication would receive.
    let mut working = HashSet::new();
    for component in components.iter().rev() {
        if !working.insert(component.id.clone()) {
            continue;
        }
        let path = unique_path(
            "components",
            &slug(&component.handle, "component"),
            &mut used,
        )?;
        let text = print(&context, &Document::Component((*component).clone()))?;
        let mut model = (*component).clone();
        model.revision += 1;
        let entry = DocumentEntry {
            document_id: uuid::Uuid::new_v4().to_string(),
            kind: DocumentKind::Component,
            path: path.clone(),
            entity_id: component.id.clone(),
            base_revision: Some(component.revision),
            recovered: false,
        };
        records.push(DiagramRecord::capture(
            working_key(&entry),
            &text,
            identity_base(&Document::Component(model)),
            &context,
        )?);
        manifest.documents.push(entry);
        files.push((path, text));
    }
    if !working.is_empty() {
        directories.push("components".to_owned());
    }

    let mut changes: Vec<_> = directories
        .into_iter()
        .map(|directory| {
            Ok(ProjectChange::CreateDirectory {
                path: ProjectPath::new(directory)?,
            })
        })
        .collect::<Result<_>>()?;
    for (path, text) in files {
        changes.push(ProjectChange::Write {
            path,
            expected: FileState::Missing,
            bytes: text.into_bytes(),
        });
    }
    let diagram = DiagramFile {
        format_version: 1,
        records,
    };
    changes.push(ProjectChange::Write {
        path: ProjectPath::new(".apiforge/diagram.json")?,
        expected: FileState::Missing,
        bytes: diagram.to_bytes()?,
    });
    changes.push(ProjectChange::Write {
        path: manifest_path,
        expected: FileState::Missing,
        bytes: manifest.to_public_bytes()?,
    });
    changes.extend(crate::session::ignore_local_data(&store)?);
    changes.extend(crate::git_rules::lf_attributes(&store)?);
    store.commit(changes)?;
    Ok(manifest)
}

fn print(context: &Project, document: &Document) -> Result<String> {
    apiforge_script::print_folder_document(context, document).map_err(|_| Error::Unprintable)
}

fn sha256(text: &str) -> String {
    match FileState::of(text.as_bytes()) {
        FileState::File { sha256, .. } => sha256,
        _ => unreachable!("bytes always have a file state"),
    }
}

fn working_key(entry: &DocumentEntry) -> DocumentKey {
    DocumentKey::Working {
        document_id: entry.document_id.clone(),
    }
}

fn slug(name: &str, fallback: &str) -> String {
    let mut slug = String::new();
    for character in name.to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_end_matches('-').chars().take(60).collect();
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        fallback.into()
    } else {
        slug.into()
    }
}

fn unique_path(directory: &str, stem: &str, used: &mut HashSet<String>) -> Result<ProjectPath> {
    let mut suffix = 1;
    loop {
        let name = if suffix == 1 {
            format!("{directory}/{stem}.afs")
        } else {
            format!("{directory}/{stem}-{suffix}.afs")
        };
        // Reserved device names such as `con` become valid with a suffix.
        if let Ok(path) = ProjectPath::working_source(name.clone())
            && used.insert(name)
        {
            return Ok(path);
        }
        suffix += 1;
    }
}
