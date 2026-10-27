//! Project navigation, settings, recovery decisions and asynchronous storage
//! through named widgets (Stage II P7a–P7e, ADR 0013).
use super::recovery_tests::{FOREIGN, INTENDED, LOCAL, assert_active, interrupt, source_project};
use super::*;
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_disjoint, assert_within_window, folder, folder_harness,
    settle,
};
use egui_kittest::kittest::{NodeT, Queryable};
use std::{fs, path::Path, sync::mpsc};

const EDITOR: &str = "Contents of main.afs";

pub(super) fn app(harness: &mut FolderHarness) -> &mut FolderApp {
    harness.state_mut().as_mut().unwrap()
}

/// Lists `root` after the open project as the only recent projects, as an
/// earlier session would have. Recent projects are global metadata, so the
/// list other tests left in the shared data directory is replaced.
pub(super) fn remember(harness: &mut FolderHarness, root: &Path) {
    let bytes = fs::read(root.join("apiforge.project.json")).unwrap();
    let manifest = apiforge_project::ProjectManifest::from_bytes(&bytes).unwrap();
    let current = folder(harness).root.clone();
    let mut entries = folder(harness).navigation.recent.entries.clone();
    entries.retain(|entry| entry.path == current);
    entries.push(navigation::Recent {
        id: manifest.id,
        name: manifest.name,
        path: root.to_owned(),
    });
    recent(harness, entries);
}

fn recent(harness: &mut FolderHarness, entries: Vec<navigation::Recent>) {
    app(harness).navigation.recent.entries = entries;
    harness.step();
}

/// Scrolls a named widget into view, as a user would before using it. A click
/// goes to the centre of its rectangle, which may be clipped by a scroll area.
pub(super) fn reveal(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).scroll_to_me();
    settle(harness);
}

pub(super) fn type_into(harness: &mut FolderHarness, label: &str, text: &str) {
    reveal(harness, label);
    harness.get_by_label(label).focus();
    harness.step();
    harness.get_by_label(label).type_text(text);
    settle(harness);
}

/// Replaces the whole value of the focused-by-name field.
pub(super) fn replace_text(harness: &mut FolderHarness, label: &str, text: &str) {
    reveal(harness, label);
    harness.get_by_label(label).focus();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.key_press(Key::Backspace);
    if !text.is_empty() {
        harness.get_by_label(label).type_text(text);
    }
    settle(harness);
}

pub(super) fn choose(harness: &mut FolderHarness, label: &str) {
    reveal(harness, label);
    harness.get_by_label(label).click();
    settle(harness);
}

pub(super) fn open_project_menu(harness: &mut FolderHarness, item: &str) {
    choose(harness, "Project");
    choose(harness, item);
}

pub(super) fn has_label(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label(label).is_some()
}

fn editor_text(harness: &FolderHarness) -> String {
    harness.get_by_label(EDITOR).value().unwrap()
}

fn saved_name(root: &Path) -> String {
    let bytes = fs::read(root.join("apiforge.project.json")).unwrap();
    apiforge_project::ProjectManifest::from_bytes(&bytes)
        .unwrap()
        .name
}

/// Makes the watcher read the folder now instead of on its next tick.
pub(super) fn follow_disk(harness: &mut FolderHarness) {
    let app = app(harness);
    app.watcher.rescan();
    harness.step();
    let app = harness.state_mut().as_mut().unwrap();
    let session = app.session.as_mut().unwrap();
    if let Some(outcome) = app.watcher.finish(session) {
        app.follow_external(outcome);
    }
    settle(harness);
}

/// Holds the storage worker until the returned sender is used.
fn hold_storage(harness: &mut FolderHarness) -> mpsc::Sender<()> {
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel::<()>();
    app(harness).storage_io(
        "Reading project…",
        move || {
            started.send(()).unwrap();
            let _ = resume.recv();
        },
        |_, result| result.unwrap(),
    );
    harness.step();
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    release
}

