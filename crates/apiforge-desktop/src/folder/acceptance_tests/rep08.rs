//! REP-08 (#455): in Run results a value opens to its leaves; *Tree | JSON*;
//! *Expand all* and *Collapse all* hold from step to step; *Copy* takes the
//! value as JSON. A row of the tree is DIAG-10's row (#531, criterion 7), its
//! *Check* and *To variable* in words where the panel has room. On the
//! `labels` reference scenario (FIX-01) and on DIAG-10's fixture `order`.
use super::*;
use crate::testing::{MINIMUM, assert_within_window, labels_board};
use egui_kittest::kittest::NodeT;

const MAXIMISE: &str = "Maximise run results";
const TREE: &str = "Show as tree";
const JSON: &str = "Show as JSON";
const EXPAND_ALL: &str = "Expand all";
const COLLAPSE_ALL: &str = "Collapse all";
const COPY: &str = "Copy as JSON";
const CHECK: &str = "Check";
const TO_VARIABLE: &str = "To variable";

/// The request of criterion 1 and its error.
const WEIGHT_31: &str = "Weight 31 kg";
const WEIGHT_0: &str = "Weight 0 kg";
const ERROR: [(&str, &str); 3] = [
    ("code", "\"out_of_range\""),
    ("field", "\"weight\""),
    ("message", "\"Weight must be at most 30 kg.\""),
];

/// DIAG-10's fixture `order`: seven fields, an object, a list of two
/// objects, a long text, a boolean and a null.
const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const ORDER: &str = r#"scenario "Order" {
  begin = start
  order = GET "/orders/o-1042" {
    sample 200 { id: "o-1042", total: 172.5, paid: true, coupon: null, customer: { name: "Anna Nowak", email: "anna.nowak@example.test" }, items: [{ productId: 123, name: "Kawa ziarnista", quantity: 2 }, { productId: 124, name: "Filtr papierowy", quantity: 1 }], note: "Leave the parcel with the neighbour at number 14 if nobody answers the door." }
  }
  found = assert order.status == 200
}
"#;
const NOTE: &str =
    "\"Leave the parcel with the neighbour at number 14 if nobody answers the door.\"";

/// The rows of `order`'s body as Run results open it, with no click: each
/// name and the value on its row.
const ROWS: [(&str, &str); 13] = [
    ("body", "object · 7"),
    ("id", "\"o-1042\""),
    ("total", "172.5"),
    ("paid", "true"),
    ("coupon", "null"),
    ("customer", "object · 2"),
    ("email", "\"anna.nowak@example.test\""),
    ("items", "array · 2"),
    ("[0]", "object · 3"),
    ("productId", "123"),
    ("quantity", "2"),
    ("[1]", "object · 3"),
    ("note", NOTE),
];

/// `labels` after one simulation, its Run results maximised.
fn labels_after_a_run(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = labels_board(size);
    run(&mut harness);
    (project, harness, id)
}

/// `order` in the demo shop after one simulation, its Run results
/// maximised on the step `order`.
fn order_after_a_run(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), ORDER).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    run(&mut harness);
    select_step(&mut harness, "order");
    (project, harness, id)
}

