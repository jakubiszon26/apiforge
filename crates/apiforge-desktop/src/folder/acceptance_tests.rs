//! Stage II cross-package acceptance through the real window (ADR 0013).
//!
//! The package suites each drive their own feature. These drive whole user
//! lifecycles across packages — managed startup over migration, editing over
//! publication over execution, and one document's conflict beside another
//! document's unsaved work — because that is where a package that keeps its
//! own state correct can still leave the next one with the wrong state.
use super::activation_tests::{catalogue, legacy};
use super::project_interaction_tests::{
    app, choose, follow_disk, has_label, open_project_menu, remember, replace_text, type_into,
};
use super::*;
use crate::testing::{FolderHarness, WIDE, folder, folder_harness, managed_harness, settle};
use apiforge_core::run::{RunResult, RunStatus};
use egui_kittest::kittest::Queryable;
use std::fs;

/// The accessible name of a document's editor, which is its registry path.
fn editor(harness: &FolderHarness, document_id: &str) -> String {
    format!(
        "Contents of {}",
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .document(document_id)
            .unwrap()
            .entry()
            .path
            .as_str()
    )
}

fn only(harness: &FolderHarness, kind: DocumentKind) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().kind == kind)
        .expect("the demo has one of each")
        .id()
        .to_owned()
}

fn path_of(harness: &FolderHarness, document_id: &str) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(document_id)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned()
}

fn source_of(harness: &FolderHarness, document_id: &str) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(document_id)
        .unwrap()
        .text()
        .to_owned()
}

fn text_of(harness: &FolderHarness, path: &str) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .text_file(path)
        .unwrap_or_else(|| panic!("{path} is open as text"))
        .text()
        .to_owned()
}

fn saved_name(root: &std::path::Path) -> String {
    apiforge_project::ProjectManifest::from_bytes(
        &fs::read(root.join("apiforge.project.json")).unwrap(),
    )
    .unwrap()
    .name
}

/// Steps until the run started from the window has published its last report.
fn finish_run(harness: &mut FolderHarness) -> RunResult {
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
    let active = folder(harness).workbench.active().unwrap().to_owned();
    folder(harness)
        .run_result(&active)
        .expect("a finished run leaves a report")
        .clone()
}

/// Every block the report names, which is the identity the stored layout, the
/// diagram selection and a later report all share.
fn reported_blocks(report: &RunResult) -> Vec<String> {
    report
        .steps
        .iter()
        .map(|step| step.node_id.clone())
        .collect()
}

/// P7g over P3 over P4 over P6b over P6d over P1: managed startup migrates a
/// legacy workspace, and from then on the migrated project behaves like any
/// other — it is edited, saved, published from, run, closed and reopened, and
/// the restart has to find exactly what the session left.
#[test]
fn a_migrated_project_is_edited_published_run_and_unchanged_after_a_restart() {
    let data = tempfile::tempdir().unwrap();
    let workspace = legacy(data.path());

    let mut harness = managed_harness(data.path(), WIDE);
    assert!(
        folder(&harness).open_error.is_none(),
        "{:?}",
        folder(&harness).open_error
    );
    let migration = catalogue(data.path()).migration_id;
    let root = folder(&harness).root.clone();
    let scenario = only(&harness, DocumentKind::Scenario);
    let component = only(&harness, DocumentKind::Component);
    let scenario_path = path_of(&harness, &scenario);
    let revisions = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .manifest()
        .revisions
        .len();

    // Edit the migrated source in Code and save it, as Ctrl+S does.
    let label = editor(&harness, &scenario);
    type_into(&mut harness, &label, "// reviewed after migration\n");
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .is_dirty()
    );
    choose(&mut harness, "Save");
    let saved = source_of(&harness, &scenario);
    assert!(saved.starts_with("// reviewed after migration\n"));
    assert_eq!(
        fs::read_to_string(root.join(&scenario_path)).unwrap(),
        saved
    );

    // Publish the next revision of a migrated component. The scenario keeps
    // the revision it was migrated against, so its source must not change.
    app(&mut harness).activate_tab(&component);
    settle(&mut harness);
    choose(&mut harness, "Publish");
    assert_eq!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .manifest()
            .revisions
            .len(),
        revisions + 1
    );
    assert_eq!(source_of(&harness, &scenario), saved);

    // Run the saved scenario and keep its report for the restart.
    app(&mut harness).activate_tab(&scenario);
    settle(&mut harness);
    choose(&mut harness, "Run");
    let report = finish_run(&mut harness);
    assert_eq!(report.status, RunStatus::Passed);
    assert!(folder(&harness).fresh_run(&scenario).is_some());

    // Close the window the way the application closes it, then start again.
    let deadline = Instant::now() + Duration::from_secs(15);
    while !app(&mut harness).prepare_close() {
        assert!(
            folder(&harness).close_error.is_none(),
            "{:?}",
            folder(&harness).close_error
        );
        assert!(Instant::now() < deadline, "close did not finish");
        harness.step();
    }
    drop(harness);

    let mut harness = managed_harness(data.path(), WIDE);
    assert_eq!(folder(&harness).root, root, "the same project reopens");
    assert_eq!(
        catalogue(data.path()).migration_id,
        migration,
        "a restart does not migrate again"
    );
    assert_eq!(
        catalogue(data.path()).projects.len(),
        workspace.projects.len()
    );
    let scenario = only(&harness, DocumentKind::Scenario);
    assert_eq!(source_of(&harness, &scenario), saved, "the saved source");
    assert!(
        !folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .is_dirty()
    );
    assert_eq!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .manifest()
            .revisions
            .len(),
        revisions + 1,
        "the published revision survived"
    );

    // The same saved project runs the same way, over the same blocks: the
    // identity the earlier session reported is the identity it reopens with.
    app(&mut harness).activate_tab(&scenario);
    settle(&mut harness);
    choose(&mut harness, "Run");
    let again = finish_run(&mut harness);
    assert_eq!(again.status, report.status);
    assert_eq!(reported_blocks(&again), reported_blocks(&report));
}

/// P7g over P5 over P2 over P6b over P7b: a managed project created from
/// nothing adopts a source that appeared in its folder, that source's
/// unqualified call is fixed through its diagnostic, and the pin and the
/// project settings have to be there after a restart.
#[test]
fn a_created_project_registers_a_found_source_pins_its_call_and_keeps_settings() {
    let data = tempfile::tempdir().unwrap();
    let mut harness = managed_harness(data.path(), WIDE);
    let root = folder(&harness).root.clone();
    assert!(
        !data.path().join(apiforge_project::CATALOGUE_FILE).exists(),
        "nothing was migrated"
    );
    let handle = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .context()
        .components
        .first()
        .expect("the created project publishes components")
        .handle
        .clone();

    // A source appears in the folder calling a published component without
    // naming its revision, which folder projects reject.
    fs::write(
        root.join("found.afs"),
        format!("scenario \"Found\" {{\n  call = ({handle}())\n}}\n"),
    )
    .unwrap();
    follow_disk(&mut harness);
    app(&mut harness).explorer_action(explorer::Action::Register("found.afs".into()));
    settle(&mut harness);
    let found = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "found.afs")
        .expect("the found source is registered")
        .id()
        .to_owned();
    assert_eq!(folder(&harness).workbench.active(), Some(found.as_str()));
    assert_eq!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&found)
            .unwrap()
            .analysis(),
        AnalysisState::Invalid,
        "an unqualified call is a diagnostic, not a silent latest"
    );

    // The diagnostic's own fix is what pins it.
    choose(&mut harness, "Apply fix");
    settle(&mut harness);
    assert!(source_of(&harness, &found).contains(&format!("{handle}@1(")));
    assert_eq!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&found)
            .unwrap()
            .analysis(),
        AnalysisState::Valid
    );
    choose(&mut harness, "Save");
    let pinned = source_of(&harness, &found);
    assert_eq!(fs::read_to_string(root.join("found.afs")).unwrap(), pinned);

    // Project settings are a separate transaction from any source.
    open_project_menu(&mut harness, "Project settings…");
    replace_text(&mut harness, "Project name", "Adopted");
    choose(&mut harness, "Apply settings");
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );
    choose(&mut harness, "Save all");
    assert!(
        !folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );
    drop(harness);

    let harness = managed_harness(data.path(), WIDE);
    assert_eq!(folder(&harness).root, root);
    let session = folder(&harness).session.as_ref().unwrap();
    assert_eq!(session.manifest().name, "Adopted");
    let found = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "found.afs")
        .expect("the registration survived");
    assert_eq!(found.text(), pinned, "the pin survived");
    assert_eq!(found.analysis(), AnalysisState::Valid);
}

/// P4 over P5 over P7c over P2: two documents are edited independently, one is
/// taken over by an external writer, and resolving that one's conflict must
/// not touch the other's unsaved text, its dirty state or its text history.
#[test]
fn resolving_one_documents_conflict_leaves_another_documents_unsaved_work_and_history() {
    let directory = shop();
    let mut harness = folder_harness(directory.path(), WIDE);
    let scenario = only(&harness, DocumentKind::Scenario);
    let component = only(&harness, DocumentKind::Component);
    let scenario_path = path_of(&harness, &scenario);
    let component_original = source_of(&harness, &component);

    // Unsaved work in both documents, typed into their own editors.
    let component_label = editor(&harness, &component);
    app(&mut harness).activate_tab(&component);
    settle(&mut harness);
    type_into(&mut harness, &component_label, "// component note\n");
    let component_edited = source_of(&harness, &component);
    assert!(component_edited.starts_with("// component note\n"));

    let scenario_label = editor(&harness, &scenario);
    app(&mut harness).activate_tab(&scenario);
    settle(&mut harness);
    type_into(&mut harness, &scenario_label, "// scenario note\n");

    // Something else rewrites only the scenario's file.
    let foreign = format!(
        "{}\n// written outside\n",
        fs::read_to_string(directory.path().join(&scenario_path))
            .unwrap()
            .trim_end()
    );
    fs::write(directory.path().join(&scenario_path), &foreign).unwrap();
    follow_disk(&mut harness);
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .conflict()
            .is_some(),
        "unsaved work against a foreign write is a conflict"
    );
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&component)
            .unwrap()
            .conflict()
            .is_none(),
        "the other document was not written to"
    );
    assert_eq!(source_of(&harness, &component), component_edited);

    // Take the version on disk for the conflicted document only.
    choose(&mut harness, "Review conflict…");
    assert!(has_label(&harness, "Review file conflict"));
    choose(&mut harness, "Use version on disk");
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .conflict()
            .is_none()
    );
    assert_eq!(source_of(&harness, &scenario), foreign);

    // The untouched document kept its work, its dirty state and its history.
    let session = folder(&harness).session.as_ref().unwrap();
    let document = session.document(&component).unwrap();
    assert_eq!(document.text(), component_edited);
    assert!(document.is_dirty());
    assert_eq!(
        fs::read_to_string(directory.path().join(path_of(&harness, &component))).unwrap(),
        component_original,
        "and nothing of it was saved"
    );

    app(&mut harness).activate_tab(&component);
    settle(&mut harness);
    harness.get_by_label(&component_label).focus();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut harness);
    assert_eq!(
        source_of(&harness, &component),
        component_original,
        "its own text history is still the one the editor walks"
    );
}

