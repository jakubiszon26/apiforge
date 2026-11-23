# DIAG-15: The Checks tab of a step can add a check

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: **Analysis, waits for the owner** (issue #547) · class 2, REQ,
  and **A — *Add check* opens DIAG-08's menu over the response fields**
  decided by the owner on 2026-11-23; open: where the button sits and
  how the menu reads (question 1 below) · no `ready:*` label yet
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A tester who opens the *Checks* tab of a request to add a check finds
the way there, with and without checks already listed, and a check
listed there leads to the check.

## Finding

Looked at on `main` (`46de17d5`) with `review_req_check_making`: a
request `cart` with a sample of six fields, never run.

- The tab of a request no check reads is one sentence, *No check reads
  this step yet.*, and nothing to press; with checks it is a list of
  cards that react to nothing (`inspector/checks.rs`, `show` and `row`:
  the card senses a hover only).
- Six of six first-time runs on v0.2.0 went there to add a check
  (AB-01 round 1, #537). The three ways that work are elsewhere: ✓ in
  *Output data*, *Check* in Run results, *Comparison* in the Library.
- DIAG-08's menu is the menu **of one field** (*total equals 172.5* …).
  From the tab no field is chosen yet, so the decided A needs one step
  before it: which field.

## What the owner will see

- New: *Add check* in the *Checks* tab of a step that has outputs.
- New: a click on a check's card selects the check (its settings open,
  the board shows it); the card shows a hand and a hover fill.
- Removed or hidden: nothing. Default behaviour that changes: none.

## Mock-up

`docs/qa/2026-11-23/DIAG-15/check-making.png`, upper row (the lower row
is DIAG-16), from `review_req_check_making` on `mock/req-check-making`
at `dab4de5a`. Whole windows of A with the menu open:
`a-menu-1600.png`, `a-menu-960.png`.

| Option | Look | For the person at the window |
| --- | --- | --- |
| **A — a button, then a menu of fields (recommended)** | *Add check* is the first line of the tab. It opens *Add a check of*: the HTTP status and the response's fields as Output data writes them; a field opens DIAG-08's four sentences beside it | One place to press, the same in the empty and the full tab; two clicks to a check; the list of checks does not move |
| B — the response stands in the tab | No button: under the list, *Add a check from the response* and the tree of Output data with its ✓ and {} | Nothing to find and exactly the menu already known; but Output data is shown twice, {} has no business in *Checks*, and a list of four checks pushes the tree off the panel at 960 × 540 |

**Found on the mock-up (A):** at 960 × 540 the menu of this small
sample — thirteen rows of 38 px — is as high as the window
(`a-menu-960.png`). A real response is larger. Criteria 3 and 4 answer
it: rows as high as Output data's, nested fields folded, a scroll.

## Acceptance criteria

Written for A; tests in `folder/acceptance_tests/diag15.rs`.

1. The *Checks* tab of a request shows *Add check* as its first line,
   with no check and with checks; a step with no output shows none.
2. *Add check* → `total` → *total is a number* adds the check DIAG-08's
   ✓ adds from that row: same line in the file, one undo step, the
   check selected. Before a run it writes from the sample (DIAG-09);
   where the step takes no edit the button is disabled and says why.
3. The menu lists the step's outputs in the order of Output data, a row
   26 px high; an object or a list is folded and opens in place.
4. At 960 × 540 with the fixture `order` the menu stays inside the
   window and scrolls; `assert_within_window`.
5. With no sample and no run the menu says *Run the request or add a
   sample response to list its fields.* and offers the status.
6. A click on a check's card selects that check; Return on a focused
   card does the same; its accessible name stays distinct (UX-27).
7. LAY-08's criterion 5 holds: three checks fit the Inspector at
   960 × 540 under the new line (its test passes unchanged, or the line
   shares the summary's row).
8. Escape closes the menu and keeps the selection. Design review at
   960 × 540 and 1600 × 1000, against the board.

## Not in scope

The *Steps* view's *Add check* (REQ-02a #458). A typed path
(DIAG-16 #550). The mark on a field already checked (DIAG-12 #533):
when both are in, the menu's rows carry DIAG-12's mark.

## Proposals beyond the request

| Proposal | Problem solved | Size, risk | Trade-offs | Where |
| --- | --- | --- | --- | --- |
| The menu *Add a check* writes its rows as the check will read (`total == 172.5`, `total is "number"`, `exists total`, `nonempty total`) | DIAG-02 decided one notation; the menu still says *equals*, *is a number*. With DIAG-12 both stand in one menu (`DIAG-12/marks.png`, *A · pressed*) | S · `visible UI`; amends DIAG-08's decided wording | The words are easier for a first-time tester; the signs are what appears on the card a second later | candidate issue, `decision-needed` |
| A search line in the menu above twelve fields | A response of sixty fields is a long scroll | S | A third thing in a menu; REQ-11 (#538) may shorten lists first | later |

## Technical appendix

`inspector/checks.rs` `show`; the menu reuses `fields::check_menu` and
`FieldCheck::offered`, the action is `Action::AssertField` as from the
tree. The fields come from `response::source` and `response::fields`,
as in `binding::field_picker`. Selecting a check: the Inspector's
existing selection action. The mock's `mock_add_check` is a sketch, not
the code to keep.
