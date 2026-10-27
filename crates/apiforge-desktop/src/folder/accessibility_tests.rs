use super::recovery_tests::{FOREIGN, LOCAL, interrupt, source_project};
use super::*;
use eframe::egui::accesskit::{Action, Node, Role};
use std::fs;

/// A node's accessible name. AccessKit computes it from the node's own label
/// or, when the visible label is a separate widget, from the `labelled_by`
/// relation; a node with the label role carries its text in `value`.
fn accessible_name(tree: &eframe::egui::accesskit::TreeUpdate, node: &Node) -> Option<String> {
    if let Some(label) = node.label().filter(|label| !label.trim().is_empty()) {
        return Some(label.to_owned());
    }
    let parts: Vec<String> = node
        .labelled_by()
        .iter()
        .filter_map(|wanted| tree.nodes.iter().find(|(id, _)| id == wanted))
        .filter_map(|(_, label)| label.value().or_else(|| label.label()))
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Every interactive accessibility node of one frame without a name, as
/// `role (value)`, at `size`.
fn unnamed(app: &mut FolderApp, context: &egui::Context, size: egui::Vec2) -> Vec<String> {
    context.enable_accesskit();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
    let tree = output
        .platform_output
        .accesskit_update
        .expect("accessibility tree");
    tree.nodes
        .iter()
        .filter(|(_, node)| {
            (matches!(
                node.role(),
                Role::Button
                    | Role::CheckBox
                    | Role::ComboBox
                    | Role::TextInput
                    | Role::MultilineTextInput
                    | Role::Tab
                    | Role::MenuItem
                    | Role::RadioButton
                    | Role::Slider
                    | Role::SpinButton
                    | Role::Link
                    | Role::ListBoxOption
                    | Role::TreeItem
            ) || node.supports_action(Action::Focus))
                && accessible_name(&tree, node).is_none()
        })
        .map(|(_, node)| {
            format!(
                "{:?} ({:?}) at {:?}",
                node.role(),
                node.value(),
                node.bounds()
            )
        })
        .collect()
}

/// Draws `app` settled at both window sizes and asserts that every control
/// it exposes has a name.
fn assert_named(screen: &str, app: &mut FolderApp, context: &egui::Context) {
    wait_storage(app, context);
    for size in [egui::vec2(960.0, 540.0), egui::vec2(1600.0, 1000.0)] {
        for _ in 0..3 {
            unnamed(app, context, size);
        }
        let found = unnamed(app, context, size);
        assert!(found.is_empty(), "{screen} at {size:?}: unnamed {found:?}");
    }
}

#[test]
fn stage_ii_screens_and_dialogs_name_every_control() {
    // The inspector's block fields are older shared controls, reviewed with M7;
    // a selection would show them, so the board is audited without one.
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    for mode in [Mode::Code, Mode::Split, Mode::Diagram] {
        app.workbench.mode = mode;
        assert_named(&format!("{mode:?}"), &mut app, &context);
    }
    app.workbench.mode = Mode::Code;
    let path = SCENARIO.to_owned();
    let prompts = [
        ("settings", None),
        ("environment", None),
        ("close tab", Some(Prompt::CloseTab(id.clone()))),
        (
            "new document",
            Some(Prompt::NewDocument {
                kind: DocumentKind::Scenario,
                folder: None,
                name: "new.afs".into(),
                error: None,
            }),
        ),
        (
            "rename",
            Some(Prompt::Rename {
                path: path.clone(),
                name: "renamed.afs".into(),
                error: None,
            }),
        ),
        (
            "move",
            Some(Prompt::MoveTo {
                path: path.clone(),
                folder: String::new(),
                error: None,
            }),
        ),
        (
            "new folder",
            Some(Prompt::NewFolder {
                parent: String::new(),
                name: "folder".into(),
                error: None,
            }),
        ),
    ];
    for (screen, prompt) in prompts {
        match (screen, prompt) {
            ("settings", _) => app.open_project_settings(),
            ("environment", _) => app.open_environment(),
            (_, prompt) => app.prompt = prompt,
        }
        assert_named(screen, &mut app, &context);
        app.prompt = None;
    }
    app.request_delete(vec![path]);
    assert_named("delete", &mut app, &context);
    app.prompt = None;

    let (directory, id) = source_project();
    let (context, mut app) = open(directory.path());
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, LOCAL, CODE)
        .unwrap();
    app.session.as_mut().unwrap().settle(&id).unwrap();
    fs::write(directory.path().join("main.afs"), FOREIGN).unwrap();
    watch(&mut app, &context);
    assert_named("conflict banner", &mut app, &context);
    app.review_conflict(apiforge_project::ConflictTarget::Document(id));
    wait_storage(&mut app, &context);
    assert!(matches!(app.prompt, Some(Prompt::Conflict(_))));
    assert_named("conflict review", &mut app, &context);

    let (directory, _) = source_project();
    interrupt(directory.path());
    let (context, mut app) = open(directory.path());
    assert_named("transaction recovery", &mut app, &context);

    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("found.afs"), LOCAL).unwrap();
    let (context, mut app) = open(directory.path());
    assert_named("initialise", &mut app, &context);
}