/// A refused departure reports why it was refused. Once the user has kept the
/// window and dealt with the cause, that refusal belongs to the attempt that
/// is over: the next departure has to be judged on the state it finds.
#[test]
fn a_cancelled_switch_leaves_no_refusal_behind_for_the_next_one() {
    let (alpha, _) = named_folder("Stale Alpha");
    let (beta, _) = named_folder("Stale Beta");
    let mut harness = folder_harness(alpha.path(), WIDE);
    remember(&mut harness, beta.path());

    // Unsaved configuration is the one kind of work a departure refuses.
    open_project_menu(&mut harness, "Project settings…");
    replace_text(&mut harness, "Project name", "Stale Renamed");
    choose(&mut harness, "Apply settings");
    open_project_menu(&mut harness, "Stale Beta");
    assert!(has_label(&harness, "Unsaved work is not protected"));
    choose(&mut harness, "Keep editing");

    choose(&mut harness, "Save all");
    assert!(
        !folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );
    assert_eq!(
        app(&mut harness).close_error(),
        None,
        "the refusal of the cancelled attempt is still waiting to be reported"
    );
}

/// P7a over P7b over P6a over P1: leaving a project that has unsaved work of
/// three different kinds. Unsaved project configuration is the one kind that
/// stops the departure rather than being carried, because nothing can decide
/// for the user whether a renamed project keeps its new name. Keeping the
/// window must leave all three intact, and once the configuration is decided
/// the switch carries a document's source and a project text file both ways.
#[test]
fn unsaved_configuration_stops_a_switch_that_otherwise_carries_source_and_text() {
    let (alpha, source) = named_folder("Carry Alpha");
    let (beta, _) = named_folder("Carry Beta");
    fs::write(alpha.path().join("notes.md"), "# Notes\n").unwrap();

    let mut harness = folder_harness(alpha.path(), WIDE);
    remember(&mut harness, beta.path());
    follow_disk(&mut harness);

    type_into(&mut harness, "Contents of main.afs", "// unfinished\n");
    let edited_source = source_of(&harness, &source);
    assert!(edited_source.starts_with("// unfinished\n"));

    app(&mut harness).open_text("notes.md");
    settle(&mut harness);
    type_into(&mut harness, "Contents of notes.md", "draft line\n");
    let edited_text = text_of(&harness, "notes.md");
    assert_eq!(edited_text, "# Notes\ndraft line\n");

    open_project_menu(&mut harness, "Project settings…");
    replace_text(&mut harness, "Project name", "Carry Renamed");
    choose(&mut harness, "Apply settings");
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );

    // The switch stops at an explicit decision about the configuration.
    open_project_menu(&mut harness, "Carry Beta");
    assert!(has_label(&harness, "Unsaved work is not protected"));
    assert_eq!(
        folder(&harness).root,
        alpha.path(),
        "the current project is kept until the configuration is decided"
    );
    choose(&mut harness, "Keep editing");
    assert_eq!(folder(&harness).root, alpha.path());
    assert_eq!(source_of(&harness, &source), edited_source);
    assert_eq!(text_of(&harness, "notes.md"), edited_text);
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );
    assert_eq!(saved_name(alpha.path()), "Carry Alpha");

    // Deciding it with Save all saves the two dirty files as well, so fresh
    // unsaved work is what the switch then has to carry.
    choose(&mut harness, "Save all");
    assert!(
        !folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .is_manifest_dirty()
    );
    assert_eq!(saved_name(alpha.path()), "Carry Renamed");
    assert_eq!(
        fs::read_to_string(alpha.path().join("main.afs")).unwrap(),
        edited_source
    );

    app(&mut harness).activate_tab(&source);
    settle(&mut harness);
    type_into(&mut harness, "Contents of main.afs", "// later\n");
    let later_source = source_of(&harness, &source);
    app(&mut harness).open_text("notes.md");
    settle(&mut harness);
    type_into(&mut harness, "Contents of notes.md", "later line\n");
    let later_text = text_of(&harness, "notes.md");

    open_project_menu(&mut harness, "Carry Beta");
    assert_eq!(folder(&harness).root, beta.path());
    assert_eq!(
        fs::read_to_string(alpha.path().join("main.afs")).unwrap(),
        edited_source,
        "leaving saves nothing"
    );
    assert_eq!(
        fs::read_to_string(alpha.path().join("notes.md")).unwrap(),
        edited_text
    );
    assert!(alpha.path().join(".apiforge/local/recovery.json").is_file());

    open_project_menu(&mut harness, "Carry Renamed");
    assert_eq!(folder(&harness).root, alpha.path());
    let session = folder(&harness).session.as_ref().unwrap();
    let returned = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "main.afs")
        .unwrap();
    assert_eq!(returned.text(), later_source);
    assert!(returned.is_dirty());
    assert_eq!(
        text_of(&harness, "notes.md"),
        later_text,
        "the text file came back open and unsaved"
    );
    assert_eq!(saved_name(alpha.path()), "Carry Renamed");
}

/// KTLO-08: `--screenshot --folder … --mode` shows the folder window in the
/// requested mode at startup, and that request is a capture setting, never the
/// project's: the view written back keeps the mode the folder would restore.
#[test]
fn a_mode_requested_at_startup_is_shown_and_not_written_back() {
    use crate::testing::{MINIMUM, folder_harness_in, shop_project};

    let project = shop_project();
    let harness = folder_harness(project.path(), WIDE);
    assert_eq!(
        folder(&harness).workbench.mode,
        Mode::Code,
        "without a request the folder's own mode is shown, as before"
    );
    drop(harness);

    for mode in [Mode::Code, Mode::Diagram, Mode::Split] {
        for size in [MINIMUM, WIDE] {
            let project = shop_project();
            let mut harness = folder_harness_in(project.path(), size, mode);
            let app = harness.state_mut().as_mut().unwrap();
            assert!(app.has_document(), "the demo shop opened");
            assert_eq!(app.workbench.mode, mode, "{mode:?} at {size:?} is shown");
            assert_eq!(
                app.capture_view().map(|view| view.mode),
                Some(Mode::Code),
                "{mode:?} is not written back as the project's view"
            );
        }
    }
}

// ----- MCP-07: the lock names its holder; the window takes the project back -----

mod mcp07 {
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use apiforge_storage::project::{Holder, HolderRole, ProjectStore};
    use chrono::TimeZone;
    use egui_kittest::kittest::NodeT;

    /// The bar's region, named for assistive technology and for these tests.
    const BAR: &str = "Project holder";
    const PAUSED: &str = "editing is paused here; this window follows every change";
    /// Criterion 3's bound: the window is the writer this soon after a release.
    const TAKE_BACK: Duration = Duration::from_secs(2);

    /// 10:42 on the wall clock of the machine running the tests, as the
    /// holder file spells it: UTC, RFC 3339.
    fn at(hour: u32, minute: u32) -> String {
        chrono::Local
            .with_ymd_and_hms(2026, 10, 27, hour, minute, 11)
            .single()
            .expect("an unambiguous local time")
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    }

    fn agent() -> Holder {
        Holder::new(HolderRole::Agent, Some("Claude Code".into())).with_since(at(10, 42))
    }

    fn hold(root: &std::path::Path, holder: &Holder) -> ProjectStore {
        ProjectStore::open_exclusive_as(root, holder).unwrap()
    }

    fn read_only(harness: &FolderHarness) -> bool {
        folder(harness).session.as_ref().unwrap().is_read_only()
    }

    fn disabled(harness: &FolderHarness, label: &str) -> bool {
        harness.get_by_label(label).accesskit_node().is_disabled()
    }

