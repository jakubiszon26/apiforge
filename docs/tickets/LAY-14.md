# LAY-14: Wire and input tracing — which field feeds which input

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Approved 2026-11-17 with decisions 1 A, 2 A, 3 A (issue #298);
  implemented on `feat/lay-14-wire-tracing`. Was S–M · 20 min: LAY-23,
  LAY-25, LAY-32 and LAY-10 delivered part of it.
- Analysis model: Fable · Implementation model: Opus

## Goal

Every line and every chip on the board says, in one statement, which field
of which step feeds which input of which step (`valid_cart.value.id →
checkout.body.cartId`), and a click keeps it on screen while the person
reads.

## Already on main

- A value read through chips (three or more readers, or a variable read
  far): pointing at the chip or at its input row shows the statement in a
  box and draws the wire (LAY-23, LAY-25, LAY-32).
- Selecting a step lights its wires, rings the ports at their ends and dims
  the rest (LAY-10).
- A click on a wire selects it (it turns orange, Delete removes it); Esc or
  a click on the empty board ends it.

## What the owner will see

- New: pointing at a drawn line (a value with one or two readers) lights it,
  rings the port at each end and shows its statement in the same box a chip
  has, at the pointer.
- New: a click on a line or on a chip keeps it: the statement and the rings
  stay when the pointer leaves, the cards not at its ends and the other
  lines dim as they do for a selected step, until Esc or a click on the
  empty board.
- Changed: pointing at an input row lists every value that feeds it, one per
  line. Today it lists only the values that come through chips: on
  `checkout`'s *Body* it says `expected_total → checkout.body.total` and
  leaves out `valid_cart.value.id → checkout.body.cartId`, which arrives on
  a line.
- Removed or hidden: nothing. Default behaviour that changes: a click on a
  chip, which does nothing today, keeps its wire (question 3 decides whether
  a click on an input row changes too).

## Decisions

Taken: Q9 = A (2026-11-05), focus is a selection behaviour and tracing is
its second level.

Taken 2026-11-17, on the mock-up below: 1 **A**, 2 **A**, 3 **A**, and
the mock-up approved with this scope. The options as they were put:

1. **Where the statement stands.** **A** the box at the thing pointed at, as
   chips have today: one form for lines and chips, always beside the
   pointer. · **B** a pill at the line's midpoint, as round 3 drew: it sits
   on the line, but on a long line it is far from the pointer or outside the
   window, and it covers the line where two run together.
2. **What the rest of the board does.** **A** pointing only lights the line;
   a click also dims the rest, as a selected step does. · **B** pointing
   dims too: the board flickers as the pointer crosses lines. · **C** never
   dims: a kept line is told from the others by colour only.
3. **A click on an input row.** **A** stays what it is, selecting the card,
   which already lights all its wires; lines and chips are what a click
   keeps. · **B** keeps the row's values, as the ticket first said: changes
   what a click on a card's row does.

Whether the per-field reader list of LAY-18 joins this ticket is asked on
#302; it has no mock-up yet and is not part of this estimate.

## Mock-up

`docs/qa/2026-11-17/LAY-14/` on `mock/lay-14-18`: `shop-30` tidied, checks
folded, at 60 %, at 1600 × 1000 and 960 × 540 (`-1600`, `-960`). The
`before` views are `main` at dd44c05 with the real pointer and click; the
others are a throwaway drawing on the same board. The black arrow marks
where the pointer is.

| View | Shows |
| --- | --- |
| `lay14-wire-point-before` | main: pointing at the line `valid_cart → checkout` says nothing |
| `lay14-wire-point-a-box` | question 1 A: the line lit, both ports ringed, the box at the pointer |
| `lay14-wire-point-b-pill` | question 1 B: the pill at the midpoint |
| `lay14-wire-point-dim` | question 2 B: pointing dims the rest |
| `lay14-wire-pin-before` | main: the line clicked is orange and says nothing |
| `lay14-wire-pin-a-dim` | question 2 A: the line kept, statement and rings stay, the rest dims |
| `lay14-wire-pin-c-no-dim` | question 2 C: kept, nothing dims |
| `lay14-row-point-before` | main: `checkout`'s *Body* row names only the chip's value |
| `lay14-row-point-after` | the row names both values, both wires lit |
| `lay14-chip-click-before` | main: a clicked chip leaves nothing |
| `lay14-chip-click-after` | the chip's wire kept, as a line is |

Round 3 (`lay00r3-d-shop-30-trace-60`, `lay00r3-trace-input-60`), approved
as direction on 2026-11-05, is superseded by this mock-up once approved.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay14.rs`, on
`shop-30` tidied. Written for the recommended options; a different answer
changes criteria 1 and 3 only.

1. With the pointer within 6 px of the line `valid_cart → checkout`, the
   line is drawn at full strength, the ports at both its ends are ringed and
   the box reads `valid_cart.value.id → checkout.body.cartId`; no card and
   no other line changes. Moving the pointer away ends it. Test:
   `lay14_criterion_1_pointing_at_a_line_states_it`.
2. Pointing at `checkout`'s *Body* row shows two lines in the box,
   `valid_cart.value.id → checkout.body.cartId` and `expected_total →
   checkout.body.total`, and both wires lit with their ports ringed. Test:
   `lay14_criterion_2_a_row_names_every_value`.
3. A click on that line, or on the chip `expected_total` at `checkout`,
   keeps the statement, the rings and the wire after the pointer has left;
   every card not at its ends and every other line is dimmed as LAY-10
   dims; Esc ends it, and so does a click on the empty board. Ctrl+click
   adds a second line with its own statement. Delete still removes a kept
   line. Test: `lay14_criterion_3_a_click_keeps_it_until_esc`.
4. Tracing moves no card and no camera, and the box is whole inside the
   board at 960 × 540 and 1600 × 1000, also when the pointer is at the
   board's edge. Test: `lay14_criterion_4_tracing_moves_nothing_and_fits`.

The former criterion 5 (reporting the 12 px clearance) is dropped: LAY-03
replaced it with the 20 px rule and its own measure.

## Not in scope

Dashed ordering lines (they are already labelled `after …`); relations out
of view (LAY-15); the per-field list at a reader count (LAY-18, #302);
routing changes; the card's own tooltip, which still appears under the
pointer on a row.

## Technical appendix

- `canvas/mod.rs`: `hit_wire` (the 6 px test a click uses) run on hover over
  `drawn` and `drawn_revealed`; `pointed_input` extended from `tags` to the
  board wires of the row; the kept set is the existing `selected_wires`,
  which a click on a chip now fills with `tag.wires`; rings pushed into
  `port_rings` in `theme::ACCENT`; dimming through the `lit` set and
  `Look::dimmed` LAY-10 uses.
- `canvas/readers.rs`: `texts` already builds the statement of any data
  wire; `paint_statement` gains the clamp to the board.
- The mock branch carries the drawing as `MockTrace` (marked `MOCK`); it is
  not the implementation and is not merged.
- As built: the tracing lives in `canvas/trace.rs` (what is pointed at, the
  statements, the rings, the kept boxes and where they stand);
  `canvas/mod.rs` only calls it. A line is pointed at against the routes the
  last frame drew, as the arrows at the badges are.
