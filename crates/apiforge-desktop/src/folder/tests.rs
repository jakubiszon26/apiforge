use super::*;
use crate::buffers::APPLY_DELAY;

#[path = "acceptance_tests.rs"]
mod acceptance_tests;
#[path = "accessibility_tests.rs"]
mod accessibility_tests;
#[path = "activation_tests.rs"]
mod activation_tests;
#[path = "component_run_tests.rs"]
mod component_run_tests;
#[path = "dog_tests.rs"]
mod dog_tests;
#[path = "editor_tests.rs"]
mod editor_tests;
#[path = "header_tests.rs"]
mod header_tests;
#[path = "interaction_tests.rs"]
mod interaction_tests;
#[path = "library_tests.rs"]
mod library_tests;
#[path = "popup_tests.rs"]
mod popup_tests;
#[path = "project_interaction_tests.rs"]
mod project_interaction_tests;
#[path = "recovery_tests.rs"]
mod recovery_tests;
#[path = "report_tests.rs"]
mod report_tests;
#[path = "storage_tests.rs"]
mod storage_tests;

fn frame(app: &mut FolderApp, context: &egui::Context, events: Vec<egui::Event>) {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            events,
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
}

fn key(key: Key, modifiers: Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

const COMMAND: Modifiers = Modifiers {
    ctrl: true,
    command: true,
    alt: false,
    shift: false,
    mac_cmd: false,
};

fn command_shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..COMMAND
    }
}

/// The demo shop as a folder project: one scenario and three components.
fn shop() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    apiforge_project::prepare_project(directory.path(), &apiforge_core::demos::shop_project())
        .unwrap();
    directory
}

fn open(root: &std::path::Path) -> (egui::Context, FolderApp) {
    let context = egui::Context::default();
    let mut app = FolderApp::open(&context, root.to_owned());
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    (context, app)
}

fn wait_storage(app: &mut FolderApp, context: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !app.is_settled() && Instant::now() < deadline {
        frame(app, context, vec![]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!app.storage.is_busy(), "storage did not become idle");
}

fn await_close(app: &mut FolderApp, context: &egui::Context) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if app.prepare_close() {
            return true;
        }
        frame(app, context, vec![]);
        if app.close_error.is_some() {
            return false;
        }
        assert!(Instant::now() < deadline, "close did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn document(app: &FolderApp, path: &str) -> String {
    app.session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .map(|document| document.id().to_owned())
        .unwrap()
}

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const COMPONENT: &str = "components/add-to-cart.afs";

fn text(app: &FolderApp, document_id: &str) -> String {
    app.session
        .as_ref()
        .unwrap()
        .document(document_id)
        .unwrap()
        .text()
        .to_owned()
}

fn type_into_code(app: &mut FolderApp, context: &egui::Context, typed: &str) {
    let active = app.workbench.active().unwrap().to_owned();
    context.memory_mut(|memory| memory.request_focus(editor_id(&active)));
    frame(app, context, vec![]);
    frame(app, context, vec![egui::Event::Text(typed.into())]);
}

/// Lets typing pause past the debounce, then waits for every analysis.
fn settle(app: &mut FolderApp, context: &egui::Context) {
    std::thread::sleep(APPLY_DELAY + Duration::from_millis(10));
    frame(app, context, vec![]);
    let session = app.session.as_mut().unwrap();
    app.analyser.finish(session);
    frame(app, context, vec![]);
}

fn model(app: &FolderApp, document_id: &str) -> Document {
    app.session
        .as_ref()
        .unwrap()
        .document(document_id)
        .unwrap()
        .model()
        .cloned()
        .unwrap()
}

fn first_node(app: &FolderApp, document_id: &str) -> apiforge_core::format::Node {
    graph_of(&model(app, document_id), false).nodes[0].clone()
}

fn move_first_node(app: &mut FolderApp, document_id: &str, to: Position) {
    let node = first_node(app, document_id);
    app.active_board_command(Command::Move(vec![(node.id, to)]));
}

#[test]
fn a_folder_without_a_project_is_written_only_when_initialised() {
    let directory = tempfile::tempdir().unwrap();
    let (context, mut app) = open(directory.path());
    frame(&mut app, &context, vec![]);
    assert!(app.uninitialised && !app.has_document());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);

    app.new_name = "Payments".into();
    app.initialise();
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    let session = app.session.as_ref().unwrap();
    assert_eq!(session.manifest().name, "Payments");
    assert!(session.documents().is_empty(), "an empty project is valid");
    assert_eq!(
        app.workbench.mode,
        Mode::Code,
        "a new project starts in Code"
    );
}

#[test]
fn a_first_open_shows_the_first_scenario_in_code() {
    let directory = shop();
    let (_context, app) = open(directory.path());
    assert_eq!(app.workbench.mode, Mode::Code);
    assert_eq!(
        app.workbench.active(),
        Some(document(&app, SCENARIO).as_str())
    );
}

#[test]
fn switching_modes_and_tabs_keeps_unsaved_text_its_history_and_the_caret() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let component = document(&app, COMPONENT);
    let original = text(&app, &scenario);

    type_into_code(&mut app, &context, "// note\n");
    let typed = text(&app, &scenario);
    assert_eq!(typed, format!("// note\n{original}"));
    let caret = app.workbench.active_tab().unwrap().selection.clone();
    assert_eq!(caret.caret, "// note\n".len());

    // Ctrl+E with the caret in the code still switches the view.
    frame(&mut app, &context, vec![key(Key::E, COMMAND)]);
    assert_eq!(app.workbench.mode, Mode::Diagram);
    frame(&mut app, &context, vec![]);
    app.workbench.mode = Mode::Split;
    frame(&mut app, &context, vec![]);
    app.workbench.open(&component);
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::Tab, COMMAND)]);
    assert_eq!(app.workbench.active(), Some(scenario.as_str()), "wraps");
    frame(&mut app, &context, vec![key(Key::E, COMMAND)]);
    assert_eq!(app.workbench.mode, Mode::Code, "Split goes to Code");
    frame(&mut app, &context, vec![]);

    assert_eq!(text(&app, &scenario), typed, "nothing saved or reset");
    assert_eq!(
        code::selection(&context, editor_id(&scenario), &typed),
        Some((caret.anchor, caret.caret)),
        "the caret came back"
    );
    // The text edit's own history still holds the typing.
    context.memory_mut(|memory| memory.request_focus(editor_id(&scenario)));
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::Z, COMMAND)]);
    assert_eq!(text(&app, &scenario), original);
}

#[test]
fn model_history_survives_switches_and_ctrl_z_follows_the_focus() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let component = document(&app, COMPONENT);
    let moved = Position { x: 900.0, y: 700.0 };
    app.workbench.mode = Mode::Diagram;
    frame(&mut app, &context, vec![]);
    let before = first_node(&app, &scenario).position;
    move_first_node(&mut app, &scenario, moved.clone());
    assert_eq!(first_node(&app, &scenario).position, moved);
    let session = app.session.as_ref().unwrap();
    assert!(session.document(&scenario).unwrap().is_layout_dirty());
    assert!(
        !session.document(&scenario).unwrap().is_dirty(),
        "layout only"
    );

    app.workbench.open(&component);
    frame(&mut app, &context, vec![]);
    app.workbench.mode = Mode::Code;
    frame(&mut app, &context, vec![]);
    app.workbench.open(&scenario);
    frame(&mut app, &context, vec![]);

    // The caret is in the code: Ctrl+Z belongs to the text, not the model.
    context.memory_mut(|memory| memory.request_focus(editor_id(&scenario)));
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::Z, COMMAND)]);
    assert_eq!(first_node(&app, &scenario).position, moved);
    context.memory_mut(|memory| memory.surrender_focus(editor_id(&scenario)));
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::Z, COMMAND)]);
    assert_eq!(first_node(&app, &scenario).position, before);
    frame(&mut app, &context, vec![key(Key::Z, command_shift())]);
    assert_eq!(first_node(&app, &scenario).position, moved);
}

#[test]
fn invalid_code_locks_only_its_own_diagram_and_shows_the_last_valid_one() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let component = document(&app, COMPONENT);
    type_into_code(&mut app, &context, "}}}");
    settle(&mut app, &context);
    let snapshot = app.snapshot(&scenario).unwrap();
    assert_eq!(snapshot.analysis, AnalysisState::Invalid);
    assert_eq!(snapshot.model_state, ModelState::LastValid);
    assert!(!snapshot.editable && !snapshot.diagnostics.is_empty());

    app.workbench.mode = Mode::Diagram;
    frame(&mut app, &context, vec![]);
    let before = first_node(&app, &scenario).position;
    move_first_node(&mut app, &scenario, Position { x: 1.0, y: 2.0 });
    assert_eq!(first_node(&app, &scenario).position, before, "read-only");

    app.workbench.open(&component);
    frame(&mut app, &context, vec![]);
    move_first_node(&mut app, &component, Position { x: 3.0, y: 4.0 });
    assert_eq!(
        first_node(&app, &component).position,
        Position { x: 3.0, y: 4.0 },
        "another document stays editable"
    );
}

#[test]
fn a_graphical_edit_reprints_the_source_and_retires_its_text_history() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    // A blank first line is valid but not how the printer writes it.
    context.memory_mut(|memory| memory.request_focus(editor_id(&scenario)));
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::Enter, Modifiers::NONE)]);
    settle(&mut app, &context);
    assert!(text(&app, &scenario).starts_with('\n'));
    let node = first_node(&app, &scenario);
    let current = model(&app, &scenario);
    let mut renamed = graph_of(&current, false).clone();
    renamed.nodes[0].label = Some("Renamed product".into());
    app.apply_edit(&scenario, &current, false, Ok(renamed.into()));
    frame(&mut app, &context, vec![]);
    let reprinted = text(&app, &scenario);
    assert!(reprinted.contains("Renamed product"), "{reprinted}");
    assert!(reprinted.starts_with("scenario"), "canonical print");
    assert_eq!(first_node(&app, &scenario).id, node.id);

    // Undoing in the text cannot type back the source the graph replaced.
    context.memory_mut(|memory| memory.request_focus(editor_id(&scenario)));
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::Z, COMMAND)]);
    assert_eq!(text(&app, &scenario), reprinted);
}

