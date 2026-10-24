use super::*;
use crate::project::tests::{initialise, path, write};

fn interrupted() -> ProjectError {
    ProjectError::io(
        Path::new("fault-injection"),
        io::Error::other("simulated process stop"),
    )
}

fn save_plan() -> Vec<ProjectChange> {
    vec![
        write(
            "scenarios/main.afs",
            FileState::of(b"scenario \"Main\" {}\n"),
            b"scenario \"Saved\" {}\n",
        ),
        write(
            ".apiforge/diagram.json",
            FileState::Missing,
            b"{\"formatVersion\":1,\"records\":[]}",
        ),
    ]
}

#[test]
fn restart_at_every_transaction_boundary_never_accepts_mixed_files() {
    // preflight, staging directory, two payloads, descriptor, activation,
    // each target replacement, then committed head.
    for stop in 0..9 {
        let directory = tempfile::tempdir().unwrap();
        let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
        initialise(&writer);
        let mut count = 0;
        let result = writer.commit_with_hook(save_plan(), |_| {
            let current = count;
            count += 1;
            if current == stop {
                Err(interrupted())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err(), "stop {stop} was reached");
        drop(writer);
        let reader = ProjectStore::open_read_only(directory.path()).unwrap();
        if (5..8).contains(&stop) {
            assert!(
                matches!(reader.load(), Err(ProjectError::RecoveryRequired)),
                "stop {stop}"
            );
        } else {
            assert_eq!(
                reader.load().unwrap().1.generation,
                if stop < 5 { 1 } else { 2 }
            );
        }
        let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
        assert_eq!(
            writer.recover().unwrap(),
            if (5..8).contains(&stop) {
                Some(2)
            } else {
                None
            }
        );
        let (_, snapshot) = writer.load().unwrap();
        assert_eq!(
            snapshot.files[&path("scenarios/main.afs")].as_deref(),
            Some(if stop < 5 {
                b"scenario \"Main\" {}\n".as_slice()
            } else {
                b"scenario \"Saved\" {}\n".as_slice()
            })
        );
        assert_eq!(
            snapshot.files[&path(".apiforge/diagram.json")].is_some(),
            stop >= 5
        );
    }
}

#[test]
fn replay_can_itself_be_interrupted_at_every_operation_and_commit() {
    for stop in 0..3 {
        let directory = tempfile::tempdir().unwrap();
        let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
        initialise(&writer);
        writer
            .commit_with_hook(save_plan(), |boundary| {
                if boundary == "activation" {
                    Err(interrupted())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        drop(writer);
        let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
        let mut count = 0;
        writer
            .recover_with_hook(|_| {
                let current = count;
                count += 1;
                if current == stop {
                    Err(interrupted())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        drop(writer);
        let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
        writer.recover().unwrap();
        let (_, snapshot) = writer.load().unwrap();
        assert_eq!(snapshot.generation, 2);
        assert_eq!(
            snapshot.files[&path("scenarios/main.afs")].as_deref(),
            Some(b"scenario \"Saved\" {}\n".as_slice())
        );
        assert!(snapshot.files[&path(".apiforge/diagram.json")].is_some());
    }
}

#[test]
fn recovery_preserves_foreign_target_and_staged_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    writer
        .commit_with_hook(save_plan(), |boundary| {
            if boundary == "activation" {
                Err(interrupted())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    fs::write(
        directory.path().join("scenarios/main.afs"),
        b"foreign source!",
    )
    .unwrap();
    let head = writer.head().unwrap();
    let staging = writer
        .transaction_path(head.active_transaction_id.as_deref().unwrap())
        .unwrap();
    assert!(matches!(
        writer.recover(),
        Err(ProjectError::Conflict { .. })
    ));
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"foreign source!"
    );
    assert_eq!(
        fs::read(staging.join("0")).unwrap(),
        b"scenario \"Saved\" {}\n"
    );
    assert_eq!(writer.head().unwrap(), head);
    assert!(matches!(
        writer.commit(save_plan()),
        Err(ProjectError::RecoveryRequired)
    ));
}

#[test]
fn tampered_staged_data_never_replays_and_unknown_head_is_preserved() {
    for target in ["0", "transaction.json"] {
        let directory = tempfile::tempdir().unwrap();
        let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
        initialise(&writer);
        writer
            .commit_with_hook(save_plan(), |boundary| {
                if boundary == "activation" {
                    Err(interrupted())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        let head = writer.head().unwrap();
        let staging = writer
            .transaction_path(head.active_transaction_id.as_deref().unwrap())
            .unwrap();
        fs::write(staging.join(target), b"corrupt").unwrap();
        assert!(writer.recover().is_err());
        assert_eq!(
            fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
            b"scenario \"Main\" {}\n"
        );
        assert_eq!(fs::read(staging.join(target)).unwrap(), b"corrupt");
    }
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    let bytes = b"{\"formatVersion\":9,\"generation\":0,\"activeTransactionId\":null}";
    fs::write(directory.path().join(HEAD), bytes).unwrap();
    assert!(matches!(
        writer.recover(),
        Err(ProjectError::UnsupportedVersion)
    ));
    assert!(matches!(
        writer.commit(save_plan()),
        Err(ProjectError::UnsupportedVersion)
    ));
    assert_eq!(fs::read(directory.path().join(HEAD)).unwrap(), bytes);
}

#[test]
fn external_edit_during_staging_prevents_activation() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    let result = writer.commit_with_hook(save_plan(), |boundary| {
        if boundary == "descriptor" {
            fs::write(directory.path().join("scenarios/main.afs"), b"foreign").unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(ProjectError::Conflict { .. })));
    assert!(writer.head().unwrap().active_transaction_id.is_none());
    assert_eq!(writer.head().unwrap().generation, 1);
    assert!(!directory.path().join(".apiforge/diagram.json").exists());
}

#[test]
fn stable_reads_retry_equal_length_edits_and_stop_after_three_attempts() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    let source = path("scenarios/main.afs");
    let file = directory.path().join(source.as_str());
    let snapshot = writer
        .stable_read_with_hook(
            || Ok(vec![source.clone()]),
            |phase, attempt| {
                if phase == "read" && attempt == 0 {
                    fs::write(&file, b"scenario \"Else\" {}\n").unwrap();
                }
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        snapshot.files[&source].as_deref(),
        Some(b"scenario \"Else\" {}\n".as_slice())
    );
    let mut attempts = 0;
    let result = writer.stable_read_with_hook(
        || Ok(vec![source.clone()]),
        |phase, attempt| {
            if phase == "read" {
                attempts += 1;
                fs::write(
                    &file,
                    if attempt % 2 == 0 {
                        b"scenario \"Main\" {}\n"
                    } else {
                        b"scenario \"Else\" {}\n"
                    },
                )
                .unwrap();
            }
            Ok(())
        },
    );
    assert!(matches!(result, Err(ProjectError::Busy)));
    assert_eq!(attempts, 3);
}

#[test]
fn head_is_rechecked_after_fingerprints_and_manifest_changes_block_activation() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    let result = writer.stable_read_with_hook(
        || Ok(vec![path("scenarios/main.afs")]),
        |phase, _| {
            if phase == "recheck" {
                writer.write_head(&Head {
                    active_transaction_id: Some("interrupted".into()),
                    ..writer.head().unwrap()
                })?;
            }
            Ok(())
        },
    );
    assert!(matches!(result, Err(ProjectError::RecoveryRequired)));
    writer
        .write_head(&Head {
            format_version: 1,
            generation: 1,
            active_transaction_id: None,
        })
        .unwrap();
    let result = writer.commit_with_hook(save_plan(), |phase| {
        if phase == "descriptor" {
            fs::write(
                directory.path().join(MANIFEST_PATH),
                b"invalid external manifest",
            )
            .unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(ProjectError::Busy)));
    assert!(writer.head().unwrap().active_transaction_id.is_none());
}

#[test]
fn ordered_directory_operations_preflight_and_replay_after_parent_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    assert!(matches!(
        writer.commit(vec![write(
            "missing/child.afs",
            FileState::Missing,
            b"source"
        )]),
        Err(ProjectError::InvalidTransaction)
    ));
    assert!(matches!(
        writer.commit(vec![ProjectChange::RemoveDirectory {
            path: path("scenarios")
        }]),
        Err(ProjectError::InvalidTransaction)
    ));
    assert_eq!(writer.head().unwrap().generation, 1);
    let source = b"scenario \"Main\" {}\n";
    let (_, snapshot) = writer.load().unwrap();
    let mut manifest = crate::project::tests::manifest();
    manifest.documents.clear();
    let changes = vec![
        ProjectChange::RemoveFile {
            path: path("scenarios/main.afs"),
            expected: FileState::of(source),
        },
        ProjectChange::RemoveDirectory {
            path: path("scenarios"),
        },
        write(
            MANIFEST_PATH,
            snapshot.state(&path(MANIFEST_PATH)).unwrap(),
            &manifest.to_public_bytes().unwrap(),
        ),
    ];
    let mut applied = 0;
    writer
        .commit_with_hook(changes, |phase| {
            if phase == "operation" {
                applied += 1;
                if applied == 2 {
                    return Err(interrupted());
                }
            }
            Ok(())
        })
        .unwrap_err();
    assert!(!directory.path().join("scenarios").exists());
    drop(writer);
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    assert_eq!(writer.recover().unwrap(), Some(2));
    assert!(writer.load().unwrap().0.documents.is_empty());
}

#[test]
fn unactivated_staging_is_removed_after_a_failure_and_when_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    let staging = directory.path().join(".apiforge/local/transactions");
    let source = directory.path().join("scenarios/main.afs");
    let result = writer.commit_with_hook(
        vec![write(
            "scenarios/main.afs",
            FileState::of(b"scenario \"Main\" {}\n"),
            b"scenario \"Saved\" {}\n",
        )],
        |phase| {
            if phase == "descriptor" {
                fs::write(&source, b"scenario \"Foreign\" {}\n").unwrap();
            }
            Ok(())
        },
    );
    assert!(matches!(result, Err(ProjectError::Conflict { .. })));
    assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
    assert_eq!(fs::read(&source).unwrap(), b"scenario \"Foreign\" {}\n");

    // A writer that crashed before activation leaves staging nobody references.
    let orphan = staging.join("0b7c2f0e-6f5f-4d5b-9a3e-2b1c0d9e8f7a");
    fs::create_dir(&orphan).unwrap();
    fs::write(orphan.join("0"), b"payload").unwrap();
    drop(writer);
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
    assert_eq!(writer.load().unwrap().0, crate::project::tests::manifest());
}
