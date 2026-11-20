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
