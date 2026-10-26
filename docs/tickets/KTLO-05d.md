# KTLO-05d: The Add step popup lists Variables and Operators below 1440 px

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Approved (issue #23; second half of KTLO-05a criterion 5, option A)
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

In Split below 1440 px, where the *Block library* panel is hidden, the
existing *Add step* popup offers the same Variables and Operators sections as
the panel, so nothing is reachable only at wide windows.

## What the owner will see

- New: Variables and Operators sections in the *Add step* popup when the
  panel is not shown (Split below 1440 px).
- Changed: nothing at 1440 px and wider, nor in Diagram, where the panel
  carries the sections.
- Removed or hidden: nothing.
- Migrated or converted: nothing.
- Default behaviour that changes: none.

## Mock-up

The approved KTLO-05a mock-up covers the panel; the popup is the existing one
with two more sections. Layout: the two sections are stacked under the
existing steps, inside the popup's existing scroll area, with the panel's own
captions (*VARIABLES* with *Add variable*, *OPERATORS* with its groups); the
popup keeps its 300 px width and stays within 960 × 540. This is the obvious
layout (no tabs, no new control), so no design decision was put to the owner;
captures of the popup open at 960 × 540 and 1600 × 1000 are in the QA report.

## Acceptance criteria

1. At 1439 px and below in Split, the popup lists the document's variables
   with *Add variable*, and the operator chips; each adds exactly as the
   panel does (at the board's visible centre, selected, source updated, tab
   dirty).
2. At 1440 px and above, the popup is unchanged (the panel carries the
   sections).
3. The accessibility audit passes with the popup open.

## Not in scope

Hiding the panel in Split by choice (backlog, *Later*); drag-and-drop out of
the popup.

## Technical appendix

`library.rs` splits the Variables and Operators parts of
`Library::show_blocks` into functions the popup calls too, so both show the
same chips. The folder window builds the panel's `library::Context` in one
place and hands it to the board (`canvas::Input`) when the panel is not in
view; `canvas/menu.rs::ActionMenu` shows the sections when it has one, and the
board turns their actions into the same `Command`s the panel's click sends,
at its visible centre.
