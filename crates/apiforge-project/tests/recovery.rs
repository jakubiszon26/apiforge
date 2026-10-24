use apiforge_core::format::Position;
use apiforge_project::*;
use apiforge_storage::project::{FieldRecovery, TextSelection};
use std::fs;

const SCENARIO: &str = "scenario \"Main\" {\n  total = 2\n}\n";
const RECOVERY: &str = ".apiforge/local/recovery.json";

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

#[test]
fn creating_a_project_ignores_local_data_without_rewriting_gitignore() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join(".gitignore"), "target").unwrap();
    ProjectSession::create(directory.path(), "Folder").unwrap();
    assert_eq!(
        fs::read_to_string(directory.path().join(".gitignore")).unwrap(),
        "target\n/.apiforge/local/\n"
    );

    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join(".gitignore"), ".apiforge/local\n").unwrap();
    ProjectSession::create(directory.path(), "Folder").unwrap();
    assert_eq!(
        fs::read_to_string(directory.path().join(".gitignore")).unwrap(),
        ".apiforge/local\n"
    );

    let directory = tempfile::tempdir().unwrap();
    ProjectSession::create(directory.path(), "Folder").unwrap();
    assert_eq!(
        fs::read_to_string(directory.path().join(".gitignore")).unwrap(),
        "/.apiforge/local/\n"
    );
}

#[test]
fn unsaved_text_layout_selection_and_fields_survive_a_restart_without_saving() {
    let (directory, mut session, id) = project();
    let edited = SCENARIO.replace('2', "3");
    session.edit_source(&id, &edited, Origin::SESSION).unwrap();
    session.settle(&id).unwrap();
    let mut moved = scenario(&session, &id);
    moved.graph.nodes[0].position = Position { x: 640.0, y: 320.0 };
    moved.graph.nodes[0].auto_placed = false;
    session
        .apply_model_edit(&id, Document::Scenario(moved.clone()), Origin::SESSION)
        .unwrap();
    let selection = TextSelection {
        anchor: 5,
        caret: 2,
    };
    session.set_selection(&id, selection.clone()).unwrap();
    let field = FieldRecovery {
        widget: 9,
        base_hash: "0".repeat(64),
        text: "unfinished".into(),
        selection: TextSelection {
            anchor: 0,
            caret: 3,
        },
    };
    session.set_field_drafts(&id, vec![field.clone()]).unwrap();
    assert!(session.save_recovery().unwrap());
    assert!(
        !session.save_recovery().unwrap(),
        "unchanged recovery is not rewritten"
    );
    drop(session);

    let source = directory.path().join("scenarios/main.afs");
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        SCENARIO,
        "never saved"
    );
    let session = ProjectSession::open(directory.path()).unwrap();
    assert!(session.recovered_drafts().is_empty());
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), edited);
    assert!(document.is_dirty() && document.is_layout_dirty());
    assert_eq!(document.selection(), &selection);
    assert_eq!(document.field_drafts(), &[field]);
    assert_eq!(scenario(&session, &id), moved);
}

#[test]
fn a_draft_over_a_changed_file_waits_for_keep_or_discard() {
    let (directory, mut session, id) = project();
    let draft = SCENARIO.replace('2', "3");
    session.edit_source(&id, &draft, Origin::SESSION).unwrap();
    session.save_recovery().unwrap();
    drop(session);
    let source = directory.path().join("scenarios/main.afs");
    let external = SCENARIO.replace('2', "9");
    fs::write(&source, &external).unwrap();

    let mut session = ProjectSession::open(directory.path()).unwrap();
    let drafts = session.recovered_drafts();
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].0, DraftStatus::Stale);
    assert_eq!(session.document(&id).unwrap().text(), external);
    assert!(matches!(
        session.edit_source(&id, SCENARIO, Origin::SESSION),
        Err(Error::DraftDecisionRequired)
    ));
    assert!(matches!(
        session.save(&id),
        Err(Error::DraftDecisionRequired)
    ));
    // Undecided drafts are preserved by unrelated recovery writes and restarts.
    session.save_recovery().unwrap();
    drop(session);
    let mut session = ProjectSession::open(directory.path()).unwrap();
    assert_eq!(session.recovered_drafts().len(), 1);

    session.keep_draft(&id).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), draft);
    assert!(document.is_dirty());
    session.save(&id).unwrap();
    assert_eq!(fs::read_to_string(&source).unwrap(), draft);

    // Discarding instead keeps the file as it is.
    session.edit_source(&id, SCENARIO, Origin::SESSION).unwrap();
    session.save_recovery().unwrap();
    drop(session);
    fs::write(&source, &external).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    session.discard_draft(&id).unwrap();
    assert_eq!(session.document(&id).unwrap().text(), external);
    session.save_recovery().unwrap();
    drop(session);
    let session = ProjectSession::open(directory.path()).unwrap();
    assert!(session.recovered_drafts().is_empty());
}

