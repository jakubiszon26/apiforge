use super::*;
use apiforge_core::block::Block;
use apiforge_core::graph::VALUE_INPUT;
use serde_json::json;
use std::time::Duration;

fn frame(app: &mut App, ctx: &egui::Context, size: egui::Vec2, events: Vec<egui::Event>) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
}

fn apply(app: &mut App, text: &str) {
    app.buffers
        .typed(&app.code_context(), text.into(), Instant::now());
    app.settle_code();
    assert!(!app.document_locked());
}

fn value(app: &App, cleanup: bool) -> Binding {
    let scenario = app.editor.scenario();
    let graph = if cleanup {
        &scenario.cleanup
    } else {
        &scenario.graph
    };
    graph.nodes[0].inputs[VALUE_INPUT].clone()
}

#[test]
fn first_paste_survives_intervening_frames_and_is_saved_before_debounce() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    app.code_open = true;
    let size = egui::vec2(1600.0, 1000.0);
    frame(&mut app, &ctx, size, vec![]);
    let id = app.code_editor_id();
    ctx.memory_mut(|m| m.request_focus(id));
    frame(
        &mut app,
        &ctx,
        size,
        vec![
            egui::Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            },
            egui::Event::Paste("scenario \"Pasted\" { answer = 42 }".into()),
        ],
    );
    let key = app.code_context();
    assert_eq!(app.buffers.drafts().count(), 1);
    for _ in 0..3 {
        frame(&mut app, &ctx, size, vec![]);
    }
    assert!(app.buffers.get(&key).unwrap().text.contains("answer = 42"));
    app.settle_code();
    assert_eq!(app.editor.graph().nodes[0].handle, "answer");
    assert_eq!(value(&app, false), Binding::literal(json!(42)));
}

#[test]
fn run_flushes_pending_code_instead_of_reporting_a_false_pass() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    apply(
        &mut app,
        "scenario \"Run\" { value = 1 ok = assert value == 1 }",
    );
    app.buffers.typed(
        &app.code_context(),
        "scenario \"Run\" { value = 2 ok = assert value == 1 }".into(),
        Instant::now(),
    );
    app.start_run(RunKind::Full);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.is_running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
        app.poll_run();
    }
    assert_eq!(value(&app, false), Binding::literal(json!(2)));
    assert_eq!(app.run.as_ref().unwrap().status, RunStatus::Failed);
    assert!(!app.is_run_stale());
}

#[test]
fn invalid_code_rejects_toolbar_library_inspector_and_report_mutations() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    apply(&mut app, "scenario \"Lock\" { value = 1 }");
    app.buffers.typed(
        &app.code_context(),
        "scenario \"Lock\" { value =".into(),
        Instant::now(),
    );
    app.settle_code();
    let before = app.editor.project().clone();
    let node = app.editor.graph().nodes[0].clone();
    app.editor.selection = vec![node.id.clone()];
    app.board_command(Command::Duplicate);
    app.board_command(Command::Move(vec![(
        node.id.clone(),
        Position { x: 500.0, y: 500.0 },
    )]));
    app.library_action(library::Action::AddNode {
        block: Block::Value,
        component: None,
    });
    let mut empty = app.editor.graph().clone();
    empty.nodes.clear();
    app.apply_graph(empty);
    app.report_action(report::Action::SetErrorPolicy(
        apiforge_core::model::ErrorPolicy::Stop,
    ));
    assert_eq!(app.editor.project(), &before);
    app.buffers.discard(&app.code_context());
    app.board_command(Command::Duplicate);
    assert_eq!(app.editor.graph().nodes.len(), 2);
}

#[test]
fn whole_code_undo_and_redo_restore_both_phases_from_cleanup() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    apply(
        &mut app,
        "scenario \"Before\" { main = 1 cleanup { teardown = 2 } }",
    );
    app.editor.set_phase(Phase::Cleanup);
    apply(
        &mut app,
        "scenario \"After\" { main = 10 cleanup { teardown = 20 } }",
    );
    app.editor.undo();
    assert_eq!(value(&app, false), Binding::literal(json!(1)));
    assert_eq!(value(&app, true), Binding::literal(json!(2)));
    assert_eq!(app.editor.scenario().name, "Before");
    app.editor.redo();
    assert_eq!(value(&app, false), Binding::literal(json!(10)));
    assert_eq!(value(&app, true), Binding::literal(json!(20)));
    assert_eq!(app.editor.scenario().name, "After");
}

