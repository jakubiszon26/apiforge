# LAY-02: A scenario opened from code is placed in layers

- Size: S–M · Owner review: 15 min · Risk: `default` (a scenario without
  positions is laid out on open, Q10 = A; the cards and wires do not change)
- State: Backlog (issue #33) · decided by the owner on 2026-11-05 (Q10 = A:
  a scenario without positions is laid out when opened and the positions
  kept until the next save writes `diagram.json`; steps added in code keep
  today's incremental placement; chips placed beside their first reader and
  moved by Tidy up — P8 of [lay.md](../sprints/lay.md)) · LAY sprint, part
  A, the layout track's first ticket
  ([review of 2026-11-10](../sprints/lay.md#11-mid-sprint-review-2026-11-10)),
  risk `default` added by that decision
- Analysis model: Fable · Implementation model: Opus

## Goal

Opening a scenario that has no diagram yet shows it laid out left to right,
not as one column of touching cards.

## What the owner will see

- New: a scenario file without a `diagram.json` opens with its steps in
  columns, as *Tidy up* would place them
  (today: [one stack](../qa/2026-10-26/LAY/layout-flow-placed.png)).
- Changed: nothing for scenarios that already have positions; a single block
  added in code still lands beside its neighbours as before.
- Default behaviour that changes (Q10 = A): a scenario without positions is
  laid out when opened; the positions are kept until the next save writes
  `diagram.json`, as any edit would. Variable chips are placed beside their
  first reader and moved by Tidy up (P8). Removed, migrated: nothing.

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
4. Added (Q10 = A): the positions of an opened scenario are not written
   until the next save; a second open before any save gives the same
   positions; a code edit adding one or three steps keeps today's
   incremental placement, no re-layout. Test:
   `lay02_criterion_4_positions_kept_until_save_and_edits_stay_incremental`.
5. Added (P8): a variable chip is placed beside its first reader on open and
   moves with Tidy up; Fit view is within 10 % of the cards' box on
   `shop-30.afs`. Test: `lay02_criterion_5_chips_beside_their_first_reader`.

6. Added 2026-11-10: the scenario is laid out at the board's density (the
   card heights the board draws, LAY-16), and the first frame shows it
   fitted: at least one card is inside the viewport and Fit's zoom is the
   board's. Test: `lay02_criterion_6_opened_at_the_boards_density_and_in_view`.

Tests: criteria 1 and 2 in `crates/apiforge-layout/src/layered.rs`
(`place_with_every_block_fresh_gives_the_columns_of_tidy`,
`place_with_one_fresh_block_is_unchanged`); criteria 3 to 6 in
`crates/apiforge-desktop/src/folder/acceptance_tests/lay02.rs`
(`lay02_criterion_3_order_flow_opens_in_layers`, and as named above), on
the LAY-01 fixtures under `fixtures/scenarios/lay/`.

## Regression potential

A scenario that has positions must open exactly where it was: the test of
criterion 4 opens one with a saved `diagram.json` and compares every
position. UX-34's paste and LAY-16's placement of a new block (full density
until the next Tidy up) keep their tests.

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
