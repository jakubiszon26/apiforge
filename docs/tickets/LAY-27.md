# LAY-27: A collapsed check line carries the checks' numbers (7 · 1 check)

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Analysis · the shape decided by the owner on 2026-11-15
  ([decision](#owner-decision-2026-11-15)); one question open, captured and
  posted to the issue ([Q1](#open-question-q1)) · issue #344 · LAY sprint, after
  LAY-25 (merged, #339)
- Analysis model: Fable (alias; version not confirmed — owner allowed this
  once on 2026-11-15) · Implementation model: Opus

## Goal

Badges number every step, checks included (LAY-07, Q2 = A), and checks are
collapsed by default (LAY-11), so a board reads 6, 8, 10 and the hidden
numbers look like missing steps. The owner asked "where is step 7?" on the
LAY-25 captures; 7 was `quantity_ok`, inside `quantity`'s *1 check*. The
check line says which numbers it holds, so a person counting the badges
finds every number on the board.

## What the board does today

Looked at in the merged captures of LAY-25
([1600 × 1000, 56 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/becf5dfdae732f672dd93b683754ecaf9df60d91/docs/qa/2026-11-15/LAY-25/lay25-saved-1600.png),
[960 × 540, 25 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/becf5dfdae732f672dd93b683754ecaf9df60d91/docs/qa/2026-11-15/LAY-25/lay25-saved-960.png)):

- From 45 % up a step with checks has the check line: its own row on a
  compact card (`1 check`), the second statement of the footer on a full
  card (`HTTP 200 · ✓ 2 checks`). In the first capture the badges read 8,
  10, 12 … and the lines read `1 check`.
- Between 25 % and 45 % the line is a pill under the name, drawn only where
  it fits. On a compact card it does not: the second capture has badges and
  names and no word about checks. Below 25 % a card is its band, its border
  and its badge (LAY-09).
- The line is there with checks expanded too, beside the checks' own cards
  and badges.

## Owner decision (2026-11-15)

> The collapsed check line on the card names the numbers it holds:
> `7 · 1 check`, `7–8 · 2 checks`; a set that is not contiguous lists them,
> `7, 9 · 2 checks`. Numbers stay equal to the Run results rows; nothing
> else changes.

## What the owner will see

- Changed: the check line of a step whose checks are folded starts with
  their numbers, then reads as it does today: `7 · 1 check`,
  `7–8 · 2 checks`, `21, 23 · 2 checks`; after a run
  `7–8 · ✓ 2 checks`, `7–9 · ✕ 1/3 failed · 1 skipped`; on a full card
  `HTTP 200 · 7–8 · ✓ 2 checks`. Colours, the red footer and border of a
  failed check, the click that opens the Inspector's list: as today.
- The numbers are those of the checks **not drawn as cards**. A gate, or a
  check another step reads, keeps its card and its badge and is not named
  in the line (it is still counted, as today). A check with two subjects
  is named in both lines, as it is counted in both.
- Where the line has less room than its text, the words give way and the
  numbers stay: a line is cut at its right end, and a pill wider than its
  card reads the numbers alone (`7–8`), in the line's colour.
- Unchanged: with checks expanded, and on a step with *Expand checks here*,
  the line reads as today, without numbers — every check has its badge.
  Badges, Run results, the Inspector's list, the positions of the cards.
- Below 25 % a card that has no room for its name is its band, its border
  and its badge (LAY-09); the numbers of folded checks are not on the
  board there.
- Removed or hidden, migrated, default behaviour: nothing.

## Open question (Q1)

Below 45 % a compact card has no room for the pill under its name, so at
960 × 540 the board would still read 6, 8, 10. Where do the numbers of the
folded checks go there? Three answers, captured on `far-value.afs` at
960 × 540 after *Fit the view* (the frames below):

- **B (recommended, after the captures): a second, outlined badge** on the
  card's bottom-left corner with the numbers (`7`, `21, 23`). Every name
  keeps its width and the numbers read as what they are, step numbers. It
  is a new mark; in the frame it meets no chip, port or label, and
  criterion 7 holds it to that.
- **A: on the name's row, at its right end** — `quantity 7`, muted, 9 px.
  Nothing new on the card, but the names pay: `checkout` becomes `ch…`
  beside `21, 23`, `remove_coupon` `remo…`, `shipping` `shipp…`. The
  analysis recommended A before the frames; they speak against it.
- **C: nothing there.** The numbers appear from 45 % up; at 960 × 540 the
  gaps stay. The smallest change.

Found while capturing: Fit at 960 × 540 puts this board a hair under 25 %
(the control shows 25 %), where LAY-09 writes a name only where it fits and
no pill. So A and B follow the name, not the 25 % line: the numbers are
written wherever the card writes its name. Criterion 7 is written for B and
is reworded by the answer. Everything else in this ticket follows from the
decision above and waits for nothing.

## Mock-up

`mock/lay-27-overview` at `fa51466`, view `review_lay27`, fixture
`far-value.afs`, rendered by the real board; each frame asserts a card in
view. The mock draws A and B on compact cards only and is not for merge.

| # | Capture | Shows |
| --- | --- | --- |
| 1 | [the line, 1600 × 1000, Fit (56 %)](../qa/2026-11-15/LAY-27/lay27-line-1600.png) | The decided part: `7 · 1 check` under `quantity`, `21, 23 · 2 checks` under `checkout`, `23 · 1 check` under `lookup`. `cart` reads `19 · 1 check`: `same_item` compares `view` with `cart`'s product, so 19 stands under both. The frame is the same for A, B and C |
| 2 | [A, 960 × 540, Fit](../qa/2026-11-15/LAY-27/lay27-a-name-row-960.png) | The numbers at the right end of the name's row; `ch…  21, 23`, `remo…  17`, `apply_…  9`, `shipp…  15` |
| 3 | [B, 960 × 540, Fit](../qa/2026-11-15/LAY-27/lay27-b-second-badge-960.png) | An outlined badge on each card's bottom-left corner: 7, 9, 11, 13, 15, 17, 19, `21, 23`, 23, and 19 under `cart`; names as today |
| 4 | [C, 960 × 540, Fit](../qa/2026-11-15/LAY-27/lay27-c-nothing-960.png) | Today's board: badges 6, 8, 10 … and no word about checks |

## Acceptance criteria

Tests in `folder/acceptance_tests/lay27.rs` on `far-value.afs` and
`shop-30.afs`, checks collapsed unless said; the expected numbers are read
from `graph::sequence`, not written by hand, except where a criterion
quotes them.

1. The numbers are written as the decision says: `[7]` → `7`, `[7, 8]` →
   `7–8`, `[7, 9]` → `7, 9`, `[7, 8, 9, 12]` → `7–9, 12`; ascending, a run
   of two or more as a range with an en dash, then ` · ` and today's text.
   A unit test in `canvas/collapse.rs`.
2. On `far-value.afs` at 1600 × 1000 after *Fit the view*, `quantity`'s
   line reads `7 · 1 check`, `checkout`'s `21, 23 · 2 checks` (its own
   condition and the one it shares with `lookup`) and `lookup`'s
   `23 · 1 check`, and `cart`'s `19 · 1 check` (`same_item` reads it
   and `view`); every number from 1 to the number of steps is written
   on the board, as a badge or in a check line, and no number is both.
3. On `shop-30.afs` a step with two or more consecutive folded checks reads
   a range (`login`); a check inside a branch of `paid` is named in its
   subject's line by its place in the engine's order, like any other; the
   check with two subjects is named in both lines.
4. After a simulation of both fixtures, every number in a check line is
   the number of that check's row in Run results, and the line keeps its
   state after the numbers (`… · ✓ 2 checks`, `… · ✕ 1/3 failed`), red
   with the red border where a check failed (LAY-11, Q15).
5. On the Cleanup board of `shop-30.afs` the numbers start from 1, as its
   badges do: `removed` reads `2 · 1 check`.
6. A component call is one step with one number, and the steps inside it
   take none: on `far-value.afs` `product` and `cart` are 4 and 5, and the
   numbers on the board end at the number of the scenario's steps.
7. *(Q1, written for B.)* Below 45 %, at 960 × 540 and at 1600 × 1000, a
   card that writes its name and whose pill has no room carries the
   numbers in an outlined badge on its bottom-left corner, at least 9 px
   high, over no badge, chip, port, wire label or other card; a pill that
   fits reads the numbers before the words, and the numbers alone when
   the text is wider than the card. Every number of `far-value.afs` is on
   the board at 960 × 540 after *Fit the view*. A card that writes no
   name gets nothing.
8. No text is smaller after zooming in across 0.45 and 0.75, and zooming
   moves no card (LAY-09, criteria 2 and 5, still hold with the numbers).
9. With *Collapse checks* off, and for a step with *Expand checks here*,
   the line is today's text byte for byte, and the checks' cards carry
   their badges. A gate and a check another step reads keep card and
   badge and are in no line's numbers.
10. Adding a step in code before step 5 renumbers the lines in the next
    frame, with the badges.
11. `review_lay27` renders `far-value.afs` with checks collapsed at
    1600 × 1000 (Fit) and 960 × 540 (25 %), before and after a simulation,
    and one full card (*Compact cards* off); each asserts a card in view.
    Two captures in the pull request: the two sizes after a run.

## Not in scope

Numbers in the Inspector's *Checks* list or in Run results; the badge
itself; which checks fold (LAY-11); a card too small for its name; a
tooltip on the line.

## Estimate

**S · 15 min · `visible UI`.** Complexity: one formatter, one list of
numbers on the aggregate, the short form in the pill. Uncertainty: low
above 45 %; Q1 for the rest. Regression potential: tests that quote the
collapsed line (`3 checks` in LAY-11 criterion 2, `2 checks` in LAY-09
criterion 3, LAY-21's rows) are reworded in the acceptance-test commit;
the width of a compact *Checks* row. Verification: window tests and the
review view. Under 300 changed lines.

## Technical appendix

- `canvas/collapse.rs`: `Aggregate` gains the numbers of its folded checks;
  `aggregates` takes the folded set and the numbers. `Aggregate::text`
  puts them first; a second method gives the numbers alone.
- `canvas/mod.rs` (about line 1327): `fold` already returns the checks not
  drawn — exactly the set to name, *Expand checks here* and the expanded
  toggle included — and `numbers` is `sequence(input.graph)` per board, so
  Cleanup starts from 1 and a component call is one node. `numbers` moves
  above `aggregates`.
- `canvas/paint.rs`: `pill` chooses the short form by width; the footer of
  a full card (about line 1050) and the compact *Checks* row (about line
  1357) take `text()` as they do. For Q1 the Overview paths (about lines 850
  and 1236) add the mark; the mock's `second_badge` is its sketch.
- `canvas/layout.rs` line 921 writes the row's placeholder text
  (`1 check`), which `paint` replaces; the row's size does not depend on
  it.
- The accessible name of the line (`Checks of <step>`) and its click area
  are unchanged.
