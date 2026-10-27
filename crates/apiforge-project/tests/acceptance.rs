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

const BROKEN: &str = "scenario \"Main\" {\n  answer = \n";

type Blocks = Vec<(String, apiforge_core::format::Position)>;

/// The identity and position of every block of a scenario document.
fn blocks(document: &DocumentSession) -> Blocks {
    let Some(Document::Scenario(scenario)) = document.model() else {
        panic!("{} has no scenario model", document.entry().path.as_str())
    };
    scenario
        .graph
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.position.clone()))
        .collect()
}

/// The blocks a later load of the saved project gives the document at `path`.
fn reopened_blocks(root: &Path, path: &str) -> Blocks {
    let session = ProjectSession::open_saved_read_only(root).unwrap();
    blocks(document_at(&session, path))
}

fn document_at<'a>(session: &'a ProjectSession, path: &str) -> &'a DocumentSession {
    session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} is registered"))
}

/// Whether the Git-tracked `.apiforge/diagram.json` holds the identity record
/// of the working document `document_id`.
fn has_record(root: &Path, document_id: &str) -> bool {
    let Ok(bytes) = std::fs::read(root.join(".apiforge/diagram.json")) else {
        return false;
    };
    let file = apiforge_storage::project::DiagramFile::from_bytes(&bytes).unwrap();
    file.records.iter().any(|record| {
        record.key
            == apiforge_storage::project::DocumentKey::Working {
                document_id: document_id.into(),
            }
    })
}

fn registration(path: &str) -> Registration {
    Registration {
        path: path.into(),
        kind: DocumentKind::Scenario,
    }
}

/// Imports one scenario file with the destination the session proposes and
/// returns its document ID and project path.
fn import_one(session: &mut ProjectSession, file: &Path) -> (String, String) {
    let candidates = session.prepare_import(&[file.to_owned()], "").unwrap();
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
    (imported[0].clone(), path)
}

/// KTLO-04 (#6), criterion 1. The same contract as for a created document, for
/// the two ways an existing file becomes one at initialisation and import. The
/// owner decided on 2026-10-26 to seal the identities and the layout at the
/// document's first successful analysis; a valid source analyses during its
/// registration, so the record is there as soon as the registration returns.
#[test]
fn a_registered_or_imported_document_keeps_its_block_identities_when_the_project_is_reopened() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("found.afs"), SOURCE).unwrap();
    let session =
        ProjectSession::initialise(root, "Registered", &[registration("found.afs")]).unwrap();
    let found = document_at(&session, "found.afs");
    let (found_id, initialised) = (found.id().to_owned(), blocks(found));
    assert_eq!(initialised.len(), 2);
    assert!(
        has_record(root, &found_id),
        "initialisation analysed the source cleanly, so its record is sealed"
    );
    drop(session);
    assert_eq!(
        reopened_blocks(root, "found.afs"),
        initialised,
        "initialisation registered the source without sealing its identity"
    );

    let mut session = ProjectSession::open(root).unwrap();
    let incoming = root.join("incoming.afs");
    std::fs::write(&incoming, SOURCE).unwrap();
    let (imported_id, path) = import_one(&mut session, &incoming);
    let imported = blocks(session.document(&imported_id).unwrap());
    assert!(
        has_record(root, &imported_id),
        "the import analysed the copy cleanly, so its record is sealed"
    );
    drop(session);
    assert_eq!(
        reopened_blocks(root, &path),
        imported,
        "import copied the source without sealing its identity"
    );
    assert_eq!(reopened_blocks(root, "found.afs"), initialised);
}

/// KTLO-04 (#6), criterion 2: a source found in an open project and registered
/// in place keeps its identities like one registered at initialisation.
#[test]
fn a_source_registered_in_an_open_project_keeps_its_block_identities_when_it_is_reopened() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut session = ProjectSession::create(root, "Found later").unwrap();
    std::fs::write(root.join("found.afs"), SOURCE).unwrap();
    let ids = session
        .register_existing(&[registration("found.afs")], Origin::SESSION)
        .unwrap();
    let registered = blocks(session.document(&ids[0]).unwrap());
    assert!(
        has_record(root, &ids[0]),
        "the registration analysed the source cleanly, so its record is sealed"
    );
    drop(session);
    assert_eq!(reopened_blocks(root, "found.afs"), registered);
}

