//! DIAG-06a (#391): a step put at a place on the board — moved after a step
//! of its braces, first in the braces of an `if`, or added by *Add step*
//! after the selection — keeps the chain: the text passes the check after
//! every edit, a move moves only the lines of what moved, and each edit is
//! one step of Undo and Redo that gives the file back byte for byte.
//!
//! On `labels`, `shop-30` (in the demo shop, whose components it calls) and
//! `rooms/booking-day` (in its own folder project), the last two marked as
//! DIAG-06g marks them: a free comment, blank lines and attributes in a
//! non-canonical order. Every edit ends by checking that the text passes the
//! check and reads back as the document.
use apiforge_core::{
    block::Block,
    chain::{braces, is_step},
    edit::{OutputRef, Place, add_node, add_step_after, move_by_one, place},
    format::{Graph, Position},
    graph::ORDER_PORT,
    model::{Branch, NodeKind},
};
use apiforge_project::*;
use apiforge_script::{LowerContext, Severity, lower, parse, semantic};
use std::{fs, path::Path};

const ORIGIN: Origin = Origin(7);

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

/// `shop-30` as DIAG-06g marks it: `catalogue` writes `query` before
/// `headers`, `refund` its sample before its headers, blank lines stand
/// between statements and a free comment closes the braces of `paid`.
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
        "      sample 204 {}\n    }\n\n    // Partial refunds are not covered yet.\n  } else {\n",
    )
}

/// `rooms/booking-day` as DIAG-06g marks it: `notice` writes its body before
/// its headers, blank lines stand around `sent` and in the braces of
/// `booked`, and a free comment closes its then-braces.
fn rooms_text() -> String {
    let text = fixture("rooms/scenarios/booking-day.afs");
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
        "    one_booking = assert calendar.body.items[0].id == first.body.id\n    // A third booking is tried by hand.\n",
    )
}

struct Opened {
    name: &'static str,
    _directory: tempfile::TempDir,
    session: ProjectSession,
    id: String,
}

fn in_the_shop(name: &'static str, path: &str, text: &str) -> Opened {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, path, text)
        .unwrap();
    opened(Opened {
        name,
        _directory: directory,
        session,
        id,
    })
}

fn shop() -> Opened {
    in_the_shop("shop-30", "diag06a/shop-30.afs", &shop_text())
}

fn labels() -> Opened {
    in_the_shop("labels", "diag06a/labels.afs", &fixture("labels.afs"))
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
    })
}

/// The file is read as a valid model with its marks still in the text.
fn opened(mut opened: Opened) -> Opened {
    passes(&mut opened.session, &opened.id, opened.name);
    opened
}

// The edits as the board makes them -------------------------------------------

fn text(session: &ProjectSession, id: &str) -> String {
    session.document(id).unwrap().text().to_owned()
}

fn model(session: &ProjectSession, id: &str) -> Document {
    session.document(id).unwrap().model().unwrap().clone()
}

fn main_graph(document: &Document) -> &Graph {
    match document {
        Document::Scenario(scenario) => &scenario.graph,
        Document::Component(component) => &component.graph,
    }
}

fn main_graph_mut(document: &mut Document) -> &mut Graph {
    match document {
        Document::Scenario(scenario) => &mut scenario.graph,
        Document::Component(component) => &mut component.graph,
    }
}

fn id_of(document: &Document, handle: &str) -> String {
    main_graph(document)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a block `{handle}`"))
        .id
        .clone()
}

/// The text passes the check and reads back as the document.
fn passes(session: &mut ProjectSession, id: &str, name: &str) {
    session.settle(id).unwrap();
    let document = session.document(id).unwrap();
    let errors: Vec<_> = document
        .diagnostics()
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "{name}\n{}\n{errors:#?}",
        document.text()
    );
    assert!(!document.text().contains("@after"), "{name}");
    let model = document.model().expect("the text has a model").clone();
    let context = session.context().clone();
    let read = lower(&parse(document.text()), &LowerContext::folder(&context));
    assert!(!read.has_errors(), "{name}: {:#?}", read.diagnostics);
    assert_eq!(
        semantic(&read.document.unwrap(), &context),
        semantic(&model, &context),
        "{name}: the text reads back as the document"
    );
}

