# KTLO-17: The block library collapses to a rail in Diagram and Split

- Size: S · Owner review: 20 min · Risk: `visible UI`, `format` (if the
  state is remembered per project, decision 1)
- State: Analysis (issue #42) — mock-up posted on the issue, waits for the owner's
  decisions below
- Analysis model: Fable · Implementation model: — (Opus 5.5 when approved)
- Mock-up branch: `mock/ktlo-ui-wishes` (throwaway; deleted after the decision)
- Owner's wish of 2026-10-26; the *Later* entry "hiding the block library
  panel in Split by choice" is this ticket.

## Goal

When the board needs the room, the *Block library* folds away to a narrow
rail with one click, in Diagram and in Split, and comes back with one click.

## What the owner will see

- **New:** a chevron button *Collapse block library* at the right of the
  panel's caption row (`BLOCK LIBRARY 10`). Collapsed, the panel becomes a
  36 px rail with one icon button, *Show block library*; the board takes the
  freed width. The rail stays where the panel was: left of the board in
  Diagram, between the code and the board in Split.
- **Changed:** while the library is collapsed the *Add step* popup lists
  Variables and Operators, exactly as it does today in Split below 1440 px
  (KTLO-05d), so nothing is reachable only through the panel. The empty
  board's hint reads "Add the first step" while the library is collapsed
  (KTLO-05a criterion 9 already ties the hint to the library being in view).
- **Removed or hidden:** nothing by default; the panel hides only when the
  person collapses it.
- **Migrated or converted:** nothing.
- **Default behaviour that changes:** none. Diagram and Split open with the
  panel expanded, as today. With decision 1 = A, a project that was closed
  with the library collapsed reopens with it collapsed.

Split below 1440 px is unchanged: no panel, no rail, the popup is the
library (KTLO-05a decision 1).

## Mock-up

`docs/qa/2026-10-26/KTLO-17/`: `mock-diagram-collapsed-960x540.png`,
`mock-diagram-collapsed-1600x1000.png`, `mock-split-collapsed-1600x1000.png`
(the rail) and `mock-diagram-expanded-1600x1000.png` (the caption row with
the new chevron). Posted on the issue.

## Decisions for the owner

1. **Remember the collapsed state per project?**
   - **A (recommended):** yes, with the panel sizes in the project's local
     view state (`.apiforge/local/workspace.json`, not tracked by Git). The
     library comes back the way it was left, like its width (KTLO-05c).
     Consequence: the file gains a field; an older build ignores it
     (KTLO-05c made the panel sizes tolerant of unknown fields), hence the
     `format` flag.
   - **B:** no; every start opens the library expanded. Nothing written.
2. **A keyboard shortcut?** Ctrl+B toggles the library (free today; it is
   the sidebar shortcut in most editors). Recommended. Without it the chevron
   and the rail button are the only way.
3. **The mock-up** as a whole: the chevron's place, the 36 px rail and its
   icon.

## Acceptance criteria

1. In Diagram, the library panel's caption row holds a button named
   *Collapse block library*. Clicking it replaces the panel by a rail at
   most 40 px wide that holds a button named *Show block library*; the
   board's left edge moves to the rail's right edge (interaction test
   through the named widgets at 960 × 540 and 1600 × 1000).
2. Clicking *Show block library* (and, with decision 2, Ctrl+B in either
   state) restores the panel at the width it had before collapsing.
3. In Split at 1440 px and wider the same two controls behave the same;
   below 1440 px nothing changes: no rail, and the popup is as after
   KTLO-05d.
4. While the library is collapsed, the *Add step* popup lists Variables and
   Operators; while it is expanded, the popup is unchanged.
5. The empty board's hint reads "Add the first step" while the library is
   collapsed and "Drag a block from the library or add the first step" while
   it is expanded.
6. With decision 1 = A: collapsing, closing and reopening the project
   restores the collapsed rail; a view state without the field opens
   expanded; a view state with the field loads in a build without it
   (KTLO-05c criterion 3 extended to the new field).
7. The accessibility audit (`folder/accessibility_tests.rs`) passes with the
   library collapsed and expanded, in Diagram and Split; at 960 × 540 every
   control of the rail, the board toolbar and the inspector is inside the
   window (`assert_within_window`).

## Not in scope

A second tab in the panel (project files in Diagram: KTLO-11). Collapsing
the inspector or the code panel. Any change below 1440 px in Split.

## Technical appendix

- The panel is `FolderApp::library_panel` in `folder/mod.rs`
  (`egui::Panel::left("folder-library")`), called from the Diagram arm and
  from the Split arm when `width >= SPLIT_INSPECTOR_WIDTH`. The rail is the
  same function drawing a fixed-width, non-resizable panel with one
  `widgets::icon_button` when the flag is set.
- "Panel in view" already drives two things that must follow the flag: the
  `library::Context` handed to the board for the popup
  (`canvas::Input`, KTLO-05d) and the empty-board hint in `canvas/mod.rs`.
  Search for the Split `width >= SPLIT_INSPECTOR_WIDTH` condition and make
  both read one predicate, `library_in_view()`.
- Decision 1 = A: `apiforge_storage::project::PanelSizes` tolerates unknown
  fields since KTLO-05c; add `library_collapsed: bool` with
  `#[serde(default)]` there, or a sibling of `panels` in `WorkspaceState`
  (which has `deny_unknown_fields`; an older build would then reject the
  whole file and open the default view — acceptable per the module's
  contract, but the `panels` route is the gentler one). `Workbench::restore`
  and `state()` carry it.
- Decision 2: `FolderApp::shortcuts` in `folder/mod.rs` (Ctrl+S, Ctrl+E,
  Ctrl+Tab, Ctrl+W, undo/redo today); `Key::B` is unused.
- KTLO-12 (one header above the board) moves the board's header, not the
  library's caption row; no conflict. Reuse the `Icon::ChevronRight` /
  `ChevronDown` pair the run results bar uses; `Icon::Panel` or `Layers`
  for the rail button.
- Tests: `folder/library_tests.rs` holds the KTLO-05b/c/d tests to extend;
  the review view goes next to `review_folder_modes_*` in `testing.rs`.
