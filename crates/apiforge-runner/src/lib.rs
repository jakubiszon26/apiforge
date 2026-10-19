//! Execution engine: runs a scenario of a validated project and reports each step.
//!
//! Semantics follow the TypeScript reference runner: serial execution in dependency
//! order, Start reachability, IF branches, components, an independent cleanup phase,
//! diagnostics (up to a node, a single step) and redaction of everything published.
//! Requests are sent with [`HttpTransport`] in live mode; simulation mode uses the
//! saved sample responses and sends nothing.
//!
//! [`run_scenario`] is the async entry point. GUI code uses [`RunnerService`], which
//! owns its own runtime and exposes the newest snapshot of each run.
mod engine;
mod http;
pub mod plan;
pub mod redact;
mod request;
mod service;
mod support;

pub use engine::run_scenario;
pub use http::{HttpTransport, TransportError};
pub use request::{RunError, RunLimits, RunOptions, RunRequest, validate};
pub use service::{MAX_ACTIVE_RUNS, Notify, RunHandle, RunnerService};
pub use support::{SeededRandom, rfc3339_millis};
