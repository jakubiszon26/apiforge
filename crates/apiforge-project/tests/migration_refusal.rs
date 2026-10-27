//! KTLO-03: a legacy workspace that migration refuses, and what the refusal
//! leaves behind. The workspaces are synthetic (decision 2 of 2026-10-26),
//! built by `support/legacy_workspace.rs`.
#[path = "support/legacy_workspace.rs"]
mod legacy_workspace;

use apiforge_core::format::{Node, Project, Scenario};
use apiforge_project::*;
use apiforge_storage::project::{DiagramRecord, DocumentKey, IdentityBase};
use legacy_workspace::*;

fn check(scenario: &Scenario) -> &Node {
    scenario
        .graph
        .nodes
        .iter()
        .find(|node| node.handle == "total_is_paid")
        .unwrap()
}

fn input_names(node: &Node) -> Vec<&str> {
    node.inputs.keys().map(String::as_str).collect()
}

/// Criterion 1, the cause. The six-project workspace was refused on its
/// fifth project, "Order desk", with `Verification { reason: "a scenario
/// differs after reloading" }`, nothing activated and four projects left in
/// staging: before KTLO-04 (`280d6bd`) and with it (`18f25a5`, part 1's QA
/// report). Migration now seals its checkpoints in source order (decision 6
/// of 2026-10-27), so the workspace migrates (criterion 2); what remains true
/// is the cause, stated here on the models.
///
/// Its check had its request replaced on the board, so the legacy model holds
/// the check's inputs as `expected, actual`. Printed and read back, the source
/// gives them as `actual, expected`. Compared as values the semantics are
/// equal, so the printer accepts the source, but the checkpoint's semantic
/// digest hashes the inputs in their stored order: a checkpoint sealed from
/// the legacy order does not describe its own source, the reload falls back to
/// reconciling against it, and the reconciled scenario lists its blocks in
/// source order, which differs from the workspace.
#[test]
fn a_check_whose_request_was_replaced_stores_its_inputs_out_of_source_order() {
    let workspace = six_projects();
    // The cause, stated on the models themselves.
    let project = workspace
        .projects
        .iter()
        .find(|project| project.id == ORDER_DESK_ID)
        .unwrap();
    let legacy = &project.scenarios[0];
    assert_eq!(legacy.name, ORDER_TOTAL_NAME);
    let mut context: Project = project.clone();
    context.scenarios.clear();
    let document = Document::Scenario(legacy.clone());
    let text = apiforge_script::print_folder_document(&context, &document)
        .expect("the printer accepts the legacy scenario");
    let lowered = apiforge_script::lower(
        &apiforge_script::parse(&text),
        &apiforge_script::LowerContext::folder(&context),
    );
    let Some(Document::Scenario(read_back)) = lowered.document else {
        panic!("the printed source lowers: {:?}", lowered.diagnostics);
    };
    assert_eq!(input_names(check(legacy)), ["expected", "actual"]);
    assert_eq!(input_names(check(&read_back)), ["actual", "expected"]);
    assert_eq!(
        apiforge_script::semantic(&document, &context),
        apiforge_script::semantic(&Document::Scenario(read_back.clone()), &context),
        "the same semantics, compared as values"
    );
    let sealed = DiagramRecord::capture(
        DocumentKey::Working {
            document_id: "document".into(),
        },
        &text,
        IdentityBase::Scenario(legacy.clone()),
        &context,
    )
    .unwrap();
    assert!(
        !sealed.describes(&text, &IdentityBase::Scenario(read_back), &context),
        "the checkpoint sealed from the legacy model does not describe its source"
    );
}

