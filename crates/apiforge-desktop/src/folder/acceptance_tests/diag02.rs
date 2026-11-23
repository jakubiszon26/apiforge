//! DIAG-02 (#145): a check reads the same everywhere and says what it
//! compares. One function writes a check's predicate in the signs of the
//! code (`==`, `!=`, `>`, `contains`, `nonempty`); the card's title, the
//! folded line of LAY-30 and the Inspector's *Checks* tab call it. A check
//! hung under its step leaves the step's name and the leading `body.` out
//! (`error.code == "out_of_range"`); its handle and the step it belongs to
//! are a small line under the title. A check that stands alone or reads two
//! steps keeps the names. The title wraps to at most three rows and is cut
//! inside the path, never at the start of the expected value; the whole
//! text is on hover. A check with `@label` is titled by the label.
//!
//! On the reference scenario `labels` (FIX-01) and on the room project's
//! `booking-day`.
use super::lay23::{Checks, board, graph, show, tidied_text};
use super::*;
use crate::canvas::{CheckText, TextKind, check_text, fit_title};
use crate::testing::labels_board;
use apiforge_core::format::{Binding, Graph, Node};
use apiforge_core::model::NodeKind;
use apiforge_core::template_text::Scope;
use egui_kittest::kittest::NodeT;
use std::path::Path;

/// The bar's switch of every check card (LAY-30).
const CHECKS: &str = "Checks";

/// How a check card is named for assistive technology: its name and kind.
fn card_name(node: &Node) -> String {
    format!("{} (Comparison)", node.display_name())
}

/// The text a check card shows as its title, as the accessibility tree
/// carries it: the value of the card's node.
fn card_text(harness: &FolderHarness, node: &Node) -> String {
    let name = card_name(node);
    harness
        .get_by_label(&name)
        .accesskit_node()
        .value()
        .map(|value| value.to_string())
        .unwrap_or_else(|| panic!("{name} carries no text"))
}

/// `labels` on its board at 1600 × 1000 with every check card shown, as
/// the bar's *Checks* shows them.
fn labels_with_checks() -> (tempfile::TempDir, FolderHarness, String) {
    labels_with_checks_at(WIDE)
}

/// As [`labels_with_checks`], at `size`.
fn labels_with_checks_at(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = labels_board(size);
    harness.get_by_label(CHECKS).click();
    settle(&mut harness);
    harness.step();
    assert!(
        !board(&harness, &id).debug_collapsed(),
        "Checks shows every check card"
    );
    (project, harness, id)
}

fn node<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
}

fn node_by_id<'a>(graph: &'a Graph, id: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("a node {id}"))
}

/// The expected operand of a check as the code writes it.
fn expected_operand(node: &Node) -> String {
    match node.inputs.get("expected") {
        Some(Binding::Literal { value }) => apiforge_core::js::json_stringify(value),
        Some(Binding::Parameter { name, path }) if path.is_empty() => format!("param.{name}"),
        other => panic!("{}: an expected value {other:?}", node.handle),
    }
}

/// Nine checks of one step, one per predicate, as the code writes them.
const PREDICATES: &str = r#"scenario "Predicates" {
  begin = start
  cart = POST "/cart" {
    body { items: 1 }
    sample 201 { id: "c-1", total: 172.5, items: ["screen"] }
  }
  other = GET "/other" {
    sample 200 { total: 172.5 }
  }
  equal = assert cart.status == 201
  not_equal = assert cart.status != 500
  greater = assert cart.body.total > 0
  less = assert cart.body.total < 1000
  near = assert cart.body.total ~= 172.5 within 0.01
  has_id = assert exists cart.body.id
  has_items = assert nonempty cart.body.items
  has_screen = assert cart.body.items contains "screen"
  typed = assert cart.body.id is "string"
  same_total = assert cart.body.total == other.body.total
}
"#;

/// [`PREDICATES`] lowered against the demo shop: its graph and project.
fn predicates() -> (Graph, apiforge_core::format::Project) {
    use apiforge_script::{Document, LowerContext, lower, parse};
    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&parse(PREDICATES), &LowerContext::folder(&project));
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("the scenario lowers: {:#?}", lowered.diagnostics);
    };
    (scenario.graph, project)
}

