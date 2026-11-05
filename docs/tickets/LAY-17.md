# LAY-17: Wrapping by dependencies — the row break respects the wires

- Size: M · Owner review: 20 min · Risk: `default`
- State: Backlog (issue #301) · added to the sprint by the owner on
  2026-11-05 (Q12 = C; the second opinion's item 4 of
  [lay.md 4.5](../sprints/lay.md#45-second-opinion-part-two-local-context))
  · LAY sprint, wave 2, after LAY-06
- Analysis model: Fable · Implementation model: Opus

## Goal

Shape D's rows break where the scenario can be cut, not after a fixed count
of columns: fewer wires cross from one row to another, no request is parted
from its satellites, and a branch is not split in two by the break.

## What the owner will see

- Changed: where Tidy up starts a new row. The break is chosen among the
  neighbouring column boundaries (four to six columns at the default row
  width) by four costs: the wires that would cross rows, a request split
  from its satellites, a cut branch and the rows' heights; the order is
  unchanged. It runs at the first layout and at Tidy up, never on a resize
  of the window. Default behaviour that changes: the positions Tidy up
  produces for a board that wraps.
- Removed or hidden: nothing.

## Mock-up

Round 3 reported where the fixed wrap cuts `shop-30`
(`lay00r3-d-shop-30-fit`, `mock/lay-00-round-3` at `ea981d8`): the break
after five columns parts `checkout` from its checks. No further mock-up: the
visible change is the row break, judged on the QA captures of `shop-30` and
`shop-50` against round 3's, with the LAY-01 metrics.

## Acceptance criteria

Tests in `crates/apiforge-layout/src/layered.rs` (`mod tests`) and a
window test in `acceptance_tests.rs`, `mod lay17`.

1. On `shop-30` the wires crossing a row boundary are no more than with the
   fixed five-column wrap, and no request is placed in a different row from
   its satellites. Test: `lay17_criterion_1_shop_30_breaks_better`.
2. A branch whose steps fit one row is never split by the break; one that
   does not fit is split once, at its own step boundary. Test:
   `lay17_criterion_2_a_branch_is_not_cut`.
3. The board stays at most 1920 px wide at the default row width (LAY-06's
   bound); a row is no taller than its tallest short stack. Test:
   `lay17_criterion_3_the_bounds_hold`.
4. Deterministic: two layouts of the same scenario are equal; a window
   resize changes no position. Test: `lay17_criterion_4_deterministic_and_resize_free`.
5. The LAY-01 table before and after is in the pull request body for the
   three scenarios.

## Not in scope

The shape itself (LAY-06); a row width chosen per window (the row width is
a parameter); routing (LAY-03, LAY-04, LAY-05).

## Technical appendix

`layered`'s spine mode (LAY-06) gives rows after the columns are known; this
ticket inserts a choice of the break column among `default - 1 ..= default +
1` minimising a weighted sum of the four costs, computed from the edges and
the satellites' subjects; ties keep the default.
