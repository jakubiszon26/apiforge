//! LAY-26 (#341): the Code view warns where a far step reads a value of a
//! request or a component call directly, with *Apply fix*, which saves the
//! value in a variable as LAY-25's *Save as variable* does. Far is LAY-25's
//! rule read off the document (`edit::far_reads`). On `far-value` and
//! `shop-30`; the command line's side is in `apiforge-cli/tests/folder.rs`,
//! MCP's in `apiforge-mcp/tests/acceptance.rs`.
use super::*;
use crate::testing::MINIMUM;
use apiforge_core::{
    edit::{self, ValueRef},
    format::{Binding, Graph, Project},
};
use apiforge_script::{Diagnostic, FAR_READ, LowerContext, Severity};
use egui_kittest::kittest::{NodeT, Queryable};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";

/// The three values `far-value` reads far: as code writes them, with the
/// step, output and path they name (`add_to_cart` has one output, `cart`).
const VALUES: [(&str, &str, &str, &str); 3] = [
    ("cart.id", "cart", "cart", "id"),
    ("cart.productId", "cart", "cart", "productId"),
    ("session.body.currency", "session", "body", "currency"),
];

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// `source` as the demo shop's scenario, open in `mode` at `size`.
fn opened(
    source: &str,
    size: egui::Vec2,
    mode: Mode,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, mode);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    (project, harness, id)
}

fn in_code(source: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    opened(source, size, Mode::Code)
}

fn diagnostics(harness: &FolderHarness, id: &str) -> Vec<Diagnostic> {
    folder(harness).snapshot(id).unwrap().diagnostics
}

/// The far-read warnings, with the index each has in the problems list.
fn warnings(harness: &FolderHarness, id: &str) -> Vec<(usize, Diagnostic)> {
    diagnostics(harness, id)
        .into_iter()
        .enumerate()
        .filter(|(_, diagnostic)| diagnostic.message == FAR_READ)
        .collect()
}

/// The ranges of the far-read warnings.
fn warned(harness: &FolderHarness, id: &str) -> BTreeSet<(usize, usize)> {
    warnings(harness, id)
        .into_iter()
        .map(|(_, diagnostic)| (diagnostic.range.start, diagnostic.range.end))
        .collect()
}

fn model(harness: &FolderHarness, id: &str) -> Arc<apiforge_script::Document> {
    folder(harness)
        .snapshot(id)
        .unwrap()
        .model
        .expect("a valid model")
}

fn context(harness: &FolderHarness) -> Arc<Project> {
    folder(harness).context_project()
}

fn graphs(document: &apiforge_script::Document) -> (&Graph, Option<&Graph>) {
    match document {
        apiforge_script::Document::Scenario(scenario) => (&scenario.graph, Some(&scenario.cleanup)),
        apiforge_script::Document::Component(component) => (&component.graph, None),
    }
}

/// The value a row of [`VALUES`] names, in `document`'s identities.
fn value_of(document: &apiforge_script::Document, row: (&str, &str, &str, &str)) -> ValueRef {
    let (main, cleanup) = graphs(document);
    let source = main
        .nodes
        .iter()
        .chain(cleanup.into_iter().flat_map(|graph| &graph.nodes))
        .find(|node| node.handle == row.1)
        .unwrap_or_else(|| panic!("a step {}", row.1));
    ValueRef::new(source.id.clone(), row.2, row.3)
}

/// The byte ranges of every occurrence of `needle` in `text`.
fn occurrences(text: &str, needle: &str) -> Vec<(usize, usize)> {
    text.match_indices(needle)
        .map(|(start, found)| (start, start + found.len()))
        .collect()
}

/// The byte range of the line of `text` that starts, trimmed, with `start`.
fn line_range(text: &str, start: &str) -> std::ops::Range<usize> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_start().starts_with(start) {
            return at..at + line.len();
        }
        at += line.len();
    }
    panic!("a line {start}");
}

/// A model as the statements say it: every node by handle with what it
/// reads (references by handle), its kind, configuration, label and note;
/// the order edges by handle. Identities, positions and the order of the
/// nodes are not part of it.
type Shape = (BTreeMap<String, String>, BTreeSet<String>);

