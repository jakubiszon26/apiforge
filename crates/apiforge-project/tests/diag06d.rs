//! DIAG-06d (#493): an `if` moves with its braces and a statement moves
//! across braces — into a branch, out of one, from *Yes* to *No*; a check
//! goes into a branch from *Yes* or *No* and out of it to after the `if`.
//! After every move the text passes the check, the moved lines are the only
//! change besides their indentation, every statement stands in the braces
//! the move names with every input as it was, and one Undo gives the file
//! back byte for byte.
//!
//! On `shop-30` (in the demo shop, whose components it calls) and
//! `rooms/booking-day` (in its own folder project), the last two marked as
//! DIAG-06g marks them: a free comment, blank lines and attributes in a
//! non-canonical order. Every edit ends by checking that the text passes the
//! check and reads back as the document.
use apiforge_core::{
    chain::{braces, is_step},
    edit::{Place, place},
    format::Graph,
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
    in_the_shop("shop-30", "diag06d/shop-30.afs", &shop_text())
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

/// The file after a move differs from the file before only by lines that
/// stand in another place, the same lines in the same order apart from
/// their leading indentation; nothing is reformatted.
fn only_moved(before: &str, after: &str, name: &str) {
    let trimmed = |text: &str| -> Vec<String> {
        text.lines()
            .map(|line| line.trim_start().to_owned())
            .collect()
    };
    let (a, b) = (trimmed(before), trimmed(after));
    let mut sorted_a = a.clone();
    let mut sorted_b = b.clone();
    sorted_a.sort();
    sorted_b.sort();
    assert_eq!(
        sorted_a, sorted_b,
        "{name}: no line changes but its indentation\n--- before\n{before}\n--- after\n{after}"
    );
    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let (mut removed, mut added) = (vec![], vec![]);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            removed.push(a[i].as_str());
            i += 1;
        } else {
            added.push(b[j].as_str());
            j += 1;
        }
    }
    removed.extend(a[i..].iter().map(String::as_str));
    added.extend(b[j..].iter().map(String::as_str));
    assert_eq!(removed, added, "{name}: only whole lines move");
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

/// The braces a statement stands in, by handle.
fn braced(document: &Document, handle: &str) -> Option<(String, Branch)> {
    let graph = main_graph(document);
    braces(graph)
        .get(id_of(document, handle).as_str())
        .map(|&(x, branch)| {
            let x = graph.nodes.iter().find(|node| node.id == x).unwrap();
            (x.handle.clone(), branch)
        })
}

// Criteria 6 and 8 ------------------------------------------------------------

enum Move {
    After(&'static str, &'static str),
    First(&'static str, &'static str, Branch),
}

/// Moves across braces, of an `if` with its braces and of checks, on the
/// three files, with the braces the statement stands in after.
fn move_cases() -> Vec<(fn() -> Opened, Move, Option<(&'static str, Branch)>)> {
    vec![
        (shop, Move::After("logout", "order"), None),
        (
            shop,
            Move::First("address", "paid", Branch::True),
            Some(("paid", Branch::True)),
        ),
        (
            shop,
            Move::After("refund", "not_created"),
            Some(("paid", Branch::False)),
        ),
        (
            shop,
            Move::First("checkout_status", "paid", Branch::False),
            Some(("paid", Branch::False)),
        ),
        (shop, Move::After("receipt_total", "paid"), None),
        (rooms, Move::After("booked", "first"), None),
        (
            rooms,
            Move::First("notice", "booked", Branch::False),
            Some(("booked", Branch::False)),
        ),
        (rooms, Move::After("calendar", "notice"), None),
    ]
}

#[test]
fn diag06d_criterion_6_one_undo_gives_the_file_back() {
    for (open, case, expected) in move_cases() {
        let mut opened = open();
        let before = text(&opened.session, &opened.id);
        let (result, step) = match case {
            Move::After(step, anchor) => (moved_after(&mut opened, step, anchor), step),
            Move::First(step, x, branch) => (moved_first(&mut opened, step, x, branch), step),
        };
        result.unwrap_or_else(|refusal| panic!("{}: `{step}` moves: {refusal}", opened.name));
        passes(&mut opened.session, &opened.id, opened.name);
        let after = text(&opened.session, &opened.id);
        only_moved(&before, &after, opened.name);
        let document = model(&opened.session, &opened.id);
        assert_eq!(
            braced(&document, step),
            expected.map(|(x, branch)| (x.to_owned(), branch)),
            "{}: `{step}` stands in the braces the move names",
            opened.name
        );
        let Opened {
            name, session, id, ..
        } = &mut opened;
        undone_and_redone(session, id, &before, name);
    }
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

/// Every input of every statement, by identifier.
fn inputs(document: &Document) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = main_graph(document)
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.clone(),
                serde_json::to_string(&node.inputs).expect("inputs serialize"),
            )
        })
        .collect();
    all.sort();
    all
}