#[test]
fn ctrl_s_saves_exact_invalid_text_and_save_all_saves_each_document() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let component = document(&app, COMPONENT);
    type_into_code(&mut app, &context, "}}}");
    let invalid = text(&app, &scenario);
    frame(&mut app, &context, vec![key(Key::S, COMMAND)]);
    wait_storage(&mut app, &context);
    assert_eq!(
        std::fs::read_to_string(directory.path().join(SCENARIO)).unwrap(),
        invalid,
        "saved exactly, errors included"
    );

    // A hidden document and the active one, each with unsaved work.
    let edited = format!("// helper\n{}", text(&app, &component));
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&component, &edited, CODE)
        .unwrap();
    type_into_code(&mut app, &context, "x");
    frame(&mut app, &context, vec![key(Key::S, command_shift())]);
    wait_storage(&mut app, &context);
    assert_eq!(
        std::fs::read_to_string(directory.path().join(COMPONENT)).unwrap(),
        edited
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join(SCENARIO)).unwrap(),
        text(&app, &scenario)
    );
    let session = app.session.as_ref().unwrap();
    assert!(
        session
            .documents()
            .iter()
            .all(|document| !document.is_dirty())
    );
}

#[test]
fn closing_a_tab_with_unsaved_work_asks_and_discarding_restores_the_saved_file() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let original = text(&app, &scenario);
    type_into_code(&mut app, &context, "// draft\n");
    frame(&mut app, &context, vec![key(Key::W, COMMAND)]);
    assert!(matches!(&app.prompt, Some(Prompt::CloseTab(id)) if *id == scenario));
    assert_eq!(
        app.workbench.active(),
        Some(scenario.as_str()),
        "still open"
    );

    app.prompt = None;
    app.close_tab_with(&scenario, false);
    wait_storage(&mut app, &context);
    assert_eq!(text(&app, &scenario), original);
    assert_eq!(app.workbench.active(), None);
    assert_eq!(
        std::fs::read_to_string(directory.path().join(SCENARIO)).unwrap(),
        original
    );
}

#[test]
fn the_view_and_unsaved_work_come_back_after_closing_the_window() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let component = document(&app, COMPONENT);
    let saved = std::fs::read_to_string(directory.path().join(COMPONENT)).unwrap();
    app.workbench.open(&component);
    frame(&mut app, &context, vec![]);
    type_into_code(&mut app, &context, "// unsaved\n");
    let unsaved = text(&app, &component);
    app.workbench.mode = Mode::Split;
    frame(&mut app, &context, vec![]);
    assert!(
        await_close(&mut app, &context),
        "unsaved work is in local recovery"
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join(COMPONENT)).unwrap(),
        saved,
        "closing never saves a source"
    );

    let (_context, reopened) = open(directory.path());
    assert_eq!(reopened.workbench.mode, Mode::Split);
    let order: Vec<_> = reopened
        .workbench
        .tabs()
        .iter()
        .map(|tab| tab.document_id.clone())
        .collect();
    assert_eq!(order, [scenario, component.clone()]);
    assert_eq!(reopened.workbench.active(), Some(component.as_str()));
    assert_eq!(text(&reopened, &component), unsaved);
    assert_eq!(
        reopened.workbench.active_tab().unwrap().selection.caret,
        "// unsaved\n".len()
    );
}

#[test]
fn a_read_only_window_keeps_edits_in_memory_and_refuses_to_close_silently() {
    let directory = shop();
    let (_writer_context, _writer) = open(directory.path());
    let (context, mut reader) = open(directory.path());
    assert!(reader.session.as_ref().unwrap().is_read_only());
    let scenario = document(&reader, SCENARIO);
    let original = std::fs::read_to_string(directory.path().join(SCENARIO)).unwrap();
    type_into_code(&mut reader, &context, "// mine\n");
    reader.notice = None;
    frame(&mut reader, &context, vec![key(Key::S, COMMAND)]);
    wait_storage(&mut reader, &context);
    assert!(reader.notice.as_ref().is_some_and(|notice| notice.error));
    assert_eq!(
        std::fs::read_to_string(directory.path().join(SCENARIO)).unwrap(),
        original
    );
    assert!(!await_close(&mut reader, &context));
    assert!(reader.close_error().is_some());
    assert!(text(&reader, &scenario).starts_with("// mine"));
}

#[test]
fn new_documents_start_from_valid_sources_with_unused_paths() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    for _ in 0..2 {
        app.create_document(DocumentKind::Scenario, None, "Checkout \"fast\" path")
            .unwrap();
        wait_storage(&mut app, &context);
    }
    app.create_document(DocumentKind::Component, None, "3d secure check")
        .unwrap();
    wait_storage(&mut app, &context);
    settle(&mut app, &context);
    let session = app.session.as_ref().unwrap();
    let paths: Vec<_> = session
        .documents()
        .iter()
        .map(|document| document.entry().path.as_str())
        .collect();
    assert!(paths.contains(&"scenarios/checkout-fast-path.afs"));
    assert!(paths.contains(&"scenarios/checkout-fast-path-2.afs"));
    assert!(paths.contains(&"components/3d-secure-check.afs"));
    for document in session.documents() {
        assert_eq!(
            document.analysis(),
            AnalysisState::Valid,
            "{}: {:?}",
            document.entry().path.as_str(),
            document.diagnostics()
        );
    }
}

#[test]
fn line_and_column_count_characters() {
    assert_eq!(line_column("ab\nżółw", 0), (1, 1));
    assert_eq!(line_column("ab\nżółw", 3), (2, 1));
    assert_eq!(line_column("ab\nżółw", 5), (2, 2));
    assert_eq!(line_column("ab\nżółw", 6), (2, 2), "inside a character");
}

#[test]
fn every_mode_draws_at_the_minimum_window_with_an_invalid_document() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    type_into_code(&mut app, &context, "}}}");
    settle(&mut app, &context);
    for mode in [Mode::Code, Mode::Diagram, Mode::Split] {
        app.workbench.mode = mode;
        for _ in 0..2 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 540.0),
                    )),
                    ..Default::default()
                },
                |ui| app.ui(ui),
            );
            output.textures_delta.clear();
        }
        assert_eq!(app.workbench.mode, mode);
    }
}

/// KTLO-23a adds a component tab (decision 6): its toolbar, with *Publish*,
/// *Run options* and *Run*, is wider than a scenario's, and the title keeps
/// one width on both kinds of tab.
#[test]
fn long_project_names_do_not_overlap_header_actions_in_writable_or_read_only_windows() {
    let name = "Customer integration and contract verification workspace";
    let (directory, scenario) = named_folder(name);
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let component = session
        .create_document(
            DocumentKind::Component,
            "components/helper.afs",
            "component Helper \"Helper\" {\n  export result: number = 2\n}\n",
        )
        .unwrap();
    drop(session);
    let (context, mut writer) = open(directory.path());
    let (read_context, mut reader) = open(directory.path());
    assert!(reader.session.as_ref().unwrap().is_read_only());
    for (app, context) in [(&mut writer, &context), (&mut reader, &read_context)] {
        for size in [egui::vec2(960.0, 540.0), egui::vec2(1600.0, 1000.0)] {
            let mut titles = Vec::new();
            for (tab, actions) in [
                (&scenario, &["Save", "Save all", "Run options", "Run"][..]),
                (
                    &component,
                    &["Save", "Save all", "Publish", "Run options", "Run"][..],
                ),
            ] {
                app.workbench.open(tab);
                let _ = painted(app, context, size);
                let labels = painted(app, context, size);
                let title = labels
                    .iter()
                    .find(|(text, rect)| text == name && rect.top() < 40.0)
                    .expect("the project name is visible")
                    .1;
                assert!(title.height() <= 18.0, "the header uses compact UI text");
                let project = labels.iter().find(|(text, _)| text == "Project").unwrap().1;
                let code = labels.iter().find(|(text, _)| text == "Code").unwrap().1;
                assert!(project.right() + 8.0 <= title.left());
                assert!(title.right() + 8.0 <= code.left(), "{title:?}, {code:?}");
                for action in actions {
                    let rect = labels
                        .iter()
                        .find(|(text, rect)| text == action && rect.top() < 40.0)
                        .unwrap_or_else(|| panic!("{action} on the header"))
                        .1;
                    assert!(rect.right() <= size.x, "{action} remains visible");
                }
                titles.push(title.width());
            }
            assert_eq!(titles[0], titles[1], "one title width on every tab");
        }
    }
}

#[test]
fn unfinished_inspector_text_joins_recovery_and_returns_on_reopening() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let project_id = app.session.as_ref().unwrap().manifest().id.clone();
    app.workbench.mode = Mode::Diagram;
    let node = first_node(&app, &scenario);
    app.selections.insert(scenario.clone(), vec![node.id]);
    let draft = CodeDraft {
        project_id: project_id.clone(),
        target: DraftTarget::Field {
            owner: Box::new(inspector::drafts::document_target(&scenario)),
            widget: 42,
        },
        base_hash: apiforge_storage::draft_base_hash("80"),
        text: "{{ unfinished".into(),
        cursor: 4,
    };
    inspector::drafts::restore_entries(&context, vec![draft.clone()]);
    frame(&mut app, &context, vec![]);
    let fields = app
        .session
        .as_ref()
        .unwrap()
        .document(&scenario)
        .unwrap()
        .field_drafts()
        .to_vec();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].text, draft.text);
    let closed = await_close(&mut app, &context);
    assert!(closed, "{:?}", app.close_error());

    let (reopened_context, _reopened) = open(directory.path());
    assert_eq!(
        inspector::drafts::entries(&reopened_context),
        vec![draft],
        "the field gets its text back"
    );
}

fn unfinished_field(app: &FolderApp, document_id: &str) -> CodeDraft {
    CodeDraft {
        project_id: app.session.as_ref().unwrap().manifest().id.clone(),
        target: DraftTarget::Field {
            owner: Box::new(inspector::drafts::document_target(document_id)),
            widget: 42,
        },
        base_hash: apiforge_storage::draft_base_hash("80"),
        text: "{{ unfinished".into(),
        cursor: 4,
    }
}

#[test]
fn discarding_a_tab_also_forgets_its_unfinished_inspector_text() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let draft = unfinished_field(&app, &scenario);
    let project_id = draft.project_id.clone();
    inspector::drafts::restore_entries(&context, vec![draft]);
    app.sync_field_drafts(&scenario, &project_id);
    assert_eq!(
        app.session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .field_drafts()
            .len(),
        1
    );

    app.close_tab_with(&scenario, false);
    wait_storage(&mut app, &context);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .field_drafts()
            .is_empty()
    );
    app.workbench.open(&scenario);
    app.workbench.mode = Mode::Diagram;
    let node = first_node(&app, &scenario);
    app.selections.insert(scenario.clone(), vec![node.id]);
    frame(&mut app, &context, vec![]);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .field_drafts()
            .is_empty(),
        "Discard must clear the inspector registry as well as the session"
    );
}

