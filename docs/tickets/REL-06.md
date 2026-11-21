# REL-06: v0.2.1 — the owner's findings on the 0.2.0 packages

- Size: S · Owner review: 15 min · Risk: none of its own (it ships the
  `default`, `removal` and `visible UI` changes of its tickets)
- State: **Approved, ready for implementation** (issue #524) · decided
  by the owner on 2026-11-21: **a release candidate first**,
  `v0.2.1-rc.1`, then `v0.2.1` after the owner has tried the packages ·
  last of 0.2.1, **after its six tickets have merged** · labelled
  `ready:opus`
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

One patch release carries what the owner found on the v0.2.0-rc.1
packages on 2026-11-21, and nothing else.

## What it carries

UX-41 (#519), UX-42 (#520), DIAG-09 (#399), DIAG-08 (#521), LANG-22
(#522), DEMO-01 (#523). Not the LANG sprint's second half, not DIAG-06b
(#491): what else has merged to `main` by then is listed in the pull
request and put to the owner, not assumed.

## What the owner will see

- New: the release `v0.2.1` with the assets of v0.2.0's shape; `0.2.1`
  in `--version` and in the window.
- As decided for REL-05 (#414) and unchanged unless the owner says so:
  the owner pushes the tags; published as a pre-release; the notes are
  the list GitHub generates.
- Decided (question 13): a candidate `v0.2.1-rc.1` first, as for 0.2.0;
  `v0.2.1` after the owner has tried its packages.
- Mock-up: not applicable.

## Release conditions

Checked on the release's commit and recorded in the pull request.

1. The six tickets are merged and their issues closed.
2. A project saved by v0.2.0 opens, checks and runs unchanged, with one
   named exception: a type name outside the six is now an error
   (LANG-22). The language version does not change.
3. Every working source of the repository passes `check` and prints
   unchanged (REL-05's condition 1), the new demo's files among them.
4. A first start in an empty data directory opens the new demo; a data
   directory of v0.2.0 opens its project untouched (DEMO-01, criteria 1
   and 6), once by hand on the release binary.
5. The regression smoke list passes on the release binary on Linux and
   from the Windows artifacts.
6. The workspace version is `0.2.1`; `README.md` names it and says in
   one paragraph what changed for a tester.

## Technical appendix

Branch `release/0.2.1`. `Cargo.toml` workspace version, `README.md`,
`docs/roadmap.md`, `docs/backlog.md` (the 0.2.1 section closes).
