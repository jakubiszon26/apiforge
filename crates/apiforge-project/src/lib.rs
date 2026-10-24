//! Folder project sessions (ADR 0012). One `ProjectSession` owns every document
//! of a folder project: its source, analysis state, model and model history.
//! Views and workers hold no project copies; they send commands naming stable
//! document IDs and receive versioned events. Nothing here depends on egui.
mod analysis;
mod context;
mod document;
mod layout;
mod migration;
mod prepare;
mod session;

pub use analysis::{Analysis, AnalysisInput, AnalysisTicket, Restored};
pub use document::{AnalysisState, DocumentSession, ModelState, Unavailable};
pub use layout::{FixedPlacer, Placer};
pub use migration::{
    CATALOGUE_FILE, CatalogueEntry, MigrationError, MigrationOutcome, ProjectCatalogue,
    migrate_workspace,
};
pub use prepare::prepare_project;
pub use session::{
    Declaration, DeletePreview, DraftStatus, Event, EventKind, ExternalChanges, ExternalCheck,
    ExternalConflict, ExternalOutcome, ImportCandidate, ImportItem, ObservedFile, Origin,
    ProjectSession, Published, Registration, RegistryState, SaveOutcome, SourceSurvey,
    TextFileSession, declaration, rename_handle,
};

pub use apiforge_script::Document;
pub use apiforge_storage::project::{
    DocumentEntry, DocumentKind, ProjectError, ProjectManifest, RecoveryText,
};

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
    #[error("No text file with this path is open.")]
    UnknownTextFile,
    #[error("This file is a registered document; open it as a document.")]
    RegisteredDocument,
    #[error("This file is API Forge's own project data and is not edited as text.")]
    ManagedData,
    #[error("Save or discard the unsaved text of this file before closing it.")]
    UnsavedTextFile,
    #[error("The document's source is pending, invalid or unavailable; its model is locked.")]
    ModelLocked,
    #[error("The edited model is not a valid definition for this document.")]
    InvalidModel,
    #[error("The model cannot be written as source without semantic loss.")]
    Unprintable,
    #[error("Published component {component_id}@{revision} is not a valid definition.")]
    InvalidRevision { component_id: String, revision: u32 },
    #[error("Only a component can be published.")]
    NotAComponent,
    #[error("Component {component_id} already has revision {revision}.")]
    RevisionExists { component_id: String, revision: u32 },
    #[error(
        "This component is published as {published}; Stage II does not rename a published component."
    )]
    HandleChanged { published: String },
    #[error(
        "Reprinting this source would drop {comments} comment(s) that belong to no statement; decide explicitly first."
    )]
    CommentsWouldBeLost { comments: usize },
    #[error("A recovered draft of this document needs a keep or discard decision first.")]
    DraftDecisionRequired,
    #[error("Local recovery data is unreadable; export or discard unsaved work explicitly.")]
    RecoveryUnavailable,
    #[error("Save the project configuration or discard it explicitly before closing.")]
    UnsavedConfiguration,
    #[error("The analysis result is stale and was discarded.")]
    StaleAnalysis,
    #[error("This diagnostic has no suggested fix.")]
    NoFix,
    #[error("The document source exceeds the 1 MiB limit.")]
    SourceTooLarge,
    #[error("A registered source must keep the .afs extension outside .apiforge.")]
    RegisteredSourceExtension,
    #[error("{} is not UTF-8 text.", .0.display())]
    NotText(std::path::PathBuf),
    #[error("The source does not declare the chosen kind of document.")]
    KindMismatch,
    #[error("Another component already uses the handle {0}.")]
    HandleCollision(String),
    #[error("The file changed outside API Forge; reload it or export your version first.")]
    ExternalConflict,
    #[error("The project registry changed outside API Forge; reload the project first.")]
    RegistryChanged,
    #[error(transparent)]
    Migration(#[from] MigrationError),
    #[error("A legacy project cannot be written as a folder project: {0}")]
    Unrepresentable(&'static str),
}

impl From<apiforge_storage::StorageError> for Error {
    fn from(error: apiforge_storage::StorageError) -> Self {
        Self::Migration(MigrationError::Workspace(error))
    }
}
