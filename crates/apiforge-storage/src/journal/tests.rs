use super::*;
use apiforge_core::{journal::EffectState, run::Phase};

fn entry(sequence: u64, state: EffectState) -> JournalEntry {
    JournalEntry {
        sequence,
        node_id: format!("node-{sequence}"),
        handle: format!("step_{sequence}"),
        label: Some("Create order".into()),
        phase: Phase::Main,
        method: "POST".into(),
        url: "https://api.example.com/orders".into(),
        started_at: "2026-10-23T10:00:00.000Z".into(),
        state,
        status: None,
        detail: None,
    }
}

fn run(id: &str) -> JournalRun {
    JournalRun::new(id, "scen-1", "Checkout", "2026-10-23T10:00:00.000Z")
}

/// Opens a journal in a fresh directory, keeping the temporary directory alive.
fn open() -> (tempfile::TempDir, LoadedJournal) {
    let directory = tempfile::tempdir().unwrap();
    let store = WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
    let loaded = JournalStore::open(store).unwrap();
    (directory, loaded)
}

#[test]
fn an_interrupted_run_is_found_by_the_next_session() {
    let (directory, loaded) = open();
    let path = directory.path().join("workspace.json");
    loaded.store.begin_run(&run("run-1")).unwrap();
    loaded
        .store
        .record("run-1", &entry(1, EffectState::InFlight))
        .unwrap();
    // The file is on disk before the request it describes is sent.
    assert!(directory.path().join("journal.json").exists());
    drop(loaded);

    let store = WorkspaceStore::open_exclusive(&path).unwrap();
    let reopened = JournalStore::open(store).unwrap();
    assert!(!reopened.unreadable);
    assert_eq!(reopened.interrupted.len(), 1);
    let interrupted = &reopened.interrupted[0];
    assert_eq!(interrupted.scenario_name, "Checkout");
    assert_eq!(interrupted.cleanup, CleanupState::NotStarted);
    assert_eq!(interrupted.entries[0].state, EffectState::InFlight);
    assert_eq!(interrupted.effects().count(), 1);
}

#[test]
fn a_finished_run_leaves_no_file_behind() {
    let (directory, loaded) = open();
    loaded.store.begin_run(&run("run-1")).unwrap();
    loaded
        .store
        .record("run-1", &entry(1, EffectState::Completed))
        .unwrap();
    loaded.store.cleanup_started("run-1").unwrap();
    loaded.store.finish_run("run-1").unwrap();
    assert!(loaded.store.snapshot().is_empty());
    assert!(!directory.path().join("journal.json").exists());
}

#[test]
fn a_second_run_does_not_erase_an_unacknowledged_one() {
    let (_directory, loaded) = open();
    loaded.store.begin_run(&run("crashed")).unwrap();
    loaded
        .store
        .record("crashed", &entry(1, EffectState::Uncertain))
        .unwrap();
    loaded.store.begin_run(&run("fresh")).unwrap();
    loaded.store.finish_run("fresh").unwrap();
    let remaining = loaded.store.snapshot();
    assert_eq!(remaining.runs.len(), 1);
    assert_eq!(remaining.runs[0].run_id, "crashed");

    loaded.store.discard(&["crashed".to_owned()]).unwrap();
    assert!(loaded.store.snapshot().is_empty());
}

#[test]
fn resolving_an_entry_updates_it_on_disk() {
    let (directory, loaded) = open();
    let path = directory.path().join("workspace.json");
    loaded.store.begin_run(&run("run-1")).unwrap();
    loaded
        .store
        .record("run-1", &entry(1, EffectState::InFlight))
        .unwrap();
    let mut resolved = entry(1, EffectState::NotSent);
    resolved.detail = Some("dns: failed to lookup address information".into());
    loaded.store.record("run-1", &resolved).unwrap();
    drop(loaded);

    let reopened = JournalStore::open(WorkspaceStore::open_exclusive(&path).unwrap()).unwrap();
    let run = &reopened.interrupted[0];
    assert_eq!(run.entries.len(), 1);
    assert_eq!(run.entries[0].state, EffectState::NotSent);
    // A request that never left the process is not an effect on the API.
    assert_eq!(run.effects().count(), 0);
}

#[test]
fn recording_an_unknown_run_is_an_error() {
    let (_directory, loaded) = open();
    assert!(
        loaded
            .store
            .record("absent", &entry(1, EffectState::InFlight))
            .is_err()
    );
    assert!(loaded.store.cleanup_started("absent").is_err());
}

#[test]
fn a_read_only_session_cannot_own_the_journal() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let _writer = WorkspaceStore::open_exclusive(&path).unwrap();
    let reader = WorkspaceStore::open_read_only(&path);
    assert!(matches!(
        JournalStore::open(reader.unwrap()),
        Err(StorageError::ReadOnly)
    ));
}

#[test]
fn an_unreadable_journal_is_preserved_and_the_session_starts_empty() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let journal = directory.path().join("journal.json");
    std::fs::write(&journal, b"{ not json").unwrap();
    let loaded = JournalStore::open(WorkspaceStore::open_exclusive(&path).unwrap()).unwrap();
    assert!(loaded.unreadable);
    assert!(loaded.interrupted.is_empty());
    assert!(!journal.exists());
    assert_eq!(
        std::fs::read(directory.path().join("journal.unreadable.json")).unwrap(),
        b"{ not json"
    );
}

#[test]
fn an_unknown_version_is_never_rewritten() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let journal = directory.path().join("journal.json");
    std::fs::write(&journal, br#"{"version": 2, "runs": []}"#).unwrap();
    assert!(matches!(
        JournalStore::open(WorkspaceStore::open_exclusive(&path).unwrap()),
        Err(StorageError::UnsupportedJournalVersion)
    ));
    assert_eq!(
        std::fs::read(&journal).unwrap(),
        br#"{"version": 2, "runs": []}"#
    );
}

#[test]
fn an_implausible_journal_is_rejected_rather_than_trusted() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut journal = Journal::default();
    let mut run = run("run-1");
    run.record(entry(1, EffectState::InFlight));
    run.entries.push(entry(1, EffectState::Completed));
    journal.begin(run);
    std::fs::write(
        directory.path().join("journal.json"),
        serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    let loaded = JournalStore::open(WorkspaceStore::open_exclusive(&path).unwrap()).unwrap();
    assert!(loaded.unreadable);
}

#[test]
fn the_journal_is_shared_with_the_workspace_directory_lock() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let loaded = JournalStore::open(WorkspaceStore::open_exclusive(&path).unwrap()).unwrap();
    assert_eq!(loaded.store.path(), directory.path().join("journal.json"));
    assert!(matches!(
        WorkspaceStore::open_exclusive(directory.path().join("other.json")),
        Err(StorageError::Locked { .. })
    ));
}
