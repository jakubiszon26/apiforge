# RUN-01: The chain goes on after an error; only a missing value blocks

- Size: M · Owner review: 20 min · Risk: `default` (what runs after an error
  under `on_error continue`)
- State: **Approved, ready for implementation.** The rules and the eight
  criteria are [LANG-00, section 1](LANG-00.md#1-what-a-chain-edge-means-after-an-error-run-01),
  decided by the owner on 2026-11-16 (#388, LANG quiz, decision 1) and
  approved with LANG-00 on 2026-11-17, two of its consequences among them
  ([Confirmed at approval](LANG-00.md#confirmed-at-approval), 1 and 2). This
  file adds nothing to decide: it names the test of each criterion, the
  regressions and the files.
- Issue [#126](https://github.com/jakubiszon26/apiforge/issues/126) · LANG
  sprint, first half · branch `feat/run-01-chain-after-error`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

With `on_error continue` a scenario keeps running after a step that failed:
the next step of the chain waits until the one before it has **finished**,
whatever its result, and is held back only when a value it reads does not
exist. With `on_error stop`, the default, nothing changes.

## What happens today

Read from `main` (eb2dc01), `crates/apiforge-runner/src/engine.rs` (`gate`).

| # | Fact | Where |
| --- | --- | --- |
| F1 | In the main flow a step is *blocked* when any statement it waits for errored, was blocked or was skipped, also one it only follows and reads nothing from | `gate`: `source.status.is_unavailable()` for every dependency |
| F2 | The cleanup already knows "after it finished": an order-only dependency never blocks there, and a value is judged one by one (`is_available`) | `gate`, `Phase::Cleanup` |
| F3 | In a chained cleanup a step skipped for a missing value still takes the steps after it along: they are found *inactive* because the only step they follow was skipped | `gate`: `inactive`, asked before anything else |
| F4 | `on_error stop` never reaches these rules after the first failure: the run stops (`stopped`, RUN-04) | `execute_graph` |

## What the owner will see

- Default behaviour that changes (decided): under `on_error continue` a
  step that reads nothing from a failed step **now runs** — `ship` after a
  failed `pay`, unless `ship` reads the payment. A scenario that must not go
  on uses `on_error stop` or an `if`.
- Changed: a blocked step names the statement whose value is missing, on
  the command line, for an agent and in Run results (the cause the window
  already follows, LAY-19). No word on screen changes.
- Changed: in a cleanup written as a chain, the steps after a skipped step
  run.
- Removed or hidden, migrated: nothing. No file changes; no syntax.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

All under `on_error continue` unless said; each is run simulated **and**
live (against the local test server the runner's live tests use). The tests
are a new file `crates/apiforge-runner/tests/run01.rs`, one function per
criterion, named below.

1. `a` errors without an answer; `b` follows and reads `a`; `c` follows `b`
   and reads nothing: `b` is blocked and names `a`; `c` runs.
   Test: `run01_criterion_1_blocked_travels_through_values_only`.
2. `a` answers 500 with a body; a step that reads a field the body has
   runs; a step that reads a field it lacks errors with *Missing value*;
   the step after either runs.
   Test: `run01_criterion_2_an_answer_has_its_outputs_at_any_status`.
3. A check that reads a value which does not exist is blocked; a check on a
   later step that ran passes or fails on its own.
   Test: `run01_criterion_3_checks_follow_their_values`.
4. An error inside a branch: the next step of the branch and the step after
   the `if` each run unless they read a missing value. An `if` that cannot
   decide is blocked, every statement in both of its braces, at any depth,
   is blocked by it, and the step after it runs — also when one branch
   holds no step. Tests: `run01_criterion_4_an_error_inside_a_branch`,
   `run01_criterion_4_an_if_that_cannot_decide`.
5. A component call that fails after computing an export: a step reading
   that export runs; a step reading another export is blocked.
   Test: `run01_criterion_5_exports_computed_before_the_failure`.
6. In a cleanup of three chained steps where the second lacks its value,
   the first and third run and the second is skipped.
   Test: `run01_criterion_6_cleanup_goes_on_after_a_skipped_step`.
7. Under `on_error stop` every run of the repository's run corpus is
   unchanged. Test: `simulation_runs_match_the_reference`
   (`tests/run_corpus.rs`) with **no** new divergence for
   `error-policy-stop`, and `tests/run04.rs` unchanged and passing.
8. The run is *failed* in 1–6; for each blocked step the report's cause
   names the statement whose value was missing (`BlockedBy { step }`).
   Asserted inside each test of 1–6, and once through the command line:
   `run01_criterion_8_the_command_line_names_the_missing_statement` in
   `crates/apiforge-cli/tests/run.rs`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| `on_error stop`: anything at all | criterion 7; `run04.rs`; `error-policy-stop` of the corpus |
| A false *Data condition* must still hold back the steps that read it (it guards data; rule 4) | `conditions` of the corpus, unchanged; `lay19` window tests unchanged |
| A step on a branch not taken stays *skipped*, and what reads its value stays as today | `if-true`, `if-false` of the corpus, unchanged |
| RUN-03: a step's checks still run before the next step | `run03.rs` unchanged and passing |
| The run corpus under `continue`: steps that today are *blocked* only by order now run (`next` in `components`, `join` in `merge-error`, and whatever else the run shows) | each change is listed in `STEP_DIVERGENCES` of `run_corpus.rs` with "RUN-01" and the rule it follows; the generated fixtures are not edited |
| The window's words for a step not run (LAY-19) | `folder/acceptance_tests/lay19.rs` unchanged and passing |

## Not in scope

- "After success only" for one step under `continue`: an `if` or
  `on_error stop` says it.
- A value from whichever branch ran: LANG-09 (#353).
- Any change to `.afs` text or to how a chain is written: LANG-13 (#386).

## Technical appendix

**The rule, as the gate applies it** (main flow, `ErrorPolicy::Continue`;
the cleanup differs only where said):

1. *Finished* is any recorded result. An ordering edge waits for its source
   to be finished and for nothing else.
2. A step all of whose ordering sources are off the path (a branch not
   taken, or a step skipped as outside the path) is skipped as today. A
   source that errored, failed or was blocked is **on** the path.
3. *A value exists* when the result of the statement that makes it has that
   output (`status`, `headers`, `body`, an export, `passed`, `value`). The
   path inside it is not judged here: a field the answer lacks is the
   reading step's own *Missing value* error, as today (confirmed
   consequence 2). In the cleanup the whole path is judged and the step is
   skipped, as today.
4. A step that reads an output which does not exist is *blocked*, cause
   `BlockedBy { step: <the statement that should have made it> }`. A false
   Data condition keeps its own cause (`ConditionUnmet`).
5. An `if` without `passed` (blocked or errored) is undecided: a statement
   standing in its braces, at any depth, is blocked by it. A statement
   stands in the braces of branch `b` of `x` when it follows `x.b` and
   everything else it follows stands in the same braces — the reading of
   `apiforge-script/src/branches.rs`. **Do not move that file**: LANG-13
   edits the script crate at the same time. The runner reads the braces
   itself, beside `ExecutionPlan` (`plan.rs`).
6. In the cleanup a skipped step is finished like any other: the step
   after it is not *inactive* because of it (F3).

**Files.** `crates/apiforge-runner/src/engine.rs` (`gate`, `Gate`),
`plan.rs`; `crates/apiforge-core/src/run.rs` (the doc comments of
`is_unavailable` and `NotRunCause::BlockedBy`; no variant is added, the
report's shape does not change); `crates/apiforge-runner/tests/run01.rs`
(new), `run_corpus.rs` (divergences); `crates/apiforge-cli/tests/run.rs`;
`docs/language/reference.md`; `docs/decisions/0015-run-after-an-error.md`
(new: the nine rules of LANG-00 section 1; number 0015 is reserved for this
ticket).

**The reference.** One new section, `## When a step fails`, after
*Cleanup*, holds the rules. Elsewhere only the sentences that contradict it
change: the `on_error` bullet of *File shape*, "When that last step failed
with an error or was blocked, the statement does not run" in *Branches*,
and the cleanup's sentence on a missing input if it implies the rest is
skipped. LANG-13 rewrites *Start and ordering* and the `@after` bullets of
*Branches*; keeping this ticket's text in its own section keeps the two
apart.

**Test sources.** `run01.rs` follows `run04.rs` (`.afs` text lowered in the
demo shop project). The text is written in the language `main` reads when
the branch is pushed: with `@after` before LANG-13 merges, without it
after. Whichever of the two merges second merges `main` first and deletes
or leaves out those lines; nothing else in the tests depends on the
spelling.

**Corpus.** `fixtures/runs/*.json` is generated and frozen; a changed
expectation is a divergence recorded in `run_corpus.rs`, never an edit of
the fixture (`fixtures/README.md`).
