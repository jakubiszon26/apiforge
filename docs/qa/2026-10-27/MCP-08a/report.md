# MCP-08a: Observer mode — editing paused, the window follows every commit — QA report

- Branch: `feat/mcp-08a-observer`. Captures are from the release binary of
  `cf6ee98`; the commit under review is `609b868`: review round 1's fixes (`28c7329`) and test messages CodeQL flagged
  (see *Review round 1* below).
- Built with: `cargo build --release --locked` (Rust 1.99.0)
- Compared against mock-up: `docs/qa/2026-10-27/MCP-00/m1-observer-*.png`
  (M1, round 4, approved 2026-10-27). The *Take back* button in M1's bar
  belongs to MCP-08b and is not in this ticket.
- Verification on `28c7329` (`609b868` changes only test messages), all exit 0:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --all-targets --locked`: 1179 passed, 0 failed,
    31 ignored
  - `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
- Acceptance tests first: commit `c3b8508`. Before the implementation 7
  desktop, 4 project and 1 storage test failed for the right reason: typing
  accepted, the old checkpoint's positions, the *reloaded* notice, no
  last-change text, the settings entry disabled, a recovery error during a
  living holder's commit. Three passed already and pin what the change
  keeps: the save after the take-back (criterion 8), the deleted document
  at session level, and the storage mixed-read test (criterion 5), because
  the stable read already never mixes a source with another checkpoint.

## Review round 1

The automated review of #110 raised fifteen points; all were valid and are
fixed in `28c7329`. Those that change behaviour:
- A check that meets the holder's commit in progress leaves the window as it
  was until the next check; before, it fell back to following files one by
  one from a possibly partial commit.
- No live run starts from an observing window by any path, including a
  confirmation shown before observing began.
- Opening a called component from the board works while observing.
- The environment and settings dialogs follow the observing state while
  open; observing, *Apply environment* applies the session's secret values
  only.
- The bar's last change takes at most half the bar and is cut with the full
  text on hover; a long file name no longer pushes the holder out.
- The deletion notice names only documents whose tab closed; a commit of a
  source and the settings names both.
- A failed probe at opening counts as a living holder (saved state only).

The release-binary captures were not retaken: what changed is drawn the
same for the short names they show.

## How the release binary was driven

- **Host.** Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver,
  X11 backend. There is no XTEST here, so the binary was never clicked or
  typed into; screens of a running window were grabbed from the Xvfb
  display.
- **The demo shop** came from a managed start that migrated
  `fixtures/workspace.json` in an empty data directory, as CI does. The
  runner's locale made it *Sklep demonstracyjny*.
- **The holder was simulated**, as in MCP-07: a scratch Python process (not
  committed) took the OS lock that `open_exclusive` takes (`flock(LOCK_EX)`
  on `.apiforge/local/project.lock`), wrote the agent's line into
  `holder.json` (*Claude Code*, since 10:42 local) and held both. Its
  "commit" replaced the scenario file atomically, without a transaction,
  because no shipped program commits as an agent yet (MCP-03 onwards). The
  transaction path is covered by the acceptance tests, whose holder is a
  real `ProjectSession` that took the lock first.

## Linux

### Scenario 1 (criterion 1): a window opened beside a holder observes

1. The simulated agent holds the shop.
2. `--screenshot` in Code, Diagram and Split at 960 × 540 and 1600 × 1000.

Expected: *Observing* badge, the holder bar, *Save* and *Save all*
disabled; the board legible without a read-only notice; the library's
blocks, *Add step* and *Tidy up* disabled; the explorer's *New* disabled;
*Run* enabled.

Observed: as expected.
[code 960](observer-code-960x540.png),
[code 1600](observer-code-1600x1000.png),
[diagram 960](observer-diagram-960x540.png),
[diagram 1600](observer-diagram-1600x1000.png),
[split 960](observer-split-960x540.png),
[split 1600](observer-split-1600x1000.png).

**Harness evidence** (`folder/acceptance_tests.rs`, module `mcp08a`,
`observing_pauses_every_edit_and_still_simulates`): text typed into the
code view does not change the source; dragging *HTTP request* onto the
board and dragging a card change nothing; *Output value of Expected price*
(a socket, where a wire starts) is disabled; a card can still be selected
and the inspector's *Block name* is disabled; the document stays clean;
*Run live requests* is disabled; *Run* finishes a simulation with every
step.

