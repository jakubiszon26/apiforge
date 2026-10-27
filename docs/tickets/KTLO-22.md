# KTLO-22: Publishing from the header is sometimes not recorded yet when the test asserts it

- Size: S · Owner review: 15 min · Risk: none
- State: Approved 2026-10-27 (issue #51) — joins the KTLO sprint; implementation
  starts when a pull request awaiting acceptance frees a slot
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

`folder::tests::publishing_from_the_header_requires_a_comment_decision_and_cancel_keeps_source`
passes or fails on the same commit depending on the run, on Windows so far
(runs 40367559871 on `5cf3660`, a Markdown-only change, and 40410023496). A
red run that is not a regression costs every review, which is what KTLO-21
(#49) was raised for.

## What the owner will see

- New: nothing.
- Changed: nothing in the application. The test waits for the publication
  it started before it reads the manifest.
- Removed, hidden, migrated or default behaviour: nothing.

## Mock-up

Not applicable: no interface change.

## Cause, from the code

Publishing is storage work. `component_action(Publish)`
(`folder/publication.rs:28`) queues the job through `storage_project`, which
runs `session.publish` on the storage worker thread and applies the result —
the new revision in the manifest — only when a later frame's `poll_storage`
receives it (`folder/storage.rs:218-258`). The worker's `request_repaint` is
what wakes the real window; the test has no loop, only the frames it drives.

The test (`folder/tests.rs:1716-1722`) clicks *Publish*, clicks *Continue* in
the comment-loss prompt and asserts `manifest().revisions.len() == revisions +
1` at once. `click` drives three frames (move, press, release;
`tests.rs:1287`). On a fast machine the worker has published and the third
frame has applied it; on a slower runner the result is still on the channel,
the count is still 3, and the assertion fails. Nothing is lost: one more frame
would have shown the revision. Forty-four other places in the same file call
`wait_storage` after an action that touches storage; this one does not.

The same window exists before the first assertion of the test, where the
first *Publish* is expected to produce the `CommentLoss` prompt: that attempt
also runs on the worker and fails there with `CommentsWouldBeLost`. It has not
been seen to fail, presumably because an attempt that stops before writing is
faster than one that writes the revision, but it is the same race and is
closed the same way.

## Decision

**The test waits; the publication stays on the worker.** Everything that
writes to the project runs on the storage worker so the window never blocks
on the disk; a publication that completed before the click returned would be
the one exception, and would reopen the question for every other storage
action. This is the agent's decision (test strategy), recorded here because
the owner asked which of the two it would be.

## Acceptance criteria

1. The cause is shown before the fix: the test is run in a loop on Linux with
   the storage worker slowed — the harness or the loop adds the load, not the
   application — and the assertion at `tests.rs:1718` fails in a measurable
   fraction of rounds; without the load, or with the fix, it fails in none.
   The counts are in the QA report. If no load reproduces it in 200 rounds,
   the report says so and the code-evidenced cause above stands on its own.
2. After the fix the test calls `wait_storage` after every click that queues
   a publication (the first *Publish*, and *Continue*), and asserts the
   storage is idle before reading the prompt or the manifest.
3. Nothing in `crates/apiforge-desktop/src/folder/` outside `tests.rs`
   changes.
4. The verification commands of the development guide pass; the test passes
   30 consecutive rounds locally under the load of criterion 1.

## Not in scope

The `settle()` finding in the backlog's *Later* (it waits on analysis, not
storage); the Windows runner's speed; any other test that reads storage state
without `wait_storage` — if the implementation finds one, it is listed in the
QA report as a candidate for a ticket, not fixed here.

## Technical appendix

- `wait_storage` (`tests.rs:83`) frames until `is_settled()` and asserts the
  storage is idle; it is the wait every other storage-touching test uses.
- For criterion 1 a reproduction without touching the application: run the
  test under `taskset` on one core with a CPU burner on the same core, or
  with `cargo test -- --test-threads=1` while another process compiles, as
  KTLO-21 did; count the rounds with a shell loop. A `std::thread::sleep` in
  the storage worker behind `cfg(test)` would also show it but changes the
  application's code and is not wanted.
