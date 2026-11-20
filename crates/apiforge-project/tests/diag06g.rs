//! DIAG-06g (#502): Undo and Redo restore the file byte for byte. What Undo
//! brings back is the text that was there — its free comments, its blank
//! lines, the author's attribute order — and every settled change of the
//! text is a step of the one history, also a change that touches no block.
//!
//! On `shop-30` (in the demo shop, whose components it calls) and
//! `rooms/booking-day` (in its own folder project), each with a free comment,
//! blank lines and attributes in a non-canonical order placed in the
//! statements the tests edit.
use apiforge_core::{
    block::Block,
    edit::{OutputRef, add_node, connect, duplicate_selection},
    format::{Binding, Graph, Position, Project},
    graph::{ORDER_PORT, delete_nodes},
};
use apiforge_project::*;
use std::{collections::HashSet, fs, path::Path, sync::mpsc};

const ORIGIN: Origin = Origin(7);
const CODE: Origin = Origin(8);

// Fixtures -------------------------------------------------------------------

fn repository() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn fixture(name: &str) -> String {
    fs::read_to_string(repository().join("fixtures/scenarios/lay").join(name)).unwrap()
}

/// Replaces `from`, which `text` holds exactly once, by `to`.
fn marked(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "`{from}` is written once");
    text.replacen(from, to, 1)
}

/// The free comment inside the braces of an `if`, which belongs to no
/// statement.
const SHOP_COMMENT: &str = "    // Partial refunds are not covered yet.\n";
const ROOMS_COMMENT: &str = "    // A third booking is tried by hand.\n";

/// `shop-30`, marked: `catalogue` writes `query` before `headers`, `refund`
/// its sample before its headers, blank lines stand between statements and
/// a free comment closes the braces of `paid`.
fn shop_text() -> String {
    let text = fixture("shop-30.afs");
    let text = marked(
        &text,
        "    headers { Authorization: \"Bearer {{login.body.token}}\" }\n    query { limit: 20 }\n",
        "    query { limit: 20 }\n    headers { Authorization: \"Bearer {{login.body.token}}\" }\n",
    );
    let text = marked(
        &text,
        "  listed = assert nonempty catalogue.body.items\n",
        "  listed = assert nonempty catalogue.body.items\n\n",
    );
    let text = marked(
        &text,
        "  cart_total = assert cart.body.total ~= expected_total within 0.01\n",
        "  cart_total = assert cart.body.total ~= expected_total within 0.01\n\n",
    );
    let text = marked(
        &text,
        "      headers { Authorization: \"Bearer {{login.body.token}}\" }\n      sample 200 { refunded: true, amount: 172.5 }\n",
        "      sample 200 { refunded: true, amount: 172.5 }\n      headers { Authorization: \"Bearer {{login.body.token}}\" }\n",
    );
    let text = marked(
        &text,
        "    receipt_total = assert receipt.body.total ~= cart.body.total within 0.01\n",
        "    receipt_total = assert receipt.body.total ~= cart.body.total within 0.01\n\n",
    );
    marked(
        &text,
        "      sample 204 {}\n    }\n  } else {\n",
        &format!("      sample 204 {{}}\n    }}\n\n{SHOP_COMMENT}  }} else {{\n"),
    )
}

/// `rooms/booking-day`, marked: `rooms` writes `query` before `headers`,
/// `notice` its body before its headers, blank lines stand between
/// statements and a free comment closes the then-braces of `booked`.
fn rooms_text() -> String {
    let text = fixture("rooms/scenarios/booking-day.afs");
    let text = marked(
        &text,
        "    headers { Authorization: \"Bearer {{token}}\" }\n    query { floor: 2 }\n",
        "    query { floor: 2 }\n    headers { Authorization: \"Bearer {{token}}\" }\n",
    );
    let text = marked(
        &text,
        "    headers { Authorization: \"Bearer {{token}}\" }\n    body { channel: \"email\", text: \"Your booking is confirmed.\" }\n",
        "    body { channel: \"email\", text: \"Your booking is confirmed.\" }\n    headers { Authorization: \"Bearer {{token}}\" }\n",
    );
    let text = marked(
        &text,
        "  sent = assert notice.status == 202\n",
        "\n  sent = assert notice.status == 202\n\n",
    );
    let text = marked(
        &text,
        "    refused = assert second.status == 409\n",
        "    refused = assert second.status == 409\n\n",
    );
    marked(
        &text,
        "    one_booking = assert calendar.body.items[0].id == first.body.id\n",
        &format!(
            "    one_booking = assert calendar.body.items[0].id == first.body.id\n{ROOMS_COMMENT}"
        ),
    )
}

