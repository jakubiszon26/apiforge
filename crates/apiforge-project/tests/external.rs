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

// ----- MCP-08a: an observer follows every commit of the holder ----------------

mod observer {
    use super::*;

    /// The holder's edits, as the agent's would be.
    const HOLDER: Origin = Origin(90);

    /// The project held by the session that created it, and a second session
    /// beside it, which can only observe.
    fn held() -> (tempfile::TempDir, ProjectSession, ProjectSession, String) {
        let (directory, holder, id) = project();
        let observer = ProjectSession::open(directory.path()).unwrap();
        assert!(observer.is_read_only(), "the holder keeps the lock");
        (directory, holder, observer, id)
    }

    fn blocks(session: &ProjectSession, id: &str) -> Vec<(String, f64, f64)> {
        let Some(Document::Scenario(scenario)) = session.document(id).unwrap().model() else {
            panic!("a valid scenario");
        };
        scenario
            .graph
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.position.x, node.position.y))
            .collect()
    }

    /// Moves every block of the holder's scenario, as a layout edit does.
    fn move_blocks(holder: &mut ProjectSession, id: &str, dx: f64, dy: f64) {
        let Some(Document::Scenario(mut scenario)) = holder.document(id).unwrap().model().cloned()
        else {
            panic!("a valid scenario");
        };
        for node in &mut scenario.graph.nodes {
            node.position.x += dx;
            node.position.y += dy;
        }
        holder
            .apply_model_edit(id, Document::Scenario(scenario), HOLDER)
            .unwrap();
    }

    /// ADR 0014 decision 5: a commit of the holder is followed by reloading
    /// the project in a new epoch, with the identities and positions of the
    /// holder's checkpoint, not ones this session would derive itself.
    #[test]
    fn a_commit_is_followed_with_the_holders_identities_and_positions() {
        let (_directory, mut holder, mut observer, id) = held();
        let epoch = observer.epoch();
        holder.edit_source(&id, CHANGED, HOLDER).unwrap();
        holder.settle(&id).unwrap();
        move_blocks(&mut holder, &id, 240.0, 160.0);
        holder.save(&id).unwrap();

        let outcome = poll(&mut observer);
        assert!(outcome.project_reloaded, "{outcome:?}");
        assert!(observer.epoch() > epoch, "a reload starts a new epoch");
        assert_eq!(observer.document(&id).unwrap().text(), CHANGED);
        assert_eq!(blocks(&observer, &id), blocks(&holder, &id));
        assert!(poll(&mut observer).is_empty(), "the commit is the baseline");
    }

    /// MCP-08a criterion 2, planning decision 3: a commit that changes only
    /// the checkpoint, the blocks moved and the source untouched, is
    /// followed too; the shared diagram file is fingerprinted directly.
    #[test]
    fn a_layout_only_commit_is_followed() {
        let (_directory, mut holder, mut observer, id) = held();
        move_blocks(&mut holder, &id, -80.0, 40.0);
        holder.save(&id).unwrap();
        assert_eq!(holder.document(&id).unwrap().text(), SCENARIO);

        let outcome = poll(&mut observer);
        assert!(outcome.project_reloaded, "{outcome:?}");
        assert_eq!(blocks(&observer, &id), blocks(&holder, &id));
    }

    /// Planning decision 1: the observer shows the saved state only. The
    /// holder's unsaved work in its recovery file belongs to the holder, at
    /// opening and after following a commit alike.
    #[test]
    fn an_observer_shows_the_saved_state_not_the_holders_unsaved_work() {
        let (directory, mut holder, id) = project();
        let other = holder
            .create_document(DocumentKind::Scenario, "scenarios/other.afs", OTHER)
            .unwrap();
        let unsaved = SCENARIO.replace('2', "5");
        holder.edit_source(&id, &unsaved, HOLDER).unwrap();
        assert!(
            holder.save_recovery().unwrap(),
            "the holder protects its work"
        );

        let mut observer = ProjectSession::open(directory.path()).unwrap();
        assert!(observer.is_read_only());
        assert_eq!(observer.document(&id).unwrap().text(), SCENARIO);
        assert!(observer.recovered_drafts().is_empty());

        holder
            .edit_source(&other, &OTHER.replace('1', "4"), HOLDER)
            .unwrap();
        holder.save(&other).unwrap();
        assert!(holder.save_recovery().unwrap());
        let outcome = poll(&mut observer);
        assert!(outcome.project_reloaded, "{outcome:?}");
        assert_eq!(observer.document(&id).unwrap().text(), SCENARIO);
        assert!(observer.recovered_drafts().is_empty());
    }

    /// MCP-08a criterion 6: an analysis of the text before a followed commit
    /// that finishes after it is refused; the model stays the committed one.
    #[test]
    fn a_late_analysis_of_the_previous_text_is_refused_after_a_followed_commit() {
        let (_directory, mut holder, mut observer, id) = held();
        let epoch = observer.epoch();
        // Started on the old text, delayed until after the commit.
        let late = observer.analysis_for(&id).unwrap();
        holder.edit_source(&id, CHANGED, HOLDER).unwrap();
        holder.save(&id).unwrap();
        let outcome = poll(&mut observer);
        assert!(outcome.project_reloaded, "{outcome:?}");
        assert!(observer.epoch() > epoch, "a reload starts a new epoch");
        let model = observer.document(&id).unwrap().model().cloned();

        assert!(matches!(
            observer.apply_analysis(late.run()),
            Err(Error::StaleAnalysis)
        ));
        let document = observer.document(&id).unwrap();
        assert_eq!(document.text(), CHANGED);
        assert_eq!(document.model().cloned(), model, "never the earlier state");
    }

    /// MCP-08a criterion 7 at the session: a document the holder deleted is
    /// gone after the reload; the others stay.
    #[test]
    fn a_document_the_holder_deleted_is_gone_after_the_reload() {
        let (directory, mut holder, id) = project();
        holder
            .create_document(DocumentKind::Scenario, "scenarios/other.afs", OTHER)
            .unwrap();
        let mut observer = ProjectSession::open(directory.path()).unwrap();
        assert_eq!(observer.documents().len(), 2);
        let preview = holder.preview_delete(&["scenarios/other.afs"]).unwrap();
        holder.delete(&preview, HOLDER).unwrap();
        let outcome = poll(&mut observer);
        assert!(outcome.project_reloaded, "{outcome:?}");
        assert_eq!(observer.documents().len(), 1);
        assert!(observer.document(&id).is_some());
    }
}
