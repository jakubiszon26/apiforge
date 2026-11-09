# LAY-20: The Checks list covers a component call and a Cleanup check that reads a Main step

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Implemented, awaiting acceptance (issue #308) · decisions taken
  2026-11-07 · LAY sprint, wave 1, before LAY-11
- Analysis model: Fable · Implementation model: Opus

## Goal

Every check with a subject is listed under that subject, so that LAY-11 can
hide its card without hiding a failure: a component call gets the *Checks*
tab, and a Cleanup check that reads a Main step appears under that step with
the moment word *Cleanup*.

## What the owner will see

- New: the Inspector of a component call (`promoted`) has the third tab
  *Checks*, with the same list as a request's: predicate, sources, status,
  moment, the count line (`1 check`).
- New: the list of a Main step (`cart`) also names the Cleanup checks that
  read it, after its Main checks, titled `cart_id_kept — Cleanup` before the
  check has a result and with the result after a run. It is counted apart,
  like a gate: `3 checks · 1 shared · 1 in Cleanup` (decision Q20.2 = A).
- Rule for LAY-11, recorded here: a check collapses only under a subject on
  its own board. A Cleanup check whose subjects are all Main steps stays a
  card on the Cleanup board (it has no wire there to fold into) and is
  listed under its Main subject; a component call is a subject like a
  request, so its checks collapse under the call and its footer carries the
  aggregate.
- Changed, removed, hidden: nothing. Default behaviour that changes: none.

## Facts (mock `mock/lay-20-checks` at `d253fed`)

- `check_subjects` already counts a component call as a subject; only the
  Inspector's tab gate (`NodeKind::Request`) leaves its checks unlisted.
- The list reads the board shown, so a Cleanup check reading `cart` is not
  listed under `cart` today; on the Cleanup board it is a card without a
  wire.
- At 960 × 540 the fourth row of `cart` is below the fold (the list
  scrolls, accepted in LAY-08); at 1600 × 1000 all rows show.
- The sources line names the export port of a component call
  (`promoted.product.promotion.price` for the code's
  `promoted.promotion.price`): LAY-08's rule, unchanged here.

## Decisions (owner, 2026-11-07)

Taken on the captures of `mock/lay-20-checks` (`d253fed`), recorded on #308.

- **Q20.1 = A — The tab on a component call.** The same *Checks* tab and
  list as a request's. Under LAY-11 a call's checks collapse under the call.
- **Q20.2 = A — A Cleanup check under its Main step.** In the same list,
  counted apart like a gate: the count reads `3 checks · 1 in Cleanup` and
  the row reads `cart_id_kept — Cleanup`. The Main step's footer aggregate
  (LAY-11) leaves it out, and its card stays on the Cleanup board.

A failure is never hidden by that counting: a failed Cleanup row still
colours the line above the list, as a failed gate does, and the row carries
the failure in words. Tests:
`lay20_a_failed_cleanup_check_is_shown_and_counted_apart` through the window
and `the_summary_counts_what_became_of_the_checks` for the colour.

## Mock-up

`mock/lay-20-checks` at `d253fed`, `docs/qa/2026-11-07/LAY-20/`:
`lay20-component-960`, `-1600`; `lay20-cleanup-counted-`, `-apart-`,
`-section-960`/`-1600`; `lay20-cleanup-board-960`; `lay20-facts.txt`. The
fixture is `shop-30` with `promo_priced = assert promoted.promotion.price >
0` in Main and `cart_id_kept = assert cart.body.id == "c-1"` in Cleanup,
edited in the harness as LAY-08's tests do; `shop-30.afs` itself does not
change, so LAY-01's metrics hold.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`, `mod
lay20`, on `shop-30.afs` with the two checks above added in the harness.
The wording follows the owner's answers.

1. The Inspector of `promoted` has a *Checks* tab; it lists `promo_priced`
   with predicate, sources, status and moment, and the count line reads `1
   check`. Test: `lay20_criterion_1_a_component_call_lists_its_checks`.
2. Under `cart` on the Main board, `cart_id_kept` is listed after the Main
   checks with the title `cart_id_kept — Cleanup`; the count line reads
   `3 checks · 1 shared · 1 in Cleanup`. Test:
   `lay20_criterion_2_a_cleanup_check_is_listed_under_its_main_step`.
3. After a simulation the row shows the check's result
   (`cart_id_kept — Passed`) and the count line follows it
   (`3/3 ✓ · 1 shared · 1 in Cleanup`). Test:
   `lay20_criterion_3_the_cleanup_result_after_a_run`.
4. On the Cleanup board the list of `removed` names `gone`; `cart_id_kept`
   is listed under no Cleanup step and its card is still there. Test:
   `lay20_criterion_4_the_cleanup_board_lists_its_own_checks`.
5. The unit tests of `inspector::checks` and LAY-08's criteria still pass;
   a request's list is unchanged when the scenario has no Cleanup check.
   Tests: the existing `lay08_*` tests and
   `a_cleanup_check_is_listed_under_the_main_step_it_reads`. The count line
   still stands whole at 960 × 540, the fourth row below the fold as LAY-08
   accepted. Test: `lay20_the_count_line_stands_at_the_smallest_window`.

## Not in scope

Hiding cards and the footer aggregate (LAY-11, which applies the rule
above); the wording of component references (LAY-08's rule); a check inside
a component's own graph (listed in that component's board, as today).

## Technical appendix

As built:

- `inspector/mod.rs`: the tab gate is `Request | Component`; `Context` gained
  `cleanup: Option<&Graph>`, the scenario's Cleanup graph while the board
  shows Main. Both windows fill it: `folder/diagram.rs` (the folder window)
  and `app.rs` (the workspace window, through `Editor::sibling_graph`); it is
  `None` on the Cleanup board and inside a component.
- `inspector/checks.rs`: `rows` runs `pass` once over the board and once over
  the Cleanup graph, the second resolving sources in both so that the Main
  steps it reads are found; a Cleanup row's moment is *Cleanup* until it has
  a result; `summary` counts those rows apart and leaves them out of the pass
  fraction, while a failed one still colours the line. A Cleanup graph
  without a check costs no second pass.
- `apiforge-core::graph`: `check_subjects_in(&[&Graph], check)` is
  `check_subjects` over several graphs, which spares the Inspector an owned
  combined graph per frame; `check_subjects` now calls it with one.
- About 130 changed lines outside tests. The mock's `mock_lay20` switch is not
  merged.
