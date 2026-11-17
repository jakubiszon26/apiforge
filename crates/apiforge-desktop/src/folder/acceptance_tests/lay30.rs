//! Mock-up of LAY-30 (#383): checks expanded by default, their switch on the
//! board's bar and on a step. Throwaway; `APIFORGE_LAY30` chooses the
//! variant (unset: `main`).
use super::lay23::{Checks, id_of, show, tidied};
use super::*;

/// The board's bar and what hangs under it, `below` px of it.
fn bar(harness: &FolderHarness, below: f32) -> egui::Rect {
    let tidy = harness.get_by_label("Tidy up").rect();
    egui::Rect::from_min_max(
        egui::pos2(tidy.left() - 560.0, tidy.top() - 14.0),
        egui::pos2(tidy.right() + 170.0, tidy.bottom() + 14.0 + below),
    )
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay30_mock() {
    let variant = std::env::var("APIFORGE_LAY30").unwrap_or_else(|_| "main".to_owned());
    let name = |view: &str| format!("mock-{variant}-{view}");
    if variant == "main" {
        // Today: a new install folds the checks; the switch is in the menu.
        let (_project, mut harness, id) = tidied("shop-30", WIDE, Checks::Collapsed);
        choose(&mut harness, "Fit the view");
        harness.step();
        crate::testing::capture(&mut harness, &name("default-fit-1600"), None);
        {
            let (_project, mut small, _) =
                tidied("shop-30", crate::testing::MINIMUM, Checks::Collapsed);
            choose(&mut small, "Fit the view");
            small.step();
            crate::testing::capture(&mut small, &name("default-fit-960"), None);
        }
        show(&mut harness, &id, "cart", 1.0);
        harness.get_by_label("Board view").click();
        settle(&mut harness);
        harness.step();
        let region = bar(&harness, 250.0);
        crate::testing::capture(&mut harness, &name("menu-1600"), Some(region));
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);
        harness.get_by_label("Checks of cart").click();
        settle(&mut harness);
        harness.step();
        crate::testing::capture(&mut harness, &name("step-inspector-1600"), None);
        return;
    }
    for (size, suffix) in [(WIDE, "1600"), (crate::testing::MINIMUM, "960")] {
        let (_project, mut harness, id) = tidied("shop-30", size, Checks::Expanded);
        choose(&mut harness, "Fit the view");
        harness.step();
        crate::testing::capture(&mut harness, &name(&format!("default-fit-{suffix}")), None);
        let region = bar(&harness, 0.0);
        crate::testing::capture(&mut harness, &name(&format!("bar-{suffix}")), Some(region));
        if suffix == "960" {
            continue;
        }
        show(&mut harness, &id, "cart_created", 1.0);
        crate::testing::capture(&mut harness, &name("step-100-1600"), None);
        if variant == "a" {
            // The checks of `cart` folded from its own switch.
            let cart = id_of(&harness, &id, "cart");
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .unwrap()
                .collapsed_here
                .insert(cart);
            settle(&mut harness);
            harness.step();
            crate::testing::capture(&mut harness, &name("step-folded-100-1600"), None);
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .unwrap()
                .collapsed_here
                .clear();
        }
        // The board's switch, off.
        harness.get_by_label("Checks").click();
        settle(&mut harness);
        harness.step();
        choose(&mut harness, "Fit the view");
        harness.step();
        crate::testing::capture(&mut harness, &name("collapsed-fit-1600"), None);
        let region = bar(&harness, 0.0);
        crate::testing::capture(&mut harness, &name("collapsed-bar-1600"), Some(region));
    }
}
