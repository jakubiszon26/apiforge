# LANG-20: `apiforge fix --folder` converts a whole project to language 2, or changes nothing

- Size: M · Owner review: 15 min · Risk: `format` (every source of a
  project is rewritten by the fix; the manifest's version is raised)
- State: **Decided; labelled when one independent review has read it.**
  The owner cut the scope on 2026-11-20
  ([#481](https://github.com/jakubiszon26/apiforge/issues/481)): before
  0.2.0 nobody uses the application, so one command that converts a
  project whole is enough. That replaces the decision of the same day to
  convert file by file with a record, and with it the record, the state
  "being updated", the window's offer, the MCP tool and everything about
  the trash: LANG-20b, 20c, 20d, 20f, LANG-21a and LANG-21b are in the
  backlog, to be taken after 0.2.0 only if users appear.
- Issue [#481](https://github.com/jakubiszon26/apiforge/issues/481) · LANG
  sprint, first half, a condition of 0.2.0 · branch
  `feat/lang-20-project-fix`
- Analysis model: Fable 5.2 · Implementation model: —

## Owner decision (2026-11-20)

**A project is converted whole, by one command, in one commit; or
nothing is written and the command says what to do by hand.** No record
of converted files, no state between the two languages, nothing in the
window beyond the sentences that name the command.

This ticket's pull request amends the two places of
`docs/tickets/LANG-00.md` that say otherwise, and only them: section 3
("no transaction over the project … cut") and *Confirmed at approval* 3
("cut: a … transactional migration") become "a project is converted in
one commit of the project store, whole or not at all; no state between
the two languages; no record, no window flow, no rollback (Git)".
Everything else of LANG-00 stands: the version is detected, the messages
name both versions, the fix keeps comments.

## Goal

A project written for 0.1.0 becomes a language-2 project by one command
that either does all of it or tells the person exactly what stands in
the way.

## What is already on `main`

- The fix of one file, with its own refusals: `fix_source`
  (`apiforge-script/src/fix.rs`, LANG-13); an example binary applies it
  to files.
- A language-1 project opens and can be edited; a run is refused, on the
  command line, for an agent and in the window, with a sentence that
  names both versions and `apiforge fix --folder <directory>`; its
  `@after` errors carry no per-line fix (LANG-15, LANG-13:
  `apiforge-project/src/language.rs`, `analysis.rs`).
- A commit of the project store that refuses when something it only read
  has changed: `ProjectStore::commit_with_read_set` (LANG-20e).

## What the owner will see

- New: `apiforge fix --folder <directory>`.
  - When **every** scenario and component source of a language-1 project
    can be converted without changing what runs: all of them and the
    manifest (`"formatVersion": 2, "languageVersion": 2`) are written
    together; the command lists each file with what was removed; the
    project then passes `check` and runs.
  - Otherwise **nothing is written**; the command lists, per file, each
    statement in the way with its line, the reason and what to do, and
    exits with an error.
- New: `--check` prints the same report and never writes.
- Changed: in a language-1 project the error on an `@after` line says
  what the run refusal says — run the command; do not delete the line by
  hand — if it does not already.
- Migrated or converted: only by the person's command. Published
  revisions are not touched.
- Removed or hidden: nothing.

## Mock-up

Not applicable: a command and its text output; the window and the agent
show sentences they show today.

## The rule

1. **Every source is judged by its 0.1.0 reading**, also one without any
   `@after`: the steps it runs today, in their order, must be the steps
   written, in the order written, and the converted text must pass the
   language-2 check. A source is *convertible* (possibly unchanged),
   *blocked* (a step runs nowhere today, or runs in another place than it
   is written, or the result would not pass the check — named with the
   statement, the reason and what to do) or *unreadable* (missing, not
   text, or it does not parse).
2. One source that is not convertible refuses the whole project.
3. The commit is one: the converted sources and the manifest, through
   `commit_with_read_set`, with a read set of **every** source and the
   manifest as the command read them. A file changed in between refuses
   the commit; nothing is written.
4. A project too large for one commit of the store (64 MiB, 1 024
   operations) is refused with a sentence that says so. It is not
   converted in parts.
5. An interrupted commit is finished by the store's recovery at the next
   writable open, as any commit; the manifest is part of it, so the
   project is language 1 with every file as it was, or — once recovery
   has run — language 2 with every file converted.
6. What to do by hand, as the report says it: for a step that runs
   nowhere today — delete it, or write `@after(<the step above it>)` on
   it, which the command then removes; for a step written out of its
   place — move its lines; for a file that does not read — repair it.
   A source this build wrote or edited on the board **before** the
   project was converted is judged like any other and may be blocked;
   the report says that restoring it from Git is the other way.

## Acceptance criteria

Tests in `crates/apiforge-cli/tests/fix.rs` (C) and
`crates/apiforge-project/tests/lang20.rs` (P). The language-1 project is
a copy of `fixtures/scenarios/lay/rooms` as it was before LANG-13, kept
under `crates/apiforge-cli/tests/fixtures/rooms-language-1/`; its
blocked variant has one statement moved above the step it follows and
one request nothing orders.

1. Every source convertible: one run writes them and the manifest; exit
   code 0; each changed file is listed with what was removed; `check
   --folder` passes and `run` runs; the simulated order of steps of each
   scenario equals the order recorded from 0.1.0's reading before.
   Test: C `lang20_criterion_1_*`.
2. One source blocked: **no file and no manifest changes** (bytes
   compared), exit code 1, the report names file, line, statement,
   reason and what to do, for every blocked statement of every file.
   After the person's edit the same command converts the project.
   Test: C `lang20_criterion_2_*`.
3. A source without any `@after` whose request ran nowhere in 0.1.0
   blocks the project and names the request; with `@after(<step above>)`
   written on it the project converts and the request runs there.
   Test: C `lang20_criterion_3`.
4. An unreadable source (missing; a syntax error) refuses the project
   under its own heading. Test: C `lang20_criterion_4_*`.
5. A source changed on disk between the command's reading and its
   commit: nothing is written and the command names the file. The same
   for the manifest. Test: P `lang20_criterion_5_*` (the change made by
   the test between the two steps).
6. An interrupted commit (the store's fault injection): after the next
   writable open every source and the manifest are the converted ones;
   before it, opened read-only, the project is not treated as language 2
   unless its manifest says so. Test: P `lang20_criterion_6`.
7. A project over the store's limits (the test lowers them) is refused
   and unchanged. Test: P `lang20_criterion_7`.
8. `--check` prints the report of criteria 1 and 2 and writes nothing;
   its exit code is the run's. A project already in language 2: nothing
   changes, exit code 0, one line. A project held by an open window or
   an agent session is refused as `apply` is.
   Test: C `lang20_criterion_8_*`.
9. No comment is lost: the comments of each converted source equal those
   before; published revisions are byte-identical.
   Test: C `lang20_criterion_9`.
10. In a language-1 project the error on an `@after` line, on the
    command line, for an agent (`check`, `check_text`) and in the
    window's Problems, names `apiforge fix --folder <directory>` and
    carries no fix. Test: P `lang20_criterion_10`; the LANG-15 tests
    that assert the run refusal's sentence unchanged.
11. `README.md` and `docs/folder-projects.md` name the command and say
    what it refuses; ADR 0010 says that a project is converted whole;
    `docs/tickets/LANG-00.md` is amended as above. QA step: read in the
    pull request.

## Regression potential

| What can break | Covered by |
| --- | --- |
| The command writes a person's files: a line or a comment lost, a step that starts to run | criteria 1, 2, 3 and 9; LANG-13's `fix_source` tests unchanged |
| A project left between the two languages | criteria 2, 5 and 6: one commit, with the manifest in it |
| Language-2 projects and every other command | criterion 8; the suites unchanged |
| LANG-15's and LANG-13's sentences for a language-1 project | criterion 10; their tests unchanged |

## Not in scope

- Converting part of a project; a record of converted files; accepting a
  file as written; anything in the window or for an agent beyond the
  sentences; the trash, renames and new files in a project not yet
  converted: LANG-20b, 20c, 20d, 20f, LANG-21a, LANG-21b — backlog, after
  0.2.0 and only if users appear.
- What the fix of one file does or refuses: LANG-13.

## Technical appendix

`apiforge-project/src/language.rs` (read every source, judge it with
`fix_source` and the language-2 check, build the report, commit);
`apiforge-cli/src/main.rs` and a new `fix.rs`; the analysis's message of
criterion 10 where it withholds the fix (`analysis.rs`), if it does not
name the command yet; `README.md`, `docs/folder-projects.md`, ADR 0010,
`docs/tickets/LANG-00.md`. This ticket does not touch `apiforge-script`,
`apiforge-storage`, `apiforge-mcp` or `apiforge-desktop`.
