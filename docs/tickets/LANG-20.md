# LANG-20: `apiforge fix` updates a whole project, and the build reads language 2

- Size: M · Owner review: 15 min · Risk: `format` (the manifest's version is
  raised; every source of a project is rewritten by the fix)
- State: **Approved rules, ready for implementation after LANG-13 (#386)
  and LANG-15 (#412) have merged.** Split from LANG-13 on 2026-11-20 to
  keep both `M`. Everything here is decided in [LANG-00](LANG-00.md):
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

One command brings a project written for 0.1.0 to language 2, and from
this ticket on the build reads language 2: a fixed project runs, a project
not yet fixed says so and names the command.

## What the owner will see

- New: `apiforge fix --folder <directory>` fixes every scenario and
  component source of the project with the fix of LANG-13, prints for each
  file what it removed or could not place, and — when every source passes
  the check — writes `"formatVersion": 2, "languageVersion": 2` to the
  manifest as its last write.
- New: the MCP tool `fix` does the same for an agent, under the rules of
  `write_source` (it takes the project for the agent session; an open
  window with unsaved work refuses).
- Changed: this build reads language 2. A project still in language 1
  opens and can be edited; `check` names the command; nothing runs until
  the fix has passed (LANG-15's messages, live from here).
- Changed: a new project, the demo project and a migrated legacy workspace
  are written as language 2. The reference's first line says 2.
- Migrated or converted: only by the person's command. Published revisions
  are not touched.
- Removed or hidden: nothing.

## Mock-up

Not applicable: a command and its text output. The window's own way to
start the fix is LANG-21 (#482).

## Acceptance criteria

Tests in `crates/apiforge-cli/tests/fix.rs` (C),
`crates/apiforge-project/tests/lang20.rs` (P) and
`crates/apiforge-mcp/tests/acceptance.rs` (M). The language-1 project of
the tests is a copy of `fixtures/scenarios/lay/rooms` as it is on `main`
before LANG-13, kept under `crates/apiforge-cli/tests/fixtures/rooms-language-1/`.

1. On a language-1 project whose files are written in run order: every
   source is fixed, the manifest is raised last, the exit code is 0, each
   changed file is listed with its count of removed lines; `check --folder`
   then passes and `run` runs. Test: C `lang20_criterion_1_*`.
2. When one source cannot be fixed: that file is byte-identical, the
   others are fixed, the manifest is unchanged, the exit code is 1 and the
   report names file, line and statement. After the person's edit a second
   run finishes the project. Test: C `lang20_criterion_2_*`.
3. A file is written whole or not at all: with a write that fails halfway
   (the store's fault injection), the file on disk is the old or the new
   text and the manifest is not raised. Test: P `lang20_criterion_3_*`.
4. Run on a project already in language 2: nothing changes, exit code 0,
   one line says so. Test: C `lang20_criterion_4_*`.
5. No comment is lost: for every source, the comments of the fixed text
   equal those before. Test: C `lang20_criterion_5_*` on a source with
   comments above, beside and between statements.
6. `LANGUAGE_VERSION` is 2; the tests of LANG-15 ignored for this ticket
   run and pass, the `#[ignore]` lines are gone. Test: those tests.
7. The MCP `fix` gives the same result and the same report as the command
   on the same project; a project held by a window with unsaved work is
   refused as `write_source` is. Test: M `lang20_criterion_7_*`.
8. Every folder project of the repository
   (`fixtures/scenarios/lay/rooms`) carries the raised manifest, written by
   the command; a new project and a migrated `fixtures/workspace.json`
   carry it too; the reference's first line reads `Language version: 2 · …`
   with the date of this ticket. Tests: P `lang20_criterion_8_*`,
   `the_reference_opens_with_its_version_line`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| Every test project of the suite is created as language 2 from here on; a test that compares a manifest's bytes sees the two new fields | the whole suite; such expectations are updated and listed in the pull request |
| The fix writes user files: a comment or a line lost | criteria 2, 3 and 5; LANG-13's `fix_source` tests |
| The smoke list: start, open, run, save, migrate | smoke list 1, 2, 6, 7, 9 on the release binary |
| A project open in the window while the command runs | criterion 7's refusal; the holder tests of MCP-07 unchanged |

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
- `apiforge-script/src/lib.rs`: `LANGUAGE_VERSION = 2`.
- `docs/language/reference.md` line 1; `README.md` and
  `docs/folder-projects.md` name the command; ADR 0010 gains the rule that
  the fix raises the version.
- The owner's own projects are fixed by the owner with the released
  command (REL-05, condition 2); none of their content enters the
  repository.
