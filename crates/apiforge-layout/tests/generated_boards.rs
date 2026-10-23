//! The guarantees of [ADR 0006](../../../docs/decisions/0006-layout-and-wire-routing.md)
//! on generated boards rather than on hand-picked geometry.
//!
//! Boards come from a seeded generator, so a failure is reproducible from the seed
//! printed in the assertion. The generator makes the awkward cases on purpose:
//! cards of mixed sizes, backward and long wires, ports on every side, dense boards
//! where channels are scarce, and boards where cards overlap.

use apiforge_layout::{
    Block, Edge, Graph, Piece, Point, Port, Rect, Route, Routing, Side, Size, Wire, any_overlap,
    place, route, smooth, tidy, violations,
};

/// A small deterministic generator; the layout crate carries no dependencies.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        // xorshift64*, enough for generating geometry.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            return 0;
        }
        (self.next() % limit as u64) as usize
    }

    fn float(&mut self, low: f32, high: f32) -> f32 {
        let unit = (self.next() >> 11) as f32 / (1u64 << 53) as f32;
        low + (high - low) * unit
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

struct Board {
    blocks: Vec<Block>,
    edges: Vec<Edge>,
}

/// A board of `count` cards with a plausible mix of wires.
fn board(seed: u64, count: usize, dense: bool) -> Board {
    let mut random = Random(seed | 1);
    let spread = if dense { 1.0 } else { 2.5 };
    let blocks: Vec<Block> = (0..count)
        .map(|_| {
            let size = Size::new(random.float(160.0, 360.0), random.float(80.0, 220.0));
            let position = Point::new(
                random.float(0.0, 400.0 * spread * (count as f32).sqrt()),
                random.float(0.0, 260.0 * spread * (count as f32).sqrt()),
            );
            let block = Block::new(size, position);
            if random.chance(25) {
                block.pinned()
            } else {
                block
            }
        })
        .collect();
    let mut edges = Vec::new();
    for target in 1..count {
        // Most cards depend on an earlier one, which keeps the graph acyclic and
        // layered; some depend on two, and some on none.
        if random.chance(85) {
            edges.push(
                Edge::new(random.below(target), target)
                    .with_offsets(random.float(0.0, 100.0), random.float(0.0, 100.0)),
            );
        }
        if random.chance(30) && target > 1 {
            edges.push(Edge::new(random.below(target), target));
        }
    }
    Board { blocks, edges }
}

/// Wires for a laid-out board: one per edge, on the sides the board uses, plus a
/// few backward wires that have to travel against the flow.
fn wires(board: &Board, blocks: &[Block], seed: u64) -> Vec<Wire> {
    let mut random = Random(seed | 3);
    let port = |block: usize, side: Side, size: Size, random: &mut Random| {
        let along = if side.is_horizontal() {
            random.float(8.0, (size.height - 8.0).max(8.0))
        } else {
            random.float(8.0, (size.width - 8.0).max(8.0))
        };
        Port::new(block, side, along)
    };
    let mut wires: Vec<Wire> = board
        .edges
        .iter()
        .map(|edge| {
            // Data wires leave right and arrive left; ordering wires go downwards.
            let order = random.chance(30);
            let (from_side, to_side) = if order {
                (Side::Bottom, Side::Top)
            } else {
                (Side::Right, Side::Left)
            };
            Wire::new(
                port(
                    edge.source,
                    from_side,
                    blocks[edge.source].size,
                    &mut random,
                ),
                port(edge.target, to_side, blocks[edge.target].size, &mut random),
            )
        })
        .collect();
    for _ in 0..(blocks.len() / 8) {
        let (from, to) = (random.below(blocks.len()), random.below(blocks.len()));
        if from != to {
            wires.push(Wire::new(
                port(from, Side::Right, blocks[from].size, &mut random),
                port(to, Side::Left, blocks[to].size, &mut random),
            ));
        }
    }
    wires
}

/// The same board with nothing pinned, which is what a whole-board tidy up gets.
fn unpinned(board: &Board) -> Vec<Block> {
    board
        .blocks
        .iter()
        .map(|block| Block::new(block.size, block.position))
        .collect()
}

fn rectangles(positions: &[Point], blocks: &[Block]) -> Vec<Rect> {
    positions
        .iter()
        .zip(blocks)
        .map(|(position, block)| Rect::from_size(*position, block.size))
        .collect()
}

/// Whether every segment of a route is axis-aligned and the ends sit on the ports.
fn assert_orthogonal_and_anchored(routes: &[Route], wires: &[Wire], cards: &[Rect], seed: u64) {
    for (index, routed) in routes.iter().enumerate() {
        if routed.points.is_empty() {
            continue;
        }
        let wire = wires[index];
        assert_eq!(
            routed.points.first(),
            Some(&wire.from.point_on(&cards[wire.from.block])),
            "seed {seed}: wire {index} does not start on its port"
        );
        assert_eq!(
            routed.points.last(),
            Some(&wire.to.point_on(&cards[wire.to.block])),
            "seed {seed}: wire {index} does not end on its port"
        );
        for (from, to) in routed.segments() {
            let horizontal = (from.y - to.y).abs() < 0.01;
            let vertical = (from.x - to.x).abs() < 0.01;
            assert!(
                horizontal || vertical,
                "seed {seed}: wire {index} has a diagonal segment {from:?} -> {to:?}"
            );
        }
    }
}

#[test]
fn laid_out_boards_have_no_overlapping_cards_and_no_route_through_a_card() {
    let routing = Routing::default();
    for seed in 1..=60u64 {
        let count = 3 + (seed as usize * 7) % 18;
        let dense = seed % 3 == 0;
        let board = board(seed, count, dense);
        let blocks = unpinned(&board);
        let graph = Graph::new(&blocks, &board.edges);
        let positions = tidy(&graph);
        assert!(
            !any_overlap(&positions, &blocks),
            "seed {seed}: tidy up left cards overlapping"
        );
        let cards = rectangles(&positions, &blocks);
        let wires = wires(&board, &board.blocks, seed);
        let routes = route(&cards, &wires, &routing);
        assert_orthogonal_and_anchored(&routes, &wires, &cards, seed);
        assert_eq!(
            violations(&routes, &wires, &cards),
            [],
            "seed {seed}: a final route reaches into a card that is not its own"
        );
        // A tidied board has room everywhere, so every wire gets a real route and
        // none falls back to the plain corner of an enclosed port.
        let incomplete = routes.iter().filter(|route| !route.complete).count();
        assert_eq!(
            incomplete, 0,
            "seed {seed}: {incomplete} wires found no route on a tidied board"
        );
    }
}

/// Cards the user has dragged on top of each other are the one documented
/// exception: a port inside another card may be left through its stub. Nothing
/// else may cross a card, and nothing may panic.
#[test]
fn boards_with_overlapping_cards_keep_the_documented_exception() {
    let routing = Routing::default();
    for seed in 1..=40u64 {
        let count = 4 + (seed as usize * 5) % 10;
        let mut board = board(seed, count, true);
        // Pile several cards onto one spot.
        for index in (0..count).step_by(3) {
            board.blocks[index].position = Point::new(300.0 + index as f32 * 8.0, 200.0);
        }
        let positions: Vec<Point> = board.blocks.iter().map(|block| block.position).collect();
        let cards = rectangles(&positions, &board.blocks);
        let wires = wires(&board, &board.blocks, seed);
        let routes = route(&cards, &wires, &routing);
        assert_orthogonal_and_anchored(&routes, &wires, &cards, seed);
        assert_eq!(
            violations(&routes, &wires, &cards),
            [],
            "seed {seed}: a route left an overlapping card by more than its stub"
        );
    }
}

#[test]
fn smoothing_a_generated_board_keeps_every_route_off_the_cards() {
    let routing = Routing::default();
    for seed in 1..=40u64 {
        let count = 4 + (seed as usize * 3) % 14;
        let board = board(seed, count, seed % 2 == 0);
        let blocks = unpinned(&board);
        let graph = Graph::new(&blocks, &board.edges);
        let positions = tidy(&graph);
        let cards = rectangles(&positions, &blocks);
        let wires = wires(&board, &blocks, seed);
        let routes = route(&cards, &wires, &routing);
        for (index, routed) in routes.iter().enumerate() {
            let wire = wires[index];
            let obstacles: Vec<Rect> = cards
                .iter()
                .enumerate()
                .filter(|(card, _)| *card != wire.from.block && *card != wire.to.block)
                .map(|(_, card)| card.inflate(routing.margin))
                .collect();
            let pieces = smooth(routed, apiforge_layout::DEFAULT_RADIUS, &obstacles);
            let touched: Vec<usize> = obstacles
                .iter()
                .enumerate()
                .filter(|(_, obstacle)| {
                    pieces
                        .iter()
                        .any(|piece| obstacle.overlaps(&piece.bounds()))
                })
                .map(|(card, _)| card)
                .collect();
            assert_eq!(
                touched,
                Vec::<usize>::new(),
                "seed {seed}: smoothed wire {index} reaches into a card"
            );
            // Smoothing keeps the ends and joins without gaps.
            if let (Some(first), Some(last)) = (pieces.first(), pieces.last()) {
                assert!(first.from().distance(routed.points[0]) < 0.01);
                assert!(last.to().distance(routed.points[routed.points.len() - 1]) < 0.01);
            }
            for pair in pieces.windows(2) {
                assert!(
                    pair[0].to().distance(pair[1].from()) < 0.01,
                    "seed {seed}: smoothed wire {index} has a gap"
                );
            }
        }
    }
}

#[test]
fn a_bridge_sits_on_its_own_wire_and_only_where_wires_cross() {
    let routing = Routing::default();
    for seed in 1..=40u64 {
        let count = 5 + (seed as usize * 11) % 12;
        let board = board(seed, count, true);
        let blocks = unpinned(&board);
        let graph = Graph::new(&blocks, &board.edges);
        let positions = tidy(&graph);
        let cards = rectangles(&positions, &blocks);
        let wires = wires(&board, &blocks, seed);
        let routes = route(&cards, &wires, &routing);
        for (index, routed) in routes.iter().enumerate() {
            for bridge in &routed.bridges {
                let on_own_wire = routed.segments().any(|(from, to)| {
                    let within = |value: f32, a: f32, b: f32| {
                        value >= a.min(b) - 0.02 && value <= a.max(b) + 0.02
                    };
                    within(bridge.at.x, from.x, to.x) && within(bridge.at.y, from.y, to.y)
                });
                assert!(
                    on_own_wire,
                    "seed {seed}: bridge {bridge:?} of wire {index} is not on that wire"
                );
                let crossed = routes.iter().enumerate().any(|(other, candidate)| {
                    other != index
                        && candidate.segments().any(|(from, to)| {
                            let within = |value: f32, a: f32, b: f32| {
                                value >= a.min(b) - 0.02 && value <= a.max(b) + 0.02
                            };
                            within(bridge.at.x, from.x, to.x) && within(bridge.at.y, from.y, to.y)
                        })
                });
                assert!(
                    crossed,
                    "seed {seed}: bridge {bridge:?} of wire {index} crosses nothing"
                );
            }
        }
        // Every crossing is drawn exactly once, by one of the two wires.
        let mut crossings = 0;
        for (index, left) in routes.iter().enumerate() {
            for right in &routes[index + 1..] {
                for (a1, a2) in left.segments() {
                    for (b1, b2) in right.segments() {
                        if apiforge_layout::crossing(a1, a2, b1, b2).is_some() {
                            crossings += 1;
                        }
                    }
                }
            }
        }
        let drawn: usize = routes.iter().map(|route| route.bridges.len()).sum();
        assert_eq!(
            drawn, crossings,
            "seed {seed}: {drawn} bridges for {crossings} crossings"
        );
    }
}

#[test]
fn layout_and_routing_are_reproducible() {
    let routing = Routing::default();
    for seed in 1..=20u64 {
        let board = board(seed, 4 + (seed as usize % 9), seed % 2 == 0);
        let blocks = unpinned(&board);
        let graph = Graph::new(&blocks, &board.edges);
        assert_eq!(tidy(&graph), tidy(&graph), "seed {seed}: tidy up differs");
        let positions = tidy(&graph);
        let cards = rectangles(&positions, &blocks);
        let wires = wires(&board, &blocks, seed);
        assert_eq!(
            route(&cards, &wires, &routing),
            route(&cards, &wires, &routing),
            "seed {seed}: routing differs"
        );
    }
}

#[test]
fn placing_new_cards_never_overlaps_and_never_moves_the_others() {
    for seed in 1..=40u64 {
        let count = 4 + (seed as usize * 13) % 12;
        let board = board(seed, count, true);
        let graph = Graph::new(&board.blocks, &board.edges);
        let fresh: Vec<usize> = (0..count).filter(|index| index % 4 == 3).collect();
        if fresh.is_empty() {
            continue;
        }
        let placed = place(&graph, &fresh);
        let mut positions: Vec<Point> = board.blocks.iter().map(|block| block.position).collect();
        for (index, position) in fresh.iter().zip(&placed) {
            positions[*index] = *position;
        }
        // The cards that were already there are untouched.
        for (index, block) in board.blocks.iter().enumerate() {
            if !fresh.contains(&index) {
                assert_eq!(
                    positions[index], block.position,
                    "seed {seed}: card {index} moved"
                );
            }
        }
        // A new card overlaps nothing.
        let cards = rectangles(&positions, &board.blocks);
        for &index in &fresh {
            for (other, card) in cards.iter().enumerate() {
                if other != index && !fresh.contains(&other) {
                    assert!(
                        !cards[index].overlaps(card),
                        "seed {seed}: new card {index} overlaps {other}"
                    );
                }
            }
        }
    }
}

/// A board of one card and no wires, and a board of wires with no cards, are the
/// shapes a fresh scenario and a half-applied edit produce.
#[test]
fn degenerate_boards_are_handled_without_a_panic() {
    let routing = Routing::default();
    let single = [Block::new(Size::new(280.0, 120.0), Point::new(10.0, 10.0))];
    assert_eq!(tidy(&Graph::new(&single, &[])).len(), 1);
    assert!(route(&[], &[], &routing).is_empty());
    let cards = [Rect::from_size(
        Point::new(0.0, 0.0),
        Size::new(280.0, 120.0),
    )];
    let self_wire = [Wire::new(
        Port::new(0, Side::Right, 60.0),
        Port::new(0, Side::Left, 60.0),
    )];
    let routes = route(&cards, &self_wire, &routing);
    assert_eq!(routes.len(), 1);
    // A card with zero size still routes rather than dividing by its height.
    let flat = [Rect::from_size(Point::new(0.0, 0.0), Size::new(0.0, 0.0))];
    let flat_wire = [Wire::new(
        Port::new(0, Side::Right, 0.0),
        Port::new(0, Side::Left, 0.0),
    )];
    assert_eq!(route(&flat, &flat_wire, &routing).len(), 1);
    assert!(smooth(&Route::default(), 24.0, &[]).is_empty());
    let _: Vec<Piece> = smooth(&routes[0], 24.0, &[]);
}

/// Tidying a selection: everything outside it is pinned, so it must stay exactly
/// where it is and the laid-out cards must not land on it.
#[test]
fn tidying_a_selection_keeps_off_the_cards_outside_it() {
    for seed in 1..=40u64 {
        let count = 5 + (seed as usize * 7) % 12;
        let mut board = board(seed, count, true);
        // Spread the pinned cards out, as a board the user has arranged.
        for (index, block) in board.blocks.iter_mut().enumerate() {
            block.pinned = index % 3 == 0;
            if block.pinned {
                block.position = Point::new(index as f32 * 520.0, (index % 4) as f32 * 300.0);
            }
        }
        let graph = Graph::new(&board.blocks, &board.edges);
        let positions = tidy(&graph);
        let cards = rectangles(&positions, &board.blocks);
        for (index, block) in board.blocks.iter().enumerate() {
            if block.pinned {
                assert_eq!(
                    positions[index], block.position,
                    "seed {seed}: pinned card {index} moved"
                );
            }
        }
        for (index, card) in cards.iter().enumerate() {
            for (other, pinned) in cards.iter().enumerate() {
                if other != index && board.blocks[other].pinned {
                    assert!(
                        !card.overlaps(pinned),
                        "seed {seed}: card {index} landed on pinned card {other}"
                    );
                }
            }
        }
    }
}
