//! LANG-20 (#481): `apiforge fix --folder` converts a project written for
//! 0.1.0 whole, in one commit of the project store, or writes nothing. The
//! command's own tests are in `apiforge-cli/tests/fix.rs`; these cover what
//! the command cannot be stopped in the middle of: a file changed between
//! its reading and its commit (5), an interrupted commit (6), the store's
//! refusal for its limits (7), and the sentence on an `@after` line of a
//! language-1 project (10).
use apiforge_project::{
    DocumentKind, Error, ProjectError, ProjectSession,
    language::{self, FixPlan, Planned, SourceOutcome},
};
use apiforge_storage::project::{
    MANIFEST_PATH, ProjectManifest, ProjectStore,
    faults::{self, Boundary, Phase},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[path = "support/language.rs"]
mod support;

const BOOKING_DAY: &str = "scenarios/booking-day.afs";
const SIGN_IN: &str = "components/sign-in.afs";

/// The room booking as 0.1.0 wrote it (the fixture of the command's tests).
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../apiforge-cli/tests/fixtures/rooms-language-1")
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A copy of the language-1 project, in a folder whose name has a space.
fn project() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("My rooms");
    copy(&fixture(), &root);
    (directory, root)
}

/// Every file of the project but its local data, which a writer's lock
/// creates: what a person sees changed.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if relative.starts_with(".apiforge/local") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

fn plan(root: &Path) -> FixPlan {
    match language::plan_fix(root, false).unwrap() {
        Planned::Plan(plan) => plan,
        Planned::Current => panic!("a language-1 project"),
    }
}

/// The project converted by the command, from a fresh copy: what each file
/// is after a conversion that was not interrupted.
fn converted() -> BTreeMap<PathBuf, Vec<u8>> {
    let (_directory, root) = project();
    let plan = plan(&root);
    assert!(plan.report().convertible(), "{:#?}", plan.report());
    plan.commit().unwrap();
    files(&root)
}

fn append(root: &Path, path: &str, text: &str) {
    let mut bytes = fs::read(root.join(path)).unwrap();
    bytes.extend_from_slice(text.as_bytes());
    fs::write(root.join(path), bytes).unwrap();
}

/// The plan writes `path` (or not) as the test says it does.
fn outcome<'a>(plan: &'a FixPlan, path: &str) -> &'a SourceOutcome {
    &plan
        .report()
        .sources
        .iter()
        .find(|source| source.path == path)
        .unwrap()
        .outcome
}

fn refused_naming(plan: FixPlan, path: &str) {
    let error = plan.commit().unwrap_err();
    assert!(
        matches!(&error, Error::ChangedWhileConverting { path: named } if named == path),
        "{error:?}"
    );
    assert!(error.to_string().starts_with(path), "{error}");
    assert!(error.to_string().contains("nothing was written"), "{error}");
}

// Criterion 5 ----------------------------------------------------------------

/// A source the command would write, changed between its reading and the
/// commit: nothing is written and the file is named.
#[test]
fn lang20_criterion_5_a_source_it_would_write_changed() {
    let (_directory, root) = project();
    let plan = plan(&root);
    assert!(matches!(
        outcome(&plan, BOOKING_DAY),
        SourceOutcome::Converted(_)
    ));
    append(&root, BOOKING_DAY, "// changed meanwhile\n");
    let before = files(&root);
    refused_naming(plan, BOOKING_DAY);
    assert_eq!(files(&root), before, "nothing written");
    assert_eq!(support::language_of(&root), 1);
}

/// A source the command would not write, changed meanwhile: the read set
/// refuses the commit all the same.
#[test]
fn lang20_criterion_5_a_source_it_would_not_write_changed() {
    let (_directory, root) = project();
    let plan = plan(&root);
    assert_eq!(outcome(&plan, SIGN_IN), &SourceOutcome::Unchanged);
    append(&root, SIGN_IN, "// changed meanwhile\n");
    let before = files(&root);
    refused_naming(plan, SIGN_IN);
    assert_eq!(files(&root), before, "nothing written");
}