#[test]
fn keeping_a_stale_draft_keeps_its_unfinished_inspector_text() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let draft = unfinished_field(&app, &scenario);
    let project_id = draft.project_id.clone();
    inspector::drafts::restore_entries(&context, vec![draft.clone()]);
    app.sync_field_drafts(&scenario, &project_id);
    assert!(await_close(&mut app, &context));
    let path = directory.path().join(SCENARIO);
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("// external change\n{source}")).unwrap();

    let (context, mut reopened) = open(directory.path());
    assert!(reopened.snapshot(&scenario).unwrap().held);
    // The same action as the Keep unsaved work button.
    reopened
        .session
        .as_mut()
        .unwrap()
        .keep_draft(&scenario)
        .unwrap();
    assert_eq!(
        reopened
            .session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .field_drafts()
            .len(),
        1
    );
    reopened.workbench.mode = Mode::Diagram;
    let node = first_node(&reopened, &scenario);
    reopened.selections.insert(scenario.clone(), vec![node.id]);
    frame(&mut reopened, &context, vec![]);
    assert_eq!(
        reopened
            .session
            .as_ref()
            .unwrap()
            .document(&scenario)
            .unwrap()
            .field_drafts()
            .len(),
        1,
        "Showing the inspector must not erase the fields of a kept recovery draft"
    );
}

#[test]
fn a_failed_discard_on_close_keeps_the_window_usable() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    type_into_code(&mut app, &context, "// unfinished\n");
    app.session.as_mut().unwrap().save_recovery().unwrap();
    type_into_code(&mut app, &context, "// later work\n");
    // A foreign edit makes both the protective close and clearing recovery fail.
    std::fs::write(directory.path().join(".apiforge/local/recovery.json"), b"{").unwrap();
    assert!(!await_close(&mut app, &context));
    let error = app.close_error().unwrap();
    app.abort_close(error);

    fn label_position(shape: &egui::epaint::Shape) -> Option<egui::Pos2> {
        match shape {
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(label_position),
            egui::epaint::Shape::Text(text)
                if text.galley.job.text == "Discard unsaved work and close" =>
            {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            _ => None,
        }
    }
    frame(&mut app, &context, vec![]);
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    let position = output
        .shapes
        .iter()
        .find_map(|shape| label_position(&shape.shape))
        .unwrap();
    output.textures_delta.clear();
    for pressed in [true, false] {
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    // Discarding runs on the storage worker, after any external-change check
    // in flight; its answer can take more than the click's two frames.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(app.prompt, Some(Prompt::CloseFailed(_))) && Instant::now() < deadline {
        frame(&mut app, &context, vec![]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        matches!(app.prompt, Some(Prompt::CloseFailed(_))),
        "The failing discard should report an error"
    );
    assert!(
        app.session.is_some(),
        "A failed discard must retain the session for Keep editing or Try again"
    );
}

#[test]
fn discarding_a_tab_clears_unfinished_text_held_by_the_url_field() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let component = document(&app, COMPONENT);
    app.workbench.open(&component);
    app.workbench.mode = Mode::Diagram;
    let current = model(&app, &component);
    let node = graph_of(&current, false)
        .nodes
        .iter()
        .find(|node| node.kind == apiforge_core::model::NodeKind::Request)
        .unwrap()
        .clone();
    app.selections
        .insert(component.clone(), vec![node.id.clone()]);
    frame(&mut app, &context, vec![]);
    let url_id = egui::Id::new(("inspector", &node.id))
        .with("url")
        .with("text");
    context.memory_mut(|memory| memory.request_focus(url_id));
    frame(&mut app, &context, vec![]);
    frame(
        &mut app,
        &context,
        vec![egui::Event::Text("{{ unfinished".into())],
    );
    assert!(
        !app.session
            .as_ref()
            .unwrap()
            .document(&component)
            .unwrap()
            .field_drafts()
            .is_empty(),
        "The actual URL widget must contain an unfinished draft before discard"
    );
    app.close_tab_with(&component, false);
    wait_storage(&mut app, &context);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&component)
            .unwrap()
            .field_drafts()
            .is_empty()
    );
    app.workbench.open(&component);
    app.selections.insert(component.clone(), vec![node.id]);
    frame(&mut app, &context, vec![]);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&component)
            .unwrap()
            .field_drafts()
            .is_empty(),
        "Discard must also clear the widget's cached invalid TemplateDraft"
    );
}

#[test]
fn discarding_a_tab_clears_unfinished_text_held_by_the_body_field() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let component = document(&app, COMPONENT);
    app.workbench.open(&component);
    app.workbench.mode = Mode::Diagram;
    let current = model(&app, &component);
    let node = graph_of(&current, false)
        .nodes
        .iter()
        .find(|node| node.kind == apiforge_core::model::NodeKind::Request)
        .unwrap()
        .clone();
    app.selections
        .insert(component.clone(), vec![node.id.clone()]);
    frame(&mut app, &context, vec![]);
    let body_id = egui::Id::new(("inspector", &node.id))
        .with(("argument", apiforge_core::graph::BODY_INPUT))
        .with("body-source")
        .with("text");
    context.memory_mut(|memory| memory.request_focus(body_id));
    frame(&mut app, &context, vec![]);
    frame(
        &mut app,
        &context,
        vec![egui::Event::Text("{{ unfinished".into())],
    );
    assert!(
        !app.session
            .as_ref()
            .unwrap()
            .document(&component)
            .unwrap()
            .field_drafts()
            .is_empty(),
        "The actual body widget must contain an unfinished draft before discard"
    );
    app.close_tab_with(&component, false);
    wait_storage(&mut app, &context);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&component)
            .unwrap()
            .field_drafts()
            .is_empty()
    );
    app.workbench.open(&component);
    app.selections.insert(component.clone(), vec![node.id]);
    frame(&mut app, &context, vec![]);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&component)
            .unwrap()
            .field_drafts()
            .is_empty(),
        "Discard must also clear the widget's cached invalid BodyDraft"
    );
}

/// Checks the folder now, as the watcher would, and applies the result.
fn watch(app: &mut FolderApp, context: &egui::Context) {
    app.watcher.rescan();
    frame(app, context, vec![]);
    let session = app.session.as_mut().unwrap();
    if let Some(outcome) = app.watcher.finish(session) {
        app.follow_external(outcome);
    }
    frame(app, context, vec![]);
    wait_storage(app, context);
}

fn rows(app: &FolderApp) -> Vec<String> {
    app.explorer_rows()
        .into_iter()
        .map(|row| row.path)
        .collect()
}

#[test]
fn the_explorer_shows_the_real_tree_and_folds_folders() {
    let directory = shop();
    std::fs::write(directory.path().join("README.md"), "notes").unwrap();
    std::fs::write(
        directory.path().join("loose.afs"),
        "scenario \"Loose\" {}\n",
    )
    .unwrap();
    let (context, mut app) = open(directory.path());
    watch(&mut app, &context);
    let shown = rows(&app);
    assert!(shown.contains(&"components".to_owned()));
    assert!(shown.contains(&"README.md".to_owned()));
    assert!(!shown.iter().any(|path| path.starts_with(".apiforge")));
    assert!(!shown.contains(&COMPONENT.to_owned()), "folded");
    app.explorer_action(explorer::Action::Toggle("components".into()));
    wait_storage(&mut app, &context);
    assert!(rows(&app).contains(&COMPONENT.to_owned()));
    let loose = app
        .explorer_rows()
        .into_iter()
        .find(|row| row.path == "loose.afs")
        .unwrap();
    assert_eq!(loose.kind, explorer::RowKind::Source, "not registered");
}

#[test]
fn renaming_an_open_document_keeps_its_tab_unsaved_text_and_history() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    type_into_code(&mut app, &context, "// note\n");
    let edited = text(&app, &id);
    app.relocate(SCENARIO, "flows/renamed.afs").unwrap();
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    assert_eq!(app.workbench.active(), Some(id.as_str()));
    assert_eq!(text(&app, &id), edited);
    assert!(app.snapshot(&id).unwrap().dirty);
    assert_eq!(app.snapshot(&id).unwrap().path, "flows/renamed.afs");
    assert!(app.workbench.expanded.contains("flows"));
    watch(&mut app, &context);
    assert!(rows(&app).contains(&"flows/renamed.afs".to_owned()));
    // Moving into an occupied path is refused and changes nothing.
    std::fs::write(directory.path().join("taken.afs"), "x").unwrap();
    app.relocate("flows/renamed.afs", "taken.afs").unwrap();
    wait_storage(&mut app, &context);
    assert!(app.notice.as_ref().is_some_and(|notice| notice.error));
    assert_eq!(app.snapshot(&id).unwrap().path, "flows/renamed.afs");
}

#[test]
fn deleting_a_folder_closes_its_tabs_and_the_trash_restores_unsaved_work() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    type_into_code(&mut app, &context, "// unsaved\n");
    let edited = text(&app, &id);
    app.request_delete(vec!["scenarios".into()]);
    wait_storage(&mut app, &context);
    let Some(Prompt::Delete { preview, .. }) = app.prompt.take() else {
        panic!("a delete confirmation");
    };
    assert_eq!(preview.unsaved_documents(), std::slice::from_ref(&id));
    app.delete_confirmed(&preview).unwrap();
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    assert!(app.workbench.tabs().iter().all(|tab| tab.document_id != id));
    assert!(!directory.path().join("scenarios").exists());

    app.open_trash();
    wait_storage(&mut app, &context);
    let Some(Prompt::Trash { operations, .. }) = app.prompt.take() else {
        panic!("the trash");
    };
    let operation = operations[0].operation_id.clone();
    app.restore(&operation, &operations).unwrap();
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    assert_eq!(app.workbench.active(), Some(id.as_str()));
    assert_eq!(text(&app, &id), edited, "unsaved work returns unsaved");
    assert!(directory.path().join(SCENARIO).exists());
}

