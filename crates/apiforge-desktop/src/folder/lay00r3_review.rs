//! LAY-00 round 3: design review views of shape D with every proposal on one
//! board (throwaway branch `mock/lay-00-round-3`). The switches in
//! `crate::mock_lay00` change the real board and Inspector for these views.
use super::*;
use crate::canvas::lay00r3;
use crate::mock_lay00::{self as mock, Anchor, Flags};
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, capture, folder, folder_harness_in, settle, shop_project,
};
use apiforge_core::{
    format::OptionalJson,
    graph::{WireKind, wires},
    model::{Branch, NodeKind},
    run::{Phase, StepResult, StepStatus},
};

pub(super) const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

pub(super) fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    std::fs::read_to_string(path).unwrap()
}

fn note(line: &str) {
    let Some(target) = std::env::var_os("CARGO_TARGET_DIR") else {
        return;
    };
    let path = std::path::PathBuf::from(target).join("lay00r3-metrics.txt");
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{line}").unwrap();
}

pub(super) fn app(harness: &mut FolderHarness) -> &mut FolderApp {
    harness.state_mut().as_mut().unwrap()
}

pub(super) fn document(harness: &FolderHarness) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == SCENARIO)
        .unwrap()
        .id()
        .to_owned()
}

pub(super) fn graph(harness: &FolderHarness) -> Graph {
    let id = document(harness);
    let model = folder(harness).snapshot(&id).unwrap().model.unwrap();
    graph_of(&model, false).clone()
}

pub(super) fn id_of(graph: &Graph, handle: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no {handle}"))
        .id
        .clone()
}

pub(super) fn select(harness: &mut FolderHarness, ids: Vec<String>) {
    let document = document(harness);
    app(harness).selections.insert(document, ids);
    settle(harness);
}

/// The scenario on the board of the demo shop at `size`.
pub(super) fn open(scenario: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, Graph) {
    mock::set(Flags::default());
    mock::set_steps(None);
    mock::clear_rects();
    mock::set_inspector(None);
    let project = shop_project();
    std::fs::write(project.path().join(SCENARIO), fixture(scenario)).unwrap();
    let mut harness = folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let graph = graph(&harness);
    (project, harness, graph)
}

/// A finished run, faked: every step passed except `failed` and `skipped`
/// checks; the steps of an unselected branch are skipped "outside path".
pub(super) fn run(graph: &Graph, failed: &[(&str, &str, &str)], skipped: &[&str]) -> Vec<StepResult> {
    let otherwise: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.branch == Some(Branch::False))
        .map(|edge| edge.target.as_str())
        .collect();
    graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let mut step = StepResult::new(node, Phase::Main);
            if node.kind == NodeKind::Request {
                step.duration_ms = 30 + (index as u64 * 7) % 90;
                if let Some(code) = node.config.sample_status {
                    step.outputs
                        .insert("status".to_owned(), serde_json::json!(code));
                }
            }
            if node.kind == NodeKind::If {
                step.outputs
                    .insert("value".to_owned(), serde_json::json!(true));
            }
            if otherwise.contains(&node.id.as_str()) {
                step.status = StepStatus::Skipped;
                step.message = Some("outside path".to_owned());
            }
            if let Some((_, actual, expected)) = failed.iter().find(|f| f.0 == node.handle) {
                step.status = StepStatus::Failed;
                step.actual = OptionalJson::Present(serde_json::from_str(actual).unwrap());
                step.expected = OptionalJson::Present(serde_json::from_str(expected).unwrap());
            }
            if skipped.contains(&node.handle.as_str()) {
                step.status = StepStatus::Skipped;
                step.message = Some("stopped".to_owned());
            }
            step
        })
        .collect()
}

pub(super) fn shot(harness: &mut FolderHarness, name: &str) {
    settle(harness);
    harness.step();
    capture(harness, name, None);
}

pub(super) fn flags(collapsed: bool) -> Flags {
    Flags {
        on: true,
        collapsed,
        columns: 5,
        bands: true,
        ..Flags::default()
    }
}