/// The manifest, changed meanwhile.
#[test]
fn lang20_criterion_5_the_manifest_changed() {
    let (_directory, root) = project();
    let plan = plan(&root);
    let path = root.join(MANIFEST_PATH);
    let mut manifest = ProjectManifest::from_bytes(&fs::read(&path).unwrap()).unwrap();
    manifest.name = "Room booking, renamed".into();
    fs::write(&path, manifest.to_public_bytes().unwrap()).unwrap();
    let before = files(&root);
    refused_naming(plan, MANIFEST_PATH);
    assert_eq!(files(&root), before, "nothing written");
}

/// `formatVersion: 2, languageVersion: 1`, which the schema allows, is
/// raised like `formatVersion: 1`.
#[test]
fn lang20_criterion_5_format_2_language_1_is_raised() {
    let (_directory, root) = project();
    let path = root.join(MANIFEST_PATH);
    let mut manifest = ProjectManifest::from_bytes(&fs::read(&path).unwrap()).unwrap();
    (manifest.format_version, manifest.language_version) = (2, Some(1));
    fs::write(&path, manifest.to_public_bytes().unwrap()).unwrap();
    plan(&root).commit().unwrap();
    let manifest = ProjectManifest::from_bytes(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        (manifest.format_version, manifest.language_version),
        (2, Some(2))
    );
    assert_eq!(files(&root), converted(), "converted as from format 1");
}

// Criterion 6 ----------------------------------------------------------------

fn injected(path: &Path) -> ProjectError {
    ProjectError::Io {
        path: path.to_owned(),
        source: std::io::Error::other("interrupted after activation"),
    }
}

/// The commit interrupted after its activation, while it replaces the
/// manifest: the store has an unfinished commit.
fn interrupted(root: &Path) {
    let plan = plan(root);
    let error = faults::with_hook(
        |event| {
            if event.boundary == Boundary::Replace
                && event.phase == Phase::Before
                && event.path.ends_with(MANIFEST_PATH)
            {
                return Err(injected(&event.path));
            }
            Ok(())
        },
        || plan.commit(),
    )
    .unwrap_err();
    assert!(!error.to_string().is_empty());
}

/// An interrupted commit: before recovery the project is not treated as
/// language 2 unless its manifest says so; after the next writable open's
/// recovery every source and the manifest are the converted ones.
#[test]
fn lang20_criterion_6_an_interrupted_commit_is_finished_by_recovery() {
    let (_directory, root) = project();
    interrupted(&root);
    match ProjectSession::open_saved_read_only(&root) {
        Err(error) => assert!(
            matches!(error, Error::Storage(ProjectError::RecoveryRequired)),
            "{error:?}"
        ),
        Ok(session) => assert!(
            support::language_of(&root) == 2 || session.language_problem().is_some(),
            "treated as language 2 by a manifest of language 1"
        ),
    }
    let store = ProjectStore::open_exclusive(&root).unwrap();
    store.recover().unwrap();
    drop(store);
    assert_eq!(files(&root), converted());
    assert_eq!(support::language_of(&root), 2);
}

/// A project with an unfinished commit when the command starts is refused,
/// with the store's sentence, and unchanged.
#[test]
fn lang20_criterion_6_an_unfinished_commit_refuses_the_command() {
    let (_directory, root) = project();
    interrupted(&root);
    let before = files(&root);
    for check in [true, false] {
        let error = language::plan_fix(&root, check).err().expect("refused");
        assert!(
            matches!(error, Error::Storage(ProjectError::RecoveryRequired)),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            ProjectError::RecoveryRequired.to_string()
        );
    }
    assert_eq!(files(&root), before);
}

// Criterion 7 ----------------------------------------------------------------

/// The store's refusal of the commit for its limits becomes the command's
/// sentence; a changed file is named by its registered path; anything else
/// is the store's own sentence.
#[test]
fn lang20_criterion_7() {
    let root = Path::new("/work/rooms");
    let error = language::commit_refusal(ProjectError::Limit, root);
    assert!(matches!(error, Error::TooLargeToConvert), "{error:?}");
    assert_eq!(
        error.to_string(),
        "This project is too large to convert in one step."
    );
    let error = language::commit_refusal(
        ProjectError::Conflict {
            path: root.join("scenarios").join("booking-day.afs"),
        },
        root,
    );
    assert!(
        matches!(&error, Error::ChangedWhileConverting { path } if path == BOOKING_DAY),
        "{error:?}"
    );
    let error = language::commit_refusal(ProjectError::Busy, root);
    assert_eq!(error.to_string(), ProjectError::Busy.to_string());
}

