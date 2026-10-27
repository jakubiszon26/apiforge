//! Stage II cross-package acceptance: what one package writes is what the next
//! one accepts (docs/stage-ii-code-workspace.md, "Acceptance must cover").
//!
//! Every package has its own tests. These cover the seams between them, which
//! the per-package suites cannot see: migration (P3) produces folder projects
//! that only the folder loader (P1/P2) and the folder CLI (P6c) ever read, a
//! revision published through a session (P6b) is executed by a separate
//! process, and the window's run snapshot and the CLI's are built by two
//! different functions that must agree.
use apiforge_core::{
    ImportLimits,
    format::Workspace,
    import_document,
    model::NodeKind,
    run::{RunMode, RunResult, RunStatus},
};
use apiforge_project::{Document, DocumentKind, Origin, ProjectSession, migrate_workspace};
use apiforge_runner::{RunRequest, RunnerService};
use apiforge_storage::WorkspaceStore;
use serde_json::Value;
use std::{
    ffi::OsStr,
    fs,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

fn run(root: &Path, command: &str, rest: &[&OsStr]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .arg(command)
        .arg("--folder")
        .arg(root)
        .args(rest)
        .output()
        .unwrap()
}

fn stdout(output: &std::process::Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}

/// A version-two workspace of every fixture project, written exactly as an
/// installation of the previous version left one.
fn legacy_workspace(data: &Path) -> Workspace {
    let workspace = import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace();
    let store = WorkspaceStore::open_exclusive(data.join("workspace.json")).unwrap();
    store.save(&workspace).unwrap();
    workspace
}

fn document_id(session: &ProjectSession, path: &str) -> String {
    session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} is registered"))
        .id()
        .to_owned()
}

/// A project whose scenario calls one component by an explicit revision, so a
/// publication and a pin are separately observable in what it executes.
const COMPONENT: &str = "components/helper.afs";
const SCENARIO: &str = "scenarios/main.afs";

fn pinned_project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Pinned").unwrap();
    session
        .create_document(
            DocumentKind::Component,
            COMPONENT,
            "component Helper \"Helper\" {\n  export result: number = 2\n}\n",
        )
        .unwrap();
    let component = document_id(&session, COMPONENT);
    session.publish(&component, Origin::SESSION).unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            SCENARIO,
            "scenario \"Main\" {\n  call = (Helper@1())\n  two = assert call == 2\n}\n",
        )
        .unwrap();
    drop(session);
    directory
}

