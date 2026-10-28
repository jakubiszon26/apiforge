//! One plain function per tool. Each call opens a fresh read-only session on
//! the saved project: the stable read guards against a transaction in
//! progress, and nothing is written, not even `.apiforge/local`.
use crate::{
    error::{ErrorCode, ToolError},
    executor::{self, CancellationToken, DEADLINE, EndedBy, Notify},
    folder,
    positions::Lines,
    reference,
};
use apiforge_core::{
    format::Port,
    js,
    model::ValueType,
    run::{RunMode, RunResult, RunStatus, StepResult},
};
use apiforge_project::{
    DocumentKind, DocumentSession, Origin, ProjectSession, RegistrationProblem, Unavailable,
};
use apiforge_runner::{RunRequest, RunTarget};
use apiforge_script::MAX_DIAGNOSTICS;
use apiforge_storage::project::IdentityBase;
use apiforge_storage::project::ProjectError;
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// A result larger than this is refused with `result_too_large`, never cut.
pub const MAX_RESULT_BYTES: usize = 2 * 1024 * 1024;

/// A `text` argument larger than this is refused with `too_large` before
/// anything else.
pub const MAX_TEXT_BYTES: usize = apiforge_script::MAX_SOURCE_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    /// This server, for the agent session it keeps until `finish_editing`.
    AgentSession,
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
    /// SHA-256 of the saved bytes; absent when the file cannot be read.
    pub source_sha256: Option<String>,
    /// `missing`, `not_utf8` or `too_large` when the saved file cannot be
    /// read as source.
    pub unavailable: Option<String>,
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
    /// The version of the language reference `language_reference` serves.
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
    /// `missing`, `not_utf8` or `too_large` when the saved file cannot be
    /// read as source.
    pub unavailable: Option<String>,
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

pub fn project_overview(folder: &Path) -> Result<ProjectOverview, ToolError> {
    let session = open(folder)?;
    let manifest = session.manifest();
    let holder = match session.store().has_live_writer() {
        Ok(false) => Holder::None,
        Ok(true) => Holder::AnotherProcess,
        Err(error) => return Err(storage_error(apiforge_project::Error::Storage(error))),
    };
    bounded(ProjectOverview {
        name: manifest.name.clone(),
        base_url: manifest.environment.base_url.clone(),
        variables: manifest.environment.variables.keys().cloned().collect(),
        secret_names: manifest.environment.secret_names.clone(),
        contracts: manifest
            .contracts
            .iter()
            .map(|contract| ContractSummary {
                id: contract.id.clone(),
                name: contract.name.clone(),
            })
            .collect(),
        documents: session
            .documents()
            .iter()
            .map(|document| DocumentSummary {
                kind: match document.entry().kind {
                    DocumentKind::Scenario => Kind::Scenario,
                    DocumentKind::Component => Kind::Component,
                },
                path: document.entry().path.as_str().to_owned(),
                handle: match document.entry().kind {
                    DocumentKind::Scenario => None,
                    DocumentKind::Component => apiforge_project::declaration(document.text())
                        .and_then(|declaration| declaration.handle),
                },
                base_revision: document.entry().base_revision,
                source_sha256: document.saved_sha256().map(str::to_owned),
                unavailable: unavailable(document),
            })
            .collect(),
        revisions: manifest
            .revisions
            .iter()
            .map(|revision| RevisionSummary {
                handle: revision.handle.clone(),
                revision: revision.revision,
                path: revision.path.as_str().to_owned(),
                source_sha256: revision.source_hash.clone(),
            })
            .collect(),
        generation: session.loaded_generation(),
        holder,
        live_requests: LiveRequests {
            allowed: false,
            source: "no --allow-live".into(),
        },
        server_version: env!("CARGO_PKG_VERSION").into(),
        reference_version: reference::version().to_owned(),
    })
}

