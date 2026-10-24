//! Upgrade of a version-two workspace into folder projects.
use apiforge_core::{
    ImportLimits,
    format::{Position, ProxySource, Workspace},
    import_document,
};
use apiforge_project::*;
use apiforge_storage::{CodeDraft, DraftTarget, Drafts, WorkspaceStore, draft_base_hash};
use std::{collections::BTreeMap, fs, path::Path};

const INPUTS: [&str; 3] = ["workspace.json", "drafts.json", "journal.json"];

/// The fixture workspace, with proxies and a second revision of a component
/// whose first revision the shop scenario keeps using.
fn workspace() -> Workspace {
    let mut workspace = import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace();
    let shop = workspace
        .projects
        .iter_mut()
        .find(|project| project.id == "shop")
        .unwrap();
    let scenario = &mut shop.scenarios[0];
    let value = scenario
        .graph
        .nodes
        .iter_mut()
        .find(|node| node.kind == apiforge_core::model::NodeKind::Value)
        .unwrap();
    value.config.variable = Some(true);
    let variable = value.id.clone();
    scenario.graph = apiforge_core::edit::add_proxy(
        &scenario.graph,
        ProxySource::Variable { node_id: variable },
        Position { x: 640.0, y: -96.0 },
    )
    .unwrap();
    let mut newer = shop.components[0].clone();
    newer.revision = 2;
    newer.name.push_str(" v2");
    shop.components.push(newer);
    workspace
}

fn printed(workspace: &Workspace, project: &str, document: &Document) -> String {
    let project = workspace.projects.iter().find(|p| p.id == project).unwrap();
    match document {
        Document::Scenario(scenario) => apiforge_script::print_scenario(project, scenario),
        Document::Component(component) => apiforge_script::print_component(project, component),
    }
    .unwrap()
}

struct Legacy {
    directory: tempfile::TempDir,
    store: WorkspaceStore,
    workspace: Workspace,
}

fn legacy() -> Legacy {
    let directory = tempfile::tempdir().unwrap();
    let store = WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
    let workspace = workspace();
    store.save(&workspace).unwrap();
    let shop = workspace.projects.iter().find(|p| p.id == "shop").unwrap();
    let tickets = workspace
        .projects
        .iter()
        .find(|p| p.id == "tickets")
        .unwrap();
    let scenario = Document::Scenario(shop.scenarios[0].clone());
    let old = shop
        .components
        .iter()
        .find(|component| component.revision == 1)
        .unwrap();
    let old_text = printed(&workspace, "shop", &Document::Component(old.clone()));
    let scenario_text = printed(&workspace, "shop", &scenario);
    let tickets_text = printed(
        &workspace,
        "tickets",
        &Document::Scenario(tickets.scenarios[0].clone()),
    );
    let drafts = Drafts {
        version: 1,
        entries: vec![
            CodeDraft {
                project_id: "shop".into(),
                target: DraftTarget::Scenario {
                    id: shop.scenarios[0].id.clone(),
                },
                base_hash: draft_base_hash(&scenario_text),
                text: format!("{scenario_text}\n"),
                cursor: 3,
            },
            CodeDraft {
                project_id: "shop".into(),
                target: DraftTarget::Component {
                    id: old.id.clone(),
                    revision: 1,
                },
                base_hash: draft_base_hash(&old_text),
                text: old_text.replace(&old.name, "Unfinished rename"),
                cursor: 0,
            },
            // An inactive project's unfinished inspector field.
            CodeDraft {
                project_id: "tickets".into(),
                target: DraftTarget::Field {
                    owner: Box::new(DraftTarget::Scenario {
                        id: tickets.scenarios[0].id.clone(),
                    }),
                    widget: 42,
                },
                base_hash: draft_base_hash(&tickets_text),
                text: "{ broken".into(),
                cursor: 2,
            },
            CodeDraft {
                project_id: "shop".into(),
                target: DraftTarget::Scenario {
                    id: "deleted-scenario".into(),
                },
                base_hash: draft_base_hash("gone"),
                text: "scenario \"Gone\" {}".into(),
                cursor: 0,
            },
        ],
    };
    store.save_drafts(&drafts, &[]).unwrap();
    fs::write(
        directory.path().join("journal.json"),
        br#"{"version":1,"runs":[]}"#,
    )
    .unwrap();
    Legacy {
        directory,
        store,
        workspace,
    }
}

fn originals(directory: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    INPUTS
        .iter()
        .map(|name| ((*name).to_owned(), fs::read(directory.join(name)).ok()))
        .collect()
}

