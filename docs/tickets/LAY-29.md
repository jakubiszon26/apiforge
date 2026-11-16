# LAY-29: A wire never runs under its own card

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: implemented, awaiting acceptance (issue #382, pull request #405) ·
  LAY sprint, layout track, ahead of part B
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

**Decided by the owner on 2026-11-16, on the mock-ups: A.** An ordering
line leaves its socket at the bottom (*Next*, *Yes*, *No*) and goes round
the card; rule 4 of shape D changes; the cost on `shop-30` (4 → 6
crossings, Fit 51 % → 49 %, 60 px taller) is accepted. What was put to the
owner follows.

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

- Changed: ordering lines leave *Next*, *Yes* and *No*; the rail above
  a row is reached past the card's side. Tidy up makes the room. A board
  tidied before keeps its positions, and its ordering lines go round
  their cards where the room is.
- Changed: where cards lie on each other, a line no longer passes under
  a card it joins. A port the other card lies over is left by a longer
  stub; a line with no way round at all is drawn as the plain lighter
  corner of an enclosed port (3 of 256 wires on generated piles).
- Removed or hidden, migrated, default behaviour, file format: nothing.

## Mock-up

`docs/qa/2026-11-16/LAY-29/mock-*.png`, from `review_lay29_mock` on
`mock/lay-29` (`APIFORGE_LAY29=a|b`).

## Acceptance criteria

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
   inside a foreign card is (ADR 0006); on the generated piles no route
   passes under a card it joins (71 of 256 wires before).
5. The window draws it: the lines of criterion 1 on the tidied board, a
   bridge at every crossing; Tidy up twice gives the same board.
6. Review captures at 960 × 540 and 1600 × 1000, each asserting a card in
   view.
7. The budget of ADR 0006 holds: full routing of its board within 150 ms.

## Measured

`quality`, compact cards; *before* is `main` at `cf4fcf2`.

| Board, view | Crossings | Bends | Board, px | Fit 1600 × 1000 |
| --- | --- | --- | --- | --- |
| `shop-30`, chips, checks folded (default) | 4 → 6 | 50 → 58 | 1880 × 1470 → 1880 × 1530 | 0.517 → 0.497 |
| `shop-30`, chips, checks drawn | 10 → 12 | — → 104 | 1920 × 1710 → 1900 × 1690 | 0.444 → 0.450 |
| `shop-30`, all wires, checks folded | 75 → 82 | — → 144 | 2020 × 1650 → 2080 × 1710 | 0.461 → 0.444 |
| `shop-30`, all wires, checks drawn | 105 → 106 | 198 → 208 | 2160 × 2010 → 2160 × 1950 | 0.378 → 0.390 |
| `fan-out`, chips, checks drawn | 5 → 5 | — → 76 | 2008 × 1360 → 2008 × 1360 | 0.559 → 0.559 |
| `fan-out`, all wires, checks folded | 42 → 42 | 96 → 112 | 2128 × 1468 → 2128 × 1508 | 0.518 → 0.504 |
| `fan-out`, all wires, checks drawn | 90 → 79 | — → 170 | 2228 × 1640 → 2228 × 1696 | 0.463 → 0.448 |
| `shop-50`, chips, checks drawn | 26 → 19 | — → 176 | 1968 × 3218 → 1948 × 3098 | 0.236 → 0.245 |
| `shop-50`, all wires, checks drawn | 340 → 315 | — → 390 | 2388 × 3690 → 2388 × 3878 | 0.206 → 0.196 |

- `packed`, `close` and `incomplete` are 0 on all 24 boards, as before;
  `under_own` is empty on all of them. Bends before are given where they
  were measured.
- In the window `shop-30` in the default view fits at 49 % (51 %).
- Limits of earlier tickets follow the decision: LAY-06 criterion 4 (an
  ordering line leaves the bottom socket and reaches the rail) and
  criterion 8 (Fit 0.50 → 0.48); LAY-28 criteria 3 and 5 (`shop-30`: 75 →
  82, 105 → 106, 10 → 12 crossings).
- The budget's board routes in 133 ms (132 ms, budget 150 ms); `shop-50`
  with every wire and check in 35 ms.
- Captures: [five steps after *Start*, 960 × 540](../qa/2026-11-16/LAY-29/after-five-960.png),
  [the same with every wire, 1600 × 1000](../qa/2026-11-16/LAY-29/after-five-1600.png),
  [`shop-30` at Fit, 1600 × 1000](../qa/2026-11-16/LAY-29/after-shop-30-1600.png).

## Not in scope

- Highlighting a selected card's lines and ringing their targets: LAY-10
  (#294).
- A step and its checks as one group (LAY-31, #384); checks expanded by
  default (LAY-30, #383); remaining crossings, revisited after LAY-31.
- One successor per step (#388 FLOW-00, #385 LANG-12): the fan from
  *Start* goes away then; this rule stays for every other card.

## Technical appendix

- `canvas/routing.rs`: `rail_anchor` is gone, an ordering wire starts at
  `output_anchor` like any other. `canvas/mod.rs` and `paint.rs` are
  untouched.
- `apiforge-layout/src/route.rs`: the attempts that let a wire under its
  own cards are gone; `clear_stub` leaves the wire's other card when it
  lies over the port; `violations` counts that stub as the exception;
  `under_own` is the measure. `routing_order` is as it was: routing the
  ordering lines first was tried and gives more crossings (8 for 6 on
  `shop-30` in the default view).
- `boards.txt` written again; ADR 0006 amended.
- Tests: `crates/apiforge-layout/tests/lay29.rs`,
  `folder/acceptance_tests/lay29.rs`.
