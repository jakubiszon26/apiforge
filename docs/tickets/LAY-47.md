# LAY-47: A value many steps read runs in one corridor, so data lines cross each other less

- Size: M (to be confirmed by the measurement below) · Owner review:
  20 min · Risk: `visible UI`
- State: **Analysis, not started.** Asked for by the owner on 2026-11-20
  as a later ticket of LAY-2, outside 0.2.0, after the feasibility gate
  of LAY-45 (#468). No decision is taken yet and no criterion below is
  approved: they say what the analysis has to measure and show.
- Issue [#506](https://github.com/jakubiszon26/apiforge/issues/506) ·
  LAY-2 · branch `feat/lay-47-data-corridor`
- Filed on 2026-11-20 as LAY-46 by mistake: that number is #469 (the
  checks line at the Overview zoom). Renumbered the same day.
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

With *All wires* on, the lines of one value that many steps read travel
together and leave their corridor one by one, so that they cross each
other and the other data lines less, and each can still be followed.

## What is known

Measured at LAY-45's gate (the table on #468, `main` after LANG-13,
after Tidy up, 1600 × 1000, compact cards):

- On `shop-30` with every wire the data lines alone cross each other 52
  times with the checks folded and 77 with them drawn. `login`'s token is
  read by nine later steps: nine lines leave one socket, go down through
  two rows and cross the lines of those rows and each other.
- LAY-28 already orders the lines that share a corridor so that they
  leave it without crossing each other (its criterion 1). What it does
  not do is make the lines of one value **share** a corridor: each is
  routed on its own.
- In *Chips*, the default, the same fan is not drawn at all (LAY-23), so
  this ticket changes only what *All wires* shows.
- The owner's rule (2026-11-15): two lines are never closer than can be
  followed, 20 px; a board that sprawls is better than a dense one. A
  shared corridor that puts nine lines 20 px apart is 180 px wide; lines
  of one value drawn as one trunk with branches would be one line wide,
  and would look different from today.

## What the analysis has to decide

1. **One trunk or nine lines.** Whether the lines of one value are drawn
   as separate lines side by side in a corridor (today's look, more
   room), or as one trunk that branches at each reader (a new look: the
   owner's decision, with captures of both on `shop-30` and `fan-out`).
2. **Where the corridor runs**: in the column gap right of the source,
   in the row gaps, or outside the rows; and what it does to the fit
   zoom at 960 × 540 and 1600 × 1000.
3. **What is counted.** The three measures of LAY-45's criterion 8; the
   target is fewer data-against-data crossings on `shop-30`, `shop-50`
   and `fan-out` with every wire, and no more on any board — a figure is
   proposed only with the measurement that shows it can be reached.
4. Whether pointing at a trunk or at one of its branches marks the whole
   value or one reader (LAY-14's tracing).

## Acceptance criteria

To be written from the measurement and the owner's answers. Each will
name its test; the candidates are: data-against-data crossings per
reference board, before and after; `packed` 0; the fit zoom not smaller
by more than a stated amount; LAY-28's and LAY-45's suites unchanged
where the routing of other lines is concerned; the routing budget of
ADR 0006.

## Not in scope

The chain's lines (LAY-45). *Chips* mode. Where Tidy up puts cards.

## Technical appendix

`crates/apiforge-layout/src/route.rs`, `order.rs` (the order of lines in
a corridor, LAY-28), `crates/apiforge-desktop/src/canvas/routing.rs`;
the `quality` example, which prints the two kinds of crossing apart
since LAY-45.
