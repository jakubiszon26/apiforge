# LAY-29: A wire never runs under its own card

- Size: M · Owner review: 20 min (option A) or S–M · 15 min (option B) ·
  Risk: `visible UI`
- State: Analysis, one owner decision open (issue #382) · LAY sprint,
  layout track, ahead of part B
- Analysis and implementation model: Fable 5.2 (the owner's rule for wire
  separation: the planning model analyses and develops the fix)

## Goal

On a scenario where several steps are ordered after *Start*, the dashed
ordering lines appear above the card as if from nowhere. Every line is to
leave a socket of its card, outwards, and be seen from the socket on.

## What was found

The lines do not run under the card. They **start on its top edge**, above
the *Next* socket, where there is no socket: that is rule 4 of shape D
(LAY-06, [ADR 0006](../decisions/0006-layout-and-wire-routing.md)), "an
ordering wire leaves the top of its source, above its control socket", so
that it rises straight to the rail above the row. The card is drawn over
the place where the line begins, which reads as a line coming out from
under it. Reproduced on `main` at `cf4fcf2`:
[five steps after *Start*](../qa/2026-11-16/LAY-29/mock-main-five-960.png);
the same on `shop-30` at *cart* and at the *Yes* of *paid*
([Fit, 1600 × 1000](../qa/2026-11-16/LAY-29/mock-main-shop-30-1600.png)).

The cause named at triage, the router giving up its own cards on its last
attempts, is real but is not what the owner saw: on the 24 reference
boards and on 60 generated tidied boards no piece of a line lies inside a
card it joins. It happens only where cards lie on each other (71 of 256
wires on the generated piles), and is the second part of this ticket.

## The decision: where an ordering line leaves its card

Two designs look different, so the owner chooses. Both rendered by the
throwaway branch `mock/lay-29`, tidied, default view.

| | A: from the socket at the bottom, round the card | B: a socket on the top edge, where the rail leaves |
| --- | --- | --- |
| Five steps after *Start*, 960 × 540 | [capture](../qa/2026-11-16/LAY-29/mock-a-five-960.png), [Fit at 1600](../qa/2026-11-16/LAY-29/mock-a-five-1600.png) | [capture](../qa/2026-11-16/LAY-29/mock-b-five-960.png) |
| An `if` (*paid*), 960 × 540 | [capture](../qa/2026-11-16/LAY-29/mock-a-if-960.png) | [capture](../qa/2026-11-16/LAY-29/mock-b-if-960.png) |
| `shop-30` at Fit, 1600 × 1000 | [capture](../qa/2026-11-16/LAY-29/mock-a-shop-30-1600.png) | as [`main`](../qa/2026-11-16/LAY-29/mock-main-shop-30-1600.png) plus the sockets |
| What a card says | Order comes in at the top and goes out at *Next*, *Yes*, *No*: every line starts at a named socket | The top edge has a socket in and a socket out; the one out has no name, and on a card with one exit it sits where the one in does unless it is moved aside |
| What moves | Lines go down, past the card and up; a row is 20 to 60 px taller; lines round an `if` | Nothing |

Measured on the mock-up (`main` and B are the same routes):

| Board, view | Crossings | Bends | Board, px | Fit 1600 × 1000 |
| --- | --- | --- | --- | --- |
| Five after *Start*, default | 0 → 0 | 6 → 12 | 1800 × 306 → 1800 × 326 | 60 % → 60 % |
| `shop-30`, default (chips, checks folded) | 4 → 6 | 50 → 58 | 1800 × 1470 → 1800 × 1530 | 51 % → 49 % |
| `shop-30`, all wires, checks drawn | 105 → 106 | 198 → 208 | — | 38 % → 39 % |
| `fan-out`, all wires, checks folded | 42 → 42 | 96 → 112 | — | 52 % → 50 % |

**Recommended: A.** It is what the note asks in its own words ("leaves a
port outwards and is visible from the port on"), and it gives the card one
grammar: in at the top, out at the bottom, by name. The price is two more
crossings and 2 % of Fit on `shop-30`, which brings the default view to
49 % against the sprint's 50 %, and lines that go round an `if`. B is the
cheap answer: nothing moves, but the line still starts away from *Next*.
A changes rule 4 of shape D, which the owner decided in LAY-06; B keeps
it. The mock-up of A is the router as it is, given the other port; the
implementation may do better than its numbers, not worse.

## What the owner will see

- Changed (A): ordering lines leave *Next*, *Yes* and *No*; the rail above
  a row is reached past the card's side. Tidy up makes the room.
- Changed (B): a square socket on the top edge of a card where a rail
  leaves it.
- Changed (both): where cards lie on each other, a line no longer passes
  under the card it joins when a way round exists.
- Removed or hidden, migrated, default behaviour, file format: nothing.

## Mock-up

`docs/qa/2026-11-16/LAY-29/mock-*.png`, from `review_lay29_mock` on
`mock/lay-29` (`APIFORGE_LAY29=a|b`).

## Acceptance criteria

Written for A; with B, criteria 1 and 2 read "starts at a socket the card
draws" and criterion 3 keeps the limits of `main`.

1. Five steps ordered after *Start*, tidied: every ordering line of
   *Start* begins at its *Next* socket on the bottom edge and leaves
   downwards; none has a point on the top edge of *Start*.
2. On all 24 reference boards every line begins and ends at a socket its
   card draws, and no piece of a line lies inside a card it joins
   (`under_own` is empty).
3. The gap of LAY-03 holds (`packed` 0 on all 24) and no line touches
   another card (`violations` empty); `shop-30` in the default view has at
   most 6 crossings and fits 1600 × 1000 at 48 % or more; with every wire
   and check drawn at most 106 crossings.
4. Router: a wire's own cards are in its way on every attempt. A port
   that lies inside the wire's other card is left through its stub, as one
   inside a foreign card is (ADR 0006); on the generated piles fewer than
   71 of 256 wires pass under a card they join, and none where its two
   cards overlap no other.
5. The window draws it: the lines of criterion 1 on the tidied board, a
   bridge at every crossing; Tidy up twice gives the same board.
6. Review captures at 960 × 540 and 1600 × 1000, each asserting a card in
   view.
7. The budget of ADR 0006 holds: full routing of its board within 150 ms.

## Not in scope

- Highlighting a selected card's lines and ringing their targets: LAY-10
  (#294).
- A step and its checks as one group (LAY-31, #384); checks expanded by
  default (LAY-30, #383); remaining crossings, revisited after LAY-31.
- One successor per step (#388 FLOW-00, #385 LANG-12): the fan from
  *Start* goes away then; this rule stays for every other card.

## Technical appendix

- A: `canvas/routing.rs` (`rail_anchor` goes: an ordering wire starts at
  `output_anchor`); `apiforge-layout/src/route.rs` (`routing_order` knows
  a rail by its two top ports today; the fallback rungs; `clear_stub` for
  a port inside the wire's other card; `under_own`, the measure);
  `layered.rs` only if Tidy up's room under a row needs it; the rail
  tests of LAY-06 and the rail labels of LAY-24; `boards.txt` written
  again; ADR 0006 amended (rule 4 of shape D, the guarantee).
- B: the socket in `canvas/paint.rs`, which LAY-10 is changing: after
  LAY-10 is merged. The router part as in A.
- Tests: `crates/apiforge-layout/tests/lay29.rs`,
  `folder/acceptance_tests/lay29.rs`.