    /// Steps the window in real time until it is the writer, and returns how
    /// long that took after `released`.
    fn await_writer(harness: &mut FolderHarness, released: Instant) -> Duration {
        let deadline = released + Duration::from_secs(10);
        while read_only(harness) {
            assert!(
                Instant::now() < deadline,
                "the window never took the project back"
            );
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        let taken = released.elapsed();
        settle(harness);
        taken
    }

    /// Criterion 2: the bar names the holder and when it took the project;
    /// the header badge says the window is observing; the old notice is gone.
    #[test]
    fn a_window_opened_while_another_process_holds_the_project_names_the_holder() {
        for (holder, text) in [
            (
                agent(),
                format!("Claude Code via apiforge mcp · since 10:42 · {PAUSED}"),
            ),
            (
                Holder::window().with_since(at(10, 40)),
                format!("Another API Forge window · since 10:40 · {PAUSED}"),
            ),
            (
                Holder::command("apply").with_since(at(10, 41)),
                format!("apiforge apply · since 10:41 · {PAUSED}"),
            ),
        ] {
            let directory = shop();
            let owner = hold(directory.path(), &holder);
            let harness = folder_harness(directory.path(), WIDE);
            assert!(read_only(&harness));
            assert!(has_label(&harness, &text), "{text}");
            assert!(has_label(&harness, "Observing"));
            assert!(!has_label(&harness, "Read-only"));
            assert!(has_label(&harness, BAR));
            assert!(
                harness
                    .query_by_label_contains("Another window is editing this project")
                    .is_none(),
                "the notice under the tabs is gone"
            );
            drop(harness);
            drop(owner);
        }
    }

    /// Criterion 2: a lock held by an older build, which wrote no
    /// description, or a description that cannot be read, gets the generic
    /// wording.
    #[test]
    fn a_holder_that_wrote_no_readable_description_is_another_process() {
        for description in [None, Some(&b"{ not a description"[..])] {
            let directory = shop();
            let owner = ProjectStore::open_exclusive(directory.path()).unwrap();
            let file = directory.path().join(".apiforge/local/holder.json");
            match description {
                None => fs::remove_file(&file).unwrap(),
                Some(bytes) => fs::write(&file, bytes).unwrap(),
            }
            let harness = folder_harness(directory.path(), WIDE);
            assert!(has_label(&harness, &format!("Another process · {PAUSED}")));
            assert!(has_label(&harness, "Observing"));
            drop(harness);
            drop(owner);
        }
    }

    /// Review round 1 of #106: a description this user cannot read is the
    /// generic wording too, not a plain read-only window. Unix only: Windows
    /// permissions cannot be modelled with a mode here.
    #[cfg(unix)]
    #[test]
    fn a_holder_whose_description_cannot_be_read_is_another_process() {
        use std::os::unix::fs::PermissionsExt;
        let directory = shop();
        let owner = hold(directory.path(), &agent());
        let file = directory.path().join(".apiforge/local/holder.json");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
        // Privileged users read it regardless; then there is nothing to verify.
        if fs::read(&file).is_ok() {
            return;
        }
        let harness = folder_harness(directory.path(), WIDE);
        assert!(has_label(&harness, &format!("Another process · {PAUSED}")));
        assert!(has_label(&harness, "Observing"));
        drop(harness);
        drop(owner);
    }

    /// Criterion 3: the holder lets go; within two seconds the window is the
    /// writer, the badge and the bar are gone, Save is enabled and saves.
    /// MCP-08a (decision 12) pauses typing while observing, so the text is
    /// typed after the take-back.
    #[test]
    fn a_released_project_is_taken_back_within_two_seconds_and_saves() {
        let directory = shop();
        let owner = hold(directory.path(), &agent());
        let mut harness = folder_harness(directory.path(), WIDE);
        let scenario = only(&harness, DocumentKind::Scenario);
        let label = editor(&harness, &scenario);
        assert!(disabled(&harness, "Save"), "a read-only window cannot save");

        drop(owner);
        let taken = await_writer(&mut harness, Instant::now());
        assert!(taken <= TAKE_BACK, "taken back after {taken:?}");
        assert!(!has_label(&harness, "Observing"));
        assert!(!has_label(&harness, BAR));
        assert!(
            harness
                .query_by_label_contains("editing is paused here")
                .is_none()
        );
        type_into(&mut harness, &label, "// typed after the take-back\n");
        let typed = source_of(&harness, &scenario);
        assert!(typed.contains("// typed after the take-back"));
        assert!(!disabled(&harness, "Save"));
        assert!(!disabled(&harness, "Save all"));
        choose(&mut harness, "Save");
        let path = path_of(&harness, &scenario);
        assert_eq!(
            fs::read_to_string(directory.path().join(path)).unwrap(),
            typed
        );
        assert!(
            ProjectStore::open_read_only(directory.path())
                .unwrap()
                .holder()
                .unwrap()
                .is_some_and(|holder| holder.role() == Some(HolderRole::Window)),
            "the window now names itself as the holder"
        );
    }

    /// Criterion 3: *Publish* is enabled again on a component's tab.
    #[test]
    fn a_taken_back_window_can_publish_again() {
        let directory = shop();
        let owner = hold(directory.path(), &agent());
        let mut harness = folder_harness(directory.path(), WIDE);
        let component = only(&harness, DocumentKind::Component);
        app(&mut harness).activate_tab(&component);
        settle(&mut harness);
        let label = editor(&harness, &component);
        assert!(disabled(&harness, "Publish"));
        drop(owner);
        await_writer(&mut harness, Instant::now());
        // Typing is paused while observing (MCP-08a), so the change that
        // makes a next revision is typed once the window is the writer.
        type_into(&mut harness, &label, "// next revision\n");
        assert!(!disabled(&harness, "Publish"));
    }

    /// Criterion 4: while the holder holds the lock the window waits, however
    /// long, and never touches the lock or the holder's description.
    #[test]
    fn a_held_project_is_never_taken_however_long_it_is_held() {
        let directory = shop();
        let owner = hold(directory.path(), &agent());
        let lock = directory.path().join(".apiforge/local/project.lock");
        let description = directory.path().join(".apiforge/local/holder.json");
        let described = fs::read(&description).unwrap();
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&lock).unwrap().ino()
        };
        let mut harness = folder_harness(directory.path(), WIDE);
        let text = format!("Claude Code via apiforge mcp · since 10:42 · {PAUSED}");
        let until = Instant::now() + watcher::POLL_INTERVAL * 4;
        while Instant::now() < until {
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(&mut harness);
        assert!(read_only(&harness));
        assert!(has_label(&harness, &text));
        assert!(has_label(&harness, "Observing"));
        assert!(!owner.is_read_only(), "the holder still holds the project");
        // Held, so only its length: Windows refuses to read a locked file.
        assert_eq!(
            fs::metadata(&lock).unwrap().len(),
            0,
            "the lock file is untouched"
        );
        assert_eq!(fs::read(&description).unwrap(), described);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(fs::metadata(&lock).unwrap().ino(), identity);
        }
        drop(owner);
        await_writer(&mut harness, Instant::now());
    }

    /// Criterion 5: a holder that died with an activated transaction leaves
    /// it to the window, which opens the recovery view it opens at start, now
    /// able to finish it.
    #[test]
    fn a_transaction_left_by_the_holder_opens_the_recovery_view() {
        use super::super::recovery_tests::{INTENDED, assert_active, interrupt, source_project};
        let (directory, _) = source_project();
        let owner = hold(directory.path(), &agent());
        let mut harness = folder_harness(directory.path(), WIDE);
        assert!(read_only(&harness));
        interrupt(directory.path());
        drop(owner);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !has_label(&harness, "Recover interrupted file operation") {
            assert!(Instant::now() < deadline, "the recovery view opens");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(&mut harness);
        assert!(!disabled(&harness, "Finish intended operation"));
        choose(&mut harness, "Finish intended operation");
        assert_active(directory.path(), false);
        assert_eq!(
            fs::read_to_string(directory.path().join("main.afs")).unwrap(),
            INTENDED
        );
        choose(&mut harness, "Reload project");
        await_writer(&mut harness, Instant::now());
        assert!(!has_label(&harness, "Observing"));
    }

    /// Criterion 5: a holder that died during a live run leaves its journal;
    /// on take-back the window lists the interrupted run, as at start.
    #[test]
    fn a_live_run_interrupted_in_the_holder_is_reported_on_take_back() {
        use apiforge_core::journal::{JournalRun, JournalSink};
        let directory = shop();
        let owner = hold(directory.path(), &Holder::command("run"));
        let journal = apiforge_storage::JournalStore::open_project(owner.clone()).unwrap();
        journal
            .store
            .begin_run(&JournalRun::new(
                "interrupted",
                "main",
                "Main",
                "2026-10-27T10:41:00Z",
            ))
            .unwrap();
        drop(journal);
        let mut harness = folder_harness(directory.path(), WIDE);
        assert!(
            !has_label(&harness, "Mark as handled"),
            "not while observing"
        );
        drop(owner);
        await_writer(&mut harness, Instant::now());
        assert!(has_label(&harness, "Mark as handled"));
    }

    /// Criterion 6: at the minimum window the bar and the badge stay inside
    /// the window in every mode, and the Problems caption and the diagram's
    /// zoom bar sit above the bar.
    #[test]
    fn the_bar_and_the_badge_fit_the_minimum_window_in_every_mode() {
        let directory = shop();
        let owner = hold(directory.path(), &agent());
        let mut harness = folder_harness(directory.path(), MINIMUM);
        let text = format!("Claude Code via apiforge mcp · since 10:42 · {PAUSED}");
        for (mode, above) in [
            ("Code", &["PROBLEMS"][..]),
            ("Diagram", &["Zoom out", "Zoom in", "Fit the view"][..]),
            ("Split", &["PROBLEMS", "Zoom out", "Fit the view"][..]),
        ] {
            choose(&mut harness, mode);
            assert_within_window(&harness, &[BAR, text.as_str(), "Observing"]);
            let bar = harness.get_by_label(BAR).rect();
            assert!(
                (bar.height() - 36.0).abs() < 0.5,
                "{mode}: the bar is 36 px high, not {}",
                bar.height()
            );
            for label in above {
                assert!(
                    harness.get_by_label(label).rect().bottom() <= bar.top(),
                    "{mode}: {label} sits above the bar"
                );
                assert_disjoint(&harness, &[BAR, label]);
            }
        }
        drop(owner);
    }

    /// The bar and the badge for every holder, beside the approved captures
    /// `docs/qa/2026-10-27/MCP-00/m1-*.png`.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp07_holder_bar() {
        use crate::testing::capture;
        let holders = [
            ("agent", Some(agent())),
            ("window", Some(Holder::window().with_since(at(10, 40)))),
            (
                "apply",
                Some(Holder::command("apply").with_since(at(10, 41))),
            ),
            ("unknown", None),
        ];
        for (name, holder) in holders {
            for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
                let directory = shop();
                let owner = match &holder {
                    Some(holder) => hold(directory.path(), holder),
                    None => {
                        let owner = ProjectStore::open_exclusive(directory.path()).unwrap();
                        fs::remove_file(directory.path().join(".apiforge/local/holder.json"))
                            .unwrap();
                        owner
                    }
                };
                let mut harness = folder_harness(directory.path(), size);
                let modes: &[&str] = if name == "agent" {
                    &["Code", "Diagram", "Split"]
                } else {
                    &["Code"]
                };
                for mode in modes {
                    choose(&mut harness, mode);
                    capture(
                        &mut harness,
                        &format!("mcp07-{name}-{}-{suffix}", mode.to_lowercase()),
                        None,
                    );
                }
                drop(harness);
                drop(owner);
            }
        }
    }
}

// ----- MCP-08a: observer mode — editing paused, every commit followed -------------

mod mcp08a {
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use apiforge_core::format::Position;
    use apiforge_project::{FixedPlacer, Holder, HolderRole};
    use apiforge_storage::project::{DiagramFile, DocumentKey, IdentityBase};
    use chrono::TimeZone;
    use egui_kittest::kittest::NodeT;

    /// The bar's region and its fixed wording (MCP-07).
    const BAR: &str = "Project holder";
    const PAUSED: &str = "editing is paused here; this window follows every change";
    /// The ticket's bound: the window shows a commit this soon after it.
    const FOLLOW: Duration = Duration::from_secs(2);
    /// The holder's edits, as the agent's would be.
    const HOLDER: Origin = Origin(90);

    fn agent() -> Holder {
        let since = chrono::Local
            .with_ymd_and_hms(2026, 10, 27, 10, 42, 11)
            .single()
            .expect("an unambiguous local time")
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        Holder::new(HolderRole::Agent, Some("Claude Code".into())).with_since(since)
    }

    /// The holder: a second session in this process that took the writer
    /// lock first, as the MCP server's does, and writes through it.
    fn holder(root: &std::path::Path) -> ProjectSession {
        let session = ProjectSession::open_with_as(root, Arc::new(FixedPlacer), &agent()).unwrap();
        assert!(!session.is_read_only(), "the holder is the writer");
        session
    }

    fn disabled(harness: &FolderHarness, label: &str) -> bool {
        harness.get_by_label(label).accesskit_node().is_disabled()
    }

    fn read_only(harness: &FolderHarness) -> bool {
        folder(harness).session.as_ref().unwrap().is_read_only()
    }

    fn has_tab(harness: &FolderHarness, document_id: &str) -> bool {
        folder(harness)
            .workbench
            .tabs()
            .iter()
            .any(|tab| tab.document_id == document_id)
    }

