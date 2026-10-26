# LAY-02: A scenario opened from code is placed in layers

- Size: S · Owner review: 15 min · Risk: none (no visible design changes;
  the first placement of a new scenario improves)
- State: Backlog
- Analysis model: Fable · Implementation model: —

## Goal

Opening a scenario that has no diagram yet shows it laid out left to right,
not as one column of touching cards.

## What the owner will see

- New: a scenario file without a `diagram.json` opens with its steps in
  columns, as *Tidy up* would place them
  (today: [one stack](../qa/2026-10-26/LAY/layout-flow-placed.png)).
- Changed: nothing for scenarios that already have positions; a single block
  added in code still lands beside its neighbours as before.
- Removed, migrated, defaults: nothing.

## Mock-up

Not applicable: the result is the existing *Tidy up* layout; LAY-06 decides
its future shape.

## Acceptance criteria

1. Layout crate: `place` with every block fresh gives the same columns as
   `tidy`, and no two cards overlap or touch (row gap kept).
2. Layout crate: `place` with one fresh block among placed ones behaves as
   today (existing tests unchanged).
3. Folder window: opening the project from
   [`order-flow.afs`](../qa/2026-10-26/LAY/order-flow.afs) without a diagram
   shows one column per layer and `assert_disjoint` holds for every card.

## Not in scope

Which shape a tidied scenario should have (LAY-06).

## Technical appendix

`layered::place` (`crates/apiforge-layout/src/layered.rs`): `start_position`
takes x from `existing_columns`, which is empty when nothing is placed, and y
from neighbours that exclude the other fresh blocks. Fix: when a layer has no
column, take it from the layered coordinates of the fresh blocks (`coordinates`
over the fresh subgraph), and let a fresh block placed earlier in the same call
count as a neighbour. The desktop callers (`folder/placement.rs`,
`app.rs::place_new_graph_blocks`) need no change.