pub fn read_source(folder: &Path, path: &str) -> Result<SourceText, ToolError> {
    let (session, id) = open_document(folder, path)?;
    let (document, valid) = folder::check_document(&session, &id)?;
    let (diagnostics, omitted) = diagnostics(document);
    bounded(SourceText {
        path: document.entry().path.as_str().to_owned(),
        text: match document.unavailable() {
            None => document.text().to_owned(),
            Some(_) => String::new(),
        },
        source_sha256: document.saved_sha256().map(str::to_owned),
        generation: session.loaded_generation(),
        valid,
        unavailable: unavailable(document),
        diagnostics,
        omitted,
    })
}

pub fn check(folder: &Path, path: Option<&str>) -> Result<CheckResult, ToolError> {
    let (session, selected) = match path {
        Some(path) => {
            let (session, id) = open_document(folder, path)?;
            (session, Some(id))
        }
        None => (open(folder)?, None),
    };
    let mut documents = Vec::new();
    for document in session
        .documents()
        .iter()
        .filter(|document| selected.as_deref().is_none_or(|id| document.id() == id))
    {
        let (document, valid) = folder::check_document(&session, document.id())?;
        let (diagnostics, omitted) = diagnostics(document);
        documents.push(DocumentCheck {
            path: document.entry().path.as_str().to_owned(),
            source_sha256: document.saved_sha256().map(str::to_owned),
            valid,
            unavailable: unavailable(document),
            diagnostics,
            omitted,
        });
    }
    bounded(CheckResult {
        generation: session.loaded_generation(),
        valid: documents.iter().all(|document| document.valid),
        documents,
    })
}

pub fn print(folder: &Path, path: &str) -> Result<Printed, ToolError> {
    let (session, id) = open_document(folder, path)?;
    let (document, valid) = folder::check_document(&session, &id)?;
    let invalid = |message: &str, diagnostics: Vec<Diagnostic>, omitted: usize| {
        ToolError::new(ErrorCode::InvalidText, message)
            .with_details(json!({ "diagnostics": diagnostics, "omitted": omitted }))
    };
    let model = match document.model() {
        Some(model) if valid => model,
        _ => {
            let (diagnostics, omitted) = diagnostics(document);
            return Err(invalid(
                "The saved source has no valid definition",
                diagnostics,
                omitted,
            ));
        }
    };
    let text =
        apiforge_script::print_folder_document(session.context(), model).map_err(|found| {
            let lines = Lines::new(document.text());
            let diagnostics = found.iter().map(|d| diagnostic(&lines, d)).collect();
            invalid("Cannot print the definition", diagnostics, 0)
        })?;
    bounded(Printed {
        path: document.entry().path.as_str().to_owned(),
        text,
        source_sha256: document.saved_sha256().map(str::to_owned),
        generation: session.loaded_generation(),
    })
}

/// A fresh read-only session on the saved project. Creates nothing. A
/// registered source over 1 MiB is served as `unavailable: "too_large"`.
pub(crate) fn open(folder: &Path) -> Result<ProjectSession, ToolError> {
    ProjectSession::open_saved_tolerant(folder).map_err(storage_error)
}

/// The path is checked before the folder is touched.
fn open_document(folder: &Path, path: &str) -> Result<(ProjectSession, String), ToolError> {
    folder::validate(path)?;
    let session = open(folder)?;
    let id = folder::select(&session, path)?;
    Ok((session, id))
}

pub(crate) fn storage_error(error: apiforge_project::Error) -> ToolError {
    let code = match &error {
        apiforge_project::Error::NotAProject => ErrorCode::NotAProject,
        apiforge_project::Error::Storage(ProjectError::RecoveryRequired) => {
            ErrorCode::RecoveryRequired
        }
        _ => ErrorCode::ProjectUnreadable,
    };
    ToolError::new(code, error.to_string())
}

