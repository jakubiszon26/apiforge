# KTLO-03: A refused migration stops in the *Migration stopped* window; the six-project workspace migrates — QA report

Both parts of KTLO-03. Part 2 is first; part 1, merged in PR #78, follows
unchanged below except that its headings moved one level down.

## Part 2

- Branch: `feat/ktlo-03-stop-window`, from `main` at `18f25a5` (part 1
  merged). The window work, written on top of part 1 before it was merged,
  came in by merge (`3f188ac`): acceptance tests `4dd45e8`, implementation
  `e511ee1`, corrections `2f2600e` (buttons on the last row) and `d447a29`
  (the grown workspace window centred). Then: acceptance tests for criteria 2
  and 3 `cfe82b7`, the fix of decision 6 `1c291cb`, the named refusal of an
  unprintable scenario `6a849a7`.
- Ticket: [KTLO-03](../../../tickets/KTLO-03.md); decisions 1–2 of
  2026-10-26 and 3–7 of 2026-10-27 on #5. Compared against
  `mock-migration-stop-*.png` in this directory.
- Built with: `cargo build --release -p apiforge-desktop --locked`; the tests
  with `cargo test --locked` (debug), isolated `XDG_*`.
- Machine: Linux, 3 cores shared with two other builds (load average 10–14).
- How the release runs were reached: the release binary started with **no
  arguments**, `XDG_DATA_HOME` pointing at a fresh copy of a data directory
  holding a synthetic `workspace.json` (written by
  `crates/apiforge-project/tests/support/legacy_workspace.rs` through the
  legacy writer) and no catalogue — the owner's start. *broken* is
  `with_a_broken_project()` (*Support desk*, then *Partner feed*); *six* is
  `six_projects()`. Each run on a fresh Xvfb screen of the stated size with
  Lavapipe and **no window manager**, so the window has no title bar or frame.
  Clicks were real pointer events through the X server's XTEST extension;
  captures by X `GetImage` of the whole screen or the window alone.
  `--screenshot` was not used: it cannot show managed startup. All captures
  below were **retaken on this branch** (`6a849a7`'s release binary); those
  committed at `fd6ded7` showed the earlier refusal of *Order desk*, which no
  longer happens.

| View | Release binary | Approved mock-up |
| --- | --- | --- |
| *broken*, screen 960 × 540 | [after](after-migration-stop-960x540.png) | `mock-migration-stop-960x540.png` |
| *broken*, screen 1600 × 1000 | [after](after-migration-stop-1600x1000.png) | `mock-migration-stop-1600x1000.png` |
| The window alone | [after](after-migration-stop-window.png) | `mock-migration-stop-window.png` |
| After *Open the previous workspace* | [after](after-open-previous-1600x1000.png) | — |
| *six*, screen 1600 × 1000 | [after](after-six-projects-migrated-1600x1000.png) | — |

### Scenarios

| # | Criterion | Test | Result |
| --- | --- | --- | --- |
| 1 | the six-project workspace migrates | `the_six_project_workspace_migrates_with_ktlo_04_merged` | failed at `cfe82b7` (refused on *Order desk*); passes from `1c291cb`, without its ignore |
| 2 | a still-refused migration names project and scenario; originals byte-identical; staging kept; nothing published or activated | `a_refused_migration_names_the_project_and_the_scenario_and_activates_nothing` | failed at `cfe82b7` and `1c291cb` (`Unprintable`, naming neither); passes from `6a849a7` |
| 3 | the refusal is its own start and opens the stop window | `ktlo03_a_refused_migration_starts_the_stop_window_not_the_workspace_window` | failed at `cfe82b7` (no project named); passes |
| 4 | the window: reason, two named buttons, both screen sizes, *Open the previous workspace*, *Quit*, the audit | `ktlo03_the_stop_window_says_what_was_refused_and_offers_two_ways_out`, `ktlo03_open_the_previous_workspace_opens_the_workspace_window`, `ktlo03_quit_closes_the_window_and_opens_nothing`, `the_migration_stop_window_names_every_control` | failed at `4dd45e8`; pass |
| 5 | the cause, on the models | `a_check_whose_request_was_replaced_stores_its_inputs_out_of_source_order` | passes (the refusal itself was reproduced in part 1) |
| 6 | verification commands; the migration suite unchanged | see *Verification* | pass |

#### Scenario 1: the six-project workspace migrates (criterion 2)

1. Start the release binary with the *six* data directory.

Expected: no stop window; the six projects migrated and the catalogue
activated; the project that was active opens.
Observed: the folder window with *Demo shop* opened at once
([capture](after-six-projects-migrated-1600x1000.png)); nothing on standard
error; `projects.json` lists `shop`, `tickets`, `blocks-poc`, `workflow-poc`,
`order-desk`, `support-desk` with `shop` active, and `projects/` holds those six
folders; `workspace.json` byte-identical afterwards (`sha256sum -c`). The
test reopens every migrated folder and finds every scenario equal to the
workspace's, including *Order desk*'s *Order total is checked*.

The fix (decision 6): before preparing a project, migration prints and reads
back each scenario and component and orders every block's inputs, and every
expression's operands, as the source reads them back; the checkpoint it
seals then describes its own source. Nothing outside `migration.rs` changes,
existing folder projects are untouched, and the 13 migration tests pass
unchanged.

#### Scenario 2: a migration that is still refused (criterion 3)

Criterion 3 needed a new broken project, since *Order desk* now migrates. With
the fix, none of the map orders the legacy writer can save was found to be
refused (tried: object fields, literal objects, scenario parameters, sample
responses, and `-0.0` literals), and verification restores every scenario
from its checkpoint. What migration still refuses is a scenario that cannot be
written as a folder source: *Partner feed* has one request, `load_feed`, whose
address is the environment value `PARTNER_URL`, which the project never sets.
A legacy document may keep such a name and the legacy writer saves it; a
folder source may not. Until `6a849a7` that surfaced as `Error::Unprintable`
("The model cannot be written as source without semantic loss."), naming
neither project nor scenario; migration now checks every scenario prints
before preparing a project. Observed message:

```text
Migrated project “Partner feed” (partner-feed) does not match the workspace: a scenario cannot be written as code without loss (scenario “Feed is fetched from an unset address”).
```

`workspace.json`, `workspace.json.bak`, `drafts.json` and `journal.json` are
byte-identical before and after; no catalogue and no `projects/` directory;
under `migration/` the folder of *Support desk*, prepared first, and the empty
folder of *Partner feed* stay. A second attempt is refused with the same
message and still activates nothing. The staged folders are not
byte-identical across attempts (each attempt prepares them afresh, with new
document IDs), as part 1 recorded.

#### Scenario 3: the start that migration refuses (criteria 4 and 5)

1. Start the release binary with the *broken* data directory.

Expected: one window, 480 × 276 points, centred, with *Migration stopped*,
the paragraph naming the project, the reason and the scenario, the muted
line, and *Open the previous workspace* and *Quit*, neither primary; nothing
else opens; standard error says why.
Observed: as expected, and laid out as in the mock-up, on both screens: the
window at (240, 132) on 960 × 540 and (560, 362) on 1600 × 1000, 480 × 276.
The reason wraps to four lines. Standard error:
`Migration stopped: project “Partner feed”: a scenario cannot be written as
code without loss (scenario “Feed is fetched from an unset address”).`
`workspace.json` byte-identical afterwards; no `projects.json`.

The paragraph's last sentence, "The scenario … did not read back the same
from the migrated folder", was written for the verification refusal. For this
refusal it is loosely true (the scenario's source cannot be read back as the
scenario) but reads less precisely; the wording of reasons is outside this
ticket.

The first run at `fd6ded7` showed the two buttons 45 points above the
window's bottom edge instead of the mock-up's 24; fixed in `2f2600e`, and the
interaction test measures it.

#### Scenario 4: *Open the previous workspace* (criterion 5)

1. In scenario 3 on 1600 × 1000, click *Open the previous workspace*.

Expected: the workspace window `--legacy` opens, in the same native window,
at the workspace window's size.
Observed: the workspace window with *Support desk*'s *Ticket lookup*, at
(0, 0) filling the 1600 × 1000 screen
([capture](after-open-previous-1600x1000.png)); `workspace.json`
byte-identical when the process was stopped. Without a window manager the
placement is the binary's own (`d447a29` centres it, clamped to the screen).

#### Scenario 5: *Quit* (criterion 5)

1. In scenario 3 on 960 × 540, click *Quit*.

Expected: the application exits; nothing else opens.
Observed: the process exited by itself with code 0; no other window;
`workspace.json` byte-identical.

### Verification

On the branch head before this report (`6a849a7`):

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
- `cargo test --workspace --all-targets --locked --no-fail-fast`: 1 080
  passed, 1 failed, 26 ignored over 54 binaries. The failure was
  `folder::tests::a_large_source_follows_the_caret_once_its_edit_is_analysed`,
  "storage did not become idle" — a 15-second wait for storage while the
  load average was 13 on 3 cores; it touches no migration code, passed alone
  right after (11 s) and in the desktop suite below.
- `cargo test -p apiforge-desktop --locked`: 412 passed, 0 failed, 24
  ignored.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit 0.
- The new and changed tests ten times in a row: `migration_refusal` (3
  passed) and the desktop `ktlo03` and `migration_stop` tests (6 passed, the
  design review ignored), 10 of 10 each.
- The migration suite (`--test migration`): 13 passed, unchanged.

### Windows

Not run on Windows by hand. CI runs the suites on the Windows runner; see the
pull request's checks. The `qa-captures-windows-2022` artifact does not show
managed startup, so it shows neither the stop window nor a migration. Not
accepted for Windows on this report alone.

### Not verified

- Whether the owner's real workspace now migrates. The fixture reproduced
  the owner's refusal by an ordinary board edit and now migrates; the real
  workspace may hold another cause as well.
- A component that cannot be written as code still refuses with the
  anonymous `Unprintable` message (the stop window then shows the general
  paragraph); only scenarios are named.
- The suspected same input-order mismatch in a folder session (a board edit
  that removes and re-wires an input, then a save): its own ticket
  (decision 7).
- With a window manager: centring, the title bar, and where the grown
  workspace window lands.
- Keyboard: Tab order between the two buttons, Enter and Escape in the stop
  window.
- Double scale (960 × 540 at 2×).
- A refusal other than verification (an occupied folder, changed inputs): the
  window shows the general paragraph; covered by no test.
- Windows, other than CI.

### Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb.

| # | Result |
| --- | --- |
| 9 | Pass: the six-project synthetic workspace migrates on start (scenario 1); the four-project `fixtures/workspace.json` by the unchanged migration suite (13 passed). |
| 10 | Pass for the stop window at 960 × 540: both buttons visible and usable (scenario 5 clicked *Quit* there). |
| 1–8 | Not run by hand; nothing they cover changes, and the workspace suite passes. |

## Part 1 (PR #78, merged)

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

### What was found

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

### Scenarios

| # | Criterion | Test | Result |
| --- | --- | --- | --- |
| 1 | the synthetic six-project workspace is refused, and why | `a_six_project_workspace_is_refused_on_a_check_whose_request_was_replaced` | passes at `280d6bd` (before KTLO-04, the test copied onto that commit) and on this branch: refused on project `order-desk`, "a scenario differs after reloading" |
| 2 | with KTLO-04 the six projects migrate | `the_six_project_workspace_migrates_with_ktlo_04_merged` | **fails** (refused on Order desk); `#[ignore = "KTLO-03: …"]` since `d66d416` |
| 3 | a still-refused migration names project and scenario; originals byte-identical; nothing activated | `a_refused_migration_names_the_project_and_the_scenario_and_activates_nothing` | failed at `737e56f` (the message named neither); passes |
| 4 (first half) | `managed_start` reports the refusal as its own `Start` variant; the other causes stay `Legacy` | `ktlo03_a_refused_migration_starts_the_stop_window_not_the_workspace_window`, `ktlo03_the_other_causes_still_open_the_workspace_window` | the first failed at `737e56f` (`Legacy`); both pass |
| 6 | verification commands; migration suite unchanged | see *Verification* | pass |

#### 1. The reproduction (criterion 1)

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

#### 2. The error names the project and the scenario (criterion 3)

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

#### 3. Startup (criterion 4, first half)

`activation::decide` on a data directory with the broken workspace answers
`Start::Refused` with project *Order desk*, scenario *Order total is checked*
and the reason; no catalogue. Unreadable drafts and a workspace that cannot be
loaded still answer `Legacy`; the lock and the backup decision are the
existing tests `a_workspace_another_window_owns_…` and
`a_workspace_awaiting_a_backup_decision_…`, which pass unchanged.

#### 4. The release binary

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

### Verification

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

### Windows

Not run on Windows by hand. CI runs the suites on the Windows runner; see the
pull request's checks. Nothing on screen changes in this part, so its
`qa-captures-windows-2022` show nothing of it. Not accepted for Windows on this
report alone.

### Not verified

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

### Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb.

| # | Result |
| --- | --- |
| 9 | The four-project `fixtures/workspace.json` migration is covered by the unchanged migration suite (13 passed); not run by hand. The six-project synthetic workspace is refused, as recorded above. |
| 1–8, 10 | Not run by hand; nothing they cover changes in this part, and the workspace suite passes. |
