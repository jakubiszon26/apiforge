//! The durable record of request ids: `.apiforge/local/agent-requests.json`,
//! machine-local and untracked, written atomically under the writer lock.
//!
//! A live run's id is recorded before its first request goes out (MCP-06
//! criterion 9, owner decision A), as the run journal is; a new server that
//! finds it answers `unknown_outcome` instead of running it again. A
//! publication's id (MCP-05b) is recorded before its transaction and gets
//! the result right after the commit, so a new server answers a repeated
//! publication from the record, or `unknown_outcome` when an earlier server
//! stopped between the two. The same id may name a live run and a
//! publication: they are kept apart by `kind`.
//!
//! The record keeps the most recent `KEEP` entries of both kinds; an older
//! one is forgotten. A record that cannot be read refuses every live run
//! and every publication with a `request_id` until it is moved aside: going
//! on would risk doing one twice.
use crate::{
    agent::PublishResult,
    error::{ErrorCode, ToolError},
};
use apiforge_storage::project::{ProjectError, ProjectStore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{sync::Mutex, time::SystemTime};

/// The record's name under `.apiforge/local`.
pub const FILE: &str = "agent-requests.json";
/// Where the record lives, as the messages and the guide name it.
pub const PATH: &str = ".apiforge/local/agent-requests.json";
/// The entries the record keeps, of both kinds, the oldest dropped first.
pub const KEEP: usize = 1000;
/// The longest `request_id` accepted, so the record stays bounded.
pub const MAX_ID_BYTES: usize = 256;
/// Version 2 tags every entry with its `kind`; version 1 held live runs
/// only, and is still read.
const VERSION: u32 = 2;

/// One read, change and write of the record at a time in this process: a
/// live run and a publication of the same server may record together, and
/// the writer lock does not tell its own threads apart.
static UPDATING: Mutex<()> = Mutex::new(());

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u32,
    requests: Vec<Entry>,
}

/// Version 1: live runs only, without a `kind`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordV1 {
    #[serde(rename = "version")]
    _version: u32,
    requests: Vec<Started>,
}

/// The version alone, read before the rest.
#[derive(Deserialize)]
struct Versioned {
    version: u32,
}

/// An entry of the record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Entry {
    LiveRun(Started),
    Publish(Publication),
}

impl Entry {
    /// The same kind and the same id: the entry `other` replaces.
    fn names(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::LiveRun(a), Self::LiveRun(b)) => a.id == b.id,
            (Self::Publish(a), Self::Publish(b)) => a.id == b.id,
            _ => false,
        }
    }
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

/// A publication with a `request_id`: what it was asked to publish, when,
/// and, once the commit is done, its result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Publication {
    pub id: String,
    /// The working component's registered path.
    pub path: String,
    /// The hash the call named.
    pub expected_source_sha256: String,
    /// RFC 3339, when the transaction was about to start.
    pub started_at: String,
    /// The result, written right after the commit. Absent when the server
    /// that started the publication did not get to write it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<PublishResult>,
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

/// The live run an earlier process started with `id`, if the record has it.
pub fn find_live_run(store: &ProjectStore, id: &str) -> Result<Option<Started>, ToolError> {
    Ok(read(store)?
        .requests
        .into_iter()
        .rev()
        .find_map(|entry| match entry {
            Entry::LiveRun(started) if started.id == id => Some(started),
            _ => None,
        }))
}

/// The publication recorded with `id`, if the record has it.
pub fn find_publish(store: &ProjectStore, id: &str) -> Result<Option<Publication>, ToolError> {
    Ok(read(store)?
        .requests
        .into_iter()
        .rev()
        .find_map(|entry| match entry {
            Entry::Publish(publication) if publication.id == id => Some(publication),
            _ => None,
        }))
}

/// Records that a live run with `id` is about to send its first request.
pub fn record(store: &ProjectStore, id: &str, path: &str) -> Result<(), ToolError> {
    update(store, |record| {
        push(
            record,
            Entry::LiveRun(Started {
                id: id.to_owned(),
                path: path.to_owned(),
                started_at: now(),
            }),
        );
    })
}

/// Records that a publication with `id` is about to start its transaction,
/// without a result.
pub fn record_publish_started(
    store: &ProjectStore,
    id: &str,
    path: &str,
    expected_source_sha256: &str,
) -> Result<(), ToolError> {
    update(store, |record| {
        push(
            record,
            Entry::Publish(Publication {
                id: id.to_owned(),
                path: path.to_owned(),
                expected_source_sha256: expected_source_sha256.to_owned(),
                started_at: now(),
                result: None,
            }),
        );
    })
}

