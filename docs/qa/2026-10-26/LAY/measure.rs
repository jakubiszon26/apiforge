//! Throwaway measurement for the layout analysis: never committed.
use apiforge_layout::{Block, Edge, Graph, Point, Port, Rect, Routing, Side, Size, Wire, route, tidy, layers_of};

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
    fn below(&mut self, limit: usize) -> usize { if limit == 0 { 0 } else { (self.next() % limit as u64) as usize } }
    fn chance(&mut self, percent: u64) -> bool { self.next() % 100 < percent }
}

const WIDTH: f32 = 280.0;
const HEADER: f32 = 64.0;
const INPUT_ROW: f32 = 52.0;
const OUTPUT_ROW: f32 = 28.0;
const FOOTER: f32 = 36.0;
const CONTROL: f32 = 28.0;

struct Card { inputs: usize, outputs: usize }
impl Card {
    fn height(&self) -> f32 { HEADER + self.inputs as f32 * INPUT_ROW + self.outputs as f32 * OUTPUT_ROW + FOOTER + CONTROL }
    fn input_y(&self, i: usize) -> f32 { HEADER + i as f32 * INPUT_ROW + 30.0 }
    fn output_y(&self, i: usize) -> f32 { HEADER + self.inputs as f32 * INPUT_ROW + i as f32 * OUTPUT_ROW + OUTPUT_ROW / 2.0 + 2.0 }
}

/// A scenario-like board: a chain of ordering edges with a few branches, and data
/// wires from recent predecessors into the inputs.
fn scenario(seed: u64, count: usize) -> (Vec<Card>, Vec<Block>, Vec<Edge>, Vec<Wire>) {
    let mut random = Random(seed | 1);
    let cards: Vec<Card> = (0..count).map(|_| Card { inputs: 1 + random.below(3), outputs: 1 + random.below(2) }).collect();
    let blocks: Vec<Block> = cards.iter().enumerate().map(|(i, card)| Block::new(Size::new(WIDTH, card.height()), Point::new(i as f32 * 10.0, i as f32 * 7.0))).collect();
    let mut edges = Vec::new();
    let mut wires = Vec::new();
    for target in 1..count {
        // Ordering: usually from the previous card, sometimes a branch from two back.
        let source = if random.chance(80) || target < 2 { target - 1 } else { target - 2 };
        if std::env::var_os("NO_ORDER").is_none() {
            edges.push(Edge::new(source, target));
            wires.push(Wire::new(Port::new(source, Side::Bottom, WIDTH / 2.0), Port::new(target, Side::Top, WIDTH / 2.0)));
        }
        // Data: each input is fed by one of the last four cards, with 70 % chance.
        for input in 0..cards[target].inputs {
            if !random.chance(70) { continue; }
            let back = 1 + random.below(4.min(target));
            let source = target - back;
            let out = random.below(cards[source].outputs);
            edges.push(Edge::new(source, target).with_offsets(cards[source].output_y(out), cards[target].input_y(input)));
            wires.push(Wire::new(Port::new(source, Side::Right, cards[source].output_y(out)), Port::new(target, Side::Left, cards[target].input_y(input))));
        }
    }
    (cards, blocks, edges, wires)
}

struct Stats { crossings: usize, bends: usize, wires: usize, straight: usize, close: usize, coincident: usize, long_edges: usize, length: f32, incomplete: usize }

