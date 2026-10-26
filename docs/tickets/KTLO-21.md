# KTLO-21: A reader still sees a live writer right after the writer is dropped

- Size: S estimated, **M measured** · Owner review: 20 min · Risk: none
- State: Awaiting acceptance (issue #49)
- Analysis model: Fable (ticket), Opus 5 (cause) · Implementation model: Opus 5

## Goal

`project::tests::readers_can_probe_for_a_live_writer_without_creating_its_lock`
passes or fails on the same commit depending on the run. A red CI run that
means nothing makes every review slower, because a failure can no longer be
read as a regression.

## What the owner will see

- New: nothing.
- Changed: `ProjectStore::has_live_writer` waits out a lock that is about to be
  released before it reports a writer, so it can now take up to 200 ms to
  answer `true` — the same wait `ProjectStore::open_exclusive` already took
  before reporting another writer. Answering `false` is as fast as before. No
  caller in the application uses the probe yet.
- Removed, hidden, migrated or default behaviour: nothing.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. The cause is shown by a reproduction that distinguishes it from load alone:
   with another thread spawning processes the probe reports a released writer
   in a measurable fraction of rounds, and with no spawning it never does
   (*Cause*).
2. The probe never reports a writer that `open_exclusive` would then be able to
   take: both wait out the same window on the same lock, so the two cannot
   contradict each other.
3. A store that holds the writer lock itself answers `true` without touching
   the filesystem and without waiting.
4. The acceptance test for criteria 1 and 2 fails before the fix in every run
   and passes after it; both tests run in the ordinary suite rather than behind
   `#[ignore]`. The test for criterion 3 passes before the fix as well, because
   its purpose is to bound the wait the fix introduces.
5. The verification commands of the development guide pass.

## Cause

Not the release path. `drop` releases the advisory lock synchronously when the
last `File` for it is closed; what outlives the drop is a *second* descriptor
for the same lock in another process.

`Command::spawn` forks this process, and the child holds a duplicate of every
open descriptor — the writer's `project.lock` among them — until its `exec`,
where `O_CLOEXEC` closes it. The lock is held by an open file description, so
the duplicate keeps it alive. A thread that drops its writer inside that window
has released its own descriptor and still cannot see the lock disappear.

The storage test binary spawns processes from several tests
(`project::fault_tests` at `fault_tests.rs:456` and `:496`, the second once per
persistence boundary, and `session::tests` at `tests.rs:290`). Under
`--test-threads` those forks overlap other tests' writers, which is why the
failure follows the load of a run and not the commit: the same commit
`adcdb16` failed on the `push` run and passed on the `pull_request` run.

Measured on this branch, 4 000 rounds of open · probe · drop · probe against a
private temporary folder:

| Another thread spawning processes | Probes that saw a released writer |
| --- | --- |
| yes (6 573 spawns during the run) | 328 of 4 000 |
| no | 0 of 4 000 |

`open_exclusive` already knew this race and already waited it out — 20 attempts
10 ms apart, with the inherited descriptor named in its comment. `has_live_writer`
did not, and reported the first `WouldBlock` as a live writer. The two probes
therefore disagreed about the same lock: the probe could report a live writer
while `open_exclusive`, one line later in the same test, took the lock without
trouble.

Ruled out: the lock file being unlinked late (it is never unlinked by `drop` —
`native_writer_lock_is_exclusive_and_killing_it_releases_ownership_without_replacing_the_lock_file`
asserts the file survives); a writer surviving in `Arc<Owner>` past the drop
(the test holds the only clone); and a shared temporary folder (each round has
its own).

## Decision

**The probe confirms a would-block; `drop` is left alone.** The window is kept
alive by a descriptor in another process, so the dropping thread has nothing
left to release and could not make the release observable even in principle.

Both probes now wait out the same window through one helper, so the answer of
`has_live_writer` and the outcome of `open_exclusive` cannot contradict each
other. The agent's decision (test strategy and internal architecture); it is
recorded here because it changes the timing of a public method.

Accepted with the decision: a probe against a genuinely live writer now pays
the confirmation window before answering `true`. Nothing in the application
calls the probe yet; a future caller that polls it belongs on the storage
worker, as every other folder API already does.

## Not in scope

The `#[ignore]`d tests of other tickets; `open_exclusive`'s window itself,
which is unchanged in length.

## Actual

Changed lines: **393** (370 added, 23 removed) at `0d900c0`, over the 300 of
the S class and inside the 800 of M, so the ticket is not split. Measured at a
named commit because the figure cannot count the paragraph that states it; the
correction that put this number here is the only change after it.

| Part | Changed lines |
| --- | --- |
| The fix (`src/project/transaction.rs`) | 70 (47 added, 23 removed) |
| Acceptance tests (`tests/acceptance.rs`) | 97 |
| This ticket | 121 |
| QA report | 104 |
| Backlog row | 1 |

The estimate held for the work itself — the change to the code is 47 added
lines against 23 removed, and the writing was what grew. The size rule counts
tests and docs, so the measurement is recorded as M rather than argued down:
what the owner reviews really is longer than an S ticket.
