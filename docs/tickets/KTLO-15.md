# KTLO-15: A foreign write to a scenario file is sometimes not seen as a conflict on Windows

- Size: S · Owner review: 10 min · Risk: none
- State: Awaiting acceptance (issue #25)
- Analysis model: Fable (ticket), Opus 5.5 (cause, from the code and a local
  reproduction) · Implementation model: Opus 5.5

## Goal

A push to `main` is green on Windows every time; a foreign write to an open,
dirty document is reported as a conflict reliably.

## What the owner will see

- New: nothing.
- Changed: nothing in the application. The cause was in the test helper, not
  in the product: the window does see the outside edit, at the latest on its
  next once-a-second check. Only the test runs differ: the desktop test no
  longer fails now and then on the Windows runner.
- Removed or hidden: nothing.
- Migrated or converted: nothing.
- Default behaviour that changes: none.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. The cause is established from the code and reproduced locally, not
   guessed. Done: see *Cause*.
2. The cause is in the test harness, so the harness waits for the right event
   (a check of the folder that starts after the test's write); no assertion
   is weakened. Test:
   `folder::watcher::tests::finishing_checks_writes_made_while_a_slow_check_was_running`,
   committed failing first (`30d8db6`).
3. Not applicable: the product's change detection compares length and SHA-256
   of the content, not modification times.
4. The named test fails 0 times in 200 runs under load locally; CI on the pull
   request passes on both runners.

## Cause

**How the window sees an outside change.** `folder/watcher.rs` starts an
`ExternalCheck` on a worker thread once a second, and at once after a
`rescan()` (focus regained, file operation finished). The check
(`apiforge-project/src/external.rs`) scans the tree with
`ProjectStore::scan_tree` and compares each document's `FileState`
(`bytes` and `sha256` of the content) with the state the session last saved
or loaded; a difference is read and applied, and a document with unsaved work
gets a conflict. Modification times play no part, so neither the NTFS time
resolution nor `std::fs::write` truncating before writing can hide a change:
a half-written file differs in hash too, and the next check reads it again.

**Why the test missed it.** The test helper `follow_disk`
(`folder/project_interaction_tests.rs`) did `rescan()`, one frame, then the
test-only `Watcher::finish`, which waited for *whichever check was running*
and applied it. If the once-a-second check had started before the test's
`fs::write` (the test types for several frames first), `poll` does not start
another while one runs, so `finish` applied that earlier check: it had read
the old file, reported nothing, and the assertion "unsaved work against a
foreign write is a conflict" failed. A second check started in `settle()`
usually rescued the test, but `settle()` does not wait for the watcher, so on
a slow runner it could return first. Windows makes the window wider: spawning
a thread and hashing files is slower there, so a check is more often still
running when the test writes.

A second, related way: a check that meets a transaction in progress (the
recovery write fires one second after typing) answers `Busy`, and `finish`
returned nothing for it as well. Not observed locally; covered by the same fix.

**Reproduction (Linux, 3 cores, 6 busy loops).** 0 failures in 200 runs of
the unmodified test. With a diagnostic print in `follow_disk`, 1 failure in
200 runs: a check was already running when `follow_disk` was called in 9 of
the 200, the failing run among them; in the other 8 a second check started by
`settle()` happened to finish in time. After the fix: 0 failures in 200 runs.

**Fix.** `Watcher::finish` (test-only) waits for a running check and sets its
answer aside, then checks the folder itself and applies that result,
repeating a check that answered `Busy`. `follow_disk` and the older `watch`
helper call it unchanged. No product code changed.

## Not in scope

- `settle()` not waiting for a running watcher check, and its pending-analysis
  gap (backlog, *Later*).
- Other timing-sensitive desktop tests.

## Actual

Changed lines: 230 (acceptance test 35, fix 20, QA report 83, this ticket
92), counted with `git diff --stat origin/main...HEAD`. One round.
