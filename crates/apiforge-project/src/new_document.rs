//! Validation of a document that is not registered yet (MCP-00, "Validation
//! before the first write"): the path, the header and the handle a
//! registration would meet, and the text analysed in the project's context as
//! if it were registered. Nothing is read beyond the registry and the folder
//! tree, and nothing is written.
use super::ProjectSession;
use crate::Result;
use apiforge_script::Diagnostic;
use apiforge_storage::project::DocumentKind;

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
            Self::PathRegistered => "A document is already registered under this path",
            Self::PathExists => "A file already exists at this path",
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
        let _ = (kind, path, text);
        unimplemented!("MCP-03")
    }
}
