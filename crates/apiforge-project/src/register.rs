//! Registering sources: initialising a folder that already holds `.afs` files
//! and importing files from elsewhere. Registration never rewrites a source in
//! place; import copies it with new identity and never publishes a component.
use super::{EventKind, Origin, ProjectSession, document_from, has_manifest, ignore_local_data};
use crate::{Error, Result, layout::FixedPlacer};
use apiforge_script::lexer::{TokenKind, lex};
use apiforge_storage::project::{
    DocumentEntry, DocumentKind, FileState, MANIFEST_PATH, MAX_SOURCE_BYTES, ProjectChange,
    ProjectError, ProjectPath, ProjectStore, TreeEntryKind,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

/// What a source's header declares. Invalid source still declares its kind
/// when the header itself is readable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub kind: DocumentKind,
    /// The component handle, when the header names one.
    pub handle: Option<String>,
}

/// Reads the declaration from the first tokens of the source.
pub fn declaration(text: &str) -> Option<Declaration> {
    let (tokens, _) = lex(text);
    let mut words = tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment));
    let first = words.next()?;
    match (&text[first.range.clone()], first.kind) {
        ("scenario", TokenKind::Identifier) => Some(Declaration {
            kind: DocumentKind::Scenario,
            handle: None,
        }),
        ("component", TokenKind::Identifier) => Some(Declaration {
            kind: DocumentKind::Component,
            handle: words
                .next()
                .filter(|token| token.kind == TokenKind::Identifier)
                .map(|token| text[token.range.clone()].to_owned()),
        }),
        _ => None,
    }
}

/// An unregistered `.afs` file found in a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSurvey {
    pub path: String,
    /// `None` when the header is unrecognisable or the file is not UTF-8 text:
    /// registration then needs an explicit kind.
    pub declaration: Option<Declaration>,
}

/// One source to register when initialising a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
    pub path: String,
    pub kind: DocumentKind,
}

/// A file offered for import, read once from outside the project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportCandidate {
    pub source: PathBuf,
    pub text: String,
    pub declaration: Option<Declaration>,
    /// An unused path in the chosen folder, also unique among the candidates.
    pub destination: String,
    /// The declared handle is already used by another component.
    pub handle_collision: bool,
}

/// One accepted import: the source text, its kind, where it goes and, for a
/// component whose handle collides, the handle to declare instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportItem {
    pub text: String,
    pub kind: DocumentKind,
    pub destination: String,
    pub handle: Option<String>,
}

impl ProjectSession {
    /// Lists `.afs` files in a folder without a project, for an explicit
    /// Initialise decision. Nothing is written.
    pub fn survey(root: impl AsRef<Path>) -> Result<Vec<SourceSurvey>> {
        let root = root.as_ref();
        if has_manifest(root)? {
            return Err(Error::AlreadyAProject);
        }
        let store = ProjectStore::open_read_only(root)?;
        let scan = store.scan_tree()?;
        let mut surveys = Vec::new();
        for entry in scan.entries {
            if !entry.portable
                || !entry.path.ends_with(".afs")
                || !matches!(entry.kind, TreeEntryKind::File { .. })
                || ProjectPath::working_source(entry.path.as_str()).is_err()
            {
                continue;
            }
            let path = ProjectPath::new(entry.path.as_str())?;
            let snapshot = store.read(std::slice::from_ref(&path))?;
            let declaration = snapshot.files[&path]
                .as_deref()
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .and_then(declaration);
            surveys.push(SourceSurvey {
                path: entry.path,
                declaration,
            });
        }
        Ok(surveys)
    }