/// Moves the step `step` of the main flow to `at` as the board does: by
/// [`place`], refused with its sentence, or applied as one move.
fn move_ids(
    session: &mut ProjectSession,
    id: &str,
    step: &str,
    at: Place<'_>,
) -> std::result::Result<(), String> {
    let before = model(session, id);
    let context = session.context().clone();
    let (graph, moved) =
        place(main_graph(&before), &context, step, at).map_err(|error| error.to_string())?;
    let mut next = before.clone();
    *main_graph_mut(&mut next) = graph;
    session
        .apply_move(id, next, &moved, ORIGIN)
        .map_err(|error| panic!("a move place allows is written: {error}"))
}

fn moved_after(opened: &mut Opened, step: &str, anchor: &str) -> std::result::Result<(), String> {
    let before = model(&opened.session, &opened.id);
    let (step, anchor) = (id_of(&before, step), id_of(&before, anchor));
    move_ids(
        &mut opened.session,
        &opened.id,
        &step,
        Place::After(&anchor),
    )
}

fn moved_first(
    opened: &mut Opened,
    step: &str,
    x: &str,
    branch: Branch,
) -> std::result::Result<(), String> {
    let before = model(&opened.session, &opened.id);
    let (step, x) = (id_of(&before, step), id_of(&before, x));
    move_ids(
        &mut opened.session,
        &opened.id,
        &step,
        Place::First(&x, branch),
    )
}

/// *Add step* on the bar with `selected` selected: a request after the step
/// [`add_step_after`] names, or last in the main flow.
fn add_step_ids(session: &mut ProjectSession, id: &str, selected: &[String]) {
    let before = model(session, id);
    let graph = main_graph(&before);
    let selection: Vec<&str> = selected.iter().map(String::as_str).collect();
    let anchor = add_step_after(graph, &selection).map(str::to_owned);
    let added = add_node(
        graph,
        Block::Request,
        Position { x: 40.0, y: 40.0 },
        None,
        anchor.as_deref().map(|node_id| OutputRef {
            node_id,
            port: ORDER_PORT,
        }),
    )
    .expect("the step is added");
    let mut next = before.clone();
    *main_graph_mut(&mut next) = added.graph;
    match session.apply_model_edit(id, next.clone(), ORIGIN) {
        Err(Error::CommentsWouldBeLost { .. }) => {
            session.accept_comment_loss(id).unwrap();
            session.apply_model_edit(id, next, ORIGIN).unwrap();
        }
        result => result.unwrap(),
    }
}

/// Criterion 2: the file after a move differs from the file before only by
/// lines that stand in another place; nothing is reformatted. Since DIAG-06d
/// a move into other braces also changes the leading indentation of the
/// moved lines, by the difference of depth, and nothing else: the lines are
/// compared without it. Where they stand is checked by reading the text
/// back as the document.
fn only_moved(before: &str, after: &str, name: &str) {
    let lines = |text: &str| -> Vec<String> {
        // An `else` a move opens or empties is written as the writer
        // writes it.
        let mut lines: Vec<String> = text
            .lines()
            .map(|line| line.trim_start().to_owned())
            .filter(|line| line != "} else {")
            .collect();
        lines.sort();
        lines
    };
    assert_ne!(before, after, "{name}: the move changes the file");
    assert_eq!(
        lines(before),
        lines(after),
        "{name}: no line changes but its place and its indentation\n--- before\n{before}\n--- after\n{after}"
    );
}

/// One Undo after an edit gives `before` back byte for byte, one Redo
/// `after`.
fn undone_and_redone(session: &mut ProjectSession, id: &str, before: &str, name: &str) {
    let after = text(session, id);
    assert!(session.undo(id, ORIGIN).unwrap(), "{name}: Undo steps back");
    assert_eq!(
        text(session, id),
        before,
        "{name}: Undo gives the file back"
    );
    assert!(
        session.redo(id, ORIGIN).unwrap(),
        "{name}: Redo steps forward"
    );
    assert_eq!(text(session, id), after, "{name}: Redo gives the edit back");
}

