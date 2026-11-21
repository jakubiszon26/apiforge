//! DIAG-10 (#531): the Inspector's *Output data* reads as a tree. A field is
//! one row — fold mark, its own name, its value as the language writes it
//! and coloured by its type, ✓ and {} — 26 px high, nested 14 px a level.
//! An output's heading is its first row (no `$`), the note above is a mark
//! and a sentence, and a value cut at the panel's edge opens on a click.
//! On the request `order`, whose sample holds every kind of value.
use super::*;
use crate::dnd::Payload;
use crate::testing::MINIMUM;
use apiforge_core::format::{Binding, Graph, Node};
use apiforge_core::model::NodeKind;
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// The fixture `order`: seven fields, an object, a list of two objects, a
/// long text, a boolean and a null.
const ORDER: &str = r#"scenario "Order" {
  begin = start
  order = GET "/orders/o-1042" {
    sample 200 { id: "o-1042", total: 172.5, paid: true, coupon: null, customer: { name: "Anna Nowak", email: "anna.nowak@example.test" }, items: [{ productId: 123, name: "Kawa ziarnista", quantity: 2 }, { productId: 124, name: "Filtr papierowy", quantity: 1 }], note: "Leave the parcel with the neighbour at number 14 if nobody answers the door." }
  }
  found = assert order.status == 200
}
"#;

/// A response nested ten levels deep.
const DEEP: &str = r#"scenario "Deep" {
  begin = start
  order = GET "/deep" {
    sample 200 { l1: { l2: { l3: { l4: { l5: { l6: { l7: { l8: { l9: { l10: 1 } } } } } } } } } }
  }
}
"#;

/// `note` as the language writes it.
const NOTE: &str =
    "\"Leave the parcel with the neighbour at number 14 if nobody answers the door.\"";

const CHECK: &str = "Add a check";
const VARIABLE: &str = "Assign to variable";

/// The rows of `order`'s sample with `items` and `[0]` open: each name and
/// the value on its row.
const ROWS: [(&str, &str); 14] = [
    ("Response", "object · 7"),
    ("id", "\"o-1042\""),
    ("total", "172.5"),
    ("paid", "true"),
    ("coupon", "null"),
    ("customer", "object · 2"),
    ("items", "array · 2"),
    ("[0]", "object · 3"),
    ("productId", "123"),
    ("name", "\"Kawa ziarnista\""),
    ("quantity", "2"),
    ("[1]", "object · 3"),
    ("note", NOTE),
    ("HTTP status", "200"),
];

/// `source` as the demo shop's scenario, on its board at `size`, never run.
fn opened(size: egui::Vec2, source: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
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

/// The step `handle` selected.
fn select(harness: &mut FolderHarness, id: &str, handle: &str) {
    let step = node(&graph(harness, id), handle).id.clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
}

/// The step `handle` selected, the Inspector on *Output data*.
fn output_data(harness: &mut FolderHarness, id: &str, handle: &str) {
    select(harness, id, handle);
    super::inspector_tab(harness, "Output data")
        .expect("the Inspector has an Output data tab")
        .click();
    settle(harness);
}

/// A click of the primary button at `at`; the pointer then leaves.
fn click_at(harness: &mut FolderHarness, at: egui::Pos2) {
    harness.hover_at(at);
    harness.drag_at(at);
    harness.drop_at(at);
    settle(harness);
}

/// The Inspector's left edge.
fn edge(harness: &FolderHarness) -> f32 {
    harness.get_by_label("Resize inspector").rect().left()
}

/// The Inspector, from its edge to the window's right and bottom.
fn panel(harness: &FolderHarness) -> egui::Rect {
    let window = harness.ctx.content_rect();
    egui::Rect::from_min_max(egui::pos2(edge(harness), window.top()), window.max)
}

/// The name `field` on its row in the Inspector.
fn name(harness: &FolderHarness, field: &str) -> egui::Rect {
    let edge = edge(harness);
    harness
        .query_all_by_label(field)
        .map(|node| node.rect())
        .find(|rect| rect.left() > edge)
        .unwrap_or_else(|| panic!("the Inspector shows a row named {field}"))
}

/// Whether the Inspector shows a row named `field`.
fn shows(harness: &FolderHarness, field: &str) -> bool {
    let edge = edge(harness);
    harness
        .query_all_by_label(field)
        .any(|node| node.rect().left() > edge)
}

/// The widget labelled `label` on the row whose name is at `row`.
fn on_row<'a>(
    harness: &'a FolderHarness,
    row: egui::Rect,
    label: &'a str,
) -> Option<egui_kittest::Node<'a>> {
    harness
        .query_all_by_label(label)
        .filter(|node| (node.rect().center().y - row.center().y).abs() < 13.0)
        .min_by(|a, b| {
            let a = (a.rect().center().y - row.center().y).abs();
            let b = (b.rect().center().y - row.center().y).abs();
            a.total_cmp(&b)
        })
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

