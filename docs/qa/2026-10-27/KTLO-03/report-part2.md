# KTLO-03 part 2: A refused migration stops in the *Migration stopped* window — QA report

- Branch: prepared on top of part 1 (#78); to be opened from `main` once
  part 1 is merged. Acceptance tests `4dd45e8`, implementation `e511ee1`, two
  corrections found by this QA run: `2f2600e` (buttons on the last row) and
  `d447a29` (the grown workspace window centred).
- Ticket: [KTLO-03](../../../tickets/KTLO-03.md); decisions 1 (report and
  stop), 3 (the window, A), 4 (other causes untouched, A) and 5 (the mock-up)
  on #5. Compared against `mock-migration-stop-*.png` in this directory.
- Built with: `cargo build --release -p apiforge-desktop --locked`.
- How the state was reached: the release binary started with **no
  arguments**, `XDG_DATA_HOME` pointing at a copy of a data directory that
  holds the synthetic six-project `workspace.json` (written by the test
  generator `crates/apiforge-project/tests/support/legacy_workspace.rs`
  through the legacy writer) and no catalogue — the owner's start. Each run on
  a fresh Xvfb screen of the stated size with Lavapipe and **no window
  manager**, so the window has no title bar or frame. Clicks were real pointer
  events through the X server's XTEST extension; captures by X `GetImage` of
  the whole screen (or the window alone). `--screenshot` was not used: it
  cannot show managed startup.

| View | Release binary | Approved mock-up |
| --- | --- | --- |
| Screen 960 × 540 | [after](after-migration-stop-960x540.png) | `mock-migration-stop-960x540.png` |
| Screen 1600 × 1000 | [after](after-migration-stop-1600x1000.png) | `mock-migration-stop-1600x1000.png` |
| The window alone | [after](after-migration-stop-window.png) | `mock-migration-stop-window.png` |
| After *Open the previous workspace* | [after](after-open-previous-1600x1000.png) | — |

## Linux

### Scenario 1: the start that migration refuses (criteria 4 and 5)
1. Start the application with the data directory above.

Expected: one window, 480 × 276 points, centred, with *Migration stopped*,
the paragraph naming *Order desk*, the reason and the scenario *Order total
is checked*, the muted line, and *Open the previous workspace* and *Quit*,
neither primary; nothing else opens; standard error says why.
Observed: as expected, and as in the mock-up, on both screens: the window at
(240, 132) on 960 × 540 and (560, 362) on 1600 × 1000, 480 × 276. Standard
error: `Migration stopped: project “Order desk”: a scenario differs after
reloading (scenario “Order total is checked”).` `workspace.json` was
byte-identical afterwards and there was no `projects.json`.

The first capture showed the two buttons 45 points above the window's bottom
edge instead of the mock-up's 24: they were centred in the space left below
the text. Fixed in `2f2600e`; the interaction test now measures it, and fails
on the earlier layout ("ends on the margin 384").

### Scenario 2: *Open the previous workspace* (criterion 5)
1. In scenario 1 on 1600 × 1000, click *Open the previous workspace*.

Expected: the workspace window `--legacy` opens, in the same native window,
at the workspace window's size.
Observed: the workspace window with the demo shop, 1600 × 1000. On the first
run it grew from the stop window's corner, so most of it lay off the screen;
`d447a29` centres it at its new size (clamped to the screen), and the capture
shows it at (0, 0) filling the screen. Without a window manager this is the
binary's own placement; a desktop's window manager may place it differently.

### Scenario 3: *Quit* (criterion 5)
1. In scenario 1 on 960 × 540, click *Quit*.

Expected: the application exits; nothing else opens.
Observed: the process exited by itself with code 0; no other window.

### Scenario 4: tests at both sizes, accessibility (criterion 5)
`ktlo03_the_stop_window_says_what_was_refused_and_offers_two_ways_out`,
`ktlo03_open_the_previous_workspace_opens_the_workspace_window`,
`ktlo03_quit_closes_the_window_and_opens_nothing` at 960 × 540 and
1600 × 1000, and the audit `the_migration_stop_window_names_every_control`
(both buttons named, no unnamed control) at the window's own size and centred
on both screens; `ktlo03_a_refused_migration_starts_the_stop_window_not_the_workspace_window`
asserts `Opening::Stopped`. All failed at `4dd45e8` (nothing drawn, the
workspace window opened) and pass; 10 of 10 runs in a row. Design review
`review_migration_stop` matches the mock-up.

## Verification

On the part 2 head: `cargo fmt --all -- --check` clean; `cargo clippy
--workspace --all-targets --locked -- -D warnings` clean; the desktop suite
402 passed, 0 failed, 23 ignored; the six new and changed desktop tests 10 of
10. The whole workspace suite passed on the unsplit branch that holds the
same code (exit 0) before the two corrections; it is to be rerun when this
part is opened from `main`.

## Windows

Not run. CI will run it when this part is opened as a pull request; the
`qa-captures-windows-2022` artifact does not show managed startup.

## Not verified

- With a window manager: centring, the title bar, and where the grown
  workspace window lands.
- Keyboard: Tab order between the two buttons, Enter and Escape in the stop
  window.
- Double scale (960 × 540 at 2×).
- A refusal other than verification (an occupied folder, changed inputs): the
  window shows the general paragraph; covered by no test.
- Windows.

## Regression smoke list

Linux, 2026-10-27, release binary of part 2 under Xvfb.

| # | Result |
| --- | --- |
| 1 | Not run with an empty data directory for this part; the refused start is scenario 1. |
| 10 | Pass for the stop window at 960 × 540: both buttons visible and usable (scenario 3 clicked *Quit* there). |
| 2–9 | Not run by hand; nothing they cover changes. |
