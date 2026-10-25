use super::*;
use crate::project::tests::{initialise, manifest, path};
use std::{fs, io, path::Path};

const SOURCE: &[u8] = b"scenario \"Main\" {}\n";

fn setup() -> (tempfile::TempDir, ProjectStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    (directory, store)
}

fn ids() -> Vec<String> {
    vec![manifest().documents[0].document_id.clone()]
}

fn draft() -> RecoveryDocument {
    let document = manifest().documents.remove(0);
    RecoveryDocument {
        document_id: document.document_id,
        entity_id: document.entity_id,
        kind: document.kind,
        base_revision: None,
        path: document.path,
        base_hash: match FileState::of(SOURCE) {
            FileState::File { sha256, .. } => Some(sha256),
            _ => unreachable!(),
        },
        source_version: 99,
        text: "scenario \"Unsaved\" {\r\n".into(),
        selection: TextSelection {
            anchor: 0,
            caret: 3,
        },
        fields: vec![FieldRecovery {
            widget: 7,
            base_hash: "d".repeat(64),
            text: "{{env.pending".into(),
            selection: TextSelection {
                anchor: 1,
                caret: 2,
            },
        }],
        checkpoint: None,
    }
}

fn with_layout_and_recovery() -> (
    tempfile::TempDir,
    ProjectStore,
    RecoveryDocument,
    DiagramRecord,
) {
    let (directory, store) = setup();
    let (_, mut record) = crate::project::diagram::tests::fixture();
    record.key = DocumentKey::Working {
        document_id: ids().remove(0),
    };
    if let IdentityBase::Scenario(scenario) = &mut record.identity_base {
        scenario.id = manifest().documents[0].entity_id.clone();
    }
    store
        .commit(vec![write(
            path(DIAGRAM),
            FileState::Missing,
            DiagramFile {
                format_version: 1,
                records: vec![record.clone()],
            }
            .to_bytes()
            .unwrap(),
        )])
        .unwrap();
    let mut dirty = draft();
    let mut checkpoint = record.clone();
    if let IdentityBase::Scenario(scenario) = &mut checkpoint.identity_base {
        scenario.graph.nodes[0].position.x += 501.0;
    }
    dirty.checkpoint = Some(checkpoint);
    let preview = store.load_source_recovery(&manifest().id).unwrap();
    store
        .save_source_recovery(
            &preview,
            &SourceRecovery {
                format_version: 1,
                project_id: manifest().id,
                documents: vec![dirty.clone()],
                texts: Vec::new(),
            },
        )
        .unwrap();
    (directory, store, dirty, record)
}

fn interrupted() -> ProjectError {
    ProjectError::io(
        Path::new("fault-injection"),
        io::Error::other("simulated stop"),
    )
}

fn find_index(directory: &Path) -> TrashIndex {
    let operation = fs::read_dir(directory.join(TRASH))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    TrashIndex::from_bytes(&fs::read(operation.path().join("index.json")).unwrap()).unwrap()
}

#[test]
fn clean_delete_and_restore_round_trip_exact_source_and_identity_after_restart() {
    let (directory, store) = setup();
    let preview = store.preview_trash(&ids()).unwrap();
    assert_eq!(preview.documents(), manifest().documents);
    let deleted = store.trash_documents(&preview, &[]).unwrap();
    assert_eq!(deleted.generation, 2);
    assert!(!directory.path().join("scenarios/main.afs").exists());
    assert!(store.load().unwrap().0.documents.is_empty());
    assert_eq!(
        fs::read(
            directory.path().join(
                operation_path(&deleted.operation_id, "0.afs")
                    .unwrap()
                    .as_str()
            )
        )
        .unwrap(),
        SOURCE
    );
    drop(store);
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    assert_eq!(preview.index().entries[0].document, manifest().documents[0]);
    let restored = store.restore_trash(&preview, &[]).unwrap();
    assert_eq!(restored.generation, 3);
    assert_eq!(store.load().unwrap().0, manifest());
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        SOURCE
    );
    assert!(find_index(directory.path()).restored_at_unix_ms.is_some());
    assert!(
        store.preview_trash_restore(&deleted.operation_id).is_err(),
        "same deletion cannot be restored twice"
    );
    assert!(
        directory
            .path()
            .join(
                operation_path(&deleted.operation_id, "0.afs")
                    .unwrap()
                    .as_str()
            )
            .exists(),
        "no automatic trash expiry"
    );
}