// ----- P7a: Open Folder, recent projects and recovery before switching ------------

#[test]
fn the_project_menu_lists_recent_projects_inside_the_window() {
    let (alpha, _) = named_folder("Menu Alpha");
    let (beta, _) = named_folder("Menu Beta");
    for size in [MINIMUM, WIDE] {
        let mut harness = folder_harness(alpha.path(), size);
        remember(&mut harness, beta.path());
        choose(&mut harness, "Project");
        let items = [
            "Open Folder…",
            "Project settings…",
            "Test environment…",
            "Menu Beta",
        ];
        assert_within_window(&harness, &items);
        assert_disjoint(&harness, &items);
    }
}

#[test]
fn a_full_recent_list_keeps_the_menu_inside_the_window() {
    let (alpha, _) = named_folder("Full Alpha");
    let (last, _) = named_folder("Full Last");
    let mut entries: Vec<_> = (1..32)
        .map(|index| navigation::Recent {
            id: format!("recent-{index}"),
            name: format!("Recent project {index}"),
            path: alpha.path().join(format!("missing-{index}")),
        })
        .collect();
    let bytes = fs::read(last.path().join("apiforge.project.json")).unwrap();
    entries.push(navigation::Recent {
        id: apiforge_project::ProjectManifest::from_bytes(&bytes)
            .unwrap()
            .id,
        name: "Full Last".into(),
        path: last.path().to_owned(),
    });
    for size in [MINIMUM, WIDE] {
        let mut harness = folder_harness(alpha.path(), size);
        recent(&mut harness, entries.clone());
        choose(&mut harness, "Project");
        let items = ["Open Folder…", "Project settings…", "Test environment…"];
        assert_within_window(&harness, &items);
        reveal(&mut harness, "Full Last");
        assert_within_window(&harness, &["Full Last"]);
        assert_disjoint(&harness, &["Full Last", "Test environment…"]);
        choose(&mut harness, "Full Last");
        assert_eq!(folder(&harness).root, last.path());
    }
}

#[test]
fn switching_to_a_recent_project_protects_and_returns_unsaved_work() {
    let (alpha, _) = named_folder("Switch Alpha");
    let (beta, _) = named_folder("Switch Beta");
    let original = fs::read_to_string(alpha.path().join("main.afs")).unwrap();
    let mut harness = folder_harness(alpha.path(), MINIMUM);
    remember(&mut harness, beta.path());
    type_into(&mut harness, EDITOR, "// unfinished\n");
    let edited = editor_text(&harness);
    assert!(edited.starts_with("// unfinished\n"));

    open_project_menu(&mut harness, "Switch Beta");
    assert_eq!(folder(&harness).root, beta.path());
    assert_eq!(
        folder(&harness).session.as_ref().unwrap().manifest().name,
        "Switch Beta"
    );
    assert_eq!(
        fs::read_to_string(alpha.path().join("main.afs")).unwrap(),
        original
    );
    assert!(alpha.path().join(".apiforge/local/recovery.json").is_file());

    open_project_menu(&mut harness, "Switch Alpha");
    assert_eq!(folder(&harness).root, alpha.path());
    assert_eq!(editor_text(&harness), edited);
    assert_eq!(
        folder(&harness).navigation.recent.active.as_deref(),
        Some(alpha.path())
    );
}

