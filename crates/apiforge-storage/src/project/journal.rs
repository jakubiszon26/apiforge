//! Journal operations share the project owner's writer lock and I/O queue.
use super::*;

impl ProjectStore {
    pub(crate) fn journal_access(
        &self,
        action: &mut dyn FnMut(&Path) -> std::result::Result<(), crate::StorageError>,
    ) -> std::result::Result<(), crate::StorageError> {
        self.writable()?;
        let _worker = self
            .owner
            .worker
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if self.head()?.active_transaction_id.is_some() {
            return Err(ProjectError::RecoveryRequired.into());
        }
        let path = self.journal_path()?;
        action(&path)
    }

    /// Where the project's run journal lives, `.apiforge/local/journal.json`.
    pub(crate) fn journal_path(&self) -> Result<PathBuf> {
        path::resolve(self.root(), JOURNAL)
    }
}

const JOURNAL: &str = ".apiforge/local/journal.json";
