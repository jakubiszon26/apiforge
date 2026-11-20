//! Acceptance tests for this crate's sprint tickets. Each test names the
//! criterion it covers in `docs/tickets/<ID>.md`.
use apiforge_storage::project::ProjectStore;
use std::time::{Duration, Instant};

/// KTLO-21, criteria 1 and 2: a probe never reports a writer that was already
/// dropped. `Command::spawn` forks this process, and the child keeps a
/// duplicate of the writer's lock descriptor until its exec, so the lock still
/// looks held after the only `File` for it has been closed. The spawning thread
/// creates that window deliberately; without it the race cannot occur at all.
#[cfg(unix)]
#[test]
fn a_reader_never_sees_a_writer_that_was_already_dropped() {
    use std::{
        process::{Command, Stdio},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    const ROUNDS: u32 = 600;

    let stop = Arc::new(AtomicBool::new(false));
    let spawning = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut spawned = 0u32;
            while !stop.load(Ordering::Relaxed) {
                let mut child = Command::new("/bin/true")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("spawn /bin/true");
                child.wait().expect("wait for /bin/true");
                spawned += 1;
            }
            spawned
        })
    };

    let directory = tempfile::tempdir().expect("temporary folder");
    let reader = ProjectStore::open_read_only(directory.path()).expect("read-only store");

    // A live writer is seen once, outside the loop: confirming a would-block
    // costs the confirmation window, and the released lock is what this test is
    // about.
    let writer = ProjectStore::open_exclusive(directory.path()).expect("writer");
    assert!(
        reader.has_live_writer().expect("probe"),
        "a live writer was not seen"
    );
    drop(writer);

    let mut rounds = 0;
    let mut stale = 0;
    // Stops at the first stale answer: one is already a defect, and the run
    // before the fix stays short.
    while rounds < ROUNDS && stale == 0 {
        rounds += 1;
        let writer = ProjectStore::open_exclusive(directory.path()).expect("writer");
        drop(writer);
        if reader.has_live_writer().expect("probe") {
            stale += 1;
        }
        // A probe that reports a writer must not contradict opening one.
        assert!(
            !ProjectStore::open(directory.path())
                .expect("writer after the probe")
                .is_read_only(),
            "round {rounds}: the probe kept the next writer out"
        );
    }
    stop.store(true, Ordering::Relaxed);
    let spawned = spawning.join().expect("spawning thread");
    assert_eq!(
        stale, 0,
        "a dropped writer was still reported as live in round {rounds} of {ROUNDS}, \
         alongside {spawned} process spawns"
    );
}

/// KTLO-21, criterion 3: the session holding the writer lock answers from what
/// it already knows, so confirming a would-block never makes a writer wait for
/// its own lock. Passes before the fix as well; it bounds the wait the fix adds.
#[test]
fn a_writer_reports_its_own_lock_without_waiting() {
    let directory = tempfile::tempdir().expect("temporary folder");
    let writer = ProjectStore::open_exclusive(directory.path()).expect("writer");
    let start = Instant::now();
    assert!(writer.has_live_writer().expect("probe"));
    let waited = start.elapsed();
    assert!(
        waited < Duration::from_millis(50),
        "the writer waited {waited:?} for its own lock"
    );
}

mod lang15 {
    //! LANG-15 (#412): the manifest names the language its files are written
    //! in. Storage reads and keeps the number; it does not compare it with
    //! the build's (that is `apiforge-project`'s `language`).
    use apiforge_storage::project::{ProjectError, ProjectManifest};
    use serde_json::{Value, json};

    /// A valid manifest with no document, of `formatVersion` 1.
    fn version_one() -> Value {
        json!({
            "formatVersion": 1,
            "id": "project-id",
            "name": "Language",
            "environment": { "baseUrl": "", "variables": {}, "secretNames": [] },
            "contracts": [],
            "documents": [],
            "revisions": []
        })
    }

    /// `version_one` as `formatVersion` 2 of language `language`.
    fn version_two(language: Value) -> Value {
        let mut manifest = version_one();
        manifest["formatVersion"] = 2.into();
        manifest["languageVersion"] = language;
        manifest
    }

    fn read(manifest: &Value) -> Result<ProjectManifest, ProjectError> {
        ProjectManifest::from_bytes(&serde_json::to_vec_pretty(manifest).unwrap())
    }

    /// Criterion 1: `formatVersion: 2` is read with its `languageVersion`.
    #[test]
    fn lang15_criterion_1_format_2_is_read_with_its_language() {
        let manifest = read(&version_two(3.into())).expect("a format-2 manifest");
        assert_eq!(manifest.format_version, 2);
        assert_eq!(manifest.language_version, Some(3));
        assert_eq!(manifest.language(), 3);
    }

