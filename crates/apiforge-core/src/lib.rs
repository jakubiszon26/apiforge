//! GUI-independent project documents, validated imports and expression semantics.
//!
//! Parsing never runs requests. A document becomes available to callers only after
//! structural and graph validation; the v1 DTO stays intact for lossless round trips.
pub mod binding;
pub mod format;
pub mod graph;
pub mod ids;
pub mod js;
pub mod model;
pub mod operator;
pub mod path;
pub mod predicate;
pub mod run;
mod strict_json;
pub mod template;
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