/// Why the saved file cannot be read as source, if it cannot.
fn unavailable(document: &DocumentSession) -> Option<String> {
    document.unavailable().map(|unavailable| {
        match unavailable {
            Unavailable::Missing => "missing",
            Unavailable::NotUtf8 => "not_utf8",
            Unavailable::TooLarge => "too_large",
        }
        .to_owned()
    })
}

/// The kept diagnostics with positions, and how many the analysis omitted.
pub(crate) fn diagnostics(document: &DocumentSession) -> (Vec<Diagnostic>, usize) {
    positioned(
        document.text(),
        document.diagnostics(),
        document.omitted_diagnostics(),
    )
}

/// Diagnostics of `text` with positions; the analysis keeps at most
/// `MAX_DIAGNOSTICS`, and this never returns more.
pub(crate) fn positioned(
    text: &str,
    found: &[apiforge_script::Diagnostic],
    omitted: usize,
) -> (Vec<Diagnostic>, usize) {
    let lines = Lines::new(text);
    let kept = &found[..found.len().min(MAX_DIAGNOSTICS)];
    (
        kept.iter().map(|d| diagnostic(&lines, d)).collect(),
        omitted + (found.len() - kept.len()),
    )
}

fn diagnostic(lines: &Lines<'_>, found: &apiforge_script::Diagnostic) -> Diagnostic {
    let (line, column, end_line, end_column) = lines.range(found.range.clone());
    Diagnostic {
        line,
        column,
        end_line,
        end_column,
        severity: match found.severity {
            apiforge_script::Severity::Error => Severity::Error,
            apiforge_script::Severity::Warning => Severity::Warning,
        },
        message: found.message.clone(),
        fix: found.fix.as_ref().map(|fix| {
            let (line, column, end_line, end_column) = lines.range(fix.range.clone());
            Fix {
                replacement: fix.replacement.clone(),
                line,
                column,
                end_line,
                end_column,
            }
        }),
    }
}

/// Refuses a result over `MAX_RESULT_BYTES` as the model will read it.
fn bounded<T: Serialize>(result: T) -> Result<T, ToolError> {
    bound(Ok(result))
}

/// A published revision the analysis or the run was evaluated against.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct UsedSource {
    pub path: String,
    pub handle: String,
    pub revision: u32,
    pub source_sha256: String,
}

/// Why registering a new document would be refused.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Problem {
    /// `path_rejected`, `path_exists`, `handle_exists` or `invalid_text`.
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TextCheck {
    pub path: String,
    /// A document is registered under the path. When `false` the text was
    /// checked as a new document: `kind` and `handle` from its header, and
    /// `problems` a registration would meet.
    pub registered: bool,
    pub kind: Option<Kind>,
    /// The component handle the header declares; absent for a scenario.
    pub handle: Option<String>,
    /// The text has a valid definition (warnings allowed) and, for a new
    /// document, no registration problem.
    pub valid: bool,
    /// The project generation the text was checked at.
    pub generation: u64,
    /// SHA-256 of the saved document the text would replace; absent for a
    /// new document.
    pub source_sha256: Option<String>,
    /// The published revisions of the project's context the analysis used.
    pub used: Vec<UsedSource>,
    pub problems: Vec<Problem>,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: usize,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct RunArguments {
    /// The registered path of a saved scenario or component.
    pub path: String,
    /// A component's input values by input name, as JSON. The defaults
    /// written in code fill the rest. A scenario takes none.
    #[serde(default)]
    pub inputs: Option<Map<String, Value>>,
    /// The runner's complete JSON report instead of the compact steps.
    #[serde(default)]
    pub full: bool,
    /// Real requests. Refused with `live_disabled` until live requests are
    /// enabled on the server.
    #[serde(default)]
    pub live: bool,
    /// Echoed in the result, so a retried call can be recognised.
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Passed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Passed,
    Failed,
    Error,
    Blocked,
    Skipped,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Main,
    Cleanup,
}

/// One step of a run, with a component's inner steps nested as `apiforge
/// run` prints them.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Step {
    pub handle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub status: StepStatus,
    pub phase: Phase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// What a comparison that did not pass wanted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    /// What it got.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
    /// The request was on its way when the step ended: the API may have
    /// received it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub may_have_reached_api: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<Step>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunReport {
    pub path: String,
    pub status: Verdict,
    pub ended_by: EndedBy,
    /// The project generation the run was prepared at.
    pub generation: u64,
    /// SHA-256 of the saved document that ran.
    pub source_sha256: Option<String>,
    /// The published revisions of the project's context the run used.
    pub used: Vec<UsedSource>,
    /// Known secret values are replaced in every report.
    pub redacted: bool,
    /// Every step, nested ones included.
    pub step_count: usize,
    /// The compact steps; absent with `full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<Step>>,
    /// The runner's complete JSON report; only with `full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// The run limits of one server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunSettings {
    /// A run still going after this long is cancelled (`timed_out`).
    pub deadline: Duration,
    /// A test seam: a pause after every snapshot the runner publishes, so a
    /// simulation lasts long enough to be cancelled.
    pub step_delay: Option<Duration>,
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            deadline: DEADLINE,
            step_delay: None,
        }
    }
}