/// One of the two reference files, open in a session of its project, with
/// the handles of the statements the edits touch.
struct Opened {
    name: &'static str,
    _directory: tempfile::TempDir,
    session: ProjectSession,
    id: String,
    /// A step whose *Next* a new request is added from.
    next: &'static str,
    /// A value wired into `wired`'s body.
    value: &'static str,
    wired: &'static str,
    /// A step with its checks, deleted together.
    deleted: [&'static str; 2],
    /// An `if` with statements, a blank line and a free comment in its braces.
    branch: &'static str,
    /// A request whose address the inspector changes.
    inspected: &'static str,
    /// A step copied and pasted.
    copied: &'static str,
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn shop() -> Opened {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, "diag06g/shop-30.afs", &shop_text())
        .unwrap();
    opened(Opened {
        name: "shop-30",
        _directory: directory,
        session,
        id,
        next: "cart",
        value: "expected_total",
        wired: "logout",
        deleted: ["refund", "refund_amount"],
        branch: "paid",
        inspected: "catalogue",
        copied: "address",
    })
}

fn rooms() -> Opened {
    let directory = tempfile::tempdir().unwrap();
    copy_tree(
        &repository().join("fixtures/scenarios/lay/rooms"),
        directory.path(),
    );
    fs::write(
        directory.path().join("scenarios/booking-day.afs"),
        rooms_text(),
    )
    .unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    let id = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "scenarios/booking-day.afs")
        .unwrap()
        .id()
        .to_owned();
    opened(Opened {
        name: "rooms/booking-day",
        _directory: directory,
        session,
        id,
        next: "first",
        value: "slot",
        wired: "policy",
        deleted: ["notice", "sent"],
        branch: "booked",
        inspected: "rooms",
        copied: "policy",
    })
}

/// The file is read as a valid model with its marks still in the text.
fn opened(opened: Opened) -> Opened {
    let document = opened.session.document(&opened.id).unwrap();
    assert!(
        document.model().is_some(),
        "{}: the marked file is valid\n{:#?}",
        opened.name,
        document.diagnostics()
    );
    assert!(document.text().contains("\n\n"), "{}", opened.name);
    opened
}

fn both() -> [Opened; 2] {
    [shop(), rooms()]
}

// Edits as the board makes them ---------------------------------------------

fn text(session: &ProjectSession, id: &str) -> String {
    session.document(id).unwrap().text().to_owned()
}

fn model(session: &ProjectSession, id: &str) -> Document {
    session.document(id).unwrap().model().unwrap().clone()
}

fn main_graph(document: &mut Document) -> &mut Graph {
    match document {
        Document::Scenario(scenario) => &mut scenario.graph,
        Document::Component(component) => &mut component.graph,
    }
}

fn id_of(document: &Document, handle: &str) -> String {
    main_graph(&mut document.clone())
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a block `{handle}`"))
        .id
        .clone()
}

/// Applies a board edit; a whole-file print that would drop a free comment
/// is accepted, as the person would after the window's question.
fn edit(session: &mut ProjectSession, id: &str, next: Document) {
    match session.apply_model_edit(id, next.clone(), ORIGIN) {
        Err(Error::CommentsWouldBeLost { .. }) => {
            session.accept_comment_loss(id).unwrap();
            session.apply_model_edit(id, next, ORIGIN).unwrap();
        }
        result => result.unwrap(),
    }
}

/// The board's edit `change` on the current model.
fn change(opened: &mut Opened, change: impl FnOnce(&mut Graph, &Project)) {
    let mut next = model(&opened.session, &opened.id);
    let context = opened.session.context().clone();
    change(main_graph(&mut next), &context);
    edit(&mut opened.session, &opened.id, next);
}

fn add_block(opened: &mut Opened) {
    change(opened, |graph, _| {
        *graph = add_node(
            graph,
            Block::Request,
            Position { x: 40.0, y: 40.0 },
            None,
            None,
        )
        .expect("the block is added")
        .graph;
    });
}

fn add_from_next(opened: &mut Opened) {
    let source = id_of(&model(&opened.session, &opened.id), opened.next);
    change(opened, |graph, _| {
        *graph = add_node(
            graph,
            Block::Request,
            Position { x: 40.0, y: 40.0 },
            None,
            Some(OutputRef {
                node_id: &source,
                port: ORDER_PORT,
            }),
        )
        .expect("the step is added")
        .graph;
    });
}

