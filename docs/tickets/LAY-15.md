# LAY-15: Off-screen relations — edge chips by role, the jump, and Back

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: approved with its mock-ups on 2026-11-05 (Q9 = A); amended by the
  owner's decision of 2026-11-18 (Q4 = A, #296): the whole ticket before
  the close of LAY, plus *Back* restores the Inspector's scroll and open
  section, and a kept trace gets the chip too. Accepted on captures before
  merge (#296, Plan A) · issue #299 · LAY sprint, part of its close
- Analysis model: Fable · Implementation model: Opus

## Goal

On a board larger than the window, the source of the value a person is
following is out of view. Today the line leaves the edge, the statement
names the source and nothing leads there: one pans up, loses the place and
pans back. A chip on the board's edge says what is out there and in which
direction, a click goes there at the same zoom, and *Back* returns to
exactly where one was — the board and the Inspector.

## What the board does today

Looked at on `main` 2f6922c, `booking-day` of the room project
(`fixtures/scenarios/lay/rooms/`, 29 steps) at 76 %, 1280 × 800:

- A click on `second`'s room chip keeps the trace: the statement
  `rooms.body.items[1].id → second.body.roomId` stands at the top left of
  the board, the line runs up and out of view. `rooms` is two rows up.
- A click on the statement clears the trace. Panning to `rooms` by hand
  keeps the zoom, the selection, the trace and the Inspector, but `rooms`
  then sits half under the Inspector, and the only way back is to pan
  again.
- Selecting `second` rings its three sources (`token`, `rooms`, `slot`),
  none of them in view.

## What the owner will see

"Edge chip" below is the new chip on the board's edge; the chips beside a
card's inputs (`← rooms…[1]`, LAY-23) are not changed.

- New: **an edge chip for each related card out of view**, labelled by
  role with an arrow for the direction: `Source: rooms.body.items[1].id ↑`,
  `Reader: calendar.query.from →`; a step read through several fields is
  named once, `Source: slot · 2 fields ↑`. It is shown
  - while a step is selected (LAY-10), for its sources and its readers;
  - **while a trace is kept** (a click on a line or on an input's chip,
    LAY-14), for the end of that line that is out of view — decided
    2026-11-18.

  A card counts as out of view when its header row is not wholly in the
  board's free area (not under the Inspector, the minimap or the bars).
  More than two on one edge are one chip, `↑ 3 sources`; a click on it
  unfolds it into its members, stacked along the edge, until the selection
  or the trace changes. Checks and order relations get no edge chip.
- New: **a click on an edge chip goes to that card** at the same zoom, the
  card whole inside the free area. The selection, the kept trace and its
  statement, and the Inspector stay as they are.
- New: **`← Back to second`** at the top left of the board after a jump.
  It restores the camera and the zoom exactly, the selection, the kept
  trace, and the Inspector — the same step, **the same scroll position and
  the same sections open** (decided 2026-11-18) — also when the person
  selected another card or opened another step's Inspector at the
  destination. It returns to where the **first** jump started, however
  many jumps followed, and stays until it is used or the step it returns
  to is removed.
- An edge chip, *Back* and a kept statement never cover the selected card
  or each other: where the statement would stand under *Back*, it stands
  below it.
- Changed, removed or hidden, migrated, default behaviour: nothing. Nothing
  is saved: a jump and *Back* do not mark the document unsaved.

## Mock-up

Approved as direction on 2026-11-05 (#37): round 3
[`lay00r3-focus-offscreen-60`](https://raw.githubusercontent.com/jakubiszon26/apiforge/ea981d87be67c3fb3fc3ec2e95c09593e588a211/docs/qa/2026-11-04/LAY-00/lay00r3-focus-offscreen-60.png),
[`lay00r3-focus-offscreen-back`](https://raw.githubusercontent.com/jakubiszon26/apiforge/ea981d87be67c3fb3fc3ec2e95c09593e588a211/docs/qa/2026-11-04/LAY-00/lay00r3-focus-offscreen-back.png);
round 4, with the labels by role,
[`lay00r4-walk-small-2`](https://raw.githubusercontent.com/jakubiszon26/apiforge/df6305b5a7edae98edecd756eade545c0921f228/docs/qa/2026-11-04/LAY-00/lay00r4-walk-small-2.png)
and `lay00r4-walk-1..5`.

Checked against `main` on 2026-11-18; it still fits, with three things the
board has gained since, none of which changes what was approved:

- The cards are compact and the inputs carry their own chips (LAY-16,
  LAY-23). The edge chip keeps the mock-up's pill and its teal outline, so
  it does not read as an input's chip (grey, beside a socket).
- A kept trace and its statement did not exist (LAY-14). The statement can
  stand where the mock-up puts *Back*; the rule above orders them.
- The round-3 labels (`↑ login · token`) are superseded by round 4's
  (`Source: login.body.token ↑`), as the ticket already said.

The pull request carries three captures on `booking-day` at 76 %,
1280 × 800: the kept trace with its edge chip, the destination with
*Back*, and the return with the Inspector open at *Body value source*.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay15.rs` on
a temporary copy of `fixtures/scenarios/lay/rooms/`, `booking-day`, tidied,
chips on, checks folded, zoom 76 %, 1280 × 800, the camera placed so that
`second` is in view and `rooms` is not; clicks go to accessible buttons
(`Source: rooms.body.items[1].id (jump)`, `Back to second`).

1. **Kept trace.** A click on `second`'s room chip shows exactly one edge
   chip, on the top edge, reading `Source: rooms.body.items[1].id ↑`; Esc
   removes it with the trace. A kept trace whose two ends are in view
   shows none. Test: `lay15_criterion_1_a_kept_trace_has_its_edge_chip`.
2. **Selection.** `second` selected shows `↑ 3 sources`; a click unfolds
   it into three chips, `Source: token ↑`, `Source: rooms.body.items[1].id
   ↑` and `Source: slot · 2 fields ↑` (one read: the reference as the code
   writes it; several reads of one step: the step and how many); two
   sources on one edge are two
   chips from the start; a card whose header is partly out of the free
   area has a chip, a card wholly in it has none. Test:
   `lay15_criterion_2_a_selected_step_has_its_edge_chips`.
3. **The jump.** A click on the edge chip of criterion 1 puts `rooms`
   whole inside the free area — with the Inspector open too — at zoom
   76 % exactly; the selection, the kept trace, its statement and the
   Inspector's step are unchanged; the document is not marked unsaved.
   The camera moves by the chip's own path, not `Canvas::reveal()`. Test:
   `lay15_criterion_3_the_jump_keeps_the_zoom`.
4. **Back, the board.** After the jump `Back to second` is shown; a click
   makes the camera struct equal to the one before the jump, with the
   selection and the kept trace as they were, and *Back* is gone. Test:
   `lay15_criterion_4_back_restores_the_board`.
5. **Back, the Inspector.** `second` selected, its Inspector scrolled to
   *Body value source* and that section opened; jump to `rooms`; select
   `rooms` there (the Inspector shows `rooms`); *Back* → `second` is
   selected, the Inspector shows it at the same scroll offset with *Body
   value source* open and the sections that were closed still closed.
   Test: `lay15_criterion_5_back_restores_the_inspector`.
6. **One origin.** Jump from `second` to `rooms`, select `rooms`, jump
   from its edge chip to a reader: *Back* still reads `Back to second`
   and returns there. Removing `second` removes *Back*. Test:
   `lay15_criterion_6_back_returns_to_the_first_origin`.
7. **Nothing is covered.** At 1280 × 800 and at 960 × 540 with the
   Inspector open, no edge chip, *Back* or kept statement overlaps the
   selected card, each other, the minimap or the zoom bar; every edge
   chip is inside the board. Test:
   `lay15_criterion_7_chips_cover_nothing`.
8. **The owner's walk, by clicks.** Room chip of `second` → edge chip →
   `rooms` in view with the traced line's other end ringed → *Back* →
   the view of the start; four clicks, no pan, no zoom. Test:
   `lay15_criterion_8_the_walk_by_clicks`.
9. Opening the Inspector does not pan the board away from the selection
   with edge chips present (UX-18). Test:
   `lay15_criterion_9_the_inspector_keeps_the_place`.

## Regression potential

- The camera: LAY-18's `lay18_criterion_4` (the camera does not move on
  hover; flaky on Linux CI, #432) and UX-18's tests guard it; the jump is
  the only new writer of the camera and only on a click.
- The trace's lifetime (LAY-14: a kept wire is a selected wire) — `lay14`
  runs unchanged; *Back* restores a trace by selecting its wires again.
- The Inspector's scroll: restoring an offset one frame after the
  selection changes must not fight UX-13/LAY-16's "scroll to the input"
  on a double-click; `lay16` and the UX-13 test run unchanged.

## Not in scope

- Edge chips for checks or order relations; the minimap; the focus itself
  (LAY-10); where the lines of a selected step run (W4–W6 of the trial,
  LAY-2).
- *Change source* on the kept statement (the source editor's ticket).
- A history of several places to go back to.

## Technical appendix

- The camera is zoom and pan, a `Copy` struct, so *Back* stores it with
  the selection, the kept wires (`trace::Memory`) and the Inspector's
  state: the scroll offset of the area in `inspector/mod.rs` and the open
  state of its collapsing sections (`body-binding`, `samples`,
  `manual-path`, by their ids).
- Edge chips are laid in `canvas/mod.rs` from the selected step's
  relations (`Focus`, `apiforge_core::graph::relations`) or the ends of
  the kept wires (`canvas/trace.rs`), against the board's free area; the
  harness clicks them as accessible buttons.
- The label of a source is the reference as the code writes it (the left
  side of the chip statement, `readers::texts`); of a reader, its step and
  input.
- The fixture `fixtures/scenarios/lay/rooms/` is added by this ticket's
  first commit (the same files ride on LAY-39's branch; identical content
  merges cleanly).
