# LAY-18: Compact shows the data in use — `body.token → 6 readers`

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: Backlog (issue #302) · added to the sprint by the owner on
  2026-11-05 (Q12 = C; the second opinion's item 5 of
  [lay.md 4.5](../sprints/lay.md#45-second-opinion-part-two-local-context);
  W11 of the audit) · LAY sprint, wave 1, after LAY-14
- Analysis model: Fable · Implementation model: Opus

## Goal

A request card says which of its answer's fields the scenario goes on to
use, and how much: `body.token → 6 readers`, `status → 1 check`, one line
per field read, instead of the fixed output rows; a click starts the trace
of those wires (LAY-14), so the token that feeds half the scenario (W11) is
found from its source.

## What the owner will see

- New: at the compact density (LAY-16) the output part of a request card is
  one line per field some step or check reads — `body.token → 6 readers`,
  `status → 1 check` — in place of the output port rows; a field nothing
  reads is not listed. A click on the line pins LAY-14's trace to every
  reader, until Esc. The card's box is unchanged by the line's wording.
- Changed: how a request's outputs read at the compact density. Removed or
  hidden: the unused output rows at that density (already hidden by LAY-16;
  this ticket replaces the used ones with the summary). Default behaviour
  that changes: none beyond LAY-16's. The port model and the project format
  are unchanged.

## Mock-up

Not rendered: the line replaces a row LAY-16 already sizes, in LAY-16's
captures (`lay00r4-card-60`, `mock/lay-00-round-4` at `df6305b`); the QA
captures of `login` and `cart` at 60 % and 100 % show it, and the owner
judges the wording there. If the wording needs a second look, one capture
round before the pull request.

## Acceptance criteria

Tests in `acceptance_tests.rs`, `mod lay18`, on `shop-30.afs`.

1. `login` at the compact density reads `body.token → N readers` with N
   equal to the number of wires from that field; `status → 1 check` for a
   field one check reads. Test: `lay18_criterion_1_the_line_counts_the_readers`.
2. A field nothing reads is not listed; a request nothing reads shows one
   grey line *Nothing reads this step*. Test:
   `lay18_criterion_2_unread_fields_are_not_listed`.
3. Clicking the line pins the trace to every reader (LAY-14's pills on each
   wire), until Esc. Test: `lay18_criterion_3_a_click_traces_every_reader`.
4. The card's world box is the same with a one-reader and a six-reader line
   (the wording never resizes the card). Test:
   `lay18_criterion_4_the_wording_moves_nothing`.
5. At Full the line is followed by the field's value from the last run, as
   LAY-16 writes input values. Test: `lay18_criterion_5_full_adds_the_value`.

## Not in scope

A port-model change; listing readers by name on the card (the Inspector
does, via LAY-10's relations); descriptions (DIAG-01).

## Technical appendix

The readers come from LAY-10's `Relations` (readers of the step grouped by
source field); the line is a row kind in `canvas/layout.rs` with the same
height as a port row, so LAY-16's box is kept; the click calls LAY-14's pin
with the edge set.