fn wire(opened: &mut Opened) {
    let before = model(&opened.session, &opened.id);
    let (value, wired) = (id_of(&before, opened.value), id_of(&before, opened.wired));
    change(opened, |graph, project| {
        *graph = connect(graph, project, &value, "value", &wired, "body", &[])
            .expect("the value is wired");
    });
}

fn delete(opened: &mut Opened, handles: &[&str]) {
    let before = model(&opened.session, &opened.id);
    let ids: Vec<String> = handles
        .iter()
        .map(|handle| id_of(&before, handle))
        .collect();
    change(opened, |graph, _| {
        *graph = delete_nodes(graph, &ids.iter().map(String::as_str).collect());
    });
}

fn inspect(opened: &mut Opened) {
    let inspected = id_of(&model(&opened.session, &opened.id), opened.inspected);
    change(opened, |graph, _| {
        let node = graph
            .nodes
            .iter_mut()
            .find(|node| node.id == inspected)
            .unwrap();
        node.inputs.insert(
            "url".into(),
            Binding::Literal {
                value: "/diag06g".into(),
            },
        );
    });
}

fn paste(opened: &mut Opened) {
    let copied = id_of(&model(&opened.session, &opened.id), opened.copied);
    change(opened, |graph, project| {
        *graph = duplicate_selection(graph, project, &HashSet::from([copied.as_str()]))
            .expect("the copy is pasted")
            .graph;
    });
}

/// One Undo gives `before` back byte for byte, one Redo `after`.
fn undone_and_redone(
    session: &mut ProjectSession,
    id: &str,
    before: &str,
    after: &str,
    what: &str,
) {
    assert!(session.undo(id, ORIGIN).unwrap(), "{what}: Undo steps back");
    assert_eq!(
        text(session, id),
        before,
        "{what}: Undo gives the file back"
    );
    assert!(
        session.redo(id, ORIGIN).unwrap(),
        "{what}: Redo steps forward"
    );
    assert_eq!(text(session, id), after, "{what}: Redo gives the edit back");
}

