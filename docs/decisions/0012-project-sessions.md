# ADR 0012: Shared document sessions, analysis versions and execution snapshots

- Status: accepted for Stage II; P1 cache and P2 sessions implemented; desktop views
  behind `--folder` (P4); CLI and run integration pending
- Date: 2026-10-24
- Scope: new `apiforge-project`, desktop, language context and CLI integration
- Plan: [Stage II](../stage-ii-code-workspace.md)
- Related: [format](0010-folder-projects.md), [storage](0011-project-transactions.md)

## Context and compatibility

The present desktop combines document orchestration and views in `App`. More tabs
and a future second window must share source, model and history, rather than race
independent editor copies. Source-save state, analysis state and model availability
are separate: invalid source can be saved successfully.

For folders this decision replaces ADR 0007's model-backed Clean/Applied/Invalid
storage assumptions with explicit source documents and recovery. It retains the
250 ms debounce, invalid/pending edit protection, two histories and deliberate
stale-draft handling. Legacy JSON mode keeps ADR 0007's current behaviour. There
is no parser, graph semantics or runner rewrite.

## Ownership and interfaces

`apiforge-project` depends on core/script/layout/storage as needed, never egui.
`ProjectSession` owns project configuration, published definitions, document
sessions and event sequencing. A single session owner applies commands serially;
background workers receive immutable inputs and return candidate results.

`DocumentSession` owns document identity, text, source version, diagnostics,
model history, current/last-valid snapshots and save baselines. The egui adapter
owns per-document `TextEditState`, retained with a stable document widget identity
across Code/Split renderers and tab switches. A future simultaneous second code
surface must share text-history commands; Stage II does not render two independent
editable code widgets for one document. Diagram view state is independent from
document mutations and history.

The public command/event surface includes:

| Commands | Events/results |
| --- | --- |
| Open/initialise project, read document | Project opened, document registered/selected |
| Edit source, apply graph edit, settle | Source/model changed, analysis pending/completed |
| Save document/configuration/all | Save started/committed/failed with exact version |
| Publish component | Revision committed with component ID and number |
| Create/import/rename/move/delete/restore | Registry/explorer change committed or failed |
| Restore/keep/discard recovery | Recovery applied, stale decision needed or discarded |
| Prepare run | Immutable run snapshot, decision required or refusal |

Commands name document IDs, not active-view positions. Assign a monotonic event
sequence and origin token to semantic changes. Rendering/selection events do not
echo back as another source/model edit. Reuse core editing operations and existing
layout algorithms; do not fork semantics into desktop and CLI implementations.

## Analysis tickets and dependencies

Each analysis input/result carries:

```text
AnalysisTicket {
    sessionId,
    sessionEpoch,
    documentId,
    sourceVersion,
    projectContextGeneration
}
```

Assign a fresh session ID on open/reload/replacement, so results from a closed
session cannot match a reopened one. Advance the epoch on reload/replacement.
Increment source
version on each source change, including undo and graphical projection. Advance
context generation on environment, contract, registry or published-definition
changes. Conservatively invalidate outstanding results for the whole context;
precise dependency-only invalidation is a later optimisation.

Apply results only when all ticket fields still match. Debounce 250 ms per dirty
document even when hidden, scheduling repaint/work independently of rendered tabs.
Source → model uses parse/lower/reconcile and validation against one immutable
context. Revalidate current tickets before installing the model. Publication
becomes visible to this context only after its storage commit.

The configuration/dependency digest for cache and execution is a deterministic
hash of the public environment, contracts, exact referenced published definitions,
their identities/revisions and the folder resolution policy version. Include
transitive component dependencies in the snapshot. Runtime secret values are not
included in durable digests or snapshots. In-memory run preparation additionally
captures current session secrets without serialising them.

## Document state and allowed actions

Store orthogonal axes rather than a large enum of combinations:

- Save state: clean against the last observed disk source or dirty.
- Analysis: pending, valid or invalid.
- Conflict: none, external source, restored draft or storage recovery.
- Model: current, verified last-valid or absent.

| Situation | Diagram | Graph mutations | Run |
| --- | --- | --- | --- |
| Valid, saved source | Current | Allowed | Allowed |
| Valid, unsaved source | Current, unsaved indicator | Allowed | GUI allowed, unsaved indicator |
| Pending source | Previous, pending indicator | Blocked | Settle and wait first |
| Invalid, last-valid snapshot exists | Last-valid, stale banner | Blocked | GUI confirmation only |
| Invalid, no last-valid snapshot | Unavailable | Blocked | Refused |
| Unresolved conflict | Preserved preview | Blocked | Refused |

