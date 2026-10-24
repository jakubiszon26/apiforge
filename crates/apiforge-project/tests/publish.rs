//! Component publication: the next revision, its immutability, what it does
//! and does not change, and the decisions it requires first.
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

fn project() -> (tempfile::TempDir, ProjectSession) {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &shop()).unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    (directory, session)
}

/// A working component document, with the entity it publishes revisions of.
fn component(session: &ProjectSession) -> (String, String) {
    let document = session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Component)
        .expect("the shop has components");
    (document.id().to_owned(), document.entry().entity_id.clone())
}

/// The handle a component source declares.
fn handle(session: &ProjectSession, id: &str) -> String {
    session
        .document(id)
        .unwrap()
        .text()
        .split_whitespace()
        .nth(1)
        .expect("component <handle>")
        .to_owned()
}

fn scenario(session: &ProjectSession) -> String {
    session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Scenario)
        .expect("a scenario")
        .id()
        .to_owned()
}

fn published(session: &ProjectSession, entity: &str) -> Vec<u32> {
    let mut revisions: Vec<u32> = session
        .manifest()
        .revisions
        .iter()
        .filter(|entry| entry.component_id == entity)
        .map(|entry| entry.revision)
        .collect();
    revisions.sort_unstable();
    revisions
}

#[test]
fn publishing_creates_the_next_revision_and_leaves_existing_instances_alone() {
    let (directory, mut session) = project();
    let (id, entity) = component(&session);
    let scenario_id = scenario(&session);
    let before = published(&session, &entity);
    let pinned = session.document(&scenario_id).unwrap().text().to_owned();
    let next = *before.last().unwrap() + 1;
    assert_eq!(session.publishable(&id).unwrap(), next);

    // Change the working component, then publish it.
    let text = session.document(&id).unwrap().text().to_owned();
    let edited = text.replacen('"', "\"Published ", 1);
    session.edit_source(&id, &edited, Origin(1)).unwrap();
    session.settle(&id).unwrap();
    let outcome = session.publish(&id, Origin(1)).unwrap();
    assert_eq!(outcome.revision, next);
    assert_eq!(outcome.component_id, entity);

    // The registry, the immutable source and the working document's base.
    let mut after = before.clone();
    after.push(next);
    assert_eq!(published(&session, &entity), after);
    let revision_path = directory
        .path()
        .join(format!(".apiforge/revisions/{entity}/{next}.afs"));
    assert!(revision_path.is_file(), "the revision source is written");
    let entry = session
        .manifest()
        .documents
        .iter()
        .find(|entry| entry.document_id == id)
        .unwrap();
    assert_eq!(entry.base_revision, Some(next));
    assert!(
        !session.document(&id).unwrap().is_dirty(),
        "publishing saves the working text it published"
    );
    assert_eq!(session.document(&id).unwrap().text(), edited);

    // A scenario pinned to an earlier revision is untouched.
    assert_eq!(session.document(&scenario_id).unwrap().text(), pinned);
    session.settle(&scenario_id).unwrap();
    assert_eq!(
        session.document(&scenario_id).unwrap().analysis(),
        AnalysisState::Valid,
        "an existing instance keeps resolving to its own revision"
    );

    // The new revision resolves, and only after the commit.
    assert!(
        session
            .context()
            .components
            .iter()
            .any(|component| component.id == entity && component.revision == next)
    );
    drop(session);
    let session = ProjectSession::open(directory.path()).unwrap();
    assert_eq!(published(&session, &entity), after);
    assert!(
        session
            .context()
            .components
            .iter()
            .any(|component| component.id == entity && component.revision == next)
    );
}

#[test]
fn a_published_revision_is_immutable_and_its_working_document_keeps_editing() {
    let (directory, mut session) = project();
    let (id, entity) = component(&session);
    let next = session.publishable(&id).unwrap();
    session.publish(&id, Origin(1)).unwrap();
    let path = directory
        .path()
        .join(format!(".apiforge/revisions/{entity}/{next}.afs"));
    let source = std::fs::read_to_string(&path).unwrap();

    // Editing and publishing again writes the following revision instead.
    let text = session.document(&id).unwrap().text().to_owned();
    session
        .edit_source(&id, &text.replacen('"', "\"Second ", 1), Origin(1))
        .unwrap();
    session.settle(&id).unwrap();
    let outcome = session.publish(&id, Origin(1)).unwrap();
    assert_eq!(outcome.revision, next + 1);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        source,
        "an existing published file is never rewritten"
    );
    assert_eq!(published(&session, &entity).len(), 3);
}