#[test]
fn external_edits_reload_clean_tabs_and_pause_saving_unsaved_ones() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let component = document(&app, COMPONENT);
    app.workbench.open(&component);
    let component_file = directory.path().join(COMPONENT);
    let changed = std::fs::read_to_string(&component_file).unwrap().replacen(
        '{',
        "{\n  // edited elsewhere",
        1,
    );
    std::fs::write(&component_file, &changed).unwrap();
    watch(&mut app, &context);
    assert_eq!(text(&app, &component), changed);

    app.workbench.open(&scenario);
    frame(&mut app, &context, vec![]);
    type_into_code(&mut app, &context, "// mine\n");
    let scenario_file = directory.path().join(SCENARIO);
    std::fs::write(&scenario_file, "scenario \"Theirs\" {}\n").unwrap();
    watch(&mut app, &context);
    assert_eq!(
        app.snapshot(&scenario).unwrap().conflict,
        Some(ExternalConflict::Changed)
    );
    assert!(app.notice.as_ref().is_some_and(|notice| notice.error));
    frame(&mut app, &context, vec![key(Key::S, COMMAND)]);
    wait_storage(&mut app, &context);
    assert_eq!(
        std::fs::read_to_string(&scenario_file).unwrap(),
        "scenario \"Theirs\" {}\n",
        "Ctrl+S never overwrites the external version"
    );
    app.session
        .as_mut()
        .unwrap()
        .accept_external(&scenario, DIAGRAM)
        .unwrap();
    frame(&mut app, &context, vec![]);
    assert_eq!(text(&app, &scenario), "scenario \"Theirs\" {}\n");
}

#[test]
fn the_explorer_is_driven_by_the_keyboard() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    watch(&mut app, &context);
    let id = document(&app, COMPONENT);
    app.explorer.selected = Some("components".into());
    context.memory_mut(|memory| memory.request_focus(egui::Id::new("folder-explorer")));
    frame(&mut app, &context, vec![]);
    frame(
        &mut app,
        &context,
        vec![key(Key::ArrowRight, Modifiers::NONE)],
    );
    assert!(app.workbench.expanded.contains("components"));
    frame(
        &mut app,
        &context,
        vec![key(Key::ArrowDown, Modifiers::NONE)],
    );
    let selected = app.explorer.selected.clone().unwrap();
    assert!(selected.starts_with("components/"), "{selected}");
    app.explorer.selected = Some(COMPONENT.into());
    frame(&mut app, &context, vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(app.workbench.active(), Some(id.as_str()));
    context.memory_mut(|memory| memory.request_focus(egui::Id::new("folder-explorer")));
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::F2, Modifiers::NONE)]);
    assert!(matches!(&app.prompt, Some(Prompt::Rename { path, .. }) if path == COMPONENT));
    // The dialog's field took the keyboard; the explorer gets it back on a click.
    app.prompt = None;
    frame(&mut app, &context, vec![]);
    context.memory_mut(|memory| memory.request_focus(egui::Id::new("folder-explorer")));
    frame(&mut app, &context, vec![]);
    frame(
        &mut app,
        &context,
        vec![key(Key::ArrowLeft, Modifiers::NONE)],
    );
    assert_eq!(app.explorer.selected.as_deref(), Some("components"));
}

#[test]
fn initialising_registers_the_chosen_existing_sources() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("main.afs"), "scenario \"Main\" {}\n").unwrap();
    std::fs::write(directory.path().join("odd.afs"), "?\n").unwrap();
    let (context, mut app) = open(directory.path());
    assert_eq!(app.survey.len(), 2);
    let odd = app
        .survey
        .iter()
        .position(|row| row.path == "odd.afs")
        .unwrap();
    assert_eq!(app.survey[odd].kind, None);
    app.survey[odd].include = false;
    app.new_name = "Existing".into();
    app.initialise();
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    let session = app.session.as_ref().unwrap();
    assert_eq!(session.documents().len(), 1);
    assert_eq!(session.documents()[0].entry().path.as_str(), "main.afs");
    assert_eq!(
        std::fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        "scenario \"Main\" {}\n"
    );
}

#[test]
fn a_found_source_is_registered_from_the_explorer() {
    let directory = shop();
    std::fs::write(
        directory.path().join("found.afs"),
        "scenario \"Found\" {}\n",
    )
    .unwrap();
    let (context, mut app) = open(directory.path());
    watch(&mut app, &context);
    app.explorer_action(explorer::Action::Register("found.afs".into()));
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    let id = document(&app, "found.afs");
    assert_eq!(app.workbench.active(), Some(id.as_str()));
    watch(&mut app, &context);
    let row = app
        .explorer_rows()
        .into_iter()
        .find(|row| row.path == "found.afs")
        .unwrap();
    assert!(matches!(row.kind, explorer::RowKind::Document { .. }));
}

/// BUG-035: the library's uniform card size left two analysed blocks 120 points
/// apart, which the painted cards are taller than.
#[test]
fn fresh_cards_are_placed_at_the_sizes_the_board_draws() {
    let directory = tempfile::tempdir().unwrap();
    let mut session =
        ProjectSession::create_with(directory.path(), "Placement", Arc::new(CardPlacer)).unwrap();
    let id = session
        .create_document(
            DocumentKind::Scenario,
            "scenarios/alpha.afs",
            "scenario \"QA Alpha\" {\n  alpha = 7\n  beta = alpha + 3\n}\n",
        )
        .unwrap();
    drop(session);
    let (context, mut app) = open(directory.path());
    app.workbench.open(&id);
    settle(&mut app, &context);
    let model = model(&app, &id);
    let graph = graph_of(&model, false);
    assert_eq!(graph.nodes.len(), 2);
    let project = app.session.as_ref().unwrap().context().clone();
    let cards: Vec<egui::Rect> = graph
        .nodes
        .iter()
        .map(|node| canvas::Card::build(node, graph, &project).rect())
        .collect();
    assert!(
        !cards[0].intersects(cards[1]),
        "painted cards overlap: {:?} and {:?}",
        cards[0],
        cards[1]
    );
}

// ----- P6 UI quality regressions ------------------------------------------
//
// These drive the window with real pointer and keyboard events: calling an
// action directly would not show that a click reaches it.

/// Every text actually visible this frame, with the rectangle it occupies.
fn painted(
    app: &mut FolderApp,
    context: &egui::Context,
    size: egui::Vec2,
) -> Vec<(String, egui::Rect)> {
    fn collect(shape: &egui::epaint::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, clip, out);
                }
            }
            egui::epaint::Shape::Text(text) => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                if clip.contains(rect.center()) {
                    out.push((text.galley.job.text.clone(), rect));
                }
            }
            _ => {}
        }
    }
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
    let mut texts = Vec::new();
    for shape in &output.shapes {
        collect(&shape.shape, shape.clip_rect, &mut texts);
    }
    texts
}

/// The centre of a painted explorer row, which is left of the code area.
fn row_at(app: &mut FolderApp, context: &egui::Context, name: &str) -> egui::Pos2 {
    fn find(app: &mut FolderApp, context: &egui::Context, name: &str) -> Option<egui::Pos2> {
        fn collect(
            shape: &egui::epaint::Shape,
            clip: egui::Rect,
            out: &mut Vec<(String, egui::Rect, egui::Rect)>,
        ) {
            match shape {
                egui::epaint::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, clip, out);
                    }
                }
                egui::epaint::Shape::Text(text) => out.push((
                    text.galley.job.text.clone(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                    clip,
                )),
                _ => {}
            }
        }
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                ..Default::default()
            },
            |ui| app.ui(ui),
        );
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for shape in &output.shapes {
            collect(&shape.shape, shape.clip_rect, &mut texts);
        }
        texts
            .into_iter()
            .find(|(text, rect, clip)| {
                text == name
                    && rect.center().x < 230.0
                    && rect.center().y > 90.0
                    && clip.contains(rect.center())
            })
            .map(|(_, rect, _)| rect.center())
    }
    find(app, context, name).unwrap_or_else(|| panic!("no visible explorer row {name}"))
}

fn click(app: &mut FolderApp, context: &egui::Context, at: egui::Pos2) {
    frame(app, context, vec![egui::Event::PointerMoved(at)]);
    for pressed in [true, false] {
        frame(
            app,
            context,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    wait_storage(app, context);
}

#[test]
fn one_explorer_click_opens_a_file_and_the_project_row_is_its_own_destination() {
    let directory = shop();
    std::fs::write(directory.path().join("README.md"), "# Shop\n").unwrap();
    let (context, mut app) = open(directory.path());
    watch(&mut app, &context);
    let scenario = document(&app, SCENARIO);
    let component = document(&app, COMPONENT);
    // Another document is active to begin with.
    app.workbench.open(&component);
    frame(&mut app, &context, vec![]);
    app.workbench.expanded.insert("scenarios".into());
    frame(&mut app, &context, vec![]);

    let at = row_at(&mut app, &context, file_name(SCENARIO));
    click(&mut app, &context, at);
    assert_eq!(
        app.workbench.active(),
        Some(scenario.as_str()),
        "one click must open the file it selects"
    );
    // Clicking it again keeps one tab for it.
    let tabs = app.workbench.tabs().len();
    click(&mut app, &context, at);
    assert_eq!(app.workbench.tabs().len(), tabs);
    assert_eq!(app.workbench.active(), Some(scenario.as_str()));

    // The project itself is a row, and new items go into it.
    let root = row_at(&mut app, &context, "Demo shop");
    click(&mut app, &context, root);
    assert_eq!(app.explorer.selected.as_deref(), Some(""));
    let rows = app.explorer_rows();
    let selected = rows.iter().find(|row| row.path.is_empty()).unwrap();
    assert_eq!(explorer::folder_of(Some(selected)), "");
    assert!(!selected.portable, "the project is not renamed or deleted");
    // Selecting the project does not hide its tree; its chevron folds it.
    assert!(app.explorer_rows().len() > 1);
    let chevron = egui::pos2(19.0, root.y);
    click(&mut app, &context, chevron);
    assert_eq!(
        app.explorer_rows().len(),
        1,
        "the chevron folds the project"
    );
    click(&mut app, &context, chevron);
    assert!(app.explorer_rows().len() > 1);
}

#[test]
fn the_keyboard_reaches_the_project_row_and_opens_a_text_file() {
    let directory = shop();
    std::fs::write(directory.path().join("README.md"), "# Shop\n").unwrap();
    let (context, mut app) = open(directory.path());
    watch(&mut app, &context);
    let at = row_at(&mut app, &context, "README.md");
    click(&mut app, &context, at);
    assert_eq!(
        app.workbench.active(),
        Some(text_tab("README.md").as_str()),
        "a project text file opens in its own tab"
    );
    // Up from the first file reaches the project row, which folds with Left.
    context.memory_mut(|memory| memory.request_focus(egui::Id::new("folder-explorer")));
    frame(&mut app, &context, vec![]);
    for _ in 0..12 {
        frame(&mut app, &context, vec![key(Key::ArrowUp, Modifiers::NONE)]);
    }
    assert_eq!(app.explorer.selected.as_deref(), Some(""));
    frame(
        &mut app,
        &context,
        vec![key(Key::ArrowLeft, Modifiers::NONE)],
    );
    assert_eq!(app.explorer_rows().len(), 1, "Left folds the project");
    frame(
        &mut app,
        &context,
        vec![key(Key::ArrowRight, Modifiers::NONE)],
    );
    assert!(app.explorer_rows().len() > 1);
}

#[test]
fn a_project_text_file_is_edited_saved_exactly_and_recovered() {
    let directory = shop();
    std::fs::write(directory.path().join(".gitignore"), "/.apiforge/local/\n").unwrap();
    let (context, mut app) = open(directory.path());
    watch(&mut app, &context);
    app.open_text(".gitignore");
    wait_storage(&mut app, &context);
    let tab = text_tab(".gitignore");
    frame(&mut app, &context, vec![]);
    assert_eq!(app.workbench.active(), Some(tab.as_str()));

    context.memory_mut(|memory| memory.request_focus(editor_id(&tab)));
    frame(&mut app, &context, vec![]);
    frame(
        &mut app,
        &context,
        vec![egui::Event::Text("target\n".into())],
    );
    let file = app
        .session
        .as_ref()
        .unwrap()
        .text_file(".gitignore")
        .unwrap();
    assert!(file.is_dirty() && file.text().contains("target"));

    // The saved bytes are exactly what the editor holds.
    frame(&mut app, &context, vec![key(Key::S, COMMAND)]);
    wait_storage(&mut app, &context);
    let text = app
        .session
        .as_ref()
        .unwrap()
        .text_file(".gitignore")
        .unwrap()
        .text()
        .to_owned();
    assert_eq!(
        std::fs::read_to_string(directory.path().join(".gitignore")).unwrap(),
        text
    );

    // Unsaved work survives closing the window and comes back in its tab.
    frame(&mut app, &context, vec![egui::Event::Text("more\n".into())]);
    frame(&mut app, &context, vec![]);
    app.changed_at = Some(Instant::now() - RECOVERY_DELAY * 2);
    frame(&mut app, &context, vec![]);
    assert!(
        await_close(&mut app, &context),
        "recovery protects the unsaved text"
    );
    drop(app);
    let (context, mut app) = open(directory.path());
    frame(&mut app, &context, vec![]);
    let file = app
        .session
        .as_ref()
        .unwrap()
        .text_file(".gitignore")
        .unwrap();
    assert!(file.is_dirty() && file.text().contains("more"));
    assert!(
        app.workbench
            .tabs()
            .iter()
            .any(|state| state.document_id == tab),
        "the text file returns as a tab"
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join(".gitignore")).unwrap(),
        text,
        "recovery never saved the file"
    );

    // Closing its tab asks, and discarding returns the saved bytes.
    app.request_close_tab(&tab);
    assert!(matches!(app.prompt, Some(Prompt::CloseTab(_))));
    app.close_tab_with(&tab, false);
    wait_storage(&mut app, &context);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .text_file(".gitignore")
            .is_none()
    );
    assert!(!app.workbench.tabs().iter().any(|s| s.document_id == tab));
}

