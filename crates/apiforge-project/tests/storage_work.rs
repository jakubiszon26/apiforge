use apiforge_core::format::Position;
use apiforge_project::*;
use std::{fs, sync::mpsc};

const SAVED: &str = "scenario \"Main\" { answer = 1 }\n";
const OLDER: &str = "scenario \"Main\" { answer = 2 }\n";
const NEWER: &str = "scenario \"Main\" { answer = 3 }\n";

fn project() -> (tempfile::TempDir, ProjectSession, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Worker").unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, "main.afs", SAVED)
        .unwrap();
    (directory, session, id)
}

#[test]
fn saving_an_older_snapshot_preserves_newer_typing_and_acknowledges_only_its_version() {
    let (directory, mut session, id) = project();
    session.edit_source(&id, OLDER, Origin(1)).unwrap();
    let snapshot = session.storage_snapshot();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let document_id = id.clone();
    let worker = std::thread::spawn(move || {
        snapshot.run(StorageMode::PreserveEdits, |candidate| {
            started.send(()).unwrap();
            resume.recv().unwrap();
            candidate.save(&document_id)
        })
    });
    ready.recv().unwrap();
    session.edit_source(&id, NEWER, Origin(1)).unwrap();
    let before = session.last_sequence();
    release.send(()).unwrap();
    let saved = session
        .complete_storage(worker.join().unwrap())
        .unwrap()
        .unwrap();
    let live = session.document(&id).unwrap();
    assert_eq!(live.text(), NEWER);
    assert!(live.is_dirty());
    assert!(live.source_version() > saved.source_version);
    assert_eq!(live.analysis(), AnalysisState::Pending);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        OLDER
    );
    let events = session.events_since(before);
    assert!(events.iter().any(|event| matches!(event.kind, EventKind::Saved { source_version, .. } if source_version == saved.source_version)));
    assert!(!events.iter().any(|event| matches!(
        event.kind,
        EventKind::SourceChanged { .. } | EventKind::ModelChanged { .. }
    )));
    assert!(
        events
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    session.save(&id).unwrap();
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        NEWER
    );
}

#[test]
fn a_newer_layout_and_its_history_survive_a_source_save_completion() {
    let (_directory, mut session, id) = project();
    session.edit_source(&id, OLDER, Origin(1)).unwrap();
    session.settle(&id).unwrap();
    let snapshot = session.storage_snapshot();
    let mut model = session.document(&id).unwrap().model().unwrap().clone();
    let Document::Scenario(scenario) = &mut model else {
        panic!()
    };
    let previous = scenario.graph.nodes[0].position.clone();
    let moved = Position {
        x: previous.x + 100.0,
        y: previous.y + 60.0,
    };
    scenario.graph.nodes[0].position = moved.clone();
    session.apply_model_edit(&id, model, Origin(2)).unwrap();
    let completion = snapshot.run(StorageMode::PreserveEdits, |candidate| candidate.save(&id));
    session.complete_storage(completion).unwrap().unwrap();
    let live = session.document(&id).unwrap();
    assert!(!live.is_dirty() && live.is_layout_dirty());
    let Document::Scenario(scenario) = live.model().unwrap() else {
        panic!()
    };
    assert_eq!(scenario.graph.nodes[0].position, moved);
    assert!(session.undo(&id, Origin(2)).unwrap());
    let Document::Scenario(scenario) = session.document(&id).unwrap().model().unwrap() else {
        panic!()
    };
    assert_eq!(scenario.graph.nodes[0].position, previous);
    assert!(session.redo(&id, Origin(2)).unwrap());
}

#[test]
fn recovery_completion_cannot_mark_unsaved_settings_clean_after_secret_invalidation() {
    let (_directory, mut session, _) = project();
    session
        .edit_configuration(|manifest| manifest.name = "Unsaved".into(), Origin(1))
        .unwrap();
    let snapshot = session.storage_snapshot();
    session.invalidate_runtime_inputs(Origin(1));
    let completion = snapshot.run(StorageMode::PreserveEdits, ProjectSession::save_recovery);
    session.complete_storage(completion).unwrap().unwrap();
    assert!(session.is_manifest_dirty());
    assert_eq!(session.manifest().name, "Unsaved");
    assert!(
        session
            .documents()
            .iter()
            .all(|document| document.analysis() == AnalysisState::Pending)
    );
}