/// Writes the result into the publication recorded with `id`; one that is
/// no longer there is recorded again with it.
pub fn record_publish_result(
    store: &ProjectStore,
    id: &str,
    expected_source_sha256: &str,
    result: &PublishResult,
) -> Result<(), ToolError> {
    let result = PublishResult {
        replayed: false,
        ..result.clone()
    };
    update(store, |record| {
        let found = record
            .requests
            .iter_mut()
            .rev()
            .find_map(|entry| match entry {
                Entry::Publish(publication) if publication.id == id => Some(publication),
                _ => None,
            });
        match found {
            Some(publication) => publication.result = Some(result),
            None => push(
                record,
                Entry::Publish(Publication {
                    id: id.to_owned(),
                    path: result.path.clone(),
                    expected_source_sha256: expected_source_sha256.to_owned(),
                    started_at: now(),
                    result: Some(result),
                }),
            ),
        }
    })
}

/// Removes the publication recorded with `id`: it was refused before its
/// commit began, so the same id may publish once the cause is fixed.
pub fn forget_publish(store: &ProjectStore, id: &str) -> Result<(), ToolError> {
    update(store, |record| {
        record
            .requests
            .retain(|entry| !matches!(entry, Entry::Publish(publication) if publication.id == id));
    })
}

fn now() -> String {
    apiforge_runner::rfc3339_millis(SystemTime::now())
}

/// Reads the record, changes it and writes it as version 2. A record that
/// cannot be read is never overwritten.
fn update(store: &ProjectStore, change: impl FnOnce(&mut Record)) -> Result<(), ToolError> {
    let _updating = UPDATING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut record = read(store)?;
    change(&mut record);
    record.version = VERSION;
    let bytes = serde_json::to_vec_pretty(&record).map_err(|_| unwritable(None))?;
    store
        .write_local(FILE, &bytes)
        .map_err(|error| unwritable(Some(error)))
}

/// Adds `entry`, replacing one of the same kind and id, and keeps the
/// newest `KEEP` of both kinds.
fn push(record: &mut Record, entry: Entry) {
    record.requests.retain(|kept| !kept.names(&entry));
    record.requests.push(entry);
    let excess = record.requests.len().saturating_sub(KEEP);
    record.requests.drain(..excess);
}

fn read(store: &ProjectStore) -> Result<Record, ToolError> {
    match store.read_local(FILE) {
        Ok(Some(bytes)) => parse(&bytes),
        Ok(None) => Ok(Record {
            version: VERSION,
            requests: Vec::new(),
        }),
        Err(error) => Err(unreadable(&error.to_string())),
    }
}

/// A record of version 2, or of version 1 read as live runs.
fn parse(bytes: &[u8]) -> Result<Record, ToolError> {
    let not_a_record = || unreadable("not a record of request ids");
    let version = serde_json::from_slice::<Versioned>(bytes)
        .map_err(|_| not_a_record())?
        .version;
    let requests: Vec<Entry> = match version {
        1 => serde_json::from_slice::<RecordV1>(bytes)
            .map_err(|_| not_a_record())?
            .requests
            .into_iter()
            .map(Entry::LiveRun)
            .collect(),
        VERSION => {
            serde_json::from_slice::<Record>(bytes)
                .map_err(|_| not_a_record())?
                .requests
        }
        _ => return Err(unreadable("an unsupported version")),
    };
    if requests.len() > KEEP {
        return Err(unreadable("too many entries"));
    }
    Ok(Record { version, requests })
}

fn unreadable(why: &str) -> ToolError {
    ToolError::new(
        ErrorCode::ProjectUnreadable,
        format!(
            "The record of request ids, {PATH}, cannot be read ({why}); nothing was sent or published. A live run or a publication with a request_id is refused until the file is moved aside, because this server could not tell whether the id already ran. Without a request_id both still work"
        ),
    )
    .with_details(json!({ "file": PATH }))
}

