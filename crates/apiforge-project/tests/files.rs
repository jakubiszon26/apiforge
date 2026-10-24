use apiforge_core::format::Position;
use apiforge_project::*;
use apiforge_storage::project::{ProjectError, RestoreDestination};
use std::fs;

const SCENARIO: &str = "scenario \"Main\" {\n  total = 2\n}\n";
const OTHER: &str = "scenario \"Other\" {\n  count = 1\n}\n";

fn project() -> (tempfile::TempDir, ProjectSession, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Folder").unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    (directory, session, id)
}

fn scenario(session: &ProjectSession, id: &str) -> apiforge_core::format::Scenario {
    match session.document(id).unwrap().model().cloned() {
        Some(Document::Scenario(scenario)) => scenario,
        other => panic!("scenario model expected, got {other:?}"),
    }
}

fn path_of(session: &ProjectSession, id: &str) -> String {
    session
        .document(id)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned()
}

fn conflict(error: Error) -> bool {
    matches!(error, Error::Storage(ProjectError::Conflict { .. }))
}

#[test]
fn renaming_a_dirty_document_keeps_its_identity_text_and_saved_bytes() {
    let (directory, mut session, id) = project();
    let edited = SCENARIO.replace('2', "3");
    session.edit_source(&id, &edited, Origin::SESSION).unwrap();
    session.settle(&id).unwrap();
    let before = session.last_sequence();
    let moved = session
        .relocate("scenarios/main.afs", "flows/renamed.afs", Origin(7))
        .unwrap();
    assert_eq!(moved, std::slice::from_ref(&id));
    assert_eq!(path_of(&session, &id), "flows/renamed.afs");
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), edited);
    assert!(document.is_dirty());
    assert!(
        session
            .events_since(before)
            .iter()
            .any(|event| event.kind == EventKind::DocumentMoved && event.origin == Origin(7))
    );
    let root = directory.path();
    assert!(!root.join("scenarios/main.afs").exists());
    assert!(root.join("scenarios").is_dir(), "the old folder stays");
    assert_eq!(
        fs::read_to_string(root.join("flows/renamed.afs")).unwrap(),
        SCENARIO,
        "the saved bytes move; unsaved text is not saved"
    );
    session.save_recovery().unwrap();
    drop(session);
    let session = ProjectSession::open(root).unwrap();
    assert!(session.recovered_drafts().is_empty());
    assert_eq!(path_of(&session, &id), "flows/renamed.afs");
    assert_eq!(session.document(&id).unwrap().text(), edited);
}

#[test]
fn moving_a_directory_keeps_ids_layout_and_other_files() {
    let (directory, mut session, id) = project();
    let other = session
        .create_document(DocumentKind::Scenario, "scenarios/nested/other.afs", OTHER)
        .unwrap();
    let root = directory.path();
    fs::write(root.join("scenarios/notes.md"), b"notes\r\n").unwrap();
    let mut model = scenario(&session, &id);
    model.graph.nodes[0].position = Position { x: 640.0, y: 320.0 };
    model.graph.nodes[0].auto_placed = false;
    session
        .apply_model_edit(&id, Document::Scenario(model.clone()), Origin::SESSION)
        .unwrap();
    session.save(&id).unwrap();
    let mut moved = session
        .relocate("scenarios", "suite/scenarios", Origin::SESSION)
        .unwrap();
    moved.sort();
    let mut expected = vec![id.clone(), other.clone()];
    expected.sort();
    assert_eq!(moved, expected);
    assert!(!root.join("scenarios").exists());
    assert_eq!(
        fs::read(root.join("suite/scenarios/notes.md")).unwrap(),
        b"notes\r\n"
    );
    drop(session);
    let session = ProjectSession::open(root).unwrap();
    assert_eq!(path_of(&session, &id), "suite/scenarios/main.afs");
    assert_eq!(
        path_of(&session, &other),
        "suite/scenarios/nested/other.afs"
    );
    assert_eq!(
        scenario(&session, &id),
        model,
        "identity and layout survive"
    );
}

