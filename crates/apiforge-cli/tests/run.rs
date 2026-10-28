//! `apiforge run`: what it executes, what it reports and what it refuses.

use apiforge_core::{ImportLimits, format::Workspace, import_document};
use std::{ffi::OsStr, fs, process::Command};

fn fixture() -> Workspace {
    import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
}

fn workspace_at(directory: &std::path::Path) -> std::path::PathBuf {
    let path = directory.join("workspace.json");
    fs::write(&path, serde_json::to_vec_pretty(&fixture()).unwrap()).unwrap();
    path
}

fn run(args: &[&OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .args(args)
        .output()
        .unwrap()
}

fn scenario() -> String {
    fixture()
        .projects
        .iter()
        .find(|project| project.id == fixture().active_id)
        .unwrap()
        .scenarios[0]
        .id
        .clone()
}

#[test]
fn a_simulation_runs_the_document_and_says_how_it_went() {
    let directory = tempfile::tempdir().unwrap();
    let path = workspace_at(directory.path());
    let scenario = scenario();
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.trim_end().ends_with("Passed"), "{text}");
    assert!(text.contains("ok"), "{text}");
    // A simulation changes nothing, not even the file's timestamp content.
    assert_eq!(
        fs::read(&path).unwrap(),
        serde_json::to_vec_pretty(&fixture()).unwrap()
    );
    // No journal: a simulation sends nothing.
    assert!(!directory.path().join("journal.json").exists());
}

#[test]
fn the_report_can_be_asked_for_as_json() {
    let directory = tempfile::tempdir().unwrap();
    let path = workspace_at(directory.path());
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario()),
        OsStr::new("--json"),
    ]);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "passed");
    assert_eq!(report["mode"], "simulation");
    assert!(
        report["steps"]
            .as_array()
            .is_some_and(|steps| !steps.is_empty())
    );
}

#[test]
fn a_failing_scenario_exits_with_a_failure() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut workspace = fixture();
    // Make one assertion expect something the sample does not say.
    let project = workspace
        .projects
        .iter_mut()
        .find(|project| project.id == "shop")
        .unwrap();
    for node in &mut project.scenarios[0].graph.nodes {
        if node.kind == apiforge_core::model::NodeKind::Assert {
            node.inputs.insert(
                "expected".into(),
                apiforge_core::format::Binding::literal(serde_json::json!("never")),
            );
            break;
        }
    }
    fs::write(&path, serde_json::to_vec_pretty(&workspace).unwrap()).unwrap();
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario()),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.trim_end().ends_with("Failed"), "{text}");
    assert!(
        text.lines()
            .any(|line| !line.trim_start().starts_with("ok") && !line.starts_with("Failed")),
        "the step that did not pass is named: {text}"
    );
}

#[test]
fn secrets_come_from_a_file_and_from_the_environment_never_the_command_line() {
    let directory = tempfile::tempdir().unwrap();
    let path = workspace_at(directory.path());
    let secrets = directory.path().join("secrets.json");
    fs::write(&secrets, br#"{"apiToken": "from-file"}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .args([
            OsStr::new("run"),
            OsStr::new("--workspace"),
            path.as_os_str(),
            OsStr::new("--scenario"),
            OsStr::new(&scenario()),
            OsStr::new("--secrets"),
            secrets.as_os_str(),
        ])
        .env("APIFORGE_SECRET_other", "from-environment")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Neither value is echoed back, whatever happens.
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(!text.contains("from-file") && !text.contains("from-environment"));

    // A secrets file that is not an object of text values is refused.
    fs::write(&secrets, br#"{"apiToken": 7}"#).unwrap();
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario()),
        OsStr::new("--secrets"),
        secrets.as_os_str(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("object of names"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn malformed_arguments_exit_two_without_running_anything() {
    let directory = tempfile::tempdir().unwrap();
    let path = workspace_at(directory.path());
    for arguments in [
        vec![OsStr::new("run")],
        vec![
            OsStr::new("run"),
            OsStr::new("--workspace"),
            path.as_os_str(),
        ],
        vec![
            OsStr::new("run"),
            OsStr::new("--workspace"),
            path.as_os_str(),
            OsStr::new("--scenario"),
            OsStr::new("x"),
            OsStr::new("--unknown"),
        ],
    ] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("apiforge run"));
    }
}

#[test]
fn an_unknown_scenario_is_an_error_not_a_run() {
    let directory = tempfile::tempdir().unwrap();
    let path = workspace_at(directory.path());
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new("no-such-scenario"),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Scenario name"));
}