/// Criterion 1 for one board edit, on both files.
fn criterion_1(edit: impl Fn(&mut Opened)) {
    for mut opened in both() {
        let before = text(&opened.session, &opened.id);
        edit(&mut opened);
        let after = text(&opened.session, &opened.id);
        assert_ne!(after, before, "{}: the edit changes the file", opened.name);
        let Opened {
            name, session, id, ..
        } = &mut opened;
        undone_and_redone(session, id, &before, &after, name);
    }
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn diag06g_criterion_1_adding_a_block() {
    criterion_1(add_block);
}

#[test]
fn diag06g_criterion_1_a_step_from_next() {
    criterion_1(add_from_next);
}

#[test]
fn diag06g_criterion_1_wiring_a_value() {
    criterion_1(wire);
}

#[test]
fn diag06g_criterion_1_deleting_a_step_with_its_checks() {
    criterion_1(|opened| {
        let deleted = opened.deleted;
        delete(opened, &deleted);
    });
}

#[test]
fn diag06g_criterion_1_deleting_an_if() {
    criterion_1(|opened| {
        let branch = opened.branch;
        delete(opened, &[branch]);
    });
}

#[test]
fn diag06g_criterion_1_an_inspector_change() {
    criterion_1(inspect);
}

#[test]
fn diag06g_criterion_1_a_paste() {
    criterion_1(paste);
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag06g_criterion_2() {
    for mut opened in both() {
        let mut texts = vec![text(&opened.session, &opened.id)];
        let deleted = opened.deleted;
        delete(&mut opened, &deleted);
        texts.push(text(&opened.session, &opened.id));
        inspect(&mut opened);
        texts.push(text(&opened.session, &opened.id));
        let branch = opened.branch;
        delete(&mut opened, &[branch]);
        texts.push(text(&opened.session, &opened.id));
        let Opened {
            name, session, id, ..
        } = &mut opened;
        for (step, expected) in texts.iter().rev().skip(1).enumerate() {
            assert!(
                session.undo(id, ORIGIN).unwrap(),
                "{name}: Undo {}",
                step + 1
            );
            assert_eq!(&text(session, id), expected, "{name}: Undo {}", step + 1);
        }
        assert!(!session.undo(id, ORIGIN).unwrap(), "{name}: three steps");
        for (step, expected) in texts.iter().skip(1).enumerate() {
            assert!(
                session.redo(id, ORIGIN).unwrap(),
                "{name}: Redo {}",
                step + 1
            );
            assert_eq!(&text(session, id), expected, "{name}: Redo {}", step + 1);
        }
        assert!(!session.redo(id, ORIGIN).unwrap(), "{name}: three steps");
    }
}

// Criterion 3 ----------------------------------------------------------------

/// A delete on the board, then `typed` in Code, settled or not: the first
/// Undo takes back what was typed, the second the delete, and two Redos give
/// both back, the typing last.
fn interleaved(typed: impl Fn(&str) -> String, settled: bool) {
    for mut opened in both() {
        let before = text(&opened.session, &opened.id);
        let deleted = opened.deleted;
        delete(&mut opened, &deleted);
        let after_delete = text(&opened.session, &opened.id);
        let typed = typed(&after_delete);
        assert_ne!(typed, after_delete);
        let Opened {
            name, session, id, ..
        } = &mut opened;
        session.edit_source(id, &typed, CODE).unwrap();
        if settled {
            session.settle(id).unwrap();
            assert_eq!(
                session.document(id).unwrap().analysis(),
                AnalysisState::Valid
            );
        }
        assert!(session.undo(id, ORIGIN).unwrap(), "{name}: the first Undo");
        assert_eq!(
            text(session, id),
            after_delete,
            "{name}: the first Undo takes back what was typed"
        );
        assert!(session.undo(id, ORIGIN).unwrap(), "{name}: the second Undo");
        assert_eq!(
            text(session, id),
            before,
            "{name}: the second Undo the delete"
        );
        assert!(session.redo(id, ORIGIN).unwrap());
        assert_eq!(text(session, id), after_delete, "{name}: the first Redo");
        assert!(session.redo(id, ORIGIN).unwrap());
        assert_eq!(
            text(session, id),
            typed,
            "{name}: the typing comes back last"
        );
        assert!(!session.redo(id, ORIGIN).unwrap(), "{name}");
    }
}

/// A line inserted in front of the line that starts `begin = start`.
fn before_start(text: &str, line: &str) -> String {
    marked(
        text,
        "  begin = start\n",
        &format!("{line}  begin = start\n"),
    )
}

#[test]
fn diag06g_criterion_3_a_comment() {
    interleaved(
        |text| before_start(text, "  // Checked by hand on Monday.\n\n"),
        true,
    );
}

#[test]
fn diag06g_criterion_3_a_blank_line() {
    interleaved(|text| before_start(text, "\n"), true);
}

#[test]
fn diag06g_criterion_3_an_edit_still_being_typed() {
    interleaved(|text| before_start(text, "  // Still being typed\n"), false);
}

/// Typed text that does not analyse is never a step: Undo waits for code
/// that analyses and loses nothing.
#[test]
fn diag06g_criterion_3_an_unfinished_edit_is_not_lost() {
    let mut opened = shop();
    let deleted = opened.deleted;
    delete(&mut opened, &deleted);
    let Opened { session, id, .. } = &mut opened;
    let broken = before_start(&text(session, id), "  half = \n");
    session.edit_source(id, &broken, CODE).unwrap();
    assert!(matches!(session.undo(id, ORIGIN), Err(Error::ModelLocked)));
    assert_eq!(text(session, id), broken, "nothing typed is lost");
}

// Criterion 4 ----------------------------------------------------------------

/// A save of the document held by the test until `meanwhile` has run on the
/// live session.
fn held_save(session: &mut ProjectSession, id: &str, meanwhile: impl FnOnce(&mut ProjectSession)) {
    let snapshot = session.storage_snapshot();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel::<()>();
    let document = id.to_owned();
    let worker = std::thread::spawn(move || {
        snapshot.run(StorageMode::PreserveEdits, |candidate| {
            started.send(()).unwrap();
            resume.recv().unwrap();
            candidate.save(&document)
        })
    });
    ready.recv().unwrap();
    meanwhile(session);
    release.send(()).unwrap();
    session
        .complete_storage(worker.join().unwrap())
        .unwrap()
        .unwrap();
}

/// The history passes through `texts` backwards with Undo and forwards with
/// Redo, and no further either way.
fn passes_through(session: &mut ProjectSession, id: &str, texts: &[String], name: &str) {
    assert_eq!(&text(session, id), texts.last().unwrap(), "{name}");
    for expected in texts.iter().rev().skip(1) {
        assert!(session.undo(id, ORIGIN).unwrap(), "{name}");
        assert_eq!(&text(session, id), expected, "{name}: Undo");
    }
    assert!(!session.undo(id, ORIGIN).unwrap(), "{name}");
    for expected in texts.iter().skip(1) {
        assert!(session.redo(id, ORIGIN).unwrap(), "{name}");
        assert_eq!(&text(session, id), expected, "{name}: Redo");
    }
    assert!(!session.redo(id, ORIGIN).unwrap(), "{name}");
}

#[test]
fn diag06g_criterion_4_an_edit_settled_during_a_save() {
    for mut opened in both() {
        let original = text(&opened.session, &opened.id);
        let deleted = opened.deleted;
        delete(&mut opened, &deleted);
        let saved = text(&opened.session, &opened.id);
        let typed = before_start(&saved, "  // Typed while saving.\n");
        let Opened {
            name,
            session,
            id,
            _directory,
            ..
        } = &mut opened;
        held_save(session, id, |session| {
            session.edit_source(id, &typed, CODE).unwrap();
            session.settle(id).unwrap();
        });
        let path = session
            .document(id)
            .unwrap()
            .entry()
            .path
            .as_str()
            .to_owned();
        assert_eq!(
            fs::read_to_string(_directory.path().join(path)).unwrap(),
            saved,
            "{name}: the file holds the saved text"
        );
        assert!(session.document(id).unwrap().is_dirty(), "{name}");
        passes_through(session, id, &[original, saved, typed], name);
    }
}

#[test]
fn diag06g_criterion_4_typing_goes_on_while_the_typed_text_is_saved() {
    for mut opened in both() {
        let original = text(&opened.session, &opened.id);
        let saved = before_start(&original, "  // Saved while typing.\n");
        let typed = before_start(&saved, "  // Typed while saving.\n");
        let Opened {
            name, session, id, ..
        } = &mut opened;
        // The save settles the text it saves; the live session goes on typing.
        session.edit_source(id, &saved, CODE).unwrap();
        held_save(session, id, |session| {
            session.edit_source(id, &typed, CODE).unwrap();
        });
        assert!(session.document(id).unwrap().is_dirty(), "{name}");
        passes_through(session, id, &[original, saved, typed], name);
    }
}

// Criterion 5 ----------------------------------------------------------------

fn poll(session: &mut ProjectSession) -> ExternalOutcome {
    let changes = session.external_check().run().unwrap();
    session.apply_external(changes).unwrap()
}

/// No step is left to install over the text the document now follows.
fn cleared(session: &mut ProjectSession, id: &str, follows: &str, what: &str) {
    let document = session.document(id).unwrap();
    assert!(!document.can_undo() && !document.can_redo(), "{what}");
    assert!(!session.undo(id, ORIGIN).unwrap(), "{what}");
    assert!(!session.redo(id, ORIGIN).unwrap(), "{what}");
    assert_eq!(text(session, id), follows, "{what}");
}

#[test]
fn diag06g_criterion_5_a_reload() {
    let mut opened = shop();
    let deleted = opened.deleted;
    delete(&mut opened, &deleted);
    let Opened { session, id, .. } = &mut opened;
    session.save(id).unwrap();
    let saved = text(session, id);
    session.reload().unwrap();
    cleared(session, id, &saved, "after a reload");
}

#[test]
fn diag06g_criterion_5_a_foreign_write_taken_in() {
    let mut opened = shop();
    let deleted = opened.deleted;
    delete(&mut opened, &deleted);
    let Opened {
        session,
        id,
        _directory,
        ..
    } = &mut opened;
    session.save(id).unwrap();
    let path = session
        .document(id)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();
    let foreign = before_start(&text(session, id), "  // Written by another editor.\n");
    fs::write(_directory.path().join(path), &foreign).unwrap();
    assert_eq!(poll(session).reloaded, std::slice::from_ref(id));
    cleared(session, id, &foreign, "after a foreign write taken in");
}

#[test]
fn diag06g_criterion_5_a_conflict_resolved() {
    let mut opened = shop();
    let deleted = opened.deleted;
    delete(&mut opened, &deleted);
    let Opened {
        session,
        id,
        _directory,
        ..
    } = &mut opened;
    let path = session
        .document(id)
        .unwrap()
        .entry()
        .path
        .as_str()
        .to_owned();
    let foreign = before_start(&shop_text(), "  // Written by another editor.\n");
    fs::write(_directory.path().join(path), &foreign).unwrap();
    assert_eq!(poll(session).conflicts, std::slice::from_ref(id));
    session.accept_external(id, ORIGIN).unwrap();
    cleared(session, id, &foreign, "after the conflict is resolved");
}

// Criterion 6 ----------------------------------------------------------------

const SMALL: &str = "scenario \"Small\" {\n  total = 2\n}\n";

fn small() -> (tempfile::TempDir, ProjectSession, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Small").unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, "small.afs", SMALL)
        .unwrap();
    (directory, session, id)
}

#[test]
fn diag06g_criterion_6() {
    // The number of entries: a hundred steps back, as today.
    let (_directory, mut session, id) = small();
    for step in 0..105 {
        let typed = SMALL.replace("total = 2", &format!("total = 2\n  // step {step}"));
        session.edit_source(&id, &typed, CODE).unwrap();
        session.settle(&id).unwrap();
    }
    let mut steps = 0;
    while session.undo(&id, ORIGIN).unwrap() {
        steps += 1;
    }
    assert_eq!(steps, 100, "the history keeps a hundred steps");
    assert!(
        text(&session, &id).contains("// step 4\n"),
        "the oldest kept"
    );

    // The source limit: no step holds a text over 1 MiB, from Code or from
    // the board.
    let limit = apiforge_storage::project::MAX_SOURCE_BYTES;
    let (_directory, mut session, id) = small();
    let filler = format!("  // {}\n", "x".repeat(limit - SMALL.len() - 12));
    let near = SMALL.replace("  total = 2\n", &format!("{filler}  total = 2\n"));
    assert!(near.len() <= limit);
    session.edit_source(&id, &near, CODE).unwrap();
    session.settle(&id).unwrap();
    let over = format!("{near}{}", " ".repeat(64));
    assert!(matches!(
        session.edit_source(&id, &over, CODE),
        Err(Error::SourceTooLarge)
    ));
    let mut next = model(&session, &id);
    *main_graph(&mut next) = add_node(
        main_graph(&mut next.clone()),
        Block::Value,
        Position { x: 40.0, y: 40.0 },
        None,
        None,
    )
    .unwrap()
    .graph;
    assert!(matches!(
        session.apply_model_edit(&id, next, ORIGIN),
        Err(Error::SourceTooLarge)
    ));
    assert_eq!(
        text(&session, &id),
        near,
        "the refused edit changes nothing"
    );
    assert!(session.undo(&id, ORIGIN).unwrap());
    assert_eq!(text(&session, &id), SMALL);
    assert!(!session.undo(&id, ORIGIN).unwrap(), "one step only");
}

// Criterion 7 ----------------------------------------------------------------

/// The document after an Undo is the one read from the restored text, with
/// the identities and positions it had (KTLO-26): after a board edit and
/// after a change typed in Code.
#[test]
fn diag06g_criterion_7() {
    for mut opened in both() {
        // A card moved first, so that its position is the person's own.
        let moved = id_of(&model(&opened.session, &opened.id), opened.inspected);
        change(&mut opened, |graph, _| {
            let node = graph
                .nodes
                .iter_mut()
                .find(|node| node.id == moved)
                .unwrap();
            node.position.x += 120.0;
            node.position.y += 80.0;
        });
        let before = model(&opened.session, &opened.id);
        let deleted = opened.deleted;
        delete(&mut opened, &deleted);
        let after = model(&opened.session, &opened.id);
        let typed = before_start(
            &text(&opened.session, &opened.id),
            "  // Typed after the delete.\n\n",
        );
        let Opened {
            name, session, id, ..
        } = &mut opened;
        session.edit_source(id, &typed, CODE).unwrap();
        session.settle(id).unwrap();

        assert!(session.undo(id, ORIGIN).unwrap());
        let document = session.document(id).unwrap();
        assert!(document.model_describes_text(), "{name}");
        assert_eq!(document.model_state(), ModelState::Current, "{name}");
        assert_eq!(document.model(), Some(&after), "{name}: after the typing");

        assert!(session.undo(id, ORIGIN).unwrap());
        let document = session.document(id).unwrap();
        assert!(document.model_describes_text(), "{name}");
        assert_eq!(document.model(), Some(&before), "{name}: after the delete");
        // The same model a fresh analysis of the restored text keeps.
        session.invalidate_runtime_inputs(ORIGIN);
        session.settle(id).unwrap();
        assert_eq!(model(session, id), before, "{name}: read from the text");
    }
}