/// KTLO-04 (#6), criterion 3. An invalid source seals nothing: the checkpoint
/// never seals garbage. Its record appears at the first analysis that turns it
/// valid, which for saved source is the analysis of the fixed file. Text edited
/// in the session is not saved source (ADR 0010: "Shared checkpoints describe
/// saved valid source"), so its record comes with the save, as for any edit.
#[test]
fn an_invalid_source_seals_its_identities_at_its_first_successful_analysis() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("found.afs"), BROKEN).unwrap();
    let mut session =
        ProjectSession::initialise(root, "Fixed later", &[registration("found.afs")]).unwrap();
    let found = document_at(&session, "found.afs").id().to_owned();
    assert_eq!(
        session.document(&found).unwrap().analysis(),
        AnalysisState::Invalid
    );
    assert!(!has_record(root, &found), "an invalid source seals nothing");

    // The file is fixed outside the session, as by another editor or a pull.
    std::fs::write(root.join("found.afs"), SOURCE).unwrap();
    let changes = session.external_check().run().unwrap();
    let outcome = session.apply_external(changes).unwrap();
    assert_eq!(outcome.reloaded, std::slice::from_ref(&found));
    let fixed = blocks(session.document(&found).unwrap());
    assert!(
        has_record(root, &found),
        "the first analysis that turned the saved source valid seals its record"
    );
    drop(session);
    assert_eq!(reopened_blocks(root, "found.afs"), fixed);

    // An imported copy of an invalid file, fixed in the session.
    let mut session = ProjectSession::open(root).unwrap();
    let incoming = root.join("incoming.afs");
    std::fs::write(&incoming, BROKEN).unwrap();
    let (imported, path) = import_one(&mut session, &incoming);
    assert!(
        !has_record(root, &imported),
        "an invalid copy seals nothing"
    );
    session
        .edit_source(&imported, SOURCE, Origin::SESSION)
        .unwrap();
    session.settle(&imported).unwrap();
    assert_eq!(
        session.document(&imported).unwrap().analysis(),
        AnalysisState::Valid
    );
    assert!(
        !has_record(root, &imported),
        "unsaved text is not saved source and is not sealed"
    );
    assert!(session.save(&imported).unwrap().checkpoint);
    let edited = blocks(session.document(&imported).unwrap());
    assert!(has_record(root, &imported));
    drop(session);
    assert_eq!(reopened_blocks(root, &path), edited);
}

/// KTLO-04 (#6), criterion 4: the limitation the owner accepted with the
/// decision of 2026-10-26. The record is sealed by the first successful
/// analysis in a session that may write the project. A document whose source
/// turned valid while the project was closed, before any such analysis, has no
/// record yet; until a writing session opens the project, every load that only
/// reads it (a second window, `apiforge check`) lowers fresh identities, so
/// the blocks drift between those loads. This test names that drift so that
/// the decision stays visible; the first writing session ends it.
#[test]
fn a_document_closed_before_its_first_successful_analysis_gets_fresh_identities_until_one_seals_them()
 {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("found.afs"), BROKEN).unwrap();
    let session =
        ProjectSession::initialise(root, "Closed early", &[registration("found.afs")]).unwrap();
    let found = document_at(&session, "found.afs").id().to_owned();
    drop(session);
    std::fs::write(root.join("found.afs"), SOURCE).unwrap();

    let first = reopened_blocks(root, "found.afs");
    let second = reopened_blocks(root, "found.afs");
    assert_ne!(
        first, second,
        "the accepted limitation: without a record each load allocates fresh identities"
    );
    assert!(
        !has_record(root, &found),
        "a load that only reads the project writes nothing"
    );

    let session = ProjectSession::open(root).unwrap();
    let sealed = blocks(session.document(&found).unwrap());
    drop(session);
    assert!(
        has_record(root, &found),
        "the first successful analysis in a writing session seals the record"
    );
    assert_eq!(reopened_blocks(root, "found.afs"), sealed);
    assert_eq!(reopened_blocks(root, "found.afs"), sealed);
}

