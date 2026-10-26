# KTLO-14: CI has failed on every push to main since Stage II began

- Size: S · Owner review: 10 min · Risk: none
- State: Awaiting acceptance (issue #18)
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

A push to `main` is green on both runners, so that a red check means something
again.

## What the owner will see

- New: a green check on `main` for the CI workflow, on Linux and Windows.
- Changed: nothing in the application.
- Removed or hidden: nothing.
- Migrated or converted: nothing.
- Default behaviour that changes: none.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. The Linux job opens the native window: the release binary no longer panics
   with `Library libxkbcommon-x11.so could not be loaded`.
2. The Windows job passes `fixtures::tests::generated_fixtures_are_up_to_date`.
3. The workflow is green on both runners for the pull request, and again on
   `main` after the merge.

## Causes and evidence

Every run of `rust.yml` on `main` from `febcca9` through `b27ed77` failed on
both runners. Baseline evidence: run
[40237916475](https://github.com/jakubiszon26/apiforge/actions/runs/40237916475)
(pull request #2, documentation only).

1. **Linux**, step *Open native window and capture it*: winit, under X11, loads
   `libxkbcommon-x11.so.0` through `xkbcommon-dl` at runtime. The apt list
   installed `libxkbcommon-dev`, which pulls in `libxkbcommon0`
   (`libxkbcommon.so.0`) but not `libxkbcommon-x11-0`. Fix: add
   `libxkbcommon-x11-0` to the apt list.
2. **Windows**, step *Run workspace unit and integration tests*: the repository
   had no `.gitattributes`, so the runner (`core.autocrlf=true`) checked the
   JSON fixtures out with CRLF and the byte comparison with freshly generated
   LF output reported every file under `fixtures/projects` and `fixtures/runs`
   as stale. Fix: a root `.gitattributes` with `*.json text eol=lf` and
   `*.afs text eol=lf`. No tracked file had CRLF in the index, so nothing was
   renormalised. This file is the repository's own; it is unrelated to the
   per-project `.gitattributes` the application writes (ADR 0010/0011, B8).

3. **Both runners, intermittently**: desktop tests failed now and then
   (`renaming_a_dirty_text_tab_…` on ubuntu, `a_created_project_registers_…`
   on Windows). `ProjectStore::scan_tree` read the transaction head without
   the writer lock and answered `RecoveryRequired` when the window's
   once-a-second external-change check overlapped its own rename or save;
   the window then showed the interrupted-transaction screen. Reproduced
   locally under CPU load (3 of 450 runs, then 6 of 75 for the pair). Fix: the
   scan answers `Busy` while this owner's worker holds the lock
   (`a_scan_during_this_owners_own_commit_is_busy_not_an_interrupted_transaction`,
   committed failing first), and the test-only `is_settled()` also waits for
   analysis and the check. After the fix: 0 of 450 runs of the pair failed.

## Not in scope

- Making the fixture comparison line-ending-neutral: the attribute is enough.
- The re-run on `main` after the merge is the owner's step.

## Actual

Changed lines: 158 (workflow 10, `.gitattributes` 4, `docs/development.md` 5,
storage fix 9 and its tests 46, desktop test settling 7, this ticket 77).
Three rounds: the two causes above, the Windows probe fix (`bcdb106`), then
cause 3. Green on both runners on `0d88f05`: runs
[40277674373](https://github.com/jakubiszon26/apiforge/actions/runs/40277674373) (push) and
[40277679824](https://github.com/jakubiszon26/apiforge/actions/runs/40277679824) (pull request).