/// One run at a time per server.
#[derive(Clone, Debug, Default)]
pub struct Runs {
    busy: Arc<AtomicBool>,
    settings: RunSettings,
}

impl Runs {
    pub fn new(settings: RunSettings) -> Self {
        Self {
            busy: Arc::default(),
            settings,
        }
    }

    /// A run is in progress.
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Reference {
    /// The version on the reference's first line.
    pub version: String,
    /// The reference, Markdown.
    pub text: String,
}

/// The diagnostics `text` would have as the document at `path`, or as a new
/// document there. Nothing is written.
pub fn check_text(folder: &Path, path: &str, text: &str) -> Result<TextCheck, ToolError> {
    bound(check_proposal(folder, path, text))
}

fn check_proposal(folder: &Path, path: &str, text: &str) -> Result<TextCheck, ToolError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(
            ToolError::new(ErrorCode::TooLarge, "The text exceeds the 1 MiB limit")
                .with_details(json!({ "bytes": text.len(), "limit": MAX_TEXT_BYTES })),
        );
    }
    let validated = folder::validate(path)?;
    let mut session = open(folder)?;
    let used = used_sources(&session);
    let generation = session.loaded_generation();
    let registered = session
        .documents()
        .iter()
        .find(|document| document.entry().path == validated)
        .map(|document| document.id().to_owned());
    let Some(id) = registered else {
        let check = session
            .check_new_document(None, path, text)
            .map_err(storage_error)?;
        let (diagnostics, omitted) =
            positioned(text, &check.diagnostics, check.omitted_diagnostics);
        return Ok(TextCheck {
            path: validated.as_str().to_owned(),
            registered: false,
            kind: check.kind.map(kind),
            handle: check.handle.clone(),
            valid: check.is_valid(),
            generation,
            source_sha256: None,
            used,
            problems: check.problems.iter().map(|p| problem(*p)).collect(),
            diagnostics,
            omitted,
        });
    };
    let saved = session
        .document(&id)
        .and_then(DocumentSession::saved_sha256)
        .map(str::to_owned);
    session
        .edit_source(&id, text, Origin::SESSION)
        .map_err(|error| match error {
            apiforge_project::Error::ModelLocked => ToolError::new(
                ErrorCode::ProjectUnreadable,
                "The saved document cannot be read as source; read_source says why",
            ),
            error => storage_error(error),
        })?;
    session.settle(&id).map_err(storage_error)?;
    let (document, valid) = folder::check_document(&session, &id)?;
    let (diagnostics, omitted) = diagnostics(document);
    let kind = kind(document.entry().kind);
    Ok(TextCheck {
        path: validated.as_str().to_owned(),
        registered: true,
        kind: Some(kind),
        handle: match kind {
            Kind::Scenario => None,
            Kind::Component => apiforge_project::declaration(text).and_then(|d| d.handle),
        },
        valid,
        generation,
        source_sha256: saved,
        used,
        problems: Vec::new(),
        diagnostics,
        omitted,
    })
}