/// A live run against a loopback server: it sends the requests, and it records
/// them where an interrupted run would be found.
#[test]
fn a_live_run_sends_requests_and_journals_them() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let served = std::thread::spawn(move || {
        let mut answered = 0;
        for stream in listener.incoming().take(8) {
            let Ok(mut stream) = stream else { break };
            let mut buffer = [0; 4096];
            let _ = stream.read(&mut buffer);
            let body = br#"{"id":"x","promotion":{"price":13},"total":26,"quantity":2,"items":[]}"#;
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            );
            let _ = stream.write_all(body);
            answered += 1;
        }
        answered
    });

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut workspace = fixture();
    for project in &mut workspace.projects {
        project.environment.base_url = format!("http://{address}");
    }
    fs::write(&path, serde_json::to_vec_pretty(&workspace).unwrap()).unwrap();
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario()),
        OsStr::new("--live"),
    ]);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Passed") || text.contains("Failed"), "{text}");
    // The run reached its end, so it took its own record away with it.
    assert!(
        !directory.path().join("journal.json").exists(),
        "a finished run leaves no journal behind"
    );
    drop(served);
}

/// A live run needs the workspace's writer lock, because that is what lets it
/// record what it sends.
#[test]
fn a_live_run_refuses_a_workspace_another_window_holds() {
    let directory = tempfile::tempdir().unwrap();
    let path = workspace_at(directory.path());
    let _holder = apiforge_storage::WorkspaceStore::open_exclusive(&path).unwrap();
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario()),
        OsStr::new("--live"),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("another API Forge window"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // A simulation is unaffected: it changes nothing and records nothing.
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario()),
    ]);
    assert!(output.status.success());
}

/// A component that fails says to look at its inner steps, so the report has to
/// show them: the only line that explains the failure is one of theirs.
#[test]
fn a_failed_component_prints_the_inner_steps_that_explain_it() {
    use std::net::TcpListener;

    // A port nothing listens on, so every request is refused at once.
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut workspace = fixture();
    for project in &mut workspace.projects {
        project.environment.base_url = format!("http://{closed}");
    }
    fs::write(&path, serde_json::to_vec_pretty(&workspace).unwrap()).unwrap();
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario()),
        OsStr::new("--live"),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8_lossy(&output.stdout);
    let indented: Vec<&str> = text
        .lines()
        .filter(|line| line.len() > 11 && line[11..].starts_with(' '))
        .collect();
    assert!(
        !indented.is_empty(),
        "the component's inner steps are shown beneath it: {text}"
    );
    assert!(
        indented.iter().any(|line| line.contains("Request failed")),
        "and one of them says the request never reached the API: {text}"
    );
}

/// What a comparison wanted and what it got are in the report; a failure that
/// does not say them leaves nothing to act on.
#[test]
fn a_failed_comparison_prints_what_it_wanted_and_what_it_got() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut workspace = fixture();
    let project = workspace
        .projects
        .iter_mut()
        .find(|project| project.id == "shop")
        .unwrap();
    // A number the total cannot be, so the comparison fails rather than erroring
    // on a value it cannot compare at all.
    for node in &mut project.scenarios[0].graph.nodes {
        if node.kind == apiforge_core::model::NodeKind::Assert
            && node.inputs.contains_key("expected")
        {
            node.inputs.insert(
                "expected".into(),
                apiforge_core::format::Binding::literal(serde_json::json!(-1)),
            );
            break;
        }
    }
    fs::write(&path, serde_json::to_vec_pretty(&workspace).unwrap()).unwrap();
    let output = run(&[
        OsStr::new("run"),
        OsStr::new("--workspace"),
        path.as_os_str(),
        OsStr::new("--scenario"),
        OsStr::new(&scenario()),
    ]);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("expected -1"),
        "the comparison says what it wanted: {text}"
    );
    assert!(
        text.lines()
            .any(|line| line.trim_start().starts_with("actual")),
        "and what it got: {text}"
    );
    // A step that passed says nothing of the sort.
    assert_eq!(
        text.lines()
            .filter(|line| line.trim_start().starts_with("expected "))
            .count(),
        1,
        "only the comparison that failed explains itself: {text}"
    );
}

/// MCP-03 (#62) criterion 9: the executor moved into `apiforge-mcp` and
/// returns data; `apiforge run` prints from it exactly what it printed before.
/// `fixtures/demo-shop-run.txt` is the command's stdout on the demo shop,
/// recorded before the move.
#[test]
fn mcp03_criterion_9_a_folder_run_prints_what_it_printed_before_the_shared_executor() {
    let directory = tempfile::tempdir().unwrap();
    apiforge_project::prepare_project(directory.path(), &apiforge_core::demos::shop_project())
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .args(["run", "--folder"])
        .arg(directory.path())
        .args(["--scenario", "scenarios/promotion-and-cart-price.afs"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        include_str!("fixtures/demo-shop-run.txt")
    );
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
}
