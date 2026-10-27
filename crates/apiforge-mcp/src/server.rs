//! The tools over MCP stdio.
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ApiforgeServer {
    pub folder: PathBuf,
}

/// Serves until stdin ends. Builds its own runtime.
pub fn serve_stdio(_folder: PathBuf) -> std::io::Result<()> {
    unimplemented!("MCP-01")
}