pub(crate) fn kind(kind: DocumentKind) -> Kind {
    match kind {
        DocumentKind::Scenario => Kind::Scenario,
        DocumentKind::Component => Kind::Component,
    }
}

pub(crate) fn problem(problem: RegistrationProblem) -> Problem {
    Problem {
        code: match problem {
            RegistrationProblem::PathRejected => ErrorCode::PathRejected,
            RegistrationProblem::PathRegistered | RegistrationProblem::PathExists => {
                ErrorCode::PathExists
            }
            RegistrationProblem::HandleExists => ErrorCode::HandleExists,
            RegistrationProblem::NoKind | RegistrationProblem::KindMismatch => {
                ErrorCode::InvalidText
            }
        },
        message: problem.message().to_owned(),
    }
}

/// Every published revision: the context an analysis and a run are
/// evaluated in.
fn used_sources(session: &ProjectSession) -> Vec<UsedSource> {
    session
        .manifest()
        .revisions
        .iter()
        .map(|revision| UsedSource {
            path: revision.path.as_str().to_owned(),
            handle: revision.handle.clone(),
            revision: revision.revision,
            source_sha256: revision.source_hash.clone(),
        })
        .collect()
}

/// Holds the server's run slot until dropped.
struct Slot(Arc<AtomicBool>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Runs {
    fn claim(&self) -> Result<Slot, ToolError> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Slot(self.busy.clone()))
            .map_err(|_| {
                ToolError::new(
                    ErrorCode::RunInProgress,
                    "Another run is in progress on this server; wait for its result or cancel it",
                )
            })
    }
}

/// A simulation of the saved scenario or component at `arguments.path`.
pub fn run(
    folder: &Path,
    arguments: &RunArguments,
    runs: &Runs,
    cancel: CancellationToken,
) -> Result<RunReport, ToolError> {
    bound(simulate(folder, arguments, runs, cancel))
}

