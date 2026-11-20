//! DIAG-06f (#496): pasted and duplicated statements land in the chain, in
//! the braces of the place, and keep what they read; every input of a
//! statement that was there before is unchanged; one Undo gives the file
//! back byte for byte.
//!
//! On `labels`, `shop-30` (in the demo shop, whose components it calls) and
//! `rooms/booking-day` (in its own folder project), the last two marked as
//! DIAG-06g marks them: a free comment, blank lines and attributes in a
//! non-canonical order. Every edit ends by checking that the text passes the
//! check and reads back as the document.
use apiforge_core::{
    chain::{braces, is_step},
    edit::{
        Fragment, PasteTarget, Place, add_step_after, copy_statements, duplicate_statements,
        paste_statements, place,
    },
    format::Graph,
    model::{Branch, NodeKind},
};
use apiforge_project::*;
use apiforge_script::{LowerContext, Severity, lower, parse, semantic};
use std::{collections::HashSet, fs, path::Path};

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
    in_the_shop("shop-30", "diag06f/shop-30.afs", &shop_text())
}

fn labels() -> Opened {
    in_the_shop("labels", "diag06f/labels.afs", &fixture("labels.afs"))
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

fn scenario(document: &Document) -> &apiforge_core::format::Scenario {
    match document {
        Document::Scenario(scenario) => scenario,
        Document::Component(_) => panic!("a scenario"),
    }
}

fn main_graph_mut(document: &mut Document) -> &mut Graph {
    match document {
        Document::Scenario(scenario) => &mut scenario.graph,
        Document::Component(component) => &mut component.graph,
    }
}

fn id_of(document: &Document, handle: &str) -> String {
    scenario(document)
        .graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a block `{handle}`"))
        .id
        .clone()
}

fn ids_of(document: &Document, handles: &[&str]) -> Vec<String> {
    handles
        .iter()
        .map(|handle| id_of(document, handle))
        .collect()
}

/// Installs `next` as the board installs a paste or a duplicate: one model
/// edit whose chain the edit made. A whole-file print that would drop a free
/// comment is accepted, as the person would after the window's question.
fn install(session: &mut ProjectSession, id: &str, next: Document) {
    match session.apply_pasted(id, next.clone(), ORIGIN) {
        Err(Error::CommentsWouldBeLost { .. }) => {
            session.accept_comment_loss(id).unwrap();
            session.apply_pasted(id, next, ORIGIN).unwrap();
        }
        result => result.unwrap_or_else(|error| panic!("a paste is written: {error}")),
    }
}

/// *Duplicate* of `ids` in the main flow, as the board does it: refused
/// with its sentence, or applied as one edit. The copies are returned.
fn duplicate_ids(
    session: &mut ProjectSession,
    id: &str,
    ids: &[String],
) -> std::result::Result<Vec<String>, String> {
    let before = model(session, id);
    let context = session.context().clone();
    let main = &scenario(&before).graph;
    let cleanup = &scenario(&before).cleanup;
    let parameters: Vec<String> = scenario(&before).parameters.keys().cloned().collect();
    let target = PasteTarget {
        graph: main,
        visible: &[],
        reserved: &[cleanup],
        parameters: Some(&parameters),
        project: &context,
        component: false,
    };
    let selection: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let pasted = duplicate_statements(&target, &selection).map_err(|error| error.to_string())?;
    let mut next = before.clone();
    *main_graph_mut(&mut next) = pasted.graph;
    install(session, id, next);
    Ok(pasted.ids)
}

/// The board's *Copy* of `ids` in the main flow.
fn copy_ids(session: &ProjectSession, id: &str, ids: &[String]) -> Fragment {
    let before = model(session, id);
    let main = &scenario(&before).graph;
    let selection: HashSet<&str> = ids.iter().map(String::as_str).collect();
    copy_statements(main, main, &selection)
}

/// The board's *Paste* of `fragment` into the main flow with `selected`
/// selected: after the step *Add step* names, else last.
fn paste_ids(
    session: &mut ProjectSession,
    id: &str,
    fragment: &Fragment,
    selected: &[String],
) -> std::result::Result<Vec<String>, String> {
    let before = model(session, id);
    let context = session.context().clone();
    let main = &scenario(&before).graph;
    let cleanup = &scenario(&before).cleanup;
    let parameters: Vec<String> = scenario(&before).parameters.keys().cloned().collect();
    let target = PasteTarget {
        graph: main,
        visible: &[],
        reserved: &[cleanup],
        parameters: Some(&parameters),
        project: &context,
        component: false,
    };
    let selection: Vec<&str> = selected.iter().map(String::as_str).collect();
    let after = add_step_after(main, &selection);
    let pasted = paste_statements(&target, fragment, after).map_err(|error| error.to_string())?;
    let mut next = before.clone();
    *main_graph_mut(&mut next) = pasted.graph;
    install(session, id, next);
    Ok(pasted.ids)
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

/// Every input of every statement that was there before, compared binding
/// by binding.
fn untouched(before: &Document, after: &Document, name: &str) {
    let (before, after) = (scenario(before), scenario(after));
    let old: Vec<_> = before
        .graph
        .nodes
        .iter()
        .chain(&before.cleanup.nodes)
        .collect();
    let new: Vec<_> = after
        .graph
        .nodes
        .iter()
        .chain(&after.cleanup.nodes)
        .collect();
    for was in old {
        let node = new
            .iter()
            .find(|node| node.id == was.id)
            .unwrap_or_else(|| panic!("{name}: `{}` stays", was.handle));
        assert_eq!(
            was.inputs.keys().collect::<Vec<_>>(),
            node.inputs.keys().collect::<Vec<_>>(),
            "{name}: the inputs of `{}`",
            node.handle
        );
        for (input, binding) in &was.inputs {
            assert_eq!(
                Some(binding),
                node.inputs.get(input),
                "{name}: `{}`.{input} is unchanged",
                node.handle
            );
        }
    }
}

/// The braces of the statement `id` of `graph`, by node identifiers.
fn braces_of(graph: &Graph, id: &str) -> Option<(String, Branch)> {
    braces(graph)
        .get(id)
        .map(|(x, branch)| ((*x).to_owned(), *branch))
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

// Criterion 6 ----------------------------------------------------------------

enum Edit {
    /// *Duplicate* of the statements.
    Duplicate(&'static [&'static str]),
    /// *Copy* of the first statements, *Paste* with the second selected.
    Paste(&'static [&'static str], &'static [&'static str]),
}

