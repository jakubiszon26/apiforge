# KTLO-03 part 1: A refused migration is reproduced, names its project and scenario, and is its own start — QA report

- Branch: `feat/ktlo-03-migration-stop`; reproduction `8d6401a`, acceptance
  tests `737e56f`, implementation `d66d416`, `main` merged in `80aacad`,
  clippy correction `5e5e3fd`.
- Ticket: [KTLO-03](../../../tickets/KTLO-03.md), decisions 1–2 of 2026-10-26
  and 3–5 of 2026-10-27 on #5. This is the first of two parts (see the
  ticket's *What the implementation found*): the *Migration stopped* window
  is part 2, so this part changes **nothing on screen**.
- Built with: `cargo build --release -p apiforge-desktop --locked` for the
  release run; the tests with `cargo test --locked` (debug).
- Machine: Linux, 3 cores shared with other builds (load average 9–17).

## What was found

The six-project fixture reproduces the owner's refusal, **with and without
KTLO-04**, and KTLO-04 is not its cause. Migration seals every checkpoint
itself (`prepare_project`), so no migrated document lacks one. What is wrong
is the checkpoint's semantic digest: it hashes the projection *in stored
order*. A check whose request was replaced on the board stores its inputs as
`expected, actual`; the printed source reads them back as `actual, expected`.
The printer compares the two as values, finds them equal and writes the
source; the digest differs, so on reload the checkpoint does not describe the
source, the scenario is only reconciled against it, its blocks come back in
source order, and verification refuses the project. Criterion 2 is therefore
**not met**; the fix is put to the owner in the ticket.

## Scenarios

| # | Criterion | Test | Result |
| --- | --- | --- | --- |
| 1 | the synthetic six-project workspace is refused, and why | `a_six_project_workspace_is_refused_on_a_check_whose_request_was_replaced` | passes at `280d6bd` (before KTLO-04, the test copied onto that commit) and on this branch: refused on project `order-desk`, "a scenario differs after reloading" |
| 2 | with KTLO-04 the six projects migrate | `the_six_project_workspace_migrates_with_ktlo_04_merged` | **fails** (refused on Order desk); `#[ignore = "KTLO-03: …"]` since `d66d416` |
| 3 | a still-refused migration names project and scenario; originals byte-identical; nothing activated | `a_refused_migration_names_the_project_and_the_scenario_and_activates_nothing` | failed at `737e56f` (the message named neither); passes |
| 4 (first half) | `managed_start` reports the refusal as its own `Start` variant; the other causes stay `Legacy` | `ktlo03_a_refused_migration_starts_the_stop_window_not_the_workspace_window`, `ktlo03_the_other_causes_still_open_the_workspace_window` | the first failed at `737e56f` (`Legacy`); both pass |
| 6 | verification commands; migration suite unchanged | see *Verification* | pass |

### 1. The reproduction (criterion 1)

1. Build the workspace with the board's own edit operations and save it with
   the legacy `WorkspaceStore` writer: the four demo projects (Demo shop,
   tickets, blocks, workflow), then *Order desk* and *Support desk*. In Order
   desk's scenario *Order total is checked*, a request `load_order` feeds a
   comparison `total_is_paid`; the request is deleted, `reload_order` is added
   after Start and its `status` wired into the comparison's `actual`.
2. `migrate_workspace`.

Expected (from the owner's report): refused on the fifth project with
`Verification { reason: "a scenario differs after reloading" }`, no catalogue,
four projects left in staging.
Observed: exactly that — refused on `order-desk`; `migration/index.json`
lists `shop`, `tickets`, `blocks-poc`, `workflow-poc`; no `projects.json`. The
test also states the cause on the models: stored inputs `expected, actual`,
read back `actual, expected`, equal semantics as values, and a checkpoint
sealed from the legacy model that does not `describe` its own source.

Run on `280d6bd` (main before KTLO-04, in a scratch worktree with the two test
files copied in): passes, so it is refused the same way. Run on this branch
(KTLO-04 merged): passes.

### 2. The error names the project and the scenario (criterion 3)

A two-project workspace, *Support desk* then the broken *Order desk*, with a
journal. Observed message:

```text
Migrated project “Order desk” (order-desk) does not match the workspace: a scenario differs after reloading (scenario “Order total is checked”).
```

`workspace.json`, `workspace.json.bak`, `drafts.json` and `journal.json` are
byte-identical before and after; no catalogue and no `projects/` directory;
the staged folders stay under `migration/`. A second attempt is refused with
the same message and still activates nothing.

The criterion says the staged folders are byte-identical before and after.
That cannot hold across attempts — each attempt prepares the unpublished
folders afresh, with new document IDs — so the test checks that staging is
kept, not its bytes.

### 3. Startup (criterion 4, first half)

`activation::decide` on a data directory with the broken workspace answers
`Start::Refused` with project *Order desk*, scenario *Order total is checked*
and the reason; no catalogue. Unreadable drafts and a workspace that cannot be
loaded still answer `Legacy`; the lock and the backup decision are the
existing tests `a_workspace_another_window_owns_…` and
`a_workspace_awaiting_a_backup_decision_…`, which pass unchanged.

### 4. The release binary

The release binary of this branch, no arguments, under Xvfb (1600 × 1000,
Lavapipe, no window manager), with `XDG_DATA_HOME` pointing at a copy of the
six-project data directory and no catalogue. Standard error:

```text
Migration refused project “Order desk”: a scenario differs after reloading (scenario “Order total is checked”). Opening the workspace instead.
```

The workspace window opened, as before this change
([capture](part1-refused-1600x1000.png)). `workspace.json` was byte-identical
afterwards (`sha256sum -c`), there was no `projects.json`, and `migration/`
held the four demo projects in its index plus the refused `order-desk`
folder. Part 2 replaces this window with the *Migration stopped* window of
the mock-up.

## Verification

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean
  after `5e5e3fd` (an unused re-export left by the split).
- `cargo test --workspace --all-targets --locked --no-fail-fast` with
  isolated `XDG_*` on the merged tree: 1 059 passed, 0 failed, 25 ignored over
  53 binaries.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit 0.
- The new tests ten times in a row: `migration_refusal` (2 passed, 1 ignored)
  and the desktop `ktlo03` tests (2 passed), 10 of 10 each.
- The migration suite (`--test migration`): 13 passed, unchanged.

## Windows

Not run on Windows by hand. CI runs the suites on the Windows runner; see the
pull request's checks. Nothing on screen changes in this part, so its
`qa-captures-windows-2022` show nothing of it. Not accepted for Windows on this
report alone.

## Not verified

- **Criterion 2**: the six projects do not migrate; the cause and two ways to
  fix it are in the ticket for the owner's decision.
- Whether the owner's real workspace is refused for this reason. The fixture
  reproduces the same refusal by an ordinary board edit; the owner's
  workspace may hold another cause as well.
- The *Migration stopped* window (rest of criterion 4, and 5): part 2.
- A storage error opening the workspace as a `Legacy` cause: not tested (the
  tests run as root, so a permission error cannot be provoked); the code path
  is unchanged.
- The suspected same mismatch in a folder session (a board edit that removes
  and re-wires an input, then a save): not tested.
- Windows, other than CI.

## Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb.

| # | Result |
| --- | --- |
| 9 | The four-project `fixtures/workspace.json` migration is covered by the unchanged migration suite (13 passed); not run by hand. The six-project synthetic workspace is refused, as recorded above. |
| 1–8, 10 | Not run by hand; nothing they cover changes in this part, and the workspace suite passes. |
