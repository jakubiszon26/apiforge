# REL-05: v0.2.0 — one release for the one-chain language

- Size: S · Owner review: 15 min · Risk: none of its own (it ships the
  `format` and `default` changes of its tickets)
- State: **Approved, ready for implementation** (the release
  candidate's pull request). Restated on 2026-11-21 for the state after
  the cut of 2026-11-20, and for the owner's answers of that day on
  [#414](https://github.com/jakubiszon26/apiforge/issues/414): a release
  candidate first, then `v0.2.0` after the packages were tried (1 = A);
  **the owner pushes the tags, no agent does** (2 = B); published as a
  pre-release, as the workflow does (3 = A); the notes are the list
  GitHub generates, with no text of ours and no change of the workflow
  (4 = B); DIAG-06b after the release, first in line (5 = A). The owner
  has no 0.1.0 project of their own: condition 2 records "none".
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
- The release notes are the list generated from the merged pull
  requests, as for v0.1.0. What changes in a file, and the one command
  `apiforge fix --folder <directory>`, are in `README.md`.
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
| 2 | The fix was applied to every working source of the repository, and to the owner's test projects, with the same order of steps before and after | **Done** for the repository (#488). The owner's projects: **none** (the owner, 2026-11-21) |
| 3 | The fix keeps every comment of a file it changes | **Done** (LANG-13 #488; LANG-20 #512, `lang20_criterion_9`) |
| 4 | A project of language 1 in 0.2.0 does not run and says why, naming the command; 0.1.0 refuses a project of 0.2.0 and says why | **Done** in tests (LANG-15 #486 `lang15_criterion_3`–`5`; LANG-13 raised the build to language 2; LANG-20 #512 `lang20_criterion_10`). **To do, once by hand**: the released 0.1.0 binary opens a project saved by the candidate; the message is noted in the pull request |
| 5 | No gesture on the board writes a file that does not pass `check` | **Done when #516 has merged**: the property tests `diag06a_criterion_10`, `diag06d_criterion_7`, `diag06e_criterion_8`, `diag06f_criterion_7`, and DIAG-06c's criterion 8 |
| 6 | Under `on_error continue` a chain goes on after an error and stops only at a missing value | **Done** (RUN-01 #487, `run01_criterion_1`–`8`) |
| 7 | The reference describes language 2 and nothing else; ADR 0003, 0005, 0010, 0011 and 0015 say what the tickets decided | **Done** — `@after` stands in the reference only under *Diagnostics and what to do* and in `README.md` only where it tells how a 0.1.0 project is converted. **To do here**: the check `grep -n "@after" docs/language/reference.md README.md` and a reading of the five ADRs, in the QA section |
| 8 | The regression smoke list passes on the release binary, on Linux and from the Windows artifacts of the candidate | **To do here** (Linux by the implementer; Windows from the candidate's run, or by the owner) |
| 9 | The workspace version is `0.2.0`; `README.md` and `docs/roadmap.md` name it and say what changes in a file | **To do here** |

No longer conditions, cut on 2026-11-20: anything about a project that
is partly converted, a record of converted files, an agent's `fix`, the
window's own update.

Known and accepted, said in `README.md` under the release: two layout tests are ignored for
tickets outside this release (LAY-36 #419, a routing that fails about
one run in five; LAY-39, two chips a pixel too wide, an owner's decision
pending); with every wire drawn, `shop-50` crosses more than before
(147 and 192, LAY-47 #506).

## The release candidate's pull request

1. `Cargo.toml` (`workspace.package.version = "0.2.0"`) and `Cargo.lock`.
   Test: `crates/apiforge-desktop/tests/version.rs` and the command
   line's `--version` test, which read the workspace's version.
2. `rel05_cli_and_mcp_agree_on_step_order` in
   `crates/apiforge-mcp/tests/acceptance.rs` (condition 1): for every
   working scenario of `fixtures/`, the simulated run through `apiforge
   run --json` and through the MCP tool `run` list the same steps in the
   same order.
3. `README.md` and `docs/roadmap.md`: the version; a short section *What
   changes from 0.1.0* — steps run in the order written and `@after` is
   gone; a scenario has a `start`; `apiforge fix --folder <directory>`
   converts a 0.1.0 project whole or says what stands in the way
   (`--check` to look first); 0.1.0 cannot open a converted project;
   under `on_error continue` only a missing value holds a step back —
   and the known limits above. No file under `docs/releases/`, no change
   of `.github/workflows/release.yml`.
4. The QA section of the pull request:
   - the table of conditions with the candidate's commit;
   - condition 4 by hand: the released 0.1.0 binary (from the v0.1.0
     release's assets) opens a project saved by the candidate; the
     message is quoted;
   - condition 7: `grep -n "@after" docs/language/reference.md
     README.md` and one line per ADR (0003, 0005, 0010, 0011, 0015);
   - condition 8: the smoke list on the release binary, on Linux by the
     implementer, and on Windows from the `qa-captures-windows-2022`
     artifacts of the pull request's run with the `ci:windows` label —
     what those artifacts cannot show (flows 3 to 8) is listed as *for
     the owner's run of the release candidate*, not as passed;
   - condition 2: "none".

The pull request is merged by the owner. Nothing in it creates or pushes
a tag.

## The tag and the publication — the owner's

After the pull request has merged, **the owner** pushes `v0.2.0-rc.1`
from `main`; the workflow builds the packages and publishes a
pre-release with the generated notes; the owner tries the packages,
Windows among them, and then pushes `v0.2.0`. No agent creates, pushes
or deletes a tag or a release, and none edits a published release. An
agent may be asked to read a failed run of the workflow and report.

For the owner, the two commands, on an up-to-date `main`:
`git tag v0.2.0-rc.1 && git push origin v0.2.0-rc.1`, and later the same
with `v0.2.0`.

## Not in scope

Support for several language versions, a migration framework, rollback
(the owner, 2026-11-16). Anything of the second half of LANG. Any change
of the release workflow. Tags and releases.

## Technical appendix

`Cargo.toml`, `Cargo.lock`, `README.md`, `docs/roadmap.md`,
`crates/apiforge-mcp/tests/acceptance.rs`. The branch `release/0.2.0`
was made from an older `main`: merge `main` into it first.
