# LANG-15: A project names its language version, and a build that cannot read it says so

- Size: M · Owner review: 20 min · Risk: `format` (the manifest)
- State: **Approved, ready for implementation.** The rules are
  [LANG-00, section 2](LANG-00.md#2-the-language-version-lang-15); the owner
  took decisions 2 and 3 on 2026-11-16 (`formatVersion: 2` with
  `languageVersion`; an older project opens, is checked and edited, and
  runs only after the fix) and approved LANG-00 on 2026-11-17. This file
  adds the criteria's tests, the regressions, the files and one technical
  decision about order ([below](#the-build-reads-language-1-until-lang-20)).
  The `format` flag is covered by those two decisions: this ticket writes
  nothing to a manifest that they do not name.
- Issue [#412](https://github.com/jakubiszon26/apiforge/issues/412) · LANG
  sprint, first half · branch `feat/lang-15-language-version`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A project says which `.afs` language its files are written in, and a build
that reads another one says so — in the window, on the command line and to
an agent — instead of misreading the files or calling them corrupt.

## What happens today

`crates/apiforge-storage/src/project/schema.rs`: the manifest is read
strictly (`deny_unknown_fields`), its `formatVersion` first, and anything
but `1` is `ProjectError::UnsupportedVersion` (*Unsupported folder format
version; existing data was preserved.*). Nothing names the language; the
reference's first line says "Reference version: 1".

## What the owner will see

- New, in a manifest of a project in language 2 or later:
  `"formatVersion": 2, "languageVersion": <n>`. A manifest of
  `formatVersion: 1` has no such field and is language 1; this build keeps
  writing it exactly so (see the next section).
- New: a project written in a **newer** language is not opened. The
  window's open error, the command line and the MCP server say *This
  project is written in language 3; this build reads language 2. Update
  API Forge.* (with the two numbers of the case). Nothing is written.
- New: a project written in an **older** language opens for reading and
  editing; `check` reports it for the project and names the command
  `apiforge fix --folder <directory>`; a run is refused with the same
  sentence. Seen only from LANG-20 on.
- Changed: the reference's first line reads `Language version: <n> · …`;
  `project_overview` names the build's and the project's language.
- Removed or hidden, migrated: nothing. No existing file is rewritten.

## Mock-up

Not applicable: no new element. The two messages use the paths that exist
— the error shown when a project cannot be opened, and *Cannot run this
scenario: …*. Whether the window also offers the fix itself is LANG-21's (#482)
question, with a mock-up.

## The build reads language 1 until LANG-20

Language 2 is the language of LANG-13 (#386). `main` cannot claim to read
it before it does, and it cannot refuse language 1 while every fixture is
written in it. So this ticket delivers the **mechanism** with the constant
`LANGUAGE_VERSION = 1`, and LANG-20 raises the constant to 2 together with
`apiforge fix` for a project. With the constant at 1 no manifest on disk
changes. The comparison takes both numbers as arguments, so the older and
the newer case are tested now; the tests that need a real older project
through the command line, the MCP server and the window are written here
and ignored with `#[ignore = "LANG-20: the build reads language 2 from
that ticket on"]`, which LANG-20 removes.

## Acceptance criteria

1. A manifest of `formatVersion: 2` is read with its `languageVersion`; a
   manifest of `formatVersion: 1` is read as language 1 and must not carry
   the field; `formatVersion: 2` without the field, `languageVersion: 0`
   and `formatVersion: 3` are refused with the messages of today
   (*Invalid folder project metadata*, *Unsupported folder format
   version*). Tests: `lang15_criterion_1_*` in
   `crates/apiforge-storage/tests/acceptance.rs`.
2. A manifest is written back in the format it was read in: saving,
   registering a document or publishing in a language-1 project leaves
   `formatVersion: 1` and no field; a new project of a build of language
   `n ≥ 2` is written as `formatVersion: 2, languageVersion: n`, of
   language 1 as today. Tests: `lang15_criterion_2_*` in
   `crates/apiforge-project/tests/acceptance.rs`, the second through the
   function that takes the build's language.
3. A project whose language is higher than the build's (`LANGUAGE_VERSION
   + 1` in the test's manifest): `check`, `print`, `apply`, `run`, every
   MCP tool that opens the project and the window refuse with the one
   sentence that names both versions; no file of the project changes
   (bytes compared before and after). Tests: `lang15_criterion_3_*` in
   `crates/apiforge-cli/tests/folder.rs`, `crates/apiforge-mcp/tests/acceptance.rs`
   and `crates/apiforge-desktop/src/folder/acceptance_tests/lang15.rs`.
4. A project whose language is lower than the build's: it opens; `check
   --folder` exits 1 and its first line is for `apiforge.project.json`,
   naming both versions and `apiforge fix --folder <directory>`; the MCP
   `check` carries the same problem; `run` (command line, MCP, window) is
   refused with that sentence and sends nothing. Editing and saving a
   source work. Tests: the pure comparison,
   `lang15_criterion_4_an_older_project_is_told_apart`, runs now; the three
   end-to-end tests `lang15_criterion_4_*` are written now and ignored for
   LANG-20.
5. The released 0.1.0 refuses a project of `formatVersion: 2` with
   *Unsupported folder format version*. Test:
   `lang15_criterion_5_the_first_release_refuses_format_2` asserts the
   manifest's bytes begin the way 0.1.0 inspects them (`formatVersion`
   readable on its own, value 2). QA step, once by hand with the released
   0.1.0 binary: open such a project, note the message in the pull request.
6. The reference's first line is `Language version: <n> · grammar of ADR
   0005 as of <date>`, `n` equal to `LANGUAGE_VERSION`; `project_overview`
   returns `language_version` (the build's) and `project_language_version`;
   `reference_version` keeps its name and reads the same number. Tests:
   `the_reference_opens_with_its_version_line`
   (`crates/apiforge-project/tests/reference.rs`, changed to read the
   constant), `the_first_line_declares_the_version`
   (`crates/apiforge-mcp/src/reference.rs`), `lang15_criterion_6_overview`
   in the MCP acceptance tests.

## Regression potential

| What can break | Covered by |
| --- | --- |
| Every existing project (all `formatVersion: 1`) must open, save and run exactly as before | the whole suite unchanged; criterion 2; smoke list 2, 6, 7 |
| The manifest's bytes for a language-1 project must not change (a changed manifest is a Git diff in every user project) | criterion 2 compares bytes; `crates/apiforge-storage/src/project/tests.rs` unchanged |
| The other versioned files (`diagram.json`, trash, transactions, recovery, the cache) keep their own `formatVersion: 1` | their tests unchanged; this ticket touches only `ProjectManifest` |
| Migration of a legacy workspace still writes a project that opens | `crates/apiforge-project/tests/migration.rs` unchanged; smoke list 9 |

## Not in scope

- Raising the build's language to 2, `apiforge fix` for a project, and the
  write that raises a manifest: LANG-20 (#481).
- The rules of language 2 and the fix of a file: LANG-13 (#386).
- A button or notice in the window that offers the fix: LANG-21 (#482).
- Reading or running several language versions in one build (cut by the
  owner, 2026-11-16).

## Technical appendix

- `apiforge-script` owns `pub const LANGUAGE_VERSION: u32 = 1;`
  (`lib.rs`). `apiforge-storage` only stores the number:
  `ProjectManifest.language_version: Option<u32>`, absent exactly when
  `format_version == 1`; `ProjectManifest::language()` gives 1 for absent.
  Storage does not depend on the script crate and does not compare.
- `apiforge-project` compares when a session opens
  (`session.rs`, every `open*`): newer → a new `Error` variant whose text
  is the sentence above, so the command line, the MCP server (`agent.rs`
  maps it to a refusal) and the window show it without a text of their
  own. Older → the session opens and keeps the fact; `check` and the run
  entry points (`execution.rs`) read it. A pure function
  `language::support(project, build) -> Same | Older | Newer` holds the
  rule and takes both numbers.
- A new manifest (`prepare.rs`, `migration.rs`, `files.rs`) is written
  for the build's language through one constructor; today's literal
  `format_version: 1` in those files goes through it.
- `docs/decisions/0010-folder-projects.md`: the manifest table gains the
  two fields and the rule "a build reads one language version".
- `docs/language/reference.md`: **line 1 only** (the prefix). LANG-13
  leaves line 1 alone; LANG-20 sets the number and the date.
- Files: `crates/apiforge-storage/src/project/schema.rs`, `mod.rs`;
  `crates/apiforge-script/src/lib.rs` (the constant only);
  `crates/apiforge-project/src/{session,execution,prepare,migration,files,lib}.rs`
  and a new `language.rs`; `crates/apiforge-cli/src/{folder,run}.rs`;
  `crates/apiforge-mcp/src/{tools,agent,reference}.rs`;
  `crates/apiforge-desktop/src/folder/{opening,runs}.rs` only if the
  existing error paths need a line to carry the sentence.
