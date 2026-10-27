# MCP-05: Publish a component from the server

- Size: S · Owner review: 15 min · Risk: none on screen; a new Git-tracked
  file per revision (decision 3)
- State: Backlog (issue #64) · MCP-00 decision 3 = A taken 2026-10-27 ·
  after MCP-04
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

A component the model wrote can be used: `publish` turns the working
component into its next immutable revision, so a scenario may call
`handle@2(…)` and run — exactly what *Publish* in the window does.

## What the owner will see

- New: the tool `publish`. On disk: `.apiforge/revisions/<handle>/<n>.afs`,
  the revision's entry and checkpoint in `apiforge.project.json` and
  `.apiforge/diagram.json`, and the working document's base revision
  advanced — the same transaction as the window's *Publish*.
- Changed, removed, migrated, defaults: nothing.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `publish(path)` on a valid, saved component writes the next unused
   revision and returns the handle, the revision number and the generation;
   `project_overview` then lists the revision and `check_text` accepts a call
   to it.
2. Nothing existing is overwritten: publishing twice yields revisions `n`
   and `n + 1`, both files intact.
3. A component with errors, a document that is not a component, a component
   with an external conflict, and a project whose checkpoint file is
   unreadable are each refused with the session's own message
   (`InvalidModel`, `NotAComponent`, `ExternalConflict`, `InvalidCheckpoint`)
   and write nothing.
4. The lock rules of MCP-04 apply: refused while the window holds the
   project, held only for the call.
5. The window, open read-only beside the server, lists the new revision after
   its project reload (manifest changed, nothing unsaved) — headless
   interaction test.
6. The verification commands of the development guide pass.

## Not in scope

Renaming a published component (Stage II does not); deleting a revision
(nothing does); choosing the revision number (always the next unused).

## Technical appendix

- `publishable` then `publish` (`crates/apiforge-project/src/publish.rs:33,73`)
  on an exclusive saved session; `Published { component_id, revision,
  generation }` is the result. `publish` validates the stable saved
  dependency set itself, so the tool adds no check of its own.
- Tests in `crates/apiforge-mcp/tests/acceptance.rs` with the demo shop
  (three components at revision 1) and `crates/apiforge-project/tests/publish.rs`
  as the pattern.
- Tool annotation `destructiveHint: false`, `idempotentHint: false`.