    /// Steps the window in real time until `shown` holds, and returns how
    /// long that took after `committed`.
    fn await_shown(
        harness: &mut FolderHarness,
        committed: Instant,
        what: &str,
        shown: impl Fn(&FolderHarness) -> bool,
    ) -> Duration {
        let deadline = committed + Duration::from_secs(10);
        while !shown(harness) {
            assert!(Instant::now() < deadline, "the window never showed {what}");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        // Settled first: the bound covers the state the person sees,
        // analysis included (review of #110).
        settle(harness);
        committed.elapsed()
    }

    fn await_writer(harness: &mut FolderHarness) {
        await_shown(harness, Instant::now(), "the take-back", |harness| {
            !read_only(harness)
        });
    }

    fn model_of(harness: &FolderHarness, document_id: &str) -> Document {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .document(document_id)
            .unwrap()
            .model()
            .cloned()
            .expect("a valid model")
    }

    fn blocks(model: &Document) -> Vec<(String, Position)> {
        graph_of(model, false)
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.position.clone()))
            .collect()
    }

    /// The blocks of a document's record in the checkpoint on disk.
    fn checkpoint_blocks(root: &std::path::Path, document_id: &str) -> Vec<(String, Position)> {
        let file = DiagramFile::from_bytes(&fs::read(root.join(".apiforge/diagram.json")).unwrap())
            .unwrap();
        let record = file
            .records
            .iter()
            .find(|record| {
                record.key
                    == DocumentKey::Working {
                        document_id: document_id.to_owned(),
                    }
            })
            .expect("a save wrote a checkpoint");
        let model = match &record.identity_base {
            IdentityBase::Scenario(scenario) => Document::Scenario(scenario.clone()),
            IdentityBase::Component(component) => Document::Component(component.clone()),
        };
        blocks(&model)
    }

    /// `model` with every block of its main board moved by `(dx, dy)`.
    fn moved(model: &Document, dx: f64, dy: f64) -> Document {
        let mut graph = graph_of(model, false).clone();
        for node in &mut graph.nodes {
            node.position.x += dx;
            node.position.y += dy;
        }
        with_graph(model, false, graph)
    }

    /// The bar's last-change text (*10:43 · promotion.afs saved*), if any.
    fn last_change(harness: &FolderHarness) -> Option<String> {
        harness
            .query_all_by_label_contains(" · ")
            .filter_map(|node| {
                let node = node.accesskit_node();
                node.label().or_else(|| node.value())
            })
            .find(|label| {
                !label.contains(PAUSED)
                    && [" saved", " added", " deleted", " published"]
                        .iter()
                        .any(|end| label.ends_with(end))
            })
    }

    /// Criterion 1: while another process holds the project, the window
    /// observes. The code view takes no keystroke, the board no drop, wire or
    /// move, the inspector and the explorer's commands are disabled; *Run*
    /// still simulates; *Run live requests* is disabled.
    #[test]
    fn observing_pauses_every_edit_and_still_simulates() {
        let directory = shop();
        let _owner = holder(directory.path());
        let mut harness = folder_harness(directory.path(), WIDE);
        assert!(has_label(&harness, "Observing"));
        assert!(has_label(&harness, BAR));
        let scenario = only(&harness, DocumentKind::Scenario);
        let before = source_of(&harness, &scenario);

        let label = editor(&harness, &scenario);
        type_into(&mut harness, &label, "// typed while observing\n");
        assert_eq!(
            source_of(&harness, &scenario),
            before,
            "the code view takes no keystroke"
        );
        assert!(disabled(&harness, "New"), "the explorer cannot create");

        choose(&mut harness, "Diagram");
        let blocks_before = blocks(&model_of(&harness, &scenario));
        assert!(
            disabled(&harness, "HTTP request"),
            "the library offers no block"
        );
        let start = harness.get_by_label("HTTP request").rect().center();
        let board = harness.get_by_label("Diagram board").rect();
        let drop = board.left_bottom() + egui::vec2(80.0, -80.0);
        harness.hover_at(start);
        harness.step();
        harness.drag_at(start);
        harness.step();
        for step in 1..=8 {
            harness.hover_at(start + (drop - start) * (step as f32 / 8.0));
            harness.step();
        }
        harness.drop_at(drop);
        settle(&mut harness);
        assert!(
            disabled(&harness, "Output value of Expected price"),
            "no wire starts from a socket"
        );
        // A card can still be selected, for reading; it cannot be moved.
        let card = harness
            .get_by_label("Expected price (Value)")
            .rect()
            .center();
        harness.hover_at(card);
        harness.step();
        harness.drag_at(card);
        harness.step();
        for step in 1..=8 {
            harness.hover_at(card + egui::vec2(15.0, 10.0) * step as f32);
            harness.step();
        }
        harness.drop_at(card + egui::vec2(120.0, 80.0));
        settle(&mut harness);
        assert_eq!(
            blocks(&model_of(&harness, &scenario)),
            blocks_before,
            "the board accepts no drop, wire or move"
        );
        choose(&mut harness, "Expected price (Value)");
        assert!(
            folder(&harness)
                .selections
                .get(&scenario)
                .is_some_and(|selection| !selection.is_empty()),
            "a block can still be selected"
        );
        assert!(
            harness
                .get_by_role_and_label(egui::accesskit::Role::TextInput, "Block name")
                .accesskit_node()
                .is_disabled(),
            "the inspector is disabled"
        );
        let document = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap();
        assert!(!document.is_dirty() && !document.is_layout_dirty());

        choose(&mut harness, "Run options");
        assert!(
            disabled(&harness, "Run live requests"),
            "no live request is sent from an observing window"
        );
        harness.key_press(Key::Escape);
        settle(&mut harness);
        choose(&mut harness, "Run");
        let report = finish_run(&mut harness);
        assert!(!report.steps.is_empty(), "the simulation still runs");
    }

    /// Criterion 2: the holder saves a changed scenario with its blocks
    /// moved; within two seconds the window shows the text, the blocks with
    /// the identities and positions of the holder's checkpoint, and then the
    /// diagnostics of a later save; the tab, the camera and the selection
    /// stay where they were.
    #[test]
    fn the_holders_save_is_shown_within_two_seconds_where_the_person_was() {
        let directory = shop();
        let mut owner = holder(directory.path());
        let mut harness = folder_harness(directory.path(), WIDE);
        let scenario = only(&harness, DocumentKind::Scenario);
        choose(&mut harness, "Diagram");
        choose(&mut harness, "Expected price (Value)");
        choose(&mut harness, "Zoom in");
        let selection = folder(&harness).selections.get(&scenario).cloned();
        assert!(selection.as_ref().is_some_and(|ids| !ids.is_empty()));
        let camera = folder(&harness).canvases[&scenario].camera();
        assert!(camera.is_some(), "the camera was moved");

        let text = owner
            .document(&scenario)
            .unwrap()
            .text()
            .replace("80 * 1", "80 * 2");
        owner.edit_source(&scenario, &text, HOLDER).unwrap();
        owner.settle(&scenario).unwrap();
        let model = owner.document(&scenario).unwrap().model().cloned().unwrap();
        owner
            .apply_model_edit(&scenario, moved(&model, 240.0, 160.0), HOLDER)
            .unwrap();
        owner.save(&scenario).unwrap();
        let taken = await_shown(&mut harness, Instant::now(), "the new text", |harness| {
            source_of(harness, &scenario) == text
        });
        assert!(taken <= FOLLOW, "shown after {taken:?}");
        assert_eq!(
            blocks(&model_of(&harness, &scenario)),
            checkpoint_blocks(directory.path(), &scenario),
            "the identities and positions of the holder's checkpoint"
        );
        assert_eq!(folder(&harness).workbench.active(), Some(scenario.as_str()));
        assert_eq!(folder(&harness).canvases[&scenario].camera(), camera);
        assert_eq!(
            folder(&harness).selections.get(&scenario).cloned(),
            selection
        );

        let broken = format!("{text}\nbroken = \n");
        owner.edit_source(&scenario, &broken, HOLDER).unwrap();
        owner.save(&scenario).unwrap();
        let taken = await_shown(&mut harness, Instant::now(), "the diagnostics", |harness| {
            source_of(harness, &scenario) == broken
        });
        assert!(taken <= FOLLOW, "shown after {taken:?}");
        choose(&mut harness, "Code");
        assert!(
            harness.query_by_label_contains("Line ").is_some(),
            "Problems lists the new diagnostics"
        );
        assert_eq!(folder(&harness).workbench.active(), Some(scenario.as_str()));
    }

    /// Criterion 3: a document the holder registers is listed within two
    /// seconds, without a *reloaded* notice, and the active tab stays.
    #[test]
    fn a_document_the_holder_registers_is_listed_without_a_notice() {
        let directory = shop();
        let mut owner = holder(directory.path());
        let mut harness = folder_harness(directory.path(), WIDE);
        let active = folder(&harness).workbench.active().map(str::to_owned);
        owner
            .create_document(
                DocumentKind::Scenario,
                "scenarios/added-by-agent.afs",
                "scenario \"Added by agent\" {\n  total = 2\n}\n",
            )
            .unwrap();
        let taken = await_shown(
            &mut harness,
            Instant::now(),
            "the new document",
            // The explorer's row; the bar names the file as well once the
            // commit is followed, so more than one node may match.
            |harness| {
                harness
                    .query_all_by_label_contains("added-by-agent.afs")
                    .next()
                    .is_some()
            },
        );
        assert!(taken <= FOLLOW, "listed after {taken:?}");
        assert!(
            folder(&harness).notice.is_none(),
            "no notice: {:?}",
            folder(&harness)
                .notice
                .as_ref()
                .map(|notice| notice.text.clone())
        );
        assert_eq!(
            folder(&harness).workbench.active().map(str::to_owned),
            active
        );
        assert!(
            last_change(&harness).is_some_and(|text| text.contains("added-by-agent.afs")),
            "the bar says what changed last: {:?}",
            last_change(&harness)
        );
    }

    /// Criterion 4: a revision the holder publishes is shown in the
    /// component's tab within two seconds, and a scenario calling it
    /// analyses without error.
    #[test]
    fn a_revision_the_holder_publishes_is_followed() {
        let directory = shop();
        let mut owner = holder(directory.path());
        let mut harness = folder_harness(directory.path(), WIDE);
        let scenario = only(&harness, DocumentKind::Scenario);
        let component = owner
            .documents()
            .iter()
            .find(|document| {
                document.entry().kind == DocumentKind::Component
                    && document.text().starts_with("component active_promotion")
            })
            .expect("the shop's scenario calls the active_promotion component")
            .id()
            .to_owned();
        app(&mut harness).activate_tab(&component);
        settle(&mut harness);
        let revisions = owner.manifest().revisions.len();

        let text = owner.document(&component).unwrap().text().to_owned();
        let edited = text.replacen('"', "\"Published ", 1);
        owner.edit_source(&component, &edited, HOLDER).unwrap();
        owner.settle(&component).unwrap();
        let published = owner.publish(&component, HOLDER).unwrap();
        let calling = owner.document(&scenario).unwrap().text().replace(
            "active_promotion@1(",
            &format!("active_promotion@{}(", published.revision),
        );
        assert_ne!(calling, owner.document(&scenario).unwrap().text());
        owner.edit_source(&scenario, &calling, HOLDER).unwrap();
        owner.save(&scenario).unwrap();

        let taken = await_shown(&mut harness, Instant::now(), "the revision", |harness| {
            source_of(harness, &scenario) == calling
        });
        assert!(taken <= FOLLOW, "shown after {taken:?}");
        assert!(
            folder(&harness).notice.is_none(),
            "followed without a reload notice: {:?}",
            folder(&harness)
                .notice
                .as_ref()
                .map(|notice| notice.text.clone())
        );
        assert_eq!(source_of(&harness, &component), edited);
        assert_eq!(
            folder(&harness).workbench.active(),
            Some(component.as_str())
        );
        let session = folder(&harness).session.as_ref().unwrap();
        assert_eq!(session.manifest().revisions.len(), revisions + 1);
        assert_eq!(
            session.document(&scenario).unwrap().analysis(),
            AnalysisState::Valid,
            "a scenario calling the new revision analyses without error"
        );
    }

    /// Criterion 7: a tab whose document the holder deleted closes with a
    /// notice; every other tab stays.
    #[test]
    fn a_tab_whose_document_the_holder_deleted_closes_with_a_notice() {
        let directory = shop();
        let mut owner = holder(directory.path());
        let extra = owner
            .create_document(
                DocumentKind::Scenario,
                "scenarios/short-lived.afs",
                "scenario \"Short lived\" {\n  total = 2\n}\n",
            )
            .unwrap();
        let mut harness = folder_harness(directory.path(), WIDE);
        let scenario = only(&harness, DocumentKind::Scenario);
        app(&mut harness).activate_tab(&scenario);
        app(&mut harness).activate_tab(&extra);
        settle(&mut harness);
        let preview = owner
            .preview_delete(&["scenarios/short-lived.afs"])
            .unwrap();
        owner.delete(&preview, HOLDER).unwrap();
        let taken = await_shown(&mut harness, Instant::now(), "the closed tab", |harness| {
            !has_tab(harness, &extra)
        });
        assert!(taken <= FOLLOW, "closed after {taken:?}");
        assert!(has_tab(&harness, &scenario), "every other tab stays");
        let notice = folder(&harness)
            .notice
            .as_ref()
            .map(|notice| notice.text.clone())
            .unwrap_or_default();
        assert!(notice.contains("short-lived.afs"), "notice: {notice:?}");
    }

    /// Criterion 8: after following the holder's commits and taking the
    /// project back, a save in the window succeeds with no conflict on the
    /// checkpoint: its baseline is the one on disk.
    #[test]
    fn after_the_take_back_a_save_meets_no_checkpoint_conflict() {
        let directory = shop();
        let mut owner = holder(directory.path());
        let mut harness = folder_harness(directory.path(), WIDE);
        let scenario = only(&harness, DocumentKind::Scenario);
        let text = owner
            .document(&scenario)
            .unwrap()
            .text()
            .replace("80 * 1", "80 * 3");
        owner.edit_source(&scenario, &text, HOLDER).unwrap();
        owner.settle(&scenario).unwrap();
        let model = owner.document(&scenario).unwrap().model().cloned().unwrap();
        owner
            .apply_model_edit(&scenario, moved(&model, -120.0, 60.0), HOLDER)
            .unwrap();
        owner.save(&scenario).unwrap();
        await_shown(&mut harness, Instant::now(), "the new text", |harness| {
            source_of(harness, &scenario) == text
        });
        drop(owner);
        await_writer(&mut harness);

        let label = editor(&harness, &scenario);
        type_into(&mut harness, &label, "// the window's own change\n");
        settle(&mut harness);
        let model = model_of(&harness, &scenario);
        let moved_model = moved(&model, 30.0, 20.0);
        app(&mut harness)
            .session
            .as_mut()
            .unwrap()
            .apply_model_edit(&scenario, moved_model, DIAGRAM)
            .unwrap();
        settle(&mut harness);
        choose(&mut harness, "Save");
        let session = folder(&harness).session.as_ref().unwrap();
        let document = session.document(&scenario).unwrap();
        assert!(
            !document.is_dirty() && !document.is_layout_dirty(),
            "saved: {:?}",
            folder(&harness)
                .notice
                .as_ref()
                .map(|notice| notice.text.clone())
        );
        assert!(document.conflict().is_none());
        let saved = path_of(&harness, &scenario);
        assert!(
            fs::read_to_string(directory.path().join(saved))
                .unwrap()
                .contains("// the window's own change")
        );
        assert_eq!(
            checkpoint_blocks(directory.path(), &scenario),
            blocks(&model_of(&harness, &scenario)),
            "the checkpoint holds the window's layout"
        );
    }

    /// Criterion 9: at 960 × 540 the badge and the bar with its last-change
    /// text stay inside the window in Code, Diagram and Split, the text
    /// beside the holder and not over it.
    #[test]
    fn the_bar_with_its_last_change_fits_the_minimum_window_in_every_mode() {
        let directory = shop();
        let mut owner = holder(directory.path());
        let mut harness = folder_harness(directory.path(), MINIMUM);
        let scenario = only(&harness, DocumentKind::Scenario);
        let text = format!("{}// saved by the agent\n", source_of(&harness, &scenario));
        owner.edit_source(&scenario, &text, HOLDER).unwrap();
        owner.save(&scenario).unwrap();
        await_shown(&mut harness, Instant::now(), "the commit", |harness| {
            last_change(harness).is_some()
        });
        let change = last_change(&harness).unwrap();
        assert!(
            change.ends_with("promotion-and-cart-price.afs saved"),
            "{change}"
        );
        let holder_text = format!("Claude Code via apiforge mcp · since 10:42 · {PAUSED}");
        for mode in ["Code", "Diagram", "Split"] {
            choose(&mut harness, mode);
            assert_within_window(&harness, &[BAR, change.as_str(), "Observing"]);
            assert_disjoint(&harness, &[holder_text.as_str(), change.as_str()]);
        }

        // Review of #110: a long file name is cut, and both texts keep
        // their place in the bar.
        owner
            .create_document(
                DocumentKind::Scenario,
                "scenarios/a-scenario-whose-name-is-far-too-long-to-fit-beside-the-holder.afs",
                "scenario \"Long\" {\n  total = 2\n}\n",
            )
            .unwrap();
        await_shown(&mut harness, Instant::now(), "the long name", |harness| {
            last_change(harness).is_some_and(|text| text.contains("far-too-long"))
        });
        let change = last_change(&harness).unwrap();
        choose(&mut harness, "Code");
        assert_within_window(&harness, &[BAR, change.as_str(), holder_text.as_str()]);
        assert_disjoint(&harness, &[holder_text.as_str(), change.as_str()]);
        let bar = harness.get_by_label(BAR).rect();
        for label in [change.as_str(), holder_text.as_str()] {
            let rect = harness.get_by_label(label).rect();
            assert!(
                rect.left() >= bar.left() && rect.right() <= bar.right(),
                "{label} stays in the bar"
            );
        }
    }

    /// The carry-over from the review of #106: *Project settings* opens with
    /// every control disabled and *Environment* with its public part
    /// disabled while observing; neither leaves configuration behind that
    /// would stop the take-back.
    #[test]
    fn settings_open_disabled_while_observing_and_the_take_back_still_works() {
        let directory = shop();
        let owner = holder(directory.path());
        let mut harness = folder_harness(directory.path(), WIDE);
        open_project_menu(&mut harness, "Project settings…");
        assert!(has_label(&harness, "Project name"), "the dialog opens");
        assert!(disabled(&harness, "Project name"));
        assert!(disabled(&harness, "Apply settings"));
        choose(&mut harness, "Cancel");
        open_project_menu(&mut harness, "Test environment…");
        assert!(
            disabled(&harness, "Base URL"),
            "the public part is disabled"
        );
        assert!(disabled(&harness, "New variable name"));
        assert!(disabled(&harness, "New secret name"));
        choose(&mut harness, "Cancel");
        assert!(
            !folder(&harness)
                .session
                .as_ref()
                .unwrap()
                .is_manifest_dirty()
        );
        drop(owner);
        await_writer(&mut harness);
        assert!(!has_label(&harness, "Observing"));
    }

    /// The observer, the followed commit and the bar's last change, beside
    /// the approved captures `docs/qa/2026-10-27/MCP-00/m1-observer-*.png`.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp08a_observer() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = shop();
            let mut owner = holder(directory.path());
            let mut harness = folder_harness(directory.path(), size);
            let scenario = only(&harness, DocumentKind::Scenario);
            let text = owner
                .document(&scenario)
                .unwrap()
                .text()
                .replace("80 * 1", "80 * 2");
            owner.edit_source(&scenario, &text, HOLDER).unwrap();
            owner.save(&scenario).unwrap();
            await_shown(&mut harness, Instant::now(), "the commit", |harness| {
                last_change(harness).is_some()
            });
            for mode in ["Code", "Diagram", "Split"] {
                choose(&mut harness, mode);
                capture(
                    &mut harness,
                    &format!("mcp08a-observer-{}-{suffix}", mode.to_lowercase()),
                    None,
                );
            }
            drop(harness);
            drop(owner);
        }
    }
}

