# MCP-08c: The hand-over prompt when the window has unsaved work

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Backlog (issue #92) · MCP-00 decision 13 = A taken 2026-10-27 ·
  mock-up M2 approved 2026-10-27 · after MCP-08b
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

When the agent asks for the project while the person has unsaved work, the
window asks the person instead of refusing: save everything and hand over,
or keep editing. Nothing is overwritten, discarded or declared saved on their
behalf.

## What the owner will see

- New: a prompt *An agent asks to edit this project* listing the unsaved
  documents, fields, configuration and layout, with *Save all and hand over*
  and *Not now*. *Save all and hand over* saves everything (invalid text
  included, as Ctrl+S does), then hands over as MCP-08b does. *Not now*
  leaves the person editing; the agent gets `handover_refused` with *the
  person chose to keep editing*. *Discard and hand over* (decision 13 B)
  was declined: 13 = A.
- Changed: MCP-08b's refusal with unsaved work becomes this prompt.
- Removed, hidden, migrated, defaults: nothing.

## Mock-up

M2, approved 2026-10-27: `docs/qa/2026-10-27/MCP-00/m2-*.png` — the prompt
at 960 × 540 and 1600 × 1000 with two unsaved documents and a dirty project
setting listed.

## Acceptance criteria

1. With unsaved text in two documents and a dirty project name, the agent's
   write opens the prompt listing all three; the agent's call waits up to 30
   s, then returns `handover_pending` with nothing written; the prompt stays.
2. *Save all and hand over*: every listed item is saved (bytes on disk equal
   the unsaved text, the manifest carries the new name), the window becomes
   the observer, and the agent's next call succeeds; a save that fails (a
   storage seam) keeps the window as the writer, shows the error, and the
   agent is refused with the reason.
3. *Not now*: nothing is saved, the window stays the writer, the agent's
   pending or next call is `handover_refused` with *the person chose to keep
   editing*; the agent's call after a later Ctrl+S in the window is granted.
4. A held recovery draft (a restored draft not yet decided) counts as unsaved
   work and is named in the prompt; the prompt offers no way to decide it
   (the existing recovery prompt does).
5. The prompt stays inside the window at 960 × 540; Escape is *Not now*.
6. The QA report compares the prompt against M2; the verification commands
   of the development guide pass.

## Not in scope

Discarding work from this prompt (decision 13 B); a second prompt when the
agent asks again after *Not now* (it asks again; the person sees the prompt
again — by design).

## Technical appendix

- `Prompt::Handover { request, unsaved: Vec<UnsavedItem> }` rendered by
  `file_prompts.rs`'s pattern; *Save all and hand over* calls `save_all()`
  and, on every result `Ok`, the MCP-08b hand-over.
- The list comes from the session: dirty documents, documents with
  `fields`, `manifest_dirty`, `layout_dirty`, `recovery.held`.
- Tests in `folder/acceptance_tests.rs` with the server spawned as in
  MCP-08b.
