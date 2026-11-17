//! LAY-32 (#387): a value shown as a chip is read only through chips. Once an
//! output port is drawn with chips, every reader of it gets a chip, the one
//! beside the source too; no line leaves a port that shows a reader count.
//! The unit is the output port. On `fan-out` and `shop-30`, tidied.
use super::lay23::{
    Checks, board, chips, fixture, graph, handle_of, id_of, measures, revealed, select, show,
    tidied, tidied_text, wires,
};
use super::*;
use apiforge_core::{
    format::Graph,
    graph::{WireKind, wires as document_wires},
};
use std::collections::{BTreeMap, BTreeSet};

/// The data wires of each output port in the document, by `(source, port)`:
/// their ids and the steps they go into.
fn ports(graph: &Graph) -> BTreeMap<(String, String), (BTreeSet<String>, BTreeSet<String>)> {
    let mut ports: BTreeMap<(String, String), (BTreeSet<String>, BTreeSet<String>)> =
        BTreeMap::new();
    for wire in document_wires(graph) {
        let Some(port) = wire.source_port.filter(|_| wire.kind == WireKind::Data) else {
            continue;
        };
        let entry = ports.entry((wire.source, port)).or_default();
        entry.0.insert(wire.id);
        entry.1.insert(wire.target);
    }
    ports
}

/// The data lines the last frame drew at rest from `source`, by id: not
/// brought back by pointing or selecting.
fn lines_from(harness: &FolderHarness, id: &str, source: &str) -> BTreeSet<String> {
    board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| !wire.dashed && !wire.revealed && wire.source.as_deref() == Some(source))
        .map(|wire| wire.id.clone())
        .collect()
}

