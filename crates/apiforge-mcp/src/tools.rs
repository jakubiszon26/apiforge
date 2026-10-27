//! One plain function per tool. Each call opens a fresh read-only session on
//! the saved project: the stable read guards against a transaction in
//! progress, and nothing is written, not even `.apiforge/local`.
use crate::error::ToolError;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Scenario,
    Component,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// Who holds the project's writer lock right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Holder {
    None,
    AnotherProcess,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ContractSummary {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DocumentSummary {
    pub kind: Kind,
    /// The registered path every tool names the document by.
    pub path: String,
    /// The component handle from the source header; absent for a scenario.
    pub handle: Option<String>,
    /// The published revision a working component is based on.
    pub base_revision: Option<u32>,
    /// SHA-256 of the saved bytes; absent when the file is missing.
    pub source_sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RevisionSummary {
    pub handle: String,
    pub revision: u32,
    pub path: String,
    pub source_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct LiveRequests {
    pub allowed: bool,
    /// Why live requests are allowed or not.
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ProjectOverview {
    pub name: String,
    pub base_url: String,
    /// Environment variable names; values are not returned.
    pub variables: Vec<String>,
    /// Secret names only; values never leave the server.
    pub secret_names: Vec<String>,
    pub contracts: Vec<ContractSummary>,
    pub documents: Vec<DocumentSummary>,
    pub revisions: Vec<RevisionSummary>,
    /// The project's head generation; it advances on every commit.
    pub generation: u64,
    pub holder: Holder,
    pub live_requests: LiveRequests,
    pub server_version: String,
    /// The version of the language reference; empty until it is served.
    pub reference_version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Fix {
    pub replacement: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

/// Positions are 1-based and counted in Unicode scalar values; the end is
/// exclusive; a `\r` before `\n` belongs to the line break.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub severity: Severity,
    pub message: String,
    pub fix: Option<Fix>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SourceText {
    pub path: String,
    pub text: String,
    pub source_sha256: Option<String>,
    pub generation: u64,
    /// The saved source has a valid definition (warnings allowed).
    pub valid: bool,
    /// `missing`, `not_utf8` or `too_large` when the saved file cannot be
    /// read as source; the text is then empty.
    pub unavailable: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
    /// Diagnostics beyond the 1 000 per document the analysis keeps.
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DocumentCheck {
    pub path: String,
    pub source_sha256: Option<String>,
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CheckResult {
    pub generation: u64,
    /// Every checked document has a valid definition.
    pub valid: bool,
    pub documents: Vec<DocumentCheck>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Printed {
    pub path: String,
    /// The canonical text, byte for byte what `apiforge print` prints.
    pub text: String,
    pub source_sha256: Option<String>,
    pub generation: u64,
}

pub fn project_overview(_folder: &Path) -> Result<ProjectOverview, ToolError> {
    unimplemented!("MCP-01")
}

pub fn read_source(_folder: &Path, _path: &str) -> Result<SourceText, ToolError> {
    unimplemented!("MCP-01")
}

pub fn check(_folder: &Path, _path: Option<&str>) -> Result<CheckResult, ToolError> {
    unimplemented!("MCP-01")
}

pub fn print(_folder: &Path, _path: &str) -> Result<Printed, ToolError> {
    unimplemented!("MCP-01")
}
