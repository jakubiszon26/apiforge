# ADR 0010: Folder projects, source identity and component revisions

- Status: accepted for Stage II; P1 storage implemented; folder/session integration pending
- Date: 2026-10-24
- Scope: core authoring validation, new `apiforge-project`, storage, desktop and CLI
- Plan: [Stage II](../stage-ii-code-workspace.md)
- Related: [transactions](0011-project-transactions.md), [sessions](0012-project-sessions.md)

## Context and compatibility

The existing model is the saved truth in a version-two JSON workspace. Folder
projects make `.afs` the durable definition while retaining the language, model,
printer and reconciliation. One directory is one project; definitions remain
editable in both code and diagram views.

For folder projects this decision replaces ADR 0002's single-document container
and ADR 0005's model-as-durable-truth and implicit-latest component policy. ADR
0004's semantic model v2 and ADR 0005's grammar/equivalence/identity contracts remain.
Legacy JSON import/export, session opening and CLI commands retain their existing
rules. The folder format has its own version and does not pretend to be JSON v3.

## Format version 1

```text
apiforge.project.json             shared project configuration and registry
scenarios/**/*.afs               default scenario source location
components/**/*.afs              default working-component source location
.apiforge/diagram.json           shared identity/layout checkpoints
.apiforge/revisions/<id>/<n>.afs  immutable published component sources
.apiforge/local/                 ignored session, recovery, view and storage data
```

Working sources may live elsewhere under the root, outside technical directories.
Only registered `.afs` are project definitions. Discovery of new external `.afs`
offers registration; it does not silently execute, publish or rewrite them.

Use camelCase JSON fields, strict versioned structures and relative paths with
`/` separators. A new manifest contains:

| Field | Type and meaning |
| --- | --- |
| `formatVersion` | Integer `1` |
| `id`, `name` | Stable project ID and display name |
| `environment` | Existing public environment structure, excluding secret values |
| `contracts` | Existing contract definitions |
| `documents` | Array of working/recovered document entries |
| `revisions` | Array of published component-revision entries |

A document entry contains `documentId`, `kind` (`scenario` or `component`), `path`
and `entityId`. Component entries additionally contain nullable `baseRevision`;
null denotes an unpublished component. Recovered component drafts have a distinct
document ID/path but may share an entity ID with the primary working document.
There is at most one primary working document per entity; a `recovered` boolean
distinguishes additional recovered drafts. IDs are opaque and stable, never derived
from a display name or current path. Reuse existing entity IDs during migration;
new document IDs use full random UUIDs with collision checks.

A revision entry contains `componentId`, `handle`, positive `revision`, `path` and
lowercase SHA-256 `sourceHash`. Its key is `(componentId, revision)`. A handle
belongs to one component ID; revisions of that component share it. Paths, document
IDs and revision keys are unique. Missing or mismatched revision source is an
integrity error; do not select another revision or substitute working source.

Retain existing per-source (1 MiB), depth and graph/model limits. Bound manifest,
diagram and reconstructed public project payloads to 16 MiB each. Authoring may
contain no valid scenario; otherwise use existing scenario/component/contract
counts and validation rules. Recovery has the existing bounded per-buffer rules
and a 16 MiB total payload cap; exceeding it reports failure and retains memory,
not a truncated success. Do not silently drop documents or revisions to fit limits.

An unsupported shared format is preserved without writes. Bad shared metadata is
reported and requires explicit recovery before replacement. Technical file errors
never cause the app to initialise a fresh project over an existing one.

## Identity and diagram checkpoints

`diagram.json` has `formatVersion: 1` and `records`. A record is keyed either by
working `documentId` or by `(componentId, revision)`, never by filename alone. It
contains `sourceHash`, a context digest, model/cache version, semantic digest and
an `identityBase`: the derived core `Document` snapshot with IDs, ordering edges,
positions, `autoPlaced` and graph proxies intact. Preserve proxy IDs, source
bindings and positions during migration and reconciliation. Shared checkpoints
describe saved valid source; local recovery describes changes not yet saved. Working and published layouts are
separate records, even if their initial content is identical.

