use apiforge_project::{AnalysisState, DocumentKind, Origin, ProjectSession, prepare_project};
use std::{
    ffi::OsStr,
    fs,
    process::{Command, Output},
};

fn run(root: &std::path::Path, command: &str, rest: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .arg(command)
        .arg("--folder")
        .arg(root)
        .args(rest)
        .output()
        .unwrap()
}

#[test]
fn cli_and_gui_share_saved_semantics_identities_and_explicit_revisions() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    for document in session.documents() {
        let path = document.entry().path.as_str();
        let output = run(
            directory.path(),
            "print",
            &[OsStr::new("--file"), OsStr::new(path)],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let expected =
            apiforge_script::print_folder_document(session.context(), document.model().unwrap())
                .unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
    assert!(run(directory.path(), "check", &[]).status.success());
    let scenario = session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Scenario)
        .unwrap();
    let id = scenario.id().to_owned();
    let path = scenario.entry().path.as_str().to_owned();
    let original = scenario.model().unwrap().clone();
    let input = directory.path().join("incoming.txt");
    let edited = scenario
        .text()
        .replacen("scenario \"", "scenario \"Changed ", 1);
    fs::write(&input, &edited).unwrap();
    assert!(
        !run(
            directory.path(),
            "apply",
            &[OsStr::new("--file"), OsStr::new(&path), input.as_os_str()]
        )
        .status
        .success(),
        "a GUI writer holds the project lock"
    );
    drop(session);
    assert!(
        run(
            directory.path(),
            "apply",
            &[OsStr::new("--file"), OsStr::new(&path), input.as_os_str()]
        )
        .status
        .success()
    );
    let session = ProjectSession::open(directory.path()).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), edited);
    assert_eq!(document.analysis(), AnalysisState::Valid);
    let apiforge_project::Document::Scenario(before) = original else {
        panic!()
    };
    let Some(apiforge_project::Document::Scenario(after)) = document.model() else {
        panic!()
    };
    assert_eq!(before.id, after.id);
    assert_eq!(before.graph, after.graph);
    assert_eq!(before.cleanup, after.cleanup);
}

#[test]
fn folder_cli_ignores_dirty_recovery_and_cache_and_never_executes_invalid_saved_source() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Saved").unwrap();
    let source = "scenario \"Saved\" {\n  answer = 1\n}\n";
    let id = session
        .create_document(DocumentKind::Scenario, "saved.afs", source)
        .unwrap();
    session
        .edit_source(&id, &source.replace('1', "2"), Origin::SESSION)
        .unwrap();
    session.settle(&id).unwrap();
    session.save_last_valid().unwrap();
    session.save_recovery().unwrap();
    let output = run(
        directory.path(),
        "print",
        &[OsStr::new("--file"), OsStr::new("saved.afs")],
    );
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("answer = 1")
    );
    drop(session);
    fs::write(
        directory.path().join("saved.afs"),
        "scenario \"Saved\" { answer = }",
    )
    .unwrap();
    assert!(!run(directory.path(), "check", &[]).status.success());
    assert!(
        !run(
            directory.path(),
            "print",
            &[OsStr::new("--file"), OsStr::new("saved.afs")]
        )
        .status
        .success()
    );
    assert!(
        !run(
            directory.path(),
            "run",
            &[OsStr::new("--scenario"), OsStr::new("saved.afs")]
        )
        .status
        .success()
    );
}

#[test]
fn invalid_apply_and_unqualified_calls_preserve_the_saved_project() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    let document = session
        .documents()
        .iter()
        .find(|d| d.entry().kind == DocumentKind::Scenario)
        .unwrap();
    let path = document.entry().path.as_str().to_owned();
    let original = document.text().to_owned();
    let input = directory.path().join("incoming.txt");
    let unpinned = original.replace("@1(", "(");
    assert_ne!(original, unpinned);
    drop(session);
    for text in ["scenario \"Broken\" { value = }", &unpinned] {
        fs::write(&input, text).unwrap();
        let output = run(
            directory.path(),
            "apply",
            &[OsStr::new("--file"), OsStr::new(&path), input.as_os_str()],
        );
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            fs::read_to_string(directory.path().join(&path)).unwrap(),
            original
        );
    }
}

#[test]
fn saved_folder_runs_match_gui_snapshots_and_ignore_an_unrelated_invalid_scenario() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Run").unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            "good.afs",
            "scenario \"Good\" {\n  answer = 7\n  equal = assert answer == 7\n}\n",
        )
        .unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            "bad.afs",
            "scenario \"Bad\" { value = }",
        )
        .unwrap();
    drop(session);
    let output = run(
        directory.path(),
        "run",
        &[
            OsStr::new("--scenario"),
            OsStr::new("good.afs"),
            OsStr::new("--json"),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: apiforge_core::run::RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report.status, apiforge_core::run::RunStatus::Passed);
    assert_eq!(report.steps.len(), 2);
}

#[test]
fn folder_arguments_reject_ambiguous_modes_and_unsafe_or_unregistered_paths() {
    let directory = tempfile::tempdir().unwrap();
    let session = ProjectSession::create(directory.path(), "Arguments").unwrap();
    drop(session);
    for (command, args) in [
        ("print", vec![]),
        ("apply", vec!["--file", "x.afs"]),
        ("check", vec!["--folder", "duplicate"]),
        ("check", vec!["--workspace", "legacy.json"]),
    ] {
        let rest: Vec<_> = args.iter().map(OsStr::new).collect();
        assert_eq!(run(directory.path(), command, &rest).status.code(), Some(2));
    }
    for path in [
        "../outside.afs",
        ".apiforge/revisions/id/1.afs",
        "unregistered.afs",
    ] {
        assert_eq!(
            run(
                directory.path(),
                "print",
                &[OsStr::new("--file"), OsStr::new(path)]
            )
            .status
            .code(),
            Some(1)
        );
    }
}

#[test]
fn read_only_folder_commands_leave_an_untouched_checkout_without_local_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Checkout").unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            "saved.afs",
            "scenario \"Saved\" {\n  value = 1\n}\n",
        )
        .unwrap();
    drop(session);
    fs::remove_dir_all(directory.path().join(".apiforge/local")).unwrap();
    assert!(run(directory.path(), "check", &[]).status.success());
    assert!(
        run(
            directory.path(),
            "print",
            &[OsStr::new("--file"), OsStr::new("saved.afs")]
        )
        .status
        .success()
    );
    assert!(
        run(
            directory.path(),
            "run",
            &[OsStr::new("--scenario"), OsStr::new("saved.afs")]
        )
        .status
        .success()
    );
    assert!(!directory.path().join(".apiforge/local").exists());
}

#[test]
fn a_live_folder_run_requires_its_project_lock_and_a_writable_journal() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Live").unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            "saved.afs",
            "scenario \"Saved\" {\n  value = 1\n}\n",
        )
        .unwrap();
    let arguments = [
        OsStr::new("--scenario"),
        OsStr::new("saved.afs"),
        OsStr::new("--live"),
    ];
    assert!(!run(directory.path(), "run", &arguments).status.success());
    drop(session);
    let journal = directory.path().join(".apiforge/local/journal.json");
    fs::create_dir(&journal).unwrap();
    assert!(!run(directory.path(), "run", &arguments).status.success());
    fs::remove_dir(&journal).unwrap();
    assert!(run(directory.path(), "run", &arguments).status.success());
    assert!(!journal.exists(), "a finished run removes its own record");
}