### Scenario 2 (criteria 2 and 9): the window follows a commit

1. The simulated agent holds the shop; start the release window `--folder
   <shop>` and leave it 10 s.
2. The holder replaces the scenario with `80 * 1` changed to `80 * 2`.
3. Grab the screen until the bar changes, then once more after 1 s.

Expected: within two seconds the code shows `80 * 2`, *Saved*, no notice;
the bar's right end says *HH:MM · promocja-i-cena-w-koszyku.afs saved*;
the tab stays.

Observed: the bar changed 0.57 s after the write.
[before](live-before-1600x1000.png),
[after](live-followed-1600x1000.png): line 9 reads `80 * 2`, *Saved*,
*No problems*, no notice under the tabs, and the bar ends *23:05 ·
promocja-i-cena-w-koszyku.afs saved*.

**Harness evidence:**
- `the_holders_save_is_shown_within_two_seconds_where_the_person_was`: the
  holder (a real session) saves a changed scenario with every block moved;
  within 2 s the window shows the text and its blocks have the identities
  and positions of the record in `.apiforge/diagram.json`; the active tab,
  the camera (zoomed in before) and the selected block are unchanged; a
  later invalid save shows its diagnostics in Problems.
- `the_bar_with_its_last_change_fits_the_minimum_window_in_every_mode`: at
  960 × 540 in Code, Diagram and Split the badge, the bar and *… ·
  promotion-and-cart-price.afs saved* are inside the window, and the last
  change does not overlap the holder's text. Captures
  ([code](harness-mcp08a-code-960x540.png),
  [diagram](harness-mcp08a-diagram-960x540.png),
  [split](harness-mcp08a-split-960x540.png),
  [diagram 1600](harness-mcp08a-diagram-1600x1000.png)) are a design
  review aid, not QA evidence.

### Scenario 3 (criteria 3, 4, 7): registrations, revisions, deletions

Harness only (the stand-in holder cannot commit a registry change):
- `a_document_the_holder_registers_is_listed_without_a_notice`: the
  explorer lists `added-by-agent.afs` within 2 s; no notice; the active tab
  is unchanged; the bar says *… added-by-agent.afs added*.
- `a_revision_the_holder_publishes_is_followed`: the holder publishes
  `active_promotion` and saves the scenario calling the new revision;
  within 2 s the component's tab shows the published text, the registry
  lists the revision, the scenario analyses as valid, no notice.
- `a_tab_whose_document_the_holder_deleted_closes_with_a_notice`: its tab
  closes with *Closed short-lived.afs: deleted while this window was
  observing.*; the scenario's tab stays.

### Scenario 4 (criteria 5 and 6): never half a commit, never an older state

- Storage (`transaction_tests.rs`):
  `an_observer_never_reads_a_source_without_its_checkpoint` holds the
  commit of a source and its checkpoint at every boundary and reads from
  another store: always the old pair, the new pair, or "come back";
  `an_observers_check_during_a_living_holders_commit_is_busy`: the
  window's check is busy, not the recovery screen, while the holder lives
  (planning decision 2).
- Project (`tests/external.rs`, module `observer`): a followed commit
  reloads in a new epoch with the holder's identities and positions; a
  layout-only commit is followed (the checkpoint is fingerprinted, planning
  decision 3); an observer shows the saved state, never the holder's
  recovery drafts (planning decision 1); an analysis of the previous text
  started before the commit is refused after it (`StaleAnalysis`) and the
  model stays the committed one.

### Scenario 5 (criterion 8 and the carry-over from #106): the take-back

1. In scenario 2's window, the holder releases the lock.

Expected: within two seconds the window is the writer: badge and bar gone,
the window's own description in `holder.json`.