/// The board's size and fit zoom.
fn measure(harness: &mut FolderHarness, label: &str) {
    settle(harness);
    harness.step();
    let metrics = lay00r3::metrics();
    note(&format!(
        "{label}: board {:.0} x {:.0}, fit zoom {:.3}, hidden {} cards {} wires, clearance min {:.1} px, {} of {} segments under 12 px",
        metrics.width,
        metrics.height,
        metrics.zoom,
        metrics.hidden_cards,
        metrics.hidden_wires,
        metrics.clearance,
        metrics.tight,
        metrics.segments
    ));
}

pub(super) fn shop_30_run(graph: &Graph) -> (Vec<StepResult>, Vec<String>) {
    let steps = run(
        graph,
        &[("checkout_total", "180.0", "172.5")],
        &["checkout_status"],
    );
    let not_run = ["refund", "refund_ok", "refunded", "refund_amount"]
        .iter()
        .map(|handle| id_of(graph, handle))
        .collect();
    (steps, not_run)
}

/// Items 1–8 and 11 on shop-30.
#[test]
#[ignore = "design-review view"]
fn review_lay00r3_d_shop_30() {
    for (size, name) in [(WIDE, "1600x1000"), (MINIMUM, "960x540")] {
        for collapsed in [true, false] {
            let (_project, mut harness, graph) = open("shop-30", size);
            let (steps, not_run) = shop_30_run(&graph);
            mock::set_steps(Some(steps));
            mock::set(Flags {
                not_run,
                ..flags(collapsed)
            });
            let state = if collapsed { "collapsed" } else { "expanded" };
            measure(&mut harness, &format!("shop-30 {state} {name}"));
            if size == WIDE {
                let spine: Vec<String> = lay00r3::spine()
                    .iter()
                    .map(|(id, row, column)| {
                        let handle = &graph.nodes.iter().find(|n| &n.id == id).unwrap().handle;
                        format!("{handle}@{row}.{column}")
                    })
                    .collect();
                note(&format!("  spine: {}", spine.join(" ")));
                if !collapsed {
                    shot(&mut harness, "lay00r3-d-shop-30-expanded-fit");
                }
            }
            if size == WIDE && collapsed {
                shot(&mut harness, "lay00r3-d-shop-30-fit");
                for zoom in [35, 60] {
                    mock::update(|f| f.view = Some((zoom as f32 / 100.0, Anchor::Rows(0, 1))));
                    shot(&mut harness, &format!("lay00r3-d-shop-30-{zoom}"));
                }
                let mut total = 0.0;
                let mut empty = 0.0;
                let mut lines = Vec::new();
                for (id, (kind, height, gap)) in lay00r3::empty() {
                    if kind == NodeKind::Request {
                        total += height;
                        empty += gap;
                        let handle = &graph.nodes.iter().find(|n| n.id == id).unwrap().handle;
                        lines.push(format!("{handle} {height:.0}/{gap:.0}"));
                    }
                }
                lines.sort();
                note(&format!(
                    "compact requests: {:.0} px of {:.0} empty ({:.0} %): {}",
                    empty,
                    total,
                    100.0 * empty / total.max(1.0),
                    lines.join(", ")
                ));
                // Item 6: selected cart, payment running.
                let cart = id_of(&graph, "cart");
                let payment = id_of(&graph, "payment");
                mock::update(|f| {
                    f.running = Some(payment.clone());
                    f.view = Some((0.6, Anchor::Cards(vec![cart.clone(), payment.clone()])));
                });
                select(&mut harness, vec![cart.clone()]);
                shot(&mut harness, "lay00r3-d-shop-30-states-60");
                select(&mut harness, vec![]);
                mock::update(|f| f.running = None);
                // Item 7: the token wire into cart, traced.
                let login = id_of(&graph, "login");
                let trace = wires(&graph)
                    .into_iter()
                    .find(|w| w.kind == WireKind::Data && w.source == login && w.target == cart)
                    .expect("login feeds cart")
                    .id;
                mock::update(|f| {
                    f.trace = Some(trace);
                    f.view = Some((0.6, Anchor::Rows(0, 1)));
                });
                shot(&mut harness, "lay00r3-d-shop-30-trace-60");
                // The same with the router's clearance raised, for comparison.
                mock::update(|f| {
                    f.trace = None;
                    f.clearance = Some(18.0);
                });
                measure(&mut harness, "shop-30 collapsed clearance 18");
                shot(&mut harness, "lay00r3-d-shop-30-clearance18-60");
                mock::update(|f| f.clearance = None);
                // Item 11: the Inspector's checks of checkout.
                let checkout = id_of(&graph, "checkout");
                mock::update(|f| {
                    f.inspector = Some(checkout.clone());
                    f.view = Some((0.6, Anchor::Cards(vec![checkout.clone()])));
                });
                select(&mut harness, vec![checkout.clone()]);
                shot(&mut harness, "lay00r3-inspector-checks-window");
                let panel = mock::inspector_rect().expect("the Inspector was drawn");
                capture(&mut harness, "lay00r3-inspector-checks", Some(panel));
                // An aggregate crop: checkout's card at 60 %.
                if let Some(rect) = mock::rect_of(&checkout) {
                    capture(&mut harness, "lay00r3-aggregate-checkout", Some(rect.expand(24.0)));
                }
            }
            mock::set(Flags::default());
            mock::set_steps(None);
        }
    }
}