fn written(
    graph: &Graph,
    project: &apiforge_core::format::Project,
    handle: &str,
    own: Option<&str>,
) -> CheckText {
    let scope = Scope {
        nodes: &graph.nodes,
        project,
        request: None,
    };
    let own = own.map(|handle| node(graph, handle).id.as_str());
    check_text(node(graph, handle), scope, graph, own)
}

/// Criterion 1, unit: one function writes each predicate of `Predicate`
/// in the sign of the code. Under its own step the step's name and the
/// leading `body.` are left out; without one the names stay; a check of
/// two steps under one of them (the folded line of LAY-30) leaves out only
/// that step's name.
#[test]
fn diag02_criterion_1_one_function_writes_every_predicate() {
    let (graph, project) = predicates();
    for (handle, own, named) in [
        ("equal", "status == 201", "cart.status == 201"),
        ("not_equal", "status != 500", "cart.status != 500"),
        ("greater", "total > 0", "cart.body.total > 0"),
        ("less", "total < 1000", "cart.body.total < 1000"),
        (
            "near",
            "total ~= 172.5 within 0.01",
            "cart.body.total ~= 172.5 within 0.01",
        ),
        ("has_id", "exists id", "exists cart.body.id"),
        ("has_items", "nonempty items", "nonempty cart.body.items"),
        (
            "has_screen",
            "items contains \"screen\"",
            "cart.body.items contains \"screen\"",
        ),
        ("typed", "id is \"string\"", "cart.body.id is \"string\""),
    ] {
        assert_eq!(
            written(&graph, &project, handle, Some("cart")).text(),
            own,
            "{handle} under its step"
        );
        assert_eq!(
            written(&graph, &project, handle, None).text(),
            named,
            "{handle} with its names"
        );
    }
    // Every predicate is covered above.
    assert_eq!(apiforge_core::model::Predicate::ALL.len(), 9);
    assert_eq!(
        written(&graph, &project, "same_total", None).text(),
        "cart.body.total == other.body.total",
        "a check of two steps keeps both names"
    );
    assert_eq!(
        written(&graph, &project, "same_total", Some("cart")).text(),
        "body.total == other.body.total",
        "under one of its two steps only that step's name is left out"
    );
}

/// Criterion 1, window: on `labels` the card of `weight_31_code` and the
/// Inspector's *Checks* tab of `weight_31` write the same text; the card's
/// small line names the handle and the step with its number.
#[test]
fn diag02_criterion_1_the_card_and_the_inspector_write_the_same_text() {
    let (_project, mut harness, id) = labels_with_checks();
    let graph = graph(&harness, &id);
    let check = node(&graph, "weight_31_code").clone();
    let step = node(&graph, "weight_31").clone();
    show(&mut harness, &id, "weight_31_code", 1.0);
    let card = card_text(&harness, &check);
    assert_eq!(card, "error.code == \"out_of_range\"");

    let number = apiforge_core::graph::sequence(&graph)
        .iter()
        .position(|node| *node == step.id)
        .unwrap()
        + 1;
    let small = format!("weight_31_code · of {number} Weight 31 kg");
    let texts = board(&harness, &id).debug_texts(&check.id);
    assert!(
        texts.iter().any(|text| text.text == small),
        "the small line {small:?} among {texts:?}"
    );

    app(&mut harness)
        .selections
        .insert(id.clone(), vec![step.id.clone()]);
    settle(&mut harness);
    super::open_checks_tab(&mut harness);
    let row = harness.get_by_label("Check weight_31_code").rect();
    let written = harness
        .query_all_by_label(&card)
        .filter(|label| row.contains_rect(label.rect()))
        .count();
    assert_eq!(written, 1, "the Inspector writes {card:?} in the row");
    // The names are left out where the step is shown beside the text: the
    // Inspector is the step's, and the row's sources name it in full.
    let sources = "Sources: weight_31.body.error.code";
    assert!(
        harness
            .query_all_by_label(sources)
            .any(|label| row.contains_rect(label.rect())),
        "the row names the step: {sources:?}"
    );
    for old in ["equals", "is equal to"] {
        assert!(
            harness.query_by_label_contains(old).is_none(),
            "the wording {old:?} has left the window"
        );
    }
}

