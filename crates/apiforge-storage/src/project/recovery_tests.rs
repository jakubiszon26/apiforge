use super::*;
use crate::project::tests::{initialise, manifest, path, write};
use std::{fs, io, path::Path};

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
            base_hash: match FileState::of(b"scenario \"Main\" {}\n") {
                FileState::File { sha256, .. } => Some(sha256),
                _ => unreachable!(),
            },
            source_version: 7,
            text: "// żółw\r\nscenario \"unfinished\" {\r\n".into(),
            selection: TextSelection {
                anchor: 3,
                caret: 5,
            },
            fields: vec![FieldRecovery {
                widget: 42,
                base_hash: "a".repeat(64),
                text: "{{env.unfinished".into(),
                selection: TextSelection {
                    anchor: 4,
                    caret: 16,
                },
            }],
            checkpoint: None,
        }],
        texts: Vec::new(),
    }
}

fn setup() -> (tempfile::TempDir, ProjectStore, RecoveryRead) {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    let preview = store.load_source_recovery(&manifest().id).unwrap();
    (directory, store, preview)
}

fn interrupted() -> ProjectError {
    ProjectError::io(
        Path::new("fault-injection"),
        io::Error::other("simulated crash"),
    )
}

#[test]
fn restart_retains_exact_invalid_text_field_drafts_versions_and_selections_without_saving_source() {
    let (directory, store, preview) = setup();
    assert!(preview.recovery().is_none());
    let recovery = recovery();
    let next = store.save_source_recovery(&preview, &recovery).unwrap();
    assert_eq!(next.recovery(), Some(&recovery));
    assert!(!directory.path().join(BACKUP_PATH).exists());
    drop(store);
    let store = ProjectStore::open_read_only(directory.path()).unwrap();
    let loaded = store.load_source_recovery(&manifest().id).unwrap();
    assert_eq!(loaded.recovery(), Some(&recovery));
    assert!(!loaded.is_backup_candidate());
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"scenario \"Main\" {}\n"
    );
    assert_eq!(
        store.read(&[]).unwrap().generation,
        1,
        "local recovery does not commit shared definitions"
    );
}

#[test]
fn backups_rotate_only_checked_primary_and_restore_requires_an_unchanged_preview() {
    let (directory, store, preview) = setup();
    let first = recovery();
    let saved = store.save_source_recovery(&preview, &first).unwrap();
    let mut second = first.clone();
    second.documents[0].text.push_str("next edit");
    store.save_source_recovery(&saved, &second).unwrap();
    assert_eq!(
        SourceRecovery::from_bytes(&fs::read(directory.path().join(BACKUP_PATH)).unwrap()).unwrap(),
        first
    );
    fs::write(directory.path().join(RECOVERY_PATH), b"{truncated").unwrap();
    let candidate = store.load_source_recovery(&first.project_id).unwrap();
    assert!(candidate.is_backup_candidate());
    assert_eq!(candidate.recovery(), Some(&first));
    assert!(matches!(
        store.save_source_recovery(&candidate, &second),
        Err(ProjectError::RecoveryDecisionRequired)
    ));
    assert_eq!(
        fs::read(directory.path().join(RECOVERY_PATH)).unwrap(),
        b"{truncated"
    );
    let restored = store.restore_source_recovery_backup(&candidate).unwrap();
    assert!(!restored.is_backup_candidate());
    assert_eq!(restored.recovery(), Some(&first));
    assert_eq!(
        SourceRecovery::from_bytes(&fs::read(directory.path().join(BACKUP_PATH)).unwrap()).unwrap(),
        first
    );
    assert!(matches!(
        store.restore_source_recovery_backup(&restored),
        Err(ProjectError::NoRecoveryCandidate)
    ));
}

