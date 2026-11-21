//! DIAG-08 (#521): the ✓ on a field of *Output data* — the Inspector's and
//! the same tree in Run results — opens the menu *Add a check*: *equals
//! ‹value›*, *is a ‹type›*, *exists*, *is not empty*. Each writes one
//! ordinary check under the step, one undo step, and the file passes
//! `check`. The window names a value's type in the language's six words:
//! `string`, `number`, `boolean`, `object`, `array`, `null`.
use super::*;
use crate::testing::MINIMUM;
use apiforge_core::format::{Binding, Graph, Node};
use apiforge_core::model::{NodeKind, Predicate};
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// A request whose sample has a value of each of the six types.
const SOURCE: &str = r#"scenario "A cart" {
  begin = start
  cart = POST "/carts" {
    sample 201 { id: "c-1", total: 172.5, paid: false, owner: { name: "Ann" }, items: ["coffee"], coupon: null }
  }
}
"#;

/// The ✓ on a row of *Output data*, in the Inspector.
const CHECK: &str = "Add a check";
/// The six words, in the order the window lists them.
const WORDS: [&str; 6] = ["string", "number", "boolean", "object", "array", "null"];
/// The words the Inspector used before: none of them names a type any more.
const OLD_WORDS: [&str; 6] = ["Text", "Number", "Yes / no", "Object", "List", "Null"];

/// `SOURCE` as the demo shop's scenario, on its board at `size`, never run.
fn opened(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), SOURCE).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    assert!(
        folder(&harness).fresh_run(&id).is_none(),
        "the scenario has never run"
    );
    (project, harness, id)
}

/// A simulation of the scenario, finished.
fn run(harness: &mut FolderHarness) {
    choose(harness, "Run simulation");
    let run = finish_run(harness);
    assert_eq!(run.status, RunStatus::Passed, "{:?}", run.error);
}

fn graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

fn node<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
}

/// The steps `after` has and `before` has not.
fn added<'a>(before: &Graph, after: &'a Graph) -> Vec<&'a Node> {
    after
        .nodes
        .iter()
        .filter(|node| before.nodes.iter().all(|old| old.id != node.id))
        .collect()
}

/// `cart` selected, the Inspector on *Output data*. A check added is
/// selected after its edit, so each action selects the step again.
fn output_data(harness: &mut FolderHarness, id: &str) {
    let step = node(&graph(harness, id), "cart").id.clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
    super::inspector_tab(harness, "Output data")
        .expect("the Inspector has an Output data tab")
        .click();
    settle(harness);
}

/// The ✓ on the row of `field` in the Inspector's tree: the nearest one
/// under the field's label, which stands on the row above its value.
fn button<'a>(harness: &'a FolderHarness, field: &str) -> egui_kittest::Node<'a> {
    let edge = harness.get_by_label("Resize inspector").rect().left();
    let label = harness
        .query_all_by_label(field)
        .find(|node| node.rect().left() > edge)
        .unwrap_or_else(|| panic!("the Inspector shows the field {field}"));
    let y = label.rect().center().y;
    harness
        .query_all_by_label(CHECK)
        .filter(|button| button.rect().left() > edge && button.rect().center().y > y)
        .min_by(|a, b| (a.rect().center().y - y).total_cmp(&(b.rect().center().y - y)))
        .unwrap_or_else(|| panic!("the {field} row has {CHECK}"))
}

/// Opens the menu *Add a check* of `field` in the Inspector.
fn open_menu(harness: &mut FolderHarness, field: &str) {
    button(harness, field).scroll_to_me();
    settle(harness);
    button(harness, field).click();
    settle(harness);
    assert!(
        has_label(harness, "Add a check menu"),
        "the ✓ on {field} opens the menu Add a check"
    );
}

/// Chooses `item` in the open menu.
fn pick(harness: &mut FolderHarness, item: &str) {
    harness
        .query_by_label(item)
        .unwrap_or_else(|| panic!("the menu offers {item}"))
        .click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
}

