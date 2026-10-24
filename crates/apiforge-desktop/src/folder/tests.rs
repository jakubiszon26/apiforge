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
        app.create_document(DocumentKind::Scenario, "Checkout \"fast\" path")
            .unwrap();
    }
    app.create_document(DocumentKind::Component, "3d secure check")
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
