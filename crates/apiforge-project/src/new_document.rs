//! Validation of a document that is not registered yet (MCP-00, "Validation
//! before the first write"): the path, the header and the handle a
//! registration would meet, and the text analysed in the project's context as
//! if it were registered. Nothing is written.
use super::{ProjectSession, declaration};
use crate::{
    Result,
    analysis::{AnalysisInput, AnalysisTicket},
};
use apiforge_script::Diagnostic;
use apiforge_storage::project::{
    DocumentEntry, DocumentKind, ProjectError, ProjectPath, ProjectStore,
};

/// Why a registration of the text at the path would be refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistrationProblem {
    /// The path is not a working source: absolute, `..`, inside `.apiforge`,
    /// not portable or not ending in `.afs`.
    PathRejected,
    /// A document is registered under this path, or under one that differs
    /// only in letter case.
    PathRegistered,
    /// A file or directory exists at this path, or at one that differs only
    /// in letter case.
    PathExists,
    /// Another component already declares this handle.
    HandleExists,
    /// The header declares neither a scenario nor a component.
    NoKind,
    /// The header declares another kind than the one asked for.
    KindMismatch,
}

impl RegistrationProblem {
    /// A static sentence; it never quotes the path or the text.
    pub fn message(self) -> &'static str {
        match self {
            Self::PathRejected => {
                "A document path must be relative, portable, outside .apiforge and end in .afs"
            }
            Self::PathRegistered => {
                "A document is already registered under this path or one differing only in case"
            }
            Self::PathExists => "A file already exists at this path or one differing only in case",
            Self::HandleExists => "Another component already uses this handle",
            Self::NoKind => "The header declares neither a scenario nor a component",
            Self::KindMismatch => "The header declares another kind of document",
        }
    }
}

/// What registering a new document with this text would meet.
#[derive(Clone, Debug)]
pub struct NewDocumentCheck {
    /// The kind the header declares.
    pub kind: Option<DocumentKind>,
    /// The handle a component's header declares; `None` for a scenario.
    pub handle: Option<String>,
    pub problems: Vec<RegistrationProblem>,
    /// The diagnostics of the text analysed as that document.
    pub diagnostics: Vec<Diagnostic>,
    /// Diagnostics beyond `MAX_DIAGNOSTICS`, counted and not kept.
    pub omitted_diagnostics: usize,
    /// The text has a valid definition in the project's context.
    pub analysis_valid: bool,
}

impl NewDocumentCheck {
    /// A registration would succeed and the document would be valid.
    pub fn is_valid(&self) -> bool {
        self.problems.is_empty() && self.analysis_valid
    }
}

impl ProjectSession {
    /// Checks `text` as a new document at `path` without registering or
    /// writing anything. `kind`, when given, must be what the header declares.
    pub fn check_new_document(
        &self,
        kind: Option<DocumentKind>,
        path: &str,
        text: &str,
    ) -> Result<NewDocumentCheck> {
        let mut problems = Vec::new();
        let valid_path = ProjectPath::working_source(path).ok();
        match &valid_path {
            Some(path) => {
                let lower = path.as_str().to_lowercase();
                if self
                    .manifest
                    .documents
                    .iter()
                    .any(|entry| entry.path.as_str().to_lowercase() == lower)
                {
                    problems.push(RegistrationProblem::PathRegistered);
                } else if exists(&self.store, path)? {
                    problems.push(RegistrationProblem::PathExists);
                }
            }
            None => problems.push(RegistrationProblem::PathRejected),
        }
        let declared = declaration(text);
        let handle = declared.as_ref().and_then(|d| d.handle.clone());
        match (&declared, kind) {
            (None, _) => problems.push(RegistrationProblem::NoKind),
            (Some(declared), Some(kind)) if declared.kind != kind => {
                problems.push(RegistrationProblem::KindMismatch);
            }
            _ => {}
        }
        if let Some(handle) = &handle
            && self.component_handles().contains(handle)
        {
            problems.push(RegistrationProblem::HandleExists);
        }
        // The text analysed as the document it would become: a registry
        // entry that is never pushed, the project's context, no identity base.
        let entry = DocumentEntry {
            document_id: uuid::Uuid::new_v4().to_string(),
            kind: kind
                .or(declared.as_ref().map(|d| d.kind))
                .unwrap_or(DocumentKind::Scenario),
            path: match valid_path {
                Some(path) => path,
                None => ProjectPath::working_source("new.afs")?,
            },
            entity_id: uuid::Uuid::new_v4().to_string(),
            base_revision: None,
            recovered: false,
        };
        let analysis = AnalysisInput {
            ticket: AnalysisTicket {
                session_id: self.session_id.clone(),
                session_epoch: self.epoch,
                document_id: entry.document_id.clone(),
                source_version: 0,
                context_generation: self.context_generation,
            },
            target: self.target(&entry),
            text: text.into(),
            context: self.context.clone(),
            base: None,
            checkpoint: None,
            placer: self.placer.clone(),
        }
        .run();
        Ok(NewDocumentCheck {
            kind: declared.map(|d| d.kind),
            handle,
            problems,
            analysis_valid: analysis.is_valid(),
            diagnostics: analysis.diagnostics,
            omitted_diagnostics: analysis.omitted_diagnostics,
        })
    }
}

/// A file or directory is at `path`, or at a path that differs from it only
/// in letter case.
fn exists(store: &ProjectStore, path: &ProjectPath) -> Result<bool> {
    let file = store.root().join(path.as_str());
    match std::fs::symlink_metadata(&file) {
        Ok(_) => return Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => return Err(ProjectError::Io { path: file, source }.into()),
    }
    let lower = path.as_str().to_lowercase();
    Ok(store
        .scan_tree()?
        .entries
        .iter()
        .any(|entry| entry.path.to_lowercase() == lower))
}