/// A simulation of the active scenario, passed, with Run results maximised.
fn run(harness: &mut FolderHarness) {
    choose(harness, "Run simulation");
    let report = finish_run(harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    choose(harness, MAXIMISE);
}

/// Chooses the step `step` in the list of Run results, as a click on its row.
fn select_step(harness: &mut FolderHarness, step: &str) {
    let label = harness
        .query_all_by_label_contains(&format!("{step} · "))
        .find(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
        .and_then(|node| node.accesskit_node().label())
        .unwrap_or_else(|| panic!("Run results list the step {step}"));
    harness.get_by_label(&label).scroll_to_me();
    settle(harness);
    harness.get_by_label(&label).click();
    settle(harness);
}

/// Where the selected step's details start: the left of its first tab.
fn details(harness: &FolderHarness) -> f32 {
    harness
        .query_all_by_label_contains("Outputs  ")
        .map(|node| node.rect().left())
        .next()
        .expect("the step's details show their Outputs tab")
}

/// Chooses the details' tab whose label starts with `tab`.
fn open_tab(harness: &mut FolderHarness, tab: &str) {
    let label = harness
        .query_all_by_label_contains(tab)
        .filter_map(|node| node.accesskit_node().label())
        .find(|label| label.starts_with(tab))
        .unwrap_or_else(|| panic!("the details have a tab {tab}"));
    harness.get_by_label(&label).click();
    settle(harness);
}

/// The name `field` on its row in the details, if it is shown.
fn row_named(harness: &FolderHarness, field: &str) -> Option<egui::Rect> {
    let left = details(harness);
    harness
        .query_all_by_label(field)
        .map(|node| node.rect())
        .find(|rect| rect.left() >= left)
}

fn name(harness: &FolderHarness, field: &str) -> egui::Rect {
    row_named(harness, field).unwrap_or_else(|| panic!("the details show a row named {field}"))
}

/// The value `value` on the row of `field`, right of its name.
fn value(harness: &FolderHarness, field: &str, value: &str) -> egui::Rect {
    let row = name(harness, field);
    harness
        .query_all_by_label(value)
        .map(|node| node.rect())
        .filter(|rect| rect.left() >= row.right())
        .find(|rect| (rect.top() - row.top()).abs() < 13.0)
        .unwrap_or_else(|| panic!("the row {field} reads {value}"))
}

/// The widget labelled `label` on the row whose name is at `row`.
fn on_row(harness: &FolderHarness, row: egui::Rect, label: &str) -> Option<egui::Rect> {
    harness
        .query_all_by_label(label)
        .map(|node| node.rect())
        .filter(|rect| (rect.center().y - row.center().y).abs() < 13.0)
        .min_by(|a, b| {
            let a = (a.center().y - row.center().y).abs();
            let b = (b.center().y - row.center().y).abs();
            a.total_cmp(&b)
        })
}

/// *Check* and *To variable* on the row of `field`.
fn actions(harness: &FolderHarness, field: &str) -> (egui::Rect, egui::Rect) {
    let row = name(harness, field);
    let check =
        on_row(harness, row, CHECK).unwrap_or_else(|| panic!("the row {field} has {CHECK}"));
    let variable = on_row(harness, row, TO_VARIABLE)
        .unwrap_or_else(|| panic!("the row {field} has {TO_VARIABLE}"));
    (check, variable)
}

/// A click of the primary button at `at`; the pointer then leaves.
fn click_at(harness: &mut FolderHarness, at: egui::Pos2) {
    harness.hover_at(at);
    harness.drag_at(at);
    harness.drop_at(at);
    harness.remove_cursor();
    settle(harness);
}

/// The text shapes of the last frame whose text is `text`, right of
/// `left`, with the galley they are drawn with.
fn painted(
    harness: &FolderHarness,
    text: &str,
    left: f32,
) -> Vec<(egui::Rect, std::sync::Arc<egui::Galley>)> {
    fn walk(
        shape: &egui::Shape,
        text: &str,
        found: &mut Vec<(egui::Rect, std::sync::Arc<egui::Galley>)>,
    ) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    walk(shape, text, found);
                }
            }
            egui::Shape::Text(shape) if shape.galley.job.text == text => {
                found.push((shape.visual_bounding_rect(), shape.galley.clone()));
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, text, &mut found);
    }
    found.retain(|(rect, _)| rect.left() >= left);
    found
}

/// The galley `text` is drawn with on the row of `field`.
fn galley(harness: &FolderHarness, field: &str, text: &str) -> std::sync::Arc<egui::Galley> {
    let row = value(harness, field, text);
    painted(harness, text, details(harness))
        .into_iter()
        .find(|(rect, _)| rect.intersects(row))
        .unwrap_or_else(|| panic!("{text} is drawn on the row {field}"))
        .1
}