#[test]
fn dirty_text_fields_and_unsaved_layout_return_to_local_recovery_without_being_saved() {
    let (directory, store, mut dirty, saved_layout) = with_layout_and_recovery();
    dirty.text.push_str("latest in-memory edit");
    dirty.source_version += 1;
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[dirty.clone()])
        .unwrap();
    let index = find_index(directory.path());
    assert_eq!(index.entries[0].recovery.as_ref(), Some(&dirty));
    assert_eq!(index.entries[0].checkpoint.as_ref(), Some(&saved_layout));
    assert!(
        store
            .load_source_recovery(&manifest().id)
            .unwrap()
            .recovery()
            .unwrap()
            .documents
            .is_empty()
    );
    let (_, snapshot) = store.load().unwrap();
    assert!(
        DiagramFile::from_bytes(snapshot.files[&path(DIAGRAM)].as_deref().unwrap())
            .unwrap()
            .records
            .is_empty()
    );
    drop(store);
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    store.restore_trash(&preview, &[]).unwrap();
    drop(store);
    let store = ProjectStore::open_read_only(directory.path()).unwrap();
    assert_eq!(
        store
            .load_source_recovery(&manifest().id)
            .unwrap()
            .recovery()
            .unwrap()
            .documents,
        vec![dirty]
    );
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        SOURCE
    );
    let (_, snapshot) = store.load().unwrap();
    assert_eq!(
        DiagramFile::from_bytes(snapshot.files[&path(DIAGRAM)].as_deref().unwrap())
            .unwrap()
            .records,
        vec![saved_layout]
    );
}

#[test]
fn occupied_path_requires_a_new_destination_and_parent_directories_are_explicit() {
    let (directory, store, dirty, _) = with_layout_and_recovery();
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[])
        .unwrap();
    fs::write(directory.path().join("scenarios/main.afs"), b"foreign file").unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    assert!(matches!(
        store.restore_trash(&preview, &[]),
        Err(ProjectError::Conflict { .. })
    ));
    let destination = path("recovered/deep/renamed.afs");
    store
        .restore_trash(
            &preview,
            &[RestoreDestination {
                document_id: ids().remove(0),
                path: destination.clone(),
            }],
        )
        .unwrap();
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"foreign file"
    );
    assert_eq!(
        fs::read(directory.path().join(destination.as_str())).unwrap(),
        SOURCE
    );
    let mut expected = dirty;
    expected.path = destination.clone();
    assert_eq!(
        store
            .load_source_recovery(&manifest().id)
            .unwrap()
            .recovery()
            .unwrap()
            .documents,
        vec![expected]
    );
    let restored = store.load().unwrap().0.documents.remove(0);
    assert_eq!(restored.document_id, ids()[0]);
    assert_eq!(restored.path, destination);
}

