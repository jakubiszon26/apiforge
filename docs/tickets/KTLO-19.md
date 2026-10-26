# KTLO-19: Run results can fill the window

- Size: S · Owner review: 20 min · Risk: `visible UI`
- State: Analysis (issue #44) — mock-up posted on the issue, waits for the owner's
  decisions below
- Analysis model: Fable · Implementation model: — (Opus 5.5 when approved)
- Mock-up branch: `mock/ktlo-ui-wishes` (throwaway; deleted after the decision)
- Owner's wish of 2026-10-26.

## Goal

After a run, the *Run results* panel can take the whole window below the tab
strip with one click, so a long report is read without scrolling in a strip
560 px high, and one click brings the board (or the code and board) back.

## What the owner will see

- **New:** an icon button *Maximise run results* at the right of the run
  results bar, beside *Continue remaining steps / Stop on first error*.
  Maximised, the panel starts under the tab strip and ends at the window's
  bottom; the library, board, inspector and, in Split, the code are not drawn
  while it is maximised. The button becomes *Restore run results*. The
  chevron that collapses the panel, Escape, switching tab or mode also
  restore the layout.
- **Changed:** nothing in the panel's content. The remembered height of the
  normal state is kept and used after restoring.
- **Removed or hidden:** nothing by default; the board is hidden only while
  the person keeps the panel maximised.
- **Migrated or converted:** nothing.
- **Default behaviour that changes:** none. The panel opens at its normal
  height after a run, as today. The maximised state is not remembered.

## Mock-up

`docs/qa/2026-10-26/KTLO-19/`: `mock-diagram-before-1600x1000.png` (the bar
with the new button at the normal height), `mock-diagram-maximised-960x540.png`,
`mock-diagram-maximised-1600x1000.png` and `mock-split-maximised-1600x1000.png`.
Posted on the issue.

## Decisions for the owner

1. **Diagram and Split only, or Code too?**
   - **A (recommended):** all three modes. The panel is the same widget and
     has the same 560 px ceiling in Code; one button that works everywhere
     is one behaviour to learn and one test.
   - **B (as asked):** Diagram and Split only; in Code the button is absent.
2. **The mock-up** as a whole: the button's place on the bar and the
   maximised layout.

## Acceptance criteria

1. With a run open in Diagram, the run results bar holds a button named
   *Maximise run results*. Clicking it makes the panel's top edge equal to
   the tab strip's bottom edge and its bottom edge the window's bottom; no
   widget of the library, board toolbar or inspector is in the accessibility
   tree (interaction test at 960 × 540 and 1600 × 1000).
2. The button is then named *Restore run results*; clicking it, pressing
   Escape, or clicking the *Run results* chevron returns the panel to the
   height it had before and the board's widgets to the tree.
3. In Split, criteria 1–2 hold and the code panel is also absent while
   maximised and back after restoring.
4. Switching the active tab or the mode while maximised restores the layout.
5. Selecting a step while maximised, then restoring, shows that node
   selected on the board with its inspector open (the existing
   `SelectNode` action unchanged).
6. With decision 1 = A: criteria 1–2 hold in Code.
7. The accessibility audit passes while maximised; at 960 × 540 every
   control of the bar and the step list is inside the window.

## Not in scope

A detached window for the report; remembering the maximised state; any
change to the report's content or to the Problems section (KTLO-18).

## Technical appendix

- `ReportPanel::show` (`report.rs`) draws `egui::Panel::bottom` with a
  height clamped to `(available - 200).min(560)`; `toggle_bar` draws the bar.
  Maximised: a `bool` on `ReportPanel`, a distinct panel id (the `size_epoch`
  pattern), `default_size(ui.available_height())` and the range pinned to it.
- The caller, `FolderApp::run_report` (`folder/runs.rs`), runs before the
  mode `match` in `folder/mod.rs`; when the panel reports it is maximised,
  the `match` is skipped (the central area stays empty under the panel). The
  Escape key goes into `FolderApp::shortcuts` guarded by the flag so that it
  does not steal Escape from the board's own uses.
- `Workbench` already tells when the active tab or mode changed (the tab
  strip and `toggle`); clear the flag there.
- Tests: `folder/acceptance_tests.rs` has `choose(&mut harness, "Run")` and
  the settled report; `testing.rs` gets a review view with a run.