/// KTLO-04 (#6), criterion 5. Migration writes every scenario and component
/// as a registered document of a folder project; each of them, and each
/// published revision, has its record as soon as the migration finishes, so
/// the migrated identities and layout are the ones a reopened project shows.
#[test]
fn a_migrated_project_has_a_record_for_every_document_and_revision() {
    let workspace = apiforge_core::import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &apiforge_core::ImportLimits::default(),
    )
    .unwrap()
    .into_workspace();
    let directory = tempfile::tempdir().unwrap();
    let store =
        apiforge_storage::WorkspaceStore::open_exclusive(directory.path().join("workspace.json"))
            .unwrap();
    store.save(&workspace).unwrap();
    let outcome = migrate_workspace(&store).unwrap();
    assert_eq!(outcome.catalogue.projects.len(), workspace.projects.len());
    for project in &outcome.catalogue.projects {
        let root = project.path.as_path();
        let bytes = std::fs::read(root.join(".apiforge/diagram.json")).unwrap();
        let file = apiforge_storage::project::DiagramFile::from_bytes(&bytes).unwrap();
        let session = ProjectSession::open_saved_read_only(root).unwrap();
        assert!(!session.documents().is_empty());
        for document in session.documents() {
            assert!(
                has_record(root, document.id()),
                "{} in {} has no record",
                document.entry().path.as_str(),
                project.project_id
            );
            if document.entry().kind == DocumentKind::Scenario {
                assert_eq!(
                    blocks(document),
                    reopened_blocks(root, document.entry().path.as_str())
                );
            }
        }
        for revision in &session.manifest().revisions {
            let key = apiforge_storage::project::DocumentKey::Revision {
                component_id: revision.component_id.clone(),
                revision: revision.revision,
            };
            assert!(file.records.iter().any(|record| record.key == key));
        }
    }
}