// ----- MCP-04: the window beside the server's agent session ------------------

mod mcp04 {
    use super::*;
    use crate::testing::{MINIMUM, shop_project};
    use apiforge_mcp::{
        ErrorCode,
        agent::{AgentSession, CreateDocumentArguments, WriteSourceArguments},
        tools::Kind,
    };
    use apiforge_storage::project::{
        FileState, ProjectError,
        faults::{self, Boundary, Phase},
    };
    use std::path::Path;

    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
    const DISCOUNT: &str = "components/discount.afs";
    const DISCOUNT_TEXT: &str = "component discount \"Discount\" {\n  description \"A tenth off a price.\"\n  @label(\"Price\")\n  input price: number = 100\n  @label(\"Discounted\")\n  export discounted: number = param.price * 0.9\n}\n";
    const UNSAVED: &str = "Changed outside API Forge while unsaved here";

    fn sha256(bytes: &[u8]) -> String {
        match FileState::of(bytes) {
            FileState::File { sha256, .. } => sha256,
            _ => unreachable!(),
        }
    }

    fn commented(text: &str, note: &str) -> String {
        text.replacen(" {\n", &format!(" {{\n  // {note}\n"), 1)
    }

    /// The server's agent session, started by a write of the saved text that
    /// changes nothing: from then on it holds the writer lock.
    fn begin(root: &Path) -> AgentSession {
        let agent = AgentSession::with_environment(root.to_owned(), Vec::new());
        let saved = fs::read(root.join(SCENARIO)).unwrap();
        agent
            .write_source(&write(
                std::str::from_utf8(&saved).unwrap(),
                &sha256(&saved),
            ))
            .unwrap();
        assert!(agent.is_active());
        agent
    }

    fn write(text: &str, expected: &str) -> WriteSourceArguments {
        WriteSourceArguments {
            path: SCENARIO.into(),
            text: text.into(),
            expected_source_sha256: expected.into(),
            expected_generation: None,
        }
    }

