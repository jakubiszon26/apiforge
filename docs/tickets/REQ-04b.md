# REQ-04b: Run to here — run the scenario up to the selected step

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: **Decided** (2026-11-19, 8 = B; [REQ-00](REQ-00.md)). Mock-up
  approved (the button of the [Body](REQ-00.md#mock-up) capture, the
  menu of board 6). After REQ-02a and REQ-04a.
  Issue [#461](https://github.com/jakubiszon26/apiforge/issues/461).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

While working on one request the person runs the scenario as far as
that request and reads its response, without running what follows.

## What the owner will see

- New: *Run to here* on the request pane and in a card's menu.

## Acceptance criteria

Tests in `folder/acceptance_tests/req04b.rs`.

1. *Run to here* starts a run with `Partial::Through`; on `labels`, on
   *Weight 0 kg*, 15 steps run, its four checks among them.
   `req04b_criterion_1_*`
2. It goes through `request_run` and `start_run_with` with a kind added;
   no second `RunRequest` is built. Observing, it is refused with
   `LIVE_PAUSED`; live without a journal it is refused; in *Simulation*
   nothing is sent; a document with errors gets the prompt of a run.
   `req04b_criterion_2_*`
3. The steps after it read *Not run* — "the run stopped at *name*", in
   *Steps*, on the board and in Run results; the summary says the run
   was partial and names the step; the verdict is not *Passed*.
   `req04b_criterion_3_*`
4. Cleanup ran and its rows carry results. `req04b_criterion_4_*`
5. A step in a branch not taken is *Not run* with `not_run.rs`'s reason.
   `req04b_criterion_5_*`
6. The partial run is the document's last run: a full run replaces it.
   `req04b_criterion_6_*`

## Captures the owner judges on the pull request

The summary line of a partial run (today: "Diagnostic run · may use
saved inputs · does not pass the full test"); the rows after the stop;
the button at 960 × 540.

## Not in scope

Ctrl+Enter (P5, the owner's); *Send alone* (REQ-05).

## Technical appendix

- Files: `folder/runs.rs` (the kind, the stop kept beside the report),
  `steps/request.rs` and `steps/list_edit.rs` (the button, the item),
  `not_run.rs` (the words), `report.rs` (the summary line), the test
  file. The engine and `NotRunCause` are not changed: the window words
  the steps a partial report leaves out.
- Regression: the gates of a live run — criterion 2 names each; a full
  run's report and freshness (`report_tests.rs`) unchanged.