Ctrl+S can persist invalid source; that changes save state only. An invalid or
pending document never prevents editing an unrelated document. Store failed-save
status separately from diagnostics. A successful save of an older version does
not mark newer text clean. A restored draft with a different base hash requires
keep/edit/discard before application; a repaint or restart cannot count as consent.

Source has one owning scenario including cleanup, or one working component.
Inspector field drafts are scoped to stable document/widget identities and feed
the same model/source pipeline; conflicting or invalid field text never silently
replaces canonical source. Settle valid field edits before source saving and run
preparation; unresolved fields participate in the document's mutation guard.

## History, layout and projection

Use per-document model history across main/cleanup and tab switches. Text focus
routes Ctrl+Z/Ctrl+Y to that document's text history; otherwise use model history.
An applied source analysis is one model edit using existing grouping rules.
Graphical undo/redo projects source and resets obsolete text history/completions.
Do not clear model history merely because the explorer or another document gained
focus. Explicit document replacement/reload establishes a new history baseline.

Clean/valid source may be reprinted after a graphical semantic change. Restore
caret/selection by stable node/statement where possible; never reprint pending,
invalid or conflicted text. A graphical edit changes source and dirty state,
while block movement changes layout only. Metadata for unsaved semantics belongs
to local recovery until source commit. Shared checkpoints never silently publish
another tab's unsaved source.

Canonical printing can change spacing/order and removes comments not represented
as notes. Keep current parser warnings. When a graphical edit or publication
would discard such comments, retain the original and request a one-time decision
for the current source version before applying the destructive projection. Source
edits, formatting and explicit discard remain reversible in text history; ordinary
Save never formats. Do not add recurring confirmations for harmless whitespace.

## Last-valid provenance

`last-valid.json` is a bounded version-one cache with entries containing project
ID, document key, exact `sourceHash`, source version/provenance, model version,
cache version, context digest, semantic digest and a public model/context snapshot.
The snapshot includes the exact referenced published components and contracts plus
public environment configuration, so an explicit last-valid run cannot accidentally
mix an old graph with a newly changed dependency. Validate structure, all digests,
IDs and model semantics before accepting an entry. Cache entries never contain
runtime secrets; invalid/unsupported entries are preserved and ignored.

P1 retains exact source text in each cache entry and lowers it with the folder
policy to verify source/model semantic equality. Explicit revision pins are
required without changing legacy lowering. The closed context contains exactly
the transitive published component revisions, public contracts/configuration and
no placeholder scenario. A full snapshot digest also binds identities, positions,
proxies and provenance; the wire DTO is private and exposed snapshots are
immutable. Capture and load both validate these invariants. Cache matching helpers
report equality only: they never promote a snapshot to a current model. Session
analysis tickets and GUI run confirmation remain P2/P6 integration work.


If current source is valid, derive its current model from that source/context.
Never declare cache current from filename or document ID alone. A matching
checkpoint can restore identity, but valid source remains semantic authority.
If current source is invalid, an accepted cache can supply the labelled diagram
preview and a candidate last-valid run. No cache means an unavailable diagram,
not an empty graph or an executable default. Cache loss does not make valid code
unusable and must not cause source replacement.

## Runs and journal ownership

Settle source/field edits before preparing a run. A valid GUI document may run
without source save; saved-folder CLI runs use only stable committed source.
Invalid source can request a verified last-valid GUI run with a description of
its earlier code/configuration. Bind confirmation to the exact snapshot digest
and current relevant session versions; a change invalidates the decision. A
pending run resumes only against the prepared snapshot, not a fresh implicit load.

Construct an immutable `RunRequest` with the selected scenario, public configuration,
transitive exact published revisions and separately held session secrets. This
preserves the existing cloned-project execution model. Never resolve components
from live session state during execution. Changes mark reports stale, not mutate
an active run. CLI refuses invalid source and never implicitly executes local
last-valid cache. Another invalid scenario outside the selected dependency closure
does not block running the selected valid scenario.

Use an abstract shared writer-session handle for the journal instead of requiring
a concrete WorkspaceStore. Retain ADR 0009's request-before-send durability,
redaction and deliberate interrupted-run review. Project close/switch must await
or explicitly cancel an active run and finish its cleanup through the existing
nonblocking close path before releasing its owning session.

## Acceptance

Test source edits settling in inactive tabs, mode switches during debounce,
independent histories, source/field recovery conflicts and obsolete save completions.
Reject a late result after source change, context change, reload and close/reopen.
Two test view subscribers must observe the same model version without duplicate
commands. Cache from another document, source, model version or context never
becomes current. Exercise explicit last-valid confirmation and its invalidation,
immutable run dependencies, report staleness and preserved journal ownership.
