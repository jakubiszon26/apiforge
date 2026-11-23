//! DIAG-15 (#547): the *Checks* tab of a step adds a check. *Add check* is
//! the tab's first line, with and without checks; it opens *Add a check
//! of*, the step's outputs as *Output data* writes them, and a field there
//! opens DIAG-08's menu of four checks. At 960 × 540 the list folds and
//! scrolls inside the window. A check's card in the list selects the check.
use super::*;
use crate::testing::{MINIMUM, assert_disjoint, assert_within_window};
use apiforge_core::format::{Binding, Graph, Node};
use apiforge_core::model::{NodeKind, Predicate};
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// DIAG-08's request: a sample with a value of each of the six types.
const SOURCE: &str = r#"scenario "A cart" {
  begin = start
  cart = POST "/carts" {
    sample 201 { id: "c-1", total: 172.5, paid: false, owner: { name: "Ann" }, items: ["coffee"], coupon: null }
  }
}
"#;

/// The same request with two checks.
const CHECKS: &str = "  status_created = assert cart.status == 201\n  \
                      total_value = assert cart.body.total == 172.5\n";

/// The same request with no sample, never run.
const NO_SAMPLE: &str = r#"scenario "A cart" {
  begin = start
  cart = POST "/carts" {
  }
}
"#;

/// DIAG-10's fixture `order`: a larger sample, nested.
const ORDER: &str = r#"scenario "Order" {
  begin = start
  order = GET "/orders/o-1042" {
    sample 200 { id: "o-1042", total: 172.5, paid: true, coupon: null, customer: { name: "Anna Nowak", email: "anna.nowak@example.test" }, items: [{ productId: 123, name: "Kawa ziarnista", quantity: 2 }, { productId: 124, name: "Filtr papierowy", quantity: 1 }], note: "Leave the parcel with the neighbour at number 14 if nobody answers the door." }
  }
  found = assert order.status == 200
}
"#;

/// The button on the tab's first line.
const ADD: &str = "Add check";
/// The menu it opens.
const MENU: &str = "Add check menu";
/// DIAG-08's menu of one field.
const FIELD_MENU: &str = "Add a check menu";
/// What the menu says with no sample and no run (criterion 5).
const NO_FIELDS: &str = "Run the request or add a sample response to list its fields.";
/// The hover of a disabled button on a board whose code has errors.
const LAST_VALID: &str =
    "The code has errors. This is the last valid version; it is read-only until the code is fixed.";

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

/// `SOURCE` with `lines` written after `cart`.
fn with(lines: &str) -> String {
    SOURCE.replace("  }\n}\n", &format!("  }}\n{lines}}}\n"))
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

/// The ids of the selected steps.
fn selected(harness: &mut FolderHarness, id: &str) -> Vec<String> {
    app(harness).selections.get(id).cloned().unwrap_or_default()
}

/// The step `handle` selected, the Inspector on its *Checks* tab.
fn checks_tab(harness: &mut FolderHarness, id: &str, handle: &str) {
    let step = node(&graph(harness, id), handle).id.clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
    super::open_checks_tab(harness);
}

/// The step `handle` selected, the Inspector on *Output data*.
fn output_data(harness: &mut FolderHarness, id: &str, handle: &str) {
    let step = node(&graph(harness, id), handle).id.clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
    super::inspector_tab(harness, "Output data")
        .expect("the Inspector has an Output data tab")
        .click();
    settle(harness);
}

/// Whether some widget is labelled `label`.
fn shows(harness: &FolderHarness, label: &str) -> bool {
    harness.query_all_by_label(label).next().is_some()
}

/// The Inspector's left edge.
fn edge(harness: &FolderHarness) -> f32 {
    harness.get_by_label("Resize inspector").rect().left()
}

/// *Add check* in the Inspector.
fn add_button(harness: &FolderHarness) -> Option<egui_kittest::Node<'_>> {
    let edge = edge(harness);
    harness
        .query_all_by_label(ADD)
        .find(|node| node.rect().left() > edge)
}

/// Opens the menu *Add a check of*.
fn open_menu(harness: &mut FolderHarness) {
    add_button(harness)
        .expect("the Checks tab has Add check")
        .click();
    settle(harness);
    assert!(shows(harness, MENU), "Add check opens its menu");
}