The snapshot is a reconciliation checkpoint, not another source of current
semantics. Validate its structure, key and digests before use. Load current source
and current dependencies first. If source changed, reconcile the newly lowered
document against the checkpoint. If no usable checkpoint exists, allocate new
identities and auto-layout valid source, and report that layout/identity could not
be recovered. Do not fail otherwise valid source merely because a checkpoint is
missing. Preserve malformed metadata for deliberate replacement/recovery.

Use existing reconciliation exactly:

1. An unchanged handle and node kind retain identity and position even if other
   semantic fields changed.
2. An unambiguous rename retains identity according to ADR 0005.
3. Other insertions and kind changes receive fresh identity.
4. A deletion applied as a separate model change removes the identity; a later
   insertion is new. Deletion/recreation entirely between two analyses cannot be
   distinguished from an edit with the same handle/kind. Do not claim otherwise.

Do not infer identity from position, revision filename or hash alone. Layout uses
the model's `autoPlaced`; there is no additional `pinned` field. Moving/renaming a
working file changes its registry path, not its declaration or entity ID. Changing
a registered source's document kind requires explicit replacement/new identity,
rather than silently converting its context.

## Explicit component revisions

Folder sources must write calls as `Handle@revision(...)`. The grammar continues
to accept unqualified calls, but folder authoring/check/run validation emits an
error with an explicit pin-to-current-revision fix. Applying that fix edits source
and participates in history. Loading, saving or publishing another component never
silently applies it. JSON mode retains its implicit-latest interpretation.

The library/completion insert exact revisions. The existing printer already emits
the revision in a model component call; migration therefore preserves pinned
bindings without redesigning the language. Validate policy through a folder-mode
lowering option shared by GUI and CLI; do not implement a second text parser.

Working components lower only against published dependencies. They do not become
dependencies of scenarios until published. Components can be imported and saved
with errors, but cannot be published or executed while invalid. Imported files
keep their spelling and are not rewritten to canonical source automatically.

Publish first settles source and validates the document/context. One transaction
saves its current working text, creates the next unused revision source, records
the revision and its diagram checkpoint, and advances the working base revision.
Existing published files are never overwritten. Expose the revision to resolution
only after commit. Publishing from an older recovered draft uses its own source
and creates a revision after the current maximum; it does not rebase or replace
the primary working document. Other working drafts keep their own base revisions.

The working document retains user formatting; published source is canonical and
contains explicit call revisions. Preserve existing note/comment semantics, with
the destructive-format warning specified in ADR 0012. Old instances keep their
revision. Handle changes after the first publication are rejected in Stage II;
copy as a new component instead. Working-file deletion does not remove published
revisions. Published-revision deletion is outside Stage II.

## Source, workspace and Git

The manifest, working `.afs`, published sources and diagram records are shared
project data. Local workspace, recovery, caches, trash, journal, locks and storage
head/transactions live under `.apiforge/local/`, ignored by Git. Append that rule
without rewriting an existing `.gitignore`. Secrets stay session-only.

Opening a root without a manifest is read-only discovery until explicit
initialisation. Initialisation registers sources, creates configuration and adopts
safe default locations without rewriting existing `.afs`. An unparseable header
needs a kind choice. Existing source collisions are never overwritten. Opening
any project does not run its requests.

## Consequences and acceptance

The model stays useful for execution and reconciliation, but cannot silently
restore semantics over an edited source. Identity/layout remain portable with Git;
local cache may be discarded without changing valid source behaviour. Copying only
an `.afs` intentionally loses project identity/layout and may lack dependencies.

Tests cover every existing equivalence/identity case, missing and mismatched
checkpoints, main/cleanup edges, recovered drafts, move/rename, component-only
projects, publication immutability and a revision-1 scenario remaining unchanged
after publication of revision 2. The full folder save/restart round trip gates
migration work.