fn shape(document: &apiforge_script::Document) -> (Shape, Shape) {
    let (main, cleanup) = graphs(document);
    let handles: BTreeMap<String, String> = main
        .nodes
        .iter()
        .chain(cleanup.into_iter().flat_map(|graph| &graph.nodes))
        .map(|node| (node.id.clone(), node.handle.clone()))
        .collect();
    let handle = |id: &str| handles.get(id).cloned().unwrap_or_else(|| id.to_owned());
    let one = |graph: &Graph| -> Shape {
        let nodes = graph
            .nodes
            .iter()
            .map(|node| {
                let inputs: BTreeMap<String, serde_json::Value> = node
                    .inputs
                    .iter()
                    .map(|(name, binding)| {
                        let named = binding
                            .map_leaves(&mut |leaf, _| {
                                Some(match leaf {
                                    Binding::Reference {
                                        node_id,
                                        port,
                                        path,
                                    } => Binding::reference(handle(node_id), port, path),
                                    other => other.clone(),
                                })
                            })
                            .unwrap_or_else(|| binding.clone());
                        (name.clone(), serde_json::to_value(named).unwrap())
                    })
                    .collect();
                (
                    node.handle.clone(),
                    format!(
                        "{:?} {} {:?} {:?} {}",
                        node.kind,
                        serde_json::to_string(&node.config).unwrap(),
                        node.label,
                        node.note,
                        serde_json::to_string(&inputs).unwrap(),
                    ),
                )
            })
            .collect();
        let edges = graph
            .edges
            .iter()
            .map(|edge| {
                format!(
                    "{} -> {} {:?}",
                    handle(&edge.source),
                    handle(&edge.target),
                    edge.branch
                )
            })
            .collect();
        (nodes, edges)
    };
    (
        one(main),
        cleanup.map(one).unwrap_or_else(|| one(&Graph::default())),
    )
}

/// `text` lowered as a folder scenario in `project`, as the session does
/// before it gives the statements identities.
fn lowered(text: &str, project: &Project) -> apiforge_script::Document {
    let tree = apiforge_script::parse(text);
    let lowered = apiforge_script::lower(&tree, &LowerContext::folder(project));
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
    lowered.document.expect("a document")
}

/// The board's edit of `value` on `document`: LAY-25's `edit::save_values`.
fn board_edit(
    document: &apiforge_script::Document,
    project: &Project,
    value: &ValueRef,
) -> apiforge_script::Document {
    let (main, cleanup) = graphs(document);
    let saved = edit::save_values(main, cleanup, project, std::slice::from_ref(value)).unwrap();
    match document {
        apiforge_script::Document::Scenario(scenario) => {
            let mut scenario = scenario.clone();
            scenario.graph = saved.main;
            scenario.cleanup = saved.cleanup.unwrap_or_default();
            apiforge_script::Document::Scenario(scenario)
        }
        apiforge_script::Document::Component(component) => {
            let mut component = component.clone();
            component.graph = saved.main;
            apiforge_script::Document::Component(component)
        }
    }
}

/// Applies the fix of the first far-read warning on `value`'s text.
fn apply_fix_on(harness: &mut FolderHarness, id: &str, value: &str) {
    let text = source_of(harness, id);
    let (index, _) = warnings(harness, id)
        .into_iter()
        .find(|(_, diagnostic)| &text[diagnostic.range.clone()] == value)
        .unwrap_or_else(|| panic!("a warning on {value}"));
    app(harness).apply_source_fix(id, index);
    settle(harness);
}

