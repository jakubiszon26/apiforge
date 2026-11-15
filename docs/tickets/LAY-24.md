# LAY-24: A mark on the card replaces the `@after` rail into the next step

- Size: S–M · Owner review: 20 min · Risk: `visible UI`, `default`,
  `removal` (rails not drawn by default)
- State: Decided 2026-11-15 ([decisions](#owner-decisions-2026-11-15)) ·
  mock-up approved (B) · issue #334 · LAY sprint, before the checkpoint
  (#296), after LAY-25 and LAY-26
- Analysis model: Fable · Implementation model: Opus

## Goal

The owner's decision 4 on LAY-23 (2026-11-12): an `@after` whose target is
the next step of the spine is not drawn as a dashed rail; the card says it,
for example `after quantity_ok`. Rails to steps further away stay. On the
captures the owner chose the mark: a dashed arrow into the step's badge,
the name in a tooltip.

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
- The options rendered left out the same rails, so these are the numbers
  of the mark chosen.

## The mark

"Into the next step": the target stands on the spine and the rail comes
from the column just before it — from the step there or from a check of
it, folded or drawn as a card — and is not a branch wire. The first step of
a row is the next step of the last one of the row above.

**B, a dashed arrow into the badge** (the owner's choice): left of the
step's badge, level with the card's top-left corner, a short dashed line
with an arrowhead, muted; no words. The same shape at every zoom.
[1600 × 1000](../qa/2026-11-15/LAY-24/lay24-b-glyph-1600.png) ·
[960 × 540](../qa/2026-11-15/LAY-24/lay24-b-glyph-960.png) ·
[`note` selected, 60 %](../qa/2026-11-15/LAY-24/lay24-b-glyph-selected-60.png)

- Pointing at the arrow shows `after quantity_ok` and draws the rail.
- Selecting the step at either end draws the rail in the accent colour,
  with the label a rail redrawn from a folded check has today, and lights
  the arrow.
- In *All wires* and in the workspace window every rail is drawn and no
  arrow.

Also rendered and not chosen (captures in the
[decision card](https://github.com/jakubiszon26/apiforge/issues/334#issuecomment-6898734331)):
the words on a tab above the card (the planning model's recommendation),
the words inside the header, and nothing until a card is selected.

Seen in the mock-up, for the implementation: the arrow stands in the gap
left of the card, where a chip of the card's first input, a wire of the
column gap and the previous card's continuation mark also stand — at
960 × 540 the arrow of `apply_coupon` almost touches the chip below it
(criterion 5); the label of the rail brought back sits on the rail's
level, where a chip's wire brought back for the same card runs through
it (capture *`note` selected*, criterion 3).

## Owner decisions (2026-11-15)

Quiz on the captures at `4f3f633`, recorded on #334.

1. **The mark: B**, a dashed arrow into the badge; no words, the check's
   name in a tooltip. Not A, the planning model's recommendation.
2. **Mode.** Marks only in *Chips for shared values* (the default); *All
   wires* keeps every rail.
3. **The rail comes back** while a card at either end is selected or the
   pointer is on the mark.
4. **When.** Before the checkpoint (#296), after LAY-25 and LAY-26.

Not asked, the planning model's default: checks drawn as cards follow the
same rule — a rail from a check card into the next step is a mark.

## What the owner will see

- New: the arrow at the badge of a step whose `@after` names the step
  before or a check of it; its tooltip; its rail while the arrow is
  pointed at or a card at either end is selected.
- Changed: —
- Removed or hidden: the dashed rails into the next step, by default; each
  is one hover or one click away.
- Migrated or converted: nothing. The scenario and `diagram.json` are
  untouched.
- Default behaviour that changes: the rails into the next step are not
  drawn in the default *Wires* mode.

## Mock-up

Approved 2026-11-15 (B). `mock/lay-24-after-mark` at `f778b9b`
(`crates/apiforge-desktop/src/canvas/lay24.rs`, `Mode::Glyph`; views
`review_lay24`, `review_lay24_measures`), rendered by the real board with
a placeholder arrow. The captures the owner decided on, in
[`docs/qa/2026-11-15/LAY-24/`](../qa/2026-11-15/LAY-24/), each opened and
each asserting a card in view: today at
[1600](../qa/2026-11-15/LAY-24/lay24-today-1600.png) and
[960](../qa/2026-11-15/LAY-24/lay24-today-960.png), and the three of B
above. The mock-up has no tooltip and no pointer gesture; criteria 3 and 6
name them.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay24.rs`, on
`fan-out` and `shop-30`, tidied, in *Chips for shared values* unless a
criterion says otherwise, at 1600 × 1000 and 960 × 540.

1. On `fan-out` no dashed wire is drawn, checks collapsed and checks drawn
   as cards; each of the ten steps with an `@after` (`catalogue`,
   `quantity`, `apply_coupon`, `note`, `stock`, `shipping`,
   `remove_coupon`, `view`, `checkout`, `lookup`) has one arrow at its
   badge, and `begin`, `product` and `cart` have none.
2. On `shop-30` the rail from `cart` to `address` (`after cart_total`, two
   columns) and every branch wire are drawn exactly as in *All wires*;
   `login`, `checkout` and `logout` have an arrow and no rail.
3. Selecting the step at either end of a marked `@after` draws its rail in
   the accent colour, dashed, and lights its arrow; the rail of a folded
   check carries its label `after <check>`, and no wire or rail drawn in
   that frame passes through that label. Esc or a click on the board returns to the
   arrows. No card moves, the camera stays, the wires and rails that were
   drawn keep their routes, the document is not marked changed.
4. Moving the pointer onto an arrow draws that one rail and lights the
   arrow without a click; moving it away removes the rail. The arrow's
   hover area is at least 16 × 16 px on screen at every zoom.
5. The arrow is at least 14 px long on screen at every zoom, muted at
   rest and accent while its rail is drawn, and its box overlaps no
   badge, chip, reader count, continuation mark, card or drawn wire
   (rendered at 0.25, 0.44, 0.60 and 1.0, and at Fit at both sizes). At
   Fit at 960 × 540 all ten arrows of `fan-out` are inside the viewport;
   Fit shows the arrows of the first column whole.
6. Pointing at an arrow shows the tooltip `after quantity_ok`: the word
   `after` and the names the step's `@after` writes for that rail, in the
   code's order, comma-separated when the step waits for two of the step
   before (`after a_ok, b_ok`). The arrow's accessible name is
   `<step> after <names>` (`apply_coupon after quantity_ok`).
7. The rail brought back can be selected and deleted as any ordering wire:
   the `@after` leaves the code and the arrow leaves the card in the same
   frame. An ordering wire dragged from a step into the next one is an
   arrow once dropped, and its `@after` is in the code.
8. In *All wires*, and in the workspace window, every rail is drawn and no
   arrow, exactly the wires `main` draws at `cefb72e`; switching the mode
   moves no card and no camera.
9. `fan-out` in the default mode has at most 2 crossings and 4 coincident
   pairs, checks collapsed (1 and 3 in the mock-up, 9 and 5 today), and at
   most 2 crossings with checks drawn (1; 16 today). `quality` leaves the
   marked rails out of the `chips` boards and `boards.txt` is regenerated.
10. Nothing is written: after Tidy up, hovering and selecting, the scenario
    file and `diagram.json` are byte for byte what they were.
11. Review views `review_lay24` — `fan-out` at 1600 × 1000 and 960 × 540,
    `note` selected at 60 %, an arrow pointed at with its tooltip, and
    `shop-30` — each asserting a card in view and opened by the agent that
    takes them.

## Not in scope

Rails to steps further away and branch wires (LAY-13 for the bands);
LAY-03's nudge of rails that stay on one level; the workspace window;
words on the card; any change to what `@after` means or to the code the
board writes.

## Estimate

**S–M · 20 min · `visible UI`, `default`, `removal`**, as LAY-23 guessed
and unchanged by the choice of B. Complexity: one crate, the canvas only —
the rule (`shape::plan` gives the columns), one painter, one hover target
with a tooltip, the rails joining the wires LAY-23 already brings back.
Uncertainty: none open; the mock-up is the rule and the arrow in 200
lines. Regression potential: bounded — the tests of LAY-06 (criterion 4,
rails) and LAY-11 (the label of a redrawn `@after`) name *All wires* since
LAY-23 and stay as they are (decision 2); LAY-23's criterion 7 and
`boards.txt` read the default mode and are re-measured. Verification:
window tests and the review captures.

## Technical appendix

- `canvas/mod.rs` `Canvas::show`: in `WireMode::Chips` the rails are taken
  out after `folded_wires` and the chips' partition, before routing, and
  join `revealed_wires` when pointed at or selected (the second router of
  LAY-23), so the routes of the drawn wires do not move.
- `canvas/shape.rs` `plan`: spine columns and the column a satellite hangs
  under; the mock-up's `lay24::marks` is the rule, checks drawn included.
- `paint::wire_label` already writes `after <check>` for a rail redrawn
  from a folded check's subject (LAY-11); the tooltip takes the same text,
  or `after <step>` for a rail from the step itself.
- The badge of LAY-07 moves left of the corner or up to clear the name and
  a chip (`paint::badge`, `clear_of`); the arrow is laid out from the badge
  as drawn, and gives way as the badge does (criterion 5).
- Fit: the arrow reaches about 30 px left of a first-column card; it joins
  `extents` as the chips do.
- LAY-14 (#298) hovers; the arrow is one more hover target with a
  statement, like a chip.
