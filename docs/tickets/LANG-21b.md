# LANG-21b: The list of statements to place by hand leads to each one, and the count follows each save

- Size: M · Owner review: 15 min · Risk: `visible UI`
- State: **Approved, ready for implementation after LANG-21a (#482) has
  merged.** Split from LANG-21 on 2026-11-20. Owner's answers on #482:
  5 = A (*Show*, *Show the list*), 9 = A (the count follows each save);
  views 4a and 4b of the approved mock-up. Not a condition of 0.2.0.
- Issue [#495](https://github.com/jakubiszon26/apiforge/issues/495) · LANG
  sprint · branch `feat/lang-21b-place-by-hand`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

When a project cannot be updated as it is, the person gets from the list
to each statement in one click, and sees the list shrink as they work,
without pressing the button to find out.

## What the owner will see

- New (answer 5, view 4a): each row of the list has *Show*: the dialog
  closes, the file opens with the statement selected, and that statement
  is the first of the file's problems.
- New (view 4b): while files are left to place by hand — after an
  update that converted only some, or none — the band adds the count of
  statements left and *Show the list* to LANG-21a's line (*being
  updated: n of m files*, or the first offer when nothing is converted
  yet).
- New (answer 9): every save checks the project again, writing nothing;
  the count follows, and at none left the band reads as when the project
  opened.
- Removed or hidden, migrated: nothing.

## Mock-up

Approved on #482: views 4a and 4b at 960 × 540 and 1600 × 1000. Release
captures in the pull request.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lang21b.rs`
(D) and `crates/apiforge-project/tests/lang21b.rs` (P), on LANG-21a's
two projects with blocked files (some convertible; none).

1. *Show* on a row closes the dialog, opens that file in Code (or Split,
   as the window is) and selects the statement's lines; the row's problem
   is the first of that file's Problems, with the plan's sentence.
   Test: D `lang21b_criterion_1_*`.
2. While the plan lists statements to place by hand, the band shows
   *k statements to place by hand* and *Show the list*, which opens
   LANG-21a's dialog with the current plan, beside LANG-21a's line and
   button — in a project with a record and in one where nothing could be
   converted yet. Test: D `lang21b_criterion_2_*`.
3. After each save the plan is made again, nothing is written (no
   source, no record, no manifest), and the band's count equals the
   plan's; when it is none, the band is LANG-21a's alone. Before the
   button was first pressed in this session, and in a project without a
   record, no plan is made at a save.
   Tests: P `lang21b_criterion_3_*`, D `lang21b_criterion_3_*`.
4. The check at a save does not hold the window: for a project of 50
   sources it takes under 250 ms on the reference machine or runs off the
   frame, and a plan that arrives after a later save is dropped.
   Test: P `lang21b_criterion_4_*`.
5. Accessible names of every *Show* are distinct (#235); the list is
   reachable by keyboard. Test: `accessibility_tests`, D.

## Regression potential

The Problems section's order and its tests (KTLO-18, UX-20); the band's
other notices (`notices`, `tab_band_tests`); saving (`saving.rs` suites
unchanged: the check is after the save, never part of it).

## Not in scope

The offer, the dialogs and the update (LANG-21a). Converting anything.

## Technical appendix

`crates/apiforge-desktop/src/folder/` (`update_project.rs` of LANG-21a,
`notices.rs`, `navigation.rs`, `analysis.rs` for the problem's place),
`crates/apiforge-project/src/language.rs` (the plan, called again).