/// Criterion 2: on `labels` with *Checks* shown, at 100 %, every check card
/// under its request shows its expected value in full.
#[test]
fn diag02_criterion_2_every_check_shows_its_expected_value_in_full() {
    let (_project, mut harness, id) = labels_with_checks();
    let graph = graph(&harness, &id);
    let checks: Vec<Node> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Assert)
        .cloned()
        .collect();
    assert_eq!(checks.len(), 33, "labels has thirty-three checks");
    for check in &checks {
        show(&mut harness, &id, &check.handle, 1.0);
        let shown = card_text(&harness, check);
        let expected = expected_operand(check);
        assert!(
            shown.ends_with(&expected) && !shown.contains('…'),
            "{} shows {shown:?}, not its expected value {expected} in full",
            check.handle
        );
        let subjects = apiforge_core::graph::check_subjects(&graph, check).subjects;
        let [subject] = subjects.as_slice() else {
            panic!("{} hangs under one request", check.handle);
        };
        let step = &node_by_id(&graph, subject.step).handle;
        assert!(
            !shown.contains(&format!("{step}.")) && !shown.starts_with("body."),
            "{}: the step's name and `body.` are left out of {shown:?}",
            check.handle
        );
    }
}

/// A temporary copy of the room project.
fn room_project() -> tempfile::TempDir {
    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), &target).unwrap();
            }
        }
    }
    let project = tempfile::tempdir().unwrap();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/rooms"),
        project.path(),
    );
    project
}

/// Criterion 3: a check that reads two steps (`invited` of
/// `rooms/booking-day`) keeps both step names.
#[test]
fn diag02_criterion_3_a_check_of_two_steps_keeps_both_names() {
    let project = room_project();
    let mut harness = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut harness).debug_draw_checks();
    app(&mut harness).review_open("scenarios/booking-day.afs", None);
    settle(&mut harness);
    let id = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "scenarios/booking-day.afs")
        .unwrap()
        .id()
        .to_owned();
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    show(&mut harness, &id, "invited", 1.0);
    let graph = graph(&harness, &id);
    assert_eq!(
        card_text(&harness, node(&graph, "invited")),
        "invite.body.invited == team.body.size"
    );
    // A check of one step leaves its name out beside it.
    show(&mut harness, &id, "held", 1.0);
    assert_eq!(card_text(&harness, node(&graph, "held")), "amount == 40");
}

/// A request with a deep response and a check of a long path.
const LONG: &str = r#"scenario "Long" {
  begin = start
  @label("Weight 31 kg")
  weight_31 = POST "https://parcels.example.test/labels" {
    body { weight: 31 }
    sample 422 { error: { details: { reasons: [{ constraints: { maximum: { message: { text: "Weight must be at most 30 kg." } } } }] } } }
  }
  weight_31_reason = assert weight_31.body.error.details.reasons[0].constraints.maximum.message.text == "Weight must be at most 30 kg."
  @label("Refused for weight")
  weight_31_status = assert weight_31.status == 422
}
"#;

/// The rows of a fitted title, as text.
fn rows(text: &CheckText, per_row: usize) -> (String, Vec<String>) {
    let title = fit_title(text, per_row, 3);
    let rows = title
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|part| part.text.as_str())
                .collect::<String>()
        })
        .collect();
    (title.shown, rows)
}

