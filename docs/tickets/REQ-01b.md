# REQ-01b: Steps — a step is added and moved in the list; an empty scenario offers the first request

- Size: M · Owner review: 20 min · Risk: `visible UI`, `default`
- State: **Decided** ([REQ-00](REQ-00.md): 6 ruled, 7 = A; WS-04 #548 =
  A, folded here by the owner on 2026-11-23). Mock-up approved. After
  REQ-01a, DIAG-22 (#598) and WS-05b (#596).
  Issue [#600](https://github.com/jakubiszon26/apiforge/issues/600); closes #548.
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A test is built in the list without the board: a step is put where the
pointer is and moved one place at a time.

## What the owner will see

- New: `+` between two cards under the pointer; an *Add step* row at the
  end of Main and of Cleanup; a card's menu with *Add step after*, *Move
  earlier*, *Move later* ([boards 6 and 7](REQ-00.md#mock-up)).
- Default that changes (decided, #548): a scenario just created opens in
  Diagram; a project with no scenario shows *Files*. A remembered choice
  and `--mode` still win.

## Acceptance criteria

Tests in `folder/acceptance_tests/req01b.rs`.

1. `+` between two cards and the *Add step* rows open the board's *Add
   step* and insert where they stand, inside the braces of a branch when
   between two cards of it; the text passes the check.
   `req01b_criterion_1_*`
2. *Add step after* in the card's menu inserts right after that step by
   the operation DIAG-22 leaves, never bound to the step's output.
   `req01b_criterion_2_*`
3. *Move earlier*, *Move later*, Alt+↑ and Alt+↓ call
   `board_edit::move_by_one`; a refusal shows the board's reason; the
   moved card stays selected and in view, so the key repeats.
   `req01b_criterion_3_*`
4. There is no drag of a card: a press and a move on a card scrolls or
   selects, nothing else. `req01b_criterion_4_*`
5. An empty scenario shows the *Add step* row alone, in *Steps* and with
   the same words as the empty board. `req01b_criterion_5_*`
6. A scenario just created opens in Diagram and a project with no
   scenario shows *Files*; a remembered choice and `--mode` win
   (LAY-40's tests extended, not replaced). `req01b_criterion_6_*`
7. At 960 × 540 the `+`, the rows and the menu stay inside the window.
   `req01b_criterion_7_*`
8. QA step: REQ-06 part 1 up to the DELETE with its check, Code never
   opened.

## Captures the owner judges on the pull request

The `+` inside a branch; a refused move with its reason; the empty
scenario; a new project's first window.

## Not in scope

*Move to Cleanup* (REQ-14); dragging (REQ-06's proposal 5, not
scheduled).

## Technical appendix

- Files: new `steps/list_edit.rs`; `steps/list.rs` (the slots);
  `folder/steps_view.rs`; `folder/opening.rs` and `folder/explorer.rs`
  (criterion 6); the test file. No change to `canvas/`.
- Reuse: DIAG-06a's insert and DIAG-06b's `move_by_one` and refusals
  (`board_edit.rs`, `canvas/menu.rs` for the words).
- Regression: the opening default — LAY-40's and WS-05b's tests; the
  board's moves — DIAG-06b's tests unchanged.