// Criterion 11 ---------------------------------------------------------------

/// The moves of the three files: forwards and backwards in the main flow,
/// in a branch, and first in the braces of an `if`.
enum Move {
    After(&'static str, &'static str),
    First(&'static str, &'static str, Branch),
}

fn move_cases() -> Vec<(fn() -> Opened, Move)> {
    vec![
        (labels, Move::After("weight_0", "no_token")),
        (labels, Move::After("no_token", "postcode_null")),
        (shop, Move::After("address", "cart")),
        (shop, Move::After("cart_view", "address")),
        (shop, Move::After("refund", "payment")),
        (shop, Move::First("logout", "paid", Branch::True)),
        (rooms, Move::After("notice", "first")),
        (rooms, Move::After("equipment", "holidays")),
        (rooms, Move::First("calendar", "booked", Branch::True)),
    ]
}

#[test]
fn diag06a_criterion_11_a_move_is_one_step_of_undo() {
    for (open, case) in move_cases() {
        let mut opened = open();
        let before = text(&opened.session, &opened.id);
        let result = match case {
            Move::After(step, anchor) => moved_after(&mut opened, step, anchor),
            Move::First(step, x, branch) => moved_first(&mut opened, step, x, branch),
        };
        result.unwrap_or_else(|refusal| panic!("{}: the move is done: {refusal}", opened.name));
        passes(&mut opened.session, &opened.id, opened.name);
        let after = text(&opened.session, &opened.id);
        only_moved(&before, &after, opened.name);
        let Opened {
            name, session, id, ..
        } = &mut opened;
        undone_and_redone(session, id, &before, name);
    }
}

#[test]
fn diag06a_criterion_11_add_step_is_one_step_of_undo() {
    for (open, selected) in [
        (labels as fn() -> Opened, &["weight_0"][..]),
        (shop, &["receipt_total"][..]),
        (shop, &["paid"][..]),
        (rooms, &["first", "notice"][..]),
        (rooms, &[][..]),
    ] {
        let mut opened = open();
        let before = text(&opened.session, &opened.id);
        let document = model(&opened.session, &opened.id);
        let ids: Vec<String> = selected
            .iter()
            .map(|handle| id_of(&document, handle))
            .collect();
        add_step_ids(&mut opened.session, &opened.id, &ids);
        passes(&mut opened.session, &opened.id, opened.name);
        assert_ne!(text(&opened.session, &opened.id), before, "{}", opened.name);
        let Opened {
            name, session, id, ..
        } = &mut opened;
        undone_and_redone(session, id, &before, name);
    }
}

/// A refused move changes nothing: not the text, not the model.
#[test]
fn diag06a_criterion_4_a_refused_move_changes_nothing() {
    let mut opened = shop();
    let (before_text, before) = (
        text(&opened.session, &opened.id),
        model(&opened.session, &opened.id),
    );
    // `cart` reads `login` and `catalogue`'s items; `checkout` reads
    // `cart_view`'s condition.
    for (step, anchor) in [("cart", "begin"), ("cart_view", "checkout")] {
        let refusal = moved_after(&mut opened, step, anchor).expect_err("the move is refused");
        assert!(
            refusal.starts_with('`') && refusal.contains("; it stays below `"),
            "{refusal}"
        );
    }
    assert_eq!(text(&opened.session, &opened.id), before_text);
    assert_eq!(model(&opened.session, &opened.id), before);
}

// Criterion 10 ---------------------------------------------------------------

