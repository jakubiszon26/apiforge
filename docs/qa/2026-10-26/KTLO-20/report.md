# KTLO-20: A sleeping dog keeps the empty board company — QA report

- Branch: `feat/ktlo-20-sleeping-dog`, from `main` at `22e94d4`; binary and
  captures built at the branch head.
- Built with: `cargo build -p apiforge-desktop --release --locked`
- Compared against the mock-up the owner approved in
  [#46](https://github.com/jakubiszon26/apiforge/issues/46), copied into this
  directory as `mock-*`; criteria from [the ticket](../../../tickets/KTLO-20.md),
  which reached `main` with #45 while this branch was being verified and was
  merged into it.
- Owner decisions carried out: animated on a four-second loop, only while an
  empty board is visible, off in the test harness; only the "no file open"
  hint; the mock-up approved as a whole (2026-10-26); the drawing's nodes
  named but hidden in the accessibility tree, and one pull request over the
  M cap (2026-10-27).
- Verification: the development guide's commands; output in the pull request.

## Captures

`after-*.png`: the release binary of this branch, `--screenshot --folder` of
the migrated demo shop under Xvfb, with the project's last tab closed. The
mock-ups are harness captures of the same views, with an 8 px margin the
native window lacks; their project is the prepared demo ("Demo shop") rather
than the migrated one ("Sklep demonstracyjny").

| View | Approved mock-up | After (release) |
| --- | --- | --- |
| Diagram 960 × 540 | [mock](mock-diagram-960x540.png) | [after](after-diagram-960x540.png) |
| Diagram 1600 × 1000 | [mock](mock-diagram-1600x1000.png) | [after](after-diagram-1600x1000.png) |
| Split 960 × 540 | [mock](mock-split-960x540.png) | [after](after-split-960x540.png) |
| Split 1600 × 1000 | [mock](mock-split-1600x1000.png) | [after](after-split-1600x1000.png) |

## Linux

### Scenario 1: the empty board in Diagram and Split
1. Open the migrated demo shop (`--folder …`) with no tab open.
2. Capture Diagram, then Split, at 960 × 540 and at 1600 × 1000.

Expected: the grey loaf asleep — sleep bubble, three paws, "z z Z" in the
application's bold font, the ground shadow, no soft drop shadow — above "Open
a scenario or component to see its diagram.", the pair centred in the board.
Observed: as in the mock-ups at both sizes and in both modes (`after-*.png`);
the drawing, its place and the hint under it are the approved frame. Tests:
`the_hint_sits_under_the_drawing_and_the_pair_is_centred_in_the_board`,
`the_drawing_is_220_points_wide_at_960_by_540_and_280_at_1600_by_1000`.

### Scenario 2: closing the last tab, and opening a file again
1. In Diagram, close the only tab; then open the scenario from the document
   list in Code and return to Diagram. Repeat in Split.

Expected: closing shows the drawing; opening a file hides it.
Observed: as expected in both modes. Test:
`closing_the_last_tab_shows_the_drawing_and_opening_a_file_hides_it`.

### Scenario 3: the board's other hints and the code side of Split
Expected: the code side of Split keeps "Open a scenario or component from the
document list."; a text file's hint stays alone, justified across its board.
Observed: as expected; `after-split-*.png` show the code hint unchanged to the
left of the board. Tests: `the_code_side_of_split_keeps_its_own_hint`,
`the_text_file_hint_keeps_the_board_to_itself`. A file whose code has errors
and one missing from the folder keep their own hints too
(`a_file_with_errors_or_a_missing_one_keeps_its_own_hint`); they and
"Checking the code…" share the one path this ticket did not touch,
`centred_hint`, and were not captured.

### Scenario 4: the four-second loop
Expected: the drawing breathes, its shadow and bubble swell, the ears and tail
sway and the letters float up and fade, over four seconds, only while an empty
board is on screen; the test harness holds the rest frame.
Observed: the release captures are taken eight frames in, a thirtieth of the
loop, and are the rest frame to the eye — which is what the mock-up comparison
above relies on. The loop in motion was not watched frame by frame; what is
proved automatically is its period and rest frame
(`the_loop_is_four_seconds_long_and_rests_at_the_approved_frame`,
`the_letters_only_float_up`), that it asks for the next frame only from an
empty board (`the_loop_runs_only_while_an_empty_board_is_visible`) and that it
is off in the harness (`an_empty_board_asks_for_no_redraw_in_the_test_harness`).

### Scenario 5: accessibility and design review (criteria 5, 7)
Observed: `the_empty_board_names_every_control` audits Diagram and Split with
no file open at both sizes and finds no unnamed control. Criterion 5 asks that
the drawing add nothing a screen reader announces. It is painted decoration,
so it is marked hidden in the accessibility tree — what `aria-hidden` is to a
decorative picture — and only the hint is announced, as today. It does carry a
name behind that flag ("Sleeping dog"), as does the empty board area ("Diagram
board", the name the canvas already uses when a document fills it), because
`docs/gui-testing.md` asks that a widget a test must measure be nameable; the
assertions on the drawing's box and place rest on those two names. So the
drawing's nodes exist in the tree, named, but marked hidden; a screen reader
announces only the hint. **Criterion 5 is met as to its purpose, by the owner's
decision of 2026-10-27** (decision 4 of the ticket, issue #46): the hidden
named nodes stay.

Design review of `review_empty_board` at 960 × 540, 1600 × 1000 and 960 × 540
at double scale: nothing overlaps, nothing is cut off and the pair is centred.

Criterion 7, double scale: [Diagram](review-diagram-960x540-2x.png) and
[Split](review-split-960x540-2x.png) at 960 × 540 with double scale, each
1920 × 1080 pixels, rendered by `review_empty_board` (`tools/gui/review.sh
review_empty_board`) at commit `cdd8146`. Observed: the outline, the ears, the
eyes and the "z z Z" are drawn at the doubled resolution with smooth curves and
no blur or stair-stepping; the drawing has the shape, proportions and place
above the hint of the single-scale captures, at twice the pixels. These are
headless harness captures, which the ticket names for this criterion; the
release binary was not captured at double scale (see "Not verified").

## Windows

Not run on Windows by hand. CI runs the desktop suite, including the eleven
KTLO-20 tests, on the Windows runner and uploads `qa-captures-windows-2022`;
those captures show a board with a file open, not an empty one. Not accepted
for Windows on this report alone.

## Not verified

- The loop on screen, frame by frame, and what it costs a laptop battery: a
  window showing an empty board now redraws about sixty times a second, as
  decision 1 states. No measurement was taken.
- The system's reduced-motion setting, which egui does not expose and the
  owner's decision does not ask for.
- The mock-up rendered the drawing 275 points wide at 1600 × 1000, because its
  draft sizing scaled with the board; the implementation keeps the ticket's
  280. That 5-point difference is the only deliberate departure from the
  approved capture.
- A board too small for the drawing (a very short window, or one whose run
  report leaves no room) falls back to the hint alone. No capture, and no
  test: the minimum window is 960 × 540 and fits the pair.
- The release binary at double scale: criterion 7 rests on the harness
  captures above. Xvfb draws at a scale of one, and no native double-scale
  capture was taken.
- Windows, other than the CI suite.

## Regression smoke list

Linux, 2026-10-26, release binary of this branch under Xvfb.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with a legacy workspace migrated it and opened a project; stderr empty. |
| 2 | Pass: `--folder` opens the migrated demo shop in Diagram and Split (`after-*`). |
| 3–9 | Not run by hand: this change adds a drawing to the empty board and touches nothing else; the workspace suite that covers them passes, and `apiforge-convert-v1 --fixtures --check` passes. |
| 10 | Pass: at 960 × 540 Diagram and Split show every control, with the drawing and its hint inside the window (`after-*-960x540.png`). |