#[test]
fn failed_recovery_keeps_the_current_project_until_the_user_decides() {
    let (alpha, _) = named_folder("Failure Alpha");
    let (beta, _) = named_folder("Failure Beta");
    let recovery = alpha.path().join(".apiforge/local/recovery.json");
    let actions = [
        "Keep editing",
        "Try again",
        "Export and open folder",
        "Discard unsaved work and open folder",
    ];
    for size in [MINIMUM, WIDE] {
        let mut harness = folder_harness(alpha.path(), size);
        remember(&mut harness, beta.path());
        type_into(&mut harness, EDITOR, "// unfinished\n");
        let edited = editor_text(&harness);
        fs::write(&recovery, b"foreign recovery").unwrap();

        open_project_menu(&mut harness, "Failure Beta");
        assert!(has_label(&harness, "Unsaved work is not protected"));
        assert_within_window(&harness, &actions);
        assert_disjoint(&harness, &actions);
        assert_eq!(folder(&harness).root, alpha.path());

        choose(&mut harness, "Keep editing");
        assert!(!folder(&harness).switching());
        assert_eq!(folder(&harness).root, alpha.path());
        assert_eq!(editor_text(&harness), edited);
        assert_eq!(fs::read(&recovery).unwrap(), b"foreign recovery");
    }
}

#[test]
fn discarding_after_failed_recovery_opens_the_folder_and_leaves_files_alone() {
    let (alpha, _) = named_folder("Discard Alpha");
    let (beta, _) = named_folder("Discard Beta");
    let original = fs::read_to_string(alpha.path().join("main.afs")).unwrap();
    let recovery = alpha.path().join(".apiforge/local/recovery.json");
    let mut harness = folder_harness(alpha.path(), MINIMUM);
    remember(&mut harness, beta.path());
    type_into(&mut harness, EDITOR, "// unfinished\n");
    fs::write(&recovery, b"foreign recovery").unwrap();
    open_project_menu(&mut harness, "Discard Beta");
    choose(&mut harness, "Discard unsaved work and open folder");
    assert_eq!(folder(&harness).root, beta.path());
    assert_eq!(
        fs::read_to_string(alpha.path().join("main.afs")).unwrap(),
        original
    );
    assert_eq!(fs::read(&recovery).unwrap(), b"foreign recovery");
}

// ----- P7a/P7e: a recent folder without a project, explicit initialisation --------

#[test]
fn a_recent_folder_without_a_project_is_written_only_by_initialise() {
    let (alpha, _) = named_folder("Start Alpha");
    let empty = tempfile::tempdir().unwrap();
    let mut harness = folder_harness(alpha.path(), MINIMUM);
    recent(
        &mut harness,
        vec![navigation::Recent {
            id: "empty-folder".into(),
            name: "Empty folder".into(),
            path: empty.path().to_owned(),
        }],
    );
    open_project_menu(&mut harness, "Empty folder");
    assert!(folder(&harness).uninitialised);
    assert!(fs::read_dir(empty.path()).unwrap().next().is_none());
    replace_text(&mut harness, "Project name", "");
    assert!(
        harness
            .get_by_label("Initialise project")
            .accesskit_node()
            .is_disabled()
    );
    assert_within_window(&harness, &["Project", "Project name", "Initialise project"]);

    replace_text(&mut harness, "Project name", "Started here");
    choose(&mut harness, "Initialise project");
    assert!(folder(&harness).session.is_some());
    assert_eq!(saved_name(empty.path()), "Started here");
    let attributes = fs::read_to_string(empty.path().join(".gitattributes")).unwrap();
    assert!(attributes.contains("*.afs"), "{attributes}");
    assert!(attributes.contains("apiforge.project.json"), "{attributes}");
}

#[test]
fn the_start_screen_name_field_has_the_height_of_the_other_form_fields() {
    let empty = tempfile::tempdir().unwrap();
    for size in [MINIMUM, WIDE] {
        let harness = folder_harness(empty.path(), size);
        let field = harness.get_by_label("Project name").rect();
        assert!(
            field.height() >= theme::CONTROL_HEIGHT,
            "the field is {} high",
            field.height()
        );
        assert_disjoint(&harness, &["Project name", "Initialise project"]);
    }
}

// ----- P7b: project settings and the test environment ------------------------------

