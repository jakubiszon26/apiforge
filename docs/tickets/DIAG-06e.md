# DIAG-06e: Deleting a step that is read, and pasting or duplicating steps, keep the chain and the check

- Size: M · Owner review: 15 min · Risk: `default`, `visible UI`
- State: **Decided; waits for the independent review before it is
  labelled.** The owner answered the three questions on
  [#494](https://github.com/jakubiszon26/apiforge/issues/494) on
  2026-11-20: 1 = A (a delete of a step that is read is refused, naming
  the readers), 2 = A (*Duplicate* directly after its original; *Paste*
  after the selected step, else at the end), 3 = A (a reference that
  still finds its step is kept; otherwise the paste is refused). Split
  from DIAG-06a on 2026-11-20. After DIAG-06a (#391). A condition of
  0.2.0 (REL-05, condition 5).
- Branch `feat/diag-06e-delete-and-paste`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

The two edits that can still write a file which does not pass the check
stop doing so, and a copy lands where the person expects it.

## What happens today

Read from `main`: deleting a block removes every input that refers to it
(`apiforge-core/src/graph.rs`, `delete_nodes`), so deleting a request
whose value another request's URL reads leaves that request without its
URL, which the check refuses (`validation.rs`). A paste removes the
references it cannot resolve (`edit/clipboard.rs`), with the same result.
*Start* can be deleted while the flow has steps, which language 2
refuses. Pasted and duplicated steps are written last (LANG-13).

## Decisions

The owner's, 2026-11-20 (#494): the three answers above.

Ruled by the planning model, following from rules already decided:

- A step's own checks and values — those that read only it — are deleted
  with it (question 16's group).
- *Start* cannot be deleted while its flow has a step: *A scenario starts
  at Start. Delete its steps first.* (Rule 1 of language 2.)
- Several selected blocks are judged together: a reader that is deleted
  in the same gesture is no reader.

## Acceptance criteria

Tests named `diag06e`, in the files of DIAG-06a;
every test of an edit ends by asserting the check.

1. Deleting a step that another step, outside the selection, reads is
   refused and names the readers; with the readers selected too, all are
   deleted and the neighbours join. Tests: K, D `diag06e_criterion_1_*`.
2. A step's own checks and values go with it; a check that also reads
   another step stays and — if it then reads nothing that exists — the
   delete is refused as in 1. Tests: K, D `diag06e_criterion_2_*`.
3. *Start* with steps after it is not deleted; alone, it is.
   Test: D `diag06e_criterion_3`.
4. *Duplicate* writes the copy directly after its original, with its own
   checks; several duplicated steps keep their order after the last of
   them. Test: D `diag06e_criterion_4_*`; the UX-34 tests restated where
   they assert the place, listed in the pull request.
5. *Paste* writes the steps after the selected step (DIAG-06a's rule for
   *Add step*), else last in the main flow, in their copied order.
   Test: D `diag06e_criterion_5_*`.
6. A pasted reference whose step stands above the place of the paste is
   kept; a paste that would leave a step reading what is not there is
   refused and names it; nothing is pasted in part.
   Tests: K, D `diag06e_criterion_6_*`, also from another scenario.
7. The property test of DIAG-06a's criterion 10 also deletes, duplicates
   and pastes. Test: P `diag06e_criterion_7`. REL-05's condition 5 names
   this test.
8. Each edit is one step of Undo and Redo. Test: P.

## Regression potential

Copy and paste (UX-34), *Save as component* (which copies a selection),
the legacy window (its functions keep their behaviour, DIAG-06a's
criterion 9), deleting a component call or a variable that is read (the
same rule as a step).

## Not in scope

Moving (DIAG-06a, DIAG-06d). Deleting an `if` with its braces asks first
(DIAG-06d); this ticket's rule 1 then applies to what is inside it.

## Technical appendix

`apiforge-core/src/edit/place.rs`, `clipboard.rs`; `graph.rs`
(`delete_nodes` unchanged for the legacy caller; the folder path asks
first); `apiforge-desktop/src/board_edit.rs`.
