# LAY-16: Compact request geometry — the card's box follows what it shows

- Size: M · Owner review: 25 min · Risk: `visible UI`, `default`
- State: Backlog (issue #300) · decided by the owner on 2026-11-05 (Q13 = A,
  Q14 = A, Q16 = A) · LAY sprint, wave 1, first
- Analysis model: Fable · Implementation model: Opus

## Goal

A request card is as tall as what it says, not as tall as every port it
could have: at the compact density `login` and `cart` are at least 40 %
shorter than today, zooming never moves a card, and the 30-step board's fit
at 1600 × 1000 is measured and reported here; the fit of 50 % or more is
LAY-06's, on this geometry, because the fit is bounded by the board's width.

## What the owner will see

- New: a request card's world box follows the content it shows at the
  board's density — the header, two result lines (execution, checks), the
  ports in use and one summary line per configured input (`Body · 2
  fields`, `Query · 1 parameter`); unconnected output ports are hidden
  (Q13 = A). A configured input never disappears, wired or not.
- New: a board-level *Compact cards* application preference beside the
  *Checks* toggle (Q14 = A); switching it is a deliberate action that Tidy
  up repacks. The compact density is the default.
- The compact box is the card at every zoom (Q16 = A): at Full it writes
  the input values, and editing happens in the Inspector or by double-click
  on the row; the box never changes with the zoom.
- Changed: the request card's height and the inline editing at Full (moved
  to the Inspector and double-click). Removed or hidden: unconnected output
  port rows at the compact density; inline fields on the card. Default
  behaviour that changes: the card geometry and the editing place — the
  owner's decisions Q14 and Q16.

## Decisions taken (owner, 2026-11-05)

> Q13 = A: Compact's content. Q14 = A: a *Compact cards* application
> preference, Tidy up repacks, zooming never moves a card. Q16 = A: the
> compact box at every zoom; editing in the Inspector or by double-click.
> Q16's option C ("compact only as a density the person switches on;
> today's geometry stays the default") was offered and not chosen, so the
> compact density is the default and *Compact cards* off restores today's
> geometry.

## Mock-up

