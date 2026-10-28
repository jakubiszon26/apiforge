//! `apiforge mcp`: the project as a Model Context Protocol server (MCP-00).
//! Each read tool is a plain function over a fresh read-only
//! `ProjectSession` (`tools`); the write tools go through the agent session
//! (`agent`), which holds the writer lock until the model finishes editing;
//! `server` exposes them over stdio with `rmcp`; `handover` is the control
//! channel to the window (MCP-08b).
pub mod agent;
pub mod error;
pub mod executor;
pub mod folder;
pub mod handover;
pub mod live;
pub mod positions;
pub mod reference;
pub mod requests;
pub mod resources;
pub mod secrets;
mod server;
#[cfg(feature = "test-support")]
pub mod test_support;
pub mod tools;

pub use error::{ErrorCode, ToolError, Wrote};
pub use server::{ApiforgeServer, ServerSettings, serve_stdio, serve_stdio_with};