fn cases() -> Vec<(fn() -> Opened, Edit)> {
    vec![
        (labels, Edit::Duplicate(&["no_token"])),
        (labels, Edit::Duplicate(&["weight_31", "weight_0_status"])),
        (labels, Edit::Paste(&["weight_0"], &["no_token"])),
        (shop, Edit::Duplicate(&["cart"])),
        (shop, Edit::Duplicate(&["receipt", "not_created"])),
        (shop, Edit::Duplicate(&["paid"])),
        (shop, Edit::Paste(&["cart_view"], &["address"])),
        (shop, Edit::Paste(&["payment", "receipt"], &["order"])),
        (shop, Edit::Paste(&["checkout_total"], &["refund"])),
        (shop, Edit::Paste(&["paid"], &[])),
        (rooms, Edit::Duplicate(&["notice"])),
        (rooms, Edit::Duplicate(&["booked"])),
        (rooms, Edit::Paste(&["calendar", "one_booking"], &["sent"])),
        (rooms, Edit::Paste(&["invite"], &["second"])),
    ]
}

#[test]
fn diag06f_criterion_6() {
    for (open, edit) in cases() {
        let mut opened = open();
        let before = model(&opened.session, &opened.id);
        let main = &scenario(&before).graph;
        let written = braces(main);
        let Opened {
            name, session, id, ..
        } = &mut opened;
        // Where each copy is expected: in the braces of its original for a
        // duplicate, in the braces of the place for a paste; inside a copied
        // `if`, in the copy of its braces.
        let (copies, originals, place) = match edit {
            Edit::Duplicate(handles) => {
                let ids = ids_of(&before, handles);
                let copies = duplicate_ids(session, id, &ids)
                    .unwrap_or_else(|refusal| panic!("{name}: {handles:?}: {refusal}"));
                (copies, None, None)
            }
            Edit::Paste(copied, selected) => {
                let fragment = copy_ids(session, id, &ids_of(&before, copied));
                let selected = ids_of(&before, selected);
                let selection: Vec<&str> = selected.iter().map(String::as_str).collect();
                let place = add_step_after(main, &selection)
                    .and_then(|anchor| written.get(anchor).copied())
                    .map(|(x, branch)| (x.to_owned(), branch));
                let copies = paste_ids(session, id, &fragment, &selected)
                    .unwrap_or_else(|refusal| panic!("{name}: {copied:?}: {refusal}"));
                (copies, Some(fragment), place)
            }
        };
        passes(session, id, name);
        let after = model(session, id);
        untouched(&before, &after, name);
        let graph = &scenario(&after).graph;
        let handle = |id: &str| {
            graph
                .nodes
                .iter()
                .chain(&main.nodes)
                .find(|node| node.id == id)
                .map(|node| node.handle.clone())
                .unwrap()
        };
        for copy in &copies {
            let copy_handle = handle(copy);
            let stands = braces_of(graph, copy);
            // A copy inside a copied `if` stands in a copy's braces.
            if let Some((x, _)) = &stands
                && copies.contains(x)
            {
                continue;
            }
            let expected = match &originals {
                // The original of a duplicate is named as the copy without
                // its number.
                None => {
                    let original = copy_handle
                        .strip_suffix("_2")
                        .unwrap_or_else(|| panic!("{name}: `{copy_handle}` is numbered"));
                    let original = main
                        .nodes
                        .iter()
                        .find(|node| node.handle == original)
                        .unwrap();
                    braces_of(main, &original.id)
                }
                Some(_) => place.clone(),
            };
            assert_eq!(
                stands.map(|(x, branch)| (handle(&x), branch)),
                expected.map(|(x, branch)| (handle(&x), branch)),
                "{name}: `{copy_handle}` stands in the braces of the place"
            );
        }
    }
}

