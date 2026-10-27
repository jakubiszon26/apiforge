//! Stage II cross-package acceptance at library level: durable state that one
//! package writes and another reads back after a restart.
//!
//! The per-package suites each restart a session to check their own records.
//! These check the records together, which is where a package that writes one
//! file and leaves another unwritten becomes visible.
use apiforge_project::*;
use std::path::Path;

const SOURCE: &str = "scenario \"Main\" {\n  answer = 1\n  checked = assert answer == 1\n}\n";

fn scenario_node_ids(root: &Path, path: &str) -> Vec<String> {
    let session = ProjectSession::open_saved_read_only(root).unwrap();
    let document = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} is registered"));
    let Some(Document::Scenario(scenario)) = document.model() else {
        panic!("{path} has no scenario model")
    };
    scenario
        .graph
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect()
}

/// ADR 0010: "Shared checkpoints describe saved valid source." A document
/// created through a session is saved valid source from the moment it is
/// written, and `prepare_project` seals a checkpoint for every document it
/// creates. Without one, every reload allocates fresh block identities, so
/// stored layout, diagram selections and run-report step IDs refer to blocks
/// that no longer exist.
#[test]
fn a_created_document_keeps_its_block_identities_when_the_project_is_reopened() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Created").unwrap();
    session
        .create_document(DocumentKind::Scenario, "main.afs", SOURCE)
        .unwrap();
    drop(session);

    let first = scenario_node_ids(directory.path(), "main.afs");
    assert_eq!(first.len(), 2);
    assert_eq!(
        first,
        scenario_node_ids(directory.path(), "main.afs"),
        "the same saved source must lower to the same identities"
    );
}

/// The same contract for the two ways an existing file becomes a document.
/// Registration reads the file it is given, so the checkpoint it would seal is
/// derived from bytes that are already on disk, exactly as for a created
/// document.
#[test]
#[ignore = "registration and import seal no identity checkpoint; whether they may write shared diagram.json is the owner's decision (see the final report)"]
fn a_registered_or_imported_document_keeps_its_block_identities_when_the_project_is_reopened() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("found.afs"), SOURCE).unwrap();
    let registration = Registration {
        path: "found.afs".into(),
        kind: DocumentKind::Scenario,
    };
    let session =
        ProjectSession::initialise(directory.path(), "Registered", &[registration]).unwrap();
    drop(session);
    let initialised = scenario_node_ids(directory.path(), "found.afs");
    assert_eq!(
        initialised,
        scenario_node_ids(directory.path(), "found.afs"),
        "initialisation registered the source without sealing its identity"
    );

    let mut session = ProjectSession::open(directory.path()).unwrap();
    let incoming = directory.path().join("incoming.afs");
    std::fs::write(&incoming, SOURCE).unwrap();
    let candidates = session.prepare_import(&[incoming], "").unwrap();
    let items: Vec<_> = candidates
        .into_iter()
        .map(|candidate| ImportItem {
            text: candidate.text,
            kind: DocumentKind::Scenario,
            destination: candidate.destination,
            handle: None,
        })
        .collect();
    let imported = session.import(&items, Origin::SESSION).unwrap();
    let path = session
        .document(&imported[0])
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();
    drop(session);
    let first = scenario_node_ids(directory.path(), &path);
    assert_eq!(
        first,
        scenario_node_ids(directory.path(), &path),
        "import copied the source without sealing its identity"
    );
}

/// A prepared project, as the window opens one. An explorer file operation and
/// a source save are separate packages (P5 and P1) over one shared checkpoint
/// file, so the file operation has to leave the session able to save.
fn prepared_demo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    assert!(
        directory.path().join(".apiforge/diagram.json").is_file(),
        "a prepared project has shared checkpoints"
    );
    directory
}

fn only(session: &ProjectSession, kind: DocumentKind) -> String {
    session
        .documents()
        .iter()
        .find(|document| document.entry().kind == kind)
        .expect("the demo has one of each")
        .id()
        .to_owned()
}

