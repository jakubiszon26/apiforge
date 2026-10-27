# MCP-07: The lock names its holder; the window says who and takes the project back — QA report

- Branch: `feat/mcp-07-lock-holder`. Captures are from the release binary of
  `452f53c`, which is the implementation on `main` at `2c9c64c`. The commit
  under review is `6f2aa60`: review round 1's fixes after a merge of `main`
  (#103, #104). See *Review round 1* below.
- Built with: `cargo build --release --locked` (Rust 1.99.0)
- Compared against mock-up: `docs/qa/2026-10-27/MCP-00/m1-*.png` (M1, round
  4, approved 2026-10-27; on PR #103). The *Take back* button and the
  last-change text in M1's bar belong to MCP-08b and MCP-08a, so they are
  not in this ticket.
- Decision during implementation: the description is written to
  `.apiforge/local/holder.json`, not into the lock file (option A on #66,
  2026-10-27). On Windows the lock is mandatory and the lock file cannot be
  read while it is held.
- Verification on `452f53c`, all exit 0:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --all-targets --locked`: 1130 passed, 0 failed,
    30 ignored
  - `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`

  The twelve MCP-07 desktop tests passed five runs in a row. The acceptance
  commit `3871d03` did not compile, because `Holder`, `HolderRole`,
  `open_exclusive_as` and `holder()` did not exist. Once the storage commit
  added them, all eight desktop acceptance tests failed for the right
  reason: no bar and no *Observing* badge, and the window never became
  writable.

## How the release binary was driven

- **Host.** Linux 7.3, the user's Wayland session, RADV. There is no Xvfb or
  XTEST on this host, so the binary was never clicked or typed into.
- **Captures.** Every capture is a `--screenshot OUT.png --folder <shop>
  --mode … --size …` run. The demo shop came from a first start with no
  arguments on empty `XDG_DATA_HOME` and `XDG_CONFIG_HOME` directories.
- **Agent and `apply` holders were simulated.** No shipped program describes
  itself as an agent yet (MCP-01 onwards), and `apply` exits in
  milliseconds. A scratch Python process stood in for both, and it was not
  committed:
  - It took the same OS lock that `open_exclusive` takes (`flock(LOCK_EX)`
    on `.apiforge/local/project.lock`).
  - It wrote the JSON line that `open_exclusive_as` writes into
    `holder.json`.
  - It held both until it was killed.
  - For the older-build case it held the lock and wrote no `holder.json`.

  The real `apply` was checked for its description separately (scenario 1).
- **The window holder is real.** A second release window held the project
  (scenario 3).
- **Where the harness was used instead.** Typing while observing, saving
  after the take-back, the recovery view and the journal review need input
  the host cannot send. They are covered by the `egui_kittest` acceptance
  tests through named widgets. Captures named `harness-*.png` are a design
  review aid, not QA evidence.

## Review round 1

The review of 2026-10-27 found two blocking defects. Both are fixed in `6f2aa60`.

1. **Windows CI was red.** Three tests read the lock file while it was
   held, and Windows refuses that read (error 33, `ERROR_LOCK_VIOLATION`):
   - `mcp07::a_held_project_is_never_taken_however_long_it_is_held`;
   - two storage tests.

   They now check only the file's length. The production code never reads
   the lock file.
2. **An unreadable `holder.json` hid the bar.** A file this user cannot
   read, such as mode 000 or another user's private file, made `holder()`
   return an error. The window dropped that error and showed a plain
   *Read-only* badge with no bar, so criterion 2 failed. Now, while a
   writer is live, any description that cannot be opened or read is
   `Holder::unknown()`, and the window shows *Another process · …*.

   New Unix-only tests cover a mode-000 file:
   - `mcp07_an_unreadable_description_with_a_live_writer_is_an_unknown_holder` (storage);
   - `a_holder_whose_description_cannot_be_read_is_another_process` (desktop).

   Windows permissions cannot be modelled with a file mode, so these tests
   are compiled for Unix only, not ignored.

The bar's text is now formatted once per description instead of on every
frame. Its wording is unchanged.

Verification on `6f2aa60`, all exit 0:
- `cargo fmt --all -- --check`
- clippy with `-D warnings`
- the workspace tests: 1139 passed, 0 failed, 30 ignored
- the fixture check

The MCP-07 storage and desktop tests passed three runs in a row. The
release-binary captures were not retaken: what this round changed is
neither drawn differently nor reached by them.

## Linux

### Scenario 1 (criterion 1): the holder's description

1. `apiforge apply --folder <shop> --file scenarios/promotion-and-cart-price.afs edited.afs`.
2. Read `.apiforge/local/holder.json` and `.apiforge/local/project.lock`.

Expected: `holder.json` holds one JSON line with the program, role
`command`, client `apply`, the start time in UTC and `endpoint: null`. It
holds no path and no secret. The lock file stays empty and is never replaced.

Observed: `{"version":1,"program":"apiforge","role":"command","client":"apply","since":"2026-10-27T20:35:49Z","endpoint":null}`.
The lock file was 0 bytes throughout. Its inode (238657) was the same under
the simulated agent, under the window that took the project back, and after
`apply`.

The storage tests cover the rest:
- a reader gets the description while the writer lives;
- a reader ignores a description left behind;
- a live writer with a missing or unreadable description is an unknown
  holder;
- reading the holder creates no files.

KTLO-21's lock tests (`writer_lock_is_retained_by_clones_and_never_replaced`,
`readers_can_probe_for_a_live_writer_without_creating_its_lock`) are
unchanged and pass.

### Scenario 2 (criterion 2): a window opened while a process holds the project

1. The simulated agent holds the shop (`Claude Code`, since 10:42 local).
2. `--screenshot` in Code at 960 × 540 and 1600 × 1000, Diagram at 960 × 540
   and 1600 × 1000, and Split at 960 × 540.
3. The same with the simulated `apply` holder (since 10:41), and with a
   holder that wrote no description.

Expected:
- a 36 px bar at the bottom names the holder in the warning colour;
- the header badge reads *Observing*;
- *Save* and *Save all* are disabled;
- the notice under the tabs is gone;
- a holder with no description gets *Another process*.

Observed: as expected.
- Agent:
  - [code 960](mcp07-agent-code-960x540.png)
  - [code 1600](mcp07-agent-code-1600x1000.png)
  - [diagram 960](mcp07-agent-diagram-960x540.png)
  - [diagram 1600](mcp07-agent-diagram-1600x1000.png)
  - [split 960](mcp07-agent-split-960x540.png)
- `apply`:
  - [code 960](mcp07-apply-code-960x540.png)
  - [code 1600](mcp07-apply-code-1600x1000.png)
- Older build: [code 960](mcp07-unknown-code-960x540.png), which reads
  *Another process · editing is paused here; this window follows every
  change*.

**Compared with M1:**
- The badge position and colour, the bar's height, background, separator,
  16 px padding, meta font, warning colour and wording match
  `m1-observer-code-960x540.png`, `m1-holder-apply-code-960x540.png` and
  `m1-observer-diagram-1600x1000.png`.
- The middle and right of the bar are empty, because those parts belong to
  MCP-08a and MCP-08b.
- The M1 captures were drawn in the test harness, which leaves an 8 px
  margin around the window. The native window has none, so the native
  captures sit 8 px further up and to the left.
- The harness captures of this branch
  ([code](harness-mcp07-agent-code-960x540.png),
  [diagram](harness-mcp07-agent-diagram-1600x1000.png),
  [split](harness-mcp07-agent-split-960x540.png)) overlay M1 to the pixel,
  except one row: the Problems caption and the zoom bar sit 1 px higher than
  in M1. This bar is exactly 36 px, as the ticket says, and the test
  measures it.

### Scenario 3 (criteria 3 and 4): another window, waiting, and the take-back

1. The simulated agent holds the shop. Start the release window `--folder
   <shop>` and leave it open for 8 s.
2. Read `holder.json` and the lock file, and check the holder is still
   alive.
3. Kill the holder, then poll `holder.json` every 50 ms.
4. With that window still open, `--screenshot` a second window at 960 × 540
   and 1600 × 1000.

Expected:
- while the holder lives, nothing changes: the description is the agent's
  and the lock file is untouched;
- within two seconds of the release, the window is the writer and writes
  its own description;
- the second window names it.

Observed:
- After 8 s the holder was alive, the description was still the agent's,
  and the lock was 0 bytes at the same inode.
- 0.98 s after the release, `holder.json` read
  `{"version":1,"program":"apiforge","role":"window","client":null,"since":"2026-10-27T20:35:47Z","endpoint":null}`.
  An earlier run on the first build measured 0.52 s.
- The second window shows *Another API Forge window · since 21:35 · …*
  ([960](mcp07-window-code-960x540.png), [1600](mcp07-window-code-1600x1000.png)).
  This is the `m1-holder-window-code-960x540.png` view. 21:35 is local time
  (UTC+1) for 20:35:47Z.

**Harness evidence** (named widgets, `folder/acceptance_tests.rs`, module
`mcp07`):
- `a_released_project_is_taken_back_within_two_seconds_and_saves`: text is
  typed while observing and *Save* is disabled. After the holder drops the
  lock, the window is the writer in well under the 2 s bound. *Observing*
  and the bar are gone, *Save* and *Save all* are enabled, the typed text is
  kept, and *Save* writes it to disk.
- `a_taken_back_window_can_publish_again`: *Publish* is enabled again.
- `a_held_project_is_never_taken_however_long_it_is_held`: the lock is held
  for four poll intervals. The window stays read-only with the bar and the
  badge, and the lock bytes, the lock's inode and the description are
  unchanged.

### Scenario 4 (criterion 5): what the holder left behind

Covered in the harness only: the host cannot click the binary, and a
transaction left half-done cannot be produced by the shipped CLI.

- `a_transaction_left_by_the_holder_opens_the_recovery_view`: an activated
  transaction is left behind and the holder drops the lock. *Recover
  interrupted file operation* opens with *Finish intended operation*
  enabled. Finishing writes the intended bytes. *Reload project* is
  followed by the take-back.
- `a_live_run_interrupted_in_the_holder_is_reported_on_take_back`: a journal
  with an interrupted live run is left behind. While observing there is no
  review. After the take-back, the journal review (*Mark as handled*) opens,
  as at start.

### Scenario 5 (criterion 6): the minimum window

Harness test `the_bar_and_the_badge_fit_the_minimum_window_in_every_mode`, at
960 × 540 in Code, Diagram and Split:
- the bar, its text and *Observing* are inside the window
  (`assert_within_window`);
- the bar is 36 px high;
- *PROBLEMS*, *Zoom out*, *Zoom in* and *Fit the view* each end above the
  bar and do not overlap it.

In the release captures the zoom bar and the Problems section sit above the
bar ([diagram 960](mcp07-agent-diagram-960x540.png),
[split 960](mcp07-agent-split-960x540.png)).

### Changes the owner should know about

- **The Problems row no longer adds "Read-only" while observing.** Its
  status now reads *Saved*, as in M1. Before, it read *Saved · Read-only*.
- **A folder this user cannot write keeps the *Read-only* badge, with no
  bar.** No process holds it, so there is nobody to name. M1 does not show
  this case.
- **New dependency.** The desktop shows the start time on the local wall
  clock through `chrono = "=0.4.45"` (features `clock` and `std` only). The
  workspace forbids `unsafe`, so the local offset cannot be read from the
  OS directly.

## Windows

### CI captures

The captures come from CI run 40582097175 (push, commit `b722e66`, Build &
Test (windows-2022): success), artifact `qa-captures-windows-2022`:

- [code 960](windows/code-960x540.png)
- [diagram 960](windows/diagram-960x540.png)
- [split 960](windows/split-960x540.png)
- [code 1600](windows/code-1600x1000.png)
- [diagram 1600](windows/diagram-1600x1000.png)
- [split 1600](windows/split-1600x1000.png)

What these captures show:

- **The writer state, not the observer state.** They show the demo shop in
  Code, Diagram and Split, held by nothing else. The runner's locale made it
  *Sklep demonstracyjny*. No CI capture exercises a held project, so none
  shows the holder bar or the *Observing* badge.
- **Two sizes, not three.** The runner's display is 1024 × 768, so the three
  captures named `1600x1000` are 1024 × 768. The run warns about each one.

On Windows the bar and the take-back are covered by the MCP-07 desktop
tests, which passed in that run. Those tests include reading `holder.json`
while the lock is held, which is what option A on #66 relies on.

### Still owed

This is the first MCP ticket that touches the window, so **the Windows run
of the regression smoke list owed since KTLO-07 is owed here.** The CI
captures do not replace it. It remains owed, from the owner's local build.

## Not verified

- Windows, as above: the observer state and the KTLO-07 smoke list were not
  run on Windows; only the CI captures and the desktop tests were.
- Simulated holders. The release-binary captures for the agent and `apply`
  use a stand-in process that takes the same OS lock and writes the same
  line. No shipped program describes itself as an agent yet.
- Harness only. The recovery view and the journal review on take-back, and
  typing while observing followed by a save after the take-back, were
  checked through `egui_kittest` named widgets, not in the release binary.
- A holder that dies mid-commit. This was not reproduced with a real killed
  process; the transaction case uses a prepared activated transaction, as
  the existing recovery tests do.
- Screen reader. The bar is announced as *Project holder* (a named region)
  plus its text, but no screen reader session was run.

## Regression smoke list (Linux, 2026-10-27, release binary of `452f53c`)

| # | Flow | Result |
| --- | --- | --- |
| 1 | Start with no arguments | **Pass**. Empty data directory; the shop project was created and the window stayed open until killed after 4 s. Stderr held only the driver's `radv is not a conformant Vulkan implementation` warning |
| 2 | Open a folder project | **Pass**. Explorer tree with `.gitattributes`, `.gitignore`, `apiforge.project.json`; scenario in Code ([capture](item2-folder-1600x1000.png)). Expanding `components` was not driven in the binary (no input) |
| 3–6 | Library, wiring, board follows, simulation | Not run in the binary (no pointer input on this host); the desktop suite covers them and passes |
| 7 | Save and reopen | **Pass through the CLI**. `apply` of an edited scenario, exit 0, bytes on disk identical to the input (`cmp`); `check` exit 0; reopened in the window with the edit (`80 * 2`) and *Saved* ([capture](item7-reopened-960x540.png)). Saving with Ctrl+S in the window: harness only (scenario 3) |
| 8 | Close a dirty tab, kill with unsaved work | Not run in the binary (needs typing); the desktop suite's close and recovery tests pass |
| 9 | Migrate a legacy workspace | Not run; this change does not touch migration; the migration suites pass |
| 10 | Window at 960 × 540 | **Pass**. Every control visible in [Code](item10-code-960x540.png), [Diagram](item10-diagram-960x540.png) and [Split](item10-split-960x540.png); and with the bar shown (scenario 2) |
