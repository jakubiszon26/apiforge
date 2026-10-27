# MCP-05: Publish a component from the server

- Size: S · Owner review: 15 min · Risk: none on screen; a new Git-tracked
  file per revision (decision 3)
- State: Backlog (issue #64) · MCP-00 decision 3 = A taken 2026-10-27 ·
  after MCP-04 · revised 2026-10-27 (MCP-00 revision 2: comment-loss
  consent, the expected hash, lost responses)
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

A component the model wrote can be used: `publish` turns the working
component into its next immutable revision, so a scenario may call
`handle@2(…)` and run — exactly what *Publish* in the window does, including
the question the window asks before a comment would be lost.

## What the owner will see

- New: the tool `publish`. On disk: `.apiforge/revisions/<handle>/<n>.afs`,
  the revision's entry and checkpoint in `apiforge.project.json` and
  `.apiforge/diagram.json`, and the working document's base revision
  advanced — the same transaction as the window's *Publish*.
- Changed, removed, migrated, defaults: nothing.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `publish(path, expected_source_sha256)` on a valid, saved component whose
   hash matches writes the next unused revision and returns the handle, the
   revision number, its `source_sha256`, `generation`, `wrote: true`;
   `project_overview` then lists the revision with that hash and `check_text`
   accepts a call to it.
2. A different hash on disk is `stale_source`, nothing written.
3. Nothing existing is overwritten: publishing twice yields revisions `n`
   and `n + 1`, both files intact.
4. A component whose canonical printing would drop a comment is refused
   with `comments_would_be_lost` listing the comments, nothing written; the
   same call with `accept_comment_loss: true` and the same hash publishes;
   with a different hash it is `stale_source` and the consent does not carry
   over.
5. A component with errors, a document that is not a component, and a
   project whose checkpoint file is unreadable are each refused with the
   session's own message (`invalid_text`, `not_a_component`,
   `invalid_checkpoint`) and write nothing.
6. `request_id`: a second `publish` with a `request_id` the server has
   already answered returns the recorded result and creates no revision; a
   fresh server does not remember it (the overview's hashes are the way to
   tell).
7. The agent-session rules of MCP-04 apply: refused with the holder while a
   window or another process holds the project; the lock stays with the
   session afterwards.
8. The window, open read-only beside the server, lists the new revision
   after its project reload (manifest changed, nothing unsaved) — headless
   interaction test.
9. The verification commands of the development guide pass.

## Not in scope

Renaming a published component; deleting a revision; choosing the revision
number (always the next unused); a consent that outlives the hash.

## Technical appendix

- `publishable` then `publish` (`crates/apiforge-project/src/publish.rs:33,73`)
  on the agent session's `ProjectSession`; `accept_comment_loss` binds to
  `source_version` in the session (`comments.rs`), which the server calls only
  after the hash check, so the binding to the hash holds.
- The `request_id` table is a bounded `HashMap<String, PublishResult>` in the
  server state, shared with MCP-06's live runs.
- Tests in `crates/apiforge-mcp/tests/acceptance.rs` with the demo shop
  (three components at revision 1) and a component with an unattached
  comment; `crates/apiforge-project/tests/publish.rs` is the pattern.
- Tool annotation `destructiveHint: false`, `idempotentHint: false`.
