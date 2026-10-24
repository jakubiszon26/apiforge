//! Folder project sessions (ADR 0012). One `ProjectSession` owns every document
//! of a folder project: its source, analysis state, model and model history.
//! Views and workers hold no project copies; they send commands naming stable
//! document IDs and receive versioned events. Nothing here depends on egui.
mod analysis;
mod context;
mod document;
mod layout;
mod prepare;
mod session;

pub use analysis::{Analysis, AnalysisInput, AnalysisTicket, Restored};
pub use document::{AnalysisState, DocumentSession, ModelState, Unavailable};
pub use layout::{FixedPlacer, Placer};
pub use prepare::prepare_project;
pub use session::{Event, EventKind, Origin, ProjectSession, SaveOutcome};

pub use apiforge_script::Document;
pub use apiforge_storage::project::{DocumentEntry, DocumentKind, ProjectError, ProjectManifest};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] ProjectError),
    #[error("The folder has no API Forge project; initialise it explicitly first.")]
    NotAProject,
    #[error("The folder already contains an API Forge project.")]
    AlreadyAProject,
    #[error("No document has this ID.")]
    UnknownDocument,
    #[error("The document's source is pending, invalid or unavailable; its model is locked.")]
    ModelLocked,
    #[error("The edited model is not a valid definition for this document.")]
    InvalidModel,
    #[error("The model cannot be written as source without semantic loss.")]
    Unprintable,
    #[error("Published component {component_id}@{revision} is not a valid definition.")]
    InvalidRevision { component_id: String, revision: u32 },
    #[error("The analysis result is stale and was discarded.")]
    StaleAnalysis,
    #[error("The document source exceeds the 1 MiB limit.")]
    SourceTooLarge,
    #[error("A legacy project cannot be written as a folder project: {0}")]
    Unrepresentable(&'static str),
}
