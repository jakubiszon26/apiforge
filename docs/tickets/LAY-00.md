# LAY-00: Automatic layout and wire routing — readability analysis

- Date: 2026-10-26 · Analysis model: Fable
- Scope: *Tidy up*, the placement of blocks that code introduces, and the
  wire router (`apiforge-layout`, ADR 0006)
- Goal in the owner's words: fewer crossings, no wires running on top of or
  right next to each other, a board that reads at a glance.

## What was looked at

The demo shop board and a 14-step scenario written for this analysis
([`order-flow.afs`](../qa/2026-10-26/LAY/order-flow.afs)), opened in the folder
window at 1600 × 1000 and tidied; and generated scenario-like boards (a chain
of steps, each input fed by one of the last four steps) measured with
[`measure.rs`](../qa/2026-10-26/LAY/measure.rs), a throwaway example of the
layout crate. Captures are in [`docs/qa/2026-10-26/LAY`](../qa/2026-10-26/LAY).

## Findings

1. **A scenario opened from code lands in one vertical stack.** Blocks that
   have no position are placed one at a time, and a fresh block only looks at
   neighbours that already have a position. When the whole scenario is fresh,
   nobody has one, so every block starts at the origin and the free-slot
   search walks down: one column, cards touching
   ([capture](../qa/2026-10-26/LAY/layout-flow-placed.png)). → LAY-02.
2. **No wire is ever straight.** The layout ignores ports: the desktop never
   passes port offsets, and the crate never reads them. Rows are balanced on
   card centres, so a chain of cards jitters up and down by tens of pixels and
   every wire between neighbours needs two bends; ports that are 5 px apart
   get a 5 px jog. On generated boards 0 of 17 wires are straight. → LAY-04.
3. **Wires are drawn on top of each other.** Nudging only separates segments
   on exactly the same pixel line, orders them along the channel instead of
   across it, and refuses an offset that enters the 12 px clearance around a
   card — so wires hugging a card edge fall back to the same line. 5 coincident
   pairs per 12-card board; 14 with ordering wires
   ([crop](../qa/2026-10-26/LAY/crop-flow-left.png)). → LAY-03.
4. **A port that feeds several cards reads as one wire.** The fan-out shares a
   trunk and splits without any visible lane
   ([crop](../qa/2026-10-26/LAY/crop-demo.png)). Same mechanism as 3.
5. **Long wires pile onto the same rails and force crossings.** Wires are
   routed longest first, pay for crossing only the wires routed before them,
   and share the lines at the clearance of the tallest card. A short wire into
   a card then has to cross the long one that took the lane beside the card.
   0.3 crossings per wire on data-only boards, 1.5 with ordering wires. → LAY-05.
6. **Ordering wires and the horizontal flow contradict each other.** An
   ordering wire leaves the bottom centre and enters the top centre of a card
   to the right: four bends, and it crosses every data wire between the same
   two cards by construction. A 14-step scenario tidies into a 3600 px line
   ([capture](../qa/2026-10-26/LAY/layout-flow-tidied.png)). The remedy changes
   what a tidied board looks like, so it is the owner's choice. → LAY-06.

Side finding, outside this analysis: in the cleanup board a comparison that
reads `removed.status` shows *Missing step → status* although `removed` is the
cleanup's own request.

## Measurements (today)

Ten generated boards per row, averages; `close` is a pair of parallel segments
of different wires within 8 px for at least 20 px, `coincident` within 0.5 px.

| Board | wires | crossings | bends | straight | close | coincident |
| --- | --- | --- | --- | --- | --- | --- |
| 12 cards, data wires only | 17.3 | 5.5 | 39.6 | 0.0 | 2.3 | 4.9 |
| 25 cards, data wires only | 32.7 | 23.1 | 81.8 | 0.4 | 8.7 | 8.5 |
| 12 cards, with ordering chain | 28.3 | 42.4 | 95.4 | 0.0 | 21.8 | 13.8 |
| 25 cards, with ordering chain | 56.7 | 81.2 | 198.8 | 0.0 | 46.9 | 26.3 |

Doubling the channel gap halves `close` and changes nothing else; raising the
crossing cost from 120 to 400 changes nothing. The router's parameters are not
the problem; the layout's ignorance of ports and the nudge's defects are.

## Tickets, in the order to do them

| Ticket | Goal | Size |
| --- | --- | --- |
| [LAY-01](LAY-01.md) | A quality measurement the other tickets prove themselves against | S |
| [LAY-02](LAY-02.md) | A scenario opened from code is placed in layers | S |
| [LAY-03](LAY-03.md) | Wires are never drawn on top of each other | S |
| [LAY-04](LAY-04.md) | Ports decide rows: wires between neighbours run straight | M |
| [LAY-06](LAY-06.md) | The shape of a tidied scenario — owner decision with mock-ups | S · decision |
| [LAY-05](LAY-05.md) | Lanes between columns: a short wire does not cross a long one | M |

LAY-01 to LAY-04 change no design and need no mock-up: the board keeps its
cards and wires, only their geometry improves. LAY-06 changes what the owner
sees and stops for a decision. LAY-05 is measured again after LAY-04 before it
is scheduled; LAY-04 may remove most of what it targets.
