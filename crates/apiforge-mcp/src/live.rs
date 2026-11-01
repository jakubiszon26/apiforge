//! Live requests from the server (MCP-06): the permission slot, and what the
//! run journal says about runs an earlier process left unfinished.
use crate::{
    error::{ErrorCode, ToolError},
    secrets,
    tools::LiveRequests,
};
use apiforge_core::journal::{CleanupState, JournalRun};
use apiforge_storage::{
    JournalStore, StorageError,
    project::{ProjectError, ProjectStore},
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::watch;

/// The refusal while no window is connected and the flag was not given.
pub const NOT_ENABLED: &str = "live requests are not enabled on this server";
/// The refusal while a connected window's setting is off.
pub const OFF_IN_SETTINGS: &str = "live requests are off in this project's settings";

/// Who allows live requests right now. While a window is connected over the
/// control channel it decides (MCP-09); otherwise the start flag does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// No window is connected: `--allow-live` was given or not.
    Flag(bool),
    /// A connected window's per-project setting.
    Window { live: bool },
}

/// The server's current permission, behind a `watch` channel: while any
/// window is connected over the control channel the slot is `Window`, the
/// newest consent heard (off until one is), and when the last window goes
/// it is the flag again (MCP-09). No tool writes it.
#[derive(Clone, Debug)]
pub struct LivePermission {
    flag: bool,
    slot: Arc<watch::Sender<Permission>>,
}

impl LivePermission {
    pub fn new(flag: bool) -> Self {
        Self {
            flag,
            slot: Arc::new(watch::Sender::new(Permission::Flag(flag))),
        }
    }

    pub fn current(&self) -> Permission {
        *self.slot.borrow()
    }

    /// A connected window's setting, from the control channel's
    /// `permission { live }` (MCP-09).
    pub fn set_window(&self, live: bool) {
        self.slot.send_replace(Permission::Window { live });
    }

    /// The window's channel is gone: the start flag applies again.
    pub fn window_disconnected(&self) {
        self.slot.send_replace(Permission::Flag(self.flag));
    }

    /// Follows every change of the slot.
    pub fn subscribe(&self) -> watch::Receiver<Permission> {
        self.slot.subscribe()
    }

    /// The start flag, which decides while no window is connected.
    pub fn flag(&self) -> bool {
        self.flag
    }

    /// What `project_overview.live_requests` reports.
    pub fn describe(&self) -> LiveRequests {
        describe(self.current())
    }

    /// Refuses a live run unless the current permission allows it.
    pub(crate) fn check(&self) -> Result<(), ToolError> {
        match self.current() {
            Permission::Flag(true) | Permission::Window { live: true } => Ok(()),
            refused => Err(refusal(refused)),
        }
    }
}

/// What `project_overview.live_requests` says of `permission`.
fn describe(permission: Permission) -> LiveRequests {
    let (allowed, source) = match permission {
        Permission::Flag(true) => (true, "--allow-live"),
        Permission::Flag(false) => (false, "no --allow-live"),
        Permission::Window { live: true } => (true, "window setting"),
        Permission::Window { live: false } => (false, "off in the project's settings"),
    };
    LiveRequests {
        allowed,
        source: source.to_owned(),
    }
}

/// The refusal of a live run under `permission`: `live_disabled`, with the
/// source of the decision.
pub(crate) fn refusal(permission: Permission) -> ToolError {
    let message = match permission {
        Permission::Window { .. } => OFF_IN_SETTINGS,
        Permission::Flag(_) => NOT_ENABLED,
    };
    ToolError::new(ErrorCode::LiveDisabled, message)
        .with_details(json!({ "source": describe(permission).source }))
}

/// A live run an earlier process left unfinished, as `apiforge run` prints
/// it: the requests that may have changed the API.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct InterruptedRun {
    /// The scenario's or component's name when the run started.
    pub scenario: String,
    pub started_at: String,
    /// The run had reached its cleanup.
    pub cleanup_started: bool,
    pub requests: Vec<InterruptedRequest>,
    /// Older requests of a long run the journal no longer holds.
    pub elided: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct InterruptedRequest {
    /// The step's label, or its handle.
    pub step: String,
    pub method: String,
    /// Redacted as in a report.
    pub url: String,
    /// `Sending`, `Performed` or `Possibly performed`, as `apiforge run`
    /// prints it.
    pub state: String,
}

/// The runs `journal` holds, as the model reads them: only the requests
/// that may have changed the API, as `apiforge run` lists them, with every
/// known value masked again (the journal was redacted with the secrets of
/// the process that wrote it).
pub(crate) fn interrupted(runs: &[JournalRun], values: &[String]) -> Vec<InterruptedRun> {
    let masked = |text: &str| secrets::redact(text, values);
    runs.iter()
        .map(|run| InterruptedRun {
            scenario: masked(&run.scenario_name),
            started_at: run.started_at.clone(),
            cleanup_started: run.cleanup == CleanupState::Started,
            requests: run
                .effects()
                .map(|entry| InterruptedRequest {
                    step: masked(entry.display_name()),
                    method: entry.method.clone(),
                    url: masked(&entry.url),
                    state: entry.state.label().to_owned(),
                })
                .collect(),
            elided: run.elided,
        })
        .collect()
}

/// What the project's journal holds, read without opening it for writing:
/// through `store` when it holds the project (this server's agent session),
/// otherwise only while no other process holds it, since another writer's
/// journal may describe a run still in progress.
pub(crate) fn read_journal(store: &ProjectStore) -> Result<(Vec<JournalRun>, bool), ToolError> {
    if store.is_read_only() {
        match store.has_live_writer() {
            Ok(false) => {}
            Ok(true) => return Ok((Vec::new(), false)),
            Err(error) => return Err(journal_error(error.into())),
        }
    }
    JournalStore::read_project(store)
        .map(|read| (read.runs, read.unreadable))
        .map_err(journal_error)
}

/// A journal that cannot be opened or read: an interrupted transaction is
/// `recovery_required`, anything else `project_unreadable`.
pub(crate) fn journal_error(error: StorageError) -> ToolError {
    match error {
        StorageError::Project(ProjectError::RecoveryRequired) => ToolError::new(
            ErrorCode::RecoveryRequired,
            "The project has an interrupted file operation; the API Forge window finishes it when it opens the project",
        ),
        error => ToolError::new(ErrorCode::ProjectUnreadable, error.to_string()),
    }
}
