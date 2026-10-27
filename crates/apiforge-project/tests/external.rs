use apiforge_project::*;
use std::fs;

const SCENARIO: &str = "scenario \"Main\" {\n  total = 2\n}\n";
const CHANGED: &str = "scenario \"Main\" {\n  total = 9\n}\n";
const OTHER: &str = "scenario \"Other\" {\n  count = 1\n}\n";

fn project() -> (tempfile::TempDir, ProjectSession, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Folder").unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    (directory, session, id)
}

fn poll(session: &mut ProjectSession) -> ExternalOutcome {
    let changes = session.external_check().run().unwrap();
    session.apply_external(changes).unwrap()
}

#[test]
fn nothing_changes_without_external_edits() {
    let (_directory, mut session, _) = project();
    assert!(poll(&mut session).is_empty());
    let before = session.last_sequence();
    assert!(poll(&mut session).is_empty());
    assert_eq!(session.last_sequence(), before, "polling emits nothing");
}

#[test]
fn a_clean_document_follows_an_external_edit_of_equal_length() {
    let (directory, mut session, id) = project();
    assert_eq!(SCENARIO.len(), CHANGED.len());
    fs::write(directory.path().join("scenarios/main.afs"), CHANGED).unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.reloaded, std::slice::from_ref(&id));
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), CHANGED);
    assert!(!document.is_dirty());
    assert_eq!(document.analysis(), AnalysisState::Valid);
    assert!(
        poll(&mut session).is_empty(),
        "the new state is the baseline"
    );
}

#[test]
fn unsaved_work_gets_a_conflict_and_is_never_overwritten_or_saved_over() {
    let (directory, mut session, id) = project();
    let local = SCENARIO.replace('2', "5");
    session.edit_source(&id, &local, Origin::SESSION).unwrap();
    let file = directory.path().join("scenarios/main.afs");
    fs::write(&file, CHANGED).unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.conflicts, std::slice::from_ref(&id));
    let document = session.document(&id).unwrap();
    assert_eq!(document.conflict(), Some(ExternalConflict::Changed));
    assert_eq!(document.text(), local, "local work is kept");
    assert!(matches!(session.save(&id), Err(Error::ExternalConflict)));
    assert_eq!(fs::read_to_string(&file).unwrap(), CHANGED);
    // Unsaved work stays protected while the conflict lasts.
    session.save_recovery().unwrap();

    session.accept_external(&id, Origin::SESSION).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), CHANGED);
    assert_eq!(document.conflict(), None);
    assert!(!document.needs_recovery());
}

#[test]
fn a_deleted_file_with_unsaved_work_can_be_recreated_explicitly() {
    let (directory, mut session, id) = project();
    let local = SCENARIO.replace('2', "5");
    session.edit_source(&id, &local, Origin::SESSION).unwrap();
    let file = directory.path().join("scenarios/main.afs");
    fs::remove_file(&file).unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.conflicts, std::slice::from_ref(&id));
    assert_eq!(
        session.document(&id).unwrap().conflict(),
        Some(ExternalConflict::Deleted)
    );
    session.keep_deleted(&id).unwrap();
    session.save(&id).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), local);
}

#[test]
fn a_clean_deleted_file_becomes_missing_and_returns_when_restored() {
    let (directory, mut session, id) = project();
    let file = directory.path().join("scenarios/main.afs");
    fs::remove_file(&file).unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.missing, std::slice::from_ref(&id));
    assert_eq!(
        session.document(&id).unwrap().unavailable(),
        Some(Unavailable::Missing)
    );
    fs::write(&file, SCENARIO).unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.reloaded, std::slice::from_ref(&id));
    let document = session.document(&id).unwrap();
    assert_eq!(document.unavailable(), None);
    assert_eq!(document.text(), SCENARIO);
}