/// Item 9: a branch of four steps across two rows, on shop-50.
#[test]
#[ignore = "design-review view"]
fn review_lay00r3_d_branch_two_rows() {
    let (_project, mut harness, graph) = open("shop-50", WIDE);
    mock::set_steps(Some(run(&graph, &[], &[])));
    mock::set(flags(true));
    measure(&mut harness, "shop-50 collapsed 1600x1000");
    let spine: Vec<String> = lay00r3::spine()
        .iter()
        .map(|(id, row, column)| {
            let handle = &graph.nodes.iter().find(|n| &n.id == id).unwrap().handle;
            format!("{handle}@{row}.{column}")
        })
        .collect();
    note(&format!("  spine: {}", spine.join(" ")));
    shot(&mut harness, "lay00r3-d-branch-two-rows-fit");
    let ids: Vec<String> = ["paid", "payment", "notify", "cancel"]
        .iter()
        .filter(|h| graph.nodes.iter().any(|n| n.handle == **h))
        .map(|h| id_of(&graph, h))
        .collect();
    mock::update(|f| f.view = Some((0.6, Anchor::Cards(ids))));
    shot(&mut harness, "lay00r3-d-branch-two-rows-60");
    mock::set(Flags::default());
    mock::set_steps(None);
}

/// Item 10: shop-50 in the small window.
#[test]
#[ignore = "design-review view"]
fn review_lay00r3_d_shop_50_960() {
    for collapsed in [true, false] {
        let (_project, mut harness, graph) = open("shop-50", MINIMUM);
        mock::set_steps(Some(run(&graph, &[], &[])));
        mock::set(flags(collapsed));
        let state = if collapsed { "collapsed" } else { "expanded" };
        measure(&mut harness, &format!("shop-50 {state} 960x540"));
        if collapsed {
            shot(&mut harness, "lay00r3-d-shop-50-960-fit");
            mock::update(|f| f.view = Some((0.6, Anchor::Rows(0, 0))));
            shot(&mut harness, "lay00r3-d-shop-50-960-60");
        }
        mock::set(Flags::default());
        mock::set_steps(None);
    }
}

/// The small reference: order-flow at 60 %, collapsed, after a run.
#[test]
#[ignore = "design-review view"]
fn review_lay00r3_d_order_flow() {
    let (_project, mut harness, graph) = open("order-flow", WIDE);
    mock::set_steps(Some(run(&graph, &[("cart_total", "29.0", "28.0")], &[])));
    mock::set(flags(true));
    measure(&mut harness, "order-flow collapsed 1600x1000");
    mock::update(|f| f.view = Some((0.6, Anchor::Rows(0, 1))));
    shot(&mut harness, "lay00r3-d-order-flow-60");
    mock::set(Flags::default());
    mock::set_steps(None);
}

pub(super) fn view(harness: &mut FolderHarness, graph: &Graph, zoom: f32, handles: &[&str]) {
    let ids = handles.iter().map(|h| id_of(graph, h)).collect();
    mock::update(|f| f.view = Some((zoom, Anchor::Cards(ids))));
    settle(harness);
}

