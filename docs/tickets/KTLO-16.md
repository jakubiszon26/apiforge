# KTLO-16: The Linux folder-projects CI step sometimes hangs until the job's 30-minute limit

- Size: S · Owner review: 10 min · Risk: none
- State: Awaiting acceptance (issue #31)
- Analysis model: Fable (ticket), Opus 5.5 (cause) · Implementation model: Opus 5.5

## Goal

A push to `main` is green on Linux every time; when the step does hang it
fails within its own timeout with its log intact.

## What the owner will see

- New: in CI, the step prints its process group first, and a phase that
  overruns prints its log before it is stopped.
- Changed: `--screenshot` on a window with no document after 30 s (for
  example `--folder` on a missing folder) exits with `Capture failed: the
  window had no document after 30 s` instead of staying open. Developer flag
  only; normal start-up is unchanged.
- Removed, hidden, migrated or default behaviour: nothing.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. The cause is shown locally and by the step's diagnostics in CI: *Cause*.
2. Every phase has a hard deadline (60 s to readiness for managed startup,
   45 s per capture, by the clock); an overrun prints the phase's log, sends
   TERM to its group and KILL 10 s later. Worst case without a failure is
   about 9 minutes, under the 11-minute step timeout.
3. `--screenshot` without a document is fixed in the binary, test first
   (`f686461`), fix `aea1b24`.
4. The step passes on the pull request and on the sibling push run.

## Cause

The step started managed startup with `setsid xvfb-run … &` and at once read
its process group with `ps -o pgid= -p $run_pid`. Before the child has
called `setsid()`, that read returns the step shell's own group, and the
step then sent `kill -TERM -- -<that group>`. On the GitHub Linux runner
that group is the runner's: the new step's diagnostics in run 40345206306
show the step shell in group 2054 with `Runner.Worker`, `Runner.Listener`
and the hosted-compute agent. The TERM stopped the runner itself while the
application, already in its own session, kept running. That matches both
cancelled push runs: no step output, the step's timeout ineffective because
nothing was left to enforce it, the job ended by the 30-minute limit.

Locally the read raced in 4 of 1 200 tries, and forcing it reproduced the
symptom (step and stand-in runner gone, application still running). The old
step did not hit the race in 60 looped runs (178 reads), so the link to the
two hangs is this mechanism, not a captured recurrence.

Ruled out: the binary ignoring SIGTERM, and `setsid` forking so that `$!`
names the wrong process. Fix: each phase's group ID is its own PID.

## Not in scope

The Windows step (no process groups); the storage test named in the issue.

## Actual

Changed lines: 299 (`git diff --stat origin/main...HEAD`). One round.
