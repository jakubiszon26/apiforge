# LAY-01: A quality measurement for layout and routing

- Size: S · Owner review: 10 min · Risk: none
- State: Backlog (issue #32) · LAY sprint, wave 0 (may start now, no
  decision) · re-audited in [lay.md](../sprints/lay.md) section 8 on
  2026-11-04/05: the criteria 5–7 below were added there
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
5. Added (LAY plan, 2026-11-05): `fixtures/scenarios/lay/` holds the three
   reference scenarios `order-flow` (14 steps), `shop-30` (exactly 30 steps,
   with one shared check and one used gate; round 3's grew to 34 and is
   restored) and `shop-50`; all three parse and simulate, deterministically.
6. Added: the command also runs on the three fixtures and prints board
   width and height and the fit zoom at 1600 × 1000 and 960 × 540, card and
   chip collisions, and the 12 px clearance violations (LAY-14's metric).
7. Added: rounds 2 and 3 of the LAY-00 mock-ups are re-measured on the
   restored `shop-30` and the numbers recorded in lay.md 4.6.

## Not in scope

Thresholds that fail CI; they come when the numbers have settled (LAY-05).

## Technical appendix

Start from [`docs/qa/2026-10-26/LAY/measure.rs`](../qa/2026-10-26/LAY/measure.rs),
which produced the table in [LAY-00](LAY-00.md); move it to
`crates/apiforge-layout/examples/quality.rs`, share the random generator with
`examples/bench.rs`, and use the card geometry of the desktop (280 px wide,
header 64, input row 52, output row 28, footer 36, control 28) so the boards
look like the application's.
