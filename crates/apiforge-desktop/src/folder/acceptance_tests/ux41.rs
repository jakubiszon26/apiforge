//! UX-41 (#519): the selected step no longer looks like an error. A
//! selected card has a 2 px blue frame on its edge with a faint halo and no
//! corner handles, and the selected step's wires are the same blue (B,
//! decided 2026-11-21); red always means failed. On `shop-30` after a
//! failed simulation, as LAY-10's tests open it. The owner's decision of
//! 2026-11-21 on #519 adds criteria 6-8: the selection's other marks, on
//! the minimap, on a chip and the box a drag draws to select, take the
//! same blue.

use super::lay07;
use super::lay10::{after_failure_in, empty_point, failing, fit};
use super::lay16::{
    canvas, document, graph, id_of, move_to, press, release, select, show, switch_density, world_of,
};
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

/// Criterion 6: a selected card is marked on the minimap in
/// `theme::SELECTION`, and only the selection is marked.
#[test]
fn ux41_criterion_6_the_minimap_marks_the_selection_in_its_colour() {
    let (_project, mut harness, _gate) = after_failure_in(WIDE, true);
    select(&mut harness, &["cart"]);
    fit(&mut harness);
    assert!(
        canvas(&harness).debug_minimap().is_some(),
        "the minimap is drawn"
    );
    assert_eq!(
        canvas(&harness).debug_minimap_marks(),
        [(id_of(&harness, "cart"), theme::SELECTION)].as_slice(),
        "cart, and only cart, is marked in the selection's colour"
    );
}

/// Criterion 7: a selected chip is outlined in `theme::SELECTION`, an
/// unselected one is not, and the minimap marks the chip in it too.
#[test]
fn ux41_criterion_7_a_selected_chip_is_outlined_in_its_colour() {
    let (_project, mut harness, _gate) = after_failure_in(WIDE, true);
    let cart = world_of(&harness, "cart");
    app(&mut harness).active_board_command(crate::canvas::Command::AddProxy {
        source: apiforge_core::format::ProxySource::Environment {
            name: "baseUrl".into(),
        },
        position: apiforge_core::format::Position {
            x: f64::from(cart.left()),
            y: f64::from(cart.bottom() + 120.0),
        },
    });
    settle(&mut harness);
    let chip = graph(&harness)
        .proxies
        .last()
        .expect("the chip is on the board")
        .id
        .clone();
    select(&mut harness, &[]);
    show(&mut harness, "cart", 0.6);
    let outline = canvas(&harness)
        .debug_chip_outline(&chip)
        .expect("the chip is drawn");
    assert_ne!(outline, theme::SELECTION, "an unselected chip");

    let document = document(&harness);
    app(&mut harness)
        .selections
        .insert(document, vec![chip.clone()]);
    settle(&mut harness);
    harness.step();
    assert_eq!(
        canvas(&harness).debug_chip_outline(&chip),
        Some(theme::SELECTION),
        "the selected chip's outline"
    );
    assert!(
        canvas(&harness)
            .debug_minimap_marks()
            .contains(&(chip, theme::SELECTION)),
        "the minimap marks the selected chip: {:?}",
        canvas(&harness).debug_minimap_marks()
    );
}

/// Criterion 8: the box a drag on the empty board draws to select is
/// `theme::SELECTION`, its fill a faint wash of it.
#[test]
fn ux41_criterion_8_the_selecting_box_is_in_its_colour() {
    let (_project, mut harness, _gate) = after_failure_in(WIDE, true);
    select(&mut harness, &[]);
    fit(&mut harness);
    let from = empty_point(&harness);
    let to = from + egui::vec2(80.0, 60.0);
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    assert_eq!(
        canvas(&harness).debug_marquee(),
        Some((theme::SELECTION.gamma_multiply(0.08), theme::SELECTION)),
        "the selecting box"
    );
    release(&mut harness, to);
    settle(&mut harness);
    harness.step();
    assert_eq!(
        canvas(&harness).debug_marquee(),
        None,
        "gone after the drag"
    );
}

/// The selection's other marks (criteria 6-8) in one view at
/// 1600 × 1000: a chip selected beside `cart`, marked on the minimap, and
/// the selecting box half drawn over the empty board.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux41_marks() {
    let (_project, mut harness, _gate) = after_failure_in(WIDE, true);
    harness.get_by_label("Run results").click();
    settle(&mut harness);
    let cart = world_of(&harness, "cart");
    app(&mut harness).active_board_command(crate::canvas::Command::AddProxy {
        source: apiforge_core::format::ProxySource::Environment {
            name: "baseUrl".into(),
        },
        position: apiforge_core::format::Position {
            x: f64::from(cart.left()),
            y: f64::from(cart.bottom() + 120.0),
        },
    });
    settle(&mut harness);
    let chip = graph(&harness).proxies.last().expect("the chip").id.clone();
    let document = document(&harness);
    app(&mut harness).selections.insert(document, vec![chip]);
    settle(&mut harness);
    show(&mut harness, "cart", 0.6);
    let from = empty_point(&harness);
    let to = from + egui::vec2(160.0, 110.0);
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    assert!(
        canvas(&harness).debug_marquee().is_some(),
        "the box is drawn"
    );
    crate::testing::capture(&mut harness, "ux41-marks-1600", None);
    release(&mut harness, to);
}