/// A trash transaction rewrites the shared checkpoint file to carry the
/// records of the documents it moves. A session that kept its earlier copy
/// rejects its own next source save as a foreign change.
#[test]
fn deleting_a_file_into_the_trash_still_allows_saving_another_document() {
    let directory = prepared_demo();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let scenario = only(&session, DocumentKind::Scenario);
    let component_path = session
        .document(&only(&session, DocumentKind::Component))
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();

    let preview = session.preview_delete(&[&component_path]).unwrap();
    session.delete(&preview, Origin::SESSION).unwrap();

    let edited = format!("{}\n", session.document(&scenario).unwrap().text());
    session
        .edit_source(&scenario, &edited, Origin::SESSION)
        .unwrap();
    session.settle(&scenario).unwrap();
    let outcome = session.save(&scenario).expect("the save must not conflict");
    assert!(outcome.checkpoint);
    assert_eq!(
        std::fs::read_to_string(
            directory
                .path()
                .join("scenarios/promotion-and-cart-price.afs")
        )
        .unwrap(),
        edited
    );
}

/// The trash preserves each deleted document's checkpoint precisely so that a
/// restore returns its blocks, not just its bytes.
#[test]
fn restoring_a_deleted_document_brings_back_its_block_identities() {
    let directory = prepared_demo();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let scenario = only(&session, DocumentKind::Scenario);
    let path = session
        .document(&scenario)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();
    let Some(Document::Scenario(before)) = session.document(&scenario).unwrap().model() else {
        panic!("scenario")
    };
    let identities: Vec<_> = before
        .graph
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();
    let positions: Vec<_> = before
        .graph
        .nodes
        .iter()
        .map(|node| node.position.clone())
        .collect();

    let preview = session.preview_delete(&[&path]).unwrap();
    let commit = session.delete(&preview, Origin::SESSION).unwrap();
    session.restore(&commit.operation_id, &[]).unwrap();

    let Some(Document::Scenario(after)) = session.document(&scenario).unwrap().model() else {
        panic!("the document is registered again")
    };
    assert_eq!(
        after
            .graph
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>(),
        identities
    );
    assert_eq!(
        after
            .graph
            .nodes
            .iter()
            .map(|node| node.position.clone())
            .collect::<Vec<_>>(),
        positions,
        "the restored layout is the layout the trash kept"
    );
}

/// Publication, project configuration and the trash each have their own
/// durable records. A project that used all three has to reopen consistent in
/// all of them at once: a revision that is still immutable, settings that are
/// still saved, and a restored file that is a document again.
#[test]
fn a_publication_settings_change_and_trash_restore_survive_one_restart_together() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Composite").unwrap();
    let component = session
        .create_document(
            DocumentKind::Component,
            "components/helper.afs",
            "component Helper \"Helper\" {\n  export result: number = 2\n}\n",
        )
        .unwrap();
    let published = session.publish(&component, Origin::SESSION).unwrap();
    assert_eq!(published.revision, 1);
    let revision_bytes = std::fs::read(
        directory
            .path()
            .join(".apiforge/revisions")
            .join(&published.component_id)
            .join("1.afs"),
    )
    .unwrap();

    let scenario = session
        .create_document(
            DocumentKind::Scenario,
            "scenarios/main.afs",
            "scenario \"Main\" {\n  call = (Helper@1())\n  two = assert call == 2\n}\n",
        )
        .unwrap();
    let spare = session
        .create_document(DocumentKind::Scenario, "scenarios/spare.afs", SOURCE)
        .unwrap();

    session
        .edit_configuration(
            |manifest| manifest.name = "Composite renamed".into(),
            Origin::SESSION,
        )
        .unwrap();
    assert!(session.is_manifest_dirty());
    session.save_configuration().unwrap();

    // Deleting the spare scenario into the trash and restoring it has to leave
    // it registered, not merely present on disk.
    let preview = session.preview_delete(&["scenarios/spare.afs"]).unwrap();
    assert_eq!(preview.documents().len(), 1);
    let commit = session.delete(&preview, Origin::SESSION).unwrap();
    assert!(session.document(&spare).is_none());
    assert!(
        session
            .trash_operations()
            .unwrap()
            .iter()
            .any(|index| index.operation_id == commit.operation_id),
        "the delete is in the trash index"
    );
    session.restore(&commit.operation_id, &[]).unwrap();
    drop(session);

    let session = ProjectSession::open_saved_read_only(directory.path()).unwrap();
    assert_eq!(session.manifest().name, "Composite renamed");
    assert_eq!(
        std::fs::read(
            directory
                .path()
                .join(".apiforge/revisions")
                .join(&published.component_id)
                .join("1.afs")
        )
        .unwrap(),
        revision_bytes,
        "a published revision is immutable"
    );
    assert!(
        session
            .context()
            .components
            .iter()
            .any(|component| component.id == published.component_id && component.revision == 1)
    );
    assert_eq!(
        session.document(&scenario).unwrap().analysis(),
        AnalysisState::Valid,
        "the pinned call still resolves"
    );
    let restored = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "scenarios/spare.afs")
        .expect("the restored file is a registered document again");
    assert_eq!(restored.text(), SOURCE);
    assert_eq!(restored.analysis(), AnalysisState::Valid);
}

