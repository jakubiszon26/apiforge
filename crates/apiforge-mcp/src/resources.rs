//! MCP resources: `apiforge://reference`, the `.afs` language reference, and
//! `apiforge://source/<path>`, the saved text of each registered document.
//! A source URI's path is percent-decoded and must pass
//! `ProjectPath::working_source` before the folder is touched.
use crate::error::ToolError;
use std::path::Path;

pub const REFERENCE_URI: &str = "apiforge://reference";
pub const SOURCE_PREFIX: &str = "apiforge://source/";
pub const MIME_TYPE: &str = "text/plain";

/// One entry of `resources/list`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: String,
}

/// The reference and one source per registered document, in registry order.
pub fn list(folder: &Path) -> Result<Vec<Resource>, ToolError> {
    let _ = folder;
    unimplemented!("MCP-03")
}

/// The text behind `uri`.
pub fn read(folder: &Path, uri: &str) -> Result<String, ToolError> {
    let _ = (folder, uri);
    unimplemented!("MCP-03")
}