/// The outputs of the step `handle` in the last run, as JSON.
fn outputs(harness: &FolderHarness, id: &str, handle: &str) -> serde_json::Value {
    let snapshot = folder(harness).snapshot(id).unwrap();
    let graph = graph_of(snapshot.model.as_ref().unwrap(), false);
    let node = graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone();
    let report = folder(harness).run_result(id).unwrap();
    let step = report
        .steps
        .iter()
        .find(|step| step.node_id == node)
        .unwrap_or_else(|| panic!("the run has the step {handle}"));
    serde_json::to_value(&step.outputs).unwrap()
}

/// Criterion 1: on `labels`, after a simulated run, selecting *Weight 31 kg*
/// shows `code`, `field` and `message` of its error with no further click.
#[test]
fn rep08_criterion_1_a_value_opens_to_its_leaves() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _id) = labels_after_a_run(size);
        select_step(&mut harness, WEIGHT_31);
        assert!(row_named(&harness, "error").is_some(), "error is shown");
        let window = harness.ctx.content_rect();
        for (field, shown) in ERROR {
            let row = name(&harness, field).union(value(&harness, field, shown));
            assert!(
                window.contains_rect(row),
                "{field} is in view at {size:?} without scrolling: {row:?}"
            );
        }
    }
}

/// Criterion 2: *Collapse all*, then selecting another request: its value is
/// folded too; *Expand all* likewise.
#[test]
fn rep08_criterion_2_expand_all_and_collapse_all_hold_between_steps() {
    let (_project, mut harness, _id) = labels_after_a_run(WIDE);
    select_step(&mut harness, WEIGHT_31);
    assert!(row_named(&harness, "code").is_some(), "open by default");
    choose(&mut harness, COLLAPSE_ALL);
    assert!(
        row_named(&harness, "error").is_none() && row_named(&harness, "code").is_none(),
        "Collapse all folds the value"
    );
    assert!(
        row_named(&harness, "body").is_some(),
        "the outputs' rows stay"
    );

    select_step(&mut harness, WEIGHT_0);
    assert!(
        row_named(&harness, "body").is_some(),
        "another request shows its outputs"
    );
    assert!(
        row_named(&harness, "error").is_none() && row_named(&harness, "code").is_none(),
        "another request's value is folded too"
    );

    choose(&mut harness, EXPAND_ALL);
    assert!(row_named(&harness, "code").is_some(), "Expand all opens it");
    select_step(&mut harness, WEIGHT_31);
    for (field, shown) in ERROR {
        value(&harness, field, shown);
    }
}

/// Criterion 3: *JSON* shows the value as indented JSON whose text equals
/// `serde_json::to_string_pretty` of the value; *Copy* puts that text on
/// the clipboard. The switch is remembered for the project on this
/// computer, beside the other panel choices.
#[test]
fn rep08_criterion_3_json_and_copy() {
    let (_project, mut harness, id) = labels_after_a_run(WIDE);
    select_step(&mut harness, WEIGHT_31);
    let expected = serde_json::to_string_pretty(&outputs(&harness, &id, "weight_31")).unwrap();
    assert!(
        expected.contains("\n  ") && expected.contains("out_of_range"),
        "indented JSON of the outputs: {expected}"
    );
    assert!(
        !folder(&harness).workbench.panels.results_json,
        "Tree is the first view"
    );
    choose(&mut harness, JSON);
    assert!(
        harness.query_by_label(&expected).is_some(),
        "JSON shows the pretty text of the value"
    );
    assert!(
        row_named(&harness, "code").is_none(),
        "the tree is not drawn beside it"
    );
    assert!(
        folder(&harness).workbench.panels.results_json,
        "the switch is kept with the project's view"
    );

    harness.get_by_label(COPY).click();
    harness.step();
    let copied =
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            });
    assert_eq!(
        copied.as_deref(),
        Some(expected.as_str()),
        "Copy puts the text on the clipboard"
    );
    settle(&mut harness);

    // The choice holds for another step, and back.
    select_step(&mut harness, WEIGHT_0);
    let other = serde_json::to_string_pretty(&outputs(&harness, &id, "weight_0")).unwrap();
    assert!(harness.query_by_label(&other).is_some(), "JSON holds");
    choose(&mut harness, TREE);
    assert!(row_named(&harness, "body").is_some(), "Tree again");
    assert!(!folder(&harness).workbench.panels.results_json);
}