fn unwritable(error: Option<ProjectError>) -> ToolError {
    let why = error.map_or_else(|| "it could not be encoded".to_owned(), |e| e.to_string());
    ToolError::new(
        ErrorCode::ProjectUnreadable,
        format!(
            "The record of request ids, {PATH}, could not be written ({why}); nothing was sent or published"
        ),
    )
    .with_details(json!({ "file": PATH }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(id: usize) -> Entry {
        Entry::LiveRun(Started {
            id: id.to_string(),
            path: "scenarios/a.afs".into(),
            started_at: "2026-10-28T10:00:00.000Z".into(),
        })
    }

    fn publication(id: usize) -> Entry {
        Entry::Publish(Publication {
            id: id.to_string(),
            path: "components/a.afs".into(),
            expected_source_sha256: "a".repeat(64),
            started_at: "2026-10-28T10:00:00.000Z".into(),
            result: None,
        })
    }

    fn key(entry: &Entry) -> (&'static str, &str) {
        match entry {
            Entry::LiveRun(started) => ("liveRun", &started.id),
            Entry::Publish(publication) => ("publish", &publication.id),
        }
    }

    /// The record keeps the newest `KEEP` entries of both kinds together; a
    /// repeat replaces the entry of its own kind only, so the same id may
    /// name a live run and a publication.
    #[test]
    fn the_record_keeps_the_newest_ids() {
        let mut record = Record::default();
        for index in 0..=KEEP {
            let entry = if index % 2 == 0 {
                started(index)
            } else {
                publication(index)
            };
            push(&mut record, entry);
        }
        assert_eq!(record.requests.len(), KEEP);
        assert_eq!(
            key(&record.requests[0]),
            ("publish", "1"),
            "the oldest is dropped"
        );
        push(&mut record, started(4));
        assert_eq!(record.requests.len(), KEEP);
        assert_eq!(
            key(record.requests.last().unwrap()),
            ("liveRun", "4"),
            "a repeat moves to the end"
        );
        assert_eq!(
            record.requests.iter().filter(|e| key(e).1 == "4").count(),
            1
        );
        push(&mut record, publication(4));
        assert_eq!(
            record
                .requests
                .iter()
                .filter(|e| key(e).1 == "4")
                .map(|e| key(e).0)
                .collect::<Vec<_>>(),
            ["liveRun", "publish"],
            "one id, two kinds"
        );
        assert_eq!(record.requests.len(), KEEP, "and still bounded");
    }

    /// A version-1 record holds live runs without a `kind`; it is read as
    /// live-run entries and written back as version 2.
    #[test]
    fn a_version_1_record_is_read_as_live_runs() {
        let v1 = br#"{ "version": 1, "requests": [
            { "id": "run-1", "path": "scenarios/a.afs", "startedAt": "2026-10-28T10:00:00.000Z" }
        ] }"#;
        let record = parse(v1).unwrap();
        assert_eq!(
            record.requests,
            vec![Entry::LiveRun(Started {
                id: "run-1".into(),
                path: "scenarios/a.afs".into(),
                started_at: "2026-10-28T10:00:00.000Z".into(),
            })]
        );
        let written = serde_json::to_value(Record {
            version: VERSION,
            ..record
        })
        .unwrap();
        assert_eq!(written["version"], 2);
        assert_eq!(written["requests"][0]["kind"], "liveRun");
        assert_eq!(
            written["requests"][0]["startedAt"],
            "2026-10-28T10:00:00.000Z"
        );
        // A version-2 entry in a version-1 file is not a version-1 record.
        let mixed = br#"{ "version": 1, "requests": [
            { "kind": "liveRun", "id": "run-1", "path": "scenarios/a.afs", "startedAt": "2026-10-28T10:00:00.000Z" }
        ] }"#;
        assert_eq!(parse(mixed).unwrap_err().code, ErrorCode::ProjectUnreadable);
    }

    /// A publication's entry reads back as written, with and without its
    /// result; an unknown kind, a field too many or a newer version is not a
    /// record this server can read.
    #[test]
    fn a_publication_round_trips_and_nothing_unknown_is_read() {
        let mut record = Record {
            version: VERSION,
            requests: vec![publication(1), started(2)],
        };
        let bytes = serde_json::to_vec_pretty(&record).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("result"));
        assert_eq!(parse(&bytes).unwrap(), record);
        if let Entry::Publish(publication) = &mut record.requests[0] {
            publication.result = Some(PublishResult {
                path: "components/a.afs".into(),
                handle: "a".into(),
                revision: 2,
                revision_path: ".apiforge/revisions/a/2.afs".into(),
                source_sha256: "b".repeat(64),
                generation: 3,
                wrote: true,
                changed: vec![".apiforge/revisions/a/2.afs".into()],
                replayed: false,
            });
        }
        let bytes = serde_json::to_vec_pretty(&record).unwrap();
        assert_eq!(parse(&bytes).unwrap(), record);
        for unreadable in [
            br#"{ "version": 2, "requests": [ { "kind": "other", "id": "x" } ] }"#.as_slice(),
            br#"{ "version": 2, "requests": [ { "kind": "liveRun", "id": "x", "path": "a.afs", "startedAt": "t", "extra": 1 } ] }"#,
            br#"{ "version": 3, "requests": [] }"#,
            b"{ broken",
        ] {
            assert_eq!(
                parse(unreadable).unwrap_err().code,
                ErrorCode::ProjectUnreadable,
                "{}",
                String::from_utf8_lossy(unreadable)
            );
        }
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