/// Criterion 1: one warning at each far read of the three values, a read
/// inside an address's `{{…}}` included; none at `quantity`, the step after
/// `cart`, nor at a read of a variable or a parameter. The text is fixed and
/// quotes nothing from the source; each warning has its row in the problems
/// list with *Apply fix*.
#[test]
fn lay26_criterion_1_far_reads_are_warned() {
    let source = fixture("far-value");
    let (_project, mut harness, id) = in_code(&source, WIDE);
    let near = line_range(&source, "quantity = PATCH");
    let mut expected = BTreeSet::new();
    for (text, ..) in VALUES {
        for (start, end) in occurrences(&source, text) {
            if !near.contains(&start) {
                expected.insert((start, end));
            }
        }
    }
    assert_eq!(expected.len(), 15, "eight, five and two far reads");
    assert_eq!(warned(&harness, &id), expected);
    let found = warnings(&harness, &id);
    // One inside an address.
    assert!(
        found.iter().any(|(_, diagnostic)| {
            source[..diagnostic.range.start].ends_with("PUT \"/carts/{{")
        })
    );
    for (_, diagnostic) in &found {
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(diagnostic.fix.is_some(), "each has its fix");
        for word in ["cart", "session", "currency", "productId", "quantity"] {
            assert!(
                !diagnostic.message.contains(word),
                "the text quotes nothing: {}",
                diagnostic.message
            );
        }
    }
    for (_, diagnostic) in &found {
        let read = &source[diagnostic.range.clone()];
        assert!(!read.starts_with("param."), "no parameter: {read}");
    }

    // The problems list: a row for each, with Apply fix.
    let rows = harness
        .query_all_by_label_contains(FAR_READ)
        .filter(|node| {
            node.accesskit_node()
                .label()
                .is_some_and(|label| label.starts_with("Line "))
        })
        .count();
    assert_eq!(rows, 15, "a row in the problems list for each");
    assert!(
        harness.query_all_by_label("Apply fix").count() >= 15,
        "Apply fix on each"
    );
    // The first row says where it is.
    let first = &found[0].1;
    let line = source[..first.range.start].matches('\n').count() + 1;
    let column = first.range.start
        - source[..first.range.start]
            .rfind('\n')
            .map_or(0, |at| at + 1)
        + 1;
    assert!(
        harness
            .query_by_label(&format!("Line {line}, column {column}   {FAR_READ}"))
            .is_some(),
        "the row names line {line}, column {column}"
    );
    harness.step();

    // shop-30: the two reads of Cleanup.
    let source = fixture("shop-30");
    let (_project, harness, id) = in_code(&source, WIDE);
    let cleanup = source.find("  cleanup {").expect("a cleanup");
    let warned = warned(&harness, &id);
    for text in ["cart.body.id", "login.body.token"] {
        let (start, end) = occurrences(&source, text)
            .into_iter()
            .find(|(start, _)| *start > cleanup)
            .unwrap_or_else(|| panic!("{text} in cleanup"));
        assert!(
            warned.contains(&(start, end)),
            "{text} in cleanup is warned"
        );
    }
    for (start, end) in &warned {
        let read = &source[*start..*end];
        for variable in ["first", "second", "param.", "env."] {
            assert!(!read.starts_with(variable), "never a variable: {read}");
        }
    }
}

/// Criterion 2: *Apply fix* saves the value as the board's *Save as
/// variable* does, and removes every warning of the value. The three fixes
/// in any order give the statements of `far-value-saved`.
#[test]
fn lay26_criterion_2_the_fix_is_the_boards_edit() {
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        let (_project, mut harness, id) = in_code(&fixture("far-value"), WIDE);
        let project = context(&harness);
        for index in order {
            let row = VALUES[index];
            let before = model(&harness, &id);
            let expected = board_edit(&before, &project, &value_of(&before, row));
            let lines = source_of(&harness, &id).lines().count();
            apply_fix_on(&mut harness, &id, row.0);
            let text = source_of(&harness, &id);
            assert_eq!(
                text.lines().count(),
                lines + 1,
                "{order:?}: {} adds one line",
                row.0
            );
            assert_eq!(
                shape(&model(&harness, &id)),
                shape(&expected),
                "{order:?}: {} as the board saves it",
                row.0
            );
            assert_eq!(
                shape(&lowered(&text, &project)),
                shape(&expected),
                "{order:?}: the text parses to the board's model"
            );
            assert!(
                warnings(&harness, &id)
                    .iter()
                    .all(|(_, diagnostic)| &text[diagnostic.range.clone()] != row.0),
                "{order:?}: no warning of {} is left",
                row.0
            );
        }
        let project = context(&harness);
        assert_eq!(
            shape(&model(&harness, &id)),
            shape(&lowered(&fixture("far-value-saved"), &project)),
            "{order:?}: the statements of far-value-saved"
        );
        assert!(warnings(&harness, &id).is_empty(), "{order:?}: none left");
    }
}