#[test]
fn deleting_working_component_never_removes_published_revisions_or_their_layout() {
    let (directory, store) = setup();
    let (_, fixture) = crate::project::diagram::tests::fixture();
    let project = apiforge_core::import_document(
        include_bytes!("../../../../fixtures/workspace.json"),
        &apiforge_core::ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .remove(0);
    let component = project.components[0].clone();
    let published = b"component Helper() {}\n";
    let FileState::File { sha256, .. } = FileState::of(published) else {
        unreachable!()
    };
    let (mut registry, snapshot) = store.load().unwrap();
    registry.documents[0].kind = DocumentKind::Component;
    registry.documents[0].entity_id = component.id.clone();
    registry.documents[0].base_revision = Some(component.revision);
    let revision_path = path(".apiforge/revisions/published/1.afs");
    registry.revisions.push(RevisionEntry {
        component_id: component.id.clone(),
        handle: component.handle.clone(),
        revision: component.revision,
        path: revision_path.clone(),
        source_hash: sha256,
    });
    let mut working = fixture;
    working.identity_base = IdentityBase::Component(component.clone());
    working.key = DocumentKey::Working {
        document_id: ids().remove(0),
    };
    let mut revision = working.clone();
    revision.key = DocumentKey::Revision {
        component_id: component.id,
        revision: component.revision,
    };
    store
        .commit(vec![
            ProjectChange::CreateDirectory {
                path: path(".apiforge/revisions"),
            },
            ProjectChange::CreateDirectory {
                path: path(".apiforge/revisions/published"),
            },
            write(
                revision_path.clone(),
                FileState::Missing,
                published.to_vec(),
            ),
            write_from_snapshot(
                &snapshot,
                MANIFEST_PATH,
                registry.to_public_bytes().unwrap(),
            )
            .unwrap(),
            write(
                path(DIAGRAM),
                FileState::Missing,
                DiagramFile {
                    format_version: 1,
                    records: vec![working, revision.clone()],
                }
                .to_bytes()
                .unwrap(),
            ),
        ])
        .unwrap();
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[])
        .unwrap();
    let (loaded, snapshot) = store.load().unwrap();
    assert_eq!(loaded.revisions, registry.revisions);
    assert!(loaded.documents.is_empty());
    assert_eq!(
        fs::read(directory.path().join(revision_path.as_str())).unwrap(),
        published
    );
    assert_eq!(
        DiagramFile::from_bytes(snapshot.files[&path(DIAGRAM)].as_deref().unwrap())
            .unwrap()
            .records,
        vec![revision]
    );
    store
        .restore_trash(
            &store.preview_trash_restore(&deleted.operation_id).unwrap(),
            &[],
        )
        .unwrap();
    assert_eq!(store.load().unwrap().0, registry);
}

#[test]
fn missing_saved_source_and_invalid_dirty_text_can_be_preserved_without_inventing_a_file() {
    let (directory, store) = setup();
    fs::remove_file(directory.path().join("scenarios/main.afs")).unwrap();
    let mut dirty = draft();
    dirty.base_hash = None;
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[dirty.clone()])
        .unwrap();
    assert_eq!(
        find_index(directory.path()).entries[0].source_state,
        FileState::Missing
    );
    store
        .restore_trash(
            &store.preview_trash_restore(&deleted.operation_id).unwrap(),
            &[],
        )
        .unwrap();
    assert!(!directory.path().join("scenarios/main.afs").exists());
    assert_eq!(
        store
            .load_source_recovery(&manifest().id)
            .unwrap()
            .recovery()
            .unwrap()
            .documents,
        vec![dirty]
    );
}

#[test]
fn stale_previews_and_foreign_drafts_are_rejected_before_deleting_anything() {
    for target in ["scenarios/main.afs", MANIFEST_PATH, RECOVERY_PATH, DIAGRAM] {
        let (directory, store) = setup();
        let preview = store.preview_trash(&ids()).unwrap();
        fs::write(directory.path().join(target), b"foreign").unwrap();
        assert!(matches!(
            store.trash_documents(&preview, &[]),
            Err(ProjectError::Conflict { .. })
        ));
        assert!(!directory.path().join(TRASH).exists());
    }
    let (_directory, store) = setup();
    let preview = store.preview_trash(&ids()).unwrap();
    let mut foreign = draft();
    foreign.entity_id = "wrong-owner".into();
    assert!(matches!(
        store.trash_documents(&preview, &[foreign]),
        Err(ProjectError::InvalidTrash)
    ));
    assert_eq!(store.load().unwrap().0, manifest());
    assert!(store.preview_trash(&[]).is_err());
    assert!(
        store
            .preview_trash(&[ids()[0].clone(), ids()[0].clone()])
            .is_err()
    );
    assert!(store.preview_trash(&["nonexistent".into()]).is_err());
}

