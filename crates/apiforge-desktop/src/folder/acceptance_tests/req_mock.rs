//! Mock-ups of DIAG-12, DIAG-15 and DIAG-16 (REQ): design review captures
//! for the boards. Mock branch only.
use super::*;
use crate::mock;
use crate::testing::MINIMUM;
use apiforge_core::format::Graph;
use egui_kittest::kittest::NodeT;
use std::sync::atomic::Ordering::Relaxed;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const SAMPLE: &str = r#"    sample 201 { id: "c-1", total: 172.5, paid: false, owner: { name: "Ann", email: "ann@example.com" }, items: [{ sku: "coffee", price: 12.5 }], coupon: null }
"#;

fn source(sample: bool, checks: &str) -> String {
    format!(
        "scenario \"A cart\" {{\n  begin = start\n  cart = POST \"/carts\" {{\n{}  }}\n{checks}}}\n",
        if sample { SAMPLE } else { "" }
    )
}

const CHECKS: &str = r#"  status_created = assert cart.status == 201
  total_number = assert cart.body.total is "number"
  total_value = assert cart.body.total == 172.5
  has_items = assert nonempty cart.body.items
  owner_name = assert cart.body.owner.name == "Ann"
  paid_true = assert cart.body.paid == true
"#;
const TWO: &str = r#"  status_created = assert cart.status == 201
  total_value = assert cart.body.total == 172.5
"#;

fn opened(size: egui::Vec2, text: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), text).unwrap();
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

fn select(harness: &mut FolderHarness, id: &str, handle: &str) {
    let step = graph(harness, id)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
}

fn tab(harness: &mut FolderHarness, name: &str) {
    super::inspector_tab(harness, name)
        .unwrap_or_else(|| panic!("the Inspector has a {name} tab"))
        .click();
    settle(harness);
}

fn edge(harness: &FolderHarness) -> f32 {
    harness
        .query_by_label("Resize inspector")
        .map_or(0.0, |node| node.rect().left())
}

/// The control labelled `action` on the row of `field`, right of `left`.
fn on_row<'a>(
    harness: &'a FolderHarness,
    field: &str,
    action: &'a str,
    left: f32,
) -> Option<egui_kittest::Node<'a>> {
    let y = harness
        .query_all_by_label(field)
        .filter(|node| node.rect().left() > left)
        .map(|node| node.rect().center().y)
        .last()?;
    harness
        .query_all_by_label(action)
        .filter(|node| node.rect().left() > left && (node.rect().center().y - y).abs() < 10.0)
        .next()
}

fn shot(harness: &mut FolderHarness, name: &str) {
    harness.remove_cursor();
    harness.run_steps(3);
    let at = edge(harness);
    crate::testing::capture(harness, name, None);
    let path = crate::testing::capture_dir().join("edges.txt");
    let mut all = fs::read_to_string(&path).unwrap_or_default();
    all.push_str(&format!("{name} {at}\n"));
    fs::write(path, all).unwrap();
}

