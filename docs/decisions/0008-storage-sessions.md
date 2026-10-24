# ADR 0008: Single-writer storage sessions and deliberate recovery

- Status: accepted; Linux verified, native Windows verification pending
- Date: 2026-10-23
- Scope: `apiforge-storage`, desktop persistence, future CLI and code drafts
- Milestone: S1 of the [code and canvas plan](../roadmap.md)

Stage II amendment (planned, not yet implemented):
[ADR 0011](0011-project-transactions.md) extends the lock, fingerprint and recovery
contracts to folder-project transactions. Legacy `WorkspaceStore` sessions keep
the behaviour below; native Windows verification remains a release gate.

## Decision

`WorkspaceStore::open_exclusive(path)` holds an exclusive advisory
`std::fs::File::try_lock` lock on `workspace.lock` in the containing directory.
The directory lock covers the workspace, backups and `drafts.json`, including
when callers use different workspace filenames in the same directory. Clones and
the background saver share the lock until the last handle is dropped. The lock
file stays on disk; its existence is not evidence of a live writer. The OS releases
the lock after process termination, so stale PID files and forced takeover are
unnecessary. Never delete or replace the lock file to unlock a workspace.

The desktop falls back to `open_read_only` when the lock is held by another
window. It displays a persistent warning and permits session-only editing and
export, with no autosave. Every storage mutation also refuses read-only access.

Each load establishes a byte count and SHA-256 fingerprint of the primary and
backup. Each save checks both against the last observed state before touching
either file. Replaced, created, deleted and equal-length modified files all cause
`ExternallyModified`. A mutex serializes operations across clones within a process.
SHA-256 uses the already-pinned `ring` dependency. Reads are bounded to 16 MiB.

This protects cooperating API Forge writers and detects external changes before
saving; advisory locks cannot prevent an unrelated program from writing during
the save itself. A failed write is never acknowledged as saved. If a partial write
changed fingerprints, a reload is required before retrying.

## User decisions

- An external change pauses autosave. The modal offers **Reload and discard local
  changes** or **Continue without saving**. The latter retains the in-memory
  document for editing/export and releases the writer. There is no automatic
  overwrite path. Reload runs on the serial persistence worker, discards queued
  saves and resets board state and history to the loaded workspace.
- Loading a corrupt or missing primary can return a backup candidate, but it does
  not apply it to the editor or authorize a save. **Restore backup** explicitly
  restores the previewed backup; **Continue without saving** (also Escape) opens
  a temporary session. The primary and backup fingerprints are checked again at
  confirmation. A newer/legacy workspace or oversized file is never recovered
  by silently falling back to an older backup.
- Closing while a decision is pending cancels the close request and leaves the
  decision usable. Failed-save retry handling remains available for I/O errors.

## Code drafts

S1 provides bounded, versioned `Drafts` / `CodeDraft` storage ahead of the M6
editor. Entries contain project and scenario/component identity, a base SHA-256
hash, source text and a UTF-8 cursor offset. Duplicate contexts, invalid offsets,
invalid hashes and text above 1 MiB per buffer are rejected before writing. The
caller supplies known secret values; source containing one is rejected. Callers
must store source with environment references, never text with secrets substituted.

Drafts use the same directory lock, independent primary/backup fingerprints,
atomic writes and explicit backup recovery. Unknown versions are preserved.
Workspace recovery never applies drafts. The M6 editor will restore buffers and
handle base-hash mismatches according to [ADR 0007](0007-code-editing-state.md).
There is no cross-file transaction: the base hash is how M6 recognizes a workspace
and draft saved at different times.

## Evidence

Automated tests cover another process holding the lock and releasing it after
being killed, clone lifetime, read-only mutations, equal-length edits, file
creation/deletion, backup replacement, confirmation races, draft validation,
queued saves during conflicts and the desktop recovery/reload decisions.

The read-only secondary window, an external-change conflict with reload and
deliberate backup recovery were also exercised in the native application on Linux,
with an isolated home and fixture data, at 1600 × 1000 and at the 960 × 540 minimum.

Windows uses the same standard-library locking API, with a writable handle for
file flushing. Native Windows process/GUI execution and power-loss durability
remain release gates; Linux results do not close those gates.