#[test]
fn the_project_settings_dialog_fits_and_validates_the_name() {
    let (directory, _) = named_folder("Settings layout");
    let controls = ["Project name", "Data contracts", "Apply settings", "Cancel"];
    for size in [MINIMUM, WIDE] {
        let mut harness = folder_harness(directory.path(), size);
        open_project_menu(&mut harness, "Project settings…");
        assert_within_window(&harness, &controls);
        assert_disjoint(&harness, &controls);
        replace_text(&mut harness, "Project name", "");
        assert!(
            harness
                .get_by_label("Apply settings")
                .accesskit_node()
                .is_disabled()
        );
        choose(&mut harness, "Cancel");
        assert!(folder(&harness).prompt.is_none());
        assert_eq!(
            folder(&harness).session.as_ref().unwrap().manifest().name,
            "Settings layout"
        );
    }
}

#[test]
fn applied_settings_stay_unsaved_until_save_all_without_a_source_tab() {
    let (directory, _) = named_folder("Settings before");
    let source = fs::read(directory.path().join("main.afs")).unwrap();
    let mut harness = folder_harness(directory.path(), MINIMUM);
    choose(&mut harness, "Close main.afs");
    assert!(folder(&harness).workbench.active().is_none());

    open_project_menu(&mut harness, "Project settings…");
    replace_text(&mut harness, "Project name", "Settings after");
    choose(&mut harness, "Apply settings");
    let session = folder(&harness).session.as_ref().unwrap();
    assert_eq!(session.manifest().name, "Settings after");
    assert!(session.is_manifest_dirty());
    assert_eq!(saved_name(directory.path()), "Settings before");
    assert!(has_label(
        &harness,
        "Project settings have been applied. Use Save all to save them."
    ));
    assert!(
        !harness
            .get_by_label("Save all")
            .accesskit_node()
            .is_disabled()
    );

    choose(&mut harness, "Save all");
    assert_eq!(saved_name(directory.path()), "Settings after");
    assert!(
        !folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );
    assert!(
        harness
            .get_by_label("Save all")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(fs::read(directory.path().join("main.afs")).unwrap(), source);
}

#[test]
fn the_environment_dialog_fits_the_window() {
    let (directory, _) = named_folder("Environment layout");
    let footer = ["Apply environment", "Cancel"];
    for size in [MINIMUM, WIDE] {
        let mut harness = folder_harness(directory.path(), size);
        open_project_menu(&mut harness, "Test environment…");
        // The form scrolls above a fixed footer; each field is checked in view.
        for field in [
            "Base URL",
            "New variable name",
            "Add variable",
            "New secret name",
            "Add secret",
        ] {
            reveal(&mut harness, field);
            let labels = [field, footer[0], footer[1]];
            assert_within_window(&harness, &labels);
            assert_disjoint(&harness, &labels);
        }
    }
}

#[test]
fn secret_values_stay_in_the_session_and_only_names_are_saved() {
    let (directory, _) = named_folder("Secrets");
    let mut harness = folder_harness(directory.path(), MINIMUM);
    open_project_menu(&mut harness, "Test environment…");
    type_into(&mut harness, "New secret name", "token");
    choose(&mut harness, "Add secret");
    type_into(&mut harness, "Value of token", "first-session-value");
    assert_within_window(&harness, &["Secret name", "Value of token", "Show secret"]);
    choose(&mut harness, "Apply environment");
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );
    choose(&mut harness, "Save all");
    let saved = fs::read_to_string(directory.path().join("apiforge.project.json")).unwrap();
    assert!(saved.contains("token"));
    assert!(!saved.contains("first-session-value"));
    assert_eq!(
        folder(&harness).runs.secrets["token"],
        "first-session-value"
    );

    // A changed value alone leaves the public settings clean.
    open_project_menu(&mut harness, "Test environment…");
    replace_text(&mut harness, "Value of token", "second-session-value");
    choose(&mut harness, "Apply environment");
    assert!(
        !folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );
    assert_eq!(
        folder(&harness).runs.secrets["token"],
        "second-session-value"
    );
    let saved = fs::read_to_string(directory.path().join("apiforge.project.json")).unwrap();
    assert!(!saved.contains("session-value"));

    drop(harness);
    let reopened = folder_harness(directory.path(), MINIMUM);
    assert!(folder(&reopened).runs.secrets.is_empty());
}

