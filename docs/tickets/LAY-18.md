# LAY-18: A reader count names the fields read — `login.body.token → 10 readers`

- Size: S · Owner review: 10 min · Risk: `visible UI`
- State: Analysis, re-checked against `main` at 514f06a on 2026-11-17.
  The owner chose the shape and approved the mock-up the same day
  (decisions 1, 2 and 4); decision 3 waits for him with its own mock-up
  (issue #302, `decision-needed`). Was M · 20 min, *Compact shows the data
  in use*: LAY-16, LAY-23, LAY-32, LAY-27/31 and LAY-14 delivered most of
  it. Implementation after LAY-37 (#423).
- Analysis model: Fable · Implementation model: Opus

## Goal

A value that feeds half the scenario is found from its source (W11): the
count beside a widely read output says which fields are read and by whom,
`login.body.token → 10 readers`, `login.body.userId → 1 check`.

## Already on main

Seen on `shop-30`, Compact, checks folded, at 1600 × 1000 and 960 × 540
(captures `lay18-*-before-*`):

- An output read by three or more steps shows its reader count in the gap
  beside the port (`→ 11`, `→ 11 readers` where there is room) and no line
  leaves it (LAY-23, LAY-32). This is the ticket's line, at the port
  instead of on the card, and the card's box never changes with it.
- A click on the count draws every wire of the port until Esc, a click on
  the board or a second click on the count.
- An output read by one or two steps draws its lines; pointing at a line or
  at an input row states the field (LAY-14).
- A compact request card lists only the outputs something reads: `address`,
  which nothing reads, has no output row (LAY-16).
- A step's checks are counted on its card (`3–4 · 2 checks`, LAY-27).

What `main` does not say: **which field**. `login`'s count says `→ 11` at
*Response*; that ten steps read `token` and one check reads `userId` is
found only by clicking and following eleven wires. Pointing at the count
shows the sentence *Draw every wire of this value*.

## What the owner will see

- Changed: pointing at a reader count shows, in the box a chip and a line
  have (LAY-14), one line per field read, most read first, each with the
  number of its readers and their names: `login.body.token → 10 readers:
  catalogue, cart, cart_view, address, checkout, order, +4 more` and
  `login.body.userId → 1 check: known_user`. A field read only by checks
  says *check* or *checks*. At most six readers are named.
- Changed: a click on the count, which already draws every wire of the
  port, also keeps the list until the wires go (Esc, a click on the empty
  board, a second click on the count).
- Removed or hidden: the tooltip sentence *Draw every wire of this value*
  at a count; the list stands in its place. Default behaviour that changes:
  none. Nothing on a card changes; the port model and the project format
  are unchanged.

## Decisions

Taken by the owner on 2026-11-17 (quiz on the mock-up at 9409004):

1. **The shape: A**, the list at the count, shown by pointing and kept by
   a click. Not B (one count per field in the gap), not C (close as
   delivered).
2. **No** grey line *Nothing reads this step*.
4. **The mock-up is approved** for the above.

Open, recommended option first (captures `lay18-value-*`, `shop-30` at
Full after a simulation):

3. **The last run's value.** **A** in the list: after a run each line names
   the field's value, `cart.body.id = "c-1" → 3 readers: …`, at either
   density; nothing is added at rest. A secret stays as the run report has
   it (`login.body.token = "[REDACTED]"`). · **B** beside the count at
   Full, the most read field's value (`→ 5 · id "c-1"`): always visible,
   but the gap holds about eight characters, so `login`'s count stays
   `→ 11 readers` and `catalogue`'s reads `→ 3 · [{"id":1…`; it needs the
   gap LAY-37 gives the chips. · **C** not shown: Run results and the
   Inspector have it. With A the review grows to 15 minutes.

## Mock-up

Not yet approved. Design-review captures of `review_lay18` on
`mock/lay-18-data-in-use`, `shop-30` tidied, Compact, checks folded, in
[docs/qa/2026-11-17/LAY-18](../qa/2026-11-17/LAY-18):

| View | Main | A | B |
| --- | --- | --- | --- |
| `login` at 60 %, at rest | `lay18-login-rest-before-1600`, `-960` | as main | `lay18-login-rest-b-fields-1600`, `-960` |
| `login`'s count pointed at | `lay18-login-point-before-1600` | `lay18-login-point-a-list-1600`, `-960` | — |
| `login`'s count clicked | `lay18-login-click-before-1600` | `lay18-login-click-a-list-1600` | — |
| `cart` at 100 % | `lay18-cart-100-point-before-960` | `lay18-cart-100-point-a-list-960` | `lay18-cart-100-rest-b-fields-1600` |

The gestures: pointing at a count; a click on it; Esc.

Decision 3, `login` and `cart` at Full after a simulation
(`review_lay18_run_value`): not shown `lay18-value-100-rest-none-1600`,
`lay18-value-100-list-none-1600`, `-960`; in the list
`lay18-value-100-list-p1-1600`, `-960`, `lay18-value-cart-100-list-p1-960`;
beside the count `lay18-value-100-rest-p2-1600`,
`lay18-value-cart-100-rest-p2-1600`, `lay18-value-60-rest-p2-960`.

## Acceptance criteria

For option A. Tests in `acceptance_tests/lay18.rs` on `shop-30`, tidied,
Compact, checks folded.

1. Pointing at `login`'s count shows two lines: `login.body.token → 10
   readers: catalogue, cart, cart_view, address, checkout, order, +4 more`
   and `login.body.userId → 1 check: known_user`. Each number is the number
   of steps of the document that read that field; the most read field is
   first. Test: `lay18_criterion_1_a_count_lists_its_fields`.
2. A field nothing reads has no line, and an output without a count is
   unchanged: its lines are drawn and stated as LAY-14 does. Test:
   `lay18_criterion_2_only_fields_read_are_listed`.
3. A click on the count keeps the list and draws every wire of the port;
   Esc, a click on the empty board or a second click on the count ends
   both. Test: `lay18_criterion_3_a_click_keeps_the_list`.
4. The list moves nothing — no card's box, no route at rest, no camera —
   and its box is whole inside the board at 960 × 540 and 1600 × 1000.
   Test: `lay18_criterion_4_the_list_moves_nothing_and_fits`.
5. The count keeps its accessible name (`login.body → 11 readers`), so the
   tests of LAY-23 and LAY-32 pass unchanged.

6. Only if decision 3 = A: after a run, each line names the field's value
   from the report, cut to 28 characters, before the arrow; without a run,
   or for a field the report has no value for, the line is as in
   criterion 1. Test: `lay18_criterion_6_the_list_names_the_run_value`.

## Not in scope

Anything on the card itself: a line per field in place of the output rows,
and the grey line (decision 2). A port-model change. Naming every reader
beyond six (the Inspector lists them, LAY-10's relations). Relations out of
view (LAY-15). Descriptions (DIAG-01).

## Technical appendix

`canvas/readers.rs`: the fields of a port from the document's data wires
(the reference leaf's path, written as `texts` writes a statement) and the
list's text; `readers::paint_statement` already lays out several lines.
`canvas/mod.rs`: the count's hover and `shown_port` decide when the list is
painted; the hover text goes. Starts after LAY-37 (#423) is merged: it
rewrites the count's layout in the same two files. The mock branch carries
a throwaway drawing of A and B (`Mock18`); not for merge.