/// Criterion 2: the `var` line follows its source; where a variable already
/// holds the value no line is added and the readers read that one.
#[test]
fn lay26_criterion_2_a_held_value_is_read_through_its_variable() {
    let (_project, mut harness, id) = in_code(&fixture("far-value"), WIDE);
    apply_fix_on(&mut harness, &id, "session.body.currency");
    let text = source_of(&harness, &id);
    let session = line_range(&text, "sample 201 { id: \"s-1\", currency: \"PLN\" }");
    let after = &text[session.end..];
    let after = &after[after.find('\n').unwrap() + 1..];
    assert!(
        after.starts_with("  var currency = session.body.currency\n"),
        "the line right after session: {after}"
    );

    let source = fixture("far-value").replace(
        "  cart = add_to_cart@1(product: product, quantity: 2)\n",
        "  cart = add_to_cart@1(product: product, quantity: 2)\n  var cartId = cart.id\n",
    );
    let (_project, mut harness, id) = in_code(&source, WIDE);
    let project = context(&harness);
    let before = model(&harness, &id);
    let expected = board_edit(&before, &project, &value_of(&before, VALUES[0]));
    apply_fix_on(&mut harness, &id, "cart.id");
    let text = source_of(&harness, &id);
    assert_eq!(
        text.lines().count(),
        source.lines().count(),
        "no line added"
    );
    assert_eq!(text.matches("cart.id").count(), 1, "only the variable");
    assert_eq!(text.matches("var cartId").count(), 1);
    assert_eq!(shape(&model(&harness, &id)), shape(&expected));
}

/// Criterion 2: the fix edits the text, it does not reprint it; one undo
/// restores the text byte for byte.
#[test]
fn lay26_criterion_2_the_fix_keeps_every_other_byte_and_one_undo_restores_it() {
    let source = fixture("far-value")
        .replace(
            "  cart = add_to_cart@1(product: product, quantity: 2)\n",
            "  cart = add_to_cart@1(product: product, quantity: 2)\n\n  // The cart's lines.\n",
        )
        .replace("body { quantity: 3 }", "body {   quantity:    3 }")
        .replace(
            "  shipping = PUT \"/carts/{{cart.id}}/shipping\" {",
            "  shipping = PUT   \"/carts/{{ cart.id }}/shipping\"  {",
        );
    let (_project, mut harness, id) = in_code(&source, WIDE);
    assert!(
        diagnostics(&harness, &id)
            .iter()
            .all(|diagnostic| diagnostic.severity == Severity::Warning),
        "the copy is valid"
    );
    let text = source_of(&harness, &id);
    assert_eq!(text, source);
    // The first problem with a fix is the first far read, of cart.id.
    let first = diagnostics(&harness, &id)
        .into_iter()
        .find(|diagnostic| diagnostic.fix.is_some())
        .unwrap();
    assert_eq!(first.message, FAR_READ);
    assert_eq!(&source[first.range.clone()], "cart.id");
    harness
        .query_all_by_label("Apply fix")
        .next()
        .expect("Apply fix")
        .click();
    settle(&mut harness);
    let fixed = source_of(&harness, &id);
    let cart = line_range(&source, "cart = add_to_cart@1");
    let mut expected = source.replace("cart.id", "cartId");
    let at = line_range(&expected, "cart = add_to_cart@1").end;
    expected.insert_str(at, "  var cartId = cart.id\n");
    assert_eq!(
        fixed, expected,
        "only the new line and the references change"
    );
    assert_eq!(&fixed[..cart.end], &source[..cart.end]);

    // In the code, as `applying_a_pin_fix_from_problems_can_be_undone_in_the_code_editor`
    // undoes a fix: the editor's own history.
    harness.get_by_label(&editor(&harness, &id)).click();
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut harness);
    harness.step();
    assert_eq!(source_of(&harness, &id), source, "one undo restores it");
}

