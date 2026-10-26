# LAY-03: Wires are never drawn on top of each other

- Size: S · Owner review: 15 min · Risk: none
- State: Backlog (issue #34)
- Analysis model: Fable · Implementation model: —

## Goal

Two wires that travel the same way run side by side with a visible gap, and
a port that feeds several cards shows one wire per card.

## What the owner will see

- New: parallel wires are spaced apart; the wires of one output separate
  after the port stub
  (today: [coincident bundles](../qa/2026-10-26/LAY/crop-flow-left.png),
  [a shared trunk](../qa/2026-10-26/LAY/crop-demo.png)).
- Changed: a nudged wire may run inside the 12 px clearance of a card, never
  inside the card.
- Removed, migrated, defaults: nothing.

## Mock-up

Not applicable: no new element; the wires keep their style.

## Acceptance criteria

1. Generated boards (LAY-01): no coincident pair where the gap between the
   cards leaves room for the channel gap times the number of wires.
2. Nudging never adds a crossing: the bridge count after nudging is not above
   the count before it.
3. The guarantee tests of ADR 0006 still pass: no nudged segment enters a card.
4. The demo board tidied: the two wires from *Product* are two visible lines
   from the stub onwards.

## Not in scope

Lane order between columns (LAY-05); a deliberate bus style for fan-outs.

## Technical appendix

`route::nudge` (`crates/apiforge-layout/src/route.rs`): channels are keyed by
the rounded line, so lines 3 px apart are two channels; the order across a
channel sorts by the segment's start along the channel (`point.x` for a
horizontal one) instead of by where the neighbouring segments come from; and
`safe` tests the offset segment against cards inflated by the margin, so a wire
hugging a clearance can never move towards the card and keeps its line. Fix:
cluster lines within `channel_gap`, sort by the mean perpendicular coordinate
of the previous and next points, test against the cards without margin, and
spread outwards from a card instead of symmetrically when one side is blocked.
