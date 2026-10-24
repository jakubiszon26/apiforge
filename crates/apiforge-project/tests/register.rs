use apiforge_project::*;
use std::fs;

const SCENARIO: &str = "// Smoke test\nscenario \"Main\" {\n  total = 2\n}\n";
const COMPONENT: &str = "component Login \"Log in\" {\n}\n";
const BROKEN: &str = "scenario \"Broken\" {\n  total = \n";

#[test]
fn declarations_are_read_from_the_header_even_in_invalid_source() {
    assert_eq!(
        declaration(SCENARIO),
        Some(Declaration {
            kind: DocumentKind::Scenario,
            handle: None
        })
    );
    assert_eq!(
        declaration(COMPONENT),
        Some(Declaration {
            kind: DocumentKind::Component,
            handle: Some("Login".into())
        })
    );
    assert_eq!(declaration(BROKEN).unwrap().kind, DocumentKind::Scenario);
    assert_eq!(declaration("step x {}"), None);
    assert_eq!(declaration(""), None);
    assert_eq!(
        rename_handle(COMPONENT, "SignIn").unwrap(),
        "component SignIn \"Log in\" {\n}\n"
    );
    assert_eq!(rename_handle(SCENARIO, "SignIn"), None);
    assert_eq!(rename_handle(COMPONENT, "not valid"), None);
}

#[test]
fn surveying_writes_nothing_and_initialising_registers_sources_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join("flows/.git")).unwrap();
    fs::write(root.join("flows/main.afs"), SCENARIO).unwrap();
    fs::write(root.join("flows/broken.afs"), BROKEN).unwrap();
    fs::write(root.join("login.afs"), COMPONENT).unwrap();
    fs::write(root.join("mystery.afs"), "?? what\n").unwrap();
    fs::write(root.join("notes.txt"), "not a source").unwrap();
    let surveys = ProjectSession::survey(root).unwrap();
    let mut listed: Vec<_> = surveys
        .iter()
        .map(|survey| {
            (
                survey.path.as_str(),
                survey.declaration.as_ref().map(|d| d.kind),
            )
        })
        .collect();
    listed.sort_by_key(|(path, _)| *path);
    assert_eq!(
        listed,
        [
            ("flows/broken.afs", Some(DocumentKind::Scenario)),
            ("flows/main.afs", Some(DocumentKind::Scenario)),
            ("login.afs", Some(DocumentKind::Component)),
            ("mystery.afs", None),
        ]
    );
    assert!(!root.join(".apiforge").exists(), "survey writes nothing");

    let registrations = [
        ("flows/main.afs", DocumentKind::Scenario),
        ("flows/broken.afs", DocumentKind::Scenario),
        ("login.afs", DocumentKind::Component),
        // An unrecognisable header registers with the explicitly chosen kind.
        ("mystery.afs", DocumentKind::Scenario),
    ]
    .map(|(path, kind)| Registration {
        path: path.into(),
        kind,
    });
    let session = ProjectSession::initialise(root, "Existing", &registrations).unwrap();
    assert_eq!(session.documents().len(), 4);
    assert_eq!(
        fs::read_to_string(root.join("flows/main.afs")).unwrap(),
        SCENARIO
    );
    assert_eq!(
        fs::read_to_string(root.join("flows/broken.afs")).unwrap(),
        BROKEN
    );
    let main = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "flows/main.afs")
        .unwrap();
    assert!(main.model().is_some());
    let broken = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "flows/broken.afs")
        .unwrap();
    assert!(!broken.diagnostics().is_empty());
    drop(session);
    assert!(matches!(
        ProjectSession::survey(root),
        Err(Error::AlreadyAProject)
    ));
    let reopened = ProjectSession::open(root).unwrap();
    assert_eq!(reopened.documents().len(), 4);
}

#[test]
fn initialising_with_a_missing_source_writes_no_manifest() {
    let directory = tempfile::tempdir().unwrap();
    let error = ProjectSession::initialise(
        directory.path(),
        "Missing",
        &[Registration {
            path: "absent.afs".into(),
            kind: DocumentKind::Scenario,
        }],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Storage(ProjectError::Conflict { .. })
    ));
    assert!(!directory.path().join("apiforge.project.json").exists());
}

#[test]
fn import_copies_sources_with_new_identity_and_resolves_handle_collisions() {
    let outside = tempfile::tempdir().unwrap();
    let main = outside.path().join("main.afs");
    let login = outside.path().join("login.afs");
    let login_again = outside.path().join("copy.afs");
    fs::write(&main, SCENARIO).unwrap();
    fs::write(&login, COMPONENT).unwrap();
    fs::write(&login_again, COMPONENT).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Imports").unwrap();
    session
        .create_document(DocumentKind::Scenario, "flows/main.afs", SCENARIO)
        .unwrap();

    let candidates = session
        .prepare_import(&[main.clone(), login.clone(), login_again.clone()], "flows")
        .unwrap();
    let destinations: Vec<_> = candidates
        .iter()
        .map(|candidate| candidate.destination.as_str())
        .collect();
    assert_eq!(
        destinations,
        ["flows/main-2.afs", "flows/login.afs", "flows/copy.afs"]
    );
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.handle_collision)
            .collect::<Vec<_>>(),
        [false, false, true]
    );

    let items: Vec<_> = candidates
        .iter()
        .map(|candidate| ImportItem {
            text: candidate.text.clone(),
            kind: candidate.declaration.as_ref().unwrap().kind,
            destination: candidate.destination.clone(),
            handle: None,
        })
        .collect();
    assert!(matches!(
        session.import(&items, Origin::SESSION),
        Err(Error::HandleCollision(handle)) if handle == "Login"
    ));
    assert!(!directory.path().join("flows/login.afs").exists());

    let mut items = items;
    items[2].handle = Some("LoginCopy".into());
    let ids = session.import(&items, Origin::SESSION).unwrap();
    assert_eq!(ids.len(), 3);
    assert_eq!(
        fs::read_to_string(directory.path().join("flows/copy.afs")).unwrap(),
        "component LoginCopy \"Log in\" {\n}\n"
    );
    assert_eq!(fs::read_to_string(&login_again).unwrap(), COMPONENT);
    let mut entities: Vec<_> = session
        .documents()
        .iter()
        .map(|document| document.entry().entity_id.clone())
        .collect();
    entities.sort();
    entities.dedup();
    assert_eq!(entities.len(), 4, "every copy has its own identity");
    assert!(
        session.manifest().revisions.is_empty(),
        "nothing is published"
    );

    // The same destinations are now occupied; nothing is overwritten.
    let error = session.import(&items[..1], Origin::SESSION).unwrap_err();
    assert!(matches!(
        error,
        Error::Storage(ProjectError::Conflict { .. })
    ));
    drop(session);
    assert_eq!(
        ProjectSession::open(directory.path())
            .unwrap()
            .documents()
            .len(),
        4
    );
}

#[test]
fn import_refuses_binary_files_and_kind_mismatches() {
    let outside = tempfile::tempdir().unwrap();
    let binary = outside.path().join("binary.afs");
    fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Imports").unwrap();
    assert!(matches!(
        session.prepare_import(&[binary], ""),
        Err(Error::NotText(_))
    ));
    let error = session
        .import(
            &[ImportItem {
                text: SCENARIO.into(),
                kind: DocumentKind::Component,
                destination: "x.afs".into(),
                handle: None,
            }],
            Origin::SESSION,
        )
        .unwrap_err();
    assert!(matches!(error, Error::KindMismatch));
    assert!(session.documents().is_empty());
}