/// The row of the menu that names `field` (a path, or an output's name).
fn row<'a>(harness: &'a FolderHarness, field: &'a str) -> Option<egui_kittest::Node<'a>> {
    let menu = harness.query_all_by_label(MENU).next()?.rect();
    harness
        .query_all_by_label(field)
        .find(|node| menu.contains_rect(node.rect()) || node.rect().left() >= menu.left())
}

/// The row of `field`, which must be there.
fn get_row<'a>(harness: &'a FolderHarness, field: &'a str) -> egui_kittest::Node<'a> {
    row(harness, field).unwrap_or_else(|| panic!("the menu lists {field}"))
}

/// Opens or folds `field` in the menu by its fold mark.
fn fold(harness: &mut FolderHarness, field: &str, mark: &str) {
    let label = format!("{mark} {field}");
    harness
        .query_all_by_label(&label)
        .next()
        .unwrap_or_else(|| panic!("the row {field} has {label}"))
        .click();
    settle(harness);
}

/// Chooses `item` in DIAG-08's open menu.
fn pick(harness: &mut FolderHarness, item: &str) {
    harness
        .query_all_by_label(item)
        .next()
        .unwrap_or_else(|| panic!("the menu offers {item}"))
        .click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
}

/// The project saved; the saved scenario.
fn saved(harness: &mut FolderHarness, root: &std::path::Path) -> String {
    choose(harness, "Save");
    settle(harness);
    fs::read_to_string(root.join(SCENARIO)).unwrap()
}

/// Criterion 1: *Add check* is the first line of the tab, with no check and
/// with checks.
#[test]
fn diag15_criterion_1_add_check_is_the_first_line() {
    let (_project, mut harness, id) = opened(WIDE, SOURCE);
    checks_tab(&mut harness, &id, "cart");
    let button = add_button(&harness)
        .expect("the Checks tab of a step with no check has Add check")
        .rect();
    let empty = harness.get_by_label("No check reads this step yet.").rect();
    assert!(
        button.bottom() <= empty.top(),
        "Add check stands above the sentence: {button:?} {empty:?}"
    );
    drop(harness);

    let (_project, mut harness, id) = opened(WIDE, &with(CHECKS));
    checks_tab(&mut harness, &id, "cart");
    let button = add_button(&harness)
        .expect("the Checks tab of a step with checks has Add check")
        .rect();
    for line in ["2 checks", "Check status_created", "Check total_value"] {
        let rect = harness.get_by_label(line).rect();
        assert!(
            button.bottom() <= rect.top(),
            "Add check stands above {line}: {button:?} {rect:?}"
        );
    }
}

/// Whether the Inspector of `node`, on its own and on its *Checks* tab,
/// shows *Add check*.
fn inspector_offers_add_check(node: Node) -> bool {
    let project = apiforge_core::demos::empty_project();
    let graph = Graph {
        nodes: vec![node.clone()],
        edges: Vec::new(),
        proxies: Vec::new(),
    };
    let mut inspector = crate::inspector::Inspector::default();
    inspector.open_checks();
    let mut themed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(WIDE)
        .build_ui(move |ui| {
            if !themed {
                crate::theme::apply(ui.ctx());
                themed = true;
                return;
            }
            let context = crate::inspector::Context {
                document: egui::Id::new("diag15"),
                node: &node,
                graph: &graph,
                board: &graph,
                cleanup: None,
                project: &project,
                parameters: &[],
                run: None,
                step: None,
                expand_here: None,
                read_only: None,
            };
            inspector.show(ui, &context);
        });
    harness.run();
    assert!(
        harness
            .query_all_by_label("No check reads this step yet.")
            .next()
            .is_some(),
        "the Checks tab is shown"
    );
    harness.query_all_by_label(ADD).next().is_some()
}

/// Criterion 1: a step with no output has no *Add check*: a call of a
/// component that exports nothing; a request has it.
#[test]
fn diag15_criterion_1_no_output_no_button() {
    let step = |kind: NodeKind| {
        let mut node = apiforge_core::graph::create_node(
            kind,
            apiforge_core::format::Position { x: 0.0, y: 0.0 },
            None,
        );
        node.id = "step".into();
        node.handle = "step".into();
        node
    };
    let call = step(NodeKind::Component);
    assert!(
        apiforge_core::graph::output_ports(&call, &apiforge_core::demos::empty_project())
            .is_empty(),
        "the call has no output"
    );
    assert!(
        !inspector_offers_add_check(call),
        "a step with no output has no Add check"
    );
    assert!(
        inspector_offers_add_check(step(NodeKind::Request)),
        "a request has Add check"
    );
}