/// A small deterministic generator (xorshift), so that a failing seed
/// can be replayed.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// A generated chain: requests that read earlier requests in their address
/// or their body, checks and values of one or two of them written under
/// them, checks and values that read no statement, and `if`s with
/// statements in their braces, nested at most once, some with empty braces.
fn generated(seed: u64) -> String {
    let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut text =
        format!("scenario \"Generated {seed}\" {{\n  param limit = 5\n  begin = start\n");
    let mut number = 0;
    fn block(
        random: &mut Random,
        text: &mut String,
        number: &mut usize,
        visible: &mut Vec<String>,
        depth: usize,
        length: usize,
    ) {
        let indent = "  ".repeat(depth + 1);
        for _ in 0..length {
            *number += 1;
            let n = *number;
            let pick = |random: &mut Random, visible: &[String]| -> Option<String> {
                (!visible.is_empty()).then(|| visible[random.below(visible.len())].clone())
            };
            match random.below(10) {
                0..=3 => {
                    let handle = format!("r{n}");
                    let address = match pick(random, visible) {
                        Some(read) if random.chance(40) => format!("/r{n}/{{{{{read}.body.id}}}}"),
                        _ => format!("/r{n}"),
                    };
                    text.push_str(&format!("{indent}{handle} = POST \"{address}\" {{\n"));
                    if let Some(read) = pick(random, visible).filter(|_| random.chance(30)) {
                        text.push_str(&format!("{indent}  body {{ id: {read}.body.id }}\n"));
                    }
                    text.push_str(&format!("{indent}  sample 200 {{ id: {n} }}\n{indent}}}\n"));
                    if random.chance(50) {
                        text.push_str(&format!(
                            "{indent}{handle}_ok = assert {handle}.status == 200\n"
                        ));
                    }
                    visible.push(handle);
                }
                4 | 5 => {
                    if let Some(read) = pick(random, visible) {
                        let other = pick(random, visible).filter(|_| random.chance(30));
                        let right =
                            other.map_or("200".to_owned(), |other| format!("{other}.status"));
                        text.push_str(&format!("{indent}c{n} = assert {read}.status == {right}\n"));
                    } else {
                        text.push_str(&format!("{indent}c{n} = assert 1 == 1\n"));
                    }
                }
                6 => match pick(random, visible) {
                    Some(read) => text.push_str(&format!("{indent}v{n} = {read}.body.id\n")),
                    None => text.push_str(&format!("{indent}v{n} = param.limit\n")),
                },
                7 => text.push_str(&format!("{indent}l{n} = 30\n")),
                _ if depth < 2 => {
                    let condition = pick(random, visible)
                        .map_or("1 == 1".to_owned(), |read| format!("{read}.status == 200"));
                    text.push_str(&format!("{indent}x{n} = if {condition} {{\n"));
                    let inner = random.below(4);
                    let mut inside = visible.clone();
                    block(random, text, number, &mut inside, depth + 1, inner);
                    if random.chance(30) {
                        text.push_str(&format!("{indent}}} else {{\n"));
                        let mut otherwise = visible.clone();
                        block(random, text, number, &mut otherwise, depth + 1, 2);
                    }
                    text.push_str(&format!("{indent}}}\n"));
                }
                _ => text.push_str(&format!("{indent}k{n} = assert 2 == 2\n")),
            }
        }
    }
    let mut visible = vec![];
    let length = 5 + random.below(10);
    block(&mut random, &mut text, &mut number, &mut visible, 0, length);
    text.push_str("}\n");
    text
}