    fn scenario(harness: &FolderHarness) -> String {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == SCENARIO)
            .unwrap()
            .id()
            .to_owned()
    }

    /// Criterion 11: a read-only window follows a `write_source` on a clean
    /// document within one scan and shows no conflict.
    #[test]
    fn a_read_only_window_follows_a_write_on_a_clean_document() {
        let project = shop_project();
        let root = project.path();
        let agent = begin(root);
        let mut harness = folder_harness(root, WIDE);
        assert!(folder(&harness).session.as_ref().unwrap().is_read_only());
        let id = scenario(&harness);
        let saved = fs::read(root.join(SCENARIO)).unwrap();
        let written = commented(std::str::from_utf8(&saved).unwrap(), "From the model.");
        agent
            .write_source(&write(&written, &sha256(&saved)))
            .unwrap();

        follow_disk(&mut harness);
        let app = folder(&harness);
        let snapshot = app.snapshot(&id).unwrap();
        assert_eq!(snapshot.text, written, "followed within one scan");
        assert_eq!(snapshot.conflict, None);
        assert!(
            app.notice.as_ref().is_none_or(|notice| !notice.error),
            "no conflict is reported"
        );
        assert!(harness.query_by_label_contains(UNSAVED).is_none());
        drop(harness);
        drop(agent);
    }

    /// Criterion 11: on a document with unsaved work in the window, the
    /// window reports the change outside and keeps the unsaved text.
    #[test]
    fn a_write_on_a_document_unsaved_in_the_window_keeps_the_unsaved_text() {
        let project = shop_project();
        let root = project.path();
        let agent = begin(root);
        let mut harness = folder_harness(root, WIDE);
        let id = scenario(&harness);
        let saved = fs::read(root.join(SCENARIO)).unwrap();
        let saved_text = std::str::from_utf8(&saved).unwrap();
        let local = commented(saved_text, "Typed in the window.");
        app(&mut harness)
            .session
            .as_mut()
            .unwrap()
            .edit_source(&id, &local, CODE)
            .unwrap();
        settle(&mut harness);

        let written = commented(saved_text, "From the model.");
        agent
            .write_source(&write(&written, &sha256(&saved)))
            .unwrap();
        follow_disk(&mut harness);
        assert!(
            harness.query_by_label_contains(UNSAVED).is_some(),
            "the window says the file changed outside while unsaved here"
        );
        let snapshot = folder(&harness).snapshot(&id).unwrap();
        assert_eq!(snapshot.text, local, "the unsaved text stays in place");
        assert_eq!(
            fs::read_to_string(root.join(SCENARIO)).unwrap(),
            written,
            "the disk holds the model's text"
        );
        drop(harness);
        drop(agent);
    }

    /// Criterion 6: a creation that failed after its activation leaves
    /// recoverable intent; once the agent session has ended, the window's
    /// recovery view finishes it.
    #[test]
    fn the_window_finishes_a_creation_that_failed_after_activation() {
        let project = shop_project();
        let root = project.path();
        let agent = AgentSession::with_environment(root.to_owned(), Vec::new());
        let error = faults::with_hook(
            |event| {
                if event.boundary == Boundary::Replace
                    && event.phase == Phase::Before
                    && event.path.ends_with(".apiforge/diagram.json")
                {
                    return Err(ProjectError::Io {
                        path: event.path.clone(),
                        source: std::io::Error::other("injected on the checkpoint's install"),
                    });
                }
                Ok(())
            },
            || {
                agent.create_document(&CreateDocumentArguments {
                    kind: Kind::Component,
                    path: DISCOUNT.into(),
                    text: DISCOUNT_TEXT.into(),
                    expected_generation: None,
                })
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::RecoveryRequired);
        assert!(agent.finish_editing().finished);

        let mut harness = folder_harness(root, MINIMUM);
        assert!(has_label(&harness, "Recover interrupted file operation"));
        choose(&mut harness, "Finish intended operation");
        assert!(has_label(&harness, "Reload project after recovery"));
        choose(&mut harness, "Open project");
        let session = folder(&harness).session.as_ref().unwrap();
        let document = session
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == DISCOUNT)
            .expect("the created component is registered");
        assert_eq!(document.text(), DISCOUNT_TEXT);
        assert_eq!(
            fs::read_to_string(root.join(DISCOUNT)).unwrap(),
            DISCOUNT_TEXT
        );
        let diagram = fs::read_to_string(root.join(".apiforge/diagram.json")).unwrap();
        assert!(
            diagram.contains(document.id()),
            "the checkpoint carries its sealed identities"
        );
    }
}

// ----- MCP-08b: the hand-over on request over the control channel ------------

mod mcp08b {
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use apiforge_mcp::test_support::{StdioServer, structured, tool_error};
    use apiforge_storage::project::{Holder, HolderRole, ProjectStore};
    use egui_kittest::kittest::NodeT;
    use serde_json::{Value, json};
    use std::{
        io::Write as _,
        path::{Path, PathBuf},
        process::Command,
    };

    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
    /// Tells [`server_process`] which folder to serve.
    const FOLDER: &str = "APIFORGE_HANDOVER_TEST_FOLDER";
    const STEP_DELAY: &str = "APIFORGE_MCP_TEST_STEP_DELAY_MS";
    const BAR: &str = "Project holder";
    const PAUSED: &str = "editing is paused here; this window follows every change";
    const REFUSED: &str = "An agent asked to edit this project; save your work first";
    const WAITING: &str = "waiting for the agent's run to finish";
    const TAKE_BACK: &str = "Take back";
    const AGENT: &str = "Claude Code via apiforge mcp";
    /// The ticket's bound: a commit shown, or the project back, this soon.
    const BOUND: Duration = Duration::from_secs(2);
    /// So long that a window following a commit sooner did not wait for a
    /// check: it was told.
    const NO_POLL: Duration = Duration::from_secs(600);

    /// The MCP server as a process of its own: this test executable running
    /// only this test, which serves `apiforge mcp` on its stdio, as the
    /// `apiforge` binary does (the spawned-helper exception of the testing
    /// process). Its harness prints a line of its own on stdout, which the
    /// stdio helper keeps apart from the protocol.
    #[test]
    #[ignore = "launched by the MCP-08b hand-over tests as the server process"]
    fn server_process() {
        let Some(folder) = std::env::var_os(FOLDER) else {
            return;
        };
        let step_delay = std::env::var(STEP_DELAY)
            .ok()
            .and_then(|milliseconds| milliseconds.parse().ok())
            .map(Duration::from_millis);
        let settings = apiforge_mcp::ServerSettings {
            run: apiforge_mcp::tools::RunSettings {
                step_delay,
                ..Default::default()
            },
            ..Default::default()
        };
        apiforge_mcp::serve_stdio_with(PathBuf::from(folder), settings).unwrap();
    }

