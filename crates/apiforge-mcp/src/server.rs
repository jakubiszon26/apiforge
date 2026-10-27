//! The tools over MCP stdio with `rmcp`. Stdout carries protocol messages
//! only; the server's own lines go to stderr and never quote an argument.
use crate::{error::ToolError, tools};
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::schema_for_output, wrapper::Parameters},
    model::{
        CacheScope, CallToolResult, ContentBlock, ListToolsResult, PaginatedRequestParams,
        ProtocolVersion, ResultType,
    },
    service::{RequestContext, ServerInitializeError},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{io, path::PathBuf, sync::Arc, time::Duration};

/// The order `tools/list` returns, from the overview to the canonical text.
const TOOL_ORDER: [&str; 4] = ["project_overview", "read_source", "check", "print"];

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

#[derive(Clone, Debug)]
pub struct ApiforgeServer {
    folder: Arc<PathBuf>,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl ApiforgeServer {
    pub fn new(folder: PathBuf) -> Self {
        Self {
            folder: Arc::new(folder),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "What the API Forge project contains: name, base URL, environment variable and secret names, contracts, every document (kind, registered path, handle, base revision, source_sha256), the published component revisions, the project generation, who holds the project and whether live requests are allowed. Reads the saved project; writes nothing.",
        output_schema = schema_for_output::<tools::ProjectOverview>(),
        annotations(title = "Project overview", read_only_hint = true)
    )]
    async fn project_overview(&self) -> Result<CallToolResult, ErrorData> {
        let folder = self.folder.clone();
        call("project_overview", move || tools::project_overview(&folder)).await
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
}

#[tool_handler(
    router = self.tool_router,
    name = "apiforge",
    instructions = "API Forge project tools. Every document is named by its registered path; call project_overview first. These tools read the saved project and write nothing."
)]
impl ServerHandler for ApiforgeServer {
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let supports_cache_hints = context
            .protocol_version()
            .is_some_and(|version| version >= ProtocolVersion::V_2026_07_28);
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
}

/// Runs a tool on the blocking pool: storage reads must not stall the
/// transport. The result is `structuredContent` and its pretty JSON text.
async fn call<T: Serialize + Send + 'static>(
    name: &'static str,
    work: impl FnOnce() -> Result<T, ToolError> + Send + 'static,
) -> Result<CallToolResult, ErrorData> {
    let result = tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ErrorData::internal_error("The tool stopped unexpectedly", None))?;
    let (value, is_error) = match result {
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
    serve_stdio_with(folder, crate::tools::RunSettings::default())
}

/// As `serve_stdio`, with the run limits of `settings`.
pub fn serve_stdio_with(folder: PathBuf, settings: crate::tools::RunSettings) -> io::Result<()> {
    let _ = settings;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let result = runtime.block_on(async move {
        let service = match ApiforgeServer::new(folder)
            .serve(rmcp::transport::stdio())
            .await
        {
            Ok(service) => service,
            // The client went away before choosing a lifecycle: a normal end.
            Err(ServerInitializeError::ConnectionClosed(_)) => return Ok(()),
            Err(error) => return Err(io::Error::other(error.to_string())),
        };
        service.waiting().await.map_err(io::Error::other)?;
        Ok(())
    });
    runtime.shutdown_timeout(Duration::from_millis(500));
    result
}
