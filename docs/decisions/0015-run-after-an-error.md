# ADR 0015: What a run does after an error

- Status: accepted 2026-11-17 (LANG-00 section 1, owner decision 1 of the
  LANG quiz in #388, consequences 1 and 2 confirmed at approval)
- Date: 2026-11-20
- Scope: `apiforge-runner` (`engine.rs` gate, `plan.rs`), the reports of
  `apiforge-core` (`run.rs`, comments only), the language reference
- Related: [Start reachability](0003-start-reachability.md),
  [scenario language](0005-scenario-language.md); tickets RUN-01 (#126),
  RUN-04 (#418), LANG-14 (#390)

## Context

An ordering edge (`@after(a)`, and after LANG-13 the order of the lines)
used to mean "after `a` succeeded". Under `on_error continue` a step that
followed a failed step was *blocked* even when it read nothing from it, and
the block travelled along the whole chain: one failed payment held back
every later step of the scenario. In the cleanup, a step skipped for a
missing value took every step after it along. The engine already told an
order edge from a data edge and already judged single values in the
cleanup; the main flow did not.

## Decision

The nine rules of LANG-00 section 1:

1. **A step is finished** when it passed, failed, errored or was blocked.
   An ordering edge waits for its source to finish, not to succeed.
2. **`on_error stop`**, the default: the first failed check, error or
   blocked step stops the main flow, as before (RUN-04). Nothing changes.
3. **`on_error continue`**: a step starts when its predecessor has finished
   and every value it reads exists. A value exists when the statement that
   makes it produced that output (`status`, `headers`, `body`, an export,
   `passed`, `value`), whatever that statement's result: a request that got
   an answer has its outputs at 500.
4. A step that reads a value which does not exist is **blocked** and names
   the statement that should have made it (`BlockedBy { step }`). Blocked
   travels only through values. A false Data condition keeps its own cause
   (`ConditionUnmet`): it guards the data of the steps that read it.
5. An answer without the field read (`a.body.id` at 500) is the reading
   step's own error, *Missing value*; the step after it runs.
6. **An `if`** that could not decide has no `passed`: it is blocked when its
   condition reads a value that does not exist, and ends as an error when
   its condition errors. Either way every statement standing in its braces,
   at any depth, is blocked by it (the outermost such `if`); the step after
   the `if` runs. A branch not taken is skipped, which is not a failure and
   blocks nothing.
7. **A component call** that fails inside finishes as an error; an export it
   computed before the failure exists and can be read, in the main flow as
   in the cleanup; the others do not.
8. **The cleanup** runs whatever happened. A cleanup step whose value does
   not exist, or whose field is missing, is skipped, and the cleanup steps
   after it run: a skipped step has finished like any other.
9. A run with a failed, errored or blocked step is *failed*.

How the runner applies them:

- The main flow under `stop` keeps the gate it had (`gate_until_stop`):
  before the first failure every source is passed or off the path, and
  after it the run stops. The main flow under `continue` and every cleanup
  use one gate (`gate_after_finished`), in this order: off the path
  (unreachable from Start, or every ordering source a branch not taken or a
  step skipped as outside the path, or, in the main flow only, no ordering
  source and a value read from a step off the path) → skipped; inside the braces of an
  undecided `if` → blocked by it; a source never run, or an output read that
  does not exist → blocked by its statement; a false Data condition read in
  the main flow → its own cause; in the cleanup, a path read that does not
  resolve → skipped.
- A statement stands in the braces of branch `b` of `x` when it follows
  `x.b` and everything else it follows stands in the same braces. The
  runner reads the braces from the ordering edges itself
  (`ExecutionPlan::enclosing`), the same reading as `apiforge-script`'s
  `branches.rs`, so the runner does not depend on the script crate.
- The report keeps its shape: no status, cause or field is added.

## Consequences

- Under `continue` a step that reads nothing from a failed step now runs —
  `ship` after a failed `pay`, unless `ship` reads the payment. A scenario
  that must not go on uses `on_error stop` or an `if`. There is no syntax
  for "after success only".
- A step that only follows a false Data condition runs; only the steps that
  read it are held back.
- LANG-14's criterion 2 (the step after an `if` does not run when the branch
  ended in an error) is replaced by rule 1 under `continue`.
- The recorded run corpus diverges where a step was blocked by order only:
  `join` in `merge-error`, `next` in `components` (rule 7), `read` and the
  cleanup step that reads it in `cleanup-rules` and `cleanup-with-start`.
  Each divergence is listed in `run_corpus.rs`; `error-policy-stop` has
  none.
- A cleanup step that reads a step the run did not reach reads "Unavailable
  data…" instead of "Step outside the active execution path."; its cause,
  `BlockedBy`, is unchanged.
