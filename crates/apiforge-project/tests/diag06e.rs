//! DIAG-06e (#494): a delete on the board is refused while a statement that
//! stays reads what would go, changes no input of a statement that stays when
//! it is done, and one Undo gives the file back byte for byte.
//!
//! On `labels`, `shop-30` (in the demo shop, whose components it calls) and
//! `rooms/booking-day` (in its own folder project), the last two marked as
//! DIAG-06g marks them: a free comment, blank lines and attributes in a
//! non-canonical order in the statements the deletes take. Every delete ends
//! by checking that the text passes the check and reads back as the document.
use apiforge_core::{
    chain::is_step,
    edit::{apply_deletion, delete_preview},
    format::Graph,
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
    in_the_shop("shop-30", "diag06e/shop-30.afs", &shop_text())
}

fn labels() -> Opened {
    in_the_shop("labels", "diag06e/labels.afs", &fixture("labels.afs"))
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

// The delete as the board makes it --------------------------------------------

fn text(session: &ProjectSession, id: &str) -> String {
    session.document(id).unwrap().text().to_owned()
}

fn model(session: &ProjectSession, id: &str) -> Document {
    session.document(id).unwrap().model().unwrap().clone()
}

fn graphs(document: &Document) -> (&Graph, Option<&Graph>) {
    match document {
        Document::Scenario(scenario) => (&scenario.graph, Some(&scenario.cleanup)),
        Document::Component(component) => (&component.graph, None),
    }
}

fn main_graph_mut(document: &mut Document) -> &mut Graph {
    match document {
        Document::Scenario(scenario) => &mut scenario.graph,
        Document::Component(component) => &mut component.graph,
    }
}

fn id_of(document: &Document, handle: &str) -> String {
    graphs(document)
        .0
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a block `{handle}`"))
        .id
        .clone()
}

/// Deletes `ids` from the main flow as the board does: by the preview,
/// refused with its sentence, or applied as one model edit. A whole-file
/// print that would drop a free comment is accepted, as the person would
/// after the window's question; whether it was asked is returned.
fn delete_ids(
    session: &mut ProjectSession,
    id: &str,
    ids: &[String],
) -> std::result::Result<bool, String> {
    let before = model(session, id);
    let (main, cleanup) = graphs(&before);
    let selection: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let deletion = delete_preview(
        main,
        &cleanup.into_iter().collect::<Vec<_>>(),
        &selection,
        true,
    );
    if let Some(refusal) = deletion.refusal() {
        return Err(refusal);
    }
    let mut next = before.clone();
    *main_graph_mut(&mut next) = apply_deletion(main, &deletion);
    match session.apply_model_edit(id, next.clone(), ORIGIN) {
        Err(Error::CommentsWouldBeLost { .. }) => {
            session.accept_comment_loss(id).unwrap();
            session.apply_model_edit(id, next, ORIGIN).unwrap();
            Ok(true)
        }
        result => result.map(|()| false).map_err(|error| error.to_string()),
    }
}

fn delete(opened: &mut Opened, handles: &[&str]) -> std::result::Result<bool, String> {
    let before = model(&opened.session, &opened.id);
    let ids: Vec<String> = handles
        .iter()
        .map(|handle| id_of(&before, handle))
        .collect();
    delete_ids(&mut opened.session, &opened.id, &ids)
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

/// Every input of every statement that stays, compared binding by binding.
fn untouched(before: &Document, after: &Document, name: &str) {
    let (main, cleanup) = graphs(after);
    let (old_main, old_cleanup) = graphs(before);
    let old: Vec<_> = old_main
        .nodes
        .iter()
        .chain(old_cleanup.into_iter().flat_map(|graph| &graph.nodes))
        .collect();
    for node in main
        .nodes
        .iter()
        .chain(cleanup.into_iter().flat_map(|graph| &graph.nodes))
    {
        let Some(was) = old.iter().find(|old| old.id == node.id) else {
            // A *Start* the written language adds is not a statement that
            // stayed.
            continue;
        };
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

fn handles(document: &Document) -> Vec<String> {
    graphs(document)
        .0
        .nodes
        .iter()
        .map(|node| node.handle.clone())
        .collect()
}

// Criterion 4 ----------------------------------------------------------------

#[test]
fn diag06e_criterion_4_surviving_inputs_are_untouched() {
    let cases: [(fn() -> Opened, &[&[&str]]); 3] = [
        (labels, &[&["no_token"], &["weight_31", "weight_0"]]),
        (
            shop,
            &[&["refund", "refund_amount"], &["paid"], &["logged_in"]],
        ),
        (rooms, &[&["notice"], &["booked"], &["big_enough"]]),
    ];
    for (open, deletes) in cases {
        let mut opened = open();
        for handles_deleted in deletes {
            let before = model(&opened.session, &opened.id);
            delete(&mut opened, handles_deleted).unwrap_or_else(|refusal| {
                panic!("{}: {handles_deleted:?} is deleted: {refusal}", opened.name)
            });
            passes(&mut opened.session, &opened.id, opened.name);
            let after = model(&opened.session, &opened.id);
            for handle in *handles_deleted {
                assert!(
                    !handles(&after).iter().any(|h| h == handle),
                    "{}: `{handle}` is deleted",
                    opened.name
                );
            }
            untouched(&before, &after, opened.name);
        }
    }
}

/// A refused delete changes nothing: not the text, not the model.
#[test]
fn diag06e_criterion_4_a_refused_delete_changes_nothing() {
    for (mut opened, refused) in [
        (shop(), &["cart"][..]),
        (shop(), &["refund"][..]),
        (rooms(), &["hold"][..]),
        (rooms(), &["first"][..]),
    ] {
        let (text_before, before) = (
            text(&opened.session, &opened.id),
            model(&opened.session, &opened.id),
        );
        assert!(
            delete(&mut opened, refused).is_err(),
            "{}: {refused:?} is refused",
            opened.name
        );
        assert_eq!(text(&opened.session, &opened.id), text_before);
        assert_eq!(model(&opened.session, &opened.id), before);
    }
}

// Criterion 7 ----------------------------------------------------------------

/// One Undo after the delete gives the file back byte for byte, one Redo the
/// file after the delete.
fn undone_and_redone(opened: &mut Opened, deleted: &[&str]) -> String {
    let before = text(&opened.session, &opened.id);
    delete(opened, deleted)
        .unwrap_or_else(|refusal| panic!("{}: {deleted:?}: {refusal}", opened.name));
    passes(&mut opened.session, &opened.id, opened.name);
    let after = text(&opened.session, &opened.id);
    assert_ne!(
        after, before,
        "{}: the delete changes the file",
        opened.name
    );
    let Opened {
        name, session, id, ..
    } = opened;
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
    assert_eq!(
        text(session, id),
        after,
        "{name}: Redo gives the delete back"
    );
    after
}

#[test]
fn diag06e_criterion_7_a_step_with_its_checks() {
    // `no_token`'s three checks and `notice`'s `sent` read only their step
    // and go with it; `refund_amount` also reads `expected_total`, so it is
    // selected with `refund`.
    undone_and_redone(&mut labels(), &["no_token"]);
    undone_and_redone(&mut shop(), &["refund", "refund_amount"]);
    undone_and_redone(&mut rooms(), &["notice"]);
}

#[test]
fn diag06e_criterion_7_an_if_with_a_free_comment_blank_lines_and_attributes() {
    for (mut opened, branch) in [(shop(), "paid"), (rooms(), "booked")] {
        let before = text(&opened.session, &opened.id);
        let after = undone_and_redone(&mut opened, &[branch]);
        assert!(
            !after.contains(&format!("{branch} = if")),
            "{}: the if is deleted",
            opened.name
        );
        assert_ne!(after, before);
    }
}

// Criterion 8 ----------------------------------------------------------------

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
/// or their optional body, checks and values of one or two of them, checks
/// and values that read no statement, and `if`s with statements in their
/// braces, nested at most once; sometimes a cleanup.
fn generated(seed: u64) -> String {
    let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut text =
        format!("scenario \"Generated {seed}\" {{\n  param limit = 5\n  begin = start\n");
    let mut number = 0;
    // The requests a statement may read, by braces: what stands before it at
    // its own depth and in every enclosing braces.
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
                    if let Some(read) = pick(random, visible).filter(|_| random.chance(40)) {
                        text.push_str(&format!("{indent}  body {{ id: {read}.body.id }}\n"));
                    }
                    text.push_str(&format!("{indent}  sample 200 {{ id: {n} }}\n{indent}}}\n"));
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
                        block(random, text, number, &mut otherwise, depth + 1, 1);
                    }
                    text.push_str(&format!("{indent}}}\n"));
                }
                _ => text.push_str(&format!("{indent}k{n} = assert 2 == 2\n")),
            }
        }
    }
    let mut visible = vec![];
    let length = 4 + random.below(10);
    block(&mut random, &mut text, &mut number, &mut visible, 0, length);
    if !visible.is_empty() && random.chance(50) {
        let read = visible[random.below(visible.len())].clone();
        text.push_str(&format!(
            "  cleanup {{\n    undo = DELETE \"/undo/{{{{{read}.body.id}}}}\" {{\n      sample 204 {{}}\n    }}\n  }}\n"
        ));
    }
    text.push_str("}\n");
    text
}