/// Criterion 4, unit: a predicate longer than three rows is cut inside the
/// path, its first and last names kept, and the expected value is whole;
/// when even that is too long, the end is cut, never the start of the
/// expected value.
#[test]
fn diag02_criterion_4_the_cutting_rule() {
    use apiforge_script::{Document, LowerContext, lower, parse};
    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&parse(LONG), &LowerContext::folder(&project));
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("the scenario lowers: {:#?}", lowered.diagnostics);
    };
    let graph = scenario.graph;
    let text = written(&graph, &project, "weight_31_reason", Some("weight_31"));
    let whole = "error.details.reasons[0].constraints.maximum.message.text == \"Weight must be at most 30 kg.\"";
    assert_eq!(text.text(), whole);

    let (shown, cut) = rows(&text, 28);
    assert!(cut.len() <= 3, "{cut:?}");
    assert!(cut.iter().all(|row| row.chars().count() <= 28), "{cut:?}");
    let (path, expected) = shown.split_once(" == ").expect("the sign stays");
    assert!(
        path.starts_with("error…") && path.ends_with(".text"),
        "the path is cut inside: {path:?}"
    );
    assert_eq!(expected, "\"Weight must be at most 30 kg.\"", "whole");

    // A text that fits is left whole and wraps at a space.
    let (shown, fits) = rows(&text, 60);
    assert_eq!(shown, whole);
    assert_eq!(fits.len(), 2, "{fits:?}");

    // Too narrow for the expected value whole: its end is cut, its start
    // stays.
    let (shown, narrow) = rows(&text, 12);
    assert!(narrow.len() <= 3, "{narrow:?}");
    assert!(shown.ends_with('…'), "{shown:?}");
    assert!(shown.contains(" == \"Weight"), "{shown:?}");
}

/// Criterion 4, window: the card of a long predicate is cut inside the
/// path, and pointing at its title shows the whole text.
#[test]
fn diag02_criterion_4_hovering_a_cut_title_shows_it_whole() {
    let (_project, mut harness, id) = tidied_text(LONG, WIDE, Checks::Expanded);
    show(&mut harness, &id, "weight_31_reason", 1.0);
    let graph = graph(&harness, &id);
    let check = node(&graph, "weight_31_reason").clone();
    let shown = card_text(&harness, &check);
    assert!(
        shown.starts_with("error…") && shown.ends_with("== \"Weight must be at most 30 kg.\""),
        "cut inside the path: {shown:?}"
    );
    let whole = "error.details.reasons[0].constraints.maximum.message.text == \"Weight must be at most 30 kg.\"";
    let name = card_name(&check);
    let shown = crate::testing::hover_until(
        &mut harness,
        |harness| {
            let card = harness.get_by_label(&name).rect();
            Some(egui::Rect::from_center_size(
                card.left_top() + egui::vec2(120.0, 24.0),
                egui::vec2(4.0, 4.0),
            ))
        },
        |harness| harness.query_by_label_contains(whole).is_some(),
    );
    assert!(shown, "pointing at the title shows {whole:?}");
}

/// Criterion 5: a check with `@label` is titled by the label, its
/// predicate the line under it.
#[test]
fn diag02_criterion_5_a_labelled_check_is_titled_by_its_label() {
    let (_project, mut harness, id) = tidied_text(LONG, WIDE, Checks::Expanded);
    show(&mut harness, &id, "weight_31_status", 1.0);
    let graph = graph(&harness, &id);
    let check = node(&graph, "weight_31_status").clone();
    assert!(harness.query_by_label(&card_name(&check)).is_some());
    let texts = board(&harness, &id).debug_texts(&check.id);
    let title = texts
        .iter()
        .find(|text| text.kind == TextKind::Name)
        .unwrap_or_else(|| panic!("a title among {texts:?}"));
    assert_eq!(title.text, "Refused for weight");
    let line = texts
        .iter()
        .find(|text| text.text == "status == 422")
        .unwrap_or_else(|| panic!("the predicate among {texts:?}"));
    assert!(
        line.rect.top() >= title.rect.bottom() - 1.0,
        "the predicate {:?} is under the title {:?}",
        line.rect,
        title.rect
    );
    assert_eq!(card_text(&harness, &check), "status == 422");
}

