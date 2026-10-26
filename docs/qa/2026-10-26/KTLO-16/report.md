# KTLO-16: The Linux folder-projects CI step hangs — QA report

- Branch: `fix/ktlo-16-ci-step-deadline`; acceptance test `f686461`, binary
  fix `aea1b24`, workflow `fd7483a`. Old binary: release build of `38c6b41`.
- Mock-up: not applicable. Ticket: [KTLO-16](../../../tickets/KTLO-16.md)
- Machine: Linux, 3 cores, load average 7 to 18 throughout. The step is the
  workflow's `run:` block under Xvfb and Lavapipe, run by a stand-in runner
  that shares the step's process group and enforces the step timeout.

## Scenarios

1. **Acceptance test before the fix.** `cargo test -p apiforge-desktop
   capture::` at `f686461`: the new test fails with `panicked at
   crates/apiforge-desktop/src/capture.rs:117:9: None`. At `aea1b24` both
   capture tests pass.
2. **`--screenshot --folder /nonexistent/folder`**, 40 s `timeout`. Old
   binary: killed at 41 s (exit 124), no PNG. New: `Capture failed: the window
   had no document after 30 s`, exit 1 at 30 s.
3. **The group read races.** `setsid sleep 2 & ps -o pgid= -p $!`, four
   series of 300: 2, 1, 0 and 1 reads returned the shell's own group.
4. **The race in the old step.** The read replaced by the step's own group,
   the value a racing read returns: step and stand-in runner ended with no
   further output (no `RUNNER: step exited`); the application (PID 762042)
   was still running afterwards.
5. **Old and new step in a loop:**

| Step | Runs | Passed | Runner survived | Leftovers | Reads that raced |
| --- | --- | --- | --- | --- | --- |
| Old, traced | 20 | 19 | 20 | 0 ¹ | 0 of 58 |
| Old, untraced | 40 | 40 | 40 | 0 | 0 of 120 |
| New, traced | 20 | 20 | 20 | 0 | none made |

¹ Runs 1 and 19 were disturbed by the harness and by scenario 4 running in
parallel. No hit in 178 reads is expected at about 1 in 300, so the loop
alone does not tie the CI hangs to the race. New step: 27 to 49 s per run.

6. **New step deadlines**, with stand-in binaries. One that ignores TERM and
   writes nothing: `Phase created was not ready within 60 s`, log printed,
   `… outlived TERM by 10 s; sending KILL`, exit 1 at 70 s, no leftover.
   One that exits at once: `Phase created exited before it was ready`, 1 s.
7. **Verification.** `cargo fmt --all -- --check` clean; `cargo clippy
   --workspace --all-targets --locked -- -D warnings` `Finished`. `cargo test
   -p apiforge-desktop --locked`, twice: 336 passed, 1 failed, 16 ignored; a
   different timing test failed each time under load (`a_large_source_…`,
   `failed_recovery_keeps_…`). Alone they pass 3 of 3 and 6 of 6.

## CI

Push run [40345206306](https://github.com/jakubiszon26/apiforge/actions/runs/40345206306)
of `fd7483a`: green on both runners, Linux step 14 s. Its first lines show
the step shell in the runner's own process group, the group a racing read
handed to `kill -TERM`:

```text
Step shell 22224, process group 2054 (PID PPID PGID SID COMMAND):
   2054       1    2054    2054 hosted-compute-
   2112    2054    2054    2054 Runner.Listener
   2132    2112    2054    2054 Runner.Worker
  22224    2132    2054    2054 bash
```

## Not verified

- That the two cancelled runs were this race: their step logs are lost. The
  mechanism is shown; a captured recurrence is not.
- Windows: step unchanged; the evidence is the CI run.
- Regression smoke list: not run; only the developer `--screenshot` flag
  changed.
