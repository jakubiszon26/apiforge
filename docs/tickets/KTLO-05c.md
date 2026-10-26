# KTLO-05c: The block library panel's width is remembered per project

- Size: S · Owner review: 10 min · Risk: format
- State: Awaiting acceptance (issue #22)
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

The width of the *Block library* panel is remembered per project like the
other panels, so reopening a project restores it.

## What the owner will see

- New: the library panel's width survives closing and reopening the project.
- Changed: a workspace state written by this build gains a `library` panel
  size. A build *older* than this one no longer rejects such a file: unknown
  panel sizes are ignored instead.
- Removed or hidden: nothing.
- Migrated or converted: nothing (the field has a default; files without it
  load as before).
- Default behaviour that changes: none.

Owner decision: option A on #8 (2026-10-26), as above.

## Mock-up

Not applicable: no visible layout change; only persistence.

## Acceptance criteria

1. A workspace state with `panels.library` round-trips through save and load
   with the same value.
2. A workspace state without `panels.library` loads with the default width.
3. A workspace state with an unknown panel-size field (simulating a newer
   build) loads; the unknown field is dropped, every known field keeps its
   value. `deny_unknown_fields` stays on the documents and the manifest.
4. In the folder window, resizing the library panel, saving and reopening the
   project restores the width (interaction test through the named panel).

## Not in scope

- Hiding the library panel in Split (backlog, *Later*).
- The popup sections below 1440 px (KTLO-05b PR B).

## Technical appendix

`PanelSizes` (`apiforge-storage`, `project/workspace.rs`) drops
`deny_unknown_fields` and gains `library: f32` defaulting to 240, validated
like the other sizes. `folder-library` (`folder/mod.rs`) opens at
`workbench.panels.library` and records its outer width each frame; the
workbench persists it with the other sizes.

## Actual

Changed lines: see the pull request. One round.
