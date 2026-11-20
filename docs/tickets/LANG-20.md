# LANG-20: `apiforge fix --folder` converts a whole project to language 2, or changes nothing

- Size: M · Owner review: 20 min · Risk: `format` (every source of a
  project is rewritten by the fix; the manifest's version is raised)
- State: **Decided; labelled when one independent review has read it.**
  Amended on 2026-11-20 after the first such review (not ready as
  written): the judgement of a source without `@after`, the sentence on
  an `@after` line, the store's limits, and what the command opens and
  reads are now said exactly. It stays one ticket of size M: the
  additions are one function made public, one message and tests.
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
- Analysis model: Fable 5.2 · Implementation model: Opus 5.5

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
  names both versions and `apiforge fix --folder <directory>` (LANG-15,
  `apiforge-project/src/language.rs`). Its `@after` errors carry no
  per-line fix (LANG-13, `analysis.rs`) — but their text still says
  "delete this line", which this ticket changes.
- `fix_source` judges a file by its 0.1.0 reading **only when it holds an
  `@after` or lacks its `start`** (`lower.rs` calls `fix::run_order` only
  then); a file without `@after` is returned unchanged, also when a
  request in it ran nowhere in 0.1.0. The comparison itself,
  `fix::run_order`, is private to the script crate.
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
   `@after`, and in every part of it — the main flow, the cleanup, the
   body of a component: the steps it runs today, in their order, must be
   the steps written, in the order written, and the converted text must
   pass the language-2 check. `fix_source` alone does not do that (above),
   so `apiforge-script` gains one public function beside it — *judge a
   language-1 source* — that always makes the comparison and returns the
   converted text or the statements in the way; `fix_source` and the
   editor's behaviour do not change. A source is *convertible* (possibly
   unchanged), *blocked* (a step runs nowhere today, or runs in another
   place than it is written, or the result would not pass the check —
   named with the statement, the reason and what to do) or *unreadable*
   (missing, not text, over the source limit of 1 MiB, or it does not
   parse).
   **Which sources**: the documents of the manifest, the recovered ones
   among them; a `.afs` file the manifest does not list is not the
   project's and is not read. Each is read on its own, so that one that
   cannot be read is named, not the whole project refused without a
   name.
2. One source that is not convertible refuses the whole project.
3. The commit is one, through `commit_with_read_set`. Its **changes**
   are the sources whose text changes and the manifest, raised to
   `"formatVersion": 2, "languageVersion": 2` (also from `formatVersion:
   2, languageVersion: 1`, which the schema allows). Its **read set** is
   every document of the manifest and the manifest itself, as the command
   read them — also the sources it does not write. A file changed in
   between refuses the commit; nothing is written.
4. A project too large for one commit of the store (64 MiB, 1 024
   operations) is not converted in parts: when the store refuses the
   commit for its limits, nothing is written and the command says *This
   project is too large to convert in one step.* in place of the store's
   general sentence. The store is not changed and its limits are not made
   adjustable for this.
5. An interrupted commit is finished by the store's recovery at the next
   writable open, as any commit; the manifest is part of it, so the
   project is language 1 with every file as it was, or — once recovery
   has run — language 2 with every file converted. A project that
   already has an unfinished commit when the command starts is refused
   with the store's sentence for it, as every command is.
6. **What the command opens.** With `--check` it opens the project
   read-only: it takes no lock, writes nothing — not under
   `.apiforge/local/` either — and works while a window or an agent
   holds the project. Without it, it first reads the manifest read-only:
   a project of a newer language is refused with LANG-15's sentence
   before any lock is taken; then it opens the project as its writer, as
   `apply` does, and a project held by someone else is refused as there.
7. What to do by hand, as the report says it: for a step that runs
   nowhere today — delete it, or write `@after(<the step above it>)` on
   it, which the command then removes; for a step written out of its
   place — move its lines; for a file that does not read — repair it.
   A source this build wrote or edited on the board **before** the
   project was converted is judged like any other and may be blocked;
   the report says that restoring it from Git is the other way.

## Acceptance criteria

