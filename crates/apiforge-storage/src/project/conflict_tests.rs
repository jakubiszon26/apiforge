use super::*;
use crate::project::tests::{initialise, path, write};
use std::{fs, io};

const SOURCE: &[u8] = b"scenario \"Main\" {}\n";
const TARGET: &[u8] = b"scenario \"Saved\" {}\n";
const FOREIGN: &[u8] = b"scenario \"Else\" {}\n";

fn interrupted() -> ProjectError {
    ProjectError::io(
        Path::new("fault-injection"),
        io::Error::other("simulated stop"),
    )
}

fn activate(store: &ProjectStore, changes: Vec<ProjectChange>) {
    let _worker = store.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
    store
        .commit_locked(changes, |phase| {
            if phase == "activation" {
                Err(interrupted())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
}

fn setup() -> (tempfile::TempDir, ProjectStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    activate(
        &store,
        vec![
            write("scenarios/main.afs", FileState::of(SOURCE), TARGET),
            write(
                ".apiforge/diagram.json",
                FileState::Missing,
                b"{\"formatVersion\":1,\"records\":[]}",
            ),
        ],
    );
    fs::write(directory.path().join("scenarios/main.afs"), FOREIGN).unwrap();
    (directory, store)
}

fn preview(store: &ProjectStore) -> TransactionRecoveryPreview {
    store.preview_transaction_recovery().unwrap().unwrap()
}

fn stop_after_approval(store: &ProjectStore) -> TransactionRecoveryPreview {
    let candidate = preview(store);
    store
        .finish_with_hook(&candidate, |phase| {
            if phase == "approval" {
                Err(interrupted())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    preview(store)
}

fn archive_indexes(root: &Path) -> Vec<PathBuf> {
    fn collect(root: &Path, paths: &mut Vec<PathBuf>) {
        if !root.is_dir() {
            return;
        }
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, paths);
            } else if path.file_name().unwrap() == "index.json" {
                paths.push(path);
            }
        }
    }
    let mut paths = Vec::new();
    collect(&root.join(ARCHIVES), &mut paths);
    paths
}

#[test]
fn read_only_preview_exposes_both_candidates_without_applying_or_approving_them() {
    let (directory, store) = setup();
    assert!(matches!(
        store.recover(),
        Err(ProjectError::Conflict { .. })
    ));
    let reader = ProjectStore::open(directory.path()).unwrap();
    let preview = preview(&reader);
    assert_eq!(preview.generation(), 1);
    assert_eq!(preview.candidates().len(), 2);
    let candidate = &preview.candidates()[0];
    assert_eq!(candidate.path().as_str(), "scenarios/main.afs");
    assert_eq!(candidate.expected(), &FileState::of(SOURCE));
    assert_eq!(candidate.target(), &FileState::of(TARGET));
    assert_eq!(candidate.current(), Some(&FileState::of(FOREIGN)));
    assert_eq!(candidate.current_bytes(), Some(FOREIGN));
    assert_eq!(candidate.intended_bytes(), Some(TARGET));
    assert_eq!(candidate.status(), RecoveryCandidateStatus::Conflict);
    assert_eq!(
        preview.candidates()[1].status(),
        RecoveryCandidateStatus::Expected
    );
    assert!(preview.archives().is_empty());
    assert!(matches!(
        reader.finish_transaction_recovery(&preview),
        Err(ProjectError::ReadOnly)
    ));
    assert!(!directory.path().join(ARCHIVES).exists());
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        FOREIGN
    );
    assert!(matches!(reader.load(), Err(ProjectError::RecoveryRequired)));
}

#[test]
fn finish_preserves_foreign_bytes_and_original_intent_then_commits_the_complete_generation() {
    let (directory, store) = setup();
    let active = store.read_active_transaction().unwrap().unwrap();
    let original = active.descriptor_bytes.clone();
    let candidate = preview(&store);
    let completed = store.finish_transaction_recovery(&candidate).unwrap();
    assert_eq!(completed.generation, 2);
    assert_eq!(completed.archives.len(), 1);
    let archive = &completed.archives[0];
    assert_eq!(archive.path.as_str(), "scenarios/main.afs");
    assert_eq!(archive.accepted, FileState::of(FOREIGN));
    let copy = archive.copy_path.as_ref().unwrap();
    assert_eq!(
        fs::read(directory.path().join(copy.as_str())).unwrap(),
        FOREIGN
    );
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        TARGET
    );
    assert_eq!(store.load().unwrap().1.generation, 2);
    assert!(store.preview_transaction_recovery().unwrap().is_none());
    assert!(matches!(
        store.finish_transaction_recovery(&candidate),
        Err(ProjectError::NoActiveTransaction)
    ));
    assert!(!active.directory.exists());
    let indexes = archive_indexes(directory.path());
    assert_eq!(indexes.len(), 1);
    let index = decode_resolution(&fs::read(&indexes[0]).unwrap()).unwrap();
    assert_eq!(index.descriptor_hash, transaction::hash(&original));
    assert_eq!(index.entries, completed.archives);
    assert!(matches!(
        store.commit(vec![ProjectChange::RemoveFile {
            path: copy.clone(),
            expected: FileState::of(FOREIGN)
        }]),
        Err(ProjectError::InvalidTransaction)
    ));
    assert_eq!(
        fs::read(directory.path().join(copy.as_str())).unwrap(),
        FOREIGN
    );
}

#[test]
fn changed_source_descriptor_payload_or_receipt_invalidates_the_preview() {
    for target in ["source", "descriptor", "payload", "receipt"] {
        let (directory, store) = setup();
        let candidate = preview(&store);
        let active = store.read_active_transaction().unwrap().unwrap();
        match target {
            "source" => fs::write(
                directory.path().join("scenarios/main.afs"),
                b"scenario \"Next\" {}\n",
            )
            .unwrap(),
            "descriptor" => {
                let mut bytes = active.descriptor_bytes.clone();
                bytes.push(b'\n');
                fs::write(active.directory.join("transaction.json"), bytes).unwrap();
            }
            "payload" => fs::write(active.directory.join("0"), b"tampered staged payload").unwrap(),
            "receipt" => {
                fs::write(active.directory.join(RECEIPT), b"{\"formatVersion\":9}").unwrap()
            }
            _ => unreachable!(),
        }
        assert!(
            store.finish_transaction_recovery(&candidate).is_err(),
            "{target}"
        );
        assert!(!directory.path().join(ARCHIVES).exists());
        assert!(!directory.path().join(".apiforge/diagram.json").exists());
        assert!(store.load().is_err());
    }
}

#[test]
fn deletion_approval_records_absence_and_can_recreate_the_intended_file() {
    let (directory, store) = setup();
    fs::remove_file(directory.path().join("scenarios/main.afs")).unwrap();
    let candidate = preview(&store);
    assert_eq!(
        candidate.candidates()[0].current(),
        Some(&FileState::Missing)
    );
    let completed = store.finish_transaction_recovery(&candidate).unwrap();
    assert_eq!(completed.archives[0].accepted, FileState::Missing);
    assert!(completed.archives[0].copy_path.is_none());
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        TARGET
    );
    assert_eq!(archive_indexes(directory.path()).len(), 1);
}

