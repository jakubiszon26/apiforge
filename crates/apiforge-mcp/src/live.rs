//! Live requests from the server (MCP-06): the permission slot, and what the
//! run journal says about runs an earlier process left unfinished.
use crate::tools::LiveRequests;
use schemars::JsonSchema;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::watch;

/// Who allows live requests right now. While a window is connected over the
/// control channel it decides (MCP-09); otherwise the start flag does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// No window is connected: `--allow-live` was given or not.
    Flag(bool),
    /// A connected window's per-project setting.
    Window { live: bool },
}

/// The server's current permission, behind a `watch` channel: MCP-09's
/// control channel writes `Window`, and its loss resets the slot to the flag.
/// No tool writes it.
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

    /// A connected window's setting: the test seam MCP-09 wires to the
    /// control channel's `permission { live }`.
    pub fn set_window(&self, _live: bool) {
        unimplemented!("MCP-06")
    }

    /// The window's channel is gone: the start flag applies again.
    pub fn window_disconnected(&self) {
        unimplemented!("MCP-06")
    }

    /// Follows every change of the slot.
    pub fn subscribe(&self) -> watch::Receiver<Permission> {
        self.slot.subscribe()
    }

    /// What `project_overview.live_requests` reports.
    pub fn describe(&self) -> LiveRequests {
        unimplemented!("MCP-06: {}", self.flag)
    }
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
