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

/// A statement the document did not have gets a position from the same layout
/// the board uses: beside its neighbours, and on top of nothing.
#[test]
fn applying_a_new_statement_places_it_clear_of_the_other_blocks() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let source = directory.path().join("scenario.afs");
    let workspace = fixture();
    let project = workspace
        .projects
        .iter()
        .find(|project| project.id == workspace.active_id)
        .unwrap();
    let scenario = &project.scenarios[0];
    fs::write(&path, serde_json::to_vec_pretty(&workspace).unwrap()).unwrap();
    let printed = run("print", &path, Some(&scenario.id), None);
    assert!(printed.status.success());
    let text = String::from_utf8(printed.stdout).unwrap();
    let before: Vec<(String, (f64, f64))> = scenario
        .graph
        .nodes
        .iter()
        .map(|node| (node.id.clone(), (node.position.x, node.position.y)))
        .collect();

    // One more statement, written at the end of the scenario body.
    let closing = text.rfind('}').expect("the scenario body is closed");
    let mut extended = text.clone();
    extended.insert_str(closing, "  var fresh_total = 1\n");
    fs::write(&source, extended).unwrap();
    let applied = run("apply", &path, Some(&scenario.id), Some(&source));
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );

    let saved: Workspace = import_document(&fs::read(&path).unwrap(), &ImportLimits::default())
        .unwrap()
        .into_workspace();
    let saved_scenario = &saved
        .projects
        .iter()
        .find(|project| project.id == saved.active_id)
        .unwrap()
        .scenarios[0];
    assert_eq!(saved_scenario.graph.nodes.len(), before.len() + 1);
    // Every block that already existed stayed exactly where it was.
    for (id, position) in &before {
        let node = saved_scenario
            .graph
            .nodes
            .iter()
            .find(|node| &node.id == id)
            .expect("an existing block kept its identity");
        assert_eq!((node.position.x, node.position.y), *position);
    }
    let fresh = saved_scenario
        .graph
        .nodes
        .iter()
        .find(|node| node.handle == "fresh_total")
        .expect("the new block was added");
    assert!(fresh.auto_placed, "the layout owns the position it chose");
    // Nominal cards of 280 × 140 must not overlap.
    for (_, (x, y)) in &before {
        let apart = (fresh.position.x - x).abs() >= 280.0 || (fresh.position.y - y).abs() >= 140.0;
        assert!(
            apart,
            "the new block at {:?} lands on a block at {:?}",
            (fresh.position.x, fresh.position.y),
            (x, y)
        );
    }
}

/// An error on line 3; a warning on line 1 after three two-byte characters,
/// at character column 17 (byte column 20).
const MIXED: &str = "scenario \"Żółw\" // c\n{\n  other = )\n}\n";

/// The 1-based line and the column, counted in characters, of `offset`.
fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
    (line, column)
}

/// Whether a line of output carries a byte range, `\d+\.\.\d+`.
fn has_byte_range(line: &str) -> bool {
    line.as_bytes()
        .windows(4)
        .any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2] == b'.' && w[3].is_ascii_digit())
}

/// #346, criterion 2: `check --workspace … <file.afs>` prints
/// `path:line:column: severity: message` with the path as given.
#[test]
fn issue346_criterion_2_workspace_check_of_a_file_prints_path_line_and_column() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let source = directory.path().join("źródło.afs");
    fs::write(&path, serde_json::to_vec(&fixture()).unwrap()).unwrap();
    fs::write(&source, MIXED).unwrap();
    let output = run("check", &path, None, Some(&source));
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let lines: Vec<&str> = stderr.lines().collect();
    let shown = source.display();
    assert!(
        lines.contains(&format!("{shown}:3:11: error: Expected an expression").as_str()),
        "{stderr}"
    );
    assert!(
        lines.contains(
            &format!(
                "{shown}:1:17: warning: {}",
                apiforge_script::UNATTACHED_COMMENT
            )
            .as_str()
        ),
        "{stderr}"
    );
    assert!(!lines.iter().any(|line| has_byte_range(line)), "{stderr}");
    assert_eq!(output.status.code(), Some(1));
}

/// #346, criterion 2: `check --workspace` without a file checks text that has
/// no file, so its lines are `line:column: severity: message`.
#[test]
fn issue346_criterion_2_workspace_check_without_a_file_prints_line_and_column() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut workspace = fixture();
    let active = workspace.active_id.clone();
    let project = workspace
        .projects
        .iter_mut()
        .find(|p| p.id == active)
        .unwrap();
    let node = project.scenarios[0]
        .graph
        .nodes
        .iter_mut()
        .find(|node| node.inputs.contains_key("id"))
        .expect("a node with an `id` input");
    node.inputs.insert(
        "id".into(),
        apiforge_core::format::Binding::Environment {
            name: "missing".into(),
        },
    );
    let printed = print_scenario(project, &project.scenarios[0]).unwrap();
    let (line, column) = line_column(&printed, printed.find("env.missing").unwrap());
    fs::write(&path, serde_json::to_vec(&workspace).unwrap()).unwrap();
    let output = run("check", &path, None, None);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let lines: Vec<&str> = stderr.lines().collect();
    assert!(
        lines.contains(&format!("{line}:{column}: error: Unknown environment name").as_str()),
        "{stderr}"
    );
    assert!(!lines.iter().any(|line| has_byte_range(line)), "{stderr}");
    assert_eq!(output.status.code(), Some(1));
}
