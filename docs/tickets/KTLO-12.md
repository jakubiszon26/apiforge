# KTLO-12: One header above the board

- Size: M, measured L, split into two pull requests (see *Delivery*) · Owner
  review: 20 min · Risk: visible UI
- State: In progress — PR A of 2. Approved 2026-10-26 — Diagram: variant A;
  Split: variant B
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

Above the board there is one row, not two: the document's name and its
Main / Cleanup tabs on the left, the board's tools on the right. The top app
bar (Project · name · Code / Diagram / Split · Save · Save all · Run options ·
Run) does not change.

## What the owner will see

- New: nothing.
- Changed: the document header row (file name · "Scenario" · Main / Cleanup)
  and the board toolbar row (steps summary · Add step · tools) become a single
  row on the board's header band.
  - In **Diagram** (variant A): file name, kind and Main / Cleanup on the
    left; steps summary, Add step and the tools on the right. When the row is
    too narrow, the kind ("Scenario") is dropped first, then the name is
    shortened with its full text on hover, then the steps summary is dropped.
    At 960 × 540 with the block library open everything but the kind fits.
  - In **Split** (variant B): only Main / Cleanup on the left, because the
    tab strip already shows the file name; the right side is the same as in
    Diagram. At 960 × 540 the steps summary does not fit and is left out.
- Removed or hidden: the separate white header row; in Split the file name
  and kind above the board (still visible on the tab strip). Decided by the
  owner on 2026-10-26.
- Migrated or converted: nothing.
- Default behaviour that changes: none.

## Mock-up

`docs/qa/2026-10-26/KTLO-12/`: `before-<view>-<size>.png` (current),
`mock-a-diagram-<size>.png` (approved for Diagram),
`mock-b-split-<size>.png` (approved for Split), sizes 960x540 and 1600x1000.
Posted on issue #16.

## Acceptance criteria

1. In Diagram and in Split there is exactly one row between the tab strip and
   the board; the top app bar is unchanged (interaction test through the
   named widgets, at 960 × 540 and 1600 × 1000).
2. Diagram shows, left to right: file name, kind, Main, Cleanup, steps
   summary, Add step, the two tool buttons; Split shows Main, Cleanup, then
   the same right-hand group.
3. Main / Cleanup keep their accessible names and behaviour; Add step and the
   tools keep theirs; keyboard tab order runs name → Main → Cleanup → Add
   step → tools.
4. At 960 × 540 nothing overlaps or is clipped (`assert_within_window`) in
   Diagram with the block library open and in Split; the drop order from
   "What the owner will see" is what happens.
5. The accessibility audit passes in both modes.
6. Design review captures match the approved mock-ups; the QA report shows
   release-binary captures at both sizes in both modes.

## Not in scope

Hiding the block library in Split (backlog, *Later*); KTLO-13 (the editor).

## Technical appendix

The document header is a `Frame` row in `FolderApp::diagram_area`
(`folder/diagram.rs`); the second row is the canvas overlay header whose
geometry is `canvas/layout.rs::board_overlays`, painted by
`paint::overlay_chrome` (summary at `overlays.banner`) with the toolbar drawn
by `Canvas::toolbar` (`canvas/mod.rs`) as an `egui::Area`. The mock-up
(branch `mock/ktlo-12-header`, commit `a2c1cf8`) deletes the Frame row, draws
the header in an `Area` over the board's header at the left, adds
`canvas::Input.header_leading` and moves the summary next to the toolbar; its
widths are hard-coded guesses. The real version measures the widths, feeds the
left width into `board_overlays` (which has a stacked fallback) and extends
its unit test. The legacy `app.rs` `Input` needs the new field. Risks: focus
order across two `Area`s; the header now sits on the board's grey band.

## Delivery

The finished change measured about 1,040 changed lines with its QA report:
size L, which is not scheduled ([estimation](../process/estimation.md#size)).
It was split before the QA report into two pull requests, along a seam that
leaves the first with nothing to see:

- **PR A (part 1 of 2)** — groundwork in the board, no visible change.
  `canvas::Input.header_leading` (`None` at every caller),
  `OverlayMeasures.leading_size` and `BoardOverlays.leading`,
  `Canvas::header_room` and `leading_rect`, `widgets::tab_value_width`, the
  steps summary's text and font as functions, and the `board_overlays` unit
  test with three header widths. It also fixes a pre-existing test race (below).
- **PR B (part 2 of 2)**, branched from `main` after PR A is merged — the
  header drawn on the board's row in `folder/diagram.rs`, the steps summary
  beside the toolbar, the keyboard order, the six acceptance tests, the
  captures and the QA report. It closes #16.

### A pre-existing test race

`folder::tests::a_large_source_follows_the_caret_once_its_edit_is_analysed`
edited the source through the session, which skips the typing pause, so its
analysis worker raced three frames of an 8,400-line editor. On `main` it
passed 4 of 4 suite runs on a 3-core machine; with the heavier acceptance
tests of part 2 in the same suite it failed in 4 of 5: twice "not searched
while the edit is pending", and twice "storage did not become idle" when
opening, the latter at a load average around 15. PR A holds the edit
pending by the typing pause, asserts it is still pending, and waits on the
analyser instead of the clock; the selection assertions are unchanged. The
15-second opening wait it shares with other folder tests is not changed.
