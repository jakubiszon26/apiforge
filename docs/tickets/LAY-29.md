# LAY-29: A wire never runs under its own card

- Size: S–M · Owner review: 15 min · Risk: `visible UI`
- State: implemented, awaiting acceptance (issue #382, pull request #405) ·
  LAY sprint, layout track, ahead of part B
- Analysis model: Fable 5.2 (the owner's rule for wire separation: the
  planning model analyses and develops the fix); narrowed scope implemented
  by Opus 5.5

## Goal

A card is painted over its lines, so a piece of a line inside a card it
joins is not seen: the line seems to come out of the card where there is
no socket. No line is to pass under a card it leaves or enters.

## Scope, narrowed by the owner on 2026-11-17

The analysis found two causes (both on #382):

1. **Cards that lie on each other.** The router avoided a wire's own cards
   only on its first attempts; where they left no way round, its last
   attempts let the wire under them. 71 of 256 wires on generated piles of
   cards ran under a card they join. This is the ticket.
2. **The fan of ordering lines above *Start*.** These lines do not run
   under the card: they begin on its top edge, above the *Next* socket, by
   rule 4 of shape D (LAY-06). The owner chose option A on 2026-11-16 (an
   ordering line leaves its bottom socket and goes round the card), and on
   2026-11-17 withdrew it from this ticket: the fan cannot be written in
   language 2 (LANG-13, #386), and the costs of A were measured on
   fixtures LANG-13 rewrites. Its implementation stays in the history of
   pull request #405 (`3951634`, `1aae304`, `adecdf8`) for a later ticket;
   the [mock-ups](../qa/2026-11-16/LAY-29/) are kept with it.

## What the owner will see

- Changed: where cards lie on each other, a line no longer passes under a
  card it joins. A port the other card lies over is left by a longer stub;
  two cards closer than a stub get a short stub between them and the line
  goes round (the owner's review of the first captures: `begin` into
  `login` must have a real route). Only a line with no way at all would be
  drawn as the plain lighter corner of an enclosed port; none on the
  generated piles.
- Unchanged: a tidied board. No line ran under its own card there, and
  every reference board routes as before.
- Removed or hidden, migrated, default behaviour, file format: nothing.

## Acceptance criteria

1. Router: a wire's own cards are in its way on every attempt. A port that
   lies inside the wire's other card is left through its stub, as one
   inside a foreign card is (ADR 0006); a port facing the other card
   closer than a stub still gets a route. On the generated piles no route
   passes under a card it joins (71 of 256 wires before) and every wire
   has a route.
2. On all 24 reference boards no piece of a line lies inside a card it
   joins (`under_own` is empty), every wire has a route, the gap of LAY-03
   holds (`packed` 0) and no line touches another card (`violations`
   empty). The limits of LAY-06 and LAY-28 are unchanged.
3. The window: on `shop-30` and `fan-out`, tidied and with cards piled on
   each other, no line is under a card it joins and every line has a
   route (`begin` into `login` on `shop-30` included); tidied, the gap
   holds.
4. Review captures at 960 × 540 and 1600 × 1000 of a piled board, before
   and after, each asserting a card in view.
5. The budget of ADR 0006 holds: full routing of its board within 150 ms.

## Measured

- Generated piles of cards (`generated_boards`): 71 of 256 wires ran under
  a card they join; none now, and every wire has a route (3 had none
  before the last attempt was added).
- The 24 reference boards: `under_own` empty before and after;
  `boards.txt` is unchanged, so crossings, bends and Fit are those of
  `main`.
- In the window, `shop-30` with every wire and every third step piled on
  the others: 5 of 42 lines ran under a card they join (`begin` into
  `login`, `order` into `receipt` and `refund`); none now, and every line
  has a route. The line from `begin` into `login` starts 12 px under
  `login`: its stub stops between the two cards and it goes round.
- The budget's board routes in 133 ms (133 ms with the router of `main`,
  budget 150 ms).
- Captures, before and after, `shop-30` piled with every wire:
  [Fit, 1600 × 1000](../qa/2026-11-17/LAY-29/before-piled-1600.png) →
  [after](../qa/2026-11-17/LAY-29/after-piled-1600.png);
  [`order`, `receipt`, `refund` at 100 %](../qa/2026-11-17/LAY-29/before-piled-receipt-1600.png) →
  [after](../qa/2026-11-17/LAY-29/after-piled-receipt-1600.png);
  [`login` at 100 %, 960 × 540](../qa/2026-11-17/LAY-29/before-piled-login-960.png) →
  [after](../qa/2026-11-17/LAY-29/after-piled-login-960.png).

## Not in scope

- Where an ordering line leaves its card (option A above): after LANG-13.
- Highlighting a selected card's lines and ringing their targets: LAY-10
  (#294).
- A step and its checks as one group (LAY-31, #384); checks expanded by
  default (LAY-30, #383).

## Technical appendix

- `apiforge-layout/src/route.rs`: the attempts that let a wire under its
  own cards are gone; `clear_stub` leaves the wire's other card when it
  lies over the port; `violations` counts that stub as the exception;
  a last attempt (`near_stub`) stops the stubs between cards closer than a
  stub and lets the wire run inside a card over both its ports; `under_own` is
  the measure. `canvas/` is unchanged.
- ADR 0006 amended.
- Tests: `crates/apiforge-layout/tests/lay29.rs`,
  `crates/apiforge-layout/tests/generated_boards.rs`
  (`lay29_criterion_1_…`), `folder/acceptance_tests/lay29.rs`.
