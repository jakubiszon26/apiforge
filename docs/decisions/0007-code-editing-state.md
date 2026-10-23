# ADR 0007: Code buffers, drafts and history

- Status: accepted
- Date: 2026-10-23
- Scope: `apiforge-desktop` (code panel, editor state), `apiforge-storage`
- Plan: [scenario code and canvas](../plans/code-and-canvas.md)

## Context

[ADR 0005](0005-scenario-language.md) makes the document the truth and code a
printed projection. While text is being typed it is often invalid, and the
document cannot represent it. The history in `editor.rs` stores graphs, so it
cannot undo keystrokes in text that never became a graph. Without explicit rules
unfinished code would be lost on reprinting, on switching scenarios, on closing the
application or after a crash, and Run would execute something other than what the
user sees.

## Decision

### One buffer per editing context

Every scenario (with its cleanup block) and every open component draft has at most
one **code buffer**. A request body opened in the panel is a view of the same
scenario, not a separate buffer: its text is a fragment of the scenario's code and
synchronises through it. A buffer is in one of three states:

| State | Meaning | Board |
| --- | --- | --- |
| **Clean** | text equals `print(document)` | editable |
| **Applied** | text differs only in formatting from a valid lowering that has been applied | editable |
| **Invalid** | text has errors; the document holds the last valid version | read-only with a "code has errors" banner |

A buffer that is Clean is not stored: it is reprinted when needed.

### Synchronisation

- **Text → document.** 250 ms after the last keystroke the buffer is parsed and
  lowered. Without errors it is reconciled ([ADR 0005](0005-scenario-language.md#identity-during-edits))
  and applied as one document edit. The buffer becomes Applied and keeps the user's
  text until the next reprint; it is not reformatted while it has focus.
- **Document → text.** A board edit, an inspector edit, a document undo or redo
  reprints a Clean or Applied buffer. The cursor is restored to the statement it was
  in, by handle. An Invalid buffer is never reprinted. Document edits on its context
  are disabled; the banner offers *Discard code changes* (reprint) and *Show
  errors*.

### Two histories with one rule

- The **text history** belongs to the buffer. While the code panel has focus,
  Ctrl+Z and Ctrl+Y undo and redo keystrokes, including those that left the text
  invalid. An undone keystroke re-enters synchronisation like any other.
- The **document history** stays in `editor.rs`. Each applied synchronisation is
  one document entry. Consecutive synchronisations of the same buffer within the
  existing merge window form one entry.
- The focused panel decides: code panel → text history, otherwise → document
  history. A document undo or redo reprints the buffer and clears its text history
  beyond that point, because those keystrokes no longer describe the document.
  Document undo is unavailable while the buffer is Invalid.

### Drafts survive everything

- An Invalid or Applied-but-unformatted buffer is a **draft**. Drafts are kept in
  memory per context, so switching scenario, project or component and returning
  shows the same text, cursor and diagnostics.
- Drafts are persisted next to the workspace (`drafts.json` beside
  `workspace.json`), keyed by project, scenario or component revision, with the
  hash of the printed document they were based on. They are written with the same
  atomic-write rules as the workspace ([ADR 0002](0002-workspace-storage.md)) on the
  autosave schedule and on close. They never contain secret values, because code
  names secrets but does not hold them.
- Closing the application saves drafts and does not block. On the next start an
  Invalid draft is restored into its buffer. When the document changed since the
  draft was taken (a different base hash), the panel shows the draft beside the
  current code and asks whether to keep editing the draft or discard it.
- Deleting a scenario or component deletes its drafts. Import and storage
  recovery never apply drafts automatically.

### Run

Run always executes the document, never the buffer. With an Invalid buffer in the
scenario being run, Run asks once: *The code has errors. Run the last valid
version?* With an Applied buffer nothing differs, because it is already in the
document.

## Consequences

- No keystroke is lost by reprinting, switching, closing or crashing after the last
  draft write. The loss window equals the autosave interval.
- The board is temporarily read-only while its code is invalid. This is deliberate:
  editing both would require merging a graph change into broken text.
- Storage gains a second file. It shares the single-writer lock and change
  detection of the storage hardening milestone (S1), which therefore lands before
  the code panel.
