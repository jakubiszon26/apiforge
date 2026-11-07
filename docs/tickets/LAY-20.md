# LAY-20: The Checks list covers a component call and a Cleanup check that reads a Main step

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Analysis (issue #308) · decisions pending (Q20.1, Q20.2 below) ·
  LAY sprint, wave 1, before LAY-11
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
  check has a result and with the result after a run. How it is counted is
  Q20.2.
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

## Decisions pending (owner)

> **Q20.1 — The tab on a component call.** A (recommended): the same
> *Checks* tab and list as a request; under LAY-11 its checks collapse under
> the call. B: no tab; a component call's checks stay cards under LAY-11.
>
> **Q20.2 — A Cleanup check under its Main step.** A (recommended): in the
> same list, counted apart like gates — `3 checks · 1 shared · 1 in
> Cleanup`; the Main step's footer aggregate (LAY-11) leaves it out; its
> card stays on the Cleanup board. B: counted in — `4 checks`, the row
> marked *Cleanup*; LAY-11 hides its card and the Main footer includes its
> result. C: under a *CLEANUP* heading of its own with its own count line;
> otherwise as A.

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
   checks with the title `cart_id_kept — Cleanup`; the count line reads as
   Q20.2 says. Test:
   `lay20_criterion_2_a_cleanup_check_is_listed_under_its_main_step`.
3. After a simulation the row shows the check's result and the count line
   follows it (`3/3 ✓ · 1 shared · 1 in Cleanup` under Q20.2 = A). Test:
   `lay20_criterion_3_the_cleanup_result_after_a_run`.
4. On the Cleanup board the list of `removed` names `gone`, and
   `cart_id_kept` is listed under no Cleanup step. Test:
   `lay20_criterion_4_the_cleanup_board_lists_its_own_checks`.
5. The unit tests of `inspector::checks` and LAY-08's criteria still pass;
   a request's list is unchanged when the scenario has no Cleanup check.
   Test: the existing `lay08_*` tests.

## Not in scope

Hiding cards and the footer aggregate (LAY-11, which applies the rule
above); the wording of component references (LAY-08's rule); a check inside
a component's own graph (listed in that component's board, as today).

## Technical appendix

- `inspector/mod.rs`: the tab gate becomes `Request | Component`;
  `Context` gets `cleanup: Option<&Graph>`, the scenario's Cleanup graph
  while the board shows Main (`folder/diagram.rs`; `None` in `app.rs`).
- `inspector/checks.rs`: `rows` runs once over the board and once over the
  Cleanup graph with a combined lookup graph for `check_subjects`; a
  Cleanup row's moment is *Cleanup* until it has a result; `summary` counts
  per Q20.2. The mock's `mock_lay20` switch is not merged.
- About 120 changed lines outside tests.
