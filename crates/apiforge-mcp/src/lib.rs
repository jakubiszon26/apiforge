//! `apiforge mcp`: the project as a Model Context Protocol server (MCP-00).
//! Each tool is a plain function over a fresh read-only `ProjectSession`
//! (`tools`); `server` exposes them over stdio with `rmcp`.
pub mod error;
pub mod folder;
pub mod positions;
mod server;
pub mod tools;

pub use error::{ErrorCode, ToolError};
pub use server::{ApiforgeServer, serve_stdio};
