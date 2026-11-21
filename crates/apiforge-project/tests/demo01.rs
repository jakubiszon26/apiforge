//! DEMO-01 (#523): the first-start demo is written from its files, and is a
//! project a session without a window opens with every document valid.
use apiforge_core::demos::FIRST_START;
use apiforge_project::{AnalysisState, Error, ProjectSession, prepare_files};

#[test]
fn the_demo_is_written_from_its_files_and_opens_valid() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let manifest = prepare_files(root, FIRST_START).unwrap();
    assert_eq!(manifest.name, "Demo shop");
    for (path, text) in FIRST_START {
        assert_eq!(std::fs::read_to_string(root.join(path)).unwrap(), *text);
    }
    for rules in [".gitignore", ".gitattributes"] {
        assert!(root.join(rules).is_file(), "{rules}");
    }
    let session = ProjectSession::open(root).unwrap();
    assert_eq!(session.documents().len(), 3);
    for document in session.documents() {
        assert_eq!(
            document.analysis(),
            AnalysisState::Valid,
            "{}",
            document.entry().path.as_str()
        );
    }
    assert_eq!(session.context().components.len(), 1, "product@1");
}

#[test]
fn the_demo_is_never_written_over_a_project() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    prepare_files(root, FIRST_START).unwrap();
    let manifest = std::fs::read(root.join("apiforge.project.json")).unwrap();
    assert!(matches!(
        prepare_files(root, FIRST_START),
        Err(Error::AlreadyAProject)
    ));
    assert_eq!(
        std::fs::read(root.join("apiforge.project.json")).unwrap(),
        manifest
    );
}
