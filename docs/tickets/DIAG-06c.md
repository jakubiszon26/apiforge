# DIAG-06c: The board shows and takes the chain

- Size: M · Owner review: 20 min · Risk: `visible UI`, `default` (where a
  dropped block lands; cards move to make room)
- State: **Approved, ready for implementation after LAY-45 (#468),
  DIAG-06a (#391) and DIAG-06d (#493) have merged. Amended on 2026-11-20
  after an independent review (criteria 5 and 6).** Owner's answers of 2026-11-20 on #391:
  questions 3 and 4 (what is shown while carrying; a block lands on the
  nearest line), 5 = A (the row makes room), 6 = A (the `if`'s own *Next*;
  no join lines; `↳ next step`; bands after 0.2.0), 9 = A (a check's one
  socket), 10 = A (a line can be selected).
- Issue [#490](https://github.com/jakubiszon26/apiforge/issues/490) ·
  LANG sprint, first half, a condition of 0.2.0 · branch
  `feat/diag-06c-board-takes-the-chain`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

What DIAG-06a makes the document do can be seen and done by hand on the
board: where a step will go is shown before the mouse is released, an
`if` has a place to continue from, and the board reads in the order it
runs after every edit.

## What the owner will see

- New (question 6): an `if` has its own *Next* socket on its right edge,
  for the step after the whole `if`, beside *Yes* and *No*, which stay as
  they are. The two lines from the ends of the branches into the next
  step are no longer drawn; the last card of each branch shows `↳ <next
  step>` instead.
- New (questions 2 and 3, views 2 and 3): while an ordering wire is
  carried from a *Next* that already has a successor, the line it
  replaces fades and a caption at the pointer says what will follow what:
  over the board *New step after `a`, before `b`*; over a step *Move `c`
  after `a`* (with *and the 3 steps inside it* for an `if`, *into Yes of
  `paid`* across braces). Released on the board, today's *Add step* menu
  opens.
- New (question 4, view 5): a block carried from the Library marks its
  place on the nearest line of a chain and lands there; far from every
  line, it lands at the end of the main flow, and the mark is at the last
  step.
- Changed (question 5, view 2b): after a step is added or moved, the
  cards after it move one place, as Tidy up would put them.
- Changed (question 9): a check, value, variable or export shows one
  order socket, which takes a wire from *Yes* or *No* only.
- Changed (question 10): a selected ordering line shows *`b` follows
  `a`*.
- Removed (decided, question 6): the join lines after an `if`.
- Not yet: bands (LAY-13). Until then a block gets into a branch by a
  line of that branch's chain or by a wire from *Yes* or *No*; an empty
  branch only by that wire.

## Mock-up

Approved on #391: views 2, 2b, 3, 5, 6 and 7, at 960 × 540 and
1600 × 1000. Views 6 and 7 are built **without their bands**, as question
6 = A says; the pull request shows release captures of that state and of
views 2, 3 and 5 for acceptance.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/diag06c.rs`
on `labels` and `rooms/booking-day`, at both window sizes where a place
on screen is asserted.

1. An `if` has a *Next* socket; a wire from it to the board opens *Add
   step* and the new step runs after the whole `if`; a wire from it to an
   existing step moves that step there (DIAG-06a's operation).
   Test: `diag06c_criterion_1_*`.
2. No line is drawn from a branch's last step, or from an empty branch,
   into the step after the `if`; the last card of a branch that holds a
   step shows `↳` and the next step's name, also as its accessible text;
   an `if` that is the last step shows none. Test: `diag06c_criterion_2_*`.
3. While a wire from a *Next* with a successor is carried, the line to
   that successor is drawn faded and the caption reads as above for the
   board, a step, an `if` and a step in other braces; the caption is in
   the accessibility tree. Test: `diag06c_criterion_3_*`.
4. A block carried from the Library: within 60 px on screen of a line of
   a chain the mark is on that line and the drop inserts there, in that
   line's braces; farther from every line the mark is behind the last
   step of the main flow and the drop adds it there. A check or value
   carried the same way is not placed by a line (it lands as today).
   Test: `diag06c_criterion_4_*`; the smoke list's flow 3 holds.
5. After an add or a move, the steps of the braces that changed — from
   the first place that changed to the end of that chain, with the cards
   hung under them and everything inside an `if` among them — stand where
   a whole *Tidy up* of the board would put them, whether they were
   placed by hand or not (question 5 = A). For a move that is both the
   place left and the place entered, so no gap stays; an `if` takes the
   room of all its cards. Every other card keeps its position, also when
   that leaves the rows uneven; the camera does not move. Test:
   `diag06c_criterion_5_*`: an add, a move later, a move earlier, an `if`
   moved, each on a tidied board and on one with a card moved by hand
   before and one after the place.
6. Order sockets by kind: *Start* one out (*Next*) and none in; a
   request and a component call one in and *Next*; an `if` one in and
   three out, *Yes*, *No* and *Next*; a check, value, variable and export
   one in, which takes *Yes* or *No* only, and none out.
   Test: `diag06c_criterion_6_*`, one per kind.
7. A selected ordering line shows *`b` follows `a`* where a selected data
   line shows its text today. Test: `diag06c_criterion_7`.
8. Every gesture of criteria 1 and 4 leaves a text that passes the check.
   Asserted in their tests.

## Regression potential

| What can break | Covered by |
| --- | --- |
| LAY-45's lines and sockets | its `lay45` suite unchanged |
| The layout limits (20 px, crossings, fit) on the reference boards with the join lines gone | `lay03`, `lay28`, `lay06` suites; figures recorded again and listed |
| Library drop for a check or value (UX-01, #433) | their suites unchanged; criterion 4 |
| Cards placed by hand (criterion 5 moves some by decision) | `lay02`, `lay33` suites unchanged |
| Accessible names (#235) | `accessibility_tests` |

## Not in scope

The operations (DIAG-06a). The refused state while carrying, the notice
after a delete, `+` on a line, *Move earlier* and *Move later*
(DIAG-06b). Bands and a drop on a band (LAY-13).

## Technical appendix

`crates/apiforge-desktop/src/canvas/` (`layout.rs` sockets, `routing.rs`,
`mod.rs` the carried wire and the caption, `paint.rs`, `shape.rs`),
`dnd.rs` and `library.rs` (the carried block), `board_edit.rs`
(placing cards after an edit: `place_under_sources`, `free_slot_for`);
`apiforge-core/src/graph.rs` only for the port name of the `if`'s *Next*
(`connect` requires a branch port from an `if` today; DIAG-06a's
operation takes the whole-`if` place). "Nearest" is measured on screen
to the drawn line, 60 px at any zoom.
