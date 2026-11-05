# LAY-06: The shape of a tidied scenario — owner decision

- Size: M (implementation; the analysis and mock-ups are done) · Owner
  review: 30 min · Risk: `default` (the positions Tidy up produces)
- State: Backlog (issue #37) · **decided by the owner on 2026-11-05: D
  hybrid** (Q1 = A) with `@after` wires drawn dashed on the rail (Q3 = A),
  in the LAY plan ([lay.md 5.1](../sprints/lay.md#51-decisions-taken-2026-11-05))
  · LAY sprint, wave 2, after LAY-02 and on LAY-16's card heights
  (criterion 2); LAY-17 (the dependency-aware row break) follows it
- Analysis model: Fable · Implementation model: Opus

## Goal

The owner chooses what a tidied scenario looks like, so that ordering wires
stop crossing data wires and a long scenario is not a 3600 px line.

## What the owner will see

Today ([capture](../qa/2026-10-26/LAY/layout-flow-tidied.png)): every step in
its own column; an ordering wire leaves the bottom of a card and enters the
top of the card to its right with four bends, crossing the data wires between
the same two cards by construction; 14 steps fill 3600 px at 31 % zoom.

Options to mock up, each as a capture of `order-flow.afs` tidied:

- **A. Horizontal flow, ordering rail above.** Columns as today; ordering
  wires run along a rail above the cards, never through a gap. No crossings
  with data wires; four bends per ordering wire; the board stays wide.
- **B. Rows that wrap.** After a chosen width (for example four columns) the
  next layer starts a new row below. The board fits a window; the wire that
  wraps is long.
- **C. Vertical flow for ordering.** Steps go top to bottom, so ordering
  wires are straight; data wires go right, down and back left into the next
  step's inputs, with bends where today they are straight.

Recommendation of 2026-10-26, **superseded**: A for scenarios without
branches, measured against B; C only if the owner prefers reading
downwards. The LAY plan added D (the hybrid) after the brief of 2026-11-03,
and the owner chose D on 2026-11-05 (below).

- Removed or hidden: nothing.
- Default behaviour that changes: the positions *Tidy up* produces.

## Mock-up

Three captures at 1600 × 1000 and 960 × 540 from a throwaway branch, posted
to the issue; the chosen one is committed under `docs/qa/<date>/LAY-06/`.

## Decision taken (owner, 2026-11-05)

> Q1 = A: **D hybrid** — a spine of Start, requests, component calls and
> `if` in engine order, satellites (checks, values) hanging under the step
> they read, wrapped after five columns at the default row width, the rail
> for `@after`. Q3 = A: explicit `@after` wires are drawn, dashed, on the
> rail. The second opinion of 2026-11-04 approved D as the direction, not as
> the interface: the heights of the rows, the routing baseline and the
> continuation mark are this ticket's criteria, not the mock-up's.

The shape was chosen on four rounds of mock-ups (`mock/lay-00-shapes`,
`mock/lay-00-shapes-2`, `mock/lay-00-round-3` at `ea981d8`,
`mock/lay-00-round-4` at `df6305b`; `lay00r3-d-shop-30-fit`, `-60`,
`lay00r3-d-shop-50-960-fit`, `lay00r4-d-shop-30-compact-fit`), measured in
[lay.md 4.1–4.8](../sprints/lay.md#4-mock-ups): on `shop-30` D with
LAY-16's compact cards fits 1600 × 1000 at 52 %; B wrapped rows and A
horizontal did not meet the brief; C vertical was not chosen.

## Acceptance criteria (implementation)

Tests in `crates/apiforge-layout/src/layered.rs` (`mod tests`) and
`acceptance_tests.rs`, `mod lay06`, on the LAY-01 reference scenarios with
checks collapsed and expanded.

1. Tidy up on `shop-30.afs` produces D: the spine in engine order left to
   right, each satellite in the column of its latest subject, stacked below
   it; the rows wrap after five columns of 280 px at the default row width
   (a parameter); the board is at most 1920 px wide. Test:
   `lay06_criterion_1_the_spine_and_the_satellites`.
2. A row is no taller than its tallest short stack: a satellite stack is
   the LAY-16 heights of its cards, and a row's height is the maximum over
   its columns, not a fixed value. Test: `lay06_criterion_2_row_heights`.
3. The fit zoom for `order-flow.afs` (14 steps) at 1600 × 1000 is at least
   0.45. Test: `lay06_criterion_3_fit_for_14_steps`.
4. No order wire runs through a gap between cards: `@after` wires and the
   spine's order wires run on the rail above the row, dashed for `@after`.
   Test: `lay06_criterion_4_order_wires_on_the_rail`.
5. The row-end mark means "next step in order": the last card of a row
   carries a mark pointing to the first card of the next row, and the test
   asserts the pair. Test: `lay06_criterion_5_the_continuation_mark`.
6. Deterministic: two Tidy ups of the same scenario are equal. Test:
   `lay06_criterion_6_deterministic`.
7. ADR 0006 is amended: the flow direction stays left to right; the shape
   is D and the rail is named.

## Not in scope

The dependency-aware row break (LAY-17); data-wire rows (LAY-04); nudge and
lanes (LAY-03, LAY-05); the branch bands (LAY-13).

## Technical appendix

Ordering edges and data edges are the same `Edge` to the layout, and ordering
ports sit on the top and bottom centre of a card (`Card::input_anchor` for
`ORDER_PORT`, `Card::output_anchor` for a control). Option A is a router
change (a preferred rail for wires with vertical ports); B is a coordinate
change in `layered::coordinates`; C swaps the axes of the layered layout and
of the port stubs.
