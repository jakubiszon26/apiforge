use super::*;
use apiforge_core::{ImportLimits, import_document};

fn workspace() -> Workspace {
    import_document(
        include_bytes!("../../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
}

fn drafts() -> Drafts {
    Drafts {
        version: 1,
        entries: vec![CodeDraft {
            project_id: "project".into(),
            target: DraftTarget::Scenario {
                id: "scenario".into(),
            },
            base_hash: "ab".repeat(32),
            text: "scenario \"Żółć\" {".into(),
            cursor: 0,
        }],
    }
}

#[test]
fn lock_is_shared_by_clones_and_released_after_the_last_one() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let store = WorkspaceStore::open_exclusive(&path).unwrap();
    let clone = store.clone();
    assert!(matches!(
        WorkspaceStore::open_exclusive(&path),
        Err(StorageError::Locked { .. })
    ));
    // Drafts share one directory lock even with a different workspace filename.
    assert!(matches!(
        WorkspaceStore::open_exclusive(directory.path().join("other.json")),
        Err(StorageError::Locked { .. })
    ));
    drop(store);
    assert!(matches!(
        WorkspaceStore::open_exclusive(&path),
        Err(StorageError::Locked { .. })
    ));
    drop(clone);
    assert!(WorkspaceStore::open_exclusive(&path).is_ok());
}

#[test]
fn a_read_only_session_reads_but_refuses_every_write() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let writer = WorkspaceStore::open_exclusive(&path).unwrap();
    let original = workspace();
    writer.save(&original).unwrap();
    writer.save_drafts(&drafts(), &[]).unwrap();
    let reader = WorkspaceStore::open_read_only(&path).unwrap();
    assert!(reader.is_read_only());
    assert_eq!(reader.load().unwrap().unwrap().workspace, original);
    assert_eq!(reader.load_drafts().unwrap().unwrap().drafts, drafts());
    assert!(matches!(
        reader.save(&original),
        Err(StorageError::ReadOnly)
    ));
    assert!(matches!(
        reader.save_drafts(&Drafts::default(), &[]),
        Err(StorageError::ReadOnly)
    ));
    assert!(matches!(
        reader.archive_legacy_workspace(),
        Err(StorageError::ReadOnly)
    ));
    assert!(matches!(
        reader.restore_backup(),
        Err(StorageError::ReadOnly)
    ));
    assert!(matches!(
        reader.restore_drafts_backup(),
        Err(StorageError::ReadOnly)
    ));
}

#[test]
fn equal_length_external_edits_require_a_reload_before_saving() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let store = WorkspaceStore::open_exclusive(&path).unwrap();
    let mut original = workspace();
    original.projects[0].name = "AAAA".into();
    store.save(&original).unwrap();
    let bytes = fs::read_to_string(&path).unwrap();
    let external = bytes.replacen("AAAA", "BBBB", 1);
    assert_eq!(bytes.len(), external.len());
    fs::write(&path, &external).unwrap();
    assert!(matches!(
        store.save(&original),
        Err(StorageError::ExternallyModified { .. })
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), external);
    assert!(!backup_path(&path).exists());
    let loaded = store.load().unwrap().unwrap().workspace;
    assert_eq!(loaded.projects[0].name, "BBBB");
    store.save(&loaded).unwrap();
    store.save(&loaded).unwrap();
}

#[test]
fn deletion_creation_and_backup_replacement_are_external_changes() {
    for change in ["delete", "backup", "create"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let store = WorkspaceStore::open_exclusive(&path).unwrap();
        let original = workspace();
        if change == "create" {
            assert!(store.load().unwrap().is_none());
            fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        } else {
            store.save(&original).unwrap();
            store.save(&original).unwrap();
            if change == "delete" {
                fs::remove_file(&path).unwrap();
            } else {
                fs::write(backup_path(&path), b"external backup").unwrap();
            }
        }
        assert!(
            matches!(
                store.save(&original),
                Err(StorageError::ExternallyModified { .. })
            ),
            "{change}"
        );
    }
}

#[test]
fn workspace_recovery_requires_confirmation_and_preserves_the_good_backup() {
    for missing in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let store = WorkspaceStore::open_exclusive(&path).unwrap();
        let original = workspace();
        store.save(&original).unwrap();
        store.save(&original).unwrap();
        let backup = fs::read(backup_path(&path)).unwrap();
        if missing {
            fs::remove_file(&path).unwrap();
        } else {
            fs::write(&path, b"{ broken").unwrap();
        }
        let preview = store.load().unwrap().unwrap();
        assert!(preview.recovered.is_some());
        assert!(matches!(
            store.save(&preview.workspace),
            Err(StorageError::RecoveryRequired { .. })
        ));
        if missing {
            assert!(!path.exists());
        } else {
            assert_eq!(fs::read(&path).unwrap(), b"{ broken");
        }
        assert_eq!(store.restore_backup().unwrap(), original);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), backup);
        assert!(store.load().unwrap().unwrap().recovered.is_none());
        store.save(&original).unwrap();
    }
}