#[test]
fn caret_navigation_changes_phase_in_both_directions() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    let text = "scenario \"Phases\" { main = 1 cleanup { teardown = 2 } }";
    apply(&mut app, text);
    app.sync_selection(text, Some(text.find("teardown").unwrap() + 2));
    assert_eq!(app.editor.phase(), Phase::Cleanup);
    assert_eq!(app.editor.selected_node().unwrap().handle, "teardown");
    app.sync_selection(text, Some(text.find("main").unwrap() + 2));
    assert_eq!(app.editor.phase(), Phase::Main);
    assert_eq!(app.editor.selected_node().unwrap().handle, "main");
}

#[test]
fn component_code_places_all_new_blocks_without_moving_existing_ones() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    let component = app.editor.project().components[0].clone();
    app.editor.enter_component(component);
    let previous = app.editor.graph().clone();
    let mut text = buffers::print(app.editor.project(), &app.code_document()).unwrap();
    text.insert_str(text.rfind('}').unwrap(), "extra_one = 1\nextra_two = 2\n");
    apply(&mut app, &text);
    for node in &previous.nodes {
        assert_eq!(
            app.editor
                .graph()
                .nodes
                .iter()
                .find(|n| n.id == node.id)
                .unwrap()
                .position,
            node.position
        );
    }
    let reference = app.editor.reference_graph();
    for fresh in app
        .editor
        .graph()
        .nodes
        .iter()
        .filter(|n| n.handle.starts_with("extra_"))
    {
        let rect = canvas::Card::build(fresh, &reference, app.editor.project()).rect();
        for other in app.editor.graph().nodes.iter().filter(|n| n.id != fresh.id) {
            assert!(
                !rect.intersects(
                    canvas::Card::build(other, &reference, app.editor.project()).rect()
                )
            );
        }
    }
    app.buffers.typed(
        &app.code_context(),
        format!("{text}\nbroken ="),
        Instant::now(),
    );
    app.settle_code();
    let graph = app.editor.graph().clone();
    let ids = graph.nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
    app.dialog = Some(Dialog::Component(Box::new(dialogs::ComponentDialog::new(
        &graph,
        &ids,
        app.editor.project(),
        Some(&app.editor.draft().unwrap().definition),
    ))));
    app.dialogs(&ctx);
    assert!(app.dialog.is_none());
}

#[test]
fn drafts_from_all_projects_are_saved_and_restored_even_if_inactive_at_startup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.json");
    let ctx = egui::Context::default();
    let mut app = App::new(
        &ctx,
        Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
    );
    let projects = app
        .editor
        .workspace()
        .projects
        .iter()
        .take(2)
        .map(|p| p.id.clone())
        .collect::<Vec<_>>();
    let mut keys = Vec::new();
    for (index, project) in projects.iter().enumerate() {
        app.editor.switch_project(project);
        let key = app.code_context();
        app.buffers.typed(
            &key,
            format!("// project {index}\nbroken ="),
            Instant::now(),
        );
        app.settle_code();
        app.save_drafts();
        keys.push(key);
    }
    let drafts = app
        .store
        .as_ref()
        .unwrap()
        .load_drafts()
        .unwrap()
        .unwrap()
        .drafts;
    assert_eq!(drafts.entries.len(), 2);
    // Restore with the second project selected, independently of preferences.
    let mut editor = Editor::new(app.editor.workspace().clone());
    editor.switch_project(&projects[1]);
    let restored = restore_drafts(&editor, Some(drafts));
    for (index, key) in keys.iter().enumerate() {
        assert!(
            restored
                .get(key)
                .unwrap()
                .text
                .contains(&format!("project {index}"))
        );
    }
    app.saver.take();
}