#[test]
fn an_invalid_or_foreign_document_cannot_be_published() {
    let (directory, mut session) = project();
    let (id, _) = component(&session);
    let scenario_id = scenario(&session);
    assert!(matches!(
        session.publishable(&scenario_id),
        Err(Error::NotAComponent)
    ));
    assert!(matches!(
        session.publish(&scenario_id, Origin(1)),
        Err(Error::NotAComponent)
    ));

    let text = session.document(&id).unwrap().text().to_owned();
    session
        .edit_source(&id, &format!("{text}\nbroken ="), Origin(1))
        .unwrap();
    session.settle(&id).unwrap();
    assert!(matches!(
        session.publish(&id, Origin(1)),
        Err(Error::InvalidModel)
    ));
    assert!(
        !directory
            .path()
            .join(".apiforge/revisions")
            .join("x")
            .exists(),
        "nothing was written"
    );

    // A second session is read-only and cannot publish at all.
    let mut reader = ProjectSession::open(directory.path()).unwrap();
    assert!(reader.is_read_only());
    let (reader_id, _) = component(&reader);
    assert!(reader.publish(&reader_id, Origin(1)).is_err());
}

#[test]
fn a_published_component_cannot_be_renamed() {
    let (_directory, mut session) = project();
    let (id, _) = component(&session);
    session.publish(&id, Origin(1)).unwrap();
    let text = session.document(&id).unwrap().text().to_owned();
    let declared = handle(&session, &id);
    let renamed = rename_handle(&text, "renamed_component").unwrap();
    assert_ne!(renamed, text);
    session.edit_source(&id, &renamed, Origin(1)).unwrap();
    session.settle(&id).unwrap();
    assert!(matches!(
        session.publish(&id, Origin(1)),
        Err(Error::HandleChanged { published }) if published == declared
    ));
}

#[test]
fn an_unqualified_call_is_pinned_by_applying_its_fix() {
    let (_directory, mut session) = project();
    let scenario_id = scenario(&session);
    let text = session.document(&scenario_id).unwrap().text().to_owned();
    let (component_id, _) = component(&session);
    let called = handle(&session, &component_id);
    let pinned = format!("{called}@1(");
    assert!(text.contains(&pinned), "the fixture pins its calls");
    let unpinned = text.replacen(&pinned, &format!("{called}("), 1);
    session
        .edit_source(&scenario_id, &unpinned, Origin(1))
        .unwrap();
    session.settle(&scenario_id).unwrap();
    assert_eq!(
        session.document(&scenario_id).unwrap().analysis(),
        AnalysisState::Invalid
    );
    let which = session
        .document(&scenario_id)
        .unwrap()
        .diagnostics()
        .iter()
        .position(|diagnostic| diagnostic.fix.is_some())
        .expect("the pin fix");
    session.apply_fix(&scenario_id, which, Origin(1)).unwrap();
    session.settle(&scenario_id).unwrap();
    let document = session.document(&scenario_id).unwrap();
    assert_eq!(document.analysis(), AnalysisState::Valid);
    assert_eq!(document.text(), text, "the fix restores the explicit pin");
    assert!(
        document.source_version() > 1,
        "the fix is a versioned source edit"
    );

    // A fix is refused while the analysis that offered it is stale.
    session
        .edit_source(&scenario_id, &unpinned, Origin(1))
        .unwrap();
    assert!(matches!(
        session.apply_fix(&scenario_id, 0, Origin(1)),
        Err(Error::StaleAnalysis)
    ));
}

#[test]
fn comments_that_belong_to_no_statement_need_a_decision_before_a_reprint() {
    let (_directory, mut session) = project();
    let (id, _) = component(&session);
    let text = session.document(&id).unwrap().text().to_owned();
    let with_comment = format!("// a loose note\n\n{text}");
    session.edit_source(&id, &with_comment, Origin(1)).unwrap();
    session.settle(&id).unwrap();
    assert_eq!(
        session.unattached_comments(&id).unwrap(),
        ["// a loose note"]
    );

    // A graphical edit and a publication both refuse until a decision is made,
    // and the original text stays exactly as it is.
    let model = session.document(&id).unwrap().model().cloned().unwrap();
    let Document::Component(mut component) = model else {
        panic!("component")
    };
    component.description = "changed".into();
    assert!(matches!(
        session.apply_model_edit(&id, Document::Component(component.clone()), Origin(1)),
        Err(Error::CommentsWouldBeLost { comments: 1 })
    ));
    assert!(matches!(
        session.publish(&id, Origin(1)),
        Err(Error::CommentsWouldBeLost { comments: 1 })
    ));
    assert_eq!(session.document(&id).unwrap().text(), with_comment);

    session.accept_comment_loss(&id).unwrap();
    session
        .apply_model_edit(&id, Document::Component(component), Origin(1))
        .unwrap();
    assert!(
        !session
            .document(&id)
            .unwrap()
            .text()
            .contains("a loose note"),
        "the accepted reprint drops it"
    );

    // A new unattached comment is a new decision.
    let reprinted = session.document(&id).unwrap().text().to_owned();
    session
        .edit_source(&id, &format!("// another\n\n{reprinted}"), Origin(1))
        .unwrap();
    session.settle(&id).unwrap();
    assert!(matches!(
        session.publish(&id, Origin(1)),
        Err(Error::CommentsWouldBeLost { comments: 1 })
    ));
    session.accept_comment_loss(&id).unwrap();
    session.publish(&id, Origin(1)).unwrap();
}