/// Criterion 7: the check cards of `labels` keep distinct accessible names.
#[test]
fn diag02_criterion_7_check_cards_keep_distinct_accessible_names() {
    let (_project, mut harness, id) = labels_with_checks();
    harness.get_by_label("Fit the view").click();
    settle(&mut harness);
    harness.step();
    let graph = graph(&harness, &id);
    let names: Vec<String> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Assert)
        .map(card_name)
        .collect();
    let distinct: std::collections::HashSet<&String> = names.iter().collect();
    assert_eq!(distinct.len(), names.len(), "{names:?}");
    for name in &names {
        assert_eq!(
            harness.query_all_by_label(name).count(),
            1,
            "{name} names one card"
        );
    }
}

/// Owner's acceptance (2026-11-23): the predicate title is bold, as in the
/// mock-up, in the monospace font's bold face.
#[test]
fn diag02_review_the_predicate_title_is_bold() {
    let (_project, mut harness, id) = labels_with_checks();
    let graph = graph(&harness, &id);
    let check = node(&graph, "weight_31_code").clone();
    show(&mut harness, &id, "weight_31_code", 1.0);
    let texts = board(&harness, &id).debug_texts(&check.id);
    let title = texts
        .iter()
        .find(|text| text.text == "error.code == \"out_of_range\"")
        .unwrap_or_else(|| panic!("the title among {texts:?}"));
    assert_eq!(title.family, crate::theme::bold_mono_family(), "{title:?}");
}

/// A response whose body has a field named like an output of its step.
const FIELDS: &str = r#"scenario "Fields" {
  begin = start
  order = GET "/order" {
    sample 200 { status: "open", headers: { x: 1 }, error: { code: "none" } }
  }
  order_open = assert order.body.status == "open"
  cart = POST "/cart" {
    body { items: 1 }
    sample 201 { id: "c-1" }
  }
  cart_created = assert cart.status == 201
  other = GET "/other" {
    sample 200 { headers: { x: 1 }, error: { code: "none" } }
  }
  other_header = assert other.body.headers.x == 1
  other_code = assert other.body.error.code == "none"
}
"#;

/// Ruling 1 with its addendum: under its one step a check leaves `body.`
/// out only when the next name is not one of the step's outputs, so
/// `order.body.status` never reads as `order.status`. Unit, card and the
/// folded line of LAY-30.
#[test]
fn diag02_ruling_1_body_stays_before_the_name_of_an_output() {
    use apiforge_script::{Document, LowerContext, lower, parse};
    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&parse(FIELDS), &LowerContext::folder(&project));
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("the scenario lowers: {:#?}", lowered.diagnostics);
    };
    let graph = scenario.graph;
    for (handle, step, own) in [
        ("order_open", "order", "body.status == \"open\""),
        ("cart_created", "cart", "status == 201"),
        ("other_header", "other", "body.headers.x == 1"),
        ("other_code", "other", "error.code == \"none\""),
    ] {
        assert_eq!(
            written(&graph, &project, handle, Some(step)).text(),
            own,
            "{handle} under its step"
        );
    }

    let (_project, mut harness, id) = tidied_text(FIELDS, WIDE, Checks::Expanded);
    let graph = super::lay23::graph(&harness, &id);
    for (handle, text) in [
        ("order_open", "body.status == \"open\""),
        ("cart_created", "status == 201"),
    ] {
        show(&mut harness, &id, handle, 1.0);
        assert_eq!(card_text(&harness, node(&graph, handle)), text, "{handle}");
    }

    let (_project, harness, id) = tidied_text(FIELDS, WIDE, Checks::Collapsed);
    let graph = super::lay23::graph(&harness, &id);
    let line = |handle: &str| {
        board(&harness, &id)
            .debug_aggregate(&node(&graph, handle).id)
            .unwrap_or_else(|| panic!("{handle} has a checks line"))
    };
    assert!(
        line("order").ends_with(" · body.status == \"open\""),
        "{}",
        line("order")
    );
    assert!(
        line("cart").ends_with(" · status == 201"),
        "{}",
        line("cart")
    );
}