fn measure(seed: u64, count: usize, routing: &Routing) -> Stats {
    let (_cards, blocks, edges, wires) = scenario(seed, count);
    let graph = Graph::new(&blocks, &edges);
    let positions = tidy(&graph);
    let layers = layers_of(&graph);
    let long_edges = edges.iter().filter(|e| layers[e.target] > layers[e.source] + 1).count();
    let rects: Vec<Rect> = positions.iter().zip(&blocks).map(|(p, b)| Rect::from_size(*p, b.size)).collect();
    let routes = route(&rects, &wires, routing);
    let crossings: usize = routes.iter().map(|r| r.bridges.len()).sum();
    let bends: usize = routes.iter().map(|r| r.bends()).sum();
    let straight = routes.iter().filter(|r| r.bends() == 0).count();
    let incomplete = routes.iter().filter(|r| !r.complete).count();
    let length: f32 = routes.iter().map(|r| r.length()).sum();
    // Parallel segments of different wires that run within 8 px of each other for
    // at least 20 px; coincident ones are within half a pixel.
    let mut close = 0; let mut coincident = 0;
    for (i, a) in routes.iter().enumerate() {
        for b in &routes[i + 1..] {
            for (a1, a2) in a.segments() {
                for (b1, b2) in b.segments() {
                    let ah = (a1.y - a2.y).abs() < 0.01; let bh = (b1.y - b2.y).abs() < 0.01;
                    if ah != bh { continue; }
                    let (gap, overlap) = if ah {
                        ((a1.y - b1.y).abs(), a1.x.max(a2.x).min(b1.x.max(b2.x)) - a1.x.min(a2.x).max(b1.x.min(b2.x)))
                    } else {
                        ((a1.x - b1.x).abs(), a1.y.max(a2.y).min(b1.y.max(b2.y)) - a1.y.min(a2.y).max(b1.y.min(b2.y)))
                    };
                    if overlap < 20.0 { continue; }
                    if gap < 0.5 { coincident += 1; } else if gap <= 8.0 { close += 1; }
                }
            }
        }
    }
    Stats { crossings, bends, wires: wires.len(), straight, close, coincident, long_edges, length, incomplete }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let gap: f32 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(6.0);
    let crossing_cost: f32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(120.0);
    let routing = Routing { channel_gap: gap, crossing_cost, ..Routing::default() };
    println!("channel_gap={gap} crossing_cost={crossing_cost}");
    println!("{:>5} {:>6} {:>8} {:>6} {:>8} {:>6} {:>6} {:>8} {:>8} {:>6}", "cards", "wires", "crossing", "bends", "straight", "close", "coinc", "longedge", "length", "incompl");
    for count in [6usize, 12, 25, 50] {
        let mut total = Stats { crossings: 0, bends: 0, wires: 0, straight: 0, close: 0, coincident: 0, long_edges: 0, length: 0.0, incomplete: 0 };
        let seeds = 10;
        for seed in 1..=seeds {
            let s = measure(seed, count, &routing);
            total.crossings += s.crossings; total.bends += s.bends; total.wires += s.wires; total.straight += s.straight;
            total.close += s.close; total.coincident += s.coincident; total.long_edges += s.long_edges; total.length += s.length; total.incomplete += s.incomplete;
        }
        let n = seeds as f32;
        println!("{:>5} {:>6.1} {:>8.1} {:>6.1} {:>8.1} {:>6.1} {:>6.1} {:>8.1} {:>8.0} {:>6.1}", count, total.wires as f32 / n, total.crossings as f32 / n, total.bends as f32 / n, total.straight as f32 / n, total.close as f32 / n, total.coincident as f32 / n, total.long_edges as f32 / n, total.length / n, total.incomplete as f32 / n);
    }
    if args.get(3).is_some() {
        // Dump one board for inspection.
        let (_c, blocks, edges, wires) = scenario(3, 12);
        let graph = Graph::new(&blocks, &edges);
        let positions = tidy(&graph);
        let rects: Vec<Rect> = positions.iter().zip(&blocks).map(|(p, b)| Rect::from_size(*p, b.size)).collect();
        for (i, r) in rects.iter().enumerate() { println!("card {i}: {:?}..{:?}", (r.min.x, r.min.y), (r.max.x, r.max.y)); }
        let routes = route(&rects, &wires, &routing);
        for (w, r) in wires.iter().zip(&routes) { println!("wire {}->{} {:?} bends={} bridges={}: {:?}", w.from.block, w.to.block, w.from.side, r.bends(), r.bridges.len(), r.points.iter().map(|p| (p.x.round(), p.y.round())).collect::<Vec<_>>()); }
    }
}
