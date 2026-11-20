//! LANG-15 (#412): a project names its language version, and the window
//! says when this build cannot read it, through the error paths it has: the
//! error shown when a project cannot be opened, and *Cannot run this
//! scenario: …*. The build reads `LANGUAGE_VERSION`; LANG-20 (#481) raises
//! it to 2.
use super::*;
use crate::testing::{
    MINIMUM,
    language::{language_of, write_language},
    shop_project,
};
use apiforge_script::LANGUAGE_VERSION;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Every file and directory under `root`, with the bytes of each file.
fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, at: &Path, files: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if path.is_dir() {
                files.insert(relative, None);
                walk(root, &path, files);
            } else {
                files.insert(relative, Some(fs::read(&path).unwrap()));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

/// Criterion 3: the window does not open a project written in a newer
/// language; its open error is the sentence that names both versions, and
/// no file of the project changes.
#[test]
fn lang15_criterion_3_the_window_refuses_a_newer_project() {
    let project = shop_project();
    let root = project.path();
    fs::remove_dir_all(root.join(".apiforge/local")).unwrap();
    let newer = LANGUAGE_VERSION + 1;
    write_language(root, newer);
    let before = tree(root);
    let sentence = format!(
        "This project is written in language {newer}; this build reads language {LANGUAGE_VERSION}. Update API Forge."
    );

    let harness = folder_harness(root, MINIMUM);
    let app = folder(&harness);
    assert!(app.session.is_none(), "the project is not opened");
    assert_eq!(app.open_error.as_deref(), Some(sentence.as_str()));
    assert!(
        harness.query_by_label(&sentence).is_some(),
        "the window shows the sentence"
    );
    drop(harness);
    assert_eq!(tree(root), before, "no file of the project changed");
}

/// Criterion 4: the window opens an older project, saves an edit, and
/// refuses a run with the sentence that names both versions and the fix;
/// nothing runs.
#[test]
#[ignore = "LANG-20: the build reads language 2 from that ticket on"]
fn lang15_criterion_4_the_window_opens_an_older_project_and_does_not_run_it() {
    let project = shop_project();
    let root = project.path();
    let older = LANGUAGE_VERSION - 1;
    write_language(root, older);
    let mut harness = folder_harness(root, MINIMUM);
    assert!(folder(&harness).open_error.is_none());
    let id = only(&harness, DocumentKind::Scenario);
    assert!(!folder(&harness).session.as_ref().unwrap().is_read_only());

    choose(&mut harness, "Run simulation");
    let deadline = Instant::now() + Duration::from_secs(5);
    while folder(&harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "no run starts");
        harness.step();
    }
    settle(&mut harness);
    assert!(folder(&harness).run_result(&id).is_none(), "nothing ran");
    let notice = folder(&harness)
        .notice()
        .map(|notice| notice.text.clone())
        .unwrap_or_default();
    assert!(
        notice.starts_with(&format!(
            "Cannot run this scenario: This project is written in language {older}; this build reads language {LANGUAGE_VERSION}."
        )) && notice.contains("apiforge fix --folder"),
        "{notice}"
    );

    let session = app(&mut harness).session.as_mut().unwrap();
    let edited = format!("{}\n", session.document(&id).unwrap().text());
    session
        .edit_source(&id, &edited, apiforge_project::Origin::SESSION)
        .unwrap();
    session.settle(&id).unwrap();
    session.save(&id).unwrap();
    assert_eq!(
        language_of(root),
        older,
        "a save does not raise the language"
    );
}
