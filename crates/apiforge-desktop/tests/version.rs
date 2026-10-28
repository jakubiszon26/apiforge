//! REL-01 (#115), criterion 5: `apiforge-desktop --version` prints the binary's
//! name and the workspace version without opening a window, which is what the
//! release workflow and a person checking a download rely on.
use std::process::Command;

#[test]
fn version_prints_the_name_and_the_workspace_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_apiforge-desktop"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("apiforge-desktop {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn version_takes_no_further_argument() {
    let output = Command::new(env!("CARGO_BIN_EXE_apiforge-desktop"))
        .args(["--version", "fixtures/workspace.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
}