/// Item 12: when a hidden check runs, in the Inspector; a gate in the aggregate.
#[test]
#[ignore = "design-review view"]
fn review_lay00r3_inspector_timing() {
    let (_project, mut harness, graph) = open("shop-30", WIDE);
    let sequence = lay00r3::sequence(&graph);
    let checkout = id_of(&graph, "checkout");
    let at = sequence.iter().position(|id| id == &checkout).unwrap();
    let done: Vec<String> = sequence[..at].to_vec();
    let (steps, _) = shop_30_run(&graph);
    let steps: Vec<StepResult> = steps
        .into_iter()
        .filter(|s| done.contains(&s.node_id))
        .collect();
    mock::set_steps(Some(steps));
    mock::set(Flags {
        running: Some(checkout.clone()),
        inspector: Some(id_of(&graph, "cart")),
        hover_check: Some(id_of(&graph, "checkout_cart")),
        ..flags(true)
    });
    view(&mut harness, &graph, 0.6, &["cart", "checkout"]);
    shot(&mut harness, "lay00r3-inspector-timing-window");
    let panel = mock::inspector_rect().expect("the Inspector was drawn");
    capture(&mut harness, "lay00r3-inspector-timing", Some(panel));
    let cart_view = id_of(&graph, "cart_view");
    if let Some(rect) = mock::rect_of(&cart_view) {
        capture(&mut harness, "lay00r3-aggregate-gate", Some(rect.expand(24.0)));
    }
    mock::set(Flags::default());
    mock::set_steps(None);
}

/// Items 13 and 15: off-screen markers during focus, the jump back, and the
/// task walk "select checkout → token's source → its check → back".
#[test]
#[ignore = "design-review view"]
fn review_lay00r3_focus_offscreen() {
    let (_project, mut harness, graph) = open("shop-30", WIDE);
    let (steps, not_run) = shop_30_run(&graph);
    mock::set_steps(Some(steps));
    mock::set(Flags {
        not_run,
        markers: true,
        ..flags(true)
    });
    let checkout = id_of(&graph, "checkout");
    select(&mut harness, vec![checkout.clone()]);
    view(&mut harness, &graph, 0.6, &["checkout", "receipt"]);
    shot(&mut harness, "lay00r3-focus-offscreen-60");
    shot(&mut harness, "lay00r3-walk-1");
    // A click on `↑ login · token` jumps there; checkout stays selected.
    mock::update(|f| f.jumped_from = Some(checkout.clone()));
    view(&mut harness, &graph, 0.6, &["login"]);
    shot(&mut harness, "lay00r3-focus-offscreen-back");
    shot(&mut harness, "lay00r3-walk-2");
    // A click on login's aggregate lists its check.
    mock::update(|f| f.inspector = Some(id_of(&graph, "login")));
    shot(&mut harness, "lay00r3-walk-3");
    // `← Back to checkout`.
    mock::update(|f| {
        f.inspector = None;
        f.jumped_from = None;
    });
    view(&mut harness, &graph, 0.6, &["checkout", "receipt"]);
    shot(&mut harness, "lay00r3-walk-4");
    mock::set(Flags::default());
    mock::set_steps(None);
}

/// Item 14: the Authorization input of cart, hovered: its one wire traced.
#[test]
#[ignore = "design-review view"]
fn review_lay00r3_trace_input() {
    let (_project, mut harness, graph) = open("shop-30", WIDE);
    let (steps, not_run) = shop_30_run(&graph);
    mock::set_steps(Some(steps));
    let cart = id_of(&graph, "cart");
    let login = id_of(&graph, "login");
    let wire = wires(&graph)
        .into_iter()
        .find(|w| w.kind == WireKind::Data && w.source == login && w.target == cart)
        .expect("login feeds cart");
    mock::set(Flags {
        not_run,
        trace: Some(wire.id.clone()),
        trace_row: wire.target_input.clone().map(|input| (cart.clone(), input)),
        ..flags(true)
    });
    view(&mut harness, &graph, 0.6, &["login", "cart"]);
    shot(&mut harness, "lay00r3-trace-input-60");
    mock::set(Flags::default());
    mock::set_steps(None);
}
