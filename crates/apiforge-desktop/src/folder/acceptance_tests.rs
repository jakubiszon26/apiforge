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