/// KTLO-05d criterion 3: the *Add step* popup names every control, with its
/// Variables and Operators sections (Split at 960 × 540) and without them
/// (Split at 1600 × 1000, beside the library panel).
#[test]
fn the_add_step_popup_names_every_control() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    app.workbench.mode = Mode::Split;
    wait_storage(&mut app, &context);
    frame(&mut app, &context, vec![]);
    app.canvases.get_mut(&id).unwrap().debug_open_menu();
    assert_named("Add step popup", &mut app, &context);
    assert!(
        app.canvases.get(&id).unwrap().debug_menu_open(),
        "the popup stayed open during the audit"
    );
}

/// The names of the controls Tab visits from the current focus, in order.
fn tab_order(app: &mut FolderApp, context: &egui::Context, presses: usize) -> Vec<String> {
    context.enable_accesskit();
    let mut visited = Vec::new();
    for _ in 0..presses {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                events: vec![key(Key::Tab, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| app.ui(ui),
        );
        output.textures_delta.clear();
        let tree = output.platform_output.accesskit_update.unwrap();
        // With nothing focused the tree reports its root, the window itself.
        let name = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == tree.focus && *id != tree.tree.as_ref().map_or(*id, |t| t.root))
            .map(|(_, node)| accessible_name(&tree, node).unwrap_or_else(|| "<unnamed>".to_owned()))
            .unwrap_or_else(|| "<window>".to_owned());
        visited.push(name);
    }
    visited
}

#[test]
fn the_inspector_names_the_controls_of_every_demo_block() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    app.workbench.mode = Mode::Diagram;
    let model = model(&app, &id);
    for cleanup in [false, true] {
        for node in &graph_of(&model, cleanup).nodes {
            if let Some(tab) = app.workbench.tab_mut(&id) {
                tab.cleanup = cleanup;
            }
            app.selections.insert(id.clone(), vec![node.id.clone()]);
            assert_named(
                &format!("{:?} {}", node.kind, node.handle),
                &mut app,
                &context,
            );
        }
    }
}

#[test]
fn tab_visits_the_window_in_reading_order_with_named_stops() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let visited = tab_order(&mut app, &context, 21);
    assert!(
        !visited.iter().any(|name| name == "<unnamed>"),
        "{visited:#?}"
    );
    let expected = [
        "Project",
        "Run",
        "Code",
        "promotion-and-cart-price.afs",
        "New",
        "Project files",
        "Resize project files",
        "Contents of scenarios/promotion-and-cart-price.afs",
    ];
    let mut at = 0;
    for name in expected {
        at += visited[at..]
            .iter()
            .position(|visit| visit == name)
            .unwrap_or_else(|| panic!("{name} after {at} in {visited:#?}"));
    }
}

#[test]
fn project_dialogs_keep_tab_inside_and_escape_cancels_them() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    for (dialog, inside) in [
        (
            "settings",
            &["Project name", "Apply settings", "Cancel", "Close"][..],
        ),
        (
            "environment",
            &["Base URL", "Apply environment", "Cancel", "Close"][..],
        ),
    ] {
        match dialog {
            "settings" => app.open_project_settings(),
            _ => app.open_environment(),
        }
        wait_storage(&mut app, &context);
        let visited = tab_order(&mut app, &context, 16);
        for name in inside {
            assert!(
                visited.iter().any(|visit| visit == name),
                "{dialog}: {visited:#?}"
            );
        }
        for outside in ["Run", "Code", "Project files"] {
            assert!(
                !visited.iter().any(|visit| visit == outside),
                "{dialog}: {visited:#?}"
            );
        }
        frame(&mut app, &context, vec![key(Key::Escape, Modifiers::NONE)]);
        wait_storage(&mut app, &context);
        assert!(app.prompt.is_none(), "{dialog} stays open after Escape");
    }
}

#[test]
fn tab_indents_in_the_editor_and_escape_then_tab_leaves_it() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    let before = text(&app, &id);
    context.memory_mut(|memory| memory.request_focus(editor_id(&id)));
    for _ in 0..3 {
        frame(&mut app, &context, vec![]);
    }
    frame(&mut app, &context, vec![key(Key::Tab, Modifiers::NONE)]);
    frame(&mut app, &context, vec![]);
    assert!(context.memory(|memory| memory.has_focus(editor_id(&id))));
    assert_ne!(text(&app, &id), before, "Tab is the language's indentation");
    frame(&mut app, &context, vec![key(Key::Escape, Modifiers::NONE)]);
    frame(&mut app, &context, vec![key(Key::Tab, Modifiers::NONE)]);
    frame(&mut app, &context, vec![]);
    assert!(
        !context.memory(|memory| memory.has_focus(editor_id(&id))),
        "the editor is not a keyboard trap"
    );
}

