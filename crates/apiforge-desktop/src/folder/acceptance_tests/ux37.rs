//! UX-37 (#271, owner decision of 2026-11-03 on #265): *Save as component*
//! still refuses by default to drop an output a later step reads, and offers
//! a checkbox under the refusal to save anyway and leave that later step's
//! input unconnected, to be fixed by hand.
use super::mcp08b::contains;
use super::ux31::{toggled, window};
use super::*;
use crate::testing::MINIMUM;
use apiforge_core::format::Graph;
use egui::accesskit::Toggled;
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const TITLE: &str = "Save as component";
const NAME: &str = "Component name";
const SAVE_AND_PUBLISH: &str = "Save and publish";
/// The refusal UX-30 shows when the cart is not kept: the cleanup's *Delete
/// cart* reads the cart's id.
const REFUSAL: &str =
    "A step after the selection reads Add to cart → cart.id; keep it as an output.";
const LEAVE: &str = "Save anyway and leave \"Delete cart\" unconnected";

fn disabled(harness: &FolderHarness, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().is_disabled()
}

/// The demo scenario at `size` in Diagram, *Add to cart* and *Promotional
/// price* selected, the dialog open and named, the cart's output unticked and
/// the check's value ticked in its place.
fn open_without_the_cart(root: &std::path::Path, size: egui::Vec2) -> (FolderHarness, String) {
    let (mut harness, id) = window(root, size, SCENARIO, "Diagram");
    open_dialog(&mut harness, &id);
    (harness, id)
}

fn open_dialog(harness: &mut FolderHarness, id: &str) {
    app(harness)
        .selections
        .insert(id.to_owned(), vec!["cart".to_owned(), "price".to_owned()]);
    settle(harness);
    choose(harness, "Selection actions");
    choose(harness, TITLE);
    assert!(has_label(harness, NAME), "the dialog is open");
    replace_text(harness, NAME, "Check cart price");
    choose(harness, "Use output 1");
    choose(harness, "Use output 2");
    assert_eq!(toggled(harness, "Use output 1"), Some(Toggled::False));
    assert_eq!(toggled(harness, "Use output 2"), Some(Toggled::True));
}

fn scenario(harness: &FolderHarness, id: &str) -> (Graph, Graph) {
    match folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .model()
        .cloned()
    {
        Some(Document::Scenario(scenario)) => (scenario.graph, scenario.cleanup),
        other => panic!("a scenario, not {other:?}"),
    }
}

/// Criterion 1: unticking the output *Delete cart* reads shows UX-30's
/// refusal and, under it, the checkbox, unticked; saving is disabled.
#[test]
fn ux37_criterion_1_the_refusal_stays_the_default() {
    for size in [MINIMUM, WIDE] {
        let directory = shop();
        let (harness, _) = open_without_the_cart(directory.path(), size);
        assert!(contains(&harness, REFUSAL), "the refusal at {size:?}");
        assert_eq!(
            toggled(&harness, LEAVE),
            Some(Toggled::False),
            "the checkbox at {size:?}"
        );
        assert!(
            harness.get_by_label(LEAVE).rect().top()
                >= harness.get_by_label_contains(REFUSAL).rect().top(),
            "the checkbox is under the refusal at {size:?}"
        );
        assert!(disabled(&harness, SAVE_AND_PUBLISH), "{size:?}");
    }
}

/// Criterion 2: ticked, saving is enabled; after *Save and publish* the
/// steps are one call, *Delete cart*'s `id` is unconnected and diagnosed,
/// and every other step of both boards is as it was.
#[test]
fn ux37_criterion_2_saved_with_the_reader_unconnected() {
    let directory = shop();
    let (mut harness, id) = open_without_the_cart(directory.path(), WIDE);
    let (main, cleanup) = scenario(&harness, &id);
    choose(&mut harness, LEAVE);
    assert_eq!(toggled(&harness, LEAVE), Some(Toggled::True));
    assert!(!disabled(&harness, SAVE_AND_PUBLISH));
    choose(&mut harness, SAVE_AND_PUBLISH);
    assert!(!has_label(&harness, NAME), "the dialog closed");
    if let Some(session) = app(&mut harness).session.as_mut() {
        session.settle(&id).unwrap();
    }
    settle(&mut harness);

    let source = source_of(&harness, &id);
    assert!(source.contains("check_cart_price@1("), "{source}");
    let (after_main, after_cleanup) = scenario(&harness, &id);
    let kept = |graph: &Graph| -> Vec<apiforge_core::format::Node> {
        graph
            .nodes
            .iter()
            .filter(|node| node.id != "cart" && node.id != "price")
            .filter(|node| {
                node.kind != apiforge_core::model::NodeKind::Component
                    || node.handle != "check_cart_price"
            })
            .cloned()
            .collect()
    };
    assert_eq!(kept(&after_main), kept(&main), "the other Main steps");
    let mut expected = cleanup.clone();
    let reader = expected
        .nodes
        .iter_mut()
        .find(|node| node.display_name() == "Delete cart")
        .unwrap();
    assert!(reader.inputs.shift_remove("id").is_some());
    assert_eq!(
        after_cleanup.nodes, expected.nodes,
        "only id is unconnected"
    );

    // The diagnostic points into the statement of *Delete cart*.
    let statement = source.find("DELETE").expect("the cleanup's request");
    let line = source[..statement].rfind('\n').map_or(0, |at| at + 1)
        ..source[statement..]
            .find('\n')
            .map_or(source.len(), |at| statement + at);
    let diagnostics = folder(&harness).snapshot(&id).unwrap().diagnostics.clone();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.range.start >= line.start && diagnostic.range.start <= line.end
        }),
        "the unconnected id is diagnosed on {:?}: {diagnostics:?}",
        &source[line.clone()]
    );
}

/// Criterion 3: one undo brings the steps and the reference back.
#[test]
fn ux37_criterion_3_undo_restores() {
    let directory = shop();
    let (mut harness, id) = open_without_the_cart(directory.path(), WIDE);
    let before = source_of(&harness, &id);
    choose(&mut harness, LEAVE);
    choose(&mut harness, SAVE_AND_PUBLISH);
    assert_ne!(source_of(&harness, &id), before);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut harness);
    assert_eq!(source_of(&harness, &id), before, "one undo restores");
}

/// Criterion 4: the choice is never remembered: cancelled and reopened, the
/// dialog shows the checkbox unticked.
#[test]
fn ux37_criterion_4_never_remembered() {
    let directory = shop();
    let (mut harness, id) = open_without_the_cart(directory.path(), WIDE);
    choose(&mut harness, LEAVE);
    assert_eq!(toggled(&harness, LEAVE), Some(Toggled::True));
    choose(&mut harness, "Cancel");
    assert!(!has_label(&harness, NAME), "the dialog closed");
    open_dialog(&mut harness, &id);
    assert_eq!(toggled(&harness, LEAVE), Some(Toggled::False));
    assert!(disabled(&harness, SAVE_AND_PUBLISH));
}
