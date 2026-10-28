//! The agent session (MCP-04, ADR 0014 decision 1): the first write takes the
//! project's writer lock and the server keeps it across calls until the model
//! calls `finish_editing`, stdin ends or the process exits. Each write builds
//! a fresh session over the saved project from that one store, checks the
//! version the model names, validates before the first byte, and commits.
use crate::{
    error::{ToolError, Wrote},
    tools::{Diagnostic, Kind},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf};

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct WriteSourceArguments {
    /// The document's registered path, as `project_overview` lists it.
    pub path: String,
    /// The complete new `.afs` text, at most 1 MiB, saved exactly as given.
    pub text: String,
    /// The `source_sha256` of the saved text this one was based on, as
    /// `read_source` returned it. A different hash on disk is `stale_source`.
    pub expected_source_sha256: String,
    /// The project `generation` the text was checked at; when given and
    /// different, `stale_project`.
    #[serde(default)]
    pub expected_generation: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CreateDocumentArguments {
    /// `scenario` or `component`; the header must declare the same.
    pub kind: Kind,
    /// The new document's path, for example `components/new-product.afs`.
    pub path: String,
    /// The complete `.afs` text, at most 1 MiB, saved exactly as given.
    pub text: String,
    /// The project `generation` the text was checked at; when given and
    /// different, `stale_project`.
    #[serde(default)]
    pub expected_generation: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct WriteResult {
    pub path: String,
    /// The commit changed the project; `false` when the disk already held
    /// exactly this text and its checkpoint.
    pub wrote: bool,
    /// The project paths the commit wrote.
    pub changed: Vec<String>,
    /// SHA-256 of the saved bytes, which equal the `text` argument.
    pub source_sha256: String,
    /// The project's head generation after the write.
    pub generation: u64,
    /// The saved text has a valid definition (warnings allowed).
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CreateResult {
    pub path: String,
    pub kind: Kind,
    /// The component handle the header declares; absent for a scenario.
    pub handle: Option<String>,
    pub wrote: bool,
    /// The project paths the commit wrote: the source, the registry and the
    /// checkpoint.
    pub changed: Vec<String>,
    pub source_sha256: String,
    /// The project's head generation after the write.
    pub generation: u64,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Finished {
    /// An agent session was open and has ended: the writer lock is released.
    pub finished: bool,
}

/// One server's agent session. Writes are serialised; the read tools never
/// wait for it.
#[derive(Debug)]
pub struct AgentSession {
    folder: PathBuf,
    environment: Option<Vec<(OsString, OsString)>>,
}

impl AgentSession {
    pub fn new(folder: PathBuf) -> Self {
        Self {
            folder,
            environment: None,
        }
    }

    /// A session that reads `APIFORGE_SECRET_<NAME>` from `environment`
    /// instead of the process environment: a test seam.
    pub fn with_environment(folder: PathBuf, environment: Vec<(OsString, OsString)>) -> Self {
        Self {
            folder,
            environment: Some(environment),
        }
    }

    /// The client's name from `initialize` or the request metadata, written
    /// in the holder description when the session takes the lock.
    pub fn set_client(&self, client: Option<String>) {
        let _ = (&self.folder, &self.environment, client);
    }

    /// The session holds the writer lock.
    pub fn is_active(&self) -> bool {
        false
    }

    pub fn write_source(&self, arguments: &WriteSourceArguments) -> Result<WriteResult, ToolError> {
        let _ = (arguments, Wrote::No);
        unimplemented!("MCP-04")
    }

    pub fn create_document(
        &self,
        arguments: &CreateDocumentArguments,
    ) -> Result<CreateResult, ToolError> {
        let _ = arguments;
        unimplemented!("MCP-04")
    }

    /// Ends the session and releases the writer lock.
    pub fn finish_editing(&self) -> Finished {
        Finished { finished: false }
    }
}
