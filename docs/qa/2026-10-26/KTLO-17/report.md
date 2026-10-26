# KTLO-17: The block library collapses to a rail in Diagram and Split — QA report

- Branch: `feat/ktlo-17-library-rail` (the PR head commit)
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Compared against mock-up: [docs/tickets/KTLO-17.md](../../../tickets/KTLO-17.md)
  (`mock-*.png` in this directory, from the docs pull request), approved as a
  whole; decision 1 = A, decision 2 = Ctrl+B
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test --workspace
  --all-targets --locked` (isolated `XDG_CONFIG_HOME` and `XDG_DATA_HOME`) and
  `apiforge-convert-v1 --fixtures --check`: all pass.

## Captures

`after-*.png` are from the **release binary** (`--screenshot … --folder …
--mode … --size …` under Xvfb with Lavapipe), on the demo shop that `--managed`
created in an empty data directory. A screenshot cannot click, so the collapsed
captures were taken after setting `panels.libraryCollapsed` to `true` in the
project's `.apiforge/local/workspace.json`, which is what the window writes when
the library is collapsed (scenario 6); the binary restored it and wrote it back.

| View | Expanded | Collapsed | Approved mock-up |
| --- | --- | --- | --- |
| Diagram 960 × 540 | [after](after-diagram-expanded-960x540.png) | [after](after-diagram-collapsed-960x540.png) | `mock-diagram-collapsed-960x540.png` |
| Diagram 1600 × 1000 | [after](after-diagram-expanded-1600x1000.png) | [after](after-diagram-collapsed-1600x1000.png) | `mock-diagram-collapsed-1600x1000.png`, `mock-diagram-expanded-1600x1000.png` |
| Split 960 × 540 | [after](after-split-expanded-960x540.png) | [after](after-split-collapsed-960x540.png) | — (unchanged below 1440 px) |
| Split 1600 × 1000 | [after](after-split-expanded-1600x1000.png) | [after](after-split-collapsed-1600x1000.png) | `mock-split-collapsed-1600x1000.png` |

## Linux

### Scenario 1: the chevron folds the library to a rail (criterion 1)
1. Open the demo shop in Diagram at 960 × 540 and at 1600 × 1000.
2. Click *Collapse block library*, the chevron at the right of `BLOCK LIBRARY 10`.

Expected: the panel becomes a rail at most 40 px wide with one button, *Show
block library*; the board starts at the rail's right edge.
Observed: as expected and as in the mock-up: the rail is 36 px with the panel
icon at its top, and in the harness the board's left edge moves from 248 to
44 px (the harness's own 8 px margin plus the rail). The chevron sits on the
caption row, inside the panel (`after-diagram-expanded-*`). Test:
`ktlo17_collapsing_the_library_leaves_a_rail_and_widens_the_board`.

### Scenario 2: the panel comes back at its width (criterion 2)
1. In Diagram, widen the library by 60 px with its resize handle.
2. Collapse it, then click *Show block library*; then press Ctrl+B twice.

Expected: each time the panel returns at the widened width; Ctrl+B folds and
unfolds it.
Observed: as expected; the edge returns within 1 px. Test:
`ktlo17_the_library_comes_back_at_its_width_by_button_and_by_ctrl_b`.

### Scenario 3: Split at 1440 px and wider, and below (criterion 3)
1. Split at 1440 × 900 and at 1600 × 1000: chevron, rail button, Ctrl+B.
2. Split at 1439 × 900 and at 960 × 540: look for the panel and the rail,
   press Ctrl+B, open *Add step*.

Expected: wide, the same behaviour with the rail between the code and the
board; narrow, neither panel nor rail, Ctrl+B does nothing, and the popup lists
Variables and Operators as after KTLO-05d.
Observed: as expected. The release binary at 960 × 540 with the flag set shows
Split exactly as before (`after-split-collapsed-960x540.png`). Test:
`ktlo17_split_collapses_when_wide_and_is_unchanged_when_narrow`.

### Scenario 4: the *Add step* popup while collapsed (criterion 4)
1. Collapse the library (Diagram at both sizes, Split at 1600 × 1000) and open
   *Add step*; close it, show the library, open it again.

Expected: collapsed, the popup lists `VARIABLES`, *Add variable*, `OPERATORS`
and the operator chips; expanded, it lists only the steps.
Observed: as expected. Test:
`ktlo17_the_popup_carries_the_library_while_it_is_collapsed`.

### Scenario 5: the empty board's hint (criterion 5)
1. Open a project with an empty Cleanup board; show Cleanup; collapse, then
   show the library.

Expected: "Add the first step." while collapsed; "Drag a block from the library
or add the first step." while expanded.
Observed: as expected in Diagram at both sizes and in Split at 1600 × 1000. The
popup and the hint read one predicate, `library_in_view`. Test:
`ktlo17_the_empty_board_hint_follows_the_collapsed_library`.

### Scenario 6: the collapsed state is remembered per project (criterion 6)
1. Collapse the library in Diagram, close the project, open it again.
2. Inspect `.apiforge/local/workspace.json`.

Expected: the project reopens with the rail, in Diagram and in Split; a view
state without the field opens expanded; a build without the field loads a file
with it.
Observed: as expected. The field is `panels.libraryCollapsed` and is written
only while collapsed: the release binary's view state after the expanded
captures had no such field, and after the collapsed captures it kept
`"libraryCollapsed": true`. A file with the field parses with KTLO-05c's panel
sizes, which ignore unknown fields. Tests:
`ktlo17_the_collapsed_library_comes_back_after_reopening_the_project`,
`the_library_collapsed_flag_round_trips`,
`a_view_state_without_the_collapsed_flag_opens_expanded`,
`a_view_state_with_the_collapsed_flag_loads_in_a_build_without_it`.

### Scenario 7: accessibility and the minimum window (criterion 7)
Expected: the audit passes expanded and collapsed in Diagram and Split; at
960 × 540 the rail, the board toolbar and the inspector are inside the window.
Observed: as expected. Tests:
`the_block_library_names_every_control_expanded_and_collapsed` (audit at both
sizes, the library folded by Ctrl+B) and
`ktlo17_the_rail_and_its_neighbours_fit_the_window`. Design review of
`review_library_rail` (harness captures, Diagram and Split, both sizes, and the
caption row): nothing cut off or overlapping; the rail's icon lines up with
the board header's row; the capture at 960 × 540 matches the mock-up.

## Windows

Not run on Windows by hand. CI runs the desktop suite, including the KTLO-17
tests, on the Windows runner and uploads the standard captures
(`qa-captures-windows-2022`, expanded only: the CI step does not collapse the
library); see the pull request's checks. Not accepted for Windows on this
report alone.

## Not verified

- Clicking the chevron, the rail button and Ctrl+B in the release binary: a
  `--screenshot` cannot click or type; the interaction tests cover them, and
  the collapsed captures come from the view state the window writes.
- Double scale (960 × 540 at 2×) design review.
- The rail button's place in the Tab order (the audit checks that it is
  named, not where Tab reaches it).
- Windows, other than the CI suite and captures.

## Regression smoke list

Linux, 2026-10-26, release binary of this branch under Xvfb. Items 3 and 10
are the ones this change touches.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr was empty. |
| 2 | Pass: `--folder` opens the project; the captures show the scenario's tab. |
| 3 | Not run by hand on the release binary (a capture cannot click). The interaction tests that add a block from the library panel by click and by drag pass (`clicking_a_block_adds_it_at_the_visible_centre_selected_and_dirty`, `dragging_a_block_onto_the_board_adds_it_at_the_drop_point`); with the library collapsed the popup adds blocks (`ktlo17_the_popup_carries_the_library_while_it_is_collapsed`). |
| 4–9 | Not run by hand for this change; the workspace suite that covers them passes, and so does `apiforge-convert-v1 --fixtures --check`. |
| 10 | Pass: at 960 × 540, Diagram and Split, expanded and collapsed (`after-*-960x540.png`), every control is visible; Code is not touched by this ticket. |