/// Closes the open menu without choosing.
fn close_menu(harness: &mut FolderHarness) {
    harness.key_press(egui::Key::Escape);
    settle(harness);
    assert!(
        !has_label(harness, "Add a check menu"),
        "Escape closes the menu"
    );
}

/// `cart` → *Output data* → ✓ on `field` → `item`: the one step it adds.
fn add(harness: &mut FolderHarness, id: &str, field: &str, item: &str) -> Node {
    output_data(harness, id);
    let before = graph(harness, id);
    open_menu(harness, field);
    pick(harness, item);
    let after = graph(harness, id);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "{item} adds one step: {new:?}");
    new[0].clone()
}

/// `check` is a check of `cart.body.<field>` with `predicate` and, when the
/// predicate takes one, the literal `expected`.
fn assert_check(
    harness: &FolderHarness,
    id: &str,
    check: &Node,
    (field, predicate): (&str, Predicate),
    expected: Option<serde_json::Value>,
) {
    let cart = node(&graph(harness, id), "cart").id.clone();
    assert_eq!(check.kind, NodeKind::Assert, "the step is a check");
    assert_eq!(
        apiforge_core::graph::predicate(check),
        predicate,
        "{field}'s check"
    );
    assert!(
        matches!(
            check.inputs.get("actual"),
            Some(Binding::Reference { node_id, port, path })
                if *node_id == cart && port == "body" && path == field
        ),
        "the check reads cart.body.{field}: {:?}",
        check.inputs
    );
    assert_eq!(
        check.inputs.get("expected"),
        expected.map(|value| Binding::Literal { value }).as_ref(),
        "{field}'s check expects"
    );
}

/// The project saved, then read back as `apiforge check` reads it; the
/// saved scenario.
fn save_and_check(harness: &mut FolderHarness, root: &std::path::Path) -> String {
    choose(harness, "Save");
    settle(harness);
    let session = apiforge_project::ProjectSession::open_saved_read_only(root).unwrap();
    for document in session.documents() {
        let (document, valid) =
            apiforge_mcp::folder::check_document(&session, document.id()).unwrap();
        assert!(
            valid,
            "{} passes check: {:?}\n{}",
            document.entry().path.as_str(),
            document.diagnostics(),
            document.text()
        );
    }
    fs::read_to_string(root.join(SCENARIO)).unwrap()
}

/// `SOURCE` with `lines` written after `cart`.
fn with(lines: &str) -> String {
    SOURCE.replace("  }\n}\n", &format!("  }}\n{lines}}}\n"))
}

/// Criterion 1: after a run, *is a number* on the row `total` adds one
/// check; the file gains `check_total = assert cart.body.total is "number"`
/// under the step, and passes `check`.
#[test]
fn diag08_criterion_1_type_check_from_a_field() {
    let (project, mut harness, id) = opened(WIDE);
    run(&mut harness);
    let check = add(&mut harness, &id, "total", "total is a number");
    assert_check(
        &harness,
        &id,
        &check,
        ("total", Predicate::Type),
        Some(serde_json::json!("number")),
    );
    assert_eq!(
        save_and_check(&mut harness, project.path()),
        with(
            "  @label(\"Check total\")\n  \
             check_total = assert cart.body.total is \"number\"\n"
        )
    );
}

/// Criterion 2: the type offered is the type of the value shown, in the
/// language's words; *is an array* writes `is "array"`.
#[test]
fn diag08_criterion_2_the_type_is_the_values() {
    let (_project, mut harness, id) = opened(WIDE);
    output_data(&mut harness, &id);
    for (field, item) in [
        ("id", "id is a string"),
        ("total", "total is a number"),
        ("paid", "paid is a boolean"),
        ("owner", "owner is an object"),
        ("items", "items is an array"),
        ("coupon", "coupon is null"),
    ] {
        open_menu(&mut harness, field);
        assert!(has_label(&harness, item), "{field}'s menu offers {item}");
        close_menu(&mut harness);
    }
    let check = add(&mut harness, &id, "items", "items is an array");
    assert_check(
        &harness,
        &id,
        &check,
        ("items", Predicate::Type),
        Some(serde_json::json!("array")),
    );
    let check = add(&mut harness, &id, "coupon", "coupon is null");
    assert_check(
        &harness,
        &id,
        &check,
        ("coupon", Predicate::Type),
        Some(serde_json::json!("null")),
    );
}

