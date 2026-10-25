//! Managed startup: what the application data directory resolves to, and what
//! it leaves on disk. Every test owns its data directory, so none of them read
//! the directory this computer actually uses.
use super::activation::{Start, activate, decide};
use super::*;
use crate::testing::{MINIMUM, managed_harness, settle};
use apiforge_core::format::Workspace;
use apiforge_project::{CATALOGUE_FILE, PROJECTS_DIRECTORY, ProjectCatalogue};
use apiforge_storage::WorkspaceStore;
use std::{collections::BTreeMap, fs, path::Path};

/// A data directory holding a legacy workspace of the demo projects, exactly
/// as a version-two installation left one.
pub(super) fn legacy(data: &Path) -> Workspace {
    let workspace = apiforge_core::import_document(
        include_bytes!("../../../../fixtures/workspace.json"),
        &apiforge_core::ImportLimits::default(),
    )
    .unwrap()
    .into_workspace();
    let store = WorkspaceStore::open_exclusive(data.join("workspace.json")).unwrap();
    store.save(&workspace).unwrap();
    drop(store);
    workspace
}

pub(super) fn catalogue(data: &Path) -> ProjectCatalogue {
    ProjectCatalogue::load(data).unwrap().expect("catalogue")
}

/// Every file of the data directory with its bytes, to prove what was and was
/// not touched.
fn snapshot(data: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(data).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            files.insert(name, fs::read(entry.path()).unwrap());
        }
    }
    files
}

fn recent(data: &Path) -> navigation::RecentProjects {
    navigation::RecentProjects::at(data)
}

#[test]
fn an_empty_data_directory_gets_one_project_to_open() {
    let data = tempfile::tempdir().unwrap();
    let mut harness = managed_harness(data.path(), MINIMUM);
    let app = crate::testing::folder(&harness);
    assert!(app.open_error.is_none(), "{:?}", app.open_error);
    assert!(!app.uninitialised, "a created project is already a project");

    // It lives where migration would have put one, and it opens a document
    // rather than an empty board.
    let root = app.root.clone();
    assert!(root.starts_with(data.path().join(PROJECTS_DIRECTORY)));
    assert!(root.join("apiforge.project.json").is_file());
    assert!(
        app.session
            .as_ref()
            .is_some_and(|s| !s.documents().is_empty()),
        "a new project opens with something in it"
    );
    assert!(
        harness
            .state()
            .as_ref()
            .unwrap()
            .workbench
            .active_tab()
            .is_some()
    );

    // No legacy workspace is written, and no migration is claimed.
    assert!(!data.path().join("workspace.json").exists());
    assert!(!data.path().join(CATALOGUE_FILE).exists());

    // The report says where it went.
    let notice = crate::testing::folder(&harness)
        .notice
        .as_ref()
        .expect("notice");
    assert!(!notice.error);
    assert!(
        notice.text.contains(&root.display().to_string()),
        "{}",
        notice.text
    );

    settle(&mut harness);
    assert_eq!(recent(data.path()).active.as_deref(), Some(root.as_path()));
}

#[test]
fn a_second_start_reopens_the_created_project_instead_of_another_one() {
    let data = tempfile::tempdir().unwrap();
    let first = {
        let harness = managed_harness(data.path(), MINIMUM);
        crate::testing::folder(&harness).root.clone()
    };
    let harness = managed_harness(data.path(), MINIMUM);
    assert_eq!(crate::testing::folder(&harness).root, first);
    let projects: Vec<_> = fs::read_dir(data.path().join(PROJECTS_DIRECTORY))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(projects.len(), 1, "{projects:?}");
}

#[test]
fn a_legacy_workspace_migrates_on_startup_and_opens_what_was_active() {
    let data = tempfile::tempdir().unwrap();
    let workspace = legacy(data.path());
    let before = snapshot(data.path());

    let harness = managed_harness(data.path(), MINIMUM);
    let app = crate::testing::folder(&harness);
    assert!(app.open_error.is_none(), "{:?}", app.open_error);

    let catalogue = catalogue(data.path());
    assert_eq!(catalogue.projects.len(), workspace.projects.len());
    let active = catalogue
        .projects
        .iter()
        .find(|entry| entry.project_id == workspace.active_id)
        .expect("the active project was migrated");
    assert_eq!(app.root, active.path);

    // The originals are evidence, not a staging area.
    let after = snapshot(data.path());
    for (name, bytes) in &before {
        assert_eq!(after.get(name), Some(bytes), "{name} was modified");
    }

    // The report names the new location and the untouched original.
    let notice = app.notice.as_ref().expect("notice");
    assert!(!notice.error);
    assert!(
        notice.text.contains("workspace.json") && notice.text.contains(PROJECTS_DIRECTORY),
        "{}",
        notice.text
    );
}

