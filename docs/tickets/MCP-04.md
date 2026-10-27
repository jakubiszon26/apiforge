# MCP-04: Save and create from the server — the agent session

- Size: M · Owner review: 30 min · Risk: none on screen; the project's
  Git-tracked files are written by a new caller (decisions 1, 2, 10)
- State: Backlog (issue #63) · MCP-00 decisions 1, 2 = A taken 2026-10-27;
  decision 10 = A taken 2026-10-27 · after MCP-03 · revised 2026-10-27 (MCP-00 revision
  2: the agent session replaces the lock per write; validation before the
  first write; versions; secrets in the text; paths)
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

The model saves what it wrote: `write_source` replaces a document's text and
`create_document` registers a new scenario or component — each checked in
the project's context first, written exactly, based on the version the model
read, under the writer lock the server holds for the whole agent session,
and never while a window or another process holds the project.

## What the owner will see

- New: three tools, `write_source`, `create_document`, `finish_editing`. On
  disk: the `.afs` file, the checkpoint `.apiforge/diagram.json` and, for a
  new document, its entry in `apiforge.project.json` — the same files, the
  same bytes, as a save in the window, and for a new document all three in
  one transaction. A window open read-only beside the server follows the
  change as it follows `apply` today (MCP-08a makes it an observer).
- Changed: nothing in the window. A write while the window holds the project
  is refused with `held_by_window` and the holder's description (the
  hand-over comes with MCP-08b).
- Removed, migrated, defaults: nothing.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `write_source(path, text, expected_source_sha256)` on a registered
   document whose saved hash matches: the text replaces the document's,
   the analysis settles and, when the text has no error, the bytes on disk
   equal the argument, the checkpoint is written, and the result has
   `wrote: true`, `changed` (the paths committed), the new `source_sha256`,
   `generation` and the diagnostics (warnings allowed). Every other file is
   untouched.
2. A text with errors is not saved (decision 2): `wrote: false`, the
   diagnostics with positions, the file unchanged.
3. A different hash on disk is `stale_source` with the current
   `source_sha256`, nothing written; `expected_generation`, when given and
   different, is `stale_project`, nothing written. A test changes the file
   with `apply` between the model's read and its write.
4. `check_new_document` validates a proposed new document without writing:
   path (`working_source`, not on disk, not registered, no case-insensitive
   collision), header kind against `kind`, handle against the registered
   handles of that kind, and the text analysed in the project's context;
   `create_document(kind, path, text)` refuses on any error — `path_exists`,
   `handle_exists`, `path_rejected`, `invalid_text` with the diagnostics —
   and then no source, no registry entry and no checkpoint exist.
5. On success `create_document` commits source, registry entry and checkpoint
   **in one transaction** (one head generation advance; a fault injected
   between the staged files leaves nothing installed before activation) and
   returns the path, handle, `source_sha256`, `generation`, `wrote: true`.
6. A failure after activation (fault injected on the checkpoint's install)
   answers `recovery_required` with `wrote: "unknown"`; every later write of
   the session answers the same; the window's recovery view finishes the
   operation once it holds the project (an existing test pattern of
   `transaction_view`).
7. The agent session: the first write takes the writer lock and the server
   keeps it across calls — after `write_source` returns,
   `ProjectStore::open_exclusive` on the folder **fails** with `Locked`, and
   succeeds within a second of `finish_editing` or of EOF on the server's
   stdin. The lock file's description names `apiforge mcp`, the role
   `agent`, the start time and the client when known (MCP-07's format).
8. A write while another process holds the lock is a tool error quoting the
   holder's description (`held_by_window` when it is a window, `held_by_other`
   otherwise), `wrote: false`, nothing written; read tools in the same
   session still work; no advice to edit the file directly.
9. A text that contains a known secret value (set through
   `APIFORGE_SECRET_<NAME>` for the test) is refused by `write_source` and
   `create_document` with `secret_in_text`, the value not echoed, nothing
   written.
10. Identities survive: a scenario saved by the server, then opened in the
    window, keeps the positions of blocks whose handle and kind did not
    change; new blocks have the identities the server sealed (a test opens
    the saved project with `ProjectSession::open` and compares node ids and
    positions with the checkpoint).
11. The window test: with a read-only window on the project (headless
    `egui_kittest`), a `write_source` on a clean document is followed within
    one scan and shows no conflict; on a document with unsaved work in the
    window it shows *Changed outside API Forge while unsaved here* and leaves
    the unsaved text in place (today's behaviour; MCP-08a replaces the
    follow with a reload).
12. The verification commands of the development guide pass.

## Not in scope

Publishing (MCP-05). Renaming, moving or deleting documents. Saving project
configuration. Writing a plain text file. The hand-over from a window
(MCP-08b) and the observer (MCP-08a). Formatting on save: the bytes are
saved as given, as `apply` does; the model can `print` first.

## Technical appendix

- The agent session: one `ProjectStore` from `open_exclusive` held in the
  server state; each call builds a fresh `ProjectSession` from a clone of
  that store (new `ProjectSession::from_store_saved(store)`, the public form
  of `load_saved`); the store is dropped on `finish_editing`, EOF and exit,
  which releases the OS lock.
- `write_source`: `edit_source` → `settle` → the hash check against
  `document.saved` → `save` (`session.rs:900`), whose transaction carries
  `expected: document.saved` as the storage's own guard.
- `check_new_document`: the analysis input of a synthetic `DocumentEntry`
  through `analysis_for`-like construction without pushing a
  `DocumentSession`; `register.rs::declaration` for the header; handle
  comparison over `documents()`.
- `create_document` for MCP: a new `create_document_sealed` on the session
  that commits `[parent dirs, Write source (expected Missing), Write
  diagram.json (expected diagram_saved), Write manifest]` with the record
  from `DiagramRecord::capture` on the validated model.
- Lock description: `open_exclusive` writes it (MCP-07 defines the format;
  this ticket writes the agent's fields if MCP-07 has merged, otherwise
  nothing and MCP-07 fills it).
- Tool annotations: `destructiveHint: false` for `create_document`, `true`
  for `write_source`; `idempotentHint: true` for `write_source` (the same
  text and hash twice is a no-op).
- Tests in `crates/apiforge-mcp/tests/acceptance.rs` with the demo shop; the
  fault injection uses the storage's I/O seam (`fault_tests.rs` pattern).