#[test]
fn a_text_file_has_no_diagram_and_cannot_be_registered_as_one() {
    let directory = shop();
    std::fs::write(directory.path().join("README.md"), "# Shop\n").unwrap();
    let (context, mut app) = open(directory.path());
    watch(&mut app, &context);
    app.open_text("README.md");
    wait_storage(&mut app, &context);
    app.workbench.mode = Mode::Diagram;
    let texts = painted(&mut app, &context, egui::vec2(1600.0, 1000.0));
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.contains("text file, not a scenario")),
        "the diagram says why it is empty: {texts:?}"
    );
    // API Forge's own data is never a text file.
    app.open_text(".apiforge/diagram.json");
    wait_storage(&mut app, &context);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .text_file(".apiforge/diagram.json")
            .is_none()
    );
    assert!(app.notice.as_ref().is_some_and(|notice| notice.error));
}

#[test]
fn a_tabs_close_control_closes_only_its_own_file_and_keeps_asking() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let scenario = document(&app, SCENARIO);
    let component = document(&app, COMPONENT);
    app.workbench.open(&component);
    type_into_code(&mut app, &context, "// note\n");
    settle(&mut app, &context);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&component)
            .unwrap()
            .is_dirty()
    );
    // The close control of the clean tab is inside its own surface.
    let clean = tab_close(&mut app, &context, file_name(SCENARIO));
    click(&mut app, &context, clean);
    assert!(
        !app.workbench
            .tabs()
            .iter()
            .any(|tab| tab.document_id == scenario),
        "the close control closes its tab"
    );
    assert_eq!(app.workbench.active(), Some(component.as_str()));
    // The dirty tab still asks before losing work.
    let dirty = tab_close(&mut app, &context, file_name(COMPONENT));
    click(&mut app, &context, dirty);
    assert!(matches!(app.prompt, Some(Prompt::CloseTab(_))));
    assert!(
        app.workbench
            .tabs()
            .iter()
            .any(|tab| tab.document_id == component)
    );
}

/// The centre of a tab's close control in the window, from the painted cross.
fn tab_close(app: &mut FolderApp, context: &egui::Context, name: &str) -> egui::Pos2 {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
    fn walk<'a>(shape: &'a egui::epaint::Shape, out: &mut Vec<&'a egui::epaint::Shape>) {
        if let egui::epaint::Shape::Vec(shapes) = shape {
            for shape in shapes {
                walk(shape, out);
            }
        } else {
            out.push(shape);
        }
    }
    let mut shapes = Vec::new();
    for shape in &output.shapes {
        walk(&shape.shape, &mut shapes);
    }
    let label = shapes
        .iter()
        .find_map(|shape| match shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text.starts_with(name) => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no tab for {name}"));
    // The nearest cross to the right of the label is this tab's control.
    let points: Vec<egui::Pos2> = shapes
        .iter()
        .filter_map(|shape| match shape {
            egui::epaint::Shape::LineSegment { points, .. } => Some(*points),
            _ => None,
        })
        .flatten()
        // Only the tab strip, and only to the right of this tab's name.
        .filter(|point| point.x > label.right() && point.y < 90.0)
        .collect();
    let left = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let near: Vec<egui::Pos2> = points
        .into_iter()
        .filter(|point| point.x < left + chrome::TAB_CLOSE + 2.0)
        .collect();
    assert!(near.len() >= 4, "a tab needs a visible close icon");
    let x = near.iter().map(|point| point.x).sum::<f32>() / near.len() as f32;
    let y = near.iter().map(|point| point.y).sum::<f32>() / near.len() as f32;
    egui::pos2(x, y)
}

#[test]
fn equal_file_names_are_told_apart_in_the_tab_strip_and_long_ones_are_bounded() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let session = app.session.as_mut().unwrap();
    let nested = session
        .create_document(
            DocumentKind::Scenario,
            "other/promotion-and-cart-price.afs",
            "scenario \"Elsewhere\" {}\n",
        )
        .unwrap();
    let long = session
        .create_document(
            DocumentKind::Scenario,
            "a-very-long-descriptive-scenario-file-name-indeed.afs",
            "scenario \"Long\" {}\n",
        )
        .unwrap();
    app.workbench.open(&nested);
    app.workbench.open(&long);
    let texts = painted(&mut app, &context, egui::vec2(1600.0, 1000.0));
    let tabs: Vec<&String> = texts
        .iter()
        .filter(|(text, _)| text.contains("promotion-and-cart-price"))
        .map(|(text, _)| text)
        .collect();
    assert!(
        tabs.iter().any(|text| text.contains("other/")),
        "equal names need their folder: {tabs:?}"
    );
    let long_tab = texts
        .iter()
        .find(|(text, rect)| text.starts_with("a-very-long") && rect.center().y < 90.0)
        .expect("the long name has a tab");
    assert!(
        long_tab.1.width() <= chrome::TAB_LABEL_WIDTH + 1.0,
        "a long tab label is bounded: {} points",
        long_tab.1.width()
    );
}

#[test]
fn ctrl_tab_brings_the_active_tab_back_into_view() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let session = app.session.as_mut().unwrap();
    let mut last = String::new();
    for index in 0..16 {
        last = session
            .create_document(
                DocumentKind::Scenario,
                &format!("task-{index:02}.afs"),
                &format!("scenario \"Task {index}\" {{}}\n"),
            )
            .unwrap();
        app.workbench.open(&last);
    }
    // The first tab is active again, far to the left of the strip.
    let first = app.workbench.tabs()[0].document_id.clone();
    app.workbench.open(&first);
    let small = egui::vec2(960.0, 540.0);
    let texts = painted(&mut app, &context, small);
    assert!(
        texts
            .iter()
            .any(|(text, rect)| text.starts_with(file_name(SCENARIO)) && rect.right() < 960.0),
        "the active tab must be visible: {texts:?}"
    );
    // Cycling forward to the last tab reveals it too.
    for _ in 0..16 {
        frame(&mut app, &context, vec![key(Key::Tab, COMMAND)]);
    }
    assert_eq!(app.workbench.active(), Some(last.as_str()));
    let texts = painted(&mut app, &context, small);
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.starts_with("task-15.afs")),
        "the tab reached by Ctrl+Tab must be visible: {texts:?}"
    );
}

#[test]
fn publishing_from_the_header_requires_a_comment_decision_and_cancel_keeps_source() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, COMPONENT);
    app.workbench.open(&id);
    let original = format!("// loose comment\n\n{}", text(&app, &id));
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &original, CODE)
        .unwrap();
    app.session.as_mut().unwrap().settle(&id).unwrap();
    let revisions = app.session.as_ref().unwrap().manifest().revisions.len();
    let at = label_position(&mut app, &context, "Publish");
    click(&mut app, &context, at);
    assert!(matches!(app.prompt, Some(Prompt::CommentLoss { .. })));
    assert_eq!(text(&app, &id), original);
    let at = label_position(&mut app, &context, "Cancel");
    click(&mut app, &context, at);
    assert!(app.prompt.is_none());
    assert_eq!(
        app.session.as_ref().unwrap().manifest().revisions.len(),
        revisions
    );
    let at = label_position(&mut app, &context, "Publish");
    click(&mut app, &context, at);
    let at = label_position(&mut app, &context, "Continue");
    click(&mut app, &context, at);
    assert_eq!(
        app.session.as_ref().unwrap().manifest().revisions.len(),
        revisions + 1
    );
    assert_eq!(
        text(&app, &id),
        original,
        "publication preserves working formatting and comments"
    );
}

