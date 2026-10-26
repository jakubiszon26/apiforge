# KTLO-15: A foreign write is sometimes not seen as a conflict on Windows — QA report

- Branch and commit: `fix/ktlo-15-foreign-write-conflict`; acceptance test
  `30d8db6`, fix `a6eb766`
- Built with: `cargo test -p apiforge-desktop --locked` (debug test binary);
  no release binary, because no product code changed
- Compared against mock-up: not applicable, no interface change
- Ticket: [KTLO-15](../../../tickets/KTLO-15.md)

## What changed

Only test code. The test-only `Watcher::finish`, used by the `follow_disk`
helper, now checks the folder after the test's own write instead of applying
whichever check happened to be running. The application's change detection
(length and SHA-256 of each file, once a second) is unchanged.

## Linux

Machine: 3 cores, six shell busy loops running for the whole series.

### Scenario 1: the acceptance test fails before the fix, for the right reason

1. Check out `30d8db6`.
2. `cargo test -p apiforge-desktop --locked folder::watcher`.

Expected: `finishing_checks_writes_made_while_a_slow_check_was_running` fails
because the stale check's answer was applied and nothing was reported.
Observed: as expected:

```text
assertion `left == right` failed: the write made while the slow check ran is followed
  left: None
 right: Some(["142f7558-0f40-4932-af9e-ffa2924f34c0"])
```

### Scenario 2: the flaky test, before the fix, under load

1. Build the test binary at `origin/main` (`bccd252`).
2. Run
   `folder::tests::acceptance_tests::resolving_one_documents_conflict_leaves_another_documents_unsaved_work_and_history`
   200 times under load.
3. Run it again with a diagnostic print in `follow_disk` that says whether a
   check was already running when it was called.

Expected: occasional failures with the CI message.
Observed: 0 of 200 failed without the print. With the print, 1 of 200 failed;
a check was already running in 9 of the 200, and the other 8 were rescued by
a second check that `settle()` happened to wait out.
The failing run printed `busy before rescan: true` and failed with
"unsaved work against a foreign write is a conflict", the CI failure.

### Scenario 3: the flaky test, after the fix, under load

1. Build the test binary at `a6eb766`.
2. Run the same test 200 times under the same load.

Expected: no failure.
Observed: `runs=200 failed=0`.

### Scenario 4: the verification commands

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked
-- -D warnings`; `cargo test -p apiforge-storage --locked` (169 passed,
2 ignored); `cargo test -p apiforge-desktop --locked` (335 passed,
16 ignored). Observed: all pass.

## Windows

Not run locally: no Windows machine here. The evidence is the CI run of the
pull request on `windows-2022`; one green run cannot show that an
intermittent failure is gone, so the owner may want to re-run the job a few
times.

## Not verified

- The `Busy` path of the fix (a check that meets the recovery write in
  progress is repeated): reasoned from the code, not observed failing and not
  covered by a dedicated test.
- That Windows failed for the same reason as the reproduced Linux failure:
  the CI logs show only the assertion; the mechanism is the one the code
  allows on every platform.
- The regression smoke list: not run, because no product code changed and the
  release binary is identical to `main`'s.
