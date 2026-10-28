//! The tools and resources over MCP stdio with `rmcp`. Stdout carries
//! protocol messages only; the server's own lines go to stderr and never
//! quote an argument.
use crate::{
    agent::{AgentSession, CreateDocumentArguments, WriteSourceArguments},
    error::{ErrorCode, ToolError},
    executor::{CancellationToken, EndedBy},
    resources,
    tools::{self, RunArguments, RunSettings, Runs},
};
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
use std::{
    io,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

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

const INSTRUCTIONS: &str = "API Forge project tools. Every document is named by its registered path; call project_overview first and language_reference before writing .afs. check_text checks a proposed text without saving it; run simulates the saved scenario or component and sends nothing. write_source and create_document save a checked text: the first write takes the project for this agent session and keeps it, so the window only watches, until you call finish_editing. Call finish_editing when your edits are done.";

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

#[derive(Clone, Debug)]
pub struct ApiforgeServer {
    folder: Arc<PathBuf>,
    runs: Runs,
    agent: Arc<AgentSession>,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl ApiforgeServer {
    pub fn new(folder: PathBuf) -> Self {
        Self::with_run_settings(folder, RunSettings::default())
    }

    /// A server whose runs follow `settings`.
    pub fn with_run_settings(folder: PathBuf, settings: RunSettings) -> Self {
        Self {
            agent: Arc::new(AgentSession::new(folder.clone())),
            folder: Arc::new(folder),
            runs: Runs::new(settings),
            tool_router: Self::tool_router(),
        }
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
        let folder = self.folder.clone();
        let agent = self.agent.clone();
        call("project_overview", move || {
            let mut overview = tools::project_overview(&folder)?;
            if overview.holder == tools::Holder::AnotherProcess && agent.is_active() {
                overview.holder = tools::Holder::AgentSession;
            }
            Ok(overview)
        })
        .await
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
        let folder = self.folder.clone();
        call("read_source", move || {
            tools::read_source(&folder, &arguments.path)
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
        let folder = self.folder.clone();
        call("check_text", move || {
            tools::check_text(&folder, &arguments.path, &arguments.text)
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
        let folder = self.folder.clone();
        call("print", move || tools::print(&folder, &arguments.path)).await
    }

    #[tool(
        description = "Simulates the saved scenario or component at `path` with the sample responses written in it: nothing is sent and nothing is written. Returns `status` (passed, failed or cancelled; a failed verdict is a result, not an error), every step with handle, label, status, message and, for a failed comparison, expected and actual, a component's steps nested beneath it, the generation and source_sha256. A component runs alone with `inputs` by name, its defaults filling the rest. `full: true` returns the runner's complete JSON report instead of the steps. run executes the saved source; check_text checks a proposal. One run at a time; a cancelled request or the ten-minute deadline stops the run and its cleanup still runs.",
        output_schema = schema_for_output::<tools::RunReport>(),
        annotations(title = "Run (simulation)", read_only_hint = true)
    )]
    async fn run(
        &self,
        Parameters(arguments): Parameters<RunArguments>,
        cancel: CancellationToken,
    ) -> Result<CallToolResult, ErrorData> {
        let folder = self.folder.clone();
        let runs = self.runs.clone();
        call("run", move || {
            let report = tools::run(&folder, &arguments, &runs, cancel)?;
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
        description = "Saves a new text for the registered document at `path`, exactly as given. The text is checked in the project's context first and saved only without errors (warnings allowed), together with its layout checkpoint, the same bytes as a save in the window. Name the version you read: `expected_source_sha256` from read_source (a different hash on disk is `stale_source`, nothing written) and optionally `expected_generation` (`stale_project`). The first write takes the project for this agent session and keeps it until finish_editing; while a window or another process holds the project the write is refused (`held_by_window`, `held_by_other`). A text containing a known secret value is `secret_in_text`. The same text and hash twice is a no-op.",
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
        agent.set_client(context.client_info().map(|client| client.name));
        call("write_source", move || agent.write_source(&arguments)).await
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
        agent.set_client(context.client_info().map(|client| client.name));
        call("create_document", move || agent.create_document(&arguments)).await
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
        let folder = self.folder.clone();
        let uri = request.uri;
        let read = {
            let uri = uri.clone();
            blocking("resources/read", move || resources::read(&folder, &uri)).await?
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

/// Serves until stdin ends, then returns. The runtime exists only here.
pub fn serve_stdio(folder: PathBuf) -> io::Result<()> {
    serve_stdio_with(folder, RunSettings::default())
}

/// As `serve_stdio`, with the run limits of `settings`. At the end of stdin a
/// run in progress has rmcp's five-second drain to finish and be answered;
/// after that it is cancelled, and its cleanup is waited for up to ten seconds.
pub fn serve_stdio_with(folder: PathBuf, settings: RunSettings) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let server = ApiforgeServer::with_run_settings(folder, settings);
    let runs = server.runs().clone();
    let agent = server.agent().clone();
    let result = runtime.block_on(async move {
        let service = match server.serve(rmcp::transport::stdio()).await {
            Ok(service) => service,
            // The client went away before choosing a lifecycle: a normal end.
            Err(ServerInitializeError::ConnectionClosed(_)) => return Ok(()),
            Err(error) => return Err(io::Error::other(error.to_string())),
        };
        service.waiting().await.map_err(io::Error::other)?;
        // Ending the service, after its drain, cancelled every request still
        // running, a run among them.
        let started = Instant::now();
        while runs.is_busy() && started.elapsed() < CLEANUP_WAIT {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok(())
    });
    // The end of stdin ends the agent session: the writer lock is released
    // before the runtime winds down.
    let _ = agent.finish_editing();
    runtime.shutdown_timeout(Duration::from_millis(500));
    result
}