/// KTLO-22 criterion 2: once a second the watcher queues an exclusive storage
/// job that changes nothing. A *Continue* pressed while that job is still on
/// the worker publishes once the job completes, instead of being forgotten and
/// asking again.
#[test]
fn continue_in_the_comment_prompt_publishes_while_an_exclusive_check_is_in_flight() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, COMPONENT);
    app.workbench.open(&id);
    let original = format!("// loose comment\n\n{}", text(&app, &id));
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &original, CODE)
        .unwrap();
    app.session.as_mut().unwrap().settle(&id).unwrap();
    let revisions = app.session.as_ref().unwrap().manifest().revisions.len();
    let at = label_position(&mut app, &context, "Publish");
    click(&mut app, &context, at);
    assert!(matches!(app.prompt, Some(Prompt::CommentLoss { .. })));

    // The watcher's job, held on the worker until the decision is taken.
    let (release, held) = std::sync::mpsc::channel::<()>();
    app.storage_project(
        "Checking external changes…",
        apiforge_project::StorageMode::Exclusive,
        false,
        move |_| {
            let _ = held.recv();
            Ok(())
        },
        |_, _: apiforge_project::Result<()>| {},
    );
    frame(&mut app, &context, vec![]);
    assert!(app.storage.owns_session(), "the check is on the worker");

    // `click` waits for storage, which is held, so the press is sent directly.
    let at = label_position(&mut app, &context, "Continue");
    frame(&mut app, &context, vec![egui::Event::PointerMoved(at)]);
    for pressed in [true, false] {
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    assert!(
        app.storage.owns_session(),
        "the decision was taken while the check was in flight"
    );
    release.send(()).unwrap();
    wait_storage(&mut app, &context);

    assert!(
        !matches!(app.prompt, Some(Prompt::CommentLoss { .. })),
        "the prompt does not come back"
    );
    assert_eq!(
        app.session.as_ref().unwrap().manifest().revisions.len(),
        revisions + 1
    );
    assert!(
        app.notice
            .as_ref()
            .is_some_and(|notice| !notice.error && notice.text.starts_with("Published revision")),
        "notice: {:?}",
        app.notice.as_ref().map(|notice| &notice.text)
    );
    assert_eq!(text(&app, &id), original);
}

#[test]
fn a_stale_comment_dialog_cannot_authorise_an_edit_of_newer_source() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, COMPONENT);
    let original = format!("// loose\n\n{}", text(&app, &id));
    let session = app.session.as_mut().unwrap();
    session.edit_source(&id, &original, CODE).unwrap();
    session.settle(&id).unwrap();
    let ticket = session.analysis_for(&id).unwrap().ticket;
    session
        .edit_source(&id, &format!("{original}\n"), CODE)
        .unwrap();
    session.settle(&id).unwrap();
    let revisions = session.manifest().revisions.len();
    app.confirm_comment_loss(&ticket, publication::Action::Publish);
    assert_eq!(
        app.session.as_ref().unwrap().manifest().revisions.len(),
        revisions
    );
    frame(&mut app, &context, vec![]);
    assert!(app.notice.as_ref().unwrap().text.contains("changed"));
}

#[test]
fn applying_a_pin_fix_from_problems_can_be_undone_in_the_code_editor() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    let original = text(&app, &id);
    let unpinned = original.replace("@1(", "(");
    assert_ne!(unpinned, original);
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &unpinned, CODE)
        .unwrap();
    app.session.as_mut().unwrap().settle(&id).unwrap();
    app.workbench.open(&id);
    frame(&mut app, &context, vec![]);
    let at = label_position(&mut app, &context, "Apply fix");
    click(&mut app, &context, at);
    assert_ne!(text(&app, &id), unpinned);
    frame(&mut app, &context, vec![]);
    click(&mut app, &context, egui::pos2(500.0, 160.0));
    frame(&mut app, &context, vec![key(Key::Z, COMMAND)]);
    assert_eq!(
        text(&app, &id),
        unpinned,
        "the source fix belongs to text undo"
    );
}

fn label_position(app: &mut FolderApp, context: &egui::Context, label: &str) -> egui::Pos2 {
    // A newly opened modal first measures itself in an invisible sizing pass.
    for _ in 0..3 {
        if let Some((_, rect)) = painted(app, context, egui::vec2(1600.0, 1000.0))
            .into_iter()
            .find(|(text, _)| text == label)
        {
            return rect.center();
        }
    }
    panic!("no visible label {label}")
}

fn run_project() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Runs").unwrap();
    let id = session
        .create_document(
            DocumentKind::Scenario,
            "main.afs",
            "scenario \"Main\" {\n  answer = 1\n  equal = assert answer == 2\n}\n",
        )
        .unwrap();
    drop(session);
    (directory, id)
}

fn finish_folder_run(app: &mut FolderApp) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.runs.handle.is_some() && Instant::now() < deadline {
        app.poll_run();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.runs.handle.is_none(), "the run must finish");
}

#[test]
fn folder_run_settles_unsaved_code_and_reports_in_every_mode_then_becomes_stale() {
    let (directory, id) = run_project();
    let (context, mut app) = open(directory.path());
    let edited = text(&app, &id).replacen("answer = 1", "answer = 2", 1);
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited, CODE)
        .unwrap();
    let at = label_position(&mut app, &context, "Run");
    click(&mut app, &context, at);
    finish_folder_run(&mut app);
    assert_eq!(
        app.run_result(&id).unwrap().status,
        apiforge_core::run::RunStatus::Passed
    );
    assert!(
        std::fs::read_to_string(directory.path().join("main.afs"))
            .unwrap()
            .contains("answer = 1")
    );
    assert!(app.fresh_run(&id).is_some());
    for mode in [Mode::Code, Mode::Diagram, Mode::Split] {
        app.workbench.mode = mode;
        let shown = painted(&mut app, &context, egui::vec2(960.0, 540.0));
        assert!(
            shown.iter().any(|(text, _)| text.contains("Passed")),
            "report is visible in {mode:?}"
        );
    }
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited.replace("answer = 2", "answer = 3"), CODE)
        .unwrap();
    assert!(
        app.fresh_run(&id).is_none(),
        "editing invalidates report use and output completion"
    );
    assert!(
        app.run_result(&id).is_some(),
        "the stale report remains available"
    );
}

#[test]
fn folder_last_valid_dialog_is_explicit_and_rejects_a_later_source_version() {
    let (directory, id) = run_project();
    let (context, mut app) = open(directory.path());
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, "invalid", CODE)
        .unwrap();
    app.request_run(&id, false);
    assert!(matches!(app.prompt, Some(Prompt::Run { .. })));
    assert!(app.runs.handle.is_none());
    let at = label_position(&mut app, &context, "Cancel");
    click(&mut app, &context, at);
    assert!(app.runs.handle.is_none());
    app.request_run(&id, false);
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, "another invalid source", CODE)
        .unwrap();
    let at = label_position(&mut app, &context, "Run this version");
    click(&mut app, &context, at);
    assert!(app.runs.handle.is_none());
    app.request_run(&id, false);
    let at = label_position(&mut app, &context, "Run this version");
    click(&mut app, &context, at);
    finish_folder_run(&mut app);
    assert!(app.run_result(&id).is_some());
    assert!(
        app.fresh_run(&id).is_none(),
        "an earlier-version report is labelled stale"
    );
}

#[test]
fn close_export_keeps_read_only_and_failed_recovery_work_until_export_succeeds() {
    let (directory, id) = run_project();
    let (_writer_context, _writer) = open(directory.path());
    let (context, mut reader) = open(directory.path());
    assert!(reader.session.as_ref().unwrap().is_read_only());
    let edited = format!("{}\n// unsaved", text(&reader, &id));
    reader
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited, CODE)
        .unwrap();
    assert!(!await_close(&mut reader, &context));
    reader.abort_close("Read-only recovery".into());
    let _ = painted(&mut reader, &context, egui::vec2(960.0, 540.0));
    let labels = painted(&mut reader, &context, egui::vec2(960.0, 540.0));
    assert!(labels.iter().any(|(text, _)| text == "Export and close"));
    let contents = reader.session.as_ref().unwrap().export_unsaved().unwrap();
    reader.complete_close_export(Some(Err("disk full".into())), &contents);
    assert!(reader.session.is_some());
    reader.complete_close_export(None, &contents);
    assert!(reader.session.is_some());
    reader
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &format!("{edited}\n"), CODE)
        .unwrap();
    reader.complete_close_export(Some(Ok(directory.path().join("export.json"))), &contents);
    assert!(
        reader.session.is_some(),
        "an older export cannot authorise closing newer work"
    );
    let latest = reader.session.as_ref().unwrap().export_unsaved().unwrap();
    let output = directory.path().join("export.json");
    apiforge_storage::write_atomic(&output, &latest).unwrap();
    reader.complete_close_export(Some(Ok(output)), &latest);
    wait_storage(&mut reader, &context);
    assert!(
        reader.session.is_none(),
        "{}; {:?}; busy={}",
        reader
            .notice
            .as_ref()
            .map_or("", |notice| notice.text.as_str()),
        reader.close_error,
        reader.storage.is_busy()
    );
    assert!(reader.persistence.close_ready);
}