#[test]
fn a_recovered_draft_publishes_after_the_maximum_without_rebasing_the_working_copy() {
    let (directory, session) = project();
    let (id, entity) = component(&session);
    let primary = session.document(&id).unwrap().text().to_owned();
    let next = session.publishable(&id).unwrap();

    let mut manifest = session.manifest().clone();
    let mut draft = session.document(&id).unwrap().entry().clone();
    draft.document_id = "recovered-component".into();
    draft.path = apiforge_storage::project::ProjectPath::working_source("recovered.afs").unwrap();
    draft.recovered = true;
    let recovered = draft.document_id.clone();
    manifest.documents.push(draft);
    drop(session);
    let recovered_text = primary.replacen('"', "\"Recovered ", 1);
    std::fs::write(directory.path().join("recovered.afs"), &recovered_text).unwrap();
    std::fs::write(
        directory.path().join("apiforge.project.json"),
        manifest.to_public_bytes().unwrap(),
    )
    .unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();

    session.publish(&id, Origin(1)).unwrap();
    assert_eq!(session.publishable(&id).unwrap(), next + 1);
    assert_eq!(
        session.document(&recovered).unwrap().entry().base_revision,
        Some(1)
    );
    assert_eq!(
        session.publish(&recovered, Origin(1)).unwrap().revision,
        next + 1
    );
    assert_eq!(
        session.document(&id).unwrap().text(),
        primary,
        "publishing another document never rewrites this one"
    );
    assert_eq!(published(&session, &entity), vec![1, next, next + 1]);
    assert_eq!(
        session.document(&id).unwrap().entry().base_revision,
        Some(next)
    );
    assert_eq!(
        session.document(&recovered).unwrap().entry().base_revision,
        Some(next + 1)
    );
    assert_eq!(session.document(&recovered).unwrap().text(), recovered_text);
    drop(session);
    let reopened = ProjectSession::open(directory.path()).unwrap();
    assert_eq!(
        reopened.document(&id).unwrap().entry().base_revision,
        Some(next)
    );
    assert_eq!(
        reopened.document(&recovered).unwrap().entry().base_revision,
        Some(next + 1)
    );
}

#[test]
fn publication_checks_unchanged_working_source_and_preserves_foreign_bytes() {
    let (directory, mut session) = project();
    let (id, entity) = component(&session);
    let revision = session.publishable(&id).unwrap();
    let path = directory
        .path()
        .join(session.document(&id).unwrap().entry().path.as_str());
    std::fs::write(&path, "foreign = bytes").unwrap();
    assert!(session.publish(&id, Origin(1)).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "foreign = bytes");
    assert!(
        !directory
            .path()
            .join(format!(".apiforge/revisions/{entity}/{revision}.afs"))
            .exists()
    );
    assert!(
        !session
            .context()
            .components
            .iter()
            .any(|c| c.id == entity && c.revision == revision)
    );
}

#[test]
fn comment_decisions_expire_on_any_source_edit_but_layout_does_not_reprint() {
    let (_directory, mut session) = project();
    let (id, _) = component(&session);
    let text = format!("// loose\n\n{}", session.document(&id).unwrap().text());
    session.edit_source(&id, &text, Origin(1)).unwrap();
    session.settle(&id).unwrap();
    let Document::Component(mut model) = session.document(&id).unwrap().model().unwrap().clone()
    else {
        panic!()
    };
    model.graph.nodes[0].position.x += 100.;
    session
        .apply_model_edit(&id, Document::Component(model), Origin(1))
        .unwrap();
    assert_eq!(session.document(&id).unwrap().text(), text);
    session.accept_comment_loss(&id).unwrap();
    session
        .edit_source(&id, &format!("{text}\n"), Origin(1))
        .unwrap();
    session.settle(&id).unwrap();
    assert!(matches!(
        session.publish(&id, Origin(1)),
        Err(Error::CommentsWouldBeLost { .. })
    ));
    assert!(
        matches!(
            session.undo(&id, Origin(1)),
            Err(Error::CommentsWouldBeLost { .. })
        ) || session.document(&id).unwrap().text().contains("// loose")
    );
}

#[test]
fn a_new_component_can_publish_and_checkpoints_keep_its_identity_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Components").unwrap();
    let source = "component Fresh \"Fresh\" {\n  answer = 7\n}\n";
    let id = session
        .create_document(DocumentKind::Component, "fresh.afs", source)
        .unwrap();
    let model = session.document(&id).unwrap().model().unwrap().clone();
    let result = session.publish(&id, Origin(1)).unwrap();
    assert_eq!(result.revision, 1);
    let Document::Component(component) = model else {
        panic!()
    };
    drop(session);
    let session = ProjectSession::open(directory.path()).unwrap();
    let revision = session
        .context()
        .components
        .iter()
        .find(|c| c.id == component.id && c.revision == 1)
        .unwrap();
    assert_eq!(revision.graph, component.graph);
    assert_eq!(
        session.document(&id).unwrap().entry().base_revision,
        Some(1)
    );
}
