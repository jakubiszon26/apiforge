# DIAG-06e: A delete keeps the check: it is refused when something read would go, and an `if` asks once

- Size: M · Owner review: 20 min · Risk: `default` (a delete that worked
  is refused), `visible UI` (one confirmation, refusal sentences)
- State: **Approved; labelled with the ruling of the independent
  review's second pass.** After LANG-13 (#386) and DIAG-06g (#502, Undo
  restores the file), which that pass split out of this ticket. Owner's answers of 2026-11-20: question 1 on
  [#494](https://github.com/jakubiszon26/apiforge/issues/494) = A (a
  delete of a step that is read is refused, naming the readers) and
  question 7 on #391 = A (deleting an `if` with steps asks once).
  Restated the same day after an independent review: paste and duplicate
  are DIAG-06f (#496); the deletion of an `if` came here from DIAG-06d,
  so that no ticket deletes without the rule below; Undo restores the
  source. It needs none of the moves and can be built beside LANG-06 and
  LAY-45, **before** DIAG-06a.
- A condition of 0.2.0 (REL-05, condition 5) · branch
  `feat/diag-06e-delete-and-paste`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

Deleting on the board never leaves a step without something it reads,
never takes a condition's steps away without asking, and can always be
taken back exactly.

## What happens today

Read from `main`: deleting a block removes every input that refers to it
(`apiforge-core/src/graph.rs`, `delete_nodes`). A request whose URL read
it is left without a URL, which the check refuses (`validation.rs`); a
request whose optional body read it stays valid and silently sends
something else. An `if` is deleted alone and its steps stay behind.
*Start* can be deleted while the flow has steps. Undo keeps the previous
document, not the previous text (`apiforge-project/src/session.rs`,
history): the text is written again from the model, so a free comment or
a blank line that stood in what was deleted does not come back.

## What the owner will see

- Changed (question 1 = A): deleting a step, value or variable that a
  statement outside the selection reads is refused: *`pay` and `receipt`
  read `cart`. Change or delete them first.* Selected together, all are
  deleted.
- Changed: what is deleted with a step is said before, not found after:
  its own checks and values — those that read only it, and what reads
  only those — go with it, as they move with it (question 16).
- Changed (question 7 = A): deleting an `if` that has statements in its
  braces asks once — *Delete `paid` and the 3 steps inside it?* — and
  then deletes all of it. An `if` with empty braces is deleted without a
  question.
- New: *Start* is not deleted while its flow has a step: *A scenario
  starts at Start. Delete its steps first.* (Rule 1 of language 2.)
- Removed or hidden, migrated: nothing.

## Mock-up

Not applicable beyond the confirmation, which uses the existing
confirmation dialog; the pull request shows it at both sizes. View 4b of
#391 is what a delete in the middle leaves.

## The deletion, in one place

One function answers "what would this delete do", and the confirmation,
the refusal and the delete itself all use its answer:

1. **What goes**: the selected blocks; for an `if`, every statement in
   its braces at any depth; and a check, value or variable that **reads
   at least one statement that goes and no statement that stays**
   (applied until nothing more is added). A statement that reads no
   statement at all — a literal value, a check of constants, a value of
   a parameter or of `env` only — belongs to nothing and stays.
2. **Who is left reading**: every statement that stays — in the main
   flow **and in the cleanup**, which sees the main flow — with a
   reference, anywhere in any of its inputs, required or optional, to
   something that goes.
3. If 2 is not empty the delete is refused and names them. Otherwise it
   is done, and **no input of a statement that stays is changed**.
4. The legacy window keeps `delete_nodes` as it is (DIAG-06a, contract
   1); the folder window calls this.

## Acceptance criteria

Tests: `crates/apiforge-core/src/edit/` unit tests (K),
`crates/apiforge-project/tests/diag06e.rs` (P),
`crates/apiforge-desktop/src/folder/acceptance_tests/diag06e.rs` (D), on
`labels`, `shop-30` and `rooms/booking-day`. Every test of an edit ends
by asserting that the text passes the check and reads back as the
document.

1. Deleting a step that a statement outside the selection reads is
   refused, names every reader and changes nothing; with the readers
   selected too, all are deleted and the neighbours join. Also when the
   reader is in the cleanup, and when the reader's input is optional (a
   body): refused, the body is not dropped.
   Tests: K `delete_preview_*`, D `diag06e_criterion_1_*`.
2. A statement that stays and reads **anything** that goes refuses the
   delete, also when it reads something else that stays: with `both =
   assert a.status == b.status`, deleting `a` is refused and names
   `both`. Test: K, D `diag06e_criterion_2`.
3. What goes is closed over a step's own statements: `a`, a value `v`
   that reads only `a`, a request `b` whose body reads `v` — deleting `a`
   is refused and names `b` (through `v`); with `b` selected too, `a`,
   `v` and `b` go. And it takes nothing that reads no statement: with
   `timeout = 30`, a check `ok = assert 1 == 1` and `limit =
   param.limit` in the document, deleting `a` leaves all three, also when
   another request reads `timeout`. Tests: K `delete_preview_expands`,
   `delete_preview_leaves_what_reads_no_statement`; D
   `diag06e_criterion_3_*`.
4. After a delete that is done, every input of every statement that
   stays is equal to what it was. Test: K, P
   `diag06e_criterion_4_surviving_inputs_are_untouched` (compared
   binding by binding, not by the check passing).
5. Deleting an `if` with statements in its braces asks once, naming the
   `if` and the count of steps that go; *Cancel* changes nothing;
   *Delete* removes all of it and joins its neighbours. A statement
   outside that reads something inside refuses the delete before any
   question, by criterion 1. An `if` with empty braces is deleted without
   a question. Tests: D `diag06e_criterion_5_*`.
6. *Start* with steps after it is not deleted; alone, it is.
   Test: D `diag06e_criterion_6`.
7. One Undo after each delete of this ticket — a step with its checks;
   an `if` whose braces hold a free comment, blank lines and attributes
   in the author's order — gives the file back byte for byte, and Redo
   the file after the delete (DIAG-06g's property, asserted here for
   these edits). Tests: P `diag06e_criterion_7_*`, D
   `diag06e_criterion_7`.
8. A property test deletes random selections on generated chains with
   `if`s and checks: either a refusal with nothing changed, or criterion
   4 and the check; then Undo, and the bytes are those before.
   Test: P `diag06e_criterion_8`. REL-05's condition 5 names it.
9. The legacy window's delete tests pass unchanged
   (`app_regressions.rs`). Test: those suites.

## Regression potential

| What can break | Covered by |
| --- | --- |
| Deleting what nothing reads, the common case | criteria 4 and 8; the board suites' deletes unchanged |
| *Save as component* and *Extract*, which remove what they move out | their suites unchanged; they keep their own rule |
| The legacy window | criterion 9 |

## Not in scope

Moving (DIAG-06a, DIAG-06d). Paste and duplicate (DIAG-06f). The notice
after a delete (DIAG-06b).

## Technical appendix

`apiforge-core/src/edit/` (a new `delete.rs`: the preview and the delete;
`graph.rs` `delete_nodes` untouched), `apiforge-desktop/src/board_edit.rs`, the existing confirmation
dialog, the banner. `apiforge-script/src/edit.rs` is not changed: removing
a statement's lines is what it does today.
