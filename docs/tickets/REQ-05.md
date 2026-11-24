# REQ-05: Send this request alone, with the values of the last run

- Size: M · Owner review: 25 min · Risk: `visible UI`, `default`, a permission control
- State: **Decided** (2026-11-19, 8 = B; [REQ-00](REQ-00.md): 3.3 and
  3.4 = A by the owner; 3.1, 3.2, 3.5 ruled). Mock-up approved
  ([board 3](REQ-00.md#mock-up)). After REQ-04b.
  Issue [#462](https://github.com/jakubiszon26/apiforge/issues/462).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

One request is sent by itself, as in a request client: its inputs come
from the last run, nothing before or after it runs.

## What the owner will see

- New: *Send alone* on the request pane; its response in the response
  pane marked *SENT ALONE*, with "not part of a run: the run's verdict
  is unchanged · *Show the run's response*".

## Acceptance criteria

Tests in `folder/acceptance_tests/req05.rs`; unit tests of
`steps/single.rs`.

1. *Send alone* runs `Partial::Alone` with the document's last run;
   exactly one request is sent; no Cleanup. `req05_criterion_1_*`
2. A request that reads no step is sent with no last run.
   `req05_criterion_2_*`
3. Refused before anything is sent, naming the step and the field and
   offering *Run to here*, when a value it reads is missing: no last
   run, the step did not run or has no such field, or the last run was
   in the other mode. `req05_criterion_3_*`
4. In a live send, a value the last run holds only as `[REDACTED]` is
   missing in the same way; `[REDACTED]` is never sent. In a simulation
   it is not a refusal. `req05_criterion_4_*`
5. A document newer than the run is sent; the pane says "values from
   the run of *hh:mm*". A document with errors is refused with the first
   error; the last valid version is never sent alone.
   `req05_criterion_5_*`
6. It goes through `request_run` and `start_run_with`, as REQ-04b: live
   only when the switch says *Live*; observing → `LIVE_PAUSED`; no
   journal → refused; a live send is journalled. `apiforge-mcp` is not
   changed. `req05_criterion_6_*`
7. The result is kept beside the document's report, never published as
   it: the cards, Run results, the verdict and the report's freshness
   are those of the run; *Show the run's response* shows the run's; a
   new run, or a second single send of the step, replaces it.
   `req05_criterion_7_*`
8. The request's checks are evaluated and listed in the pane with
   results, counted nowhere else; a check that needs a missing value
   reads *Not run* with the step. `req05_criterion_8_*`
9. A simulated send shows the sample marked *SENT ALONE · simulation*
   and sends nothing. `req05_criterion_9_*`
10. A single send of a request that is not a GET says that nothing
    after it runs, Cleanup included. `req05_criterion_10_*`
11. Nothing is written to the document; Run results does not open.
    `req05_criterion_11_*`

## Captures the owner judges on the pull request

The refusal with its *Run to here*; the refusal for a redacted value;
"values from the run of *hh:mm*"; the line of criterion 10; the checks
of a single send; all at 960 × 540 too.

## Not in scope

Sending a secret of the last run (candidate #601); Ctrl+Enter (P5).

## Technical appendix

- Files: new `steps/single.rs` (what is missing, pure; the kept
  result); `folder/runs.rs` (the kind, routing the finished handle away
  from `publish`); `steps/request.rs`, `steps/response.rs`; the test
  file.
- Named interplay (why `ready:opus`): the live permission. One entry,
  `request_run`; criterion 6 tests each gate; REQ-04b's tests stay
  green.
- Regression: the document's report is never replaced (criterion 7,
  `report_tests.rs`); redaction (criterion 4).