/// KTLO-22 criterion 1: the folder window checks for external changes once a
/// second with an exclusive storage job that starts from a snapshot of the
/// session. A comment-loss decision taken on the live session while such a job
/// runs is about a source the job left alone, so it survives the job's
/// completion and the publication behind it goes ahead.
#[test]
fn a_comment_decision_survives_an_exclusive_job_that_left_its_document_alone() {
    let directory = prepared_demo();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let component = only(&session, DocumentKind::Component);
    let text = session.document(&component).unwrap().text().to_owned();
    session
        .edit_source(
            &component,
            &format!("// a loose note\n\n{text}"),
            Origin::SESSION,
        )
        .unwrap();
    session.settle(&component).unwrap();
    assert!(matches!(
        session.publish(&component, Origin::SESSION),
        Err(Error::CommentsWouldBeLost { comments: 1 })
    ));

    let revisions = session.manifest().revisions.len();

    let snapshot = session.storage_snapshot();
    session.accept_comment_loss(&component).unwrap();
    let completion = snapshot.run(StorageMode::Exclusive, |_| ());
    session.complete_storage(completion).unwrap();

    session
        .publish(&component, Origin::SESSION)
        .expect("the decision taken during the job still holds");
    assert_eq!(session.manifest().revisions.len(), revisions + 1);
}

/// KTLO-22 criterion 1, for the other per-document decision an exclusive job
/// used to overwrite: where the caret and the selection are, in a document and
/// in a plain text file.
#[test]
fn a_selection_survives_an_exclusive_job_that_left_its_file_alone() {
    let directory = prepared_demo();
    std::fs::write(directory.path().join("README.md"), "# Shop\n\nNotes.\n").unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let component = only(&session, DocumentKind::Component);
    session.open_text_file("README.md").unwrap();

    let snapshot = session.storage_snapshot();
    let in_document = apiforge_storage::project::TextSelection {
        anchor: 4,
        caret: 1,
    };
    let in_text = apiforge_storage::project::TextSelection {
        anchor: 8,
        caret: 14,
    };
    session
        .set_selection(&component, in_document.clone())
        .unwrap();
    session
        .set_text_selection("README.md", in_text.clone())
        .unwrap();
    let completion = snapshot.run(StorageMode::Exclusive, |_| ());
    session.complete_storage(completion).unwrap();

    assert_eq!(
        session.document(&component).unwrap().selection(),
        &in_document
    );
    assert_eq!(
        session.text_file("README.md").unwrap().selection(),
        &in_text
    );
}