/// Tabs to the control named `name` and presses Space on it, as a keyboard
/// user opens a menu.
fn open_by_keyboard(app: &mut FolderApp, context: &egui::Context, name: &str) {
    context.memory_mut(|memory| memory.surrender_focus(memory.focused().unwrap_or(egui::Id::NULL)));
    for _ in 0..40 {
        if tab_order(app, context, 1)[0] == name {
            frame(app, context, vec![key(Key::Space, Modifiers::NONE)]);
            frame(app, context, vec![]);
            return;
        }
    }
    panic!("Tab never reached {name}");
}

#[test]
fn menus_opened_from_the_keyboard_are_named_and_reachable() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    for (button, menu, first) in [
        ("Project", "Project menu", "Open Folder…"),
        ("Run options", "Run options menu", "Run live requests"),
        ("New", "New menu", "New scenario"),
    ] {
        open_by_keyboard(&mut app, &context, button);
        assert_named(button, &mut app, &context);
        let visited = tab_order(&mut app, &context, 3);
        assert_eq!(visited[0], menu, "{visited:?}");
        assert!(
            visited.iter().any(|name| name.starts_with(first)),
            "{visited:?}"
        );
        frame(&mut app, &context, vec![key(Key::Escape, Modifiers::NONE)]);
        frame(&mut app, &context, vec![]);
    }
}

/// Whether one frame of `app` at the default window exposes a node named `name`.
fn exposes(app: &mut FolderApp, context: &egui::Context, name: &str) -> bool {
    context.enable_accesskit();
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
    let tree = output.platform_output.accesskit_update.unwrap();
    tree.nodes
        .iter()
        .any(|(_, node)| accessible_name(&tree, node).as_deref() == Some(name))
}

/// KTLO-17 criterion 7: the audit passes with the library expanded and
/// collapsed (Ctrl+B), in Diagram and in Split.
#[test]
fn the_block_library_names_every_control_expanded_and_collapsed() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    for mode in [Mode::Diagram, Mode::Split] {
        app.workbench.mode = mode;
        wait_storage(&mut app, &context);
        frame(&mut app, &context, vec![]);
        assert!(
            exposes(&mut app, &context, "Collapse block library"),
            "{mode:?}: the expanded panel has its chevron"
        );
        assert_named(&format!("{mode:?}, library expanded"), &mut app, &context);
        frame(&mut app, &context, vec![key(Key::B, COMMAND)]);
        assert!(
            exposes(&mut app, &context, "Show block library"),
            "{mode:?}: Ctrl+B folds the library to its rail"
        );
        assert_named(&format!("{mode:?}, library collapsed"), &mut app, &context);
        frame(&mut app, &context, vec![key(Key::B, COMMAND)]);
    }
}

/// Where the control named `name` is, for a pointer click at 1600 × 1000.
fn centre_of(app: &mut FolderApp, context: &egui::Context, name: &str) -> egui::Pos2 {
    context.enable_accesskit();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
    let tree = output.platform_output.accesskit_update.unwrap();
    let bounds = tree
        .nodes
        .iter()
        .find(|(_, node)| accessible_name(&tree, node).as_deref() == Some(name))
        .and_then(|(_, node)| node.bounds())
        .unwrap_or_else(|| panic!("no control named {name}"));
    egui::pos2(
        ((bounds.x0 + bounds.x1) / 2.0) as f32,
        ((bounds.y0 + bounds.y1) / 2.0) as f32,
    )
}

/// KTLO-19 criterion 7: the audit passes with the run results maximised, in
/// every mode.
#[test]
fn the_maximised_run_results_name_every_control() {
    let directory = shop();
    let (context, mut app) = open(directory.path());
    let id = document(&app, SCENARIO);
    app.request_run(&id, false);
    finish_folder_run(&mut app);
    for mode in [Mode::Diagram, Mode::Split, Mode::Code] {
        app.workbench.mode = mode;
        frame(&mut app, &context, vec![]);
        let at = centre_of(&mut app, &context, "Maximise run results");
        click(&mut app, &context, at);
        assert!(
            exposes(&mut app, &context, "Restore run results"),
            "{mode:?}: the run results are maximised"
        );
        assert_named(
            &format!("{mode:?}, run results maximised"),
            &mut app,
            &context,
        );
    }
}