/// ✓ and {} on the row of `field`.
fn actions(harness: &FolderHarness, field: &str) -> (egui::Rect, egui::Rect) {
    let row = name(harness, field);
    let check = on_row(harness, row, CHECK)
        .unwrap_or_else(|| panic!("the row {field} has {CHECK}"))
        .rect();
    let variable = on_row(harness, row, VARIABLE)
        .unwrap_or_else(|| panic!("the row {field} has {VARIABLE}"))
        .rect();
    (check, variable)
}

/// Clicks the fold mark `mark` (*Expand field* or *Collapse field*) on the
/// row of `field`.
fn fold(harness: &mut FolderHarness, field: &str, mark: &str) {
    let row = name(harness, field);
    let toggle = on_row(harness, row, mark).unwrap_or_else(|| panic!("the row {field} has {mark}"));
    assert!(
        toggle.rect().right() <= row.left(),
        "the fold mark stands left of {field}'s name"
    );
    toggle.click();
    harness.remove_cursor();
    settle(harness);
}

/// The text shapes of the last frame right of the Inspector's edge whose
/// text is `text`, with where they are drawn.
fn painted(harness: &FolderHarness, text: &str) -> Vec<(egui::Rect, std::sync::Arc<egui::Galley>)> {
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
    let edge = edge(harness);
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, text, &mut found);
    }
    found.retain(|(rect, _)| rect.left() > edge);
    found
}

/// The colour `value` is drawn in on the row of `field`.
fn colour(harness: &FolderHarness, field: &str, text: &str) -> egui::Color32 {
    let row = value(harness, field, text);
    let (_, galley) = painted(harness, text)
        .into_iter()
        .find(|(rect, _)| rect.intersects(row))
        .unwrap_or_else(|| panic!("{text} is drawn on the row {field}"));
    galley.job.sections[0].format.color
}

/// The galley `text` is drawn with on the row of `field`.
fn galley(harness: &FolderHarness, field: &str, text: &str) -> std::sync::Arc<egui::Galley> {
    let row = value(harness, field, text);
    painted(harness, text)
        .into_iter()
        .find(|(rect, _)| rect.intersects(row))
        .unwrap_or_else(|| panic!("{text} is drawn on the row {field}"))
        .1
}

/// Criterion 1: each field's name, value, ✓ and {} share one row at most
/// 26 px high; with the sample at 1600 × 1000 all fourteen rows are in the
/// panel without scrolling.
#[test]
fn diag10_criterion_1_a_field_on_one_line() {
    let (_project, mut harness, id) = opened(WIDE, ORDER);
    output_data(&mut harness, &id, "order");
    fold(&mut harness, "items", "Expand field");
    fold(&mut harness, "[0]", "Expand field");
    let panel = panel(&harness);
    for (field, shown) in ROWS {
        let row = name(&harness, field);
        let value = value(&harness, field, shown);
        let (check, variable) = actions(&harness, field);
        let whole = row.union(value).union(check).union(variable);
        assert!(
            whole.height() <= 26.0,
            "{field}'s name, value, ✓ and {{}} share one row at most 26 px high: {whole:?}"
        );
        assert!(
            panel.contains_rect(whole),
            "the row {field} is in the panel without scrolling: {whole:?} in {panel:?}"
        );
    }
}

