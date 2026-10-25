use super::*;
use apiforge_storage::project::FileState;
use std::{fs, path::Path};

const ORIGINAL: &str = "scenario \"Main\" { answer = 1 }\n";
const INTENDED: &str = "scenario \"Main\" { answer = 3 }\n";
const FOREIGN: &str = "scenario \"Main\" { answer = 5 }\n";
const LOCAL: &str = "scenario \"Main\" { answer = 7 }\n";

fn interrupt(root: &Path) -> PathBuf {
    let head_path = root.join(".apiforge/local/head.json");
    let mut head: serde_json::Value =
        serde_json::from_slice(&fs::read(&head_path).unwrap()).unwrap();
    let generation = head["generation"].as_u64().unwrap();
    let id = "12345678-1234-4321-8765-123456789012";
    let directory = root.join(".apiforge/local/transactions").join(id);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("0"), INTENDED).unwrap();
    fs::write(directory.join("transaction.json"), serde_json::to_vec(&serde_json::json!({
        "formatVersion":1,"transactionId":id,"baseGeneration":generation,"targetGeneration":generation+1,
        "operations":[{"path":"main.afs","expected":FileState::of(ORIGINAL.as_bytes()),"target":FileState::of(INTENDED.as_bytes()),"payload":0}]
    })).unwrap()).unwrap();
    head["activeTransactionId"] = id.into();
    fs::write(head_path, serde_json::to_vec(&head).unwrap()).unwrap();
    fs::write(root.join("main.afs"), FOREIGN).unwrap();
    directory
}

fn source_project() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Recovery review").unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, "main.afs", ORIGINAL)
        .unwrap();
    drop(session);
    (directory, id)
}

fn choose(app: &mut FolderApp, context: &egui::Context, label: &str) {
    // A different dialog body changes the modal's measured size; let egui place
    // it at its new centre before choosing a painted action.
    frame(app, context, vec![]);
    frame(app, context, vec![]);
    let at = label_position(app, context, label);
    click(app, context, at);
}

fn assert_active(root: &Path, active: bool) {
    let head: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(".apiforge/local/head.json")).unwrap()).unwrap();
    assert_eq!(!head["activeTransactionId"].is_null(), active);
}

#[test]
fn native_conflict_decision_requires_refresh_after_another_foreign_change() {
    let (directory, id) = source_project();
    let (context, mut app) = open(directory.path());
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, LOCAL, CODE)
        .unwrap();
    app.session.as_mut().unwrap().settle(&id).unwrap();
    fs::write(directory.path().join("main.afs"), FOREIGN).unwrap();
    watch(&mut app, &context);
    choose(&mut app, &context, "Review conflict…");
    assert!(matches!(app.prompt, Some(Prompt::Conflict(_))));
    let latest = FOREIGN.replace('5', "9");
    fs::write(directory.path().join("main.afs"), &latest).unwrap();
    choose(&mut app, &context, "Use version on disk");
    assert_eq!(text(&app, &id), LOCAL);
    assert!(matches!(app.prompt, Some(Prompt::Conflict(_))));
    choose(&mut app, &context, "Refresh preview");
    choose(&mut app, &context, "Use version on disk");
    assert_eq!(text(&app, &id), latest);
    assert!(app.prompt.is_none());
}

#[test]
fn escaping_a_conflict_review_preserves_source_fields_and_disk() {
    let (directory, id) = source_project();
    let (context, mut app) = open(directory.path());
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, LOCAL, CODE)
        .unwrap();
    fs::write(directory.path().join("main.afs"), FOREIGN).unwrap();
    watch(&mut app, &context);
    app.review_conflict(apiforge_project::ConflictTarget::Document(id.clone()));
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.prompt.is_none());
    assert_eq!(text(&app, &id), LOCAL);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        FOREIGN
    );
}

#[test]
fn a_text_deletion_can_be_accepted_in_the_dialog_without_leaving_a_dirty_tab() {
    let (directory, _) = source_project();
    fs::write(directory.path().join("notes.txt"), "saved\r\n").unwrap();
    let (context, mut app) = open(directory.path());
    app.open_text("notes.txt");
    app.session
        .as_mut()
        .unwrap()
        .edit_text("notes.txt", "local\r\n", CODE)
        .unwrap();
    fs::remove_file(directory.path().join("notes.txt")).unwrap();
    watch(&mut app, &context);
    choose(&mut app, &context, "Review conflict…");
    choose(&mut app, &context, "Accept deletion");
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .text_file("notes.txt")
            .is_none()
    );
    assert!(
        app.workbench
            .tabs()
            .iter()
            .all(|tab| tab.document_id != text_tab("notes.txt"))
    );
    assert!(!directory.path().join("notes.txt").exists());
}

