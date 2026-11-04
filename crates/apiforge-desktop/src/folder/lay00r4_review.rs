//! LAY-00 round 4: four narrow proofs on shop-30 (throwaway branch
//! `mock/lay-00-round-4`) — one compact card across the zoom boundaries,
//! collapse and expand in one camera, a coherent partial run, and the walk
//! executed by clicks. Built on the round-3 views' helpers.
use super::lay00r3_review::{app, document, flags, id_of, open, run, shop_30_run};
use super::*;
use crate::canvas::lay00r3;
use crate::mock_lay00::{self as mock, Anchor, Flags};
use crate::testing::{FolderHarness, MINIMUM, WIDE, capture, folder, settle};
use apiforge_core::run::StepResult;
use std::collections::HashMap;
use apiforge_core::model::NodeKind;
use eframe::egui::Rect;
use egui_kittest::kittest::Queryable;

fn note(line: &str) {
    let Some(target) = std::env::var_os("CARGO_TARGET_DIR") else {
        return;
    };
    let path = std::path::PathBuf::from(target).join("lay00r4-metrics.txt");
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{line}").unwrap();
}

fn shot(harness: &mut FolderHarness, name: &str) {
    settle(harness);
    harness.step();
    capture(harness, name, None);
}

fn handle(graph: &Graph, id: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| id.to_owned(), |node| node.handle.clone())
}

/// Round 4's board: compact cards, collapsed checks, used gates visible.
fn compact(not_run: Vec<String>) -> Flags {
    Flags {
        not_run,
        dense: true,
        gates_visible: true,
        ..flags(true)
    }
}