#[test]
fn read_only_sessions_preview_but_never_delete_or_restore() {
    let (directory, store) = setup();
    let reader = ProjectStore::open(directory.path()).unwrap();
    assert!(reader.is_read_only());
    let preview = reader.preview_trash(&ids()).unwrap();
    assert!(matches!(
        reader.trash_documents(&preview, &[]),
        Err(ProjectError::ReadOnly)
    ));
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[])
        .unwrap();
    let preview = reader.preview_trash_restore(&deleted.operation_id).unwrap();
    assert!(matches!(
        reader.restore_trash(&preview, &[]),
        Err(ProjectError::ReadOnly)
    ));
    assert!(!directory.path().join("scenarios/main.afs").exists());
}

#[test]
fn known_secrets_block_source_draft_checkpoint_and_restore_copies_before_activation() {
    for in_saved_source in [false, true] {
        let (directory, store) = setup();
        let mut dirty = draft();
        dirty.fields[0].text.push_str("runtime-canary-secret");
        if in_saved_source {
            fs::write(
                directory.path().join("scenarios/main.afs"),
                b"runtime-canary-secret",
            )
            .unwrap();
        }
        let preview = store.preview_trash(&ids()).unwrap();
        store.set_known_secrets(["runtime-canary-secret".into()]);
        assert!(matches!(
            store.trash_documents(
                &preview,
                if in_saved_source {
                    &[]
                } else {
                    std::slice::from_ref(&dirty)
                }
            ),
            Err(ProjectError::Secret)
        ));
        assert!(directory.path().join("scenarios/main.afs").exists());
        assert!(!directory.path().join(TRASH).exists());
    }
    let (directory, store) = setup();
    fs::write(
        directory.path().join("scenarios/main.afs"),
        b"runtime-canary-secret",
    )
    .unwrap();
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[])
        .unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    store.set_known_secrets(["runtime-canary-secret".into()]);
    assert!(matches!(
        store.restore_trash(&preview, &[]),
        Err(ProjectError::Secret)
    ));
    assert!(!directory.path().join("scenarios/main.afs").exists());
    assert!(find_index(directory.path()).restored_at_unix_ms.is_none());
}

#[test]
fn changed_payload_unknown_index_and_unsafe_destinations_are_preserved() {
    let (directory, store) = setup();
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[])
        .unwrap();
    let payload = directory.path().join(
        operation_path(&deleted.operation_id, "0.afs")
            .unwrap()
            .as_str(),
    );
    fs::write(&payload, b"corrupt payload").unwrap();
    assert!(matches!(
        store.preview_trash_restore(&deleted.operation_id),
        Err(ProjectError::InvalidTrash)
    ));
    fs::write(&payload, SOURCE).unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    assert!(
        store
            .restore_trash(
                &preview,
                &[RestoreDestination {
                    document_id: ids().remove(0),
                    path: path(".apiforge/revisions/other/1.afs")
                }]
            )
            .is_err()
    );
    let index_path = directory.path().join(
        operation_path(&deleted.operation_id, "index.json")
            .unwrap()
            .as_str(),
    );
    let future = b"{\"formatVersion\":2,\"future\":true}";
    fs::write(&index_path, future).unwrap();
    assert!(matches!(
        store.preview_trash_restore(&deleted.operation_id),
        Err(ProjectError::UnsupportedVersion)
    ));
    assert_eq!(fs::read(index_path).unwrap(), future);
    assert!(!directory.path().join("scenarios/main.afs").exists());
}

