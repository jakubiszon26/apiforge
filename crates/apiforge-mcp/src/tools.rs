//! One plain function per tool. Each call opens a fresh read-only session on
//! the saved project: the stable read guards against a transaction in
//! progress, and nothing is written, not even `.apiforge/local`.
use crate::{
    error::{ErrorCode, ToolError},
    folder,
    positions::Lines,
};
use apiforge_project::{DocumentKind, DocumentSession, ProjectSession, Unavailable};
use apiforge_script::MAX_DIAGNOSTICS;
use apiforge_storage::project::ProjectError;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;
use std::path::Path;

/// A result larger than this is refused with `result_too_large`, never cut.
pub const MAX_RESULT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Scenario,
    Component,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// Who holds the project's writer lock right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Holder {
    None,
    AnotherProcess,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ContractSummary {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DocumentSummary {
    pub kind: Kind,
    /// The registered path every tool names the document by.
    pub path: String,
    /// The component handle from the source header; absent for a scenario.
    pub handle: Option<String>,
    /// The published revision a working component is based on.
    pub base_revision: Option<u32>,
    /// SHA-256 of the saved bytes; absent when the file is missing.
    pub source_sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RevisionSummary {
    pub handle: String,
    pub revision: u32,
    pub path: String,
    pub source_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct LiveRequests {
    pub allowed: bool,
    /// Why live requests are allowed or not.
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ProjectOverview {
    pub name: String,
    pub base_url: String,
    /// Environment variable names; values are not returned.
    pub variables: Vec<String>,
    /// Secret names only; values never leave the server.
    pub secret_names: Vec<String>,
    pub contracts: Vec<ContractSummary>,
    pub documents: Vec<DocumentSummary>,
    pub revisions: Vec<RevisionSummary>,
    /// The project's head generation; it advances on every commit.
    pub generation: u64,
    pub holder: Holder,
    pub live_requests: LiveRequests,
    pub server_version: String,
    /// The version of the language reference; empty until it is served.
    pub reference_version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Fix {
    pub replacement: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

/// Positions are 1-based and counted in Unicode scalar values; the end is
/// exclusive; a `\r` before `\n` belongs to the line break.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub severity: Severity,
    pub message: String,
    pub fix: Option<Fix>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SourceText {
    pub path: String,
    pub text: String,
    pub source_sha256: Option<String>,
    pub generation: u64,
    /// The saved source has a valid definition (warnings allowed).
    pub valid: bool,
    /// `missing`, `not_utf8` or `too_large` when the saved file cannot be
    /// read as source; the text is then empty.
    pub unavailable: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
    /// Diagnostics beyond the 1 000 per document the analysis keeps.
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DocumentCheck {
    pub path: String,
    pub source_sha256: Option<String>,
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CheckResult {
    pub generation: u64,
    /// Every checked document has a valid definition.
    pub valid: bool,
    pub documents: Vec<DocumentCheck>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Printed {
    pub path: String,
    /// The canonical text, byte for byte what `apiforge print` prints.
    pub text: String,
    pub source_sha256: Option<String>,
    pub generation: u64,
}

pub fn project_overview(folder: &Path) -> Result<ProjectOverview, ToolError> {
    let session = open(folder)?;
    let manifest = session.manifest();
    let holder = match session.store().has_live_writer() {
        Ok(false) => Holder::None,
        Ok(true) => Holder::AnotherProcess,
        Err(error) => return Err(storage_error(apiforge_project::Error::Storage(error))),
    };
    bounded(ProjectOverview {
        name: manifest.name.clone(),
        base_url: manifest.environment.base_url.clone(),
        variables: manifest.environment.variables.keys().cloned().collect(),
        secret_names: manifest.environment.secret_names.clone(),
        contracts: manifest
            .contracts
            .iter()
            .map(|contract| ContractSummary {
                id: contract.id.clone(),
                name: contract.name.clone(),
            })
            .collect(),
        documents: session
            .documents()
            .iter()
            .map(|document| DocumentSummary {
                kind: match document.entry().kind {
                    DocumentKind::Scenario => Kind::Scenario,
                    DocumentKind::Component => Kind::Component,
                },
                path: document.entry().path.as_str().to_owned(),
                handle: match document.entry().kind {
                    DocumentKind::Scenario => None,
                    DocumentKind::Component => apiforge_project::declaration(document.text())
                        .and_then(|declaration| declaration.handle),
                },
                base_revision: document.entry().base_revision,
                source_sha256: document.saved_sha256().map(str::to_owned),
            })
            .collect(),
        revisions: manifest
            .revisions
            .iter()
            .map(|revision| RevisionSummary {
                handle: revision.handle.clone(),
                revision: revision.revision,
                path: revision.path.as_str().to_owned(),
                source_sha256: revision.source_hash.clone(),
            })
            .collect(),
        generation: session.loaded_generation(),
        holder,
        live_requests: LiveRequests {
            allowed: false,
            source: "no --allow-live".into(),
        },
        server_version: env!("CARGO_PKG_VERSION").into(),
        reference_version: String::new(),
    })
}

pub fn read_source(folder: &Path, path: &str) -> Result<SourceText, ToolError> {
    let (session, id) = open_document(folder, path)?;
    let (document, valid) = folder::check_document(&session, &id)?;
    let (diagnostics, omitted) = diagnostics(document);
    bounded(SourceText {
        path: document.entry().path.as_str().to_owned(),
        text: match document.unavailable() {
            None => document.text().to_owned(),
            Some(_) => String::new(),
        },
        source_sha256: document.saved_sha256().map(str::to_owned),
        generation: session.loaded_generation(),
        valid,
        unavailable: document.unavailable().map(|unavailable| {
            match unavailable {
                Unavailable::Missing => "missing",
                Unavailable::NotUtf8 => "not_utf8",
                Unavailable::TooLarge => "too_large",
            }
            .to_owned()
        }),
        diagnostics,
        omitted,
    })
}

pub fn check(folder: &Path, path: Option<&str>) -> Result<CheckResult, ToolError> {
    let (session, selected) = match path {
        Some(path) => {
            let (session, id) = open_document(folder, path)?;
            (session, Some(id))
        }
        None => (open(folder)?, None),
    };
    let mut documents = Vec::new();
    for document in session
        .documents()
        .iter()
        .filter(|document| selected.as_deref().is_none_or(|id| document.id() == id))
    {
        let (document, valid) = folder::check_document(&session, document.id())?;
        let (diagnostics, omitted) = diagnostics(document);
        documents.push(DocumentCheck {
            path: document.entry().path.as_str().to_owned(),
            source_sha256: document.saved_sha256().map(str::to_owned),
            valid,
            diagnostics,
            omitted,
        });
    }
    bounded(CheckResult {
        generation: session.loaded_generation(),
        valid: documents.iter().all(|document| document.valid),
        documents,
    })
}

pub fn print(folder: &Path, path: &str) -> Result<Printed, ToolError> {
    let (session, id) = open_document(folder, path)?;
    let (document, valid) = folder::check_document(&session, &id)?;
    let invalid = |message: &str, diagnostics: Vec<Diagnostic>, omitted: usize| {
        ToolError::new(ErrorCode::InvalidText, message)
            .with_details(json!({ "diagnostics": diagnostics, "omitted": omitted }))
    };
    let model = match document.model() {
        Some(model) if valid => model,
        _ => {
            let (diagnostics, omitted) = diagnostics(document);
            return Err(invalid(
                "The saved source has no valid definition",
                diagnostics,
                omitted,
            ));
        }
    };
    let text =
        apiforge_script::print_folder_document(session.context(), model).map_err(|found| {
            let lines = Lines::new(document.text());
            let diagnostics = found.iter().map(|d| diagnostic(&lines, d)).collect();
            invalid("Cannot print the definition", diagnostics, 0)
        })?;
    bounded(Printed {
        path: document.entry().path.as_str().to_owned(),
        text,
        source_sha256: document.saved_sha256().map(str::to_owned),
        generation: session.loaded_generation(),
    })
}

/// A fresh read-only session on the saved project. Creates nothing.
fn open(folder: &Path) -> Result<ProjectSession, ToolError> {
    ProjectSession::open_saved_read_only(folder).map_err(storage_error)
}

/// The path is checked before the folder is touched.
fn open_document(folder: &Path, path: &str) -> Result<(ProjectSession, String), ToolError> {
    folder::validate(path)?;
    let session = open(folder)?;
    let id = folder::select(&session, path)?;
    Ok((session, id))
}

fn storage_error(error: apiforge_project::Error) -> ToolError {
    let code = match &error {
        apiforge_project::Error::NotAProject => ErrorCode::NotAProject,
        apiforge_project::Error::Storage(ProjectError::RecoveryRequired) => {
            ErrorCode::RecoveryRequired
        }
        _ => ErrorCode::ProjectUnreadable,
    };
    ToolError::new(code, error.to_string())
}

/// The kept diagnostics with positions, and how many the analysis omitted.
fn diagnostics(document: &DocumentSession) -> (Vec<Diagnostic>, usize) {
    let lines = Lines::new(document.text());
    let found = document.diagnostics();
    let kept = &found[..found.len().min(MAX_DIAGNOSTICS)];
    (
        kept.iter().map(|d| diagnostic(&lines, d)).collect(),
        document.omitted_diagnostics() + (found.len() - kept.len()),
    )
}

fn diagnostic(lines: &Lines<'_>, found: &apiforge_script::Diagnostic) -> Diagnostic {
    let (line, column, end_line, end_column) = lines.range(found.range.clone());
    Diagnostic {
        line,
        column,
        end_line,
        end_column,
        severity: match found.severity {
            apiforge_script::Severity::Error => Severity::Error,
            apiforge_script::Severity::Warning => Severity::Warning,
        },
        message: found.message.clone(),
        fix: found.fix.as_ref().map(|fix| {
            let (line, column, end_line, end_column) = lines.range(fix.range.clone());
            Fix {
                replacement: fix.replacement.clone(),
                line,
                column,
                end_line,
                end_column,
            }
        }),
    }
}

/// Refuses a result over `MAX_RESULT_BYTES` as the model will read it.
fn bounded<T: Serialize>(result: T) -> Result<T, ToolError> {
    let size = serde_json::to_vec_pretty(&result).map_or(usize::MAX, |bytes| bytes.len());
    if size > MAX_RESULT_BYTES {
        return Err(ToolError::new(
            ErrorCode::ResultTooLarge,
            "The result exceeds 2 MiB; ask for one document at a time",
        )
        .with_details(json!({ "bytes": size, "limit": MAX_RESULT_BYTES })));
    }
    Ok(result)
}