/// Criterion 1: on `fan-out` the neighbour `quantity` reads `cartId` and
/// `itemId` through chips; on `fan-out` and `shop-30`, checks collapsed and
/// drawn, no port that shows a count has a line leaving it. Prints the
/// lines and crossings of each board (criterion 5).
#[test]
fn lay32_criterion_1_a_port_with_a_count_draws_no_line() {
    let mut both = Vec::new();
    for name in ["fan-out", "shop-30"] {
        for checks in [Checks::Collapsed, Checks::Expanded] {
            let (_project, harness, id) = tidied(name, WIDE, checks);
            let graph = graph(&harness, &id);
            let ports = ports(&graph);
            let counts = board(&harness, &id).debug_counts();
            assert!(!counts.is_empty(), "{name} {checks:?} counts readers");
            for count in counts {
                let key = (count.count.source.clone(), count.count.port.clone());
                let lines: BTreeSet<String> = lines_from(&harness, &id, &count.count.source)
                    .intersection(&ports[&key].0)
                    .cloned()
                    .collect();
                if !lines.is_empty() {
                    both.push(format!(
                        "{name} {checks:?}: {} and {lines:?}",
                        count.count.label
                    ));
                }
            }
            let (lines, crossings, pairs) = measures(&harness, &id);
            errln!(
                "LAY-32 {name} {checks:?}: {lines} lines, {crossings} crossings, \
                 {pairs} coincident pairs, {} chips",
                board(&harness, &id).debug_tags().len()
            );
        }
    }

    let (_project, harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    let chips = chips(&harness, &id);
    for value in ["cartId", "itemId"] {
        assert!(
            chips.contains(&("quantity".to_owned(), value.to_owned())),
            "quantity, beside {value}, carries its chip: {chips:?}"
        );
        assert!(
            !wires(&harness, &id).contains(&(value.to_owned(), "quantity".to_owned())),
            "no wire from {value} to quantity"
        );
    }
    assert!(both.is_empty(), "a port with a count has lines: {both:#?}");
}

/// Criterion 2: a saved variable with at least one far reader has a chip
/// at every reader; with only readers beside it and fewer than three, it
/// stays wired.
#[test]
fn lay32_criterion_2_a_variable_read_far_is_read_through_chips() {
    // `first` is read by `product` alone, beside it: a wire, no chip, no
    // count.
    let (_project, harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    let first = id_of(&harness, &id, "first");
    assert!(wires(&harness, &id).contains(&("first".to_owned(), "product".to_owned())));
    assert!(
        !chips(&harness, &id).iter().any(|(_, text)| text == "first"),
        "no chip of first"
    );
    assert!(
        !board(&harness, &id)
            .debug_counts()
            .iter()
            .any(|count| count.count.source == first),
        "no count at first"
    );

    // `lookup`, far below, reads `first` too: both readers have chips and no
    // line leaves `first`.
    let text = fixture("fan-out").replace(
        "query { email: param.email, cart: cartId }",
        "query { email: param.email, cart: cartId, item: first.id }",
    );
    assert!(text.contains("item: first.id"));
    let (_project, harness, id) = tidied_text(&text, WIDE, Checks::Collapsed);
    let first = id_of(&harness, &id, "first");
    let chips = chips(&harness, &id);
    for reader in ["product", "lookup"] {
        assert!(
            chips.contains(&(reader.to_owned(), "first".to_owned())),
            "{reader} carries a chip of first: {chips:?}"
        );
    }
    assert!(
        lines_from(&harness, &id, &first).is_empty(),
        "no line leaves first: {:?}",
        wires(&harness, &id)
    );
    let count = board(&harness, &id)
        .debug_counts()
        .iter()
        .find(|count| count.count.source == first)
        .cloned()
        .expect("first counts its readers");
    assert_eq!(count.count.label, "first → 2 readers");
}

/// Criterion 3: the count at a port is the number of steps that read that
/// output in the document, checks collapsed and drawn.
#[test]
fn lay32_criterion_3_the_count_is_the_number_of_readers() {
    for name in ["fan-out", "shop-30"] {
        for checks in [Checks::Collapsed, Checks::Expanded] {
            let (_project, harness, id) = tidied(name, WIDE, checks);
            let graph = graph(&harness, &id);
            let ports = ports(&graph);
            for count in board(&harness, &id).debug_counts() {
                let (_, readers) = &ports[&(count.count.source.clone(), count.count.port.clone())];
                assert_eq!(
                    count.count.readers,
                    readers.len(),
                    "{name} {checks:?}: {} counts {:?}",
                    count.count.label,
                    readers
                        .iter()
                        .map(|reader| handle_of(&graph, reader))
                        .collect::<Vec<_>>()
                );
                // `1 reader` for a card hung under its source (LAY-31).
                let noun = if readers.len() == 1 {
                    "reader"
                } else {
                    "readers"
                };
                assert!(
                    count
                        .count
                        .label
                        .ends_with(&format!("→ {} {noun}", readers.len())),
                    "{}",
                    count.count.label
                );
            }
        }
    }
}

/// Criterion 4: below the threshold nothing changes, and the wires of the
/// neighbour's chips come back by pointing and by selecting as any chip's.
#[test]
fn lay32_criterion_4_below_the_threshold_and_brought_back() {
    let (_project, mut harness, id) = tidied("fan-out", WIDE, Checks::Collapsed);
    // Neither reader hangs under its source, which would read it through a
    // chip (LAY-31).
    for (source, target) in [("first", "product"), ("orderNumber", "lookup")] {
        assert!(
            wires(&harness, &id).contains(&(source.to_owned(), target.to_owned())),
            "{source} -> {target} stays a wire"
        );
    }
    show(&mut harness, &id, "quantity", 1.0);
    let quantity = id_of(&harness, &id, "quantity");
    let tags: Vec<_> = board(&harness, &id)
        .debug_tags()
        .iter()
        .filter(|tag| tag.tag.reader == quantity)
        .cloned()
        .collect();
    assert!(tags.len() >= 2, "quantity reads cartId and itemId");
    let own: BTreeSet<String> = tags.iter().flat_map(|tag| tag.tag.wires.clone()).collect();

    // Pointing at its chip.
    let tag = &tags[0];
    harness.get_by_label(&tag.tag.statement).hover();
    harness.step();
    harness.step();
    let shown = revealed(&harness, &id);
    assert!(
        tag.tag.wires.iter().all(|wire| shown.contains(wire)),
        "pointing at {} draws its wire: {shown:?}",
        tag.tag.statement
    );
    let empty = board(&harness, &id).debug_focus().left_top() + egui::vec2(8.0, 8.0);
    harness.hover_at(empty);
    harness.step();
    harness.step();
    assert!(revealed(&harness, &id).is_empty());

    // Selecting it.
    select(&mut harness, &id, &["quantity"]);
    assert_eq!(revealed(&harness, &id), own, "quantity selected");
    select(&mut harness, &id, &[]);
    assert!(revealed(&harness, &id).is_empty());
}

/// The review views for the owner: at 1600 × 1000 `fan-out` at 60 % on
/// `quantity`, beside `cartId` and `itemId`, which reads both through chips;
/// at 960 × 540 `shop-30` at 60 % on the port with the most readers. Each
/// asserts a card in view.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay32() {
    for (name, size, label) in [
        ("fan-out", WIDE, "lay32-fan-out-quantity-60-1600"),
        ("shop-30", crate::testing::MINIMUM, "lay32-shop-30-60-960"),
    ] {
        let (_project, mut harness, id) = tidied(name, size, Checks::Collapsed);
        let centre = if name == "fan-out" {
            "quantity".to_owned()
        } else {
            let graph = graph(&harness, &id);
            let count = board(&harness, &id)
                .debug_counts()
                .iter()
                .max_by_key(|count| count.count.readers)
                .cloned()
                .expect("shop-30 counts readers");
            handle_of(&graph, &count.count.source)
        };
        show(&mut harness, &id, &centre, 0.6);
        let canvas = board(&harness, &id);
        let focus = canvas.debug_focus();
        assert!(
            canvas
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{label}: a card is in view"
        );
        crate::testing::capture(&mut harness, label, None);
    }
}
