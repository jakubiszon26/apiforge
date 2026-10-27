use apiforge_project::*;
use apiforge_storage::project::FieldRecovery;

fn project() -> (tempfile::TempDir, ProjectSession, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Runs").unwrap();
    let id = session
        .create_document(
            DocumentKind::Scenario,
            "main.afs",
            "scenario \"Main\" {\n  answer = 1\n}\n",
        )
        .unwrap();
    (directory, session, id)
}

#[test]
fn run_preparation_settles_unsaved_code_and_keeps_the_running_snapshot_immutable() {
    let (_directory, mut session, id) = project();
    let source = session.document(&id).unwrap().text().replace('1', "2");
    session.edit_source(&id, &source, Origin::SESSION).unwrap();
    let prepared = session.prepare_run(&id).unwrap();
    assert!(!prepared.last_valid);
    assert!(prepared.unsaved);
    let project = prepared.project().unwrap();
    session
        .edit_source(&id, &source.replace('2', "3"), Origin::SESSION)
        .unwrap();
    assert!(session.validate_run(&prepared).is_err());
    assert_eq!(prepared.project().unwrap(), project);
    assert!(prepared.snapshot.source().contains("answer = 2"));
}

#[test]
fn last_valid_confirmation_is_bound_to_source_configuration_fields_and_session() {
    let (directory, mut session, id) = project();
    let earlier = session.context().clone();
    session
        .edit_source(&id, "scenario \"Main\" { value = }", Origin::SESSION)
        .unwrap();
    let prepared = session.prepare_run(&id).unwrap();
    assert!(prepared.last_valid);
    session.validate_run(&prepared).unwrap();
    session
        .edit_configuration(
            |manifest| manifest.environment.base_url = "https://new.example".into(),
            Origin::SESSION,
        )
        .unwrap();
    assert!(session.validate_run(&prepared).is_err());
    let changed = session.prepare_run(&id).unwrap();
    assert_eq!(
        changed.snapshot.context().environment,
        earlier.environment,
        "the earlier graph keeps its earlier configuration"
    );
    session
        .set_field_drafts(
            &id,
            vec![FieldRecovery {
                widget: 1,
                base_hash: "0".repeat(64),
                text: "unfinished".into(),
                selection: apiforge_storage::project::TextSelection {
                    anchor: 0,
                    caret: 0,
                },
            }],
        )
        .unwrap();
    assert!(session.validate_run(&changed).is_err());
    session.discard_configuration().unwrap();
    drop(session);
    let mut reopened = ProjectSession::open(directory.path()).unwrap();
    assert!(reopened.validate_run(&prepared).is_err());
}

#[test]
fn invalid_source_requires_a_verified_candidate_and_conflicts_never_run() {
    let (directory, mut session, id) = project();
    session.edit_source(&id, "broken", Origin::SESSION).unwrap();
    session.save(&id).unwrap();
    drop(session);
    let mut session = ProjectSession::open(directory.path()).unwrap();
    assert!(
        session.prepare_run(&id).is_err(),
        "no verified cache means no run"
    );
    session
        .edit_source(
            &id,
            "scenario \"Fixed\" {\n  answer = 7\n}\n",
            Origin::SESSION,
        )
        .unwrap();
    session.settle(&id).unwrap();
    std::fs::write(directory.path().join("main.afs"), "foreign").unwrap();
    let check = session.external_check().run().unwrap();
    session.apply_external(check).unwrap();
    assert!(session.prepare_run(&id).is_err());
}

#[test]
fn a_cached_candidate_retains_its_earlier_configuration_after_restart() {
    let (directory, mut session, id) = project();
    let earlier = session.context().environment.clone();
    session.save_last_valid().unwrap();
    session
        .edit_source(&id, "invalid", Origin::SESSION)
        .unwrap();
    session.save(&id).unwrap();
    session
        .edit_configuration(
            |manifest| manifest.environment.base_url = "https://new.example".into(),
            Origin::SESSION,
        )
        .unwrap();
    session.save_configuration().unwrap();
    drop(session);
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let prepared = session.prepare_run(&id).unwrap();
    assert!(prepared.last_valid);
    assert_eq!(prepared.snapshot.context().environment, earlier);
    assert_ne!(
        prepared.snapshot.context().environment,
        session.context().environment
    );
}

#[test]
fn unsaved_export_keeps_exact_source_fields_and_public_dirty_settings() {
    let (_directory, mut session, id) = project();
    let text = "// exact\r\ninvalid source\r\n";
    session.edit_source(&id, text, Origin::SESSION).unwrap();
    session
        .edit_configuration(
            |manifest| {
                manifest.name = "Unsaved project name".into();
                manifest.environment.secret_names.push("token".into());
                manifest
                    .environment
                    .variables
                    .insert("token".into(), "session-secret".into());
            },
            Origin::SESSION,
        )
        .unwrap();
    let bytes = session.export_unsaved().unwrap();
    let export: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(export["project"]["name"], "Unsaved project name");
    assert_eq!(export["recovery"]["documents"][0]["text"], text);
    assert!(!String::from_utf8(bytes).unwrap().contains("session-secret"));
    assert!(session.document(&id).unwrap().is_dirty());
    assert!(session.is_manifest_dirty());
}

/// KTLO-23a criteria 2–4: a component document prepares a run of its working
/// text, saved or not, with the published revisions it calls; nothing on disk
/// changes.
#[test]
fn ktlo23_a_component_prepares_a_run_of_its_working_text_and_its_pinned_calls() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Components").unwrap();
    let helper = session
        .create_document(
            DocumentKind::Component,
            "components/helper.afs",
            "component Helper \"Helper\" {\n  export result: number = 2\n}\n",
        )
        .unwrap();
    session.publish(&helper, Origin::SESSION).unwrap();
    let outer = session
        .create_document(
            DocumentKind::Component,
            "components/outer.afs",
            "component Outer \"Outer\" {\n  call = (Helper@1())\n  export inner: number = call.result\n}\n",
        )
        .unwrap();
    let saved = std::fs::read(directory.path().join("components/helper.afs")).unwrap();
    let edited = session
        .document(&helper)
        .unwrap()
        .text()
        .replace("= 2", "= 3");
    session
        .edit_source(&helper, &edited, Origin::SESSION)
        .unwrap();

    let prepared = session.prepare_run(&helper).unwrap();
    assert!(!prepared.last_valid);
    assert!(prepared.unsaved);
    let project = prepared.project().unwrap();
    assert!(project.scenarios.is_empty(), "a component runs on its own");
    let own: Vec<_> = project
        .components
        .iter()
        .filter(|component| component.handle == "Helper")
        .collect();
    assert_eq!(own.len(), 1, "only the working text of the helper runs");
    assert_eq!(
        own[0].revision, 2,
        "its next revision, unused by revision 1"
    );
    assert!(
        serde_json::to_string(&own[0].graph)
            .unwrap()
            .contains(":3}"),
        "the unsaved value"
    );

    let project = session.prepare_run(&outer).unwrap().project().unwrap();
    let mut revisions: Vec<(String, u32)> = project
        .components
        .iter()
        .map(|component| (component.handle.clone(), component.revision))
        .collect();
    revisions.sort();
    assert_eq!(
        revisions,
        [("Helper".to_owned(), 1), ("Outer".to_owned(), 1)],
        "the outer component and the published revision it pins"
    );
    assert_eq!(
        std::fs::read(directory.path().join("components/helper.afs")).unwrap(),
        saved,
        "preparing a run writes nothing"
    );
}