/// Proof 1: one compact card (`cart`) at the zoom boundaries, the same card
/// today at 0.60, and shop-30 repacked with every request compact.
#[test]
#[ignore = "design-review view"]
fn review_lay00r4_card() {
    let (_project, mut harness, graph) = open("shop-30", WIDE);
    let (steps, not_run) = shop_30_run(&graph);
    mock::set_steps(Some(steps));
    let cart = id_of(&graph, "cart");
    let login = id_of(&graph, "login");
    // Today's geometry, the same board otherwise.
    mock::set(Flags {
        view: Some((0.6, Anchor::Cards(vec![cart.clone()]))),
        ..compact(not_run.clone())
    });
    mock::update(|f| f.dense = false);
    settle(&mut harness);
    harness.step();
    let today = mock::world();
    let rect = mock::rect_of(&cart).unwrap();
    shot(&mut harness, "lay00r4-whole-today-60");
    capture(&mut harness, "lay00r4-card-60-today", Some(rect.expand(40.0)));
    // Compact.
    mock::update(|f| f.dense = true);
    let mut boxes = Vec::new();
    for zoom in [44, 46, 60, 74, 76, 90] {
        mock::update(|f| f.view = Some((zoom as f32 / 100.0, Anchor::Cards(vec![cart.clone()]))));
        settle(&mut harness);
        harness.step();
        let world = mock::world()[&cart];
        boxes.push(format!("{zoom}: {:.0}x{:.0}", world.width(), world.height()));
        let rect = mock::rect_of(&cart).unwrap();
        capture(&mut harness, &format!("lay00r4-card-{zoom}"), Some(rect.expand(40.0)));
    }
    let dense = mock::world();
    note(&format!("proof 1: cart world box per zoom: {}", boxes.join(", ")));
    for (name, id) in [("login", &login), ("cart", &cart)] {
        note(&format!(
            "proof 1: {name} height today {:.0} px, compact {:.0} px",
            today[id].height(),
            dense[id].height()
        ));
    }
    let mut requests: Vec<(String, f32, f32)> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Request && today.contains_key(&n.id))
        .map(|n| (n.handle.clone(), today[&n.id].height(), dense[&n.id].height()))
        .collect();
    requests.sort_by(|a, b| a.0.cmp(&b.0));
    note(&format!(
        "proof 1: requests today/compact: {}",
        requests
            .iter()
            .map(|(h, t, d)| format!("{h} {t:.0}/{d:.0}"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    // The whole board, repacked.
    mock::update(|f| f.view = None);
    settle(&mut harness);
    harness.step();
    let metrics = lay00r3::metrics();
    note(&format!(
        "proof 1: shop-30 compact collapsed board {:.0} x {:.0}, fit zoom {:.3} (round 3: 1900 x 2296, 0.33)",
        metrics.width, metrics.height, metrics.zoom
    ));
    let spine: Vec<String> = lay00r3::spine()
        .iter()
        .map(|(id, row, column)| format!("{}@{row}.{column}", handle(&graph, id)))
        .collect();
    note(&format!("  spine: {}", spine.join(" ")));
    shot(&mut harness, "lay00r4-d-shop-30-compact-fit");
    mock::update(|f| f.view = Some((0.6, Anchor::Rows(0, 1))));
    shot(&mut harness, "lay00r4-d-shop-30-compact-60");
    mock::set(Flags::default());
    mock::set_steps(None);
}

fn positions(graph: &Graph, world: &HashMap<String, Rect>) -> Vec<(String, Rect)> {
    let mut list: Vec<(String, Rect)> = world
        .iter()
        .map(|(id, rect)| (handle(graph, id), *rect))
        .collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    list
}

fn moves(graph: &Graph, before: &HashMap<String, Rect>, after: &HashMap<String, Rect>) -> String {
    let mut moved = Vec::new();
    let mut added = Vec::new();
    for (name, rect) in positions(graph, after) {
        let id = graph.nodes.iter().find(|n| n.handle == name).unwrap().id.clone();
        match before.get(&id) {
            Some(old) if (old.min - rect.min).length() > 0.5 => {
                let d = rect.min - old.min;
                moved.push(format!("{name} ({:+.0}, {:+.0})", d.x, d.y));
            }
            Some(_) => {}
            None => added.push(format!("{name} at ({:.0}, {:.0})", rect.min.x, rect.min.y)),
        }
    }
    format!(
        "moved [{}]; shown [{}]",
        moved.join(", "),
        added.join(", ")
    )
}

/// Proof 2: collapse and expand in one camera on rows 2–3 at 0.60, with the
/// used gate `valid_cart` and the shared check `checkout_cart`.
#[test]
#[ignore = "design-review view"]
fn review_lay00r4_collapse() {
    let (_project, mut harness, graph) = open("shop-30", WIDE);
    let (steps, not_run) = shop_30_run(&graph);
    mock::set_steps(Some(steps));
    let checkout = id_of(&graph, "checkout");
    mock::set(Flags {
        free_camera: true,
        view: Some((0.6, Anchor::Rows(1, 2))),
        ..compact(not_run)
    });
    settle(&mut harness);
    harness.step();
    let camera = mock::camera();
    let collapsed = mock::world();
    note(&format!(
        "proof 2: collapsed positions: {}",
        positions(&graph, &collapsed)
            .iter()
            .map(|(h, r)| format!("{h} ({:.0},{:.0} {:.0}h)", r.min.x, r.min.y, r.height()))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    shot(&mut harness, "lay00r4-collapse-collapsed");
    mock::update(|f| f.collapsed = false);
    shot(&mut harness, "lay00r4-collapse-expanded");
    let expanded = mock::world();
    note(&format!("proof 2: collapsed → expanded: {}", moves(&graph, &collapsed, &expanded)));
    assert_eq!(mock::camera(), camera, "the camera stays");
    mock::update(|f| {
        f.collapsed = true;
        f.expand_here = Some(checkout.clone());
    });
    shot(&mut harness, "lay00r4-collapse-expand-here");
    let here = mock::world();
    note(&format!("proof 2: collapsed → expand here: {}", moves(&graph, &collapsed, &here)));
    let shown: Vec<&String> = here.keys().filter(|id| !collapsed.contains_key(*id)).collect();
    let mut overlaps = Vec::new();
    for id in &shown {
        for (other, rect) in &here {
            if !shown.contains(&other) && rect.intersects(here[*id]) {
                let cut = rect.intersect(here[*id]);
                overlaps.push(format!(
                    "{} over {} ({:.0} x {:.0} px)",
                    handle(&graph, id),
                    handle(&graph, other),
                    cut.width(),
                    cut.height()
                ));
            }
        }
    }
    note(&format!(
        "proof 2: expand here overlaps: [{}]; stack reaches {:.0} px past its row",
        overlaps.join(", "),
        lay00r3::expand_delta()
    ));
    assert_eq!(mock::camera(), camera, "the camera stays");
    mock::update(|f| f.expand_shift = true);
    shot(&mut harness, "lay00r4-collapse-expand-shift");
    let shift = mock::world();
    note(&format!("proof 2: collapsed → expand here, rows shift: {}", moves(&graph, &collapsed, &shift)));
    assert_eq!(mock::camera(), camera, "the camera stays");
    mock::set(Flags::default());
    mock::set_steps(None);
}

/// Proof 3: a run in progress — finished up to the step before `running`,
/// one failed check on `checkout`, the gate passed; `cart` selected.
#[test]
#[ignore = "design-review view"]
fn review_lay00r4_run() {
    let (_project, mut harness, graph) = open("shop-30", WIDE);
    let sequence = lay00r3::sequence(&graph);
    let numbered: Vec<String> = sequence
        .iter()
        .enumerate()
        .map(|(i, id)| format!("{}={}", i + 1, handle(&graph, id)))
        .collect();
    note(&format!("proof 3: engine order: {}", numbered.join(" ")));
    let running = id_of(&graph, "order");
    let at = sequence.iter().position(|id| id == &running).unwrap();
    let all = run(&graph, &[("checkout_total", "180.0", "172.5")], &[]);
    let done: Vec<StepResult> = all
        .iter()
        .filter(|s| sequence[..at].contains(&s.node_id))
        .cloned()
        .collect();
    let pending: Vec<String> = sequence[at + 1..].to_vec();
    note(&format!(
        "proof 3: finished 1–{at}, running {} = {}, pending {}–{}",
        at + 1,
        handle(&graph, &running),
        at + 2,
        sequence.len()
    ));
    mock::set_steps(Some(done));
    let cart = id_of(&graph, "cart");
    let checkout = id_of(&graph, "checkout");
    let view = Some((0.6, Anchor::Cards(vec![cart.clone(), checkout.clone(), running.clone()])));
    mock::set(Flags {
        running: Some(running.clone()),
        view: view.clone(),
        ..compact(Vec::new())
    });
    super::lay00r3_review::select(&mut harness, vec![cart.clone()]);
    shot(&mut harness, "lay00r4-run-focus-60");
    super::lay00r3_review::select(&mut harness, vec![]);
    shot(&mut harness, "lay00r4-run-60");
    // The previous run's results on the steps not reached yet, stale.
    let mut previous = all.clone();
    previous.retain(|s| sequence[..at].contains(&s.node_id) || pending.contains(&s.node_id));
    mock::set_steps(Some(previous));
    mock::update(|f| f.stale = pending.clone());
    shot(&mut harness, "lay00r4-run-stale-60");
    mock::set(Flags::default());
    mock::set_steps(None);
}

fn click_at(harness: &mut FolderHarness, at: egui::Pos2) {
    harness.hover_at(at);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    settle(harness);
    harness.step();
}

fn selection(harness: &mut FolderHarness) -> Vec<String> {
    let document = document(harness);
    app(harness)
        .selections
        .get(&document)
        .cloned()
        .unwrap_or_default()
}

/// Proof 4: the walk, executed by clicks.
fn walk(size: egui::Vec2, prefix: &str) {
    let (_project, mut harness, graph) = open("shop-30", size);
    let (steps, not_run) = shop_30_run(&graph);
    mock::set_steps(Some(steps));
    let checkout = id_of(&graph, "checkout");
    let login = id_of(&graph, "login");
    mock::update_walk(|walk| *walk = mock::Walk::default());
    mock::set(Flags {
        free_camera: true,
        markers: true,
        inspector_follows: true,
        view: Some((0.6, Anchor::Top(checkout.clone()))),
        ..compact(not_run)
    });
    settle(&mut harness);
    harness.step();
    let start = mock::camera();
    shot(&mut harness, &format!("{prefix}-1"));
    let mut results = Vec::new();
    // 1. Select checkout: a click on its header.
    let rect = mock::rect_of(&checkout).expect("checkout is on screen");
    click_at(&mut harness, rect.center_top() + egui::vec2(0.0, 12.0));
    let selected = mock::camera();
    results.push(format!("select: selection {:?}", selection(&mut harness).iter().map(|id| handle(&graph, id)).collect::<Vec<_>>()));
    results.push(format!(
        "select: camera zoom {:.3} → {:.3}, pan ({:.1},{:.1}) → ({:.1},{:.1})",
        start.0, selected.0, start.1.x, start.1.y, selected.1.x, selected.1.y
    ));
    assert_eq!(selection(&mut harness), vec![checkout.clone()]);
    shot(&mut harness, &format!("{prefix}-2"));
    // 2. The chip to the token's source.
    let label = "Source: login.body.token ↑";
    let chip = harness
        .query_by_label(label)
        .unwrap_or_else(|| panic!("no chip {label}"))
        .rect();
    click_at(&mut harness, chip.center());
    let jumped = mock::camera();
    let login_rect = mock::rect_of(&login).expect("login drawn");
    let back = harness.query_by_label("Back to checkout").is_some();
    results.push(format!(
        "chip: zoom {:.3} → {:.3}; login header on screen at ({:.0},{:.0}); Back chip {back}; selection {:?}",
        selected.0,
        jumped.0,
        login_rect.left(),
        login_rect.top(),
        selection(&mut harness).iter().map(|id| handle(&graph, id)).collect::<Vec<_>>()
    ));
    assert!(back, "Back to checkout appears");
    assert_eq!(jumped.0, selected.0, "the zoom stays");
    shot(&mut harness, &format!("{prefix}-3"));
    // 3. login's checks line: the Inspector lists its check.
    let line = harness
        .query_by_label("Checks of login")
        .expect("login's checks line")
        .rect();
    click_at(&mut harness, line.center());
    let inspector = mock::get().inspector.map(|id| handle(&graph, &id));
    let listed = harness.query_by_label_contains("logged_in").is_some();
    results.push(format!("checks: Inspector subject {inspector:?}, lists logged_in {listed}; selection {:?}",
        selection(&mut harness).iter().map(|id| handle(&graph, id)).collect::<Vec<_>>()));
    assert_eq!(inspector.as_deref(), Some("login"));
    assert!(listed, "the Inspector lists login's check");
    shot(&mut harness, &format!("{prefix}-4"));
    // 4. Back.
    let back = harness.query_by_label("Back to checkout").unwrap().rect();
    click_at(&mut harness, back.center());
    let restored = mock::camera();
    let subject = mock::get().inspector;
    let checks_of_checkout = harness.query_by_label_contains("checkout_total").is_some();
    results.push(format!(
        "back: camera ({:.3}, {:.2},{:.2}) vs before the jump ({:.3}, {:.2},{:.2}): exact {}; selection {:?}; Inspector override {subject:?}, lists checkout_total {checks_of_checkout}",
        restored.0,
        restored.1.x,
        restored.1.y,
        selected.0,
        selected.1.x,
        selected.1.y,
        restored == selected,
        selection(&mut harness).iter().map(|id| handle(&graph, id)).collect::<Vec<_>>()
    ));
    assert_eq!(restored, selected, "the camera is restored exactly");
    assert_eq!(selection(&mut harness), vec![checkout.clone()]);
    assert!(subject.is_none() && checks_of_checkout, "the Inspector shows checkout");
    shot(&mut harness, &format!("{prefix}-5"));
    let walk = mock::walk();
    note(&format!(
        "proof 4 {prefix} ({:.0}x{:.0}): 4 clicks, 0 pans, 0 zooms by the person; {} camera moves by the app; {}",
        size.x,
        size.y,
        walk.app_moves,
        results.join(" | ")
    ));
    let _ = folder(&harness);
    mock::set(Flags::default());
    mock::set_steps(None);
}

#[test]
#[ignore = "design-review view"]
fn review_lay00r4_walk() {
    walk(WIDE, "lay00r4-walk");
    walk(MINIMUM, "lay00r4-walk-small");
}

/// Extra: a branch across two rows on shop-50; each band fragment repeats
/// its header with the branch's state.
#[test]
#[ignore = "design-review view"]
fn review_lay00r4_band() {
    let (_project, mut harness, graph) = open("shop-50", WIDE);
    mock::set_steps(Some(run(&graph, &[], &[])));
    let ids: Vec<String> = ["paid", "payment", "notify", "cancel"]
        .iter()
        .filter(|h| graph.nodes.iter().any(|n| n.handle == **h))
        .map(|h| id_of(&graph, h))
        .collect();
    mock::set(Flags {
        view: Some((0.6, Anchor::Cards(ids))),
        ..compact(Vec::new())
    });
    shot(&mut harness, "lay00r4-band-continued-60");
    mock::set(Flags::default());
    mock::set_steps(None);
}