    fn spawn(root: &Path, envs: &[(&str, &str)]) -> StdioServer {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "folder::tests::acceptance_tests::mcp08b::server_process",
                "--nocapture",
                "--quiet",
                "--test-threads=1",
            ])
            .env(FOLDER, root)
            .envs(envs.iter().copied());
        StdioServer::spawn(command)
    }

    /// A server initialised by the client `client`.
    fn agent(root: &Path, client: &str) -> StdioServer {
        agent_with(root, client, &[])
    }

    fn agent_with(root: &Path, client: &str, envs: &[(&str, &str)]) -> StdioServer {
        let mut server = spawn(root, envs);
        server.initialize(client);
        server
    }

    /// Answers request `id` while the window keeps drawing frames, as the
    /// person's window does while the agent waits.
    fn answer(harness: &mut FolderHarness, server: &mut StdioServer, id: u64) -> Value {
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            if let Some(answer) = server.poll(id) {
                return answer;
            }
            assert!(Instant::now() < deadline, "no answer to request {id}");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn call(
        harness: &mut FolderHarness,
        server: &mut StdioServer,
        tool: &str,
        arguments: Value,
    ) -> Value {
        let id = server.start_call(tool, arguments);
        answer(harness, server, id)
    }

    /// The saved text of `path` and its hash, as the model reads them.
    fn read(server: &mut StdioServer, path: &str) -> (String, String) {
        let read = server.call("read_source", json!({ "path": path }));
        let read = structured(&read);
        (
            read["text"].as_str().unwrap().to_owned(),
            read["source_sha256"].as_str().unwrap().to_owned(),
        )
    }

    fn commented(text: &str, note: &str) -> String {
        text.replacen(" {\n", &format!(" {{\n  // {note}\n"), 1)
    }

    /// The model's `write_source` of `text` over the version it just read.
    fn write(
        harness: &mut FolderHarness,
        server: &mut StdioServer,
        path: &str,
        text: &str,
    ) -> Value {
        let (_, hash) = read(server, path);
        call(
            harness,
            server,
            "write_source",
            json!({ "path": path, "text": text, "expected_source_sha256": hash }),
        )
    }

    /// Starts a slow simulation of the scenario and returns its request once
    /// it holds the server's run slot. The slot is probed with runs of a
    /// document that does not exist: `run_in_progress` while the slot is
    /// held, otherwise at once `unknown_document`, never a second run.
    fn start_run(harness: &mut FolderHarness, server: &mut StdioServer) -> u64 {
        let mut run = server.start_call("run", json!({ "path": SCENARIO }));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let probe = call(harness, server, "run", json!({ "path": "missing.afs" }));
            if probe["result"]["structuredContent"]["code"] == "run_in_progress" {
                return run;
            }
            assert!(
                Instant::now() < deadline,
                "the run never held the slot: {probe}"
            );
            // A probe that came first held the slot for a moment, and the
            // run was refused: start it again.
            if server.poll(run).is_some() {
                run = server.start_call("run", json!({ "path": SCENARIO }));
            }
            harness.step();
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// A write that hands the project to the agent: the window was clean.
    fn hand_over(harness: &mut FolderHarness, server: &mut StdioServer, note: &str) -> String {
        let (text, _) = read(server, SCENARIO);
        let written = commented(&text, note);
        let answer = write(harness, server, SCENARIO, &written);
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        settle(harness);
        written
    }

    /// Steps the window in real time until `done` holds, and returns how
    /// long that took after `since`.
    fn await_until(
        harness: &mut FolderHarness,
        since: Instant,
        what: &str,
        done: impl Fn(&FolderHarness) -> bool,
    ) -> Duration {
        let deadline = since + Duration::from_secs(15);
        while !done(harness) {
            assert!(Instant::now() < deadline, "the window never showed {what}");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        let took = since.elapsed();
        settle(harness);
        took
    }

    fn read_only(harness: &FolderHarness) -> bool {
        folder(harness).session.as_ref().unwrap().is_read_only()
    }

    fn disabled(harness: &FolderHarness, label: &str) -> bool {
        harness.get_by_label(label).accesskit_node().is_disabled()
    }

    fn contains(harness: &FolderHarness, text: &str) -> bool {
        harness.query_by_label_contains(text).is_some()
    }

    /// The full label of the node whose label contains `text`.
    fn label_containing(harness: &FolderHarness, text: &str) -> String {
        let node = harness.get_by_label_contains(text);
        let node = node.accesskit_node();
        node.label()
            .or_else(|| node.value())
            .expect("a labelled node")
    }

    /// Who holds the project, as a reader finds out.
    fn holder(root: &Path) -> Option<Holder> {
        ProjectStore::open_read_only(root)
            .unwrap()
            .holder()
            .unwrap()
    }

    /// Steps the window until the holder file names an endpoint with the
    /// role `role`, and returns it.
    fn await_endpoint(harness: &mut FolderHarness, root: &Path, role: HolderRole) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(holder) = holder(root)
                && holder.role() == Some(role)
                && let Some(endpoint) = holder.endpoint()
            {
                return endpoint.to_owned();
            }
            assert!(
                Instant::now() < deadline,
                "the holder never named an endpoint"
            );
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Where a same-user client finds `endpoint`: a socket in a private
    /// directory under the runtime directory (the temporary directory, then
    /// `/tmp`, when it would make a socket path over 100 bytes), or a named
    /// pipe.
    #[cfg(unix)]
    fn address(endpoint: &str) -> PathBuf {
        let socket = |directory: PathBuf| directory.join(endpoint).join("socket");
        std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|directory| directory.is_absolute())
            .into_iter()
            .chain([std::env::temp_dir()])
            .map(socket)
            .find(|path| path.as_os_str().len() <= 100)
            .unwrap_or_else(|| socket(PathBuf::from("/tmp")))
    }

    #[cfg(windows)]
    fn address(endpoint: &str) -> PathBuf {
        PathBuf::from(format!(r"\\.\pipe\{endpoint}"))
    }

    /// A raw connection to `endpoint`, as a client of the channel opens it.
    #[cfg(unix)]
    fn connect(endpoint: &str) -> std::io::Result<impl std::io::Write> {
        std::os::unix::net::UnixStream::connect(address(endpoint))
    }

    #[cfg(windows)]
    fn connect(endpoint: &str) -> std::io::Result<impl std::io::Write> {
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(address(endpoint))
    }

    /// A random name and nothing else: no path, no separator.
    fn assert_private_name(endpoint: &str) {
        let random = endpoint
            .strip_prefix("apiforge-")
            .unwrap_or_else(|| panic!("{endpoint:?} is not apiforge-<random>"));
        assert_eq!(random.len(), 32, "{endpoint:?}");
        assert!(
            random
                .chars()
                .all(|character| character.is_ascii_hexdigit() && !character.is_uppercase()),
            "{endpoint:?}"
        );
    }

    /// Criterion 1: the window that holds the project listens and names its
    /// endpoint in the holder file; so does the server during an agent
    /// session; a reader finds each through `ProjectStore::holder()`.
    #[test]
    fn the_holder_listens_and_a_reader_finds_its_endpoint() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        assert!(!read_only(&harness));
        let window = await_endpoint(&mut harness, root, HolderRole::Window);
        assert_private_name(&window);
        assert!(connect(&window).is_ok(), "the window accepts connections");
        drop(harness);

        let mut server = agent(root, "Claude Code");
        let (text, hash) = read(&mut server, SCENARIO);
        let written = server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": commented(&text, "Mine."), "expected_source_sha256": hash }),
        );
        assert_eq!(structured(&written)["wrote"], true);
        let described = holder(root).expect("the server holds the project");
        assert_eq!(described.role(), Some(HolderRole::Agent));
        assert_eq!(described.client(), Some("Claude Code"));
        let endpoint = described.endpoint().expect("the server names its endpoint");
        assert_private_name(endpoint);
        assert_ne!(endpoint, window);
        assert!(connect(endpoint).is_ok(), "the server accepts connections");
        let ended = server.close();
        assert_eq!(ended.status.code(), Some(0), "{}", ended.stderr);
    }

    /// Criterion 2: with the window open and clean, the server's first
    /// `write_source` asks over the channel; the window releases the lock and
    /// observes, naming the agent; the write proceeds; and the window shows
    /// the change within two seconds because the server said `saved`, not
    /// because a check came round: checks here are ten minutes apart.
    #[test]
    fn a_clean_window_hands_over_and_shows_the_save_it_was_told_of() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = only(&harness, DocumentKind::Scenario);
        app(&mut harness).watcher.set_interval(NO_POLL);

        let mut server = agent(root, "Claude Code");
        let (text, hash) = read(&mut server, SCENARIO);
        let written = commented(&text, "Written by the agent.");
        let answer = call(
            &mut harness,
            &mut server,
            "write_source",
            json!({ "path": SCENARIO, "text": written, "expected_source_sha256": hash }),
        );
        let returned = Instant::now();
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        assert_eq!(fs::read_to_string(root.join(SCENARIO)).unwrap(), written);
        let took = await_until(&mut harness, returned, "the agent's save", |harness| {
            source_of(harness, &scenario) == written
        });
        assert!(took < BOUND, "shown {took:?} after the write returned");
        assert!(read_only(&harness), "the window let the project go");
        assert!(has_label(&harness, "Observing"));
        assert!(
            contains(&harness, &format!("{AGENT} · ")),
            "the bar names the agent"
        );
        assert!(contains(&harness, PAUSED));
        assert!(has_label(&harness, TAKE_BACK));
        let described = holder(root).expect("the server holds the project");
        assert_eq!(described.role(), Some(HolderRole::Agent));
        drop(harness);
        drop(server);
    }

    /// Criterion 3: with unsaved work the window refuses; the write is
    /// `handover_refused` with the reason; the window says so in the bar;
    /// nothing is written and the person's text is untouched.
    #[test]
    fn unsaved_work_refuses_the_hand_over_and_keeps_the_text() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = only(&harness, DocumentKind::Scenario);
        let label = editor(&harness, &scenario);
        type_into(&mut harness, &label, "// typed by the person\n");
        let typed = source_of(&harness, &scenario);
        assert!(typed.contains("// typed by the person"));
        let saved = fs::read_to_string(root.join(SCENARIO)).unwrap();

        let mut server = agent(root, "Claude Code");
        let answer = write(
            &mut harness,
            &mut server,
            SCENARIO,
            &commented(&saved, "Written by the agent."),
        );
        let error = tool_error(&answer);
        assert_eq!(error["code"], "handover_refused", "{answer}");
        assert_eq!(error["wrote"], false);
        assert_eq!(error["details"]["reason"], "the window has unsaved work");
        settle(&mut harness);
        assert!(contains(&harness, REFUSED), "the window says why");
        assert!(!read_only(&harness), "the window keeps the project");
        assert_eq!(fs::read_to_string(root.join(SCENARIO)).unwrap(), saved);
        assert_eq!(
            source_of(&harness, &scenario),
            typed,
            "the text is untouched"
        );
        assert_eq!(
            holder(root).and_then(|holder| holder.role()),
            Some(HolderRole::Window)
        );
        drop(harness);
        drop(server);
    }

    /// Criterion 4: a window opened while the agent holds the project reads
    /// the holder, connects, and observes from the start; the agent's later
    /// commit appears within two seconds, told over the channel.
    #[test]
    fn a_window_opened_while_the_agent_holds_connects_and_follows() {
        let directory = shop();
        let root = directory.path();
        let mut server = agent(root, "Claude Code");
        let (text, hash) = read(&mut server, SCENARIO);
        let first = commented(&text, "First.");
        let written = server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": first, "expected_source_sha256": hash }),
        );
        assert_eq!(structured(&written)["wrote"], true);

        let mut harness = folder_harness(root, WIDE);
        assert!(read_only(&harness));
        assert!(has_label(&harness, "Observing"));
        assert!(contains(&harness, &format!("{AGENT} · ")));
        await_until(&mut harness, Instant::now(), "the connection", |harness| {
            has_label(harness, TAKE_BACK)
        });
        let scenario = only(&harness, DocumentKind::Scenario);
        app(&mut harness).watcher.set_interval(NO_POLL);

        let second = commented(&first, "Second.");
        let answer = write(&mut harness, &mut server, SCENARIO, &second);
        let returned = Instant::now();
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        let took = await_until(&mut harness, returned, "the second save", |harness| {
            source_of(harness, &scenario) == second
        });
        assert!(took < BOUND, "shown {took:?} after the write returned");
        drop(harness);
        drop(server);
    }

    /// Criterion 5: *Take back* while the agent is idle; the window is the
    /// writer within two seconds; the agent's next write asks again and the
    /// clean window grants it again.
    #[test]
    fn take_back_while_the_agent_is_idle_and_the_next_write_asks_again() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        let written = hand_over(&mut harness, &mut server, "Written by the agent.");
        assert!(read_only(&harness));
        assert!(!disabled(&harness, TAKE_BACK));

        choose(&mut harness, TAKE_BACK);
        let pressed = Instant::now();
        let took = await_until(&mut harness, pressed, "the take-back", |harness| {
            !read_only(harness)
        });
        assert!(took < BOUND, "writable {took:?} after Take back");
        assert!(!has_label(&harness, "Observing"));
        assert!(!has_label(&harness, TAKE_BACK));
        assert!(!contains(&harness, PAUSED));
        assert_eq!(
            holder(root).and_then(|holder| holder.role()),
            Some(HolderRole::Window)
        );
        assert_eq!(fs::read_to_string(root.join(SCENARIO)).unwrap(), written);

        let again = commented(&written, "Written again.");
        let answer = write(&mut harness, &mut server, SCENARIO, &again);
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        settle(&mut harness);
        assert!(read_only(&harness), "a clean window hands over again");
        assert!(has_label(&harness, TAKE_BACK));
        assert_eq!(fs::read_to_string(root.join(SCENARIO)).unwrap(), again);
        drop(harness);
        drop(server);
    }

    /// Criterion 6: *Take back* during the agent's run: the run finishes,
    /// cleanup included, while the window says it is waiting for it; then
    /// the window takes the project back.
    #[test]
    fn take_back_waits_for_the_agents_run_and_its_cleanup() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent_with(root, "Claude Code", &[(STEP_DELAY, "300")]);
        hand_over(&mut harness, &mut server, "Written by the agent.");
        let run = start_run(&mut harness, &mut server);

        choose(&mut harness, TAKE_BACK);
        await_until(&mut harness, Instant::now(), "the wait", |harness| {
            contains(harness, WAITING)
        });
        assert!(read_only(&harness), "not before the run has finished");
        assert!(disabled(&harness, TAKE_BACK));

        let deadline = Instant::now() + Duration::from_secs(45);
        let report = loop {
            if let Some(report) = server.poll(run) {
                break report;
            }
            assert!(Instant::now() < deadline, "the run never finished");
            assert!(read_only(&harness), "the window waits for the run");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        };
        let finished = Instant::now();
        let report = structured(&report);
        assert_eq!(report["ended_by"], "completed", "{report}");
        assert!(
            report["steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|step| step["phase"] == "cleanup"),
            "the cleanup ran: {report}"
        );
        let took = await_until(&mut harness, finished, "the take-back", |harness| {
            !read_only(harness)
        });
        assert!(took < BOUND, "writable {took:?} after the run");
        assert!(!contains(&harness, WAITING));
        drop(harness);
        drop(server);
    }

    /// Criterion 6: *Take back* while a save is in flight: the save commits
    /// or fails whole before the release, and `wrote` says which.
    #[test]
    fn take_back_during_a_write_lets_it_commit_or_fail_whole() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        let first = hand_over(&mut harness, &mut server, "First.");
        let (_, hash) = read(&mut server, SCENARIO);
        let second = commented(&first, "Second.");
        let write = server.start_call(
            "write_source",
            json!({ "path": SCENARIO, "text": second, "expected_source_sha256": hash }),
        );
        harness.get_by_label(TAKE_BACK).click();
        let answer = answer(&mut harness, &mut server, write);
        let on_disk = fs::read_to_string(root.join(SCENARIO)).unwrap();
        match answer["result"]["structuredContent"]["wrote"].as_bool() {
            Some(true) => assert_eq!(on_disk, second),
            Some(false) => assert_eq!(on_disk, first),
            None => panic!("wrote is true or false, never half: {answer}"),
        }
        await_until(
            &mut harness,
            Instant::now(),
            "a settled holder",
            // Taken back, or handed over again to a write that came after
            // the release: Take back is enabled only in a new hand-over.
            |harness| {
                !read_only(harness)
                    || (has_label(harness, TAKE_BACK) && !disabled(harness, TAKE_BACK))
            },
        );
        drop(harness);
        drop(server);
    }

    /// Criterion 7: `finish_editing`, then EOF: each time the window is the
    /// writer within two seconds, and after EOF the server process is gone.
    #[test]
    fn finishing_or_disconnecting_gives_the_project_back_within_two_seconds() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        let written = hand_over(&mut harness, &mut server, "First.");
        let finished = call(&mut harness, &mut server, "finish_editing", json!({}));
        let answered = Instant::now();
        assert_eq!(structured(&finished)["finished"], true);
        let took = await_until(
            &mut harness,
            answered,
            "the end of the session",
            |harness| !read_only(harness),
        );
        assert!(took < BOUND, "writable {took:?} after finish_editing");

        let answer = write(
            &mut harness,
            &mut server,
            SCENARIO,
            &commented(&written, "Second."),
        );
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        settle(&mut harness);
        assert!(read_only(&harness));
        let closed = Instant::now();
        let ended = server.close();
        assert_eq!(ended.status.code(), Some(0), "{}", ended.stderr);
        let took = await_until(&mut harness, closed, "the end of stdin", |harness| {
            !read_only(harness)
        });
        assert!(took < BOUND, "writable {took:?} after EOF");
        assert!(!has_label(&harness, TAKE_BACK));
    }

    /// Criterion 8: the server is killed with an activated transaction; the
    /// window takes the project back and opens the recovery view (MCP-07
    /// criterion 5).
    #[test]
    fn a_killed_server_leaves_its_transaction_to_the_recovery_view() {
        use super::super::recovery_tests::{INTENDED, assert_active, interrupt, source_project};
        let (directory, _) = source_project();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        let (text, _) = read(&mut server, "main.afs");
        // The same text: the session takes the project and writes nothing.
        let answer = write(&mut harness, &mut server, "main.afs", &text);
        assert!(structured(&answer)["wrote"].is_boolean(), "{answer}");
        settle(&mut harness);
        assert!(read_only(&harness));
        // The server's commit was activated when it died.
        interrupt(root);
        let ended = server.kill();
        assert!(!ended.status.success());
        await_until(
            &mut harness,
            Instant::now(),
            "the recovery view",
            |harness| has_label(harness, "Recover interrupted file operation"),
        );
        choose(&mut harness, "Finish intended operation");
        assert_active(root, false);
        assert_eq!(fs::read_to_string(root.join("main.afs")).unwrap(), INTENDED);
    }

    /// Criterion 9: a second agent's write is refused with the first's
    /// description, and the window still observes the first.
    #[test]
    fn a_second_agent_is_refused_with_the_first_ones_description() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut first = agent(root, "Claude Code");
        let written = hand_over(&mut harness, &mut first, "First.");
        let mut second = agent(root, "Other agent");
        let answer = write(
            &mut harness,
            &mut second,
            SCENARIO,
            &commented(&written, "Second agent."),
        );
        let error = tool_error(&answer);
        assert_eq!(error["code"], "held_by_other", "{answer}");
        assert_eq!(error["wrote"], false);
        assert!(
            error["message"].as_str().unwrap().contains(AGENT),
            "{answer}"
        );
        assert_eq!(error["details"]["holder"]["client"], "Claude Code");
        assert_eq!(fs::read_to_string(root.join(SCENARIO)).unwrap(), written);
        settle(&mut harness);
        assert!(read_only(&harness));
        assert!(contains(&harness, &format!("{AGENT} · ")));
        assert!(!contains(&harness, "Other agent"));
        drop(harness);
        drop(second);
        drop(first);
    }

    /// Criterion 10: the window closes while observing; the server keeps the
    /// lock, its next write succeeds, and it says the channel went on stderr
    /// only.
    #[test]
    fn closing_the_observing_window_leaves_the_agent_writing() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        let written = hand_over(&mut harness, &mut server, "First.");
        drop(harness);

        let (_, hash) = read(&mut server, SCENARIO);
        let second = commented(&written, "Second.");
        let answer = server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": second, "expected_source_sha256": hash }),
        );
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        assert_eq!(
            holder(root).and_then(|holder| holder.role()),
            Some(HolderRole::Agent),
            "the server keeps the lock"
        );
        let ended = server.close();
        assert_eq!(ended.status.code(), Some(0), "{}", ended.stderr);
        assert!(
            ended.stderr.contains("apiforge mcp: channel:"),
            "the loss is logged: {}",
            ended.stderr
        );
        for message in &ended.stdout {
            assert_eq!(message["jsonrpc"], "2.0", "{message}");
        }
        assert!(
            ended.noise.iter().all(|line| !line.contains("channel")),
            "nothing about the channel on stdout: {:?}",
            ended.noise
        );
    }

    /// Criterion 11: a pause of more than ten poll intervals between two
    /// writes changes nothing: the window still observes and the second
    /// write succeeds. No take-back by time.
    #[test]
    fn a_long_pause_between_writes_changes_nothing() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        let written = hand_over(&mut harness, &mut server, "First.");
        // Checks every 100 ms during the pause: a window waiting for that
        // often never settles, so the pause only steps it.
        let interval = Duration::from_millis(100);
        app(&mut harness).watcher.set_interval(interval);
        let paused = Instant::now();
        while paused.elapsed() < interval * 15 {
            harness.step();
            assert!(read_only(&harness), "never taken back by time");
            std::thread::sleep(Duration::from_millis(10));
        }
        app(&mut harness)
            .watcher
            .set_interval(crate::folder::watcher::POLL_INTERVAL);
        assert!(has_label(&harness, "Observing"));
        let answer = write(
            &mut harness,
            &mut server,
            SCENARIO,
            &commented(&written, "After the pause."),
        );
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        assert!(read_only(&harness));
        drop(harness);
        drop(server);
    }

    /// Criterion 12: the channel is same-user and carries no path: a random
    /// name under the runtime directory (or a named pipe); the holder file
    /// holds the name only; a malformed or unknown message is ignored and
    /// logged, and both ends keep working.
    #[test]
    fn the_channel_is_private_and_ignores_what_it_cannot_read() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        let window = await_endpoint(&mut harness, root, HolderRole::Window);
        assert_private_name(&window);
        let line = fs::read_to_string(root.join(".apiforge/local/holder.json")).unwrap();
        assert!(
            !line.contains('/') && !line.contains('\\'),
            "no path in the holder file: {line}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::{FileTypeExt, PermissionsExt};
            let socket = address(&window);
            let metadata = fs::metadata(&socket).unwrap();
            assert!(metadata.file_type().is_socket(), "{socket:?}");
            assert_eq!(metadata.permissions().mode() & 0o077, 0, "{socket:?}");
            let parent = fs::metadata(socket.parent().unwrap()).unwrap();
            assert_eq!(parent.permissions().mode() & 0o777, 0o700);
        }
        let garbage: &[u8] = b"not json\n{\"version\":9,\"type\":\"request\"}\n{\"version\":1,\"type\":\"bogus\"}\n{\"version\":1,\"type\":\"take_back\",\"extra\":\n";
        {
            let mut stream = connect(&window).unwrap();
            stream.write_all(garbage).unwrap();
        }
        for _ in 0..20 {
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(&mut harness);
        assert!(!read_only(&harness), "the window ignored it");

        let mut server = agent(root, "Claude Code");
        hand_over(&mut harness, &mut server, "Still handed over.");
        let endpoint = holder(root).unwrap().endpoint().unwrap().to_owned();
        assert_private_name(&endpoint);
        {
            let mut stream = connect(&endpoint).unwrap();
            stream.write_all(garbage).unwrap();
        }
        let overview = server.call("project_overview", json!({}));
        assert_eq!(structured(&overview)["holder"], "agent_session");
        assert!(read_only(&harness));
        drop(harness);
        let ended = server.close();
        assert_eq!(ended.status.code(), Some(0), "{}", ended.stderr);
        assert!(
            ended.stderr.contains("ignored"),
            "the server logs what it ignored: {}",
            ended.stderr
        );
    }

    /// The bar while the agent holds the project (mock-up M1, round 4):
    /// *Take back* at its right end, its right edge on *Run*'s, inside the
    /// window and clear of the texts, in every mode at both sizes.
    #[test]
    fn take_back_sits_at_the_right_end_of_the_bar_under_run() {
        for size in [MINIMUM, WIDE] {
            let directory = shop();
            let root = directory.path();
            let mut harness = folder_harness(root, size);
            await_endpoint(&mut harness, root, HolderRole::Window);
            let mut server = agent(root, "Claude Code");
            hand_over(&mut harness, &mut server, "Written by the agent.");
            let holder_text = label_containing(&harness, &format!("{AGENT} · "));
            for mode in ["Code", "Diagram", "Split"] {
                choose(&mut harness, mode);
                let take = harness.get_by_label(TAKE_BACK).rect();
                let run = harness.get_by_label("Run").rect();
                assert!(
                    (take.right() - run.right()).abs() <= 1.0,
                    "{mode} at {size:?}: Take back ends at {}, Run at {}",
                    take.right(),
                    run.right()
                );
                let bar = harness.get_by_label(BAR).rect();
                assert!(bar.contains_rect(take), "{mode} at {size:?}");
                assert_within_window(&harness, &[BAR, TAKE_BACK, "Observing"]);
                assert_disjoint(&harness, &[holder_text.as_str(), TAKE_BACK]);
            }
            drop(harness);
            drop(server);
        }
    }

    /// The hand-over states beside M1 round 4: observing with *Take back*,
    /// the wait for a run, taken back, the refusal with unsaved work.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp08b_hand_over() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = shop();
            let root = directory.path();
            let mut harness = folder_harness(root, size);
            await_endpoint(&mut harness, root, HolderRole::Window);
            let mut server = agent_with(root, "Claude Code", &[(STEP_DELAY, "300")]);
            hand_over(&mut harness, &mut server, "Written by the agent.");
            for mode in ["Code", "Diagram", "Split"] {
                choose(&mut harness, mode);
                capture(
                    &mut harness,
                    &format!("mcp08b-observing-{}-{suffix}", mode.to_lowercase()),
                    None,
                );
            }
            choose(&mut harness, "Code");
            let run = start_run(&mut harness, &mut server);
            choose(&mut harness, TAKE_BACK);
            await_until(&mut harness, Instant::now(), "the wait", |harness| {
                contains(harness, WAITING)
            });
            capture(&mut harness, &format!("mcp08b-waiting-{suffix}"), None);
            answer(&mut harness, &mut server, run);
            await_until(&mut harness, Instant::now(), "the take-back", |harness| {
                !read_only(harness)
            });
            capture(&mut harness, &format!("mcp08b-taken-back-{suffix}"), None);
            let scenario = only(&harness, DocumentKind::Scenario);
            let label = editor(&harness, &scenario);
            type_into(&mut harness, &label, "// typed by the person\n");
            let (text, _) = read(&mut server, SCENARIO);
            write(
                &mut harness,
                &mut server,
                SCENARIO,
                &commented(&text, "Refused."),
            );
            settle(&mut harness);
            capture(&mut harness, &format!("mcp08b-refused-{suffix}"), None);
            drop(harness);
            drop(server);
        }
    }
}