/// Criterion 2: *Add check* → `total` → *total is a number* writes the
/// line DIAG-08's ✓ writes from that row, as one undo step, and selects
/// the check; before any run, from the sample (DIAG-09).
#[test]
fn diag15_criterion_2_the_check_of_dia08s_menu() {
    // What DIAG-08's ✓ in Output data writes.
    let (project, mut harness, id) = opened(WIDE, SOURCE);
    output_data(&mut harness, &id, "cart");
    let edge = edge(&harness);
    let label = harness
        .query_all_by_label("total")
        .find(|node| node.rect().left() > edge)
        .expect("Output data shows total")
        .rect();
    harness
        .query_all_by_label("Add a check")
        .filter(|node| node.rect().left() > edge)
        .min_by(|a, b| {
            let y = label.center().y;
            (a.rect().center().y - y)
                .abs()
                .total_cmp(&(b.rect().center().y - y).abs())
        })
        .expect("the row total has ✓")
        .click();
    settle(&mut harness);
    pick(&mut harness, "total is a number");
    let expected = saved(&mut harness, project.path());
    drop(harness);

    // The same from the Checks tab.
    let (project, mut harness, id) = opened(WIDE, SOURCE);
    checks_tab(&mut harness, &id, "cart");
    let before = graph(&harness, &id);
    let text = source_of(&harness, &id);
    open_menu(&mut harness);
    get_row(&harness, "total").click();
    settle(&mut harness);
    assert!(
        shows(&harness, FIELD_MENU),
        "total opens DIAG-08's menu Add a check"
    );
    for item in [
        "total equals 172.5",
        "total is a number",
        "total exists",
        "total is not empty",
    ] {
        assert!(shows(&harness, item), "the menu offers {item}");
    }
    pick(&mut harness, "total is a number");
    let after = graph(&harness, &id);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "one step added: {new:?}");
    let check = new[0].clone();
    assert_eq!(check.kind, NodeKind::Assert);
    assert_eq!(apiforge_core::graph::predicate(&check), Predicate::Type);
    let cart = node(&after, "cart").id.clone();
    assert!(
        matches!(
            check.inputs.get("actual"),
            Some(Binding::Reference { node_id, port, path })
                if *node_id == cart && port == "body" && path == "total"
        ),
        "the check reads cart.body.total: {:?}",
        check.inputs
    );
    assert_eq!(
        selected(&mut harness, &id),
        vec![check.id.clone()],
        "the check is selected"
    );
    assert!(!shows(&harness, MENU), "the menu has closed");
    assert!(
        folder(&harness).fresh_run(&id).is_none(),
        "written from the sample, never run"
    );
    assert_eq!(
        saved(&mut harness, project.path()),
        expected,
        "the same line in the file as DIAG-08's ✓ writes"
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut harness);
    assert_eq!(source_of(&harness, &id), text, "one Undo takes it back");
}

/// Criterion 2: where the step takes no edit, *Add check* is disabled and
/// its hover says why.
#[test]
fn diag15_criterion_2_disabled_where_no_edit() {
    let (_project, mut harness, id) = opened(WIDE, SOURCE);
    checks_tab(&mut harness, &id, "cart");
    let broken = format!("{}\n}}\n", source_of(&harness, &id));
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &broken, CODE)
        .unwrap();
    settle(&mut harness);
    assert!(
        !folder(&harness).snapshot(&id).unwrap().editable,
        "the board takes no edit"
    );
    assert!(
        add_button(&harness)
            .expect("Add check is still shown")
            .accesskit_node()
            .is_disabled(),
        "Add check is disabled"
    );
    let said = |harness: &FolderHarness| harness.query_all_by_label_contains(LAST_VALID).count();
    let elsewhere = said(&harness);
    let shown = crate::testing::hover_until(
        &mut harness,
        |harness| add_button(harness).map(|node| node.rect()),
        |harness| said(harness) > elsewhere,
    );
    assert!(shown, "pointing at Add check says why");
    harness.remove_cursor();
    settle(&mut harness);
    let before = graph(&harness, &id);
    add_button(&harness).unwrap().click();
    settle(&mut harness);
    assert!(!shows(&harness, MENU), "no menu opens");
    assert_eq!(graph(&harness, &id), before, "nothing changes");
}