#[test]
fn missing_primary_offers_backup_and_foreign_or_future_primary_never_falls_back() {
    let (directory, store, preview) = setup();
    let recovery = recovery();
    let saved = store.save_source_recovery(&preview, &recovery).unwrap();
    store.save_source_recovery(&saved, &recovery).unwrap();
    fs::remove_file(directory.path().join(RECOVERY_PATH)).unwrap();
    let candidate = store.load_source_recovery(&recovery.project_id).unwrap();
    assert!(candidate.is_backup_candidate());
    store.restore_source_recovery_backup(&candidate).unwrap();
    for (bytes, future) in [
        (b"{\"formatVersion\":2,\"future\":true}".as_slice(), true),
        (
            &{
                let mut foreign = recovery.clone();
                foreign.project_id = "another-project".into();
                foreign.to_bytes().unwrap()
            },
            false,
        ),
    ] {
        fs::write(directory.path().join(RECOVERY_PATH), bytes).unwrap();
        let error = store
            .load_source_recovery(&recovery.project_id)
            .err()
            .unwrap();
        assert!(if future {
            matches!(error, ProjectError::UnsupportedVersion)
        } else {
            matches!(error, ProjectError::ForeignRecovery)
        });
        assert_eq!(
            fs::read(directory.path().join(RECOVERY_PATH)).unwrap(),
            bytes
        );
    }
}

#[test]
fn primary_backup_and_manifest_changes_invalidate_the_save_and_restore_preview() {
    for target in [RECOVERY_PATH, BACKUP_PATH, MANIFEST_PATH] {
        let (directory, store, preview) = setup();
        let recovery = recovery();
        let saved = store.save_source_recovery(&preview, &recovery).unwrap();
        store.save_source_recovery(&saved, &recovery).unwrap();
        let saved = store.load_source_recovery(&recovery.project_id).unwrap();
        fs::write(directory.path().join(target), b"foreign").unwrap();
        assert!(matches!(
            store.save_source_recovery(&saved, &recovery),
            Err(ProjectError::Conflict { .. })
        ));
        assert_eq!(fs::read(directory.path().join(target)).unwrap(), b"foreign");
    }
    for target in [RECOVERY_PATH, BACKUP_PATH, MANIFEST_PATH] {
        let (directory, store, preview) = setup();
        let recovery = recovery();
        let saved = store.save_source_recovery(&preview, &recovery).unwrap();
        store.save_source_recovery(&saved, &recovery).unwrap();
        fs::write(directory.path().join(RECOVERY_PATH), b"corrupt").unwrap();
        let candidate = store.load_source_recovery(&recovery.project_id).unwrap();
        fs::write(directory.path().join(target), b"foreign").unwrap();
        assert!(matches!(
            store.restore_source_recovery_backup(&candidate),
            Err(ProjectError::Conflict { .. })
        ));
        assert_eq!(fs::read(directory.path().join(target)).unwrap(), b"foreign");
    }
}

#[test]
fn a_read_only_session_can_preview_but_never_claim_durable_recovery() {
    let (directory, store, preview) = setup();
    let recovery = recovery();
    let saved = store.save_source_recovery(&preview, &recovery).unwrap();
    store.save_source_recovery(&saved, &recovery).unwrap();
    fs::write(directory.path().join(RECOVERY_PATH), b"corrupt").unwrap();
    let reader = ProjectStore::open(directory.path()).unwrap();
    let preview = reader.load_source_recovery(&recovery.project_id).unwrap();
    assert!(preview.is_backup_candidate());
    assert!(matches!(
        reader.save_source_recovery(&preview, &recovery),
        Err(ProjectError::ReadOnly)
    ));
    assert!(matches!(
        reader.restore_source_recovery_backup(&preview),
        Err(ProjectError::ReadOnly)
    ));
    assert_eq!(
        fs::read(directory.path().join(RECOVERY_PATH)).unwrap(),
        b"corrupt"
    );
}

#[test]
fn unknown_or_foreign_backup_is_preserved_even_when_primary_is_valid() {
    for future in [false, true] {
        let (directory, store, preview) = setup();
        let recovery = recovery();
        let saved = store.save_source_recovery(&preview, &recovery).unwrap();
        let bytes = if future {
            b"{\"formatVersion\":3}".to_vec()
        } else {
            let mut foreign = recovery.clone();
            foreign.project_id = "foreign".into();
            foreign.to_bytes().unwrap()
        };
        fs::write(directory.path().join(BACKUP_PATH), &bytes).unwrap();
        let preview = store.load_source_recovery(&recovery.project_id).unwrap();
        let error = store
            .save_source_recovery(&preview, &recovery)
            .err()
            .unwrap();
        assert!(if future {
            matches!(error, ProjectError::UnsupportedVersion)
        } else {
            matches!(error, ProjectError::ForeignRecovery)
        });
        assert_eq!(fs::read(directory.path().join(BACKUP_PATH)).unwrap(), bytes);
        assert!(saved.recovery().is_some());
    }
}

