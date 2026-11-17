# LAY-13: Branch bands — one band per branch, one wire into the band

- Size: M · Owner review: 25 min · Risk: `visible UI`, `removal`
- State: Analysis, mock-up awaiting approval (issue #297) · direction
  decided by the owner on 2026-11-05 (Q11 = A) · re-checked on 2026-11-17
  against `main` at `ae8a348` (LANG-14, LAY-34, LAY-37 merged) · LAY
  sprint, part B, after LAY-34 and before LAY-19, LAY-17
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

The steps of an `if` branch are read as one group: a neutral band behind
them names the condition and the branch, and one wire from the `if` enters
the band instead of one wire per step.

## What changed since 2026-11-05

- **The band is the braces.** LANG-14 and LAY-34 made the braces a fact of
  the document: a statement stands in the braces of `x.then` when it
  follows `x.then` and everything else it follows stands there too
  (`apiforge-script/src/branches.rs`). The band holds exactly these
  statements. `refund` in `shop-30`, written `@after(paid.then)`, is one
  of them; `logout`, which follows a check, is not.
- **LAY-13 now comes before LAY-17**, so a branch can be cut by a row
  break. `shop-50` has it: `paid → No` is in two rows.
- **Shape D hangs a check under the step it reads**, wherever that step
  is. In `shop-50` the first statement of `paid → No`, `not_created`,
  reads `order` and hangs under `order`, left of the `if`. The band is
  drawn there; where the card is placed is LAY-17's.
- **LAY-31** (a step and its checks are one group, 24 px apart;
  implemented, awaiting acceptance) and **LAY-37** (chips keep clear,
  Tidy up leaves them room) set the room a band has. Both were rendered
  against; see *With LAY-31 and LAY-37*.

## What the owner will see

- New: a band behind the cards of each branch: a neutral grey-blue tint
  and a 1 px outline, the same for a branch that ran, was skipped or
  failed. Its header, in a 20 px strip above the cards and right of the
  first card's badge, reads `paid → Yes · ran`, `paid → No · skipped` or
  `created → Yes · failed`; without a run, `paid → Yes`. A branch in two
  rows has a band in each, the later ones `paid → No · continued ·
  skipped`. Under 45 % zoom the strip is lower than the smallest words
  and the band is drawn without them; pointing at the band says them.
- New: the band reaches round the chips at its cards' sockets
  (`← order`, `→ 3`), so no chip lies on its outline.
- Changed: one dashed wire per branch, in the branch's colour, from the
  `if` to the band's edge at the header's height. It ends at the edge,
  with the arrowhead; the words `yes` and `no` on the wire go, the header
  says them.
- Removed or hidden: the wire from the `if` to every further statement of
  the branch (`shop-30`: 5 wires become 2; `shop-50`: 10 become 4). The
  edges of the document are unchanged. While one statement of a band is
  selected its own wire from the `if` is drawn again, lit, and can be
  selected and deleted as today (question 2).
- Default behaviour that changes: none. Nothing is written; no card moves.

## Mock-up

Rendered by the throwaway branch `mock/lay-13-branch-bands`
(`review_lay13_mock`, `APIFORGE_LAY13=a|b`), tidied, run in simulation,
*Run results* folded, in
[`docs/qa/2026-11-17/LAY-13/`](../qa/2026-11-17/LAY-13/). The direction
(round 3 `lay00r3-d-branch-two-rows-60`, round 4
`lay00r4-band-continued-60`) was approved on 2026-11-05; those were drawn
over a board of that day.

| View | Before (`main`) | A | B |
| --- | --- | --- | --- |
| `shop-30`, checks drawn, 60 %, 1600 × 1000 | [capture](../qa/2026-11-17/LAY-13/lay13-main-shop-30-60-1600.png) | [capture](../qa/2026-11-17/LAY-13/lay13-a-shop-30-60-1600.png) | [capture](../qa/2026-11-17/LAY-13/lay13-b-shop-30-60-1600.png) |
| `shop-30`, checks drawn, Fit, 1600 × 1000 | [capture](../qa/2026-11-17/LAY-13/lay13-main-shop-30-fit-1600.png) | [capture](../qa/2026-11-17/LAY-13/lay13-a-shop-30-fit-1600.png) | |
| `shop-30`, checks folded, 60 %, 960 × 540 | [capture](../qa/2026-11-17/LAY-13/lay13-main-shop-30-folded-60-960.png) | [capture](../qa/2026-11-17/LAY-13/lay13-a-shop-30-folded-60-960.png) | |
| `shop-30`, checks folded, Fit, 960 × 540 | [capture](../qa/2026-11-17/LAY-13/lay13-main-shop-30-folded-fit-960.png) | [capture](../qa/2026-11-17/LAY-13/lay13-a-shop-30-folded-fit-960.png) | |
| `shop-50`, a branch in two rows, 60 %, 1600 × 1000 | [capture](../qa/2026-11-17/LAY-13/lay13-main-shop-50-60-1600.png) | [capture](../qa/2026-11-17/LAY-13/lay13-a-shop-50-60-1600.png) | [capture](../qa/2026-11-17/LAY-13/lay13-b-shop-50-60-1600.png) |
| `checkout.afs`, 100 %, 1600 × 1000 | [capture](../qa/2026-11-17/LAY-13/lay13-main-checkout-100-1600.png) | [capture](../qa/2026-11-17/LAY-13/lay13-a-checkout-100-1600.png) | |
| `shop-30` on LAY-31's branch, 60 % | | [capture](../qa/2026-11-17/LAY-13/lay13-a-lay31-shop-30-60-1600.png) | |

A and B differ in one thing, what the band is drawn round:

- **A (recommended): the branch's steps, each with everything hung under
  it.** One rectangle per row. A check that reads a step of the branch
  but is written after the braces (`refund_amount` in `shop-30`) is
  inside, because it hangs under that step and cannot run without it.
- **B: the statements of the braces, nothing else.** The outline steps in
  where a card hung under a step of the branch is not in the braces
  (`refund_amount`), so the band is not a rectangle and it cuts a group of
  LAY-31 in two.

## With LAY-31 and LAY-37

- **A group is never cut (A).** The band's unit is LAY-31's group: a
  step and the cards hung under it. Dragging a step takes its hung cards
  along (LAY-31, question 2 = A); the band follows the cards, it has no
  position of its own.
- **The header fits LAY-31's gap.** The strip is 20 px (16 px of words,
  4 px under them); the gap above a hung card is 24 px and does not grow.
  A branch whose first card hangs under the `if` (`paid → No` in
  `shop-30`) starts 4 px under the `if`, its header in the gap. Rendered
  on `feat/lay-31-checks-group`: it fits, nothing overlaps. No layout rule
  is added.
