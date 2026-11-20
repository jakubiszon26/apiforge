# DIAG-06f: Pasted and duplicated steps land in the chain and keep what they read

- Size: M · Owner review: 15 min · Risk: `default`, `visible UI`
- State: **Decided; waits for the independent review before it is
  labelled.** Split from DIAG-06e on 2026-11-20. The owner's answers of
  that day: on #494, 2 = A (*Duplicate* directly after its original;
  *Paste* after the selected step, else at the end) and 3 = A (a
  reference that still finds its step is kept; otherwise the paste is
  refused); on [#496](https://github.com/jakubiszon26/apiforge/issues/496),
  A — an `if` is copied and duplicated with everything in its braces, and
  steps selected without their `if` are pasted as ordinary steps of the
  place. After DIAG-06d (#493). A condition of 0.2.0.
- Branch `feat/diag-06f-paste-and-duplicate`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A copied case stands beside its original and runs there; a paste never
leaves a step reading what is not there.

## What happens today

Read from `main`: *Copy* takes exactly the selected blocks and the edges
between them (`apiforge-core/src/edit/clipboard.rs`), so an `if`
selected alone is copied empty and steps selected without their `if`
lose their branch. A paste drops the references it cannot resolve, which
can leave a request without its URL. Pasted steps are written last
(LANG-13).

## What the owner will see

- Default behaviour that changes (answer 2 = A): *Duplicate* writes the
  copy directly after its original; *Paste* after the selected step, as
  *Add step* (DIAG-06a), else last in the main flow.
- Changed (answer 3 = A): a pasted reference whose step stands above the
  place of the paste is kept; a paste that would leave a statement
  reading what is not there is refused: *`pay` reads `cart`, which is not
  in this scenario. Copy it too.*
- Default behaviour that changes (the answer on #496): *Copy* and
  *Duplicate* take an `if` with everything in its braces, also when only
  the `if` is selected; steps selected without their `if` are pasted as
  ordinary steps of the place they are pasted to.
- Removed or hidden, migrated: nothing.

## Mock-up

Not applicable: no new element; the banner's sentence.

## Acceptance criteria

Tests named `diag06f`, in the files of
DIAG-06a; every test of an edit ends by asserting the check. Placement
goes through DIAG-06a's contract.

1. *Duplicate* of one step writes the copy, with the step's own checks
   and values, directly after the original and those. Of several: what
   is selected is taken per braces; in each braces the copies follow the
   last selected step of that braces, in their order. A selected
   statement that stands inside a selected `if` is copied once, with it.
   Tests: K, D `diag06f_criterion_1_*`, among them a selection across
   *Yes* and *No*.
2. An `if` is copied and duplicated with every statement in its braces,
   at any depth, also when only the `if` is selected; the copy's branches
   hold what the original's hold. Steps selected without their `if` are
   pasted as steps of the braces of the place, in their order, with no
   branch of their own. Tests: K `fragment_takes_the_braces`,
   `fragment_without_its_if_is_plain_steps`; D `diag06f_criterion_2_*`.
3. *Paste* writes the fragment after the selected step, in its braces
   (DIAG-06a's rule for *Add step*, also for several selected and for a
   check selected), else last in the main flow, in the copied order.
   Test: D `diag06f_criterion_3_*`.
4. A reference of a pasted statement is kept when it resolves at the
   place: its step exists there by identity or handle, **has that
   output**, and the result has no cycle (the step stands above the
   place, in DIAG-06a's sense). Whether the value will exist at run time
   across branches is not judged, as in DIAG-06a.
   Tests: K `paste_keeps_*`.
5. A paste is refused, names what is missing and changes nothing — the
   file, the selection and the Undo history are as before — when after it
   a statement would read: a step that is not there; a step of that name
   without that output; a scenario parameter or a component input the
   target does not declare; something inside a nested binding that does
   not resolve; a step below the place (a cycle). Nothing is pasted in
   part. Tests: K `paste_refuses_*`, one per case; D
   `diag06f_criterion_5_*`, also from another scenario.
6. After a paste or a duplicate, every input of every statement that was
   there before is equal to what it was, and every pasted statement
   stands in the braces of the place.
   Test: P `diag06f_criterion_6` (compared binding by binding).
7. The property test of DIAG-06a's criterion 10 also duplicates and
   pastes; after Undo the bytes are those before (DIAG-06e, criterion 7).
   Test: P `diag06f_criterion_7`.
8. The legacy window's copy and paste tests pass unchanged.

## Regression potential

Copy and paste as UX-34 built them (its tests restated where they assert
the place or the fragment, listed in the pull request); *Save as
component*, which copies a selection by its own rule (suite unchanged);
handles numbered when taken (unchanged).

## Not in scope

Deleting (DIAG-06e). Moving (DIAG-06a, DIAG-06d). Copying between
projects' components and their revisions beyond what paste does today.

## Technical appendix

`apiforge-core/src/edit/clipboard.rs` (the fragment and its resolution;
the legacy caller keeps today's functions), `place.rs`,
`apiforge-desktop/src/board_edit.rs`.
