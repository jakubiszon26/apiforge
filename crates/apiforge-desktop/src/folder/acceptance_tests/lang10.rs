//! LANG-10 (#354): the built-in `uuid()` reaches the window through the
//! operator catalogue. The block library lists *New UUID* under Text, and its
//! chip adds a Value block that computes `uuid()` with no operand slot.
use super::*;
use crate::testing::{MINIMUM, shop_project};
use apiforge_core::{
    format::Binding,
    graph::{VALUE_INPUT, input_slots},
    model::NodeKind,
    operator::Operator,
};

const CHIP: &str = "New UUID";

/// The scenario of the demo shop in Diagram, with the library's Text group of
/// operators open and the groups open by default folded, so it is in view.
fn text_operators(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let project = shop_project();
    let mut harness = folder_harness(project.path(), size);
    harness.get_by_label("Diagram").click();
    settle(&mut harness);
    harness.get_by_label("Operators").click();
    settle(&mut harness);
    for group in ["Comparisons", "Logic", "Text"] {
        harness.get_by_label(group).click();
        settle(&mut harness);
    }
    harness.get_by_label(CHIP).scroll_to_me();
    settle(&mut harness);
    (project, harness)
}

fn active_board(harness: &FolderHarness) -> Vec<apiforge_core::format::Node> {
    let app = folder(harness);
    let id = app.workbench.active().unwrap();
    let model = app
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .model()
        .expect("a valid model")
        .clone();
    graph_of(&model, false).nodes.clone()
}

/// The Text group lists *New UUID*; the chip adds a Value block computing
/// `uuid()`, which has no operand slot to wire.
#[test]
fn lang10_the_library_adds_a_new_uuid_block_without_operand_slots() {
    let (_project, mut harness) = text_operators(MINIMUM);
    let before = active_board(&harness).len();
    harness.get_by_label(CHIP).click();
    settle(&mut harness);

    let nodes = active_board(&harness);
    assert_eq!(nodes.len(), before + 1, "the chip adds a block");
    let added = nodes.last().unwrap();
    assert_eq!(added.kind, NodeKind::Value);
    assert_eq!(
        added.inputs.get(VALUE_INPUT),
        Some(&Binding::Expression {
            operator: Operator::Uuid,
            inputs: Default::default(),
        })
    );
    assert!(
        input_slots(added, &apiforge_core::demos::shop_project()).is_empty(),
        "New UUID has no operand slot"
    );
}

/// The library entry and the added block, for the pull request's QA.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lang10() {
    let (_project, mut harness) = text_operators(MINIMUM);
    harness.get_by_label(CHIP).click();
    settle(&mut harness);
    crate::testing::capture(&mut harness, "lang10-library-960", None);
}
