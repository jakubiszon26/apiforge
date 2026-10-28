//! `apiforge mcp`: the project as a Model Context Protocol server (MCP-00).
//! Each read tool is a plain function over a fresh read-only
//! `ProjectSession` (`tools`); the write tools go through the agent session
//! (`agent`), which holds the writer lock until the model finishes editing;
//! `server` exposes them over stdio with `rmcp`.
pub mod agent;
pub mod error;
pub mod executor;
pub mod folder;
pub mod live;
pub mod positions;
pub mod reference;
pub mod resources;
pub mod secrets;
mod server;
pub mod tools;

pub use error::{ErrorCode, ToolError, Wrote};
pub use server::{ApiforgeServer, ServerSettings, serve_stdio, serve_stdio_with};