/// Criterion 4: the switch and the buttons are on Outputs, Inputs and *HTTP
/// request*, with distinct accessible labels, within the window at 960 × 540
/// and 1600 × 1000.
#[test]
fn rep08_criterion_4_the_controls_on_every_tab() {
    let controls = [TREE, JSON, EXPAND_ALL, COLLAPSE_ALL, COPY];
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _id) = labels_after_a_run(size);
        select_step(&mut harness, WEIGHT_31);
        for tab in ["Outputs  ", "Inputs  ", "HTTP request"] {
            open_tab(&mut harness, tab);
            // `get_by_label` fails on a label that is not unique.
            assert_within_window(&harness, &controls);
            crate::testing::assert_disjoint(&harness, &controls);
        }
    }
}

/// Criterion 5: a value with more than 2 000 nodes opens to its second level
/// and a line says so. The opening rule itself is unit-tested beside it in
/// `report.rs`; here the window says so.
#[test]
fn rep08_criterion_5_a_large_value_opens_to_its_second_level() {
    let items: Vec<String> = (0..700)
        .map(|index| format!("{{ id: {index}, tags: [\"a\", \"b\"] }}"))
        .collect();
    let source = format!(
        "scenario \"Large\" {{\n  begin = start\n  order = GET \"/large\" {{\n    sample 200 {{ items: [{}] }}\n  }}\n}}\n",
        items.join(", ")
    );
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    run(&mut harness);
    select_step(&mut harness, "order");
    assert!(
        harness
            .query_all_by_label_contains("opened to its second level")
            .next()
            .is_some(),
        "a line says the value opened to its second level"
    );
    assert!(row_named(&harness, "items").is_some(), "the first level");
    assert!(row_named(&harness, "[0]").is_some(), "the second level");
    assert!(
        row_named(&harness, "tags").is_none(),
        "the third level stays folded"
    );
}

/// Criterion 5, the opening rule: a value opens to its leaves unless it has
/// more than 2 000 nodes, when it opens to its second level; *Expand all*
/// opens any value to its leaves and *Collapse all* folds every row.
#[test]
fn rep08_criterion_5_the_opening_rule() {
    use crate::report::{Folding, Opening, opening};
    use serde_json::json;
    let small = json!({ "error": { "code": "out_of_range", "items": [1, 2, 3] } });
    assert_eq!(opening(Folding::Default, &small), Opening::Leaves);
    assert_eq!(opening(Folding::ExpandAll, &small), Opening::Leaves);
    assert_eq!(opening(Folding::CollapseAll, &small), Opening::Closed);
    // The root, the list and 1 998 numbers: 2 000 nodes, not more.
    let at_the_limit = json!({ "items": (0..1998).collect::<Vec<_>>() });
    assert_eq!(opening(Folding::Default, &at_the_limit), Opening::Leaves);
    let over = json!({ "items": (0..1999).collect::<Vec<_>>() });
    assert_eq!(opening(Folding::Default, &over), Opening::Levels(2));
    assert_eq!(opening(Folding::ExpandAll, &over), Opening::Leaves);
    assert_eq!(opening(Folding::CollapseAll, &over), Opening::Closed);
    // A value opens to its leaves only where Run results draw all its open
    // rows at once (review round, 2026-11-23): 100 objects of 5 fields under
    // the root (601 rows) open to their second level, which draws 102.
    let wide = json!({
        "items": (0..150)
            .map(|id| json!({ "id": id, "name": "n", "price": 1.5, "stock": 3, "active": true }))
            .collect::<Vec<_>>()
    });
    assert_eq!(opening(Folding::Default, &wide), Opening::Levels(2));
    // Where the second level would not be drawn whole either, four lists of
    // 100 numbers under the root (405 rows), the value opens to its first.
    let lists = json!({
        "a": (0..100).collect::<Vec<_>>(),
        "b": (0..100).collect::<Vec<_>>(),
        "c": (0..100).collect::<Vec<_>>(),
        "d": (0..100).collect::<Vec<_>>(),
    });
    assert_eq!(opening(Folding::Default, &lists), Opening::Levels(1));
    // Three lists of 100 objects (904 rows open) draw 304 at their second.
    let objects = json!({
        "a": vec![json!({ "x": 1, "y": 2 }); 100],
        "b": vec![json!({ "x": 1, "y": 2 }); 100],
        "c": vec![json!({ "x": 1, "y": 2 }); 100],
    });
    assert_eq!(opening(Folding::Default, &objects), Opening::Levels(2));
    // What each opens: a row at `depth` is open, or folded.
    assert!(Opening::Leaves.open(0) && Opening::Leaves.open(9));
    assert!(Opening::Levels(2).open(1) && !Opening::Levels(2).open(2));
    assert!(!Opening::Closed.open(0));
}

