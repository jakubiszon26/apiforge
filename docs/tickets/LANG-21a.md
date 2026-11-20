# LANG-21a: The window offers the update of a project written in the older language

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: **Decided; waits for the independent review's last pass before
  it is labelled**; then ready after LANG-20b (#497) has merged. The
  owner's answer of 2026-11-20 on #482: the window takes a file the
  person rewrote by hand — *This file is ready as written* (A). The owner chose on 2026-11-20 that 0.2.0 has it (option A on
  [#482](https://github.com/jakubiszon26/apiforge/issues/482)) and
  answered the nine questions of the round-1 mock-up, each with A; the
  mock-up is approved as a whole. **Split on 2026-11-20** after an
  independent review found it an L: this ticket is the offer, the check
  and the update (answers 1–4, 6–8); *Show*, *Show the list* and the
  count that follows each save (answers 5 and 9) are LANG-21b (#495).
  Amended by the same review: what the check reads, what *Save all*
  cannot save, and a check that has gone stale.
- **Restated on 2026-11-20** for the owner's two answers on #481: a
  project is converted file by file, with a record (LANG-20), and the
  window's *Update the project* converts what it can (question 2 = A):
  one dialog with two parts, then the band *being updated: n of m files*.
  Views 3a and 4a of the approved mock-up become that one dialog; the
  pull request shows it and the band's third state for acceptance.
- **Restated again on 2026-11-20** after the independent review of the
  shared contract: the plan, its read set, the guarded commit and what
  happens after a save are LANG-20b's (#497) and the window only calls
  them; the window can finish a project when nothing is left to convert;
  a listed source with an error has its place in the dialog; *Save all*
  is the one that exists.
- Issue [#482](https://github.com/jakubiszon26/apiforge/issues/482) · LANG
  sprint, first half · branch `feat/lang-21-update-the-project`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A tester who never opens a terminal brings a 0.1.0 project to language 2
from the window: one button, a look at what will change before anything
is written, and — when the project cannot be updated as it is — a list
that leads to each statement to place by hand.

## What happens today (with LANG-13 and LANG-20)

A language-1 project opens; every `@after` line is an error without a
fix; a run is refused with a sentence that names `apiforge fix --folder`.
Only the command or an agent's `fix` converts the project.

## What the owner will see

- New (question 1): while a project is in language 1, the band under the
  tabs says so on every tab and in every mode and carries *Update the
  project*.
- New (question 2, and question 2 = A on #481): the button writes
  nothing. It makes LANG-20's plan and opens one dialog with two parts:
  **what will change**, per file, and **what is left to place by hand**,
  per file with its statements; either part is absent when it is empty.
  Its button reads *Update N files* and converts those; with nothing to
  convert there is no button.
- New: after an update that left files, the band reads *This project is
  being updated: n of m files are in the new language* with *Update the
  project*; the same band shows when the project was partly updated by
  the command or by an agent. Nothing runs until it is finished.
- New (question 3): with unsaved work the dialog lists it and its button
  is *Save all and update*.
- New (question 4): after an update the open files show the new text and
  a notice in the band counts what changed; it stays until dismissed.
  When nothing was left, the offer is gone and the project runs.
- LANG-21b (question 5): the list of statements to place by hand is per file,
  each with *Show*: the dialog closes, the file opens with the statement
  selected, the statement is the first of the file's problems, and the
  band keeps the count, *Show the list* and the button.
- New (the owner's answer of 2026-11-20): in the dialog a file left to
  do by hand has *This file is ready as written*; after one confirmation
  it is listed as updated if it passes the check.
- New (question 6): the offer cannot be dismissed while the project is in
  language 1; *Update the project…* is also an entry of the *Project*
  menu.
- New (question 7): when an agent session or a command holds the project,
  or the project is read-only, the button is disabled with the reason
  beside it; when the agent's own `fix` succeeds the window reloads and
  the offer goes.
- Changed (question 8): in a language-1 project the error on an `@after`
  line ends with *Update the project.* instead of the advice to delete
  the line; the run refusal names the button, and the command after it.
  **This ticket owns that wording**: LANG-13 writes the message for a
  language-2 project and leaves the language-1 case without a fix
  (its amendment A5); the text shown there is changed here, in the place
  that withholds the fix.
- LANG-21b (question 9): after an update that could not be done, every save
  checks again (nothing is written) and the band's count follows; at none
  left the band reads as when the project opened.
- Removed or hidden, migrated: nothing. Converted: only by the button's
  second step.

## Mock-up

Approved on #482: views 1, 2, 3a, 3b, 4a, 4b and 5 at 960 × 540 and
1600 × 1000 (`docs/qa/2026-11-20/LANG-21/` on the branch
`mock/lang-21-update-project`, `b3b9410`). View 1b was not chosen. Views
3a and 4a are built as the two parts of one dialog (question 2 = A on
#481); the band's third state, *being updated*, has no view. The pull
request shows release captures of every view, of the two-part dialog on
the partly convertible project and of that band, for acceptance.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lang21.rs`
(D) and `crates/apiforge-project/tests/lang21.rs` (P). Three projects, all
synthetic: the language-1 copy of `rooms` that LANG-20 keeps as test data
(everything convertible); that copy with one statement moved above the
step it follows and one request nothing orders (two files blocked, the
others convertible); and a language-1 copy of `labels` with every file
blocked.

1. A language-1 project without a record shows the band with its
   sentence and *Update the project* in Code, Diagram and Split and on
   every tab; with a record it shows *being updated: n of m files* and
   the button; a language-2 project shows none. At both window sizes the
   button is inside the window. Test: D `lang21_criterion_1_*`.
2. The button writes nothing: after it, every file, the record and the
   manifest are byte-identical. The dialog shows LANG-20b's **plan made
   from what would be saved** — for a file with unsaved changes its text
   in the editor, for every other the file — in up to three parts, each
   absent when empty: **will change** (the files to convert, with what is
   removed), **left to do by hand** (blocked files with their statements
   and the reason; files that cannot be read), and **already updated,
   with errors** (listed files that do not pass the check, with their
   first error and the count). With nothing unsaved it equals the report
   of `apiforge fix --folder --check`. Test: D `lang21_criterion_2_*`,
   on the three projects, on one whose only obstacle is a listed file
   with an error, and with an unsaved change that removes a blocker and
   one that adds one.
3. The dialog's button commits the plan through LANG-20b's functions,
   and what it reads is what the plan will do:
   - *Update N files* when there is something to convert. Everything
     convertible and nothing else in the way: the project is language 2
     afterwards, there is no record, the offer is gone and a run starts.
     Some left: the record lists the converted sources, the band reads
     *being updated: n of m files*, a run is refused with LANG-20a's
     sentence, and the button shows the plan for what is left.
   - **Finish the update** when nothing is left to convert and every
     source is updated and passes — all were listed and the last error
     was repaired, or the last blocked file was deleted: it writes the
     manifest, removes the record, and a run starts.
   - No button when neither applies; the parts say why.
   The files, the record and the manifest are those the command writes
   for the same project. The open tabs show the new text without a
   conflict prompt; a notice gives the counts and stays until dismissed.
   When anything the plan read has changed since — a buffer, any source
   on disk, the record, the manifest, who holds the project — nothing is
   written and the dialog shows the new plan with one line saying why
   (LANG-20b's read set; the window adds only its buffers' versions).
   Test: D `lang21_criterion_3_*`, one per case, among them the finish
   after a repaired error, after a deleted file, a foreign write to a
   file the plan does not write, and an edit between the check and the
   button.
4. A project partly updated elsewhere — by the command or by an agent's
   `fix` while the window observes — shows the band of criterion 1 for a
   record and the same dialog; when the agent's `fix` finishes the
   project, the window shows it updated and no offer.
   Test: D `lang21_criterion_4_*` (with the MCP harness of `mcp08b`).
5. With unsaved work the dialog names what is unsaved first — files and
   settings — and its button reads *Save all and update* (or *Save all
   and finish*). It runs the window's **Save all as it is today** (which
   goes on after a file that fails) and then LANG-20b's *after a save*:
   the plan is made again from the disk and committed only when it would
   write exactly what the dialog showed; the settings just saved are in
   the manifest written. *Cancel* saves nothing.
   - What *Save all* does not save — a source held because it does not
     read, an unfinished field of the Inspector, a file in conflict — is
     named in the dialog, which then offers no button: *Finish or discard
     these first.*
   - A save that fails for a file: nothing is converted; the other files
     are saved as *Save all* saves them; the dialog names the file that
     failed.
   - A foreign write during the save, or a saved buffer that changes what
     is blocked: nothing is converted and the dialog shows the new plan.
   Test: D `lang21_criterion_5_*`, one per case, among them unsaved
   settings (kept after the update), a blocked file with unsaved changes,
   and the state of the files after the one that failed.
6. *This file is ready as written* (the owner's answer on #482): each
   file of *left to do by hand* that can be read has it. It asks once —
   *Steps that did not run in 0.1.0 will run where they are written.* —
   and on *Yes* does what `--as-written` does through LANG-20b: the file
   is listed as updated when it passes the language-2 check, with no byte
   of it changed; when it does not pass, nothing is listed and the dialog
   shows its errors. The dialog then shows the new plan. With unsaved
   changes in that file the action is offered after they are saved (the
   dialog's *Save all* path), never on a text that is not on disk. A file
   that cannot be read does not have it.
   Test: D `lang21_criterion_6_*`: the request nothing orders, accepted
   and then running where it is written after the finish; a file that
   fails the check; *No*; a file with unsaved changes.
7. The band has no dismiss control while the project is in language 1;
   *Project ▸ Update the project…* does what the button does and is
   absent or disabled in a language-2 project.
   Test: D `lang21_criterion_7_*`.
8. Held by an agent session or by a command, and read-only: the button
   and the menu entry are disabled and the reason is beside the button
   and in the accessibility tree. Test: D `lang21_criterion_8_*`.
9. In a language-1 project the error on an `@after` line of a source not
   yet converted ends with *Update the project.* and carries no fix; in a
   converted source and in a language-2 project it is LANG-13's text with
   its fix; the run refusal names the button first; LANG-20's refusal of
   a board edit (*Update the project first.*) is shown in the board's
   banner with the same button beside it.
   Tests: P `lang21_criterion_9_*`, D `lang21_criterion_9_*`.
10. Accessible names of the band in its three states and of the dialog's
   two parts are distinct (#235); everything is reachable by keyboard.
   Test: `accessibility_tests`, D `lang21_criterion_10`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| The band under the tabs (storage, stale, recovered notices; #320) | `notices` and `tab_band_tests` unchanged; criterion 1 at 960 × 540 |
| A rewrite of open files must not raise the foreign-write conflict | criterion 3; `conflicts` tests unchanged |
| Hand-over and the holder rules (MCP-07, MCP-08b) | criterion 7; their suites unchanged |
| The command and the window diverge | criteria 2 and 3: one plan, one commit, LANG-20's |
| Unsaved work lost at the reload after the update | criterion 5: no update while anything stays unsaved |
| Save all (unsaved prompts, MCP-08c) | criterion 5; their suites unchanged |

## Not in scope

- What the update does to a file, `--as-written`, the report's content:
  LANG-13 and LANG-20b.
- A project in a newer language (LANG-15's sentence, no button).
- Undo of the update: Git.

## Technical appendix

`crates/apiforge-desktop/src/folder/` (`notices.rs`, `chrome.rs`, a new
`update_project.rs` for the two dialogs, `runs.rs` for the refusal's
text, `holder.rs`), the *Project* menu; `crates/apiforge-project/src/`
(`analysis.rs`: the wording of criterion 8 where the fix is withheld).
No function of this ticket plans, compares fingerprints or converts: the
window calls LANG-20b's plan, commit and *after a save*.
