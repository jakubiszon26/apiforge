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
    assert_eq!(outcome.reloaded, [found.clone()]);
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
