# REQ-02a: The selected request and its response side by side

- Size: M · Owner review: 30 min · Risk: `visible UI`, `default`
- State: **Decided** ([REQ-00](REQ-00.md): 2, 4 = A, P4). Mock-up
  approved. After REQ-01a, REP-07b (#381) and REQ-15 (#576).
  Issue [#458](https://github.com/jakubiszon26/apiforge/issues/458).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

One request is edited and inspected in one place: what it sends on one
side, what came back and what was checked on the other.

## What the owner will see

- New: in *Steps*, a selected request shows the list, the request pane
  and the response pane ([Body](REQ-00.md#mock-up), the rail).
- Default that changes (decided, P4): in *Steps* a run does not open Run
  results; the bar with the verdict stays and a click opens the panel.
  Canvas, Code and Split as today.
- Removed or hidden: nothing; the Inspector is unchanged in *Canvas* and
  is not shown in *Steps*.

## Acceptance criteria

Tests in `folder/acceptance_tests/req02a.rs`.

1. Selecting a request in *Steps* shows the three columns and no
   Inspector; any other row shows the list alone; *Canvas* is unchanged.
   `req02a_criterion_1_*`
2. Method, URL, Body, Headers, Query, Sample and Settings are edited by
   the Inspector's own field functions (`inspector/fields.rs`), not
   copies: for each field the same edit gives the same `.afs` text in
   both places. `req02a_criterion_2_*`
3. The response pane shows the last run's status, time and size, the
   body as REP-08 shows a value, the headers and the request sent; *Not
   run yet* without one; one line when the document changed after it.
   `req02a_criterion_3_*`
4. The request's checks are listed by REP-07a's rows with results, a
   failed one with its actual value; *Add check* adds one as Run results
   does. `req02a_criterion_4_*`
5. The tab row shows every tab that fits and puts the rest under *More*;
   at 1600 × 1000 all seven fit; at 960 × 540 every tab is reachable.
   `req02a_criterion_5_*`
6. Where the two panes cannot each hold 320 px the list is a rail of
   numbers and results; a click on the rail opens the list over the
   panes, a choice or Escape closes it; the hover of a number is the
   step's name. `req02a_criterion_6_*`, `assert_within_window`
7. In *Steps* a run leaves Run results as it was, closed or open; in
   Canvas, Code and Split a run opens it as today.
   `req02a_criterion_7_*`
8. *Add a sample response* (REQ-15), taken in *Steps*, opens the
   *Sample* tab of the request pane with the caret in it.
   `req02a_criterion_8_*`
9. *Run to here* and *Send alone* are not drawn by this ticket; the row
   they will stand in is REQ-04b's and REQ-05's. Review of the diff.

## Captures the owner judges on the pull request

The rail opened over the panes; the line "changed after the run"; *Not
run yet*; the tab row at 960; a selected value, `if` and call.

## Not in scope

The body editor's features (REQ-02b); headers as a table and Auth
(REQ-03); running (REQ-04b, REQ-05); anything leaving the Inspector.

## Technical appendix

- Files: new `steps/panes.rs`, `steps/request.rs`, `steps/response.rs`;
  `folder/steps_view.rs`; `folder/runs.rs` (one condition on
  `panel.open`); `report.rs` only to make an existing function callable;
  the test file.
- Regression: P4 — criterion 7 in all four modes; the Inspector —
  criterion 2 compares texts; REQ-15's own tests stay green.
