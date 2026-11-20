# LANG-20d: Sources come and go while a project is being updated; an old source is not edited on the board

- Size: M · Owner review: 20 min · Risk: `format` (the record and the
  manifest are written; a trash entry gains a field), `default` (a board
  edit in an old source is refused)
- State: **Decided; waits for the independent review before it is
  labelled.** The owner's answer of 2026-11-20 on
  [#499](https://github.com/jakubiszon26/apiforge/issues/499): A — a
  file deleted in the old language and restored after the update is
  converted by the restore when it can be, and otherwise the restore is
  refused and the file stays in the trash. Split from LANG-20a on 2026-11-20. Builds the
  owner's decisions on #481: file by file, with a record (question 4 =
  B); a board edit in a source not yet converted is refused (question 1 =
  A). After LANG-20a (#481); rule 4's conversion
  uses LANG-20b's judgement and is enabled when LANG-20b (#497) has
  merged, whichever of the two merges second.
- LANG sprint, first half, a condition of 0.2.0 · branch
  `feat/lang-20d-sources-come-and-go`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

The record stays true while the project changes under it: a source that
is created, deleted, restored or renamed never ends up read in the wrong
language, and an old source is never rewritten in the new language by an
edit on the board.

## What happens today (with LANG-20a)

The record lists sources by `documentId`. The trash removes a document
from the manifest and keeps no note of its language
(`apiforge-storage/src/project/trash.rs`); a restore writes the original
bytes and registers them again. A rename changes the manifest's path.
None of them knows the record.

## What the owner will see

- New (question 1 = A on #481): in a project of the older language —
  with or without a record — a source that is not converted cannot be
  edited on the board. The board shows it; adding, wiring, moving a step,
  deleting and the Inspector's fields are refused with *Update the
  project first.* Its text can be edited in Code; a card can be moved.
- New: a source this build creates in such a project is written in
  language 2 and listed as updated at once.
- Changed: deleting, restoring and renaming a source keep the record
  right (below). The trash remembers which language a deleted source was
  in.
- New (the owner's answer on #499): restoring, into an updated project,
  a file that was deleted while it was still in the old language judges
  it as the update would. When it can be converted it is restored
  converted and the restore says so — *Restored and updated: 3 lines
  removed.* When it cannot, the restore is refused, names the statement
  and why, and the file stays in the trash, where its text can be read.
- Migrated, removed: nothing.

## Mock-up

Not applicable: the board's banner and the restore's existing messages.

## The rules

1. **Created.** A source this build creates in a language-1 project (the
   window's *New scenario* and *New component*, *Save as component*, MCP
   `create_document`) is written in language 2 and listed in the same
   commit; when there was no record, the record and the manifest's raise
   to `formatVersion: 2, languageVersion: 1` come in that commit. A file
   registered as it is (`register_existing`, a file that appears on
   disk) is an old source: not listed, nothing else changes. LANG-15's
   test that a registration leaves `formatVersion: 1` is restated: true
   for a file registered as it is, saving and publishing; not for a
   source this build writes new.
2. **Deleted.** Moving a listed source to the trash removes its entry
   from the record in the same commit. The trash entry notes the
   language the source was read in (*2* when it was listed or the project
   was language 2; *1* otherwise). An entry without the note — put there
   by 0.1.0 or before this ticket — is language 1 when the project was
   not yet language 2 when it was deleted, which the entry's manifest
   snapshot says, else 2.
3. **Restored into a project being updated**, or into a language-1
   project: a language-2 source comes back listed (the record and the
   manifest created if need be, as in rule 1); a language-1 source comes
   back unlisted.
4. **Restored into a language-2 project**: a language-2 source comes
   back as it is. A language-1 source is judged as LANG-20b's plan judges
   an unlisted source (its 0.1.0 reading against the order written; the
   converted text against the language-2 check): *to convert* — the
   converted text is what is written, in the restore's one commit, and
   the restore's message gives what was removed; *blocked* or
   *unreadable* — nothing is written, the entry stays in the trash, and
   the refusal names the statement and the reason. The trash's bytes are
   never changed, so a refused restore can be tried again after the
   project or the build changes. Until LANG-20b has merged, such a
   restore is refused with *This file is in the older language; it can be
   restored when the update is available.*
5. **Renamed or moved**: the entry keeps its `documentId`; its path and
   its place in the sorted list change in the same commit.
6. **Deleted for good** (the trash emptied): nothing in the record.
7. No rule here finishes an update: when the last unconverted source is
   deleted, the project is still being updated until the update is run
   and finishes it (LANG-20b).

## Acceptance criteria

Tests in `crates/apiforge-project/tests/lang20d.rs` (P),
`crates/apiforge-storage/src/project/trash_tests.rs` (T) and
`crates/apiforge-desktop/src/folder/acceptance_tests/lang20d.rs` (D), on
LANG-20a's language-1 `rooms` with a record and on a language-1 copy of
`labels`.

1. Rule 1, for each way of creating a source, with and without a record
   before; a file registered as it is stays unlisted and the manifest's
   bytes stay. Tests: P `lang20d_criterion_1_*`; LANG-15's criterion 2
   test restated and named in the pull request.
2. A listed source moved to the trash: the record has no entry for it,
   the project loads, and the entry in the trash says language 2.
   An unlisted one: the record is unchanged and the entry says language
   1. Tests: P, T `lang20d_criterion_2_*`.
3. Rule 3: both languages, restored into a project with a record and
   into one without. Tests: P `lang20d_criterion_3_*`.
4. Rule 4. A language-1 source that can be converted: after the restore
   the file holds the converted text, the project loads and passes the
   check, the message names the count, and the trash no longer holds it.
   One that cannot — the review's case: an unlisted source with a request
   without `@after` that ran nowhere in 0.1.0, deleted, the project
   finished, then restored — is refused, names the request, writes
   nothing, and the trash still holds the entry byte for byte; so that
   request does not run. A language-2 source comes back as it is.
   Tests: P `lang20d_criterion_4_*`, one per case; D
   `lang20d_criterion_4` for the two messages in the window.
5. A rename and a move of a listed source: one commit; the record's
   entry has the new path at its sorted place; the project loads.
   Test: P `lang20d_criterion_5`.
6. In a language-1 project, with or without a record, a board edit in a
   source the record does not list is refused with *Update the project
   first.* and changes neither the document nor the file: adding a
   block, wiring, moving a card's step, deleting, an Inspector change —
   in the window and through a session's model edit. Moving a card and a
   text edit in Code work. The same edits work in a listed source and in
   a language-2 project. Tests: P `lang20d_criterion_6_*`, D
   `lang20d_criterion_6_*` (on `labels` in language 1).
7. After each of these, an open second session (an observing window)
   follows without a reopen (LANG-20a, criterion 7).
   Test: P `lang20d_criterion_7`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| The trash in every other project: entries without the note, restores, limits | `trash_tests` unchanged; criterion 2's entry without a note |
| LANG-15: a language-1 project's manifest must not change by saving, registering as is, publishing | criterion 1's negative cases |
| Board edits in language-2 projects and in listed sources | criterion 6's second half; the board suites unchanged |
| *Save as component* creates a source from a selection of an old scenario | criterion 6 refuses it in an unlisted source (it is a board edit); allowed in a listed one, where criterion 1 lists the new component |

## Not in scope

The record's format and a project's states (LANG-20a). Converting
(LANG-20b). The window's offer (LANG-21a).

## Technical appendix

`apiforge-storage/src/project/trash.rs` (the note in an entry; its
schema's version), `apiforge-project/src/session.rs` (create, the board
refusal where a model edit is installed), `files.rs` (rename, move,
delete, restore), `new_document.rs`, `language.rs`;
`apiforge-desktop/src/folder/save_component.rs` only for the refusal's
sentence.