#[test]
fn a_valid_stale_draft_never_overwrites_the_document_before_a_choice() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.json");
    let ctx = egui::Context::default();
    let mut seed = App::new(&ctx, Source::Session(default_workspace()));
    apply(&mut seed, "scenario \"Conflict\" { value = 1 }");
    let old = buffers::print(seed.editor.project(), &seed.code_document()).unwrap();
    apply(&mut seed, "scenario \"Conflict\" { value = 2 }");
    let store = WorkspaceStore::open_exclusive(&path).unwrap();
    store.save(seed.editor.workspace()).unwrap();
    store
        .save_drafts(
            &apiforge_storage::Drafts {
                version: 1,
                entries: vec![apiforge_storage::CodeDraft {
                    project_id: seed.editor.project().id.clone(),
                    target: apiforge_storage::DraftTarget::Scenario {
                        id: seed.editor.scenario().id.clone(),
                    },
                    base_hash: apiforge_storage::draft_base_hash(&old),
                    text: old,
                    cursor: 0,
                }],
            },
            &[],
        )
        .unwrap();
    let mut app = App::new(&ctx, Source::Storage(store.clone()));
    app.code_open = true;
    for _ in 0..3 {
        frame(&mut app, &ctx, egui::vec2(1600.0, 1000.0), vec![]);
    }
    assert!(app.buffers.get(&app.code_context()).unwrap().stale);
    assert_eq!(value(&app, false), Binding::literal(json!(2)));
    let drafts = store.load_drafts().unwrap().unwrap().drafts;
    assert_ne!(
        drafts.entries[0].base_hash,
        apiforge_storage::draft_base_hash(
            &buffers::print(app.editor.project(), &app.code_document()).unwrap()
        )
    );
    app.buffers.discard(&app.code_context());
    frame(&mut app, &ctx, egui::vec2(1600.0, 1000.0), vec![]);
    assert_eq!(value(&app, false), Binding::literal(json!(2)));
}

#[test]
fn resizing_open_panels_keeps_a_useful_board_at_minimum_window_size() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    app.editor.selection = vec![app.editor.graph().nodes[0].id.clone()];
    app.code_open = true;
    for size in [egui::vec2(1600.0, 1000.0), egui::vec2(960.0, 540.0)] {
        for _ in 0..3 {
            frame(&mut app, &ctx, size, vec![]);
        }
        let code = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("code-panel"))
            .unwrap()
            .outer_rect;
        assert!(code.right() <= size.x - 350.0, "{code:?}");
    }
}

#[test]
fn unsaved_component_text_keeps_the_saved_revision_as_its_base() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    app.store = Some(WorkspaceStore::open_exclusive(dir.path().join("workspace.json")).unwrap());
    app.editor
        .enter_component(app.editor.project().components[0].clone());
    let mut text = buffers::print(app.editor.project(), &app.code_document()).unwrap();
    text.insert_str(text.rfind('}').unwrap(), "extra = 1\n");
    apply(&mut app, &text);
    let key = app.code_context();
    app.buffers
        .typed(&key, format!("{text}\nbroken ="), Instant::now());
    app.settle_code();
    app.save_drafts();
    let saved = app
        .store
        .as_ref()
        .unwrap()
        .load_drafts()
        .unwrap()
        .unwrap()
        .drafts;
    let editor = Editor::new(app.editor.workspace().clone());
    let restored = restore_drafts(&editor, Some(saved));
    let buffer = restored.get(&key).unwrap();
    assert!(
        !buffer.stale,
        "the saved component revision has not changed"
    );
    assert!(buffer.text.contains("extra = 1"));
    assert!(buffer.text.ends_with("broken ="));
}

#[test]
fn moving_the_caret_to_cleanup_preserves_whole_scenario_undo() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    apply(
        &mut app,
        "scenario \"Before\" { main = 1 cleanup { teardown = 2 } }",
    );
    let changed = "scenario \"After\" { main = 10 cleanup { teardown = 20 } }";
    apply(&mut app, changed);
    app.sync_selection(changed, Some(changed.find("teardown").unwrap() + 2));
    assert!(
        app.editor.can_undo(),
        "caret navigation must not delete history"
    );
    app.editor.undo();
    assert_eq!(value(&app, false), Binding::literal(json!(1)));
    assert_eq!(value(&app, true), Binding::literal(json!(2)));
    app.editor.set_phase(Phase::Main);
    app.editor.redo();
    assert_eq!(value(&app, false), Binding::literal(json!(10)));
    assert_eq!(value(&app, true), Binding::literal(json!(20)));
}

#[test]
fn closing_code_does_not_leave_valid_pending_typing_locked_forever() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    app.code_open = false;
    app.buffers.typed(
        &app.code_context(),
        "scenario \"Hidden\" { answer = 42 }".into(),
        Instant::now() - buffers::APPLY_DELAY,
    );
    frame(&mut app, &ctx, egui::vec2(1600.0, 1000.0), vec![]);
    assert!(!app.document_locked());
    assert_eq!(app.editor.scenario().name, "Hidden");
    assert_eq!(value(&app, false), Binding::literal(json!(42)));
}

