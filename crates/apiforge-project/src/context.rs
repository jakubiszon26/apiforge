//! The folder resolution context: public configuration from the manifest plus
//! every published component revision, derived from its immutable source.
use crate::{
    Error, Result,
    analysis::{Target, derive},
    layout::Placer,
};
use apiforge_core::format::{Component, FORMAT_VERSION, Project};
use apiforge_script::Document;
use apiforge_storage::project::{
    DiagramFile, DocumentKey, DocumentKind, ProjectManifest, ProjectPath, ProjectSnapshot,
};

/// Configuration only: no scenarios and no components.
pub(crate) fn base_project(manifest: &ProjectManifest) -> Project {
    let mut environment = manifest.environment.clone();
    environment
        .variables
        .retain(|name, _| !environment.secret_names.contains(name));
    Project {
        version: FORMAT_VERSION,
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        scenarios: Vec::new(),
        components: Vec::new(),
        contracts: manifest.contracts.clone(),
        environment,
    }
}

/// A revision may call any revision published before it, so derive them in
/// rounds until no further revision resolves. A revision that never lowers
/// is an integrity error: the loader never substitutes another definition.
pub(crate) fn resolve(
    manifest: &ProjectManifest,
    snapshot: &ProjectSnapshot,
    diagram: Option<&DiagramFile>,
    placer: &dyn Placer,
) -> Result<Project> {
    let mut project = base_project(manifest);
    let mut pending: Vec<_> = manifest.revisions.iter().collect();
    pending.sort_by_key(|entry| (entry.component_id.clone(), entry.revision));
    while !pending.is_empty() {
        let mut resolved = Vec::new();
        let mut unresolved = Vec::new();
        for entry in pending {
            let text = source(snapshot, &entry.path).ok_or_else(|| Error::InvalidRevision {
                component_id: entry.component_id.clone(),
                revision: entry.revision,
            })?;
            let target = Target {
                kind: DocumentKind::Component,
                entity_id: entry.component_id.clone(),
                revision: entry.revision,
            };
            let key = DocumentKey::Revision {
                component_id: entry.component_id.clone(),
                revision: entry.revision,
            };
            let checkpoint = diagram.and_then(|file| file.records.iter().find(|r| r.key == key));
            match derive(&target, text, &project, None, checkpoint, placer) {
                (_, Some((Document::Component(component), _)))
                    if component.handle == entry.handle =>
                {
                    resolved.push(component);
                }
                _ => unresolved.push(entry),
            }
        }
        if resolved.is_empty() {
            let entry = unresolved[0];
            return Err(Error::InvalidRevision {
                component_id: entry.component_id.clone(),
                revision: entry.revision,
            });
        }
        project.components.extend(resolved);
        pending = unresolved;
    }
    sort_components(&mut project.components);
    Ok(project)
}

pub(crate) fn sort_components(components: &mut [Component]) {
    components.sort_by(|left, right| (&left.id, left.revision).cmp(&(&right.id, right.revision)));
}

/// UTF-8 source of a file in a stable snapshot; `None` when missing or not text.
pub(crate) fn source<'a>(snapshot: &'a ProjectSnapshot, path: &ProjectPath) -> Option<&'a str> {
    let bytes = snapshot.files.get(path)?.as_deref()?;
    std::str::from_utf8(bytes).ok()
}