#[test]
fn a_changed_recovery_candidate_is_never_applied() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let store = WorkspaceStore::open_exclusive(&path).unwrap();
    store.save(&workspace()).unwrap();
    store.save(&workspace()).unwrap();
    fs::write(&path, b"{ broken").unwrap();
    store.load().unwrap();
    fs::write(backup_path(&path), b"changed backup").unwrap();
    assert!(matches!(
        store.restore_backup(),
        Err(StorageError::ExternallyModified { .. })
    ));
    assert_eq!(fs::read(&path).unwrap(), b"{ broken");
}

#[test]
fn drafts_have_independent_fingerprints_and_the_same_recovery_rules() {
    let directory = tempfile::tempdir().unwrap();
    let store = WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
    let path = directory.path().join("drafts.json");
    let original = drafts();
    store.save_drafts(&original, &[]).unwrap();
    store.save_drafts(&original, &[]).unwrap();
    store.save(&workspace()).unwrap();
    assert_eq!(store.load_drafts().unwrap().unwrap().drafts, original);
    fs::write(&path, b"{ unfinished").unwrap();
    assert!(matches!(
        store.save_drafts(&original, &[]),
        Err(StorageError::ExternallyModified { .. })
    ));
    let loaded = store.load_drafts().unwrap().unwrap();
    assert!(loaded.recovered);
    assert!(matches!(
        store.save_drafts(&original, &[]),
        Err(StorageError::RecoveryRequired { .. })
    ));
    assert_eq!(store.restore_drafts_backup().unwrap(), original);
    assert!(!store.load_drafts().unwrap().unwrap().recovered);
    fs::write(&path, br#"{"version":2,"entries":[]}"#).unwrap();
    assert!(matches!(
        store.load_drafts(),
        Err(StorageError::UnsupportedDraftVersion)
    ));
    assert!(matches!(
        store.restore_drafts_backup(),
        Err(StorageError::UnsupportedDraftVersion) | Err(StorageError::NoRecovery { .. })
    ));
}

#[test]
fn invalid_or_secret_drafts_never_replace_existing_files() {
    let directory = tempfile::tempdir().unwrap();
    let store = WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
    let path = directory.path().join("drafts.json");
    store.save_drafts(&drafts(), &[]).unwrap();
    let previous = fs::read(&path).unwrap();
    for case in ["cursor", "duplicate", "large", "hash", "secret"] {
        let mut changed = drafts();
        match case {
            "cursor" => changed.entries[0].cursor = changed.entries[0].text.find('Ż').unwrap() + 1,
            "duplicate" => changed.entries.push(changed.entries[0].clone()),
            "large" => changed.entries[0].text = "x".repeat(MAX_CODE_BYTES + 1),
            "hash" => changed.entries[0].base_hash = "invalid".into(),
            "secret" => changed.entries[0].text.push_str("canary-secret"),
            _ => unreachable!(),
        }
        assert!(
            store.save_drafts(&changed, &["canary-secret"]).is_err(),
            "{case}"
        );
        assert_eq!(fs::read(&path).unwrap(), previous);
        assert!(!backup_path(&path).exists());
    }
}

#[test]
fn another_process_cannot_lock_and_killing_the_writer_releases_the_lock() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let marker = directory.path().join("ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "session::tests::lock_test_child"])
        .env("APIFORGE_LOCK_TEST_PATH", &path)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let ready = marker.exists();
    let blocked = matches!(
        WorkspaceStore::open_exclusive(&path),
        Err(StorageError::Locked { .. })
    );
    let _ = child.kill();
    child.wait().unwrap();
    assert!(ready, "lock helper did not start");
    assert!(blocked, "another process acquired the lock");
    assert!(WorkspaceStore::open_exclusive(&path).is_ok());
}

#[test]
#[ignore = "launched by the parent process lock test"]
fn lock_test_child() {
    let path = PathBuf::from(
        std::env::var_os("APIFORGE_LOCK_TEST_PATH").expect("parent supplies test path"),
    );
    let _store = WorkspaceStore::open_exclusive(&path).unwrap();
    fs::write(path.with_file_name("ready"), b"ready").unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
