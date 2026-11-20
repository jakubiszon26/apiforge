# DIAG-06a: Every edit on the board keeps the chain: place, move and remove a step

- Size: M · Owner review: 25 min · Risk: `default` (what *Add step*, a
  paste and a wire to an existing step do), `visible UI` (one
  confirmation, one menu entry, refusal sentences)
- State: **Approved, ready for implementation after LANG-13 (#386),
  LANG-06 (#143) and LAY-45 (#468) have merged.** The owner answered the
  quiz on the round-1 mock-up on 2026-11-20, every question with A
  ([#391](https://github.com/jakubiszon26/apiforge/issues/391)); FLOW-00's
  decision 6 (a second wire inserts) stands. This ticket builds questions
  3, 4 (*Add step*), 7, 9, 10 and 13–16 as operations and through the
  controls that exist; what is new to see on the board is DIAG-06c (#490),
  the feedback DIAG-06b (#491).
- Issue [#391](https://github.com/jakubiszon26/apiforge/issues/391) · LANG
  sprint, first half, a condition of 0.2.0 (REL-05) · branch
  `feat/diag-06a-edits-keep-the-chain`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

Whatever a person does on the board — add, wire, move, paste, delete —
the steps stay one chain per braces and the file passes the check. An
ordering wire dropped on a step that exists moves that step; nothing on
the board is refused silently or does nothing.

## What happens today (on `main` with LANG-13)

A block from the library is written last and runs last; a step from a
card's *Next* is inserted after it; a removed step joins its neighbours.
An ordering wire drawn between two steps that both exist, and a deleted
ordering wire, change nothing (LANG-13, consequence 3). *Add step* on the
bar writes the step last. A check takes an ordering wire from any step.
An `if` with steps in its braces is deleted without a word, and its steps
stay behind outside it.

## What the owner will see

- Changed (question 3): an ordering wire from a step's *Next* dropped on
  a step that is already in the chain **moves that step** to directly
  after the step the wire came from; its old neighbours join. One Undo
  takes it back.
- Changed (question 13): an `if` moves with everything in its braces.
- Changed (question 14): a step dropped behind a step of another braces
  goes into those braces — into a branch, out of one, from *Yes* to *No*.
- Changed (question 16): the checks and values written directly under a
  step that read only that step move with it.
- New (question 15): a move that would put a step above a step whose
  value it reads is refused: *`pay` reads `cart.body.id`; it stays below
  `cart`.* In today's banner; DIAG-06b shows it while carrying.
- Default behaviour that changes (question 4): *Add step* on the bar, and
  a paste or duplicate of steps, put the new steps after the selected
  step, else at the end of the main flow. (For a paste this follows from
  the answer for *Add step*; the owner may veto it at acceptance.)
- Changed (question 7): deleting an `if` that has steps in its braces
  asks once — *Delete `paid` and the 3 steps inside it?* — and then
  deletes all of it; Undo brings it back.
- Changed (question 9): a check, value, variable or export takes an
  ordering wire from *Yes* or *No* only, which puts it into that branch;
  a wire from a step's *Next* is refused: *Only steps are ordered. A check
  runs when what it reads exists.* New: *Move out of the branch* in such
  a card's menu.
- Changed (question 10): *Delete* on a selected ordering line is refused:
  *The line is the order of the steps. Move or delete a step.*
- Removed or hidden: nothing. Migrated: nothing.

## Mock-up

Approved on #391 (views 2–4 for what a gesture leaves). This ticket adds
no element except the confirmation of question 7, which uses the
existing confirmation dialog; the pull request shows it and one board
before and after a move, at 960 × 540 and 1600 × 1000.

## Acceptance criteria

Tests: `crates/apiforge-core/src/edit/` unit tests (K),
`crates/apiforge-script/src/edit.rs` beside the LAY-34 tests (S),
`crates/apiforge-project/tests/diag06a.rs` (P),
`crates/apiforge-desktop/src/folder/acceptance_tests/diag06a.rs` (D).
On `labels`, `shop-30` and `rooms/booking-day`. **Every test of an edit
ends by asserting that the text passes the check and reads back as the
document** (REL-05, condition 5).

1. An ordering wire from *Next* of `a` to a step `c` elsewhere in the
   same braces: `c` runs directly after `a` (and `a`'s checks), the step
   that followed `a` follows `c`, the neighbours `c` left are joined.
   Tests: K `place_moves_a_step`, D `diag06a_criterion_1_*` (forwards and
   backwards in the chain).
2. The file after a move differs from the file before only by the moved
   statement's lines standing in the new place, with its note and the
   comments attached to it; no other line changes and nothing is
   reformatted. Test: S `a_moved_statement_takes_only_its_own_lines`.
3. An `if` moves with every statement in its braces, at any depth.
   Tests: K `place_moves_an_if_with_its_braces`, S (the text), D
   `diag06a_criterion_3`.
4. A step moved behind a step that stands in other braces stands in those
   braces afterwards: into a branch, out to the main flow, from *Yes* to
   *No*; a step cannot be moved into its own braces (an `if` into
   itself): refused. Tests: K `place_across_braces_*`, D
   `diag06a_criterion_4_*`.
5. The checks, values and variables written directly under a step that
   read that step and no later one move with it, in their order; one that
   also reads another step stays. Tests: K, S
   `a_step_moves_with_its_checks`, D `diag06a_criterion_5`.
6. A move that would put a step above a step whose value it reads —
   itself, a statement that moves with it, or a statement inside a moved
   `if` — is refused, names the reader, the value and the step it stays
   below, and changes nothing. A move **down** past a step that reads the
   moved one is refused the same way. Tests: K `place_refuses_*`, D
   `diag06a_criterion_6_*` (the banner's text).
7. *Add step* on the bar: with a step selected the new step runs directly
   after it, in its braces; with an `if` selected, after the whole `if`;
   with nothing selected, last in the main flow. A paste and a duplicate
   of steps follow the same rule and keep their own order. Tests: D
   `diag06a_criterion_7_*`; the UX-34 paste tests restated where they
   assert the place, listed in the pull request.
8. A second step from a *Next* that has a successor, and a step from
   *Yes* or *No*, are inserted as today (FLOW-00 decision 6), also in the
   cleanup and in a component. Tests: the DIAG-07 suite unchanged; D
   `diag06a_criterion_8_*` for the cleanup and a component.
9. Deleting an `if` with steps in its braces asks once, naming the `if`
   and the count; *Delete* removes all of it and joins its neighbours,
   *Cancel* changes nothing; one Undo restores it. An `if` with empty
   braces, and any other step, is deleted without a question as today.
   Tests: D `diag06a_criterion_9_*`.
10. A check, value, variable or export: a wire from *Yes* or *No* puts it
    into that branch; a wire from a step's *Next* is refused with the
    sentence above and changes nothing; *Move out of the branch* writes
    it after the `if`'s braces. Tests: K, D `diag06a_criterion_10_*`.
11. *Delete* on a selected ordering line is refused with the sentence
    above; the line stays selectable. Test: D `diag06a_criterion_11`.
12. No sequence of these edits writes a file that fails the check: a
    property test applies random places, moves, deletes and pastes to
    generated chains with `if`s and checks the text after each.
    Test: P `diag06a_criterion_12_edits_keep_the_check`.
13. Each edit is one step of Undo and Redo. Test: P
    `diag06a_criterion_13_*`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| LAY-34: an edit changes only the statement edited | criterion 2; the `edit.rs` suite unchanged |
| #433 and DIAG-07: where a check and a step from *Next* are written | their suites unchanged |
| A move changes what runs under a condition (question 14, decided) | criterion 4 asserts the run order before and after on `shop-30` |
| Data wires and their validation (`connect`: cycles, bound inputs) | the `connect` unit tests unchanged for data ports |
| LANG-13's interim (an ordering wire changes nothing) is taken out, not left beside the new rule | P tests of LANG-13 criterion 9 restated for the move, listed |
| Comments and attribute order under an edit (LANG-06) | LANG-06's criterion 6 test unchanged |

## Not in scope

- The `if`'s own *Next* socket, the caption while carrying, a block
  dropped on a line, the row making room: DIAG-06c (#490). Until then a
  step is put after a whole `if` by *Add step* with the `if` selected.
- The refusal shown while carrying, the notice after a delete, `+` on a
  line, *Move earlier* and *Move later*: DIAG-06b (#491).
- Bands: LAY-13 (#297).

## Technical appendix

- One operation in `apiforge-core/src/edit/` (a new `place.rs`):
  *put step `s` directly after step `a`* or *first in braces `b`*, for a
  new or an existing step. It carries the `if`'s braces and the step's
  own checks, checks the data dependencies both ways, and returns the
  graph with its ordering edges as a chain. `connect` for an ordering
  port, `add_node`, `paste_selection` and `delete_nodes` go through it;
  the edge rebuild LANG-13 put into `apiforge-project/src/session.rs` for
  the interim is removed where this makes it unnecessary.
- `apiforge-script/src/edit.rs`: a statement whose place changed is cut
  and written in its new place (`classify`, `insert`); after LANG-06,
  which edits the same file.
- `apiforge-desktop`: `board_edit.rs`, `canvas/menu.rs` (the entry),
  the existing confirmation dialog, the banner's sentences.
- The document stays a graph of ordering edges; no list of steps is
  stored (the owner's condition of 2026-11-17).
