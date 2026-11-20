# LANG-20: `apiforge fix` updates a whole project

- Size: M · Owner review: 15 min · Risk: `format` (the manifest's version is
  raised; every source of a project is rewritten by the fix)
- State: **Approved rules, ready for implementation after LANG-13 (#386)
  has merged.** Split from LANG-13 on 2026-11-20 to keep both `M`.
  **Amended the same day** after an independent review: the conversion is
  whole or not at all ([below](#one-conversion-whole-or-not-at-all)), and
  the raised constant and the run refusal moved into LANG-13, so that
  `main` never runs a language-1 project with the new meaning. Everything here is decided in [LANG-00](LANG-00.md):
  section 2, rules 3–5 (the fix raises the number as its last write, when
  every source passes; with an error left the number stays and the report
  lists what the person places by hand), section 3 (the window, the
  command line and the MCP server call the same function; a file is
  written whole or not at all; no transaction over the project, no undo
  beyond Git) and the LANG quiz's decision 5 on #388 (one command for a
  whole project).
- Issue [#481](https://github.com/jakubiszon26/apiforge/issues/481) · LANG sprint, first half · branch
  `feat/lang-20-project-fix`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

One command brings a project written for 0.1.0 to language 2. A project
is in one language or the other, never between.

## What the owner will see

- New: `apiforge fix --folder <directory>` reads every scenario and
  component source of a language-1 project, and — only when **every** one
  of them can be converted without changing what runs — writes them all
  and raises the manifest to `"formatVersion": 2, "languageVersion": 2`
  in one step. Otherwise it writes nothing and lists, per file, the
  statements the person places by hand.
- New: `--check` prints the same report and never writes.
- New: `--as-written <relative.afs>` (repeatable) takes a file as already
  written for language 2 — for a file the person converted by hand.
- New: the MCP tool `fix` does the same for an agent, under the rules of
  `write_source` (it takes the project for the agent session; an open
  window with unsaved work refuses).
- Migrated or converted: only by the person's command. Published
  revisions are not touched.
- Removed or hidden: nothing.

## Mock-up

Not applicable: a command and its text output. The window's own way to
start it is LANG-21 (#482).

## One conversion, whole or not at all

A converted file holds no `@after`; a language-1 file whose steps were
never connected holds none either, and in 0.1.0 those steps did not run.
Nothing in the text tells the two apart. So no state may exist in which
some files of a language-1 project are converted and others are not:

1. While the manifest says language 1, **every** source is judged by its
   0.1.0 reading, also one without `@after`: the steps it runs today, in
   their order, must be the steps written, in the order written. A step
   that runs nowhere today, or runs in another place than it is written,
   refuses the file and is named, with what to do (delete it, move it, or
   pass the file with `--as-written`).
2. All writes and the manifest go through one commit of the project
   store (`ProjectStore::commit`, the transaction of ADR 0011 that a new
   project is already written with). A crash leaves the project as it was
   or converted; the store's recovery finishes or undoes it at the next
   open. This is the store's existing commit, not a new mechanism: LANG-00
   cut building one, and "a file is written whole or not at all" holds
   for all of them together.
3. LANG-00's "with an error left the number stays and the report lists
   what the person places by hand" holds as written: with an error left,
   nothing is written.
4. In the window and for an agent, a language-1 project shows its errors
   without the per-line fix (LANG-13, A5): the project is converted by
   this command, by the MCP tool, or by LANG-21's button.

## Acceptance criteria

Tests in `crates/apiforge-cli/tests/fix.rs` (C),
`crates/apiforge-project/tests/lang20.rs` (P) and
`crates/apiforge-mcp/tests/acceptance.rs` (M). The language-1 project of
the tests is a copy of `fixtures/scenarios/lay/rooms` as it was on `main`
before LANG-13, kept under `crates/apiforge-cli/tests/fixtures/rooms-language-1/`.

1. On a language-1 project whose files are all convertible: every source
   is written, the manifest is raised, the exit code is 0, each changed
   file is listed with what was removed; `check --folder` then passes and
   `run` runs. Test: C `lang20_criterion_1_*`.
2. When one source cannot be converted: **no file and no manifest
   changes** (bytes compared), the exit code is 1 and the report names
   file, line, statement and what to do. After the person's edit the
   command converts the project. Test: C `lang20_criterion_2_*`.
3. A language-1 file without any `@after` whose request ran nowhere in
   0.1.0 refuses the project and names the request; the same project
   passes when the file is given with `--as-written`, and the request then
   runs where it is written. Test: C `lang20_criterion_3_*`.
4. A failure in the middle of the commit (the store's fault injection)
   leaves, after the next open, either every file and the manifest as
   before or all of them converted. Test: P `lang20_criterion_4_*`.
5. `--check` prints the report of criteria 1 and 2 and writes nothing;
   its exit code is the one the conversion would have.
   Test: C `lang20_criterion_5_*`.
6. On a project already in language 2: nothing changes, exit code 0, one
   line says so. Test: C `lang20_criterion_6_*`.
7. No comment is lost: for every source, the comments of the converted
   text equal those before. Test: C `lang20_criterion_7_*` on a source
   with comments above, beside and between statements.
8. The MCP `fix` gives the same result and the same report as the command
   on the same project, also with `check: true`; a project held by a
   window with unsaved work is refused as `write_source` is.
   Test: M `lang20_criterion_8_*`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| The command writes a person's files: a comment or a line lost, a step that starts to run | criteria 2, 3, 4 and 7; LANG-13's `fix_source` tests |
| The smoke list: start, open, run, save, migrate | smoke list 1, 2, 6, 7, 9 on the release binary |
| A project open in the window while the command runs | criterion 8's refusal; the holder tests of MCP-07 unchanged |

## Not in scope

- A button or notice in the window that starts the fix: LANG-21 (#482).
- Anything the fix of a file does or refuses: LANG-13.
- Rollback, a transaction over the project, reading language 1 for
  running (cut by the owner, 2026-11-16 and 2026-11-17).

## Technical appendix

- One function in `apiforge-project` (`language.rs`, beside LANG-15's
  comparison): fix every source through the session's store, then raise
  the manifest. `apiforge-cli` (`main.rs`, a new `fix.rs`) and
  `apiforge-mcp` (`tools.rs`, `server.rs`) only call it.
- The constant, the run refusal and the reference's first line are
  LANG-13's (its amendment A4); this ticket does not touch
  `apiforge-script`.
- `README.md` and `docs/folder-projects.md` name the command; ADR 0010
  gains the rule that the conversion raises the version and is whole.
- The owner's own projects are fixed by the owner with the released
  command (REL-05, condition 2); none of their content enters the
  repository.
