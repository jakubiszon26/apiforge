# DIAG-12: A field of a response that a check already reads is marked

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: **Analysis, waits for the owner** (issue #533) · questions 3
  and 4 below · REQ, with REP-07 (#381) · no `ready:*` label yet
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

Looking at a response, the tester sees which fields are already checked
and by what, in the Inspector's *Output data* and in Run results, and
does not write the same check twice.

## Finding

Looked at on `main` (`46de17d5`) with REP-08's branch (#563) merged
over it, on a request `cart` with six checks (two on `total`, one that
fails on `paid`, one on a nested `owner.name`, one on the status).

- *Output data* marks nothing: `total` with two checks and `id` with
  none are the same row.
- Run results marks only a check added from these results since the
  run (*Check added*, UX-01), and that row then offers no second check.
- The row is one in both places since DIAG-10 and REP-08
  (`inspector/fields.rs`, `tree_row`); it ends in ✓ and {} in the
  Inspector, in *Check* and *To variable* in Run results.

## What the owner will see

- New: the mark, on DIAG-10's row in both places; pointing at it names
  the checks as DIAG-02 writes them (`total == 172.5`, the step's name
  left out, since the row is that step's).
- New (A): the menu of ✓ first lists the field's checks; a click
  selects the check (in Run results, REP-07's row of it).
- Changed: in Run results the mark has the colour of the outcome —
  passed, failed, not run. *Check added* stays as it is for a check
  added since the run.
- Removed or hidden: nothing. Default behaviour that changes: none.

## Mock-up

`docs/qa/2026-11-23/DIAG-12/marks.png`: the Inspector above, Run
results below; from `review_req_check_making` on
`mock/req-check-making` at `dab4de5a` (static marks on a real row).
Whole windows of A: `a-inspector-960.png`, `a-results-960.png`,
`a-results-1600.png`.

Question 3, the mark:

| Option | Look | For the person at the window |
| --- | --- | --- |
| **A — the ✓ is filled (recommended)** | The button that adds a check is a filled square where a check exists; in Run results *Check* reads *2 checks* or *1 failed* | The mark is where the hand goes to check a field, costs no width at 960 × 540, and opens the checks themselves |
| B — a chip beside the value | `== 201`, `nonempty`, *2 checks* in a pill before ✓ | What is checked is read without pointing; the value loses up to a third of a narrow row, and a long expected value is cut anyway |
| C — a line at the row's edge | A 3 px line left of the row, as an editor marks covered lines | The quietest and good for scanning a long response; says nothing itself and is far from ✓ |

Question 4, what is marked:

| Option | For the person at the window |
| --- | --- |
| **A — checks only; the field the check names (recommended)** | `nonempty items` marks `items`, not its children. A folded field with a checked field inside carries the outlined mark (*1 check inside*), so a folded response still shows where its checks are. A field a later step reads is not marked |
| B — also a field a later step reads | The {} is filled the same way where a step or a variable reads the field. Answers "is this used", a different question from "is this checked"; two marks on a row, and it needs its own mock-up before approval |

## Acceptance criteria

Written for 3 A and 4 A; tests in `folder/acceptance_tests/diag12.rs`
on the board's fixture.

1. In *Output data* of `cart` the rows `total`, `paid`, `items`,
   `owner.name` and *HTTP status* carry the mark and `id`, `coupon` do
   not; before a run and after it.
2. The mark's hover and accessible name list the checks in the text of
   `canvas::check_text` (DIAG-02), one a line.
3. `owner` folded carries the outlined mark and the hover *1 check
   inside*; opened, it carries none and `name` carries the mark.
4. The ✓ of a marked row opens the menu with the field's checks above
   *Add a check*; choosing one selects that check; adding a second
   check from the same menu works.
5. In Run results the mark is green where every check of the field
   passed, red where one failed (and reads *1 failed*), neutral where
   none ran; where *Check* is an icon the mark is the icon's.
6. A check in Cleanup that reads a Main step's field marks it too
   (LAY-20); a check of a component's inner step does not mark the
   caller's row.
7. Adding or deleting a check updates the marks in the same frame's
   edit; undo restores them. A response of 1000 fields and 200 checks
   draws without a per-row walk of the graph (one index per edit; a
   unit test counts the builds).
8. Nothing on the row moves: DIAG-10's criteria and REP-08's pass
   unchanged at 960 × 540 and 1600 × 1000. Design review against the
   board.

## Not in scope

REP-07's list (#381) and its detail pane, which marks the field of the
selected check with this mark, emphasised: REP-07 calls this ticket's
index and draws nothing of its own. Marks in the *Steps* view
(REQ-02a #458). Whether a second check may be added on a *Check added*
row before the next run.

## Proposals beyond the request

| Proposal | Problem solved | Size, risk | Trade-offs | Where |
| --- | --- | --- | --- | --- |
| *4 of 9 fields checked* above the tree | How much of a response is covered is counted by eye | S · `visible UI` | A number invites checking everything; cheap once the index exists | later, after this ticket |
| The mark on the rows of DIAG-15's menu | *Add check* does not say what is already checked | none extra | — | DIAG-15, when both are in |
| A field a later step reads is marked (4 B) | Deleting or renaming a field's source breaks a step nobody remembered | S–M · `visible UI` | A second mark on the row | candidate, own mock-up first |

## Technical appendix

One index per document edit: `(step, port, path) → checks`, built from
`Binding::references` of the nodes `inspector/checks.rs` `is_check`
accepts; the Inspector's `Context` and `report::Context` hand it to
`fields::json_tree` and `results_tree` (REP-08's `Results` already
takes `checked`). Outcomes from `RunResult::steps`. Risk named for M:
the index and the run's results disagree after an edit since the run —
criterion 5's "none ran" covers a check the run does not know. Merges
after REP-08 (#563); REP-07 after this or with its index stubbed.