/// Criterion 3: `far-value-saved` has no such warning.
#[test]
fn lay26_criterion_3_the_saved_scenario_has_none() {
    let (_project, harness, id) = in_code(&fixture("far-value-saved"), WIDE);
    model(&harness, &id);
    assert!(warnings(&harness, &id).is_empty());
}

/// The steps of `document` by id, Main and Cleanup.
fn nodes_by_id(
    document: &apiforge_script::Document,
) -> BTreeMap<String, apiforge_core::format::Node> {
    let (main, cleanup) = graphs(document);
    main.nodes
        .iter()
        .chain(cleanup.into_iter().flat_map(|graph| &graph.nodes))
        .map(|node| (node.id.clone(), node.clone()))
        .collect()
}

/// The statement whose range holds `at`, innermost, by handle.
fn statement_at(text: &str, at: usize) -> String {
    fn find(statements: &[apiforge_script::ast::Statement], at: usize) -> Option<String> {
        statements
            .iter()
            .find(|statement| statement.range.contains(&at))
            .map(|statement| match &statement.form {
                apiforge_script::ast::Form::If {
                    then, otherwise, ..
                } => find(then, at)
                    .or_else(|| find(otherwise.as_deref().unwrap_or_default(), at))
                    .unwrap_or_else(|| statement.handle.clone()),
                _ => statement.handle.clone(),
            })
    }
    let tree = apiforge_script::parse(text);
    let file = tree.file.expect("a file");
    find(&file.statements, at)
        .or_else(|| find(file.cleanup.as_deref().unwrap_or_default(), at))
        .unwrap_or_else(|| panic!("a statement at {at}"))
}

/// Criterion 6: the rule is LAY-25's, through `edit::far_reads`: every read
/// the warning names is a far read there, every far read is warned, and on
/// the tidied board the reader is in Cleanup or not beside its source.
#[test]
fn lay26_criterion_6_the_rule_is_the_boards() {
    for name in ["far-value", "shop-30"] {
        let source = fixture(name);
        let (_project, harness, id) = in_code(&source, WIDE);
        let document = model(&harness, &id);
        let nodes = nodes_by_id(&document);
        let by_handle: BTreeMap<&str, &str> = nodes
            .values()
            .map(|node| (node.handle.as_str(), node.id.as_str()))
            .collect();
        let (main, cleanup) = graphs(&document);
        let far: BTreeSet<(String, String)> = edit::far_reads(main, cleanup)
            .into_iter()
            .map(|read| {
                let project = context(&harness);
                (
                    read.reader.clone(),
                    edit::value_text(main, cleanup, &project, &read.value),
                )
            })
            .collect();
        let named: BTreeSet<(String, String)> = warnings(&harness, &id)
            .into_iter()
            .map(|(_, diagnostic)| {
                let reader = statement_at(&source, diagnostic.range.start);
                (
                    by_handle[reader.as_str()].to_owned(),
                    source[diagnostic.range.clone()].to_owned(),
                )
            })
            .collect();
        assert!(!named.is_empty(), "{name}: warnings");
        assert_eq!(named, far, "{name}: the warned reads are the far reads");

        // On the tidied board.
        let (_project, mut harness, id) = opened(&source, WIDE, Mode::Diagram);
        app(&mut harness).selections.insert(id.clone(), Vec::new());
        settle(&mut harness);
        choose(&mut harness, TIDY);
        harness.step();
        let document = model(&harness, &id);
        let nodes = nodes_by_id(&document);
        let (main, cleanup) = graphs(&document);
        let in_cleanup: BTreeSet<&str> = cleanup
            .into_iter()
            .flat_map(|graph| &graph.nodes)
            .map(|node| node.id.as_str())
            .collect();
        let canvas = folder(&harness).canvases.get(&id).expect("the board");
        let rect = |node: &str| {
            canvas
                .debug_world(node)
                .or_else(|| {
                    canvas
                        .folded_into(main, cleanup, node)
                        .and_then(|host| canvas.debug_world(&host))
                })
                .unwrap_or_else(|| panic!("{name}: {} is on the board", nodes[node].handle))
        };
        let spine: Vec<egui::Rect> = main
            .nodes
            .iter()
            .filter(|node| crate::canvas::shape::on_spine(node.kind))
            .filter_map(|node| canvas.debug_world(&node.id))
            .collect();
        let mut rows: Vec<f32> = spine.iter().map(|rect| rect.top()).collect();
        rows.sort_by(f32::total_cmp);
        rows.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        let row = |y: f32| {
            rows.iter()
                .rposition(|top| *top <= y + 1.0)
                .unwrap_or_default()
        };
        let mut lefts: Vec<f32> = spine.iter().map(|rect| rect.left()).collect();
        lefts.sort_by(f32::total_cmp);
        lefts.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        let pitch = lefts
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .fold(f32::INFINITY, f32::min);
        for read in edit::far_reads(main, cleanup) {
            if in_cleanup.contains(read.reader.as_str()) {
                continue;
            }
            let (from, to) = (rect(&read.value.node_id), rect(&read.reader));
            let step = to.left() - from.left();
            let beside = row(from.top()) == row(to.top())
                && (step.abs() < pitch / 2.0 || (step - pitch).abs() < pitch / 2.0);
            assert!(
                !beside,
                "{name}: {} reads {} far, and is not beside it",
                nodes[&read.reader].handle, nodes[&read.value.node_id].handle
            );
        }
    }
}