/// Criterion 2: a child's name starts 14 px right of its parent's at every
/// level to ten; the second item of `items` is named `[1]`; pointing at
/// `name` under it shows `items[1].name` and `string`, and dragging it
/// carries the whole path.
#[test]
fn diag10_criterion_2_nesting_and_names() {
    let (_project, mut harness, id) = opened(WIDE, DEEP);
    output_data(&mut harness, &id, "order");
    let levels: Vec<String> = std::iter::once("Response".to_owned())
        .chain((1..=10).map(|level| format!("l{level}")))
        .collect();
    for level in &levels[1..10] {
        fold(&mut harness, level, "Expand field");
    }
    for pair in levels.windows(2) {
        let step = name(&harness, &pair[1]).left() - name(&harness, &pair[0]).left();
        assert!(
            (step - 14.0).abs() < 0.5,
            "{} starts 14 px right of {}: {step}",
            pair[1],
            pair[0]
        );
    }
    drop(harness);

    let (_project, mut harness, id) = opened(WIDE, ORDER);
    output_data(&mut harness, &id, "order");
    fold(&mut harness, "items", "Expand field");
    assert!(shows(&harness, "[0]") && shows(&harness, "[1]"));
    fold(&mut harness, "[1]", "Expand field");
    assert!(
        !shows(&harness, "items[1].name"),
        "a nested row shows its own name, not its path"
    );
    let path = |harness: &FolderHarness| {
        harness
            .query_all_by_label_contains("items[1].name")
            .filter_map(|node| node.accesskit_node().label())
            .find(|label| label.contains("string"))
    };
    let shown = crate::testing::hover_until(
        &mut harness,
        |harness| Some(name(harness, "name")),
        |harness| path(harness).is_some(),
    );
    assert!(shown, "pointing at name shows items[1].name and string");
    harness.remove_cursor();
    settle(&mut harness);

    // Dragging the name carries the whole path.
    let order = node(&graph(&harness, &id), "order").id.clone();
    let from = name(&harness, "name").center();
    harness.hover_at(from);
    harness.step();
    harness.drag_at(from);
    harness.step();
    for dx in [10.0, 30.0, 60.0] {
        harness.hover_at(from + egui::vec2(-dx, 0.0));
        harness.step();
    }
    let payload = egui::DragAndDrop::payload::<Payload>(&harness.ctx)
        .expect("dragging a name carries a payload");
    match payload.as_ref() {
        Payload::Variable { label, binding } => {
            assert_eq!(label, "items[1].name", "the drag is named by the path");
            assert_eq!(
                binding,
                &Binding::reference(&order, "body", "items[1].name"),
                "the drag reads order.body.items[1].name"
            );
        }
        _ => panic!("the drag carries a field"),
    }
    harness.key_press(egui::Key::Escape);
    harness.drop_at(from);
    settle(&mut harness);
}

/// Criterion 3: the value of `total` is drawn in the number colour, `id`
/// in the text colour, `paid` and `coupon` in the third; `customer` reads
/// `object · 2` and `items` `array · 2`.
#[test]
fn diag10_criterion_3_a_value_coloured_by_its_type() {
    let (_project, mut harness, id) = opened(WIDE, ORDER);
    output_data(&mut harness, &id, "order");
    fold(&mut harness, "items", "Expand field");
    fold(&mut harness, "[0]", "Expand field");
    let number = colour(&harness, "total", "172.5");
    let text = colour(&harness, "id", "\"o-1042\"");
    let literal = colour(&harness, "paid", "true");
    assert_eq!(
        colour(&harness, "coupon", "null"),
        literal,
        "true and null share the third colour"
    );
    assert_eq!(
        colour(&harness, "productId", "123"),
        number,
        "every number is drawn in the number colour"
    );
    assert_eq!(
        colour(&harness, "name", "\"Kawa ziarnista\""),
        text,
        "every text is drawn in the text colour"
    );
    for (one, other, what) in [
        (number, text, "a number and a text"),
        (number, literal, "a number and a literal"),
        (text, literal, "a text and a literal"),
    ] {
        assert_ne!(one, other, "{what} are drawn in different colours");
    }
    for colour in [number, text, literal] {
        assert_ne!(colour, crate::theme::MUTED, "a value is not muted any more");
        assert_ne!(colour, crate::theme::TEXT, "a value is coloured");
    }
    value(&harness, "customer", "object · 2");
    value(&harness, "items", "array · 2");
}