#[test]
fn saving_settings_acknowledges_the_saved_public_context_while_retaining_newer_settings() {
    let (directory, mut session, _) = project();
    session
        .edit_configuration(|manifest| manifest.name = "First".into(), Origin(1))
        .unwrap();
    let snapshot = session.storage_snapshot();
    session
        .edit_configuration(|manifest| manifest.name = "Second".into(), Origin(1))
        .unwrap();
    let completion = snapshot.run(
        StorageMode::PreserveEdits,
        ProjectSession::save_configuration,
    );
    session.complete_storage(completion).unwrap().unwrap();
    assert!(session.is_manifest_dirty());
    assert_eq!(session.manifest().name, "Second");
    let saved = ProjectManifest::from_bytes(
        &fs::read(directory.path().join("apiforge.project.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved.name, "First");
    session.save_configuration().unwrap();
    let saved = ProjectManifest::from_bytes(
        &fs::read(directory.path().join("apiforge.project.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved.name, "Second");
}

#[test]
fn late_storage_completion_cannot_replace_a_reopened_session() {
    let (directory, mut old, id) = project();
    old.edit_source(&id, OLDER, Origin(1)).unwrap();
    let snapshot = old.storage_snapshot();
    drop(old);
    let mut reopened = ProjectSession::open(directory.path()).unwrap();
    assert!(
        reopened.is_read_only(),
        "the in-flight worker retains writer ownership"
    );
    let completion = snapshot.run(StorageMode::PreserveEdits, |candidate| candidate.save(&id));
    assert!(matches!(
        reopened.complete_storage(completion),
        Err(Error::StaleStorage)
    ));
    assert_eq!(reopened.document(&id).unwrap().text(), SAVED);
}

#[test]
fn a_destructive_result_cannot_replace_memory_changed_since_its_start() {
    let (directory, mut session, id) = project();
    let snapshot = session.storage_snapshot();
    session.edit_source(&id, NEWER, Origin(1)).unwrap();
    let completion = snapshot.run(StorageMode::Exclusive, |candidate| {
        candidate.create_document(DocumentKind::Scenario, "other.afs", SAVED)
    });
    assert!(matches!(
        session.complete_storage(completion),
        Err(Error::StaleStorage)
    ));
    assert_eq!(session.document(&id).unwrap().text(), NEWER);
    let changes = session.external_check().run().unwrap();
    session.apply_external(changes).unwrap();
    assert_eq!(session.registry_state(), RegistryState::Changed);
    session.reload_project().unwrap();
    assert_eq!(session.document(&id).unwrap().text(), NEWER);
    assert!(directory.path().join("other.afs").exists());
}

#[test]
fn a_text_open_can_complete_beside_newer_source_edits() {
    let (directory, mut session, id) = project();
    fs::write(directory.path().join("notes.txt"), "exact\r\n").unwrap();
    let snapshot = session.storage_snapshot();
    session.edit_source(&id, NEWER, Origin(1)).unwrap();
    let completion = snapshot.run(StorageMode::PreserveEdits, |candidate| {
        candidate.open_text_file("notes.txt").map(|_| ())
    });
    session.complete_storage(completion).unwrap().unwrap();
    assert_eq!(session.document(&id).unwrap().text(), NEWER);
    assert_eq!(session.text_file("notes.txt").unwrap().text(), "exact\r\n");
}

#[test]
fn an_older_text_save_does_not_clean_or_replace_newer_exact_text() {
    let (directory, mut session, _) = project();
    fs::write(directory.path().join("notes.txt"), "saved\r\n").unwrap();
    session.open_text_file("notes.txt").unwrap();
    session
        .edit_text("notes.txt", "older\r\n", Origin(1))
        .unwrap();
    let snapshot = session.storage_snapshot();
    session
        .edit_text("notes.txt", "newer\r\n", Origin(1))
        .unwrap();
    let completion = snapshot.run(StorageMode::PreserveEdits, |candidate| {
        candidate.save_text_file("notes.txt")
    });
    session.complete_storage(completion).unwrap().unwrap();
    assert_eq!(session.text_file("notes.txt").unwrap().text(), "newer\r\n");
    assert!(session.text_file("notes.txt").unwrap().is_dirty());
    assert_eq!(
        fs::read(directory.path().join("notes.txt")).unwrap(),
        b"older\r\n"
    );
}