fn document<'a>(
    session: &'a ProjectSession,
    kind: DocumentKind,
    entity: &str,
) -> &'a DocumentSession {
    session
        .documents()
        .iter()
        .find(|d| d.entry().kind == kind && d.entry().entity_id == entity && !d.entry().recovered)
        .unwrap()
}

#[test]
fn every_project_and_draft_migrates_into_an_activated_catalogue() {
    let legacy = legacy();
    let before = originals(legacy.directory.path());
    let outcome = migrate_workspace(&legacy.store).unwrap();
    assert!(!outcome.already_complete && !outcome.resumed);
    assert_eq!(
        originals(legacy.directory.path()),
        before,
        "originals untouched"
    );
    assert_eq!(outcome.unmigrated_drafts.len(), 1);
    let catalogue = ProjectCatalogue::load(legacy.directory.path())
        .unwrap()
        .unwrap();
    assert_eq!(catalogue, outcome.catalogue);
    assert_eq!(catalogue.projects.len(), legacy.workspace.projects.len());
    assert_eq!(
        catalogue.active_project_id.as_deref(),
        Some(legacy.workspace.active_id.as_str())
    );
    assert_eq!(
        catalogue.legacy_journal.as_deref(),
        Some(legacy.directory.path().join("journal.json").as_path())
    );
    let migration = legacy.directory.path().join("migration");
    let leftovers: Vec<_> = fs::read_dir(&migration)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        leftovers,
        ["index.json"],
        "staging is removed, the index stays"
    );

    let shop_path = &catalogue
        .projects
        .iter()
        .find(|p| p.project_id == "shop")
        .unwrap()
        .path;
    let shop = ProjectSession::open(shop_path).unwrap();
    assert!(
        shop.recovered_drafts().is_empty(),
        "matching drafts restore"
    );
    let original = &legacy
        .workspace
        .projects
        .iter()
        .find(|p| p.id == "shop")
        .unwrap()
        .scenarios[0];
    let scenario = document(&shop, DocumentKind::Scenario, &original.id);
    assert!(scenario.is_dirty(), "the unsaved scenario draft is back");
    assert_eq!(scenario.selection().caret, 3);
    assert_eq!(
        scenario.model(),
        Some(&Document::Scenario(original.clone()))
    );

    // The older component draft became its own recovered working document.
    let recovered: Vec<_> = shop
        .documents()
        .iter()
        .filter(|d| d.entry().recovered)
        .collect();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].entry().base_revision, Some(1));
    assert!(recovered[0].text().contains("Unfinished rename"));
    assert!(recovered[0].is_dirty());
    let working: Vec<_> = shop
        .documents()
        .iter()
        .filter(|d| d.entry().entity_id == recovered[0].entry().entity_id && !d.entry().recovered)
        .collect();
    assert_eq!(
        working[0].entry().base_revision,
        Some(2),
        "primary stays on latest"
    );
    assert!(!working[0].is_dirty());

    let tickets_path = &catalogue
        .projects
        .iter()
        .find(|p| p.project_id == "tickets")
        .unwrap()
        .path;
    let tickets = ProjectSession::open(tickets_path).unwrap();
    let id = &legacy
        .workspace
        .projects
        .iter()
        .find(|p| p.id == "tickets")
        .unwrap()
        .scenarios[0]
        .id;
    let scenario = document(&tickets, DocumentKind::Scenario, id);
    assert!(!scenario.is_dirty());
    assert_eq!(scenario.field_drafts().len(), 1);
    assert_eq!(scenario.field_drafts()[0].widget, 42);
    assert_eq!(scenario.field_drafts()[0].text, "{ broken");

    let again = migrate_workspace(&legacy.store).unwrap();
    assert!(again.already_complete);
    assert_eq!(again.catalogue, catalogue);
}

#[test]
fn an_occupied_destination_stops_before_activation_and_a_retry_resumes() {
    let legacy = legacy();
    let projects = legacy.directory.path().join("projects");
    let foreign = projects.join("tickets");
    fs::create_dir_all(&foreign).unwrap();
    fs::write(foreign.join("notes.txt"), "mine").unwrap();
    assert!(matches!(
        migrate_workspace(&legacy.store),
        Err(Error::Migration(MigrationError::Occupied { .. }))
    ));
    assert!(
        ProjectCatalogue::load(legacy.directory.path())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fs::read_to_string(foreign.join("notes.txt")).unwrap(),
        "mine"
    );
    assert_eq!(
        fs::read_dir(&projects).unwrap().count(),
        1,
        "nothing published"
    );

    fs::remove_dir_all(&foreign).unwrap();
    let outcome = migrate_workspace(&legacy.store).unwrap();
    assert!(outcome.resumed);
    assert_eq!(
        fs::read_dir(&projects).unwrap().count(),
        legacy.workspace.projects.len()
    );
}