/// Criterion 3: *exists* and *is not empty* write `assert exists …` and
/// `assert nonempty …`; *equals* writes what *Add comparison* wrote: the
/// value shown, expected.
#[test]
fn diag08_criterion_3_the_other_checks() {
    let (project, mut harness, id) = opened(WIDE);
    run(&mut harness);
    let check = add(&mut harness, &id, "total", "total equals 172.5");
    assert_check(
        &harness,
        &id,
        &check,
        ("total", Predicate::Equals),
        Some(serde_json::json!(172.5)),
    );
    let check = add(&mut harness, &id, "id", "id exists");
    assert_check(&harness, &id, &check, ("id", Predicate::Exists), None);
    let check = add(&mut harness, &id, "items", "items is not empty");
    assert_check(&harness, &id, &check, ("items", Predicate::NotEmpty), None);
    assert_eq!(
        save_and_check(&mut harness, project.path()),
        with(
            "  @label(\"Check total\")\n  \
             check_total = assert cart.body.total == 172.5\n  \
             @label(\"Check id\")\n  \
             check_id = assert exists cart.body.id\n  \
             @label(\"Check items\")\n  \
             check_items = assert nonempty cart.body.items\n"
        )
    );
}

/// The checks line of `cart`'s card in the last frame, if it has one.
fn checks_line(harness: &mut FolderHarness, id: &str) -> Option<String> {
    harness.step();
    let cart = node(&graph(harness, id), "cart").id.clone();
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
        .debug_aggregate(&cart)
}

/// Criterion 4: each added check is one undo step, and appears on the
/// card's checks line, in the language's words.
#[test]
fn diag08_criterion_4_one_undo_step() {
    let (_project, mut harness, id) = opened(WIDE);
    assert!(
        folder(&harness).preferences.checks_collapsed,
        "checks are folded into the card's checks line"
    );
    assert_eq!(checks_line(&mut harness, &id), None, "cart has no check");
    for (field, item, line) in [
        ("total", "total is a number", "is \"number\""),
        ("id", "id exists", "exists"),
        ("items", "items is not empty", "nonempty"),
        ("total", "total equals 172.5", "== 172.5"),
    ] {
        let before = source_of(&harness, &id);
        add(&mut harness, &id, field, item);
        assert_ne!(source_of(&harness, &id), before, "{item} writes the file");
        let shown = checks_line(&mut harness, &id).unwrap_or_default();
        assert!(
            shown.contains(field) && shown.contains(line),
            "the card's checks line shows {item}: {shown:?}"
        );
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        settle(&mut harness);
        assert_eq!(
            source_of(&harness, &id),
            before,
            "one Undo takes {item} back"
        );
        assert_eq!(checks_line(&mut harness, &id), None, "and its line");
    }
}

/// Opens the list `name` of the Inspector and returns whether each of
/// `words` is offered and none of `OLD_WORDS` is; then closes it.
fn assert_lists_the_words(harness: &mut FolderHarness, name: &str, words: &[&str]) {
    harness.get_by_label(name).scroll_to_me();
    settle(harness);
    harness.get_by_label(name).click();
    settle(harness);
    for word in words {
        assert!(
            harness
                .query_all_by_label(word)
                .any(|node| node.accesskit_node().role() != egui::accesskit::Role::ComboBox),
            "{name} offers {word}"
        );
    }
    // The Library's block *Object* is a step, not a type: the list and the
    // Inspector stand right of the Inspector's edge.
    let edge = harness.get_by_label("Resize inspector").rect().left();
    for word in OLD_WORDS {
        assert!(
            harness
                .query_all_by_label(word)
                .all(|node| node.rect().left() < edge),
            "{name} does not say {word}"
        );
    }
    harness.key_press(egui::Key::Escape);
    settle(harness);
}

