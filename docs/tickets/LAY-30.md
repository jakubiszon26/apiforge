# LAY-30: A step with one check writes it on its folded line; the checks switch sits on the board's bar and on each step

- Size: M · Owner review: 25 min · Risk: `visible UI`
- State: **ready** — mock-up accepted by the owner on 2026-11-18. Re-scoped by the owner's
  decisions of 2026-11-18 (Q1 = A, Q2 = A, #296); the switch on the bar and
  the arrow on a step stay as approved on 2026-11-17 (#383). Ready ·
  issue #383 · LAY sprint, part of its close, the cards track (before
  LAY-41)
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

The first view of a scenario is its requests, calls and branches; what a
step asserts opens at the step being read. A step with one short check
says it in place — `9 · status == 409` — so a person sees that a test
checks only the status without opening anything. A larger set stays a
count, and one visible control on the bar, and one on each step, shows
the check cards.

## Decided by the owner

- 2026-11-18, Q1 = A: a step with **exactly one** check writes the
  predicate on its folded line; after a run with ✓ or ✕; two or more stay
  a count.
- 2026-11-18, Q2 = A: checks are **folded by default for everyone**. The
  decision of 2026-11-16 "expanded by default" and decision 4 of
  2026-11-17 ("an existing installation starts once with checks drawn")
  are withdrawn; `preferences.rs` is not touched.
- Kept from 2026-11-16/17 (#383), mock-up approved: the switch is a button
  with the word *Checks* and an eye **on the board's bar**, out of the
  view menu; **an arrow at the end of a step's checks line** shows or
  folds that step's checks, the rest of the line still opens the
  Inspector's list; the board's switch sets every step and forgets the
  single choices; a step's own choice is remembered per document in
  `.apiforge/local/` (this computer, outside Git, no format change).

## What the board does today

`main` 2f6922c, the room project, 1280 × 800: checks are folded, a step
reads `9 · 1 check`; the predicate is two actions away (the line, then the
Inspector's list). The switch is *Collapse checks* in the view menu;
*Expand checks here* is a link in the Inspector's list, and after it the
board is not laid again: a neighbour's chips leave their rows and a line
runs through an input's chip.

## What the owner will see

- Changed: the checks line of a step with exactly one check, folded, and
  no gate: `9 · status == 409` before a run, `9 · ✓ status == 409` when it
  passed, `9 · ✕ status == 409` in red when it failed (the red footer and
  border as today). The number stays first (LAY-27).
- The predicate is written as the code writes it, with the step's own
  name left out: `second.status == 409` → `status == 409`;
  `calendar.body.items[0].id == first.body.id` →
  `body.items[0].id == first.body.id`. Another step's value keeps its
  name, so a check of two steps reads on both.
- Wider than the row, it is cut with `…` before the arrow; pointing at
  the line shows it whole.
- Unchanged: two or more checks (`7, 11 · 2 checks`, `✕ 1/2 failed`); a
  step whose one check did not run (`11 · 1 skipped`, as today); gates;
  the pill at the Overview zoom writes the predicate where it fits and the
  count where it does not.
- New: *Checks* with an eye on the board's bar; the arrow on each step's
  line (`›` folded, `⌄` shown). With a step's checks shown, its line
  reads as today without the predicate — the check has its own card.
- Moved: *Collapse checks* leaves the view menu (decided 2026-11-16).
- Stored: a step's own choice in `.apiforge/local/`.
- Default behaviour: none changes.

## Mock-up

Throwaway branch `mock/lay-close` (`review_laymock_checks`), the room
project's `double-booking` at 56 %, 1280 × 800, in
[`docs/qa/2026-11-18/LAY-30/`](../qa/2026-11-18/LAY-30/):

| View | Capture |
| --- | --- |
| At rest: `second` reads `9 · status == 409`, `calendar` `11 · body.items[0].id == first.bo…`, `first` `7, 11 · 2 checks`; *Checks* with a struck eye on the bar; an arrow at the end of each line | [lay30-line-at-rest](../qa/2026-11-18/LAY-30/lay30-line-at-rest.png) |
| Passed: `9 · ✓ status == 409` | [lay30-line-passed](../qa/2026-11-18/LAY-30/lay30-line-passed.png) |
| Failed (the API accepts the second booking): `9 · ✕ status == 409` in red on `second`; `calendar`, stopped, reads `11 · 1 skipped` | [lay30-line-failed](../qa/2026-11-18/LAY-30/lay30-line-failed.png) |
| `second`'s arrow opened: `refused` hangs under it. The capture also shows today's defect that criterion 8 forbids: `calendar`'s chips pushed off their rows | [lay30-step-open](../qa/2026-11-18/LAY-30/lay30-step-open.png) |

The bar's button and the arrow are those approved on 2026-11-17
(`mock/lay-30-checks-switch` at c11925d). In the mock-up the cut of a long
predicate is the row's clip, not yet `…`.

## Questions for the owner — decided

The owner accepted the mock-up on 2026-11-18 with every recommended option (1 = A, 2 = A, 3 = A; quiz, recorded on #383). The questions are kept below as asked. Folded into the ticket by Opus 5.5: Fable has no tokens left until 2026-11-22 (model policy).

Merge order: after LAY-15 (#442: `canvas/edge.rs`, `trace.rs`, `layout.rs` `Card::header_rect`, `app.rs`) and LAY-39 (#438: `canvas/readers.rs`) where this ticket touches the same files; merge `origin/main` first.


1. **The line as drawn** — number, then ✓ or ✕, then the predicate.
   **A (recommended):** as drawn. **B:** the sign first (`✓ 9 · status ==
   409`).
2. **How much of the path is written.** **A (recommended):** everything
   but the step's own name (`body.items[0].id == first.body.id`): it is
   what the code says, and the index is the kind of detail the board
   otherwise hides. **B:** the last field only (`id == first.body.id`):
   fits more often, drops `body.items[0]`.
3. **A step's one check that did not run.** **A (recommended):** the
   count, as today (`11 · 1 skipped`), as drawn. **B:** the predicate with
   a dash (`11 · – body.items[0].id == …`).

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay30.rs` on
a temporary copy of `fixtures/scenarios/lay/rooms/`, 1280 × 800, and unit
tests of the text in `canvas/collapse.rs`.

1. Folded, before a run, on `double-booking`: `second`'s line is
   `9 · status == 409`, `calendar`'s `11 · body.items[0].id ==
   first.body.id`, `first`'s `7, 11 · 2 checks`.
2. After a passing simulation `second` reads `9 · ✓ status == 409`; with
   the sample of `second` answering 201 it reads `9 · ✕ status == 409`,
   its footer and border red, and `calendar` `11 · 1 skipped`.
3. A step with one check and a gate, or two checks, reads as on `main`;
   a check of two steps is written on each, each leaving out its own
   name.
4. A predicate wider than its row ends with `…` and never runs under the
   arrow; pointing at the line shows the whole text.
5. The bar holds *Checks*, pressed while check cards are drawn; at
   960 × 540 and 1280 × 800 it is inside the window and overlaps no
   neighbour. The view menu has no *Collapse checks*. One click draws
   every check card, a second folds them; the camera stays; the state
   survives a restart; a new installation and an existing one both start
   folded.
6. The line of a request or component card ends with a button named
   *Expand checks of second* / *Collapse checks of second*: one click
   shows or folds that step's checks only, in either state of the board,
   and selects nothing; a click on the rest of the line opens the
   Inspector's list. The Inspector's list offers the same switch in both
   directions.
7. A step's own choice survives closing the document and the application;
   after it `git status` in the project is clean and the `.afs` and
   `.apiforge/diagram.json` are byte for byte the same. The board's switch
   forgets the single choices.
8. **Opening one step's checks displaces nothing (W7).** On `booking-day`
   at 76 %, the arrow of a step in a middle row (`hold`) opened: its
   check card overlaps no card; every chip and count of every other card
   has the rectangle and the text it had before; no line at rest runs
   through a chip. Folding it again gives the board of before.
9. A check folded by its step's switch that fails turns the step's line
   and border red (LAY-11 criterion 1); a gate whose result is read later
   stays drawn.

## Regression potential

- LAY-11, LAY-27 and LAY-31's tests read the line's text: they run
  unchanged for two or more checks; a fixture step with one check gains
  the predicate and the test that reads it is named in the pull request.
- Criterion 8 may need the hung cards to take room from the rows below.
  No board-geometry work is planned in LAY (#296): if it cannot be met by
  placing the opened checks and laying the chips again, the implementer
  stops and reports, and the criterion goes to the owner.

## Not in scope

- The Overview line (LAY-41). A folded check as a far reader (LAY-35,
  #410, LAY-2). Routing of the opened checks' lines beyond criterion 8
  (W7, LAY-2).

## Technical appendix

- `canvas/collapse.rs` `Aggregate::text()` (the mock's `single`,
  `mock_predicate`); `paint.rs` (the row's cut, `Pill::layout`);
  `canvas/mod.rs` (`toolbar`, `fold`, `expanded_here`, the arrow beside
  `aggregate_area`); `inspector/checks.rs`; the per-document store under
  `.apiforge/local/`.
- The predicate's operands print as a card's line prints a reference
  (`layout::source_text`), with the subject's handle removed; the signs
  are the script printer's (`==`, `!=`, `>`, `<`, `exists`, `nonempty`,
  `contains`, `is`, `~=`).
