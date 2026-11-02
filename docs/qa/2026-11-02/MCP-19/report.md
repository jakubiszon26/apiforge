# MCP-19: On Windows a commit beside a reader is no longer refused — QA report

- Branch: `docs/mcp-19-windows-recovery` (PR #203), with `main` at `3dfc3f9`
  (REL-02, KTLO-32) merged in `79021e1`. Acceptance tests `d9165cf`;
  implementation `d1f27ff`. The ad hoc loop workflow of criterion 6 was
  added in `9f58c18`, `47e72fa` and `cd9477f` and removed with this report
  (`cd3f8f4`). Review round 1 (cubic on `cd3f8f4`, ten comments) in
  `4e5a60e`: start handshakes in three tests and documentation, no
  assertion changed; round 2 (cubic on `4e5a60e`: five repeats of round 1,
  one new) in the commit that adds this line: one documentation sentence.
- Built with: Rust 1.99.0, `cargo test --locked` on the GitHub runners
  `windows-2022` and `ubuntu-24.04`, and locally on Linux. "Before" is the
  acceptance commit `d9165cf` (the storage code of `main` at `3dfc3f9`);
  "after" is `d1f27ff` and later heads, whose code is the same.
- Mock-up: not applicable; nothing in the window changes. Issue:
  [#191](https://github.com/jakubiszon26/apiforge/issues/191). Ticket:
  [`docs/tickets/MCP-19.md`](../../../tickets/MCP-19.md). No owner decision.
- Analysis model: Fable. Implementation model: Claude Opus 5.5.

## What changed

All in `apiforge-storage`, as ADR 0011 now records (*Lock, versions and
stable reads*, amended 2026-11-02):

1. Every replaced file — each target of a folder transaction, `head.json`,
   `holder.json`, the run journal, the legacy workspace — is installed with
   `std::fs::rename` (`file::install`) instead of `NamedTempFile::persist`.
   New files keep `persist_noclobber`.
2. On Windows only, a bounded wait (`file::while_held`, every 20 ms, at
   most one second): the replace on *Access is denied* or a sharing
   violation, and a directory removal on *directory not empty* while the
   directory lists only names its own transaction removed. Past the bound
   the error stands as before. On Linux nothing waits.
3. `ProjectStore::scan_tree` takes the store's worker on a writer store, so
   the window's check waits behind the window's own save or delete instead
   of reading the files it replaces or removes. A read-only store's scan is
   unchanged.

## Criteria and their evidence

| # | Criterion | Tests | Windows before (`d9165cf`) | After |
| --- | --- | --- | --- | --- |
| 1 | A commit replaces a target that a Rust reader holds open; the reader keeps the old bytes; a stable read sees the new generation; no transaction left active | `project::replace_tests::mcp19_criterion_1_a_commit_replaces_a_target_a_reader_holds_open` | **Failed**: *the commit beside a reader failed after 1.0948ms: Cannot access …\scenarios\main.afs: Access is denied. (os error 5)* (run 41472473910) | Passed on both runners |
| 2 | A reader that shares reading only: released 200 ms after the first attempt, the commit succeeds within the bound; held 2 s, the commit fails after activation, `recover()` finishes it | `…mcp19_criterion_2_a_commit_waits_for_a_reader_that_does_not_share_delete`, `…mcp19_criterion_2_a_reader_held_beyond_the_bound_leaves_recoverable_intent` (Windows only) | **Failed**: *the commit failed 1.0008ms after its first attempt: … Access is denied*; *the commit gave up 1.0934ms after its first attempt, before the bound* (run 41472473910) | Passed on Windows |
| 3 | A writer's scan waits behind its commit and sees its generation; a read-only store's is not held. A folder deleted while the window's check is forced to run: the check waits, sees the folder gone; the tab closes | `…mcp19_criterion_3_a_scan_waits_behind_this_writers_commit`; desktop `folder::tests::acceptance_tests::mcp19::mcp19_criterion_3_a_folder_deleted_beside_the_windows_check_closes_its_tab` | **Failed** on both platforms: *the writer's scan answered during its own commit: Ok(Err(Busy))* (run 41472473910); *the check answered during the delete: … The folder changed while reading* (run 41464019711, both runners) | Passed on both runners |
| 4 | Ten agent writes beside a window checking continuously: every one `wrote: true`, no `recovery_required`, the window follows each | desktop `folder::tests::acceptance_tests::mcp10::mcp19_criterion_4_ten_agent_writes_beside_a_window_that_checks_continuously`; `mcp10_criterion_3` unchanged | Passed once in run 41464019711; **failed once in 20** in the loop (below) | Passed in every run and 60 of 60 loop rounds per runner |
| 5 | ADR 0011 amended; *native Windows durability unverified* stays | — | — | `docs/decisions/0011-project-transactions.md` |
| 6 | The loop: 20 of 20 on each runner after; before, a failure or the attempts named | the loop below | 1 of 20 rounds failed on Windows | 0 of 20 in each of three loops, per runner |
| 7 | `fault_tests.rs`, `transaction_tests.rs` pass unchanged; no assertion weakened | the storage suite | — | Passed; neither file is changed by this branch |
| 8 | The verification commands of the development guide | — | — | Passed locally on Linux and in CI |

## Windows

### Scenario 1: the storage acceptance tests at the acceptance commit

The pull request's CI on `d9165cf` (run 41464019711) stopped at the first
failing test binary, the desktop's, before the storage tests ran, so the
ad hoc workflow ran them alone on `windows-2022` at that commit: job
*Storage acceptance at d9165cf, before the fix* of run 41472473910,
`cargo test -p apiforge-storage --lib --locked -- mcp19 --test-threads=1`.

Expected: all four fail: the replace refused with *Access is denied* about
a millisecond after the reader took the file, beside a reader with Rust's
default share mode (criterion 1) and beside one sharing reading only
(criterion 2 within the bound); the commit beyond the bound giving up at
once instead of after one second; the writer's scan answering `Busy` during
the commit instead of waiting.
Observed: as expected, *0 passed; 4 failed*, with the messages quoted in the
table. This is the cause the analysis named: `MoveFileExW` refuses the
replace while any handle is open, even one that shares delete.

### Scenario 2: the same tests and the suite after the fix

CI of `d1f27ff` (push run 41465821527, pull_request run 41465834540), of
`47e72fa` (41466533907), of `cd9477f` (41472473949, 41472482827), of
`cd3f8f4` (41477673452, 41477687100) and of review round 1's `4e5a60e`
(41483273288, 41483284555): every job green. In 41465821527 on `windows-2022` all six MCP-19 tests passed:
criterion 1, both criterion 2 cases (within the bound, and the 2 s reader
with recovery after it let go), criterion 3's storage and desktop tests,
criterion 4. The folder-project step of each run (release binary, demo shop
folder, managed startup) passed on both runners.

### Scenario 3: the loop (criterion 6)

KTLO-33's `flake-loop.yml` has not merged, so its steps ran ad hoc on this
branch (`.github/workflows/mcp-19-loop.yml`, now removed): the desktop test
binary built once, then 20 rounds of
`cargo test -p apiforge-desktop --bin apiforge-desktop --locked -- --exact`
over five tests — `mcp10_criterion_3…`,
`deleting_a_folder_closes_its_tabs_and_the_trash_restores_unsaved_work`,
`mcp08a::the_holders_save_is_shown_within_two_seconds_where_the_person_was`,
`mcp07::a_transaction_left_by_the_holder_opens_the_recovery_view` and
criterion 4's test — every round run, failures counted, logs uploaded as
`mcp-19-loop-<os>`. Each round's log shows *running 5 tests*.

| Run | Commit | windows-2022 | ubuntu-24.04 |
| --- | --- | --- | --- |
| 41464039984 | `9f58c18` (before) | **1 of 20 failed** | 0 of 20 failed |
| 41465821357 | `d1f27ff` (after) | 0 of 20 failed | 0 of 20 failed |
| 41466533791 | `47e72fa` (after) | 0 of 20 failed | 0 of 20 failed |
| 41472473910 | `cd9477f` (after) | 0 of 20 failed | 0 of 20 failed |

Expected before: at least one failure of `mcp10_criterion_3`,
`mcp08a::the_holders_save…` or `deleting_a_folder…` on Windows.
Observed before: none of those three failed in 20 rounds. The one failure,
round 9, was criterion 4's new test: the agent's `read_source` before its
next write answered `project_unreadable` — *Cannot access
…\.apiforge\local\head.json: Access is denied. (os error 5)* — while the
window checked continuously. It is a refusal on the reader's side of the
same file, not the writer's replace the analysis describes; how the
unfixed primitive led to it is not established (see *Not verified*).
Observed after: 0 failures in 60 rounds on each runner, and none in the ten
CI runs after the fix.

## Linux

The verification commands of the development guide on `d1f27ff`, locally:
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
--locked -- -D warnings`, `cargo test --workspace --all-targets --locked`
(61 test binaries, all *ok*) and `cargo run -p apiforge-convert-v1 --locked
-- --fixtures --check`: all passed. Before the fix the two criterion 3 tests
failed on Linux as on Windows; criterion 1 passed, since Linux never
refused the replace. The storage suite, `fault_tests.rs` and
`transaction_tests.rs` included, passes; its existing
`a_scan_during_this_owners_own_commit_is_busy_not_an_interrupted_transaction`
calls `scan_tree` inside a commit on the committing thread, which is why
the scan does not wait for a worker its own thread holds.

## Not verified

- The mechanism of the one before-fix loop failure (an *Access is denied*
  opening `head.json` to read it): not reproduced, so not explained. It did
  not recur after the fix in 60 Windows loop rounds or ten CI runs.
- A third program holding a project file without sharing delete, outside
  the tests (an editor, antivirus): only the test's
  `share_mode(FILE_SHARE_READ)` reader shows the bound and the recoverable
  intent past it.
- A directory removal that waits for a delete-pending name (part 2's
  `ERROR_DIR_NOT_EMPTY` case): no test drives a reader that keeps a removed
  file's name; criterion 3's desktop test removes the folder behind the
  worker, which no longer races, and the bound for this case is not
  exercised on a runner.
- File systems without POSIX rename (ReFS, network shares), and Windows
  before 10 1607: not run; the bound then applies to every open handle.
- Directory-metadata durability on Windows: ADR 0011's open release gate,
  unchanged.
- The owner's local Windows build: not run.
- The regression smoke list: not run by hand; nothing in the window
  changes. The CI folder-project step exercised the release binary's
  folder saves on both runners in every run after the fix.