/// Criterion 5: the Inspector's *Expected* list of a *Has type* check, the
/// type list of a literal, the menu, the card's line, the Inspector's tree
/// and Run results use the language's six words.
#[test]
fn diag08_criterion_5_one_vocabulary() {
    let (_project, mut harness, id) = opened(WIDE);
    // The Inspector's tree: `object · 1`, `array · 1`.
    output_data(&mut harness, &id);
    for preview in ["object · 1", "array · 1"] {
        assert!(has_label(&harness, preview), "the tree says {preview}");
    }
    for old in ["Object · 1", "List · 1"] {
        assert!(!has_label(&harness, old), "the tree does not say {old}");
    }
    // The menu (criterion 2) and the card's line (criterion 4) say
    // `number`; the new check is selected, its Expected list is open.
    add(&mut harness, &id, "total", "total is a number");
    assert_lists_the_words(&mut harness, "Expected type", &WORDS);
    // A literal's type list: null for the empty value, then the five.
    add(&mut harness, &id, "total", "total equals 172.5");
    assert_lists_the_words(&mut harness, "Value type", &WORDS);
    // Run results: the step's data and the type check's requirement.
    run(&mut harness);
    choose(&mut harness, "Maximise run results");
    select(&mut harness, "cart");
    for summary in ["object (1)", "array (1)"] {
        assert!(has_label(&harness, summary), "Run results say {summary}");
    }
    for old in ["Object (1)", "List (1)"] {
        assert!(!has_label(&harness, old), "Run results do not say {old}");
    }
    select(&mut harness, "Check total");
    assert!(
        harness.query_by_label_contains("is number").is_some(),
        "the type check's requirement reads is number"
    );
}

/// Chooses the first row of a step named `name` in the step list of Run
/// results: its label, or its handle where it has none.
fn select(harness: &mut FolderHarness, name: &str) {
    let row = format!("{name} · ");
    let label = harness
        .query_all_by_label_contains(&row)
        .filter_map(|found| found.accesskit_node().label())
        .find(|label| label.starts_with(&row))
        .unwrap_or_else(|| panic!("{name} has a row in the step list"));
    harness.query_all_by_label(&label).next().unwrap().scroll_to_me();
    settle(harness);
    harness.query_all_by_label(&label).next().unwrap().click();
    settle(harness);
}

/// Criterion 6: the same menu on the same tree in Run results: *Check* on
/// `total` opens it, *is a number* adds the check, and the row reads
/// *Check added*.
#[test]
fn diag08_criterion_6_run_results() {
    let (project, mut harness, id) = opened(WIDE);
    run(&mut harness);
    choose(&mut harness, "Maximise run results");
    select(&mut harness, "cart");
    let before = graph(&harness, &id);
    // The *Check* level with the label `total` in Run results.
    harness
        .query_all_by_label("total")
        .find_map(|label| {
            let y = label.rect().center().y;
            harness
                .query_all_by_label("Check")
                .find(|check| (check.rect().center().y - y).abs() < 8.0)
        })
        .expect("the total row of Run results offers Check")
        .click();
    settle(&mut harness);
    assert!(
        has_label(&harness, "Add a check menu"),
        "Check opens the menu Add a check"
    );
    for item in [
        "total equals 172.5",
        "total is a number",
        "total exists",
        "total is not empty",
    ] {
        assert!(has_label(&harness, item), "the menu offers {item}");
    }
    pick(&mut harness, "total is a number");
    let after = graph(&harness, &id);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "one step added: {new:?}");
    assert_check(
        &harness,
        &id,
        new[0],
        ("total", Predicate::Type),
        Some(serde_json::json!("number")),
    );
    assert!(
        has_label(&harness, "Check added"),
        "the row reads Check added"
    );
    assert_eq!(
        save_and_check(&mut harness, project.path()),
        with(
            "  @label(\"Check total\")\n  \
             check_total = assert cart.body.total is \"number\"\n"
        )
    );
}

