# Handoff

One file, overwritten at each handoff. It says where the work stands when a
working day ends, so that the next session needs nothing but the
repository and the issues. State lives in the issues and pull requests;
this page points at them and orders the next day.

- Written: 2026-11-19, late evening local, by the planning model
  (Fable 5.2).
- `main`: `eb2dc01`. Read from `git` and `gh` at that moment.
- This file is on the branch `docs/req-sprint-plan`, in the pull request
  that closes LAY in the backlog.

## 1. Where the sprints stand

- **LAY is closed** ([#296](https://github.com/jakubiszon26/apiforge/issues/296), 2026-11-19). The owner's run of build
  `1d11f07` on a scenario of their own is the acceptance. Record:
  [LAY plan, 13.7](sprints/lay.md#137-closed-2026-11-19). **The
  retrospective is owed** and comes before LANG starts.
- **LANG is the current sprint**, not started: part A to 0.2.0
  ([plan](sprints/lang.md)). Three candidates wait for its planning:
  [#470](https://github.com/jakubiszon26/apiforge/issues/470), [#471](https://github.com/jakubiszon26/apiforge/issues/471), [#472](https://github.com/jakubiszon26/apiforge/issues/472).
- **REQ is the next sprint** ([plan](sprints/req.md), `sprint:req`),
  decided on first-round mock-ups; it follows 0.2.0.
- **LAY-2** is not scheduled: [#297](https://github.com/jakubiszon26/apiforge/issues/297), [#301](https://github.com/jakubiszon26/apiforge/issues/301), [#410](https://github.com/jakubiszon26/apiforge/issues/410), [#419](https://github.com/jakubiszon26/apiforge/issues/419),
  [#427](https://github.com/jakubiszon26/apiforge/issues/427), [#445](https://github.com/jakubiszon26/apiforge/issues/445), [#466](https://github.com/jakubiszon26/apiforge/issues/466) to [#469](https://github.com/jakubiszon26/apiforge/issues/469).

## 2. Open pull requests

One: this documentation pull request. No code is in review.

## 3. Ready now

Labelled `ready:opus`, approved and bounded; none is started. They belong
to REQ, do not depend on the language, and are taken when the owner opens
the sprint or pulls them forward:

1. FIX-01 [#454](https://github.com/jakubiszon26/apiforge/issues/454) — the reference scenario `labels`. First: the others'
   tests use it. Taken before LANG-13 it is added with its `@after` lines.
2. DIAG-02 [#145](https://github.com/jakubiszon26/apiforge/issues/145) and REP-08 [#455](https://github.com/jakubiszon26/apiforge/issues/455) — side by side (`canvas/` and
   `inspector/checks.rs`; `report.rs`).
3. REP-07 [#381](https://github.com/jakubiszon26/apiforge/issues/381) — after both.

Each shows a design-review capture at 960 × 540 and 1600 × 1000 in its
pull request for the owner's acceptance before merge.

## 4. Next, in order

1. The LAY retrospective ([process](process/retrospective.md)): the
   measures, the exit criteria ticked with their evidence in the backlog,
   three actions. What it should look at: the sprint was shortened and its
   acceptance replaced twice; the acceptance ran on a shape no reference
   board had; a decision taken one day and withdrawn the next (LAY-29's
   option A) left the defect the owner then met; "folded by default" was
   decided without measuring the board it leaves; first-round mock-ups
   against design-review captures.
2. LANG planning: the order of part A is in the plan; place or decline
   the three candidates; decide whether FIX-01 is taken before LANG-13.
3. REQ-00 [#456](https://github.com/jakubiszon26/apiforge/issues/456) once LANG-13 is merged: the captures on `mock/req-00`
   and the quiz; then REQ-01 to REQ-05 get their numbered criteria.

## 5. Waiting for the owner

- The acceptance of this pull request.
- At LANG planning: LANG-17, LANG-18, LANG-19.
- At REQ planning: REQ-07 [#464](https://github.com/jakubiszon26/apiforge/issues/464) and REQ-08 [#465](https://github.com/jakubiszon26/apiforge/issues/465); the five
  questions of REQ-05 [#462](https://github.com/jakubiszon26/apiforge/issues/462), the live permission of a single send among
  them; the questions listed on REQ-00.
- Whether the three ready tickets start now or with the sprint.

## 6. Open problems and risks

- `AGENTS.md` still names `sprint:mcp` in its example of listing a
  sprint's issues; the current sprint label is `sprint:lang`.
- The exit criteria of LAY in the backlog are unticked until the
  retrospective names their evidence.
- LAY-46 [#469](https://github.com/jakubiszon26/apiforge/issues/469) was seen at the Overview zoom only and is not yet
  reproduced at 100 %.
- #452 (Windows test speed) is open; the Windows job runs on `main` only.
- No automatic review until 2026-12-01: each pull request gets a written
  review before it merges.
- The mock-ups of 2026-11-19 are drawings; the Fit of 65–70 % on LAY-44's
  is an estimate, its criterion asks for 60 %.
