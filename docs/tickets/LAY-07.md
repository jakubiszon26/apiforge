# LAY-07: Sequence badges on every step, and the running step

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Backlog (issue #291) · decided by the owner on 2026-11-05 (Q2 = A)
  in the LAY plan ([lay.md 5.1](../sprints/lay.md#51-decisions-taken-2026-11-05))
  · LAY sprint, part A, after LAY-09 and LAY-22
  ([review of 2026-11-10](../sprints/lay.md#11-mid-sprint-review-2026-11-10));
  UX-25 (#233) is merged
- Analysis model: Fable · Implementation model: Opus

## Goal

A person reading a large scenario sees in what order the steps run without
following wires: every card carries the engine's sequence number, the same
number as its row in Run results, and the running step is told apart from
the pending and the finished ones.

## What the owner will see

- New: a 20 px circle at the card's top-left with the engine order (P1 of
  the plan). Every step is numbered, checks included (Q2 = A), so the badge
  of step *k* is the number of its Run results row. The numbers are a pure
  function of the graph and are recomputed on every edit.
- New: while a run is on, the running step's badge is filled with an
  activity glyph and its footer reads *Running…*; pending steps are muted;
  finished steps show their result. The running mark is a glyph in the
  badge, not a colour shared with the selection (4.4 item 6, 4.7 item 6).
- Changed: nothing else on the card. Removed or hidden: nothing. Default
  behaviour that changes: none; the badge is always shown.
- The badge is drawn at every detail level with a minimum on-screen size
  (a two-digit number at least 9 px high at Fit view, LAY-09's rule).

## Decisions taken (owner, 2026-11-05, quiz on the LAY plan)

> Q2 = A: badges on every step, the engine's order, equal to the Run
> results row; checks included.

## Mock-up

Approved as direction with round 3 and round 4 of LAY-00
(`mock/lay-00-round-3` at `ea981d8`, `mock/lay-00-round-4` at `df6305b`;
captures `lay00r3-d-shop-30-60`, `lay00r3-d-shop-30-states-60`,
`lay00r4-run-60`, `lay00r4-run-focus-60`, `lay00r4-run-stale-60` under
`docs/qa/2026-11-04/LAY-00/` on those branches). Round 4's proof 3 showed
a real partial run with the running step distinct from the selection; the
stale-versus-current distinction was not verified there and is criterion 4.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay07.rs`, on the LAY-01 reference scenarios.

1. For every step of `shop-30.afs` the badge's number equals the index of
   the step's row in Run results after a simulation, checks included; the
   same holds for `order-flow.afs` and `shop-50.afs`. Test:
   `lay07_criterion_1_the_badge_equals_the_row`.
2. Adding a step in code before step 5 renumbers the badges in the next
   frame; the pure function in `apiforge-core` agrees with the runner's
   order on the three fixtures (the technical appendix). Tests:
   `lay07_criterion_2_badges_follow_an_edit`, and a core test
   `sequence_agrees_with_the_engine_on_the_fixtures`.
3. During a real simulation paused by a slow sample, the running step's
   badge shows the activity glyph and its footer *Running…* within one
   frame of the runner's "started" event, while the selected step is a
   different card and keeps the selection frame. Test:
   `lay07_criterion_3_the_running_step_is_marked_apart_from_the_selection`.
4. The results of a previous run are told from the current run's: while a
   run is on, a card not yet reached shows its previous result muted with
   UX-25's stale mark (UX-25, #233, is a prerequisite: this ticket starts
   after it is merged), and the capture names both states. Test:
   `lay07_criterion_4_stale_results_are_told_from_current_ones`.
5. At Fit view of `shop-30.afs` at 1600 × 1000 a two-digit badge is at
   least 9 px high on screen and does not overlap the name. Test:
   `lay07_criterion_5_the_badge_has_a_minimum_size`.
6. QA: captures of `shop-30.afs` at 60 % with a running step other than
   the selected one, at 1600 × 1000 and 960 × 540.

## Not in scope

What the Overview and Compact levels show (LAY-09); the rows of Run results
(UX-19); the aggregate of collapsed checks (LAY-11); why a step did not run
(LAY-19).

## Technical appendix

- Sequence: LAY-08's `apiforge_core::graph::sequence` (wave 0) gives the
  numbers; LAY-07 only draws them.
- Running: LAY-22 makes the runner name the step that started; the window
  marks that step running until its result arrives. This ticket adds
  nothing to the runner.
- Badge: `canvas/paint.rs::card` draws the circle at the card's top-left
  from `canvas/layout.rs` geometry; the minimum on-screen size follows
  LAY-09's rule (`(size * zoom).max(min)`); the running state comes from
  the run's live results (the runner's events), not from `StepStatus`
  alone, since there is no "started" status in `StepResult`.
- The round-4 mock code on `mock/lay-00-round-4` is not merged.