#[test]
fn saving_a_document_removes_its_draft_and_read_only_sessions_cannot_recover() {
    let (directory, mut session, id) = project();
    session
        .edit_source(&id, &SCENARIO.replace('2', "3"), Origin::SESSION)
        .unwrap();
    session.save_recovery().unwrap();
    let second = ProjectSession::open(directory.path());
    let mut second = second.unwrap();
    assert!(second.is_read_only());
    assert!(matches!(
        second.save_recovery(),
        Err(Error::Storage(ProjectError::ReadOnly))
    ));
    drop(second);
    session.save(&id).unwrap();
    let recovery = fs::read_to_string(directory.path().join(RECOVERY)).unwrap();
    assert!(recovery.contains("\"documents\": []"), "{recovery}");
}

#[test]
fn unreadable_recovery_is_preserved_and_blocks_recovery_writes() {
    let (directory, mut session, id) = project();
    session
        .edit_source(&id, &SCENARIO.replace('2', "3"), Origin::SESSION)
        .unwrap();
    session.save_recovery().unwrap();
    drop(session);
    let path = directory.path().join(RECOVERY);
    fs::write(&path, r#"{"formatVersion": 99}"#).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    assert!(session.recovery_unavailable());
    assert_eq!(session.document(&id).unwrap().text(), SCENARIO);
    assert!(matches!(
        session.save_recovery(),
        Err(Error::RecoveryUnavailable)
    ));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        r#"{"formatVersion": 99}"#
    );
}

#[test]
fn saved_invalid_source_reopens_with_its_last_valid_model_for_the_same_context() {
    let (directory, mut session, id) = project();
    let valid = scenario(&session, &id);
    assert!(session.save_last_valid().unwrap());
    assert!(
        !session.save_last_valid().unwrap(),
        "unchanged cache is not rewritten"
    );
    let broken = "scenario \"Main\" {\n  total =\n}\n";
    session.edit_source(&id, broken, Origin::SESSION).unwrap();
    session.save(&id).unwrap();
    drop(session);

    let mut session = ProjectSession::open(directory.path()).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), broken);
    assert_eq!(document.model_state(), ModelState::LastValid);
    assert!(!document.accepts_model_edits());
    assert_eq!(scenario(&session, &id), valid);

    // Another configuration makes that snapshot inapplicable.
    session
        .edit_configuration(
            |manifest| manifest.environment.base_url = "http://localhost".into(),
            Origin::SESSION,
        )
        .unwrap();
    session.save(&id).unwrap();
    drop(session);
    let session = ProjectSession::open(directory.path()).unwrap();
    assert_eq!(
        session.document(&id).unwrap().model_state(),
        ModelState::Absent
    );
}

#[test]
fn closing_requires_protected_work_or_an_explicit_discard() {
    let (directory, mut session, id) = project();
    session
        .edit_source(&id, &SCENARIO.replace('2', "3"), Origin::SESSION)
        .unwrap();
    session.close().unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    assert!(
        session.document(&id).unwrap().is_dirty(),
        "restored after close"
    );

    // A read-only session cannot protect its edits by itself.
    let mut second = ProjectSession::open(directory.path()).unwrap();
    assert!(second.is_read_only());
    second
        .edit_source(&id, &SCENARIO.replace('2', "4"), Origin::SESSION)
        .unwrap();
    let (second, error) = second.close().unwrap_err();
    assert!(matches!(error, Error::Storage(ProjectError::ReadOnly)));
    second.discard_unsaved_and_close().unwrap();

    session.discard_unsaved_and_close().unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), SCENARIO);
    assert!(!document.is_dirty());
    assert!(session.recovered_drafts().is_empty());
}