/// Criterion 3: the menu lists the step's outputs in the order of Output
/// data, a row 26 px high; an object or a list is folded and opens in
/// place.
#[test]
fn diag15_criterion_3_rows_as_output_data() {
    let (_project, mut harness, id) = opened(WIDE, ORDER);
    checks_tab(&mut harness, &id, "order");
    open_menu(&mut harness);
    let order = [
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
    let mut last = f32::NEG_INFINITY;
    for field in order {
        let rect = get_row(&harness, field).rect();
        assert!(
            rect.height() <= 26.0 + 0.5,
            "the row {field} is at most 26 px high: {rect:?}"
        );
        assert!(
            rect.top() >= last,
            "{field} follows the row above it, as in Output data"
        );
        last = rect.bottom();
    }
    for (field, value) in [
        ("Response", "object · 7"),
        ("id", "\"o-1042\""),
        ("total", "172.5"),
        ("customer", "object · 2"),
        ("items", "array · 2"),
        ("HTTP status", "200"),
    ] {
        assert_eq!(
            get_row(&harness, field).value().as_deref(),
            Some(value),
            "the row {field} writes {value}, as Output data does"
        );
    }
    // Folded: an object and a list.
    for nested in ["customer.name", "items[0]"] {
        assert!(row(&harness, nested).is_none(), "{nested} is folded");
    }
    let items = get_row(&harness, "items").rect();
    fold(&mut harness, "customer", "Expand");
    assert!(shows(&harness, MENU), "opening a field keeps the menu open");
    let name = get_row(&harness, "customer.name").rect();
    let customer = get_row(&harness, "customer").rect();
    assert!(
        name.top() >= customer.bottom() - 0.5 && name.left() > customer.left(),
        "customer.name opens in place under customer: {name:?} {customer:?}"
    );
    assert!(
        get_row(&harness, "items").rect().top() > items.top(),
        "the rows below move down"
    );
    fold(&mut harness, "customer", "Collapse");
    assert!(row(&harness, "customer.name").is_none(), "folded again");
}

/// Criterion 4: at 960 × 540 the menu of `order`, every field opened,
/// stays inside the window and scrolls.
#[test]
fn diag15_criterion_4_the_menu_scrolls_inside_the_window() {
    let (_project, mut harness, id) = opened(MINIMUM, ORDER);
    checks_tab(&mut harness, &id, "order");
    open_menu(&mut harness);
    fold(&mut harness, "customer", "Expand");
    fold(&mut harness, "items", "Expand");
    fold(&mut harness, "items[0]", "Expand");
    fold(&mut harness, "items[1]", "Expand");
    assert_within_window(&harness, &[MENU]);
    let menu = harness.get_by_label(MENU).rect();
    let status = get_row(&harness, "HTTP status").rect();
    assert!(
        !menu.contains_rect(status),
        "twenty rows do not fit the menu at 960 × 540: {status:?} in {menu:?}"
    );
    get_row(&harness, "HTTP status").scroll_to_me();
    settle(&mut harness);
    assert_within_window(&harness, &[MENU]);
    let menu = harness.get_by_label(MENU).rect();
    let status = get_row(&harness, "HTTP status").rect();
    assert!(
        menu.contains_rect(status),
        "the list scrolls to its last row: {status:?} in {menu:?}"
    );
    get_row(&harness, "HTTP status").click();
    settle(&mut harness);
    assert!(
        shows(&harness, "status equals 200"),
        "the last row opens its checks"
    );
    assert_within_window(&harness, &[MENU, FIELD_MENU]);
}

/// Criterion 5: with no sample and no run the menu says how to list the
/// fields and offers the status.
#[test]
fn diag15_criterion_5_no_sample() {
    let (_project, mut harness, id) = opened(WIDE, NO_SAMPLE);
    checks_tab(&mut harness, &id, "cart");
    open_menu(&mut harness);
    assert!(shows(&harness, NO_FIELDS), "the menu says {NO_FIELDS}");
    assert!(row(&harness, "Response").is_none(), "no response to list");
    get_row(&harness, "HTTP status").click();
    settle(&mut harness);
    assert!(
        shows(&harness, "status equals 200"),
        "the status is offered"
    );
}

/// Criterion 6: a click on a check's card selects the check; Return on a
/// focused card does the same; the cards' accessible names stay distinct.
#[test]
fn diag15_criterion_6_a_card_selects_its_check() {
    let (_project, mut harness, id) = opened(WIDE, &with(CHECKS));
    let graph = graph(&harness, &id);
    let total = node(&graph, "total_value").id.clone();
    let status = node(&graph, "status_created").id.clone();

    checks_tab(&mut harness, &id, "cart");
    for name in ["Check status_created", "Check total_value"] {
        assert_eq!(
            harness.query_all_by_label(name).count(),
            1,
            "one widget is named {name}"
        );
    }
    harness.get_by_label("Check total_value").click();
    settle(&mut harness);
    assert_eq!(selected(&mut harness, &id), vec![total], "a click selects");

    checks_tab(&mut harness, &id, "cart");
    harness.get_by_label("Check status_created").focus();
    settle(&mut harness);
    assert!(
        harness.get_by_label("Check status_created").is_focused(),
        "the card takes keyboard focus"
    );
    harness.key_press(egui::Key::Enter);
    settle(&mut harness);
    assert_eq!(selected(&mut harness, &id), vec![status], "Return selects");
}

/// Criterion 7: LAY-08's criterion 5 holds under the new line: three checks
/// fit the Inspector at 960 × 540 (`lay08_criterion_5_fits_the_small_window`
/// passes unchanged), and *Add check* stands clear of them.
#[test]
fn diag15_criterion_7_three_checks_still_fit() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-30.afs");
    let (_project, mut harness, id) = opened(MINIMUM, &fs::read_to_string(path).unwrap());
    checks_tab(&mut harness, &id, "cart");
    let rows = [
        "Check cart_created",
        "Check cart_total",
        "Check receipt_total",
    ];
    assert_within_window(&harness, &rows);
    assert_disjoint(&harness, &rows);
    let bottom = edge_bottom(&harness);
    for row in rows {
        let rect = harness.get_by_label(row).rect();
        assert!(rect.bottom() <= bottom, "{row} is clipped: {rect:?}");
    }
    let button = add_button(&harness).expect("Add check").rect();
    for row in rows {
        let rect = harness.get_by_label(row).rect();
        assert!(!button.intersects(rect), "Add check overlaps {row}");
    }
}

