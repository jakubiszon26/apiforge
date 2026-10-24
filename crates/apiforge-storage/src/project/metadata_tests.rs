use super::*;
use crate::project::tests::{initialise, path, write};
use std::fs;

fn setup() -> (tempfile::TempDir, ProjectStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    (directory, store)
}
fn empty_diagram() -> DiagramFile {
    DiagramFile {
        format_version: 1,
        records: Vec::new(),
    }
}

#[test]
fn ordinary_saves_preserve_invalid_or_future_shared_diagrams_until_explicit_recovery() {
    for original in [
        b"{corrupt checkpoint".as_slice(),
        b"{\"formatVersion\":2,\"future\":true}".as_slice(),
    ] {
        let (directory, store) = setup();
        fs::write(directory.path().join(DIAGRAM), original).unwrap();
        assert!(
            store
                .commit(vec![write(
                    DIAGRAM,
                    FileState::of(original),
                    &empty_diagram().to_bytes().unwrap()
                )])
                .is_err()
        );
        assert_eq!(fs::read(directory.path().join(DIAGRAM)).unwrap(), original);
        let preview = store.preview_diagram_recovery().unwrap();
        assert_eq!(preview.original_bytes(), Some(original));
        let completed = store
            .replace_diagram_checkpoint(&preview, &empty_diagram())
            .unwrap();
        assert_eq!(completed.generation, 2);
        let copy = completed.preserved_copy.unwrap();
        assert_eq!(
            fs::read(directory.path().join(copy.as_str())).unwrap(),
            original
        );
        assert_eq!(
            DiagramFile::from_bytes(&fs::read(directory.path().join(DIAGRAM)).unwrap()).unwrap(),
            empty_diagram()
        );
        assert_eq!(
            fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
            b"scenario \"Main\" {}\n"
        );
        assert!(matches!(
            store.commit(vec![ProjectChange::RemoveFile {
                path: copy,
                expected: FileState::of(original)
            }]),
            Err(ProjectError::InvalidTransaction)
        ));
    }
}

#[test]
fn stale_metadata_previews_read_only_and_secret_refusal_never_replace_originals() {
    let (directory, store) = setup();
    let original = b"corrupt checkpoint";
    fs::write(directory.path().join(DIAGRAM), original).unwrap();
    let preview = store.preview_diagram_recovery().unwrap();
    let reader = ProjectStore::open(directory.path()).unwrap();
    assert!(matches!(
        reader.replace_diagram_checkpoint(&preview, &empty_diagram()),
        Err(ProjectError::ReadOnly)
    ));
    store.set_known_secrets(["corrupt checkpoint".into()]);
    assert!(matches!(
        store.replace_diagram_checkpoint(&preview, &empty_diagram()),
        Err(ProjectError::Secret)
    ));
    assert!(!directory.path().join(HISTORY).exists());
    store.set_known_secrets([]);
    fs::write(directory.path().join(DIAGRAM), b"foreign replacement").unwrap();
    assert!(matches!(
        store.replace_diagram_checkpoint(&preview, &empty_diagram()),
        Err(ProjectError::Conflict { .. })
    ));
    assert_eq!(
        fs::read(directory.path().join(DIAGRAM)).unwrap(),
        b"foreign replacement"
    );
}

#[test]
fn new_shared_metadata_is_validated_before_activation() {
    let (directory, store) = setup();
    assert!(
        store
            .commit(vec![write(
                DIAGRAM,
                FileState::Missing,
                b"invalid metadata"
            )])
            .is_err()
    );
    assert!(!directory.path().join(DIAGRAM).exists());
    assert_eq!(store.read(&[]).unwrap().generation, 1);
    assert_eq!(
        fs::read_dir(directory.path().join(".apiforge/local/transactions"))
            .unwrap()
            .count(),
        0
    );
    assert!(
        store
            .commit(vec![write(
                super::super::cache::CACHE_PATH,
                FileState::Missing,
                b"unchecked cache"
            )])
            .is_err()
    );
    assert!(
        store
            .commit(vec![write(
                &format!("{HISTORY}/unchecked.bin"),
                FileState::Missing,
                b"unchecked history"
            )])
            .is_err()
    );
    assert!(!directory.path().join(HISTORY).exists());
    assert_eq!(
        store.load().unwrap().0.documents[0].path,
        path("scenarios/main.afs")
    );
}
