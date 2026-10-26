# KTLO-13: The code editor fills its panel without a bordered box

- Size: S · Owner review: 15 min · Risk: visible UI
- State: Approved 2026-10-26 — variant A (horizontal scroll), focus option 2
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

The code editor looks like an editor, not a form field: no box around the
text, the editor fills its panel, long lines scroll sideways under a gutter
that stays put.

## What the owner will see

- New: a thin focus line in the line-number gutter while the editor has
  keyboard focus (the box's coloured stroke was the only focus indicator).
- Changed: the editor has no border or fill of its own and extends to the
  bottom of its panel; long lines scroll horizontally while the line numbers
  stay fixed at the left. Today the numbers scroll away with the text and the
  box ends at the last line.
- Removed or hidden: the box around the editor. Decided by the owner on
  2026-10-26.
- Migrated or converted: nothing.
- Default behaviour that changes: none (lines were already not wrapped).

## Mock-up

`docs/qa/2026-10-26/KTLO-13/`: `before-code-<size>.png`,
`before-code-scrolled-<size>.png`, `before-split-<size>.png` (current) and
`mock-a-code-<size>.png`, `mock-a-code-scrolled-<size>.png`,
`mock-a-split-<size>.png` (approved), sizes 960x540 and 1600x1000. Posted on
issue #17. Variant B (wrapping) was rejected.

## Acceptance criteria

1. The editor paints no frame stroke or fill of its own and its text area
   reaches the bottom of the panel (interaction test at 960 × 540 and
   1600 × 1000 in Code and Split).
2. A line longer than the panel scrolls horizontally; after placing the caret
   at its end the gutter's line numbers are still at the panel's left edge.
3. When the editor has keyboard focus a focus line is visible in the gutter;
   when it does not, it is not.
4. Text files and the legacy code panel, which share the widget, still render
   and edit (existing tests pass).
5. The accessibility audit and the design review checklist (focus visible)
   pass.
6. The QA report shows release-binary captures at both sizes in Code and
   Split, including one with a long line scrolled.

## Not in scope

Wrapping (rejected); the Problems bar; large-source mode beyond "it still
works".

## Technical appendix

`CodeEditor::show` in `crates/apiforge-desktop/src/code.rs` (not
`folder/code.rs`). The mock-up (branch `mock/ktlo-13-editor`, the commit
before the captures) sets `.frame(Frame::new().inner_margin(Margin::symmetric(4, 2)))`
(egui 0.36 takes a `Frame`, which drops the default fill, stroke and focus
stroke), sets `.min_size` to the visible bottom when line numbers are on, and
makes `paint_gutter` translate to `clip_rect().left()` with a background
behind it. The gutter background must follow the panel fill, not a hard-coded
white; the focus line goes into `paint_gutter` from the editor's focus state.
In the harness a horizontal wheel event did not scroll `ScrollArea::both`;
click + End does.