#[test]
fn finishing_a_file_removal_preserves_the_foreign_contents_before_removing_it() {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    activate(
        &store,
        vec![ProjectChange::RemoveFile {
            path: path("scenarios/main.afs"),
            expected: FileState::of(SOURCE),
        }],
    );
    fs::write(directory.path().join("scenarios/main.afs"), FOREIGN).unwrap();
    let completed = store.finish_transaction_recovery(&preview(&store)).unwrap();
    assert!(!directory.path().join("scenarios/main.afs").exists());
    assert_eq!(
        fs::read(
            directory
                .path()
                .join(completed.archives[0].copy_path.as_ref().unwrap().as_str())
        )
        .unwrap(),
        FOREIGN
    );
}

#[test]
fn a_foreign_directory_is_never_recursively_removed_by_file_conflict_recovery() {
    let (directory, store) = setup();
    let source = directory.path().join("scenarios/main.afs");
    fs::remove_file(&source).unwrap();
    fs::create_dir(&source).unwrap();
    fs::write(source.join("foreign.txt"), b"important").unwrap();
    let candidate = preview(&store);
    assert_eq!(
        candidate.candidates()[0].current(),
        Some(&FileState::Directory)
    );
    assert!(matches!(
        store.finish_transaction_recovery(&candidate),
        Err(ProjectError::UnsupportedRecoveryConflict)
    ));
    assert_eq!(fs::read(source.join("foreign.txt")).unwrap(), b"important");
    assert!(!directory.path().join(ARCHIVES).exists());
}

