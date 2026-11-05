# LAY-16: Compact request geometry — the card's box follows what it shows

- Size: M · Owner review: 25 min · Risk: `visible UI`, `default`
- State: Backlog (issue #300) · decided by the owner on 2026-11-05 (Q13 = A,
  Q14 = A, Q16 = A) · LAY sprint, wave 1, first
- Analysis model: Fable · Implementation model: Opus

## Goal

A request card is as tall as what it says, not as tall as every port it
could have: at the compact density `login` and `cart` are at least 40 %
shorter than today, a 30-step board fits a 1600 × 1000 window at 52 %
instead of 33 %, and zooming never moves a card.

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
   compact density show no card collision and a fit zoom of at least 0.50
   at 1600 × 1000. Test: `lay16_criterion_6_fit_at_least_half`.

## Not in scope

What each zoom level shows (LAY-09, on this geometry); the data-in-use line
(LAY-18); the positions Tidy up produces (LAY-06).

## Technical appendix

`canvas/layout.rs`: `Card::height()` becomes a function of the card's shown
rows at the density (`CardDensity::{Compact, Full}` on the canvas settings),
`WIDTH = 280` kept; `CardDetail::at_zoom` no longer changes the box, only
the paint. The inline field editing in `paint::card` at Full is removed in
favour of the Inspector (UX-32) and a double-click handler. The mock code of
`mock/lay-00-round-4` is not merged.
