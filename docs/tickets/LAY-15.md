# LAY-15: Off-screen relations during focus — chips by role, and Back

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: Backlog (issue #299) · adopted from the second opinion of 2026-11-04
  (4.5 item 2), decided within Q9 = A on 2026-11-05 · LAY sprint, wave 1,
  after LAY-14
- Analysis model: Fable · Implementation model: Opus

## Goal

When the selected step's source or reader is out of view, a chip on the
board's edge says what it is and where (`Source: login.body.token ↑`), a
click jumps there without changing the zoom, and *Back to checkout*
restores the camera, the selection and the Inspector exactly.

## What the owner will see

- New: during focus (LAY-10), a chip on the board's edge for each related
  card out of view, labelled by role — `Source: login.body.token ↑`,
  `Reader: order.URL ←`, grouped as `↑ 3 sources` beyond two per edge. A
  card whose header is not fully in view counts as off-screen. A click jumps
  to the card at the same zoom (its own camera path, not `Canvas::reveal()`,
  which forces zoom ≥ 0.85); *Back to checkout* restores camera and zoom
  with the selection and the Inspector kept. Chips never cover the selected
  card. Chips exist for data relations (sources, readers); checks and order
  are not chipped.
- Changed: nothing. Removed or hidden: nothing. Default behaviour that
  changes: none.

## Mock-up

Round 3 `lay00r3-focus-offscreen-60`, `lay00r3-focus-offscreen-back`,
`lay00r3-walk-1..4`; round 4 proof 4 `lay00r4-walk-1..5` and
`lay00r4-walk-small-1..5` (`mock/lay-00-round-3` at `ea981d8`,
`mock/lay-00-round-4` at `df6305b`). Approved as direction on 2026-11-05.
Round 3's labels were ambiguous (`← expected_total · 2`, 4.7 item 7), hence
the role labels; round 4 asserted the walk by clicks (four clicks, no pans)
and found that at 960 × 540 with the Inspector open the chips covered the
selection, hence criterion 5.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`, `mod
lay15`, on `shop-30.afs` at 60 %, 1600 × 1000 and 960 × 540.

1. `checkout` selected with `login` out of view shows the chip `Source:
   login.body.token ↑` on the top edge; a card whose header is partly out of
   view is chipped too. Test:
   `lay15_criterion_1_an_off_screen_source_has_a_chip`.
2. Clicking the chip lands on `login` at the same zoom, by the chip's own
   camera path; the selection and the Inspector are unchanged. Test:
   `lay15_criterion_2_the_click_keeps_the_zoom`.
3. *Back to checkout* restores camera and zoom exactly (the camera struct is
   equal), with the selection and the Inspector kept. Test:
   `lay15_criterion_3_back_restores_exactly`.
4. The walk of task 8 (select `checkout` → source → the source's check →
   back → keep editing) is done by clicks on accessible buttons and chips,
   no pan, at both window sizes. Test: `lay15_criterion_4_the_walk_by_clicks`
   (shared with LAY-12's task 8).
5. Chips never overlap the selected card at 960 × 540 with the Inspector
   open; three sources on one edge read `↑ 3 sources`. Test:
   `lay15_criterion_5_chips_cover_nothing_selected_and_group`.
6. Opening the Inspector does not pan the board away from the selection
   (UX-18's rule holds with chips present). Test:
   `lay15_criterion_6_the_inspector_keeps_the_place`.

## Not in scope

Chips for checks or order relations; a minimap; the focus itself (LAY-10).

## Technical appendix

The camera is zoom and pan, a `Copy` struct (round 4's fact), so *Back*
stores the struct; the chips are laid out in `canvas/mod.rs` from LAY-10's
`Relations` and the viewport, excluding the selected card's rect; the harness
clicks them as accessible buttons `Source: … (jump)`.