/// Review round (2026-11-23): a value whose open rows are more than Run
/// results draw at once, a body of 150 objects of 5 fields, opens only as
/// far as its rows are drawn and a line says so; no row is hidden behind
/// the field display limit.
#[test]
fn rep08_a_value_opens_only_as_far_as_its_rows_are_drawn() {
    let items: Vec<String> = (0..150)
        .map(|index| {
            format!(
                "{{ id: {index}, name: \"n{index}\", price: 1.5, stock: 3, active: true }}"
            )
        })
        .collect();
    let source = format!(
        "scenario \"Wide\" {{\n  begin = start\n  order = GET \"/wide\" {{\n    sample 200 {{ items: [{}] }}\n  }}\n}}\n",
        items.join(", ")
    );
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    run(&mut harness);
    select_step(&mut harness, "order");
    assert!(
        harness
            .query_all_by_label_contains("Field display limit reached")
            .next()
            .is_none(),
        "no row is hidden behind the field display limit"
    );
    assert!(
        harness
            .query_all_by_label_contains("opened to its second level")
            .next()
            .is_some(),
        "a line says the value opened to its second level"
    );
    assert!(row_named(&harness, "items").is_some(), "the first level");
    for item in ["[0]", "[99]"] {
        assert!(
            row_named(&harness, item).is_some(),
            "the second level, {item} the last of its page included"
        );
    }
    assert!(
        row_named(&harness, "price").is_none(),
        "the third level stays folded"
    );
}