/// Criterion 7: before the first run the menu writes the same checks from
/// the step's sample, and the file passes `check`.
#[test]
fn diag08_criterion_7_before_a_run() {
    let (project, mut harness, id) = opened(WIDE);
    for (field, item, predicate, expected) in [
        (
            "total",
            "total is a number",
            Predicate::Type,
            Some(serde_json::json!("number")),
        ),
        (
            "total",
            "total equals 172.5",
            Predicate::Equals,
            Some(serde_json::json!(172.5)),
        ),
        ("id", "id exists", Predicate::Exists, None),
        ("items", "items is not empty", Predicate::NotEmpty, None),
    ] {
        let check = add(&mut harness, &id, field, item);
        assert_check(&harness, &id, &check, (field, predicate), expected);
    }
    assert!(
        folder(&harness).fresh_run(&id).is_none(),
        "the scenario has still never run"
    );
    let text = save_and_check(&mut harness, project.path());
    for line in [
        "= assert cart.body.total is \"number\"\n",
        "= assert cart.body.total == 172.5\n",
        "= assert exists cart.body.id\n",
        "= assert nonempty cart.body.items\n",
    ] {
        assert!(text.contains(line), "the file has {line:?}:\n{text}");
    }
}

/// Criterion 8: design review with the menu open on `total`, in the
/// Inspector at 960 × 540 and 1600 × 1000, the *Expected* list of the type
/// check, and the menu in Run results.
#[test]
#[ignore = "DIAG-08: design review capture; run with tools/gui/review.sh"]
fn review_diag08_check_menu() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        // Before a run, so that Run results leave the Inspector its height.
        let (_project, mut harness, id) = opened(size);
        output_data(&mut harness, &id);
        let edge = harness.get_by_label("Resize inspector").rect().left();
        let inspector = egui::Rect::from_min_max(egui::pos2(edge, 0.0), egui::pos2(size.x, size.y));
        crate::testing::capture(
            &mut harness,
            &format!("diag08-output-data-{suffix}"),
            Some(inspector),
        );
        button(&harness, "total").scroll_to_me();
        settle(&mut harness);
        button(&harness, "total").click();
        settle(&mut harness);
        crate::testing::capture(
            &mut harness,
            &format!("diag08-menu-inspector-{suffix}"),
            Some(inspector),
        );
        crate::testing::capture(&mut harness, &format!("diag08-menu-window-{suffix}"), None);
        drop(harness);

        // A type check written in the file, selected: its Expected list.
        let project = crate::testing::shop_project();
        let typed = with("  check_total = assert cart.body.total is \"number\"\n");
        fs::write(project.path().join(SCENARIO), typed).unwrap();
        let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
        app(&mut harness).review_open(SCENARIO, None);
        settle(&mut harness);
        let id = only(&harness, DocumentKind::Scenario);
        let check = node(&graph(&harness, &id), "check_total").id.clone();
        app(&mut harness).selections.insert(id.clone(), vec![check]);
        settle(&mut harness);
        harness.get_by_label("Expected type").scroll_to_me();
        settle(&mut harness);
        harness.get_by_label("Expected type").click();
        settle(&mut harness);
        crate::testing::capture(
            &mut harness,
            &format!("diag08-expected-type-{suffix}"),
            Some(inspector),
        );
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
        run(&mut harness);
        choose(&mut harness, "Maximise run results");
        select(&mut harness, "cart");
        crate::testing::capture(&mut harness, &format!("diag08-run-results-{suffix}"), None);
        let check = harness.query_all_by_label("total").find_map(|label| {
            let y = label.rect().center().y;
            harness
                .query_all_by_label("Check")
                .find(|check| (check.rect().center().y - y).abs() < 8.0)
        });
        if let Some(check) = check {
            check.click();
            settle(&mut harness);
            if has_label(&harness, "Add a check menu") {
                crate::testing::capture(
                    &mut harness,
                    &format!("diag08-run-results-menu-{suffix}"),
                    None,
                );
            }
        }
    }
}
