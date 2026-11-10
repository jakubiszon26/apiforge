# LAY-21: Every other card is compact too — value, check, Data condition, IF, component call

- Size: S–M · Owner review: 20 min · Risk: `visible UI`, `default`
- State: Backlog (issue #311) · carved out of LAY-06 by the mid-sprint review of
  2026-11-10 ([lay.md 11](../sprints/lay.md#11-mid-sprint-review-2026-11-10),
  finding R2) · LAY sprint, part A, directly after LAY-16 (#310)
- Analysis model: Fable · Implementation model: Opus

## Goal

With *Compact cards* on, every card is as small as what it says. Today only
requests are: on `shop-30` the request `cart` is the smallest card, a value
that says `items[0]` is taller and a check is the tallest, so the board's
hierarchy is upside down. After this ticket a request is the largest thing
on the board and its values and checks are short.

## What the owner will see

At the compact density (LAY-16's switch, on by default), by the rules the
owner took for requests (Q13 = A, Q14 = A, Q16 = A, the tray and the hidden
footer of 2026-11-07):

- **Value**: the header and one line with its source or expression
  (`catalogue.body.items[0]`, `A × B`).
- **Check** (*Comparison*): the header, one line that names subject and
  predicate in words (`cart.status is equal to 201`, LAY-08's wording) and
  the result line.
- ***Data condition***: the header, `all 2 checks`, the result line.
- **IF**: the header, the condition in one line, the *Yes* and *No* sockets.
- **Component call**: as a request — the header with *Component · r1* and
  the *Open* button, one line per configured, wired or missing input, the
  outputs a step reads, the result and the checks lines.
- Hidden ports stand in LAY-16's tray while the card is selected or a wire
  is carried near it. A double-click on a line opens the Inspector on that
  input. No inline field and no footer row (`+`, *Next*) on a compact card;
  right-click and the toolbar keep *Add step*.
- Changed: the height of these cards. Removed or hidden at the compact
  density: their unused rows, inline fields and footer, exactly what LAY-16
  hid on a request. *Compact cards* off restores today's cards. Default
  behaviour that changes: none beyond LAY-16's.

## Decisions

Taken by the owner for the request card and applied here unchanged:
Q13 = A, Q14 = A, Q16 = A (2026-11-05); hidden ports in a tray, the footer
hidden at compact (2026-11-07). The short value and check cards were
approved as direction with round 3 (4.7). The component call and the `if`
were not drawn in a mock-up: they follow the request's rules and are judged
by the owner on this ticket's review captures before the pull request is
marked ready.

## Mock-up

Round 3, `mock/lay-00-round-3` at `ea981d8`: `lay00r3-d-shop-30-60`,
`lay00r3-aggregate-gate` (the 70 px value and check cards, the gate).

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay21.rs`, on
`shop-30.afs` at the compact density.

1. A value card's box is its header and one line; a check's is its header
   and two lines; no value or check card is taller than `cart`. Test:
   `lay21_criterion_1_satellites_are_shorter_than_requests`.
2. `cart_created` reads `cart.status is equal to 201` and `first` reads
   `catalogue.body.items[0]`; no line is anonymous (`{ 2 fields }` without
   its name never appears). Test: `lay21_criterion_2_lines_name_their_source`.
3. Zooming from the board's minimum zoom to 1.0 moves no card of any kind.
   Test: `lay21_criterion_3_zooming_moves_no_card`.
4. A wire dragged from `cart`'s *Response* to within 120 px of a new check
   lands on *Actual value* through the tray; selecting a value card shows
   its hidden output in the tray. Test:
   `lay21_criterion_4_the_tray_reaches_hidden_ports`.
5. A double-click on a check's line opens the Inspector on that input; the
   card has no inline field and no footer; right-click opens *Add step*.
   Test: `lay21_criterion_5_editing_moves_to_the_inspector`.
6. A component call shows one line per configured, wired or missing input
   and keeps *Open*; a missing required input reads *Missing input*. Test:
   `lay21_criterion_6_a_component_call_is_compact`.
7. *Compact cards* off restores today's geometry of every kind. Test:
   `lay21_criterion_7_the_switch_restores_every_kind`.
8. `shop-30` tidied has no card collision; the test prints the board's size
   and fit at 1600 × 1000. Test: `lay21_criterion_8_no_collision`.
9. QA: `review_lay21_board` renders `shop-30` at 60 % and 100 %, at
   1600 × 1000 and 960 × 540, centred on `cart`; the agent opens each
   capture and says in the pull request what it shows.

## Regression potential

LAY-16's tests (`mod lay16`) and the card tests of UX-31/32 (the strip's
ring), UX-34 (paste) and LAY-20 keep passing; the workspace window keeps
full cards.

## Not in scope

The shape (LAY-06); collapsing checks (LAY-11); what each zoom shows and
the minimum sizes (LAY-09); badges (LAY-07); the `Start` card.

## Technical appendix

`canvas/layout.rs` holds every card's rows; LAY-16 added the compact row
kinds and the density for `NodeKind::Request`. This ticket extends the same
row builder to the other kinds; `paint::compact_card` and `paint::trays`
already draw rows and trays from them. The predicate in words is
`inspector/checks.rs`' wording (LAY-08).