/// Criterion 4: `note` is cut with an ellipsis inside the panel and ✓ and
/// {} stay within the window at 960 × 540 and 1600 × 1000; pointing at the
/// value shows all of it.
#[test]
fn diag10_criterion_4_a_long_value_is_cut() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = opened(size, ORDER);
        output_data(&mut harness, &id, "order");
        harness
            .query_all_by_label(NOTE)
            .find(|node| node.rect().left() > edge(&harness))
            .expect("the note's value is shown")
            .scroll_to_me();
        settle(&mut harness);
        let window = harness.ctx.content_rect();
        let shown = value(&harness, "note", NOTE);
        let (check, variable) = actions(&harness, "note");
        assert!(
            galley(&harness, "note", NOTE).elided,
            "the note is cut with an ellipsis at {size:?}"
        );
        assert!(
            panel(&harness).contains_rect(shown) && shown.right() <= check.left(),
            "the cut note stays inside the panel, left of ✓, at {size:?}: {shown:?}, {check:?}"
        );
        for (button, rect) in [(CHECK, check), (VARIABLE, variable)] {
            assert!(
                window.contains_rect(rect),
                "{button} on note is within the window at {size:?}: {rect:?}"
            );
        }
        let whole = |harness: &FolderHarness| harness.query_all_by_label_contains(NOTE).count();
        let before = whole(&harness);
        let hovered = crate::testing::hover_until(
            &mut harness,
            |harness| Some(value(harness, "note", NOTE)),
            |harness| whole(harness) > before,
        );
        assert!(hovered, "pointing at the note shows all of it at {size:?}");
    }
}

/// Criterion 5: clicking ▸ on `customer` shows its two fields; clicking ▾
/// hides them; the state survives selecting another step and returning.
#[test]
fn diag10_criterion_5_folding() {
    let (_project, mut harness, id) = opened(WIDE, ORDER);
    output_data(&mut harness, &id, "order");
    assert!(!shows(&harness, "email"), "customer starts folded");
    fold(&mut harness, "customer", "Expand field");
    value(&harness, "name", "\"Anna Nowak\"");
    value(&harness, "email", "\"anna.nowak@example.test\"");
    fold(&mut harness, "customer", "Collapse field");
    assert!(
        !shows(&harness, "email") && !shows(&harness, "name"),
        "▾ hides customer's fields"
    );
    fold(&mut harness, "customer", "Expand field");
    select(&mut harness, &id, "found");
    assert!(!shows(&harness, "customer"), "another step is shown");
    output_data(&mut harness, &id, "order");
    assert!(
        shows(&harness, "email"),
        "customer is still open after selecting another step and returning"
    );
}

