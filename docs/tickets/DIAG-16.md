# DIAG-16: Choose a response field takes a typed path

- Size: S · Owner review: 10 min · Risk: `visible UI`
- State: **Approved, ready for implementation** (issue #550) · decided
  by the owner on 2026-11-23 on the board, recorded on #550: **A, a row
  *Use "…"***, taken by Return or a click · the same on v0.1.0 and
  v0.2.0, not a regression · labelled `ready:opus`; the sprint is the
  owner's to name
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

The dialog *Choose a response field* does what its hint says: a path
typed there is taken.

## Finding

Looked at on `main` (`46de17d5`): a check `cart_id` reading `cart`,
which has no sample and has not run; *Response field* pressed, `id`
typed.

- The hint reads *Run the request or add a sample response. You can
  also type a path.* The field is a search: the list shows 0 fields and
  Return does nothing (`inspector/binding.rs`, `searchable_field_list`).
  Six of six first-time runs on both versions did this (AB-01 round 1).
- With a sample, a search that matches nothing says *No matching field.
  Change the search or type a path.* — the same promise.
- A path is typed under *Edit path manually*, outside the dialog and
  folded.
- Also seen: under the title the dialog says *Sample data* for a request
  that has no sample.

## What the owner will see

- Changed (A): what is typed and is not a listed field is a row *Use
  "id"*; Return or a click takes it and closes the dialog.
- Changed: the two hints stop promising what the list does not do.
- Removed or hidden: nothing; *Edit path manually* stays. Default
  behaviour that changes: Return in the dialog takes the first row
  (today it does nothing).

## Mock-up

`docs/qa/2026-11-23/DIAG-15/check-making.png`, lower row (one board
with DIAG-15), from `review_req_check_making` on
`mock/req-check-making` at `dab4de5a`. Whole windows of A:
`docs/qa/2026-11-23/DIAG-16/a-1600.png`, `a-960.png`.

Decided by the owner on 2026-11-23: **A**. The table keeps what was
offered.

| Option | Look | For the person at the window |
| --- | --- | --- |
| **A — a row *Use "id"* (chosen)** | The typed text as the list's row, with *Return ↵* at its end and a line saying it is a typed path | What every run tried now works: type, Return. One field in the dialog |
| B — a *Path* field in the dialog | The search stays a search; under the list a field *Path* and *Use path* | Nothing is ambiguous; two text fields to tell apart, and the path typed into the first one is still lost |
| C — only the hint changes | *Run the request or add a sample response to list its fields.* and a line pointing at *Edit path manually* | The smallest change; the dialog still cannot do it, the tester closes it and looks for the fold |

## Acceptance criteria

Tests in `folder/acceptance_tests/diag16.rs`.

1. With no sample and no run, typing `id` shows the row *Use "id"*;
   Return takes it: the reference's path is `id`, the dialog closes,
   one undo step. A click on the row does the same.
2. With a sample, a text that is the path of a listed field shows that
   field and no *Use* row; a text that matches other fields shows them
   first and the *Use* row last; Return takes the first row.
3. A text that is not a path the language accepts (`items[`) shows the
   row disabled with the reason, and Return takes nothing.
4. A typed path the sample does not have is taken, and is marked as the
   diagnostics mark it today (DIAG-05 #374): the test names the message.
5. The hints read *Run the request or add a sample response to list its
   fields.* and *No field of the sample matches.*; neither mentions
   typing where no row takes it.
6. The same list in the *Expected value* picker behaves the same.
7. The line under the title says *No sample yet* where the step has
   neither a sample nor a run.
8. Design review at 960 × 540 and 1600 × 1000, against the board.

## Not in scope

Feeding the list from the last run (REQ-11 #538). The value picker's
empty list after an edit (DIAG-13 #534, DIAG-14 #535).

## Proposals beyond the request

| Proposal | Problem solved | Size, risk | Trade-offs | Where |
| --- | --- | --- | --- | --- |
| Up and down move through the list, Return takes the row marked | The dialog is a search that only a pointer can finish | S · `visible UI` | A highlighted row to draw and keep in view | later; not part of this ticket |

## Technical appendix

`inspector/binding.rs`: `searchable_field_list` (also called for the
value picker) and `field_picker`; the path is checked with
`apiforge_core::path`. The origin line is `response::Source::origin`.
