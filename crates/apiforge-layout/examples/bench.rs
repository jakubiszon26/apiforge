//! Measures the operations [ADR 0006](../../../docs/decisions/0006-layout-and-wire-routing.md)
//! gives a budget, on the boards that budget names.
//!
//! Run it in release mode; a debug build is several times slower and says nothing
//! about the budget:
//!
//! ```text
//! cargo run -p apiforge-layout --example bench --release --locked [-- --strict]
//! ```
//!
//! Without `--strict` it prints the measurements and exits successfully, which is
//! what a developer wants on an unknown machine. With `--strict` it fails when a
//! budget is exceeded, for a machine whose speed is known.

use apiforge_layout::{
    Block, Edge, Graph, Point, Port, Rect, Routing, Side, Size, Wire, place, route, smooth, tidy,
};
use std::time::{Duration, Instant};

/// The same small generator the tests use, so the boards are comparable.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            0
        } else {
            (self.next() % limit as u64) as usize
        }
    }

    fn float(&mut self, low: f32, high: f32) -> f32 {
        let unit = (self.next() >> 11) as f32 / (1u64 << 53) as f32;
        low + (high - low) * unit
    }
}

/// How far back a wire reaches for its source.
#[derive(Clone, Copy)]
enum Reach {
    /// A block wired to one of its recent neighbours, as a scenario does it.
    Near,
    /// A block wired to any earlier block, which puts the two ports anywhere on
    /// the board. No budget covers this; it is measured to show the limit.
    Anywhere,
}

/// A board of `cards` cards and about `wires` wires, laid out by `tidy`.
fn board(
    cards: usize,
    wires: usize,
    reach: Reach,
) -> (Vec<Block>, Vec<Edge>, Vec<Point>, Vec<Wire>) {
    let mut random = Random(0x5EED_1234);
    let blocks: Vec<Block> = (0..cards)
        .map(|_| {
            Block::new(
                Size::new(random.float(200.0, 320.0), random.float(100.0, 200.0)),
                Point::new(random.float(0.0, 6000.0), random.float(0.0, 4000.0)),
            )
        })
        .collect();
    let mut edges = Vec::new();
    for target in 1..cards {
        let back = match reach {
            Reach::Near => 1 + random.below(4.min(target)),
            Reach::Anywhere => target - random.below(target),
        };
        edges.push(Edge::new(target - back, target));
    }
    let positions = tidy(&Graph::new(&blocks, &edges));
    let mut routed = Vec::with_capacity(wires);
    for index in 0..wires {
        let (from, to) = match edges.get(index) {
            Some(edge) => (edge.source, edge.target),
            // The extra wires reach further, as a value used far from where it is
            // produced does.
            None => {
                let to = random.below(cards);
                let back = match reach {
                    Reach::Near => 1 + random.below(12),
                    Reach::Anywhere => random.below(cards),
                };
                (to.saturating_sub(back), to)
            }
        };
        if from == to {
            continue;
        }
        routed.push(Wire::new(
            Port::new(from, Side::Right, random.float(10.0, 90.0)),
            Port::new(to, Side::Left, random.float(10.0, 90.0)),
        ));
    }
    (blocks, edges, positions, routed)
}

fn rectangles(positions: &[Point], blocks: &[Block]) -> Vec<Rect> {
    positions
        .iter()
        .zip(blocks)
        .map(|(position, block)| Rect::from_size(*position, block.size))
        .collect()
}

/// Runs `work` a few times and returns the fastest run, which is the measurement
/// least disturbed by other processes.
fn measure(rounds: usize, mut work: impl FnMut()) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..rounds {
        let started = Instant::now();
        work();
        best = best.min(started.elapsed());
    }
    best
}

fn main() {
    let strict = std::env::args().any(|argument| argument == "--strict");
    let routing = Routing::default();
    let mut failures = 0;
    {
        let (blocks, edges, positions, wires) = board(200, 400, Reach::Near);
        let cards = rectangles(&positions, &blocks);
        println!(
            "board of neighbours: {} cards, {} wires{}",
            blocks.len(),
            wires.len(),
            if cfg!(debug_assertions) {
                " (debug build: the budgets do not apply)"
            } else {
                ""
            }
        );
        let mut report = |what: &str, measured: Duration, budget: Duration| {
            let over = measured > budget;
            println!(
                "  {what}: {:.2} ms (budget {:.0} ms){}",
                measured.as_secs_f64() * 1000.0,
                budget.as_secs_f64() * 1000.0,
                if over { "  OVER" } else { "" }
            );
            if over {
                failures += 1;
            }
        };

        let graph = Graph::new(&blocks, &edges);
        report(
            "layered layout",
            measure(5, || {
                std::hint::black_box(tidy(&graph));
            }),
            Duration::from_millis(20),
        );
        report(
            "full routing",
            measure(5, || {
                std::hint::black_box(route(&cards, &wires, &routing));
            }),
            Duration::from_millis(30),
        );

        // A drag reroutes the wires of the moved card only, which is the frame
        // budget.
        let dragged = 7usize;
        let moving: Vec<Wire> = wires
            .iter()
            .copied()
            .filter(|wire| wire.from.block == dragged || wire.to.block == dragged)
            .collect();
        let mut shifted = cards.clone();
        report(
            &format!("reroute while dragging ({} wires)", moving.len()),
            measure(20, || {
                shifted[dragged] = cards[dragged].inflate(1.0);
                std::hint::black_box(route(&shifted, &moving, &routing));
            }),
            Duration::from_millis(8),
        );

        let routes = route(&cards, &wires, &routing);
        report(
            "smoothing every route",
            measure(10, || {
                for routed in &routes {
                    std::hint::black_box(smooth(routed, apiforge_layout::DEFAULT_RADIUS, &cards));
                }
            }),
            Duration::from_millis(8),
        );
        let fresh: Vec<usize> = (0..8).collect();
        report(
            "placing 8 new cards",
            measure(10, || {
                std::hint::black_box(place(&graph, &fresh));
            }),
            Duration::from_millis(8),
        );
    }

    // The same board wired at random instead of between neighbours. Each wire then
    // spans the board, so every search covers most of it. No budget applies; the
    // number is here so the cost of that shape is known rather than assumed.
    {
        let (blocks, _, positions, wires) = board(200, 400, Reach::Anywhere);
        let cards = rectangles(&positions, &blocks);
        let measured = measure(3, || {
            std::hint::black_box(route(&cards, &wires, &routing));
        });
        println!(
            "board wired at random: full routing {:.0} ms (no budget)",
            measured.as_secs_f64() * 1000.0
        );
    }

    if failures > 0 && strict {
        eprintln!("{failures} budget(s) exceeded");
        std::process::exit(1);
    }
}