/// Calculations in a check's operands, and the same calculation as a value.
const SUMS: &str = r#"scenario "Sums" {
  begin = start
  cart = POST "/cart" {
    body { items: 1 }
    sample 201 { total: 200, price: 50 }
  }
  doubled = cart.body.price * 2
  enough = assert cart.body.total - 1 > cart.body.price * 2
}
"#;

/// Ruling 4: a check's operands take the signs of the code too (`-`, `*`),
/// on the card and in the Inspector; a Value card of the same calculation
/// keeps the board's (`×`).
#[test]
fn diag02_ruling_4_operands_take_the_signs_of_the_code() {
    let (_project, mut harness, id) = tidied_text(SUMS, WIDE, Checks::Expanded);
    let graph = super::lay23::graph(&harness, &id);
    let text = "total - 1 > price * 2";
    show(&mut harness, &id, "enough", 1.0);
    assert_eq!(card_text(&harness, node(&graph, "enough")), text);

    show(&mut harness, &id, "doubled", 1.0);
    let value = board(&harness, &id).debug_texts(&node(&graph, "doubled").id);
    assert!(
        value.iter().any(|line| line.text == "cart.body.price × 2"),
        "the Value card keeps the board's sign: {value:?}"
    );

    app(&mut harness)
        .selections
        .insert(id.clone(), vec![node(&graph, "cart").id.clone()]);
    settle(&mut harness);
    super::open_checks_tab(&mut harness);
    assert!(
        harness.query_by_label(text).is_some(),
        "the Inspector writes {text:?}"
    );
}

/// The text a check card draws as its one-row title below 75 %.
fn one_row_title(harness: &FolderHarness, id: &str, check: &Node) -> String {
    let texts = board(harness, id).debug_texts(&check.id);
    texts
        .iter()
        .find(|text| text.kind == TextKind::Line && text.text.contains(" == "))
        .unwrap_or_else(|| panic!("a title among {texts:?}"))
        .text
        .clone()
}

/// A field name longer than a row of the title.
const WORD: &str = r#"scenario "Word" {
  begin = start
  cart = POST "/cart" {
    body { items: 1 }
    sample 201 { averyveryverylongfieldnamethatkeepsgoing: 1 }
  }
  long_name = assert cart.body.averyveryverylongfieldnamethatkeepsgoing == 1
}
"#;

/// Review: below 75 % the one-row title is the title as shown, not its rows
/// joined with spaces, so a row broken inside a word reads as one word.
#[test]
fn diag02_review_the_one_row_title_adds_no_space() {
    let (_project, mut harness, id) = tidied_text(WORD, WIDE, Checks::Expanded);
    let graph = super::lay23::graph(&harness, &id);
    let check = node(&graph, "long_name").clone();
    let text = "averyveryverylongfieldnamethatkeepsgoing == 1";
    show(&mut harness, &id, "long_name", 1.0);
    assert_eq!(card_text(&harness, &check), text);
    show(&mut harness, &id, "long_name", 0.6);
    assert_eq!(one_row_title(&harness, &id, &check), text);
}

/// An `if` and a *Data condition* of one comparison.
const BRANCHES: &str = r#"scenario "Branches" {
  begin = start
  order = GET "/order" {
    sample 200 { status: "created" }
  }
  ready = condition all { order.status == 200 }
  paid = if order.body.status == "created" {
    pay = POST "/pay" {
      sample 200 { paid: true }
    }
  } else {
    unpaid = assert false == true
  }
}
"#;

/// Review: a full `if` card and a full *Data condition* card of one
/// comparison write their predicate as their subtitle, not a bare sign.
#[test]
fn diag02_review_full_if_and_condition_cards_write_their_predicate() {
    let (_project, mut harness, id) = tidied_text(BRANCHES, WIDE, Checks::Expanded);
    super::lay16::switch_density(&mut harness);
    settle(&mut harness);
    let graph = super::lay23::graph(&harness, &id);
    for (handle, text) in [
        ("ready", "order.status == 200"),
        ("paid", "order.body.status == \"created\""),
    ] {
        show(&mut harness, &id, handle, 1.0);
        let texts = board(&harness, &id).debug_texts(&node(&graph, handle).id);
        assert!(
            texts
                .iter()
                .any(|line| line.kind == TextKind::Subtitle && line.text == text),
            "{handle}'s subtitle is {text:?}: {texts:?}"
        );
    }
}

