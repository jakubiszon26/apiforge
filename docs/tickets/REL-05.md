# REL-05: v0.2.0 — one release for the one-chain language

- Size: S · Owner review: 15 min · Risk: none of its own (it ships the
  `format` and `default` changes of its tickets)
- State: **Restated on 2026-11-21 for the state after the cut of
  2026-11-20; waits for the owner's answers on the tag, the publication
  and the notes** ([#414](https://github.com/jakubiszon26/apiforge/issues/414)).
  It has two parts. **The release candidate's pull request** — the
  version, the notes, one test, the checklist — can be taken by an
  implementing model once those answers are in. **The tag and the
  publication** are not: a release leaves the repository, so the tag is
  pushed only on the owner's word on this issue, by the owner or by the
  coordinating session, as for v0.1.0 (REL-01, decision 4).
- Branch `release/0.2.0`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

One release carries the one-chain language whole: a tester who updates
gets the new rules, the command that converts their project, a board
that keeps the chain, and a build that says so when a project is in the
other language.

## What it carries

Merged before the sprint's first half: RUN-03 (#389), LANG-14 (#390),
DIAG-07 (#392), LAY-34 (#396). Merged in it: FIX-01 (#485), RUN-01
(#487), LANG-15 (#486), LANG-13 (#488), LANG-20e (#504), LANG-05 (#505),
DIAG-06g (#507), LANG-06 with KTLO-37 (#508), LAY-45 (#509), DIAG-06e
(#510), LANG-20 (#512), DIAG-06a (#513), DIAG-06d (#514), DIAG-06f
(#515). Last to merge: DIAG-06c (#516). No release was cut between them.

Not in this release, by the owner's decisions of 2026-11-20: converting
a project in parts, the MCP tool `fix`, the window's *Update the
project* (LANG-20b, 20c, 20d, 20f, LANG-21a, LANG-21b — backlog, only if
users appear); DIAG-06b (#491, feedback while carrying — ready, not a
condition); bands (LAY-13); the corridor for data lines (LAY-47).

## What the owner will see

- New: the release `v0.2.0` with the assets of v0.1.0's shape; the
  version `0.2.0` in `--version` and in the window.
- New: release notes that open with what changes in a file and the one
  command, `apiforge fix --folder <directory>`.
- Default behaviour that changes, migrated: only what its tickets decided.

## Mock-up

Not applicable.

## Release conditions

State on `main` at `7c1c739` (2026-11-21), each checked again on the
release candidate's commit and recorded in the pull request's QA
section.

| # | Condition | State |
| --- | --- | --- |
| 1 | Every working source of the repository — `fixtures/scripts`, `fixtures/scenarios`, the `afs` examples of `README.md` and `docs/language/`; not published revisions, not the language-1 texts kept as test data, not `docs/qa/` — passes `check`, prints unchanged, and its simulated run lists its steps in the same order through the command line and the MCP server | **Done** for check, print and order (LANG-13 #488, `lang13_criterion_11_*`). **To do here**: the comparison of the command line with the MCP server, `rel05_cli_and_mcp_agree_on_step_order` in `crates/apiforge-mcp/tests/acceptance.rs`, over every fixture |
| 2 | The fix was applied to every working source of the repository, and to the owner's test projects, with the same order of steps before and after | **Done** for the repository (#488). **To do, the owner's**: `apiforge fix --folder` (or `--check` first) on each of their 0.1.0 projects with the candidate, then a simulated run; only "done, n projects" is recorded on the issue — nothing of those projects enters the repository |
| 3 | The fix keeps every comment of a file it changes | **Done** (LANG-13 #488; LANG-20 #512, `lang20_criterion_9`) |
| 4 | A project of language 1 in 0.2.0 does not run and says why, naming the command; 0.1.0 refuses a project of 0.2.0 and says why | **Done** in tests (LANG-15 #486 `lang15_criterion_3`–`5`; LANG-13 raised the build to language 2; LANG-20 #512 `lang20_criterion_10`). **To do, once by hand**: the released 0.1.0 binary opens a project saved by the candidate; the message is noted in the pull request |
| 5 | No gesture on the board writes a file that does not pass `check` | **Done when #516 has merged**: the property tests `diag06a_criterion_10`, `diag06d_criterion_7`, `diag06e_criterion_8`, `diag06f_criterion_7`, and DIAG-06c's criterion 8 |
| 6 | Under `on_error continue` a chain goes on after an error and stops only at a missing value | **Done** (RUN-01 #487, `run01_criterion_1`–`8`) |
| 7 | The reference describes language 2 and nothing else; ADR 0003, 0005, 0010, 0011 and 0015 say what the tickets decided | **Done** — `@after` stands in the reference only under *Diagnostics and what to do* and in `README.md` only where it tells how a 0.1.0 project is converted. **To do here**: the check `grep -n "@after" docs/language/reference.md README.md` and a reading of the five ADRs, in the QA section |
| 8 | The regression smoke list passes on the release binary, on Linux and from the Windows artifacts of the candidate | **To do here** (Linux by the implementer; Windows from the candidate's run, or by the owner) |
| 9 | The workspace version is `0.2.0`; `README.md` and `docs/roadmap.md` name it; the release notes are written | **To do here** |

No longer conditions, cut on 2026-11-20: anything about a project that
is partly converted, a record of converted files, an agent's `fix`, the
window's own update.

Known and accepted, said in the notes: two layout tests are ignored for
tickets outside this release (LAY-36 #419, a routing that fails about
one run in five; LAY-39, two chips a pixel too wide, an owner's decision
pending); with every wire drawn, `shop-50` crosses more than before
(147 and 192, LAY-47 #506).

## The release candidate's pull request

1. `Cargo.toml` (`workspace.package.version = "0.2.0"`), `Cargo.lock`;
   `README.md` and `docs/roadmap.md`. Test: `crates/apiforge-desktop/tests/version.rs`
   and the command line's `--version` test read the workspace's version.
2. `rel05_cli_and_mcp_agree_on_step_order` (condition 1).
3. The release notes as a file, `docs/releases/v0.2.0.md`, which the
   release workflow puts **before** the list it generates from the merged
   pull requests (one step of `.github/workflows/release.yml`: when
   `docs/releases/<tag>.md` exists, its text comes first; a release
   candidate `v0.2.0-rc.N` uses the file of `v0.2.0`). Checked by
   `packaging/test_release.py`. The file's sections, in this order:
   - **What changes in your files** — steps run in the order written;
     `@after` is gone; a scenario has a `start`; one command converts a
     0.1.0 project, `apiforge fix --folder <directory>` (`--check` to
     look first), and what it refuses; 0.1.0 cannot open a converted
     project.
   - **What changes when a run fails** — under `on_error continue` only
     a missing value holds a step back.
   - **On the board** — the chain is drawn side to side; a wire to a
     step moves it, also with a condition's steps and across branches;
     delete, paste and duplicate keep the chain; Undo gives the file
     back exactly.
   - **In the text** — Format keeps every comment; a compound check
     stays as typed.
   - **Not in this release** and **Known limits** — as above.
4. The QA section: the table of conditions with the candidate's commit,
   the smoke list's line, the two checks done by hand.

The pull request is merged by the owner. Nothing in it pushes a tag.

## The tag and the publication

After the pull request has merged and on the owner's word on #414:
`v0.2.0-rc.1` first; the workflow builds the packages and publishes a
pre-release; the owner tries it (Windows among them); then `v0.2.0` on
the same or a later commit of `main`. The workflow refuses a tag that is
not on `main`. Which of this the owner wants is asked on the issue.

## Not in scope

Support for several language versions, a migration framework, rollback
(the owner, 2026-11-16). Anything of the second half of LANG. Changing
what the release workflow builds or signs.

## Technical appendix

`Cargo.toml`, `Cargo.lock`, `README.md`, `docs/roadmap.md`,
`docs/releases/v0.2.0.md` (new), `.github/workflows/release.yml` (the
notes step only), `packaging/test_release.py`,
`crates/apiforge-mcp/tests/acceptance.rs`.
