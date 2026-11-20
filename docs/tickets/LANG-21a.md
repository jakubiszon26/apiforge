# LANG-21a: The window offers the update of a project written in the older language

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: **Approved, ready for implementation after LANG-20 (#481) has
  merged.** The owner chose on 2026-11-20 that 0.2.0 has it (option A on
  [#482](https://github.com/jakubiszon26/apiforge/issues/482)) and
  answered the nine questions of the round-1 mock-up, each with A; the
  mock-up is approved as a whole. **Split on 2026-11-20** after an
  independent review found it an L: this ticket is the offer, the check
  and the update (answers 1–4, 6–8); *Show*, *Show the list* and the
  count that follows each save (answers 5 and 9) are LANG-21b (#495).
  Amended by the same review: what the check reads, what *Save all*
  cannot save, and a check that has gone stale.
- **To be amended** when question 2 on #481 is answered: the owner
  chose on 2026-11-20 that a project is converted file by file, with a
  record (LANG-20). A project can then be partly updated, which the
  approved views do not show: the dialog of view 3a (what will change)
  and the list of view 4a (what is left) can both apply at once, and the
  band has a third state, *being updated: n of m files*. Criteria 2–5
  are restated for the answer; until then this ticket is not taken.
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
- New (question 2): the button writes nothing. It checks the whole
  project (LANG-20's check) and opens one dialog: what will change, per
  file, with *Update N files* — or the statements to place by hand.
- New (question 3): with unsaved work the dialog lists it and its button
  is *Save all and update*.
- New (question 4): after the update the open files show the new text,
  the offer is gone and a notice in the band counts what changed; it
  stays until dismissed.
- LANG-21b (question 5): the list of statements to place by hand is per file,
  each with *Show*: the dialog closes, the file opens with the statement
  selected, the statement is the first of the file's problems, and the
  band keeps the count, *Show the list* and the button.
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
`mock/lang-21-update-project`, `b3b9410`). View 1b was not chosen. The
pull request shows release captures of the same views.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lang21.rs`
(D) and `crates/apiforge-project/tests/lang21.rs` (P), on the language-1
copy of `rooms` that LANG-20 keeps as test data, and on that copy with
one statement moved above the step it follows and one request nothing
orders (view 4).

1. A language-1 project shows the band with its sentence and *Update the
   project* in Code, Diagram and Split and on every tab; a language-2
   project shows none. At both window sizes the button is inside the
   window. Test: D `lang21_criterion_1_*`.
2. The button writes nothing: after it, every file and the manifest are
   byte-identical. The dialog shows LANG-20's **plan made from what would
   be saved**: for a file with unsaved changes, its text in the editor;
   for every other, the file. With nothing unsaved it equals the report
   of `apiforge fix --folder --check`. Test: D `lang21_criterion_2_*`,
   among them an unsaved change that removes a blocker and one that adds
   one.
3. *Update N files* commits **that plan**: the files and the manifest
   are those the plan listed; the open tabs show the new text without a
   conflict prompt; the band's offer is gone; the notice gives the counts
   and stays until dismissed; a run then starts. When anything the plan
   was made from has changed since — a buffer, a file on disk, the
   manifest, who holds the project — nothing is written and the dialog
   shows the new plan with one line saying why.
   Test: D `lang21_criterion_3_*`, among them a foreign write and an edit
   between the check and the button.
4. A project that cannot be updated: the dialog lists the statements per
   file with their line and offers no update; nothing is written. (What
   leads to each statement is LANG-21b.)
   Test: D `lang21_criterion_4_*`.
5. With unsaved work the dialog names the unsaved files first and its
   button reads *Save all and update*; it saves them and commits the
   plan, which was made from exactly those texts; *Cancel* saves nothing.
   What *Save all* does not save — a source held because it does not
   read, an unfinished field of the Inspector, a file in conflict — is
   named in the dialog, which then offers no update: *Finish or discard
   these first.* A save that fails for one file stops there: nothing is
   converted, the files already saved stay saved, and the dialog says
   which failed. Test: D `lang21_criterion_5_*`, one per case.
6. The band has no dismiss control while the project is in language 1;
   *Project ▸ Update the project…* does what the button does and is
   absent or disabled in a language-2 project.
   Test: D `lang21_criterion_6_*`.
7. Held by an agent session or by a command, and read-only: the button
   and the menu entry are disabled and the reason is beside the button
   and in the accessibility tree; after the agent's `fix` the window
   shows the project updated and no offer.
   Test: D `lang21_criterion_7_*` (with the MCP harness of `mcp08b`).
8. In a language-1 project the error on an `@after` line ends with
   *Update the project.* and carries no fix; in a language-2 project it
   is LANG-13's text with its fix; the run refusal names the button
   first. Tests: P `lang21_criterion_8_*`, D `lang21_criterion_8_*`.
9. (Moved to LANG-21b.)
10. Accessible names of the band and both dialogs are distinct (#235); everything is reachable by keyboard.
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
  LANG-13 and LANG-20. A file converted by hand is passed as written from
  the command line only.
- A project in a newer language (LANG-15's sentence, no button).
- Undo of the update: Git.

## Technical appendix

`crates/apiforge-desktop/src/folder/` (`notices.rs`, `chrome.rs`, a new
`update_project.rs` for the two dialogs, `runs.rs` for the refusal's
text, `holder.rs`), the *Project* menu; `crates/apiforge-project/src/`
(`language.rs`: the check as a call that returns the report; `analysis.rs`:
the wording of criterion 8 where the fix is withheld). No new function
converts anything: the window calls LANG-20's.
