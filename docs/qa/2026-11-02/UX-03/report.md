# UX-03: The focus line turns grey; error marks appear after a pause in typing — QA report

- Branch: `docs/ux-03-grey-focus-line` (pull request #245). Captures are
  from the release binary of `4ca66c5`.
- Built with: `cargo build --release --locked -p apiforge-desktop`
  (Rust 1.99.0).
- Compared against mock-up: the approved UX-00 captures `ux03-now-*` and
  `ux03-no-focus-line-*` (`mock/ux-00` at `2c1fa79`), copied to
  [`../UX-00/`](../UX-00/); the ticket is
  [`docs/tickets/UX-03.md`](../../../tickets/UX-03.md).
- Analysis model: Fable. Implementation model: Claude Opus 5.5.
- Acceptance tests first: commit `4e6de42`. Before the implementation
  every `ux03_criterion_*` test but criterion 3's failed for the right
  reason: the focus line was the accent (`#E05927`), lines 3 and 5 were
  marked 0, 300 and 699 ms after the keystroke, an error corrected within
  the pause was marked, `marks_due` was true at 699 ms and no repaint was
  asked for the marks. Criterion 3 is a regression test and passed before
  and after. The same commit changed KTLO-13's focus-line test to look for
  the grey line, the change criterion 1 makes. The implementation commit
  changed no test.
- Verification: at the end of this report.

## What changed

`code.rs::paint_gutter` draws the focus line with
`Stroke::new(2.0, theme::MUTED)` instead of the selection colour, the
accent. `folder/code_view.rs` keeps, per document, the instant the editor
last changed its text (`marks_quiet_at`); until `MARKS_DELAY` (700 ms) has
passed since then it gives the editor no diagnostics, so the gutter shows
plain numbers, the text no underline and no message on hover, and it asks
egui to draw again when the marks are due. Typing, a paste, an undo and a
redo in the editor all reach the same `outcome.changed` path and restart
the pause; a board edit, a caret move, a selection and a scroll do not.
The analyser, its 250 ms pause, the PROBLEMS caption and list and the
board are untouched.

## Criteria and their evidence

Tests are in `crates/apiforge-desktop/src/folder/editor_tests.rs`,
`mod ux03`, on the demo shop in Code at 960 × 540. The marks' clock is
held and stepped by hand (`FolderApp::set_marks_clock`); no test waits on
the real clock for the marks.

| # | Criterion | Test | Release binary |
| --- | --- | --- | --- |
| 1 | The focus line is 2 px grey while the editor has focus, never red, absent without focus | `ux03_criterion_1_the_focus_line_is_grey` (and KTLO-13's `a_focus_line_shows_in_the_gutter_only_while_the_editor_has_focus`) | Scenario 1 |
| 2 | No marks during a pause shorter than 700 ms although PROBLEMS counts the errors; an error corrected within the pause is never marked; the marks of the analysis appear at 700 ms | `ux03_criterion_2_no_marks_while_typing`, `ux03_criterion_2_an_error_corrected_within_the_pause_shows_no_mark`, `ux03_criterion_2_marks_follow_the_analysis_after_the_pause` | Scenarios 2 and 3 |
| 3 | The PROBLEMS status and list unchanged; the analysis pause still 250 ms | `ux03_criterion_3_the_problems_status_is_unchanged`; the existing Problems tests | Scenarios 2 and 3 |
| 4 | Matches `ux03-now` and `ux03-no-focus-line` at both sizes | this report; `review_ux03_editor_typing` | *Against the mock-up* |
| 5 | `marks_due` false at 699 ms, true at 700; a repaint is asked for the moment the marks are due | `ux03_criterion_5_marks_due_at_the_delay`, `ux03_criterion_5_a_repaint_is_requested_for_the_marks` | Scenario 3 (the marks appear with the pointer parked) |
| 6 | Typing, paste, undo and redo restart the pause; caret moves, selecting and scrolling do not | `ux03_criterion_6_only_text_changes_restart_the_pause` | typing only (scenario 3) |
| 7 | `APPLY_DELAY` still 250 ms; ADR 0007's board tests and `mod mcp16` pass unchanged | criterion 3's test; the workspace suite | — |

The design review view `review_ux03_editor_typing` (`tools/gui/review.sh
review_ux03`) renders the focused editor, the statement half typed with the
analysis done, and the marks after the pause, at both sizes, against the
real editor. It replaces the mock-up's `mock_ux00` switches, which are not
merged. It was rendered on `4ca66c5` and looked at: apart from the grey
line, its marks state is the mock-up's `ux03-now`.

## How the release binary was driven

Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver, X11 backend,
scratch configuration, data and runtime directories. Clicks and keys went
through the X test extension (libXtst from the Ubuntu archive, unpacked
into a scratch directory, not installed), one key event per character, so
the window saw real typing; the screen was grabbed from the display at
measured times after the last key event. After a click the pointer is
parked at the window's bottom-left corner, where it hovers nothing, so no
capture below depends on a mouse move. Each size is a fresh window on a
fresh `prepare_project` of the demo shop
(`apiforge_core::demos::shop_project`), the project the tests use; the
960 × 540 window is resized through Xlib after it opens at 1600 × 1000.

## Linux

### Scenario 1 (criterion 1): the focus line

1. Open the shop. Click in the code (line 2 at 1600 × 1000, line 3 at
   960 × 540) and press End.
2. Later (after scenario 2), press Escape.

Expected: after step 1 a 2 px grey line at the gutter's right edge, from
the top of the code to the PROBLEMS bar; after step 2 none.

Observed: as expected.
[focused 1600 × 1000](ux03-focused-1600x1000.png),
[focused 960 × 540](ux03-focused-960x540.png): the line is at x = 266–267,
colour (107, 107, 107), `theme::MUTED`; no pixel of it is red.
[without focus 1600 × 1000](ux03-unfocused-1600x1000.png),
[without focus 960 × 540](ux03-unfocused-960x540.png): no line; the marks
stay once the editor has lost focus.

### Scenario 2 (criteria 2, 3): typing a statement

1. At 1600 × 1000 (and in a fresh window at 960 × 540): with the caret at
   the end of line 2, press Enter and type `total = cart.body.`
2. Capture 450 ms after the last key, then 1 500 ms after it.

Expected: at 450 ms plain grey numbers, no dot, no underline, and the
PROBLEMS caption *Unsaved changes · 2 errors* in red with the two rows
(*Line 5, column 3 Expected `=`* with *Apply fix*, *Line 3, column 11
Unknown block handle*); at 1 500 ms the same caption, and lines 3 and 5
red with their dots and the underlines of `cart.body.` and of the line
break before `on_error`.

Observed: as expected at both sizes.
[typing 1600 × 1000](ux03-typing-1600x1000.png) (451 ms),
[marks 1600 × 1000](ux03-marks-1600x1000.png) (1 500 ms),
[typing 960 × 540](ux03-typing-960x540.png) (450 ms),
[marks 960 × 540](ux03-marks-960x540.png) (1 500 ms).

### Scenario 3 (criteria 2, 3, 5): when the marks come

1. Continuing at 1600 × 1000 with the marks shown: type a space and
   Backspace (the text is as before; the pause starts again).
2. Grab the screen every 100 ms after the Backspace, 14 times, the pointer
   parked; count red pixels in the gutter beside lines 3–5, under
   `cart.body.`, and in the PROBLEMS status.

Expected: no marks before 700 ms, marks from the first frame after it; the
status red from the end of the analysis, about 250 ms after the key.

Observed (grabs not committed):

| After the last key | Gutter, red px | Underline, red px | Status, red px |
| --- | --- | --- | --- |
| 100, 200 ms | 0 | 0 | 0 (*Checking…*, grey) |
| 300 – 700 ms | 0 | 0 | 222 (*2 errors*) |
| 800 – 1 400 ms | 61 | 77–78 | 222 |

The marks came between 700 and 800 ms with nothing moving, so the window
drew itself when they were due; the status turned red between 200 and
300 ms, as before this change.

### Against the mock-up

The mock-up was rendered by the headless harness, whose window has an
8 px margin; the release window has none, so every position on this branch
is the mock-up's less 8 px.

| | `ux03-now-*` (mock-up) | This branch, release |
| --- | --- | --- |
| Focus line | 2 px at x = 274–275, accent (224, 89, 39) | 2 px at x = 266–267, grey (107, 107, 107), the owner's decision |
| Marks, once shown | red numbers and dots of lines 3 and 5 in x 249–266, y 140–181; underlines x 280–435, y 153–169 | the same box less 8 px: x 241–258, y 132–173; underlines x 272–426, y 145–161 |
| Marks while typing | shown on every keystroke (today) | none until 700 ms after the last key, the owner's decision |
| PROBLEMS caption and rows | *Unsaved changes · 2 errors* in red, two rows, *Apply fix* | the same, at the same time as before |
| Gutter width, text, explorer, header (`ux03-no-focus-line-*`) | — | unchanged: only the line's colour differs between `ux03-now` and the branch |

At 960 × 540 the release window's code is 8 px wider than the mock-up's, so
line 2 shows one more character (*demo data c*). The caret is drawn in the
release captures; the mock-up's capture of the same state has none. No
other visible difference.

## Deviations

- **Where the wait is worked out.** Criterion 5 says the editor's outcome
  carries the remaining wait; the code view works it out instead, since it
  owns the rule, and the editor stays unaware of the pause (the
  appendix's *nothing else in the editor changes*). The test reads the
  wait through a seam and the window's repaint delay. Recorded in the
  ticket.
- **The keystroke's own frame.** The text edit changes the text while it
  is drawn, so the frame of a keystroke still has the marks of the
  analysis before it; the next frame, asked for at once, has none. With
  the real clock the two are one frame apart, and in the grabs above no
  frame after typing showed a mark before 700 ms. Recorded in the ticket.

## Windows

Not run on Windows by hand. The CI run of this branch on `windows-2022`
runs the `ux03_*` tests (see *Verification*); its `qa-captures` artifact
holds the standard mode captures only, not the UX-03 states. The ticket
is not accepted for Windows on this report.

## Not verified

- Paste, undo and redo restarting the pause, and caret moves, selections
  and scrolling not restarting it, in the release binary; the criterion 6
  test covers them.
- The message on hover: a release capture hovering `cart` after the pause
  showed no tooltip within 2.3 s, which was not investigated further. Its
  absence during the pause follows from the editor getting no diagnostics
  (the same list feeds it) and has no test of its own.
- Split: the code side is the same view; the board following within
  500 ms (smoke item 5) is covered by the existing tests, not captured.
- Double scale, keyboard-only use, a screen reader session.
- Windows, beyond the CI test run.

## Regression smoke list

Linux, 2026-11-02, release binary of `4ca66c5`:

| # | Result |
| --- | --- |
| 2 | Open a folder project: the explorer shows the tree and the scenario opens in Code (both windows above) |
| 5 | Edit source: the PROBLEMS status follows within 300 ms of the last key (scenario 3); the board is covered by the tests |
| 10 | Window at 960 × 540: header, explorer, code and PROBLEMS visible in Code (scenario 2) |
| 1, 3, 4, 6–9 | Not run on the release binary for this ticket; the workspace suite covers their tests |

The owner's Windows run is pending as for the earlier tickets.

## Verification

On `4ca66c5`, all exit 0: `cargo fmt --all -- --check`; `cargo clippy
--workspace --all-targets --locked -- -D warnings`; `cargo test
--workspace --all-targets --locked`: 1 439 passed, 0 failed, 45 ignored;
`cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`.
