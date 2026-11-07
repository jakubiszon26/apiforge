# LAY-19: A failure shows its consequences — not-run steps by cause

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: Backlog (issue #303) · added to the sprint by the owner on
  2026-11-05 (Q12 = C; the second opinion's item 6 of
  [lay.md 4.5](../sprints/lay.md#45-second-opinion-part-two-local-context);
  runner-side, kept in LAY by that decision) · LAY sprint, wave 1, after
  LAY-07
- Analysis model: Fable · Implementation model: Opus

## Goal

After a failed run the board says what the failure cost: the failed step
counts the steps it kept from running, and every step that did not run names
its cause as the engine knows it — stopped, blocked, or skipped — never one
grey "did not run".

## What the owner will see

- New: a failed step's footer reads `✕ failed · 4 steps not run` — the
  steps whose not-run cause traces to that failure (blocked by it directly
  or through another blocked step, or stopped by the policy it triggered);
  a step blocked by an unrelated false *Data condition* is not counted. A not-run card's footer names its
  cause: *Stopped* (the error policy *Stop* ended the run), *Blocked: cart
  failed* (a source failed) or *Blocked: condition unmet* (a *Data
  condition* was false), *Skipped: outside path* (an unselected branch), *Cancelled* (the person
  stopped the run). Run results' summary uses the same words.
- Changed: the footer of not-run cards (today one muted status) and the Run
  results summary's wording. Removed or hidden: nothing. Default behaviour
  that changes: none. The runner knows the cause where it builds each result
  (`engine.rs:545–600`: "outside path", "stopped", "unavailable", a false
  *Data condition*, cancellation) but today writes one message for a failed
  source and an unmet condition; this ticket records the cause as an enum
  there. No engine behaviour changes.

## Mock-up

Not yet approved: the item was added by the owner without a capture
(Q12 = C). Before the acceptance tests, a design-review view on the ticket's
branch renders `shop-30` after a failed simulation at 60 % and at Fit with
the footers and the summary, posted to the issue and approved by the owner
in one quiz (the wording and the count); round 4's `lay00r4-run-60`
(`mock/lay-00-round-4` at `df6305b`) is the partial run it builds on.

## Acceptance criteria

Tests in `acceptance_tests.rs`, `mod lay19`, on `shop-30.afs`, and a
runner test.

1. After a failed simulation of `shop-30` with the policy *Stop*, the
   failed card's count equals the number of *Stopped* rows plus the
   *Blocked* rows whose cause chain contains the failed step; a step
   blocked by an unrelated false *Data condition* is not counted. Test:
   `lay19_criterion_1_the_count_equals_the_caused_rows`.
2. Each not-run card's cause equals its `StepResult` message class: a step
   reading the failed step's output is *Blocked: <step> failed*; a step
   behind a false *Data condition* is *Blocked: condition unmet*; a step
   after the stop is *Stopped*. Test:
   `lay19_criterion_2_each_cause_matches_the_engine`.
3. A step on an unselected branch reads *Skipped: outside path*, never
   *Blocked*, in a passing and in a failing run. Test:
   `lay19_criterion_3_a_branch_not_taken_is_skipped`.
4. Run results' summary line uses the same four words with the same counts
   as the cards. Test: `lay19_criterion_4_the_summary_agrees`.
5. The footer fits LAY-16's box at Compact and Overview (the count and the
   cause word are at least 9 px on screen at Fit, LAY-09's rule). Test:
   `lay19_criterion_5_fits_the_box`.
6. Runner: the causes are an enum the window reads, not strings it parses
   — `Stopped`, `BlockedBy(step)`, `ConditionUnmet`, `OutsidePath`,
   `Cancelled` — and a test asserts the mapping of every `engine.rs`
   branch that builds a Skipped, Blocked or Cancelled result. Test:
   `not_run_causes_are_an_enum_with_one_variant_per_branch`.
7. A run cancelled by the person marks the unreached steps *Cancelled*, not
   *Stopped*. Test: `lay19_criterion_7_cancelled_is_its_own_cause`.

## Not in scope

Changing when the engine stops, blocks or skips; a Blocked step's retry;
the colours of the statuses (LAY-09's borders: red failed, amber blocked).

## Technical appendix

`StepResult` gains a `cause: Option<NotRunCause>` set where `engine.rs`
builds Skipped ("outside path", "stopped"), Blocked ("unavailable" — which
today covers both a failed source and a false Data condition, so the two
branches set different variants) and Cancelled results; the message strings
are kept for the CLI. The failed step's count is the number of results whose
cause chain reaches it (`BlockedBy` transitively, or `Stopped` when that
failure triggered the stop). `paint::card`'s footer and the Run results
summary read the enum.