/// Review: a title too long is shortened inside its actual operand only,
/// and the path is split at the dots outside brackets and quotes.
#[test]
fn diag02_review_only_the_actual_path_is_shortened() {
    use crate::canvas::{Part, PartRole};
    let part = |text: &str, role| Part {
        text: text.into(),
        role,
    };
    let expected = "\"Weight must be at most 30 kg.\"";
    // A quoted key with a dot in it stays whole.
    let quoted = CheckText {
        parts: vec![
            part(
                "rows[\"a.b\"].details.reasons[0].constraints.maximum.message.text",
                PartRole::Operand,
            ),
            part(" == ", PartRole::Sign),
            part(expected, PartRole::Text),
        ],
        actual: Some(0),
        ..CheckText::default()
    };
    let shown = fit_title(&quoted, 28, 3).shown;
    assert!(
        shown.starts_with("rows[\"a.b\"]…") && shown.ends_with(&format!(" == {expected}")),
        "{shown:?}"
    );
    // A path that is the expected operand is not the one shortened: the
    // title is cut at its end.
    let reversed = CheckText {
        parts: vec![
            part(expected, PartRole::Text),
            part(" == ", PartRole::Sign),
            part(
                "error.details.reasons[0].constraints.maximum.message.text",
                PartRole::Operand,
            ),
        ],
        actual: Some(0),
        ..CheckText::default()
    };
    let shown = fit_title(&reversed, 28, 3).shown;
    assert!(
        !shown.contains("error…") && shown.ends_with('…'),
        "{shown:?}"
    );
}

/// Review: the Inspector caps a long value written in a check, as it did
/// before DIAG-02 (80 characters), in its text and its accessible label.
#[test]
fn diag02_review_the_inspector_caps_a_long_value() {
    let long = "x".repeat(200);
    let source = format!(
        r#"scenario "Long value" {{
  begin = start
  cart = POST "/cart" {{
    body {{ items: 1 }}
    sample 201 {{ note: "{long}" }}
  }}
  noted = assert cart.body.note == "{long}"
}}
"#
    );
    let (_project, mut harness, id) = tidied_text(&source, WIDE, Checks::Expanded);
    let graph = super::lay23::graph(&harness, &id);
    app(&mut harness)
        .selections
        .insert(id.clone(), vec![node(&graph, "cart").id.clone()]);
    settle(&mut harness);
    super::open_checks_tab(&mut harness);
    let capped = format!(
        "note == {}",
        crate::labels::truncate(&format!("\"{long}\""), 80)
    );
    assert!(
        harness.query_by_label(&capped).is_some(),
        "the Inspector writes {capped:?}"
    );
    assert!(harness.query_by_label_contains(&long).is_none());
}

/// The captures of the pull request: `labels` with every check card shown,
/// at 100 % on `weight_31_code`, at 960 × 540 and 1600 × 1000; the
/// whole window and the step with its checks.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag02_check_cards() {
    for (size, name) in [(crate::testing::MINIMUM, "960"), (WIDE, "1600")] {
        let (_project, mut harness, id) = labels_with_checks_at(size);
        show(&mut harness, &id, "weight_31_code", 1.0);
        crate::testing::capture(&mut harness, &format!("diag02-labels-{name}"), None);
        let graph = graph(&harness, &id);
        let region = [
            "weight_31",
            "weight_31_code",
            "weight_31_field",
            "weight_31_text",
        ]
        .into_iter()
        .filter_map(|handle| board(&harness, &id).debug_card_rect(&node(&graph, handle).id))
        .reduce(egui::Rect::union)
        .expect("the step and its checks are drawn")
        .expand2(egui::vec2(36.0, 16.0));
        crate::testing::capture(
            &mut harness,
            &format!("diag02-weight31-{name}"),
            Some(region),
        );
    }
}