#[test]
fn interrupted_project_opens_a_native_preview_and_preserves_foreign_bytes_before_finish() {
    let (directory, _) = source_project();
    interrupt(directory.path());
    let (context, mut app) = open(directory.path());
    assert!(!app.has_document());
    assert!(matches!(app.prompt, Some(Prompt::Transaction(_))));
    for _ in 0..3 {
        let _ = painted(&mut app, &context, egui::vec2(960.0, 540.0));
    }
    let labels = painted(&mut app, &context, egui::vec2(960.0, 540.0));
    for label in [
        "Cancel",
        "Refresh preview",
        "Export recovery evidence…",
        "Finish intended operation",
    ] {
        let rect = labels.iter().find(|(text, _)| text == label).unwrap().1;
        assert!(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, 540.0))
                .contains_rect(rect),
            "{label}: {rect:?}"
        );
    }
    choose(&mut app, &context, "Cancel");
    assert_active(directory.path(), true);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        FOREIGN
    );
    choose(&mut app, &context, "Review transaction recovery…");
    choose(&mut app, &context, "Finish intended operation");
    assert_active(directory.path(), false);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        INTENDED
    );
    let archive_root = directory
        .path()
        .join(".apiforge/local/conflicts/12345678-1234-4321-8765-123456789012");
    let approval = fs::read_dir(archive_root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(fs::read_to_string(approval.join("0.bin")).unwrap(), FOREIGN);
    choose(&mut app, &context, "Open project");
    assert!(
        app.has_document(),
        "open error: {:?}; notice: {:?}",
        app.open_error,
        app.notice.as_ref().map(|notice| &notice.text)
    );
    assert!(
        !app.session.as_ref().unwrap().is_read_only(),
        "the temporary recovery owner is released before opening"
    );
}

#[test]
fn a_read_only_transaction_preview_cannot_finish_the_other_writers_operation() {
    let (directory, _) = source_project();
    let owner = apiforge_storage::project::ProjectStore::open_exclusive(directory.path()).unwrap();
    interrupt(directory.path());
    let (context, mut app) = open(directory.path());
    choose(&mut app, &context, "Finish intended operation");
    assert_active(directory.path(), true);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        FOREIGN
    );
    let labels = painted(&mut app, &context, egui::vec2(960.0, 540.0));
    assert!(
        labels
            .iter()
            .any(|(label, _)| label.starts_with("Read-only:"))
    );
    assert!(!owner.export_transaction_evidence().unwrap().is_empty());
}

#[test]
fn a_second_foreign_write_invalidates_native_transaction_completion() {
    let (directory, _) = source_project();
    interrupt(directory.path());
    let (context, mut app) = open(directory.path());
    let latest = FOREIGN.replace('5', "9");
    fs::write(directory.path().join("main.afs"), &latest).unwrap();
    choose(&mut app, &context, "Finish intended operation");
    assert_active(directory.path(), true);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        latest
    );
    choose(&mut app, &context, "Refresh preview");
    choose(&mut app, &context, "Finish intended operation");
    assert_active(directory.path(), false);
}

#[test]
fn corrupt_transaction_intent_blocks_initialisation_and_remains_exportable() {
    let (directory, _) = source_project();
    let transaction = interrupt(directory.path());
    fs::write(transaction.join("transaction.json"), b"broken intent").unwrap();
    let (context, mut app) = open(directory.path());
    choose(&mut app, &context, "Finish intended operation");
    assert_active(directory.path(), true);
    assert!(!app.uninitialised);
    assert_eq!(
        fs::read(transaction.join("transaction.json")).unwrap(),
        b"broken intent"
    );
    let reader = apiforge_storage::project::ProjectStore::open_read_only(directory.path()).unwrap();
    let evidence = reader.export_transaction_evidence().unwrap();
    assert!(
        String::from_utf8(evidence)
            .unwrap()
            .contains("transaction.json")
    );
}

#[test]
fn recovering_a_switch_target_protects_the_current_projects_work_before_departure() {
    let (directory, id) = source_project();
    let (target, _) = source_project();
    interrupt(target.path());
    let (context, mut app) = open(directory.path());
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, LOCAL, CODE)
        .unwrap();
    app.request_switch(target.path().to_owned());
    assert_eq!(app.root, directory.path());
    choose(&mut app, &context, "Finish intended operation");
    choose(&mut app, &context, "Open project");
    frame(&mut app, &context, vec![]);
    assert_eq!(app.root, target.path());
    let original = ProjectSession::open(directory.path()).unwrap();
    assert_eq!(original.document(&id).unwrap().text(), LOCAL);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        ORIGINAL
    );
}

#[test]
fn recovery_reload_retains_unsaved_source_and_text_and_requires_explicit_settings_discard() {
    let (directory, id) = source_project();
    fs::write(directory.path().join("notes.txt"), "saved").unwrap();
    let (context, mut app) = open(directory.path());
    app.open_text("notes.txt");
    let session = app.session.as_mut().unwrap();
    session.edit_source(&id, LOCAL, CODE).unwrap();
    session.edit_text("notes.txt", "latest text", CODE).unwrap();
    session
        .edit_configuration(
            |manifest| manifest.name = "Unsaved settings".into(),
            DIAGRAM,
        )
        .unwrap();
    interrupt(directory.path());
    app.watcher.recovery_required = true;
    app.review_transaction(directory.path().to_owned());
    choose(&mut app, &context, "Finish intended operation");
    choose(&mut app, &context, "Reload project");
    assert!(app.watcher.recovery_required);
    assert!(app.session.as_ref().unwrap().is_manifest_dirty());
    choose(&mut app, &context, "Discard settings and reload");
    assert!(!app.watcher.recovery_required);
    let session = app.session.as_ref().unwrap();
    assert_eq!(session.manifest().name, "Recovery review");
    assert!(
        session
            .recovered_drafts()
            .iter()
            .any(|(_, draft)| draft.text == LOCAL)
    );
    assert_eq!(
        session.text_file("notes.txt").unwrap().text(),
        "latest text"
    );
    assert!(
        app.workbench
            .tabs()
            .iter()
            .any(|tab| tab.document_id == text_tab("notes.txt"))
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("notes.txt")).unwrap(),
        "saved"
    );
}