#[test]
fn restart_at_every_delete_boundary_preserves_source_or_finishes_the_whole_deletion() {
    let (_directory, store, _, _) = with_layout_and_recovery();
    let mut trace = Vec::new();
    store
        .trash_with_hook(&store.preview_trash(&ids()).unwrap(), &[], |phase| {
            trace.push(phase.to_owned());
            Ok(())
        })
        .unwrap();
    for (stop, label) in trace.iter().enumerate() {
        let (directory, store, dirty, _) = with_layout_and_recovery();
        let mut boundary = 0;
        let mut activated = false;
        let result = store.trash_with_hook(&store.preview_trash(&ids()).unwrap(), &[], |phase| {
            activated |= phase == "activation";
            let current = boundary;
            boundary += 1;
            if current == stop {
                Err(interrupted())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err(), "boundary {stop}: {label}");
        drop(store);
        let store = ProjectStore::open_exclusive(directory.path()).unwrap();
        store.recover().unwrap();
        let (registry, _) = store.load().unwrap();
        if activated {
            assert!(registry.documents.is_empty());
            assert!(!directory.path().join("scenarios/main.afs").exists());
            assert_eq!(
                find_index(directory.path()).entries[0].recovery.as_ref(),
                Some(&dirty)
            );
        } else {
            assert_eq!(registry, manifest());
            assert_eq!(
                fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
                SOURCE
            );
        }
    }
}

#[test]
fn restart_at_every_restore_boundary_never_silently_saves_the_dirty_draft() {
    let (_directory, store, _, _) = with_layout_and_recovery();
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[])
        .unwrap();
    let mut trace = Vec::new();
    store
        .restore_with_hook(
            &store.preview_trash_restore(&deleted.operation_id).unwrap(),
            &[],
            |phase| {
                trace.push(phase.to_owned());
                Ok(())
            },
        )
        .unwrap();
    for (stop, label) in trace.iter().enumerate() {
        let (directory, store, dirty, _) = with_layout_and_recovery();
        let deleted = store
            .trash_documents(&store.preview_trash(&ids()).unwrap(), &[])
            .unwrap();
        let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
        let mut boundary = 0;
        let mut activated = false;
        let result = store.restore_with_hook(&preview, &[], |phase| {
            activated |= phase == "activation";
            let current = boundary;
            boundary += 1;
            if current == stop {
                Err(interrupted())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err(), "boundary {stop}: {label}");
        drop(store);
        let store = ProjectStore::open_exclusive(directory.path()).unwrap();
        store.recover().unwrap();
        if activated {
            assert_eq!(store.load().unwrap().0, manifest());
            assert_eq!(
                fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
                SOURCE
            );
            assert_eq!(
                store
                    .load_source_recovery(&manifest().id)
                    .unwrap()
                    .recovery()
                    .unwrap()
                    .documents,
                vec![dirty]
            );
            assert!(find_index(directory.path()).restored_at_unix_ms.is_some());
        } else {
            assert!(store.load().unwrap().0.documents.is_empty());
            assert!(!directory.path().join("scenarios/main.afs").exists());
            assert!(find_index(directory.path()).restored_at_unix_ms.is_none());
        }
    }
}

#[test]
fn foreign_source_after_activation_blocks_roll_forward_and_keeps_both_candidates() {
    let (directory, store) = setup();
    let deleted = store
        .trash_documents(&store.preview_trash(&ids()).unwrap(), &[])
        .unwrap();
    store
        .restore_with_hook(
            &store.preview_trash_restore(&deleted.operation_id).unwrap(),
            &[],
            |phase| {
                if phase == "activation" {
                    fs::write(
                        directory.path().join("scenarios/main.afs"),
                        b"foreign content",
                    )
                    .unwrap();
                    return Err(interrupted());
                }
                Ok(())
            },
        )
        .unwrap_err();
    assert!(matches!(
        store.recover(),
        Err(ProjectError::Conflict { .. })
    ));
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"foreign content"
    );
    assert_eq!(
        fs::read(
            directory.path().join(
                operation_path(&deleted.operation_id, "0.afs")
                    .unwrap()
                    .as_str()
            )
        )
        .unwrap(),
        SOURCE
    );
    assert!(matches!(store.load(), Err(ProjectError::RecoveryRequired)));
}

fn add_documents(store: &ProjectStore) -> Vec<DocumentEntry> {
    let (mut registry, snapshot) = store.load().unwrap();
    let mut changes = Vec::new();
    for label in ["second", "third"] {
        let entry = DocumentEntry {
            document_id: format!("document-{label}"),
            entity_id: format!("scenario-{label}"),
            kind: DocumentKind::Scenario,
            path: path(&format!("scenarios/{label}.afs")),
            base_revision: None,
            recovered: false,
        };
        changes.push(write(
            entry.path.clone(),
            FileState::Missing,
            format!("scenario \"{label}\" {{}}\n").into_bytes(),
        ));
        registry.documents.push(entry);
    }
    changes.push(
        write_from_snapshot(
            &snapshot,
            MANIFEST_PATH,
            registry.to_public_bytes().unwrap(),
        )
        .unwrap(),
    );
    store.commit(changes).unwrap();
    registry.documents
}

#[test]
fn multi_document_operations_keep_registry_order_and_unrelated_recovery() {
    let (directory, store) = setup();
    let documents = add_documents(&store);
    let mut drafts = Vec::new();
    for entry in &documents {
        let mut draft = draft();
        draft.document_id = entry.document_id.clone();
        draft.entity_id = entry.entity_id.clone();
        draft.path = entry.path.clone();
        drafts.push(draft);
    }
    let recovery = SourceRecovery {
        format_version: 1,
        project_id: manifest().id,
        documents: drafts.clone(),
        texts: Vec::new(),
    };
    store
        .save_source_recovery(
            &store.load_source_recovery(&recovery.project_id).unwrap(),
            &recovery,
        )
        .unwrap();
    let selected = vec![
        documents[2].document_id.clone(),
        documents[0].document_id.clone(),
    ];
    let deleted = store
        .trash_documents(&store.preview_trash(&selected).unwrap(), &[])
        .unwrap();
    assert_eq!(
        store.load().unwrap().0.documents,
        vec![documents[1].clone()]
    );
    assert_eq!(
        store
            .load_source_recovery(&recovery.project_id)
            .unwrap()
            .recovery()
            .unwrap()
            .documents,
        vec![drafts[1].clone()]
    );
    assert!(directory.path().join(documents[1].path.as_str()).exists());
    store
        .restore_trash(
            &store.preview_trash_restore(&deleted.operation_id).unwrap(),
            &[],
        )
        .unwrap();
    assert_eq!(store.load().unwrap().0.documents, documents);
    let observed = store.load_source_recovery(&recovery.project_id).unwrap();
    let restored = &observed.recovery().unwrap().documents;
    for draft in drafts {
        assert!(restored.contains(&draft));
    }
}

#[test]
fn planned_case_collisions_and_symlink_parents_never_activate_a_restore() {
    let (directory, store) = setup();
    let documents = add_documents(&store);
    let selected: Vec<_> = documents[..2]
        .iter()
        .map(|entry| entry.document_id.clone())
        .collect();
    let deleted = store
        .trash_documents(&store.preview_trash(&selected).unwrap(), &[])
        .unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    let destinations = [
        RestoreDestination {
            document_id: selected[0].clone(),
            path: path("new/Duplicate.afs"),
        },
        RestoreDestination {
            document_id: selected[1].clone(),
            path: path("new/duplicate.afs"),
        },
    ];
    assert!(store.restore_trash(&preview, &destinations).is_err());
    assert!(!directory.path().join("new").exists());
    assert!(find_index(directory.path()).restored_at_unix_ms.is_none());
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), directory.path().join("linked")).unwrap();
        let destinations = [RestoreDestination {
            document_id: selected[0].clone(),
            path: path("linked/escape.afs"),
        }];
        assert!(matches!(
            store.restore_trash(&preview, &destinations),
            Err(ProjectError::UnsafePath { .. })
        ));
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    }
}