    /// Criterion 1: `formatVersion: 1` is language 1 and must not carry the
    /// field.
    #[test]
    fn lang15_criterion_1_format_1_is_language_1_without_the_field() {
        let manifest = read(&version_one()).expect("a format-1 manifest");
        assert_eq!(manifest.language_version, None);
        assert_eq!(manifest.language(), 1);

        let mut carrying = version_one();
        carrying["languageVersion"] = 1.into();
        assert!(
            matches!(read(&carrying), Err(ProjectError::InvalidMetadata)),
            "format 1 never carries languageVersion"
        );
    }

    /// Criterion 1: a format-2 manifest without its language, with language
    /// 0, and `formatVersion: 3` are refused with today's messages.
    #[test]
    fn lang15_criterion_1_malformed_and_future_manifests_are_refused() {
        let mut missing = version_one();
        missing["formatVersion"] = 2.into();
        for (case, manifest) in [
            ("format 2 without languageVersion", missing),
            ("languageVersion 0", version_two(0.into())),
            ("languageVersion -1", version_two((-1).into())),
            ("languageVersion \"2\"", version_two("2".into())),
        ] {
            let error = read(&manifest).expect_err(case);
            assert!(
                matches!(error, ProjectError::InvalidMetadata),
                "{case}: {error:?}"
            );
            assert_eq!(
                error.to_string(),
                "Invalid folder project metadata; existing data was preserved."
            );
        }
        let mut future = version_two(2.into());
        future["formatVersion"] = 3.into();
        let error = read(&future).expect_err("format 3");
        assert!(
            matches!(error, ProjectError::UnsupportedVersion),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "Unsupported folder format version; existing data was preserved."
        );
    }

    /// Criterion 5: the released 0.1.0 reads `formatVersion` on its own,
    /// before anything else, and refuses every value but 1 with *Unsupported
    /// folder format version*. A manifest written for language 2 or later
    /// must therefore carry `formatVersion: 2` where 0.1.0 looks for it.
    #[test]
    fn lang15_criterion_5_the_first_release_refuses_format_2() {
        let manifest = ProjectManifest::new(
            2,
            "project-id".into(),
            "Language".into(),
            apiforge_core::format::Environment {
                base_url: String::new(),
                variables: Default::default(),
                secret_names: Vec::new(),
            },
            Vec::new(),
        );
        let bytes = manifest.to_public_bytes().unwrap();
        // 0.1.0's first read: only `formatVersion`, every other field
        // ignored (`ProjectManifest::from_bytes` of v0.1.0).
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Version {
            format_version: u32,
        }
        let version: Version = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(version.format_version, 2);
        assert!(
            String::from_utf8(bytes)
                .unwrap()
                .starts_with("{\n  \"formatVersion\": 2,\n  \"languageVersion\": 2,\n"),
            "the version comes first, then the language"
        );
    }
}

mod lang20e {
    //! LANG-20e (#500): a commit takes, beside its changes, the state of what
    //! its caller read, and writes nothing when a path it only read has
    //! changed. The checks during staging, a path both read and written, and
    //! recovery are in `src/project/transaction_tests.rs`.
    use apiforge_storage::project::{
        FileState, MANIFEST_PATH, ProjectChange, ProjectError, ProjectManifest, ProjectPath,
        ProjectStore, ReadSet,
    };
    use std::{fs, path::Path};

    const WRITTEN: &str = "scenarios/main.afs";
    const READ: &str = "scenarios/other.afs";
    const BEFORE: &[u8] = b"scenario \"Main\" {}\n";
    const AFTER: &[u8] = b"scenario \"Saved\" {}\n";

    fn path(value: &str) -> ProjectPath {
        ProjectPath::new(value).unwrap()
    }

    fn manifest(name: &str) -> Vec<u8> {
        ProjectManifest::new(
            1,
            "project-id".into(),
            name.into(),
            apiforge_core::format::Environment {
                base_url: String::new(),
                variables: Default::default(),
                secret_names: Vec::new(),
            },
            Vec::new(),
        )
        .to_public_bytes()
        .unwrap()
    }

