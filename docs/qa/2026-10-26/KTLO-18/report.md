# KTLO-18: The Problems section expands over the code with one click — QA report

- Branch: `feat/ktlo-18-expand-problems` (the PR head commit; the captures were
  taken from the release build of `1e7feb6`, the last code commit)
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Compared against mock-up: [docs/tickets/KTLO-18.md](../../../tickets/KTLO-18.md)
  (`mock-*.png` in this directory), approved as a whole; decision 1 = A (the
  button), decision 2 = Code and Split
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test --workspace
  --all-targets --locked` (isolated `XDG_CONFIG_HOME` and `XDG_DATA_HOME`) and
  `apiforge-convert-v1 --fixtures --check`; see "Verification" below for the
  runs and their load.

## Two defects on `main` this change had to fix

Both were found because the acceptance tests failed against an implementation
that did what the ticket's appendix described.

1. **The Problems section had no resize handle.** egui's bottom panel is not
   resizable unless asked, and the code never asked; the name "Resize
   problems" was attached to nothing. The ticket ("the resize handle keeps
   working in both states") and criterion 4 assume a handle, so the section is
   now resizable, between 72 and 360 px as its range always said. **This is
   new on screen in both states**: hovering the top edge of the section shows
   the resize cursor and a dark line, and dragging it changes the height. With
   "No problems." the section is as tall as its text and a drag does not hold,
   as the panel hugged its content before.
2. **Clicking a problem did not keep its selection or the keyboard.** The
   editor, drawn after the list, saw the click as a click outside itself and
   gave up focus in the same frame; the selection then collapsed to the end
   of the range. On `main` the row put the caret at the end of the problem and
   left the editor unfocused. The selection and focus are now requested after
   the editor is drawn, so a click selects the whole range and the editor
   keeps the keyboard, which is what criterion 3 calls "as today".

## Captures

All `after-*.png` are from the **release binary** of the branch on a private
Xvfb display with Lavapipe, on the demo shop that `--managed` created in an
empty data directory, with ten lines `extra_a = missing.total` …
`extra_j = missing.total` added before the scenario's closing brace (lines
26–35, "Unknown block handle" at column 13, as in the mock-up).

- `after-code-before-960x540.png`: `--screenshot … --folder … --mode code
  --size 960x540`.
- The expanded captures cannot come from `--screenshot`, which cannot click,
  and the expanded state is deliberately not remembered. They were taken from
  the same release binary started with `--folder` (no `--screenshot`), driven
  by real pointer input: a small X client outside the repository (x11rb with
  the XTEST extension) moved the pointer, pressed and released the button on
  *Expand problems*, the *Split* tab and a problem row, dragged the section's
  edge, resized the window with `ConfigureWindow`, and read the window back
  with `GetImage`. The window was 1600 × 1000 when it opened and was resized
  to 960 × 540 and back; the Split capture was taken after growing the window
  back to 1600 × 1000 while expanded.

| View | Capture | Approved mock-up |
| --- | --- | --- |
| Code 960 × 540, before | [after](after-code-before-960x540.png) | `mock-code-before-960x540.png` |
| Code 960 × 540, expanded | [after](after-code-expanded-960x540.png) | `mock-code-expanded-960x540.png` |
| Code 1600 × 1000, expanded | [after](after-code-expanded-1600x1000.png) | `mock-code-expanded-1600x1000.png` |
| Split 1600 × 1000, expanded | [after](after-split-expanded-1600x1000.png) | `mock-split-expanded-1600x1000.png` |
| Code 1600 × 1000, expanded, line 30's problem clicked | [after](after-code-expanded-reveal-1600x1000.png) | — |
| Code 1600 × 1000, expanded, edge dragged 200 px down | [after](after-code-expanded-dragged-1600x1000.png) | — |

Differences from the mock-up: the code strip shows five whole lines and no
cut sixth line (the mock-up's 96 px strip cut through its sixth line; the
implementation rounds to whole lines, as the ticket's rendering note asks:
five lines of the 13 px code font, 95.25 px with the margins), and the fifth
line is drawn lighter, as the editor fades whichever line meets the bottom of
its panel. At 960 × 540 the expanded list shows eight of ten problems with a
scrollbar (the mock-up: seven and a cut eighth).

## Linux

### Scenario 1: *Expand problems* (criterion 1)
1. Open the demo shop with ten problems in Code at 960 × 540 and at 1600 × 1000.
2. Click the chevron at the right end of the `PROBLEMS` row, after
   "Saved · 10 errors".

Expected: the section takes the code panel's height less a strip of about
96 px; the code is still visible above it.
Observed: as expected and as in the mock-up. The code panel above the section
is 95.25 px at both sizes (criterion 1's ±1 px of 96); lines 1–5 are whole and
numbered (`after-code-expanded-*`). In the harness the strip from the editor's
top to the section's fill is 90.25 px at both sizes. Test:
`expanding_the_problems_section_keeps_a_strip_of_code_above_it`.

### Scenario 2: *Restore problems* and the view state (criterion 2)
1. Expand, then click the chevron again, now named *Restore problems*.
2. Inspect `.apiforge/local/workspace.json` while expanded; close the window
   while expanded and open the project again.

Expected: the section returns to the height it had; `panels.diagnostics`
holds that height, never the expanded one.
Observed: as expected. In the release binary the section's top edge was at
y = 868 before, and at 868 again after expanding, dragging and restoring
(1600 × 1000, section 132 px). While expanded the view state held
`"diagnostics": 132.0`; the window was closed while expanded and a
`--screenshot` of the project reopened it at 132 px, not expanded. Tests:
`restoring_the_problems_section_returns_the_height_it_had`,
`the_expanded_height_is_never_written_to_the_view_state`.

### Scenario 3: a problem row while expanded (criterion 3)
1. Expand at 1600 × 1000 and click "Line 30, column 13 Unknown block handle".

Expected: the editor selects the problem's range and has the keyboard.
Observed: as expected: `missing.total` on line 30 is selected and scrolled into
the strip (`after-code-expanded-reveal-1600x1000.png`). The same click on
`main` leaves only a caret and no focus (defect 2 above). Test:
`clicking_a_problem_while_expanded_still_reveals_its_line` (anchor and caret
equal the diagnostic's range; the editor has focus).

### Scenario 4: dragging while expanded (criterion 4)
1. Expand at 1600 × 1000, drag the section's top edge 200 px down, click
   *Restore problems*.

Expected: the drag changes the height; Restore returns the pre-expansion height.
Observed: as expected: the edge moved from y = 185 to 386, and Restore put it
back at 868. Until the edge is dragged, an expanded section follows the
window: grown from 960 × 540 to 1600 × 1000 while expanded, its top edge was
at y = 185 again, the same strip as when expanded at 1600 × 1000. After a
drag it keeps the dragged height until Restore. Test:
`dragging_while_expanded_works_and_restore_returns_the_earlier_height`.

### Scenario 5: Split (criterion 5)
1. While expanded, switch to Split at 1600 × 1000.

Expected: the code side behaves as in Code; the board shows its "code has
errors" banner.
Observed: as expected and as in the mock-up (`after-split-expanded-1600x1000.png`).
Split at 960 × 540 was checked by the test only. Test:
`the_problems_section_expands_in_split_too` (both sizes: expand, strip,
restore).

### Scenario 6: accessibility and the minimum window (criterion 6)
Expected: the audit passes in both states; at 960 × 540 the editor's first
line and the expanded list are inside the window.
Observed: as expected. The audit draws Code, Split and Diagram, then Code and
Split expanded, at both sizes: every control is named, including *Expand
problems*, *Restore problems* and the handle "Resize problems". Line 1's
number, `PROBLEMS` and *Restore problems* are inside the window at both sizes.
Tab reaches *Expand problems* and "Resize problems" before the editor, because
egui orders Tab by drawing and the section is drawn before the code above it,
as the problem rows and *Apply fix* already were. Tests:
`stage_ii_screens_and_dialogs_name_every_control`,
`expanding_the_problems_section_keeps_a_strip_of_code_above_it`,
`tab_visits_the_window_in_reading_order_with_named_stops`.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  --locked -- -D warnings` and `apiforge-convert-v1 --fixtures --check`: pass
  at `1e7feb6`.
