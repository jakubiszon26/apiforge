//! LAY-37 (#421): after Tidy up no chip touches another chip, a card or a
//! wire. In the gap between two columns the chips of the right card's inputs
//! (`← cart.id`), the counts of the left card's outputs (`→ 3`) and the
//! lines that run there keep apart: chips of one card stand a gap from each
//! other, a count and a chip facing it across the gap do not meet, and no
//! line runs through or beside a chip. On every reference board
//! (`fixtures/scenarios/lay/`, the demo shop), tidied, chips on.
use super::lay23::{Checks, board, fixture, graph, tidied_text};
use super::*;
use egui::{Pos2, Rect};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// The boards: the reference scenarios and the demo shop as it ships.
const BOARDS: [&str; 7] = [
    "fan-out",
    "far-value",
    "far-value-saved",
    "order-flow",
    "shop-30",
    "shop-50",
    "demo-shop",
];

/// The zooms a person reads chips at: from about half size, where a chip
/// still says its source, to full size.
const ZOOMS: [f32; 3] = [0.5, 0.75, 1.0];

/// The least room on screen between a chip and anything it does not belong
/// to: less than this and two marks touch.
const CLEAR: f32 = 3.0;

fn text_of(name: &str) -> String {
    if name == "demo-shop" {
        let project = crate::testing::shop_project();
        fs::read_to_string(project.path().join(SCENARIO)).unwrap()
    } else {
        fixture(name)
    }
}

/// The distance between two boxes, 0 where they meet.
fn apart(a: Rect, b: Rect) -> f32 {
    let x = (a.left() - b.right()).max(b.left() - a.right()).max(0.0);
    let y = (a.top() - b.bottom()).max(b.top() - a.bottom()).max(0.0);
    x.hypot(y)
}

/// Whether the segment `a`–`b` passes through `rect`.
fn crosses(rect: Rect, a: Pos2, b: Pos2) -> bool {
    // Liang–Barsky: clip the segment to the box.
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let mut low: f32 = 0.0;
    let mut high: f32 = 1.0;
    for (p, q) in [
        (-dx, a.x - rect.left()),
        (dx, rect.right() - a.x),
        (-dy, a.y - rect.top()),
        (dy, rect.bottom() - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                low = low.max(t);
            } else {
                high = high.min(t);
            }
        }
    }
    low <= high
}

/// What breaks the clearance on one board at one zoom: chip and chip, chip
/// and card, chip and line; and a chip that left its socket to keep it.
#[derive(Debug, Default)]
pub(super) struct Breaches {
    pub(super) chips: Vec<String>,
    pub(super) cards: Vec<String>,
    pub(super) lines: Vec<String>,
    pub(super) strays: Vec<String>,
}

pub(super) fn breaches(harness: &FolderHarness, id: &str, zoom: f32) -> Breaches {
    let canvas = board(harness, id);
    let graph = graph(harness, id);
    let handle = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map_or_else(|| id.to_owned(), |node| node.handle.clone())
    };
    // Board units: the clearance on screen at this zoom.
    let clear = CLEAR / zoom;
    let mut chips: Vec<(String, Rect)> = canvas
        .debug_tags()
        .iter()
        .map(|tag| {
            (
                format!("{} at {}", tag.shown, handle(&tag.tag.reader)),
                tag.world,
            )
        })
        .collect();
    chips.extend(canvas.debug_counts().iter().map(|count| {
        (
            format!(
                "count {} of {}",
                count.count.readers,
                handle(&count.count.source)
            ),
            count.world,
        )
    }));
    let cards: Vec<(String, Rect)> = graph
        .nodes
        .iter()
        .filter_map(|node| Some((node.handle.clone(), canvas.debug_world(&node.id)?)))
        .chain(
            canvas
                .debug_marks()
                .iter()
                .map(|mark| ("a continuation mark".to_owned(), mark.rect)),
        )
        .collect();
    let mut found = Breaches::default();
    // A chip stays beside its socket: left of its card, nearer it than a
    // card's width, and within a few of its heights above or below.
    for tag in canvas.debug_tags() {
        let socket = tag.tag.anchor;
        let (dx, dy) = (
            socket.x - tag.world.right(),
            tag.world.center().y - socket.y,
        );
        if !(dx > 0.0 && dx < 280.0 && dy.abs() <= 5.0 * tag.world.height()) {
            found.strays.push(format!(
                "{} at {}: {:?} from its socket",
                tag.shown,
                handle(&tag.tag.reader),
                (dx, dy)
            ));
        }
    }
    for (index, (name, rect)) in chips.iter().enumerate() {
        for (other, near) in &chips[index + 1..] {
            if apart(*rect, *near) < clear {
                found.chips.push(format!("{name} and {other}"));
            }
        }
        for (card, near) in &cards {
            if apart(*rect, *near) < clear {
                found.cards.push(format!("{name} and {card}"));
            }
        }
        let around = rect.expand(clear);
        for wire in canvas.debug_wires().iter().filter(|wire| !wire.revealed) {
            if wire
                .points
                .windows(2)
                .any(|segment| crosses(around, segment[0], segment[1]))
            {
                found.lines.push(format!(
                    "{name} and the line {} → {}",
                    wire.source.as_deref().map_or_else(String::new, handle),
                    handle(&wire.target)
                ));
            }
        }
    }
    found
}

