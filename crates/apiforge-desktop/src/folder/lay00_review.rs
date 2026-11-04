//! LAY-00: design review views of the card mock-ups (throwaway branch
//! `mock/lay-00-cards`). The switches in `crate::mock_lay00` change the real
//! board and Inspector for these views only.
use super::*;
use crate::mock_lay00::{self as mock, Flags};
use crate::testing::{FolderHarness, MINIMUM, WIDE, capture, folder, folder_harness, settle};
use apiforge_core::run::{Phase, StepResult};
use egui_kittest::kittest::Queryable;

const FLOW: &str = include_str!("../../../../docs/qa/2026-10-26/LAY/order-flow.afs");
const PATH: &str = "scenarios/order-flow.afs";

fn app(harness: &mut FolderHarness) -> &mut FolderApp {
    harness.state_mut().as_mut().unwrap()
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
}

fn document(harness: &FolderHarness) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == PATH)
        .unwrap_or_else(|| panic!("no {PATH}"))
        .id()
        .to_owned()
}

fn graph(harness: &FolderHarness) -> Graph {
    let id = document(harness);
    let model = folder(harness).snapshot(&id).unwrap().model.unwrap();
    graph_of(&model, false).clone()
}

fn id_of(harness: &FolderHarness, handle: &str) -> String {
    graph(harness)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no {handle}"))
        .id
        .clone()
}

fn select(harness: &mut FolderHarness, handles: &[&str]) {
    let ids: Vec<String> = handles
        .iter()
        .map(|handle| id_of(harness, handle))
        .collect();
    let document = document(harness);
    app(harness).selections.insert(document, ids);
    settle(harness);
}