- **A row of the spine** already leaves more than 20 px above its cards;
  the strip takes none from the row above.
- **Chips are inside.** The band's first and last column reach as far as
  the chips of their own cards, 6 px beyond: up to about 85 px left of a
  reader, 40 px right of a source. Tidy up's chip room (LAY-37) is what
  makes this fit: on LAY-31's branch the bands of `paid → No` and
  `paid → Yes` are 24 px apart on the board. A chip of a card outside the
  band never widens it.
- **A line of another step may cross a band**, as it crosses a row today
  (`shop-50`: the rail *after receipt_total*). The band is no obstacle to
  the router and adds no line to the board.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay13.rs`,
on `shop-30.afs`, `shop-50.afs` (its `paid → No` is in two rows) and
`order-flow.afs`, tidied, checks drawn and folded.

1. Each branch that has a card drawn has exactly one wire from the `if`
   at rest, and it ends on the outline of the branch's first band. Test:
   `lay13_criterion_1_one_wire_per_branch`.
2. A band holds every drawn statement of the braces, `refund` of
   `shop-30` included and `logout` not, with (A) the cards hung under its
   steps; no other card touches it and no two bands overlap. Test:
   `lay13_criterion_2_a_band_holds_the_branch`.
3. Every band of a branch in several rows carries the header; the later
   ones say `· continued`; with a run each says `ran`, `skipped` or
   `failed`. Test: `lay13_criterion_3_every_fragment_has_the_header`.
4. The tint and outline are the same for a branch that ran, was skipped
   and failed, and with no run. Test:
   `lay13_criterion_4_the_tint_is_neutral`.
5. No chip of a card of the band crosses its outline, and the header's
   words do not lie under a badge. Test:
   `lay13_criterion_5_chips_and_badges_keep_clear`.
6. Gesture: selecting one statement of a band draws its own wire from the
   `if`; clicking that wire selects it and *Delete* removes the edge, so
   the statement leaves the band; dragging from the `if`'s *Yes* socket to
   a step outside adds it, and the band grows. Selecting nothing draws
   one wire again. Test: `lay13_criterion_6_a_step_enters_and_leaves`.
7. Dragging a card of a band moves the band's outline with it in the same
   frame; nothing is written to the document. Test:
   `lay13_criterion_7_the_band_follows_its_cards`.
8. QA: `shop-30` and `shop-50` at 960 × 540 and 1600 × 1000, at Fit and
   at 60 %, against the captures above.

## Not in scope

Where the branch's steps are placed and where a row breaks (LAY-17); the
`if` card's own look; folding a band; an `if` inside a branch (no
reference scenario has one: the inner band is drawn inside the outer by
the same rule, without a test).

## Technical appendix

- The reading of the braces exists: `Branches::parent` in
  `apiforge-script/src/branches.rs`, `pub(crate)`. The board needs it;
  it reads `apiforge_core::format::Graph` alone, so it moves to
  `apiforge-core` (`graph.rs`) and `apiforge-script` uses it from there.
  One reading for the printer, the lowering and the band. This is the
  crate boundary that makes the ticket `M`.
- Rendering: a new `canvas/bands.rs`; `canvas/mod.rs` draws the bands
  before the continuation marks and the wires, drops the branch wires but
  the first of each band from `board_wires` before routing (so chips and
  counts see the same wires), and brings a dropped one back through the
  `revealed` set that chips' wires already use (LAY-23, LAY-10).
- The entry is routed to the band's edge, not to a card: beside the band
  it is the port, up to the header's height, across; otherwise the routed
  wire to the first step, cut at the outline, as the mock-up draws it.
- Accessibility: a band is a named node (`paid → Yes, ran, 4 steps`).
- The mock-up tells rows and columns by where the cards lie; the
  implementation reads `shape::plan`.