    /// Writes a new manifest registering existing sources exactly as they are.
    /// The folder must not contain a project, and every source must exist.
    pub fn initialise(
        root: impl AsRef<Path>,
        name: &str,
        registrations: &[Registration],
    ) -> Result<Self> {
        let root = root.as_ref();
        if has_manifest(root)? {
            return Err(Error::AlreadyAProject);
        }
        let store = ProjectStore::open_exclusive(root)?;
        let paths = registrations
            .iter()
            .map(|registration| ProjectPath::working_source(registration.path.as_str()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let snapshot = store.read(&paths)?;
        let mut manifest = super::empty_manifest(name);
        for (registration, path) in registrations.iter().zip(paths) {
            if !matches!(snapshot.state(&path), Some(FileState::File { .. })) {
                return Err(Error::Storage(ProjectError::Conflict {
                    path: root.join(path.as_str()),
                }));
            }
            manifest.documents.push(DocumentEntry {
                document_id: uuid::Uuid::new_v4().to_string(),
                kind: registration.kind,
                path,
                entity_id: uuid::Uuid::new_v4().to_string(),
                base_revision: None,
                recovered: false,
            });
        }
        let mut changes = vec![ProjectChange::Write {
            path: ProjectPath::new(MANIFEST_PATH)?,
            expected: FileState::Missing,
            bytes: manifest.to_public_bytes()?,
        }];
        changes.extend(ignore_local_data(&store)?);
        store.commit(changes)?;
        Self::load(store, Arc::new(FixedPlacer), 1)
    }

    /// Reads files for import into `folder` and proposes destinations. Files
    /// that are not UTF-8 text or exceed the source limit are refused.
    pub fn prepare_import(&self, files: &[PathBuf], folder: &str) -> Result<Vec<ImportCandidate>> {
        let folder = folder.trim_matches('/');
        if !folder.is_empty() {
            ProjectPath::new(folder)?;
        }
        let mut handles = self.component_handles();
        let mut taken: HashSet<String> = self
            .store
            .scan_tree()?
            .entries
            .into_iter()
            .map(|entry| entry.path.to_lowercase())
            .collect();
        let mut candidates = Vec::new();
        for file in files {
            let metadata = std::fs::metadata(file).map_err(|error| ProjectError::Io {
                path: file.clone(),
                source: error,
            })?;
            if metadata.len() > MAX_SOURCE_BYTES as u64 {
                return Err(Error::SourceTooLarge);
            }
            let bytes = std::fs::read(file).map_err(|error| ProjectError::Io {
                path: file.clone(),
                source: error,
            })?;
            if bytes.len() > MAX_SOURCE_BYTES {
                return Err(Error::SourceTooLarge);
            }
            let text = String::from_utf8(bytes).map_err(|_| Error::NotText(file.clone()))?;
            let declaration = declaration(&text);
            let handle_collision = match declaration.as_ref().and_then(|d| d.handle.as_ref()) {
                Some(handle) => !handles.insert(handle.clone()),
                None => false,
            };
            let stem = file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .filter(|stem| ProjectPath::new(format!("{stem}.afs")).is_ok())
                .unwrap_or("imported");
            let destination = unused(&taken, folder, stem);
            taken.insert(destination.to_lowercase());
            candidates.push(ImportCandidate {
                source: file.clone(),
                text,
                declaration,
                destination,
                handle_collision,
            });
        }
        Ok(candidates)
    }

    /// Registers copies in one transaction, each with a new document and
    /// entity ID. Nothing is overwritten and nothing is published.
    pub fn import(&mut self, items: &[ImportItem], origin: Origin) -> Result<Vec<String>> {
        if self.store.is_read_only() {
            return Err(Error::Storage(ProjectError::ReadOnly));
        }
        self.require_current_registry()?;
        let mut handles = self.component_handles();
        let mut destinations = HashSet::new();
        let mut prepared = Vec::new();
        for item in items {
            let path = ProjectPath::working_source(item.destination.as_str())?;
            if !destinations.insert(path.as_str().to_lowercase())
                || self.store.file_state(&path)? != FileState::Missing
            {
                return Err(Error::Storage(ProjectError::Conflict {
                    path: self.store.root().join(path.as_str()),
                }));
            }
            let mut text = item.text.clone();
            if let Some(handle) = &item.handle {
                text = rename_handle(&text, handle).ok_or(Error::KindMismatch)?;
            }
            if text.len() > MAX_SOURCE_BYTES {
                return Err(Error::SourceTooLarge);
            }
            let declared = declaration(&text);
            if declared.as_ref().is_some_and(|d| d.kind != item.kind) {
                return Err(Error::KindMismatch);
            }
            if let Some(handle) = declared.and_then(|d| d.handle)
                && !handles.insert(handle.clone())
            {
                return Err(Error::HandleCollision(handle));
            }
            prepared.push((path, text, item.kind));
        }
        let mut changes = Vec::new();
        let mut created = HashSet::new();
        let mut entries = Vec::new();
        for (path, text, kind) in prepared {
            for change in self.parent_directories(&path)? {
                if let ProjectChange::CreateDirectory { path } = &change
                    && !created.insert(path.clone())
                {
                    continue;
                }
                changes.push(change);
            }
            changes.push(ProjectChange::Write {
                path: path.clone(),
                expected: FileState::Missing,
                bytes: text.clone().into_bytes(),
            });
            let entry = DocumentEntry {
                document_id: self.new_document_id(),
                kind,
                path,
                entity_id: uuid::Uuid::new_v4().to_string(),
                base_revision: None,
                recovered: false,
            };
            entries.push((entry, text));
        }
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        // Registration uses the saved registry; unsaved configuration stays
        // unsaved.
        let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
        let snapshot = self.store.read(std::slice::from_ref(&manifest_path))?;
        let mut registry = self.saved_manifest(&snapshot.files[&manifest_path])?;
        registry
            .documents
            .extend(entries.iter().map(|(entry, _)| entry.clone()));
        changes.push(ProjectChange::Write {
            path: manifest_path,
            expected: self.manifest_saved.clone(),
            bytes: registry.to_public_bytes()?,
        });
        self.store.commit(changes)?;
        self.manifest_saved = FileState::of(&registry.to_public_bytes()?);
        self.manifest.documents = registry.documents;
        self.refresh_local_baselines();
        let mut ids = Vec::new();
        for (entry, text) in entries {
            let id = entry.document_id.clone();
            let mut snapshot = apiforge_storage::project::ProjectSnapshot {
                generation: 0,
                files: Default::default(),
            };
            snapshot
                .files
                .insert(entry.path.clone(), Some(text.into_bytes()));
            self.documents.push(document_from(&snapshot, entry));
            self.emit(origin, Some(&id), EventKind::DocumentRegistered);
            self.analyse_now(self.documents.len() - 1)?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Handles of published revisions and of every working component's header.
    fn component_handles(&self) -> HashSet<String> {
        let mut handles: HashSet<String> = self
            .manifest
            .revisions
            .iter()
            .map(|revision| revision.handle.clone())
            .collect();
        for document in &self.documents {
            if document.entry.kind == DocumentKind::Component
                && let Some(handle) = declaration(&document.text).and_then(|d| d.handle)
            {
                handles.insert(handle);
            }
        }
        handles
    }
}

/// Replaces the handle in a component header; `None` when the source has no
/// component header or the handle is not valid.
pub fn rename_handle(text: &str, handle: &str) -> Option<String> {
    if !apiforge_core::handle::is_valid_handle(handle) {
        return None;
    }
    let (tokens, _) = lex(text);
    let mut words = tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment));
    let first = words.next()?;
    if &text[first.range.clone()] != "component" {
        return None;
    }
    let current = words
        .next()
        .filter(|token| token.kind == TokenKind::Identifier)?;
    let mut renamed = text.to_owned();
    renamed.replace_range(current.range.clone(), handle);
    Some(renamed)
}

fn unused(taken: &HashSet<String>, folder: &str, stem: &str) -> String {
    let join = |name: String| {
        if folder.is_empty() {
            name
        } else {
            format!("{folder}/{name}")
        }
    };
    let mut candidate = join(format!("{stem}.afs"));
    let mut number = 1;
    while taken.contains(&candidate.to_lowercase()) {
        number += 1;
        candidate = join(format!("{stem}-{number}.afs"));
    }
    candidate
}
