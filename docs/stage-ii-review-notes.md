# Stage II review notes

## Shared diagram checkpoint file and Git conflicts

- Recorded: 2026-10-24
- Source: user review
- Status: open, severity reduced by the reviewer; consider when implementing
  shared checkpoint persistence
- Applies to: [ADR 0010](decisions/0010-folder-projects.md), P1 diagram schema

Stage II makes `.afs` files the durable definitions so that projects work well
with Git, but ADR 0010 currently places every layout/identity checkpoint in one
shared `.apiforge/diagram.json` file. This creates a potential merge hotspot.

Saving a changed scenario can update its `sourceHash`, semantic digest and the
full `identityBase` snapshot, including positions. Two branches editing different
scenarios could therefore have clean `.afs` merges while both modify the same
large checkpoint file. Full snapshots and unstable record ordering could make
those conflicts frequent and cumbersome to resolve during pull requests. The
actual conflict frequency has not yet been measured.

The preferred proposal from the review is to split checkpoints into separate
files keyed by stable document ID or published `(componentId, revision)`, for
example `.apiforge/diagrams/<document-id>.json`, with a separate namespace for
published revisions. This would isolate independent document changes in Git.

The alternative proposal is deterministic ordering of keys and records and a
serialization layout suitable for `git merge-file`. Deterministic output removes
ordering noise, but its effectiveness for independent edits still needs a merge
test with realistic full snapshots.

Before implementing checkpoints, decide the file granularity and verify a merge
of two branches that edit different scenarios. If the format changes, update
ADRs 0010/0011, managed-path validation and transaction/load coverage together.
This note records the concern and proposals; it does not amend the accepted
format by itself.

The reviewer subsequently softened the original severity assessment and confirmed
that ADRs 0011/0012 cover multi-file transactions and session ownership. Keep the
Git merge concern as an ergonomics question; it does not invalidate the accepted
roll-forward architecture or require a format change on its own.

## Line endings, Git checkout and source integrity

- Recorded: 2026-10-24
- Source: user review
- Status: open; decide the project initialisation policy before cross-platform
  folder acceptance
- Applies to: [ADR 0010](decisions/0010-folder-projects.md),
  [ADR 0011](decisions/0011-project-transactions.md),
  [ADR 0012](decisions/0012-project-sessions.md)

Git can convert LF files in its index to CRLF working-tree files when
`core.autocrlf=true`. This can change an `.afs` file's exact SHA-256 and byte
length without changing the intended definition. A shared checkpoint can then
lose its exact-source match and need reconciliation. More seriously, a published
revision recorded with an LF source hash can fail the folder loader's integrity
check after a CRLF checkout. Changes to JSON line endings also change raw storage
fingerprints, although a parsed JSON field's stored digest string is unaffected.

The review proposes emitting canonical/generated project files with LF and
creating or appending the following rules during explicit folder initialisation,
preserving existing `.gitattributes` content:

```gitattributes
*.afs text eol=lf
*.json text eol=lf
```

