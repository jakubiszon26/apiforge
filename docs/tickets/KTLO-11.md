# KTLO-11: Project files in Diagram — the left panel switches between Library and Files

- Size: M · Owner review: 30 min · Risk: `visible UI`, `format` (the chosen
  tab remembered per project, decision 2)
- State: Analysis — mock-up posted on the issue, waits for the owner's
  decisions below. Depends on KTLO-17 (the rail).
- Analysis model: Fable · Implementation model: — (Opus 5.5 when approved)
- Mock-up branch: `mock/ktlo-ui-wishes` (throwaway; deleted after the decision)
- Opened on 2026-10-25 as "scenario navigation in Diagram mode" (KTLO-05a
  decision 3, *Later*). Re-scoped on 2026-10-26 to the owner's wish: in
  Diagram, switch the left panel between the block library and the file
  explorer. Pulled into the KTLO sprint by the owner.

## Goal

In Diagram the person can open another scenario or component without
leaving the mode: the panel left of the board shows either the *Block
library* or the project's *Files*, and one click switches between them.

## What the owner will see

- **New:** two tabs, *Library* and *Files*, in the caption row of the left
  panel in Diagram, styled like the *Blocks · Variables · Operators* row
  inside the library. *Files* shows the same explorer as Code mode: the
  tree, *+*, delete and refresh, the same keys, the same selection that
  follows the active tab. Clicking a scenario or component opens its tab and
  the board shows it; the mode stays Diagram. Clicking a text file opens its
  tab as today (the Diagram area says the file has no diagram).
- **New:** when the panel is collapsed (KTLO-17), the rail holds two icon
  buttons, *Show block library* and *Show project files*; each expands the
  panel on that tab.
- **Changed:** the panel's width range in Diagram becomes 200–420 px (the
  library allows 200–320 today, the explorer in Code 180–420), one width for
  both tabs, remembered as the library width is (KTLO-05c).
- **Removed or hidden:** nothing. The library's content, drag-and-drop and
  click-to-add are unchanged on the *Library* tab.
- **Migrated or converted:** nothing.
- **Default behaviour that changes:** none. Diagram opens on *Library*, as
  today. With decision 2 = A, a project closed on *Files* reopens on *Files*.

## Mock-up

`docs/qa/2026-10-26/KTLO-11/`: `mock-diagram-files-960x540.png`,
`mock-diagram-files-1600x1000.png` (the *Files* tab with the explorer),
`mock-diagram-library-1600x1000.png` (the tab row with *Library* active) and
`mock-diagram-rail-1600x1000.png` (the collapsed rail with both icons).
Posted on the issue.

## Decisions for the owner

1. **Diagram only, or Split too?**
   - **A (recommended, as asked):** Diagram only. In Split the code is
     beside the board and the panel keeps one job, the library.
   - **B:** also in Split at 1440 px and wider. Same control everywhere;
     the explorer then sits between the code and the board.
2. **Remember the chosen tab per project?**
   - **A (recommended):** yes, with the collapsed state of KTLO-17 in the
     project's local view state. `format` flag as in KTLO-17.
   - **B:** no; Diagram always opens on *Library*.
3. **The mock-up** as a whole: the tab row above the panel, the explorer's
   toolbar inside it, the two icons on the rail.

## Acceptance criteria

1. In Diagram the left panel's caption row holds two tabs named *Library*
   and *Files*; *Library* shows the block sections of KTLO-05b, *Files*
   shows the explorer with the same rows as Code mode for the same project
   (interaction test: the row labels match).
2. On *Files*, clicking a scenario opens its tab, the board shows that
   scenario and the mode is still Diagram; clicking a component does the
   same; clicking a text file opens its text tab.
3. The explorer's *+*, delete, refresh and keyboard behaviour on *Files*
   produce the same results as in Code (the existing explorer tests run
   against the panel in Diagram).
4. The collapsed rail holds *Show block library* and *Show project files*;
   each expands the panel on its tab.
5. The explorer's selection follows the active tab when it changes from the
   tab strip while *Files* is shown.
6. With decision 2 = A: the chosen tab is restored on reopening the project;
   a view state without the field opens on *Library*.
7. The accessibility audit passes with each tab shown and with the rail; at
   960 × 540 every control of the panel on *Files*, the board toolbar and
   the inspector is inside the window.

## Not in scope

Any change in Split (decision 1 = A) or in Code. A files panel below 1440 px
in Split. Drag-and-drop of files. Collapsing itself (KTLO-17).

## Technical appendix

- `FolderApp::explorer_panel` (`folder/explorer.rs`) draws the explorer from
  `self.explorer` and the session; it has no dependence on the mode, so the
  *Files* tab calls it inside the Diagram panel. `follow_active` is called at
  its top; it runs whenever the panel is drawn.
- `library_panel` (`folder/mod.rs`) gains the tab row before
  `Library::show_blocks`; the tab is a `Workbench` field persisted as in
  KTLO-17 (`PanelSizes` tolerates unknown fields).
- Opening a document from the explorer goes through the explorer's `Action`
  and `workbench.open`; nothing there changes the mode. Verify the board's
  camera and selection per tab behave on switching as they do from the tab
  strip.
- Width: the panel's `size_range(200.0..=320.0)` becomes `200.0..=420.0`;
  the explorer in Code keeps `panels.documents`.
- Tests: extend `folder/library_tests.rs`; the explorer tests in
  `folder/project_interaction_tests.rs` are the ones to parametrise over the
  mode.
