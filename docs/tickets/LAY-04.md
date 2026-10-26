# LAY-04: Ports decide rows — wires between neighbours run straight

- Size: M · Owner review: 20 min · Risk: none (positions after *Tidy up*
  change; the cards and wires do not)
- State: Backlog (issue #35)
- Analysis model: Fable · Implementation model: —

## Goal

After *Tidy up*, a card sits so that the wire feeding it is a straight line
from the output port to the input port, and a chain of steps no longer jitters
up and down.

## What the owner will see

- New: straight data wires wherever one wire feeds a card from the previous
  column; cards in a chain aligned on their ports rather than on their centres
  (today: [every wire has two bends](../qa/2026-10-26/LAY/layout-demo-tidied.png)).
- Changed: *Tidy up* produces different positions than today; blocks placed
  by hand are untouched, as before.
- Removed, migrated, defaults: nothing.

## Mock-up

Not applicable: the owner sees the same board with straighter wires; the
QA report shows the demo and `order-flow.afs` tidied.

## Acceptance criteria

1. Layout crate: a chain of five cards, each consuming the previous card's
   output into its first input, tidies so that every wire has zero bends.
2. Crossing reduction uses the port offsets as its tie-breaker: two wires
   leaving one card from different ports do not cross right after the card
   when their targets can be ordered the same way.
3. LAY-01 table: straight wires ≥ 40 % on data-only boards; crossings and
   coincident pairs not above today's.
4. Demo board tidied: *Active promotion → Add to cart* and
   *Product → Active promotion* are straight.
5. ADR 0006, *As implemented*, records the new coordinate rule.

## Not in scope

Ordering wires (LAY-06); lanes between columns (LAY-05).

## Technical appendix

`Edge::with_offsets` exists and is never used: `board_edit::layout_edges`,
`folder/placement.rs` and `app.rs` build edges without offsets, and
`layered::order_within_layers` and `coordinates` never read them. Pass the
anchors the board uses (`Card::output_anchor`, `Card::input_anchor`, relative to
the card's top), break barycentre ties by offset, and in `coordinates` pull a
card to the row where its input port meets the feeding output port (the mean
of port misalignments when several wires feed it), within the gaps to its
neighbours, instead of to the mean of card centres.