Round 4 proof 1 (`mock/lay-00-round-4` at `df6305b`): `lay00r4-card-44`,
`-46`, `-60`, `-60-today`, `-74`, `-76`, `-90`,
`lay00r4-d-shop-30-compact-fit`, `-compact-60`, `lay00r4-whole-today-60`
and `lay00r4-metrics.txt` (`login` 372 → 166 px, `cart` 424 → 190 px;
`shop-30` 1900 × 1452 world, fit 0.52 at 1600 × 1000). Approved on
2026-11-05. The round-3 rendering hid `catalogue`'s configured query because
"used input" meant "has a wire" (4.7 item 3); criterion 3 forbids that.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/canvas/layout.rs` (`mod tests`) and
`crates/apiforge-desktop/src/folder/acceptance_tests.rs`, `mod lay16`, on
`shop-30.afs`.

1. At the compact density `login` and `cart` are at least 40 % shorter than
   today's `Card::height()`; the metric command prints both heights. Test:
   `lay16_criterion_1_compact_cards_are_shorter`.
2. Zooming from 0.2 to 1.0 moves no card: every world rectangle is equal at
   each zoom; at Full the card writes the input values in the same box.
   Test: `lay16_criterion_2_zooming_moves_no_card`.
3. `catalogue`'s query, configured but not wired, reads `Query · 1
   parameter` at the compact density; no configured input is hidden. Test:
   `lay16_criterion_3_a_configured_input_never_disappears`.
4. Switching *Compact cards* off restores today's geometry after Tidy up
   repacks; the preference is remembered; the switch moves no camera. Test:
   `lay16_criterion_4_the_density_switch_repacks`.
5. Double-clicking an input row at Full opens the Inspector on that input
   (UX-32's form); the card itself has no inline field. Test:
   `lay16_criterion_5_editing_moves_to_the_inspector`.
6. Routing receives the final sizes: the LAY-01 metrics on `shop-30` at the
   compact density show no card collision, and the measured fit zoom at
   1600 × 1000 is reported with the heights. Test:
   `lay16_criterion_6_no_collision_and_the_fit_reported`. Decided on
   2026-11-06: the fit of at least 0.50 moved to LAY-06, because Tidy up on
   `main` gives `shop-30` fifteen columns of 376 px (5544 px wide, fit
   0.205), so no card height can raise the fit; the mock-up's 0.52 came
   from shape D with checks collapsed.

## Not in scope

What each zoom level shows (LAY-09, on this geometry); the data-in-use line
(LAY-18); the positions Tidy up produces and the fit zoom of at least 0.50 at
1600 × 1000 (LAY-06, with LAY-11's collapse).

## Analysis: the ports a compact card hides (2026-11-06, open)

The approved rendering also leaves out the unconfigured input rows (the
drop targets for wires and variables), the `+` button and the *Next* row.
Facts, run in the harness on `main` (`mock/lay-16-inputs` at `f11ff31`,
`docs/qa/2026-11-06/LAY-16/lay16-facts.txt`):

- Today a request offers, at every zoom, four output sockets (Response,
  HTTP status, Headers, Duration) and one input socket per row (URL, Body;
  Headers and Query once configured; each named argument). The `+` button
  and *Next* exist only at Full (≥ 0.75): at 0.6 they are already absent,
  and right-click opens the same *Add step* menu. A wire from Response to a
  Body row binds it; a wire from the square socket to a card's top adds the
  order edge; the Inspector connects both (*Body value source*, *Execution
  order*) and is the only keyboard route — no key starts a wire. A variable
  from the library drops on an input row (read, not run).
- At the compact density on `main`: `login` 372 → 190, `cart` 424 → 214,
  a new request 372 → 94 (A, B, D) or 142 (C); the box is the same at every
  zoom under all four. A new request has no output socket and no Body row;
  its URL socket stays on the subtitle row, and under B a wire aimed at the
  card's edge snapped to it and silently replaced the URL by `cart.body`.
- Under every option `+` and *Next* leave the card (Q16 = A already removed
  them at Full); right-click, the order-socket drag to empty board and the
  Inspector keep the function; the keyboard loses its only on-card button
  (the toolbar's *Add step* and the Inspector remain). A configured input
  never disappears in any option; *Compact cards* off restores today.

Options, each rendered at rest, selected and during a wire drag, beside
`cart`, at 1600 × 1000 and 960 × 540 (`lay16-<option>-<state>-<width>.png`
and `-cards.png` crops; today's card in `lay16-today-*`):

- **A. Edge sockets**: the hidden sockets on the card's edge during a drag
  and on selection, labelled inside the box. The drag binds Body; on a 94 px
  card the four output labels fall on the rows (`Respo…`, `HTTP sta…`) and a
  component call with six inputs does not fit. Works, not readable at 0.6.
- **B. Nothing**: wiring a hidden port needs *Compact cards* off or the
  Inspector; the most natural drag on a fresh request rebinds its URL.
- **C. Rows kept**: every input row and the first output row stay: a new
  request reads `Body · optional`, `Response →` (142 px); `login` and `cart`
  are unchanged, so criterion 1 holds; GET requests without a body gain
  24 px; HTTP status, Headers and Duration still need the Inspector; LAY-14
  keeps a row to hover and LAY-18 a line to replace.
- **D. Tray** (new): a labelled tray outside the box — the hidden inputs on
  the left while a wire is dragged within 120 px of the card, inputs and
  outputs when the card is selected. The drag binds Body; the tray is view
  state, so the box, routing, Tidy up and LAY-06 are untouched; it takes 88
  of the 96 px column gap. LAY-14 and LAY-18 see only the shown rows.

For every option: the URL socket on the subtitle row must take a wire only
when the pointer is on it, never by the 20 px snap, or a drop meant for the
card rewrites its address. The decision is the owner's; see issue #300.

## Technical appendix

`canvas/layout.rs`: `Card::height()` becomes a function of the card's shown
rows at the density (`CardDensity::{Compact, Full}` on the canvas settings),
`WIDTH = 280` kept; `CardDetail::at_zoom` no longer changes the box, only
the paint. The inline field editing in `paint::card` at Full is removed in
favour of the Inspector (UX-32) and a double-click handler. The mock code of
`mock/lay-00-round-4` is not merged.
