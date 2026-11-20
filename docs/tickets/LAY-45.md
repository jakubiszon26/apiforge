# LAY-45: The ordering lines of a chain read left to right, row by row

- Size: M · Owner review: 20 min · Risk: `visible UI`, `removal` (the
  arrow at the badge and the mark at a row's end go)
- State: **Approved, ready for implementation; LANG-13 has merged.**
  Restated on 2026-11-20 after the feasibility gate (commits `e10b99b`
  and `03d8230` on this branch, the table on #468) and the owner's answer
  to it the same day, **A**: the chain stays side to side, and what is
  measured is what the owner's rule is about — three measures in place of
  one count (criterion 8). The work goes on from `03d8230`.
- Issue [#468](https://github.com/jakubiszon26/apiforge/issues/468) ·
  LANG sprint, first half, before DIAG-06 (#391) · branch
  `feat/lay-45-chain-side-to-side`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

The order of a scenario can be followed along one line: out of a step on
its right, into the next step on its left, and at the end of a row once
through the gap to the row below. The line is also where DIAG-06 will let
a step be inserted, so it is drawn the same in both *Wires* modes.

## What happens today

On `main` after LANG-13 every ordering line runs from a step to the next
step. Shape D's rule 4 (LAY-06) starts it on the **top** edge of its card,
runs it on a rail above the row and enters the next card from above
(`canvas/routing.rs`, "from the top edge"); the *Next* socket at the
bottom is used only to drop into a card right below. With *Chips*, the
default, the line into the next step is not drawn at all: a short arrow
points into the badge (LAY-24, `canvas/after.rs`), and the last card of a
row carries a mark that names the next step (LAY-06 criterion 5,
`canvas/shape.rs`, `marks`). With *All wires* the line to the next row
turns back and goes down between two cards of the row it left.

## What the owner will see

- Changed: a step's order goes out of a socket on its right edge at the
  title line and into a socket on the next step's left edge at the title
  line, as a short arrow between neighbours of a row — in *Chips* and in
  *All wires* alike.
- Changed: from the last card of a row one line goes through the gap
  between the rows into the first card of the next row.
- Removed (decided, question 2 = A): the arrow at the badge with its
  `after …` text on hover, and the mark at a row's end that names the
  next step. The line says both.
- Unchanged: the lines out of *Yes* and *No* of an `if` and their
  sockets; data wires and chips; the order and the columns Tidy up gives
  the cards. Changed: Tidy up leaves the room between two rows that the
  line between them needs. A card's saved position never moves by this
  ticket: a board laid out before it shows the new lines in the gaps it
  has until *Tidy up* is pressed.
- Migrated, default: nothing. Nothing is saved; it is drawing.

## Mock-up

Approved with question 2 = A: view 1 of the DIAG-06 round on #391
(`docs/qa/2026-11-20/DIAG-06/diag06-1-chain-*.png` on the branch
`mock/diag-06-board-keeps-chain`, at 960 × 540 and 1600 × 1000, Fit and
close), and M8-A of 2026-11-19. The pull request shows release captures of
`labels` and `shop-30` in both modes at both sizes, measured against view 1.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay45.rs`
unless said, on `labels`, `shop-30` and `rooms/booking-day`, after Tidy up.

1. Every ordering line between two steps of one braces starts at the
   source's right-edge socket and ends at the target's left-edge socket,
   both at the title line; none starts or ends on an edge away from a
   socket. In both *Wires* modes. Test: `lay45_criterion_1_*`.
2. Between two neighbours of a row the line is one straight segment. No
   ordering line passes between two cards of one row.
   Test: `lay45_criterion_2_*`.
3. After *Tidy up*, the line to the next row lies in the row gap, at
   least 20 px from every card and from every other line along its run
   (the owner's rule of 2026-11-15); Tidy up's row gap grows by what that
   needs and no more. On a board that was not tidied since, the line is
   routed in the gap there is, as any line between hand-placed cards is.
   Test: `lay45_criterion_3_*`; `packed` is 0 on every reference board
   (`apiforge-layout` `quality`).
4. The lines out of *Yes* and *No* keep their sockets and their look.
   Test: `lay45_criterion_4_*`; the branch tests of `lay06` unchanged.
5. The arrow at the badge and the mark at a row's end are gone in both
   modes, with their hover texts and accessible names; nothing else of a
   card moves. Test: `lay45_criterion_5_*`; the suites that asserted them
   (`lay24.rs`; LAY-06 criterion 5 in `lay06.rs`) are retired or restated
   for the line, each listed in the pull request.
6. Pointing at a line of the chain, or selecting a step, shows what
   follows what as a data line does today (the line is marked; no new
   element). Test: `lay45_criterion_6_*`.
7. A card moved by hand keeps its sockets: the line is routed from the
   right socket to the left one around the cards, at 20 px.
   Test: `lay45_criterion_7_*`.
8. Crossings, as three measures (the owner's answer of 2026-11-20). On
   every reference board, in both modes and both check states, after
   Tidy up:
   - **Data lines against data lines**: no more crossings than before
     this ticket (`main` after LANG-13; on `shop-30` with every wire 54
     folded and 77 drawn, measured at the gate as 52 and 77).
   - **The chain against a data line**: only at a right angle; drawn with
     a bridge, and the bridge is on the **data line**, so the chain reads
     as one unbroken line; no segment of the chain runs within 20 px
     alongside a data line. Their number is measured and recorded, not
     limited.
   - **`packed` is 0.**
   The figures of the gate's table on #468 are the baseline of these
   boards (`shop-30` with every wire 89 and 152 in all; *Chips* drawn 4;
   `shop-50` *Chips* 11 and 20; `fan-out` drawn 82; `labels` drawn 12):
   the tests of LAY-28 and LAY-06 criterion 11 that hold one total for a
   board are restated to these three measures and listed in the pull
   request with before and after. The condition "back to 76 and 116",
   set when LANG-13 merged, is withdrawn: 52 and 77 of them are data
   against data, which no drawing of the chain changes.
   Tests: `lay45_criterion_8_*` (the gate's, restated), the `quality`
   example printing the two kinds of crossing apart, `lay28` and `lay06`
   criterion 11.
9. The cleanup and a component's body draw their chains the same way.
   Test: `lay45_criterion_9_*`.
10. At 960 × 540 and 1600 × 1000 a board fits as before: the fit zoom of
    every reference board is not smaller than before by more than 0.02.
    Test: `lay45_criterion_10_*`.

## What the gate found (2026-11-20)

The first commit after the acceptance tests measured the reference
boards routed side to side (`03d8230`). The 20 px rule and the fit hold
on every board (criteria 3 and 10). One total of crossings cannot: a
chain drawn through a row crosses every data line that passes down
through that row, and the line to the next row every data line that goes
on to a later one. Two other routings were measured and are not taken:
the chain routed in the usual order (the row change leaves the row gap
and fails criterion 3) and a higher price for crossing the chain (more
crossings elsewhere). The owner chose to keep the drawing and measure
the two kinds of crossing apart. A corridor for a value many steps read,
where the data-against-data crossings can fall, is a later ticket of
LAY-2 (LAY-46), not a condition of this one.

## Regression potential

| What can break | Covered by |
| --- | --- |
| LAY-06 criterion 4 ("no order wire through a gap between cards") is replaced for the row gap: the row-change line is there by decision | criteria 2 and 3; the test of LAY-06 criterion 4 restated, listed in the pull request |
| The 20 px rule (LAY-03) and lines under their own card (LAY-29) | `lay03`, `lay29` suites unchanged; criterion 3 |
| Crossing limits of LAY-28 and LAY-06 criterion 11 | criterion 8: restated by the owner's decision, each listed with before and after |
| Cards grow by their sockets (#349): two new side sockets | `lay28` socket tests; criterion 10 |
| The routing budget of ADR 0006 (150 ms) | the `bench` example, in the pull request |
| Accessible names of what is removed | `accessibility_tests` updated to the decided state |

## Not in scope

- Any gesture: a second wire, a move, a drop on a line — DIAG-06 (#391).
- Branch bands and how an `if` joins the chain (DIAG-06 question 6,
  LAY-13 #297).
- The order and the columns Tidy up gives the cards; reflowing a board
  after an edit (DIAG-06c).

## Technical appendix

`crates/apiforge-desktop/src/canvas/routing.rs` (where an ordering wire
leaves and enters), `board_edit.rs` (Tidy up's row gap), `layout.rs` (the sockets of a card's sides),
`shape.rs` (`marks` goes), `after.rs` (goes), `mod.rs`, `paint.rs`;
`crates/apiforge-layout/src/route.rs` only if a side port needs a rule the
router lacks; `examples/quality.rs` and `support/boards.txt` written
again; ADR 0006 (rule 4 of shape D). The withdrawn attempt of LAY-29 on
#405 (`3951634`, `1aae304`, `adecdf8`) is worth reading, not reusing: it
moved *Start*'s fan, which LANG-13 removed.