#[test]
fn diag06d_criterion_7_moves_across_braces_keep_the_check() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let (mut refused, mut moved, mut across, mut ifs_moved, mut data_moved) = (0, 0, 0, 0, 0);
    for seed in 1..=60u64 {
        let source = generated(seed);
        let id = session
            .create_document(
                DocumentKind::Scenario,
                &format!("diag06d/generated-{seed}.afs"),
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
            let written = braces(graph);
            let statements: Vec<&str> = graph
                .nodes
                .iter()
                .filter(|node| node.kind != NodeKind::Start)
                .map(|node| node.id.as_str())
                .collect();
            if statements.is_empty() {
                break;
            }
            let statement = statements[random.below(statements.len())].to_owned();
            let node = graph
                .nodes
                .iter()
                .find(|node| node.id == statement)
                .unwrap();
            let ifs: Vec<&str> = graph
                .nodes
                .iter()
                .filter(|node| node.kind == NodeKind::If && node.id != statement)
                .map(|node| node.id.as_str())
                .collect();
            let steps: Vec<&str> = graph
                .nodes
                .iter()
                .filter(|node| is_step(node.kind) && node.id != statement)
                .map(|node| node.id.as_str())
                .collect();
            let own = written.get(statement.as_str()).copied();
            // The place, and the braces it names.
            let (at, expected): (Place<'_>, Option<(&str, Branch)>) = if !is_step(node.kind) {
                match own {
                    Some((x, _)) if random.chance(40) => (Place::After(x), written.get(x).copied()),
                    _ if !ifs.is_empty() => {
                        let x = ifs[random.below(ifs.len())];
                        let branch = if random.chance(50) {
                            Branch::True
                        } else {
                            Branch::False
                        };
                        (Place::First(x, branch), Some((x, branch)))
                    }
                    _ => continue,
                }
            } else if !ifs.is_empty() && random.chance(25) {
                let x = ifs[random.below(ifs.len())];
                let branch = if random.chance(50) {
                    Branch::True
                } else {
                    Branch::False
                };
                (Place::First(x, branch), Some((x, branch)))
            } else if !steps.is_empty() {
                let anchor = steps[random.below(steps.len())];
                (Place::After(anchor), written.get(anchor).copied())
            } else {
                continue;
            };
            let expected: Option<(String, Branch)> = expected.map(|(x, branch)| {
                let x = graph.nodes.iter().find(|node| node.id == x).unwrap();
                (x.handle.clone(), branch)
            });
            let handle = node.handle.clone();
            let kind = node.kind;
            let crosses = own
                != match at {
                    Place::After(anchor) => written.get(anchor).copied(),
                    Place::First(x, branch) => Some((x, branch)),
                };
            let result = move_ids(&mut session, &id, &statement, at);
            match result {
                Err(_) => {
                    refused += 1;
                    assert_eq!(text(&session, &id), before_text, "{name}: refused");
                    assert_eq!(model(&session, &id), before, "{name}: refused");
                }
                Ok(()) => {
                    passes(&mut session, &id, &name);
                    let after = model(&session, &id);
                    assert_eq!(
                        braced(&after, &handle),
                        expected,
                        "{name}: `{handle}` stands in the braces the move names"
                    );
                    assert_eq!(
                        inputs(&after),
                        inputs(&before),
                        "{name}: every input is kept"
                    );
                    let after_text = text(&session, &id);
                    if after_text != before_text {
                        moved += 1;
                        across += usize::from(crosses);
                        ifs_moved += usize::from(kind == NodeKind::If);
                        data_moved += usize::from(!is_step(kind));
                        only_moved(&before_text, &after_text, &name);
                        undone_and_redone(&mut session, &id, &before_text, &name);
                    }
                }
            }
        }
    }
    assert!(
        refused > 20 && moved > 60 && across > 30 && ifs_moved > 5 && data_moved > 10,
        "refused {refused}, moved {moved}, across {across}, ifs {ifs_moved}, data {data_moved}"
    );
}