/// A refused paste or duplicate changes nothing: not the text, not the
/// model, not the history.
#[test]
fn diag06f_criterion_5_a_refused_paste_changes_nothing() {
    let mut opened = shop();
    let (before_text, before) = (
        text(&opened.session, &opened.id),
        model(&opened.session, &opened.id),
    );
    let Opened {
        session, id, name, ..
    } = &mut opened;
    let fragment = copy_ids(session, id, &ids_of(&before, &["cart_view"]));
    let refusal = paste_ids(session, id, &fragment, &ids_of(&before, &["login"]))
        .expect_err("the paste is refused");
    assert!(
        refusal.contains("runs after this place"),
        "{name}: {refusal}"
    );
    assert_eq!(text(session, id), before_text);
    assert_eq!(model(session, id), before);
    assert!(
        !session.document(id).unwrap().can_undo(),
        "{name}: no step of Undo"
    );
}

// Criterion 7 ----------------------------------------------------------------

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

/// DIAG-06a's property test, with *Duplicate* and *Copy* and *Paste* among
/// its edits: every edit passes the check, a refused one changes nothing,
/// and one Undo gives the file back byte for byte.
#[test]
fn diag06f_criterion_7() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let (mut refused, mut moved, mut duplicated, mut pasted) = (0, 0, 0, 0);
    for seed in 1..=60u64 {
        let source = generated(seed);
        let id = session
            .create_document(
                DocumentKind::Scenario,
                &format!("diag06f/generated-{seed}.afs"),
                &source,
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}\n{source}"));
        let name = format!("seed {seed}");
        passes(&mut session, &id, &name);
        let mut random = Random(seed | 1);
        for _ in 0..6 {
            let before_text = text(&session, &id);
            let before = model(&session, &id);
            let graph = &scenario(&before).graph;
            let statements: Vec<&str> = graph
                .nodes
                .iter()
                .filter(|node| node.kind != NodeKind::Start)
                .map(|node| node.id.as_str())
                .collect();
            if statements.is_empty() {
                break;
            }
            let pick = |random: &mut Random, count: usize| -> Vec<String> {
                (0..count)
                    .map(|_| statements[random.below(statements.len())].to_owned())
                    .collect()
            };
            let result = match random.below(3) {
                0 => {
                    let count = 1 + random.below(2);
                    let selected = pick(&mut random, count);
                    duplicate_ids(&mut session, &id, &selected).map(|_| duplicated += 1)
                }
                1 => {
                    let count = 1 + random.below(2);
                    let copied = pick(&mut random, count);
                    let fragment = copy_ids(&session, &id, &copied);
                    let count = random.below(3);
                    let selected = pick(&mut random, count);
                    paste_ids(&mut session, &id, &fragment, &selected).map(|_| pasted += 1)
                }
                _ => {
                    let steps: Vec<&str> = graph
                        .nodes
                        .iter()
                        .filter(|node| is_step(node.kind) && node.kind != NodeKind::Start)
                        .map(|node| node.id.as_str())
                        .collect();
                    if steps.len() < 2 {
                        continue;
                    }
                    let step = steps[random.below(steps.len())].to_owned();
                    let anchor = steps[random.below(steps.len())].to_owned();
                    if anchor == step {
                        continue;
                    }
                    let context = session.context().clone();
                    match place(graph, &context, &step, Place::After(&anchor)) {
                        Err(error) => Err(error.to_string()),
                        Ok((graph, moving)) => {
                            let mut next = before.clone();
                            *main_graph_mut(&mut next) = graph;
                            session
                                .apply_move(&id, next, &moving, ORIGIN)
                                .unwrap_or_else(|error| {
                                    panic!("{name}: a move is written: {error}")
                                });
                            moved += 1;
                            Ok(())
                        }
                    }
                }
            };
            match result {
                Err(_) => {
                    refused += 1;
                    assert_eq!(text(&session, &id), before_text, "{name}: refused");
                    assert_eq!(model(&session, &id), before, "{name}: refused");
                }
                Ok(()) => {
                    passes(&mut session, &id, &name);
                    let after = model(&session, &id);
                    untouched(&before, &after, &name);
                    if text(&session, &id) != before_text {
                        undone_and_redone(&mut session, &id, &before_text, &name);
                    }
                }
            }
        }
    }
    assert!(
        duplicated > 40 && pasted > 30 && refused > 5 && moved > 10,
        "duplicated {duplicated}, pasted {pasted}, refused {refused}, moved {moved}"
    );
}
