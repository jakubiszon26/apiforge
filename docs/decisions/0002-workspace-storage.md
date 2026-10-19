# ADR 0002: Workspace storage as an atomically replaced JSON document

- Status: accepted
- Date: 2026-10-19
- Scope: `apiforge-storage`

## Context

The prototype kept the whole workspace in browser `localStorage` as a version-one
workspace JSON document, saved 350 ms after the last edit. The earlier migration plan
proposed SQLite for the native application. The current priority is feature parity with
the prototype, without losing the durability guarantees of release gate G1 (no
truncated or half-written document after a crash, a recoverable last good copy, no
secrets on disk).

The workspace is small (import limit 16 MiB, typically kilobytes), edited by one user in
one process, and always loaded and saved as a whole. Run history is session-only.

## Decision

The workspace is stored as a single version-one workspace document at
`<data dir>/apiforge/workspace.json` (`~/.local/share/apiforge` on Linux,
`%APPDATA%\apiforge` on Windows):

1. The document is validated with the importer's rules before writing.
2. Bytes go to a temporary file in the same directory, which is synced to disk and then
   atomically renamed over the target (`MoveFileEx` with replace semantics on Windows).
   On Unix the directory is synced as well.
3. Before a save, the previous file is copied to `workspace.json.bak`, but only when it
   is a readable document, so a corrupt file never replaces the last good backup.
4. Loading falls back to the backup when the main file is unreadable and tells the user;
   the corrupt main file stays on disk until the next successful save.
5. Values of variables listed in `environment.secretNames` are never written.

## Consequences

- Storage, export and import share one format and one validator; a saved workspace can
  always be opened with File → Import.
- No schema migrations are needed while the document format stays at version one; a
  future format version needs an explicit migration with a backup, as for imports.
- Conflicts between two running instances are not detected yet (gate G1, item 5). A
  lock file or revision check must be added before release.
- SQLite remains an option if run history or large workspaces need incremental
  persistence; this decision does not preclude it.
