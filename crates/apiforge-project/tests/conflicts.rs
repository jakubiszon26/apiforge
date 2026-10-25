use apiforge_project::*;
use apiforge_storage::project::{FieldRecovery, TextSelection};
use std::fs;

const SAVED: &str = "scenario \"Main\" { answer = 1 }\n";
const LOCAL: &str = "scenario \"Main\" { answer = 2 }\n";
const FOREIGN: &str = "scenario \"Main\" { answer = 3 }\n";

fn conflicted() -> (tempfile::TempDir, ProjectSession, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Conflicts").unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, "main.afs", SAVED)
        .unwrap();
    session.edit_source(&id, LOCAL, Origin(1)).unwrap();
    session.settle(&id).unwrap();
    fs::write(directory.path().join("main.afs"), FOREIGN).unwrap();
    poll(&mut session);
    (directory, session, id)
}

fn poll(session: &mut ProjectSession) {
    let changes = session.external_check().run().unwrap();
    session.apply_external(changes).unwrap();
}

#[test]
fn preview_and_export_do_not_replace_work_or_unlock_graph_mutations() {
    let (directory, mut session, id) = conflicted();
    let preview = session
        .preview_conflict(ConflictTarget::Document(id.clone()))
        .unwrap();
    assert_eq!(preview.local_bytes(), LOCAL.as_bytes());
    assert_eq!(preview.disk_bytes(), Some(FOREIGN.as_bytes()));
    let document = session.document(&id).unwrap();
    let model = document.model().unwrap().clone();
    assert!(!document.accepts_model_edits());
    assert!(matches!(
        session.apply_model_edit(&id, model, Origin(2)),
        Err(Error::ModelLocked)
    ));
    assert!(matches!(
        session.undo(&id, Origin(2)),
        Err(Error::ModelLocked)
    ));
    assert_eq!(session.document(&id).unwrap().text(), LOCAL);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        FOREIGN
    );
    session
        .resolve_conflict(&preview, ConflictDecision::UseDisk, Origin(2))
        .unwrap();
    assert_eq!(session.document(&id).unwrap().text(), FOREIGN);
    assert!(!session.document(&id).unwrap().needs_recovery());
}

#[test]
fn changed_foreign_candidate_invalidates_the_decision_and_preserves_local_work() {
    let (directory, mut session, id) = conflicted();
    let preview = session
        .preview_conflict(ConflictTarget::Document(id.clone()))
        .unwrap();
    let latest = FOREIGN.replace('3', "4");
    fs::write(directory.path().join("main.afs"), &latest).unwrap();
    assert!(matches!(
        session.resolve_conflict(&preview, ConflictDecision::UseDisk, Origin(2)),
        Err(Error::StaleConflict)
    ));
    assert_eq!(session.document(&id).unwrap().text(), LOCAL);
    let latest_preview = session
        .preview_conflict(ConflictTarget::Document(id.clone()))
        .unwrap();
    session
        .resolve_conflict(&latest_preview, ConflictDecision::UseDisk, Origin(2))
        .unwrap();
    assert_eq!(session.document(&id).unwrap().text(), latest);
}

#[test]
fn local_edits_and_unfinished_fields_invalidate_a_displayed_decision() {
    let (_directory, mut session, id) = conflicted();
    let preview = session
        .preview_conflict(ConflictTarget::Document(id.clone()))
        .unwrap();
    session
        .edit_source(&id, &LOCAL.replace('2', "5"), Origin(1))
        .unwrap();
    assert!(matches!(
        session.resolve_conflict(&preview, ConflictDecision::UseDisk, Origin(2)),
        Err(Error::StaleConflict)
    ));
    session.settle(&id).unwrap();
    let preview = session
        .preview_conflict(ConflictTarget::Document(id.clone()))
        .unwrap();
    session
        .set_field_drafts(
            &id,
            vec![FieldRecovery {
                widget: 10,
                base_hash: "a".repeat(64),
                text: "unfinished".into(),
                selection: TextSelection {
                    anchor: 0,
                    caret: 0,
                },
            }],
        )
        .unwrap();
    assert!(matches!(
        session.resolve_conflict(&preview, ConflictDecision::UseDisk, Origin(2)),
        Err(Error::StaleConflict)
    ));
    assert_eq!(session.document(&id).unwrap().field_drafts().len(), 1);
}

#[test]
fn a_preview_from_a_closed_session_cannot_discard_a_reopened_sessions_work() {
    let (directory, session, id) = conflicted();
    let preview = session
        .preview_conflict(ConflictTarget::Document(id.clone()))
        .unwrap();
    drop(session);
    let mut next = ProjectSession::open(directory.path()).unwrap();
    next.edit_source(&id, LOCAL, Origin(1)).unwrap();
    fs::write(directory.path().join("main.afs"), SAVED).unwrap();
    poll(&mut next);
    assert!(matches!(
        next.resolve_conflict(&preview, ConflictDecision::UseDisk, Origin(2)),
        Err(Error::StaleConflict)
    ));
    assert_eq!(next.document(&id).unwrap().text(), LOCAL);
}

