//! Journal operations share the project owner's writer lock and I/O queue,
//! and so do the other machine-local records a writer keeps beside it.
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

    /// A machine-local record a client of the store keeps beside the run
    /// journal, `.apiforge/local/<name>`, such as the MCP server's live-run
    /// request ids (MCP-06): never tracked, never part of the project's
    /// format. Read by any store; `None` when it does not exist.
    pub fn read_local(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.read_file(&local_record(name)?)
    }

    /// Replaces the local record `name` atomically; only the writer may.
    pub fn write_local(&self, name: &str, bytes: &[u8]) -> Result<()> {
        self.writable()?;
        let relative = local_record(name)?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ProjectError::Limit);
        }
        let _worker = self
            .owner
            .worker
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let path = path::resolve(self.root(), &relative)?;
        crate::write_atomic(&path, bytes).map_err(|error| match error {
            crate::DocumentError::Io { path, source } => ProjectError::Io { path, source },
            other => ProjectError::io(&path, std_io::Error::other(other.to_string())),
        })
    }
}

const JOURNAL: &str = ".apiforge/local/journal.json";

/// A local record's path: a plain `.json` name that is none of the store's
/// own files.
fn local_record(name: &str) -> Result<String> {
    let plain = name.ends_with(".json")
        && name.len() > ".json".len()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-.".contains(&byte)
        });
    let reserved = [
        "journal.json",
        "journal.unreadable.json",
        "head.json",
        "holder.json",
    ];
    let relative = format!(".apiforge/local/{name}");
    if !plain || reserved.contains(&name) || name.contains("..") {
        return Err(ProjectError::UnsafePath {
            path: PathBuf::from(relative),
        });
    }
    Ok(relative)
}

#[cfg(test)]
mod local_record_tests {
    use super::local_record;

    #[test]
    fn a_local_record_is_a_plain_json_name_of_its_own() {
        assert_eq!(
            local_record("agent-requests.json").unwrap(),
            ".apiforge/local/agent-requests.json"
        );
        for refused in [
            "journal.json",
            "holder.json",
            "head.json",
            "../x.json",
            "a/b.json",
            "Upper.json",
            ".json",
            "x.txt",
        ] {
            assert!(local_record(refused).is_err(), "{refused}");
        }
    }
}
