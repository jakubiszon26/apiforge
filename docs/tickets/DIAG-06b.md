# DIAG-06b: A refusal is shown while carrying; a notice after a delete; `+` on a line; Move earlier and Move later

- Size: M · Owner review: 15 min · Risk: `visible UI`, `default` (two
  keys)
- State: **Approved.** Owner's answers of 2026-11-20 on #391: 8 = A,
  11 = A, 12 = A; the keys, on #491: **A — Alt+↑ and Alt+↓**. Built next
  by the owner's decision of 2026-11-23 (#582). Not a condition of 0.2.0.
- Issue [#491](https://github.com/jakubiszon26/apiforge/issues/491) ·
  LANG sprint · branch `feat/diag-06b-feedback-and-moves`
- Analysis model: Fable 5.2 · Implementation model: Opus 5.5

## Goal

The board says why before a gesture is refused and what joined after a
delete, and a step can be inserted and reordered without dragging, also
from the keyboard.

## What the owner will see

- New (question 8, views 8 and 8b): while a wire is carried over a socket
  that will refuse it, the socket is crossed, the line is red and the
  reason stands at the pointer; on release nothing changes and the reason
  stays in the board's banner, as today.
- New (question 11, view 4b): after a step in the middle is deleted, a
  notice names what now follows what, with *Undo*, until the next action.
- New (question 12, view 5b): a `+` on the line of a chain under the
  pointer opens *Add step* and inserts there.
- New (question 12): *Move earlier* and *Move later* in a step's menu,
  with Alt+↑ and Alt+↓.
- Removed or hidden, migrated: nothing.

## Mock-up

Approved on #391: views 4b, 5b, 8 and 8b. The menu entries use the
existing menu. Release captures in the pull request.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/diag06b.rs`.

1. For each refusal of DIAG-06a (a value read, a check from *Next*, an
   `if` into itself): while carried over the target the socket is drawn
   crossed, the line red, and the reason is at the pointer and in the
   accessibility tree; released, the document is unchanged and the banner
   holds the same sentence. Test: `diag06b_criterion_1_*`.
2. After a middle step is deleted a notice reads *`c` now follows `a`*
   with *Undo*; *Undo* restores the step; the notice goes with the next
   action; deleting the last step or a check shows none.
   Test: `diag06b_criterion_2_*`.
3. Pointing at a line of a chain shows `+`; activating it opens *Add
   step*, and the chosen step is inserted on that line, in its braces.
   At 960 × 540 and 1600 × 1000 the `+` is inside the window and at least
   16 px. Test: `diag06b_criterion_3_*`.
4. *Move earlier* and *Move later* move the selected step by one step in
   its braces, with its checks (DIAG-06a's operation); at the first and
   the last place of the braces, and where a value read forbids it, the
   command is disabled and its hover text says why. Alt+↑ and Alt+↓ do
   the same while the board has the focus and a step is selected; in the
   code editor the keys keep what they do today. Test: `diag06b_criterion_4_*`.
5. Every edit here leaves a text that passes the check and is one step
   of Undo. Asserted in the tests of 2–4.

## Regression potential

The banner of today and its tests (`notices`), the *Add step* menu
(`popup_tests`), keys already bound in the board and the editor (checked
against the list in `docs/development.md` or the code's key table;
a clash is reported, not resolved).

## Not in scope

Moving across braces from the keyboard (a move by one step stays in its
braces; across braces is a wire, DIAG-06a). Bands.

## Technical appendix

`crates/apiforge-desktop/src/canvas/` (`mod.rs`, `paint.rs`, `menu.rs`),
`folder/notices.rs`, the key handling of the board.