Observed: `holder.json` read
`{"version":1,"program":"apiforge","role":"window",…}` 0.08 s after the
release was signalled (the window's check was already running);
[after](live-taken-back-1600x1000.png): no badge, no bar, the followed text
kept.

**Harness evidence:**
- `after_the_take_back_a_save_meets_no_checkpoint_conflict`: after
  following a commit that moved blocks, the window takes the project back,
  edits text and layout and saves with no conflict; the checkpoint on disk
  holds the window's layout.
- `settings_open_disabled_while_observing_and_the_take_back_still_works`:
  *Project settings…* opens with *Project name* and *Apply settings*
  disabled; *Test environment…* opens with *Base URL*, *New variable name*
  and *New secret name* disabled; no configuration is left dirty and the
  take-back succeeds.
- MCP-07's `a_released_project_is_taken_back_within_two_seconds_and_saves`
  and `a_taken_back_window_can_publish_again` now type after the take-back
  (planning decision 4).

### Compared with M1

- Badge, bar, the holder's wording and the last-change text at the bar's
  right end match `m1-observer-code-960x540.png`,
  `m1-observer-split-960x540.png` and `m1-observer-diagram-1600x1000.png`.
  The *Take back* button is absent (MCP-08b), so the last change sits at
  the right edge, where M1 has it left of the button.
- Differences the acceptance criteria require: the library's blocks, *Add
  step*, *Tidy up* and the explorer's *New* are drawn disabled (greyed); M1
  draws them as enabled. The board itself is drawn as in M1, without the
  *This diagram is read-only* notice.
- The harness captures overlay M1 apart from the missing button and the
  greyed controls above.

### Changes the owner should know about

- **Typing while observing is gone** (decision 12). Before, a read-only
  window beside another window kept typed text in memory and refused to
  close; now it takes no keystroke and closes normally. The old test that
  asserted the old behaviour was rewritten
  (`a_window_observing_another_takes_no_typing_and_closes`).
- **Project settings… is available while observing**, opening the dialog
  with every control disabled, its two tabs included; before, the menu
  entry was disabled.
- **A reload keeps the shown document** in every case, also when the writer
  reloads after a registry change; before, the code view started afresh.
- **Only beside a living holder.** A read-only window without a holder (a
  folder this user cannot write) still follows files one by one and keeps
  typing, as before.
- The observer's check asks the lock probe once per poll; with a holder
  alive that confirmation takes up to 0.2 s on the worker, which the
  two-second bound absorbs.

## Windows

### CI captures

From CI run 40598448691 (push, commit `cf6ee98`, Build & Test
(windows-2022): success), artifact `qa-captures-windows-2022`:
[code 960](windows/code-960x540.png), [diagram 960](windows/diagram-960x540.png),
[split 960](windows/split-960x540.png), [code](windows/code-1600x1000.png),
[diagram](windows/diagram-1600x1000.png), [split](windows/split-1600x1000.png).

- They show the writer state (no other holder), not the observer: no CI
  step holds the project, so none shows the bar or the paused editor.
- The three captures named `1600x1000` are 1024 × 768, the runner's display.

On Windows the observer is covered by the desktop, project and storage tests
of this ticket, which passed in that run; the runs of later heads are on the pull request.

## Not verified

- Windows, as above, and the KTLO-07 regression smoke list on Windows
  (owed from the owner's local build).
- A real agent holder in the release binary: the stand-in writes the
  source without a transaction; registrations, revisions, deletions, the
  checkpoint and the transaction boundary were checked in the harness and
  the storage and project tests only.
- Input in the release binary: typing, drops, wires and the settings
  dialogs while observing were checked through `egui_kittest` named widgets
  only.
- The hover text of *Run live requests* is not read back in a test; only
  its disabled state is asserted.
- Screen reader: the last-change text is a plain label; no screen reader
  session was run.

## Regression smoke list (Linux, 2026-10-27, release binary of `cf6ee98`)

| # | Flow | Result |
| --- | --- | --- |
| 1 | Start with no arguments | **Pass** with `--managed` on an empty data directory holding `fixtures/workspace.json`: the shop was migrated and opened; stderr held only Mesa's `DRI3` warnings |
| 2 | Open a folder project | **Pass**: explorer tree and the scenario in Code ([capture](live-taken-back-1600x1000.png)) |
| 3–6 | Library, wiring, board follows, simulation | Not run in the binary (no pointer input); the desktop suite covers them and passes |
| 7 | Save and reopen | Not run in the binary (no input); the save after the take-back is covered in the harness (scenario 5) |
| 8 | Close a dirty tab, kill with unsaved work | Not run in the binary; the desktop suite's close and recovery tests pass |
| 9 | Migrate a legacy workspace | **Pass**: `fixtures/workspace.json` migrated by the managed start of item 1; the migration suites pass |
| 10 | Window at 960 × 540 | **Pass**: every control visible in [Code](observer-code-960x540.png), [Diagram](observer-diagram-960x540.png) and [Split](observer-split-960x540.png), with the bar shown |
