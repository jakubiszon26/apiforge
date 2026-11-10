//! LAY-02 (#33): a scenario opened from code without positions is laid out
//! in layers, as *Tidy up* would place it, at the board's density and in
//! view; a chip left far from its readers is brought beside its first reader
//! by *Tidy up*. On the LAY-01 fixtures `order-flow` and `shop-30`, written
//! over the demo shop's scenario with the project's `diagram.json` removed,
//! so that nothing has a position when the window opens.
use super::*;
use crate::canvas::{Card, Chip, Command};
use crate::testing::{MINIMUM, folder_harness_in};
use apiforge_core::format::{Binding, Node, Position, ProxySource};
use std::collections::BTreeMap;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";
/// The free space the layered layout keeps between the cards of a column.
const ROW_GAP: f32 = 48.0;

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// The demo shop with `source` as its scenario and no diagram at all.
fn project_without_positions(source: &str) -> tempfile::TempDir {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    project
}

/// The window on `root`, the scenario's board shown and drawn.
fn open_at(root: &std::path::Path, size: egui::Vec2) -> (FolderHarness, String) {
    let mut harness = folder_harness_in(root, size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    (harness, id)
}

fn open(name: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = project_without_positions(&fixture(name));
    let (harness, id) = open_at(project.path(), size);
    (project, harness, id)
}

fn model(harness: &FolderHarness, id: &str) -> Document {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .model()
        .expect("a valid model")
        .clone()
}

fn main_graph(harness: &FolderHarness, id: &str) -> Graph {
    graph_of(&model(harness, id), false).clone()
}

fn canvas<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

/// Every block of the main board by handle, with its identity and position.
fn blocks(harness: &FolderHarness, id: &str) -> BTreeMap<String, (String, Position)> {
    main_graph(harness, id)
        .nodes
        .into_iter()
        .map(|node| (node.handle.clone(), (node.id.clone(), node.position)))
        .collect()
}

fn positions(harness: &FolderHarness, id: &str) -> BTreeMap<String, (i64, i64)> {
    blocks(harness, id)
        .into_iter()
        .map(|(handle, (_, at))| (handle, (at.x.round() as i64, at.y.round() as i64)))
        .collect()
}

/// The layer of each block, by the shared layout's longest path.
fn layers(graph: &Graph) -> Vec<usize> {
    let index_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
    let blocks: Vec<apiforge_layout::Block> = graph
        .nodes
        .iter()
        .map(|_| {
            apiforge_layout::Block::new(
                apiforge_layout::Size::new(1.0, 1.0),
                apiforge_layout::Point::default(),
            )
        })
        .collect();
    let edges: Vec<apiforge_layout::Edge> = apiforge_core::graph::wires(graph)
        .iter()
        .filter_map(|wire| {
            Some(apiforge_layout::Edge::new(
                index_of(&wire.source)?,
                index_of(&wire.target)?,
            ))
        })
        .collect();
    apiforge_layout::layers_of(&apiforge_layout::Graph::new(&blocks, &edges))
}

/// The cards of the last frame, on screen.
fn cards_on_screen(harness: &FolderHarness, id: &str) -> Vec<(String, egui::Rect)> {
    canvas(harness, id).debug_cards().to_vec()
}

fn assert_cards_disjoint(cards: &[(String, egui::Rect)]) {
    for (index, (first, a)) in cards.iter().enumerate() {
        for (second, b) in &cards[index + 1..] {
            assert!(
                !a.intersects(*b) || a.intersect(*b).area() == 0.0,
                "{first} at {a:?} overlaps {second} at {b:?}"
            );
        }
    }
}

/// The gap between two rectangles, zero when they touch or overlap.
fn separation(a: egui::Rect, b: egui::Rect) -> f32 {
    let dx = (b.min.x - a.max.x).max(a.min.x - b.max.x).max(0.0);
    let dy = (b.min.y - a.max.y).max(a.min.y - b.max.y).max(0.0);
    dx.max(dy)
}

/// Criterion 3: `order-flow` opens with one column per layer, the layers
/// left to right, and no card on another.
#[test]
fn lay02_criterion_3_order_flow_opens_in_layers() {
    let (_project, harness, id) = open("order-flow", WIDE);
    let graph = main_graph(&harness, &id);
    let layers = layers(&graph);
    let cards = cards_on_screen(&harness, &id);
    assert_eq!(cards.len(), graph.nodes.len(), "every card is drawn");
    let centre = |node: &Node| {
        cards
            .iter()
            .find(|(card, _)| *card == node.id)
            .unwrap_or_else(|| panic!("{} is drawn", node.handle))
            .1
            .center()
            .x
    };
    let depth = layers.iter().max().map_or(0, |max| max + 1);
    assert!(depth > 3, "order-flow is a flow of several layers");
    let mut columns: Vec<Option<f32>> = vec![None; depth];
    for (node, layer) in graph.nodes.iter().zip(&layers) {
        let x = centre(node);
        match columns[*layer] {
            Some(column) => assert!(
                (column - x).abs() < 1.0,
                "{} stands in the column of layer {layer} at {column}, not at {x}",
                node.handle
            ),
            None => columns[*layer] = Some(x),
        }
    }
    let columns: Vec<f32> = columns.into_iter().flatten().collect();
    for pair in columns.windows(2) {
        assert!(
            pair[1] > pair[0] + 1.0,
            "the layers run left to right: {columns:?}"
        );
    }
    assert_cards_disjoint(&cards);
}

/// Criterion 4: the layout made on open is recorded at the first analysis
/// (KTLO-04), so a second open shows the same identities and positions, and
/// a fresh project from the same code opens in the same positions. A code
/// edit adding one step, then three, moves nothing already placed. A scenario
/// that has positions opens exactly where it was.
#[test]
fn lay02_criterion_4_a_second_open_is_the_same_and_edits_stay_incremental() {
    let project = project_without_positions(&fixture("order-flow"));
    let (harness, id) = open_at(project.path(), WIDE);
    let opened = blocks(&harness, &id);
    drop(harness);
    assert!(
        project.path().join(DIAGRAM).exists(),
        "the first analysis records the layout"
    );
    let (harness, id) = open_at(project.path(), WIDE);
    assert_eq!(blocks(&harness, &id), opened, "the second open is the same");
    let placed = positions(&harness, &id);
    drop(harness);

    let other = project_without_positions(&fixture("order-flow"));
    let (mut harness, id) = open_at(other.path(), WIDE);
    assert_eq!(
        positions(&harness, &id),
        placed,
        "the layout on open is deterministic"
    );

    for added in [
        &["one_more = assert total > 0"][..],
        &[
            "two_more = assert total > 1",
            "three_more = assert cart.status == 201",
            "four_more = assert order.status == 201",
        ],
    ] {
        let before = positions(&harness, &id);
        let lines: String = added.iter().map(|line| format!("  {line}\n")).collect();
        app(&mut harness).review_open(
            SCENARIO,
            Some(("  cleanup {", &format!("{lines}  cleanup {{"))),
        );
        settle(&mut harness);
        let after = positions(&harness, &id);
        for (handle, at) in &before {
            assert_eq!(after.get(handle), Some(at), "{handle} stays where it was");
        }
        for line in added {
            let handle = line.split(' ').next().unwrap();
            assert!(after.contains_key(handle), "{handle} is on the board");
        }
        assert_cards_disjoint(&cards_on_screen(&harness, &id));
    }

    // A scenario with positions, the demo shop's own, opens where its
    // diagram put it.
    let shop = crate::testing::shop_project();
    let (harness, id) = open_at(shop.path(), WIDE);
    let demo = apiforge_core::demos::shop_project();
    let saved = &demo.scenarios[0].graph;
    let graph = main_graph(&harness, &id);
    assert_eq!(graph.nodes.len(), saved.nodes.len());
    for node in &graph.nodes {
        let saved = saved
            .nodes
            .iter()
            .find(|candidate| candidate.handle == node.handle)
            .unwrap_or_else(|| panic!("{} is in the demo", node.handle));
        assert_eq!(
            node.position, saved.position,
            "{} kept its place",
            node.handle
        );
    }
}

/// Criterion 5: a chip left far below the board, as an old stacked layout
/// leaves it, moves beside its first reader with *Tidy up*, and *Fit the
/// view* then fits the cards within 10 %.
#[test]
fn lay02_criterion_5_tidy_up_brings_a_chip_to_its_first_reader() {
    let (_project, mut harness, id) = open("shop-30", WIDE);
    let graph = main_graph(&harness, &id);
    let first = graph
        .nodes
        .iter()
        .find(|node| node.handle == "first")
        .expect("the variable first")
        .id
        .clone();
    let bottom = graph
        .nodes
        .iter()
        .map(|node| node.position.y)
        .fold(0.0, f64::max);
    app(&mut harness).active_board_command(Command::AddProxy {
        source: ProxySource::Variable {
            node_id: first.clone(),
        },
        position: Position {
            x: 0.0,
            y: bottom + 2000.0,
        },
    });
    settle(&mut harness);
    assert_eq!(
        main_graph(&harness, &id).proxies.len(),
        1,
        "the chip is placed"
    );

    // The chip is selected once placed; Tidy up then lays out the board as a
    // whole, with nothing selected.
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    let graph = main_graph(&harness, &id);
    let project = folder(&harness).context_project();
    let reads_first = |binding: &Binding| {
        let mut reads = false;
        binding.for_each_leaf(&mut |leaf, _| {
            reads |= matches!(leaf, Binding::Reference { node_id, .. } if *node_id == first);
        });
        reads
    };
    let reader = graph
        .nodes
        .iter()
        .find(|node| node.inputs.values().any(reads_first))
        .expect("first has a reader");
    assert_eq!(
        reader.handle, "product",
        "the first reader in document order"
    );
    let chip = Chip::build(&graph.proxies[0], &graph, &project, &[]).rect();
    let density = folder(&harness).density();
    let card = |node: &Node| Card::build(node, &graph, &project, density).rect();
    let beside = separation(chip, card(reader));
    assert!(
        beside <= 96.0,
        "the chip at {chip:?} is {beside} px from product at {:?}",
        card(reader)
    );
    for node in &graph.nodes {
        assert!(
            !chip.intersects(card(node)),
            "the chip overlaps {}",
            node.handle
        );
    }

    choose(&mut harness, FIT);
    let board = canvas(&harness, &id);
    let zoom = board.debug_zoom();
    let focus = board.debug_focus();
    let cards = board
        .debug_cards()
        .iter()
        .map(|(_, rect)| *rect)
        .reduce(egui::Rect::union)
        .expect("cards on the board");
    let cards_only = (zoom * focus.width() / cards.width())
        .min(zoom * focus.height() / cards.height())
        .clamp(0.25, 1.0);
    assert!(
        zoom >= 0.9 * cards_only,
        "Fit shows the cards at {zoom}, the cards alone would fit at {cards_only}"
    );
}

/// Criterion 5 with two chips: Tidy up of a selection takes along the chip
/// of a reader it moves and leaves the chip of a reader it does not move
/// exactly where it was, the two kept clear of each other and of every card.
#[test]
fn lay02_a_chip_whose_reader_stays_stays_where_it_was() {
    let (_project, mut harness, id) = open("shop-30", WIDE);
    let graph = main_graph(&harness, &id);
    let node = |handle: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.handle == handle)
            .unwrap_or_else(|| panic!("a step {handle}"))
            .clone()
    };
    let bottom = graph
        .nodes
        .iter()
        .map(|node| node.position.y)
        .fold(0.0, f64::max);
    for (variable, y) in [("second", bottom + 2000.0), ("first", bottom + 2200.0)] {
        app(&mut harness).active_board_command(Command::AddProxy {
            source: ProxySource::Variable {
                node_id: node(variable).id,
            },
            position: Position { x: 0.0, y },
        });
        settle(&mut harness);
    }
    // `product`, the first reader of `first`, is dragged away; Tidy up of it
    // and `promoted` brings it back. Nothing selected reads `second`.
    let product = node("product");
    app(&mut harness).active_board_command(Command::Move(vec![(
        product.id.clone(),
        Position {
            x: product.position.x + 3000.0,
            y: product.position.y + 3000.0,
        },
    )]));
    settle(&mut harness);
    let before = main_graph(&harness, &id);
    assert_eq!(before.proxies.len(), 2, "both chips are placed");
    let stays = before.proxies[0].clone();
    app(&mut harness)
        .selections
        .insert(id.clone(), vec![product.id.clone(), node("promoted").id]);
    settle(&mut harness);
    choose(&mut harness, TIDY);

    let graph = main_graph(&harness, &id);
    let project = folder(&harness).context_project();
    let density = folder(&harness).density();
    let moved = graph
        .nodes
        .iter()
        .find(|candidate| candidate.id == product.id)
        .unwrap();
    assert_ne!(
        moved.position,
        before
            .nodes
            .iter()
            .find(|candidate| candidate.id == product.id)
            .unwrap()
            .position,
        "Tidy up moved product"
    );
    let unmoved = graph
        .proxies
        .iter()
        .find(|proxy| proxy.id == stays.id)
        .unwrap();
    assert_eq!(unmoved.position, stays.position, "the chip of second stays");
    let chips: Vec<egui::Rect> = graph
        .proxies
        .iter()
        .map(|proxy| Chip::build(proxy, &graph, &project, &[]).rect())
        .collect();
    let follows = graph
        .proxies
        .iter()
        .position(|proxy| proxy.id != stays.id)
        .unwrap();
    let card = |node: &Node| Card::build(node, &graph, &project, density).rect();
    let beside = separation(chips[follows], card(moved));
    assert!(
        beside <= 96.0,
        "the chip of first is {beside} px from product"
    );
    assert!(
        separation(chips[0], chips[1]) >= 11.5,
        "the chips keep clear of each other: {chips:?}"
    );
    for chip in &chips {
        for node in &graph.nodes {
            assert!(
                !chip.intersects(card(node)),
                "a chip overlaps {}",
                node.handle
            );
        }
    }
}

