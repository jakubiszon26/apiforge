use apiforge_core::{ImportLimits, format::Workspace, import_document};
use apiforge_script::{Document, print_scenario, semantic};
use std::{
    ffi::OsStr,
    fs,
    process::{Command, Output},
};

fn fixture() -> Workspace {
    import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
}
fn run(
    command: &str,
    path: &std::path::Path,
    selector: Option<&str>,
    source: Option<&std::path::Path>,
) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_apiforge"));
    cmd.args([
        OsStr::new(command),
        OsStr::new("--workspace"),
        path.as_os_str(),
    ]);
    if let Some(selector) = selector {
        cmd.args(["--scenario", selector]);
    }
    if let Some(source) = source {
        cmd.arg(source);
    }
    cmd.output().unwrap()
}
#[test]
fn print_check_and_apply_preserve_envelopes_other_projects_and_identity() {
    for standalone in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        let source = directory.path().join("scenario.afs");
        let workspace = fixture();
        let project = &workspace
            .projects
            .iter()
            .find(|p| p.id == workspace.active_id)
            .unwrap();
        let scenario = &project.scenarios[0];
        let selector = &scenario.id;
        let original = if standalone {
            serde_json::to_vec_pretty(project).unwrap()
        } else {
            serde_json::to_vec_pretty(&workspace).unwrap()
        };
        fs::write(&path, &original).unwrap();
        let printed = run("print", &path, Some(selector), None);
        assert!(
            printed.status.success(),
            "{}",
            String::from_utf8_lossy(&printed.stderr)
        );
        fs::write(&source, &printed.stdout).unwrap();
        assert!(
            run("check", &path, Some(selector), Some(&source))
                .status
                .success()
        );
        assert!(run("check", &path, None, None).status.success());
        assert_eq!(fs::read(&path).unwrap(), original);
        // Applying printed source is a no-op: no backup rotation or ID churn.
        assert!(
            run("apply", &path, Some(selector), Some(&source))
                .status
                .success()
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(!path.with_extension("json.bak").exists());
        let text = String::from_utf8(printed.stdout).unwrap().replacen(
            "scenario \"",
            "scenario \"Renamed ",
            1,
        );
        fs::write(&source, text).unwrap();
        let applied = run("apply", &path, Some(selector), Some(&source));
        assert!(
            applied.status.success(),
            "{}",
            String::from_utf8_lossy(&applied.stderr)
        );
        let document =
            import_document(&fs::read(&path).unwrap(), &ImportLimits::default()).unwrap();
        assert_eq!(document.is_standalone_project(), standalone);
        let updated = document
            .workspace()
            .projects
            .iter()
            .find(|p| p.id == project.id)
            .unwrap();
        assert_eq!(
            updated.scenarios[0].name,
            format!("Renamed {}", scenario.name)
        );
        assert_eq!(
            updated.scenarios[0]
                .graph
                .nodes
                .iter()
                .map(|n| (&n.id, &n.position))
                .collect::<Vec<_>>(),
            scenario
                .graph
                .nodes
                .iter()
                .map(|n| (&n.id, &n.position))
                .collect::<Vec<_>>()
        );
        assert_eq!(&updated.scenarios[1..], &project.scenarios[1..]);
        assert_eq!(updated.components, project.components);
        if !standalone {
            assert_eq!(
                document
                    .workspace()
                    .projects
                    .iter()
                    .filter(|p| p.id != project.id)
                    .collect::<Vec<_>>(),
                workspace
                    .projects
                    .iter()
                    .filter(|p| p.id != project.id)
                    .collect::<Vec<_>>()
            );
        }
        let backup = import_document(
            &fs::read(path.with_extension("json.bak")).unwrap(),
            &ImportLimits::default(),
        )
        .unwrap();
        assert_eq!(backup.is_standalone_project(), standalone);
        let backed = backup
            .workspace()
            .projects
            .iter()
            .find(|p| p.id == project.id)
            .unwrap();
        assert_eq!(
            semantic(&Document::Scenario(backed.scenarios[0].clone()), backed),
            semantic(&Document::Scenario(scenario.clone()), project)
        );
    }
}
#[test]
fn apply_refuses_locked_corrupt_newer_and_recovery_candidates_without_touching_them() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let source = directory.path().join("source.afs");
    let workspace = fixture();
    let project = workspace
        .projects
        .iter()
        .find(|p| p.id == workspace.active_id)
        .unwrap();
    let scenario = &project.scenarios[0];
    fs::write(&source, print_scenario(project, scenario).unwrap()).unwrap();
    let valid = serde_json::to_vec(&workspace).unwrap();
    fs::write(&path, &valid).unwrap();
    let store = apiforge_storage::WorkspaceStore::open_exclusive(&path).unwrap();
    assert_eq!(
        run("apply", &path, Some(&scenario.id), Some(&source))
            .status
            .code(),
        Some(1)
    );
    assert_eq!(fs::read(&path).unwrap(), valid);
    drop(store);
    let backup_path = path.with_extension("json.bak");
    fs::write(&backup_path, &valid).unwrap();
    for bytes in [
        b"{ incomplete".as_slice(),
        b"{\"version\":99}".as_slice(),
        b"{\"version\":1}".as_slice(),
    ] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            run("apply", &path, Some(&scenario.id), Some(&source))
                .status
                .code(),
            Some(1)
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(&backup_path).unwrap(), valid);
    }
    fs::remove_file(&path).unwrap();
    assert_eq!(
        run("apply", &path, Some(&scenario.id), Some(&source))
            .status
            .code(),
        Some(1)
    );
    assert!(!path.exists());
    assert_eq!(fs::read(&backup_path).unwrap(), valid);
}
#[test]
fn invalid_source_and_ambiguous_selectors_are_read_only_with_private_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let source = directory.path().join("source.afs");
    let mut workspace = fixture();
    let project = workspace
        .projects
        .iter_mut()
        .find(|p| p.id == workspace.active_id)
        .unwrap();
    let mut duplicate = project.scenarios[0].clone();
    duplicate.id.push_str("-copy");
    project.scenarios.push(duplicate);
    let name = project.scenarios[0].name.clone();
    let id = project.scenarios[0].id.clone();
    let original = serde_json::to_vec(&workspace).unwrap();
    fs::write(&path, &original).unwrap();
    fs::write(
        &source,
        "scenario \"Private\" { @unknown(\"canary-private-value\") x = unknown }",
    )
    .unwrap();
    for (command, selector, source) in [
        ("apply", &id, Some(source.as_path())),
        ("check", &id, Some(source.as_path())),
        ("print", &name, None),
    ] {
        let output = run(command, &path, Some(selector), source);
        assert_eq!(output.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("canary-private-value"));
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}
#[test]
fn malformed_document_command_arguments_exit_two() {
    for args in [
        vec!["check"],
        vec!["print", "--workspace", "x"],
        vec!["apply", "--workspace", "x", "--scenario", "y"],
        vec!["check", "--workspace", "x", "--workspace", "y"],
        vec!["check", "--workspace", "x", "--unknown"],
    ] {
        assert_eq!(
            Command::new(env!("CARGO_BIN_EXE_apiforge"))
                .args(args)
                .output()
                .unwrap()
                .status
                .code(),
            Some(2)
        );
    }
}
