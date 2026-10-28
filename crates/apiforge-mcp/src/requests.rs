//! The durable record of live-run request ids (MCP-06 criterion 9, owner
//! decision A): `.apiforge/local/agent-requests.json`, machine-local and
//! untracked, written atomically under the writer lock before a live run's
//! first request goes out, as the run journal is. A new server finds there
//! an id an earlier server started and answers `unknown_outcome` instead of
//! running it again. The record keeps the most recent `KEEP` ids; an older
//! one is forgotten. A record that cannot be read refuses every live run
//! with a `request_id` until it is moved aside: running would risk sending a
//! run twice.
use crate::error::{ErrorCode, ToolError};
use apiforge_storage::project::{ProjectError, ProjectStore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::SystemTime;

/// The record's name under `.apiforge/local`.
pub const FILE: &str = "agent-requests.json";
/// Where the record lives, as the messages and the guide name it.
pub const PATH: &str = ".apiforge/local/agent-requests.json";
/// The ids the record keeps, the oldest dropped first.
pub const KEEP: usize = 1000;
/// The longest `request_id` accepted, so the record stays bounded.
pub const MAX_ID_BYTES: usize = 256;
const VERSION: u32 = 1;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u32,
    requests: Vec<Started>,
}

/// A live run started with a `request_id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Started {
    pub id: String,
    /// The registered path that ran.
    pub path: String,
    /// RFC 3339, when the run was about to send its first request.
    pub started_at: String,
}

/// Refuses a `request_id` the record could not hold.
pub fn check_id(id: &str) -> Result<(), ToolError> {
    if id.len() > MAX_ID_BYTES {
        return Err(ToolError::new(
            ErrorCode::TooLarge,
            format!("A request_id is at most {MAX_ID_BYTES} bytes"),
        )
        .with_details(json!({ "bytes": id.len(), "limit": MAX_ID_BYTES })));
    }
    Ok(())
}

/// The run an earlier process started with `id`, if the record has it.
pub fn find(store: &ProjectStore, id: &str) -> Result<Option<Started>, ToolError> {
    Ok(read(store)?
        .requests
        .into_iter()
        .rev()
        .find(|started| started.id == id))
}

/// Records that a live run with `id` is about to send its first request.
pub fn record(store: &ProjectStore, id: &str, path: &str) -> Result<(), ToolError> {
    let mut record = read(store)?;
    push(
        &mut record,
        Started {
            id: id.to_owned(),
            path: path.to_owned(),
            started_at: apiforge_runner::rfc3339_millis(SystemTime::now()),
        },
    );
    let bytes = serde_json::to_vec_pretty(&record).map_err(|_| unwritable(None))?;
    store
        .write_local(FILE, &bytes)
        .map_err(|error| unwritable(Some(error)))
}

/// Adds `started`, replacing an entry of the same id, and keeps the newest
/// `KEEP`.
fn push(record: &mut Record, started: Started) {
    record.requests.retain(|entry| entry.id != started.id);
    record.requests.push(started);
    let excess = record.requests.len().saturating_sub(KEEP);
    record.requests.drain(..excess);
}

fn read(store: &ProjectStore) -> Result<Record, ToolError> {
    let bytes = match store.read_local(FILE) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return Ok(Record {
                version: VERSION,
                requests: Vec::new(),
            });
        }
        Err(error) => return Err(unreadable(&error.to_string())),
    };
    match serde_json::from_slice::<Record>(&bytes) {
        Ok(record) if record.version == VERSION && record.requests.len() <= KEEP => Ok(record),
        Ok(_) => Err(unreadable("an unsupported version or too many entries")),
        Err(_) => Err(unreadable("not a record of request ids")),
    }
}

fn unreadable(why: &str) -> ToolError {
    ToolError::new(
        ErrorCode::ProjectUnreadable,
        format!(
            "The record of live request ids, {PATH}, cannot be read ({why}); nothing was sent. A live run with a request_id is refused until the file is moved aside, because this server could not tell whether the id already ran. A live run without a request_id still works"
        ),
    )
    .with_details(json!({ "file": PATH }))
}

fn unwritable(error: Option<ProjectError>) -> ToolError {
    let why = error.map_or_else(|| "it could not be encoded".to_owned(), |e| e.to_string());
    ToolError::new(
        ErrorCode::ProjectUnreadable,
        format!(
            "The record of live request ids, {PATH}, could not be written ({why}); nothing was sent"
        ),
    )
    .with_details(json!({ "file": PATH }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(id: usize) -> Started {
        Started {
            id: id.to_string(),
            path: "scenarios/a.afs".into(),
            started_at: "2026-10-28T10:00:00.000Z".into(),
        }
    }

    #[test]
    fn the_record_keeps_the_newest_ids() {
        let mut record = Record::default();
        for id in 0..=KEEP {
            push(&mut record, started(id));
        }
        assert_eq!(record.requests.len(), KEEP);
        assert_eq!(record.requests[0].id, "1", "the oldest is dropped");
        push(&mut record, started(5));
        assert_eq!(record.requests.len(), KEEP);
        assert_eq!(
            record.requests.last().unwrap().id,
            "5",
            "a repeat moves to the end"
        );
        assert_eq!(record.requests.iter().filter(|s| s.id == "5").count(), 1);
    }

    #[test]
    fn a_long_id_is_refused() {
        assert!(check_id(&"x".repeat(MAX_ID_BYTES)).is_ok());
        assert_eq!(
            check_id(&"x".repeat(MAX_ID_BYTES + 1)).unwrap_err().code,
            ErrorCode::TooLarge
        );
    }
}
