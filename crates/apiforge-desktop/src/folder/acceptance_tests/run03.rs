//! RUN-03 (#389): checks and values run before the next step, and the badge
//! numbers follow. `review_run03` renders the views of the pull request:
//! `shop-30` at `paid`, its checks drawn, where `not_created` now takes its
//! number before `payment`.
use super::lay28::{board, graph, tidied};
use super::*;
use crate::canvas::WireMode;

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_run03() {
    for size in [WIDE, crate::testing::MINIMUM] {
        let suffix = if size == WIDE { "1600x1000" } else { "960x540" };
        let (_project, mut harness, id) = tidied("shop-30", size, true, WireMode::Chips);
        let graph = graph(&harness, &id);
        let id_of = |handle: &str| {
            graph
                .nodes
                .iter()
                .find(|node| node.handle == handle)
                .unwrap_or_else(|| panic!("a step {handle}"))
                .id
                .clone()
        };
        let paid = id_of("paid");
        app(&mut harness)
            .canvases
            .get_mut(&id)
            .expect("the board is drawn")
            .debug_show(&paid, 1.0);
        settle(&mut harness);
        harness.step();
        let canvas = board(&harness, &id);
        let focus = canvas.debug_focus();
        assert!(
            canvas
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{suffix}: a card is in view"
        );
        // The view shows the numbers the pull request describes.
        for handle in ["paid", "not_created", "payment"] {
            assert!(
                board(&harness, &id).debug_badge(&id_of(handle)).is_some(),
                "{suffix}: {handle}'s badge is drawn"
            );
        }
        crate::testing::capture(&mut harness, &format!("run03-shop-30-paid-{suffix}"), None);
    }
}