Git documents how [`text`, `eol` and configuration interact](https://git-scm.com/docs/gitattributes#_text).
Account for existing or nested overriding attributes, already-tracked CRLF files
and the scope of the JSON pattern. Merely appending rules does not rewrite all
existing files. Do not automatically run `git add --renormalize`, modify the index
or rewrite imported originals. Ordinary source Save still preserves the exact
editor text under the accepted Stage II contract; forcing LF on every save would
require an explicit change to that contract.

The review also proposes considering CRLF-to-LF normalisation before computing
`sourceHash`. Keep this separate from transaction fingerprints: expected/target
SHA-256 and length must continue to describe exact disk bytes so that recovery
detects foreign edits and verifies staged payloads. If source/cache/revision
hashes gain newline normalisation, specify and version that policy, verify
language equivalence and preserve source offsets and exact text. Do not silently
reinterpret existing hashes or accept a mismatched immutable revision.

Acceptance should include LF publication on Linux followed by a Windows checkout
with `core.autocrlf=true`, both with and without the generated attributes; exact
CRLF source saving; manually changed line endings; and transaction recovery with
a line-ending-only foreign edit.

## Local head after an external checkout

- Recorded: 2026-10-24
- Source: user review
- Status: open for reload-generation policy; stable-read limitation clarified in
  ADR 0011
- Applies to: [ADR 0011](decisions/0011-project-transactions.md),
  [ADR 0012](decisions/0012-project-sessions.md)

`head.json` is local and ignored by Git. Switching branches can therefore replace
shared sources, registry and checkpoints while leaving the same committed local
generation. Fingerprint rechecks detect inputs that change during a read, but a
Git checkout can pause with a partly replaced tree whose bytes remain unchanged
through both passes. Unchanged head and fingerprints do not prove that every
file belongs to one Git checkout. The same limitation exists when no API Forge
writer is running.

The review proposes that the writable `ProjectSession`, after detecting an
external change and validating a reload candidate, advance storage `generation`
through a serialised, durable local metadata operation. Such an operation would
invalidate readers spanning the bump. Define its expected fingerprints, failure
behaviour and interaction with active transactions and dirty/conflicted documents
before implementing it. A read-only opener must not advance head. A reload must
not silently authorise overwriting external changes or replace dirty source.

This is a useful invalidation mechanism to evaluate, but it cannot make an
uncoordinated checkout atomic: a reader may finish before detection, entirely
after the bump, or while Git is paused between replacements. Debouncing or
repeated scans can reduce these races without proving their absence. Stage II
must retain the explicit best-effort boundary for external writers unless a
separate coordination or immutable-snapshot protocol is adopted.

Keep storage `generation` separate from the in-memory analysis ticket's
`projectContextGeneration` and `sessionEpoch`. External configuration/dependency
changes must invalidate analysis; reloading a session must invalidate its old
tickets even if local head did not change. Old save baselines remain conflicts
until the external change has been deliberately resolved.

Acceptance should pause an external checkout between file replacements, exercise
readers before and after detection/reload and with no writer open, and verify
that neither stale head nor a generation bump is treated as proof of an atomic
Git snapshot.

## P1 code review

- Recorded: 2026-10-24
- Source: review of the P1 storage increments (`c870fb8`..`86e5d9e`)
- Status: P1 accepted; items 1, 3 and 4 fixed in P2, item 2 resolved by a probe,
  item 5 open
- Applies to: [ADR 0011](decisions/0011-project-transactions.md),
  [ADR 0012](decisions/0012-project-sessions.md)

The reviewed storage passed formatting, clippy with warnings denied and the full
serial workspace test run (661 passed, two parent-launched helpers ignored). The
transaction protocol, stable reads, conflict receipts and trash match ADR 0011.
These points should be handled by the session/explorer work that uses them:

1. **One stale cache entry hides the whole last-valid cache.** `decode_cache`
   rejects the file as foreign when any entry's document has left the registry.
   Trash does not prune cache entries, so trashing a document makes every other
   document's last-valid snapshot unavailable and requires explicit replacement.
   Entries for unregistered documents should be ignored individually, or pruned
   by the operation that unregisters them. *Fixed:* loading ignores such entries
   individually; saving one is still rejected.
2. **Read-only readers report an in-flight commit as recovery.** A stable read that
   sees an active transaction returns `RecoveryRequired` without retrying. For a
   read-only session whose writer is alive this is a transient state and should
   be reported as busy; only an active transaction without a lock owner needs
   recovery. *Resolved differently:* a live writer may also be stuck in its own
   conflict recovery, which the lock cannot distinguish, so reads keep reporting
   `RecoveryRequired`. `ProjectStore::has_live_writer` lets a read-only session
   decide to retry.
3. **Pre-activation failures leave staging directories.** A conflict detected
   after staging, or a failed staged write, returns without removing
   `.apiforge/local/transactions/<id>/`. Nothing removes unreferenced staging
   later. They are never replayed, but they accumulate local data. *Fixed:*
   failures before activation remove their staging, and a writer removes
   unreferenced staging when it opens.
4. **Opening a writer store writes local infrastructure.** `ProjectStore::open`
   creates `.apiforge/local` and the lock in any directory. Sessions must check for
   a manifest with a read-only store first, as `ProjectSession::open` now does.
   Two concurrent first openers can also race in `create_dir` and report an I/O
   error instead of falling back to read-only. *Fixed:* concurrent creation is
   tolerated and an unwritable folder opens read-only. The lock is also retried
   briefly: a process spawned by another thread keeps inherited descriptors until
   its exec, which made reopening a just-closed project intermittently report
   another writer (and made the parallel storage tests flaky).
5. **Storage depends on the language crate.** Cache verification lowers source in
   `apiforge-storage`. This is acceptable while the cache verifies its own
   candidates, but analysis policy otherwise belongs to `apiforge-project`.
