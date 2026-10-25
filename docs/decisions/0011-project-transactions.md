# ADR 0011: Folder storage transactions and deliberate recovery

- Status: accepted for Stage II; P1 storage implemented and fault-tested on Linux; native Windows durability unverified
- Date: 2026-10-24
- Scope: `apiforge-storage`, folder loader, migration and run-journal integration
- Plan: [Stage II](../stage-ii-code-workspace.md)
- Related: [format](0010-folder-projects.md), [sessions](0012-project-sessions.md)

## Context and compatibility

Replacing one JSON file atomically does not atomically update source, registry and
diagram data. A folder operation must either finish durably or leave a complete,
recognisable intent that can be finished after a crash. Choosing the newest file
from each part is not recovery.

For folders this decision extends/replaces ADR 0002's single-file save and ADR
0008's workspace-specific lock/fingerprint container. Their legacy JSON behaviour
remains. ADR 0009's write-before-request, redaction and acknowledgement semantics
remain; the journal gains a project-session lock owner independent of WorkspaceStore.

## Lock, versions and stable reads

`ProjectStore` opens `.apiforge/local/project.lock` exclusively using the existing
OS-lock pattern. Cloneable ownership spans storage workers and the journal. Never
unlink, replace or force-take-over a held lock. A second process opens read-only
and performs no project-local writes, including recovery, trash or journaling.
Creating the first writable store creates missing local infrastructure; a
read-only store can read an untouched checkout without creating it.

One worker serialises mutations. SHA-256/length snapshots include create/delete
states and all targets of the operation. Atomic replacement uses a temporary file
on the target filesystem, file sync, replacement and directory sync where
supported. A sync error is a failure, never acknowledged as saved. Preserve the
Linux/Windows platform limitations already recorded in ADR 0008 until verified.

Transaction fingerprints describe exact file bytes, including line endings. Do
not normalise CRLF/LF when comparing expected/target states or checking staged
payload integrity. Any future normalised source/cache digest is a separate,
explicitly versioned policy, not a replacement for these fingerprints.

`head.json` is a small strict structure with `formatVersion: 1`, monotonically
increasing `generation` and nullable `activeTransactionId`. An absent head in a
new checkout means generation zero with no transaction. Only the writer creates
or advances it. Unknown versions block writes and are preserved.

A stable read observes head, reads/fingerprints required files, and rechecks head
and fingerprints. Accept only the same generation with no active transaction and
unchanged read inputs. Retry at most three times, then report a busy/changed
project. This also detects non-cooperating external edits on a best-effort basis;
advisory locking cannot prevent an unrelated program writing after the check.

An active transaction without a writable owner cannot be repaired by a read-only
reader; report recovery required rather than assemble mixed data. Simulations
and `check` use stable reads; mutations and live CLI runs require the writer lock.

## Transaction unit and protocol

Each source save, manifest save, publication or explorer command is one logical
transaction over its complete affected file set. Save All schedules individual
document transactions with per-document results. A pure layout update is its own
transaction when it describes saved source. Recovery updates are serialised
single-file atomic writes under the same lock during ordinary recovery saves.
Explorer operations that change recovery membership include the checked recovery
and backup post-images in their shared multi-file transaction instead.

Transaction directories are `.apiforge/local/transactions/<uuid>/`. Their versioned
descriptor contains transaction ID, base/target generations and ordered operations.
Each operation records a validated relative path, expected old state (missing or
SHA-256/length), target state and a staged replacement payload when applicable.
Directories are explicit create/remove-empty operations, not recursive unbounded
deletes. Moves and deletes are expressed as idempotent file writes/removals; a
trash destination preserves data before an original is removed.

1. Validate all paths and payloads and preflight every expected file state.
2. Stage all target payloads and the descriptor; sync files and their directories.
   Do not change shared target files yet.
3. Atomically/synchronously set `activeTransactionId` in head, retaining the base
   generation. This is the durable activation point. No target mutation may precede it.
4. Apply operations in descriptor order. If a target has its expected old state,
   install the target state. If already in target state, continue. Any third state
   is a conflict; stop without overwriting it. Recheck path boundaries immediately
   before each operation. Sync changed files/directories.