#[test]
fn unresolved_recovery_and_corrupt_shared_checkpoints_block_deletion_without_overwriting_metadata()
{
    let (directory, store, _, _) = with_layout_and_recovery();
    let recovery = store.load_source_recovery(&manifest().id).unwrap();
    store
        .save_source_recovery(&recovery, recovery.recovery().unwrap())
        .unwrap();
    fs::write(directory.path().join(RECOVERY_PATH), b"corrupt recovery").unwrap();
    assert!(matches!(
        store.preview_trash(&ids()),
        Err(ProjectError::RecoveryDecisionRequired)
    ));
    assert!(directory.path().join("scenarios/main.afs").exists());
    let candidate = store.load_source_recovery(&manifest().id).unwrap();
    store.restore_source_recovery_backup(&candidate).unwrap();
    fs::write(directory.path().join(DIAGRAM), b"corrupt checkpoint").unwrap();
    assert!(matches!(
        store.preview_trash(&ids()),
        Err(ProjectError::InvalidCheckpoint)
    ));
    assert_eq!(
        fs::read(directory.path().join(DIAGRAM)).unwrap(),
        b"corrupt checkpoint"
    );
    assert!(!directory.path().join(TRASH).exists());
}

#[test]
fn known_secrets_in_checkpoint_notes_are_rejected_before_any_original_is_removed() {
    let (directory, store, mut dirty, _) = with_layout_and_recovery();
    if let IdentityBase::Scenario(scenario) = &mut dirty.checkpoint.as_mut().unwrap().identity_base
    {
        scenario.graph.nodes[0].note = Some("runtime-checkpoint-canary".into());
    }
    let preview = store.preview_trash(&ids()).unwrap();
    store.set_known_secrets(["runtime-checkpoint-canary".into()]);
    assert!(matches!(
        store.trash_documents(&preview, &[dirty]),
        Err(ProjectError::Secret)
    ));
    assert!(directory.path().join("scenarios/main.afs").exists());
    assert!(!directory.path().join(TRASH).exists());
}