#[test]
fn a_case_only_rename_succeeds_and_nothing_is_ever_overwritten() {
    let (directory, mut session, id) = project();
    session
        .relocate("scenarios/main.afs", "scenarios/Main.afs", Origin::SESSION)
        .unwrap();
    assert_eq!(path_of(&session, &id), "scenarios/Main.afs");
    let names: Vec<_> = fs::read_dir(directory.path().join("scenarios"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["Main.afs"]);

    fs::write(directory.path().join("taken.afs"), "foreign").unwrap();
    let error = session
        .relocate("scenarios/Main.afs", "taken.afs", Origin::SESSION)
        .unwrap_err();
    assert!(conflict(error));
    assert_eq!(
        fs::read_to_string(directory.path().join("taken.afs")).unwrap(),
        "foreign"
    );
    assert_eq!(path_of(&session, &id), "scenarios/Main.afs");
}

#[test]
fn invalid_moves_are_refused_before_any_change() {
    let (directory, mut session, id) = project();
    assert!(matches!(
        session.relocate("scenarios/main.afs", "scenarios/main.txt", Origin::SESSION),
        Err(Error::RegisteredSourceExtension)
    ));
    assert!(
        session
            .relocate("scenarios", "scenarios/inner", Origin::SESSION)
            .is_err()
    );
    for refused in ["apiforge.project.json", ".apiforge/diagram.json", ".git/x"] {
        assert!(session.relocate(refused, "x", Origin::SESSION).is_err());
        assert!(
            session
                .relocate("scenarios", refused, Origin::SESSION)
                .is_err()
        );
    }
    assert!(conflict(
        session
            .relocate("absent.afs", "present.afs", Origin::SESSION)
            .unwrap_err()
    ));
    assert_eq!(path_of(&session, &id), "scenarios/main.afs");
    assert!(directory.path().join("scenarios/main.afs").exists());
}

#[test]
fn an_external_registry_change_stops_file_operations() {
    let (directory, mut session, _) = project();
    let registry = directory.path().join("apiforge.project.json");
    let changed = fs::read_to_string(&registry)
        .unwrap()
        .replace("\"Folder\"", "\"Renamed elsewhere\"");
    fs::write(&registry, &changed).unwrap();
    assert!(conflict(
        session
            .relocate("scenarios", "moved", Origin::SESSION)
            .unwrap_err()
    ));
    let preview = session.preview_delete(&["scenarios"]).unwrap();
    assert!(conflict(
        session.delete(&preview, Origin::SESSION).unwrap_err()
    ));
    assert_eq!(fs::read_to_string(&registry).unwrap(), changed);
    assert!(directory.path().join("scenarios/main.afs").exists());
}

#[test]
fn creating_folders_never_replaces_anything() {
    let (directory, mut session, _) = project();
    session.create_folder("a/b/c").unwrap();
    assert!(directory.path().join("a/b/c").is_dir());
    assert!(conflict(session.create_folder("a/b").unwrap_err()));
    assert!(conflict(
        session.create_folder("scenarios/main.afs").unwrap_err()
    ));
}

#[test]
fn deleting_a_directory_with_unsaved_work_restores_it_after_restart() {
    let (directory, mut session, id) = project();
    let root = directory.path();
    fs::write(root.join("scenarios/data.json"), b"[1]").unwrap();
    let edited = SCENARIO.replace('2', "5");
    session.edit_source(&id, &edited, Origin::SESSION).unwrap();
    session
        .edit_configuration(
            |manifest| manifest.environment.base_url = "https://unsaved.test".into(),
            Origin::SESSION,
        )
        .unwrap();
    let registry_before = fs::read(root.join("apiforge.project.json")).unwrap();

    let preview = session.preview_delete(&["scenarios"]).unwrap();
    assert_eq!(preview.documents().len(), 1);
    assert_eq!(preview.unsaved_documents(), std::slice::from_ref(&id));
    assert_eq!(preview.files().len(), 1);
    let before = session.last_sequence();
    let commit = session.delete(&preview, Origin::SESSION).unwrap();
    assert!(session.document(&id).is_none());
    assert!(
        session
            .events_since(before)
            .iter()
            .any(|event| event.kind == EventKind::DocumentRemoved)
    );
    assert!(!root.join("scenarios").exists());
    assert!(session.is_manifest_dirty(), "configuration is not saved");
    let registry = fs::read_to_string(root.join("apiforge.project.json")).unwrap();
    assert!(!registry.contains("unsaved.test"));
    assert_ne!(registry.as_bytes(), registry_before);
    session.save_configuration().unwrap();
    session.close().unwrap();

    let mut session = ProjectSession::open(root).unwrap();
    assert!(session.document(&id).is_none());
    let operations = session.trash_operations().unwrap();
    assert_eq!(operations[0].operation_id, commit.operation_id);
    let restored = session.restore(&commit.operation_id, &[]).unwrap();
    assert_eq!(restored, std::slice::from_ref(&id));
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), edited, "unsaved work returns unsaved");
    assert!(document.is_dirty());
    assert_eq!(
        fs::read_to_string(root.join("scenarios/main.afs")).unwrap(),
        SCENARIO
    );
    assert_eq!(fs::read(root.join("scenarios/data.json")).unwrap(), b"[1]");
    assert_eq!(
        session.manifest().environment.base_url,
        "https://unsaved.test"
    );
    session.save(&id).unwrap();
    drop(session);
    let session = ProjectSession::open(root).unwrap();
    assert_eq!(session.document(&id).unwrap().text(), edited);
}