// Criterion 10 ---------------------------------------------------------------

fn document_id(session: &ProjectSession, path: &str) -> String {
    session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap()
        .id()
        .to_owned()
}

/// The diagnostics of `path` as `check`, `check` for an agent and the
/// window's Problems show them.
fn diagnostics(root: &Path, path: &str) -> Vec<apiforge_script::Diagnostic> {
    let mut session = ProjectSession::open(root).unwrap();
    let id = document_id(&session, path);
    session.settle(&id).unwrap();
    session.document(&id).unwrap().diagnostics().to_vec()
}

/// The folder argument as the run refusal gives it.
fn refusal_folder(root: &Path) -> String {
    let session = ProjectSession::open_saved_read_only(root).unwrap();
    let refusal = session.language_problem().unwrap().to_string();
    let (_, rest) = refusal.split_once("apiforge fix --folder ").unwrap();
    rest.strip_suffix(" before it runs.").unwrap().to_owned()
}

/// In a language-1 project the error on an `@after` line names the command
/// with the project's folder, as the run refusal gives it, and carries no
/// fix: for a saved source and for a text an agent checks.
#[test]
fn lang20_criterion_10_an_after_line_names_the_command_in_language_1() {
    let (_directory, root) = project();
    let sentence = language::after_in_older(&root);
    assert_eq!(
        sentence,
        format!(
            "`@after` is not part of language 2. Update the project with apiforge fix --folder {}; do not delete the line by hand.",
            refusal_folder(&root)
        )
    );
    assert!(
        sentence.contains("\""),
        "the folder has a space: {sentence}"
    );
    let text = fs::read_to_string(root.join(BOOKING_DAY)).unwrap();
    let afters = text.matches("@after(").count();
    let found = diagnostics(&root, BOOKING_DAY);
    let ours: Vec<_> = found.iter().filter(|d| d.message == sentence).collect();
    assert_eq!(ours.len(), afters, "{found:#?}");
    assert!(ours.iter().all(|d| d.fix.is_none()), "{ours:#?}");
    assert!(
        found
            .iter()
            .all(|d| d.message != apiforge_script::AFTER_REMOVED),
        "{found:#?}"
    );
    for diagnostic in &ours {
        assert!(text[diagnostic.range.clone()].starts_with("@after("));
    }

    // `check_text` for an agent: a new text checked in the same project.
    let session = ProjectSession::open(&root).unwrap();
    let check = session
        .check_new_document(Some(DocumentKind::Scenario), "scenarios/new.afs", &text)
        .unwrap();
    let ours: Vec<_> = check
        .diagnostics
        .iter()
        .filter(|d| d.message == sentence)
        .collect();
    assert_eq!(ours.len(), afters, "{:#?}", check.diagnostics);
    assert!(ours.iter().all(|d| d.fix.is_none()));
}

/// In a language-2 project the same line has LANG-13's text and its fix.
#[test]
fn lang20_criterion_10_an_after_line_keeps_lang13_in_language_2() {
    let (_directory, root) = project();
    support::write_language(&root, 2);
    let text = fs::read_to_string(root.join(BOOKING_DAY)).unwrap();
    let afters = text.matches("@after(").count();
    let found = diagnostics(&root, BOOKING_DAY);
    let ours: Vec<_> = found
        .iter()
        .filter(|d| d.message == apiforge_script::AFTER_REMOVED)
        .collect();
    assert_eq!(ours.len(), afters, "{found:#?}");
    assert!(ours.iter().all(|d| d.fix.is_some()), "{ours:#?}");
    assert!(
        found
            .iter()
            .all(|d| !d.message.contains("apiforge fix --folder")),
        "{found:#?}"
    );
}
