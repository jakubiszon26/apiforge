# KTLO-13: The code editor fills its panel without a bordered box — QA report

- Branch: `feat/ktlo-13-editor-no-box` (the PR head commit)
- Built with: `cargo build --release --locked -p apiforge-desktop`
- Compared against mock-up: [docs/tickets/KTLO-13.md](../../../tickets/KTLO-13.md),
  variant A (`mock-a-*.png` in this directory), focus option 2
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test -p apiforge-desktop
  --locked` and `apiforge-convert-v1 --fixtures --check`; output in the pull
  request.

## Captures

`after-*.png` are from the **release binary** (`--screenshot … --folder …
--mode … --size …` under Xvfb). The project is the managed demo shop with its
scenario description lengthened so line 2 is wider than either window, as in
the mock-up. For `after-code-scrolled-*` the project's saved view state puts
the caret at the end of line 2; the release binary restores it and scrolls
there. `harness-focus-960x540.png` is an `egui_kittest` **harness** capture
(design review aid, not QA evidence): a screenshot cannot give the editor
keyboard focus, so the focus line is shown only there.

| View | Before | Approved mock-up | After (release) |
| --- | --- | --- | --- |
| Code 960 × 540 | [before](before-code-960x540.png) | [mock A](mock-a-code-960x540.png) | [after](after-code-960x540.png) |
| Code 1600 × 1000 | [before](before-code-1600x1000.png) | [mock A](mock-a-code-1600x1000.png) | [after](after-code-1600x1000.png) |
| Long line scrolled 960 × 540 | [before](before-code-scrolled-960x540.png) | [mock A](mock-a-code-scrolled-960x540.png) | [after](after-code-scrolled-960x540.png) |
| Long line scrolled 1600 × 1000 | [before](before-code-scrolled-1600x1000.png) | [mock A](mock-a-code-scrolled-1600x1000.png) | [after](after-code-scrolled-1600x1000.png) |
| Split 960 × 540 | [before](before-split-960x540.png) | [mock A](mock-a-split-960x540.png) | [after](after-split-960x540.png) |
| Split 1600 × 1000 | [before](before-split-1600x1000.png) | [mock A](mock-a-split-1600x1000.png) | [after](after-split-1600x1000.png) |
| Focus line 960 × 540 | — | — (option 2, no capture) | [harness](harness-focus-960x540.png) |

## Linux

### Scenario 1: the editor has no box and fills its panel (criterion 1)
1. Open the demo shop folder project in Code, then in Split, at 960 × 540 and
   1600 × 1000.

Expected: no border or fill around the code; the editor reaches the Problems
bar.
Observed: as expected, and as in mock A. At 1600 × 1000 the short scenario
leaves white space below line 27 that belongs to the editor (a click there
places the caret); there is no box edge anywhere. Test:
`the_editor_paints_no_box_and_reaches_the_bottom_of_its_panel` (Code and Split,
both sizes: no filled or stroked rectangle at the text area, its bottom within
the code panel's 6 px margin of the Problems bar).

### Scenario 2: a long line scrolls under fixed line numbers (criterion 2)
1. Open the project whose line 2 is wider than the panel.
2. Put the caret at the end of line 2 (click on it, End).

Expected: the text scrolls sideways; the line numbers stay at the panel's left
edge.
Observed: as expected, and as in mock A (`after-code-scrolled-*`). Characters
that pass under the gutter are hidden by it; the column of text right at the
gutter's edge can show part of a glyph, as the mock-up does. Test:
`a_long_line_scrolls_under_line_numbers_that_stay_at_the_left_edge` (both
sizes: caret at the end of the line, the text area moved more than 100 px left,
every line number at the same x as before and inside the panel).

### Scenario 3: Home brings the start of the line back
1. As scenario 2, then press Home.

Expected: the line's first characters are beside the gutter, not under it.
Observed: as expected. Not a ticket criterion: without a correction the text
edit scrolled its caret only to the panel's edge, which the gutter now covers,
leaving the first three characters hidden (22 px). Test:
`home_on_a_scrolled_long_line_shows_its_start_beside_the_gutter`.

### Scenario 4: keyboard focus is visible (criterion 3)
1. Click in the editor; then press Escape.

Expected: a thin line in the gutter while the editor has focus, none after.
Observed: as expected: a 2 px line in the accent colour (the colour the box's
focus stroke had) along the gutter's right edge, the full height of the panel
([harness capture](harness-focus-960x540.png)); it disappears on Escape. Test:
`a_focus_line_shows_in_the_gutter_only_while_the_editor_has_focus`.

### Scenario 5: text files and the legacy code panel (criterion 4)
Expected: they still render and edit.
Observed: the whole desktop suite passes, including the text file and legacy
code panel tests. Both use the widget with line numbers on, so they lose the
box the same way. The request body field in the inspector uses the same
widget without line numbers; it is a form field and keeps its box.

### Scenario 6: accessibility and design review (criterion 5)
Observed: the accessibility suite (`folder/accessibility_tests.rs`, whose tab
order reaches "Contents of …") passes. Design review checklist on the harness
captures of `review_code_editor`: alignment as in the mock-up; nothing
overlaps; keyboard focus visible (scenario 4); the minimum window keeps every
control. Double scale was not reviewed for this change.

## Windows

Not run on Windows by hand. CI runs the desktop suite, including the KTLO-13
tests, on the Windows runner and uploads the standard captures
(`qa-captures-windows-2022`); see the pull request's checks. Not accepted for
Windows on this report alone.

## Not verified

- Focus line in the release binary: a `--screenshot` cannot focus the editor;
  shown by the harness capture and the interaction test only.
- Clicking on the gutter while a line is scrolled places the caret in the text
  hidden beneath it; not changed by this ticket.
- Double scale (960 × 540 at 2×) design review.
- Windows, other than the CI suite and captures.

## Regression smoke list

Linux, 2026-10-26, release binary of this branch under Xvfb.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr carried only the Xvfb `libEGL` DRI3 warnings. |
| 2 | Pass: `--folder` opens the explorer tree and the scenario in Code (`after-code-*`). |
| 3–8 | Not run by hand for this change, which touches only how the code editor is drawn; the desktop suite that covers them passes. |
| 9 | Not run by hand; `apiforge-convert-v1 --fixtures --check` passes. |
| 10 | Pass: at 960 × 540 Code, Split (`after-*-960x540.png`) and Diagram show every control. |