#[test]
fn diag06e_criterion_8() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let (mut refused, mut done) = (0, 0);
    for seed in 1..=60u64 {
        let source = generated(seed);
        let id = session
            .create_document(
                DocumentKind::Scenario,
                &format!("diag06e/generated-{seed}.afs"),
                &source,
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}\n{source}"));
        let name = format!("seed {seed}");
        passes(&mut session, &id, &name);
        let mut random = Random(seed | 1);
        for _ in 0..4 {
            let before_text = text(&session, &id);
            let before = model(&session, &id);
            let nodes = &graphs(&before).0.nodes;
            let count = 1 + random.below(3);
            let ids: Vec<String> = (0..count)
                .map(|_| nodes[random.below(nodes.len())].id.clone())
                .collect();
            match delete_ids(&mut session, &id, &ids) {
                Err(_) => {
                    refused += 1;
                    assert_eq!(text(&session, &id), before_text, "{name}: refused");
                    assert_eq!(model(&session, &id), before, "{name}: refused");
                }
                Ok(_) => {
                    done += 1;
                    passes(&mut session, &id, &name);
                    let after = model(&session, &id);
                    untouched(&before, &after, &name);
                    let after_text = text(&session, &id);
                    if after_text != before_text {
                        assert!(session.undo(&id, ORIGIN).unwrap(), "{name}");
                        assert_eq!(text(&session, &id), before_text, "{name}: Undo");
                        assert!(session.redo(&id, ORIGIN).unwrap(), "{name}");
                        assert_eq!(text(&session, &id), after_text, "{name}: Redo");
                    }
                }
            }
            if graphs(&model(&session, &id))
                .0
                .nodes
                .iter()
                .all(|node| !is_step(node.kind))
            {
                break;
            }
        }
    }
    assert!(refused > 10 && done > 10, "refused {refused}, done {done}");
}
