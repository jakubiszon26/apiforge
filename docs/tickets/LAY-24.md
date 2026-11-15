# LAY-24: A mark on the card replaces the `@after` rail into the next step

- Size: S–M · Owner review: 20 min · Risk: `visible UI`, `default`,
  `removal` (rails not drawn by default)
- State: Analysis · mock-up rendered, decision open
  ([questions](#questions-for-the-owner)) · issue #334 · LAY sprint, after
  LAY-23 stage 1 (merged, #337)
- Analysis model: Fable · Implementation model: —

## Goal

The owner's decision 4 on LAY-23 (2026-11-12): an `@after` whose target is
the next step of the spine is not drawn as a dashed rail; the card says it,
for example `after quantity_ok`. Rails to steps further away stay. The
owner decides on the captures which mark it is.

It changes decision 3 of 2026-11-05 ([lay.md 5.1](../sprints/lay.md#51-decisions-taken-2026-11-05):
`@after` wires drawn, dashed, on the rail), which is why it is the owner's.

## What the board does today

Looked at in captures of `main` at `cefb72e`, *Chips for shared values* (the
default), checks collapsed, `fan-out` tidied
([1600 × 1000](../qa/2026-11-15/LAY-24/lay24-today-1600.png),
[960 × 540](../qa/2026-11-15/LAY-24/lay24-today-960.png)):

- All ten rails of `fan-out` enter the next step, each from the check of the
  step before (`@after(quantity_ok)` on `apply_coupon`). Inside a row they
  share one level above the cards, so the dashed lines of a row lie on
  each other and their labels (`after coupon_ok`, `after removed_ok`) are
  struck through by the neighbours' rails.
- A rail into the first step of a row leaves the last card of the row
  above, runs the whole width back and drops between the rows
  (`quantity → apply_coupon`, `remove_coupon → view`), beside the
  continuation mark `↳ apply_coupon` that already says the same.
- The badges of LAY-07 (8, 10, 12 …) already give the order. What the rail
  adds is that the order is written, and on which check it waits.
- At 960 × 540 the labels are cut by the rails and by each other above
  every row; none can be read whole.

## Measured

`review_lay24_measures` on the mock branch, the definitions of `quality`
(lines · crossings · coincident pairs · ink in board px), 1600 × 1000,
checks collapsed; the full table, with checks drawn, is
[lay24-measures.md](../qa/2026-11-15/LAY-24/lay24-measures.md).

| Board | Rails, into the next step | Chips (default): today → without | All wires: today → without |
| --- | --- | --- | --- |
| `fan-out` | 10, 10 | 19 · **9** · 5 · 10 700 → 9 · **1** · 3 · 3 500 | 31 · **50** · 76 · 25 100 → 21 · **10** · 71 · 18 000 |
| `far-value` (LAY-25) | 11, 11 | 16 · **16** · 2 · 9 600 → 5 · **1** · 0 · 2 200 | 30 · 48 · 103 · 30 100 → 19 · 14 · 102 · 22 700 |
| `shop-30` | 8, 3 | 25 · 4 · 14 · 14 100 → 22 · 4 · 14 · 12 800 | 42 · 76 · 54 · 39 400 → 39 · 67 · 58 · 38 200 |

- The 40 of 50 crossings of the issue are *All wires*. In the default mode
  since LAY-23 the same rails are 8 of 9 crossings on `fan-out`, 15 of 16
  on `far-value`, and two thirds or more of the ink on both.
- `shop-30` gains little: three rails of eight enter the next step and none
  of them crosses anything. The rule is for the shape "every request waits
  for the check of the one before".
- A, B and C leave out the same rails, so the numbers at rest are the same
  for all three. They differ in what a person can still read.

## Options

"Into the next step" in every option: the target stands on the spine and
the rail comes from the column just before it — from the step there or from
a check of it, folded or drawn — and is not a branch wire. The first step
of a row is the next step of the last one of the row above.

- **A. Words on a tab** (recommended). A small tab stands on the card's top
  edge, right of the badge: two dashes and `after quantity_ok`, muted. The
  rail comes back, accent, while a card at either end is selected. Where
  the card is narrower than the words (Fit at 960 × 540) the tab keeps the
  dashes alone.
  [1600](../qa/2026-11-15/LAY-24/lay24-a-tab-1600.png) ·
  [960](../qa/2026-11-15/LAY-24/lay24-a-tab-960.png) ·
  [`note` selected, 60 %](../qa/2026-11-15/LAY-24/lay24-a-tab-selected-60.png) ·
  [`shop-30`](../qa/2026-11-15/LAY-24/lay24-shop-30-a-tab-1600.png)
  ([today](../qa/2026-11-15/LAY-24/lay24-shop-30-today-1600.png))
- **A2. The same words inside the header**, right of the name.
  [1600](../qa/2026-11-15/LAY-24/lay24-a2-header-1600.png). No new shape
  outside the card, but at Fit the name leaves no room on four cards of
  ten (`apply_coupon`, `remove_coupon`, `checkout`, `lookup` show dashes
  only), so the board says the check for some steps and not for others.
- **B. A dashed arrow into the badge**, no words: `⇢ ⑩`. The name of the
  check is in a tooltip and on the rail when it comes back.
  [1600](../qa/2026-11-15/LAY-24/lay24-b-glyph-1600.png) ·
  [960](../qa/2026-11-15/LAY-24/lay24-b-glyph-960.png) ·
  [`note` selected, 60 %](../qa/2026-11-15/LAY-24/lay24-b-glyph-selected-60.png).
  The same at every zoom, the quietest; it does not say what the step
  waits for, and nine identical arrows say little more than the badges.
- **C. Nothing at rest**; the rail is drawn for a selected card.
  [1600](../qa/2026-11-15/LAY-24/lay24-c-on-demand-1600.png) ·
  [960](../qa/2026-11-15/LAY-24/lay24-c-on-demand-960.png) ·
  [`note` selected, 60 %](../qa/2026-11-15/LAY-24/lay24-c-on-demand-selected-60.png).
  The cleanest board; `product` and `cart`, which have no `@after`, look
  exactly like the steps that have one, so a written order is a hidden
  dependency until a card is clicked.

Seen in the mock-up and to be fixed by the implementation, whichever is
chosen: with A the rail brought back runs behind the tabs of its row and
repeats their words in its own label (capture *`note` selected*); a rail
that stays and passes over a marked card would run through the tab (none
does on `fan-out` or `shop-30`).

## What the owner will see

- New: the mark at a step whose `@after` names the step before or a check
  of it; its rail while a card at either end is selected.
- Changed: —
- Removed or hidden: the dashed rails into the next step, by default; each
  is one click away and (A) named on its card.
- Migrated or converted: nothing. The scenario and `diagram.json` are
  untouched.
- Default behaviour that changes: the rails into the next step are not
  drawn in the default *Wires* mode.

## Mock-up

`mock/lay-24-after-mark` at `f778b9b`
(`crates/apiforge-desktop/src/canvas/lay24.rs`, views `review_lay24` and
`review_lay24_measures`), rendered by the real board with placeholder
marks. Captures in [`docs/qa/2026-11-15/LAY-24/`](../qa/2026-11-15/LAY-24/),
each opened and each asserting a card in view; linked under
[Options](#options).

## Acceptance criteria

Written for A; the mark's shape in 1, 5 and 6 follows the owner's choice.
Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay24.rs`, on
`fan-out` and `shop-30`, tidied, at 1600 × 1000 and 960 × 540.

1. On `fan-out` in the default mode no dashed wire is drawn; each of the
   ten steps with an `@after` carries a mark that reads `after <name>` with
   the name the code writes (`after quantity_ok`, `after begin`), and
   `product` and `cart` carry none.
2. On `shop-30` the rail `cart → address` (`after cart_total`, two columns)
   and every branch wire are drawn as today; `login`, `checkout` and
   `logout` carry a mark and no rail.
3. Selecting the step at either end of a marked `@after` draws its rail in
   the accent colour and lights the mark; Esc or a click on the board
   returns to the mark. No card moves, the camera stays, the rails and
   wires that were drawn keep their routes, the document is not marked
   changed.
4. The rail brought back can be selected and deleted as any ordering wire,
   which removes the `@after` from the code and the mark from the card; an
   ordering wire dragged into the next step becomes a mark when dropped.
5. The mark's text is at least 9 px on screen at every zoom; it covers no
   badge, chip, continuation mark, card name or drawn rail; where the card
   is narrower than the words it is the dashes alone (rendered at 0.25,
   0.44, 0.60 and 1.0). Fit shows every mark whole.
6. The mark has the accessible name `<step> after <name>` and shows that
   statement on hover; a step that waits for two checks of the step before
   names both.
7. `fan-out` in the default mode has at most 2 crossings and 4 coincident
   pairs (1 and 3 in the mock-up, 9 and 5 today); `quality` prints the
   reference boards without the marked rails in the default mode and
   `boards.txt` is regenerated.
8. *All wires* and the workspace window draw every rail as today
   (question 3); switching moves no card and no camera.
9. Nothing is written: after Tidy up, selecting and hovering, the scenario
   file and `diagram.json` are byte for byte what they were.
10. Review views `review_lay24` at both sizes, each asserting a card in
    view, opened by the agent that takes them.

## Questions for the owner

1. **The mark.** A, words on a tab (recommended) · B, the arrow at the
   badge · C, nothing until selected · keep the rails, build nothing.
2. **Where the words stand** (if A). On a tab above the card (recommended)
   · inside the header, cut where the name is long.
3. **Which mode.** Marks in *Chips for shared values* only, *All wires*
   keeps every rail (recommended: LAY-23's criterion 8 says *All wires*
   draws what the board drew before) · marks in both modes.
4. **When the rail comes back.** While a step at either end is selected or
   the mark is pointed at (recommended, as a chip's wire) · on selection
   only · never, the mark is all there is.
5. **Checks drawn as cards.** The same rule: a rail from the check card
   into the next step is a mark (recommended; 16 → 1 crossings on
   `fan-out`) · the rails come back whenever checks are drawn.
6. **When.** Before the checkpoint (#296), so the owner's board is judged
   without the rails (recommended) · in part B.

## Not in scope

Rails to steps further away and branch wires (LAY-13 for the bands);
LAY-03's nudge of rails that stay on one level; the workspace window; any
change to what `@after` means or to the code the board writes.

## Estimate

**S–M · 20 min · `visible UI`, `default`, `removal`**, as LAY-23 guessed.
Complexity: one crate, the canvas only — the rule (`shape::plan` gives the
columns), one painter, the rails joining the wires LAY-23 already brings
back on selection. Uncertainty: the choice above; none technical, the
mock-up is the rule in 200 lines. Regression potential: bounded — the
tests of LAY-06 (criterion 4, rails) and LAY-11 (the label of a redrawn
`@after`) name *All wires* since LAY-23 and stay as they are if question 3
is answered as recommended; `boards.txt` changes. Verification: window
tests and the review captures.

## Technical appendix

- `canvas/mod.rs` `Canvas::show`: the rails are taken out after
  `folded_wires` and the chips' partition, before routing, and join
  `revealed_wires` when selected (the second router of LAY-23), so the
  routes of the drawn wires do not move.
- `canvas/shape.rs` `plan`: spine columns and the column a satellite hangs
  under; the mock-up's `lay24::marks` is the rule.
- `paint::wire_label` already writes `after <check>` for a rail redrawn
  from a folded check's subject (LAY-11); the mark reuses the text and the
  brought-back rail drops its label.
- Fit: the tab adds about 14 px above a first-row card; it joins
  `extents` as the chips do.
- LAY-14 (#298) hovers; the mark is one more hover target with a
  statement, like a chip.
- Open: the tab and a rail that stays share the strip above a row. Either
  the tab is given to the router as an obstacle or the rail's level clears
  the tab's height; decided in implementation, asserted by criterion 5.