5. After all operations are durable, atomically/synchronously write head with the
   target generation and null active ID. This is commit and the only point where
   the caller may receive save/publication/file-operation success.
6. Remove staging as later cleanup. Failure here does not undo a committed operation.

The worker takes ownership after activation: cancelling a dialog cannot abandon
half a transaction. New semantic edits may enter memory while a save is in flight;
acknowledge only the saved source version, leaving newer text dirty. A queued save
is planned against the current committed generation when it starts, not an older
generation captured by the UI.

Before activation a failure leaves targets untouched. After activation a failure
leaves recoverable intent and blocks subsequent project mutations until resolved.
Use roll-forward recovery; do not pretend independent file replacements support
automatic whole-project rollback. A complete staged post-image is enough for
replay; the protocol does not require copying every old source into its log.

## Restart and conflict recovery

Inspect head before ordinary project loading. With no active ID, an unreferenced
staging directory was not activated or belongs to already committed cleanup; it
must not be replayed. With an active ID, verify the descriptor, generations,
payload hashes, versions, bounds and paths, then execute the same idempotent
operations and commit. Replay itself must survive another crash at each boundary.

If descriptor/payload integrity fails, preserve it and report recovery required.
If a foreign target state is found, preserve both foreign bytes and staged data.
Offer export of the candidates and an explicit Finish intended operation action.
That action must first preserve the foreign file as a conflict copy, subject to
the same secret checks, and record the accepted current fingerprint before retry.
Failure to preserve it blocks replacement. Declining leaves the project in
recovery mode, with export available; it does not authorise partial normal use.

The initial conflict service exposes an immutable, read-only recovery preview
with expected, intended and current states plus the readable current/intended
bytes for adapter-driven export. Finish is bound to the root, active head, exact
descriptor hash, receipt fingerprint and observed target states. An edit after
preview needs another preview/decision, including a second foreign edit after
an earlier accepted conflict. Reading or reopening never counts as consent.

Preserve foreign files under
`.apiforge/local/conflicts/<transaction-id>/<approval-id>/<operation-index>.bin`,
with a version-one `index.json` recording original paths and accepted fingerprints.
An accepted absence is explicit and has no payload. Copies use no-clobber
replacement and file/directory sync. After all copies and their index are durable,
atomically persist version-one `resolutions.json` beside the active descriptor.
Both index and receipt bind the transaction ID and exact original descriptor
SHA-256. The receipt retains every accepted state/copy reference in order; later
decisions supersede earlier states for an operation without discarding old copies.
Do not rewrite the original descriptor, original expected states or staged intent.

Replay may accept an additional initial state only from this verified receipt.
Validate receipts, archive indexes and every referenced copy, then establish their
durability before replacing targets, including on restart after a failed sync.
Recheck staging and consent inputs before replay. Missing/corrupt evidence blocks
completion; unknown receipt/index versions are preserved. Before receipt activation,
unreferenced copies never authorise replay. After activation, ordinary recovery can
resume the accepted operation without asking again for the same fingerprint.

Bound receipts/indexes to 16 MiB, accepted-state entries to 1024 and preserved
payloads referenced by a receipt to 64 MiB. Apply individual source/JSON bounds
and known-session-secret checks before creating new managed copies. Failure leaves
the original/staged candidates available and the transaction unresolved. Generic
transactions cannot edit the conflict archive namespace. Retain copies and their
indexes after successful commit, even when transaction staging is cleaned up.

This action handles file contents and absence, not recursive directory recovery
or file/directory kind replacement. A directory conflict, link, oversized foreign
file or inaccessible path needs manual correction/export; it is never treated as
an empty file or silently overwritten. Intended bytes can still be previewed when
a current target cannot be safely inspected. Native transaction/conflict dialogs and their export controls remain P7c
integration work; P6d separately implements export of unsaved work on close.

Do not overwrite a foreign edit automatically, discard a transaction because its
PID is gone, or load a partial set of files as a writable session. The native GUI
must distinguish source errors, recoverable I/O failure and unresolved storage
transactions. A corrupt last-valid cache is not a reason to recover shared source.