/// Criterion 6: the rows keep the accessible names of ✓ and {}, and a name
/// dragged carries the field as before. DIAG-08's and DIAG-09's tests run
/// unchanged beside these.
#[test]
fn diag10_criterion_6_the_actions_keep_their_names() {
    let (_project, mut harness, id) = opened(WIDE, ORDER);
    output_data(&mut harness, &id, "order");
    for (field, _) in ROWS
        .iter()
        .filter(|(field, _)| !["[0]", "[1]", "productId", "name", "quantity"].contains(field))
    {
        let row = name(&harness, field);
        for button in [CHECK, VARIABLE] {
            let found = on_row(&harness, row, button)
                .unwrap_or_else(|| panic!("the row {field} has {button}"));
            assert_eq!(
                found.accesskit_node().role(),
                egui::accesskit::Role::Button,
                "{button} on {field} is a button"
            );
        }
    }
    let order = node(&graph(&harness, &id), "order").id.clone();
    let from = name(&harness, "total").center();
    harness.hover_at(from);
    harness.step();
    harness.drag_at(from);
    harness.step();
    for dx in [10.0, 30.0, 60.0] {
        harness.hover_at(from + egui::vec2(-dx, 0.0));
        harness.step();
    }
    let payload = egui::DragAndDrop::payload::<Payload>(&harness.ctx)
        .expect("dragging a name carries a payload");
    assert!(
        matches!(
            payload.as_ref(),
            Payload::Variable { label, binding }
                if label == "total" && *binding == Binding::reference(&order, "body", "total")
        ),
        "the drag carries order.body.total"
    );
    harness.key_press(egui::Key::Escape);
    harness.drop_at(from);
    settle(&mut harness);
}

/// Criterion 7: before a run the note is *Sample*; after one, *Last run*.
#[test]
fn diag10_criterion_7_the_mark() {
    let (_project, mut harness, id) = opened(WIDE, ORDER);
    output_data(&mut harness, &id, "order");
    assert!(shows(&harness, "Sample"), "before a run the mark is Sample");
    assert!(shows(&harness, "Not from an API call."));
    assert!(!shows(&harness, "Last run"));
    run(&mut harness);
    output_data(&mut harness, &id, "order");
    assert!(
        shows(&harness, "Last run"),
        "after a run the mark is Last run"
    );
    assert!(shows(&harness, "Editing the step does not update it."));
    assert!(!shows(&harness, "Sample"));
}

/// Criterion 8: an output's heading is its first row: there is no row named
/// `$`; ✓ and {} on *Response* act on the whole value; ✓ and {} are on
/// every row without pointing.
#[test]
fn diag10_criterion_8_the_heading_is_the_row() {
    let (_project, mut harness, id) = opened(WIDE, ORDER);
    output_data(&mut harness, &id, "order");
    harness.remove_cursor();
    settle(&mut harness);
    assert!(!shows(&harness, "$"), "there is no row named $");
    value(&harness, "Response", "object · 7");
    value(&harness, "HTTP status", "200");
    let rows = [
        "Response",
        "id",
        "total",
        "paid",
        "coupon",
        "customer",
        "items",
        "note",
        "HTTP status",
    ];
    let edge = edge(&harness);
    for button in [CHECK, VARIABLE] {
        assert_eq!(
            harness
                .query_all_by_label(button)
                .filter(|node| node.rect().left() > edge)
                .count(),
            rows.len(),
            "{button} is on every row without pointing"
        );
    }
    for field in rows {
        actions(&harness, field);
    }

    // ✓ on Response checks the whole value.
    let order = node(&graph(&harness, &id), "order").id.clone();
    let before = graph(&harness, &id);
    on_row(&harness, name(&harness, "Response"), CHECK)
        .unwrap()
        .click();
    settle(&mut harness);
    harness
        .query_by_label("body is an object")
        .expect("the menu of Response checks the whole body")
        .click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(&mut harness);
    let after = graph(&harness, &id);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "one check added: {new:?}");
    assert_eq!(new[0].kind, NodeKind::Assert);
    assert_eq!(
        new[0].inputs.get("actual"),
        Some(&Binding::reference(&order, "body", "")),
        "the check reads the whole of order.body"
    );

    // {} on Response assigns the whole value.
    output_data(&mut harness, &id, "order");
    let before = graph(&harness, &id);
    on_row(&harness, name(&harness, "Response"), VARIABLE)
        .unwrap()
        .click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(&mut harness);
    assert!(
        has_label(&harness, "Assign variable"),
        "the dialog to name the variable opens"
    );
    choose(&mut harness, "Assign variable");
    let after = graph(&harness, &id);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "one variable added: {new:?}");
    assert!(
        new[0]
            .inputs
            .values()
            .any(|binding| *binding == Binding::reference(&order, "body", "")),
        "the variable reads the whole of order.body: {:?}",
        new[0].inputs
    );
}

