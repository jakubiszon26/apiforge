# LAY-06: The shape of a tidied scenario — owner decision

- Size: S (analysis and mock-ups only) · Owner review: 30 min plus the
  discussion · Risk: visible UI
- State: Backlog (issue #37) · decision needed
- Analysis model: Fable · Implementation model: —

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

Recommendation: A for scenarios without branches, measured against B on the
demo and `order-flow.afs`; C only if the owner prefers reading a scenario
downwards, which also changes where new blocks land.

- Removed or hidden: nothing.
- Default behaviour that changes: the positions *Tidy up* produces.

## Mock-up

Three captures at 1600 × 1000 and 960 × 540 from a throwaway branch, posted
to the issue; the chosen one is committed under `docs/qa/<date>/LAY-06/`.

## Acceptance criteria

1. The owner has picked an option in the issue, and the choice and its
   reason are recorded here.
2. An implementation ticket exists with this file's option as its mock-up.
3. ADR 0006 is amended if the chosen option changes the flow direction.

## Not in scope

The implementation; data-wire rows (LAY-04).

## Technical appendix

Ordering edges and data edges are the same `Edge` to the layout, and ordering
ports sit on the top and bottom centre of a card (`Card::input_anchor` for
`ORDER_PORT`, `Card::output_anchor` for a control). Option A is a router
change (a preferred rail for wires with vertical ports); B is a coordinate
change in `layered::coordinates`; C swaps the axes of the layered layout and
of the port stubs.