/// Observing another window, the project settings open for reading only
/// (MCP-08a: before it the menu entry was disabled) and the environment
/// offers only the session's secret values.
#[test]
fn a_read_only_window_offers_only_session_secret_values() {
    let (directory, _) = named_folder("Read-only settings");
    let _writer = folder_harness(directory.path(), MINIMUM);
    let mut harness = folder_harness(directory.path(), MINIMUM);
    assert!(folder(&harness).session.as_ref().unwrap().is_read_only());
    choose(&mut harness, "Project");
    assert!(
        !harness
            .get_by_label("Test environment…")
            .accesskit_node()
            .is_disabled()
    );
    choose(&mut harness, "Project settings…");
    assert!(
        harness
            .get_by_label("Project name")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_by_label("Apply settings")
            .accesskit_node()
            .is_disabled()
    );
}

// ----- P7c: preview-bound conflict and recovery decisions --------------------------

#[test]
fn a_conflict_decision_applies_only_to_the_reviewed_disk_version() {
    let (directory, _) = source_project();
    let path = directory.path().join("main.afs");
    let mut harness = folder_harness(directory.path(), MINIMUM);
    replace_text(&mut harness, EDITOR, LOCAL);
    fs::write(&path, FOREIGN).unwrap();
    follow_disk(&mut harness);

    choose(&mut harness, "Review conflict…");
    assert!(has_label(&harness, "Review file conflict"));
    choose(&mut harness, "Cancel");
    assert!(folder(&harness).prompt.is_none());
    assert_eq!(editor_text(&harness), LOCAL);
    assert_eq!(fs::read_to_string(&path).unwrap(), FOREIGN);

    choose(&mut harness, "Review conflict…");
    let latest = FOREIGN.replace('5', "9");
    fs::write(&path, &latest).unwrap();
    choose(&mut harness, "Use version on disk");
    assert!(has_label(&harness, "Review file conflict"));
    assert_eq!(editor_text(&harness), LOCAL);

    choose(&mut harness, "Refresh preview");
    choose(&mut harness, "Use version on disk");
    assert!(folder(&harness).prompt.is_none());
    assert_eq!(editor_text(&harness), latest);
    assert!(has_label(&harness, "Loaded the reviewed version on disk."));
}

#[test]
fn the_conflict_dialog_fits_the_window() {
    let controls = [
        "Use version on disk",
        "Refresh preview",
        "Export disk version…",
        "Cancel",
    ];
    for size in [MINIMUM, WIDE] {
        let (directory, _) = source_project();
        let mut harness = folder_harness(directory.path(), size);
        replace_text(&mut harness, EDITOR, LOCAL);
        fs::write(directory.path().join("main.afs"), FOREIGN).unwrap();
        follow_disk(&mut harness);
        choose(&mut harness, "Review conflict…");
        assert_within_window(&harness, &controls);
        assert_disjoint(&harness, &controls);
    }
}

#[test]
fn an_interrupted_operation_is_finished_only_for_the_reviewed_files() {
    let (directory, _) = source_project();
    interrupt(directory.path());
    let path = directory.path().join("main.afs");
    let controls = [
        "Finish intended operation",
        "Refresh preview",
        "Export recovery evidence…",
        "Export current…",
        "Export intended…",
        "Cancel",
    ];
    let mut harness = folder_harness(directory.path(), MINIMUM);
    assert!(!folder(&harness).has_document());
    assert!(has_label(&harness, "Recover interrupted file operation"));
    assert_within_window(&harness, &controls);
    assert_disjoint(&harness, &controls);

    let latest = FOREIGN.replace('5', "9");
    fs::write(&path, &latest).unwrap();
    choose(&mut harness, "Finish intended operation");
    assert_active(directory.path(), true);
    assert_eq!(fs::read_to_string(&path).unwrap(), latest);

    choose(&mut harness, "Refresh preview");
    choose(&mut harness, "Finish intended operation");
    assert_active(directory.path(), false);
    assert_eq!(fs::read_to_string(&path).unwrap(), INTENDED);
    assert!(has_label(&harness, "Reload project after recovery"));

    choose(&mut harness, "Open project");
    assert!(folder(&harness).has_document());
    assert_eq!(editor_text(&harness), INTENDED);
}

