# MCP-04: Save a scenario or component from the server

- Size: M · Owner review: 30 min · Risk: none on screen; the project's
  Git-tracked files are written by a new caller (decisions 1, 2, 5)
- State: Backlog (issue #63) · MCP-00 decisions 1, 2 and 5 = A taken
  2026-10-27 · after MCP-01
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

The model saves what it wrote: `write_source` replaces a document's text and
`create_document` registers a new scenario or component — each checked
first, written exactly, under the project's writer lock, and never while the
window has the project open for editing.

## What the owner will see

- New: two tools, `write_source` and `create_document`. On disk: the `.afs`
  file, the checkpoint `.apiforge/diagram.json` (identities and positions,
  as the window writes on save) and, for a new document, its entry in
  `apiforge.project.json` — the same files, the same bytes, as a save in the
  window. In the window, if it is open read-only beside the server: the
  changed file reloads within a second; a new document appears in the
  explorer after the project reload notice.
- Changed: nothing in the window. A write while the window holds the
  project is refused with a message that says so (decision 5, A).
- Removed, migrated, defaults: nothing.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `write_source(path, text)` on a registered document opens the project as
   the writer, replaces the text, settles the analysis and, when the text has
   no error, saves it exactly (the bytes on disk equal the argument), writes
   the checkpoint, releases the lock and returns the diagnostics (warnings
   allowed) and the saved generation. The `.gitignore`, `.gitattributes` and
   every other file are untouched.
2. With decision 2 = A, a text with errors is not saved: the tool returns
   `saved: false` and the diagnostics with line and column, and the file on
   disk is unchanged.
3. `create_document(kind, path, text)` registers a new scenario or component
   at a path ending in `.afs` outside `.apiforge`, writes the file and the
   manifest in one transaction, and returns the new document's path and
   handle; the kind must match the text's header (`scenario`/`component`),
   otherwise a tool error. Creating a path that exists, or a handle another
   component already uses, is a tool error and writes nothing.
4. A write while another process holds the writer lock is a tool error
   quoting `The folder project is open in another writer session.` plus the
   advice to close the project in the window or edit the file directly;
   nothing is written; a read tool in the same session still works.
5. The server holds the lock only during the call: after `write_source`
   returns, `ProjectStore::open_exclusive` on the folder succeeds at once.
6. A write that meets an interrupted transaction or an unreadable registry is
   refused with the session's own message and writes nothing (recovery stays
   the window's job).
7. Identities survive: a scenario saved by the server, then opened in the
   window, keeps the block positions of blocks whose handle and kind did not
   change (a test opens the saved project with `ProjectSession::open` and
   compares node ids and positions with the checkpoint).
8. The window test: with a read-only window on the project (headless
   `egui_kittest`), `write_source` on a clean document is followed within
   one scan and shows no conflict; on a document with unsaved work in the
   window it shows *Changed outside API Forge while unsaved here* and leaves
   the unsaved text in place.
9. The verification commands of the development guide pass.

## Not in scope

Publishing (MCP-05). Renaming, moving or deleting documents (`relocate`,
`delete` exist; nothing asks for them). Saving project configuration
(environment, contracts). Writing a plain text file (`README.md`). A hand-over
of the lock between the window and the server (MCP-00 decision 5, B and
beyond). Formatting the text on save: the bytes are saved as given, as `apply`
does; the model can `print` first.

## Technical appendix

- `write_source` is `folder.rs::execute` for `apply` as a function:
  `open_saved_exclusive` → `select` → `edit_source` → `settle` →
  `check_document` → `save` (`session.rs:881`, commits text, checkpoint and
  dirty manifest in one transaction). `create_document` is
  `session.rs:390` with the header check via `register.rs::declaration`.
- Lock per call: the session is created and dropped inside the tool; the
  lock is an advisory OS lock on an open file, released on drop. `Locked`
  comes from `ProjectStore::open_exclusive` (`transaction.rs:168`).
- The window side is already there: `watcher.rs` + `external.rs::apply_external`
  (clean documents follow, dirty get `ExternalConflict::Changed`, a changed
  manifest reloads the project when nothing is unsaved). Criterion 8 reuses
  the helpers of `folder/project_interaction_tests.rs` (`follow_disk`).
- Tool annotations: `readOnlyHint: false`, `destructiveHint: false` for
  `create_document` (it never overwrites) and `true` for `write_source` (it
  replaces a file); Claude Code asks the person before each call unless they
  allow the tool.
- Size: tools 120, CLI sharing 40, tests 350, docs 40 — M.
