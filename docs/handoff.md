# Handoff

One file, overwritten at each handoff. It says where the work stands when a
working day ends, so that the next session needs nothing but the
repository and the issues. State lives in the issues and pull requests;
this page points at them and orders the next day.

- Written: 2026-11-20, shortly after midnight local, by the planning model
  (Fable 5.2). It replaces the handoff of 2026-11-19, late evening.
- `main`: `eb2dc01`. Read from `git` and `gh` at that moment.
- This file is on the branch `docs/req-sprint-plan`, in the pull request
  that closes LAY in the backlog (#474).

## 1. Where the sprints stand

- **LAY is closed** ([#296](https://github.com/jakubiszon26/apiforge/issues/296), 2026-11-19). The owner's run of build
  `1d11f07` on a scenario of their own is the acceptance. Record:
  [LAY plan, 13.7](sprints/lay.md#137-closed-2026-11-19). The
  [retrospective](retrospectives/2026-11-20-lay.md) is written: three of
  five exit criteria met, one in two parts of three, one replaced; three
  actions.
- **LANG is the current sprint, started by the owner on 2026-11-20**: part
  A to 0.2.0 ([plan](sprints/lang.md)). FIX-01 [#454](https://github.com/jakubiszon26/apiforge/issues/454), RUN-01 [#126](https://github.com/jakubiszon26/apiforge/issues/126),
  LANG-15 [#412](https://github.com/jakubiszon26/apiforge/issues/412) and LANG-13 [#386](https://github.com/jakubiszon26/apiforge/issues/386) are being implemented on
  2026-11-20 and merge in that order; LANG-20 [#481](https://github.com/jakubiszon26/apiforge/issues/481) was split from
  LANG-13 and follows it. Tracks and shared files: [#411](https://github.com/jakubiszon26/apiforge/issues/411#issuecomment-7025097154).
- **REQ is the next sprint** ([plan](sprints/req.md), `sprint:req`),
  decided on first-round mock-ups; it follows 0.2.0.
- **LAY-2** is not scheduled: [#297](https://github.com/jakubiszon26/apiforge/issues/297), [#301](https://github.com/jakubiszon26/apiforge/issues/301), [#410](https://github.com/jakubiszon26/apiforge/issues/410), [#419](https://github.com/jakubiszon26/apiforge/issues/419),
  [#427](https://github.com/jakubiszon26/apiforge/issues/427), [#445](https://github.com/jakubiszon26/apiforge/issues/445), [#466](https://github.com/jakubiszon26/apiforge/issues/466) to [#469](https://github.com/jakubiszon26/apiforge/issues/469).
- **The planning model's eight proposals are accepted into scope**
  (2026-11-20): LANG-17 [#470](https://github.com/jakubiszon26/apiforge/issues/470), LANG-18 [#471](https://github.com/jakubiszon26/apiforge/issues/471), LANG-19 [#472](https://github.com/jakubiszon26/apiforge/issues/472) in
  LANG's second half; REQ-07 [#464](https://github.com/jakubiszon26/apiforge/issues/464), REQ-08 [#465](https://github.com/jakubiszon26/apiforge/issues/465), REQ-09 [#477](https://github.com/jakubiszon26/apiforge/issues/477),
  REQ-10 [#478](https://github.com/jakubiszon26/apiforge/issues/478) in REQ; GEN-01 [#473](https://github.com/jakubiszon26/apiforge/issues/473) in no sprint until LANG-17 has a
  form. In scope means to be analysed; no design is approved.

## 2. Open pull requests

Two, both documentation: #474 (this one) and #476 (KTLO-44 [#475](https://github.com/jakubiszon26/apiforge/issues/475), the
process change that an analysis carries proposals). No code is in review.

## 3. Ready now

**LANG**, the current sprint: the `ready:*` labels on the issues of part A
say what may be taken; `gh issue list --label sprint:lang` lists them.

**REQ**, four tickets labelled `ready:opus`, approved and bounded; FIX-01 is
in implementation, the others are not started. They do not depend on the language and are taken when the owner
opens the sprint or pulls them forward:

1. FIX-01 [#454](https://github.com/jakubiszon26/apiforge/issues/454) — the reference scenario `labels`. First: the others'
   tests use it. It merges before LANG-13, with its `@after` lines.
2. DIAG-02 [#145](https://github.com/jakubiszon26/apiforge/issues/145) and REP-08 [#455](https://github.com/jakubiszon26/apiforge/issues/455) — side by side (`canvas/` and
   `inspector/checks.rs`; `report.rs`).
3. REP-07 [#381](https://github.com/jakubiszon26/apiforge/issues/381) — after both.

Each is accepted on the captures of its pull request's QA section: from the
release binary of the branch, at 960 × 540, and at 1600 × 1000 where layout
matters ([testing](process/testing.md#2-the-functional-qa-report)). A
design-review capture is an aid, not that evidence.

## 4. Next, in order

1. LANG part A in the merge order: FIX-01 → RUN-01 → LANG-15 → LANG-13 →
   LANG-20, DIAG-06 and LANG-06 with KTLO-37 → LANG-05 → REL-05.
2. The retrospective's process actions: KTLO-44 (#476, open), KTLO-45
   [#479](https://github.com/jakubiszon26/apiforge/issues/479), KTLO-46 [#480](https://github.com/jakubiszon26/apiforge/issues/480).
3. LANG-16 [#413](https://github.com/jakubiszon26/apiforge/issues/413), the analysis of the second half, now with LANG-17,
   LANG-18 and LANG-19 in it.
4. REQ-00 [#456](https://github.com/jakubiszon26/apiforge/issues/456) once LANG-13 is merged: the captures on `mock/req-00`
   and the quiz; then REQ-01 to REQ-05 get their numbered criteria, and
   REQ-07 to REQ-10 their analyses.

## 5. Waiting for the owner

- The acceptance of #474 and #476.
- DIAG-06 [#391](https://github.com/jakubiszon26/apiforge/issues/391): the mock-up and twelve questions, with the proposed
  order LAY-45 → DIAG-06a → DIAG-06b → LAY-13.
- LANG-05 [#142](https://github.com/jakubiszon26/apiforge/issues/142): the canonical form of a check (A/B/C). LANG-21
  [#482](https://github.com/jakubiszon26/apiforge/issues/482): whether the window offers *Update the project* in 0.2.0.
- LANG-13: four consequences to confirm at its acceptance, listed on
  [#386](https://github.com/jakubiszon26/apiforge/issues/386).
- The work-in-progress limit: #474 and #476 await acceptance, and four
  implementation tracks open drafts tonight.
- Whether DIAG-02, REP-08 and REP-07 start beside LANG once FIX-01 is on
  `main`, or with the REQ sprint.
- The five questions of REQ-05 [#462](https://github.com/jakubiszon26/apiforge/issues/462), the live permission of a single
  send among them; the questions listed on REQ-00.
- LAY-39 [#435](https://github.com/jakubiszon26/apiforge/issues/435): two of its acceptance tests are still ignored as
  "owner decision pending" on a closed issue; the question is on the issue.
- KTLO-45 and KTLO-46: the wording of two process changes, when their pull
  requests come.

## 6. Open problems and risks

- LANG started before the retrospective's actions are in the process: part
  A runs under the rules as they are; KTLO-45 and KTLO-46 apply from the
  planning of the second half and of REQ.
- Two issues are titled KTLO-44: [#325](https://github.com/jakubiszon26/apiforge/issues/325) (closed, the acceptance modules)
  and [#475](https://github.com/jakubiszon26/apiforge/issues/475) (the proposals rule). One needs another number.
- `AGENTS.md` still names `sprint:mcp` in its example of listing a
  sprint's issues; the current sprint label is `sprint:lang`.
- LAY-46 [#469](https://github.com/jakubiszon26/apiforge/issues/469) was seen at the Overview zoom only and is not yet
  reproduced at 100 %.
- #452 (Windows test speed) is open. Windows runs on `main` only; pull
  request #484 (#483), open, moves it from every push to `main` to a
  nightly run, with Linux on each push.
- No automatic review until 2026-12-01: each pull request gets a written
  review before it merges.
- The mock-ups of 2026-11-19 are drawings; the Fit of 65–70 % on LAY-44's
  is an estimate, its criterion asks for 60 %.
- The estimates of REQ-07 to REQ-10 (about 75 minutes) are the proposals'
  own, not analysed.
