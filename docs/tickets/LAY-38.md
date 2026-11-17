# LAY-38: A chip's arrow points the way the value flows

- Size: S · Owner review: 20 min · Risk: visible UI
- State: Analysis (mock-up awaiting the owner's choice; variant A recommended)
- Analysis model: Fable · Implementation model: —
- Issue: #427 · After LAY-18, which edits the same file

## Goal
A chip at an input reads the way the value moves: `cart.id →` stands left of
the card and its arrow points into the card. An input chip and an output
count can be told apart at a glance, also where only the arrow is left.

## What the owner will see
- Changed: an input chip says `cart.id →` instead of `← cart.id`; shortened
  it says `cart →` (a variable: `7 →`), then `→`.
- Changed (variant A): an input chip is filled blue-grey with dark text; an
  output count (`→ 5 readers`) stays a white chip with a grey outline, as
  today. Pointed at or selected, both turn to the accent as today.
- Unchanged: where chips stand, their size, the stub to the socket, the
  `+ var` mark, the statement shown while a chip is pointed at, the
  accessible names.
- Removed or hidden: nothing.
- Default behaviour that changes: none.

## Mock-up
Design review captures of `shop-30` after Tidy up, from the branch
`mock/lay-38-chip-direction`, in `docs/qa/2026-11-17/LAY-38/`:

- `lay38-compare-full.png`, `lay38-compare-compact.png`: `cart_view` at
  100 %, enlarged twice: main today, A, B, C side by side.
- `lay38-<main|a|b|c>-<fit|compact-100|full-100>-<1600|960>.png`: the whole
  window at 1600 × 1000 and 960 × 540.

Variants:

- **A (recommended)**: `cart.id →`, input chip filled, output count outlined.
  The fill is the hint that survives every zoom: at Fit an input is a filled
  `→`, a count an outlined `→ 5`. No new colour meaning on the board.
- **B**: the input chip is itself a pointer (a point on the side of the
  card), without the arrow glyph. The narrowest; at Fit it is an empty
  pointer, which reads as a stray bubble.
- **C**: `cart.id →`, input chip tinted blue (outline, text, soft fill).
  Blue already means a link and a request's method on the cards, and the hint
  is carried by colour alone.

Spacing (measured on `shop-30`, 35 chips, both window sizes, Fit, 60 % and
100 %, Compact and Full): in A and C every chip is exactly as wide as today
(the same glyphs in another order; widest 83.5 px at 100 %), no two chips
touch, and the nearest pair stays at the 4 px of LAY-37. B is 1.4 px
narrower at 100 % and keeps more whole texts at 60 %. Tidy up needs no more
room in any variant.

## Acceptance criteria
1. On `shop-30`, the chip at `cart_view`'s URL shows `cart.id →` and the one
   at its Headers `login.token →`; no chip text starts with `←`.
2. Shortened, a chip shows `<step> →` (a variable's chip `<badge> →`), and
   with no room for that `→`: the same steps as today, the arrow last.
3. An input chip at rest is drawn filled (the fill differs from the board
   and from a card) with the text in the card's text colour; an output count
   at rest is drawn white with the grey outline, as today. A test reads the
   two fills from what the board painted.
4. While its wire is drawn (the chip or its input pointed at, the reader or
   the source selected) a chip and a count are in the accent, as today.
5. Every chip on `shop-30` is as wide as before the change at 100 %, 60 % and
   Fit, at 1600 × 1000 and 960 × 540, Compact and Full; LAY-37's tests pass
   unchanged except for the texts they read.
6. The count's text, the `+ var` mark, the statement (LAY-14) and the
   accessible names of the chip and the count are unchanged.

## Not in scope
- The count's text and the field list at a count: LAY-18.
- The proxy chips at the cards' outputs (`↪ cart`) and the badge arrows
  (LAY-24).
- A legend or a setting for the chips' look.

## Technical appendix
- `crates/apiforge-desktop/src/canvas/readers.rs`: the three texts in
  `lay_tags`; `paint` takes which kind it draws (or a `paint_tag` beside
  it); the module's own tests and doc comment.
- `crates/apiforge-desktop/src/canvas/mod.rs`: the call that paints the
  tags; a test accessor for the fill painted, if criterion 3 needs one.
- `crates/apiforge-desktop/src/theme.rs`: one colour for the input fill
  (the mock used 222, 228, 237, the minimap's).
- Tests reading `← `: `folder/acceptance_tests/lay23.rs`, `lay25.rs`,
  `lay37.rs` (ten lines), plus a new `lay38.rs`.
- Docs that quote the old text: `docs/tickets/LAY-15.md`, `LAY-23.md`,
  `LAY-25.md` stay as the record of their tickets.
- Estimate: about 40 lines of source, 80 of tests. Rebase on LAY-18 first.
