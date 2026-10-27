//! Document selection shared by the folder commands of the CLI and the tools.
use crate::error::ToolError;
use apiforge_project::{DocumentSession, ProjectSession};

/// The document id registered under `path`.
pub fn select(_session: &ProjectSession, _path: &str) -> Result<String, ToolError> {
    unimplemented!("MCP-01")
}

/// The document and whether its saved source has a valid definition.
pub fn check_document<'a>(
    _session: &'a ProjectSession,
    _id: &str,
) -> Result<(&'a DocumentSession, bool), ToolError> {
    unimplemented!("MCP-01")
}
