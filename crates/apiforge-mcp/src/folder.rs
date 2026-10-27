//! Document selection shared by the folder commands of the CLI and the tools.
use crate::error::{ErrorCode, ToolError};
use apiforge_project::{AnalysisState, DocumentSession, ProjectSession};
use apiforge_storage::project::ProjectPath;
use serde_json::json;

/// A path argument must name a working source: relative, portable, outside
/// `.apiforge`, ending in `.afs`. The message never quotes the argument.
pub fn validate(path: &str) -> Result<ProjectPath, ToolError> {
    ProjectPath::working_source(path)
        .map_err(|error| ToolError::new(ErrorCode::PathRejected, error.to_string()))
}

/// The document id registered under `path`.
pub fn select(session: &ProjectSession, path: &str) -> Result<String, ToolError> {
    let path = validate(path)?;
    session
        .documents()
        .iter()
        .find(|document| document.entry().path == path)
        .map(|document| document.id().to_owned())
        .ok_or_else(|| {
            let registered: Vec<_> = session
                .documents()
                .iter()
                .map(|document| document.entry().path.as_str())
                .collect();
            ToolError::new(
                ErrorCode::UnknownDocument,
                "No registered document has this path",
            )
            .with_details(json!({ "registered_paths": registered }))
        })
}

/// The document and whether its saved source has a valid definition.
pub fn check_document<'a>(
    session: &'a ProjectSession,
    id: &str,
) -> Result<(&'a DocumentSession, bool), ToolError> {
    let document = session
        .document(id)
        .ok_or_else(|| ToolError::new(ErrorCode::UnknownDocument, "Unknown document"))?;
    let valid = document.analysis() == AnalysisState::Valid && document.unavailable().is_none();
    Ok((document, valid))
}
