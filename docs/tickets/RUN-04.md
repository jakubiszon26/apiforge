# RUN-04: A stopped run says "stopped" on every step it did not reach

- Size: S · Owner review: 10 min · Risk: `default` (the place of *Start* in
  the run's order, [to confirm](#to-confirm-at-approval))
- State: Analysis · issue
  [#418](https://github.com/jakubiszon26/apiforge/issues/418) · LAY sprint,
  engine track · follow-up of RUN-03 (#389)
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

Under the error policy *Stop*, a run that stopped tells the truth about the
steps it did not reach: each one reads "Stopped after an error." and names
the step that failed. *Start* is the first thing a run does, so a value
that fails cannot stop the run "before Start".

## What happens today

Read from `main` (514f06a), `crates/apiforge-runner/src/engine.rs`
(`execute_graph`, `gate`) and `crates/apiforge-core/src/graph.rs`
(`next_ready`).

| # | Fact | Where |
| --- | --- | --- |
| F1 | After a stop, only the steps whose predecessor **ran** get "Stopped after an error." with the cause `Stopped { by }`. A step whose predecessor was itself skipped is found *inactive* first and gets "Step outside the active execution path." with the cause `BlockedBy { the step before it }`. The message is false (the step is on the path) and the cause names a neighbour, not the failure | `execute_graph`: `gate.inactive` is asked before `stopped` |
| F2 | It is general, not the *Start* case: in `shop-30` with a wrong checkout total, 8 of the 11 steps not run carry it. The command line prints the message; the MCP `run` result carries the message and `blockedBy`. The window is right only because LAY-19 follows the cause to its end | LAY-19 analysis on #303 |
| F3 | Since RUN-03 a ready value or check is taken before a ready step, and *Start* is a step. A value that reads no step (`limit = 10`) therefore runs before *Start*; when it fails under *Stop*, *Start* is reported "Stopped after an error." and, by F1, everything else "outside the active execution path", blocked by `begin` | `next_ready` |
| F4 | No scenario, component or run fixture of the repository has such a value in a graph with a *Start*, so nothing pins F3 and no fixture changes with it | counted over `fixtures/` |

## Decision

Both options named on the issue, each for its own fact:

- **(b), generalised, for F1 and F2.** In the main phase, once a step has
  stopped the run, a step skipped only because something before it did not
  run is reported as stopped by that failure. A step on a branch that was
  not taken stays "outside the active execution path". No status changes:
  these steps are *Skipped* today and stay *Skipped*.
- **(a) for F3.** A ready *Start* is taken before anything else. FLOW-00's
  rule 6 says what runs "when a step has finished"; nothing in it puts a
  value before the first step, and a value is on the path only because
  *Start* reaches a step that reads it. (b) alone would leave "*Start*:
  stopped" on such a run; (a) alone would leave F1.

Considered and not taken: reporting every step after a stop as `Stopped`,
including one that reads the failed step directly (today
`BlockedBy { failure }` or `ConditionUnmet`, with "Unavailable data or an
unmet condition of an earlier step."). That message is true under either
policy, and LAY-19's approved words ("Blocked", "Not met") are read from
exactly that last link.

## What the owner will see

- Changed, command line and MCP: after a stop every step not reached reads
  `skipped … — Stopped after an error.`, and its cause is
  `stopped`, `by` the step that failed. The line "Step outside the active
  execution path." appears only on a branch that was not taken.
- Changed, the window: nothing in the usual run. LAY-19 already shows
  "Stopped · <failure> failed" on these cards and counts them as stopped.
- Changed, a scenario with a value that reads no step: *Start* runs first.
  Its badge is number 1 again and the value's is 2 (since RUN-03 it was
  the other way round); Run results list them in that order. When such a
  value fails under *Stop*, *Start* reads *Passed* and every later step
  "Stopped · <value> failed"; the summary counts one step fewer as stopped
  than in LAY-19's capture `lay19-a-value-before-start`.
- Removed or hidden, migrated, file format: nothing. No `.afs` file, saved
  run or fixture is rewritten.
- Default behaviour that changes: the place of *Start* before values that
  read no step, as above.

## To confirm at approval

The issue leaves the choice between (a) and (b) to the planning model. One
consequence is visible and the owner's to veto before the ticket is
labelled:

1. ***Start* runs before a value that reads no step** (recommended, as
   decided above). The alternative keeps today's order: such values are
   computed before *Start*, a failing one leaves *Start* "Stopped", and
   criteria 5 and 6 are dropped from this ticket. Everything else stands
   either way.

## Mock-up

Not applicable: no interface change. The window's words are LAY-19's and
do not change.

## Acceptance criteria

Tests first, in their own commit, in a new file
`crates/apiforge-runner/tests/run04.rs` unless a criterion names another.

1. **Stopped, not outside.** Policy *Stop*, a chain `begin → a → b → c → d`
   in which a check of `a` fails: `b`, `c` and `d` are *Skipped* with the
   message "Stopped after an error." and the cause
   `Stopped { by: <the check> }`. On `shop-30` with the wrong checkout
   total all 11 steps not run carry `Stopped { by: checkout_total }`
   (today 3) and none carries `BlockedBy`.
2. **A branch not taken stays outside.** In a stopped run, a step on the
   arm of an `if` that ran and took the other arm, and a step chained
   behind it, keep the cause `OutsidePath` and the message "Step outside
   the active execution path." (`not_created` in `shop-30` with the wrong
   refund amount, and a hand-written chain inside such an arm). A step in a
   branch of an `if` that itself did not run because of the stop is
   stopped (criterion 1).
3. **The message answers the cause.** In the main phase, and in the inner
   steps of a component, a step carries "Step outside the active execution
   path." if and only if its cause is `OutsidePath`. Checked by the helper
   that already walks every run in `tests/acceptance.rs`
   (`assert_causes_only_on_steps_that_did_not_run`), on its existing runs.
4. **Inside a component.** When an inner step fails under *Stop*, the
   inner steps after it are stopped by that inner step (criterion 1 on
   `children`), and the scenario's steps after the component call are
   stopped by the call.
5. **Start first.** In a scenario written `bad = "1" + 1`, `begin = start`,
   then requests of which a later one reads `bad`: the run's order is
   `begin`, `bad`, …; `begin` is *Passed*; `bad` failed or errored; every
   other main step is *Skipped*, stopped by `bad`; no step carries
   "outside the active execution path". With `limit = 10` instead, the run
   passes in the order `begin`, `limit`, the requests. A graph without a
   *Start* (a cleanup, a legacy document) keeps today's order.
6. **The board's order is the run's.** `graph::sequence` puts *Start*
   first on the scenario of criterion 5 and agrees with the engine on it,
   as written and reversed (beside
   `run03_criterion_5_sequence_agrees_with_the_engine` in
   `crates/apiforge-core/tests/acceptance.rs`). The orders of every
   reference scenario and every run fixture are unchanged:
   `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
   passes with no file regenerated, and `run_corpus` with no new
   divergence.
7. **An agent reads it.** The MCP `run` result of the scenario of
   criterion 1 gives `cause: { kind: "stopped", by: "<check's handle>" }`
   and the message "Stopped after an error." for `b`, `c` and `d`
   (`crates/apiforge-mcp/tests/acceptance.rs`). The command line prints
   the message as it is; no test of its own.
8. **The two small items of the issue.** The KTLO-27 Notes fixture
   (`crates/apiforge-desktop/src/folder/component_run_tests.rs`,
   `NOTES_SOURCE`) no longer writes `@after(echoed, fallback)` on a value:
   `size` stands inside the `else` braces, its comment says so, and the
   KTLO-27 tests pass with their assertions unchanged. `fixtures/README.md`
   says that the step order in `fixtures/runs` is produced by
   `graph::sequence`, the code under test, so `run_corpus` proves
   statuses and values, not order; the order is proved by the RUN-03 and
   RUN-04 tests.
9. Every existing test passes without a changed assertion, in particular
   `causes_on_shop_30`, the RUN-03 tests, LANG-14's criterion 2 (policy
   *Continue*: `BlockedBy` stays) and, once LAY-19 (#422) is on `main`,
   `lay19_criterion_8_a_failure_before_start`, whose assertions hold
   before and after; only its comment's "before *Start*" is reworded.

## Not in scope

- A step that reads the failed step directly keeps `BlockedBy` or
  `ConditionUnmet` and its message after a stop (see Decision).
- The policy *Continue*, cancellation and the statuses of any step.
- The cleanup phase. It never stops; a cleanup step ordered after a
  cleanup step that was skipped for a missing input appears, from the
  code, to get the same false "outside the active execution path". Not
  reproduced here; a new issue if a test shows it, not a fix in this
  ticket.
- The window's words, counts and colours (LAY-19), and a result line on
  Value cards.
- The reference's sentence "Line order means nothing" (LANG-13).

## Risks

- **Regression potential, bounded.** `next_ready` is shared by the engine
  and `graph::sequence`, which gives the badge numbers, the order checks
  fold in (`canvas/collapse.rs`), the step distance of LAY-25/26
  (`edit/shared.rs`, where *Start* does not count as a step) and the order
  of converted fixtures. F4 says nothing in the repository moves;
  criterion 6 proves it.
- **The stop rule rests on one invariant**: in the main phase, before the
  stop, a step is skipped only for a branch not taken, so an *inactive*
  step whose cause is not `OutsidePath` exists only after a stop.
  Criterion 2 covers the mixed case. If the implementer finds a run that
  breaks the invariant, that is an escalation, not a wider rule.
- **Overlap with LAY-19 (#422)**: it changes
  `crates/apiforge-core/src/run.rs` and the `chain_end` helper in
  `crates/apiforge-runner/tests/acceptance.rs`. RUN-04 starts after #422
  merges, or keeps its tests in `run04.rs` and refreshes with `main`
  before review.

## Technical appendix

- `crates/apiforge-runner/src/engine.rs`, `execute_graph`: where the
  message and cause are chosen, an inactive gate whose cause is not
  `OutsidePath`, in a graph that has `stopped`, yields
  `(STOPPED, Stopped { by })`. `gate` itself, the status and the
  precedence of `Cancelled` and of `gate.blocked` are untouched.
- `crates/apiforge-core/src/graph.rs`, `next_ready`: a ready
  `NodeKind::Start` is returned before the first ready data statement;
  its doc comment, `NodeKind::is_data`'s and `sequence`'s say so; a unit
  test beside `sequence_takes_the_first_ready_step_of_the_document`.
- Doc comments that describe the cause after a stop:
  `apiforge_core::run::NotRunCause`, the MCP copy in
  `crates/apiforge-mcp/src/tools.rs`, `in_run_order` in
  `tools/convert-v1/src/fixtures.rs`.
- Tests: `crates/apiforge-runner/tests/run04.rs` (new),
  `crates/apiforge-runner/tests/acceptance.rs` (the helper of
  criterion 3), `crates/apiforge-core/tests/acceptance.rs`,
  `crates/apiforge-mcp/tests/acceptance.rs`,
  `crates/apiforge-desktop/src/folder/component_run_tests.rs`.
- No change to `fixtures/runs`, the converter's sources, the desktop
  window's code or the language.
- Verification: the commands of the
  [development guide](../development.md#verification). Short QA section in
  the pull request: criteria → tests, and the `apiforge run` output of
  the scenario of criterion 5 before and after.