/// Criterion 9: a click on the cut value of `note` shows all of it, wrapped
/// inside the panel, with ✓ and {} still in the window; a second click cuts
/// it again; a value that fits does not react.
#[test]
fn diag10_criterion_9_a_long_value_opens_on_a_click() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = opened(size, ORDER);
        output_data(&mut harness, &id, "order");
        let click = |harness: &mut FolderHarness, field: &str, text: &str| {
            let at = value(harness, field, text).center();
            click_at(harness, at);
        };
        let scroll = |harness: &mut FolderHarness| {
            harness
                .query_all_by_label(NOTE)
                .find(|node| node.rect().left() > edge(harness))
                .expect("the note's value is shown")
                .scroll_to_me();
            settle(harness);
        };
        scroll(&mut harness);
        let cut = value(&harness, "note", NOTE);
        click(&mut harness, "note", NOTE);
        scroll(&mut harness);
        let open = value(&harness, "note", NOTE);
        let (check, variable) = actions(&harness, "note");
        let window = harness.ctx.content_rect();
        let whole = galley(&harness, "note", NOTE);
        assert!(
            !whole.elided && whole.rows.len() > 1,
            "a click shows all of the note over several lines at {size:?}"
        );
        assert!(
            open.height() > 26.0,
            "the open note is taller than a row: {open:?}"
        );
        assert!(
            panel(&harness).contains_rect(open) && open.right() <= check.left(),
            "the open note wraps inside the panel, left of ✓, at {size:?}: {open:?}"
        );
        for rect in [check, variable] {
            assert!(window.contains_rect(rect), "✓ and {{}} stay in the window");
        }
        click(&mut harness, "note", NOTE);
        scroll(&mut harness);
        assert!(
            galley(&harness, "note", NOTE).elided,
            "a second click cuts the note again at {size:?}"
        );
        assert!(
            (value(&harness, "note", NOTE).height() - cut.height()).abs() < 0.5,
            "the row is one line again"
        );
        let fits = value(&harness, "id", "\"o-1042\"");
        click(&mut harness, "id", "\"o-1042\"");
        assert_eq!(
            value(&harness, "id", "\"o-1042\""),
            fits,
            "a value that fits does not react"
        );
        assert!(!galley(&harness, "id", "\"o-1042\"").elided);
    }
}

/// Criterion 10: design review of *Output data* at 960 × 540 and
/// 1600 × 1000, to be measured against the decided board
/// (`docs/qa/2026-11-21/DIAG-10/`): the sample with `items` and `[0]` open,
/// the long value open, and the last run.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag10_output_data() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        let (_project, mut harness, id) = opened(size, ORDER);
        output_data(&mut harness, &id, "order");
        fold(&mut harness, "items", "Expand field");
        fold(&mut harness, "[0]", "Expand field");
        let inspector = panel(&harness);
        crate::testing::capture(&mut harness, &format!("diag10-window-{suffix}"), None);
        crate::testing::capture(
            &mut harness,
            &format!("diag10-inspector-{suffix}"),
            Some(inspector),
        );
        harness
            .query_all_by_label(NOTE)
            .find(|node| node.rect().left() > inspector.left())
            .expect("the note's value is shown")
            .scroll_to_me();
        settle(&mut harness);
        let at = value(&harness, "note", NOTE).center();
        click_at(&mut harness, at);
        crate::testing::capture(
            &mut harness,
            &format!("diag10-long-value-{suffix}"),
            Some(inspector),
        );
        let at = value(&harness, "note", NOTE).center();
        click_at(&mut harness, at);
        run(&mut harness);
        output_data(&mut harness, &id, "order");
        crate::testing::capture(&mut harness, &format!("diag10-after-run-{suffix}"), None);
    }
}