#[test]
fn failures_at_each_backup_boundary_leave_complete_recoverable_candidates() {
    for phase in ["backup", "before-primary", "primary"] {
        let (directory, store, preview) = setup();
        let first = recovery();
        let saved = store.save_source_recovery(&preview, &first).unwrap();
        let mut next = first.clone();
        next.documents[0].text.push_str("latest edit");
        let result = store.save_recovery_with_hook(&saved, &next, |boundary| {
            if boundary == phase {
                Err(interrupted())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        drop(store);
        let reopened = ProjectStore::open_read_only(directory.path()).unwrap();
        assert_eq!(
            reopened
                .load_source_recovery(&first.project_id)
                .unwrap()
                .recovery(),
            Some(if phase == "primary" { &next } else { &first })
        );
        assert_eq!(
            SourceRecovery::from_bytes(&fs::read(directory.path().join(BACKUP_PATH)).unwrap())
                .unwrap(),
            first
        );
    }
}

#[test]
fn backup_io_failure_and_external_edits_during_rotation_do_not_overwrite_primary() {
    let (directory, store, preview) = setup();
    let first = recovery();
    let saved = store.save_source_recovery(&preview, &first).unwrap();
    fs::create_dir(directory.path().join(BACKUP_PATH)).unwrap();
    assert!(store.save_source_recovery(&saved, &first).is_err());
    assert_eq!(
        SourceRecovery::from_bytes(&fs::read(directory.path().join(RECOVERY_PATH)).unwrap())
            .unwrap(),
        first
    );
    for target in [RECOVERY_PATH, BACKUP_PATH] {
        let (directory, store, preview) = setup();
        let saved = store.save_source_recovery(&preview, &first).unwrap();
        let result = store.save_recovery_with_hook(&saved, &first, |phase| {
            if phase == "backup" {
                fs::write(directory.path().join(target), b"foreign").unwrap();
            }
            Ok(())
        });
        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
        assert_eq!(fs::read(directory.path().join(target)).unwrap(), b"foreign");
    }
}

#[test]
fn recovery_cannot_bypass_transaction_recovery_or_be_written_as_an_unchecked_transaction() {
    let (directory, store, preview) = setup();
    assert!(matches!(
        store.commit(vec![write(
            RECOVERY_PATH,
            FileState::Missing,
            &recovery().to_bytes().unwrap()
        )]),
        Err(ProjectError::InvalidTransaction)
    ));
    fs::write(
        directory.path().join(".apiforge/local/head.json"),
        b"{\"formatVersion\":1,\"generation\":1,\"activeTransactionId\":\"pending\"}",
    )
    .unwrap();
    assert!(matches!(
        store.load_source_recovery(&manifest().id),
        Err(ProjectError::RecoveryRequired)
    ));
    assert!(matches!(
        store.save_source_recovery(&preview, &recovery()),
        Err(ProjectError::RecoveryRequired)
    ));
    assert!(!directory.path().join(RECOVERY_PATH).exists());
}

#[test]
fn stale_source_baselines_and_older_component_context_survive_without_automatic_application() {
    let mut recovery = recovery();
    let document = &mut recovery.documents[0];
    assert!(document.matches_base(&FileState::of(b"scenario \"Main\" {}\n")));
    assert!(!document.matches_base(&FileState::of(b"scenario \"Main\" {}\r\n")));
    assert!(!document.matches_base(&FileState::Missing));
    document.base_hash = None;
    assert!(document.matches_base(&FileState::Missing));
    document.kind = DocumentKind::Component;
    document.base_revision = Some(1);
    let bytes = recovery.to_bytes().unwrap();
    assert_eq!(SourceRecovery::from_bytes(&bytes).unwrap(), recovery);
    let (directory, store, preview) = setup();
    store.save_source_recovery(&preview, &recovery).unwrap();
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"scenario \"Main\" {}\n"
    );
}

#[test]
fn validation_rejects_duplicate_contexts_bad_utf8_offsets_and_unknown_fields_without_echoing_text()
{
    let original = recovery();
    let mut invalid = original.clone();
    invalid.documents[0].selection.caret = 4; // within ż
    assert!(matches!(
        invalid.to_bytes(),
        Err(ProjectError::InvalidRecovery)
    ));
    let mut invalid = original.clone();
    invalid.documents[0].fields[0].selection.caret = usize::MAX;
    assert!(invalid.to_bytes().is_err());
    let mut invalid = original.clone();
    invalid.documents.push(invalid.documents[0].clone());
    assert!(invalid.to_bytes().is_err());
    let mut invalid = original.clone();
    let duplicate = invalid.documents[0].fields[0].clone();
    invalid.documents[0].fields.push(duplicate);
    assert!(invalid.to_bytes().is_err());
    let mut invalid = original.clone();
    invalid.documents[0].base_hash = Some("not-a-hash".into());
    assert!(invalid.to_bytes().is_err());
    let mut json: serde_json::Value =
        serde_json::from_slice(&original.to_bytes().unwrap()).unwrap();
    json["documents"][0]["secret-field-name"] = "sensitive-value".into();
    let error = SourceRecovery::from_bytes(&serde_json::to_vec(&json).unwrap()).unwrap_err();
    assert!(!error.to_string().contains("sensitive"));
    let mut invalid = original;
    invalid.documents[0].text = "sensitive".into();
    invalid.documents[0].path = path(".apiforge/local/recovery.json");
    assert!(invalid.to_bytes().is_err());
}

#[test]
fn exact_buffer_limit_and_aggregate_encoded_limit_never_truncate_a_successful_save() {
    let (_directory, store, preview) = setup();
    let mut at_limit = recovery();
    at_limit.documents[0].text = "x".repeat(MAX_SOURCE_BYTES);
    let saved = store.save_source_recovery(&preview, &at_limit).unwrap();
    assert_eq!(
        saved.recovery().unwrap().documents[0].text.len(),
        MAX_SOURCE_BYTES
    );
    let mut oversized = at_limit.clone();
    oversized.documents[0].text.push('x');
    assert!(matches!(
        store.save_source_recovery(&saved, &oversized),
        Err(ProjectError::Limit)
    ));
    let mut expanded = at_limit.clone();
    expanded.documents.clear();
    for index in 0..3 {
        let mut document = at_limit.documents[0].clone();
        document.document_id = format!("document-{index}");
        document.text = "\0".repeat(MAX_SOURCE_BYTES);
        document.selection = TextSelection {
            anchor: 0,
            caret: 0,
        };
        expanded.documents.push(document);
    }
    assert!(matches!(
        store.save_source_recovery(&saved, &expanded),
        Err(ProjectError::Limit)
    ));
    assert_eq!(
        store
            .load_source_recovery(&at_limit.project_id)
            .unwrap()
            .recovery(),
        Some(&at_limit)
    );
}

#[test]
fn secret_checks_cover_new_payload_backup_rotation_and_explicit_restore() {
    let (directory, store, preview) = setup();
    let mut secret = recovery();
    secret.documents[0].text.push_str("runtime-token-123");
    store.set_known_secrets(["runtime-token-123".into()]);
    assert!(matches!(
        store.save_source_recovery(&preview, &secret),
        Err(ProjectError::Secret)
    ));
    assert!(!directory.path().join(RECOVERY_PATH).exists());
    store.set_known_secrets([]);
    let saved = store.save_source_recovery(&preview, &secret).unwrap();
    store.set_known_secrets(["runtime-token-123".into()]);
    assert!(matches!(
        store.save_source_recovery(&saved, &recovery()),
        Err(ProjectError::Secret)
    ));
    assert!(!directory.path().join(BACKUP_PATH).exists());
    store.set_known_secrets([]);
    store.save_source_recovery(&saved, &recovery()).unwrap();
    fs::write(directory.path().join(RECOVERY_PATH), b"corrupt").unwrap();
    let preview = store.load_source_recovery(&manifest().id).unwrap();
    store.set_known_secrets(["runtime-token-123".into()]);
    assert!(matches!(
        store.restore_source_recovery_backup(&preview),
        Err(ProjectError::Secret)
    ));
    assert_eq!(
        fs::read(directory.path().join(RECOVERY_PATH)).unwrap(),
        b"corrupt"
    );
}

#[test]
fn unsaved_identity_and_layout_survive_restart_and_foreign_checkpoints_are_rejected() {
    let (directory, store, preview) = setup();
    let (_, mut checkpoint) = crate::project::diagram::tests::fixture();
    let mut recovery = recovery();
    let document = &mut recovery.documents[0];
    checkpoint.key = DocumentKey::Working {
        document_id: document.document_id.clone(),
    };
    if let IdentityBase::Scenario(scenario) = &mut checkpoint.identity_base {
        scenario.id = document.entity_id.clone();
    }
    document.checkpoint = Some(checkpoint);
    store.save_source_recovery(&preview, &recovery).unwrap();
    assert!(
        !directory.path().join(".apiforge/diagram.json").exists(),
        "unsaved layout is local recovery only"
    );
    drop(store);
    let store = ProjectStore::open_read_only(directory.path()).unwrap();
    assert_eq!(
        store
            .load_source_recovery(&recovery.project_id)
            .unwrap()
            .recovery(),
        Some(&recovery)
    );
    recovery.documents[0].checkpoint.as_mut().unwrap().key = DocumentKey::Working {
        document_id: "foreign-document".into(),
    };
    assert!(recovery.to_bytes().is_err());
}

#[test]
fn an_external_replacement_after_primary_write_is_not_acknowledged_as_the_requested_save() {
    let (directory, store, preview) = setup();
    let intended = recovery();
    let mut foreign = intended.clone();
    foreign.documents[0].text.push_str("foreign edit");
    let result = store.save_recovery_with_hook(&preview, &intended, |phase| {
        if phase == "primary" {
            fs::write(
                directory.path().join(RECOVERY_PATH),
                foreign.to_bytes().unwrap(),
            )
            .unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(ProjectError::Conflict { .. })));
    assert_eq!(
        store
            .load_source_recovery(&intended.project_id)
            .unwrap()
            .recovery(),
        Some(&foreign)
    );
}

#[test]
fn text_file_drafts_round_trip_and_cannot_address_managed_data() {
    let (directory, store, preview) = setup();
    let mut desired = recovery();
    desired.texts = vec![RecoveryText {
        path: path(".gitignore"),
        base_hash: match FileState::of(b"/.apiforge/local/\n") {
            FileState::File { sha256, .. } => Some(sha256),
            _ => unreachable!(),
        },
        text: "/.apiforge/local/\r\ntarget\n".into(),
        selection: TextSelection {
            anchor: 19,
            caret: 25,
        },
    }];
    let saved = store.save_source_recovery(&preview, &desired).unwrap();
    assert_eq!(saved.recovery(), Some(&desired));
    let reread = store.load_source_recovery(&manifest().id).unwrap();
    assert_eq!(reread.recovery().unwrap().texts, desired.texts);
    assert!(
        reread.recovery().unwrap().texts[0].matches_base(&FileState::of(b"/.apiforge/local/\n"))
    );

    // A draft of a file that does not exist yet records an absent baseline.
    let mut absent = desired.clone();
    absent.texts[0].base_hash = None;
    store
        .save_source_recovery(
            &store.load_source_recovery(&manifest().id).unwrap(),
            &absent,
        )
        .unwrap();

    // The project's own data is never a text draft, and neither is a duplicate.
    let mut managed = desired.clone();
    managed.texts[0].path = path(".apiforge/diagram.json");
    assert!(matches!(
        managed.to_bytes(),
        Err(ProjectError::InvalidRecovery)
    ));
    let mut duplicate = desired.clone();
    duplicate.texts.push(duplicate.texts[0].clone());
    assert!(matches!(
        duplicate.to_bytes(),
        Err(ProjectError::InvalidRecovery)
    ));
    let mut misplaced = desired;
    misplaced.texts[0].selection.caret = 999;
    assert!(matches!(
        misplaced.to_bytes(),
        Err(ProjectError::InvalidRecovery)
    ));
    drop(directory);
}
