//! DIAG-25 (#627): *Assign to variable* gives a new variable a name in code
//! that is free on Main and Cleanup together, which share one namespace of
//! handles (DIAG-17), so the dialog does not refuse a name the other board
//! uses. On the room project's *Room booking day*, never run, through the
//! Inspector's buttons and the dialog.
use super::diag06a::{graph, id_of, passes, rooms};
use super::*;
use apiforge_core::format::{Binding, Node};

const SCENARIO: &str = "scenarios/booking-day.afs";
const VARIABLE: &str = "Assign to variable";
const STATUS: &str = "HTTP status";

/// The scenario on its board, Cleanup when `cleanup`.
fn opened(cleanup: bool) -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = rooms(SCENARIO, |_| {});
    if cleanup {
        choose(&mut harness, "Cleanup");
    }
    assert_eq!(folder(&harness).cleanup(&id), cleanup, "the board in view");
    (project, harness, id)
}

/// *Assign to variable* on `field` of the step `handle`'s response: the
/// dialog is open.
fn open_dialog(harness: &mut FolderHarness, id: &str, handle: &str, field: &str) {
    let step = id_of(&graph(harness, id), handle);
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
    super::inspector_tab(harness, "Output data")
        .expect("the Inspector has an Output data tab")
        .click();
    settle(harness);
    let edge = harness.get_by_label("Resize inspector").rect().left();
    let y = harness
        .query_all_by_label(field)
        .find(|node| node.rect().left() > edge)
        .unwrap_or_else(|| panic!("the Inspector shows the field {field}"))
        .rect()
        .center()
        .y;
    let away = |node: &egui_kittest::Node<'_>| (node.rect().center().y - y).abs();
    harness
        .query_all_by_label(VARIABLE)
        .filter(|button| button.rect().left() > edge)
        .min_by(|a, b| away(a).total_cmp(&away(b)))
        .unwrap_or_else(|| panic!("the {field} row has {VARIABLE}"))
        .click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
    assert!(
        has_label(harness, "Assign variable"),
        "the dialog to name the variable opens"
    );
}

/// Applies the open dialog as a new variable named `name` and returns the
/// variable it wrote on the board in view.
fn assigned(harness: &mut FolderHarness, id: &str, name: &str) -> Node {
    let before = graph(harness, id);
    replace_text(harness, "Variable name", name);
    choose(harness, "Assign variable");
    assert!(
        !has_label(harness, "Variable name"),
        "the dialog is applied, not refused"
    );
    let after = graph(harness, id);
    let new: Vec<&Node> = after
        .nodes
        .iter()
        .filter(|node| before.nodes.iter().all(|old| old.id != node.id))
        .collect();
    assert_eq!(new.len(), 1, "one step added: {new:?}");
    assert!(new[0].is_variable(), "the step is a variable: {new:?}");
    new[0].clone()
}

/// The variable reads `port.path` of the step `handle`.
fn assert_reads(
    harness: &FolderHarness,
    id: &str,
    variable: &Node,
    (handle, port, path): (&str, &str, &str),
) {
    let source = id_of(&graph(harness, id), handle);
    assert_eq!(
        variable.inputs.get("value"),
        Some(&Binding::reference(&source, port, path)),
        "the variable reads {handle}.{port}.{path}"
    );
}

/// The source's text before `cleanup {`, and the text from it on.
fn boards(text: &str) -> (&str, &str) {
    text.split_at(text.find("cleanup {").expect("the cleanup block"))
}

/// One Undo gives the file back byte for byte.
fn undone(harness: &mut FolderHarness, id: &str, before: &str) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(harness);
    harness.step();
    assert_eq!(
        source_of(harness, id),
        before,
        "one Undo gives the file back"
    );
}