/// KTLO-04 (#6), criterion 6. ADR 0010: "Do not fail otherwise valid source
/// merely because a checkpoint is missing." A record that cannot be written —
/// here because `diagram.json` changed on disk after the session read it — is
/// left to the next save, and the registration or import that already
/// succeeded still succeeds. The changed file is not overwritten.
#[test]
fn a_checkpoint_that_cannot_be_written_does_not_fail_the_registration_or_import() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut session = ProjectSession::create(root, "Foreign checkpoint").unwrap();
    let foreign = b"{\n  \"formatVersion\": 1,\n  \"records\": []\n}\n";
    std::fs::create_dir_all(root.join(".apiforge")).unwrap();
    std::fs::write(root.join(".apiforge/diagram.json"), foreign).unwrap();

    std::fs::write(root.join("found.afs"), SOURCE).unwrap();
    let registered = session
        .register_existing(&[registration("found.afs")], Origin::SESSION)
        .expect("the registration succeeds without its record");
    let incoming = root.join("incoming.afs");
    std::fs::write(&incoming, SOURCE).unwrap();
    let (imported, path) = import_one(&mut session, &incoming);
    for id in [&registered[0], &imported] {
        let document = session.document(id).unwrap();
        assert_eq!(document.analysis(), AnalysisState::Valid);
        assert!(!document.is_dirty());
    }
    assert_eq!(
        std::fs::read(root.join(".apiforge/diagram.json")).unwrap(),
        foreign,
        "a checkpoint changed outside the session is never overwritten"
    );
    drop(session);

    let session = ProjectSession::open_saved_read_only(root).unwrap();
    assert_eq!(session.documents().len(), 2);
    assert_eq!(
        document_at(&session, &path).analysis(),
        AnalysisState::Valid
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

/// KTLO-25 (#76). A fresh demo shop is the first project many people open, so
/// its Problems list starts empty: every document analyses valid with no
/// warning and no error. `components/product.afs` used to carry "The value's
/// type does not match this input".
#[test]
fn the_prepared_demo_shop_analyses_without_a_problem_in_any_document() {
    let directory = prepared_demo();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    session.settle_all().unwrap();

    assert!(!session.documents().is_empty());
    let problems: Vec<String> = session
        .documents()
        .iter()
        .flat_map(|document| {
            let text = document.text();
            document.diagnostics().iter().map(move |diagnostic| {
                let before = &text[..diagnostic.range.start];
                let line = before.matches('\n').count() + 1;
                let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                format!(
                    "{}:{line}:{column}: {:?} {}",
                    document.entry().path.as_str(),
                    diagnostic.severity,
                    diagnostic.message
                )
            })
        })
        .collect();
    assert!(problems.is_empty(), "{problems:#?}");
    for document in session.documents() {
        assert_eq!(
            document.analysis(),
            AnalysisState::Valid,
            "{}",
            document.entry().path.as_str()
        );
    }
}
// ---------------------------------------------------------------------------
// KTLO-26 (#79): a board edit that removes the request feeding a check and
// wires a new one back, then saves.

type Ids = Vec<(String, String)>;

/// The (handle, identity) of every block of both graphs of a scenario model, in
/// stored order.
fn ids_of(model: &Document) -> Ids {
    let Document::Scenario(scenario) = model else {
        panic!("a scenario model")
    };
    scenario
        .graph
        .nodes
        .iter()
        .chain(&scenario.cleanup.nodes)
        .map(|node| (node.handle.clone(), node.id.clone()))
        .collect()
}

/// The saved checkpoint record of the working document `document_id`.
fn record_of(root: &Path, document_id: &str) -> apiforge_storage::project::DiagramRecord {
    let bytes = std::fs::read(root.join(".apiforge/diagram.json")).unwrap();
    let file = apiforge_storage::project::DiagramFile::from_bytes(&bytes).unwrap();
    file.records
        .into_iter()
        .find(|record| {
            record.key
                == apiforge_storage::project::DocumentKey::Working {
                    document_id: document_id.into(),
                }
        })
        .expect("the document has a record")
}

/// The saved source of the document at `path`, lowered with the folder
/// policy, as the next load lowers it before it consults the checkpoint.
fn lowered_saved_source(
    root: &Path,
    path: &str,
    context: &apiforge_core::format::Project,
) -> (String, Document) {
    let text = std::fs::read_to_string(root.join(path)).unwrap();
    let lowered = apiforge_script::lower(
        &apiforge_script::parse(&text),
        &apiforge_script::LowerContext::folder(context),
    );
    let Some(document) = lowered.document else {
        panic!("the saved source lowers: {:?}", lowered.diagnostics)
    };
    (text, document)
}

fn base_of(document: &Document) -> apiforge_storage::project::IdentityBase {
    match document {
        Document::Scenario(scenario) => {
            apiforge_storage::project::IdentityBase::Scenario(scenario.clone())
        }
        Document::Component(component) => {
            apiforge_storage::project::IdentityBase::Component(component.clone())
        }
    }
}

/// The semantic digest a checkpoint of `document` for `text` carries.
fn digest_of(text: &str, document: &Document, context: &apiforge_core::format::Project) -> String {
    apiforge_storage::project::DiagramRecord::capture(
        apiforge_storage::project::DocumentKey::Working {
            document_id: "digest".into(),
        },
        text,
        base_of(document),
        context,
    )
    .unwrap()
    .semantic_digest
}

/// `model` with every block's inputs in the order `source_order` gives the
/// block of the same handle; blocks and everything else untouched.
fn with_inputs_in_source_order(model: &Document, source_order: &Document) -> Document {
    let (Document::Scenario(model), Document::Scenario(source)) = (model, source_order) else {
        panic!("scenario models")
    };
    let mut model = model.clone();
    let order: std::collections::HashMap<_, _> = source
        .graph
        .nodes
        .iter()
        .chain(&source.cleanup.nodes)
        .map(|node| {
            (
                node.handle.clone(),
                node.inputs.keys().cloned().collect::<Vec<_>>(),
            )
        })
        .collect();
    for node in model.graph.nodes.iter_mut().chain(&mut model.cleanup.nodes) {
        let keys = &order[&node.handle];
        node.inputs
            .sort_by_key(|name, _| keys.iter().position(|key| key == name));
    }
    Document::Scenario(model)
}

/// Every block's (identity, position) by handle, and every order edge's
/// (identity, source, target): what a selection and the board's layout refer
/// to, whatever the stored order.
type Layout = (
    std::collections::BTreeMap<String, (String, f64, f64)>,
    std::collections::BTreeSet<(String, String, String)>,
);

fn layout_of(model: &Document) -> Layout {
    let Document::Scenario(scenario) = model else {
        panic!("a scenario model")
    };
    let graphs = [&scenario.graph, &scenario.cleanup];
    let blocks = graphs
        .iter()
        .flat_map(|graph| &graph.nodes)
        .map(|node| {
            (
                node.handle.clone(),
                (node.id.clone(), node.position.x, node.position.y),
            )
        })
        .collect();
    let edges = graphs
        .iter()
        .flat_map(|graph| &graph.edges)
        .map(|edge| (edge.id.clone(), edge.source.clone(), edge.target.clone()))
        .collect();
    (blocks, edges)
}

/// Every block's handle and input names, in stored order.
fn order_of(model: &Document) -> Vec<(String, Vec<String>)> {
    let Document::Scenario(scenario) = model else {
        panic!("a scenario model")
    };
    scenario
        .graph
        .nodes
        .iter()
        .chain(&scenario.cleanup.nodes)
        .map(|node| (node.handle.clone(), node.inputs.keys().cloned().collect()))
        .collect()
}

/// The input names of the cleanup check, in stored order.
fn check_inputs(model: &Document) -> Vec<String> {
    let Document::Scenario(scenario) = model else {
        panic!("a scenario model")
    };
    let check = scenario
        .cleanup
        .nodes
        .iter()
        .find(|node| node.kind == apiforge_core::model::NodeKind::Assert)
        .expect("the cleanup has a check");
    check.inputs.keys().cloned().collect()
}

/// The board's edit of the demo scenario's cleanup, as the Diagram mode makes
/// it: delete the request whose status the check compares (which removes the
/// check's `actual`), add a new request and wire its status back into
/// `actual`, which appends it after `expected`.
fn rewired(model: &Document, context: &apiforge_core::format::Project) -> Document {
    use apiforge_core::{
        block::Block,
        edit::{add_node, connect},
        format::Position,
        graph::delete_nodes,
        model::NodeKind,
    };
    let Document::Scenario(scenario) = model else {
        panic!("a scenario model")
    };
    let mut scenario = scenario.clone();
    let cleanup = &scenario.cleanup;
    let request = cleanup
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Request)
        .expect("the cleanup starts with a request")
        .clone();
    let check = cleanup
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Assert)
        .expect("the cleanup has a check")
        .id
        .clone();
    let graph = delete_nodes(
        cleanup,
        &std::collections::HashSet::from([request.id.as_str()]),
    );
    let added = add_node(
        &graph,
        Block::Request,
        Position { x: 80.0, y: 240.0 },
        None,
        None,
    )
    .unwrap();
    let mut graph = added.graph;
    // The person fills the new request in the inspector as the old one was.
    let node = graph
        .nodes
        .iter_mut()
        .find(|node| node.id == added.node_id)
        .unwrap();
    node.config = request.config.clone();
    node.inputs = request.inputs.clone();
    node.handle = "delete_cart_again".into();
    let graph = connect(
        &graph,
        context,
        &added.node_id,
        "status",
        &check,
        "actual",
        &[],
    )
    .unwrap();
    scenario.cleanup = graph;
    Document::Scenario(scenario)
}

