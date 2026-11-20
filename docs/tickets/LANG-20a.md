# LANG-20a: A project being updated: the record of converted sources, and what a build does with it

- Size: M · Owner review: 20 min · Risk: `format` (a new file in a
  project; the manifest's two version fields)
- State: **Decided; waits for the independent review of this version
  before it is labelled.** The owner's decisions of 2026-11-20 on
  [#481](https://github.com/jakubiszon26/apiforge/issues/481): a project
  is converted file by file, with a record kept by Git (question 4 = B);
  a board edit in a source not yet converted is refused (question 1 = A).
  **Split the same day** after two independent reviews: this ticket is
  the record and what every part of the build does while it exists; the
  conversion itself is LANG-20b (#497); the MCP tool is LANG-20c (#498).
  **Narrowed again** after the review's second pass: creating, deleting,
  restoring and renaming a source, and the refusal of a board edit in an
  old source, are LANG-20d (#499). The build's language is already 2:
  LANG-13 raised the constant and enabled LANG-15's tests; nothing of
  that is left to this family.
  Then ready after LANG-13 (#386) has merged.
- Issue [#481](https://github.com/jakubiszon26/apiforge/issues/481) · LANG
  sprint, first half · branch `feat/lang-20-project-fix`
- Analysis model: Fable 5.2 · Implementation model: —

## Owner decision (2026-11-20, question 4 on #481: B)

**A project is converted file by file, with a record.** The files that
can be converted are written, the others are listed; a file in the
project, kept by Git, records which sources are already converted, so
that a second run tells them from old ones. Progress is kept between
runs; the project is in two languages for a while. Not chosen: whole or
not at all (the planning model's recommendation); file by file without a
record (unsafe).

It adds a file to what a project keeps. This ticket's pull request
records it in `docs/tickets/LANG-00.md` — in **both** places that say
otherwise: section 3 ("no transaction over the project, no undo beyond
the editor's and Git: cut") and *Confirmed at approval* 3 ("cut: a
reversible or transactional migration") get the amendment "a project is
updated file by file with a record; each step is one commit of the
project store" — and in ADR 0010.

## Goal

A project that is partly updated is a state every part of the build
understands: which sources are in the new language, that nothing runs
yet, and that an old source is not rewritten by accident.

## The record

- **Where and what.** `.apiforge/language-update.json`, beside
  `diagram.json`, tracked by Git like it:

  ```json
  {
    "formatVersion": 1,
    "toLanguage": 2,
    "converted": [
      { "documentId": "…", "path": "scenarios/booking-day.afs" }
    ]
  }
  ```

  One entry per line, sorted by path. A source is identified by its
  `documentId` (the manifest's); the path is for the reader. It is read
  strictly, like the manifest. An empty list is valid: every listed source
  may have been deleted while others are still to do.
- **Git.** The layout keeps a merge small; it does not make it free of
  conflicts (two branches that each add an entry after the same line
  conflict). A record that does not read — a merge left unresolved — is
  an error that names the file and what to do; it is never ignored and
  never guessed. Said in `docs/folder-projects.md`.
- **What it means.** A project is *being updated* when its manifest is
  `"formatVersion": 2, "languageVersion": 1` and the record exists. A
  source the record lists is read in language 2, as written; a source it
  does not list is a language-1 source. A language-1 manifest of
  `formatVersion: 1` has no record: every source is a language-1 source.
- **The manifest goes with it.** The record exists exactly while the
  manifest is `formatVersion: 2, languageVersion: 1`: the first write of
  a record raises the manifest to that in the same commit, so that the
  released 0.1.0 refuses the project (*Unsupported folder format
  version*) instead of running converted files with the old meaning,
  where their steps would be skipped without a word. A record beside any
  other manifest is an error naming both.
- **Who writes it.** LANG-20b's conversion, and LANG-20d when a source
  is created, deleted, restored or renamed. This ticket only reads it.
- **Another target.** A record whose `toLanguage` is not this build's
  language: the project opens for reading, nothing is read through the
  record, and the project's problem names the version that began the
  update.

## What the owner will see

- New: while a project is being updated, **nothing runs** — command
  line, agent and window refuse with *This project is being updated to
  language 2: 3 of 5 files are done. Finish the update to run it.*
- New: `check --folder` gives that as the project's problem and each
  source's problems in its own language; `project_overview` returns both
  versions, `updating` and the sources left. In a converted source the
  `@after` and `start` errors carry their fix (it is a language-2
  source); in one not converted they do not (LANG-13, A5).
- New: when the record changes on disk — by the command, by an agent, by
  Git — an open window follows: the count, which sources carry fixes and
  which can be edited on the board.
- Migrated or converted: nothing by this ticket. Removed: nothing.

## Mock-up

Not applicable: two sentences through the paths that exist (the run
refusal, the board's banner). The window's offer is LANG-21a.

## Acceptance criteria

Tests in `crates/apiforge-storage/tests/acceptance.rs` (T),
`crates/apiforge-project/tests/lang20a.rs` (P),
`crates/apiforge-cli/tests/folder.rs` (C),
`crates/apiforge-mcp/tests/acceptance.rs` (M) and
`crates/apiforge-desktop/src/folder/acceptance_tests/lang20a.rs` (D).
The projects are written by the tests: a language-1 copy of `rooms`
(kept under `crates/apiforge-project/tests/fixtures/rooms-language-1/`)
with a record that lists some of its sources, whose texts the test has
converted with LANG-13's `fix_source`.

1. The record is read and written in the format above, sorted, one entry
   per line; unknown fields, another `formatVersion`, a duplicate or
   unknown `documentId` and a text that is not this JSON (a merge's
   conflict markers) are each refused with a sentence naming the file.
   Tests: T `lang20a_criterion_1_*`.
2. The record and the manifest go together: a record beside
   `formatVersion: 1`, or beside `languageVersion: 2`, and a manifest of
   `formatVersion: 2, languageVersion: 1` without a record, are errors
   that name both files; the project opens read-only for them and
   nothing runs. Tests: P `lang20a_criterion_2_*`.
3. In a project being updated a listed source is lowered in language 2
   and its `@after` error carries its fix; an unlisted one shows its
   errors without a fix; `check --folder` exits 1 with the project's
   problem first, naming how many sources are done.
   Tests: P, C `lang20a_criterion_3_*`.
4. `run` is refused with the sentence above on the command line, through
   MCP and in the window, and sends nothing; `project_overview` returns
   `updating: true`, both versions and the sources left.
   Tests: C, M, D `lang20a_criterion_4_*`.
5. (Moved to LANG-20d.)
6. (Moved to LANG-20d.)
7. A change of the record alone on disk — no source and no manifest byte
   changes — reaches an open session: the sources' languages, their
   fixes, the board permission and the overview's count follow without a
   reopen. Tests: P `lang20a_criterion_7_*` (the record rewritten by the
   test; the session's observer of `external.rs`).
8. A record whose `toLanguage` is another language: the project opens
   for reading, no source is read through it, the problem names it.
   Test: P `lang20a_criterion_8`.
9. The released 0.1.0 refuses a project being updated (the manifest's
   bytes, as LANG-15's criterion 5 tests them). Test: T
   `lang20a_criterion_9`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| Every language-2 and every plain language-1 project: no record, nothing changes | the whole suite; criterion 2's negative cases |
| LANG-15: `formatVersion: 2` with `languageVersion: 1` | LANG-15's criterion 1 accepts it; criteria 2–4 here give it its meaning |
| The session's external-change detection (it watches the manifest, the documents, the diagram and the revisions today) | criterion 7; `external` tests unchanged |
| Git: a listed source brought back to an old text | with `@after` it shows its errors; **without** `@after` it cannot be told apart and is read as language 2 — the one case the record does not cover, said in `docs/folder-projects.md` |

## Not in scope

- Converting anything, `apiforge fix`, the plan and its commit: LANG-20b
  (#497). The MCP tool: LANG-20c (#498). The window's offer: LANG-21a.
- Reading language 1 for running.

## Technical appendix

- `apiforge-storage/src/project/`: a new `language_update.rs` (schema);
  the store lists it among the kept files.
- `apiforge-project/src/`: `language.rs` (a project's state: language 1,
  being updated, language 2, and a source's language), `session.rs` (the
  record read at open and with each external change), `analysis.rs`, `external.rs`, `execution.rs`.
- `apiforge-cli/src/{folder,run}.rs`, `apiforge-mcp/src/tools.rs` (the
  overview), `apiforge-desktop` only where the two sentences are shown.
- `docs/tickets/LANG-00.md`, ADR 0010, `docs/folder-projects.md`.