/// Criterion 1: in Cleanup, a variable named as Main's step `token` is
/// written in the cleanup as `token_2`, labelled `token`; the next one of
/// that name is `token_3`.
#[test]
fn diag25_criterion_1_a_cleanup_variable_named_as_a_main_step() {
    let (_project, mut harness, id) = opened(true);
    let before = source_of(&harness, &id);

    open_dialog(&mut harness, &id, "released", STATUS);
    let variable = assigned(&mut harness, &id, "token");
    assert_eq!(variable.handle, "token_2");
    assert_eq!(variable.label.as_deref(), Some("token"));
    assert_reads(&harness, &id, &variable, ("released", "status", ""));
    let text = passes(&harness, &id);
    let (main, cleanup) = boards(&text);
    assert!(cleanup.contains("token_2 = "), "{text}");
    assert!(!main.contains("token_2"), "{text}");
    undone(&mut harness, &id, &before);

    open_dialog(&mut harness, &id, "released", STATUS);
    assert_eq!(assigned(&mut harness, &id, "token").handle, "token_2");
    open_dialog(&mut harness, &id, "cancelled", STATUS);
    assert_eq!(assigned(&mut harness, &id, "token").handle, "token_3");
    passes(&harness, &id);
}

/// Criterion 2: from Main, a variable named as Cleanup's step `released` is
/// written in Main as `released_2`, labelled `released`.
#[test]
fn diag25_criterion_2_a_main_variable_named_as_a_cleanup_step() {
    let (_project, mut harness, id) = opened(false);
    let before = source_of(&harness, &id);

    open_dialog(&mut harness, &id, "profile", "name");
    let variable = assigned(&mut harness, &id, "released");
    assert_eq!(variable.handle, "released_2");
    assert_eq!(variable.label.as_deref(), Some("released"));
    assert_reads(&harness, &id, &variable, ("profile", "body", "name"));
    let text = passes(&harness, &id);
    let (main, cleanup) = boards(&text);
    assert!(main.contains("released_2 = "), "{text}");
    assert!(!cleanup.contains("released_2"), "{text}");
    undone(&mut harness, &id, &before);
}

/// Criterion 3: a name free on both boards is written as typed, and a
/// variable chosen in the dialog keeps its name in code, also when it is
/// given a label the other board uses as a name in code.
#[test]
fn diag25_criterion_3_other_names_unchanged() {
    let (_project, mut harness, id) = opened(true);

    open_dialog(&mut harness, &id, "released", STATUS);
    let variable = assigned(&mut harness, &id, "outcome");
    assert_eq!(variable.handle, "outcome");
    assert!(
        passes(&harness, &id).contains("outcome = "),
        "a free name is written as typed"
    );

    for name in ["outcome", "token"] {
        open_dialog(&mut harness, &id, "cancelled", STATUS);
        harness
            .query_all_by_role(egui::accesskit::Role::ComboBox)
            .find(|list| list.value().as_deref() == Some("Create a new variable"))
            .expect("the dialog's list of variables")
            .click();
        settle(&mut harness);
        let dialog = harness.get_by_label("Variable name").rect();
        harness
            .query_all_by_label("outcome")
            .filter(|item| dialog.x_range().contains(item.rect().center().x))
            .min_by(|a, b| {
                let away = |node: &egui_kittest::Node<'_>| {
                    (node.rect().center().y - dialog.center().y).abs()
                };
                away(a).total_cmp(&away(b))
            })
            .expect("the list names the variable")
            .click();
        settle(&mut harness);
        replace_text(&mut harness, "Variable name", name);
        let count = graph(&harness, &id).nodes.len();
        let before = source_of(&harness, &id);
        choose(&mut harness, "Assign variable");
        assert!(!has_label(&harness, "Variable name"), "applied");
        let board = graph(&harness, &id);
        assert_eq!(board.nodes.len(), count, "no step is added");
        let kept = board
            .nodes
            .iter()
            .find(|node| node.id == variable.id)
            .expect("the variable is kept");
        assert_eq!(kept.handle, "outcome", "its name in code is kept");
        assert_eq!(kept.display_name(), name);
        assert_reads(&harness, &id, kept, ("cancelled", "status", ""));
        passes(&harness, &id);
        undone(&mut harness, &id, &before);
    }

    let (_project, mut harness, id) = opened(false);
    open_dialog(&mut harness, &id, "profile", "name");
    assert_eq!(assigned(&mut harness, &id, "person").handle, "person");
    passes(&harness, &id);
}
