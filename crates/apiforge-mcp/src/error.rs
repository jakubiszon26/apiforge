//! The error every tool answers with when it could not do what was asked
//! (MCP-00, "The contract"). Codes form a fixed list documented in the
//! development guide; a message never quotes an argument or a source text.
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

/// The fixed list of error codes. Only some are used by the read tools;
/// the others are reserved for the tools of later tickets so the list is
/// stable from the first release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotAProject,
    ProjectUnreadable,
    UnknownDocument,
    InvalidText,
    InvalidInputs,
    NotAComponent,
    InvalidCheckpoint,
    StaleSource,
    StaleProject,
    HeldByWindow,
    HandoverPending,
    HandoverRefused,
    HeldByOther,
    LiveDisabled,
    CommentsWouldBeLost,
    SecretInText,
    SecretInSource,
    PathRejected,
    PathExists,
    HandleExists,
    RecoveryRequired,
    TooLarge,
    ResultTooLarge,
    RunInProgress,
    Cancelled,
    TimedOut,
    UnknownOutcome,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAProject => "not_a_project",
            Self::ProjectUnreadable => "project_unreadable",
            Self::UnknownDocument => "unknown_document",
            Self::InvalidText => "invalid_text",
            Self::InvalidInputs => "invalid_inputs",
            Self::NotAComponent => "not_a_component",
            Self::InvalidCheckpoint => "invalid_checkpoint",
            Self::StaleSource => "stale_source",
            Self::StaleProject => "stale_project",
            Self::HeldByWindow => "held_by_window",
            Self::HandoverPending => "handover_pending",
            Self::HandoverRefused => "handover_refused",
            Self::HeldByOther => "held_by_other",
            Self::LiveDisabled => "live_disabled",
            Self::CommentsWouldBeLost => "comments_would_be_lost",
            Self::SecretInText => "secret_in_text",
            Self::SecretInSource => "secret_in_source",
            Self::PathRejected => "path_rejected",
            Self::PathExists => "path_exists",
            Self::HandleExists => "handle_exists",
            Self::RecoveryRequired => "recovery_required",
            Self::TooLarge => "too_large",
            Self::ResultTooLarge => "result_too_large",
            Self::RunInProgress => "run_in_progress",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::UnknownOutcome => "unknown_outcome",
        }
    }
}

/// `{ code, message, wrote, details }`. The read tools never write, so
/// `wrote` is always `false` here.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ToolError {
    pub code: ErrorCode,
    pub message: String,
    pub wrote: bool,
    /// Structured context for the model: registered paths, diagnostics, how
    /// to narrow a request. An empty object when there is none.
    pub details: Value,
}

impl ToolError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            wrote: false,
            details: Value::Object(Default::default()),
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for ToolError {}
