# DIAG-06d: A step moves with an `if`'s braces and across braces; deleting an `if` asks once

- Size: M · Owner review: 20 min · Risk: `default` (a move can change the
  condition a step runs under), `visible UI` (one confirmation, one menu
  entry)
- State: **Approved, ready for implementation after DIAG-06a (#391) has
  merged.** Split from DIAG-06a on 2026-11-20. Owner's answers of
  2026-11-20 on #391: 7 = A, 9 = A, 13 = A, 14 = A.
- Issue [#493](https://github.com/jakubiszon26/apiforge/issues/493) · LANG
  sprint, first half, a condition of 0.2.0 · branch `feat/diag-06d-braces`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

The braces of an `if` are no wall for the board: a condition moves with
what is inside it, a step goes into a branch or out of one, a check is
put into a branch by a wire, and a condition with steps in it is not
deleted by one key without a word.

## What the owner will see

- Changed (question 13): an `if` dropped on by an ordering wire moves
  with everything in its braces.
- Changed (question 14): a step dropped behind a step of other braces
  goes into those braces — into a branch, out of one, from *Yes* to *No*.
  It then runs under another condition; the caption of DIAG-06c says so
  while it is carried.
- Changed (question 9): a check, value, variable or export takes an
  ordering wire from *Yes* or *No* only, which puts it into that branch;
  a wire from a step's *Next* is refused: *Only steps are ordered. A
  check runs when what it reads exists.* New: *Move out of the branch* in
  such a card's menu.
- Changed (question 7): deleting an `if` that has statements in its
  braces asks once — *Delete `paid` and the 3 steps inside it?* — and
  then deletes all of it; Undo brings it back.
- Removed: DIAG-06a's sentence "…is not possible yet".
- Migrated: nothing.

## Mock-up

Approved on #391 (views 3, 6, 7 for what a move leaves). The confirmation
uses the existing confirmation dialog; the pull request shows it and one
board before and after a move across braces, at both sizes.

## Acceptance criteria

Tests as DIAG-06a's files, named `diag06d`; every test of an edit ends by
asserting that the text passes the check and reads back as the document.
All moves go through DIAG-06a's placement contract.

1. An `if` moves with every statement in its braces, at any depth, and
   the checks that travel with it (DIAG-06a, criterion 3); the text
   differs only by those lines standing in the new place.
   Tests: K `place_moves_an_if_with_its_braces`, S, D `diag06d_criterion_1`.
2. A step moved behind a step of other braces stands in those braces
   afterwards: into a branch, out to the main flow, from *Yes* to *No*,
   into a nested branch; an `if` cannot be moved into its own braces:
   refused. The cleanup and the main flow are not joined by a move:
   refused. Tests: K `place_across_braces_*`, D `diag06d_criterion_2_*`.
3. The refusals of DIAG-06a's criterion 4 hold across braces: a reader
   moved above its producer's `if`; a producer moved into a branch when a
   reader stands above that `if`; the same through a value between them.
   Tests: K `place_refuses_across_braces_*`.
4. What a move across braces does to a run is the text's meaning, on both
   outcomes of the condition: a producer moved into *Yes* while its
   reader stays after the `if` — the reader runs when *Yes* is taken and
   is blocked, naming the producer, when *No* is (RUN-01); a reader
   outside a moved `if` likewise. The move is allowed; the check passes.
   Tests: `crates/apiforge-runner/tests/diag06d.rs`, both branches.
5. A check, value, variable or export: a wire from *Yes* or *No* puts it
   into that branch; a wire from a step's *Next* is refused with the
   sentence above; *Move out of the branch* writes it after the `if`'s
   braces; each is refused when a statement would then read what is made
   below it. Tests: K, D `diag06d_criterion_5_*`.
6. Deleting an `if` with statements in its braces asks once, naming the
   `if` and the count of steps; *Delete* removes all of it and joins its
   neighbours, *Cancel* changes nothing; one Undo restores it. An `if`
   with empty braces is deleted without a question. A step outside that
   reads something inside is DIAG-06e's question 1: until it merges, as
   after LANG-13. Tests: D `diag06d_criterion_6_*`.
7. The property test of DIAG-06a's criterion 10 also moves `if`s and
   moves across braces. Test: P `diag06d_criterion_7`.
8. Each edit is one step of Undo and Redo. Test: P.

## Regression potential

| What can break | Covered by |
| --- | --- |
| A step silently runs under another condition | criterion 4; `shop-30`'s run order asserted before and after a move |
| LANG-14 and #433: what stands in a branch's braces in the text | their suites unchanged |
| Today's wire from *Yes* to a check | criterion 5 |
| The legacy window | DIAG-06a's criterion 9 suites unchanged |

## Not in scope

The caption and the `if`'s own *Next* (DIAG-06c). Deleting a step that is
read; paste (DIAG-06e). Bands (LAY-13).

## Technical appendix

`apiforge-core/src/edit/place.rs` (the braces as a place; a subtree as
what moves), `apiforge-script/src/edit.rs` (a statement with its braces
cut and written; indentation of the moved lines follows the braces they
enter), `apiforge-desktop` (`board_edit.rs`, `canvas/menu.rs`, the
existing confirmation dialog).