fn simulate(
    folder: &Path,
    arguments: &RunArguments,
    runs: &Runs,
    cancel: CancellationToken,
) -> Result<RunReport, ToolError> {
    if arguments.live {
        return Err(ToolError::new(
            ErrorCode::LiveDisabled,
            "live requests are not enabled on this server",
        ));
    }
    let _slot = runs.claim()?;
    folder::validate(&arguments.path)?;
    let mut session = open(folder)?;
    let id = folder::select(&session, &arguments.path)?;
    let (document, valid) = folder::check_document(&session, &id)?;
    if !valid {
        let (diagnostics, omitted) = diagnostics(document);
        return Err(ToolError::new(
            ErrorCode::InvalidText,
            "The saved source has no valid definition; run executes the saved text",
        )
        .with_details(json!({ "diagnostics": diagnostics, "omitted": omitted })));
    }
    let path = document.entry().path.as_str().to_owned();
    let source_sha256 = document.saved_sha256().map(str::to_owned);
    let generation = session.loaded_generation();
    let used = used_sources(&session);
    let prepared = session.prepare_run(&id).map_err(storage_error)?;
    let project = prepared.project().map_err(storage_error)?;
    let (scenario_id, target) = match prepared.snapshot.document() {
        IdentityBase::Scenario(scenario) => {
            if let Some(inputs) = arguments.inputs.as_ref().filter(|i| !i.is_empty()) {
                return Err(ToolError::new(
                    ErrorCode::InvalidInputs,
                    "A scenario takes no inputs; only a component does",
                )
                .with_details(json!({
                    "missing": [],
                    "unknown": inputs.keys().collect::<Vec<_>>(),
                    "invalid": [],
                })));
            }
            (scenario.id.clone(), RunTarget::Scenario)
        }
        IdentityBase::Component(component) => (
            component.id.clone(),
            RunTarget::Component {
                revision: component.revision,
                inputs: component_inputs(&component.inputs, arguments.inputs.as_ref())?,
            },
        ),
    };
    let secrets = executor::secrets(None)
        .map_err(|message| ToolError::new(ErrorCode::ProjectUnreadable, message))?;
    session.store().set_known_secrets(secrets.values().cloned());
    let request = RunRequest {
        project,
        scenario_id,
        target,
        mode: RunMode::Simulation,
        target_node_id: None,
        single_node_id: None,
        previous_run: None,
        secrets,
    };
    // The executor cancels its token at the deadline; the caller's stays.
    let token = cancel.child_token();
    let notify: Notify = match runs.settings.step_delay {
        Some(delay) => {
            let stopping = token.clone();
            Arc::new(move || {
                if !stopping.is_cancelled() {
                    std::thread::sleep(delay);
                }
            })
        }
        None => Arc::new(|| {}),
    };
    let outcome = executor::execute_observed(request, None, token, runs.settings.deadline, notify)
        .map_err(|message| ToolError::new(ErrorCode::InvalidText, message))?;
    let report = &outcome.report;
    Ok(RunReport {
        path,
        status: match report.status {
            RunStatus::Passed => Verdict::Passed,
            RunStatus::Cancelled => Verdict::Cancelled,
            RunStatus::Failed | RunStatus::Running => Verdict::Failed,
        },
        ended_by: outcome.ended_by,
        generation,
        source_sha256,
        used,
        redacted: true,
        step_count: count(&report.steps),
        steps: (!arguments.full).then(|| steps(report)),
        report: arguments
            .full
            .then(|| serde_json::to_value(report))
            .transpose()
            .map_err(|error| ToolError::new(ErrorCode::InvalidText, error.to_string()))?,
        error: report.error.clone(),
        notice: report.notice.clone(),
        request_id: arguments.request_id.clone(),
    })
}

/// Every step, nested ones included.
fn count(steps: &[StepResult]) -> usize {
    steps
        .iter()
        .map(|step| 1 + step.children.as_deref().map_or(0, count))
        .sum()
}

/// A component's inputs from JSON values by the Run inputs dialog's rules
/// (KTLO-23b, KTLO-27): a value of the input's type, or text the dialog would
/// read as one; the defaults written in code fill the rest; an input without
/// a default must be given unless it is declared optional.
fn component_inputs(
    ports: &[Port],
    given: Option<&Map<String, Value>>,
) -> Result<IndexMap<String, Value>, ToolError> {
    let empty = Map::new();
    let given = given.unwrap_or(&empty);
    let unknown: Vec<&String> = given
        .keys()
        .filter(|name| !ports.iter().any(|port| &port.name == *name))
        .collect();
    let (mut missing, mut invalid, mut values) = (Vec::new(), Vec::new(), IndexMap::new());
    for port in ports {
        match given.get(&port.name) {
            Some(value) => match input_value(port.value_type, value) {
                Some(value) => {
                    values.insert(port.name.clone(), value);
                }
                None => invalid.push(&port.name),
            },
            None if port.default().is_none() && port.required != Some(false) => {
                missing.push(&port.name);
            }
            None => {}
        }
    }
    if missing.is_empty() && unknown.is_empty() && invalid.is_empty() {
        return Ok(values);
    }
    let inputs: Vec<_> = ports
        .iter()
        .map(|port| {
            json!({
                "name": port.name,
                "type": port.value_type.as_str(),
                "has_default": port.default().is_some(),
                "optional": port.required == Some(false) && port.default().is_none(),
            })
        })
        .collect();
    Err(ToolError::new(
        ErrorCode::InvalidInputs,
        "The inputs do not fit the component: see missing, unknown and invalid",
    )
    .with_details(json!({
        "missing": missing,
        "unknown": unknown,
        "invalid": invalid,
        "inputs": inputs,
    })))
}