#[test]
fn project_and_legacy_interrupted_runs_are_reviewed_under_their_own_locks() {
    use apiforge_core::journal::{JournalRun, JournalSink};
    let (directory, id) = run_project();
    let owner = ProjectSession::open(directory.path()).unwrap();
    let journal = apiforge_storage::JournalStore::open_project(owner.store().clone()).unwrap();
    journal
        .store
        .begin_run(&JournalRun::new(
            "interrupted",
            "main",
            "Main",
            "2026-10-24T00:00:00Z",
        ))
        .unwrap();
    drop(journal);
    drop(owner);
    let (context, mut app) = open(directory.path());
    assert!(matches!(app.prompt, Some(Prompt::Journal(_))));
    let at = label_position(&mut app, &context, "Remind me later");
    click(&mut app, &context, at);
    assert!(
        directory
            .path()
            .join(".apiforge/local/journal.json")
            .exists()
    );
    assert_eq!(
        app.session
            .as_ref()
            .unwrap()
            .document(&id)
            .unwrap()
            .entry()
            .path
            .as_str(),
        "main.afs"
    );
    assert!(await_close(&mut app, &context));
    let (context, mut app) = open(directory.path());
    let at = label_position(&mut app, &context, "Mark as handled");
    click(&mut app, &context, at);
    assert!(
        !directory
            .path()
            .join(".apiforge/local/journal.json")
            .exists()
    );

    let legacy_data = tempfile::tempdir().unwrap();
    let legacy =
        apiforge_storage::WorkspaceStore::open_exclusive(legacy_data.path().join("workspace.json"))
            .unwrap();
    legacy
        .save(
            &apiforge_core::import_document(
                include_bytes!("../../../../fixtures/workspace.json"),
                &apiforge_core::ImportLimits::default(),
            )
            .unwrap()
            .into_workspace(),
        )
        .unwrap();
    let journal = apiforge_storage::JournalStore::open(legacy.clone()).unwrap();
    journal
        .store
        .begin_run(&JournalRun::new(
            "legacy",
            "main",
            "Unassigned",
            "2026-10-24T00:00:00Z",
        ))
        .unwrap();
    let _ = apiforge_project::migrate_workspace(&legacy).unwrap();
    drop(journal);
    drop(legacy);
    app.review_legacy_runs_from(legacy_data.path());
    wait_storage(&mut app, &context);
    assert!(matches!(app.prompt, Some(Prompt::Journal(_))));
    let _ = painted(&mut app, &context, egui::vec2(960.0, 540.0));
    let labels = painted(&mut app, &context, egui::vec2(960.0, 540.0));
    assert!(
        labels
            .iter()
            .any(|(text, _)| text == "Unresolved legacy runs")
    );
    let at = label_position(&mut app, &context, "Mark as handled");
    click(&mut app, &context, at);
    assert!(!legacy_data.path().join("journal.json").exists());
    assert!(
        !directory
            .path()
            .join(".apiforge/local/journal.json")
            .exists()
    );
}

#[test]
fn folder_close_cancels_the_run_and_retains_its_project_lock_until_cleanup_finishes() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (entered, waiting) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let _ = stream.read(&mut [0_u8; 4096]);
        let _ = entered.send(());
        let _ = released.recv_timeout(Duration::from_secs(3));
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    });
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Cleanup").unwrap();
    session
        .edit_configuration(
            |manifest| manifest.environment.base_url = format!("http://{address}"),
            Origin::SESSION,
        )
        .unwrap();
    let id = session.create_document(DocumentKind::Scenario, "main.afs", "scenario \"Main\" {\n  response = GET \"/slow\"\n  cleanup {\n    cleanup_value = 1\n  }\n}\n").unwrap();
    drop(session);
    let (context, mut app) = open(directory.path());
    app.request_run(&id, true);
    waiting.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(
        !app.prepare_close(),
        "close is nonblocking while cancellation and cleanup run"
    );
    assert!(
        apiforge_storage::project::ProjectStore::open_read_only(directory.path())
            .unwrap()
            .has_live_writer()
            .unwrap()
    );
    let _ = release.send(());
    finish_folder_run(&mut app);
    assert!(await_close(&mut app, &context));
    assert!(
        !apiforge_storage::project::ProjectStore::open_read_only(directory.path())
            .unwrap()
            .has_live_writer()
            .unwrap()
    );
    server.join().unwrap();
}

#[test]
fn exporting_after_failed_recovery_keeps_the_foreign_recovery_file_intact() {
    let (directory, id) = run_project();
    let (context, mut app) = open(directory.path());
    let edited = format!("{}\n// latest unsaved work", text(&app, &id));
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited, CODE)
        .unwrap();
    std::fs::write(
        directory.path().join(".apiforge/local/recovery.json"),
        b"foreign recovery",
    )
    .unwrap();
    assert!(!await_close(&mut app, &context));
    settle(&mut app, &context);
    wait_storage(&mut app, &context);
    let contents = app.session.as_ref().unwrap().export_unsaved().unwrap();
    let output = directory.path().join("export.json");
    apiforge_storage::write_atomic(&output, &contents).unwrap();
    app.complete_close_export(Some(Ok(output.clone())), &contents);
    wait_storage(&mut app, &context);
    assert!(
        app.session.is_none(),
        "{}; {:?}; busy={}",
        app.notice
            .as_ref()
            .map_or("", |notice| notice.text.as_str()),
        app.close_error,
        app.storage.is_busy()
    );
    let export: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(export["recovery"]["documents"][0]["text"], edited);
    assert_eq!(
        std::fs::read(directory.path().join(".apiforge/local/recovery.json")).unwrap(),
        b"foreign recovery"
    );
}

fn named_folder(name: &str) -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), name).unwrap();
    let id = session
        .create_document(
            DocumentKind::Scenario,
            "main.afs",
            "scenario \"Main\" {\n  answer = 1\n}\n",
        )
        .unwrap();
    drop(session);
    (directory, id)
}

#[test]
fn recent_project_switching_writes_recovery_and_returns_exact_unsaved_work() {
    let (alpha, id) = named_folder("Navigation Alpha");
    let (beta, _) = named_folder("Navigation Beta");
    let (context, mut app) = open(alpha.path());
    let edited = format!("{}\nunfinished", text(&app, &id));
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited, CODE)
        .unwrap();
    let field = FieldRecovery {
        widget: 9,
        base_hash: "0".repeat(64),
        text: "unfinished field".into(),
        selection: TextSelection {
            anchor: 2,
            caret: 1,
        },
    };
    app.session
        .as_mut()
        .unwrap()
        .set_field_drafts(&id, vec![field.clone()])
        .unwrap();
    app.request_switch(beta.path().into());
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    assert_eq!(
        app.session.as_ref().unwrap().manifest().name,
        "Navigation Beta"
    );
    assert!(
        std::fs::read_to_string(alpha.path().join("main.afs"))
            .unwrap()
            .contains("answer = 1")
    );
    assert!(alpha.path().join(".apiforge/local/recovery.json").is_file());
    let at = label_position(&mut app, &context, "Project");
    click(&mut app, &context, at);
    let at = label_position(&mut app, &context, "Navigation Alpha");
    click(&mut app, &context, at);
    frame(&mut app, &context, vec![]);
    assert_eq!(text(&app, &id), edited);
    assert_eq!(
        app.session
            .as_ref()
            .unwrap()
            .document(&id)
            .unwrap()
            .field_drafts(),
        &[field]
    );
    assert_eq!(app.navigation.recent.active.as_deref(), Some(alpha.path()));
    assert_eq!(app.navigation.recent.entries[0].name, "Navigation Alpha");
}

#[test]
fn failed_switch_recovery_preserves_the_current_project_and_recent_active_path() {
    let (alpha, id) = named_folder("Navigation failure Alpha");
    let (beta, _) = named_folder("Navigation failure Beta");
    let (context, mut app) = open(alpha.path());
    let edited = format!("{}\nunfinished", text(&app, &id));
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited, CODE)
        .unwrap();
    std::fs::write(
        alpha.path().join(".apiforge/local/recovery.json"),
        b"foreign recovery",
    )
    .unwrap();
    app.request_switch(beta.path().into());
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    assert_eq!(app.root, alpha.path());
    assert_eq!(text(&app, &id), edited);
    assert_eq!(app.navigation.recent.active.as_deref(), Some(alpha.path()));
    assert!(matches!(app.prompt, Some(Prompt::CloseFailed(_))));
    let at = label_position(&mut app, &context, "Keep editing");
    click(&mut app, &context, at);
    assert!(!app.switching());
    assert_eq!(
        std::fs::read(alpha.path().join(".apiforge/local/recovery.json")).unwrap(),
        b"foreign recovery"
    );
}

#[test]
fn opening_an_uninitialised_folder_writes_no_project_and_bad_targets_keep_the_current_one() {
    let (alpha, id) = named_folder("Navigation source");
    let target = tempfile::tempdir().unwrap();
    let (context, mut app) = open(alpha.path());
    app.request_switch(target.path().join("missing"));
    wait_storage(&mut app, &context);
    assert_eq!(app.root, alpha.path());
    assert!(app.session.as_ref().unwrap().document(&id).is_some());
    app.request_switch(target.path().into());
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    assert!(app.uninitialised);
    assert!(app.session.is_none());
    assert!(!target.path().join("apiforge.project.json").exists());
    assert!(!target.path().join(".apiforge").exists());
}

#[test]
fn exporting_a_read_only_project_allows_the_pending_folder_switch() {
    let (alpha, id) = named_folder("Navigation read-only Alpha");
    let (beta, _) = named_folder("Navigation read-only Beta");
    let (_writer_context, _writer) = open(alpha.path());
    let (context, mut app) = open(alpha.path());
    let edited = format!("{}\nunfinished", text(&app, &id));
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited, CODE)
        .unwrap();
    app.request_switch(beta.path().into());
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    assert!(app.switching());
    let _ = label_position(&mut app, &context, "Export and open folder");
    let contents = app.session.as_ref().unwrap().export_unsaved().unwrap();
    let path = alpha.path().join("export.json");
    apiforge_storage::write_atomic(&path, &contents).unwrap();
    app.complete_close_export(Some(Ok(path)), &contents);
    wait_storage(&mut app, &context);
    assert_eq!(
        app.session.as_ref().unwrap().manifest().name,
        "Navigation read-only Beta"
    );
    assert!(!app.close_requested);
    assert!(!app.switching());
}

#[test]
fn empty_project_settings_apply_without_saving_and_save_all_persists_only_the_manifest() {
    let directory = tempfile::tempdir().unwrap();
    drop(ProjectSession::create(directory.path(), "Empty settings").unwrap());
    let (context, mut app) = open(directory.path());
    let at = label_position(&mut app, &context, "Project");
    click(&mut app, &context, at);
    let at = label_position(&mut app, &context, "Project settings…");
    click(&mut app, &context, at);
    frame(&mut app, &context, vec![]);
    frame(
        &mut app,
        &context,
        vec![
            key(Key::A, COMMAND),
            egui::Event::Text("Renamed empty settings".into()),
        ],
    );
    let at = label_position(&mut app, &context, "Apply settings");
    click(&mut app, &context, at);
    assert!(app.prompt.is_none());
    let session = app.session.as_ref().unwrap();
    assert_eq!(session.manifest().name, "Renamed empty settings");
    assert!(session.is_manifest_dirty());
    assert!(session.documents().is_empty());
    let saved = std::fs::read(directory.path().join("apiforge.project.json")).unwrap();
    assert_eq!(
        apiforge_project::ProjectManifest::from_bytes(&saved)
            .unwrap()
            .name,
        "Empty settings"
    );
    let at = label_position(&mut app, &context, "Save all");
    click(&mut app, &context, at);
    let saved = std::fs::read(directory.path().join("apiforge.project.json")).unwrap();
    assert_eq!(
        apiforge_project::ProjectManifest::from_bytes(&saved)
            .unwrap()
            .name,
        "Renamed empty settings"
    );
    assert!(!app.session.as_ref().unwrap().is_manifest_dirty());
}

