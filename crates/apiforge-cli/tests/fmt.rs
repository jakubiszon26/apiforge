use std::{
    ffi::OsStr,
    fs,
    process::{Command, Output},
};

fn run(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn check_is_read_only_and_formatting_is_atomic_and_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("file with spaces.afs");
    let original = "scenario \"Example\"{begin=start value=1+2*3}";
    fs::write(&path, original).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        run(&[OsStr::new("fmt"), path.as_os_str(), OsStr::new("--check")])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert!(run(&[OsStr::new("fmt"), path.as_os_str()]).status.success());
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "scenario \"Example\" {\n  begin = start\n  value = 1 + 2 * 3\n}\n"
    );
    assert!(
        run(&[OsStr::new("fmt"), OsStr::new("--check"), path.as_os_str()])
            .status
            .success()
    );
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert!(run(&[OsStr::new("fmt"), path.as_os_str()]).status.success());
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
}

#[test]
fn invalid_source_is_preserved_and_diagnostics_do_not_echo_literals() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.afs");
    let original = "scenario \"Private\" { value = \"canary-secret-value\\q\" }";
    fs::write(&path, original).unwrap();
    let output = run(&[OsStr::new("fmt"), path.as_os_str()]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("canary-secret-value"));
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn unattached_comments_warn_before_formatting_removes_them() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.afs");
    fs::write(&path, "// floating\nscenario \"Notes\" { value = 1 }").unwrap();
    let output = run(&[OsStr::new("fmt"), path.as_os_str()]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Warning"));
    assert!(!fs::read_to_string(&path).unwrap().contains("floating"));
}

#[test]
fn invalid_arguments_and_io_errors_have_distinct_exit_codes() {
    for args in [
        vec![],
        vec!["fmt"],
        vec!["fmt", "--check"],
        vec!["fmt", "--check", "--check"],
        vec!["fmt", "file.afs", "--unknown"],
        vec!["run", "file.afs"],
    ] {
        let args: Vec<_> = args.iter().map(OsStr::new).collect();
        assert_eq!(run(&args).status.code(), Some(2));
    }
    assert!(run(&[OsStr::new("--help")]).status.success());
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(
        run(&[
            OsStr::new("fmt"),
            directory.path().join("absent.afs").as_os_str()
        ])
        .status
        .code(),
        Some(1)
    );
}

#[test]
fn oversized_and_non_utf8_files_are_rejected_without_writing() {
    let directory = tempfile::tempdir().unwrap();
    for (name, bytes, message) in [
        (
            "large.afs",
            vec![b' '; apiforge_script::MAX_SOURCE_BYTES + 1],
            "1 MiB",
        ),
        ("invalid.afs", vec![0xff, 0xfe], "UTF-8"),
    ] {
        let path = directory.path().join(name);
        fs::write(&path, &bytes).unwrap();
        let output = run(&[OsStr::new("fmt"), path.as_os_str()]);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn format_expansion_cannot_produce_an_unreadable_oversized_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("expansion.afs");
    let original = format!(
        "scenario \"Expansion\" {{ value = [{}] }}",
        "0,".repeat(apiforge_script::MAX_SOURCE_BYTES / 3 + 100)
    );
    assert!(original.len() < apiforge_script::MAX_SOURCE_BYTES);
    fs::write(&path, &original).unwrap();
    let output = run(&[OsStr::new("fmt"), path.as_os_str()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Formatted source would exceed"));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}
