# LAY-45: The ordering lines of a chain read left to right, row by row

- Size: M · Owner review: 20 min · Risk: `visible UI`, `removal` (the
  arrow at the badge and the mark at a row's end go)
- State: **Approved, ready for implementation after LANG-13 (#386) has
  merged.** Owner decisions: 2026-11-19, question 12 = A (the chain is
  drawn side to side; to the next row one line through the row gap);
  2026-11-20, DIAG-06 quiz on #391, question 1 = A (its own ticket, in the
  LANG sprint, before DIAG-06) and question 2 = A (order sockets on the
  card's sides at the title line, in both *Wires* modes; the arrow at the
  badge and the mark at a row's end go).
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
  sockets; data wires and chips; where Tidy up puts the cards.
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
3. The line to the next row lies in the row gap, at least 20 px from every
   card and from every other line along its run (the owner's rule of
   2026-11-15); a row gap grows when it must. Test: `lay45_criterion_3_*`;
   `packed` is 0 on every reference board (`apiforge-layout` `quality`).
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
8. Crossings: on every reference board, in both modes and both check
   states, no more than before this ticket; `shop-30` with every wire is
   measured against 76 (checks folded) and 116 (drawn), LAY-28's limits
   before LANG-13, and the result is written on #468 whatever it is.
   Test: the `lay28` and `lay06` criterion 11 suites with their figures
   recorded again; the table is in the pull request.
9. The cleanup and a component's body draw their chains the same way.
   Test: `lay45_criterion_9_*`.
10. At 960 × 540 and 1600 × 1000 a board fits as before: the fit zoom of
    every reference board is not smaller than before by more than 0.02.
    Test: `lay45_criterion_10_*`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| LAY-06 criterion 4 ("no order wire through a gap between cards") is replaced for the row gap: the row-change line is there by decision | criteria 2 and 3; the test of LAY-06 criterion 4 restated, listed in the pull request |
| The 20 px rule (LAY-03) and lines under their own card (LAY-29) | `lay03`, `lay29` suites unchanged; criterion 3 |
| Crossing limits of LAY-28 and LAY-06 criterion 11 | criterion 8 |
| Cards grow by their sockets (#349): two new side sockets | `lay28` socket tests; criterion 10 |
| The routing budget of ADR 0006 (150 ms) | the `bench` example, in the pull request |
| Accessible names of what is removed | `accessibility_tests` updated to the decided state |

## Not in scope

- Any gesture: a second wire, a move, a drop on a line — DIAG-06 (#391).
- Branch bands and how an `if` joins the chain (DIAG-06 question 6,
  LAY-13 #297).
- Where Tidy up puts cards and rows.

## Technical appendix

`crates/apiforge-desktop/src/canvas/routing.rs` (where an ordering wire
leaves and enters), `layout.rs` (the sockets of a card's sides),
`shape.rs` (`marks` goes), `after.rs` (goes), `mod.rs`, `paint.rs`;
`crates/apiforge-layout/src/route.rs` only if a side port needs a rule the
router lacks; `examples/quality.rs` and `support/boards.txt` written
again; ADR 0006 (rule 4 of shape D). The withdrawn attempt of LAY-29 on
#405 (`3951634`, `1aae304`, `adecdf8`) is worth reading, not reusing: it
moved *Start*'s fan, which LANG-13 removed.