#[test]
fn environment_public_changes_save_separately_and_secret_values_do_not_survive_restart() {
    let (directory, id) = named_folder("Environment settings");
    let (context, mut app) = open(directory.path());
    let original = text(&app, &id);
    let mut environment = app.session.as_ref().unwrap().manifest().environment.clone();
    environment.base_url = "https://api.example.test".into();
    environment.secret_names.push("token".into());
    let secrets = [("token".into(), "secret-only-in-memory".into())]
        .into_iter()
        .collect();
    app.apply_environment(environment.clone(), secrets).unwrap();
    wait_storage(&mut app, &context);
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &format!("{original}\n// unfinished"), CODE)
        .unwrap();
    app.save_configuration();
    wait_storage(&mut app, &context);
    assert_eq!(
        std::fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        original
    );
    let saved = std::fs::read_to_string(directory.path().join("apiforge.project.json")).unwrap();
    assert!(!saved.contains("secret-only-in-memory"));
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&id)
            .unwrap()
            .is_dirty()
    );
    let prepared = app.session.as_mut().unwrap().prepare_run(&id).unwrap();
    app.apply_environment(
        environment,
        [("token".into(), "changed-session-value".into())]
            .into_iter()
            .collect(),
    )
    .unwrap();
    wait_storage(&mut app, &context);
    assert!(!app.session.as_ref().unwrap().is_manifest_dirty());
    assert!(
        app.session
            .as_mut()
            .unwrap()
            .validate_run(&prepared)
            .is_err()
    );
    assert!(await_close(&mut app, &context));
    let (_context, reopened) = open(directory.path());
    assert!(reopened.runs.secrets.is_empty());
    assert_eq!(
        reopened
            .session
            .as_ref()
            .unwrap()
            .manifest()
            .environment
            .secret_names,
        ["token"]
    );
}

#[test]
fn read_only_environment_accepts_session_values_without_writing_public_settings() {
    let (directory, id) = named_folder("Read-only settings");
    let mut writer = ProjectSession::open(directory.path()).unwrap();
    writer
        .edit_configuration(
            |manifest| manifest.environment.secret_names.push("token".into()),
            DIAGRAM,
        )
        .unwrap();
    writer.save_configuration().unwrap();
    let before = std::fs::read(directory.path().join("apiforge.project.json")).unwrap();
    let (context, mut app) = open(directory.path());
    assert!(app.session.as_ref().unwrap().is_read_only());
    let prepared = app.session.as_mut().unwrap().prepare_run(&id).unwrap();
    let environment = app.session.as_ref().unwrap().manifest().environment.clone();
    app.apply_environment(
        environment,
        [("token".into(), "read-only-session-value".into())]
            .into_iter()
            .collect(),
    )
    .unwrap();
    wait_storage(&mut app, &context);
    assert!(!app.session.as_ref().unwrap().is_manifest_dirty());
    assert!(
        app.session
            .as_mut()
            .unwrap()
            .validate_run(&prepared)
            .is_err()
    );
    assert_eq!(
        std::fs::read(directory.path().join("apiforge.project.json")).unwrap(),
        before
    );
    assert_eq!(app.runs.secrets["token"], "read-only-session-value");
}

/// A valid scenario of `steps` requests with `rows` sample rows each: two
/// blocks per step, so it stays under the block limit at any size.
fn large_scenario(steps: usize, rows: usize) -> String {
    let mut text = "scenario \"Large\" {\n  begin = start\n".to_owned();
    for index in 0..steps {
        let after = if index == 0 {
            "begin".to_owned()
        } else {
            format!("ok_{}", index - 1)
        };
        text.push_str(&format!(
            "  @after({after})\n  item_{index} = POST \"/items/{index}\" {{\n    body {{\n      \
             name: \"item {index}\",\n      price: 12.5\n    }}\n    sample 200 {{\n      rows: [\n"
        ));
        for row in 0..rows {
            let comma = if row + 1 < rows { "," } else { "" };
            text.push_str(&format!(
                "        {{ id: \"r-{index}-{row}\", price: {row}.5 }}{comma}\n"
            ));
        }
        text.push_str(&format!(
            "      ]\n    }}\n  }}\n  ok_{index} = assert item_{index}.status == 200\n"
        ));
    }
    text.push_str("}\n");
    text
}

/// A project whose only document is `text`, opened in the code view.
fn open_source(text: &str) -> (tempfile::TempDir, egui::Context, FolderApp, String) {
    let directory = tempfile::tempdir().unwrap();
    ProjectSession::create(directory.path(), "Large")
        .unwrap()
        .create_document(DocumentKind::Scenario, "large.afs", text)
        .unwrap();
    let (context, app) = open(directory.path());
    let id = app.workbench.active().unwrap().to_owned();
    (directory, context, app, id)
}

fn node_id(app: &FolderApp, document_id: &str, handle: &str) -> String {
    let model = model(app, document_id);
    graph_of(&model, false)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap()
        .id
        .clone()
}

/// Puts the caret into the editor at `at` and draws a few frames.
fn caret_at(app: &mut FolderApp, context: &egui::Context, document_id: &str, at: usize) {
    context.memory_mut(|memory| memory.request_focus(editor_id(document_id)));
    app.code.select(at, at);
    for _ in 0..3 {
        frame(app, context, vec![]);
    }
}

#[test]
fn a_large_source_follows_the_caret_once_its_edit_is_analysed() {
    // Finding the statement under the caret parses the whole source. A large
    // source being typed into keeps its board selection until analysis settles.
    let text = large_scenario(12, 700);
    assert!(text.len() > crate::code::MAX_COLOURED_BYTES);
    let (_directory, context, mut app, id) = open_source(&text);
    let inside = |handle: &str| text.find(&format!("  {handle} = assert")).unwrap() + 4;
    caret_at(&mut app, &context, &id, inside("ok_3"));
    assert_eq!(app.selections[&id], vec![node_id(&app, &id, "ok_3")]);

    let edited = text.replacen("price: 12.5", "price: 12.75", 1);
    let session = app.session.as_mut().unwrap();
    session.edit_source(&id, &edited, Origin(1)).unwrap();
    // Typing has not paused yet, so the edit stays pending for however long the
    // frames below take; a wall-clock race with the analysis worker lost this
    // under load.
    app.analyser
        .typed(&id, Instant::now() + Duration::from_secs(3600));
    caret_at(&mut app, &context, &id, inside("ok_5") + 1);
    let analysis = |app: &FolderApp| {
        let session = app.session.as_ref().unwrap();
        session.document(&id).unwrap().analysis()
    };
    assert_eq!(
        analysis(&app),
        AnalysisState::Pending,
        "the edit is pending"
    );
    assert_eq!(
        app.selections[&id],
        vec![node_id(&app, &id, "ok_3")],
        "not searched while the edit is pending"
    );

    // Typing pauses: the analysis starts on the next frame and is waited for.
    // A frame does not start it while storage work owns the session.
    wait_storage(&mut app, &context);
    app.analyser.typed(&id, Instant::now() - APPLY_DELAY);
    frame(&mut app, &context, vec![]);
    let session = app.session.as_mut().unwrap();
    app.analyser.finish(session);
    assert_eq!(analysis(&app), AnalysisState::Valid, "the edit is analysed");
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![]);
    assert_eq!(app.selections[&id], vec![node_id(&app, &id, "ok_5")]);
}

#[test]
fn completion_names_follow_a_newly_analysed_model() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    let handles = |app: &FolderApp| -> Vec<String> {
        let names = app.completion_names.as_ref().expect("drawn");
        names
            .names
            .handles
            .iter()
            .map(|handle| handle.name.clone())
            .collect()
    };
    frame(&mut app, &context, vec![]);
    assert!(!handles(&app).contains(&"extra_total".to_owned()));

    let source = text(&app, &id);
    let end = source.rfind('}').unwrap();
    let edited = format!("{}  extra_total = 1\n{}", &source[..end], &source[end..]);
    let session = app.session.as_mut().unwrap();
    session.edit_source(&id, &edited, Origin(1)).unwrap();
    settle(&mut app, &context);
    frame(&mut app, &context, vec![]);
    assert!(handles(&app).contains(&"extra_total".to_owned()));
}

/// Frame budgets for the code view at the sizes the plan names: a 1000-line
/// source and one at the 1 MiB limit, idle and while typing. Timing needs a
/// release build:
///
/// ```text
/// cargo test -p apiforge-desktop --release --locked frame_budgets -- --ignored --nocapture
/// ```
#[test]
#[ignore = "timing; run in release"]
fn frame_budgets() {
    let budget = |milliseconds: u64| Duration::from_millis(milliseconds);
    let mut over = Vec::new();
    for (name, text, budgets) in [
        (
            "1000 lines",
            large_scenario(71, 2),
            [(Mode::Code, 16, 16), (Mode::Split, 16, 16)],
        ),
        (
            "1 MiB",
            large_scenario(90, 260),
            [(Mode::Code, 16, 33), (Mode::Split, 16, 50)],
        ),
    ] {
        let (_directory, context, mut app, id) = open_source(&text);
        assert!(
            app.session
                .as_ref()
                .unwrap()
                .document(&id)
                .unwrap()
                .model()
                .is_some()
        );
        for (mode, idle_budget, typing_budget) in budgets {
            app.workbench.mode = mode;
            // On an assertion mid-source: the inspector then shows a small
            // block, so the frame measures the source rather than one sample.
            let middle = text[text.len() / 2..].find(" = assert").unwrap() + text.len() / 2;
            caret_at(&mut app, &context, &id, middle);
            let start = Instant::now();
            for _ in 0..10 {
                frame(&mut app, &context, vec![]);
            }
            let idle = start.elapsed() / 10;
            let start = Instant::now();
            for _ in 0..10 {
                frame(&mut app, &context, vec![egui::Event::Text("a".into())]);
            }
            let typing = start.elapsed() / 10;
            println!(
                "{name} ({} lines, {} bytes) {mode:?}: idle {idle:.2?} / {idle_budget} ms, \
                 typing {typing:.2?} / {typing_budget} ms",
                text.lines().count(),
                text.len()
            );
            if idle > budget(idle_budget) || typing > budget(typing_budget) {
                over.push(format!("{name} {mode:?}"));
            }
            // Undo the typing so each mode starts from the same source.
            let session = app.session.as_mut().unwrap();
            session.edit_source(&id, &text, Origin(1)).unwrap();
            settle(&mut app, &context);
        }
    }
    assert!(over.is_empty(), "over budget: {over:?}");
}