#[test]
fn the_recovery_dialog_fits_the_default_window_and_cancel_keeps_the_files() {
    let (directory, _) = source_project();
    interrupt(directory.path());
    let mut harness = folder_harness(directory.path(), WIDE);
    choose(&mut harness, "Inspect candidates");
    let controls = [
        "Finish intended operation",
        "Export current…",
        "Export intended…",
        "Cancel",
    ];
    assert_within_window(&harness, &controls);
    assert_disjoint(&harness, &controls);
    choose(&mut harness, "Cancel");
    assert_active(directory.path(), true);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        FOREIGN
    );
    assert!(has_label(&harness, "Review transaction recovery…"));
}

#[test]
fn a_read_only_window_cannot_finish_another_writers_recovery() {
    let (directory, _) = source_project();
    let _owner = apiforge_storage::project::ProjectStore::open_exclusive(directory.path()).unwrap();
    interrupt(directory.path());
    let harness = folder_harness(directory.path(), MINIMUM);
    assert!(
        harness
            .get_by_label("Finish intended operation")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .query_by_label_contains("Read-only: another writer must finish recovery")
            .is_some()
    );
    assert!(
        !harness
            .get_by_label("Export recovery evidence…")
            .accesskit_node()
            .is_disabled()
    );
}

// ----- P7d: the interface stays usable while storage works -------------------------

#[test]
fn typing_continues_while_storage_reads() {
    let (directory, _) = named_folder("Responsive typing");
    let mut harness = folder_harness(directory.path(), MINIMUM);
    let release = hold_storage(&mut harness);
    harness.get_by_label(EDITOR).focus();
    harness.step();
    let before = Instant::now();
    harness
        .get_by_label(EDITOR)
        .type_text("// typed while reading\n");
    harness.step();
    harness.step();
    assert!(before.elapsed() < Duration::from_millis(500));
    assert!(folder(&harness).storage.is_busy());
    assert!(editor_text(&harness).starts_with("// typed while reading\n"));
    release.send(()).unwrap();
    settle(&mut harness);
    assert!(editor_text(&harness).starts_with("// typed while reading\n"));
}

#[test]
fn queued_settings_show_a_working_dialog_and_apply_after_earlier_storage_work() {
    let (directory, _) = named_folder("Queued settings");
    let mut harness = folder_harness(directory.path(), MINIMUM);
    open_project_menu(&mut harness, "Project settings…");
    replace_text(&mut harness, "Project name", "Queued after");
    let release = hold_storage(&mut harness);
    harness.get_by_label("Apply settings").click();
    harness.step();
    harness.step();
    assert!(has_label(&harness, "Applying project settings…"));
    assert_within_window(&harness, &["Applying project settings…"]);
    assert_eq!(
        folder(&harness).session.as_ref().unwrap().manifest().name,
        "Queued settings"
    );
    release.send(()).unwrap();
    settle(&mut harness);
    assert!(!has_label(&harness, "Applying project settings…"));
    assert_eq!(
        folder(&harness).session.as_ref().unwrap().manifest().name,
        "Queued after"
    );
}

#[test]
fn a_switch_waits_for_storage_before_leaving_the_project() {
    let (alpha, _) = named_folder("Waiting Alpha");
    let (beta, _) = named_folder("Waiting Beta");
    let mut harness = folder_harness(alpha.path(), MINIMUM);
    remember(&mut harness, beta.path());
    choose(&mut harness, "Project");
    let release = hold_storage(&mut harness);
    harness.get_by_label("Waiting Beta").click();
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(folder(&harness).root, alpha.path());
    release.send(()).unwrap();
    settle(&mut harness);
    assert_eq!(folder(&harness).root, beta.path());
}

