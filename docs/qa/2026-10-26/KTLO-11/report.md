# KTLO-11: Project files in Diagram — the left panel switches between Library and Files — QA report

- Branch: `feat/ktlo-11-library-files-tabs`; captures from the release binary
  of `08f9dc6` (the implementation commit), before `main` (KTLO-24, KTLO-04)
  was merged in
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Compared against mock-up: [docs/tickets/KTLO-11.md](../../../tickets/KTLO-11.md)
  (`mock-*.png` in this directory), approved 2026-10-26 on #14 with one
  change: *Library | Files* are underline tabs like *Main | Cleanup*, not a
  segmented row; decision 1 = A (Diagram only), decision 2 = A (the tab is
  remembered per project)
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test --workspace
  --all-targets --locked` (isolated `XDG_CONFIG_HOME` and `XDG_DATA_HOME`) and
  `apiforge-convert-v1 --fixtures --check`: all pass, on the implementation
  and again after merging `main`. The fifteen KTLO-11 tests passed ten runs
  in a row each time. After the merge the *Refresh* check failed once under
  load, because the scan it starts runs on a worker the harness does not wait
  for; it now steps the window until the row appears.

## Captures

`after-*.png` are from the **release binary** of this branch, started with
`--managed` on an empty data directory (which created the demo shop) under a
private Xvfb with Lavapipe and no window manager. Every state was reached by
**clicking**: a small helper sent real pointer events through the X server's
XTEST extension (presses at the coordinates of *Diagram*, *Files*,
*Collapse project files*, *Show project files*, a row, a tab, …) and captured
the window with the X `GetImage` request; `--screenshot` was not used. After
each click the pointer was parked on an empty part of the board, so no hover
highlight is left on the clicked control. The window opened at 1600 × 1000;
for 960 × 540 the same window was resized through the X server, so the
board's zoom at 960 × 540 is what the window kept, as after a manual resize.

| View | *Library* | *Files* | Rail | Approved mock-up |
| --- | --- | --- | --- | --- |
| Diagram 960 × 540 | [after](after-diagram-library-960x540.png) | [after](after-diagram-files-960x540.png) | [after](after-diagram-rail-960x540.png) | `mock-diagram-files-960x540.png` |
| Diagram 1600 × 1000 | [after](after-diagram-library-1600x1000.png) | [after](after-diagram-files-1600x1000.png) | [after](after-diagram-rail-1600x1000.png) | `mock-diagram-library-1600x1000.png`, `mock-diagram-files-1600x1000.png`, `mock-diagram-rail-1600x1000.png` |

Further captures at 1600 × 1000: a component opened from *Files*
([after](after-diagram-files-opened-1600x1000.png)), the scenario chosen
again in the tab strip ([after](after-diagram-files-follows-1600x1000.png)),
the project reopened on *Files* ([after](after-reopen-files-1600x1000.png))
and Split, unchanged ([after](after-split-1600x1000.png)).

Against the mock-up: the tab row is underline tabs, as the owner asked, at
the panel's top left with the chevron at its right, level with *Main |
Cleanup* beside it; *Files* holds the explorer of Code with `EXPLORER`, *+*,
the bin and refresh; the rail holds the panel icon and the folder icon. The
tab row takes the place of the `BLOCK LIBRARY 10` caption, as in the
mock-up, so the block count is no longer shown in Diagram (Split still shows
it). The mock-up's tree has `components` expanded; the captures show the
tree as the project left it.

## Linux

### Scenario 1: the two tabs (criterion 1)
1. Open the demo shop, choose Diagram.
2. Click *Files*, then *Library*.

Expected: *Library* and *Files* on the panel's caption row, *Library*
underlined by default and showing the block sections; *Files* showing the
explorer with the rows Code shows.
Observed: as expected at both sizes (`after-diagram-library-*`,
`after-diagram-files-*`). In the harness the rows' names on *Files* equal
Code's, top to bottom, at both sizes. Split has no tabs. Test:
`ktlo11_the_panel_has_library_and_files_tabs_with_the_rows_of_code`.

### Scenario 2: opening from *Files* (criterion 2)
1. On *Files*, expand `components`, click `add-to-cart.afs`.
2. Click the scenario's row; click a text file's row.

Expected: each opens in its tab, the board shows it, the mode stays Diagram;
a text file opens its text tab with the "has no diagram" message.
Observed: in the release binary `add-to-cart.afs` opened as a second tab and
its board was shown in Diagram, its row selected
(`after-diagram-files-opened-1600x1000.png`). In the harness the same for a
component, the scenario (its one tab kept) and `README.md`, whose board says
"README.md is a text file, not a scenario or component, so it has no
diagram."; the panel stays on *Files* throughout. Tests:
`ktlo11_files_opens_documents_and_text_without_leaving_diagram`,
`ktlo11_one_click_on_files_opens_a_file_and_the_project_row_is_its_own_destination`.

### Scenario 3: the explorer's toolbar and keys (criterion 3)
1. On *Files* and in Code: *New* → *New scenario* on `components`; *Trash*;
   *Refresh* after writing a file; Delete on the project row and on a file;
   the arrow keys, Enter and F2.

Expected: the same result in both places, and a key meant for the explorer
never deletes the block selected on the board.
Observed: as expected in the harness at both sizes. The existing click and
keyboard tests of the explorer now also run on *Files*. One difference was
found by the tests and fixed: on *Files*, Delete or Backspace on a row the
explorer does not delete (the project row) reached the board and deleted its
selected block; the explorer now keeps both keys while it has the keyboard.
Tests: `ktlo11_the_explorer_on_files_does_what_it_does_in_code`,
`ktlo11_the_explorer_on_files_is_driven_by_the_keyboard`,
`ktlo11_the_keyboard_on_files_reaches_the_project_row_and_opens_a_text_file`.

### Scenario 4: the rail (criterion 4)
1. On *Files*, click the chevron; click *Show project files*.
2. Collapse again, click *Show block library*; Ctrl+B twice on *Files*.

Expected: the rail holds both buttons; each expands the panel on its tab;
Ctrl+B and the chevron keep the chosen tab.
Observed: as expected (`after-diagram-rail-*`). In the release binary the
first click on *Show project files* at 1600 × 1000 left the rail as it was;
afterwards collapse and *Show project files* worked three times out of three
at 960 × 540 and, after restarting the binary, three times out of three at
1600 × 1000. The machine was under a load of 10–15 from other builds and the
harness test passes ten runs out of ten, so this reads as a click the
synthetic driver lost, but that was not proven. Split's rail keeps its one
button. Test: `ktlo11_the_rail_brings_the_panel_back_on_either_tab`.

### Scenario 5: the selection follows the tab strip (criterion 5)
1. On *Files*, open `add-to-cart.afs`, then click the scenario's tab in the
   strip.

Expected: the explorer selects the scenario's row.
Observed: as expected (`after-diagram-files-follows-1600x1000.png`). Test:
`ktlo11_the_selection_follows_the_tab_strip_on_files`.

### Scenario 6: the tab is remembered per project (criterion 6)
1. Choose *Files*, quit, start again. Inspect `.apiforge/local/workspace.json`.

Expected: the project reopens in Diagram on *Files*; a view state without
the field opens on *Library*; an older build loads a file with it.
Observed: as expected. The field is `panels.libraryFiles`, written only
while *Files* is chosen: the release binary's view state held
`"libraryFiles": true`, and the restarted binary opened on *Files*
(`after-reopen-files-1600x1000.png`; its tooltip is the pointer left where
the previous session's last click was). Tests:
`ktlo11_the_chosen_tab_comes_back_after_reopening_the_project`,
`the_files_tab_round_trips`,
`a_view_state_without_the_files_tab_opens_on_the_library`,
`a_view_state_with_the_files_tab_loads_in_a_build_without_it`.

### Scenario 7: accessibility and the minimum window (criterion 7)
Expected: the audit passes with each tab and with the rail; at 960 × 540 the
panel's controls on *Files*, the board toolbar and the inspector are inside
the window.
Observed: as expected. Tests: `the_library_and_files_tabs_name_every_control`,
`ktlo11_the_files_tab_and_its_neighbours_fit_the_window`. Design review of
`review_library_files` and `review_library_rail` (harness captures, both
sizes): nothing cut off or overlapping; long names in the tree are shortened
with the full path on hover, as in Code.

### Scenario 8: the width (ticket, *Changed*)
Expected: in Diagram the panel widens to 420 px, one width for both tabs;
Split keeps 320 px.
Observed: as expected in the harness; a visit to Split does not narrow the
width Diagram comes back to. Test: `ktlo11_the_panel_widens_to_420_in_diagram_only`.

### Behaviour the ticket did not spell out
- The panel is drawn in Diagram whatever the active tab, so *Files* stays
  beside a text file and beside an empty board (KTLO-20's drawing moves right
  by the panel's width). Without a scenario or component, *Library* says
  "Open a scenario or component to add its blocks."
- While *Files* is shown the library is not in view, so the *Add step* popup
  lists Variables and Operators and the empty board's hint offers only the
  first step, as with the rail (KTLO-17).
- The chevron and the resize handle are named for the tab:
  *Collapse project files* and *Resize project files* on *Files*.

## Windows

Not run on Windows by hand. CI run 40513418762 on `ca8f12f` (the branch
after merging `main`) passed on `windows-2022`, including the desktop suite
with the KTLO-11 tests. Its `qa-captures-windows-2022` artifact holds all six
standard captures; the CI step does not click, so Diagram is on *Library*, as
a new project opens. [windows-diagram-960x540.png](windows-diagram-960x540.png)
shows the *Library | Files* tabs and the chevron on the panel's top row, as
on Linux (with a component's tooltip under the runner's pointer). The three
`*-1600x1000.png` captures are 1024 × 768, because the runner's display is
smaller (warnings on the run), so they are not cited as 1600 × 1000. Not
accepted for Windows on this report alone.

## Not verified

- Keyboard-only switching between *Library* and *Files* (Tab to a tab, then
  Space or Enter) in the release binary; the audit checks that the tabs are
  named, not where Tab reaches them.
- In the release binary: *New*, *Trash*, *Refresh*, Delete and the arrow keys
  on *Files*, and clicking a text file (the harness tests cover them).
- Whether the one lost rail click (scenario 4) was the driver or the window.
- Double scale (960 × 540 at 2×) design review.
- Windows, other than the CI suite and captures.

## Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb. Items 2, 3 and
10 are the ones this change touches.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr was empty in both sessions. |
| 2 | Pass: the explorer shows the tree in Code (`--managed` opens in Code) and on *Files* in Diagram; a component opened from *Files*. |
| 3 | Not run by hand on the release binary. The interaction tests that add a block from the library panel by click and by drag pass; the panel opens on *Library*. |
| 4–9 | Not run by hand for this change; the workspace suite that covers them passes, and so does `apiforge-convert-v1 --fixtures --check`. |
| 10 | Pass: at 960 × 540 in Diagram on *Library*, on *Files* and with the rail (`after-diagram-*-960x540.png`) every control is visible; Code and Split are not changed by this ticket. |
