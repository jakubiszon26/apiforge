# LAY-14: Wire and input tracing — which field feeds which input

- Size: S–M · Owner review: 20 min · Risk: `visible UI`
- State: Backlog (issue #298) · adopted from the second opinion of 2026-11-04
  (4.4, 4.5 item 3), decided within Q9 = A on 2026-11-05 · LAY sprint, wave
  1, after LAY-10
- Analysis model: Fable · Implementation model: Opus

## Goal

Pointing at a wire or at an input row says, in one statement, which field of
which step feeds which input of which step (`login.body.token →
cart.headers.Authorization`), and a click keeps it on screen while the
person reads.

## What the owner will see

- New: the hovered or selected wire is drawn at full strength with both end
  ports ringed and its field label on a pill at its midpoint; the other
  wires dim. Pointing at an input row traces every wire feeding it (a
  template or an object reads several); a click on the row pins them, a
  click on one wire pins that one, until Esc. The full trace statement
  names source step, field, target step and input.
- Input rows become hover targets. Changed: nothing else. Removed or hidden:
  nothing. Default behaviour that changes: none.

## Decisions taken

The owner's Q9 = A (2026-11-05) made focus a selection behaviour; tracing is
its second level (4.5 item 3: the selected step and the traced value are two
levels). The 12 px router clearance is already the router's margin
(`apiforge-layout/src/route.rs`, `Routing::margin = 12`) and stays a LAY-01
metric, not a change here.

## Mock-up

Round 3 `lay00r3-d-shop-30-trace-60`, `lay00r3-trace-input-60`
(`mock/lay-00-round-3` at `ea981d8`). Approved as direction on 2026-11-05.
The mock traced one wire per row; a row fed by two sources was not rendered
(round 4 skipped it) and is criterion 2.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`, `mod
lay14`, on `shop-30.afs`.

1. Hovering the wire `login → cart` or `cart`'s token input shows the pill
   `login.body.token → cart.headers.Authorization` and rings both ports;
   the other wires are dimmed. Test:
   `lay14_criterion_1_hovering_traces_the_wire`.
2. A row fed by two sources (a template) shows both wires at full strength
   with two pills. Test: `lay14_criterion_2_a_row_fed_twice_shows_both`.
3. A click on the row pins the trace; it survives moving the pointer away
   and ends on Esc; a click on one wire pins that wire only. Test:
   `lay14_criterion_3_a_click_pins_until_esc`.
4. Tracing moves no card and no camera; the pill is within the window at
   960 × 540 (clamped to the board). Test:
   `lay14_criterion_4_tracing_moves_nothing_and_fits`.
5. The 12 px clearance of every wire from the borders of cards it does not
   end on is measured by LAY-01's command on the three scenarios and
   reported, not changed here.

## Not in scope

Relations out of view (LAY-15); the data-in-use line that starts a trace
(LAY-18); routing changes (LAY-03, LAY-04, LAY-05).

## Technical appendix

The hovered wire comes from `canvas/mod.rs`'s hit test on routed wires;
input rows get a `Sense::hover()` response in `paint::card`; the pinned set
is a `Vec<EdgeId>` on the canvas, cleared by Esc; dimming reuses LAY-10's
alpha pass; the statement is built from the edge's `Binding` and both ports'
names.
