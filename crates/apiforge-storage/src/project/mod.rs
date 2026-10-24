//! Folder persistence primitives (ADR 0011). These APIs perform blocking I/O;
//! desktop callers must use a storage worker. Opening a store never initialises
//! a manifest or enables folder mode in legacy startup.
mod codec;
mod conflict;
mod diagram;
mod path;
mod recovery;
mod schema;
mod transaction;
mod trash;

pub use conflict::{
    ConflictArchive, RecoveryCandidate, RecoveryCandidateStatus, RecoveryCompletion,
    TransactionRecoveryPreview,
};
pub use diagram::{DiagramFile, DiagramRecord, DocumentKey, IdentityBase};
pub use path::ProjectPath;
pub use recovery::{FieldRecovery, RecoveryDocument, RecoveryRead, SourceRecovery, TextSelection};
pub use schema::{DocumentEntry, DocumentKind, MANIFEST_PATH, ProjectManifest, RevisionEntry};
pub use transaction::{FileState, ProjectChange, ProjectSnapshot, ProjectStore};
pub use trash::{
    RestoreDestination, TrashCommit, TrashEntry, TrashIndex, TrashPreview, TrashRestorePreview,
};

use std::{
    io,
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
        source: io::Error,
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
}

impl ProjectError {
    fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}

#[cfg(test)]
mod tests;