    /// A project at generation 1: a manifest and two sources.
    fn project() -> (tempfile::TempDir, ProjectStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::open_exclusive(directory.path()).unwrap();
        store
            .commit(vec![
                ProjectChange::CreateDirectory {
                    path: path("scenarios"),
                },
                ProjectChange::Write {
                    path: path(WRITTEN),
                    expected: FileState::Missing,
                    bytes: BEFORE.to_vec(),
                },
                ProjectChange::Write {
                    path: path(READ),
                    expected: FileState::Missing,
                    bytes: b"scenario \"Other\" {}\n".to_vec(),
                },
                ProjectChange::Write {
                    path: path(MANIFEST_PATH),
                    expected: FileState::Missing,
                    bytes: manifest("Read set"),
                },
            ])
            .unwrap();
        (directory, store)
    }

    fn state(root: &Path, relative: &str) -> FileState {
        fs::read(root.join(relative)).map_or(FileState::Missing, |bytes| FileState::of(&bytes))
    }

    /// What a caller that read `paths` saw.
    fn read_set(root: &Path, paths: &[&str]) -> ReadSet {
        paths
            .iter()
            .map(|relative| (path(relative), state(root, relative)))
            .collect()
    }

    /// The one change of these tests: `WRITTEN` saved.
    fn save() -> Vec<ProjectChange> {
        vec![ProjectChange::Write {
            path: path(WRITTEN),
            expected: FileState::of(BEFORE),
            bytes: AFTER.to_vec(),
        }]
    }

    /// Refused with the path named, nothing written, nothing to recover.
    fn assert_refused(
        result: Result<u64, ProjectError>,
        root: &Path,
        store: &ProjectStore,
        named: &str,
    ) {
        match result {
            Err(ProjectError::Conflict { path }) => {
                assert!(path.ends_with(named), "{path:?} names {named}");
                assert!(
                    ProjectError::Conflict { path: path.clone() }
                        .to_string()
                        .contains(&path.display().to_string()),
                    "the message names the path"
                );
            }
            other => panic!("expected a conflict naming {named}, got {other:?}"),
        }
        assert_eq!(fs::read(root.join(WRITTEN)).unwrap(), BEFORE, "written");
        assert_eq!(store.recover().unwrap(), None, "a journal was left");
        assert_eq!(store.load().unwrap().1.generation, 1);
        assert_eq!(
            fs::read_dir(root.join(".apiforge/local/transactions"))
                .unwrap()
                .count(),
            0,
            "staging was kept"
        );
    }

    /// Criterion 1: a read set holds paths the commit does not write, the
    /// manifest among them; unchanged, the commit goes through.
    #[test]
    fn lang20e_criterion_1_a_read_set_holds_paths_the_commit_does_not_write() {
        let (directory, store) = project();
        let root = directory.path();
        let read = read_set(root, &[READ, MANIFEST_PATH, "scenarios/absent.afs"]);
        assert_eq!(read[&path("scenarios/absent.afs")], FileState::Missing);
        assert_eq!(store.commit_with_read_set(save(), &read).unwrap(), 2);
        assert_eq!(fs::read(root.join(WRITTEN)).unwrap(), AFTER);
        assert_eq!(state(root, READ), read[&path(READ)], "only read");
    }

    /// Criterion 2: a path only read, changed before the commit takes the
    /// lock.
    #[test]
    fn lang20e_criterion_2_a_read_path_changed_before_the_lock_refuses() {
        let (directory, store) = project();
        let root = directory.path();
        let read = read_set(root, &[READ]);
        fs::write(root.join(READ), b"scenario \"Foreign\" {}\n").unwrap();
        assert_refused(
            store.commit_with_read_set(save(), &read),
            root,
            &store,
            READ,
        );
    }

    /// Criterion 2: the manifest, read and not written, changed.
    #[test]
    fn lang20e_criterion_2_the_manifest_read_and_not_written_changed_refuses() {
        let (directory, store) = project();
        let root = directory.path();
        let read = read_set(root, &[MANIFEST_PATH, READ]);
        fs::write(root.join(MANIFEST_PATH), manifest("Renamed")).unwrap();
        assert_refused(
            store.commit_with_read_set(save(), &read),
            root,
            &store,
            MANIFEST_PATH,
        );
    }

    /// Criterion 2: a path the caller saw missing, which appeared.
    #[test]
    fn lang20e_criterion_2_a_path_expected_missing_that_appeared_refuses() {
        let (directory, store) = project();
        let root = directory.path();
        let appeared = "scenarios/new.afs";
        let read = read_set(root, &[appeared]);
        assert_eq!(read[&path(appeared)], FileState::Missing);
        fs::write(root.join(appeared), b"scenario \"New\" {}\n").unwrap();
        assert_refused(
            store.commit_with_read_set(save(), &read),
            root,
            &store,
            appeared,
        );
    }
}