Tests in `crates/apiforge-cli/tests/fix.rs` (C),
`crates/apiforge-project/tests/lang20.rs` (P) and
`crates/apiforge-script/tests/lang20.rs`. The language-1 project is
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
3. A source **without any `@after`** is judged all the same: a scenario
   with a `start` and a request nothing orders blocks the project and
   names the request, and with `@after(<step above>)` written on it the
   project converts and the request runs there; a working component whose
   first request reads a request written below it is blocked and names
   both; a cleanup whose first step reads a step written below it is
   blocked. A working component written in the order it runs converts.
   Tests: `crates/apiforge-script/tests/lang20.rs` for the new function
   (one per case, and that `fix_source` still returns such files
   unchanged); C `lang20_criterion_3_*`.
4. An unreadable source — missing; a syntax error; over 1 MiB — refuses
   the project under its own heading and is named; the other sources are
   still judged and reported. A `.afs` file the manifest does not list is
   neither read nor changed. Test: C `lang20_criterion_4_*`.
5. Changed on disk between the command's reading and its commit: a
   source the command would write; a source it would **not** write; the
   manifest. Each time nothing is written and the command names the
   file. From `formatVersion: 2, languageVersion: 1` the manifest is
   raised like any other. Test: P `lang20_criterion_5_*` (the change made
   by the test between the two steps).
6. An interrupted commit (the store's fault injection): after the next
   writable open every source and the manifest are the converted ones;
   before it, opened read-only, the project is not treated as language 2
   unless its manifest says so. A project that has an unfinished commit
   before the command starts is refused and unchanged.
   Test: P `lang20_criterion_6_*`.
7. When the store refuses the commit for its limits, the command's
   sentence is the one of rule 4 and nothing is written. Test: P
   `lang20_criterion_7` on the function that turns the store's refusal
   into the report (the store's own tests prove that a refused commit
   writes nothing; no project of 64 MiB is built for this).
8. `--check` prints the report of criteria 1 and 2 and writes nothing —
   the project's whole tree, `.apiforge/local/` included, is
   byte-identical — and works while another process holds the project;
   its exit code is the run's. A project already in language 2: nothing
   changes, exit code 0, one line. A project of a newer language is
   refused with LANG-15's sentence and no lock file is created. Without
   `--check`, a project held by an open window or an agent session is
   refused as `apply` is. Test: C `lang20_criterion_8_*`, one per case.
9. No comment is lost: the comments of each converted source equal those
   before; published revisions are byte-identical.
   Test: C `lang20_criterion_9`.
10. In a language-1 project the error on an `@after` line reads *`@after`
    is not part of language 2. Update the project with apiforge fix
    --folder <directory>; do not delete the line by hand.* — with the
    project's folder, as the run refusal gives it — and carries no fix,
    on the command line (`check`), for an agent (`check`, `check_text`)
    and in the window's Problems. In a language-2 project it is LANG-13's
    text ("… delete this line") with its fix, unchanged. Test: P
    `lang20_criterion_10_*`; the LANG-15 tests of the run refusal's
    sentence unchanged.
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
| LANG-15's and LANG-13's sentences for a language-1 project | criterion 10; LANG-15's tests unchanged; a LANG-13 test that asserts "delete this line" in a language-1 project is restated and named in the pull request |
| `fix_source` and the editor's per-line fix in a language-2 project | `crates/apiforge-script/tests/lang13.rs` unchanged; criterion 3's negative case |

## Not in scope

- Converting part of a project; a record of converted files; accepting a
  file as written; anything in the window or for an agent beyond the
  sentences; the trash, renames and new files in a project not yet
  converted: LANG-20b, 20c, 20d, 20f, LANG-21a, LANG-21b — backlog, after
  0.2.0 and only if users appear.
- What the fix of one file does or refuses: LANG-13.

## Technical appendix

`apiforge-script/src/fix.rs` and `lib.rs` (the judgement of a language-1
source made public beside `fix_source`; nothing else in the crate);
`apiforge-project/src/language.rs` (read each document, judge it, build
the report, commit), `analysis.rs` and `session.rs` (criterion 10: the
sentence, and the project's folder reaching the analysis, which it does
not today); `apiforge-cli/src/main.rs` and a new `fix.rs`; `README.md`,
`docs/folder-projects.md`, ADR 0010, `docs/tickets/LANG-00.md`. This
ticket does not touch `apiforge-storage`, `apiforge-mcp` or
`apiforge-desktop` (their tests of criterion 10 read what the session
gives them).
