//! LANG-05 (#142): the short form of a compound check is the written
//! language's. A published revision keeps the stored spelling 0.1.0 wrote, so
//! publication writes `(a >= b) == true` in full and an unchanged component
//! still matches the hash of its latest revision.
use apiforge_core::{ImportLimits, format::Project, import_document};
use apiforge_project::*;

fn shop() -> Project {
    import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .into_iter()
    .find(|project| project.id == "shop")
    .unwrap()
}

#[test]
fn lang05_review_publication_keeps_the_stored_spelling_and_its_hash() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &shop()).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let document = session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Component)
        .expect("the shop has components");
    let (id, entity) = (document.id().to_owned(), document.entry().entity_id.clone());

    // The working copy is the written language: the check as typed.
    let text = session.document(&id).unwrap().text().to_owned();
    let end = text.rfind('}').unwrap();
    let edited = format!(
        "{}  lang05 = assert 2 >= 1 && 3 <= 4\n{}",
        &text[..end],
        &text[end..]
    );
    session.edit_source(&id, &edited, Origin(1)).unwrap();
    session.settle(&id).unwrap();
    assert_eq!(
        session.document(&id).unwrap().analysis(),
        AnalysisState::Valid
    );
    let published = session.publish(&id, Origin(1)).unwrap();
    assert!(
        session
            .document(&id)
            .unwrap()
            .text()
            .contains("  lang05 = assert 2 >= 1 && 3 <= 4\n"),
        "the working copy keeps the check as typed"
    );

    // The revision is written in the stored spelling, as 0.1.0 wrote it.
    let revision = std::fs::read_to_string(directory.path().join(format!(
        ".apiforge/revisions/{entity}/{}.afs",
        published.revision
    )))
    .unwrap();
    assert!(
        revision.contains("  lang05 = assert (2 >= 1 && 3 <= 4) == true\n"),
        "{revision}"
    );

    // Unchanged, the component prints as its latest revision did.
    assert_eq!(
        session.unchanged_revision(&id),
        Some(published.revision),
        "publishing again would write the same revision"
    );
    drop(session);
    let mut session = ProjectSession::open(directory.path()).unwrap();
    session.settle(&id).unwrap();
    assert_eq!(session.unchanged_revision(&id), Some(published.revision));
}