/// Review round (2026-11-23): *Expand all* and *Collapse all* hold from step
/// to step, not from run to run: a new run opens its values as by default.
#[test]
fn rep08_a_new_run_opens_its_values_as_by_default() {
    let (_project, mut harness, _id) = labels_after_a_run(WIDE);
    select_step(&mut harness, WEIGHT_31);
    choose(&mut harness, COLLAPSE_ALL);
    assert!(row_named(&harness, "code").is_none(), "Collapse all folds it");
    choose(&mut harness, "Run simulation");
    let report = finish_run(&mut harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    settle(&mut harness);
    select_step(&mut harness, WEIGHT_31);
    for (field, shown) in ERROR {
        value(&harness, field, shown);
    }
}

/// Criterion 7: a row of the tree is DIAG-10's row: one line at most 26 px
/// high, a child 14 px right of its parent, the value coloured by its type,
/// `object · 7` and `array · 2`, a long value opened by a click; *Check* and
/// *To variable* keep their words where the panel has room.
#[test]
fn rep08_criterion_7_the_row_is_diag10s() {
    let (_project, harness, _id) = order_after_a_run(WIDE);
    let window = harness.ctx.content_rect();
    for (field, shown) in ROWS {
        let row = name(&harness, field);
        let value = value(&harness, field, shown);
        let (check, variable) = actions(&harness, field);
        let whole = row.union(value).union(check).union(variable);
        assert!(
            whole.height() <= 26.0,
            "{field}'s name, value, Check and To variable share one row: {whole:?}"
        );
        assert!(
            window.contains_rect(whole),
            "the row {field} is in the window"
        );
    }
    // A child's name starts 14 px right of its parent's.
    for (parent, child) in [("body", "items"), ("items", "[0]"), ("[0]", "productId")] {
        let step = name(&harness, child).left() - name(&harness, parent).left();
        assert!(
            (step - 14.0).abs() < 0.5,
            "{child} starts 14 px right of {parent}: {step}"
        );
    }
    // The value coloured by its type.
    let left = details(&harness);
    let colour = |field: &str, text: &str| {
        let row = value(&harness, field, text);
        painted(&harness, text, left)
            .into_iter()
            .find(|(rect, _)| rect.intersects(row))
            .unwrap_or_else(|| panic!("{text} is drawn on the row {field}"))
            .1
            .job
            .sections[0]
            .format
            .color
    };
    assert_eq!(colour("total", "172.5"), theme::VALUE_NUMBER);
    assert_eq!(colour("id", "\"o-1042\""), theme::VALUE_STRING);
    assert_eq!(colour("paid", "true"), theme::VALUE_LITERAL);
    assert_eq!(colour("coupon", "null"), theme::VALUE_LITERAL);
    // The actions keep their words where the panel has room, right after
    // the value.
    let (check, variable) = actions(&harness, "id");
    assert!(
        check.width() > 40.0 && variable.width() > 40.0,
        "Check and To variable keep their words at 1600 × 1000: {check:?} {variable:?}"
    );
    let id_value = value(&harness, "id", "\"o-1042\"");
    assert!(
        check.left() >= id_value.right() && check.left() - id_value.right() < 40.0,
        "Check follows the value: {id_value:?} {check:?}"
    );
    assert!(
        painted(&harness, CHECK, left).len() >= ROWS.len(),
        "the word Check is drawn on the rows"
    );
    drop(harness);

    // A long value is cut at the panel's edge and opened by a click.
    let (_project, mut harness, _id) = order_after_a_run(MINIMUM);
    let note = || NOTE;
    harness
        .query_all_by_label(note())
        .next()
        .expect("the note's value is shown")
        .scroll_to_me();
    settle(&mut harness);
    assert!(
        galley(&harness, "note", NOTE).elided,
        "the note is cut at 960 × 540"
    );
    let at = value(&harness, "note", NOTE).center();
    click_at(&mut harness, at);
    harness
        .query_all_by_label(note())
        .next()
        .unwrap()
        .scroll_to_me();
    settle(&mut harness);
    let whole = galley(&harness, "note", NOTE);
    assert!(
        !whole.elided && whole.rows.len() > 1,
        "a click shows all of the note over several lines"
    );
    let window = harness.ctx.content_rect();
    let (check, variable) = actions(&harness, "note");
    for rect in [check, variable] {
        assert!(window.contains_rect(rect), "the actions stay in the window");
    }
}

/// Criterion 8 (design review): Run results on `Weight 31 kg` and on
/// `order` at 960 × 540 and 1600 × 1000, in Tree and in JSON, beside the
/// decided boards `docs/qa/2026-11-19/owner-run/m2.png` and
/// `docs/qa/2026-11-21/DIAG-10/run-results.png`.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rep08_run_results() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        let (_project, mut harness, _id) = labels_after_a_run(size);
        select_step(&mut harness, WEIGHT_31);
        crate::testing::capture(&mut harness, &format!("rep08-labels-tree-{suffix}"), None);
        choose(&mut harness, JSON);
        crate::testing::capture(&mut harness, &format!("rep08-labels-json-{suffix}"), None);
        choose(&mut harness, TREE);
        drop(harness);

        let (_project, mut harness, _id) = order_after_a_run(size);
        crate::testing::capture(&mut harness, &format!("rep08-order-tree-{suffix}"), None);
    }
}
