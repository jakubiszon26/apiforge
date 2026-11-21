//! UX-41 (#519): the selected step no longer looks like an error. A
//! selected card has a 2 px blue frame on its edge with a faint halo and no
//! corner handles, and the selected step's wires are the same blue (B,
//! decided 2026-11-21); red always means failed. On `shop-30` after a
//! failed simulation, as LAY-10's tests open it.

use super::lay07;
use super::lay10::{after_failure_in, failing, fit};
use super::lay16::{canvas, document, id_of, select, show, switch_density};
use super::*;
use crate::canvas::DrawnFrame;
use crate::theme;
use apiforge_core::block::Block;
use egui::Color32;

/// The blue of the approved mock-up.
const BLUE: Color32 = Color32::from_rgb(44, 108, 214);

/// The frame's halo: its two strokes, from the inner one out.
const HALO: [f32; 2] = [0.18, 0.10];

/// The hue of an opaque colour, in degrees.
fn hue(color: Color32) -> f32 {
    let [r, g, b] = [color.r(), color.g(), color.b()].map(|c| f32::from(c) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let range = max - min;
    if range == 0.0 {
        return 0.0;
    }
    let sixth = if max == r {
        ((g - b) / range).rem_euclid(6.0)
    } else if max == g {
        (b - r) / range + 2.0
    } else {
        (r - g) / range + 4.0
    };
    sixth * 60.0
}

/// The distance of two hues around the circle, in degrees.
fn hue_apart(a: Color32, b: Color32) -> f32 {
    let apart = (hue(a) - hue(b)).abs();
    apart.min(360.0 - apart)
}

/// The selection's frame around `handle`: 2 px of [`theme::SELECTION`] on
/// the card's edge, the halo outside it, no corner handle.
fn assert_frame(harness: &FolderHarness, handle: &str, context: &str) -> DrawnFrame {
    let id = id_of(harness, handle);
    let board = canvas(harness);
    assert!(board.debug_selected(&id), "{context}: {handle} is selected");
    let frame = board
        .debug_frame(&id)
        .unwrap_or_else(|| panic!("{context}: {handle} has the selection's frame"));
    assert_eq!(frame.handles, 0, "{context}: no corner handle");
    let edge: Vec<_> = frame
        .strokes
        .iter()
        .filter(|stroke| stroke.color == theme::SELECTION)
        .collect();
    assert_eq!(
        edge.len(),
        1,
        "{context}: one stroke in the selection's colour: {frame:?}"
    );
    assert_eq!(
        edge[0].gap, 0.0,
        "{context}: the frame is on the card's edge"
    );
    assert_eq!(edge[0].width, 2.0, "{context}: the frame is 2 px");
    let halo: Vec<_> = frame
        .strokes
        .iter()
        .filter(|stroke| stroke.color != theme::SELECTION)
        .collect();
    assert_eq!(
        halo.iter().map(|stroke| stroke.color).collect::<Vec<_>>(),
        HALO.iter()
            .rev()
            .map(|share| theme::SELECTION.gamma_multiply(*share))
            .collect::<Vec<_>>(),
        "{context}: the halo, two strokes of the selection's colour at 10 % and 18 %, \
         drawn from the outer one in"
    );
    assert!(
        halo[1].gap >= edge[0].gap + edge[0].width && halo[0].gap >= halo[1].gap + halo[1].width,
        "{context}: the halo is outside the frame, the faintest stroke outermost: {frame:?}"
    );
    frame
}

/// No card but the selected one has a frame.
fn assert_framed_alone(harness: &FolderHarness, handle: &str, context: &str) {
    let selected = id_of(harness, handle);
    let board = canvas(harness);
    for (id, _) in board.debug_cards() {
        if *id != selected {
            assert!(board.debug_frame(id).is_none(), "{context}: {id} is framed");
        }
    }
}

/// Criterion 1: a selected card draws the 2 px frame in
/// `theme::SELECTION` on its edge, the halo of the mock-up and no corner
/// handle, at Overview, Compact and Full: a request and a check at the
/// zooms of the three details on compact cards, the same on full cards,
/// and `begin`, which keeps the full card at every density.
#[test]
fn ux41_criterion_1_frame_without_handles() {
    let (_project, mut harness, _gate) = after_failure_in(WIDE, false);
    for density in ["compact", "full"] {
        if density == "full" {
            switch_density(&mut harness);
            settle(&mut harness);
        }
        for handle in ["cart", "cart_total", "begin"] {
            select(&mut harness, &[handle]);
            for zoom in [0.35, 0.6, 1.0] {
                show(&mut harness, handle, zoom);
                let context = format!("{handle}, {density} cards, at {zoom}");
                assert_frame(&harness, handle, &context);
                assert_framed_alone(&harness, handle, &context);
            }
        }
    }
}

/// Criterion 2: the selection's colour is one constant, the blue of the
/// mock-up, apart from the colours of a run and of the focus; the frame
/// and the selected step's wires use it, and no other wire does.
#[test]
fn ux41_criterion_2_one_colour_not_a_status() {
    assert_eq!(theme::SELECTION, BLUE, "the blue of the mock-up");
    let check = theme::block_color(Block::Comparison);
    for (name, other) in [
        ("failed", theme::DANGER),
        ("blocked", theme::WARNING),
        ("passed", theme::SUCCESS),
        ("related", crate::canvas::RELATED),
        ("check", check),
    ] {
        assert_ne!(theme::SELECTION, other, "the {name} colour");
        let apart = hue_apart(theme::SELECTION, other);
        assert!(
            apart >= 30.0,
            "the selection's hue is {apart}° from the {name} colour"
        );
    }

    let (_project, mut harness, _gate) = after_failure_in(WIDE, false);
    for handle in ["cart", "login", "begin"] {
        select(&mut harness, &[handle]);
        fit(&mut harness);
        assert_frame(&harness, handle, handle);
        let wires = canvas(&harness).debug_wires();
        let lit: Vec<_> = wires
            .iter()
            .filter(|wire| wire.lit && !wire.points.is_empty())
            .collect();
        assert!(!lit.is_empty(), "{handle}: its wires are drawn");
        for wire in &lit {
            assert_eq!(
                wire.colour,
                Some(theme::SELECTION),
                "{handle}: its wire {} is the selection's",
                wire.id
            );
        }
        for wire in wires
            .iter()
            .filter(|wire| !wire.lit && !wire.points.is_empty())
        {
            assert_ne!(
                wire.colour,
                Some(theme::SELECTION),
                "{handle}: {} is not its wire",
                wire.id
            );
        }
    }
}

/// Criterion 3: on a failed and selected step both the frame and the red
/// signal border are drawn, and the step keeps its strength. The failed
/// check drawn as a card, and its request with the checks folded.
#[test]
fn ux41_criterion_3_failure_shows_under_selection() {
    for (collapsed, handle) in [(false, "checkout_total"), (true, "checkout")] {
        let (_project, mut harness, _gate) = after_failure_in(WIDE, collapsed);
        select(&mut harness, &[handle]);
        for zoom in [0.35, 1.0] {
            show(&mut harness, handle, zoom);
            let context = format!("{handle} at {zoom}");
            let id = id_of(&harness, handle);
            assert_eq!(
                canvas(&harness).debug_signal(&id),
                Some(theme::DANGER),
                "{context}: the red border of the failure"
            );
            assert_frame(&harness, handle, &context);
            assert!(!canvas(&harness).debug_dimmed(&id), "{context}: not dimmed");
        }
    }
}

/// The views of the owner's acceptance, on `shop-30` tidied after a failed
/// simulation, the Run results folded, checks folded: the failed
/// `checkout` and the passed `cart` selected at 100 % at 1600 × 1000, and
/// `cart` selected at 60 % in the whole window at 960 × 540 and
/// 1600 × 1000.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux41() {
    for (size, selected, zoom, name) in [
        (WIDE, "checkout", 1.0, "ux41-failed-100"),
        (WIDE, "cart", 1.0, "ux41-passed-100"),
        (crate::testing::MINIMUM, "cart", 0.6, "ux41-window-960"),
        (WIDE, "cart", 0.6, "ux41-window-1600"),
    ] {
        let project = crate::testing::shop_project();
        fs::write(
            project
                .path()
                .join("scenarios/promotion-and-cart-price.afs"),
            failing(),
        )
        .unwrap();
        fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
        let (_project, mut harness, gate) = lay07::opened(project, size, false);
        let document = document(&harness);
        app(&mut harness).selections.insert(document, Vec::new());
        settle(&mut harness);
        choose(&mut harness, "Tidy up");
        harness.step();
        lay07::simulate(&mut harness, &gate);
        harness.get_by_label("Run results").click();
        settle(&mut harness);
        select(&mut harness, &[selected]);
        show(&mut harness, selected, zoom);
        let id = id_of(&harness, selected);
        assert!(
            canvas(&harness).debug_frame(&id).is_some(),
            "{name}: the selection is drawn"
        );
        let region = (zoom >= 1.0).then(|| {
            canvas(&harness)
                .debug_card_rect(&id)
                .expect("the card is drawn")
                .expand2(egui::vec2(170.0, 70.0))
        });
        crate::testing::capture(&mut harness, name, region);
    }
}