/// Criterion 6: the scenario is laid out at the sizes the board draws its
/// cards at, so *Tidy up* has nothing to move, and the first frame shows it
/// fitted: a card inside the board and Fit's own zoom.
#[test]
fn lay02_criterion_6_opened_at_the_boards_density_and_in_view() {
    for name in ["order-flow", "shop-30"] {
        for size in [WIDE, MINIMUM] {
            let (_project, mut harness, id) = open(name, size);
            let board = canvas(&harness, &id);
            let focus = board.debug_focus();
            assert!(
                board
                    .debug_cards()
                    .iter()
                    .any(|(_, rect)| focus.contains_rect(*rect)),
                "{name} at {size:?}: no card stands whole in the board {focus:?}"
            );
            let first_zoom = board.debug_zoom();
            choose(&mut harness, FIT);
            let fitted = canvas(&harness, &id).debug_zoom();
            assert!(
                (fitted - first_zoom).abs() < 1e-3,
                "{name} at {size:?}: the first frame at {first_zoom}, Fit at {fitted}"
            );

            let opened = positions(&harness, &id);
            app(&mut harness).active_board_command(Command::TidyUp(Vec::new()));
            settle(&mut harness);
            let tidied = positions(&harness, &id);
            for (handle, at) in &opened {
                let moved = tidied[handle];
                assert!(
                    (moved.0 - at.0).abs() <= 1 && (moved.1 - at.1).abs() <= 1,
                    "{name}: Tidy up moves {handle} from {at:?} to {moved:?}"
                );
            }
            // In each column the cards keep the row gap as drawn.
            let zoom = canvas(&harness, &id).debug_zoom();
            let mut cards = cards_on_screen(&harness, &id);
            cards.sort_by(|a, b| {
                (a.1.center().x.round(), a.1.top())
                    .partial_cmp(&(b.1.center().x.round(), b.1.top()))
                    .unwrap()
            });
            for pair in cards.windows(2) {
                let (upper, lower) = (pair[0].1, pair[1].1);
                if (upper.center().x - lower.center().x).abs() > 1.0 {
                    continue;
                }
                let gap = (lower.top() - upper.bottom()) / zoom;
                assert!(
                    gap >= ROW_GAP - 1.0,
                    "{name}: {} and {} are {gap} apart as drawn",
                    pair[0].0,
                    pair[1].0
                );
            }
        }
    }
}