#[test]
fn a_reappearing_deleted_document_is_never_approved_as_missing() {
    let (directory, mut session, id) = conflicted();
    let file = directory.path().join("main.afs");
    fs::remove_file(&file).unwrap();
    poll(&mut session);
    let preview = session
        .preview_conflict(ConflictTarget::Document(id.clone()))
        .unwrap();
    assert!(preview.is_deleted());
    fs::write(&file, FOREIGN).unwrap();
    assert!(matches!(
        session.resolve_conflict(&preview, ConflictDecision::KeepDeleted, Origin(2)),
        Err(Error::StaleConflict)
    ));
    assert!(session.keep_deleted(&id).is_err());
    assert!(matches!(session.save(&id), Err(Error::ExternalConflict)));
    assert_eq!(fs::read_to_string(&file).unwrap(), FOREIGN);
}

#[test]
fn text_conflicts_preserve_raw_non_utf8_candidates_and_reject_unseen_deletions() {
    let (directory, mut session, _) = conflicted();
    let file = directory.path().join("README.md");
    fs::write(&file, "saved\r\n").unwrap();
    session.open_text_file("README.md").unwrap();
    session
        .edit_text("README.md", "local\r\n", Origin(1))
        .unwrap();
    fs::write(&file, [0xff, 0, 0xfe]).unwrap();
    poll(&mut session);
    let target = ConflictTarget::Text("README.md".into());
    let preview = session.preview_conflict(target.clone()).unwrap();
    assert_eq!(preview.disk_bytes(), Some([0xff, 0, 0xfe].as_slice()));
    assert!(matches!(
        session.resolve_conflict(&preview, ConflictDecision::UseDisk, Origin(2)),
        Err(Error::NotText(_))
    ));
    assert_eq!(session.text_file("README.md").unwrap().text(), "local\r\n");
    fs::remove_file(&file).unwrap();
    poll(&mut session);
    let preview = session.preview_conflict(target).unwrap();
    fs::write(&file, "reappeared").unwrap();
    assert!(matches!(
        session.resolve_conflict(&preview, ConflictDecision::KeepDeleted, Origin(2)),
        Err(Error::StaleConflict)
    ));
    assert!(session.keep_deleted_text("README.md").is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "reappeared");
}

#[test]
fn accepting_a_text_deletion_closes_cleanly_without_recreating_an_empty_file() {
    let (directory, mut session, _) = conflicted();
    let file = directory.path().join("notes.txt");
    fs::write(&file, "saved").unwrap();
    session.open_text_file("notes.txt").unwrap();
    session.edit_text("notes.txt", "local", Origin(1)).unwrap();
    fs::remove_file(&file).unwrap();
    poll(&mut session);
    let preview = session
        .preview_conflict(ConflictTarget::Text("notes.txt".into()))
        .unwrap();
    session
        .resolve_conflict(&preview, ConflictDecision::UseDisk, Origin(2))
        .unwrap();
    assert!(!session.text_file("notes.txt").unwrap().needs_recovery());
    session.close_text_file("notes.txt").unwrap();
    assert!(!file.exists());
}

#[test]
fn a_context_change_invalidates_conflict_consent_without_changing_source() {
    let (_directory, mut session, id) = conflicted();
    let preview = session
        .preview_conflict(ConflictTarget::Document(id.clone()))
        .unwrap();
    session.invalidate_runtime_inputs(Origin(1));
    assert!(matches!(
        session.resolve_conflict(&preview, ConflictDecision::UseDisk, Origin(2)),
        Err(Error::StaleConflict)
    ));
    assert_eq!(session.document(&id).unwrap().text(), LOCAL);
}

#[test]
fn reload_preserves_the_latest_open_text_and_held_drafts_before_any_recovery_write() {
    let (directory, mut session, _) = conflicted();
    let file = directory.path().join("notes.txt");
    fs::write(&file, "saved\r\n").unwrap();
    session.open_text_file("notes.txt").unwrap();
    session
        .edit_text("notes.txt", "latest unsaved\r\n", Origin(1))
        .unwrap();
    session.reload_project().unwrap();
    assert_eq!(
        session.text_file("notes.txt").unwrap().text(),
        "latest unsaved\r\n"
    );
    assert!(!session.text_file("notes.txt").unwrap().is_held());
    fs::write(&file, "foreign\n").unwrap();
    session.reload_project().unwrap();
    let text = session.text_file("notes.txt").unwrap();
    assert_eq!(text.text(), "latest unsaved\r\n");
    assert!(
        text.is_held(),
        "changed baseline needs an explicit decision"
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), "foreign\n");
}