#[test]
fn an_external_rename_is_followed_only_for_a_unique_hash_match() {
    let (directory, mut session, id) = project();
    let root = directory.path();
    let local = SCENARIO.replace('2', "5");
    session.edit_source(&id, &local, Origin::SESSION).unwrap();
    fs::create_dir(root.join("moved")).unwrap();
    fs::rename(root.join("scenarios/main.afs"), root.join("moved/main.afs")).unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.renamed, std::slice::from_ref(&id));
    let document = session.document(&id).unwrap();
    assert_eq!(document.entry().path.as_str(), "moved/main.afs");
    assert_eq!(document.text(), local, "unsaved work follows the rename");
    assert!(poll(&mut session).is_empty());
    drop(session);
    let session = ProjectSession::open(root).unwrap();
    assert_eq!(
        session.document(&id).unwrap().entry().path.as_str(),
        "moved/main.afs"
    );

    // Two identical copies are ambiguous: the document is reported missing.
    let (directory, mut session, id) = project();
    let root = directory.path();
    fs::rename(root.join("scenarios/main.afs"), root.join("a.afs")).unwrap();
    fs::write(root.join("b.afs"), SCENARIO).unwrap();
    let outcome = poll(&mut session);
    assert!(outcome.renamed.is_empty());
    assert_eq!(outcome.missing, std::slice::from_ref(&id));
}

#[test]
fn registry_changes_reload_a_clean_project_and_block_writes_otherwise() {
    let (directory, mut session, id) = project();
    let root = directory.path();
    let registry = root.join("apiforge.project.json");
    let renamed = fs::read_to_string(&registry)
        .unwrap()
        .replace("\"Folder\"", "\"Renamed\"");
    fs::write(&registry, &renamed).unwrap();
    let outcome = poll(&mut session);
    assert!(outcome.project_reloaded);
    assert_eq!(session.manifest().name, "Renamed");
    assert_eq!(session.registry_state(), RegistryState::Current);

    // With unsaved work the change waits for an explicit reload.
    let local = SCENARIO.replace('2', "5");
    session.edit_source(&id, &local, Origin::SESSION).unwrap();
    let again = renamed.replace("\"Renamed\"", "\"Again\"");
    fs::write(&registry, &again).unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.registry, Some(RegistryState::Changed));
    assert!(matches!(session.save(&id), Err(Error::RegistryChanged)));
    assert!(matches!(
        session.create_folder("x"),
        Err(Error::RegistryChanged)
    ));
    session.reload_project().unwrap();
    assert_eq!(session.manifest().name, "Again");
    assert_eq!(session.document(&id).unwrap().text(), local);
    session.save(&id).unwrap();
    assert_eq!(
        fs::read_to_string(root.join("scenarios/main.afs")).unwrap(),
        local
    );
}

#[test]
fn an_invalid_registry_blocks_writes_and_preserves_work_until_repaired() {
    let (directory, mut session, id) = project();
    let registry = directory.path().join("apiforge.project.json");
    let saved = fs::read(&registry).unwrap();
    session
        .edit_source(&id, &SCENARIO.replace('2', "5"), Origin::SESSION)
        .unwrap();
    fs::write(&registry, b"{ not json").unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.registry, Some(RegistryState::Invalid));
    assert!(matches!(session.save(&id), Err(Error::RegistryChanged)));
    assert_eq!(fs::read(&registry).unwrap(), b"{ not json");
    assert!(session.document(&id).unwrap().is_dirty());
    fs::write(&registry, &saved).unwrap();
    let outcome = poll(&mut session);
    assert_eq!(outcome.registry, Some(RegistryState::Current));
    session.save(&id).unwrap();

    fs::remove_file(&registry).unwrap();
    assert_eq!(poll(&mut session).registry, Some(RegistryState::Missing));
}

#[test]
fn a_check_taken_before_an_own_save_is_ignored_for_that_document() {
    let (_directory, mut session, id) = project();
    let check = session.external_check();
    session.edit_source(&id, OTHER, Origin::SESSION).unwrap();
    session.save(&id).unwrap();
    let changes = check.run().unwrap();
    let outcome = session.apply_external(changes).unwrap();
    assert!(outcome.is_empty());
    assert_eq!(session.document(&id).unwrap().text(), OTHER);
}