/// Criterion 6, the density half: *Compact cards* is on, as by default, so
/// the board draws request cards shorter than the full ones (LAY-16). The
/// layout made on open uses the heights drawn: Tidy up, which packs at the
/// board's density, moves nothing, and a compact card stands closer to the
/// card below it than its full height would allow.
#[test]
fn lay02_criterion_6_opened_at_the_compact_density() {
    let (_project, mut harness, id) = open("shop-30", WIDE);
    assert_eq!(
        folder(&harness).density(),
        crate::canvas::CardDensity::Compact,
        "Compact cards is on by default"
    );
    let graph = main_graph(&harness, &id);
    let project = folder(&harness).context_project();
    let height =
        |node: &Node, density| Card::build(node, &graph, &project, density).rect().height();
    let full = crate::canvas::CardDensity::Full;
    let compact = crate::canvas::CardDensity::Compact;
    let shorter: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| height(node, compact) + 1.0 < height(node, full))
        .collect();
    assert!(!shorter.is_empty(), "the board draws some cards compact");
    // A compact card with another card below it in its column: the gap is
    // the row gap at the compact height, less than the full height leaves.
    let packed = shorter.iter().any(|node| {
        graph.nodes.iter().any(|below| {
            (below.position.x - node.position.x).abs() < 1.0
                && below.position.y > node.position.y
                && ((below.position.y - node.position.y) as f32)
                    < height(node, full) + ROW_GAP - 1.0
        })
    });
    assert!(packed, "the cards are packed at the compact heights");
    let opened = positions(&harness, &id);
    app(&mut harness).active_board_command(Command::TidyUp(Vec::new()));
    settle(&mut harness);
    assert_eq!(
        positions(&harness, &id),
        opened,
        "Tidy up has nothing to move"
    );
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay02_open() {
    for name in ["order-flow", "shop-30"] {
        for (size, label) in [(WIDE, "1600"), (MINIMUM, "960")] {
            let (_project, mut harness, id) = open(name, size);
            let board = canvas(&harness, &id);
            let focus = board.debug_focus();
            assert!(
                board
                    .debug_cards()
                    .iter()
                    .any(|(_, rect)| focus.contains_rect(*rect)),
                "{name} at {label}: a card is in view"
            );
            crate::testing::capture(&mut harness, &format!("lay02-{name}-{label}"), None);
        }
    }
}