/// Where the Inspector's list ends, inside the panel's margin.
fn edge_bottom(harness: &FolderHarness) -> f32 {
    harness.get_by_label("Resize inspector").rect().bottom() - 16.0
}

/// Criterion 8: Escape closes DIAG-08's menu, then the menu *Add a check
/// of*, and the step stays selected.
#[test]
fn diag15_criterion_8_escape_keeps_the_selection() {
    let (_project, mut harness, id) = opened(WIDE, SOURCE);
    checks_tab(&mut harness, &id, "cart");
    let cart = selected(&mut harness, &id);
    open_menu(&mut harness);
    get_row(&harness, "total").click();
    settle(&mut harness);
    assert!(shows(&harness, FIELD_MENU), "total's menu is open");
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    assert!(!shows(&harness, FIELD_MENU), "Escape closes total's menu");
    assert!(shows(&harness, MENU), "and leaves the list of fields");
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    assert!(!shows(&harness, MENU), "a second Escape closes the list");
    assert_eq!(selected(&mut harness, &id), cart, "cart stays selected");
    assert!(
        add_button(&harness).is_some(),
        "the Inspector still shows cart's Checks tab"
    );
}

/// Criterion 8: design review at 1600 × 1000 and 960 × 540, against the
/// board `check-making.png`: the tab empty and with checks, the menu open
/// on `total` with DIAG-08's menu beside it.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag15_add_check() {
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        for (source, state) in [(SOURCE.to_owned(), "empty"), (with(CHECKS), "checks")] {
            let (_project, mut harness, id) = opened(size, &source);
            checks_tab(&mut harness, &id, "cart");
            crate::testing::capture(&mut harness, &format!("diag15-{state}-{suffix}"), None);
            open_menu(&mut harness);
            get_row(&harness, "total").click();
            settle(&mut harness);
            crate::testing::capture(&mut harness, &format!("diag15-{state}-menu-{suffix}"), None);
        }
        let (_project, mut harness, id) = opened(size, ORDER);
        checks_tab(&mut harness, &id, "order");
        open_menu(&mut harness);
        fold(&mut harness, "customer", "Expand");
        fold(&mut harness, "items", "Expand");
        fold(&mut harness, "items[0]", "Expand");
        crate::testing::capture(&mut harness, &format!("diag15-order-menu-{suffix}"), None);
    }
}