#[test]
fn diag06a_criterion_10_edits_keep_the_check() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let (mut refused, mut moved, mut added) = (0, 0, 0);
    for seed in 1..=60u64 {
        let source = generated(seed);
        let id = session
            .create_document(
                DocumentKind::Scenario,
                &format!("diag06a/generated-{seed}.afs"),
                &source,
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}\n{source}"));
        let name = format!("seed {seed}");
        passes(&mut session, &id, &name);
        let mut random = Random(seed | 1);
        for _ in 0..6 {
            let before_text = text(&session, &id);
            let before = model(&session, &id);
            let graph = main_graph(&before);
            let steps: Vec<&str> = graph
                .nodes
                .iter()
                .filter(|node| is_step(node.kind) && node.kind != NodeKind::Start)
                .map(|node| node.id.as_str())
                .collect();
            if steps.is_empty() || random.chance(25) {
                // *Add step* with up to two statements selected.
                let count = random.below(3);
                let selected: Vec<String> = (0..count)
                    .map(|_| graph.nodes[random.below(graph.nodes.len())].id.clone())
                    .collect();
                add_step_ids(&mut session, &id, &selected);
                added += 1;
                passes(&mut session, &id, &name);
                undone_and_redone(&mut session, &id, &before_text, &name);
                continue;
            }
            let step = steps[random.below(steps.len())].to_owned();
            let ifs: Vec<&str> = graph
                .nodes
                .iter()
                .filter(|node| node.kind == NodeKind::If)
                .map(|node| node.id.as_str())
                .collect();
            let anchors: Vec<&str> = graph
                .nodes
                .iter()
                .filter(|node| is_step(node.kind) && node.id != step)
                .map(|node| node.id.as_str())
                .collect();
            // A place in the step's own braces most of the time.
            let own = braces(graph).get(step.as_str()).copied();
            let same: Vec<&str> = anchors
                .iter()
                .copied()
                .filter(|anchor| braces(graph).get(anchor).copied() == own)
                .collect();
            let result = if !ifs.is_empty() && random.chance(20) {
                let x = ifs[random.below(ifs.len())].to_owned();
                let branch = if random.chance(50) {
                    Branch::True
                } else {
                    Branch::False
                };
                move_ids(&mut session, &id, &step, Place::First(&x, branch))
            } else {
                let pool = if !same.is_empty() && random.chance(80) {
                    &same
                } else {
                    &anchors
                };
                if pool.is_empty() {
                    continue;
                }
                let anchor = pool[random.below(pool.len())].to_owned();
                move_ids(&mut session, &id, &step, Place::After(&anchor))
            };
            match result {
                Err(_) => {
                    refused += 1;
                    assert_eq!(text(&session, &id), before_text, "{name}: refused");
                    assert_eq!(model(&session, &id), before, "{name}: refused");
                }
                Ok(()) => {
                    passes(&mut session, &id, &name);
                    let after_text = text(&session, &id);
                    if after_text != before_text {
                        moved += 1;
                        only_moved(&before_text, &after_text, &name);
                        undone_and_redone(&mut session, &id, &before_text, &name);
                    }
                }
            }
        }
    }
    assert!(
        refused > 20 && moved > 60 && added > 20,
        "refused {refused}, moved {moved}, added {added}"
    );
}

/// DIAG-06b, criterion 5: *Move earlier* and *Move later* on generated
/// chains with `if`s leave a text that passes the check after each move,
/// move only the lines of what moved, and are one step of Undo and Redo;
/// a refused one is never applied.
#[test]
fn diag06b_criterion_5_moves_by_one_keep_the_check() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let (mut refused, mut moved) = (0, 0);
    for seed in 1..=40u64 {
        let source = generated(seed);
        let id = session
            .create_document(
                DocumentKind::Scenario,
                &format!("diag06b/generated-{seed}.afs"),
                &source,
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}\n{source}"));
        let name = format!("seed {seed}");
        passes(&mut session, &id, &name);
        let mut random = Random(seed | 1);
        for _ in 0..8 {
            let before_text = text(&session, &id);
            let before = model(&session, &id);
            let graph = main_graph(&before);
            let steps: Vec<&str> = graph
                .nodes
                .iter()
                .filter(|node| is_step(node.kind) && node.kind != NodeKind::Start)
                .map(|node| node.id.as_str())
                .collect();
            if steps.is_empty() {
                break;
            }
            let step = steps[random.below(steps.len())].to_owned();
            let later = random.chance(50);
            let context = session.context().clone();
            match move_by_one(graph, &context, &step, later) {
                Err(_) => refused += 1,
                Ok((graph, lines)) => {
                    let mut next = before.clone();
                    *main_graph_mut(&mut next) = graph;
                    session
                        .apply_move(&id, next, &lines, ORIGIN)
                        .unwrap_or_else(|error| {
                            panic!("{name}: a move by one is written: {error}")
                        });
                    passes(&mut session, &id, &name);
                    let after_text = text(&session, &id);
                    assert_ne!(after_text, before_text, "{name}: a move by one moves");
                    moved += 1;
                    only_moved(&before_text, &after_text, &name);
                    undone_and_redone(&mut session, &id, &before_text, &name);
                }
            }
        }
    }
    assert!(
        refused > 10 && moved > 60,
        "refused {refused}, moved {moved}"
    );
}
