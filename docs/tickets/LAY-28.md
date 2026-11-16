# LAY-28: Lines in one corridor are ordered so they cross less

- Size: M · Owner review: 25 min · Risk: `visible UI`
- State: implemented, awaiting acceptance (issues #348 and #349) · LAY
  sprint, layout track, started before the checkpoint (#296) by the owner's
  decision of 2026-11-16
- Analysis and implementation model: Fable 5.2 (the owner's rule for wire
  separation: the planning model analyses and develops the fix)

This ticket carries two issues that give one result, lines that can be
followed where several run together: #348, the order of lines in a
corridor, and #349, the inputs of a *Data condition* 20 px apart. They
share the reference boards and the metric table, so they share the pull
request.

## Goal

Since LAY-03 lines run 20 px apart instead of on each other, and a line
that leaves the others crosses the ones between it and its way out. The
lines of a corridor are put in the order that crosses least (#348). And
the straight pieces into the three inputs of a *Data condition*, 6 px
apart, were the last place where lines run nearer than the gap: the inputs
are 20 px apart (#349, owner decision A of 2026-11-16).

## What the owner will see

- Changed: fewer bridges where several lines share a corridor. With every
  wire drawn `shop-30` has 75 crossings instead of 82, 105 instead of 123
  with its checks drawn; `fan-out` 42 instead of 64. In the default view
  (chips) with checks drawn: `shop-30` 10 instead of 17, `fan-out` 5
  instead of 7, `shop-50` 26 instead of 38 ([Measured](#measured)).
- Changed: only the track a piece of a line lies on. Every line keeps its
  ports, its number of bends and the 20 px gap from lines and cards; no
  card moves, and Tidy up gives the same board as before.
- Changed (#349): the sockets on one side of a compact card are 20 px
  apart. A value, a check or a *Data condition* with two wired inputs is
  8 px taller, with three 28 px taller (`checkout_ok`: 96 → 124 px); its
  one line of text stays in the middle. A card with one wired input, a
  request and the full density are as they were. A board tidied before
  keeps its positions; Tidy up makes the room for the taller cards.
- Removed or hidden, migrated, default behaviour: nothing. No file format
  changes.

## Decisions

- #349: **A**, inputs 20 px apart and the card taller (owner, 2026-11-16,
  on the issue). The same rule for every compact card and for outputs: the
  issue asks for "a card's input ports", and a check with two wired inputs
  had them 8 px apart.
- #348 is the agent's decision (how lines are routed) and is reported
  here: the order is found after routing, by moving pieces between the
  tracks of their corridor, not by a new search. It cannot break the gap
  rule by construction and costs a tenth of the routing time.
- Not decided here: #382 (LAY-29), #383 (LAY-30) and #384 (LAY-31), filed
  the same day. Nothing in this ticket works against them: a piece moves
  only within the corridor it is in, between the nearest cards on either
  side, so it never comes between a step and its checks or under a card
  unless the router had put it there.

## Mock-up

Not applicable for #348: no new element. For #349 the owner decided on
the issue; the captures of the pull request show the card before and
after.

## Acceptance criteria

LAY-28 (#348):

1. Lines that share a corridor and leave it one after another do not
   cross each other: three values of one card going up a corridor to the
   three inputs of a card in the row above have no crossing (4 before).
2. The order changes tracks and nothing else. On all 24 reference boards
   and on generated ones each line keeps its ports and bends, no line
   comes onto a card, the gap is broken nowhere it was kept, no two lines
   come to lie on each other, no board has more crossings than without
   the order, and routing twice gives the same.
3. The reference boards cross less: `shop-30` with every wire at most 75
   crossings (checks folded) and 105 (drawn); with chips and checks drawn
   `shop-30` at most 10, `fan-out` 5, `shop-50` 26; all 24 boards
   together at least 15 % fewer than without the order. `packed` is 0 on
   all of them.
4. Lines brought back among lines that stay (a chip's wire, a rail) are
   ordered among themselves; the lines that stay are not moved.
5. The window draws those boards: the same limits on the tidied board,
   a bridge at every crossing, no breach of the gap.
6. Review captures at 1600 × 1000 and 960 × 540, each asserting a card in
   view.
7. The budget of ADR 0006 holds: a full routing of its board within
   150 ms.

#349:

1. On every reference board no two sockets on one side of a card are
   nearer than 20 px.
2. A card grows by what its sockets take and no more: one wired input,
   as before; two, 8 px; three, 28 px.
3. No two lines of a reference board run within 8 px of each other for
   20 px (`close` in `quality`: 0 on all 24 rows, 1 to 13 before).

## Measured

`quality`, compact cards. *Before* is `main` at `6a47c99`; *sockets* is
#349 alone (`quality -- --unordered`); *after* is both.

| Board, view | Crossings: before → sockets → after | `close` | `packed` | Board, px | Fit 1600 × 1000 |
| --- | --- | --- | --- | --- | --- |
| `shop-30`, chips, checks folded (default) | 5 → 5 → 4 | 2 → 0 | 0 | 1880 × 1462 → 1880 × 1470 | 0.520 → 0.517 |
| `shop-30`, chips, checks drawn | 13 → 17 → 10 | 2 → 0 | 0 | 1900 × 1694 → 1920 × 1710 | 0.449 → 0.444 |
| `fan-out`, chips, checks drawn | 9 → 7 → 5 | 4 → 0 | 0 | 2008 × 1332 → 2008 × 1360 | 0.565 → 0.559 |
| `shop-50`, chips, checks drawn | 38 → 38 → 26 | 5 → 0 | 0 | 1968 × 3158 → 1968 × 3218 | 0.241 → 0.236 |
| `order-flow`, all wires, checks drawn | 36 → 36 → 26 | 3 → 0 | 0 | 1944 × 1018 → 1944 × 1034 | 0.583 → 0.583 |
| `shop-30`, all wires, checks folded | 79 → 82 → 75 | 3 → 0 | 0 | 2020 × 1642 → 2020 × 1650 | 0.463 → 0.461 |
| `shop-30`, all wires, checks drawn | 118 → 123 → 105 | 7 → 0 | 0 | 2140 × 2014 → 2160 × 2010 | 0.377 → 0.378 |
| `fan-out`, all wires, checks folded | 64 → 64 → 42 | 0 → 0 | 0 | 2128 × 1440 → 2128 × 1468 | 0.528 → 0.518 |
| `fan-out`, all wires, checks drawn | 130 → 130 → 90 | 5 → 0 | 0 | 2248 × 1612 → 2228 × 1640 | 0.471 → 0.463 |
| `shop-50`, all wires, checks drawn | 408 → 409 → 340 | 13 → 0 | 0 | 2388 × 3638 → 2388 × 3690 | 0.209 → 0.206 |

- Coincident pairs are the same on every row (72 and 102 on `shop-30`
  with every wire): they are the wires of one port on the straight piece
  out of it, which LAY-03 allows. The order adds none.
- The order alone, on the boards of `main`: `shop-30` with every wire
  79 → 69 and 118 → 95. The taller cards of #349 change the board a
  little, and the two together give the table above.
- `shop-30` as a new install shows it fits 1600 × 1000 at 0.517 (0.520;
  LAY-06 criterion 8 asks for 0.50).
- Full routing of ADR 0006's board: 132 ms (122 ms before, budget
  150 ms); `shop-50` with every wire and check: 35 ms (31 ms).
- Limits of earlier tickets that LAY-03 had to loosen go back: LAY-06
  criterion 11 (2.1 → 2.05 crossings per wire), LAY-23 criterion 7 (1.1 →
  0.6 per line on `fan-out` with checks drawn), LAY-24 criterion 9 (9 → 5
  crossings). LAY-21 criterion 1 follows #349: a value or a check is its
  header and a line, 8 or 28 px taller with two or three wired inputs.

## Not in scope

- A wire under its own card and where *Start*'s ordering wires leave
  (#382, LAY-29); checks expanded by default (#383, LAY-30); a step and
  its checks as one group (#384, LAY-31).
- Changing a line's route to save a crossing: a line keeps its bends and
  the side it passes a card on. A crossing that needs another route, or a
  10 px jog taken out, stays.
- LAY-05 (#36, lanes between columns) was the earlier plan for this work;
  its criterion 1 holds on `main` already, and what is left of it is this
  ticket.
- The full density: its input rows are 52 px apart.

## Technical appendix

- `crates/apiforge-layout/src/order.rs` (new): `untangle`, called by
  `route_among` after every wire has its route, before the bridges. For
  each piece of a line that lies between two others and crosses
  something, it weighs the other tracks of the piece's corridor (the
  multiples of the gap within 12 tracks, between the nearest cards on
  either side) and an exchange of tracks with a piece of another line
  there, and makes the best move that takes crossings off the board, if
  the result keeps the rules: no new breach of `packed` for the pieces
  moved, no line into the mouth of another port. Rounds until no move, at
  most 8; at most 60 000 moves weighed, which only a board wired at
  random reaches.
- `Routing::ordered` (default on) switches it off; Tidy up's measuring
  pass (`Routing::sizing`) routes each wire alone and has nothing to
  order. `quality -- --unordered` prints the tables without it.
- `packed.rs`: `involving`, the breaches a named set of segments is in,
  shares the two tests of `packed`.
- `canvas/layout.rs`: `SOCKET_GAP`; a compact row is as tall as its
  sockets take, and they are spread 20 px apart about its middle.
- `examples/support/boards.txt` is written again by
  `lay06_criterion_10_quality_on_real_card_sizes` for the taller cards.
