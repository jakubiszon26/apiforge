//! Single-writer workspace sessions with external-change detection and explicit recovery.
mod file;
mod session;

pub use file::StorageError as DocumentError;
pub use file::{Loaded, public_project, write_atomic};
pub use session::{CodeDraft, DraftTarget, Drafts, LoadedDrafts, StorageError, WorkspaceStore};
