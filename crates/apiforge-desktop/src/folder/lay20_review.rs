//! LAY-20 mock-up views (throwaway branch `mock/lay-20-checks`): the *Checks*
//! tab of a component call, and the list of a Main step that a Cleanup check
//! reads, under each way of counting it.

use super::*;
use crate::mock_lay20::{self, Variant};
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, capture, folder, folder_harness_in, settle, shop_project,
};
use egui_kittest::kittest::Queryable;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

fn app(harness: &mut FolderHarness) -> &mut FolderApp {
    harness.state_mut().as_mut().unwrap()
}

/// shop-30 with a check on the component call `promoted` and a Cleanup check
/// that reads the Main step `cart`.
fn fixture() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-30.afs");
    let text = std::fs::read_to_string(path).unwrap();
    let edited = text
        .replacen(
            "  promoted = active_promotion@1(product: product)\n",
            "  promoted = active_promotion@1(product: product)\n  \
             promo_priced = assert promoted.promotion.price > 0\n",
            1,
        )
        .replacen(
            "    gone = assert removed.status == 204\n",
            "    gone = assert removed.status == 204\n    \
             cart_id_kept = assert cart.body.id == \"c-1\"\n",
            1,
        );
    assert_ne!(edited, text);
    assert!(edited.contains("cart_id_kept"));
    edited
}

fn open(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let project = shop_project();
    std::fs::write(project.path().join(SCENARIO), fixture()).unwrap();
    let mut harness = folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    (project, harness)
}

fn document(harness: &FolderHarness) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Scenario)
        .map(|document| document.id().to_owned())
        .expect("the scenario")
}

/// Selects the step `handle` of the Main graph and opens its checks.
fn show_checks(harness: &mut FolderHarness, handle: &str) {
    let document = document(harness);
    let model = folder(harness).snapshot(&document).unwrap().model.unwrap();
    let id = graph_of(&model, false)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no step {handle}"))
        .id
        .clone();
    app(harness).selections.insert(document, vec![id]);
    settle(harness);
    harness.get_by_label("Checks").click();
    settle(harness);
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay20_component_call() {
    for (size, name) in [(MINIMUM, "960"), (WIDE, "1600")] {
        let (_project, mut harness) = open(size);
        show_checks(&mut harness, "promoted");
        capture(&mut harness, &format!("lay20-component-{name}"), None);
    }
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay20_cleanup_check() {
    for (variant, name) in [
        (Variant::Counted, "counted"),
        (Variant::Apart, "apart"),
        (Variant::Section, "section"),
    ] {
        mock_lay20::set(variant);
        for (size, tag) in [(MINIMUM, "960"), (WIDE, "1600")] {
            let (_project, mut harness) = open(size);
            show_checks(&mut harness, "cart");
            capture(&mut harness, &format!("lay20-cleanup-{name}-{tag}"), None);
        }
    }
    mock_lay20::set(Variant::Counted);
}

/// The Cleanup board today: `cart_id_kept` is a card there, and the list of
/// `removed` names `gone`.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay20_cleanup_board() {
    let (_project, mut harness) = open(MINIMUM);
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    capture(&mut harness, "lay20-cleanup-board-960", None);
}
