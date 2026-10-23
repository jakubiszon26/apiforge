//! GUI-independent project documents, validated imports and expression semantics.
//!
//! Parsing never runs requests. A document becomes available to callers only after
//! structural and graph validation. Native documents use format version 2; frozen
//! v1 documents are handled only by the separate development converter.
pub mod binding;
pub mod block;
pub mod demos;
pub mod edit;
pub mod format;
pub mod graph;
pub mod handle;
pub mod ids;
pub mod js;
pub mod model;
pub mod operator;
pub mod path;
pub mod predicate;
pub mod run;
mod strict_json;
pub mod template;
pub mod template_text;
mod validation;

pub use format::{ImportError, ImportLimits, ValidatedDocument, import_document};

/// Validates an in-memory project with the same rules as import, e.g. before an edit
/// that changes component definitions is committed.
pub fn validate_project(project: &format::Project) -> Result<(), ImportError> {
    validation::validate_project(project, "project")
}

/// Validates an in-memory workspace with the same rules as import.
pub fn validate_workspace(workspace: &format::Workspace) -> Result<(), ImportError> {
    validation::workspace(workspace)
}
