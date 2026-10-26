# KTLO-12: One header above the board (part 2 of 2) — QA report

- Branch: `feat/ktlo-12-one-header-b2` (the PR head commit), from `main` at `5ff7123`
- Built with: `cargo build --release --locked -p apiforge-desktop`
- Compared against the approved mock-ups in this directory: `mock-a-diagram-*`
  (Diagram, variant A) and `mock-b-split-*` (Split, variant B);
  [ticket](../../../tickets/KTLO-12.md)
- Verification: the development guide's commands; output in the pull request.

## Captures

`after-*.png`: release binary of this branch, `--screenshot` of the managed
demo shop under Xvfb. The mock-ups are harness captures, with an 8 px margin
the native window lacks and a zoom a percent or two apart.

| View | Before | Approved mock-up | After (release) |
| --- | --- | --- | --- |
| Diagram 960 × 540 | [before](before-diagram-960x540.png) | [mock A](mock-a-diagram-960x540.png) | [after](after-diagram-960x540.png) |
| Diagram 1600 × 1000 | [before](before-diagram-1600x1000.png) | [mock A](mock-a-diagram-1600x1000.png) | [after](after-diagram-1600x1000.png) |
| Split 960 × 540 | [before](before-split-960x540.png) | [mock B](mock-b-split-960x540.png) | [after](after-split-960x540.png) |
| Split 1600 × 1000 | [before](before-split-1600x1000.png) | [mock B](mock-b-split-1600x1000.png) | [after](after-split-1600x1000.png) |

## Linux

### Scenario 1: one row between the tab strip and the board (criteria 1, 2)
1. Open the demo shop folder project; switch to Diagram, then to Split, at
   960 × 540 and 1600 × 1000.

Expected: no white header row; on the board's grey header band, Diagram shows
file name, "Scenario", Main, Cleanup, the steps summary, Add step and the two
tools; Split shows Main, Cleanup, then the same right-hand group. The top app
bar is unchanged.
Observed: as expected and as in the mock-ups at both sizes. Tests:
`one_row_lies_between_the_tab_strip_and_the_board`,
`the_row_shows_its_parts_left_to_right`.

### Scenario 2: the drop order when the row is narrow (criterion 4)
Expected: Diagram drops "Scenario" first, then shortens the name (full text on
hover), then drops the summary; at 960 × 540 with the block library open only
"Scenario" is gone. Split at 960 × 540 leaves out the summary.
Observed: at 960 × 540 as expected in both modes, nothing overlaps or is cut
off (`after-*-960x540.png`). The narrower levels need a board narrower than the
minimum window allows, so they were checked in the harness only, at ten widths
from 1600 to 760 px: `a_narrowing_diagram_row_drops_the_kind_then_shortens_the_name_then_the_summary`
and `at_the_minimum_window_the_row_fits_without_overlap`.

### Scenario 3: names, behaviour and keyboard order (criterion 3)
1. Click Cleanup, then Main, then Add step; then press Tab repeatedly.

Expected: the board shows cleanup, then main; Add step opens its menu; Tab
visits the file name, Main, Cleanup, Add step, Tidy up, Wire style, then the
board.
Observed: as expected in both modes and sizes. Tests:
`main_cleanup_add_step_and_the_tools_keep_their_names_and_behaviour` (it
passed before the change too, as a criterion that must hold),
`tab_runs_from_the_name_through_main_and_cleanup_to_add_step_and_the_tools`.

### Scenario 4: accessibility and design review (criteria 5, 6)
Observed: the accessibility audit `stage_ii_screens_and_dialogs_name_every_control`
passes in Diagram and Split at both sizes; the steps summary now has an
accessible name ("6 steps  ·  All steps"). Design review of
`review_folder_modes_*` (960 × 540, 1600 × 1000 and double scale): the row is
aligned and sharp, nothing overlaps, and the harness captures match the
mock-ups apart from the code editor's box in Split, which KTLO-13 removed after
the mock-ups were made.

## Windows

Not run on Windows by hand. CI runs the desktop suite, including the six
KTLO-12 tests, on the Windows runner and uploads `qa-captures-windows-2022`;
see the pull request's checks. Not accepted for Windows on this report alone.

## Not verified

- The shortened file name and its hover text, and the dropped summary in
  Diagram, in the release binary: the window cannot be narrower than
  960 × 540; covered by the harness tests only.
- A component document (name and kind, no Main / Cleanup) on the new row: no
  criterion; not captured and no test checks its row.
- The narrowing sweep samples ten widths (it opened 22 windows when first
  written); it was reduced after the first implementation and no assertion
  changed.
- Windows, other than the CI suite and captures.

## Regression smoke list

Linux, 2026-10-26, release binary of this branch under Xvfb.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr carried only the Xvfb `libEGL` DRI3 warnings. |
| 2 | Pass: `--folder` opens the project and the scenario in Diagram and Split (`after-*`). |
| 3–8 | Not run by hand: this change moves the board's header only; the desktop suite that covers them passes. |
| 9 | Not run by hand; `apiforge-convert-v1 --fixtures --check` passes. |
| 10 | Pass: at 960 × 540 Diagram and Split show every control (`after-*-960x540.png`). |