#[test]
#[ignore = "REQ mock-ups: design review capture; run with tools/gui/review.sh review_req"]
fn review_req_check_making() {
    let which = std::env::var("REQ_MOCK").unwrap_or_default();
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        // DIAG-15: the Checks tab.
        if which.is_empty() || which.contains("15") {
            for (mode, name) in [(0, "today"), (1, "a"), (2, "b")] {
                mock::ADD_CHECK.store(mode, Relaxed);
                for (checks, state) in [("", "empty"), (TWO, "checks")] {
                    let (_project, mut harness, id) = opened(size, &source(true, checks));
                    select(&mut harness, &id, "cart");
                    tab(&mut harness, "Checks");
                    shot(&mut harness, &format!("diag15-{name}-{state}-{suffix}"));
                    if mode == 1 {
                        harness.get_by_label("Add check").click();
                        settle(&mut harness);
                        let row = harness
                            .query_all_by_label_contains("total")
                            .find(|node| {
                                node.accesskit_node()
                                    .label()
                                    .is_some_and(|label| label.contains("172.5"))
                                    && !node
                                        .accesskit_node()
                                        .label()
                                        .is_some_and(|label| label.contains("=="))
                            })
                            .expect("the menu lists total");
                        let at = row.rect().center();
                        harness.hover_at(at);
                        harness.run_steps(30);
                        let name = format!("diag15-{name}-{state}-menu-{suffix}");
                        let edge = edge(&harness);
                        crate::testing::capture(&mut harness, &name, None);
                        let path = crate::testing::capture_dir().join("edges.txt");
                        let mut all = fs::read_to_string(&path).unwrap_or_default();
                        all.push_str(&format!("{name} {edge}\n"));
                        fs::write(path, all).unwrap();
                    }
                    if mode == 2 {
                        let left = edge(&harness);
                        if let Some(check) = on_row(&harness, "total", "Add a check", left) {
                            check.click();
                            settle(&mut harness);
                            shot(&mut harness, &format!("diag15-{name}-{state}-menu-{suffix}"));
                        }
                    }
                }
            }
            mock::ADD_CHECK.store(0, Relaxed);
        }

        // DIAG-16: the dialog, on a request that has no sample.
        if which.is_empty() || which.contains("16") {
            mock::PREFILL.store(true, Relaxed);
            for (mode, name) in [(0, "today"), (1, "a"), (2, "b"), (3, "c")] {
                mock::TYPED_PATH.store(mode, Relaxed);
                let text = source(false, "  cart_id = assert cart.body.id == \"c-1\"\n");
                let (_project, mut harness, id) = opened(size, &text);
                select(&mut harness, &id, "cart_id");
                let left = edge(&harness);
                let field = harness
                    .query_all_by_label_contains("▾")
                    .find(|node| node.rect().left() > left)
                    .expect("the check has a Response field button");
                field.scroll_to_me();
                settle(&mut harness);
                let field = harness
                    .query_all_by_label_contains("▾")
                    .find(|node| node.rect().left() > left)
                    .unwrap();
                field.click();
                settle(&mut harness);
                shot(&mut harness, &format!("diag16-{name}-{suffix}"));
            }
            mock::TYPED_PATH.store(0, Relaxed);
            mock::PREFILL.store(false, Relaxed);
        }

        // DIAG-12: the mark, in the Inspector and in Run results.
        if which.is_empty() || which.contains("12") {
            for (mode, name) in [(0, "today"), (1, "a"), (2, "b"), (3, "c")] {
                mock::MARK.store(mode, Relaxed);
                let (_project, mut harness, id) = opened(size, &source(true, CHECKS));
                select(&mut harness, &id, "cart");
                tab(&mut harness, "Output data");
                shot(&mut harness, &format!("diag12-{name}-inspector-{suffix}"));
                let left = edge(&harness);
                if mode == 1
                    && let Some(check) = on_row(&harness, "total", "Add a check", left)
                {
                    check.click();
                    settle(&mut harness);
                    shot(&mut harness, &format!("diag12-{name}-inspector-menu-{suffix}"));
                    harness.key_press(egui::Key::Escape);
                    settle(&mut harness);
                }
                choose(&mut harness, "Run simulation");
                let _ = finish_run(&mut harness);
                choose(&mut harness, "Maximise run results");
                let label = harness
                    .query_all_by_label_contains("cart · ")
                    .find(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
                    .and_then(|node| node.accesskit_node().label())
                    .expect("Run results list cart");
                harness.get_by_label(&label).click();
                settle(&mut harness);
                shot(&mut harness, &format!("diag12-{name}-results-{suffix}"));
                if mode == 1 {
                    let check = harness
                        .query_all_by_label("total")
                        .filter_map(|label| {
                            let y = label.rect().center().y;
                            harness
                                .query_all_by_label("Check")
                                .find(|check| (check.rect().center().y - y).abs() < 8.0)
                        })
                        .next();
                    if let Some(check) = check {
                        check.click();
                        settle(&mut harness);
                        shot(&mut harness, &format!("diag12-{name}-results-menu-{suffix}"));
                    }
                }
            }
            mock::MARK.store(0, Relaxed);
        }
    }
}
