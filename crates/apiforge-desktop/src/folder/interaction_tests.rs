//! Folder window behaviour through named widgets (ADR 0013).
use super::*;
use crate::testing::{
    MINIMUM, WIDE, assert_disjoint, assert_within_window, folder, folder_harness, settle,
    shop_project,
};
use egui_kittest::kittest::Queryable;

const HEADER: [&str; 8] = [
    "Project",
    "Code",
    "Diagram",
    "Split",
    "Save",
    "Save all",
    "Run options",
    "Run",
];

const CARDS: [&str; 6] = [
    "Product (Component · r1)",
    "Active promotion (Component · r1)",
    "Add to cart (Component · r1)",
    "Expected price (Value)",
    "Field inna_nazwa (Comparison)",
    "Promotional price (Comparison)",
];

#[test]
fn the_mode_switches_change_the_view() {
    let project = shop_project();
    let mut harness = folder_harness(project.path(), WIDE);
    assert_eq!(folder(&harness).workbench.mode, Mode::Code);
    for (label, mode) in [
        ("Diagram", Mode::Diagram),
        ("Split", Mode::Split),
        ("Code", Mode::Code),
    ] {
        harness.get_by_label(label).click();
        settle(&mut harness);
        assert_eq!(folder(&harness).workbench.mode, mode, "after {label}");
    }
}

#[test]
fn the_header_fits_the_minimum_window_in_every_mode() {
    let project = shop_project();
    let mut harness = folder_harness(project.path(), MINIMUM);
    for mode in ["Code", "Diagram", "Split"] {
        harness.get_by_label(mode).click();
        settle(&mut harness);
        assert_within_window(&harness, &HEADER);
        assert_disjoint(&harness, &HEADER);
    }
}

#[test]
fn diagram_cards_are_named_and_visible() {
    let project = shop_project();
    for mode in ["Diagram", "Split"] {
        for size in [MINIMUM, WIDE] {
            let mut harness = folder_harness(project.path(), size);
            harness.get_by_label(mode).click();
            settle(&mut harness);
            assert_within_window(&harness, &CARDS);
        }
    }
}

/// Card heights depend on their rows, so a stored position that was fine for a
/// shorter card can leave two cards overlapping. The demo is the board every
/// review view and screenshot shows, so its own cards must stay disjoint at
/// both window sizes and in both modes that draw them.
#[test]
fn the_demo_board_never_overlaps_its_own_cards() {
    let project = apiforge_core::demos::shop_project();
    let reference = project.scenarios[0].graph.clone();
    let labels: Vec<String> = reference
        .nodes
        .iter()
        .map(|node| {
            let card = canvas::Card::build(node, &reference, &project);
            format!("{} ({})", card.name, card.caption)
        })
        .collect();
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    for size in [MINIMUM, WIDE] {
        for mode in [Mode::Diagram, Mode::Split] {
            let directory = crate::testing::shop_project();
            let mut harness = folder_harness(directory.path(), size);
            harness.state_mut().as_mut().unwrap().workbench.mode = mode;
            settle(&mut harness);
            assert_disjoint(&harness, &labels);
        }
    }
}
