//! The tools and resources over MCP stdio with `rmcp`. Stdout carries
//! protocol messages only; the server's own lines go to stderr and never
//! quote an argument.
use crate::{
    agent::{AgentSession, CreateDocumentArguments, WriteSourceArguments},
    error::{ErrorCode, ToolError},
    executor::{CancellationToken, EndedBy},
    handover::Hub,
    live::{self, LivePermission},
    requests, resources,
    secrets::{self, Secrets},
    tools::{self, MAX_TEXT_BYTES, RunArguments, RunReport, RunSettings, Runs},
};
use apiforge_channel::{Message, RunMode as RunKind, RunState};
use apiforge_core::{journal::JournalRun, run::RunMode};
use apiforge_project::ProjectSession;
use apiforge_storage::{JournalStore, project::ProjectStore};
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::schema_for_output, wrapper::Parameters},
    model::{
        CacheScope, CallToolResult, ContentBlock, Implementation, ListResourcesResult,
        ListToolsResult, PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams,
        ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, ResultType,
        ServerCapabilities, ServerConfig,
    },
    service::{RequestContext, ServerInitializeError},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::{Map, Value};
use std::{
    collections::{HashSet, VecDeque},
    pin::Pin,
    sync::{Mutex, OnceLock},
    task::{Context, Poll},
};
use std::{
    io,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::io::{AsyncRead, ReadBuf};

/// The order `tools/list` returns: the overview and the reference first,
/// then reading, checking, printing and running, then writing.
const TOOL_ORDER: [&str; 10] = [
    "project_overview",
    "language_reference",
    "read_source",
    "check",
    "check_text",
    "print",
    "run",
    "write_source",
    "create_document",
    "finish_editing",
];

/// How long the end of stdin waits for a run's cleanup.
const CLEANUP_WAIT: Duration = Duration::from_secs(10);

const INSTRUCTIONS: &str = "API Forge project tools. Every document is named by its registered path; call project_overview first and language_reference before writing .afs. check_text checks a proposed text without saving it; run simulates the saved scenario or component and sends nothing, unless you ask for live: true and the person allowed live requests (project_overview.live_requests). write_source and create_document save a checked text: the first write takes the project for this agent session and keeps it, so the window only watches, until you call finish_editing. Call finish_editing when your edits are done.";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PathArguments {
    /// The document's registered path, as `project_overview` lists it, for
    /// example `components/product.afs`.
    pub path: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct CheckArguments {
    /// One registered path; every document when absent.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckTextArguments {
    /// A registered path, or the path a new document would have, for example
    /// `components/new-product.afs`.
    pub path: String,
    /// The proposed `.afs` text, at most 1 MiB.
    pub text: String,
}

/// How the server was started: `apiforge mcp [--allow-live] [--secrets
/// <file>]` and the test seams.
#[derive(Clone, Debug, Default)]
pub struct ServerSettings {
    pub run: RunSettings,
    /// `--allow-live`: live requests are allowed while no window decides.
    pub allow_live: bool,
    /// `APIFORGE_SECRET_<NAME>` and the `--secrets` file.
    pub secrets: Secrets,
    /// A test seam: a pause after every snapshot once a run is stopping, so
    /// its cleanup outlasts the end of stdin.
    pub cleanup_delay: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct ApiforgeServer {
    folder: Arc<PathBuf>,
    runs: Runs,
    agent: Arc<AgentSession>,
    /// The path of the run in progress and whether it is live, for a
    /// window waiting for it.
    running: Arc<Mutex<Option<(String, bool)>>>,
    permission: LivePermission,
    secrets: Secrets,
    cleanup_delay: Option<Duration>,
    /// Cancelled at the end of stdin: the client is gone.
    client_lost: CancellationToken,
    /// The live runs this server answered, by `request_id`.
    answered: Arc<Mutex<Answered>>,
    /// While a live run of this server is in progress, the journal's runs
    /// from before it: the one it adds is not an interrupted run.
    journal_before: Arc<Mutex<Option<Before>>>,
    tool_router: ToolRouter<Self>,
}

/// The live results a server remembers for a repeated `request_id`.
const ANSWERED: usize = 64;

/// The run a `request_id` answered: the path and the inputs.
type AnsweredRun = (String, Option<Map<String, Value>>);

/// The live results by `request_id`, oldest first, with the run each
/// answered; and the ids whose result was dropped to keep the list short.
#[derive(Debug, Default)]
struct Answered {
    recent: VecDeque<(String, AnsweredRun, RunReport)>,
    dropped: HashSet<String>,
}

#[tool_router(router = tool_router)]
impl ApiforgeServer {
    pub fn new(folder: PathBuf) -> Self {
        Self::with_settings(folder, ServerSettings::default())
    }

    /// A server whose runs follow `settings`.
    pub fn with_run_settings(folder: PathBuf, settings: RunSettings) -> Self {
        Self::with_settings(
            folder,
            ServerSettings {
                run: settings,
                ..ServerSettings::default()
            },
        )
    }

    /// A server started with `settings`.
    pub fn with_settings(folder: PathBuf, settings: ServerSettings) -> Self {
        Self {
            agent: Arc::new(AgentSession::with_secrets(
                folder.clone(),
                settings.secrets.clone(),
            )),
            folder: Arc::new(folder),
            runs: Runs::new(settings.run),
            permission: LivePermission::new(settings.allow_live),
            secrets: settings.secrets,
            cleanup_delay: settings.cleanup_delay,
            client_lost: CancellationToken::new(),
            answered: Arc::default(),
            journal_before: Arc::default(),
            running: Arc::default(),
            tool_router: Self::tool_router(),
        }
    }

    /// The live permission slot: MCP-09's control channel writes it.
    pub fn permission(&self) -> &LivePermission {
        &self.permission
    }

    /// Cancelled when stdin ends.
    pub fn client_lost(&self) -> &CancellationToken {
        &self.client_lost
    }

    /// `run` as the tool answers it, on the calling thread.
    pub fn run_tool(
        &self,
        arguments: RunArguments,
        cancel: CancellationToken,
    ) -> Result<RunReport, ToolError> {
        self.run_for(arguments, cancel, None)
    }

    /// `run` for the client named `client`, which a live run's agent
    /// session writes into the holder description.
    fn run_for(
        &self,
        arguments: RunArguments,
        cancel: CancellationToken,
        client: Option<String>,
    ) -> Result<RunReport, ToolError> {
        let secrets = self.secrets.read()?;
        let values: Vec<String> = secrets.values().cloned().collect();
        if let Some(id) = &arguments.request_id {
            secrets::refuse_in_text(id, &values)?;
            requests::check_id(id)?;
        }
        if !arguments.live {
            let mut report =
                tools::simulate(&self.folder, &arguments, &self.runs, cancel, secrets)?;
            // The simulation has run; a journal that cannot be read is said,
            // not an error.
            let (interrupted, unreadable) = self.read_journal().unwrap_or((Vec::new(), true));
            report.interrupted = live::interrupted(&interrupted, &values);
            report.journal_unreadable = unreadable;
            return tools::bound(Ok(report));
        }
        if let Some(recorded) = self.recorded(&arguments)? {
            return Ok(recorded);
        }
        self.permission.check()?;
        let _slot = self.runs.claim()?;
        let store = self.agent.writer(client)?;
        if let Some(id) = &arguments.request_id
            && let Some(started) = requests::find(&store, id)?
        {
            return Err(self.unknown_outcome(&started, &values));
        }
        let report = self.run_live(&arguments, cancel, store, secrets, &values)?;
        if let Some(id) = &arguments.request_id {
            self.record(id, &arguments, &report);
        }
        Ok(report)
    }

    /// A live run on the agent session's `store`: journalled before each
    /// request, the permission checked immediately before the start.
    fn run_live(
        &self,
        arguments: &RunArguments,
        cancel: CancellationToken,
        store: ProjectStore,
        secrets: indexmap::IndexMap<String, String>,
        values: &[String],
    ) -> Result<RunReport, ToolError> {
        crate::folder::validate(&arguments.path)?;
        let mut session =
            ProjectSession::from_store_saved(store.clone()).map_err(tools::storage_error)?;
        let prepared = tools::prepare(&mut session, arguments, RunMode::Live, secrets)?;
        drop(session);
        let prepared_path = prepared.path().to_owned();
        let journal = JournalStore::open_project(store.clone()).map_err(live::journal_error)?;
        // A change of the permission after this point does not stop the run
        // (MCP-00 decision 16 A).
        self.permission.check()?;
        // Like the journal, the record of the id is on disk before the first
        // request goes out.
        if let Some(id) = &arguments.request_id {
            requests::record(&store, id, &prepared_path)?;
        }
        let _running = JournalBefore::set(
            &self.journal_before,
            (journal.interrupted.clone(), journal.unreadable),
        );
        let (mut report, before) = tools::execute(
            prepared,
            arguments,
            Some(journal),
            &self.runs,
            cancel,
            self.cleanup_delay,
        )?;
        report.interrupted = live::interrupted(&before.interrupted, values);
        report.journal_unreadable = before.unreadable;
        tools::bound(Ok(report))
    }

    /// An id an earlier process started: what happened is unknown to this
    /// server, and nothing is sent again.
    fn unknown_outcome(&self, started: &requests::Started, values: &[String]) -> ToolError {
        let interrupted = self
            .read_journal()
            .map(|(runs, _)| live::interrupted(&runs, values))
            .unwrap_or_default();
        ToolError::new(
            ErrorCode::UnknownOutcome,
            "An earlier apiforge mcp process started a live run with this request_id; its outcome is unknown to this server and nothing was sent. The API may have received its requests once: `interrupted` lists what the run journal kept. Use a new request_id to run again",
        )
        .with_details(json!({
            "path": started.path,
            "started_at": started.started_at,
            "interrupted": interrupted,
        }))
    }

    /// The recorded result of a live run with the same `request_id`. An id
    /// that answered another run is refused; one whose result this server no
    /// longer holds is `unknown_outcome`, never run again.
    fn recorded(&self, arguments: &RunArguments) -> Result<Option<RunReport>, ToolError> {
        let Some(id) = arguments.request_id.as_ref() else {
            return Ok(None);
        };
        let answered = self.answered.lock().unwrap_or_else(|p| p.into_inner());
        if answered.dropped.contains(id) {
            return Err(ToolError::new(
                ErrorCode::UnknownOutcome,
                "This server answered a live run with this request_id but no longer holds its result; nothing was sent. The API may have received that run's requests once; use a new request_id to run again",
            ));
        }
        let Some((_, run, report)) = answered
            .recent
            .iter()
            .find(|(answered, _, _)| answered == id)
        else {
            return Ok(None);
        };
        if *run != (arguments.path.clone(), arguments.inputs.clone()) {
            return Err(ToolError::new(
                ErrorCode::InvalidInputs,
                "This request_id answered a live run of another document or other inputs; nothing was sent. Use a new request_id for a new run",
            )
            .with_details(json!({ "path": run.0 })));
        }
        Ok(Some(RunReport {
            replayed: true,
            ..report.clone()
        }))
    }

    fn record(&self, id: &str, arguments: &RunArguments, report: &RunReport) {
        let mut answered = self.answered.lock().unwrap_or_else(|p| p.into_inner());
        answered.recent.retain(|(answered, _, _)| answered != id);
        if answered.recent.len() == ANSWERED
            && let Some((dropped, _, _)) = answered.recent.pop_front()
        {
            answered.dropped.insert(dropped);
        }
        let run = (arguments.path.clone(), arguments.inputs.clone());
        answered
            .recent
            .push_back((id.to_owned(), run, report.clone()));
    }

    /// The project's journal as this server may read it: through its agent
    /// session's lock, or while no other process holds the project. While
    /// this server's own live run writes it, the file is not read: the runs
    /// from before that run are the interrupted ones.
    fn read_journal(&self) -> Result<(Vec<JournalRun>, bool), ToolError> {
        if let Some(before) = &*self
            .journal_before
            .lock()
            .unwrap_or_else(|p| p.into_inner())
        {
            return Ok(before.clone());
        }
        let store = match self.agent.held() {
            Some(store) => store,
            None => ProjectStore::open_read_only(&**self.folder)
                .map_err(|error| live::journal_error(error.into()))?,
        };
        live::read_journal(&store)
    }

    /// `project_overview` as the tool answers it.
    pub fn overview_tool(&self) -> Result<tools::ProjectOverview, ToolError> {
        let mut overview = tools::project_overview(&self.folder)?;
        if overview.holder == tools::Holder::AnotherProcess && self.agent.is_active() {
            overview.holder = tools::Holder::AgentSession;
            overview.interrupted_runs = self.read_journal()?.0.len();
        }
        overview.live_requests = self.permission.describe();
        Ok(overview)
    }

    /// `read_source` as the tool answers it.
    pub fn read_source_tool(&self, path: &str) -> Result<tools::SourceText, ToolError> {
        tools::read_source_with(&self.folder, path, &self.secrets.values()?)
    }

    /// `check_text` as the tool answers it: a text that carries a known
    /// secret value is refused before it is analysed.
    pub fn check_text_tool(&self, path: &str, text: &str) -> Result<tools::TextCheck, ToolError> {
        if text.len() <= MAX_TEXT_BYTES {
            secrets::refuse_in_text(text, &self.secrets.values()?)?;
        }
        tools::check_text(&self.folder, path, text)
    }

    /// `print` as the tool answers it.
    pub fn print_tool(&self, path: &str) -> Result<tools::Printed, ToolError> {
        tools::print_with(&self.folder, path, &self.secrets.values()?)
    }

    /// `resources/read` of `uri`: the text, or the tools' refusal.
    pub fn resource_text(&self, uri: &str) -> Result<String, ToolError> {
        resources::read_with(&self.folder, uri, &self.secrets.values()?)
    }

    /// The server's run slot, shared by every clone.
    pub fn runs(&self) -> &Runs {
        &self.runs
    }

    /// The server's agent session, shared by every clone.
    pub fn agent(&self) -> &Arc<AgentSession> {
        &self.agent
    }

    #[tool(
        description = "What the API Forge project contains: name, base URL, environment variable and secret names, contracts, every document (kind, registered path, handle, base revision, source_sha256), the published component revisions, the project generation, who holds the project, whether live requests are allowed, and the version of the language reference. Reads the saved project; writes nothing.",
        output_schema = schema_for_output::<tools::ProjectOverview>(),
        annotations(title = "Project overview", read_only_hint = true)
    )]
    async fn project_overview(&self) -> Result<CallToolResult, ErrorData> {
        let server = self.clone();
        call("project_overview", move || server.overview_tool()).await
    }

    #[tool(
        description = "The .afs language reference for models, with its version: file shape, statements, requests, expressions, assertions, cleanup, component calls, and what each diagnostic asks you to do. Read it before writing or changing a document. Also the resource apiforge://reference.",
        output_schema = schema_for_output::<tools::Reference>(),
        annotations(title = "Language reference", read_only_hint = true)
    )]
    async fn language_reference(&self) -> Result<CallToolResult, ErrorData> {
        call("language_reference", tools::language_reference).await
    }

    #[tool(
        description = "The saved text of one document by its registered path, with source_sha256, the project generation, whether it is valid, and its diagnostics (line and column 1-based in Unicode scalar values, end exclusive, severity, message, suggested fix). Writes nothing.",
        output_schema = schema_for_output::<tools::SourceText>(),
        annotations(title = "Read source", read_only_hint = true)
    )]
    async fn read_source(
        &self,
        Parameters(arguments): Parameters<PathArguments>,
    ) -> Result<CallToolResult, ErrorData> {
        let server = self.clone();
        call("read_source", move || {
            server.read_source_tool(&arguments.path)
        })
        .await
    }

    #[tool(
        description = "Diagnostics of the saved source of every document, or of one registered path: file, line, column, severity, message and suggested fix. A project with errors is reported, not refused. Beyond 1 000 diagnostics per document, `omitted` counts the rest. Writes nothing.",
        output_schema = schema_for_output::<tools::CheckResult>(),
        annotations(title = "Check", read_only_hint = true)
    )]
    async fn check(
        &self,
        Parameters(arguments): Parameters<CheckArguments>,
    ) -> Result<CallToolResult, ErrorData> {
        let folder = self.folder.clone();
        call("check", move || {
            tools::check(&folder, arguments.path.as_deref())
        })
        .await
    }

    #[tool(
        description = "Checks a proposed text without saving it: the diagnostics the text would have as the document at `path`, with positions as read_source gives them, `valid`, the project generation and the source_sha256 of what was used. For a path that is not registered, the text is checked as a new document: kind and handle from its header, and the problems a registration would meet (path_exists, handle_exists, path_rejected, invalid_text). check_text checks a proposal; run executes the saved source, so a model that wants to run its draft writes it first. Writes nothing.",
        output_schema = schema_for_output::<tools::TextCheck>(),
        annotations(title = "Check text", read_only_hint = true)
    )]
    async fn check_text(
        &self,
        Parameters(arguments): Parameters<CheckTextArguments>,
    ) -> Result<CallToolResult, ErrorData> {
        let server = self.clone();
        call("check_text", move || {
            server.check_text_tool(&arguments.path, &arguments.text)
        })
        .await
    }

    #[tool(
        description = "The canonical text of one saved document, exactly as `apiforge print` and the window write it. A document without a valid definition is the error `invalid_text` with its diagnostics. Writes nothing.",
        output_schema = schema_for_output::<tools::Printed>(),
        annotations(title = "Print", read_only_hint = true)
    )]
    async fn print(
        &self,
        Parameters(arguments): Parameters<PathArguments>,
    ) -> Result<CallToolResult, ErrorData> {
        let server = self.clone();
        call("print", move || server.print_tool(&arguments.path)).await
    }

    #[tool(
        description = "Runs the saved scenario or component at `path`: a simulation with the sample responses written in it, which sends nothing, or with `live: true` real requests to the project's API, only when the person allowed live requests (project_overview.live_requests; otherwise `live_disabled`). Returns `status` (passed, failed or cancelled; a failed verdict is a result, not an error), every step with handle, label, status, message and, for a failed comparison, expected and actual, a component's steps nested beneath it, the generation and source_sha256, and `interrupted`: live runs an earlier process left unfinished, with the requests that may have changed the API. A component runs alone with `inputs` by name, its defaults filling the rest. `full: true` returns the runner's complete JSON report instead of the steps. run executes the saved source; check_text checks a proposal. A live run records each request in the project's run journal before sending it and takes the project for this agent session as write_source does; while a window or another process holds the project it is refused (`held_by_window`, `held_by_other`). Known secret values are `[REDACTED]` in every report. Repeat a live run's `request_id` only to recover a lost answer: this server returns the recorded result and sends nothing. One run at a time; a cancelled request or the ten-minute deadline stops the run, its cleanup still runs, and a request on its way is marked `may_have_reached_api`.",
        output_schema = schema_for_output::<tools::RunReport>(),
        annotations(
            title = "Run",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn run(
        &self,
        Parameters(arguments): Parameters<RunArguments>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let server = self.clone();
        let client = context.client_info().map(|client| client.name);
        // A live run also stops when the client is gone: the end of stdin
        // cancels it at once rather than after rmcp's drain.
        let (cancel, _forwarding) = if arguments.live {
            let token = self.client_lost.child_token();
            let (request, forwarded) = (context.ct.clone(), token.clone());
            tokio::spawn(async move {
                tokio::select! {
                    () = request.cancelled() => forwarded.cancel(),
                    () = forwarded.cancelled() => {}
                }
            });
            // The run's own token ends the forwarding when the call returns.
            (token.clone(), Some(token.drop_guard()))
        } else {
            (context.ct.clone(), None)
        };
        let runs = self.runs.clone();
        let running = self.running.clone();
        call("run", move || {
            if !runs.is_busy() {
                *lock(&running) = Some((arguments.path.clone(), arguments.live));
            }
            let report = server.run_for(arguments, cancel, client);
            if !runs.is_busy() {
                *lock(&running) = None;
            }
            let report = report?;
            match report.ended_by {
                EndedBy::Completed => {}
                EndedBy::Cancelled => eprintln!("apiforge mcp: run: cancelled"),
                EndedBy::TimedOut => eprintln!("apiforge mcp: run: timed_out"),
            }
            Ok(report)
        })
        .await
    }

    #[tool(
        description = "Saves a new text for the registered document at `path`, exactly as given. The text is checked in the project's context first and saved only without errors (warnings allowed), together with its layout checkpoint, the same bytes as a save in the window. Name the version you read: `expected_source_sha256` from read_source (a different hash on disk is `stale_source`, nothing written) and optionally `expected_generation` (`stale_project`). The first write takes the project for this agent session and keeps it until finish_editing. When an API Forge window holds the project the write asks it to hand the project over: a window with nothing unsaved does so at once and watches, one with unsaved work keeps it (`handover_refused`), one that has not answered in 30 seconds is `handover_pending`, one without the channel is `held_by_window`; another process holding the project is `held_by_other`. Nothing is written when refused. A text containing a known secret value is `secret_in_text`. The same text and hash twice is a no-op.",
        output_schema = schema_for_output::<crate::agent::WriteResult>(),
        annotations(
            title = "Write source",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn write_source(
        &self,
        Parameters(arguments): Parameters<WriteSourceArguments>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let agent = self.agent.clone();
        let client = context.client_info().map(|client| client.name);
        // The session's state is held for a whole write: waited for on the
        // blocking pool, never on the transport's threads.
        call("write_source", move || {
            agent.set_client(client);
            agent.write_source(&arguments)
        })
        .await
    }

    #[tool(
        description = "Registers a new scenario or component at `path` with `text`, exactly as given. Nothing is written unless the document passes check_text as a new document: the path is a free working source (`path_rejected`, `path_exists`), the header declares `kind`, a component's handle is free (`handle_exists`) and the text has no error (`invalid_text` with the diagnostics). The source, its registry entry and its layout checkpoint are committed in one transaction. Takes the project for this agent session as write_source does.",
        output_schema = schema_for_output::<crate::agent::CreateResult>(),
        annotations(
            title = "Create document",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_document(
        &self,
        Parameters(arguments): Parameters<CreateDocumentArguments>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let agent = self.agent.clone();
        let client = context.client_info().map(|client| client.name);
        call("create_document", move || {
            agent.set_client(client);
            agent.create_document(&arguments)
        })
        .await
    }

    #[tool(
        description = "Ends this agent session: the server releases the project, and a window waiting for it can save again. Call it when your edits are done. A later write starts a new session.",
        output_schema = schema_for_output::<crate::agent::Finished>(),
        annotations(
            title = "Finish editing",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn finish_editing(&self) -> Result<CallToolResult, ErrorData> {
        let agent = self.agent.clone();
        call("finish_editing", move || Ok(agent.finish_editing())).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ApiforgeServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new("apiforge", env!("CARGO_PKG_VERSION")))
        .with_instructions(INSTRUCTIONS.to_owned())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let supports_cache_hints = supports_cache_hints(&context);
        Ok(ListToolsResult {
            result_type: Some(ResultType::COMPLETE),
            tools: TOOL_ORDER
                .iter()
                .filter_map(|name| self.tool_router.get(name).cloned())
                .collect(),
            meta: None,
            next_cursor: None,
            ttl_ms: supports_cache_hints.then_some(0),
            cache_scope: supports_cache_hints.then_some(CacheScope::Public),
        })
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let folder = self.folder.clone();
        let listed = blocking("resources/list", move || resources::list(&folder)).await?;
        let supports_cache_hints = supports_cache_hints(&context);
        Ok(ListResourcesResult {
            result_type: Some(ResultType::COMPLETE),
            resources: listed
                .into_iter()
                .map(|resource| {
                    Resource::new(resource.uri, resource.name)
                        .with_description(resource.description)
                        .with_mime_type(resources::MIME_TYPE)
                })
                .collect(),
            meta: None,
            next_cursor: None,
            ttl_ms: supports_cache_hints.then_some(0),
            cache_scope: supports_cache_hints.then_some(CacheScope::Private),
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let server = self.clone();
        let uri = request.uri;
        let read = {
            let uri = uri.clone();
            blocking("resources/read", move || server.resource_text(&uri)).await?
        };
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(read, uri).with_mime_type(resources::MIME_TYPE),
        ])
        .into())
    }
}

fn supports_cache_hints(context: &RequestContext<RoleServer>) -> bool {
    context
        .protocol_version()
        .is_some_and(|version| version >= ProtocolVersion::V_2026_07_28)
}

/// Runs a resource request on the blocking pool. A refusal is a JSON-RPC
/// error whose `data` carries the tools' `{ code, message }`.
async fn blocking<T: Send + 'static>(
    name: &'static str,
    work: impl FnOnce() -> Result<T, ToolError> + Send + 'static,
) -> Result<T, ErrorData> {
    let result = tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ErrorData::internal_error("The request stopped unexpectedly", None))?;
    result.map_err(|error| {
        eprintln!("apiforge mcp: {name}: {}", error.code.as_str());
        let data = Some(json!({ "code": error.code, "message": error.message }));
        match error.code {
            ErrorCode::PathRejected => ErrorData::invalid_params(error.message, data),
            ErrorCode::UnknownDocument => ErrorData::resource_not_found(error.message, data),
            _ => ErrorData::internal_error(error.message, data),
        }
    })
}

