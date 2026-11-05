# LAY-11: Collapse checks — the toggle, the aggregate, the Inspector list, Expand checks here

- Size: M · Owner review: 30 min · Risk: `visible UI`, `default`
- State: Backlog (issue #295) · decided by the owner on 2026-11-05 (Q4 = A,
  Q5 = A, Q6 = A, Q15 = A) · LAY sprint, wave 1, last (after LAY-08 and
  LAY-18)
- Analysis model: Fable · Implementation model: Opus

## Goal

A board of thirty steps shows its requests, not its checks: checks with a
subject fold into a second result line in the subject's footer, the
Inspector lists them in words, and a step can be expanded in place without
moving the camera. A failed check stays visible while collapsed, and a gate
whose result is read later never disappears.

## What the owner will see

- New: a toolbar toggle *Checks* (expanded / collapsed). Collapsed by
  default, as an application preference like the wire style (Q4 = A).
- Collapsed: check cards with a subject and their wires are not drawn; each
  subject shows the aggregate in its footer as a second statement after the
  request's own result (`HTTP 201 · ✓ 2 checks`, `HTTP 200 · ✕ 1/3 failed ·
  1 skipped`, a neutral grey `3 checks` before a run, never a tick for a
  check that did not run); the checks line names the state of every check
  (`1 ✓ · 1 waiting`). A click on the aggregate opens the Inspector's
  *Checks* list (LAY-08) with *Expand checks here*, a per-step, session-only
  override that hangs the step's checks under it and shifts the rows below
  by their height; the camera stays. Standalone checks stay cards.
- A failed check on a successful request: the footer turns red (`HTTP 200 ·
  ✕ 1/3 failed`), the card gets the 2 px red border at every zoom, the band
  keeps the HTTP colour, Run results keeps the check's own row (Q15 = A).
- A check with several subjects is counted under every subject it reads,
  with *Shared with: cart* in the Inspector (Q6 = A). An `@after` on a
  hidden check is drawn from its subject with the check's name. A gate
  whose value is read stays a short visible node with its wire (4.7 item 4).
- Navigation: a Run results row of a collapsed check selects its subject and
  opens the list. Tidy up in collapsed mode lays out the visible cards and
  hangs the hidden checks under their subject (D's satellite rule), so
  expanding never needs a second tidy. The toggle never moves the camera.
- Removed or hidden: check cards, by default, behind the toggle — the
  owner's decision Q4. Default behaviour that changes: a new install opens
  with checks collapsed.

## Decisions taken (owner, 2026-11-05)

> Q4 = A: collapsed by default, an application preference; *Expand checks
> here* is session-only. Q5 = A (not vetoed): the aggregate in the footer,
> a pill at Overview. Q6 = A (not vetoed): counted under every subject,
> marked shared. Q15 = A: red footer and border for a failed check on a
> green request. Rule of the model (planning, 4.7 item 4): a gate whose
> result is read later is never hidden.

## Mock-up

Round 3 `lay00r3-aggregate-checkout`, `lay00r3-aggregate-gate`,
`lay00r3-d-shop-30-fit` / `-expanded-fit`, `lay00r3-inspector-checks`;
round 4 proof 2 `lay00r4-collapse-collapsed`, `-expanded`, `-expand-here`,
`-expand-shift` (`mock/lay-00-round-3` at `ea981d8`, `mock/lay-00-round-4`
at `df6305b`). Approved as direction on 2026-11-05. Round 4 measured
*Expand checks here* on `checkout`: without the shift the expanded checks
overlapped the row below by 280 × 60 px; with the rows shifted by +180 px
nothing overlapped (criterion 5).

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`, `mod
lay11`, on `shop-30.afs`.

1. Collapsed, after a failed simulation, the failed check is visible at
   0.35, 0.60 and 1.0 as its subject's red footer and border. Test:
   `lay11_criterion_1_a_failed_check_is_visible_collapsed`.
2. Before a run every aggregate is grey (`3 checks`), no tick; after a
   partial run the checks line reads the state of every check (`1 ✓ · 1
   waiting`). Test: `lay11_criterion_2_no_tick_before_a_run`.
3. Toggling *Checks* moves no card of the visible set and no camera; the
   preference survives a restart. Test:
   `lay11_criterion_3_the_toggle_moves_nothing_and_is_remembered`.
4. Clicking `checkout`'s aggregate opens the Inspector's list with *Expand
   checks here*. Test: `lay11_criterion_4_the_aggregate_opens_the_list`.
5. *Expand checks here* on `checkout` hangs its checks under it, shifts the
   rows below by their height, overlaps nothing and keeps the camera; it is
   forgotten on restart. Test:
   `lay11_criterion_5_expand_here_shifts_the_rows_below`.
6. The two-subject check is counted under both subjects; the Inspector
   shows *Shared with: cart* under the other. Test:
   `lay11_criterion_6_a_shared_check_is_counted_twice`.
7. A *Data condition* whose value `order` reads stays visible as a short
   node with its wire while collapsed; an `@after` on a hidden check is
   drawn from its subject with the check's name. Test:
   `lay11_criterion_7_gates_and_after_wires_stay`.
8. Clicking the Run results row of a collapsed check selects its subject and
   opens the list. Test: `lay11_criterion_8_run_results_navigates_to_the_subject`.
9. Tidy up with checks collapsed places the hidden checks under their
   subject, so expanding needs no second tidy (the positions after
   expanding equal the positions Tidy up expanded would give). Test:
   `lay11_criterion_9_tidy_up_hangs_hidden_checks`.

## Not in scope

The subject function and the list's content (LAY-08); the shape (LAY-06);
what a failed step's consequences read (LAY-19); descriptions.

## Technical appendix

- A filter over cards and wires in `canvas/mod.rs` before routing, from the
  run-independent set of checks with a subject (LAY-08); hidden cards are
  not obstacles either. Tidy up in collapsed mode passes the visible set to
  the layout and places hidden checks under their subject with
  `layered::place`.
- The preference lives beside the wire style in the application settings;
  *Expand checks here* is a `HashSet<NodeId>` on the canvas, not saved.
- The aggregate reads the run's `StepResult`s of LAY-08's subjects; a check
  not yet run is *waiting*, never ticked.
