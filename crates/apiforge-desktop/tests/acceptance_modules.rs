//! KTLO-44 (#325): a ticket's window tests are a file of
//! `src/folder/acceptance_tests/`, found there by the build script; adding one
//! edits no shared file.

use std::fs;
use std::path::Path;

/// The file stems of the directory, sorted.
fn ticket_files() -> Vec<String> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/folder/acceptance_tests");
    let mut stems: Vec<String> = fs::read_dir(&directory)
        .expect("the acceptance tests directory exists")
        .map(|entry| entry.expect("a readable entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    stems.sort();
    stems
}

#[test]
fn every_file_of_the_directory_is_a_module_and_none_is_declared_by_hand() {
    let generated = fs::read_to_string(Path::new(env!("OUT_DIR")).join("acceptance_modules.rs"))
        .expect("the build script writes the module declarations");
    let declared: Vec<String> = generated
        .lines()
        .filter_map(|line| line.strip_prefix("mod "))
        .map(|rest| rest.trim_end_matches(';').to_owned())
        .collect();
    assert_eq!(declared, ticket_files());

    let shared = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/folder/acceptance_tests.rs"),
    )
    .unwrap();
    assert!(
        !shared.contains("#[path = \"acceptance_tests/"),
        "acceptance_tests.rs declares no ticket module by hand"
    );
    assert!(shared.contains("acceptance_modules.rs"));
}