/// Every file under `root` with its bytes.
fn tree(root: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let name = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                files.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

/// Criterion 2. With KTLO-04 merged and migration sealing its checkpoints in
/// source order (decision 6), the six projects migrate: six folders, every
/// scenario equal after reloading, the catalogue activated.
#[test]
fn the_six_project_workspace_migrates_with_ktlo_04_merged() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = six_projects();
    let store = write(directory.path(), &workspace);

    let outcome = migrate_workspace(&store).expect("the six projects migrate");
    let catalogue = ProjectCatalogue::load(directory.path())
        .unwrap()
        .expect("the catalogue is activated");
    assert_eq!(catalogue, outcome.catalogue);
    assert_eq!(catalogue.projects.len(), 6);
    for project in &workspace.projects {
        let entry = catalogue
            .projects
            .iter()
            .find(|entry| entry.project_id == project.id)
            .unwrap_or_else(|| panic!("{} has a folder", project.id));
        let session = ProjectSession::open(&entry.path).unwrap();
        for scenario in &project.scenarios {
            let reloaded = session
                .documents()
                .iter()
                .find(|document| {
                    document.entry().kind == DocumentKind::Scenario
                        && document.entry().entity_id == scenario.id
                })
                .and_then(|document| document.model());
            assert_eq!(
                reloaded,
                Some(&Document::Scenario(scenario.clone())),
                "{} / {}",
                project.name,
                scenario.name
            );
        }
    }
}

/// Criterion 3. A migration that is still refused names the project and the
/// scenario; the originals are byte-identical before and after, the staged
/// folders are left in staging, and nothing is activated: no catalogue and no
/// project folder. The broken project's scenario reads an environment value
/// the project never sets, which a folder source cannot hold, so it cannot be
/// written as code (`legacy_workspace::partner_feed`).
#[test]
fn a_refused_migration_names_the_project_and_the_scenario_and_activates_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = with_a_broken_project();
    let store = write(directory.path(), &workspace);
    std::fs::write(
        directory.path().join("journal.json"),
        br#"{"version":1,"runs":[]}"#,
    )
    .unwrap();
    let originals = |directory: &std::path::Path| {
        [
            "workspace.json",
            "workspace.json.bak",
            "drafts.json",
            "journal.json",
        ]
        .map(|name| std::fs::read(directory.join(name)).ok())
    };
    let before = originals(directory.path());

    let error = migrate_workspace(&store).expect_err("the broken project is refused");
    let message = error.to_string();
    match &error {
        Error::Migration(MigrationError::Verification {
            project_id,
            project_name,
            scenario,
            reason,
        }) => {
            assert_eq!(project_id, REFUSED_PROJECT_ID, "{message}");
            assert_eq!(project_name, REFUSED_PROJECT_NAME, "{message}");
            assert_eq!(scenario.as_deref(), Some(REFUSED_SCENARIO_NAME));
            assert_eq!(*reason, "a scenario cannot be written as code without loss");
        }
        other => panic!("expected a refusal naming the project, got {other:?}"),
    }
    assert!(message.contains(REFUSED_PROJECT_NAME), "{message}");
    assert!(message.contains(REFUSED_SCENARIO_NAME), "{message}");
    assert_eq!(originals(directory.path()), before, "originals untouched");
    assert!(!directory.path().join(CATALOGUE_FILE).exists());
    assert!(
        !directory.path().join(PROJECTS_DIRECTORY).exists(),
        "no project is published"
    );

    // The staged folders stay for inspection: the project prepared before the
    // refused one, and the refused one's folder, which is refused before
    // anything is written into it. The next start, which migrates again
    // before it decides what to show, is refused the same way.
    let staged = tree(&directory.path().join("migration"));
    assert!(
        staged
            .keys()
            .any(|path| path.ends_with("support-desk/apiforge.project.json")),
        "the project before it stays staged: {:?}",
        staged.keys().collect::<Vec<_>>()
    );
    let index: serde_json::Value = serde_json::from_slice(&staged["index.json"]).unwrap();
    let migration_id = index["migrationId"].as_str().unwrap();
    assert!(
        directory
            .path()
            .join("migration")
            .join(migration_id)
            .join(REFUSED_PROJECT_ID)
            .is_dir(),
        "the refused project's staged folder is kept"
    );
    drop(store);
    let store =
        apiforge_storage::WorkspaceStore::open_exclusive(directory.path().join("workspace.json"))
            .unwrap();
    let again = migrate_workspace(&store).expect_err("refused again");
    assert_eq!(again.to_string(), message);
    assert_eq!(originals(directory.path()), before);
    assert!(!directory.path().join(CATALOGUE_FILE).exists());
}
