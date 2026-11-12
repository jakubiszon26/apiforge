# LAY-09: Semantic zoom — what Overview and Compact show, with minimum sizes

- Size: S–M · Owner review: 20 min · Risk: `visible UI`
- State: Backlog (issue #293) · decided by the owner on 2026-11-05 (Q7 = A
  not vetoed, Q13 = A, Q15 = A; DIAG-00 #241 folded in by Q8 = A) · LAY
  sprint, part A, after LAY-21 and LAY-11; re-scoped by the
  [review of 2026-11-10](../sprints/lay.md#11-mid-sprint-review-2026-11-10)
  (R3, R7)
- Analysis model: Fable · Implementation model: Opus

## Goal

At every zoom a card says what matters at that distance: at Fit view the
status, the number and a readable name; at Compact the method, the path,
the two result lines and the ports in use; at Full everything. No text gets
smaller when zooming in, and a failed or blocked card is visible at every
level.

## What the owner will see

- Overview (below 0.45): the status band, the badge, the name drawn at a
  minimum on-screen size of 11 px whatever the zoom, clipped to the card,
  and the checks aggregate as a pill under the name (Q5).
- Medium (0.45 to 0.75): plus method and short path, two result lines
  (execution, checks), the ports in use and one summary line per configured
  input (`Body · 2 fields`, Q13 = A); unconnected output ports, *Optional*
  and separators appear at Full.
- Full: as today plus the badge; a check's subtitle names subject and
  predicate (LAY-08).
- A failed card has a 2 px red border and a blocked card a 2 px amber one
  at every level (Q15 = A); labels yield to the signals at extreme zoom.
- Changed: the thresholds 0.45 / 0.75 stay the working assumption (Q7 =
  A); this ticket verifies them with the minimum sizes and may move them,
  recording the final values here. Zooming never moves a card (the box is
  LAY-16's). Removed or hidden: below Full, the unconnected output ports,
  *Optional* and the separators; at extreme zoom the labels yield to the
  signals (owner's Q13 = A). Default behaviour that changes: what a card
  shows below 0.75.
- New: a check card's subtitle names its subject and predicate
  (`cart.total ≈ total`), from LAY-08's subjects.

## Re-scoped on 2026-11-10

LAY-16 and LAY-21 built the compact box, which already drops a row that is
not legible and keeps the name at every zoom. What is left here: the
minimum sizes as one rule for every card kind at both densities, the failed
and blocked borders, the aggregate pill at Overview, and two additions.

- **Fit shows every card.** The board's minimum zoom is 0.25 today and Fit
  is clamped to it, so a board that needs less is cut off (`shop-30`
  tidied needs 0.205; at 960 × 540 every reference board needs 0.06 to
  0.16). The minimum becomes 0.05; below 0.25 a card is its band, its
  border and its badge, and the name is drawn only where it fits.
- **The zoom level between 0.45 and 0.75 is called *Medium***, in code
  (`CardDetail::Medium`) and in the documents, because *Compact* is the
  density (*Compact cards*). Nothing on screen carries either word.
- A check's line naming subject and predicate moved to LAY-21 (it is the
  compact check card); criterion 6 below now covers the full card only.

## Decisions taken (owner, 2026-11-05)

> Q7 = A (not vetoed): the thresholds are a parameter until this ticket's
> minimum sizes are measured. Q13 = A: the Compact content above. Q15 = A:
> a failed check on a successful request turns the footer red and gives the
> card the 2 px red border at every zoom; the band keeps the HTTP colour.
> Q8 = A: DIAG-00's questions on detail by zoom and the failed state are
> these.

## Mock-up

Round 4 proof 1 (`mock/lay-00-round-4` at `df6305b`): `lay00r4-card-44`,
`-46`, `-60`, `-74`, `-76`, `-90` and `lay00r4-card-60-today`; round 3
`lay00r3-d-shop-30-35`, `lay00r3-d-shop-30-fit`, `lay00r3-d-shop-30-states-60`.
Approved as direction on 2026-11-05. Round 3's Compact name shrank across
0.45 (the Overview had a minimum, the Compact had none, 4.7 item 2); that
defect is criterion 2.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay09.rs`, on `shop-30.afs` after a failed simulation.

1. At 0.35 the failed card has the red 2 px border and the blocked one the
   amber one; at 0.25 a name is at least 11 px high on screen, a step number
   and the result line at least 9 px. Test:
   `lay09_criterion_1_signals_and_minimum_sizes_at_overview`.
2. No text is smaller after zooming in across a threshold: rendered at
   0.44 and 0.46, and at 0.74 and 0.76, the name's, number's and result
   line's on-screen heights are monotone non-decreasing. Test:
   `lay09_criterion_2_sizes_are_monotone_across_thresholds`.
3. At Medium `catalogue` shows `Query · 1 parameter` though its query has
   no wire; `login` shows its two result lines `HTTP 200` and `2 checks`
   (grey before a run, never a tick for a check that did not run). Test:
   `lay09_criterion_3_compact_shows_the_configured_inputs_and_two_lines`.
4. A successful request with a failed check reads `HTTP 200 · ✕ 1/3
   failed` in red with the red border at 0.35, 0.60 and 1.0, and its band
   keeps the HTTP colour. Test:
   `lay09_criterion_4_a_failed_check_on_a_green_request`.
5. Zooming from 0.2 to 1.0 moves no card: every card's world rectangle is
   identical at each zoom. Test: `lay09_criterion_5_zooming_moves_no_card`.
6. A check card's subtitle reads subject and predicate at Compact and Full.
   Test: `lay09_criterion_6_a_check_names_its_subject`.
7. The final thresholds are recorded in this file with the measurement that
   fixed them; if they differ from 0.45 / 0.75 the QA captures show the
   boundary frames.
8. Fit on `shop-50.afs` at 960 × 540 shows every card (the cards' box is
   inside the viewport), and the failed card's red border is visible in
   that frame. Test: `lay09_criterion_8_fit_shows_every_card`.

## Thresholds recorded (2026-11-12)

The thresholds stay **0.45** (Overview to Medium) and **0.75** (Medium to
Full), `MEDIUM_FROM` and `FULL_FROM` in `canvas/layout.rs`. The minimum
sizes on screen are a name 11 px, a line 9 px and a step number 9 px,
applied by `paint::min_on_screen` to every card kind at both densities.
What fixed the values, measured on `shop-30` at 1600 × 1000:

- 0.45: a compact row is 24 px, 10.8 px on screen there, and holds the
  9 px line; the name's header (34 px) is 15.3 px and holds the 11 px name.
  Below 0.45 the rows would be under 10.8 px, so the card is its band, name
  and checks pill. The previous gate, a row of at least 11 px (0.458), left
  0.45–0.458 with neither the pill nor the lines and is replaced by the
  threshold itself.
- 0.75: the smallest Full text, a type label of 11 px at 100 %, is 8.25 px
  there and is held at 9 px; the Full values are no smaller than Medium's
  summaries at 0.74 (both 9 px).
- Rendered at 0.44 / 0.46 and 0.74 / 0.76 the name is 11 / 11 and
  11.84 / 12.16 px and the result line 9 / 9 and 9 / 9.12 px on a compact
  card; on a full card the name is the same and the result line 9 px on
  both sides of both thresholds (criterion 2).
- Fit's limit is 0.05: `shop-50` needs 0.069 at 960 × 540 with Run results
  folded, `shop-30` 0.24 at 1600 × 1000 and 0.12 at 960 × 540. Below
  **0.25** a card draws its name only where it fits (a compact value or
  check card, 70–96 px high, does not at 0.24) and no pill.

## Not in scope

The card's box and density (LAY-16); the badge itself (LAY-07); the
aggregate's behaviour and the collapse (LAY-11); descriptions beside blocks
(DIAG-01, deferred); the data-in-use line (LAY-18).

## Technical appendix

- `canvas/layout.rs::CardDetail::at_zoom` keeps the thresholds as named
  constants; the three paint paths in `canvas/paint.rs` share one
  `min_on_screen(size, zoom, min)` helper so every level clamps the same
  way (round 3 clamped the Overview name only).
- The failed and blocked borders are painted after the band, from
  `StepStatus`, independent of the detail level.
- The two result lines read the run's `StepResult` for the request and the
  aggregate of LAY-08's subjects for the checks.