/// Executes `project` the way the folder window does: through the service and
/// the request the window builds from a prepared run.
fn execute(project: apiforge_core::format::Project, scenario_id: &str) -> RunResult {
    let runner = RunnerService::new(Arc::new(|| {})).unwrap();
    let handle = runner
        .start(RunRequest {
            project,
            scenario_id: scenario_id.to_owned(),
            target: apiforge_runner::RunTarget::Scenario,
            mode: RunMode::Simulation,
            target_node_id: None,
            single_node_id: None,
            previous_run: None,
            secrets: Default::default(),
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the run must finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    handle.latest().expect("a finished run has a report")
}

/// A report without the parts that differ between any two executions of the
/// same project: its identity, its clock and how long each step took.
fn comparable(report: &RunResult) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                for key in ["id", "startedAt", "finishedAt", "durationMs"] {
                    fields.remove(key);
                }
                for field in fields.values_mut() {
                    strip(field);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(report).unwrap();
    strip(&mut value);
    value
}

#[test]
fn every_migrated_project_is_accepted_by_the_folder_cli_and_prints_what_its_session_prints() {
    let data = tempfile::tempdir().unwrap();
    let workspace = legacy_workspace(data.path());
    let store = WorkspaceStore::open_exclusive(data.path().join("workspace.json")).unwrap();
    let original = fs::read(store.path()).unwrap();
    let catalogue = migrate_workspace(&store).unwrap().catalogue;
    // The CLI takes the project lock for some commands, so the migration's own
    // legacy session must be gone before the process runs.
    drop(store);
    assert_eq!(catalogue.projects.len(), workspace.projects.len());

    for entry in &catalogue.projects {
        // The folder CLI loads saved definitions only: no recovery, no
        // last-valid cache and no implicit latest revision. Nothing else
        // checks that a migrated project is self-sufficient under that policy.
        let check = run(&entry.path, "check", &[]);
        assert!(
            check.status.success(),
            "{} does not check: {}",
            entry.project_id,
            String::from_utf8_lossy(&check.stderr)
        );

        let session = ProjectSession::open_saved_read_only(&entry.path).unwrap();
        for document in session.documents() {
            let path = document.entry().path.as_str();
            let model = document
                .model()
                .unwrap_or_else(|| panic!("{} {path} has a model", entry.project_id));
            let printed = run(
                &entry.path,
                "print",
                &[OsStr::new("--file"), OsStr::new(path)],
            );
            let expected =
                apiforge_script::print_folder_document(session.context(), model).unwrap();
            assert_eq!(stdout(&printed), expected, "{} {path}", entry.project_id);

            // Migration prints every call with its revision; an unqualified
            // one would load here and fail in the CLI, which forbids them.
            let graphs = match model {
                Document::Scenario(scenario) => vec![&scenario.graph, &scenario.cleanup],
                Document::Component(component) => vec![&component.graph],
            };
            for node in graphs.iter().flat_map(|graph| &graph.nodes) {
                if node.kind == NodeKind::Component {
                    assert!(
                        node.config.revision.is_some(),
                        "{} {path} calls {} without a revision",
                        entry.project_id,
                        node.handle
                    );
                }
            }
        }
    }

    // Migration reads the originals; it never becomes their owner.
    assert_eq!(
        fs::read(data.path().join("workspace.json")).unwrap(),
        original
    );
}

#[test]
fn a_revision_published_through_a_session_changes_what_the_cli_runs_only_once_it_is_pinned() {
    let directory = pinned_project();
    let root = directory.path();
    let scenario = [OsStr::new("--scenario"), OsStr::new(SCENARIO)];

    // The saved project the window left behind runs on its own.
    assert!(run(root, "run", &scenario).status.success());

    // Publish a second revision whose exported value no longer satisfies the
    // scenario's assertion.
    let mut session = ProjectSession::open(root).unwrap();
    let component = document_id(&session, COMPONENT);
    let changed = session
        .document(&component)
        .unwrap()
        .text()
        .replace("= 2", "= 5");
    session
        .edit_source(&component, &changed, Origin::SESSION)
        .unwrap();
    session.settle(&component).unwrap();
    let published = session.publish(&component, Origin::SESSION).unwrap();
    assert_eq!(published.revision, 2);
    drop(session);

    // The existing instance keeps the revision it was written against, so the
    // separate process still checks, prints and passes exactly as before.
    assert!(run(root, "check", &[]).status.success());
    let printed = stdout(&run(
        root,
        "print",
        &[OsStr::new("--file"), OsStr::new(SCENARIO)],
    ));
    assert!(printed.contains("Helper@1("), "{printed}");
    let report: RunResult = serde_json::from_slice(
        &run(
            root,
            "run",
            &[&scenario[..], &[OsStr::new("--json")]].concat(),
        )
        .stdout,
    )
    .unwrap();
    assert_eq!(report.status, RunStatus::Passed);

    // Pinning the newer revision is the explicit step that changes behaviour.
    let mut session = ProjectSession::open(root).unwrap();
    let id = document_id(&session, SCENARIO);
    let repinned = session
        .document(&id)
        .unwrap()
        .text()
        .replace("Helper@1(", "Helper@2(");
    session
        .edit_source(&id, &repinned, Origin::SESSION)
        .unwrap();
    session.settle(&id).unwrap();
    session.save(&id).unwrap();
    drop(session);

    let printed = stdout(&run(
        root,
        "print",
        &[OsStr::new("--file"), OsStr::new(SCENARIO)],
    ));
    assert!(printed.contains("Helper@2("), "{printed}");
    let output = run(root, "run", &scenario);
    assert_eq!(
        output.status.code(),
        Some(1),
        "the newer revision breaks the assertion: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn the_window_and_the_cli_execute_the_same_snapshot_of_a_saved_project() {
    let directory = pinned_project();
    let root = directory.path();

    // The window's snapshot: prepare_run settles analysis and freezes the
    // scenario with its exact dependencies.
    let mut session = ProjectSession::open(root).unwrap();
    let id = document_id(&session, SCENARIO);
    let prepared = session.prepare_run(&id).unwrap();
    assert!(!prepared.last_valid && !prepared.unsaved);
    let project = prepared.project().unwrap();
    let scenario_id = project.scenarios.first().unwrap().id.clone();
    drop(session);

    let mine = execute(project, &scenario_id);
    let theirs: RunResult = serde_json::from_slice(
        &run(
            root,
            "run",
            &[
                OsStr::new("--scenario"),
                OsStr::new(SCENARIO),
                OsStr::new("--json"),
            ],
        )
        .stdout,
    )
    .unwrap();

    assert_eq!(mine.status, RunStatus::Passed);
    assert_eq!(mine.scenario_id, theirs.scenario_id);
    assert_eq!(comparable(&mine), comparable(&theirs));
}