/// Design review views of the project dialogs. Run by `tools/gui/review.sh`.
mod review {
    use super::*;
    use crate::testing::{capture, folder_harness_scaled};

    const SIZES: [(egui::Vec2, f32, &str); 3] = [
        (MINIMUM, 1.0, "960x540"),
        (WIDE, 1.0, "1600x1000"),
        (MINIMUM, 2.0, "960x540@2x"),
    ];

    /// Captures the state `prepare` leaves at each review size.
    fn views(
        name: &str,
        project: impl Fn() -> tempfile::TempDir,
        prepare: impl Fn(&mut FolderHarness),
    ) {
        for (size, scale, suffix) in SIZES {
            let directory = project();
            let mut harness = folder_harness_scaled(directory.path(), size, scale);
            prepare(&mut harness);
            capture(&mut harness, &format!("project-{name}-{suffix}"), None);
        }
    }

    fn named() -> tempfile::TempDir {
        named_folder("Review project").0
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_project_menu_with_recent_projects() {
        let other = named_folder("Another project with a long descriptive name").0;
        views("menu", named, |harness| {
            remember(harness, other.path());
            choose(harness, "Project");
        });
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_project_settings_dialog() {
        views("settings", named, |harness| {
            open_project_menu(harness, "Project settings…");
        });
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_project_settings_with_an_invalid_name() {
        views("settings-invalid", named, |harness| {
            open_project_menu(harness, "Project settings…");
            replace_text(harness, "Project name", "");
        });
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_environment_dialog_with_a_secret() {
        views("environment", named, |harness| {
            open_project_menu(harness, "Test environment…");
            type_into(harness, "New secret name", "token");
            choose(harness, "Add secret");
            type_into(harness, "Value of token", "session-value");
        });
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_switch_with_failed_recovery() {
        let other = named_folder("Other project").0;
        views("switch-failed", named, |harness| {
            remember(harness, other.path());
            type_into(harness, EDITOR, "// unfinished\n");
            let recovery = folder(harness).root.join(".apiforge/local/recovery.json");
            fs::write(recovery, b"foreign recovery").unwrap();
            open_project_menu(harness, "Other project");
        });
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_start_screen_of_a_folder_without_a_project() {
        views("start", || tempfile::tempdir().unwrap(), |_| {});
    }

    fn conflicted(harness: &mut FolderHarness) {
        replace_text(harness, EDITOR, LOCAL);
        fs::write(folder(harness).root.join("main.afs"), FOREIGN).unwrap();
        follow_disk(harness);
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_file_conflict_notice() {
        views("conflict-notice", || source_project().0, conflicted);
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_file_conflict_dialog() {
        views(
            "conflict",
            || source_project().0,
            |harness| {
                conflicted(harness);
                choose(harness, "Review conflict…");
            },
        );
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_transaction_recovery_dialog() {
        let project = || {
            let directory = source_project().0;
            interrupt(directory.path());
            directory
        };
        views("recovery", project, |harness| {
            choose(harness, "Inspect candidates");
        });
        views("recovery-finished", project, |harness| {
            choose(harness, "Finish intended operation");
        });
    }

    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_working_dialog() {
        for (size, scale, suffix) in SIZES {
            let directory = named();
            let mut harness = folder_harness_scaled(directory.path(), size, scale);
            open_project_menu(&mut harness, "Project settings…");
            let release = hold_storage(&mut harness);
            harness.get_by_label("Apply settings").click();
            harness.step();
            harness.step();
            capture(&mut harness, &format!("project-working-{suffix}"), None);
            release.send(()).unwrap();
            settle(&mut harness);
        }
    }
}
