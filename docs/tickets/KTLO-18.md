# KTLO-18: The Problems section expands over the code with one click

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Analysis (issue #43) — mock-up posted on the issue, waits for the owner's
  decisions below
- Analysis model: Fable · Implementation model: — (Opus 5.5 when approved)
- Mock-up branch: `mock/ktlo-ui-wishes` (throwaway; deleted after the decision)
- Owner's wish of 2026-10-26.

## Goal

When a document has many problems, the *Problems* list under the code can
be expanded with one click to most of the panel's height, read, and put back.

## What the owner will see

- **New:** a chevron button *Expand problems* at the right of the `PROBLEMS`
  caption row, before the status (`Saved · 2 errors`). Clicking it gives the
  list the panel's height minus a strip of code of 96 px (about four lines),
  so the line a problem points to is still visible when it is clicked. The
  button becomes *Restore problems*; clicking it returns the previous
  height. The resize handle keeps working in both states.
- **Changed:** nothing else. The status text, the rows, *Apply fix* and
  click-to-reveal are as today.
- **Removed or hidden:** nothing. The section cannot be collapsed below its
  current minimum (72 px).
- **Migrated or converted:** nothing.
- **Default behaviour that changes:** none. The section opens at its
  remembered height; the expanded state is not remembered.

The code panel is the same widget in Code and in Split, so the button is in
both unless the owner says otherwise (decision 2).

## Mock-up

`docs/qa/2026-10-26/KTLO-18/`: `mock-code-before-960x540.png` (today, with
problems), `mock-code-expanded-960x540.png`, `mock-code-expanded-1600x1000.png`
and `mock-split-expanded-1600x1000.png`. Posted on the issue.

## Decisions for the owner

1. **How to expand?**
   - **A (recommended):** the expand/restore button above. One click each
     way; the drag handle stays for fine adjustment.
   - **B:** no button; only raise the drag ceiling from 360 px to the panel's
     height minus 96 px. Nothing new on screen, but expanding means dragging
     a 4 px handle across the panel.
2. **Code only, or Code and Split?** Recommended: both, it is one widget and
   one behaviour. Code only is possible with a mode check.
3. **The mock-up** as a whole.

## Acceptance criteria

1. In Code the `PROBLEMS` caption row holds a button named *Expand
   problems*. Clicking it makes the section's height equal to the code
   panel's height minus 96 px (±1 px); the editor is still visible above it
   (interaction test at 960 × 540 and 1600 × 1000).
2. The button is then named *Restore problems*; clicking it returns the
   section to the height it had before, which is also the height written to
   the view state (`panels.diagnostics`).
3. Clicking a problem row while expanded selects its range in the editor and
   focuses the editor, as today.
4. Dragging the resize handle while expanded changes the height; the next
   *Restore problems* returns to the pre-expansion height.
5. With decision 2 = both: criteria 1–4 hold in Split at 960 × 540 and
   1600 × 1000.
6. The accessibility audit passes in both states; at 960 × 540 the editor's
   first line and the expanded list are inside the window.

## Not in scope

Collapsing the section to its caption; remembering the expanded state;
problems for text files (they have none); the run results panel (KTLO-19).

## Technical appendix

- `FolderApp::code_area` (`folder/code_view.rs`) draws
  `egui::Panel::bottom(("folder-diagnostics", mode))` with
  `size_range(72.0..=360.0)` and writes `panels.diagnostics` from the
  available height each frame; `diagnostics_panel` draws the caption row.
  Expanded: a `bool` on `FolderApp`, a panel id that changes with it (as
  `ReportPanel` does with `size_epoch`) so egui does not keep the dragged
  size, `default_size(ui.available_height() - 96.0)` and the ceiling raised
  to the same value; while expanded, do not write `panels.diagnostics`.
- `report.rs` already solves the same problem for the run results panel
  (`keyboard_height`, `size_epoch`); copy the pattern, not the code.
- Tests: `folder/editor_tests.rs` and the KTLO-13 acceptance tests in
  `folder/acceptance_tests.rs` know the editor's rect and the panel; a
  fixture with ten problems is three lines of invalid `.afs`.
