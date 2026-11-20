# REL-05: v0.2.0 — one release for the one-chain language

- Size: S · Owner review: 15 min · Risk: none of its own (it ships the
  `format` and `default` changes of its tickets)
- State: **Conditions decided (LANG quiz, decision 5, #388; LANG-00's cuts,
  2026-11-17); waits for its tickets.** Kept by the planning model: the
  tag is pushed by the coordinating session only after the owner agrees
  on this issue, as for v0.1.0 (REL-01, decision 4). Not a ticket an
  implementing model takes unattended.
- Issue [#414](https://github.com/jakubiszon26/apiforge/issues/414) ·
  branch `release/0.2.0`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

One release carries the one-chain language whole: a tester who updates
gets the new rules, the fix for their files, a board that keeps the chain
and a build that says so when a project is in the other language.

## What it carries

RUN-03 (#389), LANG-14 (#390), DIAG-07 (#392), LAY-34 (#396) — merged;
RUN-01 (#126), LANG-15 (#412), LANG-13 (#386), LANG-20 (#481), LAY-45
(#468), DIAG-06 (#391),
LANG-06 with KTLO-37 (#143, #211); LANG-05 (#142) if its decision is
taken in time, otherwise the next release. No release is cut between
them.

## What the owner will see

- New: the release `v0.2.0` with the assets of v0.1.0's shape; the
  version `0.2.0` in `--version` and in the window.
- New: release notes that say, before anything else, what changes in a
  file and the one command: `apiforge fix --folder <directory>`.
- Default behaviour that changes, migrated: only what its tickets decided.

## Mock-up

Not applicable.

## Release conditions

Each is checked on the release candidate's commit and recorded in the
pull request's QA section.

1. Every **working source** of the repository — `fixtures/scripts`,
   `fixtures/scenarios` and the `afs` examples of `README.md` and
   `docs/language/`; not the published revisions under
   `.apiforge/revisions/`, not the language-1 texts kept as test data
   (`tests/fixtures/lang1/`, `rooms-language-1/`), not the evidence under
   `docs/qa/` — passes `check`, prints unchanged, and
   its simulated run lists its steps in the same order through the command
   line and the MCP server. Test: LANG-13's `lang13_criterion_11_*`, and a
   new `rel05_cli_and_mcp_agree_on_step_order`
   (`crates/apiforge-mcp/tests/acceptance.rs`) over every fixture.
2. The fix was applied to every working source of the repository
   (LANG-13, criterion 11) and to the owner's test projects;
   for each the order of steps before and after is the same. QA step, the
   owner's: `apiforge fix --folder` on each of their projects with the
   release candidate, then a simulated run; only "done, n projects" is
   recorded here — nothing of those projects enters the repository.
3. The fix keeps every comment of a file it changes. Test: LANG-20's
   `lang20_criterion_7_*`.
4. A project of language 1 in 0.2.0 and a project of language 2 in 0.1.0
   are each refused with a message that says why. Tests: LANG-15's
   `lang15_criterion_3_*`, `_4_*`, `_5_*`. QA step: the released 0.1.0
   binary opens a project saved by the candidate; the message is noted.
5. No gesture on the board writes a file that does not pass `check`.
   Tests: DIAG-06's acceptance tests; until then LANG-13's
   `lang13_criterion_9_*`.
6. Under `on_error continue` a chain goes on after an error and stops only
   at a missing value. Tests: RUN-01's `run01_criterion_1`–`8`.
7. The reference describes language 2 and nothing else; ADR 0003, 0005,
   0010 and 0015 say what the tickets decided. Test: `reference.rs`; QA
   step: `grep -n "@after" docs/language/reference.md README.md` shows
   only the diagnostics table.
8. The regression smoke list passes on the release binary, on Linux and
   from the Windows artifacts of the candidate's run on `main`.
9. The workspace version is `0.2.0`; `docs/roadmap.md` and `README.md`
   name it; the release notes exist as the pull request's text and open
   with "What changes in your files".

## Not in scope

Support for several language versions, a migration framework, rollback
(the owner, 2026-11-16). Anything of the second half of LANG.

## Technical appendix

`Cargo.toml` (`workspace.package.version`), `Cargo.lock`, `README.md`,
`docs/roadmap.md`; the release workflow (`.github/workflows/release.yml`)
is unchanged and starts at the push of the tag `v0.2.0`. A release
candidate `v0.2.0-rc.1` first, as v0.1.0 had.
