//! DIAG-19 (#568): the menu *Add a check* (DIAG-08) writes the checks it
//! offers in the signs of the code, as DIAG-02 writes a check everywhere:
//! `total == 172.5`, `total is "number"`, `exists total`, `nonempty total`.
//! What is chosen is what the check's card then reads. The subject is
//! shortened as DIAG-02 shortens it under its step: no step name, no
//! leading `body.` unless the field is named like one of the step's
//! outputs (`body.status`). The same in *Output data*, Run results and the
//! *Checks* tab's *Add check*.
use super::*;
use apiforge_core::format::{Graph, Node};
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// DIAG-08's request, with a field `status` in its body and one nested.
const SOURCE: &str = r#"scenario "A cart" {
  begin = start
  cart = POST "/carts" {
    sample 201 { id: "c-1", total: 172.5, status: "open", owner: { name: "Ann" }, items: ["coffee"] }
  }
}
"#;

/// The ✓ on a row of *Output data*.
const CHECK: &str = "Add a check";
/// DIAG-08's menu.
const MENU: &str = "Add a check menu";
/// The four checks the menu offers on `total`, as the code writes them.
const OFFERED: [&str; 4] = [
    "total == 172.5",
    "total is \"number\"",
    "exists total",
    "nonempty total",
];
/// The same in DIAG-08's words, which the menu no longer writes.
const OLD: [&str; 4] = [
    "total equals 172.5",
    "total is a number",
    "total exists",
    "total is not empty",
];

/// `SOURCE` as the demo shop's scenario, on its board at `size`.
fn opened(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), SOURCE).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    (project, harness, id)
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

/// The Inspector's left edge.
fn edge(harness: &FolderHarness) -> f32 {
    harness.get_by_label("Resize inspector").rect().left()
}

/// `cart` selected, the Inspector on `tab`.
fn inspector_on(harness: &mut FolderHarness, id: &str, tab: &str) {
    let step = node(&graph(harness, id), "cart").id.clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
    super::inspector_tab(harness, tab)
        .unwrap_or_else(|| panic!("the Inspector has a {tab} tab"))
        .click();
    settle(harness);
}

/// Opens the menu of the ✓ on the row named `field` of *Output data*.
fn open_menu(harness: &mut FolderHarness, field: &str) {
    let edge = edge(harness);
    let row = harness
        .query_all_by_label(field)
        .map(|node| node.rect())
        .find(|rect| rect.left() > edge)
        .unwrap_or_else(|| panic!("Output data shows a row named {field}"));
    harness
        .query_all_by_label(CHECK)
        .filter(|button| button.rect().left() > edge)
        .min_by(|a, b| {
            let a = (a.rect().center().y - row.center().y).abs();
            let b = (b.rect().center().y - row.center().y).abs();
            a.total_cmp(&b)
        })
        .unwrap_or_else(|| panic!("the row {field} has {CHECK}"))
        .click();
    settle(harness);
    assert!(has_label(harness, MENU), "the ✓ on {field} opens {MENU}");
}

/// Opens the folded row named `field` of *Output data* by its fold mark.
fn expand(harness: &mut FolderHarness, field: &str) {
    let edge = edge(harness);
    let row = harness
        .query_all_by_label(field)
        .map(|node| node.rect())
        .find(|rect| rect.left() > edge)
        .unwrap_or_else(|| panic!("Output data shows a row named {field}"));
    harness
        .query_all_by_label("Expand field")
        .find(|mark| (mark.rect().center().y - row.center().y).abs() < 13.0)
        .unwrap_or_else(|| panic!("the row {field} is folded"))
        .click();
    harness.remove_cursor();
    settle(harness);
}

/// The items the open menu offers under *Add a check*.
fn offered(harness: &FolderHarness) -> Vec<String> {
    let menu = harness.get_by_label(MENU).rect();
    harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter(|item| menu.contains_rect(item.rect().shrink(1.0)))
        .filter_map(|item| item.accesskit_node().label())
        .collect()
}

/// Chooses `item` in the open menu.
fn pick(harness: &mut FolderHarness, item: &str) {
    harness
        .query_all_by_label(item)
        .next()
        .unwrap_or_else(|| panic!("the menu offers {item}"))
        .click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
}

/// The menu offers `expected`, each once, and none of DIAG-08's words.
fn assert_offers(harness: &FolderHarness, expected: &[&str], place: &str) {
    let items = offered(harness);
    for item in expected {
        assert_eq!(
            items.iter().filter(|found| found == item).count(),
            1,
            "{place}: the menu offers {item} once: {items:?}"
        );
    }
    for old in OLD {
        assert!(
            !items.iter().any(|found| found == old),
            "{place}: the menu no longer writes {old}: {items:?}"
        );
    }
}

/// Criterion 1: on `total` of *Output data* the menu offers the four
/// checks in the signs of the code, and none in DIAG-08's words.
#[test]
fn diag19_criterion_1_output_data_offers_code_signs() {
    let (_project, mut harness, id) = opened(WIDE);
    inspector_on(&mut harness, &id, "Output data");
    open_menu(&mut harness, "total");
    assert_offers(&harness, &OFFERED, "Output data");
}

