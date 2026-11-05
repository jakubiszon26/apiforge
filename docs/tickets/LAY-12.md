# LAY-12: Acceptance of the LAY sprint — the demo and the timed task sheet

- Size: S · Owner review: 10 min · Risk: none
- State: Backlog (issue #296) · in must by the second opinion of 2026-11-04
  (4.7) and the owner's Q12 of 2026-11-05 · LAY sprint, wave 4, last
- Analysis model: Fable · Implementation model: Opus (the measurements), the
  owner at the window for the timed tasks

## Goal

The sprint is accepted on the three reference scenarios, not on captures:
opened cold, read, run (simulation and live loopback), a failure found; the
UX task sheet is timed once before the sprint's first ticket and once at its
end, and the two small-window goals are checked at 960 × 540.

## What the owner will see

- Nothing new on screen. A pull request whose body holds the demo plan of
  [lay.md section 10](../sprints/lay.md#10-demo-plan) run end to end at
  1600 × 1000 and 960 × 540, the task sheet of
  [section 9](../sprints/lay.md#9-metrics-and-qa) with times and the number
  of zoom and pan actions before and after, and the LAY-01 metric table
  before and after each ticket.
- Task 8 (the return-to-work walk) is asserted by clicks in a test, as round
  4's proof 4 did (four clicks, no pans).

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. The demo plan's eight steps run at 1600 × 1000 and 960 × 540 on
   `order-flow.afs`, `shop-30.afs` and `shop-50.afs` in the demo shop
   project, in simulation and against the live loopback sample; each step
   has one capture in the pull request body.
2. The UX task sheet's eight tasks are timed before the sprint (on `main`
   at the sprint's start) and at its end, by the owner or an agent at the
   window; the two columns are in the pull request body. Task 8 is also
   `lay12_task_8_the_walk_by_clicks` in
   `crates/apiforge-desktop/src/folder/acceptance_tests.rs`.
3. The two small-window goals of 4.7 item 9 hold at 960 × 540 on
   `shop-50.afs` after a failed run: at Fit the failures and the selection's
   row are found without zooming; at 60 % the producers and readers of the
   selected step are named from the board and the chips. Both timed.
4. The LAY-01 table is printed on `main` before and after the sprint for the
   three scenarios: crossings, bends, close pairs, board width, fit zoom.
5. Every LAY ticket's issue is closed or carried into LAY-2 with a reason in
   the backlog.

## Not in scope

New behaviour; fixing what the demo finds (new tickets).

## Technical appendix

The timed tasks are manual by design (the plan, section 9); the deterministic
parts (badge equals row, failed check visible collapsed, no card moved by a
toggle, same positions from two opens, dimming never hides the selection)
are the tickets' own tests and are only re-run here.
