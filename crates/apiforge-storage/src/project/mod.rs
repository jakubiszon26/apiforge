//! Folder persistence primitives (ADR 0011). These APIs perform blocking I/O;
//! desktop callers must use a storage worker. Opening a store never initialises
//! a manifest or enables folder mode in legacy startup.
mod cache;
mod codec;
mod conflict;
mod diagram;
mod evidence;
mod explorer;
mod io;
mod journal;
mod metadata;
mod path;
mod recovery;
mod schema;
mod transaction;
mod trash;
mod workspace;

pub use cache::{CacheNotice, CacheProvenance, LastValidRead, LastValidSnapshot};
pub use conflict::{
    ConflictArchive, RecoveryCandidate, RecoveryCandidateStatus, RecoveryCompletion,
    TransactionRecoveryPreview,
};
pub use diagram::{DiagramFile, DiagramRecord, DocumentKey, IdentityBase};
pub use explorer::{
    MAX_TREE_DEPTH, MAX_TREE_ENTRIES, TreeEntry, TreeEntryKind, TreeScan, tree_order,
};
pub use metadata::{DiagramRecoveryPreview, MetadataReplacement};
pub use path::ProjectPath;
pub use recovery::{
    FieldRecovery, RecoveryDocument, RecoveryRead, RecoveryText, SourceRecovery, TextSelection,
};
pub use schema::{DocumentEntry, DocumentKind, MANIFEST_PATH, ProjectManifest, RevisionEntry};
pub use transaction::{FileState, ProjectChange, ProjectSnapshot, ProjectStore};
pub use trash::{
    RestoreDestination, TrashCommit, TrashEntry, TrashFile, TrashIndex, TrashPreview,
    TrashRestorePreview,
};
pub use workspace::{
    CameraState, DEFAULT_LIBRARY_WIDTH, PanelSizes, TabState, ViewMode, WorkspaceState,
};

use std::{
    io as std_io,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TRANSACTION_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TRANSACTION_OPERATIONS: usize = 1024;

pub type Result<T> = std::result::Result<T, ProjectError>;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("Cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std_io::Error,
    },
    #[error("The folder project is open in another writer session.")]
    Locked,
    #[error("The folder project is read-only.")]
    ReadOnly,
    #[error("Invalid project-relative path.")]
    InvalidPath,
    #[error("A managed path contains a link or a case collision: {path}")]
    UnsafePath { path: PathBuf },
    #[error("Invalid folder project metadata; existing data was preserved.")]
    InvalidMetadata,
    #[error("Unsupported folder format version; existing data was preserved.")]
    UnsupportedVersion,
    #[error("The operation exceeds a folder storage limit.")]
    Limit,
    #[error("The folder changed while reading; retry after external writes finish.")]
    Busy,
    #[error("A folder transaction requires recovery before reading or writing.")]
    RecoveryRequired,
    #[error("A transaction target changed outside this session: {path}")]
    Conflict { path: PathBuf },
    #[error("A new managed payload contains a known session secret.")]
    Secret,
    #[error("Published component sources cannot be changed or removed.")]
    ImmutableRevision,
    #[error("Invalid transaction plan or staged recovery data.")]
    InvalidTransaction,
    #[error("Invalid source recovery data; existing files were preserved.")]
    InvalidRecovery,
    #[error("Source recovery data belongs to another project.")]
    ForeignRecovery,
    #[error("Confirm source recovery backup restoration before saving recovery data.")]
    RecoveryDecisionRequired,
    #[error("No previewed source recovery backup is available.")]
    NoRecoveryCandidate,
    #[error("Invalid identity or layout checkpoint; source definitions were preserved.")]
    InvalidCheckpoint,
    #[error("Invalid trash operation or preserved payload; existing files were preserved.")]
    InvalidTrash,
    #[error(
        "Invalid conflict approval or preserved conflict copy; the transaction remains unresolved."
    )]
    InvalidResolution,
    #[error(
        "A directory or inaccessible path needs manual recovery before this transaction can finish."
    )]
    UnsupportedRecoveryConflict,
    #[error("No active transaction matches this recovery preview.")]
    NoActiveTransaction,
    #[error("Invalid last-valid cache; source definitions were preserved.")]
    InvalidCache,
    #[error(
        "Existing last-valid cache needs an explicit discard or recovery decision before replacement."
    )]
    CacheReplacementRequired,
}

impl ProjectError {
    fn io(path: &Path, source: std_io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}

#[cfg(test)]
mod fault_tests;
#[cfg(test)]
mod tests;