#[test]
fn oversized_or_linked_targets_remain_unavailable_but_intended_bytes_can_be_exported() {
    let (directory, store) = setup();
    let source = directory.path().join("scenarios/main.afs");
    fs::write(&source, vec![b'x'; MAX_SOURCE_BYTES + 1]).unwrap();
    let candidate = preview(&store);
    assert_eq!(
        candidate.candidates()[0].status(),
        RecoveryCandidateStatus::Unavailable
    );
    assert_eq!(candidate.candidates()[0].intended_bytes(), Some(TARGET));
    assert!(matches!(
        store.finish_transaction_recovery(&candidate),
        Err(ProjectError::UnsupportedRecoveryConflict)
    ));
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        let original = outside.path().join("original.afs");
        fs::write(&original, b"outside bytes").unwrap();
        fs::remove_file(&source).unwrap();
        std::os::unix::fs::symlink(&original, &source).unwrap();
        let candidate = preview(&store);
        assert_eq!(
            candidate.candidates()[0].status(),
            RecoveryCandidateStatus::Unavailable
        );
        assert!(matches!(
            store.finish_transaction_recovery(&candidate),
            Err(ProjectError::UnsupportedRecoveryConflict)
        ));
        assert_eq!(fs::read(original).unwrap(), b"outside bytes");
    }
    assert!(!directory.path().join(ARCHIVES).exists());
}

#[test]
fn secret_checks_apply_to_foreign_copies_and_replayed_payloads_without_hiding_read_only_candidates()
{
    let (directory, store) = setup();
    let secret = b"runtime-canary-credential";
    fs::write(directory.path().join("scenarios/main.afs"), secret).unwrap();
    store.set_known_secrets([String::from_utf8(secret.to_vec()).unwrap()]);
    let candidate = preview(&store);
    assert_eq!(
        candidate.candidates()[0].current_bytes(),
        Some(secret.as_slice())
    );
    let error = store.finish_transaction_recovery(&candidate).unwrap_err();
    assert!(matches!(error, ProjectError::Secret));
    assert!(!error.to_string().contains("runtime-canary"));
    assert!(!directory.path().join(ARCHIVES).exists());
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    activate(
        &store,
        vec![write("scenarios/main.afs", FileState::of(SOURCE), secret)],
    );
    fs::write(directory.path().join("scenarios/main.afs"), FOREIGN).unwrap();
    store.set_known_secrets([String::from_utf8(secret.to_vec()).unwrap()]);
    let candidate = preview(&store);
    assert_eq!(
        candidate.candidates()[0].intended_bytes(),
        Some(secret.as_slice())
    );
    assert!(matches!(
        store.finish_transaction_recovery(&candidate),
        Err(ProjectError::Secret)
    ));
    assert!(matches!(store.recover(), Err(ProjectError::Secret)));
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        FOREIGN
    );
}

#[test]
fn multiple_conflicting_targets_are_preserved_in_one_bound_approval() {
    let (directory, store) = setup();
    fs::write(
        directory.path().join(".apiforge/diagram.json"),
        b"foreign checkpoint bytes",
    )
    .unwrap();
    let completed = store.finish_transaction_recovery(&preview(&store)).unwrap();
    assert_eq!(completed.archives.len(), 2);
    assert_eq!(
        completed.archives[0].approval_id,
        completed.archives[1].approval_id
    );
    assert_eq!(
        fs::read(
            directory
                .path()
                .join(completed.archives[1].copy_path.as_ref().unwrap().as_str())
        )
        .unwrap(),
        b"foreign checkpoint bytes"
    );
    assert_eq!(archive_indexes(directory.path()).len(), 1);
    store.load().unwrap();
}

#[test]
fn a_later_foreign_edit_requires_another_decision_and_preserves_both_versions() {
    let (directory, store) = setup();
    let approved = stop_after_approval(&store);
    assert_eq!(
        approved.candidates()[0].status(),
        RecoveryCandidateStatus::Approved
    );
    let first = approved.archives()[0].copy_path.clone().unwrap();
    let newer = b"another external version\r\n";
    fs::write(directory.path().join("scenarios/main.afs"), newer).unwrap();
    assert!(matches!(
        store.recover(),
        Err(ProjectError::Conflict { .. })
    ));
    assert!(matches!(
        store.finish_transaction_recovery(&approved),
        Err(ProjectError::Conflict { .. })
    ));
    let candidate = preview(&store);
    assert_eq!(
        candidate.candidates()[0].status(),
        RecoveryCandidateStatus::Conflict
    );
    let completed = store.finish_transaction_recovery(&candidate).unwrap();
    assert_eq!(completed.archives.len(), 2);
    assert_eq!(
        fs::read(directory.path().join(first.as_str())).unwrap(),
        FOREIGN
    );
    assert_eq!(
        fs::read(
            directory
                .path()
                .join(completed.archives[1].copy_path.as_ref().unwrap().as_str())
        )
        .unwrap(),
        newer
    );
    assert_eq!(archive_indexes(directory.path()).len(), 2);
}