/// The value `given` gives an input of `value_type`, or `None` when it is not
/// one. Text is read as the dialog reads a field.
fn input_value(value_type: ValueType, given: &Value) -> Option<Value> {
    if let Value::String(text) = given
        && !matches!(value_type, ValueType::String | ValueType::Any)
    {
        let trimmed = js::trim(text);
        return match value_type {
            ValueType::Number => trimmed.parse::<f64>().ok().and_then(js::number_value),
            ValueType::Boolean => match trimmed {
                "true" => Some(Value::Bool(true)),
                "false" => Some(Value::Bool(false)),
                _ => None,
            },
            _ => serde_json::from_str::<Value>(trimmed)
                .ok()
                .filter(|value| fits(value_type, value)),
        };
    }
    fits(value_type, given).then(|| given.clone())
}

fn fits(value_type: ValueType, value: &Value) -> bool {
    match value_type {
        ValueType::Any => true,
        ValueType::String => value.is_string(),
        ValueType::Number => value.is_number(),
        ValueType::Boolean => value.is_boolean(),
        ValueType::Object => value.is_object(),
        ValueType::Array => value.is_array(),
    }
}

/// The runner's sentence on a request that may have reached the API.
const MAY_HAVE_HAPPENED: &str = "The API may have performed this request.";

/// The compact steps of a report.
pub fn steps(report: &RunResult) -> Vec<Step> {
    report.steps.iter().map(step).collect()
}

fn step(found: &StepResult) -> Step {
    use apiforge_core::{format::OptionalJson, run::StepStatus as Status};
    let (expected, actual) = match (&found.expected, &found.actual) {
        (OptionalJson::Present(expected), OptionalJson::Present(actual))
            if found.status != Status::Passed =>
        {
            (Some(expected.clone()), Some(actual.clone()))
        }
        _ => (None, None),
    };
    Step {
        handle: found.handle.clone(),
        label: found.label.clone(),
        status: match found.status {
            Status::Passed => StepStatus::Passed,
            Status::Failed => StepStatus::Failed,
            Status::Error => StepStatus::Error,
            Status::Blocked => StepStatus::Blocked,
            Status::Skipped => StepStatus::Skipped,
            Status::Cancelled => StepStatus::Cancelled,
        },
        phase: match found.phase {
            apiforge_core::run::Phase::Main => Phase::Main,
            apiforge_core::run::Phase::Cleanup => Phase::Cleanup,
        },
        message: found.message.clone(),
        expected,
        actual,
        may_have_reached_api: found
            .message
            .as_deref()
            .is_some_and(|message| message.ends_with(MAY_HAVE_HAPPENED)),
        steps: found
            .children
            .as_ref()
            .map(|children| children.iter().map(step).collect()),
    }
}

/// The `.afs` reference for models and its version.
pub fn language_reference() -> Result<Reference, ToolError> {
    bounded(Reference {
        version: reference::version().to_owned(),
        text: reference::TEXT.to_owned(),
    })
}

/// Refuses a result or an error over `MAX_RESULT_BYTES` as the model will
/// read it, with how to narrow the request; nothing is ever cut.
pub fn bound<T: Serialize>(result: Result<T, ToolError>) -> Result<T, ToolError> {
    let size = match &result {
        Ok(value) => serde_json::to_vec_pretty(value),
        Err(error) => serde_json::to_vec_pretty(error),
    }
    .map_or(usize::MAX, |bytes| bytes.len());
    if size > MAX_RESULT_BYTES {
        return Err(ToolError::new(
            ErrorCode::ResultTooLarge,
            "The result exceeds 2 MiB and is never cut; narrow the request where the tool allows it: run with full: false, check one path at a time",
        )
        .with_details(json!({ "bytes": size, "limit": MAX_RESULT_BYTES })));
    }
    result
}
