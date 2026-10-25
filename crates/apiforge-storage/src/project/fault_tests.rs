//! Exercise the real persistence functions, not only transaction-level callbacks.
use super::tests::{initialise, manifest, path, write};
use super::{
    io::{
        Boundary, Phase,
        testing::{Event, with_hook},
    },
    *,
};
use std::{
    cell::RefCell,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    time::{Duration, Instant},
};

const OLD: &[u8] = b"scenario \"Main\" {}\n";
const NEW: &[u8] = b"scenario \"Saved\" {}\n";

fn open_writer(root: &Path) -> ProjectStore {
    // Process-spawning tests run in parallel. An inherited lock descriptor can
    // briefly survive close until exec; wait for OS release without forcing or
    // replacing the persistent lock. A real leaked owner still fails the test.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match ProjectStore::open_exclusive(root) {
            Ok(store) => return store,
            Err(ProjectError::Locked) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(error) => panic!("writer did not become available: {error}"),
        }
    }
}

fn setup() -> (tempfile::TempDir, ProjectStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = open_writer(directory.path());
    initialise(&store);
    store
        .commit(vec![
            write("obsolete.txt", FileState::Missing, b"obsolete"),
            ProjectChange::CreateDirectory {
                path: path("empty"),
            },
        ])
        .unwrap();
    (directory, store)
}
fn changes() -> Vec<ProjectChange> {
    vec![
        write("scenarios/main.afs", FileState::of(OLD), NEW),
        write(
            ".apiforge/diagram.json",
            FileState::Missing,
            b"{\"formatVersion\":1,\"records\":[]}",
        ),
        ProjectChange::CreateDirectory { path: path("new") },
        write(
            "new/extra.afs",
            FileState::Missing,
            b"scenario \"Extra\" {}\n",
        ),
        ProjectChange::RemoveFile {
            path: path("obsolete.txt"),
            expected: FileState::of(b"obsolete"),
        },
        ProjectChange::RemoveDirectory {
            path: path("empty"),
        },
    ]
}

