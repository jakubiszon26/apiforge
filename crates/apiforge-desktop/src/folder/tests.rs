use super::*;
use crate::buffers::APPLY_DELAY;

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
    frame(&mut app, &context, vec![]);
    (context, app)
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
    assert!(app.prepare_close(), "unsaved work is in local recovery");
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
    assert!(reader.notice.as_ref().is_some_and(|notice| notice.error));
    assert_eq!(
        std::fs::read_to_string(directory.path().join(SCENARIO)).unwrap(),
        original
    );
    assert!(!reader.prepare_close());
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
    }
    app.create_document(DocumentKind::Component, None, "3d secure check")
        .unwrap();
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
    let closed = app.prepare_close();
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
    assert!(app.prepare_close());
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
    assert!(!app.prepare_close());
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

/// Waits for the folder check the watcher started and applies it.
fn watch(app: &mut FolderApp, context: &egui::Context) {
    app.watcher.rescan();
    frame(app, context, vec![]);
    let session = app.session.as_mut().unwrap();
    if let Some(outcome) = app.watcher.finish(session) {
        app.follow_external(outcome);
    }
    frame(app, context, vec![]);
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
    assert!(app.relocate("flows/renamed.afs", "taken.afs").is_err());
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
    let Some(Prompt::Delete { preview, .. }) = app.prompt.take() else {
        panic!("a delete confirmation");
    };
    assert_eq!(preview.unsaved_documents(), std::slice::from_ref(&id));
    app.delete_confirmed(&preview).unwrap();
    frame(&mut app, &context, vec![]);
    assert!(app.workbench.tabs().iter().all(|tab| tab.document_id != id));
    assert!(!directory.path().join("scenarios").exists());

    app.open_trash();
    let Some(Prompt::Trash { operations, .. }) = app.prompt.take() else {
        panic!("the trash");
    };
    let operation = operations[0].operation_id.clone();
    app.restore(&operation, &operations).unwrap();
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
    assert!(app.prepare_close(), "recovery protects the unsaved text");
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
    painted(app, context, egui::vec2(1600.0, 1000.0))
        .into_iter()
        .find(|(text, _)| text == label)
        .map(|(_, rect)| rect.center())
        .unwrap_or_else(|| panic!("no visible label {label}"))
}
