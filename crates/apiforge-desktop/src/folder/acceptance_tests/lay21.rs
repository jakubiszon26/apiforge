//! LAY-21: every other card is compact too. Value, check, *Data condition*,
//! `if` and component call follow the compact geometry LAY-16 gave requests
//! (Q13 = A, Q14 = A, Q16 = A, the tray and the hidden footer).

use super::lay16::{open, shop_30, show};
use super::*;
use egui::Rect;

/// A component call and an `if` side by side, for the review: the two kinds
/// no mock-up drew.
const CALLS: &str = r#"scenario "Calls" {
  catalogue = GET "/products" {
    sample 200 { items: [{ id: 123, price: 100 }] }
  }
  var first = catalogue.body.items[0]
  product = product@1(id: first.id)
  promoted = active_promotion@1(product: product)
  on_sale = if promoted.promotion.price < first.price {
    sale = assert promoted.promotion.price > 0
  } else {
    full_price = assert first.price == 100
  }
}
"#;

/// Asserts that the last frame drew at least one card inside the viewport.
fn assert_cards_in_view(harness: &FolderHarness, context: &str) {
    let board = super::lay16::canvas(harness);
    let viewport = board.debug_viewport();
    assert!(
        board
            .debug_cards()
            .iter()
            .any(|(_, card)| viewport.contains_rect(*card)),
        "{context}: a whole card is in the viewport"
    );
}

/// `shop-30` tidied at the compact density, centred on `cart` at 60 % and
/// 100 %, at both sizes; then a component call and an `if`.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay21_board() {
    for (size, suffix) in [(WIDE, "1600"), (crate::testing::MINIMUM, "960")] {
        let (_project, mut harness) = open(&shop_30(), size);
        harness.get_by_label("Tidy up").click();
        settle(&mut harness);
        for (zoom, percent) in [(0.6, "60"), (1.0, "100")] {
            show(&mut harness, "cart", zoom);
            assert_cards_in_view(&harness, &format!("cart at {percent} % at {suffix}"));
            crate::testing::capture(&mut harness, &format!("lay21-cart-{percent}-{suffix}"), None);
        }
    }
    let (_project, mut harness) = open(CALLS, WIDE);
    harness.get_by_label("Tidy up").click();
    settle(&mut harness);
    for (zoom, percent) in [(0.6, "60"), (1.0, "100")] {
        show(&mut harness, "promoted", zoom);
        assert_cards_in_view(&harness, &format!("the calls at {percent} %"));
        let region = ["product", "promoted", "on_sale"]
            .into_iter()
            .filter_map(|handle| {
                super::lay16::canvas(&harness)
                    .debug_card_rect(&super::lay16::id_of(&harness, handle))
            })
            .reduce(Rect::union)
            .map(|rect| rect.expand(120.0));
        crate::testing::capture(
            &mut harness,
            &format!("lay21-call-and-if-{percent}-1600"),
            region,
        );
    }
}
