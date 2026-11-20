# LANG-20: `apiforge fix` updates a project file by file, and a record says which files are done

- Size: M · Owner review: 20 min · Risk: `format` (a new file in a
  project; the manifest's two version fields; sources rewritten)
- State: **Decided; waits for the independent review before it is
  labelled.** Redesigned on 2026-11-20 for the owner's decision of that
  day (B, below), and for the answer to question 1 on
  [#481](https://github.com/jakubiszon26/apiforge/issues/481) (A: a board
  edit in a source not yet converted is refused). Then ready after
  LANG-13 (#386) has merged. Split from LANG-13 on 2026-11-20; the raised
  constant and the run refusal are LANG-13's.
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

This adds a file to what a project keeps and is recorded as the owner's
decision in `docs/tickets/LANG-00.md` (section 2, rule 4 and section 3)
and in ADR 0010 by this ticket's pull request. LANG-00's "the fix writes
each file whole or not at all and raises the version last" and "with an
error left the number stays and the report lists what the person places
by hand" hold as written.

## Goal

One command brings a project written for 0.1.0 to language 2, as far as
it can each time it is run, and never forgets or misjudges what it has
already done.

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

  One entry per line, sorted by path, so that two branches that converted
  different files merge. A source is identified by its `documentId` (the
  manifest's); the path is for the reader. An entry whose document is no
  longer in the manifest is dropped at the next run.
- **What it means.** While it exists, a source it lists is read in
  language 2, as written; a source it does not list is a language-1
  source: judged by its 0.1.0 reading when it is converted, shown with
  its errors and without a per-line fix (LANG-13, A5).
- **Created** by the first run that converts a source, in the same commit
  as that source. In that commit the manifest becomes `"formatVersion":
  2, "languageVersion": 1`: the released 0.1.0 then refuses the project
  (*Unsupported folder format version*) instead of running converted
  files with the old meaning, where their steps would be skipped without
  a word.
- **Updated** by every later run, by `--as-written`, and when this build
  creates a new source in such a project (a new source is written in
  language 2 and is listed at once; if the project has no record yet, the
  record and the manifest change above come with it).
- **Removed** in the commit that raises the manifest to
  `"languageVersion": 2`: when every source is listed or converted in
  that run **and** every source passes the check. A language-2 project
  has no record; a record found beside a language-2 manifest is ignored
  and deleted at the next write.
- **Another target.** A record whose `toLanguage` is not this build's
  language stops the update with a sentence naming the version that
  began it; nothing is read through it.

## What the owner will see

- New: `apiforge fix --folder <directory>` converts every source it can,
  lists per file the statements the person places by hand, and writes the
  record; when nothing is left it raises the manifest and removes the
  record. Exit code 0 only when the project is in language 2 afterwards.
- New: `--check` prints the same report and never writes; `--as-written
  <relative.afs>` (repeatable) lists a source the person converted by
  hand, after it passes the language-2 check.
- New: the MCP tool `fix` does the same, under the rules of
  `write_source`.
- While a project is being updated (a record exists): nothing runs — the
  refusal says *This project is being updated to language 2: 3 of 5
  files are done. Finish the update to run it.*; `check --folder` gives
  that as the project's problem and each source's problems in its own
  language; `project_overview` returns the two versions, `updating: true`
  and the files left. In a converted source the `@after` and `start`
  errors carry their fix again (it is a language-2 source).
- New (the owner's answer of 2026-11-20, question 1 = A): in a project
  of the older language, a source that is not yet converted cannot be
  edited on the board. The board shows it; an edit there — adding,
  wiring, moving, deleting, the Inspector's fields — is refused with
  *Update the project first.* Its text can be edited in Code. This holds
  with and without a record, from this ticket on.
- Migrated or converted: only by the person's command. Published
  revisions are not touched.
- Removed or hidden: nothing.

## Mock-up

Not applicable: a command and its text output. The window is LANG-21a
(#482), whose dialog changes with this decision (question 2 on #481).

## One run

1. **Plan.** Read the manifest, the record and every source (for the
   window, the text that would be saved). Each source is one of:
   *done* (listed; language 2), *to convert* (not listed; its 0.1.0
   reading and the order written agree, also when it holds no `@after`:
   then it is converted unchanged and only listed), *blocked* (not
   listed; a step runs nowhere today, or runs in another place than it is
   written — named, with what to do). The plan holds what would be
   written and the fingerprint of every file read; nothing is written.
2. **Commit.** The sources to convert, the record and — on the first
   run or the last — the manifest are written in one commit of the
   project store, which refuses when a fingerprint no longer matches
   (`ProjectStore::commit`, as every write of a session). An interrupted
   commit is rolled forward by the store's recovery at the next writable
   open, as any commit (ADR 0011); a foreign write in between stops
   recovery with its conflict, as today. Because the record is written in
   the same commit as the sources it lists, no source is ever converted
   and unlisted.
3. A *done* source that no longer passes the check (edited by hand
   since, or brought back by Git) is not converted again: it is a
   language-2 source with errors, reported as such, and it keeps the
   manifest from being raised.

## Acceptance criteria

Tests in `crates/apiforge-cli/tests/fix.rs` (C),
`crates/apiforge-project/tests/lang20.rs` (P),
`crates/apiforge-storage/tests/acceptance.rs` (T) and
`crates/apiforge-mcp/tests/acceptance.rs` (M). The language-1 project is
a copy of `fixtures/scenarios/lay/rooms` as it was before LANG-13, under
`crates/apiforge-cli/tests/fixtures/rooms-language-1/`; the blocked
variant has one statement moved above the step it follows and one request
nothing orders.

1. All sources convertible: one run converts them, raises the manifest to
   language 2 and leaves no record; exit code 0; `check` passes and `run`
   runs. Test: C `lang20_criterion_1_*`.
2. One source blocked: the others are converted and listed in the record,
   the blocked one is byte-identical, the manifest is `formatVersion: 2,
   languageVersion: 1`, the exit code is 1, the report names file, line,
   statement and what to do. Test: C `lang20_criterion_2_*`.
3. A second run after the person's edit: the done sources are not read by
   their 0.1.0 reading and not rewritten (bytes equal), the edited one is
   converted, the manifest is raised and the record removed.
   Test: C `lang20_criterion_3_*`.
4. A language-1 source without any `@after` whose request ran nowhere in
   0.1.0 is blocked and names the request, on the first run and — while
   it is unlisted — on every later one; with `--as-written` it is listed
   and the request then runs where it is written.
   Test: C `lang20_criterion_4_*`.
5. The record: the bytes are the format above, sorted, one entry per
   line; two records that list different sources merge by Git's text
   merge without a conflict (the test merges two files as text); an entry
   without a document is dropped; a record with another `toLanguage`
   stops the run. Tests: T `lang20_criterion_5_*`.
6. While the record exists: `run` (command line, MCP, window) is refused
   with the sentence above; `check` reports the project's problem and
   each source in its language; a converted source's `@after` error
   carries its fix, an unconverted one's does not; a source this build
   creates is listed in the same commit; the released 0.1.0 would refuse
   the manifest (as LANG-15's criterion 5 tests it).
   Tests: P `lang20_criterion_6_*`, M `lang20_criterion_6_overview`.
7. An interrupted commit (the store's fault injection): after the next
   writable open the sources and the record agree — every converted
   source is listed; with a stale plan nothing is written and the run
   says so. Tests: P `lang20_criterion_7_*`.
8. `--check` prints the report and writes nothing; its exit code is the
   run's. Test: C `lang20_criterion_8_*`.
9. A project already in language 2: nothing changes, exit code 0, one
   line. A stray record beside it is deleted at the next write.
   Test: C `lang20_criterion_9_*`.
10. No comment is lost in any converted source.
    Test: C `lang20_criterion_10_*`.
11. In a language-1 project, with or without a record, a board edit in a
    source the record does not list is refused with *Update the project
    first.* and changes neither the document nor the file: adding a block,
    wiring, moving a card's step, deleting, and a change in the Inspector,
    in the window and through a session's model edit; a card's position
    may still be moved (nothing of the source is written). The same edits
    work in a listed source and in a language-2 project, and a text edit
    of the unlisted source in Code is saved. Tests: P
    `lang20_criterion_11_*`,
    `crates/apiforge-desktop/src/folder/acceptance_tests/lang20.rs`
    `lang20_criterion_11_*` (on a language-1 copy of `labels`).
12. The MCP `fix` gives the same result and report as the command, also
    with `check: true`; a project held by a window with unsaved work is
    refused as `write_source` is. Test: M `lang20_criterion_12_*`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| A converted source judged again by its 0.1.0 reading (its steps then "run nowhere") | criterion 3 |
| An old source taken for a converted one (requests that never ran start to run) | criteria 4 and 6; the record is the only thing that makes a source language 2 |
| 0.1.0 opens a half-converted project and skips steps silently | criterion 6: the manifest is `formatVersion: 2` from the first converted source |
| The record and the sources disagree after a crash | criterion 7: one commit |
| LANG-15: `formatVersion: 2` with `languageVersion: 1` must open in this build as an older project | LANG-15's criterion 1 accepts it; criterion 6 here |
| The command writes a person's files | criteria 2, 3, 10 |
| An old source rewritten in the new language by an edit on the board, never judged | criterion 11 |
| Board edits in language-2 projects and in listed sources | criterion 11's second half; the board suites unchanged |
| Git: a branch switch or a revert brings back an old text of a listed source | "One run", 3: it shows its errors and blocks the raise; named in the report. A reverted source **without** `@after` cannot be told apart and is read as language 2 — the one case the record does not cover, said in the reference and in `docs/folder-projects.md` |

## Not in scope

- The window's offer and dialogs: LANG-21a (#482), LANG-21b (#495).
- What the fix of one file does or refuses: LANG-13.
- Reading language 1 for running; a record of anything but this update.

## Technical appendix

- `apiforge-storage/src/project/`: a new `language_update.rs` (the file's
  schema, strict like the manifest's, with its own `formatVersion`); the
  store lists it among the project's kept files.
- `apiforge-project/src/language.rs`: the plan and its commit; `session.rs`
  reads the record at open and when a source is created; `analysis.rs`
  gives a source its language from the record.
- The refusal of criterion 11 is in `apiforge-project/src/session.rs`,
  where a model edit is installed (one place for the window and for an
  agent); the window shows the session's sentence in its banner.
- `apiforge-cli` (`main.rs`, a new `fix.rs`), `apiforge-mcp` (`tools.rs`,
  `server.rs`): they call the two functions.
- `docs/tickets/LANG-00.md` (the decision recorded), ADR 0010 (the file
  and its lifecycle), `README.md`, `docs/folder-projects.md`.
- This ticket does not touch `apiforge-script`.