#[test]
fn damaged_or_missing_preservation_evidence_never_authorises_an_overwrite() {
    for damage in [
        "copy",
        "missing-copy",
        "index",
        "receipt",
        "future-receipt",
        "binding",
    ] {
        let (directory, store) = setup();
        let approved = stop_after_approval(&store);
        let active = store.read_active_transaction().unwrap().unwrap();
        let copy = directory
            .path()
            .join(approved.archives()[0].copy_path.as_ref().unwrap().as_str());
        match damage {
            "copy" => fs::write(&copy, b"tampered").unwrap(),
            "missing-copy" => fs::remove_file(&copy).unwrap(),
            "index" => fs::write(copy.parent().unwrap().join("index.json"), b"corrupt").unwrap(),
            "receipt" => fs::write(active.directory.join(RECEIPT), b"corrupt").unwrap(),
            "future-receipt" => {
                fs::write(active.directory.join(RECEIPT), b"{\"formatVersion\":2}").unwrap()
            }
            "binding" => {
                let mut log =
                    decode_resolution(&fs::read(active.directory.join(RECEIPT)).unwrap()).unwrap();
                log.descriptor_hash = "0".repeat(64);
                fs::write(
                    active.directory.join(RECEIPT),
                    super::super::codec::encode(&log, MAX_JSON_BYTES).unwrap(),
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(store.recover().is_err(), "{damage}");
        assert_eq!(
            fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
            FOREIGN
        );
        assert_eq!(fs::read(active.directory.join("0")).unwrap(), TARGET);
        assert!(matches!(store.load(), Err(ProjectError::RecoveryRequired)));
    }
}

#[test]
fn foreign_copy_name_collision_and_archive_directory_failure_preserve_the_original() {
    let (directory, store) = setup();
    fs::write(directory.path().join(ARCHIVES), b"occupied").unwrap();
    assert!(store.finish_transaction_recovery(&preview(&store)).is_err());
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        FOREIGN
    );
    fs::remove_file(directory.path().join(ARCHIVES)).unwrap();
    let candidate = preview(&store);
    let mut occupied = None;
    let result = store.finish_with_hook(&candidate, |phase| {
        if phase == "attempt-directory" {
            let tx = directory
                .path()
                .join(ARCHIVES)
                .join(candidate.transaction_id());
            let attempt = fs::read_dir(tx).unwrap().next().unwrap().unwrap().path();
            let path = attempt.join("0.bin");
            fs::write(&path, b"foreign archive file").unwrap();
            occupied = Some(path);
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(occupied.unwrap()).unwrap(),
        b"foreign archive file"
    );
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        FOREIGN
    );
    assert!(matches!(
        store.recover(),
        Err(ProjectError::Conflict { .. })
    ));
}

#[test]
fn source_changes_during_preservation_leave_only_unreferenced_copies_and_need_new_consent() {
    let (directory, store) = setup();
    let candidate = preview(&store);
    let result = store.finish_with_hook(&candidate, |phase| {
        if phase == "conflict-copy" {
            fs::write(
                directory.path().join("scenarios/main.afs"),
                b"new foreign content",
            )
            .unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(ProjectError::Conflict { .. })));
    assert!(preview(&store).archives().is_empty());
    assert_eq!(
        archive_indexes(directory.path()).len(),
        1,
        "unreferenced archive is preserved, not replayed"
    );
    assert!(matches!(
        store.recover(),
        Err(ProjectError::Conflict { .. })
    ));
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"new foreign content"
    );
    store.finish_transaction_recovery(&preview(&store)).unwrap();
    assert_eq!(archive_indexes(directory.path()).len(), 2);
}

#[test]
fn restart_at_every_finish_boundary_replays_only_after_durable_approval() {
    let (_directory, store) = setup();
    let mut trace = Vec::new();
    store
        .finish_with_hook(&preview(&store), |phase| {
            trace.push(phase.to_owned());
            Ok(())
        })
        .unwrap();
    for (stop, label) in trace.iter().enumerate() {
        let (directory, store) = setup();
        let active = store.read_active_transaction().unwrap().unwrap();
        let descriptor = active.descriptor_bytes.clone();
        let mut count = 0;
        let mut approved = false;
        let result = store.finish_with_hook(&preview(&store), |phase| {
            approved |= phase == "approval";
            let current = count;
            count += 1;
            if current == stop {
                Err(interrupted())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err(), "{stop}: {label}");
        drop(store);
        let store = ProjectStore::open_exclusive(directory.path()).unwrap();
        if approved {
            store.recover().unwrap();
            assert_eq!(
                fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
                TARGET
            );
            assert_eq!(store.load().unwrap().1.generation, 2);
            for index in archive_indexes(directory.path()) {
                let index = decode_resolution(&fs::read(index).unwrap()).unwrap();
                for entry in index.entries {
                    assert_eq!(
                        fs::read(directory.path().join(entry.copy_path.unwrap().as_str())).unwrap(),
                        FOREIGN
                    );
                }
            }
        } else {
            assert!(matches!(
                store.recover(),
                Err(ProjectError::Conflict { .. })
            ));
            assert_eq!(
                fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
                FOREIGN
            );
        }
        if active.directory.exists() {
            assert_eq!(
                fs::read(active.directory.join("transaction.json")).unwrap(),
                descriptor
            );
        }
    }
}

#[test]
fn resumed_approved_replay_can_be_interrupted_again_after_each_operation() {
    for stop in 0..3 {
        let (directory, store) = setup();
        stop_after_approval(&store);
        {
            let _worker = store.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
            let active = store.read_active_transaction().unwrap().unwrap();
            let approvals = store.read_conflict_approvals(&active).unwrap();
            store.make_approvals_durable(&active, &approvals).unwrap();
            let mut count = 0;
            store
                .replay(
                    &active.descriptor,
                    &active.payloads,
                    &approvals.accepted_states(),
                    &mut |_| {
                        let current = count;
                        count += 1;
                        if current == stop {
                            Err(interrupted())
                        } else {
                            Ok(())
                        }
                    },
                )
                .unwrap_err();
        }
        drop(store);
        let store = ProjectStore::open_exclusive(directory.path()).unwrap();
        store.recover().unwrap();
        assert_eq!(store.load().unwrap().1.generation, 2);
        assert_eq!(
            fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
            TARGET
        );
        assert_eq!(archive_indexes(directory.path()).len(), 1);
    }
}

#[test]
fn a_foreign_project_preview_never_authorises_another_store() {
    let (_first, first) = setup();
    let candidate = preview(&first);
    let (second, store) = setup();
    assert!(matches!(
        store.finish_transaction_recovery(&candidate),
        Err(ProjectError::NoActiveTransaction)
    ));
    assert!(!second.path().join(ARCHIVES).exists());
    assert_eq!(
        fs::read(second.path().join("scenarios/main.afs")).unwrap(),
        FOREIGN
    );
}

#[test]
fn existing_published_revisions_cannot_be_changed_but_new_revision_conflicts_keep_original_intent()
{
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    let target = ".apiforge/revisions/new-component/1.afs";
    activate(
        &store,
        vec![
            ProjectChange::CreateDirectory {
                path: path(".apiforge/revisions"),
            },
            ProjectChange::CreateDirectory {
                path: path(".apiforge/revisions/new-component"),
            },
            write(target, FileState::Missing, b"component New() {}\n"),
        ],
    );
    fs::create_dir_all(directory.path().join(".apiforge/revisions/new-component")).unwrap();
    fs::write(
        directory.path().join(target),
        b"foreign new-revision source",
    )
    .unwrap();
    let active = store.read_active_transaction().unwrap().unwrap();
    let candidate = preview(&store);
    assert_eq!(candidate.candidates()[2].expected(), &FileState::Missing);
    let completed = store.finish_transaction_recovery(&candidate).unwrap();
    assert_eq!(
        fs::read(directory.path().join(target)).unwrap(),
        b"component New() {}\n"
    );
    assert_eq!(
        fs::read(
            directory
                .path()
                .join(completed.archives[0].copy_path.as_ref().unwrap().as_str())
        )
        .unwrap(),
        b"foreign new-revision source"
    );
    assert!(matches!(
        store.commit(vec![write(
            target,
            FileState::of(b"component New() {}\n"),
            b"replacement"
        )]),
        Err(ProjectError::ImmutableRevision)
    ));
    assert!(!active.directory.exists());
}

#[test]
fn receipt_count_and_copy_byte_limits_are_checked_before_any_replay() {
    for count_limit in [false, true] {
        let (directory, store) = setup();
        let active = store.read_active_transaction().unwrap().unwrap();
        let mut log = ResolutionFile {
            format_version: 1,
            transaction_id: active.descriptor.transaction_id.clone(),
            descriptor_hash: transaction::hash(&active.descriptor_bytes),
            entries: Vec::new(),
        };
        let count = if count_limit {
            MAX_TRANSACTION_OPERATIONS + 1
        } else {
            MAX_TRANSACTION_BYTES / MAX_SOURCE_BYTES + 1
        };
        for _ in 0..count {
            let id = uuid::Uuid::new_v4().to_string();
            log.entries.push(ConflictArchive {
                approval_id: id.clone(),
                operation_index: 0,
                path: path("scenarios/main.afs"),
                accepted: FileState::File {
                    bytes: if count_limit {
                        1
                    } else {
                        MAX_SOURCE_BYTES as u64
                    },
                    sha256: "a".repeat(64),
                },
                copy_path: Some(archive_path(&active, &id, "0.bin").unwrap()),
            });
        }
        fs::write(
            active.directory.join(RECEIPT),
            super::super::codec::encode(&log, MAX_JSON_BYTES).unwrap(),
        )
        .unwrap();
        assert!(matches!(store.recover(), Err(ProjectError::Limit)));
        assert_eq!(
            fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
            FOREIGN
        );
    }
}

#[test]
fn changes_to_staging_or_preservation_evidence_after_approval_stop_before_any_target_write() {
    for phase in ["approval", "proofs-durable"] {
        for target in ["descriptor", "payload", "copy", "receipt"] {
            let (directory, store) = setup();
            let active = store.read_active_transaction().unwrap().unwrap();
            let result = store.finish_with_hook(&preview(&store), |boundary| {
                if boundary == phase {
                    match target {
                        "descriptor" => {
                            let mut bytes = active.descriptor_bytes.clone();
                            bytes.push(b'\n');
                            fs::write(active.directory.join("transaction.json"), bytes).unwrap();
                        }
                        "payload" => {
                            fs::write(active.directory.join("0"), b"new staged bytes").unwrap()
                        }
                        "copy" => {
                            let index = decode_resolution(
                                &fs::read(&archive_indexes(directory.path())[0]).unwrap(),
                            )
                            .unwrap();
                            fs::write(
                                directory
                                    .path()
                                    .join(index.entries[0].copy_path.as_ref().unwrap().as_str()),
                                b"new archive bytes",
                            )
                            .unwrap();
                        }
                        "receipt" => {
                            fs::write(active.directory.join(RECEIPT), b"corrupt approval").unwrap()
                        }
                        _ => unreachable!(),
                    }
                }
                Ok(())
            });
            assert!(result.is_err(), "{phase}: {target}");
            assert_eq!(
                fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
                FOREIGN
            );
            assert!(!directory.path().join(".apiforge/diagram.json").exists());
            assert!(active.directory.exists());
        }
    }
}

#[test]
fn a_corrupt_foreign_manifest_can_be_preserved_and_replaced_only_by_its_previewed_valid_intent() {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    let (mut manifest, snapshot) = store.load().unwrap();
    manifest.name = "Intended project name".into();
    let target = manifest.to_public_bytes().unwrap();
    activate(
        &store,
        vec![write(
            MANIFEST_PATH,
            snapshot.state(&path(MANIFEST_PATH)).unwrap(),
            &target,
        )],
    );
    let foreign = b"{corrupt externally edited manifest";
    fs::write(directory.path().join(MANIFEST_PATH), foreign).unwrap();
    assert!(matches!(
        store.recover(),
        Err(ProjectError::Conflict { .. })
    ));
    let candidate = preview(&store);
    assert_eq!(
        candidate.candidates()[0].current_bytes(),
        Some(foreign.as_slice())
    );
    let completed = store.finish_transaction_recovery(&candidate).unwrap();
    assert_eq!(store.load().unwrap().0, manifest);
    assert_eq!(
        fs::read(
            directory
                .path()
                .join(completed.archives[0].copy_path.as_ref().unwrap().as_str())
        )
        .unwrap(),
        foreign
    );
}