/// The criteria: on every reference board, tidied with its checks folded
/// (the default) and drawn, at every zoom a person reads chips at, no chip
/// or count comes within 3 px on screen of another chip or count, of a card
/// or of a line drawn at rest, and every chip stays beside its socket.
#[test]
fn lay37_chips_keep_apart_from_chips_cards_and_lines_after_tidy_up() {
    let mut failures = Vec::new();
    for name in BOARDS {
        for checks in [Checks::Collapsed, Checks::Expanded] {
            let (_project, mut harness, id) = tidied_text(&text_of(name), WIDE, checks);
            let graph = graph(&harness, &id);
            let bounds = graph
                .nodes
                .iter()
                .filter_map(|node| board(&harness, &id).debug_world(&node.id))
                .reduce(|a, b| a.union(b))
                .unwrap_or(Rect::NOTHING);
            let first = graph.nodes.first().map(|node| node.handle.clone()).unwrap();
            for zoom in ZOOMS {
                super::lay23::show(&mut harness, &id, &first, zoom);
                let found = breaches(&harness, &id, zoom);
                let canvas = board(&harness, &id);
                errln!(
                    "LAY-37 {name} {checks:?} at {zoom}: {} chips, {} counts, \
                     {} chip/chip, {} chip/card, {} chip/line; board {:.0} x {:.0}",
                    canvas.debug_tags().len(),
                    canvas.debug_counts().len(),
                    found.chips.len(),
                    found.cards.len(),
                    found.lines.len(),
                    bounds.width(),
                    bounds.height()
                );
                for (kind, list) in [
                    ("chip/chip", &found.chips),
                    ("chip/card", &found.cards),
                    ("chip/line", &found.lines),
                    ("away from its socket", &found.strays),
                ] {
                    for item in list {
                        failures.push(format!("{name} {checks:?} at {zoom}, {kind}: {item}"));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} places where a chip touches something: {failures:#?}",
        failures.len()
    );
}

/// The review views for the owner: `shop-30` tidied, checks folded, at Fit
/// in 1600 × 1000 and 960 × 540, and at 100 % on `order`, the card of the
/// report, cropped to the gap left of it: `← login` and `← cart.id` at its
/// inputs, `→ 3` at `checkout`'s port across the gap.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay37() {
    for (size, width) in [(WIDE, 1600), (crate::testing::MINIMUM, 960)] {
        let (_project, mut harness, id) = tidied_text(&fixture("shop-30"), size, Checks::Collapsed);
        choose(&mut harness, "Fit the view");
        harness.step();
        crate::testing::capture(&mut harness, &format!("lay37-shop-30-fit-{width}"), None);
        super::lay23::show(&mut harness, &id, "order", 1.0);
        let order = super::lay23::id_of(&harness, &id, "order");
        let card = board(&harness, &id)
            .debug_cards()
            .iter()
            .find(|(card, _)| *card == order)
            .map(|(_, rect)| *rect)
            .expect("order is drawn");
        let region = Rect::from_min_max(
            egui::pos2(card.left() - 280.0, card.top() - 40.0),
            egui::pos2(card.left() + 120.0, card.top() + 200.0),
        );
        crate::testing::capture(
            &mut harness,
            &format!("lay37-shop-30-order-{width}"),
            Some(region),
        );
    }
}