#[test]
fn a_crash_after_publication_reuses_its_own_folders_but_never_foreign_changes() {
    let legacy = legacy();
    let catalogue_path = legacy.directory.path().join(CATALOGUE_FILE);
    let first = migrate_workspace(&legacy.store).unwrap();
    // Simulate a crash before the catalogue: the index still lists publications.
    let index = legacy.directory.path().join("migration/index.json");
    let index_bytes = fs::read(&index);
    assert!(index_bytes.is_ok(), "the index survives until activation");
    fs::remove_file(&catalogue_path).unwrap();
    let second = migrate_workspace(&legacy.store).unwrap();
    assert!(second.resumed);
    assert_eq!(second.catalogue.migration_id, first.catalogue.migration_id);

    fs::remove_file(&catalogue_path).unwrap();
    let edited = second.catalogue.projects[0]
        .path
        .join("apiforge.project.json");
    let mut bytes = fs::read(&edited).unwrap();
    bytes.push(b'\n');
    fs::write(&edited, &bytes).unwrap();
    assert!(matches!(
        migrate_workspace(&legacy.store),
        Err(Error::Migration(MigrationError::Occupied { .. }))
    ));
    assert_eq!(fs::read(&edited).unwrap(), bytes, "never overwritten");
}

#[test]
fn changed_inputs_start_a_new_migration_and_read_only_sessions_cannot_migrate() {
    let legacy = legacy();
    let foreign = legacy.directory.path().join("projects/workflow-poc");
    fs::create_dir_all(&foreign).unwrap();
    let first = migrate_workspace(&legacy.store);
    assert!(first.is_err());
    let mut workspace = legacy.workspace.clone();
    workspace.projects[0].name.push_str(" renamed");
    legacy.store.save(&workspace).unwrap();
    fs::remove_dir_all(&foreign).unwrap();
    let outcome = migrate_workspace(&legacy.store).unwrap();
    assert!(!outcome.resumed);
    let reader = WorkspaceStore::open_read_only(legacy.store.path()).unwrap();
    assert!(matches!(
        migrate_workspace(&reader),
        Err(Error::Migration(MigrationError::ReadOnly))
    ));
}

#[test]
fn every_fixture_document_migrates_and_verifies() {
    for entry in fs::read_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/projects"
    ))
    .unwrap()
    {
        let bytes = fs::read(entry.unwrap().path()).unwrap();
        let workspace = import_document(&bytes, &ImportLimits::default())
            .unwrap()
            .into_workspace();
        let directory = tempfile::tempdir().unwrap();
        let store =
            WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
        store.save(&workspace).unwrap();
        let outcome = migrate_workspace(&store).unwrap();
        assert_eq!(outcome.catalogue.projects.len(), workspace.projects.len());
        for project in &outcome.catalogue.projects {
            let session = ProjectSession::open(&project.path).unwrap();
            assert!(session.documents().iter().all(|d| !d.is_dirty()));
        }
    }
}

#[test]
fn an_interrupted_publication_finishes_the_remaining_projects() {
    let legacy = legacy();
    let first = migrate_workspace(&legacy.store).unwrap();
    // Recreate the state after publishing all but the last project.
    let data = legacy.directory.path();
    fs::remove_file(data.join(CATALOGUE_FILE)).unwrap();
    let index_path = data.join("migration/index.json");
    let mut index: serde_json::Value =
        serde_json::from_slice(&fs::read(&index_path).unwrap()).unwrap();
    let last = index["projects"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap();
    last["published"] = false.into();
    let project_id = last["projectId"].as_str().unwrap().to_owned();
    fs::write(&index_path, serde_json::to_vec_pretty(&index).unwrap()).unwrap();
    let staging = data.join("migration").join(&first.catalogue.migration_id);
    fs::create_dir_all(&staging).unwrap();
    fs::rename(
        data.join("projects").join(&project_id),
        staging.join(&project_id),
    )
    .unwrap();

    let outcome = migrate_workspace(&legacy.store).unwrap();
    assert!(outcome.resumed);
    assert_eq!(outcome.catalogue, first.catalogue);
    assert!(data.join("projects").join(&project_id).is_dir());
    ProjectSession::open(data.join("projects").join(&project_id)).unwrap();
}