- `cargo test --workspace --all-targets --locked`: every crate passed at
  `6c5ae7a` (desktop: 367 passed, 19 ignored).
- At `1e7feb6`, under a load average of 15–23 on this 3-core machine from
  other builds, every crate but the desktop passed; the desktop had 365 passed
  and 2 failed: `a_large_source_follows_the_caret_once_its_edit_is_analysed`
  ("storage did not become idle") and
  `failed_recovery_keeps_the_current_project_until_the_user_decides`. Both
  passed three runs in three alone right after. The first is the 15-second
  opening wait recorded in KTLO-12's ticket as a pre-existing race under load;
  neither test expands the section.
- The same load found a real KTLO-18 defect first:
  `the_problems_section_expands_in_split_too` failed 2 runs in 12 at
  960 × 540 (strip 128.25 px instead of 90.25), because the expanded height
  was taken once, on the first frame after the click. `1e7feb6` makes the
  expanded section take the ceiling every frame until its edge is dragged;
  afterwards the KTLO-18 tests, the audit and the Tab-order test passed 22
  runs in 22 under that load (12, then the ten the process asks for).

## Windows

Not run on Windows by hand. CI runs the desktop suite, including the KTLO-18
tests, on the Windows runner and uploads the standard captures
(`qa-captures-windows-2022`; the CI step does not expand the section, and the
demo shop has no problems); see the pull request's checks. Not accepted for
Windows on this report alone.

## Not verified

- Keyboard-only use of the chevron and the handle (Enter or Space on the
  button, arrow keys on the handle): the audit checks that they are named, the
  Tab test where Tab reaches them; nothing presses them from the keyboard.
- Resizing the window while expanded is checked in the release binary only
  (it follows the window until the edge is dragged); there is no test for it.
- Split at 960 × 540 in the release binary (the test covers it).
- Double scale (960 × 540 at 2×).
- The expanded captures use a pointer driver outside the repository; it is not
  a project tool and is not committed.
- Windows, other than the CI suite and captures.

## Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb. Items 2, 5 and 10
are the ones this change touches.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr was empty. |
| 2 | Pass: `--folder` opens the project; the scenario opens in Code (`after-code-before-960x540.png`). |
| 3–4 | Not run by hand for this change; the workspace suite that covers them passes. |
| 5 | Partly: invalid source holds the board with its banner in Split (`after-split-expanded-1600x1000.png`); the 500 ms follow was not timed. |
| 6–9 | Not run by hand for this change; the workspace suite that covers them passes, and so does `apiforge-convert-v1 --fixtures --check`. |
| 10 | Pass for Code at 960 × 540, before and expanded: every control is visible. Diagram and Split at 960 × 540 were not captured for this change. |
