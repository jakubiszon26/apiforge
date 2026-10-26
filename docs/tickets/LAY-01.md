# LAY-01: A quality measurement for layout and routing

- Size: S · Owner review: 10 min · Risk: none
- State: Backlog (issue #32)
- Analysis model: Fable · Implementation model: —

## Goal

Before and after every layout ticket, one command says how readable the
boards are: how many crossings, bends, straight wires, and wires too close to
or on top of each other. The numbers go into the tickets as evidence.

## What the owner will see

- New: nothing on screen. A table in each layout ticket's QA report.
- Changed, removed, migrated, defaults: nothing.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `cargo run -p apiforge-layout --example quality --release` prints, for
   generated scenario-like boards of 6, 12, 25 and 50 cards with and without an
   ordering chain: wires, crossings, bends, straight wires, close pairs
   (parallel segments of different wires within 8 px for ≥ 20 px), coincident
   pairs (within 0.5 px), long edges (spanning more than one layer) and
   incomplete routes.
2. The boards are deterministic: two runs print the same table.
3. `--dump <seed> <cards>` prints one board's rectangles and routes, for
   reading a case by hand.
4. The development guide names the command beside the benchmark.

## Not in scope

Thresholds that fail CI; they come when the numbers have settled (LAY-05).

## Technical appendix

Start from [`docs/qa/2026-10-26/LAY/measure.rs`](../qa/2026-10-26/LAY/measure.rs),
which produced the table in [LAY-00](LAY-00.md); move it to
`crates/apiforge-layout/examples/quality.rs`, share the random generator with
`examples/bench.rs`, and use the card geometry of the desktop (280 px wide,
header 64, input row 52, output row 28, footer 36, control 28) so the boards
look like the application's.