#[test]
fn failing_to_save_a_draft_prevents_close_and_a_retry_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = App::new(
        &ctx,
        Source::Storage(WorkspaceStore::open_exclusive(dir.path().join("workspace.json")).unwrap()),
    );
    app.buffers.typed(
        &app.code_context(),
        "// unfinished\nbroken =".into(),
        Instant::now(),
    );
    app.settle_code();
    // A directory occupying the draft filename makes the atomic write fail.
    std::fs::create_dir(dir.path().join("drafts.json")).unwrap();
    assert!(!app.prepare_close());
    let error = app
        .close_save_error()
        .expect("the only copy of the draft is unsaved");
    assert!(error.contains("draft"), "{error}");
    app.abort_close_after_save_error(error);
    std::fs::remove_dir(dir.path().join("drafts.json")).unwrap();
    app.retry_save();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.prepare_close() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.close_save_error().is_none());
    let drafts = app
        .store
        .as_ref()
        .unwrap()
        .load_drafts()
        .unwrap()
        .unwrap()
        .drafts;
    assert!(
        drafts
            .entries
            .iter()
            .any(|draft| draft.text.contains("broken ="))
    );
}

#[test]
fn text_undo_in_another_scenario_never_restores_the_previous_scenarios_code() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    app.code_open = true;
    let size = egui::vec2(1600.0, 1000.0);
    frame(&mut app, &ctx, size, vec![]);
    let first_id = app.code_editor_id();
    ctx.memory_mut(|m| m.request_focus(first_id));
    frame(
        &mut app,
        &ctx,
        size,
        vec![egui::Event::Text("// FIRST SCENARIO\n".into())],
    );
    let first = app.code_context();
    let mut other = app.editor.scenario().clone();
    other.id = "second-text-history".into();
    other.name = "Second scenario".into();
    app.editor.add_scenario(other);
    frame(&mut app, &ctx, size, vec![]);
    ctx.memory_mut(|m| m.request_focus(app.code_editor_id()));
    frame(
        &mut app,
        &ctx,
        size,
        vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        }],
    );
    let second = app.buffers.get(&app.code_context()).unwrap();
    assert!(second.text.contains("Second scenario"), "{}", second.text);
    assert!(!second.text.contains("FIRST SCENARIO"));
    assert!(
        app.buffers
            .get(&first)
            .unwrap()
            .text
            .contains("FIRST SCENARIO")
    );
}

#[test]
fn mixed_board_edits_undo_and_redo_in_order_after_switching_phases() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    apply(
        &mut app,
        "scenario \"History\" { main = 1 cleanup { teardown = 2 } }",
    );
    let mut main = app.editor.graph().clone();
    main.nodes[0]
        .inputs
        .insert("value".into(), Binding::literal(json!(10)));
    app.apply_graph(main);
    app.editor.set_phase(Phase::Cleanup);
    let mut cleanup = app.editor.graph().clone();
    cleanup.nodes[0]
        .inputs
        .insert("value".into(), Binding::literal(json!(20)));
    app.apply_graph(cleanup);
    app.editor.set_phase(Phase::Main);
    app.editor.undo();
    assert_eq!(value(&app, false), Binding::literal(json!(10)));
    assert_eq!(value(&app, true), Binding::literal(json!(2)));
    app.editor.undo();
    assert_eq!(value(&app, false), Binding::literal(json!(1)));
    assert_eq!(value(&app, true), Binding::literal(json!(2)));
    app.editor.set_phase(Phase::Cleanup);
    app.editor.redo();
    assert_eq!(value(&app, false), Binding::literal(json!(10)));
    assert_eq!(value(&app, true), Binding::literal(json!(2)));
    app.editor.set_phase(Phase::Main);
    app.editor.redo();
    assert_eq!(value(&app, false), Binding::literal(json!(10)));
    assert_eq!(value(&app, true), Binding::literal(json!(20)));
}

#[test]
fn restored_code_caret_is_applied_before_the_first_input() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    app.code_open = true;
    let text = "scenario \"Caret\" { first = \"żółć\" second = 2 }";
    let cursor = text.find("second").unwrap();
    app.buffers.restore(
        app.code_context(),
        buffers::Draft {
            text: text.into(),
            cursor,
            stale: false,
        },
    );
    frame(&mut app, &ctx, egui::vec2(1600.0, 1000.0), vec![]);
    let state = egui::text_edit::TextEditState::load(&ctx, app.code_editor_id()).unwrap();
    assert_eq!(
        state.cursor.char_range().unwrap().primary.index.0,
        text[..cursor].chars().count()
    );
    ctx.memory_mut(|m| m.request_focus(app.code_editor_id()));
    frame(
        &mut app,
        &ctx,
        egui::vec2(1600.0, 1000.0),
        vec![egui::Event::Text("new_".into())],
    );
    assert!(
        app.buffers
            .get(&app.code_context())
            .unwrap()
            .text
            .contains("new_second")
    );
}

