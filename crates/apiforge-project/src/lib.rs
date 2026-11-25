//! Folder project sessions (ADR 0012). One `ProjectSession` owns every document
//! of a folder project: its source, analysis state, model and model history.
//! Views and workers hold no project copies; they send commands naming stable
//! document IDs and receive versioned events. Nothing here depends on egui.
mod analysis;
mod context;
mod document;
mod git_rules;
pub mod language;
mod layout;
mod migration;
mod prepare;
mod session;
mod tries;

pub use analysis::{Analysis, AnalysisInput, AnalysisTicket, Restored};
pub use document::{AnalysisState, DocumentSession, EditedText, ModelState, Unavailable};
pub use layout::{FixedPlacer, Placer};
pub use migration::{
    CATALOGUE_FILE, CatalogueEntry, MigrationError, MigrationOutcome, PROJECTS_DIRECTORY,
    ProjectCatalogue, migrate_workspace,
};
pub use prepare::{as_written, prepare_files, prepare_project};
pub use session::{
    CommitChange, CommitChangeKind, ConflictDecision, ConflictPreview, ConflictTarget, Declaration,
    DeletePreview, DraftStatus, Event, EventKind, ExternalChanges, ExternalCheck, ExternalConflict,
    ExternalOutcome, ImportCandidate, ImportItem, NewDocumentCheck, ObservedFile, Origin,
    PreparedRun, ProjectSession, Published, Registration, RegistrationProblem, RegistryState,
    SaveOutcome, SessionIdentity, SourceSurvey, StorageCompletion, StorageMode, StorageSnapshot,
    TextFileSession, declaration, rename_handle,
};
pub use tries::{Try, TryContext, TrySet};

pub use apiforge_script::Document;
pub use apiforge_storage::project::{
    DocumentEntry, DocumentKind, Holder, HolderRole, ProjectError, ProjectManifest, RecoveryText,
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
    /// No try of the set has this ID (REQ-17a).
    #[error("No unsaved request has this ID.")]
    UnknownTry,
    /// A run of a try that does not check: its first error, in the words of
    /// the text's check (REQ-17a).
    #[error("{}", diagnostic.message)]
    TryDoesNotCheck {
        diagnostic: Box<apiforge_script::Diagnostic>,
    },
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
    /// A change of the board, or a step of the history, that does not make a
    /// valid definition (#565): the validation's message as it is, and the
    /// step it is about when it names one. Nothing of it is applied.
    #[error("{} Nothing was changed.", refusal(reason, step.as_deref()))]
    EditRefused {
        reason: String,
        step: Option<String>,
    },
    #[error("The model cannot be written as source without semantic loss.")]
    Unprintable,
    /// A move on the board whose statements cannot be moved line by line in
    /// the text as it is written (DIAG-06a); the file is not printed again.
    #[error("This move cannot be written into the code as it is laid out. Move the lines in Code.")]
    MoveNotWritten,
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
    #[error("The conflict candidates changed; review a fresh preview before deciding.")]
    StaleConflict,
    #[error(
        "The storage result belongs to an obsolete project state; its in-memory result was discarded."
    )]
    StaleStorage,
    #[error("The storage worker stopped unexpectedly; in-memory work was preserved.")]
    StorageWorker,
    #[error("The project registry changed outside API Forge; reload the project first.")]
    RegistryChanged,
    #[error(transparent)]
    Migration(#[from] MigrationError),
    #[error("A legacy project cannot be written as a folder project: {0}")]
    Unrepresentable(&'static str),
    /// The project's files are written in a language this build does not
    /// read yet; nothing of it is opened (LANG-15).
    #[error(
        "This project is written in language {project}; this build reads language {build}. Update API Forge."
    )]
    NewerLanguage { project: u32, build: u32 },
    /// The project's files are written in an older language: it opens for
    /// reading and editing, `check` reports this and it does not run until
    /// `apiforge fix` has raised it (LANG-15, LANG-20).
    #[error(
        "This project is written in language {project}; this build reads language {build}. Update it with apiforge fix --folder {} before it runs.",
        language::argument(folder)
    )]
    OlderLanguage {
        project: u32,
        build: u32,
        folder: std::path::PathBuf,
    },
    /// `apiforge fix`: a source of the project cannot be converted, so
    /// nothing is written (LANG-20).
    #[error("Nothing was written: a source of this project cannot be converted yet.")]
    NotConvertible,
    /// `apiforge fix`: the store refused the commit for its limits.
    #[error("This project is too large to convert in one step.")]
    TooLargeToConvert,
    /// `apiforge fix`: a file the command read changed before its commit.
    #[error(
        "{path} changed while the project was being converted; nothing was written. Run the command again."
    )]
    ChangedWhileConverting { path: String },
}

impl Error {
    /// The first sentence of a refused edit alone, for text being typed: the
    /// keys before the refused one are edits already applied (#565).
    pub fn refusal(&self) -> Option<String> {
        match self {
            Self::EditRefused { reason, step } => Some(refusal(reason, step.as_deref())),
            _ => None,
        }
    }
}

/// The first sentence of [`Error::EditRefused`]: its reason in lower case,
/// except a first word written in capitals and the block name `Start`, and
/// without a full stop of its own.
fn refusal(reason: &str, step: Option<&str>) -> String {
    let reason = reason.strip_suffix('.').unwrap_or(reason);
    let word: String = reason.chars().take_while(|c| c.is_alphanumeric()).collect();
    let capitals = word.chars().count() > 1 && !word.chars().any(char::is_lowercase);
    let mut letters = reason.chars();
    let reason = match letters.next() {
        Some(first) if !capitals && word != "Start" => {
            first.to_lowercase().chain(letters).collect()
        }
        _ => reason.to_owned(),
    };
    match step {
        Some(step) => format!("This edit was not applied: {reason} (step \"{step}\")."),
        None => format!("This edit was not applied: {reason}."),
    }
}

impl From<apiforge_storage::StorageError> for Error {
    fn from(error: apiforge_storage::StorageError) -> Self {
        Self::Migration(MigrationError::Workspace(error))
    }
}