#[test]
fn every_migrated_project_is_offered_not_only_the_one_that_opened() {
    let data = tempfile::tempdir().unwrap();
    let workspace = legacy(data.path());
    let mut harness = managed_harness(data.path(), MINIMUM);
    settle(&mut harness);
    let recent = recent(data.path());
    assert_eq!(
        recent.entries.len(),
        workspace.projects.len(),
        "{:?}",
        recent.entries
    );
    for project in &workspace.projects {
        assert!(
            recent.entries.iter().any(|entry| entry.id == project.id),
            "{} is not offered: {:?}",
            project.id,
            recent.entries
        );
    }
}

#[test]
fn a_restart_after_migration_opens_the_catalogue_without_migrating_again() {
    let data = tempfile::tempdir().unwrap();
    legacy(data.path());
    let first = {
        let harness = managed_harness(data.path(), MINIMUM);
        crate::testing::folder(&harness).root.clone()
    };
    let migration = catalogue(data.path()).migration_id;
    let projects = snapshot(
        &data
            .path()
            .join(PROJECTS_DIRECTORY)
            .join(catalogue(data.path()).projects[0].path.file_name().unwrap()),
    );

    let harness = managed_harness(data.path(), MINIMUM);
    assert_eq!(crate::testing::folder(&harness).root, first);
    // The same migration, not a second one, and nothing re-prepared.
    assert_eq!(catalogue(data.path()).migration_id, migration);
    assert_eq!(
        snapshot(
            &data
                .path()
                .join(PROJECTS_DIRECTORY)
                .join(catalogue(data.path()).projects[0].path.file_name().unwrap(),)
        ),
        projects
    );
    // Reopening an already-managed project is not an event worth reporting.
    assert!(crate::testing::folder(&harness).notice.is_none());
}

#[test]
fn the_project_this_computer_was_last_in_is_preferred_over_the_migrated_one() {
    let data = tempfile::tempdir().unwrap();
    legacy(data.path());
    managed_harness(data.path(), MINIMUM);
    let catalogue = catalogue(data.path());
    let other = catalogue
        .projects
        .iter()
        .find(|entry| Some(entry.project_id.as_str()) != catalogue.active_project_id.as_deref())
        .expect("a second migrated project");

    // Leave this computer pointed at a different project, as switching does.
    let file = data.path().join("recent-projects.json");
    let mut stored: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    stored["active"] = serde_json::json!(other.path);
    fs::write(&file, serde_json::to_vec(&stored).unwrap()).unwrap();

    let harness = managed_harness(data.path(), MINIMUM);
    assert_eq!(crate::testing::folder(&harness).root, other.path);
}

#[test]
fn a_workspace_another_window_owns_is_left_to_the_workspace_window() {
    let data = tempfile::tempdir().unwrap();
    legacy(data.path());
    let owner = WorkspaceStore::open_exclusive(data.path().join("workspace.json")).unwrap();
    assert!(matches!(decide(data.path()), Start::Legacy(None)));
    // Nothing was migrated while another window held it.
    assert!(!data.path().join(CATALOGUE_FILE).exists());
    drop(owner);
    assert!(matches!(decide(data.path()), Start::Managed));
}

#[test]
fn a_workspace_awaiting_a_backup_decision_is_left_to_the_workspace_window() {
    let data = tempfile::tempdir().unwrap();
    legacy(data.path());
    // A backup newer than an unreadable primary is what recovery decides on.
    let workspace = data.path().join("workspace.json");
    fs::copy(&workspace, data.path().join("workspace.json.bak")).unwrap();
    fs::write(&workspace, b"{ not a workspace").unwrap();

    let Start::Legacy(Some(reason)) = decide(data.path()) else {
        panic!("a workspace needing recovery is not migrated");
    };
    assert!(reason.contains("recovery"), "{reason}");
    assert!(!data.path().join(CATALOGUE_FILE).exists());
}

#[test]
fn a_migration_that_cannot_finish_activates_nothing_and_reports_it() {
    let data = tempfile::tempdir().unwrap();
    legacy(data.path());
    // Something else already owns one destination.
    let occupied = data.path().join(PROJECTS_DIRECTORY).join("shop");
    fs::create_dir_all(&occupied).unwrap();
    fs::write(occupied.join("theirs.txt"), b"not ours").unwrap();

    let Err(error) = activate(data.path()) else {
        panic!("an occupied destination stops migration");
    };
    assert!(error.to_string().contains("shop"), "{error}");
    assert!(
        !data.path().join(CATALOGUE_FILE).exists(),
        "a failed migration activates nothing"
    );
    assert_eq!(fs::read(occupied.join("theirs.txt")).unwrap(), b"not ours");

    // Clearing the obstruction lets the same migration finish.
    fs::remove_dir_all(&occupied).unwrap();
    let harness = managed_harness(data.path(), MINIMUM);
    assert!(crate::testing::folder(&harness).open_error.is_none());
    assert!(data.path().join(CATALOGUE_FILE).exists());
}
