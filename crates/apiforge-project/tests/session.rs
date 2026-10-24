use apiforge_core::format::Position;
use apiforge_project::*;

const SCENARIO: &str = "scenario \"Main\" {\n  total = 2\n}\n";
const COMPONENT: &str = "component Helper \"Helper\" {\n  export result: number = 2\n}\n";

fn project() -> (tempfile::TempDir, ProjectSession) {
    let directory = tempfile::tempdir().unwrap();
    let session = ProjectSession::create(directory.path(), "Folder").unwrap();
    (directory, session)
}

#[test]
fn a_folder_without_a_manifest_is_never_written_by_opening_it() {
    let directory = tempfile::tempdir().unwrap();
    assert!(matches!(
        ProjectSession::open(directory.path()),
        Err(Error::NotAProject)
    ));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    ProjectSession::create(directory.path(), "Folder").unwrap();
    assert!(matches!(
        ProjectSession::create(directory.path(), "Again"),
        Err(Error::AlreadyAProject)
    ));
}

#[test]
fn empty_and_component_only_projects_are_valid_authoring_states() {
    let (directory, mut session) = project();
    assert!(session.documents().is_empty());
    let id = session
        .create_document(DocumentKind::Component, "components/helper.afs", COMPONENT)
        .unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.model_state(), ModelState::Current);
    let Some(Document::Component(component)) = document.model() else {
        panic!("component")
    };
    assert_eq!(component.id, document.entry().entity_id);
    assert_eq!(component.revision, 1, "next unpublished revision");
    session.save(&id).unwrap();
    drop(session);
    let session = ProjectSession::open(directory.path()).unwrap();
    assert_eq!(session.documents().len(), 1);
    assert_eq!(
        session.document(&id).unwrap().model_state(),
        ModelState::Current
    );
}

#[test]
fn late_results_are_rejected_after_source_context_or_session_changes() {
    let (_directory, mut session) = project();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    let view = Origin(1);

    session
        .edit_source(&id, &SCENARIO.replace('2', "3"), view)
        .unwrap();
    let late = session.analysis_for(&id).unwrap();
    session
        .edit_source(&id, &SCENARIO.replace('2', "4"), view)
        .unwrap();
    assert!(matches!(
        session.apply_analysis(late.run()),
        Err(Error::StaleAnalysis)
    ));

    // Same source, different configuration.
    let late = session.analysis_for(&id).unwrap();
    session
        .edit_configuration(
            |manifest| manifest.environment.base_url = "http://localhost".into(),
            view,
        )
        .unwrap();
    assert!(session.is_manifest_dirty());
    assert!(matches!(
        session.apply_analysis(late.run()),
        Err(Error::StaleAnalysis)
    ));

    // Reloading starts a new epoch even when nothing on disk changed.
    let late = session.analysis_for(&id).unwrap();
    session.reload().unwrap();
    assert!(matches!(
        session.apply_analysis(late.run()),
        Err(Error::StaleAnalysis)
    ));

    // A current ticket, run on a worker, applies once.
    session
        .edit_source(&id, &SCENARIO.replace('2', "5"), view)
        .unwrap();
    let current = session.pending_analyses().pop().unwrap();
    let result = std::thread::spawn(move || current.run()).join().unwrap();
    session.apply_analysis(result.clone()).unwrap();
    assert!(matches!(
        session.apply_analysis(result),
        Err(Error::StaleAnalysis)
    ));
}

#[test]
fn pending_hidden_documents_settle_and_invalid_ones_do_not_lock_others() {
    let (_directory, mut session) = project();
    let first = session
        .create_document(DocumentKind::Scenario, "scenarios/first.afs", SCENARIO)
        .unwrap();
    let second = session
        .create_document(DocumentKind::Scenario, "scenarios/second.afs", SCENARIO)
        .unwrap();
    session
        .edit_source(
            &first,
            "scenario \"Main\" {\n  total =\n}\n",
            Origin::SESSION,
        )
        .unwrap();
    assert!(!session.document(&first).unwrap().accepts_model_edits());
    assert_eq!(session.pending_analyses().len(), 1);
    session.settle_all().unwrap();
    let document = session.document(&first).unwrap();
    assert_eq!(document.analysis(), AnalysisState::Invalid);
    assert_eq!(document.model_state(), ModelState::LastValid);
    assert!(!document.diagnostics().is_empty());
    assert!(matches!(
        session.apply_model_edit(&first, document.model().unwrap().clone(), Origin::SESSION),
        Err(Error::ModelLocked)
    ));

    let Some(Document::Scenario(mut scenario)) =
        session.document(&second).unwrap().model().cloned()
    else {
        panic!("scenario")
    };
    scenario.name = "Second".into();
    session
        .apply_model_edit(&second, Document::Scenario(scenario), Origin::SESSION)
        .unwrap();
    assert!(
        session
            .document(&second)
            .unwrap()
            .text()
            .contains("\"Second\"")
    );
}