fn trace<T>(run: impl FnOnce() -> T) -> (T, Vec<Event>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let saved = events.clone();
    let result = with_hook(
        move |event| {
            saved.borrow_mut().push(event.clone());
            Ok(())
        },
        run,
    );
    (result, Rc::try_unwrap(events).unwrap().into_inner())
}
fn inject<T>(stop: usize, kind: io::ErrorKind, run: impl FnOnce() -> T) -> T {
    let mut count = 0;
    with_hook(
        move |event| {
            let index = count;
            count += 1;
            if index == stop {
                Err(ProjectError::io(
                    &event.path,
                    io::Error::new(kind, "injected persistence failure"),
                ))
            } else {
                Ok(())
            }
        },
        run,
    )
}
fn active(store: &ProjectStore) {
    let _worker = store.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
    store
        .commit_locked(changes(), |phase| {
            if phase == "activation" {
                Err(ProjectError::io(
                    Path::new("activation"),
                    io::Error::other("stop"),
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
}
fn is_activated(directory: &Path) -> bool {
    let head: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join(".apiforge/local/head.json")).unwrap())
            .unwrap();
    !head["activeTransactionId"].is_null() || head["generation"].as_u64().unwrap() == 3
}
fn assert_generation(directory: &Path, store: &ProjectStore, saved: bool) {
    let snapshot = store.load().unwrap().1;
    assert_eq!(snapshot.generation, if saved { 3 } else { 2 });
    assert_eq!(
        fs::read(directory.join("scenarios/main.afs")).unwrap(),
        if saved { NEW } else { OLD }
    );
    assert_eq!(directory.join(".apiforge/diagram.json").exists(), saved);
    assert_eq!(directory.join("new/extra.afs").exists(), saved);
    assert_eq!(directory.join("obsolete.txt").exists(), !saved);
    assert_eq!(directory.join("empty").exists(), !saved);
}

#[test]
fn enospc_and_permission_errors_at_every_commit_io_boundary_never_acknowledge_partial_generations()
{
    let (_directory, store) = setup();
    let (result, events) = trace(|| store.commit(changes()));
    result.unwrap();
    for required in [
        Boundary::TemporaryCreate,
        Boundary::Write,
        Boundary::FileSync,
        Boundary::Replace,
        Boundary::DirectorySync,
        Boundary::CreateDirectory,
        Boundary::RemoveFile,
        Boundary::RemoveDirectory,
        Boundary::Cleanup,
    ] {
        assert!(
            events.iter().any(|event| event.boundary == required),
            "uncovered {required:?}"
        );
    }
    for kind in [io::ErrorKind::StorageFull, io::ErrorKind::PermissionDenied] {
        for (stop, event) in events.iter().enumerate() {
            let (directory, store) = setup();
            let result = inject(stop, kind, || store.commit(changes()));
            let saved = is_activated(directory.path());
            if result.is_ok() {
                assert!(saved, "acknowledged {event:?}");
            }
            drop(store);
            let store = open_writer(directory.path());
            store.recover().unwrap();
            assert_generation(directory.path(), &store, saved);
        }
    }
}

#[test]
fn every_replay_io_failure_can_be_retried_after_reopening() {
    let (_directory, store) = setup();
    active(&store);
    let (result, events) = trace(|| store.recover());
    result.unwrap();
    for stop in 0..events.len() {
        let (directory, store) = setup();
        active(&store);
        let _ = inject(stop, io::ErrorKind::StorageFull, || store.recover());
        drop(store);
        let store = open_writer(directory.path());
        store.recover().unwrap();
        assert_generation(directory.path(), &store, true);
    }
}

#[test]
fn partially_written_temporary_payload_never_replaces_the_original() {
    let (directory, store) = setup();
    let bytes = vec![b'x'; MAX_SOURCE_BYTES];
    let result = with_hook(
        |event| {
            if event.boundary == Boundary::Write && event.phase == Phase::After {
                Err(ProjectError::io(
                    &event.path,
                    io::Error::from(io::ErrorKind::StorageFull),
                ))
            } else {
                Ok(())
            }
        },
        || {
            store.commit(vec![write(
                "scenarios/main.afs",
                FileState::of(OLD),
                &bytes,
            )])
        },
    );
    assert!(result.is_err());
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        OLD
    );
    assert_generation(directory.path(), &store, false);
}

fn recovery() -> SourceRecovery {
    SourceRecovery {
        format_version: 1,
        project_id: manifest().id,
        documents: vec![RecoveryDocument {
            document_id: manifest().documents[0].document_id.clone(),
            entity_id: manifest().documents[0].entity_id.clone(),
            kind: DocumentKind::Scenario,
            base_revision: None,
            path: path("scenarios/main.afs"),
            base_hash: Some(super::transaction::hash(OLD)),
            source_version: 7,
            text: "scenario \"Unfinished\" {".into(),
            selection: TextSelection {
                anchor: 0,
                caret: 2,
            },
            fields: Vec::new(),
            checkpoint: None,
        }],
        texts: Vec::new(),
    }
}
fn recovery_setup() -> (
    tempfile::TempDir,
    ProjectStore,
    RecoveryRead,
    SourceRecovery,
) {
    let (directory, store) = setup();
    let first = recovery();
    let saved = store
        .save_source_recovery(&store.load_source_recovery(&manifest().id).unwrap(), &first)
        .unwrap();
    let mut next = first;
    next.documents[0].source_version += 1;
    next.documents[0].text.push_str(" latest");
    (directory, store, saved, next)
}

#[test]
fn every_recovery_backup_and_primary_io_failure_keeps_complete_source_work_without_false_success() {
    let (_directory, store, baseline, next) = recovery_setup();
    let (result, events) = trace(|| store.save_source_recovery(&baseline, &next));
    assert!(result.is_ok());
    for kind in [io::ErrorKind::StorageFull, io::ErrorKind::PermissionDenied] {
        for stop in 0..events.len() {
            let (directory, store, baseline, next) = recovery_setup();
            let result = inject(stop, kind, || store.save_source_recovery(&baseline, &next));
            drop(store);
            let reader = ProjectStore::open_read_only(directory.path()).unwrap();
            let read = reader.load_source_recovery(&manifest().id).unwrap();
            let observed = read.recovery().unwrap();
            assert!(observed == &recovery() || observed == &next);
            if result.is_ok() {
                assert_eq!(observed, &next);
            }
            assert_eq!(
                fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
                OLD
            );
        }
    }
}

#[test]
fn every_cache_rebuild_io_failure_preserves_old_metadata_before_new_cache_is_installed() {
    fn prepare() -> (tempfile::TempDir, ProjectStore, LastValidRead) {
        let (directory, store) = setup();
        fs::write(
            directory.path().join(super::cache::CACHE_PATH),
            b"unsupported old cache",
        )
        .unwrap();
        let read = store.load_last_valid_cache(&manifest().id).unwrap();
        (directory, store, read)
    }
    let snapshot = super::cache::tests::snapshot();
    let (_directory, store, read) = prepare();
    let (result, events) =
        trace(|| store.replace_last_valid_cache(&read, std::slice::from_ref(&snapshot)));
    assert!(result.is_ok());
    assert!(
        events
            .iter()
            .any(|event| event.boundary == Boundary::InstallNew)
    );
    for stop in 0..events.len() {
        let (directory, store, read) = prepare();
        let _ = inject(stop, io::ErrorKind::StorageFull, || {
            store.replace_last_valid_cache(&read, std::slice::from_ref(&snapshot))
        });
        drop(store);
        let store = ProjectStore::open_read_only(directory.path()).unwrap();
        let cache = store.load_last_valid_cache(&manifest().id).unwrap();
        if cache.notice().is_some() {
            assert_eq!(
                fs::read(directory.path().join(super::cache::CACHE_PATH)).unwrap(),
                b"unsupported old cache"
            );
        } else {
            assert_eq!(cache.snapshots().len(), 1);
            let history = directory.path().join(super::metadata::HISTORY);
            assert!(fs::read_dir(history).unwrap().any(|entry| {
                fs::read(entry.unwrap().path().join("original.bin"))
                    .ok()
                    .as_deref()
                    == Some(b"unsupported old cache")
            }));
        }
        assert_eq!(
            fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
            OLD
        );
    }
}

#[test]
fn every_shared_metadata_replacement_failure_preserves_the_original_until_its_copy_is_durable() {
    fn prepare() -> (tempfile::TempDir, ProjectStore, DiagramRecoveryPreview) {
        let (directory, store) = setup();
        fs::write(
            directory.path().join(".apiforge/diagram.json"),
            b"old invalid checkpoint",
        )
        .unwrap();
        let read = store.preview_diagram_recovery().unwrap();
        (directory, store, read)
    }
    let replacement = DiagramFile {
        format_version: 1,
        records: Vec::new(),
    };
    let (_directory, store, read) = prepare();
    let (result, events) = trace(|| store.replace_diagram_checkpoint(&read, &replacement));
    assert!(result.is_ok());
    for stop in 0..events.len() {
        let (directory, store, read) = prepare();
        let _ = inject(stop, io::ErrorKind::PermissionDenied, || {
            store.replace_diagram_checkpoint(&read, &replacement)
        });
        let saved = is_activated(directory.path());
        drop(store);
        let store = open_writer(directory.path());
        store.recover().unwrap();
        let bytes = fs::read(directory.path().join(".apiforge/diagram.json")).unwrap();
        if saved {
            assert_eq!(DiagramFile::from_bytes(&bytes).unwrap(), replacement);
            assert!(
                fs::read_dir(directory.path().join(super::metadata::HISTORY))
                    .unwrap()
                    .any(|entry| fs::read(entry.unwrap().path().join("original.bin"))
                        .ok()
                        .as_deref()
                        == Some(b"old invalid checkpoint"))
            );
        } else {
            assert_eq!(bytes, b"old invalid checkpoint");
        }
    }
}

#[test]
fn every_conflict_preservation_and_proof_sync_failure_keeps_foreign_bytes_or_their_durable_copy() {
    fn prepare() -> (tempfile::TempDir, ProjectStore) {
        let (directory, store) = setup();
        active(&store);
        fs::write(
            directory.path().join("scenarios/main.afs"),
            b"foreign source",
        )
        .unwrap();
        (directory, store)
    }
    let (_directory, store) = prepare();
    let preview = store.preview_transaction_recovery().unwrap().unwrap();
    let (result, events) = trace(|| store.finish_transaction_recovery(&preview));
    assert!(result.is_ok());
    for stop in 0..events.len() {
        let (directory, store) = prepare();
        let preview = store.preview_transaction_recovery().unwrap().unwrap();
        let _ = inject(stop, io::ErrorKind::StorageFull, || {
            store.finish_transaction_recovery(&preview)
        });
        drop(store);
        let store = open_writer(directory.path());
        match store.recover() {
            Ok(_) => {
                assert_generation(directory.path(), &store, true);
                let root = directory.path().join(".apiforge/local/conflicts");
                assert!(
                    fs::read_dir(root)
                        .unwrap()
                        .flat_map(|entry| fs::read_dir(entry.unwrap().path()).unwrap())
                        .any(|entry| fs::read(entry.unwrap().path().join("0.bin"))
                            .ok()
                            .as_deref()
                            == Some(b"foreign source"))
                );
            }
            Err(ProjectError::Conflict { .. }) => assert_eq!(
                fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
                b"foreign source"
            ),
            Err(error) => panic!("unrecoverable boundary {stop}: {error}"),
        }
    }
}

fn wait_marker(child: &mut std::process::Child, marker: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if marker.exists() {
            return true;
        }
        if child.try_wait().unwrap().is_some() {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn native_writer_lock_is_exclusive_and_killing_it_releases_ownership_without_replacing_the_lock_file()
 {
    let (directory, store) = setup();
    drop(store);
    let marker = directory.path().join("child-marker");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "project::fault_tests::native_persistence_child",
        ])
        .env("APIFORGE_P1_CHILD_ROOT", directory.path())
        .env("APIFORGE_P1_CHILD_STOP", "lock")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let ready = wait_marker(&mut child, &marker);
    let blocked = matches!(
        ProjectStore::open_exclusive(directory.path()),
        Err(ProjectError::Locked)
    );
    let _ = child.kill();
    child.wait().unwrap();
    assert!(ready);
    assert!(blocked);
    let store = open_writer(directory.path());
    assert_generation(directory.path(), &store, false);
    assert!(
        directory
            .path()
            .join(".apiforge/local/project.lock")
            .exists()
    );
}

#[test]
fn native_process_kill_at_every_persistence_boundary_never_exposes_a_mixed_generation() {
    let (_directory, store) = setup();
    let (result, events) = trace(|| store.commit(changes()));
    result.unwrap();
    for (stop, event) in events.iter().enumerate() {
        let (directory, store) = setup();
        drop(store);
        let marker = directory.path().join("child-marker");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "project::fault_tests::native_persistence_child",
            ])
            .env("APIFORGE_P1_CHILD_ROOT", directory.path())
            .env("APIFORGE_P1_CHILD_STOP", stop.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let ready = wait_marker(&mut child, &marker);
        let activated = is_activated(directory.path());
        let reader = ProjectStore::open_read_only(directory.path()).unwrap();
        match reader.load() {
            Ok(_) => assert_generation(directory.path(), &reader, activated),
            Err(ProjectError::RecoveryRequired) => assert!(activated),
            Err(error) => panic!("boundary {stop}: {error}"),
        }
        let _ = child.kill();
        child.wait().unwrap();
        assert!(ready, "child missed boundary {stop}: {event:?}");
        let writer = open_writer(directory.path());
        writer.recover().unwrap();
        assert_generation(directory.path(), &writer, activated);
    }
}

#[test]
#[ignore = "spawned by the parent native persistence tests"]
fn native_persistence_child() {
    let root = PathBuf::from(std::env::var_os("APIFORGE_P1_CHILD_ROOT").expect("parent root"));
    let stop = std::env::var("APIFORGE_P1_CHILD_STOP").expect("parent stop");
    let store = ProjectStore::open_exclusive(&root).unwrap();
    let wait = move || {
        let mut file = fs::File::create(root.join("child-marker")).unwrap();
        file.write_all(b"ready").unwrap();
        file.sync_all().unwrap();
        let _ = io::stdin().read_to_end(&mut Vec::new());
    };
    if stop == "lock" {
        wait();
        return;
    }
    let stop: usize = stop.parse().unwrap();
    let mut count = 0;
    with_hook(
        move |_| {
            let index = count;
            count += 1;
            if index == stop {
                wait();
            }
            Ok(())
        },
        || store.commit(changes()),
    )
    .unwrap();
}