/// Criterion 2: what is chosen is what the check's card reads: each of the
/// four, chosen, adds one check whose card on the board reads the item.
#[test]
fn diag19_criterion_2_the_card_reads_what_was_chosen() {
    let (_project, mut harness, id) = opened(WIDE);
    let mut written = Vec::new();
    for item in OFFERED {
        inspector_on(&mut harness, &id, "Output data");
        let before = graph(&harness, &id);
        open_menu(&mut harness, "total");
        pick(&mut harness, item);
        let after = graph(&harness, &id);
        let new = added(&before, &after);
        assert_eq!(new.len(), 1, "{item} adds one check: {new:?}");
        written.push(new[0].clone());
    }
    // Every check card shown, as the bar's *Checks* shows them (LAY-30).
    harness.get_by_label("Checks").click();
    settle(&mut harness);
    harness.step();
    let mut cards: Vec<String> = written
        .iter()
        .flat_map(|check| {
            let name = format!("{} (Comparison)", check.display_name());
            harness
                .query_all_by_label(&name)
                .filter_map(|card| card.accesskit_node().value().map(|value| value.to_string()))
                .collect::<Vec<_>>()
        })
        .collect();
    cards.sort();
    cards.dedup();
    let mut chosen: Vec<String> = OFFERED.iter().map(|item| (*item).to_owned()).collect();
    chosen.sort();
    assert_eq!(cards, chosen, "the cards read what was chosen");
}

/// Criterion 3: the same four in Run results' menu on `total`.
#[test]
fn diag19_criterion_3_run_results() {
    let (_project, mut harness, _id) = opened(WIDE);
    choose(&mut harness, "Run simulation");
    let run = finish_run(&mut harness);
    assert_eq!(run.status, RunStatus::Passed, "{:?}", run.error);
    choose(&mut harness, "Maximise run results");
    let row = "cart · ";
    let label = harness
        .query_all_by_label_contains(row)
        .filter_map(|found| found.accesskit_node().label())
        .find(|label| label.starts_with(row))
        .expect("cart has a row in the step list");
    harness
        .query_all_by_label(&label)
        .next()
        .unwrap()
        .scroll_to_me();
    settle(&mut harness);
    harness.query_all_by_label(&label).next().unwrap().click();
    settle(&mut harness);
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
    assert!(has_label(&harness, MENU), "Check opens {MENU}");
    assert_offers(&harness, &OFFERED, "Run results");
}

/// Criterion 4: the same four from the *Checks* tab's *Add check* on
/// `total` (DIAG-15).
#[test]
fn diag19_criterion_4_checks_tab() {
    let (_project, mut harness, id) = opened(WIDE);
    let step = node(&graph(&harness, &id), "cart").id.clone();
    app(&mut harness).selections.insert(id.clone(), vec![step]);
    settle(&mut harness);
    super::open_checks_tab(&mut harness);
    let edge = edge(&harness);
    harness
        .query_all_by_label("Add check")
        .find(|node| node.rect().left() > edge)
        .expect("the Checks tab has Add check")
        .click();
    settle(&mut harness);
    let list = harness.get_by_label("Add check menu").rect();
    harness
        .query_all_by_label("total")
        .find(|node| node.rect().left() >= list.left())
        .expect("the menu lists total")
        .click();
    settle(&mut harness);
    assert!(has_label(&harness, MENU), "total opens {MENU}");
    assert_offers(&harness, &OFFERED, "Checks tab");
}

/// Criterion 5: the subject is shortened as DIAG-02 shortens it under its
/// step: a whole output by its name, a nested field by its path from the
/// body, and a body field named like an output keeps `body.`.
#[test]
fn diag19_criterion_5_subjects_as_the_card_writes_them() {
    let (_project, mut harness, id) = opened(WIDE);
    for (row, subject, kind, value) in [
        ("Response", "body", "object", None),
        ("HTTP status", "status", "number", Some("201")),
        ("status", "body.status", "string", Some("\"open\"")),
        ("name", "owner.name", "string", Some("\"Ann\"")),
    ] {
        inspector_on(&mut harness, &id, "Output data");
        if row == "name" {
            expand(&mut harness, "owner");
        }
        open_menu(&mut harness, row);
        let items = offered(&harness);
        let typed = format!("{subject} is \"{kind}\"");
        let exists = format!("exists {subject}");
        let nonempty = format!("nonempty {subject}");
        for item in [&typed, &exists, &nonempty] {
            assert!(
                items.iter().any(|found| found == item),
                "the menu of {row} offers {item}: {items:?}"
            );
        }
        // A whole object is written as the card writes it, cut where long.
        let equals = format!("{subject} == {}", value.unwrap_or(""));
        assert!(
            items.iter().any(|found| match value {
                Some(_) => *found == equals,
                None => found.starts_with(&equals),
            }),
            "the menu of {row} offers {equals}: {items:?}"
        );
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
    }
}
