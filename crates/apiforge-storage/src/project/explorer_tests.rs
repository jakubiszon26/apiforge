use super::*;
use crate::project::tests::initialise;
use std::fs;

fn setup() -> (tempfile::TempDir, ProjectStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    (directory, store)
}

fn paths(scan: &TreeScan) -> Vec<&str> {
    scan.entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect()
}

#[test]
fn scan_lists_the_real_tree_with_directories_first_and_hides_control_data() {
    let (directory, store) = setup();
    let root = directory.path();
    fs::create_dir_all(root.join(".git/objects")).unwrap();
    fs::create_dir_all(root.join("b/nested")).unwrap();
    fs::create_dir(root.join("A")).unwrap();
    fs::write(root.join("README.md"), "notes").unwrap();
    fs::write(root.join("b/nested/z.afs"), "scenario \"Z\" {}\n").unwrap();
    fs::write(root.join("b/a.txt"), "x").unwrap();
    let scan = store.scan_tree().unwrap();
    assert_eq!(
        paths(&scan),
        [
            "A",
            "b",
            "b/nested",
            "b/nested/z.afs",
            "b/a.txt",
            "scenarios",
            "scenarios/main.afs",
            "apiforge.project.json",
            "README.md",
        ]
    );
    assert!(!scan.truncated);
    assert_eq!(scan.generation, 1);
    let source = scan.entry("scenarios/main.afs").unwrap();
    assert_eq!(
        source.state(),
        Some(FileState::of(b"scenario \"Main\" {}\n"))
    );
    assert!(scan.entry(MANIFEST_PATH).unwrap().state().is_some());
    // Other file types are listed with a size but never hashed.
    assert_eq!(
        scan.entry("README.md").unwrap().kind,
        TreeEntryKind::File {
            bytes: 5,
            sha256: None
        }
    );
}

#[cfg(unix)]
#[test]
fn scan_shows_links_without_following_them_and_marks_non_portable_names() {
    let (directory, store) = setup();
    let root = directory.path();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.afs"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("linked")).unwrap();
    fs::write(root.join("what?.afs"), "").unwrap();
    let scan = store.scan_tree().unwrap();
    assert_eq!(scan.entry("linked").unwrap().kind, TreeEntryKind::Link);
    assert!(scan.entry("linked/secret.afs").is_none());
    assert!(!scan.entry("what?.afs").unwrap().portable);
    assert!(scan.entry("scenarios").unwrap().portable);
}

#[test]
fn scan_is_bounded_in_depth() {
    let (directory, store) = setup();
    let mut deep = directory.path().to_owned();
    for level in 0..MAX_TREE_DEPTH + 2 {
        deep.push(format!("d{level}"));
    }
    fs::create_dir_all(&deep).unwrap();
    let scan = store.scan_tree().unwrap();
    assert!(scan.truncated);
    let deepest = scan
        .entries
        .iter()
        .map(|entry| entry.path.matches('/').count() + 1)
        .max()
        .unwrap();
    assert_eq!(deepest, MAX_TREE_DEPTH);
}

#[test]
fn scan_refuses_a_folder_with_an_unfinished_transaction() {
    let (directory, store) = setup();
    fs::write(
        directory.path().join(".apiforge/local/head.json"),
        br#"{"formatVersion":1,"generation":1,"activeTransactionId":"0b8a3e4e-21a4-4bd5-9a59-9d7a5b0d0a11"}"#,
    )
    .unwrap();
    assert!(matches!(
        store.scan_tree(),
        Err(ProjectError::RecoveryRequired)
    ));
}

#[test]
fn listing_covers_everything_below_a_directory_and_refuses_links() {
    let (directory, store) = setup();
    let root = directory.path();
    fs::create_dir_all(root.join("docs/inner")).unwrap();
    fs::write(root.join("docs/inner/a.txt"), "a").unwrap();
    fs::write(root.join("docs/b.afs"), "b").unwrap();
    let (files, directories) = store
        .list_directory(&ProjectPath::new("docs").unwrap())
        .unwrap();
    assert_eq!(
        files.iter().map(ProjectPath::as_str).collect::<Vec<_>>(),
        ["docs/b.afs", "docs/inner/a.txt"]
    );
    assert_eq!(
        directories
            .iter()
            .map(ProjectPath::as_str)
            .collect::<Vec<_>>(),
        ["docs/inner"]
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("scenarios"), root.join("docs/link")).unwrap();
        assert!(matches!(
            store.list_directory(&ProjectPath::new("docs").unwrap()),
            Err(ProjectError::UnsafePath { .. })
        ));
    }
}