#[test]
fn a_checkpoint_key_with_foreign_entity_context_is_preserved_instead_of_silently_deleted() {
    let (directory, store, _, mut checkpoint) = with_layout_and_recovery();
    if let IdentityBase::Scenario(scenario) = &mut checkpoint.identity_base {
        scenario.id = "foreign-scenario".into();
    }
    let foreign = DiagramFile {
        format_version: 1,
        records: vec![checkpoint],
    }
    .to_bytes()
    .unwrap();
    fs::write(directory.path().join(DIAGRAM), &foreign).unwrap();
    assert!(matches!(
        store.preview_trash(&ids()),
        Err(ProjectError::InvalidCheckpoint)
    ));
    assert_eq!(fs::read(directory.path().join(DIAGRAM)).unwrap(), foreign);
    assert!(directory.path().join("scenarios/main.afs").exists());
    assert!(!directory.path().join(TRASH).exists());
}

fn paths(values: &[&str]) -> Vec<ProjectPath> {
    values.iter().map(|value| path(value)).collect()
}

#[test]
fn deleting_a_directory_preserves_every_file_and_restores_the_tree_after_restart() {
    let (directory, store) = setup();
    let root = directory.path();
    fs::create_dir_all(root.join("scenarios/empty")).unwrap();
    fs::create_dir_all(root.join("scenarios/assets")).unwrap();
    fs::write(root.join("scenarios/assets/payload.json"), b"{\"a\":1}").unwrap();
    fs::write(root.join("scenarios/notes.md"), b"exact\r\nbytes").unwrap();
    let preview = store.preview_trash_paths(&paths(&["scenarios"])).unwrap();
    assert_eq!(preview.documents(), manifest().documents);
    assert_eq!(
        preview.files(),
        paths(&["scenarios/assets/payload.json", "scenarios/notes.md"])
    );
    assert_eq!(preview.directories().last().unwrap().as_str(), "scenarios");
    let deleted = store.trash_documents(&preview, &[]).unwrap();
    assert!(!root.join("scenarios").exists());
    assert!(store.load().unwrap().0.documents.is_empty());
    let operations = store.trash_operations().unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].files.len(), 2);
    assert_eq!(operations[0].directories.len(), 3);
    drop(store);
    let store = ProjectStore::open_exclusive(root).unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    store.restore_trash(&preview, &[]).unwrap();
    assert_eq!(store.load().unwrap().0, manifest());
    assert_eq!(fs::read(root.join("scenarios/main.afs")).unwrap(), SOURCE);
    assert_eq!(
        fs::read(root.join("scenarios/notes.md")).unwrap(),
        b"exact\r\nbytes"
    );
    assert_eq!(
        fs::read(root.join("scenarios/assets/payload.json")).unwrap(),
        b"{\"a\":1}"
    );
    assert!(root.join("scenarios/empty").is_dir());
    assert!(
        store.trash_operations().unwrap()[0]
            .restored_at_unix_ms
            .is_some()
    );
}