#[test]
fn restoring_to_an_occupied_path_needs_a_destination() {
    let (directory, mut session, id) = project();
    let preview = session.preview_delete(&["scenarios/main.afs"]).unwrap();
    let commit = session.delete(&preview, Origin::SESSION).unwrap();
    let replacement = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", OTHER)
        .unwrap();
    assert!(conflict(
        session.restore(&commit.operation_id, &[]).unwrap_err()
    ));
    assert!(session.document(&id).is_none());
    session
        .restore(
            &commit.operation_id,
            &[RestoreDestination {
                document_id: id.clone(),
                path: apiforge_storage::project::ProjectPath::new("scenarios/main-1.afs").unwrap(),
            }],
        )
        .unwrap();
    assert_eq!(path_of(&session, &id), "scenarios/main-1.afs");
    assert_eq!(session.document(&id).unwrap().text(), SCENARIO);
    assert_eq!(session.document(&replacement).unwrap().text(), OTHER);
    assert_eq!(
        fs::read_to_string(directory.path().join("scenarios/main.afs")).unwrap(),
        OTHER
    );
}

#[test]
fn read_only_sessions_cannot_change_files() {
    let (directory, _writer, _) = project();
    let mut reader = ProjectSession::open(directory.path()).unwrap();
    assert!(reader.is_read_only());
    assert!(reader.create_folder("x").is_err());
    assert!(
        reader
            .relocate("scenarios", "moved", Origin::SESSION)
            .is_err()
    );
    assert!(directory.path().join("scenarios/main.afs").exists());
}

#[test]
fn a_refused_case_only_rename_changes_nothing() {
    let (directory, mut session, id) = project();
    let registry = fs::read(directory.path().join("apiforge.project.json")).unwrap();
    assert!(matches!(
        session.relocate("scenarios/main.afs", "scenarios/MAIN.AFS", Origin::SESSION),
        Err(Error::RegisteredSourceExtension)
    ));
    assert_eq!(path_of(&session, &id), "scenarios/main.afs");
    let names: Vec<_> = fs::read_dir(directory.path().join("scenarios"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["main.afs"]);
    assert_eq!(
        fs::read(directory.path().join("apiforge.project.json")).unwrap(),
        registry
    );
    // The same check covers a folder holding a registered source.
    session.create_folder("Flows").unwrap();
    session
        .relocate("scenarios/main.afs", "Flows/main.afs", Origin::SESSION)
        .unwrap();
    session.relocate("Flows", "flows", Origin::SESSION).unwrap();
    assert_eq!(path_of(&session, &id), "flows/main.afs");
}

#[test]
fn deleting_a_folder_includes_documents_whose_files_were_removed_outside() {
    let (directory, mut session, id) = project();
    let other = session
        .create_document(DocumentKind::Scenario, "scenarios/nested/other.afs", OTHER)
        .unwrap();
    let edited = SCENARIO.replace('2', "8");
    session.edit_source(&id, &edited, Origin::SESSION).unwrap();
    let root = directory.path();
    fs::remove_file(root.join("scenarios/main.afs")).unwrap();
    fs::remove_dir_all(root.join("scenarios/nested")).unwrap();

    let preview = session.preview_delete(&["scenarios"]).unwrap();
    let mut listed: Vec<_> = preview
        .documents()
        .iter()
        .map(|entry| entry.document_id.clone())
        .collect();
    listed.sort();
    let mut expected = vec![id.clone(), other.clone()];
    expected.sort();
    assert_eq!(listed, expected);
    assert_eq!(preview.unsaved_documents(), std::slice::from_ref(&id));
    let commit = session.delete(&preview, Origin::SESSION).unwrap();
    assert!(session.document(&id).is_none() && session.document(&other).is_none());
    assert!(!root.join("scenarios").exists());

    session.restore(&commit.operation_id, &[]).unwrap();
    assert!(session.document(&other).is_some());
    assert!(
        !root.join("scenarios/main.afs").exists(),
        "absence is restored"
    );
    // The unsaved work was written against the deleted file: it waits for a
    // decision, and keeping it recreates the file on save.
    assert!(
        session
            .recovered_drafts()
            .iter()
            .any(|(status, draft)| *status == DraftStatus::Stale && draft.document_id == id)
    );
    session.keep_draft(&id).unwrap();
    assert_eq!(session.document(&id).unwrap().text(), edited);
    session.save(&id).unwrap();
    assert_eq!(
        fs::read_to_string(root.join("scenarios/main.afs")).unwrap(),
        edited
    );

    // A missing document on its own can be deleted too; a missing path that
    // is not registered cannot.
    let preview = session
        .preview_delete(&["scenarios/nested/other.afs"])
        .unwrap();
    assert_eq!(preview.documents().len(), 1);
    session.delete(&preview, Origin::SESSION).unwrap();
    assert!(session.document(&other).is_none());
    assert!(session.preview_delete(&["scenarios/nothing.afs"]).is_err());
}
