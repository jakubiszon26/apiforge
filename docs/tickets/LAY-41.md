# LAY-41: At the Overview zoom a card says what the step is

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: **mock-up waits for the owner.** Decided in principle on
  2026-11-18 (Q3 = A, #296): one line — method and path, call, or
  condition — at a fixed small size, the card's size unchanged. Not ready ·
  LAY sprint, part of its close, the cards track (after LAY-30)
- Analysis model: Fable · Implementation model: —

## Goal

A scenario opens fitted to the window, and at 1280 × 800 that is 36–45 %:
every card is a blank box with a name and a number. The first view should
say what the flow is — which requests, which call, which condition — before
anything is zoomed. Each card carries one line saying what the step is.

## What the board does today

`main` 2f6922c, `double-booking` first opened at 1280 × 800: Fit is 36 %
with the library open and 45 % with it folded, both below the 45 % level
where rows are written (LAY-09). A card shows its band, name, badge and the
checks pill; `first` and `second`, both `POST /bookings`, differ only by
name. The readable whole board is three actions away (fold the library,
zoom one step, pan).

## What the owner will see

- New, below 45 % (the Overview level) and from 25 %: under the name, one
  line in the small code face at **9 px on screen at every zoom**:
  - a request: method and path, `POST /bookings`, `GET /rooms/{{ro…`;
  - a component call: the component and its revision, `sign_in · r1`;
  - an `if`, a check or a *Data condition* drawn as a card: its condition,
    `first.status is equal to 201`;
  - a value: its source or content, `{ start: "2026-12-01…`.

  The line is cut with `…` at the card's edge. The checks pill moves under
  it.
- Where the card is too low for the line (a one-row value card at 36 %),
  the line is not drawn; where the pill then has no room for its text, it
  falls back as today (the numbers on the name's row, LAY-27).
- Unchanged: every card's box, so Tidy up, Fit, the routes, the chips and
  the minimap; the name's size; everything from 45 % up; everything below
  25 %; failed and running signals.
- This amends LAY-09's table of what each zoom shows (`lay.md`).
- Removed or hidden, migrated, default behaviour: nothing.

## Mock-up

Throwaway branch `mock/lay-close` (`review_laymock_overview`), the room
project at Fit, 1280 × 800, in
[`docs/qa/2026-11-18/LAY-41/`](../qa/2026-11-18/LAY-41/):

| View | Capture |
| --- | --- |
| `double-booking` as it first opens, library open, 36 %: `sign_in · r1`, `GET /rooms`, `POST /bookings` twice, `first.status is…`, `GET /rooms/{{ro…`; `slot` and `begin` have no line | [lay41-overview-first-open](../qa/2026-11-18/LAY-41/lay41-overview-first-open.png) |
| The same with the library folded, 45 % on the label and still Overview: `slot` now has room for `{ start: "2026-12-01…` | [lay41-overview-library-folded](../qa/2026-11-18/LAY-41/lay41-overview-library-folded.png) |
| `booking-day`, 29 steps, 36 %: sixteen requests named by method and path | [lay41-overview-booking-day](../qa/2026-11-18/LAY-41/lay41-overview-booking-day.png) |

The captures also carry LAY-30's pill (`9 · status == 409`) and bar
button; they are not part of this ticket. The mock-up writes the
component's name statically.

## Questions for the owner

1. **The line as drawn** — 9 px, code face, under the name, above the
   pill. **A (recommended):** as drawn. **B:** the same in the interface
   face (narrower letters, a few more characters before `…`, method and
   path less distinct).
2. **Which cards get it.** **A (recommended):** requests, calls and
   conditions, and a value where its card has the room (as drawn).
   **B:** requests, calls and conditions only; a value stays its name.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay41.rs` on
a temporary copy of `fixtures/scenarios/lay/rooms/`, first open, Fit,
1280 × 800.

1. On `double-booking` at Fit the cards `rooms`, `first`, `second` and
   `calendar` each draw a line starting `GET /rooms`, `POST /bookings`,
   `POST /bookings`, `GET /rooms/`; `token` draws `sign_in · r1`; `booked`
   draws a line starting `first.status`.
2. The line's text is 9 px on screen at 25 %, 36 % and 44 %; at 45 % and
   above it is not drawn (the rows are); below 25 % it is not drawn.
3. The line lies inside its card, under the name and above the pill; a
   text wider than the card ends with `…`; it overlaps no badge, socket
   mark or pill. A card too low for it draws none and loses nothing it
   shows today.
4. No card's box, no route, no chip's or count's rectangle and no Fit zoom
   differs from `main` on the three room scenarios and on `shop-30` at
   1280 × 800 and 960 × 540 (compared in the test before and after the
   line is switched on by a test seam, or by LAY-01's metrics).
5. LAY-09's criteria hold: a name at least 11 px, no text smaller when
   zooming in across 45 %; a failed card is red at 35 %.
6. On `booking-day` at Fit every request card draws its method and path.

## Regression potential

The Overview painting of LAY-09, LAY-19 (the count under the name) and
LAY-27 (the numbers on the name's row) share the space under the name;
their tests (`lay09`, `lay19`, `lay27`) run unchanged and criterion 3
covers the order: name, line, pill, count.

## Not in scope

- Where Fit stops (36 % against the 45 % level; re-measured after
  LANG-13, `lay.md` section 13). The first view's mode (LAY-40). The
  folded checks line (LAY-30).

## Technical appendix

- `canvas/paint.rs`, the compact card at `CardDetail::Overview`: the line
  is the card's first row of role `Subtitle`, `Caption` or `Line`
  (`layout::Compact`), drawn between the name and `Pill::layout`; the
  other Overview tile (the full density) gets the same. A component
  call's line names the component's handle, which the card does not hold
  today (`layout.rs` builds `caption` as `Component · r1`).