#[test]
fn unsaved_configuration_must_be_discarded_before_following_a_changed_registry() {
    let (directory, mut session, _) = project();
    session
        .edit_configuration(
            |manifest| manifest.environment.base_url = "https://local.test".into(),
            Origin::SESSION,
        )
        .unwrap();
    let registry = directory.path().join("apiforge.project.json");
    let changed = fs::read_to_string(&registry)
        .unwrap()
        .replace("\"Folder\"", "\"Elsewhere\"");
    fs::write(&registry, &changed).unwrap();
    assert_eq!(poll(&mut session).registry, Some(RegistryState::Changed));
    assert!(matches!(
        session.save_configuration(),
        Err(Error::RegistryChanged)
    ));
    assert!(matches!(
        session.reload_project(),
        Err(Error::UnsavedConfiguration)
    ));
    session.discard_configuration().unwrap();
    assert!(!session.is_manifest_dirty());
    session.reload_project().unwrap();
    assert_eq!(session.manifest().name, "Elsewhere");
    assert_eq!(session.manifest().environment.base_url, "");
    assert_eq!(fs::read_to_string(&registry).unwrap(), changed);
}

#[test]
fn a_source_beyond_the_scan_limit_is_not_taken_for_deleted() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Deep").unwrap();
    let depth = apiforge_storage::project::MAX_TREE_DEPTH + 1;
    let folder: Vec<String> = (0..depth).map(|level| format!("d{level}")).collect();
    let path = format!("{}/deep.afs", folder.join("/"));
    let id = session
        .create_document(DocumentKind::Scenario, &path, SCENARIO)
        .unwrap();
    session
        .edit_source(&id, &SCENARIO.replace('2', "5"), Origin::SESSION)
        .unwrap();
    let changes = session.external_check().run().unwrap();
    assert!(changes.scan.truncated);
    assert!(session.apply_external(changes).unwrap().is_empty());
    let document = session.document(&id).unwrap();
    assert_eq!(document.unavailable(), None);
    assert_eq!(document.conflict(), None);
    session.save(&id).unwrap();

    // A real change below the limit is still seen, by a direct check.
    fs::write(directory.path().join(&path), CHANGED).unwrap();
    assert_eq!(poll(&mut session).reloaded, std::slice::from_ref(&id));
    fs::remove_file(directory.path().join(&path)).unwrap();
    assert_eq!(poll(&mut session).missing, std::slice::from_ref(&id));
}

/// MCP-07: an observing session becomes the writer only once the holder is
/// gone, keeps the text typed while it observed, and then names itself.
#[test]
fn an_observer_takes_the_writer_lock_only_when_it_is_free_and_keeps_its_text() {
    let (directory, owner, id) = project();
    let mut observer = ProjectSession::open(directory.path()).unwrap();
    assert!(observer.is_read_only());
    observer.edit_source(&id, CHANGED, Origin::SESSION).unwrap();
    assert!(!observer.take_writer(&Holder::window()).unwrap(), "held");
    assert!(observer.is_read_only());
    drop(owner);
    assert!(observer.take_writer(&Holder::window()).unwrap());
    assert!(!observer.is_read_only());
    assert_eq!(observer.document(&id).unwrap().text(), CHANGED);
    assert!(observer.document(&id).unwrap().is_dirty());
    observer.save(&id).unwrap();
    assert_eq!(
        fs::read_to_string(directory.path().join("scenarios/main.afs")).unwrap(),
        CHANGED
    );
    let reader = ProjectSession::open_saved_read_only(directory.path()).unwrap();
    assert_eq!(
        reader.store().holder().unwrap().unwrap().role(),
        Some(HolderRole::Window)
    );
}
