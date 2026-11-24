# REQ-14: A step is moved between Main and Cleanup, with its checks

- Size: M · Owner review: 15 min · Risk: `visible UI`
- State: **Decided** ([REQ-00](REQ-00.md): 8 = A, the action is in the
  step's menu; the refusal ruled). After REQ-01b and DIAG-22 (#598).
  Issue [#549](https://github.com/jakubiszon26/apiforge/issues/549).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

"Make the delete happen even when a check fails" is one action on the
step (REQ-06, criterion 4).

## What the owner will see

- New: *Move to Cleanup* and *Move to Main* in a step's menu, in *Steps*
  and on *Canvas* ([board 8](REQ-00.md#mock-up)). Nothing removed.

## Acceptance criteria

Tests in `folder/acceptance_tests/req14.rs`; the operation's unit tests
in `board_edit.rs`.

1. *Move to Cleanup* on a request or a component call moves its
   statement into `cleanup { }`, last, with the checks that read it;
   what it reads from Main still resolves; the text passes the check.
   `req14_criterion_1_*`
2. It is refused, naming the first reader ("*read* reads
   `create.body.id`"), when a step or a check that stays in Main reads
   the step. `req14_criterion_2_*`
3. *Move to Main* moves it back, last in Main, with its checks.
   `req14_criterion_3_*`
4. The item is not offered for an `if`, a value or a check alone.
   `req14_criterion_4_*`
5. A step moved out of a branch runs in Cleanup without the condition;
   the menu's hover says so. `req14_criterion_5_*`
6. The moved step is selected and in view in *Steps*; on *Canvas* the
   tab follows it. `req14_criterion_6_*`
7. One undo restores the text. `req14_criterion_7_*`
8. QA step: REQ-06 part 1's last sentence; after two runs the demo API
   holds no cart.

## Captures the owner judges on the pull request

The two menus; the refusal; the hover of criterion 5.

## Not in scope

Dragging onto the tab or the section; a paste that keeps its checks
(REQ-08 #465 owns copies).

## Technical appendix

- Files: `board_edit.rs` (the operation), `canvas/menu.rs` (the items;
  after DIAG-22, which edits the same file), `steps/list_edit.rs`, the
  test file. `canvas/mod.rs`: the dispatch of two actions, nothing else.
- Regression: handles stay unique across both graphs (DIAG-17's test);
  a Cleanup step whose input is missing is skipped with its reason
  (`NotRunCause::CleanupInputMissing`, existing tests).
