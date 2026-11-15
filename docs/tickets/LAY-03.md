# LAY-03: Lines are always far enough apart to trace

- Size: M · Owner review: 30 min · Risk: `visible UI`, `default` (what Tidy
  up leaves between cards)
- State: Re-scoped by the owner 2026-11-15 (issue #34) · implemented,
  awaiting acceptance · LAY sprint, part A, may start before the checkpoint
  (#296)
- Analysis and implementation model: Fable (alias; version not confirmed —
  the owner's exception to the model policy for this ticket)

## Goal

The owner's finding of 2026-11-15, on his own board: dashed `@after` rails
and wires run 2–4 px apart or merge into one thick band — along the top of
a row, where a vertical and a horizontal rail turn the same corner, under
the last row, and where three or four lines arrive together. *This must
never happen.* Every line is far enough from every other line and from
every card to be followed with the eye; the good example is a rail about
20 px from a card's edge. A sprawling board that needs scrolling is better
than packed lines.

It replaces the earlier scope (coincident pairs counted, capped at 30 in
*Chips* mode): the rule is now a hard minimum and the count is zero.

## What the board does today

`main` at `40239e9`, reference scenarios tidied, 1600 × 1000, compact
cards, checks folded. The captures are in
[`docs/qa/2026-11-15/LAY-03/`](../qa/2026-11-15/LAY-03/), each opened and
read; the owner's own captures are of a private project and are not here.
Pairs of parallel segments of two lines nearer than 12 px for 20 px or more:

| Board | Lines | Pairs under 12 px | Least gap |
| --- | --- | --- | --- |
| `fan-out`, chips (the default) | 19 | 17 | 0 px |
| `fan-out`, all wires | 31 | 121 | 0 px |
| `shop-30`, chips | 22 | 15 | 0 px |
| `shop-30`, all wires | 42 | 98 | 0 px |
| `shop-50`, chips | 32 | 24 | 0 px |
| `far-value`, chips | 16 | 8 | 0 px |

The four situations of the finding, each on a fixture:

1. **A rail on the card tops and on another rail** — `fan-out`, rows 2 and
   3: the rails of a row share one level 12 px above the cards, 0–4 px
   from each other, and their labels are struck through
   ([1600](../qa/2026-11-15/LAY-03/before-1-row-top-1600.png),
   [960](../qa/2026-11-15/LAY-03/before-1-row-top-960.png)).
2. **A vertical and a horizontal rail doubled round a corner** —
   `fan-out`, right of `apply_coupon`: `quantity → apply_coupon` and
   `remove_coupon → view` come down one corridor 4 px apart and turn
   together
   ([1600](../qa/2026-11-15/LAY-03/before-2-corner-1600.png),
   [960](../qa/2026-11-15/LAY-03/before-2-corner-960.png)).
3. **A band under the last row** — `shop-30`, all wires: the wires of
   `order`'s readers run 0–9 px apart for 680–1430 px under the third row
   ([1600](../qa/2026-11-15/LAY-03/before-3-under-last-row-1600.png),
   [960](../qa/2026-11-15/LAY-03/before-3-under-last-row-960.png)).
4. **A corner where three or four lines arrive** — `shop-30`, all wires,
   between the first two rows: lines 0–6 px apart turn in one place
   ([1600](../qa/2026-11-15/LAY-03/before-4-many-arrive-1600.png),
   [960](../qa/2026-11-15/LAY-03/before-4-many-arrive-960.png)).

One cause: the router gives every line the same few places — the 12 px
edge of each card — and knows nothing of the lines already there. A second
pass then moves lines that landed on one place 6 px apart, only where that
is safe, and lines 3 px apart count as two places. Nothing makes room.

## What the owner will see

- **The gap.** 20 px on the board (it scales with zoom): between any two
  lines running the same way, between two lines end to end on one level,
  and between a line and the edge of any card or chip, its own cards
  included. Rails and wires alike. Lines still cross, at right angles, with
  the bridge.
- **At a port.** The first 20 px of a line, straight out of its port, are
  as close to the neighbouring ports' lines as the ports are. Wires of one
  port leave on one line and turn off it one by one, 20 px apart, like the
  teeth of a comb: one line per reader from the turn onwards. A rail into
  a step and the rail out of it share the short piece above the card. The
  three inputs of a *Data condition* are 6 px apart: the straight pieces
  into them are that near, and the lines part where they turn.
- **Tidy up makes the room.** It widens the gap between two columns,
  between two rows, or between a card and the card hung under it, in steps
  of 20 px, only where the lines the board draws at that moment need it.
  The board grows and Fit drops ([Measured](#measured)).
- Changed: after Tidy up the gaps are 100 px between columns and rows
  (96 today) and wider where lines need it. Showing folded checks or
  switching to *All wires* moves no card, as today; the added lines keep
  the gap by going the long way round where the board has no room, and a
  second Tidy up makes it (LAY-11 promised that no second Tidy up was
  needed; that held for the cards and no longer holds for the room).
- Changed: a board tidied before this change keeps its positions. Its
  lines keep the gap too, by longer routes; where an old gap is too narrow
  for the lines that have to pass it they still run through it. Tidy up
  widens it.
- Changed: lines that lay on each other now run side by side, and a line
  leaving the others crosses them. In the default view the crossings stay
  as they were; with every check or every wire drawn there are more
  ([Measured](#measured)). The order of lines in a corridor is LAY-05.
- Removed or hidden: nothing. Migrated: nothing; no file format changes.
- Default behaviour that changes: the spacing Tidy up gives, as above.

## Mock-up

Not applicable: no new element. Before and after on the four situations,
at 1600 × 1000 and 960 × 540, are the QA captures of the pull request.

## Acceptance criteria

1. **The rule.** After Tidy up, on every reference board (`order-flow`,
   `shop-30`, `shop-50`, `fan-out`) at either density, checks folded or
   drawn, *All wires* or *Chips*: no two parallel segments of the lines
   drawn are nearer than 20 px, side by side or end to end; no segment is
   nearer than 20 px to a card or chip. Not counted: the first and last
   20 px of a line at its ports, and the shared straight piece out of (or
   into) one port.
2. **No shared track.** No two lines lie on each other anywhere, except
   wires of one port on that port's straight piece. On `order-flow` with
   every wire drawn, the two wires from `products` are two lines from
   their turns onwards; the demo shop board keeps the rule too.
3. **The metric.** `quality` prints `packed`, the number of breaches of
   criterion 1 on a board; it is 0 on all 24 reference rows. `close` and
   `coincident` stay for comparison.
4. **Tidy up widens.** On `fan-out` in *All wires* a gap between two
   columns is wider than 100 px and a gap no line needs stays 100 px; the
   same Tidy up twice moves nothing; opening a scenario from code gives
   the board Tidy up gives.
5. **Without Tidy up.** On a board left as an earlier Tidy up placed it
   (96 px gaps, 24 px under a card) the lines keep criterion 1 wherever
   the board has room, by longer routes. Where it has none — four wires of
   one port through a gap that holds three lines, two cards 24 px apart
   joined bottom to top — a line runs within the gap rather than not at
   all: at most 5 such places on the old `order-flow` (107 before) and 3
   on the old `fan-out` (92).
6. **Lines brought back.** A wire drawn while its chip is pointed at or
   its card is selected keeps the gap from the lines at rest and from
   cards, like any other.
7. **The contract of ADR 0006 holds**: no line enters a card; routing is
   deterministic; the measured cost of a full routing is recorded in the
   ADR.
8. Review captures of the four situations before and after, at both
   sizes, each asserting a card in view.

## Measured

`quality` on the boards the desktop tidies, compact cards, before (`main`
at `40239e9`) → after. `packed` is 0 on all 24 rows, the full density
included; the default view is the first four rows.

| Board, view | `packed` | Crossings | Board, px | Fit at 1600 × 1000 | at 960 × 540 |
| --- | --- | --- | --- | --- | --- |
| `order-flow`, chips, checks folded | 69 → 0 | 4 → 7 | 1888 × 758 → 1904 × 846 | 0.60 → 0.60 | 0.26 → 0.26 |
| `shop-30`, chips, checks folded | 89 → 0 | 3 → 5 | 1864 × 1478 → 1880 × 1502 | 0.51 → 0.51 | 0.20 → 0.20 |
| `shop-50`, chips, checks folded | 156 → 0 | 9 → 7 | 1912 × 2742 → 1928 × 2814 | 0.28 → 0.27 | 0.11 → 0.11 |
| `fan-out`, chips, checks folded | 92 → 0 | 9 → 5 | 1912 × 1236 → 1928 × 1280 | 0.59 → 0.59 | 0.24 → 0.23 |
| `shop-30`, chips, checks drawn | 196 → 0 | 9 → 18 | 1864 × 1550 → 1900 × 1714 | 0.49 → 0.44 | 0.19 → 0.18 |
| `fan-out`, chips, checks drawn | 222 → 0 | 16 → 35 | 1912 × 1236 → 2048 × 1432 | 0.59 → 0.53 | 0.24 → 0.21 |
| `shop-30`, all wires, checks folded | 273 → 0 | 76 → 79 | 1864 × 1478 → 2020 × 1642 | 0.51 → 0.46 | 0.20 → 0.18 |
| `fan-out`, all wires, checks folded | 265 → 0 | 50 → 64 | 1912 × 1236 → 2128 × 1440 | 0.59 → 0.53 | 0.24 → 0.21 |
| `shop-30`, all wires, checks drawn | 494 → 0 | 112 → 118 | 1864 × 1550 → 2140 × 2014 | 0.49 → 0.38 | 0.19 → 0.15 |
| `shop-50`, all wires, checks drawn | 1142 → 0 | 363 → 408 | 1912 × 2862 → 2388 × 3638 | 0.27 → 0.21 | 0.11 → 0.08 |

- `shop-30` as a new install shows it still fits 1600 × 1000 at 0.50
  (LAY-06 criterion 8), now measured with chips, the default since LAY-23;
  with every wire drawn it fits at 0.46.
- Four assertions of earlier tickets follow the new spacing and are
  changed with their reasons in place: LAY-06 criteria 1 and 2 (the gap
  between columns and under a row is 100 px or more, in steps of 20, not
  96), LAY-06 criterion 11 (`shop-30`, every wire and check: 2.07
  crossings per wire, 1.96 before, limit 2.05 → 2.1), LAY-23 criterion 7
  (`fan-out` with chips and checks drawn: 1.09 crossings per line, 0.5
  before, limit 0.6 → 1.1), LAY-11 criterion 9 and LAY-23 criterion 8
  (Tidy up after showing checks or switching *Wires* keeps every card's
  column and place and changes the gaps).
- A full routing of the benchmark board of ADR 0006 (200 cards, 398
  wires) takes 118 ms (10 ms before, budget 30 ms → 150 ms, amended in
  the ADR); `shop-50` with every wire and check routes in 30 ms.

## Not in scope

- The order of lines across a channel to save crossings (LAY-05).
- LAY-24's mark for the rail into the next step: with it there are fewer
  rails to make room for; the rule holds with and without it.
- Chips at readers, continuation marks and rail labels are drawn in the
  gaps where lines run and are not obstacles; a label on a rail still sits
  on its rail.
- Ports of one card nearer than 20 px (two operands of a value): their
  first 20 px run as close as the ports are; moving ports is card design.
- Tidy up of a selection, of a board without a spine step and of the
  workspace window's board widen nothing yet; their lines keep the gap by
  longer routes.
- A wire being dragged with its card is routed alone until the card is
  dropped, as before.

## Technical appendix

- `route` (`crates/apiforge-layout/src/route.rs`): `Routing::margin`,
  `stub` and `channel_gap` are one gap of 20 (`GAP`). The search grid
  gains a track every 20 px; running the same way within the gap of a
  segment already routed costs 200 times its length (crossing it stays
  free), except for a wire of the same port, or of a port nearer than the
  gap, on its own port's line; the stub end of every port is kept free of
  lines running into it. `nudge` is gone. Rails are routed first, then the
  shorter wires first. `route_among` routes wires among lines that stay.
- `packed` (`crates/apiforge-layout/src/packed.rs`): the breaches of
  criterion 1, used by `quality`, by the tests and by Tidy up.
- Tidy up (`board_edit.rs::tidy_in_rows`, `layered::{hang_with, make_room,
  widen, Room}`): each wire of the board as drawn
  (`canvas/resting.rs`) is routed alone to see which corridor it wants,
  and a corridor several want gets room for all; then the board is routed
  as drawn and each breach left widens the gap it lies in, until none.
  The lines drawn come from *Wires* and *Collapse checks*, which the
  window shares with the placer that lays out a scenario opened from code.
- A reader is *beside* its source (LAY-23) by its column's place in the
  row, not by a distance of 376 px, since the gap between columns varies.
- Lines brought back are routed among the lines at rest
  (`canvas/routing.rs`, `Request::among`), which stay as they are.
- Painting is untouched, so LAY-24 (#343) merges on top: its rails not
  drawn are not routed. `resting` still counts them until LAY-24 filters
  them there too, which leaves room that is not needed, never too little.
