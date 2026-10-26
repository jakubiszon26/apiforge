# LAY-05: Lanes between columns — a short wire does not cross a long one

- Size: M · Owner review: 15 min · Risk: none
- State: Backlog — measured again after LAY-04 before scheduling
- Analysis model: Fable · Implementation model: —

## Goal

In the gap between two columns, wires that enter the next card keep the order
of their ports, so a wire between neighbours does not have to cross a long
wire that arrived first.

## What the owner will see

- New: fewer bridges; long wires take the outer lanes of a gap and short ones
  the inner lanes (today:
  [a long wire beside the card forces the short one across it](../qa/2026-10-26/LAY/crop-flow-right.png)).
- Changed, removed, migrated, defaults: nothing.

## Mock-up

Not applicable.

## Acceptance criteria

1. Layout crate: two wires into one card from the previous column, one from
   a neighbour and one from three columns back, cross nowhere.
2. LAY-01 table: crossings per wire on data-only boards at most half of the
   value measured after LAY-04; no wire between adjacent columns has more than
   two bends.
3. The routing budget of ADR 0006 still holds (`bench --strict`).

## Not in scope

Ordering wires (LAY-06).

## Technical appendix

`route::route` routes longest first and pays `crossing_cost` only against
wires already routed, so the long wire takes the lane next to the card and
the short wire that follows cannot avoid it. Two approaches, to be chosen by
measurement: (a) order the vertical segments in each gap by the y of the port
they enter, after routing, as a lane assignment; (b) route wires per target
card in port order and charge a crossing to both wires. A detour that avoids
one crossing costs more than the crossing today (bend 40, crossing 120) — the
weights are revisited with the measurement in hand.