/// Criterion 7: errors come first. A text with an error has no far-read
/// warning; where the list is full a far read gives way, as every warning
/// does.
#[test]
fn lay26_criterion_7_errors_come_first() {
    let broken = fixture("far-value").replacen(
        "  quantity = PATCH",
        "  broken = unknown_step.value\n  quantity = PATCH",
        1,
    );
    assert_ne!(broken, fixture("far-value"), "the error is written");
    let (_project, harness, id) = in_code(&broken, WIDE);
    let all = diagnostics(&harness, &id);
    assert!(
        all.iter().any(|d| d.severity == Severity::Error),
        "the error"
    );
    assert!(warnings(&harness, &id).is_empty(), "no far read beside it");

    // 990 comments that belong to no statement, then fifteen far reads: the
    // list keeps a thousand. The window's own session analyses it, without
    // drawing a thousand rows, which a debug build on a slow runner cannot
    // settle in time.
    let mut full = fixture("far-value");
    for index in 0..990 {
        full.push_str(&format!("// note {index}\n"));
    }
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), &full).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let placer = super::super::placement::CardPlacer::new(
        super::super::placement::DensityHandle::new(crate::canvas::CardDensity::Full),
    );
    let session =
        apiforge_project::ProjectSession::open_with(project.path(), Arc::new(placer)).unwrap();
    let document = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == SCENARIO)
        .unwrap();
    let all = document.diagnostics();
    assert_eq!(all.len(), apiforge_script::MAX_DIAGNOSTICS);
    assert_eq!(
        all.iter().filter(|d| d.message == FAR_READ).count(),
        10,
        "as many as fit"
    );
    assert_eq!(document.omitted_diagnostics(), 5);
}

/// Criterion 7: the Code view with the warnings, at both window sizes.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay26() {
    for size in [WIDE, MINIMUM] {
        let suffix = if size == WIDE { "1600" } else { "960" };
        let (_project, mut harness, id) = in_code(&fixture("far-value"), size);
        assert!(!warnings(&harness, &id).is_empty());
        harness.step();
        assert!(
            harness
                .query_all_by_label_contains(FAR_READ)
                .next()
                .is_some(),
            "a row of the problems list"
        );
        crate::testing::capture(&mut harness, &format!("lay26-code-{suffix}"), None);
    }
}
