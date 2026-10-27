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

/// Criterion 1. Of the six projects, the fifth, "Order desk", is refused with
/// `Verification { reason: "a scenario differs after reloading" }`, nothing is
/// activated and the four projects prepared before it stay in staging.
///
/// Why: its check had its request replaced on the board, so the legacy model
/// holds the check's inputs as `expected, actual`. Printed and read back, the
/// source gives them as `actual, expected`. Compared as values the semantics
/// are equal, so the printer accepts the source, but the checkpoint's semantic digest hashes the
/// inputs in their stored order: the checkpoint sealed from the legacy model
/// does not describe its own source, the reload falls back to reconciling
/// against it, and the reconciled scenario lists its blocks in source order,
/// which differs from the workspace.
#[test]
fn a_six_project_workspace_is_refused_on_a_check_whose_request_was_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = six_projects();
    let store = write(directory.path(), &workspace);

    let error = migrate_workspace(&store).expect_err("the workspace is refused");
    match &error {
        Error::Migration(MigrationError::Verification {
            project_id, reason, ..
        }) => {
            assert_eq!(project_id, REFUSED_PROJECT_ID, "{error}");
            assert_eq!(*reason, "a scenario differs after reloading", "{error}");
        }
        other => panic!("expected a verification refusal, got {other:?}"),
    }
    assert!(
        !directory.path().join(CATALOGUE_FILE).exists(),
        "nothing is activated"
    );
    let index: serde_json::Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("migration/index.json")).unwrap(),
    )
    .unwrap();
    let prepared: Vec<&str> = index["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["projectId"].as_str().unwrap())
        .collect();
    let demos: Vec<String> = workspace.projects[..4]
        .iter()
        .map(|project| project.id.clone())
        .collect();
    assert_eq!(prepared, demos, "the four projects before it stay staged");

    // The cause, stated on the models themselves.
    let project = workspace
        .projects
        .iter()
        .find(|project| project.id == REFUSED_PROJECT_ID)
        .unwrap();
    let legacy = &project.scenarios[0];
    assert_eq!(legacy.name, REFUSED_SCENARIO_NAME);
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
