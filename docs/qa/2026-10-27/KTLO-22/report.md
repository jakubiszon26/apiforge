# KTLO-22: A Continue in the comment-loss prompt survives the watcher's exclusive check — QA report

- Branch: `fix/ktlo-22-publish-wait`; ticket `269167b`, `main` merged in
  `16c8c3b`, acceptance tests `3426673`, fix `e91dec1`.
- Mock-up: not applicable, no interface change. Ticket:
  [KTLO-22](../../../tickets/KTLO-22.md)
- Built with: `cargo test --locked` (debug). No release build and no captures:
  nothing on screen changes; what changes is that a decision already shown on
  screen is no longer forgotten.
- Machine: Linux, 3 cores, shared throughout with other sessions building the
  workspace (load average 10–19), so timings are upper bounds.

## Scenarios

### 1. The decision is lost without any timing (criterion 1)

`crates/apiforge-project/tests/acceptance.rs`, two tests: a storage snapshot is
taken in exclusive mode, the live session then accepts comment loss (or moves
the selection in a document and in a text file), and the job completes with a
candidate it did not change.

| Commit | `a_comment_decision_survives_…` | `a_selection_survives_…` |
| --- | --- | --- |
| `3426673`, before the fix | fails: `publish` returns `CommentsWouldBeLost { comments: 1 }` | fails: the selection is back at `0..0` |
| `e91dec1`, after the fix | passes | passes |

The text-file half of the selection test was checked separately: with the fix
applied to documents only, it fails with the text file's selection at `0..0`.

### 2. The loss through the window (criterion 2)

`folder::tests::continue_in_the_comment_prompt_publishes_while_an_exclusive_check_is_in_flight`:
an exclusive job that changes nothing, labelled as the watcher labels its own
("Checking external changes…"), is held on the storage worker while *Continue*
is pressed in the comment-loss prompt; then it is released.

- Before the fix (`3426673`): **10 of 10** runs failed at "the prompt does not
  come back" — the decision was wiped when the job completed, the publication
  queued behind it refused, and the prompt was shown again.
- After the fix (`e91dec1`): passes; the manifest gains one revision, the
  notice reads "Published revision …", no prompt, and the working text keeps
  its comment.

### 3. The test named in the issue, looped (criterion 3)

`publishing_from_the_header_requires_a_comment_decision_and_cancel_keeps_source`
is unchanged. Each round runs the test binary alone:

```bash
# loop.sh BIN ROUNDS LABEL [cpu]: runs
#   $BIN --exact folder::tests::publishing_from_the_header_requires_a_comment_decision_and_cancel_keeps_source
# ROUNDS times (under `taskset -c cpu` when given) and counts runs without "1 passed".
taskset -c 2 sh -c 'while :; do :; done' &   # two CPU burners on the test's core
taskset -c 2 sh -c 'while :; do :; done' &
loop.sh target/debug/deps/apiforge_desktop-… 200 after-load 2
loop.sh target/debug/deps/apiforge_desktop-… 50 after-noload
```

| Code | Load | Rounds failed |
| --- | --- | --- |
| before the fix (first round, `269167b`) | pinned to one core with two burners | **4 of 200** |
| before the fix (first round) | none added | **3 of 200** |
| before the fix plus the refuted waits (first round) | pinned with two burners | **2 of 200** |
| after the fix (`e91dec1`) | pinned to one core with two burners | **0 of 200** (11 min 21 s) |
| after the fix (`e91dec1`) | none added | **0 of 50** (44 s) |

Every failure before the fix was the CI symptom: `left: 3, right: 4` on the
revision count after *Continue*.

### 4. A job that changed a file still replaces it whole (criterion 4)

The thirteen `complete_storage` tests of `crates/apiforge-project/tests/storage_work.rs`
pass unchanged, among them
`a_destructive_result_cannot_replace_memory_changed_since_its_start`. The fix
carries a decision only into a document whose source, fields, model, history
and conflict the job left as they were, and into a text file whose version and
held state it left as they were.

### 5. Verification (criterion 5)

At `e91dec1`:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: `Finished`,
  no warning.
- `cargo test --workspace --all-targets --locked`: **1 027 passed, 0 failed,
  23 ignored** over 52 test binaries, exit status 0. The desktop suite 374
  passed and 20 ignored in 269 s under the parallel builds.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit
  status 0.

### 6. CI

The branch's CI run on the fix commit `e91dec1`:
[40485147149](https://github.com/jakubiszon26/apiforge/actions/runs/40485147149),
**success** — `Build & Test` passed on `ubuntu-24.04` and on `windows-2022`,
and its Windows log shows all four tests above (the two of criterion 1, the
one of criterion 2 and the header test) `ok`. The later commits change only
this report and the ticket; their runs are on the pull request.

## Windows

Not run locally. The defect is not platform-specific — the watcher's
once-a-second exclusive job and `complete_storage` are the same code on both —
and Windows is where it showed (runs 40367559871 and 40410023496). The only
Windows evidence is the branch's CI run above: the whole suite passed on
`windows-2022`, the new tests and the header test among it. One green run is
not a loop, so this report does not show the flake gone on Windows, only the
fix passing there.

## Not verified

- A loop of the header test on Windows: no Windows machine; only the CI run.
- That the two CI runs named in the issue were this loss: their failure is the
  same assertion (`left: 3, right: 4`) and the reproduction gives exactly it,
  but the runs' timing is not recoverable.
- A person hitting the lost *Continue* in the application: shown through the
  window's interface in scenario 2, not in the native window.
- Regression smoke list: not run. Nothing on screen changes and the fix is one
  arm of `complete_storage`, covered by its own suite and the folder tests.
- The budget and fuzzer commands of the development guide: not run, no change
  to layout, the wire router, analysis, the code view or the scenario language.

## Regression smoke list

Not run for this ticket; see *Not verified*.