/// KTLO-26 (#79), the board path. A board edit that deletes the request whose
/// status the cleanup check compares, adds a new request and wires its status
/// back into the check's `actual`, then saves, should seal a checkpoint that
/// describes the saved source, so that the next load restores the saved model
/// exactly rather than reconciling against it.
///
/// The edit leaves the check's inputs stored as `expected, actual`; the
/// printed source reads them back as `actual, expected`, and the semantic
/// digest serialises the inputs in stored order. The owner's decision A
/// (2026-10-27): `apply_model_edit` keeps the model the printed source reads
/// back, with the edited model's identities and positions, so the model after
/// the edit is in the order a reload gives and the checkpoint sealed from it
/// describes the saved source.
#[test]
fn ktlo_26_a_board_edit_that_rewires_a_check_seals_a_checkpoint_that_describes_its_source() {
    let directory = prepared_demo();
    let root = directory.path();
    let mut session = ProjectSession::open(root).unwrap();
    let id = only(&session, DocumentKind::Scenario);
    let path = session
        .document(&id)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();
    let context = (**session.context()).clone();
    let before = session.document(&id).unwrap().model().unwrap().clone();
    assert_eq!(check_inputs(&before), ["actual", "expected"]);

    let edited = rewired(&before, &context);
    assert_eq!(
        check_inputs(&edited),
        ["expected", "actual"],
        "the board's delete and connect append `actual` after `expected`"
    );
    session
        .apply_model_edit(&id, edited.clone(), Origin(7))
        .unwrap();
    let outcome = session.save(&id).unwrap();
    assert!(outcome.checkpoint, "the save writes a checkpoint");
    let saved = session.document(&id).unwrap().model().unwrap().clone();
    assert_eq!(
        layout_of(&saved),
        layout_of(&edited),
        "the session keeps every identity and position of the edited model"
    );
    assert_eq!(
        check_inputs(&saved),
        ["actual", "expected"],
        "the session keeps the check's inputs in the order the source reads back"
    );
    let record = record_of(root, &id);
    drop(session);

    let (text, lowered) = lowered_saved_source(root, &path, &context);
    assert_eq!(check_inputs(&lowered), ["actual", "expected"]);
    assert_eq!(
        apiforge_script::semantic(&saved, &context),
        apiforge_script::semantic(&lowered, &context),
        "the same semantics, compared as values"
    );
    // The cause: only the inputs' order separates the two digests.
    let source_digest = digest_of(&text, &lowered, &context);
    assert_eq!(
        digest_of(
            &text,
            &with_inputs_in_source_order(&saved, &lowered),
            &context
        ),
        source_digest,
        "the saved model with its inputs in source order digests as the source"
    );

    let reopened = ProjectSession::open_saved_read_only(root).unwrap();
    let after = reopened.document(&id).unwrap().model().unwrap().clone();
    let describes = record.describes(&text, &base_of(&lowered), &context);
    assert!(
        describes,
        "the sealed checkpoint does not describe its source: record digest {}, \
         source digest {source_digest}; ids saved {:?}, reopened {:?}; inputs saved {:?}, \
         reopened {:?}",
        record.semantic_digest,
        ids_of(&saved),
        ids_of(&after),
        check_inputs(&saved),
        check_inputs(&after),
    );
    assert_eq!(
        ids_of(&after),
        ids_of(&saved),
        "the reload restores the saved blocks"
    );
    assert_eq!(
        order_of(&after),
        order_of(&saved),
        "the blocks and inputs after the edit are in the order a reload gives"
    );
    assert_eq!(
        layout_of(&after),
        layout_of(&edited),
        "the reload restores every identity and position of the edited model"
    );
    assert_eq!(after, saved, "the reload restores the saved model exactly");
}

