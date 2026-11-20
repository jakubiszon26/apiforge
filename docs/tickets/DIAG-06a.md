# DIAG-06a: A step is placed and moved inside its braces, and the chain holds

- Size: M · Owner review: 20 min · Risk: `default` (what *Add step* and a
  wire to an existing step do), `visible UI` (refusal sentences)
- State: **Approved, ready for implementation after LANG-13 (#386),
  LANG-06 (#143) and DIAG-06e (#494) have merged.** It no longer waits
  for LAY-45 (#468), which stopped at its feasibility gate on 2026-11-20:
  this ticket's gestures are a wire between two sockets found by their
  names, and its tests read no position of a socket or a line, so they
  hold wherever LAY-45 puts them. What needs the drawn line — captions,
  a block dropped on a line — is DIAG-06c (#490), which still follows
  LAY-45. The owner answered the
  quiz on the round-1 mock-up on 2026-11-20, every question with A
  ([#391](https://github.com/jakubiszon26/apiforge/issues/391)). **Narrowed
  the same day** after an independent review found the first version an
  L: moves of an `if` and across braces are DIAG-06d (#493); deleting a
  step that is read, paste and duplicate are DIAG-06e (#494). This ticket
  keeps the one placement contract the others build on, and questions 3,
  4 (*Add step*), 10, 15 and 16.
- Issue [#391](https://github.com/jakubiszon26/apiforge/issues/391) · LANG
  sprint, first half, a condition of 0.2.0 (REL-05) · branch
  `feat/diag-06a-edits-keep-the-chain`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

An ordering wire dropped on a step of the same braces moves that step;
*Add step* puts a step where the selection is. After either, the steps
are still one chain and the file passes the check.

## What happens today (on `main` with LANG-13)

A step from a card's *Next* is inserted after it; a removed step joins
its neighbours. An ordering wire drawn between two steps that both exist,
and a deleted ordering wire, change nothing (LANG-13, consequence 3).
*Add step* on the bar writes the step last.

## What the owner will see

- Changed (question 3): an ordering wire from a step's *Next* dropped on
  a step of the same braces **moves that step** to directly after the
  step the wire came from; its old neighbours join. One Undo takes it
  back.
- Changed (question 16): the checks and values written directly under a
  step that read only that step move with it.
- New (question 15): a move after which a step would read a value made
  by a step below it is refused: *`pay` reads `cart.body.id`; it stays
  below `cart`.* In today's banner; DIAG-06b shows it while carrying.
- Default behaviour that changes (question 4): *Add step* on the bar puts
  the new step after the selected step, else at the end of the main flow.
- Changed (question 10): *Delete* on a selected ordering line is refused:
  *The line is the order of the steps. Move or delete a step.*
- Until DIAG-06d: a wire dropped on an `if` that has statements in its
  braces, or on a step of other braces, is refused: *Moving a condition
  with its steps, or a step into another branch, is not possible yet.*
  That sentence leaves with DIAG-06d; no release is cut between.
- Removed or hidden, migrated: nothing.

## Mock-up

Approved on #391 (views 2–4 for what a gesture leaves). No new element.
The pull request shows one board before and after a move, at 960 × 540
and 1600 × 1000.

## The placement contract

One operation, *put step `s` at a place*: directly after a step `a`, or
first in a braces. DIAG-06d and DIAG-06e add cases to it, not a second
one.

1. It works on the document of a **folder project**. The functions the
   legacy window calls (`connect`, `add_node`, `paste_selection`,
   `delete_nodes`, `crates/apiforge-desktop/src/app.rs`) keep their
   behaviour: the legacy JSON workspace stays outside the rules of
   language 2 (LANG-00). The chain-keeping operation is new and is called
   from the folder window's path only.
2. It returns the document the text will say, with its ordering edges as
   one chain per braces, or a refusal that names a statement.
3. It is refused when the result would not pass the validation a source
   passes — a cycle through values or order, directly or through a value
   or a variable between them — and the refusal names the reader, the
   value and the step it stays below. Whether a value will *exist* at run
   time (a reader in the other branch) is not judged here: the check does
   not judge it either.
4. The text is written by moving the statement's own lines (LAY-34) and
   reads back as the document; otherwise the edit is refused, never
   printed whole.

## Acceptance criteria

Tests: `crates/apiforge-core/src/edit/` unit tests (K),
`crates/apiforge-script/src/edit.rs` beside the LAY-34 tests (S),
`crates/apiforge-project/tests/diag06a.rs` (P),
`crates/apiforge-desktop/src/folder/acceptance_tests/diag06a.rs` (D), on
`labels`, `shop-30` and `rooms/booking-day`. **Every test of an edit ends
by asserting that the text passes the check and reads back as the
document.**

1. A wire from *Next* of `a` to a step `c` of the same braces: `c` runs
   directly after `a` and `a`'s checks, the step that followed `a`
   follows `c`, the neighbours `c` left are joined; forwards and
   backwards; in the main flow, in a branch, in the cleanup and in a
   component. Tests: K `place_moves_a_step`, D `diag06a_criterion_1_*`.
2. The file after a move differs from the file before only by the moved
   statements' lines standing in the new place, with their notes and the
   comments attached to them; nothing is reformatted.
   Test: S `a_moved_statement_takes_only_its_own_lines`.
3. The checks, values and variables written directly under a step that
   read that step and no other step move with it, in their order; one
   that also reads another step stays. Tests: K, S
   `a_step_moves_with_its_checks`, D `diag06a_criterion_3`.
4. A move is refused, with the sentence above and nothing changed,
   exactly when the result has a cycle through ordering and values: the
   moved step above the step that makes what it reads; a step below a
   step that reads it; the same through a value or a variable that
   stands between them; the same for a statement that travels with the
   step. Where a check or value is *written* is no reason: they stand
   anywhere in their block (rule 5 of language 2). Tests: K
   `place_refuses_*` (one per case) and
   `place_allows_a_data_statement_written_above`; D
   `diag06a_criterion_4_*` (the banner's text).
5. *Add step* on the bar: with a step selected the new step runs directly
   after it, in its braces; with an `if` selected, after the whole `if`;
   with several selected, after the one that runs last; with a check or
   value selected, after the step it is written under, else at the end;
   with nothing selected, last in the main flow.
   Tests: D `diag06a_criterion_5_*`.
6. A second step from a *Next* that has a successor, and a step from
   *Yes* or *No*, are inserted as today (FLOW-00 decision 6), also in the
   cleanup and in a component. Tests: the DIAG-07 suite unchanged; D
   `diag06a_criterion_6_*`.
7. *Delete* on a selected ordering line is refused with the sentence
   above; the line stays selectable. Test: D `diag06a_criterion_7`.
8. A wire dropped on an `if` with statements in its braces, or on a step
   of other braces, is refused with the sentence above and changes
   nothing. Test: D `diag06a_criterion_8_*`.
9. The legacy window: its tests of wiring, adding, pasting and deleting
   pass unchanged (`app_regressions.rs`, the `apiforge-core` edit tests
   of today). Test: those suites, named in the pull request.
10. No sequence of these edits writes a file that fails the check: a
    property test applies random places and moves to generated chains
    with `if`s and checks the text after each.
    Test: P `diag06a_criterion_10_edits_keep_the_check`.
11. Each edit is one step of Undo and Redo, and Undo gives the file back
    byte for byte (DIAG-06e, criterion 7). Test: P
    `diag06a_criterion_11_*`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| LAY-34: an edit changes only the statement edited | criterion 2; the `edit.rs` suite unchanged |
| #433 and DIAG-07: where a check and a step from *Next* are written | their suites unchanged |
| The legacy window, which shares the core's edit functions | criterion 9 |
| Data wires and their validation (`connect`: cycles, bound inputs) | the `connect` unit tests unchanged for data ports |
| LANG-13's interim (an ordering wire changes nothing) is taken out, not left beside the new rule | LANG-13's criterion 9 tests restated for the move, listed |
| Comments and attribute order under an edit (LANG-06) | LANG-06's criterion 6 test unchanged |

## Not in scope

- Moving an `if` with its braces, a move across braces, a check into or
  out of a branch, deleting an `if`: DIAG-06d (#493).
- Deleting (DIAG-06e, #494, which lands before this ticket). Paste and
  duplicate (DIAG-06f, #496): until then they do what they do after
  LANG-13.
- The `if`'s own *Next* socket, captions, a block dropped on a line, the
  row making room: DIAG-06c (#490). The refusal while carrying, the
  notice, `+`, *Move earlier* and *Move later*: DIAG-06b (#491).

## Technical appendix

- `apiforge-core/src/edit/place.rs` (new): the operation and its
  refusals; `connect.rs`, `nodes.rs` unchanged in behaviour for their
  present callers.
- `apiforge-script/src/edit.rs`: a statement whose place changed is cut
  and written in its new place (`classify`, `insert`); after LANG-06.
- `apiforge-project/src/session.rs`: the folder session calls the
  operation; the edge rebuild LANG-13 added for the interim is removed
  for the edits this ticket covers and kept for paste and duplicate
  until DIAG-06f.
- `apiforge-desktop`: `board_edit.rs` (the folder path), the banner's
  sentences.
- The document stays a graph of ordering edges; no list of steps is
  stored.
