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

/// The width `text` takes at `size` when it is not cut. Laid out in place,
/// its glyphs snap to pixels, so a whole label may be up to a pixel
/// narrower; a cut one loses at least a character.
fn whole_width(harness: &FolderHarness, text: &str, size: f32) -> f32 {
    let job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat::simple(egui::FontId::proportional(size), egui::Color32::WHITE),
    );
    harness
        .ctx
        .fonts_mut(|fonts| fonts.layout_job(job))
        .size()
        .x
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
    choose(&mut harness, "Run simulation");
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
    choose(&mut harness, "Run simulation");
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
    /// move, the inspector and the explorer's commands are disabled; *Run
    /// simulation* still simulates; *Live* (UX-02; before it, *Run live
    /// requests*) is disabled.
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
            disabled(&harness, "Live"),
            "no live request is sent from an observing window"
        );
        harness.key_press(Key::Escape);
        settle(&mut harness);
        choose(&mut harness, "Run simulation");
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
        // Following unpinned: the place kept is MCP-08a's (MCP-16 criterion 7).
        choose(&mut harness, "Follow the agent");
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
        // Following unpinned: the place kept is MCP-08a's (MCP-16 criterion 7).
        choose(&mut harness, "Follow the agent");
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
            // The explorer's row, named exactly by the file: the bar's
            // last change names the file too, inside a longer text.
            |harness| harness.query_by_label("added-by-agent.afs").is_some(),
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
        // Following unpinned: the place kept is MCP-08a's (MCP-16 criterion 7).
        choose(&mut harness, "Follow the agent");
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
    /// the project's own controls disabled (since MCP-09 the agent's toggle
    /// and *Cancel* stay enabled; since MCP-26 *Apply settings* is shown,
    /// disabled until the toggle changes) and
    /// *Environment* with its public part disabled while observing; neither leaves configuration behind that
    /// would stop the take-back.
    #[test]
    fn settings_open_disabled_while_observing_and_the_take_back_still_works() {
        let directory = shop();
        let owner = holder(directory.path());
        let mut harness = folder_harness(directory.path(), WIDE);
        open_project_menu(&mut harness, "Project settings…");
        assert!(has_label(&harness, "Project name"), "the dialog opens");
        assert!(disabled(&harness, "Project name"));
        // Hidden while observing in MCP-09 (owner decision on M3); shown
        // since MCP-26, for the toggle only (owner decision 4 = A of
        // 2026-11-02).
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
    use apiforge_mcp::test_support::{StdioServer, refusal, structured, tool_error};
    use apiforge_storage::project::{Holder, HolderRole, ProjectStore};
    use egui_kittest::kittest::NodeT;
    use serde_json::{Value, json};
    use std::{
        io::Write as _,
        path::{Path, PathBuf},
        process::Command,
    };

    pub(super) const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
    pub(super) const PRODUCT: &str = "components/product.afs";
    /// Tells [`server_process`] which folder to serve.
    const FOLDER: &str = "APIFORGE_HANDOVER_TEST_FOLDER";
    const STEP_DELAY: &str = "APIFORGE_MCP_TEST_STEP_DELAY_MS";
    const BAR: &str = "Project holder";
    const PAUSED: &str = "editing is paused here; this window follows every change";
    pub(super) const REFUSED: &str = "An agent asked to edit this project; save your work first";
    const WAITING: &str = "waiting for the agent's run to finish";
    const TAKE_BACK: &str = "Take back";
    pub(super) const AGENT: &str = "Claude Code via apiforge mcp";
    /// The ticket's bound: a commit shown, or the project back, this soon.
    const BOUND: Duration = Duration::from_secs(2);
    /// So long that a window following a commit sooner did not wait for a
    /// check: it was told.
    pub(super) const NO_POLL: Duration = Duration::from_secs(600);

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
            allow_live: std::env::var("APIFORGE_MCP12_ALLOW_LIVE").as_deref() == Ok("1"),
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
    pub(super) fn agent(root: &Path, client: &str) -> StdioServer {
        agent_with(root, client, &[])
    }

    pub(super) fn agent_with(root: &Path, client: &str, envs: &[(&str, &str)]) -> StdioServer {
        let mut server = spawn(root, envs);
        server.initialize(client);
        server
    }

    /// Answers request `id` while the window keeps drawing frames, as the
    /// person's window does while the agent waits.
    pub(super) fn answer(harness: &mut FolderHarness, server: &mut StdioServer, id: u64) -> Value {
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

    pub(super) fn call(
        harness: &mut FolderHarness,
        server: &mut StdioServer,
        tool: &str,
        arguments: Value,
    ) -> Value {
        let id = server.start_call(tool, arguments);
        answer(harness, server, id)
    }

    /// The saved text of `path` and its hash, as the model reads them.
    pub(super) fn read(server: &mut StdioServer, path: &str) -> (String, String) {
        let read = server.call("read_source", json!({ "path": path }));
        let read = structured(&read);
        (
            read["text"].as_str().unwrap().to_owned(),
            read["source_sha256"].as_str().unwrap().to_owned(),
        )
    }

    pub(super) fn commented(text: &str, note: &str) -> String {
        text.replacen(" {\n", &format!(" {{\n  // {note}\n"), 1)
    }

    /// The model's `write_source` of `text` over the version it just read.
    pub(super) fn write(
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
            if refusal(&probe).is_some_and(|error| error["code"] == "run_in_progress") {
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
    pub(super) fn hand_over(
        harness: &mut FolderHarness,
        server: &mut StdioServer,
        note: &str,
    ) -> String {
        let (text, _) = read(server, SCENARIO);
        let written = commented(&text, note);
        let answer = write(harness, server, SCENARIO, &written);
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        settle(harness);
        written
    }

    /// Steps the window in real time until `done` holds, and returns how
    /// long that took after `since`.
    pub(super) fn await_until(
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

    pub(super) fn read_only(harness: &FolderHarness) -> bool {
        folder(harness).session.as_ref().unwrap().is_read_only()
    }

    fn disabled(harness: &FolderHarness, label: &str) -> bool {
        harness.get_by_label(label).accesskit_node().is_disabled()
    }

    pub(super) fn contains(harness: &FolderHarness, text: &str) -> bool {
        harness.query_by_label_contains(text).is_some()
    }

    /// The full label of the node whose label contains `text`.
    pub(super) fn label_containing(harness: &FolderHarness, text: &str) -> String {
        let node = harness.get_by_label_contains(text);
        let node = node.accesskit_node();
        node.label()
            .or_else(|| node.value())
            .expect("a labelled node")
    }

    /// Who holds the project, as a reader finds out.
    pub(super) fn holder(root: &Path) -> Option<Holder> {
        ProjectStore::open_read_only(root)
            .unwrap()
            .holder()
            .unwrap()
    }

    /// Steps the window until the holder file names an endpoint with the
    /// role `role`, and returns it.
    pub(super) fn await_endpoint(
        harness: &mut FolderHarness,
        root: &Path,
        role: HolderRole,
    ) -> String {
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
    pub(super) fn address(endpoint: &str) -> PathBuf {
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
    pub(super) fn address(endpoint: &str) -> PathBuf {
        PathBuf::from(format!(r"\\.\pipe\{endpoint}"))
    }

    /// A raw connection to `endpoint`, as a client of the channel opens it.
    #[cfg(unix)]
    pub(super) fn connect(endpoint: &str) -> std::io::Result<impl std::io::Write> {
        std::os::unix::net::UnixStream::connect(address(endpoint))
    }

    #[cfg(windows)]
    pub(super) fn connect(endpoint: &str) -> std::io::Result<impl std::io::Write> {
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

    // MCP-12 isolates complete processes instead of changing global environment.
    #[test]
    fn mcp12_different_environments_handover_and_observation() {
        let temp = tempfile::tempdir().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "folder::tests::acceptance_tests::mcp08b::mcp12_window_process",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("APIFORGE_MCP12_TEMP", temp.path());
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            let runtime = PathBuf::from(format!(
                "/run/user/{}",
                fs::metadata(temp.path()).unwrap().uid()
            ));
            assert!(
                runtime.is_dir(),
                "MCP-12 requires provisioning {} before tests",
                runtime.display()
            );
            command.env("XDG_RUNTIME_DIR", runtime);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[ignore = "launched by MCP-12 with an isolated window environment"]
    fn mcp12_window_process() {
        let Some(temp) = std::env::var_os("APIFORGE_MCP12_TEMP").map(PathBuf::from) else {
            return;
        };
        // First write from a server whose runtime and temporary base differ.
        for eof in [false, true] {
            let directory = shop();
            let root = directory.path();
            let mut harness = folder_harness(root, WIDE);
            let endpoint = await_endpoint(&mut harness, root, HolderRole::Window);
            #[cfg(target_os = "linux")]
            assert!(address(&endpoint).starts_with("/run/user"));
            #[cfg(not(target_os = "linux"))]
            let _ = endpoint;
            let mut server = mcp12_agent(root, &temp, false);
            let overview = server.call("project_overview", json!({}));
            assert_eq!(structured(&overview)["holder"], "window");
            assert_eq!(structured(&overview)["hands_over_on_write"], true);
            let created = call(
                &mut harness,
                &mut server,
                "create_document",
                json!({
                    "kind": "scenario", "path": "scenarios/mcp12.afs", "text": "scenario \"MCP-12\" {\n}\n"
                }),
            );
            assert_eq!(structured(&created)["wrote"], true, "{created}");
            await_until(&mut harness, Instant::now(), "the new saved source", |h| {
                folder(h)
                    .session
                    .as_ref()
                    .unwrap()
                    .documents()
                    .iter()
                    .any(|d| d.entry().path.as_str() == "scenarios/mcp12.afs")
            });
            assert!(read_only(&harness));
            if eof {
                let ended = server.close();
                assert!(ended.status.success(), "{}", ended.stderr);
            } else {
                let finished = call(&mut harness, &mut server, "finish_editing", json!({}));
                assert_eq!(structured(&finished)["finished"], true);
            }
            await_until(&mut harness, Instant::now(), "window ownership", |h| {
                !read_only(h)
            });
        }
        // Reverse direction: server listens in /tmp, later window has XDG.
        let directory = shop();
        let root = directory.path();
        let mut server = mcp12_agent(root, Path::new("/tmp"), false);
        let (text, hash) = read(&mut server, SCENARIO);
        let result = server.call("write_source", json!({"path": SCENARIO, "text": commented(&text, "Before window."), "expected_source_sha256": hash}));
        assert_eq!(structured(&result)["wrote"], true);
        let mut harness = folder_harness(root, WIDE);
        app(&mut harness).watcher.set_interval(NO_POLL);
        await_until(&mut harness, Instant::now(), "observer connection", |h| {
            has_label(h, TAKE_BACK)
        });
        let scenario = only(&harness, DocumentKind::Scenario);
        let written = commented(&text, "After window.");
        let result = write(&mut harness, &mut server, SCENARIO, &written);
        assert_eq!(structured(&result)["wrote"], true);
        await_until(
            &mut harness,
            Instant::now(),
            "save event without polling",
            |h| source_of(h, &scenario) == written,
        );
        let ended = server.close();
        assert!(ended.status.success());
        await_until(
            &mut harness,
            Instant::now(),
            "window ownership after EOF",
            |h| !read_only(h),
        );
        drop(harness);
        // A permitted first live call also requests hand-over. Permission-off
        // remains live_disabled and leaves ownership in the window. Since
        // MCP-09 the open window's box decides, not the start flag.
        for permitted in [false, true] {
            let directory = shop();
            let root = directory.path();
            let api = apiforge_demo_api::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
            let manifest = root.join("apiforge.project.json");
            let mut config: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
            config["environment"]["baseUrl"] = json!(api.base_url());
            fs::write(manifest, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
            // The shop's illustrative promotion scenario intentionally fails;
            // this fixture tests a successful HTTP run and unchanged live consent.
            fs::write(root.join(SCENARIO), "scenario \"MCP-12 live\" {\n  response = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  ok = assert response.status == 200\n}\n").unwrap();
            let mut harness = folder_harness(root, WIDE);
            if permitted {
                super::mcp09::set_box(&mut harness, true);
            }
            await_endpoint(&mut harness, root, HolderRole::Window);
            let mut server = mcp12_agent(root, &temp, permitted);
            let result = call(
                &mut harness,
                &mut server,
                "run",
                json!({"path": SCENARIO, "live": true}),
            );
            if permitted {
                assert_eq!(structured(&result)["status"], "passed", "{result}");
                assert!(read_only(&harness));
                call(&mut harness, &mut server, "finish_editing", json!({}));
                await_until(
                    &mut harness,
                    Instant::now(),
                    "ownership after live run",
                    |h| !read_only(h),
                );
            } else {
                assert_eq!(tool_error(&result)["code"], "live_disabled");
                assert!(!read_only(&harness));
            }
        }
    }

    fn mcp12_agent(root: &Path, temp: &Path, allow_live: bool) -> StdioServer {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--ignored",
            "--exact",
            "folder::tests::acceptance_tests::mcp08b::server_process",
            "--nocapture",
            "--quiet",
            "--test-threads=1",
        ]);
        #[cfg(windows)]
        {
            command.env_clear();
            for name in ["PATH", "SystemRoot"] {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
        }
        #[cfg(unix)]
        command.env_remove("XDG_RUNTIME_DIR").env("TMPDIR", temp);
        #[cfg(windows)]
        let _ = temp;
        command.env(FOLDER, root).env(
            "APIFORGE_MCP12_ALLOW_LIVE",
            if allow_live { "1" } else { "0" },
        );
        let mut server = StdioServer::spawn(command);
        server.initialize("MCP-12 client");
        server
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

    /// Review of #114 (the Windows failure of run 40660700539): from the
    /// window's decision to hand over until the agent says it saved, the
    /// window starts no check of its own and has none in flight, so it never
    /// reads the files the agent's first commit replaces. Checks here are due
    /// every 20 ms, so a check that is not held back would start.
    #[test]
    fn a_granting_window_reads_nothing_until_the_agent_has_saved() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        app(&mut harness)
            .watcher
            .set_interval(Duration::from_millis(20));
        let mut server = agent(root, "Claude Code");
        let (text, _) = read(&mut server, SCENARIO);
        let answer = write(
            &mut harness,
            &mut server,
            SCENARIO,
            &commented(&text, "Written by the agent."),
        );
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        let deadline = Instant::now() + Duration::from_secs(10);
        while folder(&harness).handover.first_saved.is_none() {
            assert!(Instant::now() < deadline, "the agent never said it saved");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        let handover = &folder(&harness).handover;
        let decided = handover.decided.expect("the window decided to hand over");
        let saved = handover.first_saved.unwrap();
        assert_eq!(
            handover.granted_busy,
            Some(false),
            "no check was in flight when the window granted"
        );
        let early: Vec<_> = folder(&harness)
            .watcher
            .started
            .iter()
            .filter(|started| **started >= decided && **started < saved)
            .map(|started| started.duration_since(decided))
            .collect();
        assert!(
            early.is_empty(),
            "checks started between the decision and the agent's save: {early:?}"
        );
        // Told, the window looks at once.
        let told = Instant::now();
        while !folder(&harness)
            .watcher
            .started
            .iter()
            .any(|started| *started >= saved)
        {
            assert!(told.elapsed() < BOUND, "the window never looked");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(harness);
        drop(server);
    }

    /// The write `id` waits on the hand-over prompt (MCP-08c); the person
    /// answers *Not now* and the write is refused.
    pub(super) fn keep_editing(
        harness: &mut FolderHarness,
        server: &mut StdioServer,
        id: u64,
    ) -> Value {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !has_label(harness, super::mcp08c::PROMPT) {
            assert!(Instant::now() < deadline, "the window never asked");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        choose(harness, super::mcp08c::NOT_NOW);
        answer(harness, server, id)
    }

    /// Criterion 3, as MCP-08c changed it: with unsaved work the window
    /// asks the person, who keeps editing; the write is `handover_refused`
    /// with the reason; the window says so in the bar; nothing is written
    /// and the person's text is untouched.
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
        let (_, hash) = read(&mut server, SCENARIO);
        let id = server.start_call(
            "write_source",
            json!({ "path": SCENARIO, "text": commented(&saved, "Written by the agent."), "expected_source_sha256": hash }),
        );
        let answer = keep_editing(&mut harness, &mut server, id);
        let error = tool_error(&answer);
        assert_eq!(error["code"], "handover_refused", "{answer}");
        assert_eq!(error["wrote"], false);
        assert_eq!(error["details"]["reason"], super::mcp08c::KEEP_EDITING);
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
        // A refusal carries its error object as text (MCP-17).
        let outcome = refusal(&answer).unwrap_or_else(|| structured(&answer).clone());
        match outcome["wrote"].as_bool() {
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
    /// The revisions of the product component the window's session lists.
    fn product_revisions(harness: &FolderHarness) -> Vec<u32> {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .manifest()
            .revisions
            .iter()
            .filter(|revision| revision.handle == "product")
            .map(|revision| revision.revision)
            .collect()
    }

    /// MCP-05 beside MCP-08b: a publication while a clean window holds the
    /// project asks it, as a write does; the window hands the project over
    /// and lists the new revision it was told of, without a check of its
    /// own.
    #[test]
    fn a_publication_asks_the_window_and_the_revision_is_shown() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        app(&mut harness).watcher.set_interval(NO_POLL);
        assert_eq!(product_revisions(&harness), [1]);

        let mut server = agent(root, "Claude Code");
        let (_, hash) = read(&mut server, PRODUCT);
        let answer = call(
            &mut harness,
            &mut server,
            "publish",
            json!({ "path": PRODUCT, "expected_source_sha256": hash }),
        );
        let returned = Instant::now();
        let published = structured(&answer);
        assert_eq!(published["wrote"], true, "{answer}");
        assert_eq!(published["revision"], 2);
        let took = await_until(&mut harness, returned, "the new revision", |harness| {
            product_revisions(harness) == [1, 2]
        });
        assert!(
            took < BOUND,
            "shown {took:?} after the publication returned"
        );
        assert!(read_only(&harness), "the window let the project go");
        assert!(contains(&harness, &format!("{AGENT} · ")));
        assert_eq!(
            holder(root).and_then(|holder| holder.role()),
            Some(HolderRole::Agent)
        );
        drop(harness);
        drop(server);
    }

    /// MCP-05 beside MCP-08b: unsaved work in the window refuses the
    /// hand-over a publication asks for once the person keeps editing
    /// (MCP-08c); nothing is published.
    #[test]
    fn unsaved_work_refuses_a_publication_and_nothing_is_published() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = only(&harness, DocumentKind::Scenario);
        let label = editor(&harness, &scenario);
        type_into(&mut harness, &label, "// typed by the person\n");
        let registry = fs::read(root.join("apiforge.project.json")).unwrap();

        let mut server = agent(root, "Claude Code");
        let (_, hash) = read(&mut server, PRODUCT);
        let id = server.start_call(
            "publish",
            json!({ "path": PRODUCT, "expected_source_sha256": hash }),
        );
        let answer = keep_editing(&mut harness, &mut server, id);
        let error = tool_error(&answer);
        assert_eq!(error["code"], "handover_refused", "{answer}");
        assert_eq!(error["wrote"], false);
        settle(&mut harness);
        assert!(contains(&harness, REFUSED), "the window says why");
        assert!(!read_only(&harness), "the window keeps the project");
        assert_eq!(
            fs::read(root.join("apiforge.project.json")).unwrap(),
            registry,
            "no revision was registered"
        );
        assert_eq!(product_revisions(&harness), [1]);
        drop(harness);
        drop(server);
    }

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
                // UX-02: the run controls end with the chevron.
                let run = harness.get_by_label("Run options").rect();
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
            let (text, hash) = read(&mut server, SCENARIO);
            let id = server.start_call(
                "write_source",
                json!({ "path": SCENARIO, "text": commented(&text, "Refused."), "expected_source_sha256": hash }),
            );
            keep_editing(&mut harness, &mut server, id);
            settle(&mut harness);
            capture(&mut harness, &format!("mcp08b-refused-{suffix}"), None);
            drop(harness);
            drop(server);
        }
    }
}

// ----- MCP-08c: the hand-over prompt when the window has unsaved work --------

mod mcp08c {
    use super::mcp08b::{
        AGENT, PRODUCT, REFUSED, SCENARIO, agent, answer, await_endpoint, await_until, commented,
        contains, holder, read, read_only,
    };
    use super::*;
    use crate::testing::{MINIMUM, assert_within_window};
    use apiforge_mcp::test_support::{StdioServer, structured, tool_error};
    use apiforge_storage::project::{
        HolderRole, ProjectError,
        faults::{Boundary, Phase},
    };
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT;
    use serde_json::{Value, json};
    use std::{path::Path, sync::Arc};

    pub(super) const PROMPT: &str = "An agent asks to edit this project";
    pub(super) const SAVE_ALL: &str = "Save all and hand over";
    pub(super) const NOT_NOW: &str = "Not now";
    /// The reason the agent hears when the person keeps editing.
    pub(super) const KEEP_EDITING: &str = "the person chose to keep editing";
    /// The reason the agent hears when *Save all* failed.
    const SAVE_FAILED: &str = "the window could not save the person's work";
    const NAME: &str = "Demo shop, renamed";

    fn unsaved_code(path: &str) -> String {
        format!("{path} — unsaved code")
    }

    /// What the window reads before the agent asks: the saved scenario,
    /// component and manifest.
    struct Saved {
        scenario: Vec<u8>,
        product: Vec<u8>,
        manifest: Vec<u8>,
    }

    impl Saved {
        fn read(root: &Path) -> Self {
            Self {
                scenario: fs::read(root.join(SCENARIO)).unwrap(),
                product: fs::read(root.join(PRODUCT)).unwrap(),
                manifest: fs::read(root.join("apiforge.project.json")).unwrap(),
            }
        }

        fn assert_on_disk(&self, root: &Path) {
            assert_eq!(fs::read(root.join(SCENARIO)).unwrap(), self.scenario);
            assert_eq!(fs::read(root.join(PRODUCT)).unwrap(), self.product);
            assert_eq!(
                fs::read(root.join("apiforge.project.json")).unwrap(),
                self.manifest
            );
        }
    }

    /// The document registered at `path`.
    fn at(harness: &FolderHarness, path: &str) -> String {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == path)
            .unwrap_or_else(|| panic!("{path} is registered"))
            .id()
            .to_owned()
    }

    /// Unsaved text in the scenario and the component and an unsaved
    /// project name; returns the two texts.
    fn leave_unsaved_work(harness: &mut FolderHarness, root: &Path) -> (String, String) {
        let scenario = at(harness, SCENARIO);
        let product = at(harness, PRODUCT);
        let scenario_text = commented(
            &fs::read_to_string(root.join(SCENARIO)).unwrap(),
            "Typed by the person in the scenario.",
        );
        let product_text = commented(
            &fs::read_to_string(root.join(PRODUCT)).unwrap(),
            "Typed by the person in the component.",
        );
        let session = app(harness).session.as_mut().unwrap();
        session
            .edit_source(&scenario, &scenario_text, CODE)
            .unwrap();
        session.edit_source(&product, &product_text, CODE).unwrap();
        session
            .edit_configuration(|manifest| manifest.name = NAME.into(), DIAGRAM)
            .unwrap();
        settle(harness);
        (scenario_text, product_text)
    }

    /// The agent's `write_source` of the scenario over the version it read,
    /// started and not yet answered.
    fn start_write(server: &mut StdioServer, note: &str) -> u64 {
        let (text, hash) = read(server, SCENARIO);
        server.start_call(
            "write_source",
            json!({ "path": SCENARIO, "text": commented(&text, note), "expected_source_sha256": hash }),
        )
    }

    /// The agent's write of the scenario, answered.
    fn write_now(harness: &mut FolderHarness, server: &mut StdioServer, note: &str) -> Value {
        let id = start_write(server, note);
        answer(harness, server, id)
    }

    /// Steps the window until it shows the prompt.
    fn await_prompt(harness: &mut FolderHarness) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !has_label(harness, PROMPT) {
            assert!(Instant::now() < deadline, "the window never asked");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(harness);
    }

    /// The buttons inside the prompt: between its heading and *Not now*.
    fn prompt_buttons(harness: &FolderHarness) -> Vec<String> {
        let heading = harness.get_by_label(PROMPT).rect();
        let last = harness.get_by_label(NOT_NOW).rect();
        harness
            .query_all_by_role(Role::Button)
            .filter(|node| {
                let rect = node.rect();
                rect.top() >= heading.top()
                    && rect.bottom() <= last.bottom() + 1.0
                    && rect.left() >= heading.left() - 24.0
                    && rect.right() <= last.right() + 1.0
            })
            .filter_map(|node| node.accesskit_node().label())
            .collect()
    }

    fn reason(answer: &Value) -> Value {
        let error = tool_error(answer);
        assert_eq!(error["code"], "handover_refused", "{answer}");
        assert_eq!(error["wrote"], false, "{answer}");
        error["details"]["reason"].clone()
    }

    /// Criterion 1: unsaved text in two documents and a dirty project name;
    /// the agent's write opens the prompt listing all three, waits up to
    /// thirty seconds, then returns `handover_pending` with nothing
    /// written; the prompt stays.
    #[test]
    fn mcp08c_criterion_1_the_prompt_lists_the_work_and_the_agent_hears_pending() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let saved = Saved::read(root);
        leave_unsaved_work(&mut harness, root);

        let mut server = agent(root, "Claude Code");
        let id = start_write(&mut server, "Written by the agent.");
        let asked = Instant::now();
        await_prompt(&mut harness);
        assert!(
            contains(&harness, &format!("{AGENT} wants to edit this project")),
            "the prompt names the agent"
        );
        for item in [
            unsaved_code(SCENARIO),
            unsaved_code(PRODUCT),
            "Project settings — name".to_owned(),
        ] {
            assert!(has_label(&harness, &item), "the prompt lists {item:?}");
        }
        assert!(has_label(&harness, SAVE_ALL));
        assert!(has_label(&harness, NOT_NOW));

        let answer = answer(&mut harness, &mut server, id);
        let waited = asked.elapsed();
        let error = tool_error(&answer);
        assert_eq!(error["code"], "handover_pending", "{answer}");
        assert_eq!(error["wrote"], false);
        assert!(
            waited >= Duration::from_secs(25) && waited < Duration::from_secs(40),
            "the agent waited {waited:?}"
        );
        saved.assert_on_disk(root);
        settle(&mut harness);
        assert!(has_label(&harness, PROMPT), "the prompt stays");
        assert!(!read_only(&harness), "the window keeps the project");
        assert_eq!(
            holder(root).and_then(|holder| holder.role()),
            Some(HolderRole::Window)
        );
        drop(harness);
        drop(server);
    }

    /// Criterion 2: *Save all and hand over* saves every listed item —
    /// the bytes on disk are the unsaved texts and the manifest carries the
    /// new name — the window becomes the observer, and the agent's next
    /// call succeeds.
    #[test]
    fn mcp08c_criterion_2_save_all_saves_everything_and_hands_over() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let (scenario_text, product_text) = leave_unsaved_work(&mut harness, root);

        let mut server = agent(root, "Claude Code");
        let id = start_write(&mut server, "Written over an older version.");
        await_prompt(&mut harness);
        choose(&mut harness, SAVE_ALL);
        let pending = answer(&mut harness, &mut server, id);
        // The waiting write was based on the version before the person's
        // save: granted the project, the server finds the disk changed.
        assert_eq!(tool_error(&pending)["code"], "stale_source", "{pending}");
        assert_eq!(
            fs::read_to_string(root.join(SCENARIO)).unwrap(),
            scenario_text,
            "the person's text is saved, not written over"
        );
        assert_eq!(
            fs::read_to_string(root.join(PRODUCT)).unwrap(),
            product_text
        );
        assert_eq!(saved_name(root), NAME);
        await_until(&mut harness, Instant::now(), "the observer", read_only);
        assert!(!has_label(&harness, PROMPT));
        assert!(contains(&harness, &format!("{AGENT} · ")));

        let next = write_now(&mut harness, &mut server, "Written by the agent.");
        assert_eq!(structured(&next)["wrote"], true, "{next}");
        assert_eq!(
            holder(root).and_then(|holder| holder.role()),
            Some(HolderRole::Agent)
        );
        drop(harness);
        drop(server);
    }

    /// Criterion 2, the failing save: a save that fails at the storage
    /// keeps the window the writer, shows the error, and the agent is
    /// refused with the reason.
    #[test]
    fn mcp08c_criterion_2_a_failed_save_keeps_the_project_and_refuses_with_the_reason() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let saved = Saved::read(root);
        let (scenario_text, _) = leave_unsaved_work(&mut harness, root);
        let scenario = only(&harness, DocumentKind::Scenario);
        app(&mut harness).storage.fault = Some(Arc::new(|event| {
            if event.boundary == Boundary::Replace
                && event.phase == Phase::Before
                && event.path.ends_with(SCENARIO)
            {
                return Err(ProjectError::Io {
                    path: event.path.clone(),
                    source: std::io::Error::other("injected on the scenario's save"),
                });
            }
            Ok(())
        }));

        let mut server = agent(root, "Claude Code");
        let id = start_write(&mut server, "Written by the agent.");
        await_prompt(&mut harness);
        choose(&mut harness, SAVE_ALL);
        let answer = answer(&mut harness, &mut server, id);
        assert_eq!(reason(&answer), SAVE_FAILED);
        settle(&mut harness);
        assert!(!read_only(&harness), "the window keeps the project");
        assert_eq!(
            holder(root).and_then(|holder| holder.role()),
            Some(HolderRole::Window)
        );
        let notice = folder(&harness)
            .notice
            .as_ref()
            .expect("the error is shown");
        assert!(notice.error, "{}", notice.text);
        assert!(
            notice.text.contains("promotion-and-cart-price.afs"),
            "{}",
            notice.text
        );
        assert!(contains(&harness, "promotion-and-cart-price.afs:"));
        assert_eq!(fs::read(root.join(SCENARIO)).unwrap(), saved.scenario);
        assert_eq!(source_of(&harness, &scenario), scenario_text);
        drop(harness);
        drop(server);
    }

    /// Criterion 3: *Not now* saves nothing and keeps the window the
    /// writer; the pending call, and the next one after a call that stopped
    /// waiting, is `handover_refused` with *the person chose to keep
    /// editing*; after a later Ctrl+S the agent's call is granted.
    #[test]
    fn mcp08c_criterion_3_not_now_keeps_editing_and_ctrl_s_lets_the_next_call_in() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let saved = Saved::read(root);
        let scenario = only(&harness, DocumentKind::Scenario);
        let label = editor(&harness, &scenario);
        type_into(&mut harness, &label, "// typed by the person\n");
        let typed = source_of(&harness, &scenario);

        // The pending call.
        let mut server = agent(root, "Claude Code");
        let id = start_write(&mut server, "Written by the agent.");
        await_prompt(&mut harness);
        choose(&mut harness, NOT_NOW);
        let refused = answer(&mut harness, &mut server, id);
        assert_eq!(reason(&refused), KEEP_EDITING);
        assert!(!has_label(&harness, PROMPT));
        saved.assert_on_disk(root);
        assert!(!read_only(&harness));
        assert_eq!(source_of(&harness, &scenario), typed);
        assert!(contains(&harness, REFUSED), "the bar says why");

        // The next call: the agent stopped waiting before the answer.
        let id = start_write(&mut server, "Written by the agent.");
        await_prompt(&mut harness);
        assert!(folder(&harness).handover.has_pending());
        drop(server);
        let _unanswered = id;
        let deadline = Instant::now() + Duration::from_secs(10);
        while folder(&harness).handover.has_pending() {
            assert!(Instant::now() < deadline, "the request never ended");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(has_label(&harness, PROMPT), "the prompt stays");
        choose(&mut harness, NOT_NOW);
        let mut server = agent(root, "Claude Code");
        let next = write_now(&mut harness, &mut server, "Written by the agent.");
        assert_eq!(reason(&next), KEEP_EDITING);
        saved.assert_on_disk(root);

        // Ctrl+S, then the agent's call is granted.
        harness.get_by_label(&label).focus();
        harness.step();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        settle(&mut harness);
        assert_eq!(fs::read_to_string(root.join(SCENARIO)).unwrap(), typed);
        let granted = write_now(&mut harness, &mut server, "Written by the agent.");
        assert_eq!(structured(&granted)["wrote"], true, "{granted}");
        settle(&mut harness);
        assert!(read_only(&harness), "the window observes");
        drop(harness);
        drop(server);
    }

    /// Unsaved work of an earlier session, held for a decision because the
    /// file changed since: the window that opens next holds the draft.
    fn hold_a_draft(root: &Path) -> String {
        let saved = fs::read_to_string(root.join(SCENARIO)).unwrap();
        let draft = commented(&saved, "From an earlier session.");
        {
            let mut harness = folder_harness(root, WIDE);
            let scenario = only(&harness, DocumentKind::Scenario);
            let session = app(&mut harness).session.as_mut().unwrap();
            session.edit_source(&scenario, &draft, CODE).unwrap();
            session.save_recovery().unwrap();
        }
        fs::write(root.join(SCENARIO), format!("// changed outside\n{saved}")).unwrap();
        draft
    }

    /// Criterion 4: a held recovery draft counts as unsaved work and is
    /// named in the prompt; the prompt offers no way to decide it.
    #[test]
    fn mcp08c_criterion_4_a_held_draft_is_named_and_not_decided_in_the_prompt() {
        let directory = shop();
        let root = directory.path();
        hold_a_draft(root);
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = only(&harness, DocumentKind::Scenario);
        assert!(
            folder(&harness).snapshot(&scenario).unwrap().held,
            "the draft is held for a decision"
        );
        let saved = Saved::read(root);

        let mut server = agent(root, "Claude Code");
        let id = start_write(&mut server, "Written by the agent.");
        await_prompt(&mut harness);
        assert!(
            has_label(
                &harness,
                &format!("{SCENARIO} — unsaved work from an earlier session, to keep or discard")
            ),
            "the prompt names the held draft"
        );
        let mut buttons = prompt_buttons(&harness);
        buttons.sort();
        assert_eq!(
            buttons,
            [NOT_NOW, SAVE_ALL],
            "nothing decides the draft here"
        );
        choose(&mut harness, NOT_NOW);
        assert_eq!(reason(&answer(&mut harness, &mut server, id)), KEEP_EDITING);
        saved.assert_on_disk(root);
        assert!(folder(&harness).snapshot(&scenario).unwrap().held);
        drop(harness);
        drop(server);
    }

    /// Beside criterion 4: *Save all and hand over* with a held draft saves
    /// what a save can, keeps the project here, says so, and the agent hears
    /// that unsaved work remains; the draft is still held for its decision
    /// in its tab.
    #[test]
    fn save_all_with_a_held_draft_saves_the_rest_and_keeps_the_project() {
        let directory = shop();
        let root = directory.path();
        hold_a_draft(root);
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = only(&harness, DocumentKind::Scenario);
        let product = at(&harness, PRODUCT);
        let product_text = commented(
            &fs::read_to_string(root.join(PRODUCT)).unwrap(),
            "Typed by the person in the component.",
        );
        app(&mut harness)
            .session
            .as_mut()
            .unwrap()
            .edit_source(&product, &product_text, CODE)
            .unwrap();
        settle(&mut harness);
        let saved_scenario = fs::read(root.join(SCENARIO)).unwrap();

        let mut server = agent(root, "Claude Code");
        let id = start_write(&mut server, "Written by the agent.");
        await_prompt(&mut harness);
        choose(&mut harness, SAVE_ALL);
        let answer = answer(&mut harness, &mut server, id);
        assert_eq!(reason(&answer), "the window has unsaved work");
        settle(&mut harness);
        assert_eq!(
            fs::read_to_string(root.join(PRODUCT)).unwrap(),
            product_text
        );
        assert_eq!(fs::read(root.join(SCENARIO)).unwrap(), saved_scenario);
        assert!(!read_only(&harness), "the window keeps the project");
        assert!(folder(&harness).snapshot(&scenario).unwrap().held);
        let notice = folder(&harness)
            .notice
            .as_ref()
            .expect("the window says why");
        assert!(notice.text.contains("earlier session"), "{}", notice.text);
        drop(harness);
        drop(server);
    }

    /// Criterion 5: at 960 × 540 the prompt stays inside the window, and
    /// Escape is *Not now*.
    #[test]
    fn mcp08c_criterion_5_the_prompt_fits_960_by_540_and_escape_is_not_now() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let saved = Saved::read(root);
        leave_unsaved_work(&mut harness, root);

        let mut server = agent(root, "Claude Code");
        let id = start_write(&mut server, "Written by the agent.");
        await_prompt(&mut harness);
        let scenario_item = unsaved_code(SCENARIO);
        let product_item = unsaved_code(PRODUCT);
        assert_within_window(
            &harness,
            &[
                PROMPT,
                scenario_item.as_str(),
                product_item.as_str(),
                "Project settings — name",
                SAVE_ALL,
                NOT_NOW,
            ],
        );
        harness.key_press(Key::Escape);
        settle(&mut harness);
        assert!(!has_label(&harness, PROMPT), "Escape closes the prompt");
        assert_eq!(reason(&answer(&mut harness, &mut server, id)), KEEP_EDITING);
        saved.assert_on_disk(root);
        assert!(!read_only(&harness));
        drop(harness);
        drop(server);
    }

    /// The prompt beside M2, at both sizes: two unsaved documents and the
    /// project name.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp08c_handover_prompt() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = shop();
            let root = directory.path();
            let mut harness = folder_harness(root, size);
            await_endpoint(&mut harness, root, HolderRole::Window);
            leave_unsaved_work(&mut harness, root);
            let mut server = agent(root, "Claude Code");
            let id = start_write(&mut server, "Written by the agent.");
            await_prompt(&mut harness);
            capture(&mut harness, &format!("mcp08c-prompt-{suffix}"), None);
            choose(&mut harness, NOT_NOW);
            answer(&mut harness, &mut server, id);
            drop(harness);
            drop(server);
        }
    }
}

// ----- MCP-05: the window beside a publication from the server ---------------

mod mcp05 {
    use super::*;
    use crate::testing::shop_project;
    use apiforge_mcp::agent::{AgentSession, PublishArguments};
    use apiforge_storage::project::FileState;

    const PRODUCT: &str = "components/product.afs";

    fn sha256(bytes: &[u8]) -> String {
        match FileState::of(bytes) {
            FileState::File { sha256, .. } => sha256,
            _ => unreachable!(),
        }
    }

    /// The revisions of the product component the window's session lists.
    fn listed(harness: &FolderHarness) -> Vec<u32> {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .manifest()
            .revisions
            .iter()
            .filter(|revision| revision.handle == "product")
            .map(|revision| revision.revision)
            .collect()
    }

    /// Criterion 8: the window, open read-only beside the server, lists the
    /// revision the server published after its project reload: the
    /// registry changed and nothing is unsaved.
    #[test]
    fn a_read_only_window_lists_the_revision_the_server_published() {
        let project = shop_project();
        let root = project.path();
        let agent = AgentSession::with_environment(root.to_owned(), Vec::new());
        let saved = fs::read(root.join(PRODUCT)).unwrap();
        let arguments = PublishArguments {
            path: PRODUCT.into(),
            expected_source_sha256: sha256(&saved),
            accept_comment_loss: false,
            expected_generation: None,
            request_id: None,
        };
        // The window opens while the agent session holds the project: the
        // first publication takes the lock, the second is the one followed.
        agent.publish(&arguments).unwrap();
        assert!(agent.is_active());
        let mut harness = folder_harness(root, WIDE);
        assert!(folder(&harness).session.as_ref().unwrap().is_read_only());
        assert_eq!(listed(&harness), [1, 2]);

        let published = agent.publish(&arguments).unwrap();
        assert_eq!(published.revision, 3);
        follow_disk(&mut harness);
        assert_eq!(listed(&harness), [1, 2, 3], "the new revision is listed");
        let app = folder(&harness);
        assert!(
            app.notice.as_ref().is_none_or(|notice| !notice.error),
            "no conflict is reported"
        );
        let session = app.session.as_ref().unwrap();
        let product = session
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == PRODUCT)
            .unwrap();
        assert_eq!(product.entry().base_revision, Some(3));
        assert!(
            session
                .documents()
                .iter()
                .all(|document| !document.is_dirty() && !document.is_layout_dirty()),
            "nothing is unsaved"
        );
        drop(harness);
        drop(agent);
    }

    /// The project as Git sees it: every file outside `.apiforge/local`.
    fn shared(root: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        fn walk(
            root: &std::path::Path,
            at: &std::path::Path,
            files: &mut std::collections::BTreeMap<String, Vec<u8>>,
        ) {
            for entry in fs::read_dir(at).unwrap() {
                let path = entry.unwrap().path();
                let relative = path.strip_prefix(root).unwrap();
                if relative == std::path::Path::new(".apiforge").join("local") {
                    continue;
                }
                if path.is_dir() {
                    walk(root, &path, files);
                } else {
                    files.insert(
                        relative.to_string_lossy().replace('\\', "/"),
                        fs::read(&path).unwrap(),
                    );
                }
            }
        }
        let mut files = std::collections::BTreeMap::new();
        walk(root, root, &mut files);
        files
    }

    /// Criterion 1's "the same transaction as the window's Publish", through
    /// the window: *Publish* pressed in the component's header in one copy
    /// of the project and `publish` from the server in another leave the
    /// same files with the same bytes.
    #[test]
    fn the_windows_publish_and_the_servers_write_the_same_bytes() {
        let window = shop_project();
        let server = tempfile::tempdir().unwrap();
        for (relative, bytes) in shared(window.path()) {
            let target = server.path().join(&relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
        assert_eq!(shared(server.path()), shared(window.path()));

        let agent = AgentSession::with_environment(server.path().to_owned(), Vec::new());
        let saved = fs::read(server.path().join(PRODUCT)).unwrap();
        let published = agent
            .publish(&PublishArguments {
                path: PRODUCT.into(),
                expected_source_sha256: sha256(&saved),
                accept_comment_loss: false,
                expected_generation: None,
                request_id: None,
            })
            .unwrap();
        assert_eq!(published.revision, 2);
        drop(agent);

        let mut harness = folder_harness(window.path(), WIDE);
        let product = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == PRODUCT)
            .unwrap()
            .id()
            .to_owned();
        app(&mut harness).activate_tab(&product);
        settle(&mut harness);
        choose(&mut harness, "Publish");
        let deadline = Instant::now() + Duration::from_secs(10);
        while listed(&harness) != [1, 2] {
            assert!(Instant::now() < deadline, "the window never published");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(&mut harness);
        drop(harness);

        let window = shared(window.path());
        assert_eq!(
            window.get(&published.revision_path),
            shared(server.path()).get(&published.revision_path),
            "the revision file"
        );
        assert_eq!(window, shared(server.path()), "every shared file");
    }
}

// ----- MCP-09: the live toggle — the person allows the agent's live requests ---

mod mcp09 {
    use super::mcp08b::{
        PRODUCT, SCENARIO, agent, agent_with, answer, await_endpoint, await_until, call, connect,
        contains, hand_over, read, read_only, write,
    };
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use apiforge_mcp::test_support::{StdioServer, refusal, structured, tool_error};
    use apiforge_project::{FixedPlacer, Holder, HolderRole};
    use egui_kittest::kittest::NodeT;
    use serde_json::{Value, json};
    use std::{
        collections::BTreeMap,
        io::Write as _,
        path::{Path, PathBuf},
    };

    pub(super) const TOGGLE: &str = "Allow the MCP agent to send live requests";
    pub(super) const OFF: &str = "Off — the agent can only simulate.";
    pub(super) const ON: &str = "On — the agent may send real requests to ";
    const IN_PROGRESS: &str = "A live run is in progress; it will finish. No new one starts.";
    pub(super) const BADGE: &str = "Live allowed";
    pub(super) const NOTE: &str = "Apply changes, then use Save all to save the project settings.";
    /// The button that applies the toggle since MCP-26 (owner decision 4 =
    /// A of 2026-11-02).
    pub(super) const APPLY: &str = "Apply settings";
    const SETTINGS_OFF: &str = "live requests are off in this project's settings";
    const ALLOW_LIVE: &str = "APIFORGE_MCP12_ALLOW_LIVE";
    const STEP_DELAY: &str = "APIFORGE_MCP_TEST_STEP_DELAY_MS";
    /// Criterion 3's bound: the agent sees a change this soon.
    const SEEN: Duration = Duration::from_secs(1);
    /// A scenario that passes against the demo API, with a cleanup.
    const LIVE: &str = "scenario \"MCP-09 live\" {\n  response = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  ok = assert response.status == 200\n  cleanup {\n    again = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  }\n}\n";

    /// Where the application keeps its preferences: the test's own
    /// configuration directory, as `XDG_CONFIG_HOME` names it.
    pub(super) fn consent_file(configuration: &Path) -> PathBuf {
        configuration.join("apiforge").join("agent-consent.json")
    }

    /// The window over `root`, its preferences in `configuration`.
    pub(super) fn window(root: &Path, size: egui::Vec2, configuration: &Path) -> FolderHarness {
        let mut harness = folder_harness(root, size);
        app(&mut harness).use_configuration_directory(configuration);
        settle(&mut harness);
        harness
    }

    /// The demo shop pointed at a demo API of its own, its scenario one
    /// that passes live and has a cleanup.
    pub(super) fn live_shop() -> (tempfile::TempDir, apiforge_demo_api::DemoServerHandle) {
        let directory = shop();
        let root = directory.path();
        let api = apiforge_demo_api::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
        let manifest = root.join("apiforge.project.json");
        let mut config: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        config["environment"]["baseUrl"] = json!(api.base_url());
        fs::write(manifest, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        fs::write(root.join(SCENARIO), LIVE).unwrap();
        (directory, api)
    }

    pub(super) fn toggle(harness: &FolderHarness) -> bool {
        harness.get_by_label(TOGGLE).accesskit_node().toggled()
            == Some(egui::accesskit::Toggled::True)
    }

    pub(super) fn enabled(harness: &FolderHarness, label: &str) -> bool {
        !harness.get_by_label(label).accesskit_node().is_disabled()
    }

    pub(super) fn open_settings(harness: &mut FolderHarness) {
        open_project_menu(harness, "Project settings…");
        assert!(has_label(harness, TOGGLE), "the dialog has the toggle");
    }

    pub(super) fn close_settings(harness: &mut FolderHarness) {
        harness
            .get_by_role_and_label(egui::accesskit::Role::Button, "Close")
            .click();
        settle(harness);
        assert!(!has_label(harness, TOGGLE), "the dialog closed");
    }

    /// Sets the box through *Project settings*: a change applies with
    /// *Apply settings* (MCP-26), an unchanged box closes the dialog.
    pub(super) fn set_box(harness: &mut FolderHarness, live: bool) {
        open_settings(harness);
        if toggle(harness) == live {
            close_settings(harness);
            return;
        }
        choose(harness, TOGGLE);
        assert_eq!(toggle(harness), live);
        apply_settings(harness);
    }

    /// Presses *Apply settings*, which closes the dialog.
    pub(super) fn apply_settings(harness: &mut FolderHarness) {
        assert!(enabled(harness, APPLY), "Apply settings is enabled");
        choose(harness, APPLY);
        assert!(!has_label(harness, TOGGLE), "the dialog closed");
    }

    /// The box as *Project settings* shows it.
    pub(super) fn box_ticked(harness: &mut FolderHarness) -> bool {
        open_settings(harness);
        let ticked = toggle(harness);
        close_settings(harness);
        ticked
    }

    pub(super) fn live_run(harness: &mut FolderHarness, server: &mut StdioServer) -> Value {
        call(
            harness,
            server,
            "run",
            json!({ "path": SCENARIO, "live": true }),
        )
    }

    pub(super) fn live_requests(server: &mut StdioServer) -> Value {
        let overview = server.call("project_overview", json!({}));
        structured(&overview)["live_requests"].clone()
    }

    /// Steps the window until the agent's overview reports `expected`, and
    /// returns how long that took after `since`.
    pub(super) fn await_live_requests(
        harness: Option<&mut FolderHarness>,
        server: &mut StdioServer,
        since: Instant,
        expected: Value,
    ) -> Duration {
        let mut harness = harness;
        let deadline = since + Duration::from_secs(10);
        loop {
            let seen = live_requests(server);
            if seen == expected {
                return since.elapsed();
            }
            assert!(
                Instant::now() < deadline,
                "the overview still says {seen}, not {expected}"
            );
            if let Some(harness) = harness.as_deref_mut() {
                harness.step();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub(super) fn assert_refused_in_settings(result: &Value) {
        let error = tool_error(result);
        assert_eq!(error["code"], "live_disabled", "{result}");
        assert_eq!(error["message"], SETTINGS_OFF, "{result}");
    }

    pub(super) fn assert_sent(result: &Value) {
        let report = structured(result);
        assert_eq!(report["live"], true, "{result}");
        assert_eq!(report["status"], "passed", "{result}");
        assert_eq!(report["ended_by"], "completed", "{result}");
    }

    /// Every file under `root`, by relative path. The writer's lock file
    /// is always empty and, on Windows, cannot be read while it is held: it
    /// is listed by its length.
    pub(super) fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, directory: &Path, found: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, found);
                } else if path.is_file() {
                    let bytes = if path.ends_with(".apiforge/local/project.lock") {
                        fs::metadata(&path).unwrap().len().to_le_bytes().to_vec()
                    } else {
                        fs::read(&path).unwrap()
                    };
                    found.insert(path.strip_prefix(root).unwrap().to_owned(), bytes);
                }
            }
        }
        let mut found = BTreeMap::new();
        walk(root, root, &mut found);
        found
    }

    pub(super) fn manifest_id(root: &Path) -> String {
        let bytes = fs::read(root.join("apiforge.project.json")).unwrap();
        apiforge_project::ProjectManifest::from_bytes(&bytes)
            .unwrap()
            .id
    }

    /// A second session in this process holds the project as an agent's
    /// would, so the window observes it (MCP-07, MCP-08a).
    pub(super) fn agent_holds(root: &Path) -> ProjectSession {
        let holder = Holder::new(HolderRole::Agent, Some("Claude Code".into()));
        ProjectSession::open_with_as(root, Arc::new(FixedPlacer), &holder).unwrap()
    }

    /// Criterion 1: off for a project never set; ticked, it is ticked again
    /// when the window reopens; another project is unaffected; the setting
    /// is kept in the preferences directory by the manifest id, and nothing
    /// under the project folder changes.
    #[test]
    fn mcp09_criterion_1_the_box_is_remembered_per_project_outside_the_folder() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let root = directory.path();
        let other = tempfile::tempdir().unwrap();
        apiforge_project::prepare_project(other.path(), &apiforge_core::demos::tickets_project())
            .unwrap();
        assert_ne!(manifest_id(root), manifest_id(other.path()));

        let mut harness = window(root, WIDE, configuration.path());
        let before = files(root);
        assert!(!box_ticked(&mut harness), "off for a project never set");
        set_box(&mut harness, true);
        settle(&mut harness);
        assert_eq!(files(root), before, "the project folder is byte-identical");
        drop(harness);

        let stored: Value =
            serde_json::from_slice(&fs::read(consent_file(configuration.path())).unwrap()).unwrap();
        assert_eq!(stored["version"], 1, "{stored}");
        let id = manifest_id(root);
        let entry = &stored["projects"][&id];
        assert_eq!(entry["live"], true, "{stored}");
        assert!(entry["changedAt"].is_string(), "{stored}");
        assert!(
            stored["projects"].get(manifest_id(other.path())).is_none(),
            "{stored}"
        );

        let mut harness = window(root, WIDE, configuration.path());
        assert!(box_ticked(&mut harness), "ticked again after reopening");
        drop(harness);
        let mut harness = window(other.path(), WIDE, configuration.path());
        assert!(!box_ticked(&mut harness), "another project is unaffected");
        drop(harness);
    }

    /// Criterion 2: *Project settings* opens while observing with only the
    /// toggle, *Close* and *Cancel* enabled (owner decision of 2026-10-28);
    /// the footer note is hidden; the box applies with nothing to save.
    /// Since MCP-26 (owner decision 4 = A of 2026-11-02) *Apply settings* is
    /// shown while observing, enabled only once the box changed, and the box
    /// applies with it, not at once.
    #[test]
    fn mcp09_criterion_2_settings_open_while_observing_and_the_box_applies_with_apply() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let root = directory.path();
        let owner = agent_holds(root);
        let mut harness = window(root, WIDE, configuration.path());
        assert!(has_label(&harness, "Observing"));

        open_settings(&mut harness);
        assert!(!enabled(&harness, "Project name"));
        assert!(!enabled(&harness, "Data contracts"));
        assert!(enabled(&harness, TOGGLE));
        assert!(enabled(&harness, "Cancel"));
        assert!(
            !harness
                .get_by_role_and_label(egui::accesskit::Role::Button, "Close")
                .accesskit_node()
                .is_disabled()
        );
        assert!(has_label(&harness, APPLY), "Apply is shown (MCP-26)");
        assert!(!enabled(&harness, APPLY), "nothing to apply yet (MCP-26)");
        assert!(!contains(&harness, NOTE), "the footer note is hidden");
        assert!(contains(&harness, OFF));

        choose(&mut harness, TOGGLE);
        assert!(toggle(&harness));
        assert!(contains(&harness, ON), "the line says what the box allows");
        assert!(
            !has_label(&harness, BADGE),
            "not applied while the dialog is open (MCP-26)"
        );
        apply_settings(&mut harness);
        assert!(has_label(&harness, BADGE), "Apply settings applied it");
        let stored = fs::read_to_string(consent_file(configuration.path())).unwrap();
        assert!(stored.contains(&manifest_id(root)), "{stored}");
        assert!(
            !folder(&harness)
                .session
                .as_ref()
                .unwrap()
                .is_manifest_dirty(),
            "nothing to save"
        );
        drop(harness);
        drop(owner);
    }

    /// Criterion 3: with the agent connected and the box off a live run is
    /// refused with the settings message; ticking the box, with nothing
    /// restarted, shows in the agent's overview within a second and the next
    /// live run sends.
    #[test]
    fn mcp09_criterion_3_ticking_the_box_lets_the_next_live_run_send() {
        let configuration = tempfile::tempdir().unwrap();
        let (directory, _api) = live_shop();
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        hand_over(&mut harness, &mut server, "The agent is connected.");
        assert!(read_only(&harness));

        assert_refused_in_settings(&live_run(&mut harness, &mut server));
        assert_eq!(
            live_requests(&mut server),
            json!({ "allowed": false, "source": "off in the project's settings" })
        );

        // Since MCP-26 the box applies with *Apply settings*.
        open_settings(&mut harness);
        choose(&mut harness, TOGGLE);
        harness.get_by_label(APPLY).click();
        let changed = Instant::now();
        let took = await_live_requests(
            Some(&mut harness),
            &mut server,
            changed,
            json!({ "allowed": true, "source": "window setting" }),
        );
        assert!(took < SEEN, "the agent saw the change after {took:?}");
        settle(&mut harness);
        assert!(!has_label(&harness, TOGGLE), "the dialog closed");
        assert_sent(&live_run(&mut harness, &mut server));
        drop(harness);
        drop(server);
    }

    /// Criterion 4: a server started with `--allow-live` is refused while
    /// the window is connected with the box off, also before the hand-over,
    /// and the window keeps the project then; once the window closes the
    /// flag applies again.
    #[test]
    fn mcp09_criterion_4_the_flag_neither_enables_nor_overrides_an_open_window() {
        let configuration = tempfile::tempdir().unwrap();
        let (directory, _api) = live_shop();
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent_with(root, "Claude Code", &[(ALLOW_LIVE, "1")]);

        assert_refused_in_settings(&live_run(&mut harness, &mut server));
        assert!(!read_only(&harness), "the window keeps the project");

        hand_over(&mut harness, &mut server, "The agent is connected.");
        assert_eq!(
            live_requests(&mut server),
            json!({ "allowed": false, "source": "off in the project's settings" })
        );
        assert_refused_in_settings(&live_run(&mut harness, &mut server));

        drop(harness);
        await_live_requests(
            None,
            &mut server,
            Instant::now(),
            json!({ "allowed": true, "source": "--allow-live" }),
        );
        assert_sent(&server.call("run", json!({ "path": SCENARIO, "live": true })));
        let ended = server.close();
        assert_eq!(ended.status.code(), Some(0), "{}", ended.stderr);
    }

    /// Criterion 4, review of #189: a connection to the server that has not
    /// said a consent yet counts as off, never as the flag; when it goes the
    /// flag applies again.
    #[test]
    fn mcp09_a_window_that_has_not_said_its_consent_is_off() {
        let (directory, _api) = live_shop();
        let root = directory.path();
        let mut server = agent_with(root, "Claude Code", &[(ALLOW_LIVE, "1")]);
        let (text, hash) = read(&mut server, SCENARIO);
        let written = server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": super::mcp08b::commented(&text, "Mine."), "expected_source_sha256": hash }),
        );
        assert_eq!(structured(&written)["wrote"], true, "{written}");
        let flag = json!({ "allowed": true, "source": "--allow-live" });
        assert_eq!(live_requests(&mut server), flag);
        let endpoint = super::mcp08b::holder(root)
            .and_then(|holder| holder.endpoint().map(str::to_owned))
            .expect("the server names its endpoint");
        let silent = connect(&endpoint).unwrap();
        await_live_requests(
            None,
            &mut server,
            Instant::now(),
            json!({ "allowed": false, "source": "off in the project's settings" }),
        );
        let refused = server.call("run", json!({ "path": SCENARIO, "live": true }));
        assert_refused_in_settings(&refused);
        drop(silent);
        await_live_requests(None, &mut server, Instant::now(), flag);
        let ended = server.close();
        assert_eq!(ended.status.code(), Some(0), "{}", ended.stderr);
    }

    /// Criterion 5: a server started without `--allow-live` sends while the
    /// window is connected with the box on; its first live run asks for the
    /// project like a write.
    #[test]
    fn mcp09_criterion_5_the_box_allows_a_server_started_without_the_flag() {
        let configuration = tempfile::tempdir().unwrap();
        let (directory, _api) = live_shop();
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        set_box(&mut harness, true);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");

        assert_sent(&live_run(&mut harness, &mut server));
        assert!(read_only(&harness), "the window handed the project over");
        assert_eq!(
            live_requests(&mut server),
            json!({ "allowed": true, "source": "window setting" })
        );
        assert_sent(&live_run(&mut harness, &mut server));
        drop(harness);
        drop(server);
    }

    /// Criterion 6: no tool and no resource changes the setting, and the
    /// window takes no consent from the other side of the channel.
    #[test]
    fn mcp09_criterion_6_no_tool_resource_or_message_changes_the_setting() {
        let configuration = tempfile::tempdir().unwrap();
        let (directory, _api) = live_shop();
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        set_box(&mut harness, true);
        let file = consent_file(configuration.path());
        let stored = fs::read(&file).unwrap();
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        hand_over(&mut harness, &mut server, "The agent is connected.");
        let slot = json!({ "allowed": true, "source": "window setting" });
        assert_eq!(live_requests(&mut server), slot);

        let unchanged = |harness: &mut FolderHarness, server: &mut StdioServer, what: &str| {
            settle(harness);
            assert_eq!(fs::read(&file).unwrap(), stored, "{what} changed the file");
            assert!(has_label(harness, BADGE), "{what} changed the window's box");
            assert_eq!(live_requests(server), slot, "{what} changed the slot");
        };
        let (text, _) = read(&mut server, SCENARIO);
        let calls = [
            ("project_overview", json!({})),
            ("language_reference", json!({})),
            ("read_source", json!({ "path": SCENARIO })),
            ("check", json!({})),
            ("check_text", json!({ "path": SCENARIO, "text": text })),
            ("print", json!({ "path": SCENARIO })),
            ("run", json!({ "path": SCENARIO })),
            ("run", json!({ "path": SCENARIO, "live": true })),
            (
                "create_document",
                json!({ "kind": "scenario", "path": "scenarios/mcp09.afs", "text": "scenario \"MCP-09\" {\n}\n" }),
            ),
        ];
        for (tool, arguments) in calls {
            let result = call(&mut harness, &mut server, tool, arguments);
            assert!(
                result["result"]["isError"] != true,
                "{tool} failed: {result}"
            );
            unchanged(&mut harness, &mut server, tool);
        }
        let written = write(
            &mut harness,
            &mut server,
            SCENARIO,
            &super::mcp08b::commented(&text, "Written again."),
        );
        assert_eq!(structured(&written)["wrote"], true, "{written}");
        unchanged(&mut harness, &mut server, "write_source");
        let (_, product) = read(&mut server, PRODUCT);
        let published = call(
            &mut harness,
            &mut server,
            "publish",
            json!({ "path": PRODUCT, "expected_source_sha256": product }),
        );
        assert_eq!(structured(&published)["wrote"], true, "{published}");
        unchanged(&mut harness, &mut server, "publish");
        let listed = server.request("resources/list", json!({}));
        let uri = listed["result"]["resources"][0]["uri"]
            .as_str()
            .unwrap()
            .to_owned();
        unchanged(&mut harness, &mut server, "resources/list");
        let resource = server.request("resources/read", json!({ "uri": uri }));
        assert!(resource["result"].is_object(), "{resource}");
        unchanged(&mut harness, &mut server, "resources/read");

        let finished = call(&mut harness, &mut server, "finish_editing", json!({}));
        assert_eq!(structured(&finished)["finished"], true);
        await_until(&mut harness, Instant::now(), "the take-back", |harness| {
            !read_only(harness)
        });
        assert_eq!(
            fs::read(&file).unwrap(),
            stored,
            "finish_editing changed the file"
        );
        assert!(has_label(&harness, BADGE));

        // A `permission` sent to the window, as a server would send it, is
        // not the person's: the window ignores it.
        let endpoint = await_endpoint(&mut harness, root, HolderRole::Window);
        {
            let mut stream = connect(&endpoint).unwrap();
            stream
                .write_all(b"{\"version\":1,\"type\":\"permission\",\"live\":false}\n")
                .unwrap();
        }
        for _ in 0..20 {
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(&mut harness);
        assert!(has_label(&harness, BADGE), "the window ignored it");
        assert!(box_ticked(&mut harness));
        assert_eq!(fs::read(&file).unwrap(), stored);
        drop(harness);
        drop(server);
    }

    /// Criterion 7: unticking during a live run lets it finish, cleanup
    /// included; the dialog says so; the agent's result is complete; the
    /// next live run is refused.
    #[test]
    fn mcp09_criterion_7_unticking_during_a_live_run_lets_it_finish() {
        let configuration = tempfile::tempdir().unwrap();
        let (directory, _api) = live_shop();
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        set_box(&mut harness, true);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent_with(root, "Claude Code", &[(STEP_DELAY, "400")]);
        hand_over(&mut harness, &mut server, "The agent is connected.");

        let start = json!({ "path": SCENARIO, "live": true });
        let mut run = server.start_call("run", start.clone());
        // The run holds the server's run slot: a probe of a document that
        // does not exist is `run_in_progress` while it does.
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let probe = call(
                &mut harness,
                &mut server,
                "run",
                json!({ "path": "missing.afs" }),
            );
            if refusal(&probe).is_some_and(|error| error["code"] == "run_in_progress") {
                break;
            }
            // A probe that came first held the slot for a moment, and the
            // run was refused: start it again.
            if let Some(ended) = server.poll(run) {
                assert_eq!(
                    tool_error(&ended)["code"],
                    "run_in_progress",
                    "the run ended before it was seen: {ended}"
                );
                run = server.start_call("run", start.clone());
            }
            assert!(Instant::now() < deadline, "the run never started: {probe}");
            std::thread::sleep(Duration::from_millis(20));
        }
        // Since MCP-26 the box applies with *Apply settings*, which closes
        // the dialog; reopened, it says the run goes on.
        open_settings(&mut harness);
        choose(&mut harness, TOGGLE);
        assert!(!toggle(&harness));
        apply_settings(&mut harness);
        open_settings(&mut harness);
        assert!(!toggle(&harness));
        await_until(
            &mut harness,
            Instant::now(),
            "the in-progress line",
            |harness| contains(harness, IN_PROGRESS),
        );

        let report = answer(&mut harness, &mut server, run);
        let report = structured(&report);
        assert_eq!(report["live"], true, "{report}");
        assert_eq!(report["ended_by"], "completed", "{report}");
        assert_eq!(report["status"], "passed", "{report}");
        let steps = report["steps"].as_array().unwrap();
        assert!(
            steps
                .iter()
                .any(|step| step["phase"] == "cleanup" && step["status"] == "passed"),
            "the cleanup ran: {report}"
        );
        await_until(
            &mut harness,
            Instant::now(),
            "the line after the run",
            |harness| contains(harness, OFF),
        );
        assert!(!contains(&harness, IN_PROGRESS));
        close_settings(&mut harness);
        assert_refused_in_settings(&live_run(&mut harness, &mut server));
        drop(harness);
        drop(server);
    }

    /// Criterion 8: the *Live allowed* badge in Code, Diagram and Split at
    /// 960 × 540 while the box is ticked, and never otherwise; beside
    /// *Observing* on a component's tab, which has *Publish*, it keeps clear
    /// of the mode tabs and the toolbar.
    #[test]
    fn mcp09_criterion_8_the_badge_shows_in_every_mode_while_ticked() {
        for observing in [false, true] {
            let configuration = tempfile::tempdir().unwrap();
            let directory = shop();
            let root = directory.path();
            let owner = observing.then(|| agent_holds(root));
            let mut harness = window(root, MINIMUM, configuration.path());
            assert_eq!(has_label(&harness, "Observing"), observing);
            let component = only(&harness, DocumentKind::Component);
            for mode in ["Code", "Diagram", "Split"] {
                choose(&mut harness, mode);
                assert!(!has_label(&harness, BADGE), "{mode}: no badge while off");
            }
            set_box(&mut harness, true);
            for tab in [only(&harness, DocumentKind::Scenario), component.clone()] {
                app(&mut harness).activate_tab(&tab);
                settle(&mut harness);
                for mode in ["Code", "Diagram", "Split"] {
                    choose(&mut harness, mode);
                    assert!(has_label(&harness, BADGE), "{mode}: the badge");
                    assert_within_window(&harness, &[BADGE]);
                    let mut labels = vec![BADGE, "Code", "Diagram", "Split", "Run options"];
                    if observing {
                        labels.push("Observing");
                    }
                    if tab == component {
                        labels.push("Publish");
                    }
                    assert_disjoint(&harness, &labels);
                }
            }
            set_box(&mut harness, false);
            for mode in ["Code", "Diagram", "Split"] {
                choose(&mut harness, mode);
                assert!(!has_label(&harness, BADGE), "{mode}: gone when unticked");
            }
            drop(harness);
            drop(owner);
        }
    }

    /// The dialog and the badge beside the approved captures
    /// `docs/qa/2026-10-27/MCP-00/m3-*.png`.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp09_live_toggle() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let configuration = tempfile::tempdir().unwrap();
            let directory = shop();
            let root = directory.path();
            let owner = agent_holds(root);
            let mut harness = window(root, size, configuration.path());
            open_settings(&mut harness);
            capture(&mut harness, &format!("mcp09-settings-off-{suffix}"), None);
            choose(&mut harness, TOGGLE);
            capture(&mut harness, &format!("mcp09-settings-on-{suffix}"), None);
            apply_settings(&mut harness);
            for mode in ["Code", "Diagram", "Split"] {
                choose(&mut harness, mode);
                capture(
                    &mut harness,
                    &format!("mcp09-header-{}-{suffix}", mode.to_lowercase()),
                    None,
                );
            }
            drop(harness);
            drop(owner);
        }
    }
}

// ----- MCP-26: the live toggle applies with Apply settings -------------------

/// *Allow the MCP agent to send live requests* takes effect on *Apply
/// settings* only, and *Cancel* discards it (MCP-26, #214; owner decision 4
/// = A of 2026-11-02). While the window observes an agent, *Apply settings*
/// is shown for the toggle alone.
mod mcp26 {
    use super::mcp08b::{agent, await_endpoint, hand_over, read_only};
    use super::mcp09::{
        APPLY, BADGE, OFF, TOGGLE, agent_holds, apply_settings, assert_refused_in_settings,
        assert_sent, await_live_requests, box_ticked, consent_file, enabled, files, live_requests,
        live_run, live_shop, manifest_id, open_settings, set_box, toggle, window,
    };
    use super::*;
    use apiforge_project::HolderRole;
    use serde_json::{Value, json};
    use std::path::Path;

    /// Criterion 4: the line under a ticked box states the reach.
    const SENTENCE: &str =
        "On — the agent may send real requests to any URL the project's documents name.";
    /// Longer than the second within which the agent sees a change (MCP-09
    /// criterion 3).
    const WAIT: Duration = Duration::from_millis(1500);

    fn off() -> Value {
        json!({ "allowed": false, "source": "off in the project's settings" })
    }

    fn on() -> Value {
        json!({ "allowed": true, "source": "window setting" })
    }

    /// The project's consent as kept in the preferences directory; `None`
    /// when nothing was ever kept for it.
    fn stored_live(configuration: &Path, root: &Path) -> Option<bool> {
        let bytes = fs::read(consent_file(configuration)).ok()?;
        let stored: Value = serde_json::from_slice(&bytes).unwrap();
        stored["projects"][manifest_id(root)]["live"].as_bool()
    }

    /// Steps the window for `how_long`, as a person would leave it open.
    fn wait(harness: &mut FolderHarness, how_long: Duration) {
        let start = Instant::now();
        while start.elapsed() < how_long {
            harness.step();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Criterion 1: with the agent connected and the window observing,
    /// ticking the box changes nothing until *Apply settings*: no *Live
    /// allowed* in the header, the server still refuses with `live_disabled`
    /// and nothing is kept. *Apply settings* is shown while observing,
    /// enabled only while the box differs from what applies.
    #[test]
    fn mcp26_criterion_1_ticking_changes_nothing_until_apply_settings() {
        let configuration = tempfile::tempdir().unwrap();
        let (directory, _api) = live_shop();
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        hand_over(&mut harness, &mut server, "The agent is connected.");
        assert!(read_only(&harness), "the window observes the agent");
        assert_eq!(live_requests(&mut server), off());

        open_settings(&mut harness);
        assert!(has_label(&harness, APPLY), "Apply settings is shown");
        assert!(!enabled(&harness, APPLY), "nothing to apply yet");
        choose(&mut harness, TOGGLE);
        assert!(toggle(&harness), "the box shows the choice");
        assert!(enabled(&harness, APPLY), "the choice can be applied");
        wait(&mut harness, WAIT);
        assert!(has_label(&harness, TOGGLE), "the dialog is still open");
        assert!(!has_label(&harness, BADGE), "no Live allowed before Apply");
        assert_eq!(live_requests(&mut server), off(), "the server was not told");
        assert_refused_in_settings(&live_run(&mut harness, &mut server));
        assert_eq!(
            stored_live(configuration.path(), root),
            None,
            "nothing kept"
        );

        choose(&mut harness, TOGGLE);
        assert!(!toggle(&harness));
        assert!(
            !enabled(&harness, APPLY),
            "back as it applies: nothing to apply"
        );
        choose(&mut harness, "Cancel");
        drop(harness);
        drop(server);
    }

    /// Criterion 2: *Cancel* after changing the box closes the dialog with
    /// the box and the header as they were, observing or not, from off and
    /// from on; the dialog's *Close* does the same.
    #[test]
    fn mcp26_criterion_2_cancel_leaves_the_box_and_the_header_as_they_were() {
        for observing in [false, true] {
            for live in [false, true] {
                for leave in ["Cancel", "Close"] {
                    let case = format!("observing {observing}, live {live}, {leave}");
                    let configuration = tempfile::tempdir().unwrap();
                    let directory = shop();
                    let root = directory.path();
                    if live {
                        let mut first = window(root, WIDE, configuration.path());
                        set_box(&mut first, true);
                        drop(first);
                    }
                    let kept = fs::read(consent_file(configuration.path())).ok();
                    let owner = observing.then(|| agent_holds(root));
                    let mut harness = window(root, WIDE, configuration.path());
                    assert_eq!(has_label(&harness, "Observing"), observing, "{case}");
                    assert_eq!(has_label(&harness, BADGE), live, "{case}");

                    open_settings(&mut harness);
                    choose(&mut harness, TOGGLE);
                    assert_eq!(toggle(&harness), !live, "{case}");
                    harness
                        .get_by_role_and_label(egui::accesskit::Role::Button, leave)
                        .click();
                    settle(&mut harness);
                    assert!(!has_label(&harness, TOGGLE), "{case}: the dialog closed");
                    assert_eq!(has_label(&harness, BADGE), live, "{case}: the header");
                    assert_eq!(
                        fs::read(consent_file(configuration.path())).ok(),
                        kept,
                        "{case}: the kept consent"
                    );
                    assert_eq!(box_ticked(&mut harness), live, "{case}: the box");
                    drop(harness);
                    drop(owner);
                }
            }
        }
    }

    /// Criterion 3: *Apply settings* applies the box: the header's badge,
    /// the server's answer and the consent file, both ways. While observing
    /// it applies the box alone; otherwise the project folder and its
    /// settings stay untouched as well.
    #[test]
    fn mcp26_criterion_3_apply_settings_applies_the_box() {
        let configuration = tempfile::tempdir().unwrap();
        let (directory, _api) = live_shop();
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        hand_over(&mut harness, &mut server, "The agent is connected.");
        assert!(read_only(&harness), "the window observes the agent");

        open_settings(&mut harness);
        choose(&mut harness, TOGGLE);
        assert!(enabled(&harness, APPLY));
        harness.get_by_label(APPLY).click();
        let applied = Instant::now();
        await_live_requests(Some(&mut harness), &mut server, applied, on());
        settle(&mut harness);
        assert!(
            !has_label(&harness, TOGGLE),
            "Apply settings closed the dialog"
        );
        assert!(has_label(&harness, BADGE), "the header says Live allowed");
        assert_eq!(stored_live(configuration.path(), root), Some(true));
        assert_sent(&live_run(&mut harness, &mut server));

        open_settings(&mut harness);
        assert!(toggle(&harness), "the box shows what applies");
        choose(&mut harness, TOGGLE);
        apply_settings(&mut harness);
        await_live_requests(Some(&mut harness), &mut server, Instant::now(), off());
        assert!(!has_label(&harness, BADGE));
        assert_eq!(stored_live(configuration.path(), root), Some(false));
        assert_refused_in_settings(&live_run(&mut harness, &mut server));
        drop(harness);
        drop(server);

        // Not observing: the box applies with the project's settings, and
        // an unchanged project leaves nothing to save.
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        let before = files(root);
        open_settings(&mut harness);
        choose(&mut harness, TOGGLE);
        assert!(!has_label(&harness, BADGE), "not before Apply");
        apply_settings(&mut harness);
        settle(&mut harness);
        assert!(has_label(&harness, BADGE));
        assert_eq!(stored_live(configuration.path(), root), Some(true));
        assert_eq!(files(root), before, "the project folder is byte-identical");
        assert!(
            !folder(&harness)
                .session
                .as_ref()
                .unwrap()
                .is_manifest_dirty(),
            "nothing to save"
        );
    }

    /// Criterion 4: under a ticked box the line states the real reach, any
    /// URL the project's documents name, with or without a base URL.
    #[test]
    fn mcp26_criterion_4_the_sentence_states_the_reach() {
        let (with_base, _api) = live_shop();
        let without_base = shop();
        let manifest = without_base.path().join("apiforge.project.json");
        let mut config: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        config["environment"]["baseUrl"] = json!("");
        fs::write(manifest, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        for root in [with_base.path(), without_base.path()] {
            let configuration = tempfile::tempdir().unwrap();
            let mut harness = window(root, WIDE, configuration.path());
            open_settings(&mut harness);
            assert!(has_label(&harness, OFF));
            choose(&mut harness, TOGGLE);
            assert!(
                has_label(&harness, SENTENCE),
                "the line under the ticked box"
            );
            apply_settings(&mut harness);
            open_settings(&mut harness);
            assert!(has_label(&harness, SENTENCE), "the line once applied");
            choose(&mut harness, "Cancel");
            drop(harness);
        }
    }
}

// ----- MCP-15: a live run from the command line asks the open window ---------

/// `apiforge run --folder … --live` with the project open in this window
/// (MCP-15, #182). The command runs as a process of its own: this test
/// executable running only [`mcp15::command_process`], which calls the
/// library function the `apiforge` binary calls (the spawned-helper
/// exception of the testing process). Its harness prints a line of its own
/// on stdout, which the tests ignore.
mod mcp15 {
    use super::mcp08b::{PRODUCT, SCENARIO, await_endpoint, await_until, contains, read_only};
    use super::*;
    use crate::testing::MINIMUM;
    use apiforge_storage::project::HolderRole;
    use serde_json::{Value, json};
    use std::{
        io::Read as _,
        path::Path,
        process::{Child, Command, ExitStatus, Stdio},
        thread::JoinHandle,
    };

    /// Tells [`command_process`] which folder to run.
    const FOLDER: &str = "APIFORGE_MCP15_TEST_FOLDER";
    const PAUSED: &str = "editing is paused here; this window follows every change";
    /// Decision 1 A: the bar names the command as typed.
    const COMMAND: &str = "apiforge run --live · since ";
    const TAKE_BACK: &str = "Take back";
    const OBSERVING: &str = "Observing";
    const PROMPT: &str = "A command asks for this project";
    const SENTENCE: &str =
        "apiforge run --live wants to send live requests from this project. Unsaved work here:";
    const SAVE_ALL: &str = "Save all and hand over";
    const NOT_NOW: &str = "Not now";
    const HANDED_OVER: &str = "The open API Forge window handed the project over for this run";
    const KEPT: &str = "The window kept the project (you chose to keep editing); nothing was sent";
    /// The bound of criteria 1 and 7: the window is the writer this soon
    /// after the command's end.
    const BOUND: Duration = Duration::from_secs(2);
    /// A scenario that passes against the demo API, with a cleanup.
    const PASSES: &str = "scenario \"MCP-15 live\" {\n  response = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  ok = assert response.status == 200\n  cleanup {\n    again = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  }\n}\n";
    /// The same against a product the demo API does not have.
    const FAILS: &str = "scenario \"MCP-15 live\" {\n  response = GET \"/shop/products/999\" { sample 200 { id: 999 } }\n  ok = assert response.status == 200\n  cleanup {\n    again = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  }\n}\n";

    /// `apiforge run --folder <FOLDER> --scenario <SCENARIO> --live`, as the
    /// binary runs it, with the binary's exit code.
    #[test]
    #[ignore = "launched by the MCP-15 tests as the apiforge run --live command"]
    fn command_process() {
        let Some(folder) = std::env::var_os(FOLDER) else {
            return;
        };
        let outcome = apiforge_mcp::command::run_live(
            Path::new(&folder),
            SCENARIO,
            Default::default(),
            &mut |runs, _| {
                for run in runs {
                    eprintln!("A live run of \"{}\" did not finish", run.scenario_name);
                }
            },
        );
        let code = match outcome {
            Ok(outcome) => {
                println!("{}", outcome.report.status.label());
                i32::from(outcome.report.status != RunStatus::Passed)
            }
            Err(message) => {
                eprintln!("{message}");
                1
            }
        };
        std::process::exit(code);
    }

    /// The command, running.
    struct Running {
        child: Child,
        stderr: Option<JoinHandle<String>>,
    }

    /// How the command ended.
    struct Ended {
        status: ExitStatus,
        /// When the window first saw the process gone.
        at: Instant,
        stderr: String,
    }

    impl Running {
        fn start(root: &Path) -> Self {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "folder::tests::acceptance_tests::mcp15::command_process",
                    "--nocapture",
                    "--quiet",
                    "--test-threads=1",
                ])
                .env(FOLDER, root)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let mut stderr = child.stderr.take().unwrap();
            let stderr = std::thread::spawn(move || {
                let mut text = String::new();
                let _ = stderr.read_to_string(&mut text);
                text
            });
            Self {
                child,
                stderr: Some(stderr),
            }
        }

        /// Steps the window in real time until the command ends, calling
        /// `frame` after every frame.
        fn wait(
            mut self,
            harness: &mut FolderHarness,
            mut frame: impl FnMut(&FolderHarness),
        ) -> Ended {
            let deadline = Instant::now() + Duration::from_secs(60);
            let status = loop {
                harness.step();
                frame(harness);
                if let Some(status) = self.child.try_wait().unwrap() {
                    break status;
                }
                assert!(Instant::now() < deadline, "the command never ended");
                std::thread::sleep(Duration::from_millis(5));
            };
            let at = Instant::now();
            let stderr = self.stderr.take().unwrap().join().unwrap();
            Ended { status, at, stderr }
        }

        /// Kills the command, as Ctrl+C does.
        fn kill(mut self) -> Instant {
            self.child.kill().unwrap();
            self.child.wait().unwrap();
            Instant::now()
        }
    }

    impl Drop for Running {
        /// A test that fails half-way leaves no command holding a project.
        fn drop(&mut self) {
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }

    /// The demo shop pointed at `base_url`, its scenario `scenario`.
    fn live_shop(base_url: &str, scenario: &str) -> tempfile::TempDir {
        let directory = shop();
        let root = directory.path();
        let manifest = root.join("apiforge.project.json");
        let mut config: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        config["environment"]["baseUrl"] = json!(base_url);
        fs::write(manifest, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        fs::write(root.join(SCENARIO), scenario).unwrap();
        directory
    }

    fn api() -> apiforge_demo_api::DemoServerHandle {
        apiforge_demo_api::spawn("127.0.0.1:0".parse().unwrap()).unwrap()
    }

    /// What the window showed while the command held the project.
    #[derive(Default)]
    struct Shown {
        /// A frame showed the command's bar, the badge and the pause.
        bar: bool,
        /// A frame showed *Take back*.
        take_back: bool,
    }

    impl Shown {
        fn look(&mut self, harness: &FolderHarness) {
            if folder(harness).observing() {
                self.bar |= contains(harness, COMMAND)
                    && contains(harness, PAUSED)
                    && has_label(harness, OBSERVING);
            }
            self.take_back |= has_label(harness, TAKE_BACK);
        }
    }

    /// The document registered at `path`.
    fn at(harness: &FolderHarness, path: &str) -> String {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == path)
            .unwrap_or_else(|| panic!("{path} is registered"))
            .id()
            .to_owned()
    }

    /// Unsaved text in the scenario and the component; returns both texts.
    fn leave_unsaved_code(harness: &mut FolderHarness, root: &Path) -> (String, String) {
        let scenario = at(harness, SCENARIO);
        let product = at(harness, PRODUCT);
        let scenario_text = format!(
            "{}// typed by the person\n",
            fs::read_to_string(root.join(SCENARIO)).unwrap()
        );
        let product_text = format!(
            "{}// typed by the person\n",
            fs::read_to_string(root.join(PRODUCT)).unwrap()
        );
        let session = app(harness).session.as_mut().unwrap();
        session
            .edit_source(&scenario, &scenario_text, CODE)
            .unwrap();
        session.edit_source(&product, &product_text, CODE).unwrap();
        settle(harness);
        (scenario_text, product_text)
    }

    /// Steps the window until the prompt shows.
    fn await_prompt(harness: &mut FolderHarness, running: &mut Running) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !has_label(harness, PROMPT) {
            assert!(
                running.child.try_wait().unwrap().is_none(),
                "the command ended before the window asked"
            );
            assert!(Instant::now() < deadline, "the window never asked");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(harness);
    }

    /// Criterion 1: a clean window hands the project over at once; the bar
    /// names the command, the badge says *Observing*, *Take back* is never
    /// offered; the run passes; within two seconds of the command's exit
    /// the window is the writer again and a save there succeeds.
    #[test]
    fn mcp15_criterion_1_a_clean_window_hands_over_and_takes_the_project_back() {
        let api = api();
        let directory = live_shop(&api.base_url(), PASSES);
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut shown = Shown::default();
        let ended = Running::start(root).wait(&mut harness, |harness| shown.look(harness));
        assert_eq!(ended.status.code(), Some(0), "{}", ended.stderr);
        assert!(ended.stderr.contains(HANDED_OVER), "{}", ended.stderr);
        assert!(shown.bar, "the window showed the command's bar");
        assert!(!shown.take_back, "no Take back for a command");
        let took = await_until(&mut harness, ended.at, "the writer", |harness| {
            !read_only(harness)
        });
        assert!(took <= BOUND, "the window was the writer after {took:?}");
        assert!(!has_label(&harness, TAKE_BACK));
        assert!(!root.join(".apiforge/local/journal.json").exists());

        // A save after the take-back meets no conflict.
        let scenario = only(&harness, DocumentKind::Scenario);
        let label = editor(&harness, &scenario);
        type_into(&mut harness, &label, "// the window's own change\n");
        settle(&mut harness);
        choose(&mut harness, "Save");
        let document = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap();
        assert!(!document.is_dirty());
        assert!(document.conflict().is_none());
        assert!(
            fs::read_to_string(root.join(SCENARIO))
                .unwrap()
                .contains("// the window's own change")
        );
    }

    /// Criterion 2, *Save all and hand over*: the prompt names the command
    /// and lists both documents; the person's work is saved, the run
    /// proceeds and passes, and the window is the writer again.
    #[test]
    fn mcp15_criterion_2_save_all_saves_and_the_run_proceeds() {
        let api = api();
        let directory = live_shop(&api.base_url(), PASSES);
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let (scenario_text, product_text) = leave_unsaved_code(&mut harness, root);
        let mut running = Running::start(root);
        await_prompt(&mut harness, &mut running);
        assert!(
            has_label(&harness, SENTENCE),
            "the prompt names the command"
        );
        for path in [SCENARIO, PRODUCT] {
            let item = format!("{path} — unsaved code");
            assert!(has_label(&harness, &item), "the prompt lists {item:?}");
        }
        choose(&mut harness, SAVE_ALL);
        let ended = running.wait(&mut harness, |_| {});
        assert_eq!(ended.status.code(), Some(0), "{}", ended.stderr);
        assert_eq!(
            fs::read_to_string(root.join(SCENARIO)).unwrap(),
            scenario_text
        );
        assert_eq!(
            fs::read_to_string(root.join(PRODUCT)).unwrap(),
            product_text
        );
        let took = await_until(&mut harness, ended.at, "the writer", |harness| {
            !read_only(harness)
        });
        assert!(took <= BOUND, "{took:?}");
    }

    /// Criterion 2, *Not now*: the command exits 1 with the person's choice
    /// and sends nothing; the window stays the writer, the text untouched.
    #[test]
    fn mcp15_criterion_2_not_now_keeps_editing_and_sends_nothing() {
        // An API that counts the connections it is offered.
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        silent.set_nonblocking(true).unwrap();
        let directory = live_shop(
            &format!("http://{}/demo", silent.local_addr().unwrap()),
            PASSES,
        );
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let saved = (
            fs::read(root.join(SCENARIO)).unwrap(),
            fs::read(root.join(PRODUCT)).unwrap(),
        );
        let (scenario_text, product_text) = leave_unsaved_code(&mut harness, root);
        let mut running = Running::start(root);
        await_prompt(&mut harness, &mut running);
        choose(&mut harness, NOT_NOW);
        let ended = running.wait(&mut harness, |_| {});
        assert_eq!(ended.status.code(), Some(1), "{}", ended.stderr);
        assert!(ended.stderr.contains(KEPT), "{}", ended.stderr);
        assert!(silent.accept().is_err(), "nothing was sent");
        settle(&mut harness);
        assert!(!read_only(&harness), "the window stays the writer");
        assert!(!has_label(&harness, PROMPT));
        assert_eq!(source_of(&harness, &at(&harness, SCENARIO)), scenario_text);
        assert_eq!(source_of(&harness, &at(&harness, PRODUCT)), product_text);
        assert_eq!(fs::read(root.join(SCENARIO)).unwrap(), saved.0);
        assert_eq!(fs::read(root.join(PRODUCT)).unwrap(), saved.1);
        assert!(!root.join(".apiforge/local/journal.json").exists());
    }

    /// After *Not now* the bar does not say *An agent asked to edit this
    /// project*: no agent asked, and the command has its answer and has
    /// ended (MCP-15, the bar of MCP-08b is an agent's).
    #[test]
    fn not_now_for_a_command_leaves_no_agent_wording_in_the_bar() {
        let directory = live_shop("http://127.0.0.1:9/demo", PASSES);
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        leave_unsaved_code(&mut harness, root);
        let mut running = Running::start(root);
        await_prompt(&mut harness, &mut running);
        choose(&mut harness, NOT_NOW);
        let ended = running.wait(&mut harness, |_| {});
        assert_eq!(ended.status.code(), Some(1), "{}", ended.stderr);
        settle(&mut harness);
        assert!(!contains(&harness, "An agent asked"));
        assert!(
            !has_label(&harness, "Project holder"),
            "no bar while writing"
        );
    }

    /// While the command's live run is in progress the bar keeps mock-up
    /// (a): the command's words in full, and no status line worded for an
    /// agent (MCP-10's *Agent: running …* is an agent's).
    #[test]
    fn the_bar_of_a_running_command_keeps_its_words_and_no_agent_status() {
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        silent.set_nonblocking(true).unwrap();
        let directory = live_shop(
            &format!("http://{}/demo", silent.local_addr().unwrap()),
            PASSES,
        );
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let running = Running::start(root);
        let deadline = Instant::now() + Duration::from_secs(30);
        let _held = loop {
            match silent.accept() {
                Ok((stream, _)) => break stream,
                Err(_) => {
                    assert!(Instant::now() < deadline, "no request was sent");
                    harness.step();
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        };
        // The run has started and told the window so.
        for _ in 0..40 {
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(read_only(&harness));
        assert!(contains(&harness, COMMAND) && contains(&harness, PAUSED));
        assert!(!contains(&harness, "Agent:"));
        assert!(!has_label(&harness, TAKE_BACK));
        running.kill();
    }

    /// A command killed while the prompt is open takes its prompt with it;
    /// nothing is left to answer (review of #184).
    #[test]
    fn a_command_that_stops_waiting_closes_its_prompt() {
        let directory = live_shop("http://127.0.0.1:9/demo", PASSES);
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let (scenario_text, _) = leave_unsaved_code(&mut harness, root);
        let mut running = Running::start(root);
        await_prompt(&mut harness, &mut running);
        let killed = running.kill();
        await_until(&mut harness, killed, "the prompt closed", |harness| {
            !has_label(harness, PROMPT)
        });
        assert!(!read_only(&harness), "the window keeps the project");
        assert_eq!(source_of(&harness, &at(&harness, SCENARIO)), scenario_text);
        assert!(!folder(&harness).handover.has_declined());
    }

    /// An answer given to a command that has gone is nobody's: the next
    /// agent request is not refused with it (review of #184).
    #[test]
    fn an_answer_for_a_command_that_has_gone_is_not_kept_for_an_agent() {
        let directory = live_shop("http://127.0.0.1:9/demo", PASSES);
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        leave_unsaved_code(&mut harness, root);
        app(&mut harness).prompt = Some(Prompt::Handover {
            client: Some("run --live".to_owned()),
            role: HolderRole::Command,
        });
        settle(&mut harness);
        choose(&mut harness, NOT_NOW);
        assert!(!has_label(&harness, PROMPT));
        assert!(!folder(&harness).handover.has_declined());
        assert!(!contains(&harness, "An agent asked"));
    }

    /// Criterion 7: a run that passes and one that fails both end with the
    /// window the writer within two seconds.
    #[test]
    fn mcp15_criterion_7_a_passed_and_a_failed_run_give_the_project_back() {
        let api = api();
        for (scenario, code) in [(PASSES, 0), (FAILS, 1)] {
            let directory = live_shop(&api.base_url(), scenario);
            let root = directory.path();
            let mut harness = folder_harness(root, WIDE);
            await_endpoint(&mut harness, root, HolderRole::Window);
            let ended = Running::start(root).wait(&mut harness, |_| {});
            assert_eq!(ended.status.code(), Some(code), "{}", ended.stderr);
            assert!(ended.stderr.contains(HANDED_OVER), "{}", ended.stderr);
            let took = await_until(&mut harness, ended.at, "the writer", |harness| {
                !read_only(harness)
            });
            assert!(took <= BOUND, "{took:?}");
            drop(harness);
        }
    }

    /// Criterion 7: the command killed mid-run, its request on its way to
    /// an API that never answers, leaves its journal entry; the window takes
    /// the project back by its check and reports the interrupted run, as at
    /// start.
    #[test]
    fn mcp15_criterion_7_a_killed_command_is_taken_back_with_its_interrupted_run() {
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let directory = live_shop(
            &format!("http://{}/demo", silent.local_addr().unwrap()),
            PASSES,
        );
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let running = Running::start(root);
        silent.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let _held = loop {
            match silent.accept() {
                Ok((stream, _)) => break stream,
                Err(_) => {
                    assert!(Instant::now() < deadline, "no request was sent");
                    harness.step();
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        };
        assert!(read_only(&harness), "the window observes the run");
        assert!(!has_label(&harness, "Mark as handled"));
        let killed = running.kill();
        await_until(&mut harness, killed, "the writer", |harness| {
            !read_only(harness)
        });
        assert!(root.join(".apiforge/local/journal.json").exists());
        assert!(
            has_label(&harness, "Mark as handled"),
            "the interrupted run is reported"
        );
    }

    /// 10:42 on this machine's clock, as the holder file spells it.
    fn since(hour: u32, minute: u32) -> String {
        use chrono::TimeZone;
        chrono::Local
            .with_ymd_and_hms(2026, 11, 1, hour, minute, 11)
            .single()
            .unwrap()
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    }

    /// Design review beside mock-up (a): the bar while `apiforge run --live`
    /// holds the project, in Code at 960 × 540 and 1600 × 1000.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp15_command_bar() {
        use apiforge_storage::project::{Holder, ProjectStore};
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = shop();
            let root = directory.path();
            let holder = Holder::command("run --live").with_since(since(10, 42));
            let owner = ProjectStore::open_exclusive_as(root, &holder).unwrap();
            let mut harness = folder_harness(root, size);
            choose(&mut harness, "Code");
            assert!(has_label(&harness, &format!("{COMMAND}10:42 · {PAUSED}")));
            assert!(has_label(&harness, OBSERVING));
            assert!(!has_label(&harness, TAKE_BACK));
            crate::testing::capture(
                &mut harness,
                &format!("mcp15-command-bar-code-{suffix}"),
                None,
            );
            drop(harness);
            drop(owner);
        }
    }

    /// Design review beside mock-up (b): the hand-over prompt when the
    /// command asks and two documents hold unsaved text, at 960 × 540.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp15_command_prompt() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        choose(&mut harness, "Code");
        // Typed values, which leave no warning in *Problems*, as in (b).
        for (path, from, to) in [
            (SCENARIO, "80 * 1", "80 * 2"),
            (PRODUCT, "number = 123", "number = 124"),
        ] {
            let id = at(&harness, path);
            let text = fs::read_to_string(root.join(path))
                .unwrap()
                .replacen(from, to, 1);
            app(&mut harness)
                .session
                .as_mut()
                .unwrap()
                .edit_source(&id, &text, CODE)
                .unwrap();
        }
        settle(&mut harness);
        app(&mut harness).prompt = Some(Prompt::Handover {
            client: Some("run --live".to_owned()),
            role: HolderRole::Command,
        });
        settle(&mut harness);
        assert!(has_label(&harness, PROMPT));
        assert!(has_label(&harness, SENTENCE));
        crate::testing::capture(&mut harness, "mcp15-command-prompt-960x540", None);
    }
}

// ----- MCP-10: the agent's run status in the bottom bar ----------------------

mod mcp10 {
    use super::mcp08b::{
        AGENT, SCENARIO, address, answer, await_endpoint, await_until, call, commented, contains,
        holder, label_containing, read, read_only, write,
    };
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use apiforge_mcp::test_support::{StdioServer, structured};
    use apiforge_storage::project::HolderRole;
    use serde_json::{Value, json};
    use std::{
        io::{BufRead as _, BufReader},
        path::{Path, PathBuf},
        process::Command,
        sync::mpsc::{Receiver, channel},
    };

    /// Tells [`server_process`] which folder to serve.
    const FOLDER: &str = "APIFORGE_RUN_STATUS_TEST_FOLDER";
    /// Tells [`server_process`] to start as `--allow-live` does.
    const ALLOW_LIVE: &str = "APIFORGE_RUN_STATUS_TEST_ALLOW_LIVE";
    const STEP_DELAY: &str = "APIFORGE_MCP_TEST_STEP_DELAY_MS";
    const FILE: &str = "promotion-and-cart-price.afs";
    const BAR: &str = "Project holder";
    const TAKE_BACK: &str = "Take back";
    const PAUSED: &str = "editing is paused here; this window follows every change";
    /// Every status line starts so, and nothing else in the window does.
    const STATUS: &str = "Agent: ";
    /// The ticket's bound: the line shows within a second of each message.
    const BOUND: Duration = Duration::from_secs(1);
    /// What the bar must not move: the explorer and the tabs above it.
    const FIXED: [&str; 5] = ["EXPLORER", "Code", "Diagram", "Split", "Run simulation"];
    const MODES: [&str; 3] = ["Code", "Diagram", "Split"];

    fn running(mode: &str) -> String {
        format!("Agent: running {FILE} ({mode})…")
    }

    /// The MCP server as a process of its own on real stdio, as in
    /// `mod mcp08b`, optionally allowed to send live requests.
    #[test]
    #[ignore = "launched by the MCP-10 run status tests as the server process"]
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
            allow_live: std::env::var_os(ALLOW_LIVE).is_some(),
            ..Default::default()
        };
        apiforge_mcp::serve_stdio_with(PathBuf::from(folder), settings).unwrap();
    }

    /// A server initialised by *Claude Code* whose runs pause `step_delay`
    /// milliseconds after every step.
    fn agent(root: &Path, step_delay: u64, live: bool) -> StdioServer {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "folder::tests::acceptance_tests::mcp10::server_process",
                "--nocapture",
                "--quiet",
                "--test-threads=1",
            ])
            .env(FOLDER, root)
            .env(STEP_DELAY, step_delay.to_string());
        if live {
            command.env(ALLOW_LIVE, "1");
        }
        let mut server = StdioServer::spawn(command);
        server.initialize("Claude Code");
        server
    }

    /// The agent writes the scenario with `note` added; a clean window
    /// hands the project over and observes.
    fn agent_writes(harness: &mut FolderHarness, server: &mut StdioServer, note: &str) {
        let (text, _) = read(server, SCENARIO);
        let written = write(harness, server, SCENARIO, &commented(&text, note));
        assert_eq!(structured(&written)["wrote"], true, "{written}");
        settle(harness);
    }

    /// The agent saves a version of the scenario whose *Promotional price*
    /// comparison fails.
    fn agent_breaks_the_price(harness: &mut FolderHarness, server: &mut StdioServer) {
        let (text, _) = read(server, SCENARIO);
        assert!(text.contains("expected_price = 80 * 1"), "{text}");
        let broken = text.replacen("expected_price = 80 * 1", "expected_price = 81 * 1", 1);
        let written = write(harness, server, SCENARIO, &broken);
        assert_eq!(structured(&written)["wrote"], true, "{written}");
        settle(harness);
    }

    /// The first step that did not pass, numbered as `step_count` counts:
    /// every step in order, a component's own steps included.
    fn first_failure(steps: &[Value]) -> Option<(u64, String)> {
        fn walk(steps: &[Value], seen: &mut u64) -> Option<(u64, String)> {
            for step in steps {
                *seen += 1;
                if step["status"] == "failed" || step["status"] == "error" {
                    let label = step["label"].as_str().or(step["handle"].as_str());
                    return Some((*seen, label.unwrap().to_owned()));
                }
                if let Some(found) = step["steps"].as_array().and_then(|inner| walk(inner, seen)) {
                    return Some(found);
                }
            }
            None
        }
        walk(steps, &mut 0)
    }

    /// Connects to `endpoint` as a window does and hands over every line
    /// it is told, as JSON.
    fn observe(endpoint: &str) -> Receiver<Value> {
        #[cfg(unix)]
        let stream = std::os::unix::net::UnixStream::connect(address(endpoint)).unwrap();
        #[cfg(windows)]
        let stream = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(address(endpoint))
            .unwrap();
        let (sender, messages) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        messages
    }

    /// The next message of type `kind`, the others passed over.
    fn next(messages: &Receiver<Value>, kind: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = messages
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("the window was told no {kind} message"));
            if message["type"] == kind {
                return message;
            }
        }
    }

    /// `label` is `prefix` followed by a time on the wall clock, `HH:MM`.
    fn assert_clock(label: &str, prefix: &str) {
        let time = label
            .strip_prefix(prefix)
            .unwrap_or_else(|| panic!("{label:?} starts with {prefix:?}"));
        let digits = time.char_indices().all(|(at, character)| {
            (at == 2) == (character == ':') && (at == 2 || character.is_ascii_digit())
        });
        assert!(time.len() == 5 && digits, "{label:?} ends in HH:MM");
    }

    /// Where the explorer and the tabs are; a mode without the explorer
    /// (Diagram shows the library there) has none.
    fn fixed_rects(harness: &FolderHarness) -> Vec<Option<egui::Rect>> {
        FIXED
            .iter()
            .map(|label| harness.query_by_label(label).map(|node| node.rect()))
            .collect()
    }

    /// Waits until the bar names the agent with its start time, as the
    /// holder file gives it.
    fn await_since(harness: &mut FolderHarness) {
        await_until(harness, Instant::now(), "the holder's start", |harness| {
            contains(harness, &format!("{AGENT} · since "))
        });
    }

    /// Criterion 1: the server tells a connected window `run` with the
    /// path, the mode and `running` when a run starts, and at its end
    /// `passed`, `failed` with the step and its label, or `cancelled`.
    #[test]
    fn mcp10_criterion_1_the_server_tells_a_window_each_runs_start_and_end() {
        let directory = shop();
        let root = directory.path();
        let mut server = agent(root, 100, false);
        // The agent takes the project with no window open; a window then
        // connects to the server's endpoint, as one opened meanwhile does.
        let (text, hash) = read(&mut server, SCENARIO);
        let written = server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": commented(&text, "First."), "expected_source_sha256": hash }),
        );
        assert_eq!(structured(&written)["wrote"], true, "{written}");
        let endpoint = holder(root)
            .and_then(|holder| holder.endpoint().map(str::to_owned))
            .expect("the server names its endpoint");
        let messages = observe(&endpoint);
        // The next commit is told to the window: it hears everything after.
        let (text, hash) = read(&mut server, SCENARIO);
        server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": commented(&text, "Second."), "expected_source_sha256": hash }),
        );
        next(&messages, "saved");

        let report = server.call("run", json!({ "path": SCENARIO }));
        let report = structured(&report).clone();
        assert_eq!(report["status"], "passed", "{report}");
        assert_eq!(
            next(&messages, "run"),
            json!({ "version": 1, "type": "run", "path": SCENARIO, "mode": "simulation", "state": "running" })
        );
        assert_eq!(
            next(&messages, "run"),
            json!({
                "version": 1, "type": "run", "path": SCENARIO, "mode": "simulation",
                "state": "passed", "step_count": report["step_count"]
            })
        );

        let (text, hash) = read(&mut server, SCENARIO);
        let broken = text.replacen("expected_price = 80 * 1", "expected_price = 81 * 1", 1);
        assert_ne!(broken, text);
        server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": broken, "expected_source_sha256": hash }),
        );
        let report = server.call("run", json!({ "path": SCENARIO }));
        let report = structured(&report).clone();
        assert_eq!(report["status"], "failed", "{report}");
        let (step, label) = first_failure(report["steps"].as_array().unwrap()).unwrap();
        assert_eq!(label, "Promotional price", "{report}");
        assert_eq!(next(&messages, "run")["state"], "running");
        assert_eq!(
            next(&messages, "run"),
            json!({
                "version": 1, "type": "run", "path": SCENARIO, "mode": "simulation",
                "state": "failed", "step": step, "label": label,
                "step_count": report["step_count"]
            })
        );

        let id = server.start_call("run", json!({ "path": SCENARIO }));
        assert_eq!(next(&messages, "run")["state"], "running");
        server.send(
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/cancelled",
                "params": { "requestId": id }
            })
            .to_string(),
        );
        let ended = next(&messages, "run");
        assert_eq!(ended["state"], "cancelled", "{ended}");
        assert_eq!(ended["path"], SCENARIO);
        assert_eq!(ended["mode"], "simulation");
        drop(server);
    }

    /// A window that connects while a run is going is told that it runs,
    /// then how it ended, like the windows connected before it.
    #[test]
    fn a_window_that_connects_during_a_run_hears_that_it_runs() {
        let directory = shop();
        let root = directory.path();
        let mut server = agent(root, 200, false);
        let (text, hash) = read(&mut server, SCENARIO);
        let written = server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": commented(&text, "First."), "expected_source_sha256": hash }),
        );
        assert_eq!(structured(&written)["wrote"], true, "{written}");
        let endpoint = holder(root)
            .and_then(|holder| holder.endpoint().map(str::to_owned))
            .expect("the server names its endpoint");
        let first = observe(&endpoint);
        let (text, hash) = read(&mut server, SCENARIO);
        server.call(
            "write_source",
            json!({ "path": SCENARIO, "text": commented(&text, "Second."), "expected_source_sha256": hash }),
        );
        next(&first, "saved");
        let id = server.start_call("run", json!({ "path": SCENARIO }));
        assert_eq!(next(&first, "run")["state"], "running");

        let late = observe(&endpoint);
        let told = next(&late, "run");
        assert_eq!(told["state"], "running", "{told}");
        assert_eq!(told["path"], SCENARIO);
        let report = server.wait(id);
        let ended = next(&late, "run");
        assert_eq!(ended["state"], "passed", "{ended}");
        assert_eq!(ended["step_count"], structured(&report)["step_count"]);
        assert_eq!(next(&first, "run"), ended);
        drop(server);
    }

    /// Criterion 1: the observing window shows the line within a second of
    /// each message: running, then passed, failed or cancelled with the
    /// time the run ended.
    #[test]
    fn mcp10_criterion_1_the_window_shows_the_line_within_a_second_of_each_message() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, 200, false);
        agent_writes(&mut harness, &mut server, "Written by the agent.");
        assert!(!contains(&harness, STATUS), "no run, no status");

        let started = Instant::now();
        let id = server.start_call("run", json!({ "path": SCENARIO }));
        let took = await_until(&mut harness, started, "the running line", |harness| {
            has_label(harness, &running("simulation"))
        });
        assert!(took < BOUND, "running shown {took:?} after the run started");
        let report = answer(&mut harness, &mut server, id);
        let ended = Instant::now();
        let report = structured(&report).clone();
        assert_eq!(report["status"], "passed", "{report}");
        let passed = format!("Agent: {FILE} passed · {} steps · ", report["step_count"]);
        let took = await_until(&mut harness, ended, "the passed line", |harness| {
            contains(harness, &passed)
        });
        assert!(took < BOUND, "passed shown {took:?} after the run ended");
        assert_clock(&label_containing(&harness, &passed), &passed);
        assert!(!has_label(&harness, &running("simulation")));

        agent_breaks_the_price(&mut harness, &mut server);
        let report = call(
            &mut harness,
            &mut server,
            "run",
            json!({ "path": SCENARIO }),
        );
        let ended = Instant::now();
        let report = structured(&report).clone();
        let (step, label) = first_failure(report["steps"].as_array().unwrap()).unwrap();
        let failed = format!("Agent: {FILE} failed at step {step} · {label} · ");
        let took = await_until(&mut harness, ended, "the failed line", |harness| {
            contains(harness, &failed)
        });
        assert!(took < BOUND, "failed shown {took:?} after the run ended");
        assert_clock(&label_containing(&harness, &failed), &failed);

        let id = server.start_call("run", json!({ "path": SCENARIO }));
        await_until(
            &mut harness,
            Instant::now(),
            "the running line",
            |harness| has_label(harness, &running("simulation")),
        );
        server.send(
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/cancelled",
                "params": { "requestId": id }
            })
            .to_string(),
        );
        let cancelled = format!("Agent: {FILE} cancelled · ");
        let took = await_until(
            &mut harness,
            Instant::now(),
            "the cancelled line",
            |harness| contains(harness, &cancelled),
        );
        // Measured from the cancellation, not from the message: the run
        // stops at its next step (200 ms here) and runs its cleanup first.
        assert!(
            took < 2 * BOUND,
            "cancelled shown {took:?} after the cancellation"
        );
        assert_clock(&label_containing(&harness, &cancelled), &cancelled);
        drop(harness);
        drop(server);
    }

    /// Criterion 2: the line names the mode: *(simulation)* above, *(live)*
    /// for a live run, while it runs and once it has ended.
    #[test]
    fn mcp10_criterion_2_a_live_run_says_live() {
        let directory = shop();
        let root = directory.path();
        // Since MCP-09 the open window decides live runs: the person
        // allows them in *Project settings* first.
        let configuration = tempfile::tempdir().unwrap();
        let mut harness = super::mcp09::window(root, WIDE, configuration.path());
        super::mcp09::set_box(&mut harness, true);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, 300, true);
        // The live run takes the project for the agent session itself: the
        // clean window hands it over and observes.
        let started = Instant::now();
        let id = server.start_call("run", json!({ "path": SCENARIO, "live": true }));
        await_until(&mut harness, started, "the live running line", |harness| {
            has_label(harness, &running("live"))
        });
        assert!(read_only(&harness));
        let report = answer(&mut harness, &mut server, id);
        let report = structured(&report).clone();
        assert_eq!(report["live"], true, "{report}");
        let ended = format!(
            "Agent: {FILE} (live) {} ",
            report["status"].as_str().unwrap()
        );
        await_until(&mut harness, Instant::now(), "the live end", |harness| {
            contains(harness, &ended)
        });
        assert!(!contains(&harness, "(simulation)"));
        drop(harness);
        drop(server);
    }

    /// The bar at 960 × 540 in every mode with `status` shown: the status
    /// whole and inside the bar, the holder text shortened beside it, the
    /// explorer and the tabs where they were before.
    fn assert_status_fits(
        harness: &mut FolderHarness,
        status: &str,
        before: &[Vec<Option<egui::Rect>>],
    ) {
        for (mode, before) in MODES.iter().zip(before) {
            choose(harness, mode);
            assert_within_window(harness, &[BAR, status, TAKE_BACK]);
            let shown = harness.get_by_label(status).rect();
            assert!(
                harness.get_by_label(BAR).rect().contains_rect(shown),
                "{mode}: {status:?} sits in the bar"
            );
            let whole = whole_width(harness, status, theme::FONT_META);
            assert!(
                shown.width() + 1.0 >= whole,
                "{mode}: {status:?} is cut: {} of {whole} px",
                shown.width()
            );
            let holder = label_containing(harness, "Claude Code");
            assert!(
                holder.starts_with("Claude Code · since ") && !holder.contains(PAUSED),
                "{mode}: the holder text gives way to the status: {holder:?}"
            );
            assert_disjoint(harness, &[holder.as_str(), status, TAKE_BACK]);
            assert_eq!(
                &fixed_rects(harness),
                before,
                "{mode}: the bar moved the explorer or the tabs"
            );
        }
    }

    /// Criterion 3: at 960 × 540 in Code, Diagram and Split the status is
    /// never cut; the holder text is shortened instead; nothing above the
    /// bar moves.
    #[test]
    fn mcp10_criterion_3_the_status_is_whole_at_960_by_540_and_the_holder_gives_way() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, 300, false);
        agent_writes(&mut harness, &mut server, "Written by the agent.");
        await_since(&mut harness);
        let before: Vec<Vec<Option<egui::Rect>>> = MODES
            .iter()
            .map(|mode| {
                choose(&mut harness, mode);
                fixed_rects(&harness)
            })
            .collect();
        assert!(before[0][0].is_some(), "Code shows the explorer");

        let id = server.start_call("run", json!({ "path": SCENARIO }));
        await_until(
            &mut harness,
            Instant::now(),
            "the running line",
            |harness| has_label(harness, &running("simulation")),
        );
        assert_status_fits(&mut harness, &running("simulation"), &before);

        let report = answer(&mut harness, &mut server, id);
        let passed = format!(
            "Agent: {FILE} passed · {} steps · ",
            structured(&report)["step_count"]
        );
        await_until(&mut harness, Instant::now(), "the passed line", |harness| {
            contains(harness, &passed)
        });
        let passed = label_containing(&harness, &passed);
        assert_status_fits(&mut harness, &passed, &before);

        agent_breaks_the_price(&mut harness, &mut server);
        call(
            &mut harness,
            &mut server,
            "run",
            json!({ "path": SCENARIO }),
        );
        await_until(&mut harness, Instant::now(), "the failed line", |harness| {
            contains(harness, &format!("Agent: {FILE} failed at step "))
        });
        let failed = label_containing(&harness, &format!("Agent: {FILE} failed at step "));
        assert_status_fits(&mut harness, &failed, &before);
        drop(harness);
        drop(server);
    }

    /// MCP-19 criterion 4: the agent writes ten times while the observing
    /// window checks the folder as often as it can, so that on Windows its
    /// reads meet the agent's replaces. Every write is written, none asks
    /// for recovery, and the window follows each one.
    #[test]
    fn mcp19_criterion_4_ten_agent_writes_beside_a_window_that_checks_continuously() {
        const WRITES: usize = 10;
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, 0, false);
        agent_writes(&mut harness, &mut server, "Handed over.");
        assert!(read_only(&harness), "the window observes the agent");
        let scenario = only(&harness, DocumentKind::Scenario);
        // A check starts at every frame where none is running.
        app(&mut harness).watcher.set_interval(Duration::ZERO);
        for round in 1..=WRITES {
            let (text, _) = read(&mut server, SCENARIO);
            let written = commented(&text, &format!("Write {round} of {WRITES}."));
            let answer = write(&mut harness, &mut server, SCENARIO, &written);
            assert_eq!(
                structured(&answer)["wrote"],
                true,
                "write {round}: {answer}"
            );
            let since = Instant::now();
            while source_of(&harness, &scenario) != written {
                assert!(
                    !folder(&harness).watcher.recovery_required,
                    "write {round}: the window asks for recovery"
                );
                assert!(
                    since.elapsed() < Duration::from_secs(15),
                    "write {round}: the window never followed it"
                );
                harness.step();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                !folder(&harness).watcher.recovery_required,
                "write {round}: the window asks for recovery"
            );
            assert!(
                contains(&harness, PAUSED),
                "write {round}: the bar no longer says the window follows"
            );
        }
        app(&mut harness)
            .watcher
            .set_interval(crate::folder::watcher::POLL_INTERVAL);
        settle(&mut harness);
        drop(harness);
        drop(server);
    }

    /// Criterion 4: the status goes when the agent finishes, and when the
    /// channel drops in the middle of a run.
    #[test]
    fn mcp10_criterion_4_the_status_goes_with_the_agent_and_with_the_channel() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, 200, false);
        agent_writes(&mut harness, &mut server, "First.");
        let report = call(
            &mut harness,
            &mut server,
            "run",
            json!({ "path": SCENARIO }),
        );
        let passed = format!(
            "Agent: {FILE} passed · {} steps · ",
            structured(&report)["step_count"]
        );
        await_until(&mut harness, Instant::now(), "the passed line", |harness| {
            contains(harness, &passed)
        });
        let finished = call(&mut harness, &mut server, "finish_editing", json!({}));
        assert_eq!(structured(&finished)["finished"], true, "{finished}");
        await_until(&mut harness, Instant::now(), "the take-back", |harness| {
            !read_only(harness)
        });
        assert!(!contains(&harness, STATUS), "the agent finished");

        agent_writes(&mut harness, &mut server, "Second.");
        assert!(read_only(&harness));
        assert!(!contains(&harness, STATUS), "a new session starts clean");
        server.start_call("run", json!({ "path": SCENARIO }));
        await_until(
            &mut harness,
            Instant::now(),
            "the running line",
            |harness| has_label(harness, &running("simulation")),
        );
        let dropped = Instant::now();
        server.kill();
        await_until(&mut harness, dropped, "the status gone", |harness| {
            !contains(harness, STATUS)
        });
        await_until(&mut harness, dropped, "the take-back", |harness| {
            !read_only(harness)
        });
        assert!(!contains(&harness, STATUS));
        drop(harness);
    }

    /// The bar beside M4 round 4: running, passed and failed, at both
    /// sizes.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp10_run_status() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = shop();
            let root = directory.path();
            let mut harness = folder_harness(root, size);
            await_endpoint(&mut harness, root, HolderRole::Window);
            let mut server = agent(root, 300, false);
            agent_writes(&mut harness, &mut server, "Written by the agent.");
            await_since(&mut harness);
            let id = server.start_call("run", json!({ "path": SCENARIO }));
            await_until(
                &mut harness,
                Instant::now(),
                "the running line",
                |harness| has_label(harness, &running("simulation")),
            );
            capture(&mut harness, &format!("mcp10-running-{suffix}"), None);
            answer(&mut harness, &mut server, id);
            await_until(&mut harness, Instant::now(), "the passed line", |harness| {
                contains(harness, &format!("Agent: {FILE} passed"))
            });
            capture(&mut harness, &format!("mcp10-passed-{suffix}"), None);
            agent_breaks_the_price(&mut harness, &mut server);
            call(
                &mut harness,
                &mut server,
                "run",
                json!({ "path": SCENARIO }),
            );
            await_until(&mut harness, Instant::now(), "the failed line", |harness| {
                contains(harness, &format!("Agent: {FILE} failed"))
            });
            capture(&mut harness, &format!("mcp10-failed-{suffix}"), None);
            drop(harness);
            drop(server);
        }
    }
}

// ----- MCP-16: the observing window follows the agent's edits ----------------

mod mcp16 {
    use super::mcp08b::{
        AGENT, NO_POLL, PRODUCT, SCENARIO, agent, agent_with, answer, await_endpoint, await_until,
        call, commented, contains, hand_over, label_containing, read, read_only, write,
    };
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use apiforge_mcp::test_support::{StdioServer, structured};
    use apiforge_project::{FixedPlacer, Holder, HolderRole};
    use egui::accesskit::Toggled;
    use egui_kittest::kittest::NodeT;
    use serde_json::json;
    use std::ops::RangeInclusive;

    /// The toggle's accessible name; it reads *Follow* on screen.
    const FOLLOW: &str = "Follow the agent";
    const PINNED_HINT: &str =
        "Follows the agent to every file it changes; click to stay where you are";
    const RELEASED_HINT: &str = "Click to follow the agent again";
    const TAKE_BACK: &str = "Take back";
    const BAR: &str = "Project holder";
    const PAUSED: &str = "editing is paused here; this window follows every change";
    const PROMOTION: &str = "components/active-promotion.afs";
    const ADDED: &str = "scenarios/added-by-agent.afs";
    const STEP_DELAY: &str = "APIFORGE_MCP_TEST_STEP_DELAY_MS";
    /// The ticket's bound: the window is at the agent's document this soon.
    const BOUND: Duration = Duration::from_secs(2);
    /// The holder's own edits in the tests whose holder is in this process.
    const HOLDER: Origin = Origin(91);
    const MODES: [&str; 3] = ["Code", "Diagram", "Split"];

    fn id_of(harness: &FolderHarness, path: &str) -> String {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == path)
            .unwrap_or_else(|| panic!("{path} is registered"))
            .id()
            .to_owned()
    }

    fn is_registered(harness: &FolderHarness, path: &str) -> bool {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .any(|document| document.entry().path.as_str() == path)
    }

    fn active(harness: &FolderHarness) -> Option<String> {
        folder(harness).workbench.active().map(str::to_owned)
    }

    fn tabs(harness: &FolderHarness) -> Vec<String> {
        folder(harness)
            .workbench
            .tabs()
            .iter()
            .map(|tab| tab.document_id.clone())
            .collect()
    }

    fn scroll_of(harness: &FolderHarness, document_id: &str) -> f32 {
        folder(harness)
            .workbench
            .tabs()
            .iter()
            .find(|tab| tab.document_id == document_id)
            .unwrap_or_else(|| panic!("{document_id} has a tab"))
            .scroll
    }

    fn mode(harness: &FolderHarness) -> Mode {
        folder(harness).workbench.mode
    }

    /// *Follow* is pressed. Panics when the bar has no toggle.
    fn pressed(harness: &FolderHarness) -> bool {
        let toggle = harness.get_by_label(FOLLOW);
        match toggle.accesskit_node().toggled() {
            Some(Toggled::True) => true,
            Some(Toggled::False) => false,
            other => panic!("{FOLLOW} is a toggle, announced {other:?}"),
        }
    }

    fn marks(harness: &FolderHarness, document_id: &str) -> Option<RangeInclusive<usize>> {
        folder(harness).changed_lines(document_id)
    }

    fn in_view(harness: &FolderHarness) -> Option<(String, RangeInclusive<usize>)> {
        folder(harness).code_lines_in_view()
    }

    /// The line, counted from one, of the first line of `text` holding
    /// `needle`.
    fn line_of(text: &str, needle: &str) -> usize {
        text.lines()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} is in the text"))
            + 1
    }

    /// `text` with `count` comment lines after its first line.
    fn padded(text: &str, count: usize) -> String {
        let (first, rest) = text.split_once('\n').unwrap();
        let pad: String = (1..=count)
            .map(|line| format!("  // Note {line} of the agent.\n"))
            .collect();
        format!("{first}\n{pad}{rest}")
    }

    /// The agent writes `path` with `note` added under its first line and
    /// returns when it started the write: the window steps while the
    /// agent waits for the answer, and may follow before it comes.
    fn agent_notes(
        harness: &mut FolderHarness,
        server: &mut StdioServer,
        path: &str,
        note: &str,
    ) -> Instant {
        let (text, _) = read(server, path);
        let asked = Instant::now();
        let answer = write(harness, server, path, &commented(&text, note));
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        asked
    }

    /// The window is at `document_id` within the ticket's bound of
    /// `returned`, the moment the agent started the write that commits.
    fn await_active(harness: &mut FolderHarness, returned: Instant, document_id: &str, what: &str) {
        let took = await_until(harness, returned, what, |harness| {
            folder(harness).workbench.active() == Some(document_id)
        });
        assert!(
            took < BOUND,
            "{what}: at the document {took:?} after the write"
        );
    }

    /// The window becomes the writer again.
    fn await_writer(harness: &mut FolderHarness) {
        await_until(harness, Instant::now(), "the take-back", |harness| {
            !read_only(harness)
        });
    }

    /// Steps until the bar's text names `what`.
    fn await_bar(harness: &mut FolderHarness, what: &str) {
        await_until(harness, Instant::now(), what, |harness| {
            contains(harness, what)
        });
    }

    /// The agent's first write hands the project over, and the window has
    /// followed that commit before the agent writes again: two commits
    /// read in one check would be followed as one.
    fn observe(harness: &mut FolderHarness, server: &mut StdioServer, note: &str) {
        hand_over(harness, server, note);
        await_bar(harness, "promotion-and-cart-price.afs saved");
    }

    /// Scrolls the code of `document_id` down with the mouse wheel.
    fn scroll_code(harness: &mut FolderHarness, document_id: &str) {
        let code = harness.get_by_label(&editor(harness, document_id)).rect();
        harness.hover_at(code.left_top() + egui::vec2(120.0, 60.0));
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -160.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        settle(harness);
    }

    /// Clicks the tab of `name` in the tab strip: the explorer's row has the
    /// same name, lower in the window.
    fn click_tab(harness: &mut FolderHarness, name: &str) {
        harness
            .query_all_by_label(name)
            .min_by(|left, right| left.rect().top().total_cmp(&right.rect().top()))
            .unwrap_or_else(|| panic!("{name} is shown"))
            .click();
        settle(harness);
    }

    /// Criterion 1: *Follow* is pressed when the window becomes the
    /// observer by a hand-over, and in a window opened while the agent
    /// holds the project.
    #[test]
    fn mcp16_criterion_1_following_is_on_when_observing_starts() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        observe(&mut harness, &mut server, "Written by the agent.");
        assert!(read_only(&harness), "the window observes");
        assert!(pressed(&harness), "a hand-over starts following");
        drop(harness);

        let mut harness = folder_harness(root, WIDE);
        assert!(read_only(&harness), "opened while the agent holds it");
        await_until(&mut harness, Instant::now(), "the connection", |harness| {
            has_label(harness, TAKE_BACK)
        });
        assert!(pressed(&harness), "a window opened meanwhile follows");
        drop(harness);
        drop(server);
    }

    /// Criterion 2, in `mode`: a saved document whose tab is not active,
    /// and one with no tab, are followed within two seconds in the view the
    /// person is in; the code shows the first changed line and marks the
    /// changed lines until the agent's next commit to that document; the
    /// board keeps its camera and its selected block.
    fn a_saved_document_is_followed(view: &str, size: egui::Vec2) {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, size);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = id_of(&harness, SCENARIO);
        assert_eq!(tabs(&harness), std::slice::from_ref(&scenario));
        choose(&mut harness, view);
        let shown = mode(&harness);
        let code = shown != Mode::Diagram;

        // The hand-over: the agent's first write pads the scenario.
        let mut server = agent(root, "Claude Code");
        let (text, _) = read(&mut server, SCENARIO);
        let padded = padded(&text, 40);
        let answer = write(&mut harness, &mut server, SCENARIO, &padded);
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        await_until(&mut harness, Instant::now(), "the padding", |harness| {
            source_of(harness, &scenario) == padded
        });
        assert!(pressed(&harness));
        assert_eq!(active(&harness).as_deref(), Some(scenario.as_str()));
        if code {
            assert_eq!(marks(&harness, &scenario), Some(2..=41));
        }
        // The person reads the board: a block selected, the camera moved.
        let mut place = None;
        if !code {
            choose(&mut harness, "Expected price (Value)");
            choose(&mut harness, "Zoom in");
            place = Some((
                folder(&harness).canvases[&scenario].camera(),
                folder(&harness).selections.get(&scenario).cloned(),
            ));
            assert!(place.as_ref().is_some_and(|(camera, selection)| {
                camera.is_some() && selection.as_ref().is_some_and(|ids| !ids.is_empty())
            }));
            assert!(pressed(&harness), "selecting and zooming keep following");
        }

        // A component with no tab: opened beside the active tab.
        let returned = agent_notes(
            &mut harness,
            &mut server,
            PROMOTION,
            "Changed by the agent.",
        );
        let promotion = id_of(&harness, PROMOTION);
        await_active(&mut harness, returned, &promotion, "the component");
        assert_eq!(tabs(&harness), [scenario.clone(), promotion.clone()]);
        assert_eq!(mode(&harness), shown, "the view is the person's");
        if code {
            assert_eq!(marks(&harness, &promotion), Some(2..=2));
            let (document, lines) = in_view(&harness).expect("the code is drawn");
            assert_eq!(document, promotion);
            assert!(lines.contains(&2), "line 2 in view: {lines:?}");
        }

        // The scenario again, its tab open but not active, far down.
        let deep = padded.replacen("@label(\"Delete cart\")", "@label(\"Delete the cart\")", 1);
        assert_ne!(deep, padded);
        let line = line_of(&deep, "Delete the cart");
        assert!(line > 50, "line {line} is out of view at first");
        let returned = Instant::now();
        let answer = write(&mut harness, &mut server, SCENARIO, &deep);
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        await_active(&mut harness, returned, &scenario, "the scenario");
        await_until(&mut harness, Instant::now(), "the new text", |harness| {
            source_of(harness, &scenario) == deep
        });
        assert_eq!(tabs(&harness), [scenario.clone(), promotion.clone()]);
        assert_eq!(mode(&harness), shown, "the view is the person's");
        if code {
            assert_eq!(
                marks(&harness, &scenario),
                Some(line..=line),
                "the earlier marks went with this commit"
            );
            assert_eq!(
                marks(&harness, &promotion),
                Some(2..=2),
                "another document's marks stay until its next commit"
            );
            let (document, lines) = in_view(&harness).expect("the code is drawn");
            assert_eq!(document, scenario);
            assert!(lines.contains(&line), "line {line} in view: {lines:?}");
        } else {
            assert_eq!(
                Some((
                    folder(&harness).canvases[&scenario].camera(),
                    folder(&harness).selections.get(&scenario).cloned(),
                )),
                place,
                "the board keeps its camera and its selected block"
            );
        }
        assert!(pressed(&harness));
        drop(harness);
        drop(server);
    }

    #[test]
    fn mcp16_criterion_2_a_saved_document_is_followed_in_code() {
        a_saved_document_is_followed("Code", MINIMUM);
    }

    #[test]
    fn mcp16_criterion_2_a_saved_document_is_followed_in_diagram() {
        a_saved_document_is_followed("Diagram", WIDE);
    }

    #[test]
    fn mcp16_criterion_2_a_saved_document_is_followed_in_split() {
        a_saved_document_is_followed("Split", MINIMUM);
    }

    /// Criterion 3: a document the agent creates is open and active within
    /// two seconds, at its top, with no marks.
    #[test]
    fn mcp16_criterion_3_a_created_document_is_followed() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = id_of(&harness, SCENARIO);
        let mut server = agent(root, "Claude Code");
        observe(&mut harness, &mut server, "Written by the agent.");
        let text = format!(
            "scenario \"Added by agent\" {{\n{}  total = 2\n}}\n",
            "  // A line of the new scenario.\n".repeat(40)
        );
        let returned = Instant::now();
        let answer = call(
            &mut harness,
            &mut server,
            "create_document",
            json!({ "kind": "scenario", "path": ADDED, "text": text }),
        );
        assert_eq!(structured(&answer)["wrote"], true, "{answer}");
        await_until(&mut harness, returned, "the new document", |harness| {
            is_registered(harness, ADDED)
        });
        let added = id_of(&harness, ADDED);
        await_active(&mut harness, returned, &added, "the created document");
        assert_eq!(tabs(&harness), [scenario, added.clone()]);
        let (document, lines) = in_view(&harness).expect("the code is drawn");
        assert_eq!(document, added);
        assert_eq!(*lines.start(), 1, "at its top: {lines:?}");
        assert_eq!(marks(&harness, &added), None, "a new document has no marks");
        assert!(pressed(&harness));
        drop(harness);
        drop(server);
    }

    /// Criterion 4: a component the agent publishes comes to the front
    /// with its new revision; its code keeps its scroll.
    #[test]
    fn mcp16_criterion_4_a_publication_is_followed() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = id_of(&harness, SCENARIO);
        let product = id_of(&harness, PRODUCT);
        app(&mut harness).activate_tab(&product);
        settle(&mut harness);
        app(&mut harness).activate_tab(&scenario);
        settle(&mut harness);
        let scroll = scroll_of(&harness, &product);
        let mut server = agent(root, "Claude Code");
        observe(&mut harness, &mut server, "Written by the agent.");
        assert_eq!(active(&harness).as_deref(), Some(scenario.as_str()));

        let (_, hash) = read(&mut server, PRODUCT);
        let returned = Instant::now();
        let answer = call(
            &mut harness,
            &mut server,
            "publish",
            json!({ "path": PRODUCT, "expected_source_sha256": hash }),
        );
        assert_eq!(structured(&answer)["revision"], 2, "{answer}");
        await_active(&mut harness, returned, &product, "the published component");
        let revisions: Vec<u32> = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .manifest()
            .revisions
            .iter()
            .filter(|revision| revision.handle == "product")
            .map(|revision| revision.revision)
            .collect();
        assert_eq!(revisions, [1, 2], "the new revision is shown");
        assert_eq!(tabs(&harness), [scenario, product.clone()]);
        assert_eq!(scroll_of(&harness, &product), scroll, "no scroll");
        assert_eq!(marks(&harness, &product), None);
        drop(harness);
        drop(server);
    }

    /// The agent, as a second session in this process holding the lock,
    /// for criterion 5: the MCP server has no tool that deletes a document
    /// or changes the settings, and every holder's commit is followed alike.
    fn holder(root: &std::path::Path) -> ProjectSession {
        let agent = Holder::new(HolderRole::Agent, Some("Claude Code".into()));
        ProjectSession::open_with_as(root, Arc::new(FixedPlacer), &agent).unwrap()
    }

    /// Criterion 5: deleting the followed document closes its tab with
    /// MCP-08a's notice and the active tab is the one closing chooses, not
    /// another document of the same commit; a commit of the settings alone
    /// moves nothing.
    #[test]
    fn mcp16_criterion_5_delete_and_settings_move_nothing() {
        let directory = shop();
        let root = directory.path();
        let mut owner = holder(root);
        let mut harness = folder_harness(root, WIDE);
        app(&mut harness).watcher.set_interval(NO_POLL);
        assert!(pressed(&harness), "following is on");
        let scenario = id_of(&harness, SCENARIO);
        let product = id_of(&harness, PRODUCT);
        let extra = owner
            .create_document(
                DocumentKind::Scenario,
                "scenarios/short-lived.afs",
                "scenario \"Short lived\" {\n  total = 2\n}\n",
            )
            .unwrap();
        let committed = Instant::now();
        app(&mut harness).watcher.rescan();
        await_active(&mut harness, committed, &extra, "the created document");
        assert_eq!(tabs(&harness), [scenario.clone(), extra.clone()]);

        // One reload: the followed document deleted, another one saved.
        let preview = owner
            .preview_delete(&["scenarios/short-lived.afs"])
            .unwrap();
        owner.delete(&preview, HOLDER).unwrap();
        let text = commented(owner.document(&product).unwrap().text(), "Saved too.");
        owner.edit_source(&product, &text, HOLDER).unwrap();
        owner.save(&product).unwrap();
        app(&mut harness).watcher.rescan();
        await_until(&mut harness, Instant::now(), "the deletion", |harness| {
            source_of(harness, &product) == text
        });
        assert_eq!(
            tabs(&harness),
            std::slice::from_ref(&scenario),
            "the tab closed"
        );
        assert_eq!(
            active(&harness).as_deref(),
            Some(scenario.as_str()),
            "the tab closing chose, not the product saved beside the deletion"
        );
        let notice = folder(&harness)
            .notice
            .as_ref()
            .map(|notice| notice.text.clone())
            .unwrap_or_default();
        assert!(notice.contains("short-lived.afs"), "notice: {notice:?}");

        // The settings alone.
        owner
            .edit_configuration(|manifest| manifest.name = "Renamed shop".into(), HOLDER)
            .unwrap();
        owner.save_configuration().unwrap();
        app(&mut harness).watcher.rescan();
        await_bar(&mut harness, "project settings saved");
        assert_eq!(tabs(&harness), std::slice::from_ref(&scenario));
        assert_eq!(active(&harness).as_deref(), Some(scenario.as_str()));
        assert!(pressed(&harness));
        drop(harness);
        drop(owner);
    }

    /// Criterion 6: the agent's run changes no tab and no scroll.
    #[test]
    fn mcp16_criterion_6_a_run_moves_nothing() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent_with(root, "Claude Code", &[(STEP_DELAY, "200")]);
        observe(&mut harness, &mut server, "Written by the agent.");
        let returned = agent_notes(&mut harness, &mut server, PRODUCT, "Changed by the agent.");
        let product = id_of(&harness, PRODUCT);
        await_active(&mut harness, returned, &product, "the component");
        let place = |harness: &FolderHarness| {
            (
                tabs(harness),
                active(harness),
                scroll_of(harness, &product),
                in_view(harness),
            )
        };
        let before = place(&harness);
        assert!(pressed(&harness));

        let id = server.start_call("run", json!({ "path": SCENARIO }));
        await_until(
            &mut harness,
            Instant::now(),
            "the running line",
            |harness| contains(harness, "Agent: running "),
        );
        assert_eq!(place(&harness), before, "the run's start moves nothing");
        answer(&mut harness, &mut server, id);
        await_until(&mut harness, Instant::now(), "the passed line", |harness| {
            contains(harness, " passed · ")
        });
        assert_eq!(place(&harness), before, "the run's end moves nothing");
        assert!(pressed(&harness));
        drop(harness);
        drop(server);
    }

    /// Criterion 7: unpinned by the toggle, the agent's commits keep the
    /// active tab, the scroll, the camera and the selection; the bar still
    /// names each change.
    #[test]
    fn mcp16_criterion_7_unpinned_keeps_the_place() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = id_of(&harness, SCENARIO);
        let mut server = agent(root, "Claude Code");
        observe(&mut harness, &mut server, "Written by the agent.");
        choose(&mut harness, FOLLOW);
        assert!(!pressed(&harness), "the toggle unpins");
        scroll_code(&mut harness, &scenario);
        assert!(
            scroll_of(&harness, &scenario) > 0.0,
            "the code was scrolled"
        );
        let place = |harness: &FolderHarness| {
            (
                tabs(harness),
                active(harness),
                scroll_of(harness, &scenario),
            )
        };
        let before = place(&harness);

        agent_notes(&mut harness, &mut server, PRODUCT, "Changed by the agent.");
        await_bar(&mut harness, "product.afs saved");
        assert_eq!(place(&harness), before, "Code keeps the place");

        choose(&mut harness, "Diagram");
        choose(&mut harness, "Expected price (Value)");
        choose(&mut harness, "Zoom in");
        let board = |harness: &FolderHarness| {
            (
                folder(harness).canvases[&scenario].camera(),
                folder(harness).selections.get(&scenario).cloned(),
            )
        };
        let seen = board(&harness);
        agent_notes(
            &mut harness,
            &mut server,
            PROMOTION,
            "Changed by the agent.",
        );
        await_bar(&mut harness, "active-promotion.afs saved");
        assert_eq!(place(&harness), before, "Diagram keeps the tab");
        assert_eq!(board(&harness), seen, "the camera and the selection stay");
        assert_eq!(mode(&harness), Mode::Diagram);
        assert!(!pressed(&harness), "still unpinned");
        drop(harness);
        drop(server);
    }

    /// Criterion 8: a tab click, a file activated in the explorer, Ctrl+Tab
    /// and closing the followed tab each unpin; scrolling, zooming,
    /// selecting a block and switching the mode do not. *Follow* pins again
    /// and goes to the document of the agent's last commit at once.
    #[test]
    fn mcp16_criterion_8_navigation_unpins_and_follow_pins() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let scenario = id_of(&harness, SCENARIO);
        let product = id_of(&harness, PRODUCT);
        let promotion = id_of(&harness, PROMOTION);
        app(&mut harness).activate_tab(&product);
        settle(&mut harness);
        app(&mut harness).activate_tab(&scenario);
        settle(&mut harness);
        let mut server = agent(root, "Claude Code");
        observe(&mut harness, &mut server, "Written by the agent.");
        assert!(pressed(&harness));

        // What does not unpin.
        scroll_code(&mut harness, &scenario);
        assert!(pressed(&harness), "scrolling the code");
        for (gesture, label) in [
            ("switching to Diagram", "Diagram"),
            ("zooming", "Zoom in"),
            ("selecting a block", "Expected price (Value)"),
            ("switching to Split", "Split"),
            ("switching to Code", "Code"),
        ] {
            choose(&mut harness, label);
            assert!(pressed(&harness), "{gesture}");
        }
        assert_eq!(active(&harness).as_deref(), Some(scenario.as_str()));

        // A tab click.
        click_tab(&mut harness, "product.afs");
        assert_eq!(active(&harness).as_deref(), Some(product.as_str()));
        assert!(!pressed(&harness), "a tab click unpins");
        choose(&mut harness, FOLLOW);
        assert!(pressed(&harness));
        assert_eq!(
            active(&harness).as_deref(),
            Some(scenario.as_str()),
            "Follow goes to the agent's last change at once"
        );

        // A file activated in the explorer; the agent goes on meanwhile.
        choose(&mut harness, "active-promotion.afs");
        assert_eq!(active(&harness).as_deref(), Some(promotion.as_str()));
        assert!(!pressed(&harness), "the explorer unpins");
        agent_notes(&mut harness, &mut server, PRODUCT, "Changed by the agent.");
        await_bar(&mut harness, "product.afs saved");
        assert_eq!(
            active(&harness).as_deref(),
            Some(promotion.as_str()),
            "unpinned, the window stays"
        );
        choose(&mut harness, FOLLOW);
        assert!(pressed(&harness));
        assert_eq!(active(&harness).as_deref(), Some(product.as_str()));

        // Ctrl+Tab.
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
        settle(&mut harness);
        assert_ne!(active(&harness).as_deref(), Some(product.as_str()));
        assert!(!pressed(&harness), "Ctrl+Tab unpins");
        choose(&mut harness, FOLLOW);
        assert!(pressed(&harness));
        assert_eq!(active(&harness).as_deref(), Some(product.as_str()));

        // Closing the followed tab.
        choose(&mut harness, "Close product.afs");
        assert!(!tabs(&harness).contains(&product));
        assert!(!pressed(&harness), "closing the followed tab unpins");
        choose(&mut harness, FOLLOW);
        assert!(pressed(&harness));
        assert_eq!(active(&harness).as_deref(), Some(product.as_str()));
        drop(harness);
        drop(server);
    }

    /// Criterion 9: the toggle is named *Follow the agent* with its on/off
    /// state, Tab reaches it in the bar, Space works it, and it carries the
    /// hover text of decision 5 in both states.
    #[test]
    fn mcp16_criterion_9_the_toggle_is_accessible() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent(root, "Claude Code");
        observe(&mut harness, &mut server, "Written by the agent.");
        assert_eq!(
            harness.get_by_label(FOLLOW).accesskit_node().role(),
            egui::accesskit::Role::Button
        );
        assert!(pressed(&harness));
        let bar = harness.get_by_label(BAR).rect();
        assert!(bar.contains_rect(harness.get_by_label(FOLLOW).rect()));

        let mut reached = false;
        for _ in 0..120 {
            harness.key_press(Key::Tab);
            harness.step();
            if harness.get_by_label(FOLLOW).accesskit_node().is_focused() {
                reached = true;
                break;
            }
        }
        assert!(reached, "Tab reaches the toggle");
        harness.key_press(Key::Space);
        settle(&mut harness);
        assert!(!pressed(&harness), "Space releases it");
        harness.key_press(Key::Space);
        settle(&mut harness);
        assert!(pressed(&harness), "Space presses it again");

        harness.get_by_label(FOLLOW).hover();
        settle(&mut harness);
        harness.run_steps(60);
        assert!(has_label(&harness, PINNED_HINT), "the pressed hover text");
        choose(&mut harness, FOLLOW);
        assert!(!pressed(&harness));
        harness.get_by_label(FOLLOW).hover();
        settle(&mut harness);
        harness.run_steps(60);
        assert!(
            has_label(&harness, RELEASED_HINT),
            "the released hover text"
        );
        drop(harness);
        drop(server);
    }

    /// In every mode: the bar inside the window; `status`, *Follow* and
    /// *Take back* whole, disjoint and in this order; the holder text
    /// beside them.
    fn assert_bar_fits(harness: &mut FolderHarness, status: &str) {
        for view in MODES {
            choose(harness, view);
            assert_within_window(harness, &[BAR, status, FOLLOW, TAKE_BACK]);
            let holder = label_containing(harness, "Claude Code");
            assert_disjoint(harness, &[holder.as_str(), status, FOLLOW, TAKE_BACK]);
            let rect = |label| harness.get_by_label(label).rect();
            let shown = rect(status);
            let whole = whole_width(harness, status, theme::FONT_META);
            assert!(
                shown.width() + 1.0 >= whole,
                "{view}: {status:?} is cut: {} of {whole} px",
                shown.width()
            );
            assert!(
                shown.right() <= rect(FOLLOW).left()
                    && rect(FOLLOW).right() <= rect(TAKE_BACK).left(),
                "{view}: the status, Follow, Take back, in this order"
            );
            let bar = rect(BAR);
            for label in [status, FOLLOW, TAKE_BACK] {
                assert!(bar.contains_rect(rect(label)), "{view}: {label} in the bar");
            }
        }
    }

    /// Criterion 10: at 960 × 540 in Code, Diagram and Split, with MCP-10's
    /// status and with the last change, *Follow*, *Take back* and the text
    /// beside them are whole and disjoint inside the window; the holder
    /// text gives way.
    #[test]
    fn mcp16_criterion_10_the_bar_fits_at_the_minimum() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, MINIMUM);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let mut server = agent_with(root, "Claude Code", &[(STEP_DELAY, "300")]);
        observe(&mut harness, &mut server, "Written by the agent.");
        await_bar(&mut harness, &format!("{AGENT} · since "));

        let id = server.start_call("run", json!({ "path": SCENARIO }));
        let running = "Agent: running promotion-and-cart-price.afs (simulation)…";
        await_until(
            &mut harness,
            Instant::now(),
            "the running line",
            |harness| has_label(harness, running),
        );
        assert_bar_fits(&mut harness, running);
        let holder = label_containing(&harness, "Claude Code");
        assert!(
            !holder.contains(PAUSED),
            "the holder text gives way: {holder:?}"
        );
        answer(&mut harness, &mut server, id);
        await_bar(&mut harness, " passed · ");
        let passed = label_containing(&harness, " passed · ");
        assert_bar_fits(&mut harness, &passed);

        choose(&mut harness, FOLLOW);
        agent_notes(&mut harness, &mut server, PRODUCT, "Changed by the agent.");
        await_bar(&mut harness, "product.afs saved");
        let saved = label_containing(&harness, "product.afs saved");
        assert_bar_fits(&mut harness, &saved);
        drop(harness);
        drop(server);
    }

    /// Criterion 11: tabs that following opened stay when the agent
    /// finishes and after *Take back*, and their documents are editable.
    #[test]
    fn mcp16_criterion_11_followed_tabs_stay() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let product = id_of(&harness, PRODUCT);
        let promotion = id_of(&harness, PROMOTION);
        let mut server = agent(root, "Claude Code");
        observe(&mut harness, &mut server, "Written by the agent.");
        let returned = agent_notes(&mut harness, &mut server, PRODUCT, "Changed by the agent.");
        await_active(&mut harness, returned, &product, "the component");
        let finished = call(&mut harness, &mut server, "finish_editing", json!({}));
        assert_eq!(structured(&finished)["finished"], true, "{finished}");
        await_writer(&mut harness);
        assert!(
            tabs(&harness).contains(&product),
            "kept when the agent finishes"
        );

        observe(&mut harness, &mut server, "Written again.");
        let returned = agent_notes(
            &mut harness,
            &mut server,
            PROMOTION,
            "Changed by the agent.",
        );
        await_active(&mut harness, returned, &promotion, "the other component");
        choose(&mut harness, TAKE_BACK);
        await_writer(&mut harness);
        let open = tabs(&harness);
        assert!(
            open.contains(&product) && open.contains(&promotion),
            "kept after Take back: {open:?}"
        );
        for document in [&promotion, &product] {
            app(&mut harness).activate_tab(document);
            settle(&mut harness);
            let before = source_of(&harness, document);
            let label = editor(&harness, document);
            type_into(&mut harness, &label, "// typed\n");
            assert_ne!(source_of(&harness, document), before, "editable");
        }
        drop(harness);
        drop(server);
    }

    /// Criterion 12: the next agent session starts following again after
    /// the person unpinned it in the previous one.
    #[test]
    fn mcp16_criterion_12_following_resets_per_observation() {
        let directory = shop();
        let root = directory.path();
        let mut harness = folder_harness(root, WIDE);
        await_endpoint(&mut harness, root, HolderRole::Window);
        let product = id_of(&harness, PRODUCT);
        let mut server = agent(root, "Claude Code");
        observe(&mut harness, &mut server, "Written by the agent.");
        choose(&mut harness, FOLLOW);
        assert!(!pressed(&harness));
        let finished = call(&mut harness, &mut server, "finish_editing", json!({}));
        assert_eq!(structured(&finished)["finished"], true, "{finished}");
        await_writer(&mut harness);

        observe(&mut harness, &mut server, "Written again.");
        assert!(pressed(&harness), "a new observation follows");
        let returned = agent_notes(&mut harness, &mut server, PRODUCT, "Changed by the agent.");
        await_active(&mut harness, returned, &product, "the component");
        drop(harness);
        drop(server);
    }

    /// The four states of the mock-up f1–f4 with a real agent: following
    /// in Code with lines 12–14 changed and a run going, unpinned on another
    /// tab, following in Diagram after a passed run, following in Split.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_mcp16_follow() {
        use crate::testing::capture;
        let states = [
            ("mcp16-f1-following-code-960x540", MINIMUM, "Code"),
            ("mcp16-f2-unpinned-code-960x540", MINIMUM, "Code"),
            ("mcp16-f3-following-diagram-1600x1000", WIDE, "Diagram"),
            ("mcp16-f4-following-split-960x540", MINIMUM, "Split"),
        ];
        for (name, size, view) in states {
            let directory = shop();
            let root = directory.path();
            let mut harness = folder_harness(root, size);
            await_endpoint(&mut harness, root, HolderRole::Window);
            let scenario = id_of(&harness, SCENARIO);
            let product = id_of(&harness, PRODUCT);
            app(&mut harness).activate_tab(&product);
            settle(&mut harness);
            app(&mut harness).activate_tab(&scenario);
            choose(&mut harness, view);
            let mut server = agent_with(root, "Claude Code", &[(STEP_DELAY, "400")]);
            let (text, _) = read(&mut server, SCENARIO);
            let changed = text
                .replacen(
                    "@label(\"Promotional price\")",
                    "@label(\"Promotional total\")",
                    1,
                )
                .replacen("within 0.005", "within 0.01", 1)
                .replacen(
                    "@label(\"Field inna_nazwa\")",
                    "@label(\"Field present\")",
                    1,
                );
            assert_eq!(line_of(&changed, "Promotional total"), 12);
            assert_eq!(line_of(&changed, "Field present"), 14);
            let answer = write(&mut harness, &mut server, SCENARIO, &changed);
            assert_eq!(structured(&answer)["wrote"], true, "{answer}");
            await_until(
                &mut harness,
                Instant::now(),
                "the agent's save",
                |harness| source_of(harness, &scenario) == changed,
            );
            await_bar(&mut harness, &format!("{AGENT} · since "));
            match name {
                "mcp16-f2-unpinned-code-960x540" => {
                    choose(&mut harness, FOLLOW);
                    click_tab(&mut harness, "product.afs");
                }
                "mcp16-f3-following-diagram-1600x1000" => {
                    call(
                        &mut harness,
                        &mut server,
                        "run",
                        json!({ "path": SCENARIO }),
                    );
                    await_bar(&mut harness, " passed · ");
                }
                _ => {
                    server.start_call("run", json!({ "path": SCENARIO }));
                    await_bar(&mut harness, "Agent: running ");
                }
            }
            harness.hover_at(egui::pos2(1.0, 1.0));
            settle(&mut harness);
            capture(&mut harness, name, None);
            drop(harness);
            drop(server);
        }
    }
}

/// MCP-19 (#191): on Windows a file operation beside the window's own check
/// of the folder no longer fails because the check was reading its files.
mod mcp19 {
    use super::*;
    use apiforge_storage::project::{
        ProjectStore,
        faults::{Boundary, Phase},
    };
    use std::sync::{
        Mutex,
        mpsc::{RecvTimeoutError, channel},
    };

    const FOLDER: &str = "scenarios";
    const SOURCE: &str = "scenarios/promotion-and-cart-price.afs";
    /// Long enough that a check which is not held has answered.
    const NOT_HELD: Duration = Duration::from_millis(500);
    /// Generous: only reached when something is broken.
    const STUCK: Duration = Duration::from_secs(20);

    /// Criterion 3: the window deletes a folder while its own check of the
    /// folder is forced to run, as `Watcher::finish` runs it. The delete is
    /// stopped where it removes the folder's scenario; the check waits for
    /// it rather than reading the files it removes, then sees the folder
    /// gone. The tab closes and nothing asks for recovery.
    #[test]
    fn mcp19_criterion_3_a_folder_deleted_beside_the_windows_check_closes_its_tab() {
        let directory = shop();
        let root = directory.path();
        let (context, mut app) = open(root);
        let id = document(&app, SOURCE);
        app.workbench.open(&id);
        frame(&mut app, &context, vec![]);
        assert!(
            app.workbench.tabs().iter().any(|tab| tab.document_id == id),
            "the scenario is open"
        );
        app.request_delete(vec![FOLDER.into()]);
        wait_storage(&mut app, &context);
        let Some(Prompt::Delete { preview, .. }) = app.prompt.take() else {
            panic!("a delete confirmation");
        };

        let (stopped, at_remove) = channel::<()>();
        let (resume, resumed) = channel::<()>();
        let (stopped, resumed) = (Mutex::new(Some(stopped)), Mutex::new(resumed));
        app.storage.fault = Some(Arc::new(move |event| {
            if event.boundary == Boundary::RemoveFile
                && event.phase == Phase::Before
                && event.path.ends_with(SOURCE)
                && let Some(stopped) = stopped.lock().unwrap().take()
            {
                stopped.send(()).unwrap();
                resumed
                    .lock()
                    .unwrap()
                    .recv_timeout(STUCK)
                    .expect("the test resumes the delete");
            }
            Ok(())
        }));
        let check = app.session.as_ref().unwrap().external_check();
        app.delete_confirmed(&preview).unwrap();
        let deadline = Instant::now() + STUCK;
        while at_remove.try_recv().is_err() {
            assert!(
                Instant::now() < deadline,
                "the delete never reached the scenario's removal"
            );
            frame(&mut app, &context, vec![]);
            std::thread::sleep(Duration::from_millis(1));
        }

        let (started, starting) = channel::<()>();
        let (checked, checking) = channel();
        std::thread::spawn(move || {
            started.send(()).unwrap();
            checked.send(
                check
                    .run()
                    .map(|changes| changes.scan)
                    .map_err(|error| error.to_string()),
            )
        });
        // The check has started before the delete can go on: what follows
        // is its answer during the delete, or its wait.
        starting.recv_timeout(STUCK).expect("the check starts");
        match checking.recv_timeout(NOT_HELD) {
            Err(RecvTimeoutError::Timeout) => {}
            early => panic!(
                "the check answered during the delete: {:?}",
                early.map(|answer| answer.map(|scan| scan.generation))
            ),
        }
        resume.send(()).unwrap();
        wait_storage(&mut app, &context);
        frame(&mut app, &context, vec![]);
        app.storage.fault = None;

        let scan = checking
            .recv_timeout(STUCK)
            .expect("the check answers after the delete")
            .expect("the check reads the folder after the delete");
        assert!(
            scan.entry(FOLDER).is_none() && scan.entry(SOURCE).is_none(),
            "the check sees the folder gone"
        );
        let generation = ProjectStore::open_read_only(root)
            .unwrap()
            .read(&[])
            .unwrap()
            .generation;
        assert_eq!(
            scan.generation, generation,
            "the check reads the delete's generation"
        );
        let notice = app.notice.as_ref().expect("the delete says what it did");
        assert!(!notice.error, "the delete failed: {}", notice.text);
        assert!(
            app.workbench.tabs().iter().all(|tab| tab.document_id != id),
            "the deleted scenario's tab closed"
        );
        assert!(!root.join(FOLDER).exists(), "the folder is gone");

        watch(&mut app, &context);
        assert!(!app.watcher.recovery_required, "nothing asks for recovery");
        assert_eq!(app.watcher.problem, None);
        assert!(app.workbench.tabs().iter().all(|tab| tab.document_id != id));
        assert!(!root.join(FOLDER).exists());
    }
}

/// UX-01: several checks from one run (docs/tickets/UX-01.md). *Check* on a
/// field in Run results adds its check at once and the results stay usable;
/// the panel's bar counts the checks that wait for the next run, any other
/// change makes the results stale as before, and the model's undo takes a
/// check back. The demo shop in Split at 960 × 540 unless said otherwise.
mod ux01 {
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use egui_kittest::kittest::NodeT;

    const STEP: &str = "Add to cart · Passed";
    const ONE: &str = "1 check added · it runs on the next run";
    const TWO: &str = "2 checks added · they run on the next run";
    const STALE: &str = "Graph changed · results from an earlier version";
    const ADDED: &str = "Check added";
    const CHECK: &str = "Check";
    const TO_VARIABLE: &str = "To variable";
    const MAXIMISE: &str = "Maximise run results";
    const RESTORE: &str = "Restore run results";
    const POLICY: &str = "When a step fails";

    /// The demo shop at `size` in Split with its scenario active.
    fn window(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), size);
        let scenario = only(&harness, DocumentKind::Scenario);
        app(&mut harness).activate_tab(&scenario);
        settle(&mut harness);
        choose(&mut harness, "Split");
        (project, harness, scenario)
    }

    /// Runs the active tab with the header's run button.
    fn run(harness: &mut FolderHarness) -> RunResult {
        choose(harness, "Run simulation");
        finish_run(harness)
    }

    /// Chooses a step in the list of Run results, as a click on its row.
    fn select_step(harness: &mut FolderHarness, step: &str) {
        harness.get_by_label_contains(step).scroll_to_me();
        settle(harness);
        harness.get_by_label_contains(step).click();
        settle(harness);
    }

    /// The window after one simulation of the scenario, its Run results
    /// maximised on *Add to cart*.
    fn after_a_run(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String, RunResult) {
        let (project, mut harness, scenario) = window(size);
        let report = run(&mut harness);
        assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
        choose(&mut harness, MAXIMISE);
        select_step(&mut harness, STEP);
        assert!(has_label(&harness, "productId"), "Add to cart's outputs");
        (project, harness, scenario, report)
    }

    /// The controls on the row of `field` in the shown value tree: those of
    /// `names` whose centre is level with the field's label.
    fn on_row(harness: &FolderHarness, field: &str, names: &[&str]) -> Vec<String> {
        let y = harness.get_by_label(field).rect().center().y;
        names
            .iter()
            .filter(|name| {
                harness
                    .query_all_by_label(name)
                    .any(|node| (node.rect().center().y - y).abs() < 8.0)
            })
            .map(|name| (*name).to_owned())
            .collect()
    }

    /// What the row of `field` offers: *Check added*, or *Check* and *To
    /// variable*.
    fn row(harness: &FolderHarness, field: &str) -> Vec<String> {
        on_row(harness, field, &[ADDED, CHECK, TO_VARIABLE])
    }

    fn offers_check(harness: &FolderHarness, field: &str) {
        assert_eq!(
            row(harness, field),
            [CHECK, TO_VARIABLE],
            "the {field} row offers Check and To variable"
        );
    }

    fn reads_added(harness: &FolderHarness, field: &str) {
        assert_eq!(
            row(harness, field),
            [ADDED],
            "the {field} row reads Check added, with neither Check nor To variable"
        );
    }

    /// Clicks the *Check* on the row of `field`: the *Check* nearest the
    /// field's label, as the mock-up's helper does.
    fn check(harness: &mut FolderHarness, field: &str) {
        crate::testing::check_field(harness, field);
    }

    /// The colour `text` is painted in this frame, when it is painted.
    fn colour_of(harness: &FolderHarness, text: &str) -> Option<egui::Color32> {
        fn find(shape: &egui::epaint::Shape, text: &str) -> Option<egui::Color32> {
            match shape {
                egui::epaint::Shape::Vec(shapes) => {
                    shapes.iter().find_map(|shape| find(shape, text))
                }
                egui::epaint::Shape::Text(shape) if shape.galley.job.text == text => shape
                    .galley
                    .job
                    .sections
                    .first()
                    .map(|section| section.format.color),
                _ => None,
            }
        }
        harness
            .output()
            .shapes
            .iter()
            .find_map(|clipped| find(&clipped.shape, text))
    }

    /// The bar's text, which is one of the count, the stale text or none.
    fn bar(harness: &FolderHarness) -> Option<&'static str> {
        let shown: Vec<_> = [ONE, TWO, STALE]
            .into_iter()
            .filter(|text| has_label(harness, text))
            .collect();
        assert!(shown.len() <= 1, "one text at a time: {shown:?}");
        shown.first().copied()
    }

    fn nodes(harness: &FolderHarness, document_id: &str) -> usize {
        let model = folder(harness)
            .session
            .as_ref()
            .unwrap()
            .document(document_id)
            .unwrap()
            .model()
            .cloned()
            .expect("a valid model");
        graph_of(&model, false).nodes.len()
    }

    fn enabled(harness: &FolderHarness, label: &str) -> bool {
        !harness.get_by_label(label).accesskit_node().is_disabled()
    }

    /// No field offers *Check* or *To variable*, as on stale results today:
    /// their row shows the value only.
    fn no_check_offered(harness: &FolderHarness) {
        assert!(has_label(harness, "productId"), "the outputs are shown");
        assert_eq!(harness.query_all_by_label(CHECK).count(), 0, "no Check");
        assert_eq!(harness.query_all_by_label(TO_VARIABLE).count(), 0);
    }

    /// Ctrl+Z outside the code editor: the model's undo, the gesture that
    /// undoes a block added from the library. `redo` presses Ctrl+Shift+Z.
    fn model_history(harness: &mut FolderHarness, redo: bool) {
        let modifiers = if redo {
            Modifiers::COMMAND | Modifiers::SHIFT
        } else {
            Modifiers::COMMAND
        };
        harness.key_press_modifiers(modifiers, Key::Z);
        settle(harness);
    }

    /// Criterion 1: *Check* on `productId` adds a check card and the panel
    /// stays usable: one more field assertion in the source, *Add to cart*'s
    /// Outputs still shown, the row reads *Check added* and offers neither
    /// *Check* nor *To variable*; *Check* on `total` adds a second card
    /// without a run, and its row reads *Check added* too.
    #[test]
    fn ux01_criterion_1_two_checks_from_one_run() {
        let (_project, mut harness, scenario, report) = after_a_run(MINIMUM);
        let before = nodes(&harness, &scenario);
        offers_check(&harness, "productId");
        offers_check(&harness, "total");

        check(&mut harness, "productId");
        assert_eq!(nodes(&harness, &scenario), before + 1, "one check card");
        assert!(
            source_of(&harness, &scenario).contains("add_to_cart.productId == 123"),
            "the source gains the field's assertion:\n{}",
            source_of(&harness, &scenario)
        );
        assert!(
            has_label(&harness, "Outputs  1"),
            "the panel still shows Add to cart's Outputs"
        );
        reads_added(&harness, "productId");
        offers_check(&harness, "total");
        offers_check(&harness, "quantity");

        check(&mut harness, "total");
        assert_eq!(nodes(&harness, &scenario), before + 2, "a second card");
        assert!(source_of(&harness, &scenario).contains("add_to_cart.total == 80"));
        assert!(folder(&harness).runs.handle.is_none(), "without a run");
        assert_eq!(
            folder(&harness).run_result(&scenario).unwrap().id,
            report.id,
            "the same results"
        );
        reads_added(&harness, "productId");
        reads_added(&harness, "total");
        offers_check(&harness, "id");
        offers_check(&harness, "quantity");
    }

    /// Criterion 2 (first half): with only checks added since the run, the
    /// bar counts them in grey and *Graph changed* is absent.
    #[test]
    fn ux01_criterion_2_the_bar_counts_the_checks() {
        let (_project, mut harness, _scenario, _) = after_a_run(MINIMUM);
        assert_eq!(bar(&harness), None, "fresh results have no bar text");
        check(&mut harness, "productId");
        assert_eq!(bar(&harness), Some(ONE));
        assert_eq!(colour_of(&harness, ONE), Some(theme::MUTED), "in grey");
        check(&mut harness, "total");
        assert_eq!(bar(&harness), Some(TWO));
        assert_eq!(colour_of(&harness, TWO), Some(theme::MUTED), "in grey");
        assert!(enabled(&harness, POLICY), "the error policy stays usable");
        assert!(enabled(&harness, "Export"), "Export stays usable");
    }

    /// Criterion 2 (second half): after a check, any other change — typing a
    /// space into the code, a block added with the board's *Add step*, the
    /// other error policy — makes the bar read *Graph changed* in amber, the
    /// count is gone and no field offers *Check*, as on stale results today.
    ///
    /// Moving a block is not such a change: it moves the card, not the
    /// source, and keeps fresh results fresh today, so it keeps the count.
    #[test]
    fn ux01_criterion_2_another_change_makes_the_results_stale() {
        fn stale(harness: &mut FolderHarness, change: &str) {
            assert_eq!(bar(harness), Some(STALE), "after {change}");
            assert_eq!(
                colour_of(harness, STALE),
                Some(theme::WARNING),
                "amber after {change}"
            );
            if has_label(harness, MAXIMISE) {
                choose(harness, MAXIMISE);
            }
            no_check_offered(harness);
            assert!(!enabled(harness, POLICY), "{change}: the policy is locked");
        }

        // Typing a space into the code.
        let (_project, mut harness, scenario, _) = after_a_run(MINIMUM);
        check(&mut harness, "productId");
        assert_eq!(bar(&harness), Some(ONE));
        choose(&mut harness, RESTORE);
        let editor = editor(&harness, &scenario);
        type_into(&mut harness, &editor, " ");
        stale(&mut harness, "typing a space into the code");

        // A block added with the board's Add step.
        let (_project, mut harness, scenario, _) = after_a_run(MINIMUM);
        check(&mut harness, "productId");
        choose(&mut harness, RESTORE);
        let before = nodes(&harness, &scenario);
        choose(&mut harness, "Add step");
        choose(&mut harness, "A = B");
        assert_eq!(nodes(&harness, &scenario), before + 1, "a block is added");
        stale(&mut harness, "a block added on the board");

        // The other error policy.
        let (_project, mut harness, _scenario, _) = after_a_run(MINIMUM);
        check(&mut harness, "productId");
        assert!(enabled(&harness, POLICY));
        choose(&mut harness, POLICY);
        choose(&mut harness, "Stop on error");
        stale(&mut harness, "choosing the other error policy");

        // Moving a block keeps the count, as it keeps fresh results fresh.
        let (_project, mut harness, scenario, _) = after_a_run(MINIMUM);
        choose(&mut harness, RESTORE);
        move_card(&mut harness, &scenario, "Expected price (Value)");
        assert_eq!(bar(&harness), None, "a move keeps fresh results fresh");
        assert!(folder(&harness).fresh_run(&scenario).is_some());
        choose(&mut harness, MAXIMISE);
        select_step(&mut harness, STEP);
        check(&mut harness, "productId");
        choose(&mut harness, RESTORE);
        move_card(&mut harness, &scenario, "Expected price (Value)");
        assert_eq!(bar(&harness), Some(ONE), "a move keeps the count");
    }

    /// Drags the card named `card` on the board by (120, 80) and asserts it
    /// moved.
    fn move_card(harness: &mut FolderHarness, scenario: &str, card: &str) {
        let position = |harness: &FolderHarness| {
            let model = folder(harness)
                .session
                .as_ref()
                .unwrap()
                .document(scenario)
                .unwrap()
                .model()
                .cloned()
                .unwrap();
            graph_of(&model, false)
                .nodes
                .iter()
                .find(|node| node.id == "expected")
                .unwrap()
                .position
                .clone()
        };
        let before = position(harness);
        let start = harness.get_by_label(card).rect().center();
        harness.hover_at(start);
        harness.step();
        harness.drag_at(start);
        harness.step();
        for step in 1..=8 {
            harness.hover_at(start + egui::vec2(5.0, 3.0) * step as f32);
            harness.step();
        }
        harness.drop_at(start + egui::vec2(40.0, 24.0));
        harness.hover_at(egui::pos2(480.0, 3.0));
        settle(harness);
        assert_ne!(position(harness), before, "the card moved");
    }

    /// Criterion 3: the model's undo (Ctrl+Z outside the code editor) takes
    /// the last check back like any edit: after two checks one undo gives
    /// *1 check added* and `total` offers *Check* again, a second gives a
    /// fresh report with no bar text; redo brings *1 check added* back.
    #[test]
    fn ux01_criterion_3_undo_takes_a_check_back() {
        let (_project, mut harness, scenario, _) = after_a_run(MINIMUM);
        let before = nodes(&harness, &scenario);
        check(&mut harness, "productId");
        check(&mut harness, "total");
        assert_eq!(bar(&harness), Some(TWO));

        model_history(&mut harness, false);
        assert_eq!(nodes(&harness, &scenario), before + 1, "one check undone");
        assert_eq!(bar(&harness), Some(ONE));
        reads_added(&harness, "productId");
        offers_check(&harness, "total");

        model_history(&mut harness, false);
        assert_eq!(nodes(&harness, &scenario), before, "both undone");
        assert_eq!(bar(&harness), None, "fresh results have no bar text");
        assert!(folder(&harness).fresh_run(&scenario).is_some(), "fresh");
        offers_check(&harness, "productId");
        offers_check(&harness, "total");

        model_history(&mut harness, true);
        assert_eq!(nodes(&harness, &scenario), before + 1, "one redone");
        assert_eq!(bar(&harness), Some(ONE));
        reads_added(&harness, "productId");
        offers_check(&harness, "total");
    }

    /// Criterion 5: the next run runs the checks: two more passed steps, no
    /// bar text, and every field offers *Check* again.
    #[test]
    fn ux01_criterion_5_the_next_run_runs_the_checks() {
        let (_project, mut harness, _scenario, first) = after_a_run(MINIMUM);
        let passed = |report: &RunResult| {
            report
                .steps
                .iter()
                .filter(|step| step.status == apiforge_core::run::StepStatus::Passed)
                .count()
        };
        check(&mut harness, "productId");
        check(&mut harness, "total");
        let next = run(&mut harness);
        assert_eq!(next.status, RunStatus::Passed, "{:?}", next.error);
        assert_eq!(passed(&next), passed(&first) + 2, "the two checks ran");
        assert_eq!(bar(&harness), None, "a new count starts");
        if has_label(&harness, MAXIMISE) {
            choose(&mut harness, MAXIMISE);
        }
        select_step(&mut harness, STEP);
        for field in ["id", "productId", "quantity", "total"] {
            offers_check(&harness, field);
        }
    }

    /// Criterion 6: the board and the code keep the run's marks while only
    /// checks were added, and lose them after another change.
    #[test]
    fn ux01_criterion_6_the_board_keeps_the_run() {
        let (_project, mut harness, scenario, report) = after_a_run(MINIMUM);
        check(&mut harness, "productId");
        assert_eq!(
            folder(&harness).fresh_run(&scenario).map(|run| run.id),
            Some(report.id),
            "the cards keep the run's pass marks after a check"
        );
        choose(&mut harness, RESTORE);
        let editor = editor(&harness, &scenario);
        type_into(&mut harness, &editor, " ");
        assert!(
            folder(&harness).fresh_run(&scenario).is_none(),
            "the marks are gone after another change"
        );
    }

    /// Criterion 7: a component's own run (KTLO-23) counts its checks the
    /// same way: *Fetch product*'s `name` and `price` checked from one run,
    /// the count, undo, and an edit of the code making the results stale.
    #[test]
    fn ux01_criterion_7_a_component_run_counts_its_checks() {
        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), MINIMUM);
        let component = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == "components/product.afs")
            .unwrap()
            .id()
            .to_owned();
        app(&mut harness).activate_tab(&component);
        settle(&mut harness);
        choose(&mut harness, "Split");
        let report = run(&mut harness);
        assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
        choose(&mut harness, MAXIMISE);
        select_step(&mut harness, "Fetch product · Passed");
        let before = nodes(&harness, &component);
        offers_check(&harness, "name");

        check(&mut harness, "name");
        assert_eq!(nodes(&harness, &component), before + 1);
        reads_added(&harness, "name");
        assert_eq!(bar(&harness), Some(ONE));
        check(&mut harness, "price");
        reads_added(&harness, "price");
        assert_eq!(bar(&harness), Some(TWO));
        assert!(folder(&harness).fresh_run(&component).is_some());

        model_history(&mut harness, false);
        assert_eq!(bar(&harness), Some(ONE));
        offers_check(&harness, "price");

        choose(&mut harness, RESTORE);
        let editor = editor(&harness, &component);
        type_into(&mut harness, &editor, " ");
        assert_eq!(bar(&harness), Some(STALE));
        assert!(folder(&harness).fresh_run(&component).is_none());
    }

    /// Criterion 8: the bar fits. *Run results*, the status pill, the count,
    /// *Maximise* / *Restore run results* and the policy combo are within the
    /// window and disjoint at 960 × 540 maximised and restored, and at
    /// 1600 × 1000 at the panel's tallest normal height; a *Check added* row
    /// stays within the window where the rows show. At 960 × 540 restored the
    /// rows are below the panel's scrolled area, as in the mock-up, and an
    /// accessibility node keeps a row's place there without painting it.
    #[test]
    fn ux01_criterion_8_the_bar_fits() {
        fn fits(harness: &FolderHarness, toggle: &str, rows: bool, at: &str) {
            let labels = ["Run results", "PASSED", TWO, toggle, POLICY];
            assert_within_window(harness, &labels);
            assert_disjoint(harness, &labels);
            if !rows {
                return;
            }
            let added: Vec<_> = harness.query_all_by_label(ADDED).collect();
            assert!(!added.is_empty(), "{at}: rows shown");
            let window = harness.ctx.content_rect();
            for node in added {
                assert!(
                    window.contains_rect(node.rect()),
                    "{at}: a Check added row at {:?} reaches past the panel",
                    node.rect()
                );
            }
        }

        let (_project, mut harness, _scenario, _) = after_a_run(MINIMUM);
        check(&mut harness, "productId");
        check(&mut harness, "total");
        fits(&harness, RESTORE, true, "960 × 540 maximised");
        choose(&mut harness, RESTORE);
        fits(&harness, MAXIMISE, false, "960 × 540 restored");

        let (_project, mut harness, _scenario, _) = after_a_run(WIDE);
        choose(&mut harness, RESTORE);
        // The panel at its tallest normal height, from its edge's keyboard.
        harness.get_by_label("Resize run report").focus();
        harness.step();
        harness.key_press(Key::End);
        settle(&mut harness);
        check(&mut harness, "productId");
        check(&mut harness, "total");
        fits(
            &harness,
            MAXIMISE,
            true,
            "1600 × 1000 at the tallest normal height",
        );
        assert!(
            harness.query_all_by_label(ADDED).count() == 2,
            "both rows show at the tallest normal height"
        );
    }

    /// UX-35 (#252): the prompt a check's edit raises when the source holds
    /// a comment that belongs to no statement.
    const COMMENT_PROMPT: &str = "Continue without unattached comments?";

    /// The window after one simulation of a scenario whose source starts
    /// with a comment that belongs to no statement, its Run results
    /// maximised on *Add to cart*; with the source the run used.
    fn after_a_run_with_a_loose_comment()
    -> (tempfile::TempDir, FolderHarness, String, RunResult, String) {
        let (project, mut harness, scenario) = window(MINIMUM);
        let text = format!("// loose comment\n\n{}", source_of(&harness, &scenario));
        let session = app(&mut harness).session.as_mut().unwrap();
        session.edit_source(&scenario, &text, CODE).unwrap();
        session.settle(&scenario).unwrap();
        settle(&mut harness);
        let report = run(&mut harness);
        assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
        choose(&mut harness, MAXIMISE);
        select_step(&mut harness, STEP);
        assert!(has_label(&harness, "productId"), "Add to cart's outputs");
        assert_eq!(bar(&harness), None, "fresh results have no bar text");
        (project, harness, scenario, report, text)
    }

    /// UX-35 criterion 1: a check whose edit raised the prompt and was
    /// confirmed reads *Check added* and adds 1 to the bar's count; the
    /// results stay usable, so a second check counts too.
    #[test]
    fn ux35_criterion_1_a_confirmed_check_counts_as_added() {
        let (_project, mut harness, scenario, report, text) = after_a_run_with_a_loose_comment();
        let before = nodes(&harness, &scenario);

        check(&mut harness, "productId");
        assert!(
            has_label(&harness, COMMENT_PROMPT),
            "the check's edit asks first"
        );
        assert_eq!(source_of(&harness, &scenario), text, "nothing changed yet");
        choose(&mut harness, "Continue");
        assert!(!has_label(&harness, COMMENT_PROMPT), "the prompt closed");

        assert_eq!(nodes(&harness, &scenario), before + 1, "one check card");
        assert!(
            source_of(&harness, &scenario).contains("add_to_cart.productId == 123"),
            "the source gains the field's assertion:\n{}",
            source_of(&harness, &scenario)
        );
        assert!(has_label(&harness, "Outputs  1"), "Outputs still shown");
        reads_added(&harness, "productId");
        assert_eq!(bar(&harness), Some(ONE), "the check is counted");
        offers_check(&harness, "total");

        check(&mut harness, "total");
        assert!(!has_label(&harness, COMMENT_PROMPT), "asked only once");
        assert_eq!(nodes(&harness, &scenario), before + 2, "a second card");
        reads_added(&harness, "productId");
        reads_added(&harness, "total");
        assert_eq!(bar(&harness), Some(TWO), "both checks are counted");
        assert!(folder(&harness).runs.handle.is_none(), "without a run");
        assert_eq!(
            folder(&harness).run_result(&scenario).unwrap().id,
            report.id,
            "the same results"
        );
    }

    /// UX-35 criterion 2: a cancelled prompt adds nothing and leaves the
    /// state unchanged: the source, the cards, the fresh results whose rows
    /// still offer *Check*; asked again and confirmed, the check counts once.
    #[test]
    fn ux35_criterion_2_a_cancelled_prompt_adds_nothing() {
        let (_project, mut harness, scenario, report, text) = after_a_run_with_a_loose_comment();
        let before = nodes(&harness, &scenario);

        check(&mut harness, "productId");
        assert!(has_label(&harness, COMMENT_PROMPT), "the edit asks first");
        choose(&mut harness, "Cancel");
        assert!(!has_label(&harness, COMMENT_PROMPT), "the prompt closed");

        assert_eq!(source_of(&harness, &scenario), text, "the source is kept");
        assert_eq!(nodes(&harness, &scenario), before, "no card");
        assert_eq!(bar(&harness), None, "the results are fresh, no count");
        offers_check(&harness, "productId");
        offers_check(&harness, "total");
        assert_eq!(
            folder(&harness).run_result(&scenario).unwrap().id,
            report.id,
            "the same results"
        );

        check(&mut harness, "productId");
        choose(&mut harness, "Continue");
        assert_eq!(nodes(&harness, &scenario), before + 1, "one card");
        reads_added(&harness, "productId");
        assert_eq!(bar(&harness), Some(ONE), "counted once");
    }
}

// ----- UX-02: the run mode is a setting ---------------------------------------

/// UX-02 (#99): the window keeps a run mode per person and per project, the
/// header's one Run button runs it and says which, and *Run options* is a
/// chevron at the button's right (docs/tickets/UX-02.md). Every test uses
/// the demo shop and a configuration directory of its own.
mod ux02 {
    use super::super::accessibility_tests::{open_by_keyboard, tab_order};
    use super::mcp08b::{agent, await_endpoint, await_until, contains, hand_over, read_only};
    use super::mcp09::{agent_holds, files, manifest_id, set_box, window};
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use apiforge_core::run::RunMode;
    use apiforge_storage::project::HolderRole;
    use egui_kittest::kittest::NodeT;
    use serde_json::{Value, json};
    use std::{
        io::{Read as _, Write as _},
        path::{Path, PathBuf},
        sync::Mutex,
    };

    const SIMULATE: &str = "Run simulation";
    const LIVE: &str = "Run live";
    const OPTIONS: &str = "Run options";
    const MENU: &str = "Run options menu";
    const CAPTION: &str = "RUN MODE";
    const SIMULATION_ITEM: &str = "Simulation";
    const LIVE_ITEM: &str = "Live";
    const SIMULATION_HINT: &str = "Sample responses; no request is sent.";
    const LIVE_HINT: &str = "Requests go to the project's API.";
    const LINE: &str = "The Run button runs this mode until you change it.";
    const REVIEW: &str = "Review legacy runs";
    const ONE_OFF: &str = "Run live requests";
    const SIMULATE_HOVER: &str = "Simulate the selected scenario or component with sample responses, including unsaved valid code";
    const LIVE_HOVER: &str = "Send the requests of the selected scenario or component to the API, including unsaved valid code";
    const BADGE: &str = "Live allowed";
    const BADGE_HOVER: &str =
        "An MCP agent may send live requests in this project. Change it in Project settings.";
    const TAKE_BACK: &str = "Take back";
    /// The demo shop's name, as the header shows it.
    const NAME: &str = "Demo shop";

    fn mode_file(configuration: &Path) -> PathBuf {
        configuration.join("apiforge").join("run-mode.json")
    }

    fn consent_file(configuration: &Path) -> PathBuf {
        configuration.join("apiforge").join("agent-consent.json")
    }

    fn disabled(harness: &FolderHarness, label: &str) -> bool {
        harness.get_by_label(label).accesskit_node().is_disabled()
    }

    /// Opens the chevron's menu.
    fn open_options(harness: &mut FolderHarness) {
        choose(harness, OPTIONS);
        assert!(has_label(harness, MENU), "the chevron opens {MENU}");
    }

    fn close_options(harness: &mut FolderHarness) {
        if has_label(harness, MENU) {
            harness.key_press(Key::Escape);
            settle(harness);
        }
        assert!(!has_label(harness, MENU), "the menu closed");
    }

    /// Chooses `item`, *Simulation* or *Live*, in the chevron's menu.
    pub(super) fn choose_mode(harness: &mut FolderHarness, item: &str) {
        open_options(harness);
        choose(harness, item);
        close_options(harness);
    }

    /// Whether hovering the widget `label` shows a tooltip with `text`.
    pub(super) fn hover_shows(harness: &mut FolderHarness, label: &str, text: &str) -> bool {
        harness.get_by_label(label).hover();
        settle(harness);
        harness.run_steps(60);
        contains(harness, text)
    }

    /// The project name in the header (the explorer may name it too).
    pub(super) fn title(harness: &FolderHarness) -> egui::Rect {
        harness
            .query_all_by_label(NAME)
            .map(|node| node.rect())
            .find(|rect| rect.top() < 40.0)
            .expect("the project name in the header")
    }

    /// A loopback API that records each request line and answers every
    /// request with a product.
    fn api() -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let lines = Arc::clone(&seen);
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let lines = Arc::clone(&lines);
                std::thread::spawn(move || {
                    let mut buffer = [0; 4096];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let text = String::from_utf8_lossy(&buffer[..read]).into_owned();
                    lines
                        .lock()
                        .unwrap()
                        .push(text.lines().next().unwrap_or_default().to_owned());
                    let body = r#"{"id":123,"name":"Live","price":10}"#;
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                });
            }
        });
        (base, seen)
    }

    /// The demo shop with its base URL at `base`.
    pub(super) fn shop_at(base: &str) -> tempfile::TempDir {
        let directory = shop();
        let manifest = directory.path().join("apiforge.project.json");
        let mut config: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        config["environment"]["baseUrl"] = json!(base);
        fs::write(manifest, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        directory
    }

    /// Criterion 1: a project never set shows *Run simulation*; after
    /// *Live* is chosen, *Run live*; after *Simulation*, *Run simulation*
    /// again. The hover text names the mode.
    #[test]
    fn ux02_criterion_1_the_button_reads_its_mode() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let mut harness = window(directory.path(), WIDE, configuration.path());
        assert!(has_label(&harness, SIMULATE), "a project never set");
        assert!(!has_label(&harness, LIVE));
        assert!(!has_label(&harness, "Run"), "no bare Run");
        assert!(hover_shows(&mut harness, SIMULATE, SIMULATE_HOVER));

        choose_mode(&mut harness, LIVE_ITEM);
        assert!(has_label(&harness, LIVE), "Live chosen");
        assert!(!has_label(&harness, SIMULATE));
        assert!(hover_shows(&mut harness, LIVE, LIVE_HOVER));

        choose_mode(&mut harness, SIMULATION_ITEM);
        assert!(has_label(&harness, SIMULATE), "Simulation chosen again");
        assert!(!has_label(&harness, LIVE));
    }

    /// Criterion 1: one violet, `112 · 76 · 182`, fills *Run simulation*
    /// and colours the SIMULATION badge; *Run live* keeps the accent. The
    /// colour on screen is the QA captures' (ADR 0013, decision 8).
    #[test]
    fn ux02_criterion_1_simulation_is_one_violet() {
        assert_eq!(theme::SIMULATION, egui::Color32::from_rgb(112, 76, 182));
        assert_eq!(
            super::super::chrome::run_button(false).primary_fill(),
            theme::SIMULATION,
            "Run simulation"
        );
        assert_eq!(
            super::super::chrome::run_button(true).primary_fill(),
            theme::ACCENT,
            "Run live"
        );
        assert_eq!(
            crate::report::mode_pill(RunMode::Simulation),
            ("SIMULATION", theme::SIMULATION)
        );
        assert_eq!(
            crate::report::mode_pill(RunMode::Live),
            ("LIVE API", theme::ACCENT)
        );
    }

    /// Criterion 2: the chevron *Run options* sits at the right of the Run
    /// button and opens the menu: the caption, *Simulation* and *Live* with
    /// their hints, the line, *Review legacy runs*; no *Run live requests*;
    /// on a scenario's and on a component's tab.
    #[test]
    fn ux02_criterion_2_the_chevron_opens_the_menu() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let mut harness = window(directory.path(), MINIMUM, configuration.path());
        let component = only(&harness, DocumentKind::Component);
        for tab in [only(&harness, DocumentKind::Scenario), component] {
            app(&mut harness).activate_tab(&tab);
            settle(&mut harness);
            let run = harness.get_by_label(SIMULATE).rect();
            let chevron = harness
                .get_by_role_and_label(egui::accesskit::Role::Button, OPTIONS)
                .rect();
            assert!(
                chevron.left() >= run.right() && chevron.left() - run.right() <= 4.0,
                "the chevron at the button's right: {run:?}, {chevron:?}"
            );
            assert!(
                (chevron.center().y - run.center().y).abs() <= 1.0,
                "on the button's row"
            );
            open_options(&mut harness);
            for label in [
                CAPTION,
                SIMULATION_ITEM,
                SIMULATION_HINT,
                LIVE_ITEM,
                LIVE_HINT,
                LINE,
                REVIEW,
            ] {
                assert!(has_label(&harness, label), "{label} in the menu");
            }
            assert!(!contains(&harness, ONE_OFF), "{ONE_OFF} is nowhere");
            let order: Vec<f32> = [CAPTION, SIMULATION_ITEM, LIVE_ITEM, LINE, REVIEW]
                .iter()
                .map(|label| harness.get_by_label(label).rect().top())
                .collect();
            assert!(order.is_sorted(), "top to bottom: {order:?}");
            close_options(&mut harness);
        }
    }

    /// Criterion 2: the owner's "a little more left margin": *Simulation*
    /// and *Live* start at least 8 px right of the RUN MODE caption.
    #[test]
    fn ux02_criterion_2_the_mode_items_are_indented() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        for size in [MINIMUM, WIDE] {
            let mut harness = window(directory.path(), size, configuration.path());
            open_options(&mut harness);
            let caption = harness.get_by_label(CAPTION).rect().left();
            for item in [SIMULATION_ITEM, LIVE_ITEM] {
                let left = harness.get_by_label(item).rect().left();
                assert!(
                    left >= caption + 8.0,
                    "{item} at {left}, the caption at {caption}"
                );
            }
            close_options(&mut harness);
        }
    }

    /// Criterion 2: on a scenario's tab at 960 × 540, with *Live allowed*
    /// beside it and *Run simulation*, the widest label, the project name is
    /// whole.
    #[test]
    fn ux02_criterion_2_the_project_name_is_whole_at_960() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let mut harness = window(directory.path(), MINIMUM, configuration.path());
        set_box(&mut harness, true);
        assert!(has_label(&harness, BADGE));
        assert!(has_label(&harness, SIMULATE));
        for mode in ["Code", "Diagram", "Split"] {
            choose(&mut harness, mode);
            let width = title(&harness).width();
            let whole = whole_width(&harness, NAME, theme::FONT_BODY);
            assert!(
                width + 1.0 >= whole,
                "{mode}: the name is {width} px wide, whole it is {whole} px"
            );
        }
    }

    /// Criterion 3: the mode is kept in `apiforge/run-mode.json` in the
    /// configuration directory by the manifest id; a second window on the
    /// project starts with it, a window on another project in Simulation;
    /// the project folder is byte-identical.
    #[test]
    fn ux02_criterion_3_the_mode_is_kept_per_project_and_the_project_is_untouched() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let root = directory.path();
        let other = tempfile::tempdir().unwrap();
        apiforge_project::prepare_project(other.path(), &apiforge_core::demos::tickets_project())
            .unwrap();
        assert_ne!(manifest_id(root), manifest_id(other.path()));

        let mut harness = window(root, WIDE, configuration.path());
        let before = files(root);
        assert!(has_label(&harness, SIMULATE), "a project never set");
        choose_mode(&mut harness, LIVE_ITEM);
        assert!(has_label(&harness, LIVE));
        settle(&mut harness);
        assert_eq!(files(root), before, "the project folder is byte-identical");
        drop(harness);

        let stored: Value =
            serde_json::from_slice(&fs::read(mode_file(configuration.path())).unwrap()).unwrap();
        assert_eq!(stored["version"], 1, "{stored}");
        let entry = &stored["projects"][manifest_id(root)];
        assert_eq!(entry["mode"], "live", "{stored}");
        assert!(entry["changedAt"].is_string(), "{stored}");
        assert!(
            stored["projects"].get(manifest_id(other.path())).is_none(),
            "{stored}"
        );

        let harness = window(root, WIDE, configuration.path());
        assert!(has_label(&harness, LIVE), "a second window starts live");
        drop(harness);
        let harness = window(other.path(), WIDE, configuration.path());
        assert!(
            has_label(&harness, SIMULATE),
            "another project starts in Simulation"
        );
        drop(harness);
    }

    /// Criterion 4: *Run simulation* gives a simulation and sends nothing to
    /// the project's API; *Run live* gives a live report and the API
    /// receives the requests.
    #[test]
    fn ux02_criterion_4_the_button_runs_the_chosen_mode() {
        let configuration = tempfile::tempdir().unwrap();
        let (base, requests) = api();
        let directory = shop_at(&base);
        let mut harness = window(directory.path(), WIDE, configuration.path());

        choose(&mut harness, SIMULATE);
        let report = finish_run(&mut harness);
        assert_eq!(report.mode, RunMode::Simulation);
        assert!(!report.steps.is_empty());
        assert!(
            requests.lock().unwrap().is_empty(),
            "a simulation sends nothing: {:?}",
            requests.lock().unwrap()
        );

        choose_mode(&mut harness, LIVE_ITEM);
        choose(&mut harness, LIVE);
        let report = finish_run(&mut harness);
        assert_eq!(report.mode, RunMode::Live);
        assert!(
            !requests.lock().unwrap().is_empty(),
            "a live run sends the requests"
        );
    }

    /// Criterion 4: while the window observes, the button reads and runs
    /// *Run simulation* whatever the stored mode, *Live* is disabled with
    /// MCP-08a's reason, the stored mode is kept, and *Run live* is back
    /// after *Take back*.
    #[test]
    fn ux02_criterion_4_observing_runs_a_simulation() {
        let configuration = tempfile::tempdir().unwrap();
        let (base, requests) = api();
        let directory = shop_at(&base);
        let root = directory.path();
        let mut harness = window(root, WIDE, configuration.path());
        await_endpoint(&mut harness, root, HolderRole::Window);
        choose_mode(&mut harness, LIVE_ITEM);
        assert!(has_label(&harness, LIVE));

        let mut server = agent(root, "Claude Code");
        hand_over(&mut harness, &mut server, "Written by the agent.");
        assert!(read_only(&harness), "the window observes");
        assert!(
            has_label(&harness, SIMULATE),
            "observing reads Run simulation"
        );
        assert!(!has_label(&harness, LIVE));
        open_options(&mut harness);
        assert!(disabled(&harness, LIVE_ITEM), "Live is disabled");
        assert!(hover_shows(&mut harness, LIVE_ITEM, runs::LIVE_PAUSED));
        close_options(&mut harness);

        choose(&mut harness, SIMULATE);
        let report = finish_run(&mut harness);
        assert_eq!(report.mode, RunMode::Simulation, "observing simulates");
        assert!(requests.lock().unwrap().is_empty(), "nothing was sent");
        let stored: Value =
            serde_json::from_slice(&fs::read(mode_file(configuration.path())).unwrap()).unwrap();
        assert_eq!(
            stored["projects"][manifest_id(root)]["mode"],
            "live",
            "the stored mode is kept"
        );

        choose(&mut harness, TAKE_BACK);
        await_until(&mut harness, Instant::now(), "the take-back", |harness| {
            !read_only(harness)
        });
        assert!(has_label(&harness, LIVE), "Run live is back");
        drop(harness);
        drop(server);
    }

    /// Review round 1 of #244: while the window observes, *Simulation* is
    /// the mode shown, and clicking it is still the person's choice: the
    /// stored *Live* becomes Simulation. Opening and closing the menu
    /// writes nothing.
    #[test]
    fn choosing_simulation_while_observing_replaces_a_stored_live() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let root = directory.path();
        let harness = {
            let mut harness = window(root, WIDE, configuration.path());
            choose_mode(&mut harness, LIVE_ITEM);
            harness
        };
        drop(harness);
        let stored = || -> Value {
            serde_json::from_slice(&fs::read(mode_file(configuration.path())).unwrap()).unwrap()
        };
        let owner = agent_holds(root);
        let mut harness = window(root, WIDE, configuration.path());
        assert!(has_label(&harness, "Observing"));
        assert!(has_label(&harness, SIMULATE));
        let before = fs::read(mode_file(configuration.path())).unwrap();
        open_options(&mut harness);
        close_options(&mut harness);
        assert_eq!(
            fs::read(mode_file(configuration.path())).unwrap(),
            before,
            "opening the menu writes nothing"
        );
        choose_mode(&mut harness, SIMULATION_ITEM);
        assert_eq!(
            stored()["projects"][manifest_id(root)]["mode"],
            "simulation",
            "the click is kept"
        );
        drop(harness);
        drop(owner);
        let harness = window(root, WIDE, configuration.path());
        assert!(
            has_label(&harness, SIMULATE),
            "Simulation after the holder left"
        );
        assert!(!has_label(&harness, LIVE));
    }

    /// Criterion 5: MCP-09's *Live allowed* badge keeps its text and hover
    /// beside *Run live*, and the run mode and the agent's consent are two
    /// files that never change each other.
    #[test]
    fn ux02_criterion_5_live_allowed_is_unchanged() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let root = directory.path();
        let mut harness = window(root, MINIMUM, configuration.path());
        set_box(&mut harness, true);
        let consent = fs::read(consent_file(configuration.path())).unwrap();
        assert!(!mode_file(configuration.path()).exists(), "no mode yet");

        choose_mode(&mut harness, LIVE_ITEM);
        assert!(has_label(&harness, LIVE));
        assert!(has_label(&harness, BADGE), "the badge beside Run live");
        assert!(hover_shows(&mut harness, BADGE, BADGE_HOVER));
        assert_disjoint(&harness, &[BADGE, LIVE, OPTIONS]);
        assert!(
            harness.get_by_label(BADGE).rect().right() < harness.get_by_label(LIVE).rect().left()
        );
        assert_eq!(
            fs::read(consent_file(configuration.path())).unwrap(),
            consent,
            "choosing a mode leaves the consent alone"
        );
        let mode = fs::read(mode_file(configuration.path())).unwrap();
        set_box(&mut harness, false);
        assert!(!has_label(&harness, BADGE));
        assert!(
            has_label(&harness, LIVE),
            "the consent leaves the mode alone"
        );
        assert_eq!(fs::read(mode_file(configuration.path())).unwrap(), mode);
    }

    /// Criterion 7: the header fits at both windows, in every mode, with
    /// *Live allowed*, in either run mode, on a scenario's and on a
    /// component's tab.
    #[test]
    fn ux02_criterion_7_the_header_fits() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        for size in [MINIMUM, WIDE] {
            let mut harness = window(directory.path(), size, configuration.path());
            set_box(&mut harness, true);
            let scenario = only(&harness, DocumentKind::Scenario);
            let component = only(&harness, DocumentKind::Component);
            for (item, run) in [(SIMULATION_ITEM, SIMULATE), (LIVE_ITEM, LIVE)] {
                choose_mode(&mut harness, item);
                for tab in [&scenario, &component] {
                    app(&mut harness).activate_tab(tab);
                    settle(&mut harness);
                    for mode in ["Code", "Diagram", "Split"] {
                        choose(&mut harness, mode);
                        let mut labels = vec![
                            "Project", BADGE, "Code", "Diagram", "Split", "Save", "Save all",
                            OPTIONS, run,
                        ];
                        if tab == &component {
                            labels.push("Publish");
                        }
                        let what = format!("{run}, {mode}, {tab} at {size:?}");
                        assert_within_window(&harness, &labels);
                        assert_disjoint(&harness, &labels);
                        let title = title(&harness);
                        assert!(
                            harness.ctx.content_rect().contains_rect(title),
                            "{what}: the name"
                        );
                        for label in &labels {
                            assert!(
                                !title.intersects(harness.get_by_label(label).rect()),
                                "{what}: the name overlaps {label}"
                            );
                        }
                    }
                }
            }
            drop(harness);
        }
    }

    /// Criterion 8: a tab that cannot run, a text file or none, keeps the
    /// *Runs* menu with *Review legacy runs*, whatever the mode.
    #[test]
    fn ux02_criterion_8_the_runs_menu_is_unchanged() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        fs::write(directory.path().join("README.md"), "# Shop\n").unwrap();
        let mut harness = window(directory.path(), WIDE, configuration.path());
        choose_mode(&mut harness, LIVE_ITEM);
        app(&mut harness).open_text("README.md");
        settle(&mut harness);
        for case in ["a text file", "no tab"] {
            if case == "no tab" {
                let tabs: Vec<String> = folder(&harness)
                    .workbench
                    .tabs()
                    .iter()
                    .map(|tab| tab.document_id.clone())
                    .collect();
                for tab in tabs {
                    app(&mut harness).close_tab(&tab);
                }
                settle(&mut harness);
                assert!(folder(&harness).workbench.active().is_none(), "no tab");
            }
            for absent in [SIMULATE, LIVE, OPTIONS] {
                assert!(!has_label(&harness, absent), "{case}: no {absent}");
            }
            choose(&mut harness, "Runs");
            assert!(has_label(&harness, "Runs menu"), "{case}");
            assert!(has_label(&harness, REVIEW), "{case}");
            assert!(!has_label(&harness, LIVE_ITEM), "{case}: no mode");
            harness.key_press(Key::Escape);
            settle(&mut harness);
        }
    }

    /// Criterion 9: Tab reaches the Run button, then the chevron, between
    /// *Project* and *Code*; the chevron is a button named *Run options*
    /// whose menu, opened from the keyboard, is named and reaches the modes.
    #[test]
    fn ux02_criterion_9_the_controls_are_accessible() {
        let directory = shop();
        let (context, mut app) = open(directory.path());
        let visited = tab_order(&mut app, &context, 23);
        let mut at = 0;
        for name in ["Project", SIMULATE, OPTIONS, "Code"] {
            at += visited[at..]
                .iter()
                .position(|visit| visit == name)
                .unwrap_or_else(|| panic!("{name} after {at} in {visited:#?}"));
        }
        open_by_keyboard(&mut app, &context, OPTIONS);
        let visited = tab_order(&mut app, &context, 3);
        assert_eq!(visited[0], MENU, "{visited:?}");
        assert!(
            visited.iter().any(|name| name == SIMULATION_ITEM),
            "{visited:?}"
        );
        frame(&mut app, &context, vec![key(Key::Escape, Modifiers::NONE)]);
    }
}

// ----- UX-05: Run results and Problems drag to the whole workspace ------------

/// UX-05 (#102): Run results and Problems are dragged up to the top of their
/// workspace and released there to take it, as KTLO-19's and KTLO-18's
/// buttons do, and dragged back down to return (docs/tickets/UX-05.md). A
/// drag is a press on a named handle, moves of at most 32 px with a frame
/// after each, and a release; the workspace is the area under the tab strip.
/// Every test runs the demo shop in Split at 960 × 540 unless it says so.
mod ux05 {
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};

    pub(super) const ZONE: &str = "Release to fill the workspace";
    pub(super) const PROBLEMS_ZONE: &str = "Release to expand problems";
    /// Run results' grip, the 80 × 14 handle centred 8 px under its top.
    pub(super) const GRIP: &str = "Resize run report";
    /// egui's own edge of Run results.
    pub(super) const EDGE: &str = "Resize run results";
    pub(super) const PROBLEMS_EDGE: &str = "Resize problems";
    pub(super) const MAXIMISE: &str = "Maximise run results";
    pub(super) const RESTORE: &str = "Restore run results";
    pub(super) const EXPAND: &str = "Expand problems";
    pub(super) const RESTORE_PROBLEMS: &str = "Restore problems";
    const POLICY: &str = "When a step fails";
    /// The owner's snap distance: the zone shows within 64 px of the top.
    pub(super) const SNAP: f32 = 64.0;
    /// What maximised Run results hide in Split (KTLO-19's test).
    pub(super) const HIDDEN: [&str; 4] =
        ["Resize code", "Contents of", "Diagram board", "Add step"];

    /// The demo shop at `size` in Split, after a passed simulation of its
    /// scenario, with the scenario's id.
    pub(super) fn ran(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), size);
        let scenario = only(&harness, DocumentKind::Scenario);
        app(&mut harness).activate_tab(&scenario);
        settle(&mut harness);
        choose(&mut harness, "Split");
        choose(&mut harness, "Run simulation");
        let report = finish_run(&mut harness);
        assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
        (project, harness, scenario)
    }

    /// The workspace: from the tab strip's bottom to the window's bottom,
    /// inside the harness's own central-panel margin (as KTLO-19 measures).
    pub(super) fn workspace(harness: &FolderHarness) -> egui::Rect {
        let tabs = egui::PanelState::load(&harness.ctx, egui::Id::new("folder-tabs"))
            .expect("the tab strip is a panel")
            .outer_rect;
        let margin = egui::Frame::central_panel(&harness.ctx.global_style()).inner_margin;
        let window = harness.ctx.content_rect();
        egui::Rect::from_min_max(
            egui::pos2(window.left(), tabs.bottom()),
            egui::pos2(window.right(), window.bottom() - margin.bottomf()),
        )
    }

    /// Run results' height range in `workspace`: at least 185 px (160 px
    /// when the workspace is 460 px or more), at most the workspace less the
    /// views' least height above them. Changed by the owner's decision of
    /// 2026-11-03 (UX-36): the range no longer ends at 560 px or the
    /// workspace less 200 px.
    fn normal_range(workspace: egui::Rect) -> (f32, f32) {
        let full = workspace.height();
        let min = if full < 460.0 { 185.0 } else { 160.0 };
        (min, (full - crate::report::VIEWS_LEAST_HEIGHT).max(min))
    }

    /// Where Run results' top is at the top of their range, the ceiling.
    fn ceiling_top(harness: &FolderHarness) -> f32 {
        let workspace = workspace(harness);
        workspace.bottom() - normal_range(workspace).1
    }

    /// Where UX-05 kept Run results' top outside the zone: 560 px or the
    /// workspace less 200 px from the bottom, which UX-36 dropped.
    fn old_ceiling_top(harness: &FolderHarness) -> f32 {
        let workspace = workspace(harness);
        let full = workspace.height();
        let min = if full < 460.0 { 185.0 } else { 160.0 };
        workspace.bottom() - (full - 200.0).clamp(min, 560.0)
    }

    /// The top of Run results, from its grip.
    pub(super) fn panel_top(harness: &FolderHarness) -> f32 {
        harness.get_by_label(GRIP).rect().center().y - 8.0
    }

    /// What a failure message needs to tell a panel that went wrong from a
    /// workspace that changed under it.
    pub(super) fn state(harness: &FolderHarness) -> String {
        let app = folder(harness);
        format!(
            "workspace {:?}, {}, storage {:?}, notices {}",
            workspace(harness),
            app.report_state(),
            app.storage.label(),
            app.has_notices()
        )
    }

    /// The rect of the maximised panel (KTLO-19's id for that state).
    fn maximised_rect(harness: &FolderHarness) -> egui::Rect {
        egui::PanelState::load(&harness.ctx, egui::Id::new("report-maximised"))
            .expect("a maximised panel")
            .outer_rect
    }

    /// The active tab's Run results fill the workspace (KTLO-19's state).
    pub(super) fn maximised(harness: &FolderHarness) -> bool {
        let app = folder(harness);
        app.report_maximised(app.workbench.active())
    }

    /// The drop zone named `label`, while it is shown.
    pub(super) fn zone(harness: &FolderHarness, label: &str) -> Option<egui::Rect> {
        harness.query_by_label(label).map(|zone| zone.rect())
    }

    /// Where a press on the grip lands: its centre.
    pub(super) fn grip(harness: &FolderHarness) -> egui::Pos2 {
        harness.get_by_label(GRIP).rect().center()
    }

    /// Where a press on a panel's own edge lands: on the edge, left of the
    /// grip that Run results centres on it.
    pub(super) fn edge(harness: &FolderHarness, label: &str) -> egui::Pos2 {
        let rect = harness.get_by_label(label).rect();
        egui::pos2(rect.left() + 60.0, rect.center().y)
    }

    /// The top of the Problems section: its edge lies on it.
    pub(super) fn problems_top(harness: &FolderHarness) -> f32 {
        harness.get_by_label(PROBLEMS_EDGE).rect().center().y
    }

    /// Presses the primary button at `at`, the pointer resting there first.
    pub(super) fn press(harness: &mut FolderHarness, at: egui::Pos2) {
        harness.hover_at(at);
        harness.step();
        harness.drag_at(at);
        harness.step();
    }

    /// Moves the pressed pointer from `from` to `to`, at most 32 px a frame,
    /// then rests it for the frame that draws the panel where it was taken.
    pub(super) fn move_to(harness: &mut FolderHarness, from: egui::Pos2, to: egui::Pos2) {
        let steps = ((to - from).length() / 32.0).ceil().max(1.0);
        for step in 1..=steps as usize {
            harness.hover_at(from + (to - from) * (step as f32 / steps));
            harness.step();
        }
        harness.step();
    }

    /// Releases the button at `at`; then the pointer leaves the window.
    pub(super) fn release(harness: &mut FolderHarness, at: egui::Pos2) {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
        harness.remove_cursor();
        settle(harness);
    }

    /// Drags the grip until the panel's top is `top`, and holds it there.
    pub(super) fn hold_grip_at(harness: &mut FolderHarness, top: f32) -> egui::Pos2 {
        let from = grip(harness);
        let to = egui::pos2(from.x, top + 8.0);
        press(harness, from);
        move_to(harness, from, to);
        to
    }

    /// Drags the grip 40 px under the workspace's top and releases it there.
    fn drag_to_the_top(harness: &mut FolderHarness) {
        let top = workspace(harness).top();
        let at = hold_grip_at(harness, top + 40.0);
        release(harness, at);
    }

    /// Criterion 1: dragged by the grip or by egui's edge to 40 px under the
    /// workspace's top, the panel follows past UX-05's ceiling and the zone
    /// *Release to fill the workspace* is between the workspace's top and the
    /// panel's; held 100 px under the top instead, there is no zone. Since
    /// the owner's decision of 2026-11-03 (UX-36) the panel itself stops
    /// where the views above keep their least height, in both cases.
    #[test]
    fn ux05_criterion_1_dragging_to_the_top_shows_the_drop_zone() {
        let (_project, mut harness, _) = ran(MINIMUM);
        let workspace = workspace(&harness);
        let ceiling = old_ceiling_top(&harness);
        let highest = ceiling_top(&harness);
        for handle in [GRIP, EDGE] {
            let from = if handle == GRIP {
                grip(&harness)
            } else {
                edge(&harness, EDGE)
            };
            // How far under the panel's top the press lands.
            let offset = from.y - panel_top(&harness);
            press(&mut harness, from);
            let near = egui::pos2(from.x, workspace.top() + 40.0 + offset);
            move_to(&mut harness, from, near);
            let top = panel_top(&harness);
            assert!(
                top < ceiling - SNAP && (top - highest).abs() <= 1.0,
                "{handle}: held 40 px under the top {}, the panel's top is {top}, not \
                 {highest} (UX-05's ceiling was {ceiling})",
                workspace.top()
            );
            let shown = zone(&harness, ZONE)
                .unwrap_or_else(|| panic!("{handle}: no *{ZONE}* 40 px under the top"));
            assert!(
                shown.top() >= workspace.top() - 1.0 && shown.bottom() <= top + 1.0,
                "{handle}: the zone {shown:?} is not between the top {} and the panel's top {top}",
                workspace.top()
            );
            let far = egui::pos2(from.x, workspace.top() + 100.0 + offset);
            move_to(&mut harness, near, far);
            assert!(
                (panel_top(&harness) - (workspace.top() + 100.0).max(highest)).abs() <= 2.0,
                "{handle}: held 100 px under the top, the panel's top is {}",
                panel_top(&harness)
            );
            assert!(
                zone(&harness, ZONE).is_none(),
                "{handle}: a zone 100 px under the top"
            );
            release(&mut harness, far);
        }
    }

    /// Criterion 2: released in the zone, the panel is maximised as the
    /// button maximises it: the button reads *Restore run results*, the board
    /// and the code are gone, the grip stays at the panel's top, and the
    /// panel's rect is the one *Maximise run results* gives.
    #[test]
    fn ux05_criterion_2_the_drag_and_the_button_reach_one_state() {
        let (_project, mut harness, _) = ran(MINIMUM);
        let workspace = workspace(&harness);
        drag_to_the_top(&mut harness);
        assert!(maximised(&harness), "released in the zone, not maximised");
        assert!(
            has_label(&harness, RESTORE) && !has_label(&harness, MAXIMISE),
            "released in the zone, the button is not *{RESTORE}*"
        );
        for label in HIDDEN {
            assert!(
                harness.query_by_label_contains(label).is_none(),
                "{label:?} is drawn while maximised by the drag"
            );
        }
        assert!(
            zone(&harness, ZONE).is_none(),
            "the zone stays after the release"
        );
        let dragged = maximised_rect(&harness);
        assert!(
            (dragged.top() - workspace.top()).abs() <= 1.0,
            "the maximised panel {dragged:?} does not start at the workspace's top {}",
            workspace.top()
        );
        assert!(
            (panel_top(&harness) - dragged.top()).abs() <= 1.0,
            "the grip is not at the maximised panel's top {}: {:?}",
            dragged.top(),
            harness.get_by_label(GRIP).rect()
        );

        choose(&mut harness, RESTORE);
        assert!(!maximised(&harness), "*{RESTORE}* restores");
        choose(&mut harness, MAXIMISE);
        let clicked = maximised_rect(&harness);
        assert!(
            (dragged.min - clicked.min).length() <= 0.5
                && (dragged.max - clicked.max).length() <= 0.5,
            "the drag gives {dragged:?}, the button {clicked:?}"
        );
        assert!(has_label(&harness, RESTORE), "the button reads *{RESTORE}*");
        assert!(
            (panel_top(&harness) - clicked.top()).abs() <= 1.0,
            "the button's maximised panel has its grip at its top too"
        );
    }

    /// Criterion 2 by egui's own edge: released in the zone, the panel is
    /// maximised as by the grip.
    #[test]
    fn ux05_the_edge_released_in_the_zone_maximises_too() {
        let (_project, mut harness, _) = ran(MINIMUM);
        let workspace = workspace(&harness);
        let from = edge(&harness, EDGE);
        let to = egui::pos2(from.x, workspace.top() + 40.0);
        press(&mut harness, from);
        move_to(&mut harness, from, to);
        assert!(zone(&harness, ZONE).is_some(), "no zone by the edge");
        release(&mut harness, to);
        assert!(maximised(&harness), "released in the zone, not maximised");
        assert!(has_label(&harness, RESTORE));
        assert!(
            (maximised_rect(&harness).top() - workspace.top()).abs() <= 1.0,
            "the maximised panel starts at the workspace's top"
        );
    }

    /// Held above UX-05's ceiling, Run results leave the strip above them to
    /// the board and the code, which stay drawn during the drag and after a
    /// release outside the zone. Inverted by the owner's decision of
    /// 2026-11-03 (UX-36): UX-05 drew neither until the release.
    #[test]
    fn ux05_above_the_old_ceiling_the_views_above_are_drawn() {
        for size in [MINIMUM, WIDE] {
            let (_project, mut harness, _) = ran(size);
            let workspace = workspace(&harness);
            let at = hold_grip_at(&mut harness, workspace.top() + SNAP + 40.0);
            for label in HIDDEN {
                assert!(
                    harness.query_by_label_contains(label).is_some(),
                    "{size:?}: {label:?} is not drawn above the dragged panel"
                );
            }
            release(&mut harness, at);
            for label in HIDDEN {
                assert!(
                    harness.query_by_label_contains(label).is_some(),
                    "{size:?}: {label:?} is not back after the release"
                );
            }
        }
    }

    /// Criterion 3: in Code and in Split, *Resize problems* dragged to within
    /// 64 px of KTLO-18's ceiling shows *Release to expand problems* over the
    /// strip of code; released there, the section is expanded as *Expand
    /// problems* expands it, with the strip of code above it, and *Restore
    /// problems* returns the height from before the drag. Released outside
    /// the zone past 360 px, the section keeps the height it is released at
    /// (the owner's decision of 2026-11-03, UX-36; UX-05 returned it to
    /// 360 px): at 1600 × 1000, as at 960 × 540 the ceiling is under
    /// 360 + 64 px, and with problems listed, as a section with none keeps
    /// only the height of its caption and hint, as it does today.
    #[test]
    fn ux05_criterion_3_problems_drag_to_expand_keeps_the_strip() {
        for mode in ["Code", "Split"] {
            let project = crate::testing::shop_project();
            let mut harness = folder_harness(project.path(), MINIMUM);
            choose(&mut harness, mode);
            let rested = problems_top(&harness);
            choose(&mut harness, EXPAND);
            let expanded = problems_top(&harness);
            // The strip of code above the expanded section begins with the
            // editor, under the code panel's 6 px margin.
            let strip_top = harness.get_by_label_contains("Contents of").rect().top() - 8.0;
            choose(&mut harness, RESTORE_PROBLEMS);
            assert!((problems_top(&harness) - rested).abs() <= 1.0);

            let from = edge(&harness, PROBLEMS_EDGE);
            press(&mut harness, from);
            let far = egui::pos2(from.x, expanded + 100.0);
            move_to(&mut harness, from, far);
            assert!(
                zone(&harness, PROBLEMS_ZONE).is_none(),
                "{mode}: a zone 100 px under the ceiling"
            );
            let near = egui::pos2(from.x, expanded + 30.0);
            move_to(&mut harness, far, near);
            let shown = zone(&harness, PROBLEMS_ZONE)
                .unwrap_or_else(|| panic!("{mode}: no *{PROBLEMS_ZONE}* 30 px under the ceiling"));
            assert!(
                shown.bottom() <= expanded + 1.0 && shown.top() >= strip_top,
                "{mode}: the zone {shown:?} is not over the strip of code above {expanded}"
            );
            release(&mut harness, near);
            assert!(
                folder(&harness).problems_expanded,
                "{mode}: released in the zone, the problems are not expanded"
            );
            assert!(
                has_label(&harness, RESTORE_PROBLEMS) && !has_label(&harness, EXPAND),
                "{mode}: the button is not *{RESTORE_PROBLEMS}*"
            );
            assert!(
                (problems_top(&harness) - expanded).abs() <= 1.0,
                "{mode}: expanded by the drag, the section's top is {}, not {expanded} \
                 where the button leaves the strip of code",
                problems_top(&harness)
            );
            assert!(zone(&harness, PROBLEMS_ZONE).is_none());
            choose(&mut harness, RESTORE_PROBLEMS);
            assert!(
                (problems_top(&harness) - rested).abs() <= 1.0,
                "{mode}: restored to {}, not the height before the drag ({rested})",
                problems_top(&harness)
            );
        }

        let project = crate::testing::shop_project();
        let mut harness = folder_harness(project.path(), WIDE);
        choose(&mut harness, "Code");
        let scenario = only(&harness, DocumentKind::Scenario);
        app(&mut harness).activate_tab(&scenario);
        settle(&mut harness);
        // Three lines that do not parse, so the list has rows (KTLO-18's test).
        let label = editor(&harness, &scenario);
        replace_text(&mut harness, &label, "scenario \"Main\" {\n  ???\n}\n");
        choose(&mut harness, EXPAND);
        let expanded = problems_top(&harness);
        choose(&mut harness, RESTORE_PROBLEMS);
        let from = edge(&harness, PROBLEMS_EDGE);
        let to = egui::pos2(from.x, expanded + SNAP + 100.0);
        press(&mut harness, from);
        move_to(&mut harness, from, to);
        assert!(
            (problems_top(&harness) - to.y).abs() <= 2.0,
            "the section does not follow past 360 px: its top is {}, the pointer at {}",
            problems_top(&harness),
            to.y
        );
        assert!(zone(&harness, PROBLEMS_ZONE).is_none());
        release(&mut harness, to);
        assert!(
            !folder(&harness).problems_expanded,
            "outside the zone, expanded"
        );
        assert!(has_label(&harness, EXPAND));
        assert!(
            (problems_top(&harness) - to.y).abs() <= 2.0,
            "released outside the zone at {}, the section's top is {} ({} px high)",
            to.y,
            problems_top(&harness),
            folder(&harness).workbench.panels.diagnostics
        );
    }

    /// Criterion 6: released above UX-05's ceiling but outside the zone, the
    /// panel keeps the height it is released at; nothing is maximised and
    /// there is no zone. Changed by the owner's decision of 2026-11-03
    /// (UX-36): UX-05 returned the panel to its ceiling.
    #[test]
    fn ux05_criterion_6_releasing_outside_the_zone_keeps_the_height() {
        for size in [MINIMUM, WIDE] {
            let (_project, mut harness, _) = ran(size);
            let workspace = workspace(&harness);
            let ceiling = old_ceiling_top(&harness);
            let at = hold_grip_at(&mut harness, workspace.top() + SNAP + 40.0);
            let held = panel_top(&harness);
            assert!(
                held < ceiling - SNAP,
                "{size:?}: held above UX-05's ceiling {ceiling}, the panel's top is {held}"
            );
            assert!(
                zone(&harness, ZONE).is_none(),
                "{size:?}: a zone outside it"
            );
            release(&mut harness, at);
            assert!(!maximised(&harness), "{size:?}: maximised outside the zone");
            assert!(has_label(&harness, MAXIMISE));
            assert!(zone(&harness, ZONE).is_none());
            assert!(
                (panel_top(&harness) - held).abs() <= 1.0,
                "{size:?}: released outside the zone, the panel's top is {}, not {held} \
                 where it was released ({})",
                panel_top(&harness),
                state(&harness)
            );
        }
    }

    /// Criterion 7: maximised, the grip dragged down 200 px and released
    /// restores the panel where it was released, within its range (since the
    /// owner's decision of 2026-11-03, UX-36, up to the views' least height
    /// instead of UX-05's ceiling). At 1600 × 1000 also 500 px.
    #[test]
    fn ux05_criterion_7_dragging_down_restores() {
        for (size, distance) in [(MINIMUM, 200.0), (WIDE, 200.0), (WIDE, 500.0)] {
            let (_project, mut harness, _) = ran(size);
            choose(&mut harness, MAXIMISE);
            let workspace = workspace(&harness);
            let from = grip(&harness);
            let to = from + egui::vec2(0.0, distance);
            press(&mut harness, from);
            move_to(&mut harness, from, to);
            release(&mut harness, to);
            assert!(
                !maximised(&harness),
                "{size:?}, {distance} px: still maximised"
            );
            assert!(
                has_label(&harness, MAXIMISE) && !has_label(&harness, RESTORE),
                "{size:?}, {distance} px: the button is not *{MAXIMISE}*"
            );
            let (min, max) = normal_range(workspace);
            let expected = workspace.bottom() - (workspace.height() - distance).clamp(min, max);
            assert!(
                (panel_top(&harness) - expected).abs() <= 1.0,
                "{size:?}, {distance} px: the panel's top is {}, not {expected} \
                 (the workspace {workspace:?} before the drag; now {})",
                panel_top(&harness),
                state(&harness)
            );
        }
    }

    /// Criterion 8: KTLO-19's exits restore a panel the drag maximised —
    /// Escape, the button, another tab, another mode — at the height it had
    /// before the drag began.
    #[test]
    fn ux05_criterion_8_the_exits_restore_the_height_before_the_drag() {
        let (_project, mut harness, scenario) = ran(MINIMUM);
        let component = only(&harness, DocumentKind::Component);
        app(&mut harness).activate_tab(&component);
        app(&mut harness).activate_tab(&scenario);
        settle(&mut harness);
        let before = panel_top(&harness);
        for exit in ["Escape", "button", "tab", "mode"] {
            drag_to_the_top(&mut harness);
            assert!(maximised(&harness), "{exit}: the drag did not maximise");
            match exit {
                "Escape" => {
                    harness.key_press(Key::Escape);
                    settle(&mut harness);
                }
                "button" => choose(&mut harness, RESTORE),
                "tab" => {
                    harness.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
                    settle(&mut harness);
                    assert_eq!(
                        folder(&harness).workbench.active(),
                        Some(component.as_str())
                    );
                    harness.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
                    settle(&mut harness);
                }
                _ => {
                    choose(&mut harness, "Diagram");
                    assert!(!maximised(&harness), "another mode restores");
                    choose(&mut harness, "Split");
                }
            }
            assert!(!maximised(&harness), "{exit}: still maximised");
            assert!(has_label(&harness, MAXIMISE), "{exit}: the button");
            assert!(
                (panel_top(&harness) - before).abs() <= 1.0,
                "{exit}: restored with its top at {}, not {before} as before the drag",
                panel_top(&harness)
            );
        }
    }

    /// Criterion 9: ↑ and End on the grip stop at the ceiling and do not
    /// maximise; ↓ lowers the panel by 32 px and Home to its least height. An
    /// arrow key also moves egui's focus on, as it does today, so the grip is
    /// focused again before each key. Since the owner's decision of
    /// 2026-11-03 (UX-36) the ceiling is the top of the range the drag has,
    /// where the views above keep their least height.
    #[test]
    fn ux05_criterion_9_the_keys_stop_at_the_ceiling() {
        fn press_on_grip(harness: &mut FolderHarness, key: Key) {
            harness.get_by_label(GRIP).focus();
            harness.step();
            harness.key_press(key);
            settle(harness);
        }
        for size in [MINIMUM, WIDE] {
            let (_project, mut harness, _) = ran(size);
            let ceiling = ceiling_top(&harness);
            for key in [Key::ArrowUp, Key::End] {
                // 16 presses: the range at 1600 × 1000 now reaches 786 px (UX-36).
                for _ in 0..16 {
                    press_on_grip(&mut harness, key);
                }
                assert!(
                    !maximised(&harness),
                    "{size:?}: {key:?} maximised the panel"
                );
                assert!(
                    zone(&harness, ZONE).is_none(),
                    "{size:?}: {key:?} showed the zone"
                );
                assert!(
                    (panel_top(&harness) - ceiling).abs() <= 1.0,
                    "{size:?}: {key:?}: the panel's top is {}, not the ceiling {ceiling}",
                    panel_top(&harness)
                );
            }
            press_on_grip(&mut harness, Key::ArrowDown);
            let lowered = panel_top(&harness);
            assert!(
                (lowered - (ceiling + 32.0)).abs() <= 1.0,
                "{size:?}: ↓ lowers the panel by 32 px: {lowered}"
            );
            press_on_grip(&mut harness, Key::Home);
            let workspace = workspace(&harness);
            let (min, _) = normal_range(workspace);
            assert!(
                (panel_top(&harness) - (workspace.bottom() - min)).abs() <= 1.0,
                "{size:?}: Home gives the panel its least height: {}",
                panel_top(&harness)
            );
            assert!(!maximised(&harness) && zone(&harness, ZONE).is_none());
        }
    }

    /// The owner's decision of 2026-11-03: at its least height Run results
    /// clip nothing of the bar, the run's header and the first step row, at
    /// 960 × 540 and at 1600 × 1000.
    #[test]
    fn ux05_the_least_height_clips_no_control() {
        for size in [MINIMUM, WIDE] {
            let (_project, mut harness, scenario) = ran(size);
            harness.get_by_label(GRIP).focus();
            harness.step();
            harness.key_press(Key::Home);
            settle(&mut harness);
            let first = {
                let app = folder(&harness);
                let steps = &app.run_result(&scenario).unwrap().steps;
                let row = &crate::report::flatten_steps(steps)[0];
                format!(
                    "{} · {} · {} ms",
                    row.step.display_name(),
                    row.step.status.label(),
                    row.step.duration_ms
                )
            };
            let top = panel_top(&harness);
            let bottom = workspace(&harness).bottom();
            for label in [
                GRIP,
                "Run results",
                "PASSED",
                POLICY,
                MAXIMISE,
                "Export",
                first.as_str(),
            ] {
                let rect = harness.get_all_by_label(label).next().unwrap().rect();
                assert!(
                    rect.top() >= top - 1.0 && rect.bottom() <= bottom + 0.5,
                    "{size:?}: at the least height ({top}..{bottom}) {label:?} at {rect:?} is cut"
                );
            }
        }
    }

    /// Criterion 10: mid-drag, with the panel's top 64 px under the tab strip
    /// as in `ux05-mid-drag-*`, the zone's label, the grip, *Run results*,
    /// the status pill, the policy and the maximise button are within the
    /// window and disjoint, at 960 × 540 and 1600 × 1000.
    #[test]
    fn ux05_criterion_10_mid_drag_fits() {
        for size in [MINIMUM, WIDE] {
            let (_project, mut harness, _) = ran(size);
            let top = workspace(&harness).top() + SNAP;
            let at = hold_grip_at(&mut harness, top);
            let labels = [ZONE, GRIP, "Run results", "PASSED", POLICY, MAXIMISE];
            assert_within_window(&harness, &labels);
            assert_disjoint(&harness, &labels);
            release(&mut harness, at);
        }
    }
}

/// UX-36 (#260, the owner's feedback of 2026-11-03 on UX-05): Run results and
/// Problems resize freely, like the VS Code terminal panel. The panel stays
/// where it is released, with no clamp back to the old ceiling; the board or
/// the code stays drawn in the space above it, during the drag and after,
/// down to a header row and three lines of code; only a release within 64 px
/// of the top maximises, as UX-05 made it. A real pointer drag on the named
/// handles, as UX-05's tests drag them.
mod ux36 {
    use super::ux05::{
        EDGE, EXPAND, GRIP, MAXIMISE, PROBLEMS_EDGE, PROBLEMS_ZONE, RESTORE, RESTORE_PROBLEMS,
        SNAP, ZONE, edge, grip, hold_grip_at, maximised, move_to, panel_top, press, problems_top,
        ran, release, state, workspace, zone,
    };
    use super::*;
    use crate::testing::MINIMUM;

    /// The modes; Diagram draws the board above Run results, Code the code
    /// and its Problems, Split both.
    const MODES: [&str; 3] = ["Diagram", "Code", "Split"];
    /// The views above shrink to a small least height and no further
    /// (criterion 2): a header row and a few lines, at most this high.
    const SMALL: f32 = 120.0;
    /// The whole lines of code the code view keeps in view, at the least.
    const LINES: usize = 3;

    /// The demo shop at `size` after a passed simulation, shown in `mode`.
    fn ran_in(size: egui::Vec2, mode: &str) -> (tempfile::TempDir, FolderHarness) {
        let (project, mut harness, _) = ran(size);
        choose(&mut harness, mode);
        settle(&mut harness);
        (project, harness)
    }

    /// The views of `mode` are drawn in the strip between the workspace's
    /// top and Run results' top: the board fills it; the code shows at least
    /// three whole lines and the Problems caption row whole above the panel.
    fn assert_drawn_above(harness: &FolderHarness, mode: &str, when: &str) {
        let workspace = workspace(harness);
        let top = panel_top(harness);
        let strip = top - workspace.top();
        if mode != "Code" {
            let board = harness
                .query_by_label_contains("Diagram board")
                .unwrap_or_else(|| {
                    panic!(
                        "{when}: the board is not drawn above Run results at {top} ({})",
                        state(harness)
                    )
                })
                .rect();
            assert!(
                board.top() >= workspace.top() - 1.0
                    && board.bottom() <= top + 1.0
                    && board.height() >= strip - 1.0,
                "{when}: the board {board:?} does not fill the strip {}..{top} above Run results",
                workspace.top()
            );
        }
        if mode != "Diagram" {
            assert!(
                harness.query_by_label_contains("Contents of").is_some(),
                "{when}: the code is not drawn above Run results at {top} ({})",
                state(harness)
            );
            let lines = folder(harness)
                .code_lines_in_view()
                .map_or(0, |(_, lines)| lines.count());
            assert!(
                lines >= LINES,
                "{when}: {lines} whole lines of code above Run results at {top}, not {LINES}"
            );
            let caption = harness.get_by_label(EXPAND).rect();
            assert!(
                caption.top() >= workspace.top() - 1.0 && caption.bottom() <= top + 1.0,
                "{when}: the Problems caption row ({caption:?}) is not whole above Run results \
                 at {top}"
            );
        }
    }

    /// Criteria 1 and 2: dragged by the grip to 60 % of the workspace and then
    /// to 70 % — both past the old ceiling (the workspace less 200 px at
    /// 960 × 540, 560 px at 1600 × 1000) — the panel's top follows the
    /// pointer with the board or the code drawn above it, and released, the
    /// panel stays at that height, not maximised, with the views still drawn
    /// above it: in Diagram, Code and Split, at both sizes.
    #[test]
    fn ux36_released_anywhere_the_panel_stays_and_the_views_stay_drawn() {
        for size in [MINIMUM, WIDE] {
            for mode in MODES {
                let (_project, mut harness) = ran_in(size, mode);
                for share in [0.6, 0.7] {
                    let workspace = workspace(&harness);
                    let wanted = workspace.bottom() - share * workspace.height();
                    let at = hold_grip_at(&mut harness, wanted);
                    let when = format!("{size:?}, {mode}, held at {share}");
                    assert!(
                        (panel_top(&harness) - wanted).abs() <= 2.0,
                        "{when}: the panel's top is {}, not {wanted} ({})",
                        panel_top(&harness),
                        state(&harness)
                    );
                    assert!(zone(&harness, ZONE).is_none(), "{when}: a zone");
                    assert_drawn_above(&harness, mode, &when);
                    release(&mut harness, at);
                    let when = format!("{size:?}, {mode}, released at {share}");
                    assert!(!maximised(&harness), "{when}: maximised");
                    assert!(has_label(&harness, MAXIMISE), "{when}: the button");
                    assert!(
                        (panel_top(&harness) - wanted).abs() <= 2.0,
                        "{when}: the panel's top is {}, not {wanted} where it was released ({})",
                        panel_top(&harness),
                        state(&harness)
                    );
                    assert_drawn_above(&harness, mode, &when);
                }
            }
        }
    }

    /// Criterion 2: held as high as it goes outside the snap zone, 10 px and
    /// then 30 px under the zone's edge, the panel stops at one height that
    /// leaves the views a header row and three lines, at most 120 px, and
    /// they stay drawn; released there it stays. Problems, squeezed under the
    /// code meanwhile, keep the height they had once the panel goes down.
    #[test]
    fn ux36_the_views_above_keep_their_least_height() {
        for size in [MINIMUM, WIDE] {
            for mode in MODES {
                let (_project, mut harness) = ran_in(size, mode);
                let problems = folder(&harness).workbench.panels.diagnostics;
                let workspace = workspace(&harness);
                let rested = panel_top(&harness);
                let when = format!("{size:?}, {mode}");
                let from = grip(&harness);
                press(&mut harness, from);
                let near = egui::pos2(from.x, workspace.top() + SNAP + 10.0 + 8.0);
                move_to(&mut harness, from, near);
                let highest = panel_top(&harness);
                let far = egui::pos2(from.x, workspace.top() + SNAP + 30.0 + 8.0);
                move_to(&mut harness, near, far);
                assert!(
                    (panel_top(&harness) - highest).abs() <= 1.0,
                    "{when}: the panel's top moves from {highest} to {} above its highest",
                    panel_top(&harness)
                );
                assert!(
                    highest - workspace.top() <= SMALL,
                    "{when}: at its highest the panel leaves {} px above it, more than {SMALL}",
                    highest - workspace.top()
                );
                assert!(zone(&harness, ZONE).is_none(), "{when}: a zone");
                assert_drawn_above(&harness, mode, &format!("{when}, held at the highest"));
                release(&mut harness, far);
                assert!(!maximised(&harness), "{when}: maximised outside the zone");
                assert!(
                    (panel_top(&harness) - highest).abs() <= 1.0,
                    "{when}: released at its highest {highest}, the panel's top is {} ({})",
                    panel_top(&harness),
                    state(&harness)
                );
                assert_drawn_above(&harness, mode, &format!("{when}, released at the highest"));
                let from = grip(&harness);
                let back = egui::pos2(from.x, rested + 8.0);
                press(&mut harness, from);
                move_to(&mut harness, from, back);
                release(&mut harness, back);
                assert!(
                    (panel_top(&harness) - rested).abs() <= 2.0,
                    "{when}: dragged back to {rested}, the panel's top is {}",
                    panel_top(&harness)
                );
                let after = folder(&harness).workbench.panels.diagnostics;
                assert!(
                    (after - problems).abs() <= 1.0,
                    "{when}: Problems kept {after} px after the squeeze, not {problems}"
                );
            }
        }
    }

    /// Criterion 2, found by the native QA run: with Run results at their
    /// highest, the three lines of code are drawn above the Problems section
    /// and none under it. The editor's scroll area kept 64 px for itself,
    /// more than the code panel had, and painted its lines over the Problems
    /// caption row. In Code and Split, at both sizes.
    #[test]
    fn ux36_at_the_least_height_no_code_is_drawn_over_problems() {
        for size in [MINIMUM, WIDE] {
            for mode in ["Code", "Split"] {
                let (_project, mut harness) = ran_in(size, mode);
                let workspace = workspace(&harness);
                let at = hold_grip_at(&mut harness, workspace.top() + SNAP + 20.0);
                release(&mut harness, at);
                assert_drawn_above(&harness, mode, &format!("{size:?}, {mode}"));
                let code = egui::FontId::monospace(13.0);
                fn texts(
                    clip: egui::Rect,
                    shape: &egui::Shape,
                    into: &mut Vec<(egui::Rect, egui::epaint::TextShape)>,
                ) {
                    match shape {
                        egui::Shape::Vec(shapes) => {
                            for shape in shapes {
                                texts(clip, shape, into);
                            }
                        }
                        egui::Shape::Text(text) => into.push((clip, text.clone())),
                        _ => {}
                    }
                }
                let mut all = Vec::new();
                for clipped in &harness.output().shapes {
                    texts(clipped.clip_rect, &clipped.shape, &mut all);
                }
                // Where the section is drawn from: its caption's clip is the
                // section's own rect.
                let problems = all
                    .iter()
                    .find(|(_, text)| text.galley.job.text == "PROBLEMS")
                    .map(|(clip, _)| clip.top())
                    .unwrap_or_else(|| panic!("{size:?}, {mode}: no Problems caption"));
                {
                    for (clip, text) in all {
                        let shown = text.visual_bounding_rect().intersect(clip);
                        let monospace = text
                            .galley
                            .job
                            .sections
                            .first()
                            .is_some_and(|section| section.format.font_id == code);
                        assert!(
                            !monospace
                                || shown.is_negative()
                                || shown.top() >= panel_top(&harness) - 1.0
                                || shown.bottom() <= problems + 1.0,
                            "{size:?}, {mode}: {:?} is drawn at {shown:?}, under the Problems \
                             section's top {problems}",
                            text.galley.job.text.chars().take(40).collect::<String>()
                        );
                    }
                }
            }
        }
    }

    /// Criterion 3: held within 64 px of the top by the grip or by egui's
    /// edge, the zone *Release to fill the workspace* shows over the strip
    /// above Run results, with the views still drawn under it; released
    /// there, the panel is maximised and the button reads *Restore run
    /// results*. In every mode, at both sizes.
    #[test]
    fn ux36_only_the_snap_zone_maximises() {
        for size in [MINIMUM, WIDE] {
            for mode in MODES {
                let (_project, mut harness) = ran_in(size, mode);
                for handle in [GRIP, EDGE] {
                    let when = format!("{size:?}, {mode}, by {handle}");
                    let workspace = workspace(&harness);
                    let from = if handle == GRIP {
                        grip(&harness)
                    } else {
                        edge(&harness, EDGE)
                    };
                    let offset = from.y - panel_top(&harness);
                    let to = egui::pos2(from.x, workspace.top() + 40.0 + offset);
                    press(&mut harness, from);
                    move_to(&mut harness, from, to);
                    let shown = zone(&harness, ZONE)
                        .unwrap_or_else(|| panic!("{when}: no zone 40 px under the top"));
                    assert!(
                        shown.top() >= workspace.top() - 1.0
                            && shown.bottom() <= panel_top(&harness) + 1.0,
                        "{when}: the zone {shown:?} is not above Run results"
                    );
                    assert_drawn_above(&harness, mode, &format!("{when}, in the zone"));
                    release(&mut harness, to);
                    assert!(
                        maximised(&harness),
                        "{when}: released in the zone, not maximised"
                    );
                    assert!(
                        has_label(&harness, RESTORE) && !has_label(&harness, MAXIMISE),
                        "{when}: the button is not *{RESTORE}*"
                    );
                    choose(&mut harness, RESTORE);
                    assert!(!maximised(&harness), "{when}: *{RESTORE}* restores");
                }
            }
        }
    }

    /// Criterion 1, downwards: released at 70 %, then dragged down to 45 %
    /// and released, the panel follows and stays there; maximised, its grip
    /// dragged down to 65 % restores it at 65 %, not at the old ceiling.
    #[test]
    fn ux36_dragged_back_down_the_panel_resizes_freely() {
        for size in [MINIMUM, WIDE] {
            let (_project, mut harness) = ran_in(size, "Split");
            let workspace = workspace(&harness);
            for share in [0.7, 0.45] {
                let wanted = workspace.bottom() - share * workspace.height();
                let at = hold_grip_at(&mut harness, wanted);
                release(&mut harness, at);
                assert!(
                    (panel_top(&harness) - wanted).abs() <= 2.0,
                    "{size:?}: released at {share}, the panel's top is {}, not {wanted} ({})",
                    panel_top(&harness),
                    state(&harness)
                );
            }
            choose(&mut harness, MAXIMISE);
            let wanted = workspace.bottom() - 0.65 * workspace.height();
            let at = hold_grip_at(&mut harness, wanted);
            release(&mut harness, at);
            assert!(!maximised(&harness), "{size:?}: still maximised");
            assert!(
                (panel_top(&harness) - wanted).abs() <= 2.0,
                "{size:?}: dragged down from maximised to 65 %, the panel's top is {}, not \
                 {wanted} ({})",
                panel_top(&harness),
                state(&harness)
            );
            assert_drawn_above(
                &harness,
                "Split",
                &format!("{size:?}, restored by the drag"),
            );
        }
    }

    /// Criterion 4: Problems, with problems listed, dragged past 360 px but
    /// outside their zone and released, keep the height they are released
    /// at, with the code drawn above them; dragged back down, they follow;
    /// released in the zone, they expand as UX-05 made them, keeping
    /// KTLO-18's strip of code. In Code and Split at 1600 × 1000.
    #[test]
    fn ux36_problems_resize_freely_too() {
        for mode in ["Code", "Split"] {
            let project = crate::testing::shop_project();
            let mut harness = folder_harness(project.path(), WIDE);
            choose(&mut harness, mode);
            let scenario = only(&harness, DocumentKind::Scenario);
            app(&mut harness).activate_tab(&scenario);
            settle(&mut harness);
            let label = editor(&harness, &scenario);
            replace_text(&mut harness, &label, "scenario \"Main\" {\n  ???\n}\n");
            choose(&mut harness, EXPAND);
            let expanded = problems_top(&harness);
            choose(&mut harness, RESTORE_PROBLEMS);
            for wanted in [expanded + SNAP + 100.0, expanded + SNAP + 300.0] {
                let from = edge(&harness, PROBLEMS_EDGE);
                let to = egui::pos2(from.x, wanted);
                press(&mut harness, from);
                move_to(&mut harness, from, to);
                assert!(
                    zone(&harness, PROBLEMS_ZONE).is_none(),
                    "{mode}: a zone at {wanted}"
                );
                release(&mut harness, to);
                assert!(
                    !folder(&harness).problems_expanded,
                    "{mode}: released outside the zone at {wanted}, expanded"
                );
                assert!(
                    (problems_top(&harness) - wanted).abs() <= 2.0,
                    "{mode}: released at {wanted}, the section's top is {} (it holds {} px)",
                    problems_top(&harness),
                    folder(&harness).workbench.panels.diagnostics
                );
                let lines = folder(&harness)
                    .code_lines_in_view()
                    .map_or(0, |(_, lines)| lines.count());
                assert!(
                    lines >= LINES,
                    "{mode}: {lines} lines of code above Problems"
                );
            }
            let from = edge(&harness, PROBLEMS_EDGE);
            let to = egui::pos2(from.x, expanded + 30.0);
            press(&mut harness, from);
            move_to(&mut harness, from, to);
            assert!(
                zone(&harness, PROBLEMS_ZONE).is_some(),
                "{mode}: no zone 30 px under the ceiling"
            );
            release(&mut harness, to);
            assert!(
                folder(&harness).problems_expanded,
                "{mode}: released in the zone, not expanded"
            );
            assert!(
                (problems_top(&harness) - expanded).abs() <= 1.0,
                "{mode}: expanded by the drag, the section's top is {}, not {expanded}",
                problems_top(&harness)
            );
        }
    }
}

/// UX-04 (#101, resolves #152): while a run runs, the header says what runs,
/// just left of *Cancel run*, in grey, naming the file on another tab; when
/// the run ends the words go, and the band under the tab strip never holds
/// a *Running …* notice (docs/tickets/UX-04.md). A run that is still running
/// when a test looks is a live run against a loopback listener that accepts
/// and never answers; the run is cancelled at the end.
mod ux04 {
    use super::mcp08b::contains;
    use super::mcp09::{set_box, window};
    use super::ux02::{choose_mode, hover_shows, shop_at, title};
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use egui::accesskit::{Action, Role};
    use egui_kittest::kittest::NodeT;

    const SIMULATE: &str = "Run simulation";
    const LIVE: &str = "Run live";
    const LIVE_ITEM: &str = "Live";
    const CANCEL: &str = "Cancel run";
    const BADGE: &str = "Live allowed";
    const UNSAVED: &str = "Running unsaved code";
    const SAVED: &str = "Running saved code";
    const EARLIER: &str = "Running earlier code";
    const UNSAVED_HOVER: &str = "The run uses the code in the editor, which is not saved yet.";
    const EARLIER_HOVER: &str =
        "The editor's code has errors; the run uses the last confirmed code and configuration.";
    /// The demo shop's scenario, as the label names it on another tab.
    const FILE: &str = "promotion-and-cart-price.afs";

    /// A loopback API that accepts every connection and never answers, so a
    /// live run against it stays in progress until its step times out.
    fn silent_api() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming() {
                held.push(stream);
            }
        });
        base
    }

    /// A loopback API that closes every connection it accepts, so each
    /// request of a live run fails at once.
    fn closing_api() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                drop(stream);
            }
        });
        base
    }

    /// The window over `root` at `size`, the run mode Live, the scenario's
    /// tab active.
    fn live_window(
        root: &std::path::Path,
        size: egui::Vec2,
        configuration: &std::path::Path,
    ) -> FolderHarness {
        let mut harness = window(root, size, configuration);
        choose_mode(&mut harness, LIVE_ITEM);
        let scenario = only(&harness, DocumentKind::Scenario);
        app(&mut harness).activate_tab(&scenario);
        settle(&mut harness);
        harness
    }

    /// Replaces the first `from` in the document's code with `to`, unsaved.
    fn edit(harness: &mut FolderHarness, id: &str, from: &str, to: &str) {
        let source = source_of(harness, id);
        let edited = source.replacen(from, to, 1);
        assert_ne!(edited, source, "{from:?} is in the source");
        app(harness)
            .session
            .as_mut()
            .unwrap()
            .edit_source(id, &edited, CODE)
            .unwrap();
        app(harness).analyser.typed(id, Instant::now());
        settle(harness);
    }

    /// An unsaved, valid change to the scenario: its description.
    fn edit_validly(harness: &mut FolderHarness) {
        let scenario = only(harness, DocumentKind::Scenario);
        edit(
            harness,
            &scenario,
            "description \"",
            "description \"Edited. ",
        );
    }

    /// The header's *Running …* label: its text and rect. The header is the
    /// window's top band, above the tab strip.
    fn running(harness: &FolderHarness) -> Option<(String, egui::Rect)> {
        let strip = harness.get_by_label("Code").rect().bottom();
        let found: Vec<(String, egui::Rect)> = harness
            .query_all_by_label_contains("Running ")
            .filter(|node| node.rect().bottom() <= strip + 4.0)
            .map(|node| {
                let node_ref = node.accesskit_node();
                (
                    node_ref
                        .label()
                        .or_else(|| node_ref.value())
                        .unwrap_or_default(),
                    node.rect(),
                )
            })
            .collect();
        assert!(found.len() <= 1, "one label at most: {found:?}");
        found.into_iter().next()
    }

    /// No *Running …* notice in the band under the tab strip, nor anywhere
    /// below the header.
    fn assert_band_is_clear(harness: &FolderHarness, context: &str) {
        let notice = folder(harness)
            .notice
            .as_ref()
            .map(|notice| notice.text.clone());
        assert!(
            !notice
                .as_deref()
                .is_some_and(|text| text.starts_with("Running")),
            "{context}: the band holds {notice:?}"
        );
        let strip = harness.get_by_label("Code").rect().bottom();
        let below: Vec<String> = harness
            .query_all_by_label_contains("Running ")
            .filter(|node| node.rect().bottom() > strip + 4.0)
            .map(|node| format!("{:?}", node.accesskit_node().label()))
            .collect();
        assert!(below.is_empty(), "{context}: below the header {below:?}");
    }

    /// The label reads `text`, on *Cancel run*'s row, its right edge left of
    /// *Cancel run*'s left edge by at most the item spacing plus 8 px.
    fn assert_beside_cancel(harness: &FolderHarness, text: &str, context: &str) {
        let (shown, label) = running(harness).unwrap_or_else(|| panic!("{context}: no label"));
        assert_eq!(shown, text, "{context}");
        let cancel = harness.get_by_label(CANCEL).rect();
        assert!(
            (label.center().y - cancel.center().y).abs() <= 2.0,
            "{context}: on Cancel run's row: {label:?}, {cancel:?}"
        );
        let gap = cancel.left() - label.right();
        let spacing = harness.ctx.global_style().spacing.item_spacing.x;
        assert!(
            gap >= 0.0 && gap <= spacing + 8.0,
            "{context}: {gap} px between the label and Cancel run"
        );
    }

    /// Clicks *Cancel run* and steps until the run has ended.
    fn cancel(harness: &mut FolderHarness) {
        harness.get_by_label(CANCEL).click();
        let deadline = Instant::now() + Duration::from_secs(30);
        while folder(harness).runs.handle.is_some() {
            assert!(Instant::now() < deadline, "the run must end");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(harness);
    }

    /// Steps frame by frame until the run started from the window ends,
    /// asserting in every frame that the label shows exactly while the run
    /// runs: from the frame after the click until the frame the run ends.
    fn step_to_the_end(harness: &mut FolderHarness, context: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut frames = 0;
        loop {
            harness.step();
            frames += 1;
            let runs = folder(harness).runs.handle.is_some();
            let shown = running(harness);
            if !runs {
                assert_eq!(shown, None, "{context}: the frame the run ended");
                break;
            }
            if frames > 1 {
                assert!(
                    shown.is_some(),
                    "{context}: frame {frames} while the run runs"
                );
            }
            assert!(Instant::now() < deadline, "{context}: the run must end");
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(harness);
        assert_eq!(running(harness), None, "{context}: after the run");
        assert_band_is_clear(harness, context);
    }

    /// Criterion 1: after an unsaved valid change, *Running unsaved code*
    /// sits just left of *Cancel run*, grey at the label size, with its
    /// hover; the band holds no notice.
    #[test]
    fn ux04_criterion_1_unsaved_code_is_said_left_of_cancel_run() {
        let mut text = egui::text::LayoutJob::default();
        super::super::chrome::running_text(UNSAVED).append_to(
            &mut text,
            &egui::Style::default(),
            egui::FontSelection::Default,
            egui::Align::Center,
        );
        assert_eq!(text.sections.len(), 1);
        assert_eq!(text.sections[0].format.color, theme::MUTED, "grey");
        assert_eq!(
            text.sections[0].format.font_id.size,
            theme::FONT_LABEL,
            "the label size"
        );

        let configuration = tempfile::tempdir().unwrap();
        let base = silent_api();
        for size in [MINIMUM, WIDE] {
            let directory = shop_at(&base);
            let mut harness = live_window(directory.path(), size, configuration.path());
            assert_eq!(running(&harness), None, "{size:?}: no run, no label");
            edit_validly(&mut harness);
            choose(&mut harness, LIVE);
            assert!(
                folder(&harness).runs.handle.is_some(),
                "{size:?}: the run runs"
            );
            let context = format!("{size:?}");
            assert_beside_cancel(&harness, UNSAVED, &context);
            assert_band_is_clear(&harness, &context);
            assert!(
                hover_shows(&mut harness, UNSAVED, UNSAVED_HOVER),
                "{context}: hover"
            );
            cancel(&mut harness);
        }
    }

    /// Criterion 1: a run of saved code says so in the same place.
    #[test]
    fn ux04_criterion_1_saved_code_is_said_too() {
        let configuration = tempfile::tempdir().unwrap();
        let base = silent_api();
        let directory = shop_at(&base);
        let mut harness = live_window(directory.path(), MINIMUM, configuration.path());
        choose(&mut harness, LIVE);
        assert!(folder(&harness).runs.handle.is_some(), "the run runs");
        assert_beside_cancel(&harness, SAVED, "saved");
        assert_band_is_clear(&harness, "saved");
        cancel(&mut harness);
    }

    /// Criterion 1, refinement: a run accepted through *Run this version*
    /// while the code is invalid says *Running earlier code*, its full
    /// sentence on hover.
    #[test]
    fn ux04_criterion_1_earlier_code_is_said() {
        let configuration = tempfile::tempdir().unwrap();
        let base = silent_api();
        let directory = shop_at(&base);
        let mut harness = live_window(directory.path(), MINIMUM, configuration.path());
        let scenario = only(&harness, DocumentKind::Scenario);
        edit(&mut harness, &scenario, "{", "{ broken");
        choose(&mut harness, LIVE);
        assert!(has_label(&harness, "Run this version"), "the prompt");
        choose(&mut harness, "Run this version");
        assert!(folder(&harness).runs.handle.is_some(), "the run runs");
        assert_beside_cancel(&harness, EARLIER, "earlier");
        assert_band_is_clear(&harness, "earlier");
        assert!(hover_shows(&mut harness, EARLIER, EARLIER_HOVER), "hover");
        cancel(&mut harness);
    }

    /// Criterion 2: a simulation that passes; the label is gone the frame
    /// it ends and the band holds nothing from it.
    #[test]
    fn ux04_criterion_2_the_label_goes_when_the_run_passes() {
        let configuration = tempfile::tempdir().unwrap();
        let directory = shop();
        let mut harness = window(directory.path(), MINIMUM, configuration.path());
        edit_validly(&mut harness);
        harness.get_by_label(SIMULATE).click();
        step_to_the_end(&mut harness, "passed");
        let active = folder(&harness).workbench.active().unwrap().to_owned();
        assert_eq!(
            folder(&harness).run_result(&active).unwrap().status,
            RunStatus::Passed
        );
    }

    /// Criterion 2: a live run whose API closes every connection fails;
    /// the label goes the frame it ends.
    #[test]
    fn ux04_criterion_2_the_label_goes_when_the_run_fails() {
        let configuration = tempfile::tempdir().unwrap();
        let base = closing_api();
        let directory = shop_at(&base);
        let mut harness = live_window(directory.path(), MINIMUM, configuration.path());
        edit_validly(&mut harness);
        harness.get_by_label(LIVE).click();
        step_to_the_end(&mut harness, "failed");
        let active = folder(&harness).workbench.active().unwrap().to_owned();
        assert_eq!(
            folder(&harness).run_result(&active).unwrap().status,
            RunStatus::Failed
        );
    }

    /// Criterion 2: *Cancel run*; the label goes the frame the run ends.
    #[test]
    fn ux04_criterion_2_the_label_goes_when_the_run_is_cancelled() {
        let configuration = tempfile::tempdir().unwrap();
        let base = silent_api();
        let directory = shop_at(&base);
        let mut harness = live_window(directory.path(), MINIMUM, configuration.path());
        edit_validly(&mut harness);
        choose(&mut harness, LIVE);
        assert!(folder(&harness).runs.handle.is_some(), "the run runs");
        assert_beside_cancel(&harness, UNSAVED, "before the cancel");
        harness.get_by_label(CANCEL).click();
        step_to_the_end(&mut harness, "cancelled");
    }

    /// Criterion 3: on another tab the label names the file that runs; back
    /// on the run's tab it says *code* again.
    #[test]
    fn ux04_criterion_3_another_tab_names_the_file() {
        let configuration = tempfile::tempdir().unwrap();
        let base = silent_api();
        let directory = shop_at(&base);
        let mut harness = live_window(directory.path(), MINIMUM, configuration.path());
        let scenario = only(&harness, DocumentKind::Scenario);
        let component = only(&harness, DocumentKind::Component);
        edit_validly(&mut harness);
        choose(&mut harness, LIVE);
        assert!(folder(&harness).runs.handle.is_some(), "the run runs");
        app(&mut harness).activate_tab(&component);
        settle(&mut harness);
        assert_beside_cancel(
            &harness,
            &format!("Running unsaved {FILE}"),
            "the component's tab",
        );
        app(&mut harness).activate_tab(&scenario);
        settle(&mut harness);
        assert_beside_cancel(&harness, UNSAVED, "the run's tab");
        cancel(&mut harness);
    }

    /// Criterion 5: at both windows, in every mode, with *Live allowed*, on
    /// the run's tab and on a component's tab (*Publish*, and the label
    /// naming the file), the header's controls are within the window and
    /// disjoint, and the project name overlaps none of them.
    #[test]
    fn ux04_criterion_5_the_header_fits() {
        let configuration = tempfile::tempdir().unwrap();
        let base = silent_api();
        for size in [MINIMUM, WIDE] {
            let directory = shop_at(&base);
            let mut harness = live_window(directory.path(), size, configuration.path());
            set_box(&mut harness, true);
            let scenario = only(&harness, DocumentKind::Scenario);
            let component = only(&harness, DocumentKind::Component);
            edit_validly(&mut harness);
            choose(&mut harness, LIVE);
            assert!(folder(&harness).runs.handle.is_some(), "the run runs");
            let named = format!("Running unsaved {FILE}");
            for (tab, label) in [(&scenario, UNSAVED), (&component, named.as_str())] {
                app(&mut harness).activate_tab(tab);
                settle(&mut harness);
                for mode in ["Code", "Diagram", "Split"] {
                    choose(&mut harness, mode);
                    let what = format!("{mode}, {label} at {size:?}");
                    assert!(
                        folder(&harness).runs.handle.is_some(),
                        "{what}: still running"
                    );
                    let mut labels = vec![
                        "Project", BADGE, "Code", "Diagram", "Split", "Save", "Save all", label,
                        CANCEL,
                    ];
                    if tab == &component {
                        labels.push("Publish");
                    }
                    assert_within_window(&harness, &labels);
                    assert_disjoint(&harness, &labels);
                    let title = title(&harness);
                    for other in &labels {
                        assert!(
                            !title.intersects(harness.get_by_label(other).rect()),
                            "{what}: the name overlaps {other}"
                        );
                    }
                }
            }
            cancel(&mut harness);
        }
    }

    /// The width rule of the implementation: at 960 × 540 on a component's
    /// tab, where *Publish* also stands, the label naming the file is cut to
    /// the room the row leaves, and its hover gives the whole text with the
    /// sentence of an unsaved run.
    #[test]
    fn a_cut_label_gives_its_whole_text_on_hover() {
        let configuration = tempfile::tempdir().unwrap();
        let base = silent_api();
        let directory = shop_at(&base);
        let mut harness = live_window(directory.path(), MINIMUM, configuration.path());
        set_box(&mut harness, true);
        let component = only(&harness, DocumentKind::Component);
        edit_validly(&mut harness);
        choose(&mut harness, LIVE);
        app(&mut harness).activate_tab(&component);
        settle(&mut harness);
        let named = format!("Running unsaved {FILE}");
        let (shown, label) = running(&harness).expect("the label");
        assert_eq!(shown, named, "its name is the whole text");
        let whole = whole_width(&harness, &named, theme::FONT_LABEL);
        assert!(label.width() + 1.0 < whole, "cut: {label:?}, whole {whole}");
        let publish = harness.get_by_label("Publish").rect();
        assert!(publish.right() <= label.left(), "clear of Publish");
        harness.get_by_label(&named).hover();
        settle(&mut harness);
        harness.run_steps(60);
        assert_eq!(
            harness.query_all_by_label_contains(&named).count(),
            2,
            "the label and its hover, the whole text"
        );
        assert!(contains(&harness, UNSAVED_HOVER), "and the sentence");
        cancel(&mut harness);
    }

    /// Criterion 6: the label is a label for assistive technology, named by
    /// its text, and Tab never stops on it.
    #[test]
    fn ux04_criterion_6_the_label_is_named() {
        let configuration = tempfile::tempdir().unwrap();
        let base = silent_api();
        let directory = shop_at(&base);
        let mut harness = live_window(directory.path(), MINIMUM, configuration.path());
        edit_validly(&mut harness);
        choose(&mut harness, LIVE);
        assert!(folder(&harness).runs.handle.is_some(), "the run runs");
        assert!(running(&harness).is_some(), "the label shows");
        let node = harness.get_by_label(UNSAVED);
        let node = node.accesskit_node();
        assert_eq!(node.role(), Role::Label, "a label");
        assert!(!node.data().supports_action(Action::Focus), "not focusable");
        assert!(!contains(&harness, "Running unsaved code."), "no notice");
        cancel(&mut harness);
    }
}

/// UX-31 (#248, COMP-00 Q1 = C): a component's board shows its inputs in an
/// INPUTS strip under the board's header row, one pill per input and
/// *+ Add input*; selecting a pill rings the steps that read the input. The
/// strip stores nothing (docs/tickets/COMP-00.md).
mod ux31 {
    use super::mcp08b::contains;
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use egui::accesskit::Toggled;
    use egui_kittest::kittest::NodeT;

    const CART: &str = "components/add-to-cart.afs";
    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
    const PRODUCT: &str = "product * object · Product";
    const QUANTITY: &str = "quantity * number = 1";
    const ADD: &str = "Add input";
    const CAPTION: &str = "INPUTS";

    fn by_path(harness: &FolderHarness, path: &str) -> String {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == path)
            .unwrap_or_else(|| panic!("{path} is in the demo shop"))
            .id()
            .to_owned()
    }

    /// The demo shop at `size` with `path` open in `mode`.
    pub(super) fn window(
        root: &std::path::Path,
        size: egui::Vec2,
        path: &str,
        mode: &str,
    ) -> (FolderHarness, String) {
        let mut harness = folder_harness(root, size);
        let id = by_path(&harness, path);
        app(&mut harness).activate_tab(&id);
        settle(&mut harness);
        choose(&mut harness, mode);
        (harness, id)
    }

    /// The names of the cards the board rings, as their blocks are named.
    pub(super) fn ringed(harness: &FolderHarness, id: &str) -> Vec<String> {
        let app = folder(harness);
        let Some(Document::Component(component)) =
            app.session.as_ref().unwrap().document(id).unwrap().model()
        else {
            panic!("{id} is a component");
        };
        let mut names: Vec<String> = app
            .canvases
            .get(id)
            .expect("the board was drawn")
            .debug_ringed()
            .iter()
            .map(|ringed| {
                let node = component
                    .graph
                    .nodes
                    .iter()
                    .find(|node| &node.id == ringed)
                    .unwrap_or_else(|| panic!("{ringed} is a block of the component"));
                node.label.clone().unwrap_or_else(|| node.handle.clone())
            })
            .collect();
        names.sort();
        names
    }

    pub(super) fn toggled(harness: &FolderHarness, label: &str) -> Option<Toggled> {
        harness.get_by_label(label).accesskit_node().toggled()
    }

    fn click_at(harness: &mut FolderHarness, at: egui::Pos2) {
        harness.hover_at(at);
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
        settle(harness);
    }

    /// A point on the empty board just under the strip, at its left edge:
    /// inside the margin that fitting the view leaves free of cards.
    fn empty_board(harness: &FolderHarness) -> egui::Pos2 {
        let board = harness.get_by_label("Diagram board").rect();
        let pill = harness.get_by_label(PRODUCT).rect();
        egui::pos2(board.left() + 8.0, pill.bottom() + 20.0)
    }

    /// Criterion 1: in Diagram, the component's board shows the strip with
    /// `product` and `quantity` in declaration order, each with its required
    /// mark, its type and its contract or default, and *+ Add input* last;
    /// all of it under the board's header row, at both sizes.
    #[test]
    fn ux31_criterion_1_the_strip_shows_the_inputs_in_order() {
        for size in [MINIMUM, WIDE] {
            let directory = shop();
            let (harness, _) = window(directory.path(), size, CART, "Diagram");
            assert!(
                has_label(&harness, CAPTION),
                "the strip's caption at {size:?}"
            );
            let product = harness.get_by_label(PRODUCT).rect();
            let quantity = harness.get_by_label(QUANTITY).rect();
            let add = harness.get_by_label(ADD).rect();
            assert!(
                product.right() <= quantity.left() && quantity.right() <= add.left(),
                "product, quantity, then Add input: {product:?} {quantity:?} {add:?}"
            );
            assert!(
                (product.center().y - quantity.center().y).abs() < 1.0
                    && (quantity.center().y - add.center().y).abs() < 2.0,
                "one row"
            );
            // The tab strip names the file too; the board's header is lower.
            let header = harness
                .query_all_by_label("add-to-cart.afs")
                .map(|node| node.rect().bottom())
                .fold(f32::MIN, f32::max);
            assert!(header <= product.top(), "under the header row");
            let toolbar = harness.get_by_label("Add step").rect();
            assert!(toolbar.bottom() <= product.top(), "under the toolbar");
            assert_within_window(&harness, &[CAPTION, PRODUCT, QUANTITY, ADD]);
            assert_disjoint(&harness, &[CAPTION, PRODUCT, QUANTITY, ADD, "Add step"]);
        }
    }

    /// Criterion 1, Split: the strip is on the component's board beside the
    /// code too.
    #[test]
    fn ux31_criterion_1_the_strip_is_on_the_board_in_split() {
        let directory = shop();
        let (harness, _) = window(directory.path(), WIDE, CART, "Split");
        let board = harness.get_by_label("Diagram board").rect();
        for label in [CAPTION, PRODUCT, QUANTITY, ADD] {
            let rect = harness.get_by_label(label).rect();
            assert!(board.contains_rect(rect), "{label} on the board: {rect:?}");
        }
    }

    /// Criterion 2: a scenario's board has no strip.
    #[test]
    fn ux31_criterion_2_a_scenario_has_no_strip() {
        let directory = shop();
        for mode in ["Diagram", "Split"] {
            let (harness, _) = window(directory.path(), WIDE, SCENARIO, mode);
            assert!(harness.query_by_label("Diagram board").is_some(), "a board");
            assert!(!has_label(&harness, CAPTION), "no strip in {mode}");
            assert!(!has_label(&harness, ADD), "no Add input in {mode}");
            assert!(!contains(&harness, " * "), "no pill in {mode}");
        }
    }

    /// Criterion 3: clicking `quantity` rings the pill and *Cart item*, the
    /// one step that reads `param.quantity`, and no other card; a click on
    /// the empty board clears the ring.
    #[test]
    fn ux31_criterion_3_a_pill_rings_the_steps_that_read_it() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        assert!(ringed(&harness, &id).is_empty(), "nothing ringed at first");
        assert_eq!(toggled(&harness, QUANTITY), Some(Toggled::False));
        choose(&mut harness, QUANTITY);
        assert_eq!(ringed(&harness, &id), ["Cart item"]);
        assert_eq!(toggled(&harness, QUANTITY), Some(Toggled::True), "the pill");
        assert_eq!(toggled(&harness, PRODUCT), Some(Toggled::False));
        assert!(
            folder(&harness)
                .selections
                .get(&id)
                .is_none_or(Vec::is_empty),
            "no block is selected"
        );
        // Another pill moves the ring: `product` is read by *Product ID*.
        choose(&mut harness, PRODUCT);
        assert_eq!(ringed(&harness, &id), ["Product ID"]);
        assert_eq!(toggled(&harness, QUANTITY), Some(Toggled::False));
        let at = empty_board(&harness);
        click_at(&mut harness, at);
        assert!(
            ringed(&harness, &id).is_empty(),
            "the empty board clears it"
        );
        assert_eq!(toggled(&harness, PRODUCT), Some(Toggled::False));
    }

    /// Criterion 4: the strip adds nothing to the saved file; saving after
    /// clicks on the pills and the board leaves the `.afs` byte-identical.
    #[test]
    fn ux31_criterion_4_the_strip_stores_nothing() {
        let directory = shop();
        let file = directory.path().join(CART);
        let before = fs::read(&file).unwrap();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        choose(&mut harness, QUANTITY);
        choose(&mut harness, PRODUCT);
        let at = empty_board(&harness);
        click_at(&mut harness, at);
        choose(&mut harness, QUANTITY);
        {
            let document = folder(&harness)
                .session
                .as_ref()
                .unwrap()
                .document(&id)
                .unwrap();
            assert!(
                !document.is_dirty() && !document.is_layout_dirty(),
                "unchanged"
            );
        }
        app(&mut harness).save_document(&id).unwrap();
        settle(&mut harness);
        assert_eq!(fs::read(&file).unwrap(), before, "byte-identical");
    }

    /// The strip beside the approved captures
    /// `docs/qa/2026-11-02/COMP-00/comp00-q1-c-*.png`: Diagram and Split at
    /// both sizes, and Diagram with `quantity` selected.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_ux31_inputs_strip() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = shop();
            for mode in ["Diagram", "Split"] {
                let (mut harness, _) = window(directory.path(), size, CART, mode);
                let name = mode.to_lowercase();
                capture(&mut harness, &format!("ux31-strip-{name}-{suffix}"), None);
                if mode == "Diagram" {
                    choose(&mut harness, QUANTITY);
                    harness.hover_at(egui::pos2(480.0, 3.0));
                    settle(&mut harness);
                    capture(
                        &mut harness,
                        &format!("ux31-selected-{name}-{suffix}"),
                        None,
                    );
                }
            }
        }
    }

    /// At 960 × 540 the Inspector that opens for a selected pill leaves the
    /// strip too narrow for both pills: the selected one is scrolled into
    /// view, between the caption and *+ Add input*.
    #[test]
    fn ux31_a_selected_pill_stays_in_view_when_the_inspector_opens() {
        let directory = shop();
        let (mut harness, _) = window(directory.path(), MINIMUM, CART, "Diagram");
        choose(&mut harness, QUANTITY);
        assert!(
            has_label(&harness, "COMPONENT INPUT"),
            "the Inspector opened"
        );
        let caption = harness.get_by_label(CAPTION).rect();
        let add = harness.get_by_label(ADD).rect();
        let quantity = harness.get_by_label(QUANTITY).rect();
        assert!(
            caption.right() <= quantity.left() && quantity.right() <= add.left(),
            "quantity {quantity:?} in view between {caption:?} and {add:?}"
        );
        assert_eq!(toggled(&harness, QUANTITY), Some(Toggled::True));
    }

    /// Criterion 6: at 960 × 540, inputs that do not fit beside one another
    /// scroll in the strip, and *+ Add input* stays whole in view.
    #[test]
    fn ux31_criterion_6_add_input_stays_in_view_when_the_strip_overflows() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), MINIMUM, CART, "Diagram");
        let source = source_of(&harness, &id);
        let at = source
            .match_indices("\n  input ")
            .last()
            .map(|(at, _)| at + 1)
            .expect("an input line");
        let end = at + source[at..].find('\n').expect("a line end") + 1;
        let extra: String = [
            "customerReference",
            "deliveryInstructions",
            "preferredCurrency",
            "loyaltyProgrammeNumber",
            "promotionalCode",
            "warehouseLocation",
        ]
        .iter()
        .map(|name| format!("  input {name}: string = \"none\"\n"))
        .collect();
        let edited = format!("{}{extra}{}", &source[..end], &source[end..]);
        app(&mut harness)
            .session
            .as_mut()
            .unwrap()
            .edit_source(&id, &edited, CODE)
            .unwrap();
        app(&mut harness).analyser.typed(&id, Instant::now());
        settle(&mut harness);
        assert!(
            has_label(&harness, "warehouseLocation string = \"none\""),
            "the board follows the code"
        );
        let board = harness.get_by_label("Diagram board").rect();
        let add = harness.get_by_label(ADD).rect();
        assert!(
            board.contains_rect(add),
            "Add input whole on the board: {add:?}"
        );
        assert_within_window(&harness, &[ADD, PRODUCT]);
        assert!(
            harness.get_by_label(PRODUCT).rect().right() <= add.left(),
            "the first pill before Add input"
        );
        // The row is wider than the strip leaves it: the last pill lies past
        // Add input, scrolled out of view, rather than pushing it away.
        let last = harness
            .get_by_label("warehouseLocation string = \"none\"")
            .rect();
        assert!(
            last.right() > add.left(),
            "the last pill {last:?} is scrolled beyond Add input {add:?}"
        );
    }
}

/// UX-32 (#249, COMP-00 Q2 = A, Q3): a component's inputs are added and
/// edited in the Inspector. *+ Add input* on the INPUTS strip opens a *New
/// input* form; a selected pill opens the input's panel, whose edits are
/// live: each change is written to the code as it is typed, and an invalid
/// name stays in the field with an inline error while the code keeps the
/// last valid name (docs/tickets/COMP-00.md).
mod ux32 {
    use super::mcp08b::contains;
    use super::ux31::{ringed, toggled, window};
    use super::*;
    use crate::testing::{MINIMUM, assert_within_window};
    use egui::accesskit::Toggled;
    use egui::epaint::Shape;
    use egui_kittest::kittest::NodeT;

    const CART: &str = "components/add-to-cart.afs";
    const QUANTITY: &str = "quantity * number = 1";
    const PRODUCT: &str = "product * object · Product";
    const ADD: &str = "Add input";
    const CONFIRM_ADD: &str = "Add the new input";
    const NAME: &str = "Input name";
    const LABEL: &str = "Label on the block";
    const DEFAULT: &str = "Default value";
    const TYPE: &str = "Value type";
    const REQUIRED: &str = "Required";
    const EDITABLE: &str = "Editable on the block";
    const ACTION: &str = "Fed by the previous block (action)";
    const QUANTITY_LINE: &str = "  input quantity: number = 1\n";

    fn field_value(harness: &FolderHarness, label: &str) -> String {
        harness
            .get_by_label(label)
            .accesskit_node()
            .value()
            .unwrap_or_default()
    }

    /// Selects everything in the focused field `label` and types `text` over
    /// it, one keystroke at a time: each character is its own frame.
    fn type_over(harness: &mut FolderHarness, label: &str, text: &str) {
        harness.get_by_label(label).focus();
        harness.step();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.key_press(egui::Key::Backspace);
        settle(harness);
        for character in text.chars() {
            harness
                .get_by_label(label)
                .type_text(&character.to_string());
            settle(harness);
        }
    }

    /// Whether text containing `text` is painted on the board.
    fn painted_on_board(harness: &FolderHarness, text: &str) -> bool {
        fn any(shape: &Shape, board: egui::Rect, text: &str) -> bool {
            match shape {
                Shape::Vec(shapes) => shapes.iter().any(|shape| any(shape, board, text)),
                Shape::Text(shape) => {
                    shape.galley.job.text.contains(text)
                        && board.contains_rect(shape.visual_bounding_rect())
                }
                _ => false,
            }
        }
        let board = harness.get_by_label("Diagram board").rect();
        harness
            .output()
            .shapes
            .iter()
            .any(|clipped| any(&clipped.shape, board, text))
    }

    fn open_form(harness: &mut FolderHarness) {
        assert!(
            !harness.get_by_label(ADD).accesskit_node().is_disabled(),
            "+ Add input is enabled"
        );
        choose(harness, ADD);
    }

    /// Criterion 1: *+ Add input* opens *New input* with the name field
    /// focused, and the strip shows the input as being added; at both sizes.
    #[test]
    fn ux32_criterion_1_add_input_opens_the_new_input_form() {
        for size in [MINIMUM, WIDE] {
            let directory = shop();
            let (mut harness, _) = window(directory.path(), size, CART, "Diagram");
            open_form(&mut harness);
            assert!(has_label(&harness, "New input"), "the form at {size:?}");
            assert!(
                harness.get_by_label(NAME).is_focused(),
                "the name is focused at {size:?}"
            );
            assert!(
                contains(&harness, "being added…"),
                "the strip's pill at {size:?}"
            );
            harness.get_by_label(NAME).type_text("currency");
            settle(&mut harness);
            assert!(
                has_label(&harness, "currency being added…"),
                "the pill names the input at {size:?}"
            );
            assert_within_window(
                &harness,
                &[
                    NAME,
                    TYPE,
                    DEFAULT,
                    REQUIRED,
                    EDITABLE,
                    CONFIRM_ADD,
                    "Cancel",
                ],
            );
        }
    }

    /// Criterion 2: *Add input* with `currency`, Text, `"EUR"` and not
    /// required writes `input currency: string = "EUR"` after the last
    /// input, and its pill appears; *Cancel* writes nothing. Without a
    /// default the input is required (KTLO-32): the box is forced on and
    /// says why.
    #[test]
    fn ux32_criterion_2_add_input_writes_the_declaration() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        let before = source_of(&harness, &id);
        assert!(before.contains(QUANTITY_LINE), "{before}");

        // Cancel writes nothing.
        open_form(&mut harness);
        harness.get_by_label(NAME).type_text("currency");
        settle(&mut harness);
        choose(&mut harness, "Cancel");
        assert!(!has_label(&harness, "New input"), "the form is closed");
        assert!(!contains(&harness, "being added…"), "no pill being added");
        assert_eq!(source_of(&harness, &id), before, "Cancel writes nothing");

        open_form(&mut harness);
        harness.get_by_label(NAME).type_text("currency");
        settle(&mut harness);
        // No default yet: required, forced on and explained.
        let required = harness.get_by_label(REQUIRED);
        assert_eq!(required.accesskit_node().toggled(), Some(Toggled::True));
        assert!(required.accesskit_node().is_disabled(), "forced on");
        assert!(contains(&harness, "An input without a default is required"));
        choose(&mut harness, TYPE);
        choose(&mut harness, "Text");
        harness.get_by_label(DEFAULT).focus();
        harness.step();
        harness.get_by_label(DEFAULT).type_text("\"EUR\"");
        settle(&mut harness);
        // With a default it may be optional; it is not required here.
        assert!(
            !harness
                .get_by_label(REQUIRED)
                .accesskit_node()
                .is_disabled()
        );
        assert_eq!(toggled(&harness, REQUIRED), Some(Toggled::False));
        assert_eq!(toggled(&harness, EDITABLE), Some(Toggled::True));
        choose(&mut harness, CONFIRM_ADD);

        let expected = before.replacen(
            QUANTITY_LINE,
            &format!("{QUANTITY_LINE}  input currency: string = \"EUR\"\n"),
            1,
        );
        assert_eq!(source_of(&harness, &id), expected);
        assert!(
            has_label(&harness, "currency string = \"EUR\""),
            "the new input's pill"
        );
        assert!(!contains(&harness, "being added…"));
        assert!(!has_label(&harness, "New input"), "the form is closed");
    }

    /// Criterion 3: renaming `quantity` to `qty` renames the declaration and
    /// every `param.quantity` as typed; the pill and the *Cart item* card
    /// follow each keystroke.
    #[test]
    fn ux32_criterion_3_a_rename_is_written_as_it_is_typed() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        let before = source_of(&harness, &id);
        assert!(before.contains("param.quantity"), "{before}");
        choose(&mut harness, QUANTITY);
        assert_eq!(field_value(&harness, NAME), "quantity");
        harness.get_by_label(NAME).focus();
        harness.step();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        for typed in ["q", "qt", "qty"] {
            let character = &typed[typed.len() - 1..];
            harness.get_by_label(NAME).type_text(character);
            settle(&mut harness);
            let source = source_of(&harness, &id);
            assert!(
                source.contains(&format!("  input {typed}: number = 1\n")),
                "the declaration reads {typed}: {source}"
            );
            assert!(
                source.contains(&format!("param.{typed}")) && !source.contains("param.quantity"),
                "every reading follows {typed}: {source}"
            );
            assert!(
                has_label(&harness, &format!("{typed} * number = 1")),
                "the pill reads {typed}"
            );
            assert!(
                painted_on_board(&harness, &format!("→ {typed}"))
                    && !painted_on_board(&harness, "→ quantity"),
                "the Cart item card reads {typed}"
            );
            assert_eq!(ringed(&harness, &id), ["Cart item"], "still selected");
            assert!(harness.get_by_label(NAME).is_focused(), "still typing");
        }
        assert_eq!(
            source_of(&harness, &id),
            before
                .replace("input quantity:", "input qty:")
                .replace("param.quantity", "param.qty")
        );
    }

    /// Criterion 4: an invalid or taken name stays in the field with an
    /// inline error and the code keeps the last valid name; leaving the
    /// field puts the valid name back.
    #[test]
    fn ux32_criterion_4_an_invalid_name_stays_in_the_field() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        let before = source_of(&harness, &id);
        choose(&mut harness, QUANTITY);
        type_over(&mut harness, NAME, "1 bad");
        assert_eq!(field_value(&harness, NAME), "1 bad", "the text stays");
        assert!(contains(&harness, "Not a valid name"), "the inline error");
        assert_eq!(source_of(&harness, &id), before, "the code keeps quantity");
        assert!(has_label(&harness, QUANTITY), "the board keeps quantity");

        // Typed at once: each valid prefix of it would be written as typed.
        replace_text(&mut harness, NAME, "product");
        assert_eq!(field_value(&harness, NAME), "product");
        assert!(
            contains(&harness, "Another input is already named product"),
            "the duplicate's error"
        );
        assert_eq!(source_of(&harness, &id), before, "the code keeps quantity");

        // Leaving the field restores the valid name.
        harness.get_by_label(LABEL).focus();
        settle(&mut harness);
        assert_eq!(field_value(&harness, NAME), "quantity");
        assert!(!contains(&harness, "Not a valid name"));
        assert!(!contains(&harness, "Another input is already named"));
        assert_eq!(source_of(&harness, &id), before);
    }

    /// Criterion 5: the type, the default, *Required*, *Editable on the
    /// block* and *Fed by the previous block (action)* each write their
    /// attribute, and the panel reads it back. The action belongs to one
    /// input at a time: the component's single `action` line.
    #[test]
    fn ux32_criterion_5_each_attribute_is_written_and_read_back() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        choose(&mut harness, QUANTITY);

        type_over(&mut harness, DEFAULT, "2");
        assert!(source_of(&harness, &id).contains("  input quantity: number = 2\n"));
        assert!(has_label(&harness, "quantity * number = 2"), "the pill");

        choose(&mut harness, TYPE);
        choose(&mut harness, "Text");
        assert!(source_of(&harness, &id).contains("  input quantity: string = 2\n"));
        assert!(
            harness
                .query_by_label_contains("quantity * string")
                .is_some()
        );

        // `quantity` is declared `@required(true)` with a default: without
        // the mark it is optional, which needs no attribute.
        assert_eq!(toggled(&harness, REQUIRED), Some(Toggled::True));
        choose(&mut harness, REQUIRED);
        assert_eq!(toggled(&harness, REQUIRED), Some(Toggled::False));
        assert!(
            source_of(&harness, &id).contains(
                "  @label(\"Quantity\")\n  @exposed(true)\n  input quantity: string = 2\n"
            )
        );
        assert!(has_label(&harness, "quantity string = 2"), "the pill");
        choose(&mut harness, REQUIRED);
        assert_eq!(toggled(&harness, REQUIRED), Some(Toggled::True));
        assert!(source_of(&harness, &id).contains(
            "  @label(\"Quantity\")\n  @required(true)\n  @exposed(true)\n  input quantity:"
        ));

        choose(&mut harness, EDITABLE);
        assert_eq!(toggled(&harness, EDITABLE), Some(Toggled::False));
        assert!(
            source_of(&harness, &id).contains("  @required(true)\n  @hidden\n  input quantity:")
        );
        choose(&mut harness, EDITABLE);
        assert_eq!(toggled(&harness, EDITABLE), Some(Toggled::True));
        assert!(source_of(&harness, &id).contains("  @required(true)\n  input quantity:"));

        replace_text(&mut harness, LABEL, "Amount");
        assert!(source_of(&harness, &id).contains("  @label(\"Amount\")\n  @required(true)\n"));
        replace_text(&mut harness, LABEL, "");
        assert!(source_of(&harness, &id).contains("  @required(true)\n  input quantity:"));
        assert!(!source_of(&harness, &id).contains("@label(\"Amount\")"));

        // `product` is the action; it moves to `quantity`, then goes.
        assert!(source_of(&harness, &id).contains("  action product"));
        assert_eq!(toggled(&harness, ACTION), Some(Toggled::False));
        choose(&mut harness, ACTION);
        assert_eq!(toggled(&harness, ACTION), Some(Toggled::True));
        let source = source_of(&harness, &id);
        assert!(source.contains("  action quantity: string\n"), "{source}");
        assert!(
            source.contains("  input quantity: string = 2\n"),
            "{source}"
        );
        assert!(!source.contains("action product"), "{source}");
        choose(&mut harness, PRODUCT);
        assert_eq!(toggled(&harness, ACTION), Some(Toggled::False), "product");
        choose(&mut harness, "quantity * string = 2");
        assert_eq!(toggled(&harness, ACTION), Some(Toggled::True));
        choose(&mut harness, ACTION);
        assert!(!source_of(&harness, &id).contains("  action "));
    }

    /// Criterion 6: *Used by* lists the steps that read the input; clicking
    /// one selects its card.
    #[test]
    fn ux32_criterion_6_used_by_selects_the_reading_card() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        choose(&mut harness, QUANTITY);
        assert!(has_label(&harness, "Used by"));
        choose(&mut harness, "Select the step Cart item");
        let selected = folder(&harness).selections.get(&id).cloned();
        let body = {
            let app = folder(&harness);
            let Some(Document::Component(component)) =
                app.session.as_ref().unwrap().document(&id).unwrap().model()
            else {
                panic!("a component");
            };
            component
                .graph
                .nodes
                .iter()
                .find(|node| node.label.as_deref() == Some("Cart item"))
                .unwrap()
                .id
                .clone()
        };
        assert_eq!(selected, Some(vec![body]), "the card is selected");
        assert!(!has_label(&harness, NAME), "the block's Inspector");
    }

    /// Criterion 7: deleting a read input asks first and names its readers;
    /// an unread one goes at once; undo brings it back.
    #[test]
    fn ux32_criterion_7_delete_input_confirms_when_it_is_read() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        let before = source_of(&harness, &id);
        choose(&mut harness, QUANTITY);
        choose(&mut harness, "Delete input");
        assert_eq!(source_of(&harness, &id), before, "nothing yet");
        assert!(
            contains(&harness, "Cart item reads quantity"),
            "the confirmation names the reader"
        );
        choose(&mut harness, "Delete anyway");
        let source = source_of(&harness, &id);
        assert!(!source.contains("input quantity"), "{source}");
        assert!(!source.contains("param.quantity"), "{source}");
        assert!(!has_label(&harness, QUANTITY), "the pill is gone");
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        settle(&mut harness);
        assert_eq!(source_of(&harness, &id), before, "undo restores it");
        assert!(has_label(&harness, QUANTITY));

        // An input nothing reads is deleted at once.
        open_form(&mut harness);
        harness.get_by_label(NAME).type_text("note");
        settle(&mut harness);
        choose(&mut harness, CONFIRM_ADD);
        assert!(source_of(&harness, &id).contains("  input note: string\n"));
        // The added input is selected.
        assert!(contains(&harness, "No step reads this input yet"));
        choose(&mut harness, "Delete input");
        assert_eq!(source_of(&harness, &id), before, "deleted at once");
        assert!(!contains(&harness, "Delete anyway"));
    }

    /// At 960 × 540 the Inspector leaves the strip too narrow for every
    /// pill: the input being added stays whole in view as its name grows,
    /// between the caption and *+ Add input*.
    #[test]
    fn ux32_the_input_being_added_stays_in_view_as_it_is_named() {
        let directory = shop();
        let (mut harness, _) = window(directory.path(), MINIMUM, CART, "Diagram");
        open_form(&mut harness);
        for typed in ["c", "urrency", "Reference"] {
            harness.get_by_label(NAME).type_text(typed);
            settle(&mut harness);
        }
        let caption = harness.get_by_label("INPUTS").rect();
        let add = harness.get_by_label(ADD).rect();
        let pill = harness
            .get_by_label("currencyReference being added…")
            .rect();
        assert!(
            caption.right() <= pill.left() && pill.right() <= add.left(),
            "the pill {pill:?} whole between {caption:?} and {add:?}"
        );
    }

    /// A type the default does not fit is refused until the default
    /// changes: `quantity = 1` stays a number when *List* is chosen, and a
    /// default of another kind is an inline error that the code never gets.
    #[test]
    fn ux32_a_default_and_its_type_must_agree() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        let before = source_of(&harness, &id);
        choose(&mut harness, QUANTITY);
        choose(&mut harness, TYPE);
        choose(&mut harness, "List");
        assert_eq!(source_of(&harness, &id), before, "the type is kept");
        assert!(contains(&harness, "The default 1 is not a list"));
        type_over(&mut harness, DEFAULT, "true");
        assert!(contains(&harness, "The default is not a number"));
        assert!(source_of(&harness, &id).contains("  input quantity: number\n"));
        replace_text(&mut harness, DEFAULT, "[1, 2]");
        assert!(contains(&harness, "The default is not a number"));
        assert!(!source_of(&harness, &id).contains("[1, 2]"));
    }

    /// On a read-only board the panel's fields are disabled, but *Used by*
    /// still selects the reading card.
    #[test]
    fn ux32_used_by_is_navigation_on_a_read_only_board() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        choose(&mut harness, QUANTITY);
        // Code with an error leaves the last valid version, read-only.
        let broken = format!("{}\n  broken = (\n", source_of(&harness, &id));
        app(&mut harness)
            .session
            .as_mut()
            .unwrap()
            .edit_source(&id, &broken, CODE)
            .unwrap();
        app(&mut harness).analyser.typed(&id, Instant::now());
        settle(&mut harness);
        assert!(contains(&harness, "read-only"), "the board says so");
        assert!(harness.get_by_label(NAME).accesskit_node().is_disabled());
        assert!(
            !harness
                .get_by_label("Select the step Cart item")
                .accesskit_node()
                .is_disabled()
        );
        choose(&mut harness, "Select the step Cart item");
        assert_eq!(
            folder(&harness).selections.get(&id).map(Vec::len),
            Some(1),
            "the card is selected"
        );
    }

    /// The panels beside the approved captures
    /// `docs/qa/2026-11-02/COMP-00/comp00-q2-*.png` and `comp00-q3-*.png`.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_ux32_inputs_in_the_inspector() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = shop();
            let (mut harness, _) = window(directory.path(), size, CART, "Diagram");
            open_form(&mut harness);
            harness.get_by_label(NAME).type_text("currency");
            settle(&mut harness);
            capture(&mut harness, &format!("ux32-form-{suffix}"), None);
            harness.get_by_label(DEFAULT).focus();
            harness.step();
            harness.get_by_label(DEFAULT).type_text("\"EUR\"");
            settle(&mut harness);
            choose(&mut harness, CONFIRM_ADD);
            capture(&mut harness, &format!("ux32-added-{suffix}"), None);
            choose(&mut harness, QUANTITY);
            capture(&mut harness, &format!("ux32-edit-{suffix}"), None);
            replace_text(&mut harness, NAME, "product");
            capture(&mut harness, &format!("ux32-duplicate-{suffix}"), None);
        }
    }
}

/// UX-33 (#250, COMP-00 Q4): a *Component settings* button in the
/// component board's header row opens the component's settings in the
/// Inspector: its name, the name in code, the description, the action and
/// the lists of its inputs and outputs. A scenario's board has no such
/// button (docs/tickets/COMP-00.md).
mod ux33 {
    use super::super::component_run_tests::{NOTES, shop_with_notes};
    use super::mcp08b::contains;
    use super::ux02::hover_shows;
    use super::ux31::{toggled, window};
    use super::*;
    use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
    use egui::accesskit::Toggled;
    use egui_kittest::kittest::NodeT;

    const CART: &str = "components/add-to-cart.afs";
    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
    const SETTINGS: &str = "Component settings";
    const NAME: &str = "Component name";
    const HANDLE: &str = "Name in code";
    const DESCRIPTION: &str = "Component description";
    const ACTION: &str = "Offered after a block that outputs (action)";
    const ADD: &str = "Add input to the component";
    const PRODUCT_ROW: &str = "Input product * object · Product";
    const QUANTITY_ROW: &str = "Input quantity * number = 1";
    const CART_DESCRIPTION: &str = "Create a cart with the product and fetch its calculated price.";
    const FIXED: &str = "In code: add_to_cart · fixed once published";

    fn field_value(harness: &FolderHarness, label: &str) -> String {
        harness
            .get_by_label(label)
            .accesskit_node()
            .value()
            .unwrap_or_default()
    }

    fn open_settings(harness: &mut FolderHarness) {
        choose(harness, SETTINGS);
        assert_eq!(
            toggled(harness, SETTINGS),
            Some(Toggled::True),
            "the button is pressed while the settings are open"
        );
    }

    /// Selects everything in the field `label` and types `text` over it, one
    /// keystroke per frame; `written` checks the code after each.
    fn type_each(
        harness: &mut FolderHarness,
        label: &str,
        text: &str,
        mut written: impl FnMut(&FolderHarness, &str),
    ) {
        harness.get_by_label(label).focus();
        harness.step();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        let mut typed = String::new();
        for character in text.chars() {
            typed.push(character);
            harness
                .get_by_label(label)
                .type_text(&character.to_string());
            settle(harness);
            written(harness, &typed);
        }
    }

    /// Criterion 1: a component's board shows *Component settings* in its
    /// header row beside *Add step*, in Diagram and Split at both sizes; it
    /// keeps its label while the row has room and is an icon with a tooltip
    /// when it has not. A scenario's board has no such button.
    #[test]
    fn ux33_criterion_1_a_component_board_has_the_button_a_scenario_none() {
        for size in [MINIMUM, WIDE] {
            for mode in ["Diagram", "Split"] {
                let directory = shop();
                let (harness, _) = window(directory.path(), size, CART, mode);
                assert!(has_label(&harness, SETTINGS), "{mode} at {size:?}");
                let button = harness.get_by_label(SETTINGS).rect();
                let add_step = harness.get_by_label("Add step").rect();
                assert!(
                    (button.center().y - add_step.center().y).abs() < 2.0,
                    "{mode} at {size:?}: on the row of Add step, {button:?} {add_step:?}"
                );
                let strip = harness.get_by_label("INPUTS").rect();
                assert!(
                    button.bottom() <= strip.top(),
                    "{mode} at {size:?}: in the header row, above the strip"
                );
                assert_within_window(&harness, &[SETTINGS, "Add step"]);
                assert_disjoint(&harness, &[SETTINGS, "Add step", "Tidy up"]);

                let directory = shop();
                let (harness, _) = window(directory.path(), size, SCENARIO, mode);
                assert!(has_label(&harness, "Add step"), "the scenario's board");
                assert!(
                    !has_label(&harness, SETTINGS),
                    "{mode} at {size:?}: a scenario has no Component settings"
                );
            }
        }
        // With room, the button reads its label.
        let directory = shop();
        let (harness, _) = window(directory.path(), WIDE, CART, "Diagram");
        let button = harness.get_by_label(SETTINGS).rect();
        assert!(button.width() > 100.0, "labelled: {button:?}");
        // At 960 × 540 with the Inspector open the row has no room: an icon,
        // which names itself in a tooltip.
        let directory = shop();
        let (mut harness, _) = window(directory.path(), MINIMUM, CART, "Diagram");
        open_settings(&mut harness);
        let button = harness.get_by_label(SETTINGS).rect();
        assert!(button.width() < 40.0, "an icon: {button:?}");
        assert!(hover_shows(
            &mut harness,
            SETTINGS,
            "name, description, action, inputs and outputs"
        ));
    }

    /// Criterion 2: the button opens the panel with the component's name,
    /// name in code, description, action and the lists of its inputs and
    /// outputs, as `add-to-cart.afs` declares them; a second click closes it.
    #[test]
    fn ux33_criterion_2_the_button_opens_the_settings() {
        for size in [MINIMUM, WIDE] {
            let directory = shop();
            let (mut harness, _) = window(directory.path(), size, CART, "Diagram");
            assert!(!has_label(&harness, NAME), "closed at first");
            assert_eq!(toggled(&harness, SETTINGS), Some(Toggled::False));
            open_settings(&mut harness);
            assert!(has_label(&harness, "COMPONENT"), "the caption at {size:?}");
            assert_eq!(field_value(&harness, NAME), "Add to cart");
            assert!(contains(&harness, FIXED), "the name in code at {size:?}");
            assert_eq!(field_value(&harness, DESCRIPTION), CART_DESCRIPTION);
            assert_eq!(
                field_value(&harness, ACTION),
                "product · object · Product",
                "the action at {size:?}"
            );
            assert!(has_label(&harness, PRODUCT_ROW), "{size:?}");
            assert!(has_label(&harness, QUANTITY_ROW), "{size:?}");
            assert!(has_label(&harness, ADD), "{size:?}");
            assert!(has_label(&harness, "Output cart object · Cart"), "{size:?}");
            assert!(contains(
                &harness,
                "Outputs are the Export blocks on the board."
            ));
            assert_within_window(&harness, &[NAME, DESCRIPTION, ACTION, SETTINGS]);

            choose(&mut harness, SETTINGS);
            assert!(!has_label(&harness, NAME), "a second click closes them");
            assert_eq!(toggled(&harness, SETTINGS), Some(Toggled::False));
        }
    }

    /// Criterion 3: the description is written on each keystroke, and the
    /// name as it is changed; an empty name stays in the field with an
    /// inline error while the code keeps the last one. Published, the name in code is read-only
    /// with the *fixed once published* note.
    #[test]
    fn ux33_criterion_3_the_description_and_name_are_written_as_typed() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        let before = source_of(&harness, &id);
        assert!(before.contains(&format!("  description \"{CART_DESCRIPTION}\"\n")));
        open_settings(&mut harness);

        type_each(
            &mut harness,
            DESCRIPTION,
            "Fills a cart",
            |harness, typed| {
                let source = source_of(harness, &id);
                assert!(
                    source.contains(&format!("  description \"{typed}\"\n")),
                    "the description reads {typed}: {source}"
                );
                assert!(harness.get_by_label(DESCRIPTION).is_focused(), "typing");
            },
        );

        replace_text(&mut harness, NAME, "Add item");
        assert!(
            source_of(&harness, &id).contains("component add_to_cart \"Add item\" {\n"),
            "{}",
            source_of(&harness, &id)
        );
        assert!(has_label(&harness, "Add item"), "the panel's title follows");
        replace_text(&mut harness, NAME, "");
        assert_eq!(field_value(&harness, NAME), "", "the text stays");
        assert!(contains(&harness, "A component needs a name"));
        assert!(source_of(&harness, &id).contains("component add_to_cart \"Add item\" {\n"));

        // Published: the name in code is not a field.
        assert!(!has_label(&harness, HANDLE), "read-only once published");
        assert!(contains(&harness, FIXED));
    }

    /// Criterion 3, before the first publication: the name in code is a
    /// field, written as typed while it is a valid name no other component
    /// has; publishing makes it read-only with the note.
    #[test]
    fn ux33_criterion_3_the_name_in_code_is_editable_until_published() {
        let project = shop_with_notes();
        let (mut harness, id) = window(project.path(), WIDE, NOTES, "Diagram");
        open_settings(&mut harness);
        assert_eq!(field_value(&harness, NAME), "Notes");
        assert_eq!(field_value(&harness, HANDLE), "Notes");
        assert!(!contains(&harness, "· fixed once published"));

        type_each(&mut harness, HANDLE, "memo", |harness, typed| {
            let source = source_of(harness, &id);
            assert!(
                source.starts_with(&format!("component {typed} \"Notes\" {{\n")),
                "the name in code reads {typed}: {source}"
            );
        });

        replace_text(&mut harness, HANDLE, "add_to_cart");
        assert_eq!(field_value(&harness, HANDLE), "add_to_cart");
        assert!(contains(
            &harness,
            "Another component is already named add_to_cart"
        ));
        assert!(source_of(&harness, &id).starts_with("component memo \"Notes\" {\n"));
        replace_text(&mut harness, HANDLE, "1 bad");
        assert!(contains(&harness, "Not a valid name"));
        assert!(source_of(&harness, &id).starts_with("component memo \"Notes\" {\n"));
        // Leaving the field puts the code's name back.
        harness.get_by_label(DESCRIPTION).focus();
        settle(&mut harness);
        assert_eq!(field_value(&harness, HANDLE), "memo");

        choose(&mut harness, "Publish");
        assert!(
            folder(&harness)
                .session
                .as_ref()
                .unwrap()
                .manifest()
                .revisions
                .iter()
                .any(|revision| revision.handle == "memo"),
            "published"
        );
        assert!(!has_label(&harness, HANDLE), "read-only once published");
        assert!(contains(&harness, "In code: memo · fixed once published"));
        assert_eq!(
            field_value(&harness, NAME),
            "Notes",
            "the name stays editable"
        );
    }

    /// Criterion 4: choosing the input that feeds the action writes the
    /// component's `action` line with that input's type; *No action*
    /// removes it.
    #[test]
    fn ux33_criterion_4_the_action_is_chosen_or_removed() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, CART, "Diagram");
        assert!(source_of(&harness, &id).contains("  action product"));
        open_settings(&mut harness);

        choose(&mut harness, ACTION);
        choose(&mut harness, "quantity · number");
        let source = source_of(&harness, &id);
        assert!(source.contains("  action quantity: number\n"), "{source}");
        assert!(!source.contains("action product"), "{source}");
        assert_eq!(field_value(&harness, ACTION), "quantity · number");

        choose(&mut harness, ACTION);
        choose(&mut harness, "No action");
        let source = source_of(&harness, &id);
        assert!(!source.contains("  action "), "{source}");
        assert_eq!(field_value(&harness, ACTION), "No action");

        choose(&mut harness, ACTION);
        choose(&mut harness, "product · object · Product");
        assert!(source_of(&harness, &id).contains("  action product"));
    }

    /// Criterion 5: *+ Add input* opens UX-32's *New input* form, and a
    /// row of the inputs list opens that input's panel; either takes the
    /// Inspector from the settings, as a pill of the strip does.
    #[test]
    fn ux33_criterion_5_the_lists_open_the_input_panels() {
        let directory = shop();
        let (mut harness, _) = window(directory.path(), WIDE, CART, "Diagram");
        open_settings(&mut harness);
        choose(&mut harness, ADD);
        assert!(has_label(&harness, "New input"), "UX-32's form");
        assert!(harness.get_by_label("Input name").is_focused());
        assert!(contains(&harness, "being added…"), "the strip's pill");
        assert_eq!(toggled(&harness, SETTINGS), Some(Toggled::False));
        assert!(!has_label(&harness, NAME), "the settings gave way");

        open_settings(&mut harness);
        assert!(!has_label(&harness, "New input"), "the form gave way");
        choose(&mut harness, QUANTITY_ROW);
        assert_eq!(field_value(&harness, "Input name"), "quantity");
        assert!(has_label(&harness, "Used by"), "the input's panel");
        assert_eq!(toggled(&harness, SETTINGS), Some(Toggled::False));
        assert_eq!(
            toggled(&harness, "quantity * number = 1"),
            Some(Toggled::True),
            "its pill is selected"
        );

        // A pill of the strip, too, takes the Inspector from the settings.
        open_settings(&mut harness);
        choose(&mut harness, "product * object · Product");
        assert!(!has_label(&harness, NAME));
        assert_eq!(field_value(&harness, "Input name"), "product");
    }

    /// Review round 1: the button is disabled, and says why, only where the
    /// window has no room for the Inspector, which is Split at 960 × 540.
    #[test]
    fn ux33_the_button_waits_for_room_for_the_inspector() {
        for size in [MINIMUM, WIDE] {
            for mode in ["Diagram", "Split"] {
                let directory = shop();
                let (mut harness, _) = window(directory.path(), size, CART, mode);
                let narrow = size == MINIMUM && mode == "Split";
                assert_eq!(
                    harness
                        .get_by_label(SETTINGS)
                        .accesskit_node()
                        .is_disabled(),
                    narrow,
                    "{mode} at {size:?}"
                );
                if narrow {
                    assert!(hover_shows(
                        &mut harness,
                        SETTINGS,
                        "too narrow for the Inspector beside the code"
                    ));
                }
            }
        }
    }

    /// Found in the native QA run: the icon showed its own label as a
    /// tooltip beside the longer one. It shows one tooltip.
    #[test]
    fn ux33_the_icon_shows_one_tooltip() {
        let directory = shop();
        let (mut harness, _) = window(directory.path(), MINIMUM, CART, "Diagram");
        open_settings(&mut harness);
        assert!(hover_shows(
            &mut harness,
            SETTINGS,
            "Component settings: name, description, action, inputs and outputs"
        ));
        assert_eq!(
            harness.query_all_by_label(SETTINGS).count(),
            1,
            "only the button is named Component settings"
        );
    }

    /// The button and the panel beside the approved capture
    /// `docs/qa/2026-11-02/COMP-00/comp00-q4-settings-*.png`.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_ux33_component_settings() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            for mode in ["Diagram", "Split"] {
                let directory = shop();
                let (mut harness, _) = window(directory.path(), size, CART, mode);
                let mode = mode.to_lowercase();
                capture(&mut harness, &format!("ux33-button-{mode}-{suffix}"), None);
                if harness
                    .get_by_label(SETTINGS)
                    .accesskit_node()
                    .is_disabled()
                {
                    continue;
                }
                open_settings(&mut harness);
                capture(
                    &mut harness,
                    &format!("ux33-settings-{mode}-{suffix}"),
                    None,
                );
            }
        }
    }
}

/// UX-30 (#238, COMP-00 Q5): *Selection actions → Save as component* in a
/// folder project opens a dialog that proposes the component's inputs from
/// the wires and values entering the selection and its outputs from what is
/// read after it. By default it publishes revision 1 and calls it in place
/// of the selected steps; the other choice only creates the file
/// (docs/tickets/COMP-00.md, decision 5).
mod ux30 {
    use super::mcp08b::contains;
    use super::ux31::{toggled, window};
    use super::*;
    use crate::testing::MINIMUM;
    use apiforge_core::{model::NodeKind, run::StepResult};
    use egui::accesskit::Toggled;
    use egui_kittest::kittest::NodeT;

    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
    const FILE: &str = "components/check-cart-price.afs";
    const TITLE: &str = "Save as component";
    const NAME: &str = "Component name";
    const FILE_NAME: &str = "Component file";
    const DESCRIPTION: &str = "Component description";
    const PUBLISH: &str = "Publish revision 1 and call it here, in place of the selected steps";
    const ONLY_CREATE: &str = "Only create the file; the steps stay in the scenario";
    const SAVE_AND_PUBLISH: &str = "Save and publish";
    const CREATE_FILE: &str = "Create file";
    const CART_DESCRIPTION: &str = "Add the product to a cart and check the cart's total.";

    /// The component the dialog writes for the mock-up's choices.
    const COMPONENT: &str = r#"component check_cart_price "Check cart price" {
  description "Add the product to a cart and check the cart's total."
  @contract("Product")
  input product: object
  input quantity: number = 1
  input expected: number
  action product: object contract "Product"
  @label("Add to cart")
  add_to_cart = add_to_cart@1(product: param.product, quantity: param.quantity)
  @label("Promotional price")
  promotional_price = assert add_to_cart.total ~= param.expected within 0.005
  @label("Cart")
  @config({ contractId: "Cart", type: "object" })
  export cart: object = add_to_cart
}
"#;

    fn field_value(harness: &FolderHarness, label: &str) -> String {
        harness
            .get_by_label(label)
            .accesskit_node()
            .value()
            .unwrap_or_default()
    }

    fn disabled(harness: &FolderHarness, label: &str) -> bool {
        harness.get_by_label(label).accesskit_node().is_disabled()
    }

    /// The demo scenario at `size` in Diagram with *Add to cart* and
    /// *Promotional price* selected and the dialog opened from *Selection
    /// actions*.
    fn open_dialog(root: &std::path::Path, size: egui::Vec2) -> (FolderHarness, String) {
        let (mut harness, id) = window(root, size, SCENARIO, "Diagram");
        select_and_open(&mut harness, &id);
        (harness, id)
    }

    fn select_and_open(harness: &mut FolderHarness, id: &str) {
        app(harness)
            .selections
            .insert(id.to_owned(), vec!["cart".to_owned(), "price".to_owned()]);
        settle(harness);
        choose(harness, "Selection actions");
        choose(harness, TITLE);
        assert!(has_label(harness, NAME), "the dialog is open");
    }

    /// Names the component as the mock-up does.
    fn fill(harness: &mut FolderHarness) {
        replace_text(harness, NAME, "Check cart price");
        replace_text(harness, DESCRIPTION, CART_DESCRIPTION);
    }

    fn revisions(harness: &FolderHarness, handle: &str) -> Vec<u32> {
        folder(harness)
            .session
            .as_ref()
            .unwrap()
            .manifest()
            .revisions
            .iter()
            .filter(|revision| revision.handle == handle)
            .map(|revision| revision.revision)
            .collect()
    }

    fn components(root: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(root.join("components"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// The scenario's text with the two steps replaced by the call, which
    /// the cleanup's request now reads.
    fn called(before: &str) -> String {
        let steps = "  @label(\"Add to cart\")\n  \
                     add_to_cart = add_to_cart@1(product: active_promotion, quantity: 1)\n  \
                     @label(\"Promotional price\")\n  \
                     promotional_price = assert add_to_cart.total ~= expected_price within 0.005\n";
        let call = "  @label(\"Check cart price\")\n  \
                    check_cart_price = check_cart_price@1(product: active_promotion, expected: expected_price)\n";
        assert!(before.contains(steps), "{before}");
        before
            .replacen(steps, call, 1)
            .replacen("{{add_to_cart.id}}", "{{check_cart_price.id}}", 1)
    }

    /// Runs the active tab's simulation and returns its report.
    fn simulate(harness: &mut FolderHarness) -> RunResult {
        choose(harness, "Run simulation");
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
            .expect("the run leaves a report")
            .clone()
    }

    /// What a run found: each request sent and each check with its values,
    /// in the order they ran, however deeply they are nested in calls.
    fn findings(report: &RunResult) -> Vec<String> {
        fn walk(steps: &[StepResult], out: &mut Vec<String>) {
            for step in steps {
                if let Some(request) = &step.request {
                    out.push(format!(
                        "{} {} {:?} {:?}",
                        request.method, request.url, request.headers, request.body
                    ));
                }
                if matches!(step.kind, NodeKind::Assert | NodeKind::Condition) {
                    out.push(format!(
                        "{} · {} · {:?} · {:?}",
                        step.display_name(),
                        step.status.label(),
                        step.actual,
                        step.expected
                    ));
                }
                if let Some(children) = &step.children {
                    walk(children, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&report.steps, &mut out);
        out
    }

    /// Criterion 1: the dialog opens with the name focused and the file
    /// named after it; it proposes `product` (Object · Product, required,
    /// from a wire outside the selection), `quantity` (Number, 1 becomes its
    /// default) and `expected` (Number, required) as inputs, and `cart`
    /// ticked with the check's `value` and `passed` unticked as outputs.
    #[test]
    fn ux30_criterion_1_the_dialog_proposes_inputs_and_outputs() {
        for size in [MINIMUM, WIDE] {
            let directory = shop();
            let (mut harness, _) = open_dialog(directory.path(), size);
            assert!(has_label(&harness, TITLE), "the title at {size:?}");
            assert!(
                harness.get_by_label(NAME).is_focused(),
                "the name is focused at {size:?}"
            );
            assert_eq!(field_value(&harness, FILE_NAME), "new-component.afs");
            replace_text(&mut harness, NAME, "Check cart price");
            assert_eq!(
                field_value(&harness, FILE_NAME),
                "check-cart-price.afs",
                "the file follows the name at {size:?}"
            );
            assert_eq!(toggled(&harness, PUBLISH), Some(Toggled::True));
            assert_eq!(toggled(&harness, ONLY_CREATE), Some(Toggled::False));

            for (row, name, value_type, contract, required, caption) in [
                (
                    1,
                    "product",
                    "Object",
                    "Product",
                    true,
                    "Add to cart → product · a wire from outside the selection",
                ),
                (
                    2,
                    "quantity",
                    "Number",
                    "No contract",
                    false,
                    "Add to cart → Quantity · the value 1 becomes its default",
                ),
                (
                    3,
                    "expected",
                    "Number",
                    "No contract",
                    true,
                    "Promotional price → Expected value · a wire from outside the selection",
                ),
            ] {
                assert_eq!(field_value(&harness, &format!("Input {row} name")), name);
                assert_eq!(
                    field_value(&harness, &format!("Input {row} type")),
                    value_type,
                    "{name} at {size:?}"
                );
                assert_eq!(
                    field_value(&harness, &format!("Input {row} contract")),
                    contract,
                    "{name} at {size:?}"
                );
                assert_eq!(
                    toggled(&harness, &format!("Input {row} required")),
                    Some(if required {
                        Toggled::True
                    } else {
                        Toggled::False
                    }),
                    "{name} at {size:?}"
                );
                assert!(has_label(&harness, caption), "{caption} at {size:?}");
            }
            assert!(
                !has_label(&harness, "Input 4 name"),
                "three inputs at {size:?}"
            );

            assert_eq!(toggled(&harness, "Use output 1"), Some(Toggled::True));
            assert_eq!(field_value(&harness, "Output 1 name"), "cart");
            assert_eq!(field_value(&harness, "Output 1 type"), "Object");
            assert_eq!(field_value(&harness, "Output 1 contract"), "Cart");
            assert!(has_label(&harness, "Add to cart → cart"));
            for (row, caption) in [
                (2, "Promotional price → value"),
                (3, "Promotional price → passed"),
            ] {
                assert_eq!(
                    toggled(&harness, &format!("Use output {row}")),
                    Some(Toggled::False),
                    "{caption} at {size:?}"
                );
                assert!(has_label(&harness, caption), "{caption} at {size:?}");
            }
            assert!(!disabled(&harness, SAVE_AND_PUBLISH), "{size:?}");
        }
    }

    /// Criteria 2 and 6: *Save and publish* writes the component, publishes
    /// revision 1 and puts one call in place of the two steps, wired from
    /// the same sources and read by the same cleanup step; undo brings the
    /// steps back, the revision stays, and the dialog's footer says so.
    #[test]
    fn ux30_criteria_2_and_6_save_and_publish_calls_it_and_undo_restores() {
        let directory = shop();
        let (mut harness, id) = open_dialog(directory.path(), WIDE);
        let before = source_of(&harness, &id);
        fill(&mut harness);
        assert!(contains(
            &harness,
            "Undo brings the steps back; revision 1 stays published."
        ));
        choose(&mut harness, SAVE_AND_PUBLISH);
        assert!(!has_label(&harness, NAME), "the dialog closed");

        assert_eq!(
            fs::read_to_string(directory.path().join(FILE)).unwrap(),
            COMPONENT
        );
        assert_eq!(revisions(&harness, "check_cart_price"), vec![1]);
        let published = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .manifest()
            .revisions
            .iter()
            .find(|revision| revision.handle == "check_cart_price")
            .unwrap()
            .path
            .as_str()
            .to_owned();
        assert_eq!(
            fs::read_to_string(directory.path().join(published)).unwrap(),
            COMPONENT,
            "revision 1 is the file as written"
        );
        assert_eq!(source_of(&harness, &id), called(&before));
        assert_eq!(
            fs::read_to_string(directory.path().join(SCENARIO)).unwrap(),
            before,
            "the scenario's change waits to be saved"
        );

        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        settle(&mut harness);
        assert_eq!(source_of(&harness, &id), before, "undo restores the steps");
        assert_eq!(revisions(&harness, "check_cart_price"), vec![1]);
        assert!(directory.path().join(FILE).exists(), "the file stays");
    }

    /// Criterion 3: the scenario's simulation sends the same requests and
    /// finds the same checks before and after the extraction.
    #[test]
    fn ux30_criterion_3_the_simulation_is_the_same_after_the_extraction() {
        let directory = shop();
        let (mut harness, id) = window(directory.path(), WIDE, SCENARIO, "Diagram");
        let before = simulate(&mut harness);
        select_and_open(&mut harness, &id);
        fill(&mut harness);
        choose(&mut harness, SAVE_AND_PUBLISH);
        assert!(source_of(&harness, &id).contains("check_cart_price@1("));
        let after = simulate(&mut harness);
        assert_eq!(after.status, before.status, "{:?}", after.error);
        assert!(
            findings(&before).iter().any(|row| row.contains("/carts")),
            "the cart's requests are compared: {:?}",
            findings(&before)
        );
        assert_eq!(findings(&after), findings(&before));
    }

    /// Criterion 4: *Only create the file* writes the component and leaves
    /// the scenario as it was; nothing is published.
    #[test]
    fn ux30_criterion_4_only_create_the_file_leaves_the_scenario() {
        let directory = shop();
        let (mut harness, id) = open_dialog(directory.path(), WIDE);
        let before = source_of(&harness, &id);
        fill(&mut harness);
        choose(&mut harness, ONLY_CREATE);
        assert_eq!(toggled(&harness, ONLY_CREATE), Some(Toggled::True));
        assert!(!has_label(&harness, SAVE_AND_PUBLISH));
        choose(&mut harness, CREATE_FILE);
        assert!(!has_label(&harness, NAME), "the dialog closed");

        assert_eq!(
            fs::read_to_string(directory.path().join(FILE)).unwrap(),
            COMPONENT
        );
        assert!(revisions(&harness, "check_cart_price").is_empty());
        assert_eq!(source_of(&harness, &id), before);
    }

    /// Criterion 5: a name another component has is refused inline and
    /// disables the button; *Cancel* writes nothing.
    #[test]
    fn ux30_criterion_5_a_taken_name_is_refused_and_cancel_writes_nothing() {
        let directory = shop();
        let files = components(directory.path());
        let (mut harness, id) = open_dialog(directory.path(), WIDE);
        let before = source_of(&harness, &id);
        replace_text(&mut harness, NAME, "Add to cart");
        assert!(has_label(
            &harness,
            "A component named Add to cart already exists."
        ));
        assert!(disabled(&harness, SAVE_AND_PUBLISH));
        replace_text(&mut harness, NAME, "Check cart price");
        assert!(!disabled(&harness, SAVE_AND_PUBLISH));

        choose(&mut harness, "Cancel");
        assert!(!has_label(&harness, NAME), "the dialog closed");
        assert_eq!(components(directory.path()), files);
        assert!(revisions(&harness, "check_cart_price").is_empty());
        assert_eq!(source_of(&harness, &id), before);
    }

    /// The dialog and the scenario after it beside the approved captures
    /// `docs/qa/2026-11-02/COMP-00/comp00-q5-*.png`.
    #[test]
    #[ignore = "design review capture; run with tools/gui/review.sh"]
    fn review_ux30_save_as_component() {
        use crate::testing::capture;
        for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
            let directory = shop();
            let (mut harness, _) = open_dialog(directory.path(), size);
            fill(&mut harness);
            capture(&mut harness, &format!("ux30-dialog-{suffix}"), None);
            choose(&mut harness, SAVE_AND_PUBLISH);
            capture(&mut harness, &format!("ux30-after-{suffix}"), None);
        }
    }
}

/// UX-16: a Cleanup card that reads an earlier Cleanup step names that step,
/// a Main step still resolves, and a step in neither board reads *Missing
/// step*.
mod ux16 {
    use super::ux31::window;
    use super::*;
    use crate::{canvas, testing::MINIMUM};
    use apiforge_core::format::Binding;

    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

    /// A scenario whose Cleanup reads a Cleanup step and a Main step.
    const BOTH: &str = "scenario \"UX-16\" {\n  @label(\"Product\")\n  response = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  cleanup {\n    @label(\"Delete cart\")\n    gone = DELETE \"/shop/carts/1\" { sample 200 { deleted: true } }\n    @label(\"Cart was deleted\")\n    deleted = assert gone.status == 200\n    @label(\"Product was read\")\n    read = assert response.status == 200\n  }\n}\n";

    /// The demo shop, its scenario replaced by `text` when one is given,
    /// with the scenario's Cleanup board chosen through *Cleanup*: what each
    /// card's source rows read, as `(card, row)`.
    fn cleanup_sources(text: Option<&str>) -> Vec<(String, String)> {
        let directory = shop();
        if let Some(text) = text {
            fs::write(directory.path().join(SCENARIO), text).unwrap();
        }
        let (mut harness, id) = window(directory.path(), MINIMUM, SCENARIO, "Diagram");
        harness.get_by_label("Cleanup").click();
        settle(&mut harness);
        folder(&harness).canvases[&id].debug_sources()
    }

    fn row(card: &str, text: &str) -> (String, String) {
        (card.to_owned(), text.to_owned())
    }

    /// Criterion 1: on the demo scenario's Cleanup board, *Cart was deleted*
    /// reads *Delete cart → status*.
    #[test]
    fn the_demo_cleanup_card_names_the_cleanup_step_it_reads() {
        let sources = cleanup_sources(None);
        assert!(
            sources.contains(&row("Cart was deleted", "Delete cart → status")),
            "{sources:?}"
        );
        assert!(
            !sources
                .iter()
                .any(|(_, text)| text.starts_with("Missing step")),
            "{sources:?}"
        );
    }

    /// Criterion 2: a Cleanup card that reads a Main step still resolves it,
    /// beside one that reads a Cleanup step.
    #[test]
    fn a_cleanup_card_still_names_the_main_step_it_reads() {
        assert_eq!(
            cleanup_sources(Some(BOTH)),
            vec![
                row("Cart was deleted", "Delete cart → status"),
                row("Product was read", "Product → status"),
            ]
        );
    }

    /// Criterion 3: a reference to a step that is in neither board reads
    /// *Missing step → …*. Valid source cannot name such a step, so the
    /// demo's Cleanup card is pointed at one and built against the steps of
    /// both boards.
    #[test]
    fn a_reference_to_a_step_in_neither_board_reads_missing_step() {
        let project = apiforge_core::demos::shop_project();
        let scenario = &project.scenarios[0];
        let mut both = scenario.graph.clone();
        both.nodes.extend(scenario.cleanup.nodes.iter().cloned());
        let mut card = scenario
            .cleanup
            .nodes
            .iter()
            .find(|node| node.display_name() == "Cart was deleted")
            .unwrap()
            .clone();
        card.inputs
            .insert("actual".into(), Binding::reference("ghost", "status", ""));
        let built = canvas::Card::build(&card, &both, &project);
        let actual = built
            .inputs
            .iter()
            .find(|input| input.name == "actual")
            .unwrap();
        assert_eq!(
            format!("{:?}", actual.value),
            r#"Source("Missing step → status")"#
        );
    }
}