/// The order flow in the demo shop at `size`, tidied up, on the board;
/// `failing` makes `cart`'s sample total 29 and runs the simulation.
fn window(size: egui::Vec2, failing: bool, flags: Flags) -> (tempfile::TempDir, FolderHarness) {
    mock::set(Flags::default());
    mock::set_steps(None);
    mock::clear_rects();
    mock::set_inspector(None);
    let source = if failing {
        FLOW.replace(
            "sample 201 { id: \"c-1\", total: 28.0 }",
            "sample 201 { id: \"c-1\", total: 29.0 }",
        )
    } else {
        FLOW.to_owned()
    };
    assert!(!failing || source != FLOW);
    // The demo shop with the order flow among its scenarios.
    let mut shop = apiforge_core::demos::shop_project();
    let lowered = apiforge_script::lower(
        &apiforge_script::parse(&source),
        &apiforge_script::LowerContext::folder(&shop),
    );
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
    let Some(apiforge_script::Document::Scenario(scenario)) = lowered.document else {
        panic!("the order flow is a scenario");
    };
    shop.scenarios.push(scenario);
    let project = tempfile::tempdir().unwrap();
    apiforge_project::prepare_project(project.path(), &shop).unwrap();
    let mut harness = folder_harness(project.path(), size);
    let id = document(&harness);
    app(&mut harness).activate_tab(&id);
    settle(&mut harness);
    click(&mut harness, "Diagram");
    click(&mut harness, "Tidy up");
    // Saved, so the run's results stay fresh while the views change.
    click(&mut harness, "Save");
    if failing {
        click(&mut harness, "Run simulation");
        let deadline = Instant::now() + Duration::from_secs(30);
        while folder(&harness).runs.handle.is_some() {
            assert!(Instant::now() < deadline, "the run must finish");
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
        settle(&mut harness);
        // The results fold to their bar, so the board keeps its room.
        click(&mut harness, "Run results");
    }
    mock::set(flags);
    settle(&mut harness);
    (project, harness)
}

fn shot(harness: &mut FolderHarness, name: &str) {
    settle(harness);
    harness.step();
    capture(harness, name, None);
}

/// A crop around the cards of `handles`.
fn crop(harness: &mut FolderHarness, name: &str, handles: &[&str], margin: f32) {
    settle(harness);
    harness.step();
    let region = handles
        .iter()
        .map(|handle| {
            let id = id_of(harness, handle);
            mock::rect_of(&id).unwrap_or_else(|| panic!("{handle} was not drawn"))
        })
        .reduce(egui::Rect::union)
        .unwrap()
        .expand(margin);
    capture(harness, name, Some(region));
}

/// View 1: collapsing checks into their subjects, the toggle in the toolbar.
#[test]
#[ignore = "design-review view"]
fn review_lay00_collapse() {
    for (size, suffix) in [(WIDE, "1600x1000"), (MINIMUM, "960x540")] {
        let flags = Flags {
            toggle: true,
            fit: true,
            ..Flags::default()
        };
        let (_project, mut harness) = window(size, false, flags);
        shot(&mut harness, &format!("lay00-collapse-expanded-{suffix}"));
        click(&mut harness, "Checks ▾");
        assert!(mock::get().collapsed, "the toggle collapses");
        shot(&mut harness, &format!("lay00-collapse-collapsed-{suffix}"));
    }
    let flags = Flags {
        toggle: true,
        collapsed: true,
        fit: true,
        ..Flags::default()
    };
    let (_project, mut harness) = window(WIDE, true, flags);
    shot(&mut harness, "lay00-collapse-collapsed-failed-1600x1000");
    crop(&mut harness, "lay00-collapse-cart-failed", &["cart"], 16.0);
    // A click on the aggregate: the Inspector lists the step's checks.
    // Before the selection: the real Inspector, drawn for a selected
    // request, edits the document and leaves the run stale.
    let cart = id_of(&harness, "cart");
    mock::update(|flags| flags.inspector = Some(cart));
    select(&mut harness, &["cart"]);
    shot(&mut harness, "lay00-collapse-inspector-1600x1000");
    let panel = mock::inspector_rect().expect("the Inspector was drawn");
    capture(&mut harness, "lay00-collapse-inspector-crop", Some(panel));
}

const ZOOM_CARDS: [&str; 4] = ["total", "cart", "cart_total", "order"];

/// View 2: semantic zoom, today and proposed, after the failed run.
#[test]
#[ignore = "design-review view"]
fn review_lay00_zoom() {
    let (_project, mut harness) = window(WIDE, true, Flags::default());
    let ids: Vec<String> = ZOOM_CARDS
        .iter()
        .map(|handle| id_of(&harness, handle))
        .collect();
    for (zoom, name) in [(0.35, "35"), (0.60, "60"), (0.90, "90")] {
        for (variant, proposed, collapsed) in [
            ("today", false, false),
            ("proposed", true, false),
            ("proposed-collapsed", true, true),
        ] {
            mock::set(Flags {
                proposed,
                collapsed,
                view: Some((zoom, ids.clone())),
                ..Flags::default()
            });
            mock::clear_rects();
            settle(&mut harness);
            let cards: Vec<&str> = ZOOM_CARDS
                .iter()
                .copied()
                .filter(|handle| !collapsed || *handle != "cart_total")
                .collect();
            crop(
                &mut harness,
                &format!("lay00-zoom-{name}-{variant}"),
                &cards,
                40.0 * zoom,
            );
        }
    }
}

/// View 3: focus on the selection.
#[test]
#[ignore = "design-review view"]
fn review_lay00_focus() {
    let flags = Flags {
        focus: true,
        fit: true,
        ..Flags::default()
    };
    let (_project, mut harness) = window(WIDE, false, flags);
    shot(&mut harness, "lay00-focus-none");
    select(&mut harness, &["cart"]);
    shot(&mut harness, "lay00-focus-request-cart");
    let cart = id_of(&harness, "cart");
    mock::update(|flags| flags.view = Some((0.6, vec![cart])));
    shot(&mut harness, "lay00-focus-request-cart-60");
    mock::update(|flags| flags.view = None);
    select(&mut harness, &["cart_total"]);
    shot(&mut harness, "lay00-focus-check-cart_total");
    // Collapsed: the request's aggregate turns accent.
    mock::update(|flags| {
        flags.collapsed = true;
        flags.toggle = true;
    });
    select(&mut harness, &["cart"]);
    shot(&mut harness, "lay00-focus-request-cart-collapsed");
}

/// View 4: a run in progress, faked: the steps before `order` passed,
/// `order` runs, the rest wait.
#[test]
#[ignore = "design-review view"]
fn review_lay00_running() {
    let (_project, mut harness) = window(WIDE, false, Flags::default());
    let graph = graph(&harness);
    let sequence = mock::sequence(&graph);
    let order = id_of(&harness, "order");
    let running_at = sequence[&order];
    let steps: Vec<StepResult> = graph
        .nodes
        .iter()
        .filter(|node| sequence[&node.id] < running_at)
        .map(|node| {
            let mut step = StepResult::new(node, Phase::Main);
            step.duration_ms = match node.kind {
                apiforge_core::model::NodeKind::Request => 40 + 7 * sequence[&node.id] as u64,
                _ => 0,
            };
            step
        })
        .collect();
    mock::set_steps(Some(steps));
    mock::set(Flags {
        proposed: true,
        running: Some(order),
        fit: true,
        ..Flags::default()
    });
    shot(&mut harness, "lay00-running-1600x1000");
    let ids = ["cart_total", "order", "created"].map(|handle| id_of(&harness, handle));
    mock::update(|flags| flags.view = Some((0.6, ids.to_vec())));
    mock::clear_rects();
    crop(
        &mut harness,
        "lay00-running-order",
        &["cart_total", "order", "created"],
        24.0,
    );
    mock::set_steps(None);
}

/// View 5: the branches of `created` as bands, one wire into each.
#[test]
#[ignore = "design-review view"]
fn review_lay00_branch_band() {
    let flags = Flags {
        bands: true,
        fit: true,
        ..Flags::default()
    };
    let (_project, mut harness) = window(WIDE, false, flags);
    shot(&mut harness, "lay00-branch-band-1600x1000");
}