#[test]
fn model_history_projects_source_and_keeps_layout_separate() {
    let (_directory, mut session) = project();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    let original_text = session.document(&id).unwrap().text().to_owned();
    let Some(Document::Scenario(original)) = session.document(&id).unwrap().model().cloned() else {
        panic!("scenario")
    };
    let mut renamed = original.clone();
    renamed.name = "Renamed".into();
    session
        .apply_model_edit(&id, Document::Scenario(renamed.clone()), Origin::SESSION)
        .unwrap();
    let mut moved = renamed.clone();
    moved.graph.nodes[0].position = Position { x: 400.0, y: 80.0 };
    session
        .apply_model_edit(&id, Document::Scenario(moved), Origin::SESSION)
        .unwrap();
    let renamed_text = session.document(&id).unwrap().text().to_owned();
    assert!(renamed_text.contains("\"Renamed\""));

    assert!(session.undo(&id, Origin::SESSION).unwrap());
    assert_eq!(
        session.document(&id).unwrap().text(),
        renamed_text,
        "layout only"
    );
    assert!(session.undo(&id, Origin::SESSION).unwrap());
    let document = session.document(&id).unwrap();
    assert_eq!(
        document.model(),
        Some(&Document::Scenario(original.clone()))
    );
    assert_ne!(document.text(), renamed_text);
    assert!(!session.undo(&id, Origin::SESSION).unwrap());
    assert!(session.redo(&id, Origin::SESSION).unwrap());
    assert_eq!(session.document(&id).unwrap().text(), renamed_text);
    assert_ne!(original_text, renamed_text);
}

#[test]
fn every_view_observes_the_same_versions_and_its_own_origin() {
    let (_directory, mut session) = project();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    let code = Origin(1);
    let start = session.last_sequence();
    let version = session
        .edit_source(&id, &SCENARIO.replace('2', "3"), code)
        .unwrap();
    session.settle(&id).unwrap();
    let diagram_view: Vec<_> = session.events_since(start).to_vec();
    let code_view: Vec<_> = session.events_since(start).to_vec();
    assert_eq!(diagram_view, code_view);
    assert!(diagram_view.iter().any(|event| event.origin == code
        && event.kind
            == EventKind::SourceChanged {
                source_version: version
            }));
    assert!(diagram_view.iter().any(|event| event.kind
        == EventKind::ModelChanged {
            source_version: version
        }));
    assert!(
        diagram_view
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
}

#[test]
fn a_second_session_is_read_only_and_saves_nothing() {
    let (directory, mut session) = project();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    let mut second = ProjectSession::open(directory.path()).unwrap();
    assert!(second.is_read_only());
    second
        .edit_source(&id, &SCENARIO.replace('2', "3"), Origin::SESSION)
        .unwrap();
    assert!(matches!(
        second.save(&id),
        Err(Error::Storage(ProjectError::ReadOnly))
    ));
    assert_eq!(
        std::fs::read_to_string(directory.path().join("scenarios/main.afs")).unwrap(),
        SCENARIO
    );
    session.save(&id).unwrap();
}

#[test]
fn an_external_edit_is_a_save_conflict_not_an_overwrite() {
    let (directory, mut session) = project();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    let path = directory.path().join("scenarios/main.afs");
    std::fs::write(&path, SCENARIO.replace('2', "9")).unwrap();
    session
        .edit_source(&id, &SCENARIO.replace('2', "3"), Origin::SESSION)
        .unwrap();
    assert!(matches!(
        session.save(&id),
        Err(Error::Storage(ProjectError::Conflict { .. }))
    ));
    assert!(session.document(&id).unwrap().is_dirty());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        SCENARIO.replace('2', "9")
    );
}

#[test]
fn saving_pending_source_settles_it_and_seals_its_checkpoint() {
    let (directory, mut session) = project();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    let edited = SCENARIO.replace('2', "3");
    session.edit_source(&id, &edited, Origin::SESSION).unwrap();
    let saved = session.save(&id).unwrap();
    assert!(saved.checkpoint);
    let model = session.document(&id).unwrap().model().cloned();
    drop(session);
    let session = ProjectSession::open(directory.path()).unwrap();
    assert_eq!(session.document(&id).unwrap().text(), edited);
    assert_eq!(session.document(&id).unwrap().model().cloned(), model);
}