/// KTLO-26 (#79), the text path, for contrast. The same rewiring made by
/// editing the source (the very text the board edit prints) seals a checkpoint
/// that describes it: the model comes from lowering the text, so its inputs are
/// in source order, and the reload restores the saved model exactly. Source
/// cannot state the other order: `assert a == b` always lowers to `actual`,
/// then `expected`.
#[test]
fn ktlo_26_the_same_rewiring_typed_as_source_seals_a_checkpoint_that_describes_it() {
    let directory = prepared_demo();
    let root = directory.path();
    let mut session = ProjectSession::open(root).unwrap();
    let id = only(&session, DocumentKind::Scenario);
    let path = session
        .document(&id)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();
    let context = (**session.context()).clone();
    let before = session.document(&id).unwrap().model().unwrap().clone();
    let printed = apiforge_script::print_folder_document(&context, &rewired(&before, &context))
        .expect("the board's edit prints");

    session.edit_source(&id, &printed, Origin(7)).unwrap();
    let outcome = session.save(&id).unwrap();
    assert!(outcome.checkpoint, "the save writes a checkpoint");
    let saved = session.document(&id).unwrap().model().unwrap().clone();
    assert_eq!(check_inputs(&saved), ["actual", "expected"]);
    let record = record_of(root, &id);
    drop(session);

    let (text, lowered) = lowered_saved_source(root, &path, &context);
    assert_eq!(text, printed);
    assert!(
        record.describes(&text, &base_of(&lowered), &context),
        "the checkpoint describes its source"
    );
    let reopened = ProjectSession::open_saved_read_only(root).unwrap();
    let after = reopened.document(&id).unwrap().model().unwrap().clone();
    assert_eq!(ids_of(&after), ids_of(&saved));
    assert_eq!(after, saved, "the reload restores the saved model exactly");
}
