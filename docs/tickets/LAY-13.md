# LAY-13: Branch bands — one band per branch, one wire into the band

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: Backlog (issue #297) · decided by the owner on 2026-11-05 (Q11 = A)
  · LAY sprint, wave 2, after LAY-17
- Analysis model: Fable · Implementation model: Opus

## Goal

The steps of an `if` branch are read as one group: a neutral band behind
them names the condition and the branch, and one wire from the `if` enters
the band instead of one wire per step.

## What the owner will see

- New: a tinted band behind the cards of each branch, the tint neutral (not
  a status colour), with a header `created → Yes` naming the condition and
  the branch, repeated on every fragment of a wrapped branch with
  `· continued` and the branch's status; one dashed orthogonal wire from the
  `if` into the band; a band per row for a branch that wraps (4.7 item 8);
  steps lowered into the branch by `@after(x.then)` are included.
- Changed: the branch wires (one per step today) are drawn as one into the
  band; the edges of the model are unchanged. Removed or hidden: the
  per-step branch wires. Default behaviour that changes: none.

## Decisions taken (owner, 2026-11-05)

> Q11 = A: one band per branch, one wire into the band (option B, one wire
> per step on the rail, not chosen).

## Mock-up

Round 3 `lay00r3-d-branch-two-rows-fit`, `-60`; round 4
`lay00r4-band-continued-60` (`mock/lay-00-round-3` at `ea981d8`,
`mock/lay-00-round-4` at `df6305b`). Approved as direction on 2026-11-05;
the round-3 continuation lost its header, hence criterion 3.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`, `mod
lay13`, on `shop-50.afs` (its branch wraps) and `order-flow.afs`.

1. Each branch has exactly one wire from the `if`, into its band. Test:
   `lay13_criterion_1_one_wire_per_branch`.
2. A band contains exactly the branch's steps, those lowered by
   `@after(x.then)` included, and no other card overlaps it. Test:
   `lay13_criterion_2_a_band_holds_the_branch`.
3. Every fragment of a wrapped branch carries the header with `· continued`
   and the branch's status. Test:
   `lay13_criterion_3_every_fragment_has_the_header`.
4. The band's tint is the same for a passed, failed and not-run branch; the
   status is in the header's text and the cards, not the tint. Test:
   `lay13_criterion_4_the_tint_is_neutral`.
5. QA: `shop-50.afs` at 960 × 540 and 1600 × 1000 with a two-row branch.

## Not in scope

Where the branch's steps are placed (LAY-06, LAY-17); the `if` card's own
look; nested branches beyond one level (deferred if the fixtures have none).

## Technical appendix

Rendering only: `canvas/paint.rs` draws the band from the branch's cards'
union per row before the cards; the `if`'s branch edges are drawn as one
routed wire to the band's entry; `apiforge-layout` edges are unchanged.