#[test]
fn ctrl_e_closes_a_focused_code_editor_and_typing_still_applies() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    let dir = tempfile::tempdir().unwrap();
    app.preferences = Preferences::at(Some(dir.path().join("preferences.json")));
    app.code_open = true;
    frame(&mut app, &ctx, egui::vec2(1600.0, 1000.0), vec![]);
    ctx.memory_mut(|m| m.request_focus(app.code_editor_id()));
    app.buffers.typed(
        &app.code_context(),
        "scenario \"Closed\" { value = 42 }".into(),
        Instant::now(),
    );
    frame(
        &mut app,
        &ctx,
        egui::vec2(1600.0, 1000.0),
        vec![egui::Event::Key {
            key: Key::E,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        }],
    );
    assert!(!app.code_open);
    // No input is needed: the scheduled repaint settles hidden code.
    std::thread::sleep(buffers::APPLY_DELAY + Duration::from_millis(5));
    frame(&mut app, &ctx, egui::vec2(1600.0, 1000.0), vec![]);
    assert!(!app.document_locked());
    assert_eq!(app.editor.scenario().name, "Closed");
}

#[test]
fn a_board_reprint_clears_text_undo_that_belongs_to_an_older_document() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    apply(&mut app, "scenario \"Reprint\" { value = 1 }");
    app.code_open = true;
    let size = egui::vec2(1600.0, 1000.0);
    frame(&mut app, &ctx, size, vec![]);
    ctx.memory_mut(|m| m.request_focus(app.code_editor_id()));
    frame(
        &mut app,
        &ctx,
        size,
        vec![egui::Event::Text("// local formatting\n".into())],
    );
    app.settle_code();
    ctx.memory_mut(|m| m.surrender_focus(app.code_editor_id()));
    let mut graph = app.editor.graph().clone();
    graph.nodes[0]
        .inputs
        .insert("value".into(), Binding::literal(json!(2)));
    app.apply_graph(graph);
    frame(&mut app, &ctx, size, vec![]);
    let current = app.buffers.get(&app.code_context()).unwrap().text.clone();
    ctx.memory_mut(|m| m.request_focus(app.code_editor_id()));
    frame(
        &mut app,
        &ctx,
        size,
        vec![egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        }],
    );
    assert_eq!(app.buffers.get(&app.code_context()).unwrap().text, current);
    assert_eq!(value(&app, false), Binding::literal(json!(2)));
}

#[test]
fn a_component_draft_left_unsaved_shows_the_saved_definition_when_opened_again() {
    // The panel keeps the text it printed, so it must not hand back a draft the
    // user discarded: leaving and re-entering a component changes no revision.
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Source::Session(default_workspace()));
    app.code_open = true;
    let size = egui::vec2(1600.0, 1000.0);
    let component = app.editor.project().components[0].clone();

    app.editor.enter_component(component.clone());
    frame(&mut app, &ctx, size, vec![]);
    let saved = buffers::print(app.editor.project(), &app.code_document()).unwrap();

    let mut edited = saved.clone();
    edited.insert_str(edited.rfind('}').unwrap(), "only_in_the_draft = 1\n");
    apply(&mut app, &edited);
    frame(&mut app, &ctx, size, vec![]);
    assert!(
        app.editor
            .graph()
            .nodes
            .iter()
            .any(|node| node.handle == "only_in_the_draft")
    );

    // Leave without saving and open the same revision again, without a frame
    // in between: nothing else prints, so only the panel's own key can tell
    // that this is the saved definition once more.
    app.buffers.discard(&app.code_context());
    app.editor.leave_component();
    app.editor.enter_component(component);
    frame(&mut app, &ctx, size, vec![]);

    assert_eq!(
        buffers::print(app.editor.project(), &app.code_document()).unwrap(),
        saved,
        "the discarded draft is gone"
    );
    assert_eq!(
        app.printed.as_ref().unwrap().text.as_deref(),
        Some(saved.as_str()),
        "and the panel prints the saved definition, not the text it kept"
    );
}