#[test]
fn a_loose_file_is_deleted_without_touching_the_registry() {
    let (directory, store) = setup();
    let root = directory.path();
    fs::write(root.join("README.md"), b"read me").unwrap();
    let registry = fs::read(root.join(MANIFEST_PATH)).unwrap();
    let preview = store.preview_trash_paths(&paths(&["README.md"])).unwrap();
    assert!(preview.documents().is_empty());
    let deleted = store.trash_documents(&preview, &[]).unwrap();
    assert!(!root.join("README.md").exists());
    assert_eq!(fs::read(root.join(MANIFEST_PATH)).unwrap(), registry);
    fs::write(root.join("README.md"), b"replacement").unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    assert!(matches!(
        store.restore_trash(&preview, &[]),
        Err(ProjectError::Conflict { .. })
    ));
    assert_eq!(fs::read(root.join("README.md")).unwrap(), b"replacement");
    fs::remove_file(root.join("README.md")).unwrap();
    let preview = store.preview_trash_restore(&deleted.operation_id).unwrap();
    store.restore_trash(&preview, &[]).unwrap();
    assert_eq!(fs::read(root.join("README.md")).unwrap(), b"read me");
}

#[test]
fn the_registry_control_data_and_missing_paths_cannot_be_deleted() {
    let (directory, store) = setup();
    for refused in [MANIFEST_PATH, ".apiforge/diagram.json"] {
        assert!(matches!(
            store.preview_trash_paths(&paths(&[refused])),
            Err(ProjectError::InvalidTrash)
        ));
    }
    assert!(matches!(
        store.preview_trash_paths(&paths(&["absent.md"])),
        Err(ProjectError::Conflict { .. })
    ));
    assert!(matches!(
        store.preview_trash_paths(&[]),
        Err(ProjectError::InvalidTrash)
    ));
    assert!(directory.path().join(MANIFEST_PATH).exists());
}

#[test]
fn a_file_added_to_a_directory_after_preview_stops_the_deletion() {
    let (directory, store) = setup();
    let root = directory.path();
    let preview = store.preview_trash_paths(&paths(&["scenarios"])).unwrap();
    fs::write(root.join("scenarios/late.md"), b"late").unwrap();
    assert!(store.trash_documents(&preview, &[]).is_err());
    assert_eq!(fs::read(root.join("scenarios/main.afs")).unwrap(), SOURCE);
    assert_eq!(fs::read(root.join("scenarios/late.md")).unwrap(), b"late");
    assert_eq!(store.load().unwrap().0, manifest());
}

#[test]
fn directory_deletion_and_restoration_roll_forward_from_every_boundary() {
    for stage in ["activation", "operation"] {
        for occurrence in 0..6 {
            let (directory, store) = setup();
            let root = directory.path();
            fs::create_dir_all(root.join("scenarios/inner")).unwrap();
            fs::write(root.join("scenarios/inner/data.bin"), [0u8, 1, 2]).unwrap();
            let preview = store.preview_trash_paths(&paths(&["scenarios"])).unwrap();
            let mut seen = 0;
            let result = store.trash_with_hook(&preview, &[], |boundary| {
                if boundary == stage {
                    seen += 1;
                    if seen > occurrence {
                        return Err(interrupted());
                    }
                }
                Ok(())
            });
            drop(store);
            let store = ProjectStore::open_exclusive(root).unwrap();
            if result.is_err() {
                store.recover().unwrap();
            }
            assert!(!root.join("scenarios").exists(), "{stage} {occurrence}");
            let operation = store.trash_operations().unwrap().remove(0);
            let preview = store
                .preview_trash_restore(&operation.operation_id)
                .unwrap();
            store.restore_trash(&preview, &[]).unwrap();
            assert_eq!(
                fs::read(root.join("scenarios/inner/data.bin")).unwrap(),
                [0u8, 1, 2]
            );
            assert_eq!(store.load().unwrap().0, manifest());
        }
    }
}
