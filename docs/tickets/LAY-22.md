# LAY-22: Runner and core groundwork — the step that started, why a step did not run, a step's relations

- Size: S–M · Owner review: 10 min · Risk: none visible
- State: Backlog (issue #312) · split from LAY-07, LAY-19 and LAY-10 by the mid-sprint
  review of 2026-11-10 ([lay.md 11](../sprints/lay.md#11-mid-sprint-review-2026-11-10),
  finding R8) so that the engine track runs beside the card track · LAY
  sprint, part A
- Analysis model: Fable · Implementation model: Opus

## Goal

The three facts the board will draw exist in the engine and the model
before any card draws them: which step is running, why a step did not run,
and what a step is related to.

## What the owner will see

Nothing. No window, no wording and no default changes. The command line's
text output is unchanged. A run's JSON (the command line's report and the
MCP `run` result) gains one optional field per not-run step, `cause`; no
existing field changes.

## Acceptance criteria

1. **Started.** The runner tells its observer which step started, before
   the step executes and only for a step that executes; the result that
   follows clears it. A finished run names no running step. The command
   line and the MCP server ignore it. Test (runner):
   `a_step_is_announced_before_its_result`.
2. **Cause.** `StepResult` carries `cause: Option<NotRunCause>` with
   `Stopped { by }` (the step whose failure stopped the run under the
   policy *Stop*), `BlockedBy { step }` (a source failed or was itself not
   run), `ConditionUnmet { condition }` (a false *Data condition*),
   `OutsidePath` (a branch not taken), `Cancelled`, and one variant for a
   Cleanup step whose input is missing. Every branch of `engine.rs` that
   builds a Skipped, Blocked or Cancelled result sets exactly one; a step
   that ran has none. The message strings stay as they are. Test (runner):
   `not_run_causes_are_an_enum_with_one_variant_per_branch`.
3. **Cause on the fixture.** After the failed simulation of `shop-30` the
   task sheet uses (`checkout`'s sample `total: 170`), every not-run step's
   cause chain reaches `checkout_total`; a step on a branch not taken is
   `OutsidePath` in a passing and in a failing run. Test (runner
   `tests/acceptance.rs`): `causes_on_shop_30`.
4. **Relations.** `apiforge_core::graph::relations(graph, node)` returns
   the step's producers and readers, each with the output field and the
   input it feeds, its checks (LAY-08's `check_subjects`, inverted) and its
   gates; for a check it returns its subjects and value sources. On
   `shop-30`, `cart` has the producers `login`, `first`, `second`, the
   check `cart_total` and the reader `order`. Test (core):
   `relations_of_cart_on_shop_30`.
5. **Additive.** The MCP `run` tool's output validates against its
   `outputSchema` with `cause` present (the MCP-14 test), and a report
   written before this change still loads. Test:
   `a_report_without_causes_still_loads`.
6. `cargo test --workspace --all-targets --locked` passes; no file under
   `crates/apiforge-desktop/src/canvas/` changes.

## Regression potential

The run journal and the MCP output contract: the field is optional and
skipped when absent, so earlier reports and clients read on. The engine's
behaviour — when it stops, blocks or skips — does not change; the runner's
acceptance tests are the guard.

## Not in scope

Anything drawn: the running badge (LAY-07), the not-run footers and the
count (LAY-19), focus (LAY-10).

## Technical appendix

- `engine.rs`, the loop that builds a result for a step that does not run
  (`OUTSIDE_PATH`, `CANCELLED`, `UNAVAILABLE`, `STOPPED`): `gate` already
  knows whether the path is inactive, a source is unavailable or a Cleanup
  input is missing; it has to say which source, and whether that source is
  a false *Data condition*.
- The observer today receives the whole `RunResult` after each step
  (`notify`); the started step can travel the same way, as a field that is
  never serialised into a report, or as an event of its own — the agent's
  choice, recorded in the pull request.
- `relations` sits beside `dependencies` and `check_subjects` in
  `apiforge-core/src/graph.rs`.
