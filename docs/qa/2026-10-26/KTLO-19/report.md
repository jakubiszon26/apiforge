# KTLO-19: Run results can fill the window — QA report

- Branch: `feat/ktlo-19-max-run-results`; captures from the release binary of
  `8ae3cd0`, before `main` (KTLO-20, an empty board only) was merged in
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Compared against mock-up: [docs/tickets/KTLO-19.md](../../../tickets/KTLO-19.md)
  (`mock-*.png` in this directory), approved as a whole; decision 1 = A
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test --workspace
  --all-targets --locked` (isolated `XDG_CONFIG_HOME` and `XDG_DATA_HOME`) and
  `apiforge-convert-v1 --fixtures --check`: all pass. The five KTLO-19 tests
  passed ten runs in a row.

## Captures

`after-*.png` are from the **release binary** of this branch, started with
`--managed` on an empty data directory (which created the demo shop) under
Xvfb with Lavapipe and no window manager. The maximised state was reached by
**clicking**: a small script sent real pointer and key events through the X
server's XTEST extension (button presses at the coordinates of *Diagram*,
*Run*, *Maximise run results*, …) and captured the window with the X
`GetImage` request. `--screenshot` was not used, because it cannot click. The
window opened at 1600 × 1000; for 960 × 540 the same window was resized
through the X server and the mode buttons were clicked again, so the board's
zoom and the panel's normal height at 960 × 540 are what the window kept from
1600 × 1000, as after a manual resize.

| View | Normal height, after a run | Maximised | Approved mock-up |
| --- | --- | --- | --- |
| Diagram 960 × 540 | [after](after-diagram-normal-960x540.png) | [after](after-diagram-maximised-960x540.png) | `mock-diagram-maximised-960x540.png` |
| Diagram 1600 × 1000 | [after](after-diagram-normal-1600x1000.png) | [after](after-diagram-maximised-1600x1000.png) | `mock-diagram-before-1600x1000.png`, `mock-diagram-maximised-1600x1000.png` |
| Split 960 × 540 | [after](after-split-normal-960x540.png) | [after](after-split-maximised-960x540.png) | — |
| Split 1600 × 1000 | [after](after-split-normal-1600x1000.png) | [after](after-split-maximised-1600x1000.png) | `mock-split-maximised-1600x1000.png` |
| Code 960 × 540 | [after](after-code-normal-960x540.png) | [after](after-code-maximised-960x540.png) | — (decision 1 = A) |

## Linux

### Scenario 1: the button maximises the panel (criteria 1, 3 and 6)
1. Open the demo shop, choose Diagram, click *Run*.
2. Click *Maximise run results*, the icon left of *Continue remaining steps*.
3. Repeat in Split and in Code, at 960 × 540 and 1600 × 1000.

Expected: the panel starts under the tab strip (under the "Running saved
code." notice, which belongs to the strip) and ends at the window's bottom;
the library, board, inspector and, in Split and Code, the code are not drawn.
Observed: as expected and as in the mock-up, in the release binary for the
five views of the table. In the harness the panel's top equals the tab
strip's bottom and its bottom the window's within 1 px, and none of the hidden
widgets is in the tree. Test:
`ktlo19_the_run_results_fill_the_window_and_come_back_at_their_height`.

### Scenario 2: restoring (criteria 2, 3 and 6)
1. Maximise, then click *Restore run results*; maximise, press Escape;
   maximise, click the *Run results* chevron, then click it again.

Expected: the bar returns to where it was and the board (or the code) is back.
Observed: as expected in the release binary in Diagram at 1600 × 1000 for all
three ways, and by the button in Code at 960 × 540; the bar came back at the
same height. The chevron collapses the panel, as it always does, and leaves
the maximised state; opened again, the panel has its earlier height. The
first Escape sent to the release binary did nothing, because under Xvfb
without a window manager the window had no keyboard focus; once the script
gave it focus, Escape restored the layout each time it was pressed. The same
test covers all three ways in every mode at both sizes.

### Scenario 3: switching mode or tab (criterion 4)
Expected: choosing another mode or tab restores a maximised panel.
Observed: in the release binary, clicking Split while maximised in Diagram,
and Code while maximised in Split, drew the normal layout. Ctrl+Tab was not
tried in the release binary; the test switches modes in three directions and
tabs with Ctrl+Tab and back. Test:
`ktlo19_switching_the_mode_or_the_tab_restores_the_layout`.

### Scenario 4: a step chosen while maximised (criterion 5)
Expected: after restoring, the chosen step's block is selected on the board
with its inspector open.
Observed: as expected in the harness at both sizes; not tried in the release
binary. Test: `ktlo19_a_step_chosen_while_maximised_is_selected_on_the_board`.

### Scenario 5: accessibility and the minimum window (criterion 7)
Expected: the audit passes while maximised; at 960 × 540 every control of
the bar and every step row (once scrolled to) is inside the window.
Observed: as expected. The audit found two older unnamed controls of the
report, now named: the error policy box (*When a step fails*) and the
draggable output keys (named by their key). Tests:
`the_maximised_run_results_name_every_control`,
`ktlo19_the_maximised_bar_and_step_list_fit_the_minimum_window`. Design
review of `review_run_results_maximised` (harness captures, three modes, both
sizes): nothing cut off or overlapping; matches the mock-up.

## Windows

Not run on Windows by hand. CI runs the desktop suite, including the KTLO-19
tests, on the Windows runner and uploads the standard captures
(`qa-captures-windows-2022`), which show no run and so no maximised panel; see
the pull request's checks. Not accepted for Windows on this report alone.

## Not verified

- Escape with a window manager giving the window focus, as on a desktop; here
  the script gave it focus.
- In the release binary: Ctrl+Tab while maximised, choosing a step while
  maximised, and Split and Diagram at 960 × 540 restored by the button or the
  chevron (the tests cover them).
- Double scale (960 × 540 at 2×) design review.
- The maximise button's place in the Tab order (the audit checks its name).
- Windows, other than the CI suite and captures.

## Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb. Items 6 and 10
are the ones this change touches.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr stayed empty for the whole session. |
| 2 | Pass: the project opened with the scenario's tab in Code. |
| 3–5, 7–9 | Not run by hand for this change; the workspace suite that covers them passes, and so does `apiforge-convert-v1 --fixtures --check`. |
| 6 | Pass: *Run* (simulation) reported every step, PASSED, 8 passed, shown in Diagram, Split and Code. |
| 10 | Pass: at 960 × 540 in Diagram, Split and Code, normal and maximised, the bar's controls are visible (`after-*-960x540.png`). |
