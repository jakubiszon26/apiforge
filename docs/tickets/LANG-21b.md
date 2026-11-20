# LANG-21b: The list of statements to place by hand leads to each one, and the count follows each save

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: **Approved, ready for implementation after LANG-21a (#482) has
  merged.** Split from LANG-21 on 2026-11-20. Owner's answers on #482:
  5 = A (*Show*, *Show the list*), 9 = A (the count follows each save);
  views 4a and 4b of the approved mock-up. Not a condition of 0.2.0.
- Restated on 2026-11-20 after an independent review: where *Show*
  lands in a buffer that changed, what the count is of, a save that
  fails, a source that does not read, and the plan off the window's
  thread. The size M holds because LANG-20b gives the plan as a call.
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
   as the window is) and selects the statement. The dialog's plan was
   made when the dialog opened, from the buffers (LANG-21a, *Which plan
   the window shows*), so its lines are those of the text on screen,
   unsaved changes included, and no unsaved edit is lost. The row's
   problem is the first of that file's Problems, with the plan's
   sentence; from then on it follows the statement by its handle while
   the person types, and when the handle is gone the problem goes. A
   problem's fix is applied by what the problem is — its source version
   and range — never by its place in the list.
   Tests: D `lang21b_criterion_1_*`, among them ten unsaved lines typed
   above the statement before *Show the list* is opened, ten typed after
   *Show*, and a file with two problems that carry different fixes where
   the second is shown first and its fix changes only its own range.
2. The band shows *k statements to place by hand* and *Show the list*
   beside LANG-21a's line and button whenever the **count of the saved
   files** is not none — in a project with a record and in one where
   nothing could be converted yet. *Show the list* opens LANG-21a's
   dialog, which makes its own plan from the buffers; when that differs
   from the band's count the dialog says so in one line (LANG-21a).
   Test: D `lang21b_criterion_2_*`, among them an unsaved buffer that
   repairs a blocker: the band still counts it, the dialog shows it
   convertible and says the file is not saved yet.
3. The count is of the last plan, and a plan is made from the saved
   sources: after a save **has completed** the plan is made again,
   nothing is written (no source, no record, no manifest), and the band's
   count equals the plan's; when it is none, the band is LANG-21a's
   alone. A save that fails makes no plan and leaves the count; of a
   *Save all* that saved some files, the plan follows the saves that
   completed. A source that cannot be read (a syntax error) is counted
   as such — *… and 1 file that does not read* — and never as nothing
   left. Before the button was first pressed in this session, and in a
   project without a record, no plan is made at a save.
   Tests: P `lang21b_criterion_3_*`, D `lang21b_criterion_3_*`, one per
   case.
4. The plan for the band is made off the window's thread: frames keep
   coming while it runs (the test holds the plan and steps the window).
   It is of the saved files, so typing does not make it wrong; it is
   used only while it is for the disk the window shows — the same
   session and project, the same saved versions of the sources, the same
   record and manifest. That is checked when the plan arrives; one that
   no longer holds — a later save, a reload, a foreign write, a rename or
   a delete of a source, a change of the record alone, another project,
   a finished update — is dropped, a new one is started at once for the
   new state, and until it arrives the band shows the count as *being
   checked*, never an old number as current.
   Test: P `lang21b_criterion_4_*`, one per case.
5. The problem *Show* puts first belongs to the plan: it is listed once,
   never beside the same statement's ordinary problem as a second row;
   it goes when a later plan no longer has it (the statement was placed
   and saved) and is replaced when a later plan has it at another line;
   with the last one gone the file's Problems are its ordinary ones.
   Test: D `lang21b_criterion_5_*`.
6. When the count reaches none and the project can be finished, the band
   is LANG-21a's with its button, which then offers *Finish the update*.
   Test: D `lang21b_criterion_6`.
7. Accessible names of every *Show* are distinct (#235); the list is
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