## Paths, trash and external tools

Normalise registry paths, reject absolute paths, `..`, technical-directory targets
for working sources, reserved Windows names and case-insensitive collisions.
Separate internal storage paths from user-controlled paths. Reject symlinks and
Windows reparse-point traversal for managed operations; revalidate the root and
parents before filesystem mutation. Do not recursively follow links when scanning.

This is a local single-user application. Rechecking paths narrows races but does
not claim protection against a hostile concurrent process replacing directory
ancestors. Avoid expanding this stage into a network filesystem security service.
An external editor/Git can observe intermediate physical files; only API Forge
readers implement the stable-generation protocol. After a checkout or external
edit, validate source and metadata rather than trusting local head alone.

Local head is not a Git tree identifier and need not change when Git replaces
shared files. Fingerprint rechecks detect changes during the read, but an external
checkout can pause with a partially replaced tree that remains identical across
both checks. Stable-generation reads guarantee the transaction boundary for
cooperating API Forge writers; they cannot guarantee one atomic Git checkout from
an uncoordinated writer. Advancing generation after an observed external reload
could invalidate overlapping readers, but cannot remove that limitation. The
reload-generation policy and cross-platform line-ending proposals are tracked in
the [Stage II review notes](../stage-ii-review-notes.md).

Trash lives under `.apiforge/local/trash/<operation-id>/`, with a versioned index
of original paths, document IDs, hashes and deletion time, plus preserved payloads.
Capture dirty source/layout as part of a delete so the last edit is recoverable.
Delete means preserve first, unregister/remove originals second, then commit.
Restore is another transaction retaining document identity; it requires another
destination if the original path is occupied. No automatic expiry in Stage II.
Text/model undo does not perform filesystem undo. Published revisions cannot be
deleted through this explorer.

The initial trash service covers registered working documents. Its version-one
index records project/operation IDs, deletion time, nullable restoration time and
entries containing the original registry entry/order, exact saved-source state,
optional saved checkpoint and optional dirty recovery document. Payload names are
derived from operation ID and entry number; index data cannot redirect reads to
an arbitrary path. Missing source remains explicit rather than becoming an empty
file. Preserve all copies before removing originals and unregistering documents.
Updating shared checkpoints, recovery membership and checked backups belongs to
that same activated transaction.

Delete/restore previews bind the observed file set and generation. Changes require
another preview; live drafts supplied at deletion must match the selected document
context. Restore preserves document identity, reinstates saved source exactly and
puts unsaved text/fields/layout into local recovery. A different destination also
updates recovery's last known path without changing its original base hash. Never
overwrite occupied destinations, foreign checkpoint/recovery contexts or published
revisions. Retain the trash index/payloads after restoration, marking the operation
restored in the transaction to prevent an accidental duplicate restoration.
Explorer deletion extends the same index with unregistered files (exact bytes and
state, restored at their original paths only) and directories, removed deepest
first after their contents and recreated on restoration. A directory preview
lists its contents without following links; any link or non-portable name inside
refuses the operation, and an entry added after the preview stops the removal.
The registry and hidden control directories cannot be deleted this way. Older
indexes without these fields remain readable.

## Recovery files, limits and secrets

Local workspace is best-effort UI state. Source recovery is not best-effort: its
failed write prevents close/project switch unless the user explicitly discards or
exports the work. Atomically save versioned recovery, with a checked backup and
existing base-hash/stale-draft rules. Unknown versions are preserved. Restoring a
backup requires an explicit decision and another fingerprint check.

The initial recovery schema uses `formatVersion: 1`, `projectId` and `documents`.
Each document stores its stable document/entity IDs, kind, nullable base revision,
last known working path, nullable exact saved-source base hash, source version,
text, selection, inspector field drafts and optional identity/layout checkpoint.
Selections contain directional `anchor`/`caret` UTF-8 byte offsets at character
boundaries. Field drafts retain widget identity, base hash, text and selection.
The optional checkpoint may describe an earlier valid source while current text
is invalid; it is a candidate requiring source/context/semantic validation, never
permission to replace source or execute a model.