/// Runs a tool on the blocking pool: storage reads and runs must not stall
/// the transport. The result is `structuredContent` and its pretty JSON
/// text; a result or an error over 2 MiB becomes `result_too_large`.
async fn call<T: Serialize + Send + 'static>(
    name: &'static str,
    work: impl FnOnce() -> Result<T, ToolError> + Send + 'static,
) -> Result<CallToolResult, ErrorData> {
    let result = tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ErrorData::internal_error("The tool stopped unexpectedly", None))?;
    let (value, is_error) = match tools::bound(result) {
        Ok(value) => (serde_json::to_value(value), false),
        Err(error) => {
            eprintln!("apiforge mcp: {name}: {}", error.code.as_str());
            (serde_json::to_value(error), true)
        }
    };
    let value = value.map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    let mut result = if is_error {
        CallToolResult::structured_error(value)
    } else {
        CallToolResult::structured(value)
    };
    result.content = vec![ContentBlock::text(text)];
    Ok(result)
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The control channel for a server on `handle` (MCP-08b). A window's
/// *Take back* waits for the run in progress, cleanup included, telling the
/// window it waits; then the session ends after the write in flight, if any,
/// and the window is told `released`.
fn connect_windows(server: &ApiforgeServer, handle: tokio::runtime::Handle) -> Arc<Hub> {
    let hub = Hub::new(handle);
    server.agent.set_hub(hub.clone());
    let agent = server.agent.clone();
    let runs = server.runs.clone();
    let running = server.running.clone();
    let told = Arc::downgrade(&hub);
    hub.on_take_back(move || {
        if runs.is_busy() {
            if let Some(hub) = told.upgrade() {
                let (path, live) = lock(&running).clone().unwrap_or_default();
                hub.broadcast(&Message::Run {
                    path,
                    mode: if live {
                        RunKind::Live
                    } else {
                        RunKind::Simulation
                    },
                    state: RunState::Running,
                });
            }
            while runs.is_busy() {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        agent.give_back();
    });
    hub
}

/// Serves until stdin ends, then returns. The runtime exists only here.
pub fn serve_stdio(folder: PathBuf) -> io::Result<()> {
    serve_stdio_with(folder, ServerSettings::default())
}

/// As `serve_stdio`, with the run limits of `settings`. At the end of stdin a
/// run in progress has rmcp's five-second drain to finish and be answered;
/// after that it is cancelled, and its cleanup is waited for up to ten seconds.
pub fn serve_stdio_with(folder: PathBuf, settings: ServerSettings) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let server = ApiforgeServer::with_settings(folder, settings);
    let _hub = connect_windows(&server, runtime.handle().clone());
    let runs = server.runs().clone();
    let agent = server.agent().clone();
    let eof = Arc::new(OnceLock::new());
    let stdin = EndOfInput {
        inner: tokio::io::stdin(),
        lost: server.client_lost().clone(),
        at: eof.clone(),
    };
    let result = runtime.block_on(async move {
        let service = match server.serve((stdin, tokio::io::stdout())).await {
            Ok(service) => service,
            // The client went away before choosing a lifecycle: a normal end.
            Err(ServerInitializeError::ConnectionClosed(_)) => return Ok(()),
            Err(error) => return Err(io::Error::other(error.to_string())),
        };
        service.waiting().await.map_err(io::Error::other)?;
        // Ending the service, after its drain, cancelled every request still
        // running, a run among them; a live run was cancelled at EOF. Its
        // cleanup gets ten seconds from the end of stdin.
        let started = eof.get().copied().unwrap_or_else(Instant::now);
        while runs.is_busy() && started.elapsed() < CLEANUP_WAIT {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok(())
    });
    // The end of stdin ends the agent session: the writer lock is released,
    // and a window told so, before the runtime winds down.
    let _ = agent.finish_editing();
    runtime.shutdown_timeout(Duration::from_millis(500));
    result
}

/// Stdin that says when it ends: the client is gone, so a live run in
/// progress is cancelled at once instead of after rmcp's drain.
struct EndOfInput<R> {
    inner: R,
    lost: CancellationToken,
    at: Arc<OnceLock<Instant>>,
}

impl<R: AsyncRead + Unpin> AsyncRead for EndOfInput<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buffer.filled().len();
        let polled = Pin::new(&mut self.inner).poll_read(context, buffer);
        let ended = match &polled {
            Poll::Ready(Ok(())) => buffer.filled().len() == before && buffer.remaining() > 0,
            Poll::Ready(Err(_)) => true,
            Poll::Pending => false,
        };
        if ended {
            let _ = self.at.set(Instant::now());
            self.lost.cancel();
        }
        polled
    }
}

/// The journal's runs from before this server's live run, and whether it
/// could be read.
type Before = (Vec<JournalRun>, bool);

/// Marks a live run of this server as in progress for the journal reads of
/// the overview and `run`, until dropped.
struct JournalBefore(Arc<Mutex<Option<Before>>>);

impl JournalBefore {
    fn set(slot: &Arc<Mutex<Option<Before>>>, before: Before) -> Self {
        *slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(before);
        Self(slot.clone())
    }
}

impl Drop for JournalBefore {
    fn drop(&mut self) {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
}
