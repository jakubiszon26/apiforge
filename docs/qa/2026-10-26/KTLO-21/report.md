# KTLO-21: A reader still sees a live writer right after the writer is dropped — QA report

- Branch: `fix/ktlo-21-stale-writer-probe`; ticket `67aaf1e`, acceptance tests
  `95bea20`, test correction `8105606`, fix `acf1368`.
- Mock-up: not applicable, no interface change. Ticket:
  [KTLO-21](../../../tickets/KTLO-21.md)
- Built with: `cargo test --locked` (debug). No release build and no captures:
  nothing visible in the application changes and no caller in the desktop uses
  the probe. Agreed with the owner on the issue.
- Machine: Linux, 3 cores, shared throughout with another session building the
  workspace, so the timings below are upper bounds.

## Scenarios

### 1. The cause, measured

4 000 rounds of `open_exclusive` · probe · `drop` · probe against a private
temporary folder, once with a thread spawning `/bin/true` in a loop and once
without:

| Another thread spawning processes | Probes that saw a released writer |
| --- | --- |
| yes (6 573 spawns during the run) | 328 of 4 000 |
| no | 0 of 4 000 |

The probe is wrong only while another thread forks this process. That is the
mechanism: the child keeps a duplicate of the writer's lock descriptor until
its `exec`, and the lock belongs to the open file description, so the duplicate
holds it after the writer's own `File` is closed. `drop` is not late — it has
nothing left to release.

### 2. The acceptance test before the fix

`cargo test -p apiforge-storage --test acceptance` at `8105606`:

```text
a dropped writer was still reported as live in round 13 of 600,
alongside 53 process spawns
```

Run 15 times on the commit before the fix: **15 failed, 0 passed**, the stale
answer arriving between round 1 and round 34 of 600. The test stops at the
first one, so each run took about 0.1 s.

### 3. The acceptance test after the fix

At `acf1368`, run 20 times: **20 passed, 0 failed**. One run takes 1.9 s for
600 rounds.

That total is itself evidence that the retry exits early: if each of the
roughly 50 rounds that meet an inherited descriptor waited the whole window,
the test would take about 10 s. It takes 1.9 s, so a fork's residue is cleared
in a few milliseconds and only a writer that is really there costs the window.

### 4. The test named in the issue

`project::tests::readers_can_probe_for_a_live_writer_without_creating_its_lock`
passes. It is not the evidence, and the report does not claim it is: 12 runs of
the whole storage suite at `--test-threads 16` **before** the fix were green as
well (48 s each), so that loop never caught the original flake on this machine.
The acceptance test of scenario 2 is what reproduces it, and it is now part of
the suite.

### 5. What the fix costs

`has_live_writer` now waits out a would-block over the same window
`open_exclusive` already used — 20 retries 10 ms apart, so at most 200 ms — and
pays it only when a writer really holds the lock. A session that holds the lock
itself answers without touching the folder, which the second acceptance test
bounds at 50 ms. Answering `false` is unchanged: one `open`, one
`try_lock_shared`, no sleep.

### 6. Verification

- `cargo fmt --all -- --check`: clean.
- `cargo clippy -p apiforge-storage --all-targets --locked -- -D warnings`:
  `Finished`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: `Finished` in 1 min 28 s, no warning.
- `cargo test --workspace --all-targets --locked`: 1 001 passed, 0 failed, 21
  ignored over 52 test binaries, cargo's own exit status 0. The desktop suite
  353 passed and 18 ignored in 125.6 s, the storage suite 172 passed and 2
  ignored in 71.0 s, the new `tests/acceptance.rs` 2 passed. No test failed
  under the load of the parallel session, including the timing-sensitive
  desktop tests.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit
  status 0, no diagnostics.

## Not verified

- Windows: the mechanism is a `fork` artefact and cannot occur there, because
  `CreateProcess` inherits only handles marked inheritable and the lock file is
  not one. The acceptance test for it is `#[cfg(unix)]`; the second test runs on
  both. No Windows run was made, so the ticket is not accepted for Windows on
  this report.
- That the two CI runs named in the issue were this race: their logs are gone.
  The mechanism and the fix are shown; a captured recurrence is not.
- Regression smoke list: not run, agreed with the owner. Nothing visible in the
  application changes and the probe has no caller in the desktop.
- The budget and fuzzer commands of the development guide: not run, no change
  to layout, the wire router, analysis, the code view or the scenario language.

## Regression smoke list

Not run for this ticket; see *Not verified*.
