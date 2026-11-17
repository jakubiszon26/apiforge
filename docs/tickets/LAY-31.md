# LAY-31: A step and its checks are one group

- Size: M · Owner review: 25 min · Risk: `default`, `visible UI`
- State: Implemented, awaiting acceptance. Mock-up approved by the owner on
  2026-11-17 with questions 1 = A and 2 = A (#384). LAY sprint, layout
  track, after LAY-29 (#382, merged in #405) and before LAY-30 (#383).
- Analysis model: Fable 5.2 · Implementation model: Opus 5.5

## Goal

With checks drawn, every check sits directly under the step it checks, in
one stack, and nothing but the group's own lines is between them. The
owner's board of 2026-11-16 had `cart_created` and `empty` about 300 px
under `created`, other steps' lines in the gap and the checks' own lines
looping round the cards.

## Decided by the owner (2026-11-16)

- **A step and its checks are one group.** Checks sit directly under their
  subject, no foreign line passes between a step and its checks, lines go
  round the whole group. The rule that lines stay 20 px apart (LAY-03)
  holds outside the group; it no longer pushes a check away from its step.
- A check with several subjects sits **under its latest subject**, as
  today.

## What was found

On `main` the gap above a hung card is a corridor: Tidy up widens it by
20 px for every line that wants to pass (`layered.rs`, `Room::stacks`), and
the line that asks most often is the group's own. A check reads its step
from a socket on the step's right and takes it in on its own left, so the
line runs round the check, through the gap. On `shop-30`, checks drawn,
the gaps are 24 to 80 px and 5 600 px of line do nothing but join a step
to the card under it. On a board with more lines the same rule gives the
300 px the owner saw.

The group is the stack Tidy up already builds (`canvas/shape.rs`): a step
on the spine and every card hung under it — its checks, and the values and
data conditions that hang the same way.

## Mock-up

Rendered by the throwaway branch `mock/lay-31-checks-group`
(`review_lay31_mock`, `APIFORGE_LAY31=a|b`): `shop-30`, checks drawn,
tidied, the default view (chips for shared values), in
[`docs/qa/2026-11-17/LAY-31/`](https://github.com/jakubiszon26/apiforge/tree/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31)
of that branch (commit 6a91e43). The captures of the implementation, before
and after, are in [`docs/qa/2026-11-17/LAY-31/`](../qa/2026-11-17/LAY-31/)
of this one.

| View | Before (`main`) | A | B |
| --- | --- | --- | --- |
| Fit, 1600 × 1000 | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-main-shop-30-fit-1600.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-a-shop-30-fit-1600.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-b-shop-30-fit-1600.png) |
| Fit, 960 × 540 | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-main-shop-30-fit-960.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-a-shop-30-fit-960.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-b-shop-30-fit-960.png) |
| `cart` and its checks, 60 % | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-main-cart-1600.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-a-cart-1600.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-b-cart-1600.png) |
| `checkout` and its checks, 60 % | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-main-checkout-1600.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-a-checkout-1600.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-b-checkout-1600.png) |
| `login` and its checks, 100 % | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-main-login-1600.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-a-login-1600.png) | [capture](https://github.com/jakubiszon26/apiforge/blob/6a91e435347ae96594267655531f9012214ace08/docs/qa/2026-11-17/LAY-31/mock-b-login-1600.png) |

In both A and B the gap above a hung card is 24 px and never grows, and no
line of another card enters a group. They differ in how a hung card shows
that it reads its step.

Measured on the mock-up (`shop-30`, checks drawn, 1600 × 1000):

| | Before | A | B |
| --- | --- | --- | --- |
| Gap above a hung card, mean / worst | 39 / 80 px | 24 / 24 px | 24 / 24 px |
| Lines drawn | 19 | 10 | 19 |
| Crossings | 6 | 1 | 7 |
| Line from a step to a card hung under it | 5 604 px | 0 | 5 436 px |
| Lines of other cards in a group's gap | 1 | 0 | 0 |
| Board | 1820 × 1622 | 1800 × 1554 | 1820 × 1554 |
| Fit | 46 % | 48 % | 48 % |

The mock-up tells a group by where the cards lie and relaxes the router
roughly; the implementation may do better than these numbers, not worse.

## Decided by the owner (2026-11-17)

Both questions were answered **A** in the quiz on the mock-up, and the
mock-up was approved (M · 25 review minutes).

1. **How a card hung under its step shows that it reads it.**
   **A (chosen):** by a chip at its input (`← login.status`), the form
   a far reader already has; no line is drawn inside a group between the
   step and the cards under it. Pointing at the chip, or selecting the
   check or the step, draws the line, as for every chip (LAY-23). Nine
   lines fewer on `shop-30`, one crossing instead of six. It applies in
   the default *Chips for shared values*; *All wires* draws every line and
   looks like B. **B:** the line stays and runs inside the group, 10 px
   from the group's own cards: nothing that is drawn today disappears, and
   every check keeps a loop round itself.
2. **Dragging a step by hand.** **A (chosen):** the cards hung under
   it move with it, so the group stays one; a hung card dragged alone
   leaves the stack and stays where it is put, until Tidy up. Five more
   review minutes. **B:** as today, only the dragged card moves; a step
   pulled away leaves its checks behind, which is the picture this ticket
   was filed for, until the next Tidy up.

A third form was considered and is not proposed: a short tie drawn between
a step and each card under it. It adds a mark to a board that already has
badges, bands, halos, trays and chips, and says nothing the chip does not.

## What the owner will see

- Changed: after Tidy up, and when a scenario is laid out on opening,
  every hung card is 24 px under the card above it. Values and data
  conditions hung under a step (`first`, `second`, `valid_cart`) follow the
  same rule: they are in the group.
- Changed: lines that join other cards go round a group, never through it.
- (Question 1 = A) Not drawn at rest: the line from a step to a card hung
  under it; a chip at the card's input stands for it and the step's socket
  counts the reader.
- Default behaviour that changes: (question 2 = A) dragging a step takes
  the cards hung under it along.
- Not changed: a board with saved positions is not moved when it is
  opened; a card placed by hand stays where it is.

## Acceptance criteria

1. After Tidy up of each reference scenario, checks drawn, in both *Wires*
   settings, the top of every hung card is exactly `stack_gap` (24 px)
   under the card above it in its stack. Scenarios: `shop-30`, `shop-50`,
   `order-flow`, `fan-out`, `far-value`.
2. On the same boards no piece of a line whose two ends are outside a
   group lies inside the group's box: its cards and the gaps between them.
3. (Question 1 = A) In *Chips for shared values* no line is drawn at rest
   from a card to a card in its column under it in the same row; that
   card's input has a chip naming the value, and the source's socket shows
   the count, `1 reader` for one. Pointing at the chip, and selecting
   either card, draws the line (LAY-23 criteria 3 and 4).
4. In *All wires* a line between two cards of one group may run nearer
   than 20 px to that group's cards; it passes under no card, and it keeps
   20 px from every card and line outside the group (LAY-03).
5. A check with several subjects sits under its latest subject
   (`receipt_total` under `receipt`), and reads the other through a chip
   or a line as the *Wires* setting says.
6. `shop-30`, checks drawn, default view, 1600 × 1000: Fit is not below
   46 % and the crossings are not above 6 (today's figures); the pull
   request prints lines, crossings, coincident pairs and Fit before and
   after for the five scenarios, checks drawn and folded. *Re-checked on
   `main` d480c4b, after LAY-37 (#423) widened the gaps for chips:* Fit is
   43.8 % there and the crossings 6; the bar stays at 46 %.
7. `quality` prints for every board the number of hung cards, the mean and
   the worst gap above one, and the lines of other cards inside a group:
   the measure of LAY-01 that would have caught this.
8. With checks folded nothing changes: LAY-11's tests pass, and *Expand
   checks here* hangs the checks at the same 24 px.
9. A board saved before this change opens with its cards where they were.
10. (Question 2 = A) Dragging `cart` by 200 px moves `cart_created` and
    `cart_total` by the same offset, in one undo step; dragging
    `cart_total` alone moves only it, and it is then drawn as any card
    that is not under its subject.
11. Tidy up of `shop-50` stays within the bench budget of 150 ms.

## Not in scope

- An ordering line that leaves a hung card (`@after(cart_total)`): it is
  the group's own line and leaves through the group's gap, as in the
  mock-up. Language 2 removes `@after` (LANG-13, #386); nothing is designed
  for it here.
- Checks drawn by default and their switch: LAY-30 (#383), next.
- A folded check as a far reader: LAY-35 (#410).
- Where an ordering line leaves a card: dropped from LAY-29 (#382).

## Technical appendix

- `apiforge-layout/src/layered.rs`: `make_room` and `widen` no longer grow
  `Room::stacks`; a breach inside a group's box, or beside a card of the
  line's own group, widens nothing. Without that the mock-up ran to the
  round limit: a line that cannot keep 20 px inside a 24 px gap asks for
  room that no column or row can give.
- `apiforge-layout/src/route.rs` (shared with LAY-29, one agent, in that
  order): the cards of a wire's own group keep it at a short clearance
  instead of `margin`; the gaps of a group are obstacles for every other
  wire on every rung, the last ones included. The group is given by the
  caller (the stacks of `canvas/shape.rs::plan` for a tidied board); the
  mock-up's `stacks()` tells it by geometry, which is also the rule for a
  board placed by hand: two cards one above the other, at most the stack
  gap apart.
- `apiforge-desktop/src/canvas/readers.rs` (question 1 = A): `chipped`
  gains the case "a reader hung under the source"; `resting.rs` follows
  through `chipped`, so Tidy up sizes the board without those lines. The
  count's text for one reader.
- `canvas/mod.rs`, `moved_positions` (question 2 = A): the ids dragged are
  extended by the cards hung under a dragged spine step.
- `collapse::HANG_GAP` and `Rows::stack_gap` are the same 24 px and become
  one constant.

## As implemented

Built on `main` after LAY-29 (#405), LAY-37 (#423), LAY-14 (#420) and
LAY-19 (#422). Where the ground moved since the analysis:

- **The group is told by geometry everywhere** (`apiforge_layout::groups`):
  on a tidied board it is the stack of `shape::plan`, and the router, Tidy
  up, the board and `quality` see the same groups without the plan being
  passed down. `route_grouped` takes them; `route` and `route_among` route
  with every card alone, as before.
- **A wire's own card in a group keeps the short clearance too** (10 px),
  not only the group's other cards: an ordering line that leaves a hung
  card (`@after(coupon_ok)`) starts in the group's 24 px gap, where a
  20 px clearance from its own card left it no way out and sent Tidy up to
  the round limit.
- **`Room::stacks` is gone** and `make_room` no longer takes the folded
  checks: nothing grows above a hung card.
- **`packed_outside_groups`** is the measure of criterion 4; LAY-03's,
  LAY-28's and LAY-29's tests that held every line 20 px apart use it.
- **LAY-37's lanes** (`board_edit::lane_room`): a chip and a count face each
  other across a gap only where they are level. Since a card hung under its
  step now has a chip, every stack has chips low down and counts up at the
  step; counted as facing, they widened every gap by a lane and Fit of
  `shop-30` fell to 44.8 %. With the rule, 47.0 %.
- **Dragging** (`canvas/mod.rs`, `with_hung`): the cards hung under a
  dragged spine step are found where the document places them, folded
  checks included, so a step drags its folded checks too.
- **With checks folded** a folded check keeps its place, which no line is
  kept out of (LAY-11): with every wire drawn, `quality` counts a line of
  other cards through such a place once on `shop-30` (full cards), once on
  `shop-50` (compact) and three times on `shop-50` (full); none with chips.
- ADR 0006 is amended: the group, and the exception to the 20 px rule
  inside it.
