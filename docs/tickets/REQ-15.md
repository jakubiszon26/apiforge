# REQ-15: A request with no sample response leads to where one is set

- Size: S · Owner review: 10 min · Risk: `visible UI`
- State: **Approved, ready for implementation** (issue #576) · decided by
  the owner on 2026-11-24 on the board, recorded on #576: **A, with both
  changes it names** (*Output data* shows no *Sample* mark and no status
  for a request without a sample; the empty sample field is empty, not
  `null`) · the same on v0.1.0 and v0.2.1-rc.2, not a regression ·
  **after REP-07a (#588) and REP-07b (#381, the details)**, which
  rewrite the pane the action stands in · labelled `ready:opus` · REQ
  milestone
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

From the refusal, and from *Output data* of a request without a sample,
one action opens the sample's field with the caret in it.

## Finding

Looked at on `main` (`1923fcd3`) merged with `feat/rep-07a-request-list`.

- The refusal is the engine's `MissingSample`
  (`apiforge-runner/src/engine.rs:203`), shown as the step's message in
  the details of Run results (`report.rs`, the message frame). REP-07a's
  list marks the row *Error · 1 step not run*; nothing leads on.
- *Output data* (`inspector/mod.rs`, `outputs`) says "A request can also
  have a sample response" and has no control. Before any run it shows
  the mark *Sample* and *HTTP status 200* for a request that has no
  sample: the default status is shown as if it were set.
- The section is the last of *Configuration* (`samples`), folded, below
  *Execution order*. At 960 × 540 with Run results open the panel shows
  about 110 px.
- The field of a request without a sample reads `null`.

## What the owner will see

- Changed: *Add a sample response* beside the refusal and in the empty
  *Output data*; the sentence there; the empty sample field.
- Removed or hidden: the *Sample* mark and *HTTP status 200* that
  *Output data* shows for a request with no sample (decided, criterion 2).

## Mock-up

Approved as a whole on 2026-11-24. Board: `docs/qa/2026-11-24/REQ-15/sample.png` (Today | A | B: the
refusal in REP-07a's Run results, *Output data*, and *Configuration*
after the action). Whole windows of A: `a-refusal-960.png`,
`a-opened-960.png`, `a-1600.png`. From `review_req15_sample` on
`mock/ws-05-req-15` at `aa6518b6`.

Seen on the captures: at 960 × 540 the details of Run results show two
lines, so the action stands on the message's own line, not under it;
after the action the panel scrolls the *Sample body* label to its top,
which leaves about three lines of the field in view.

| Option | Look | For the person at the window |
| --- | --- | --- |
| **A — the action (chosen)** | *Add a sample response* beside the refusal in Run results and under a one-line *Output data*; it opens *Configuration*, unfolds the section and puts the caret in *Sample body* | One click from either place where the tester stands. Nothing moves, nothing is written until the tester types |
| B — as A, the section under *Body* | The same action; *Contract and sample response* stands above *Execution order* | The section is also found by scrolling; changes the panel's order, which REQ-02a #458 redraws anyway — better decided there |
| C — a new request is written with `sample 200 {}` | Not drawn: nothing to see but a simulation that passes | Writes a line nobody set (against REQ-13 #540) and passes against a response the API never gives |

## Acceptance criteria (A)

Tests in `folder/acceptance_tests/req15.rs`.

1. A request stopped by the missing sample shows *Add a sample response*
   on the refusal in Run results' details; a step that failed for another
   reason does not. `refusal_offers_the_action`.
2. *Output data* of a request with neither a run's data nor a sample
   reads *No response yet. Run the step, or add a sample.* with the
   action under it, and shows no *Sample* mark and no status.
   `empty_output_data_offers_the_action`.
3. The action from either place selects the request, opens
   *Configuration*, unfolds the section, scrolls *Sample body* into view
   and gives it the keyboard: text typed next lands in the field. At
   960 × 540 with Run results open the label and at least three lines of
   the field are inside the panel. `action_opens_the_sample_field`.
4. Nothing is written by the action: the document is not dirty and has
   no undo step until the tester types. `action_writes_nothing`.
5. The field of a request without a sample is empty, not `null`; valid
   JSON typed there writes the sample with the status shown.
   `empty_sample_field`.
6. On a read-only board the action is not offered. `read_only_has_no_action`.
7. A step inside a component: the action is not offered (its request is
   in another document). `component_step_has_no_action`.
8. Design review at 960 × 540 and 1600 × 1000 against the board.

## Implementer's captures the owner will judge

- The refusal's line in REP-07b's details as they then are: the action
  stays on the message's line at 960 × 540.
- The empty *Sample body* field (its hint text, if any, is grey and is
  not written).

## Not in scope

Moving the section (B: with REQ-02a #458); writing a sample for a new
request (C); the refusal's wording, which the CLI and MCP share; a hint
in Problems before the run; "Sample responses" in Run results' header.

## Files touched

`report.rs` (the message frame, a new `Action`), `inspector/mod.rs`
(`outputs`, `samples`, the `Focus` that LAY-16 uses to scroll to an
input), `folder/runs.rs` or `folder/diagram.rs` (the action's handling);
`folder/acceptance_tests/req15.rs`, `testing.rs` (a review view).
Crate: `apiforge-desktop` only. Branch from `main` once REP-07b is
merged; if its details no longer hold a message line, stop and escalate.

## Regression risk

The Inspector's remembered scroll offset and sections (LAY-15 *Back*);
the details' height at 960 × 540 (REP-07a's criteria on the list and
the details, `rep07a.rs`); DIAG-10's mark above *Output data*, which
criterion 2 hides in one case — a visible change, named here for the
owner.