Recovery writes use a preview baseline containing fingerprints of the registry,
primary and backup. A changed baseline requires another read. Rotate only a
validated primary into the checked backup, then atomically replace the primary;
each write must be durable before acknowledgement. A missing/corrupt primary may
offer a matching-project backup without writing anything. Restoring it is an
explicit operation bound to that preview and preserves the good backup. Future
versions and foreign-project recovery data are preserved. Recovery reads/writes
are serialised with other storage work and blocked by active shared transactions;
they do not advance shared definition generation or save `.afs` files. Failed
writes leave source work in memory; a caller must not treat a readable older draft
as successful persistence of a newer source version.

Versioned cache is derived and optional. Unknown/corrupt cache is preserved and
ignored with a notice; do not overwrite current source from it. ADR 0012 defines
the provenance required before display or explicit last-valid execution.

Ordinary writes refuse to replace corrupt or unsupported shared diagram metadata.
Explicit repair is bound to a preview fingerprint and preserves opaque original
bytes plus an index under `.apiforge/local/metadata-history/<id>/` before replacing
metadata. Shared checkpoint repair uses the same redo transaction for the archive
and replacement. Local cache repair durably installs its archive/index first,
then the new cache; a failure may leave an unused archive but cannot silently
lose the original. Oversized or unreadable originals require manual export and
remain untouched. Normal cache saves never replace an unrecognised cache.

The P1 I/O test seam surrounds actual temporary creation, chunked writes, file
sync, replacement/no-clobber installation, directory sync, directory creation,
removal and cleanup. Tests inject permission/disk-full failures on both sides,
interrupt replay, and kill native child writers at each transaction boundary.
These tests cover process interruption, not physical power loss or the durability
of a particular Windows filesystem. See the [acceptance record](../stage-ii-p1-acceptance.md).


Apply the existing 1 MiB source and 16 MiB JSON payload bounds from ADR 0010 to
individual staged payloads. Bound descriptor size to 16 MiB and each transaction
to 1024 file/directory operations and 64 MiB staged payload total. Check limits
before activation; report an oversized folder operation rather than silently
splitting it into separately visible transactions. Staged payloads are streamed
off the UI thread. No limit failure truncates user data.

Never substitute runtime secrets into source or derived snapshots. Strip secret
environment values with the existing public-project projection. Reject new
managed text/snapshot/staging/conflict-copy/trash-copy writes containing a known
nonempty session secret, following the existing draft protection. A diagnostic
identifies the problem without echoing the value. Values unknown to the app cannot
be reliably classified as secrets. Do not rewrite pre-existing originals or claim
to sanitise their historical content. Preserve in-memory work on a failed check.

## Migration and journal continuity

Use the complete migration procedure in the Stage II plan. Keep a versioned
migration index containing input hashes, project destinations and prepared/
published states. Original JSON/drafts remain unchanged. One atomically replaced
global project catalogue both activates all destinations and records completion.
No catalogue points at partially prepared projects. Recheck original hashes before
activation, including the journal, under the old exclusive session.

The existing journal has scenario IDs but no project IDs. Keep unresolved entries
in a legacy global review path, rather than guessing their destination. Persist
acknowledgements under a coordinating legacy lock; do not clear original entries
merely because project migration completed. Unknown/corrupt journals retain ADR
0009's preservation and warning behaviour. New project runs journal locally under
the project writer lock. Read-only/session-only runs retain the existing explicit
non-journalled warning; a write failure in an attached journal still prevents the
request as ADR 0009 requires.

## Verification

Fault-inject every descriptor/payload sync, head activation, target replacement,
directory change, commit and cleanup, then restart and repeat recovery. Include
missing files, equal-length foreign edits, replacement/deletion of staging,
unrecognised versions, insufficient space and loss of permissions. Verify no
success is acknowledged before commit, stable readers reject mixed generations,
another process cannot mutate, and killing the writer releases its OS lock.

Exercise native Linux and Windows processes, not just mocked operations. Verify
file replacement and directory metadata durability on supported platforms before
closing the corresponding release gates.
