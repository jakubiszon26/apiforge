# Stage II: folder projects and the code workspace

- Status: approved implementation specification; P0 prepared, P1 complete on Linux, P2 complete at the session-library boundary (round-trip gate passed headless), P3 migration implemented (not enabled at startup), P4–P7 pending
- Date: 2026-10-24
- Analysis baseline: `d967a79`; implementation base: `c9c5ba0` plus this planning commit
- Delivery: one branch per package, merged to `main` after review; P1 merged from `feat/stage-ii-code-workspace`
- Decisions: [folder format and components](decisions/0010-folder-projects.md),
  [storage and recovery](decisions/0011-project-transactions.md),
  [sessions and synchronisation](decisions/0012-project-sessions.md)

## Outcome and agreed scope

API Forge opens a project directory, presents its files in an explorer and edits
scenarios and components as `.afs` sources. Code has a dedicated view occupying the
main application area. Code and diagram remain equivalent whenever the source is
valid. Reopening a folder restores its workspace and unfinished work.

The agreed product decisions are:

- One directory is one project. Recent projects provide switching between folders.
- `.afs` files are the durable definitions. The in-memory model supports graphical
  editing and execution; it does not replace source files as the saved definition.
- Synchronisation is automatic; source saving is explicit with Ctrl+S. Invalid
  source can be saved, and automatic recovery protects unsaved work.
- Components have editable working documents and immutable published revisions.
  Editing a working component does not change existing scenario instances.
- Upgrade automatically migrates the existing version-two workspace into folders,
  preserving the originals and validating the complete result before switching.
- This stage supplies the shared session needed for a future second diagram
  window. The second native window itself is outside this stage.

System fullscreen, IPC, collaboration, arbitrary file-type editing, new variable-
proxy functionality and a general symbol-rename/navigation feature are outside
this stage. M3 variable proxies were merged in `c9c5ba0` and must survive folder
storage, migration and reconciliation. M7 remains tracked by the
[original roadmap](roadmap.md).

## Current implementation and review findings

The existing language already parses, lowers, prints and reconciles scenarios and
components. Desktop buffers support a 250 ms debounce, invalid-source protection,
draft recovery and code/diagram selection synchronisation. Storage provides atomic
JSON replacement, exclusive sessions and external-change detection.

The missing foundation is the folder project. The GUI imports JSON only;
`WorkspaceStore` persists the entire workspace. `App` combines synchronisation,
storage, execution and view orchestration. `Editor` has one active document history
and assumes that a scenario exists. Component editing uses nested temporary copies.

The independent review was checked against the code, rather than accepted as an
implementation specification:

| Review point | Verified decision |
| --- | --- |
| Multi-file saves, cache provenance and dependency versions need explicit contracts | Accepted; ADRs 0010–0012 define them. |
| Component calls must retain their revision | Accepted. The printer already emits revisions; lowering an unqualified call currently selects the latest. Folder validation will require explicit revisions. |
| Reconciliation can distinguish every delete/recreate with the same handle | Rejected. The same handle and kind retain identity. An intermediate deletion must have been applied to distinguish a later insertion. |
| Runs need an immutable project snapshot | Preserve the existing mechanism: the GUI already constructs `RunRequest` with a project clone. Adapt ownership rather than inventing another runner. |
| Every secret can be detected in every source or archive | Rejected as an absolute guarantee. Protect known session secrets and never persist runtime substitution. Preserve existing original files without rewriting them. |
| Unattached comments can disappear on canonical printing | Confirmed. The parser warns that formatting drops them; the roadmap's claim that they move to an enclosing block was incorrect. |

The identity implementation is `apiforge-script/src/reconcile.rs`; it preserves
`id`, `position` and `autoPlaced`. Do not introduce an independent `pinned` field.
The existing printer and reconciliation remain the semantic foundation.

## Architecture and visible behaviour

### Project and document ownership

Add `apiforge-project`, a library without egui. It owns project/document sessions,
analysis, dependency resolution and semantic commands. `apiforge-storage` owns
durable project stores, locks and transactions. Desktop and CLI share the same
loader and validation policies.

`ProjectSession` owns one `DocumentSession` per stable document ID. A document owns
its source, diagnostics, current/last-valid model and model history. The egui
adapter retains the document's text-edit history across mode and tab switches.
Views own focus, selection, scrolling and geometry; they do not clone the project.

Commands cover opening/initialisation, text edits, graph edits, settling analysis,
saving, publication and file operations. Changes emit versioned source/model,
diagnostic, save-status and explorer events. Worker results carry the session
epoch, document ID, source version and project-context generation. Stale results
are discarded, including when only dependencies or configuration changed.

Empty projects and component-only projects are valid authoring states. Extract a
shared authoring validator allowing zero scenarios without weakening legacy JSON
import or adding a hidden placeholder scenario. Execution still requires a valid
selected scenario.

### Code, Diagram and Split

Provide three explicit modes:

| Mode | Main area |
| --- | --- |
| Code | Explorer on the left, file tabs and editor in the centre, diagnostics and save status below. Diagram and inspector reserve no space. |
| Diagram | The graphical workspace and its existing tools. |
| Split | Code beside the diagram. |

Ctrl+E toggles Code and Diagram; from Split it enters Code. A visible control
selects any mode. New projects start in Code; subsequent opens restore the folder's
last mode. Switching never saves source, closes a document or resets its histories.
“Open in code” opens the owning file and reveals the selected statement. Run and
reports remain available in the main modes.

Retain one tab per document ID. Preserve tab order, active document, caret,
selection, scrolling, expanded folders, panel sizes and diagram camera in local
workspace state. The application's recent-project list is separate from project
definitions. Moving a project directory retains its ID and in-folder workspace.

### Editing and saving

Apply valid source after 250 ms even if its tab or code view is hidden. Pending,
invalid or conflicted source locks mutations of that document's model only.
Another document remains editable. A diagram without a current model shows an
explicit last-valid banner or an unavailable state, never a pretend empty graph.

Graphical semantic edits update source and mark it dirty. Moving blocks changes
layout only. Unsaved semantic changes keep their layout in local recovery until
source is saved. Preserve text history and model history as separate histories;
focus selects which Ctrl+Z/Ctrl+Y uses.

Ctrl+S saves the active source exactly, including errors, plus required dirty
manifest data. It does not format the file. Save All (Ctrl+Shift+S) runs separate
document transactions and reports each outcome. Editing project settings marks
the manifest dirty; Save All also saves it when no source tab is active. Metadata
and manifest writes never silently save another tab's dirty source.

Recovery runs after one second of inactivity and before closing or changing
projects. Closing a dirty tab offers Save, Discard or Cancel. Application close
may retain dirty tabs through successful recovery without saving `.afs`; a failed
recovery write prevents close and exposes Retry. Read-only sessions cannot claim
durable recovery and must offer export or an explicit discard decision on close.

Keep canonical printing and the existing comment-as-note contract. A graphical
edit can reformat code; when unattached comments would be lost, obtain a decision
before the first destructive reprint, retaining the original text until then.
This is not a new lossless graph-to-source patch engine.

### Explorer and external files

Show the real directory tree, hiding `.git` and `.apiforge`. Support new folders,
new scenarios/components, multi-file `.afs` import, rename, move through menus and
drag-and-drop, delete, restore from local trash, refresh and reveal in the system.
Provide keyboard navigation, Enter and explorer F2. Show dirty, error, conflict
and read-only status. Other file types can be revealed through the system, but
are not edited in this stage.

File moves retain document IDs. Explorer rename does not rename declarations or
symbols. Deleting a working component never deletes published revisions. Directory
deletion previews affected files and dirty tabs. File operations are outside text
and model undo; trash provides recovery with no automatic expiry in this stage.

Open Folder opens an existing manifest. A directory without one offers an explicit
Initialise action; picking a folder alone writes nothing. Initialisation registers
existing `.afs` without rewriting their content. Parse the declaration to select
scenario/component; an unrecognisable header requires an explicit kind choice.
Preserve unknown dependency diagnostics rather than inventing definitions.

Import copies source and assigns new identity; it does not touch the original,
overwrite a destination or silently publish a component. Resolve component-handle
collisions before registration. Unqualified calls remain editable with diagnostics
and an explicit revision-pinning fix.

Poll external changes asynchronously once per second and rescan on focus. Reload
clean files; local edits produce a conflict with reload/discard or export-copy
actions, without automatic merging. Match an external rename only when a removed
and added file have a unique hash match. Manifest changes invalidate dependent
analysis; an invalid manifest blocks writes and execution while preserving work.
All I/O stays off the UI thread; render only visible explorer rows.

### CLI and execution

Add the following alongside the existing JSON `--workspace` commands:

```text
apiforge check --project <folder> [--file <relative.afs>]
apiforge print --project <folder> --file <relative.afs>
apiforge apply --project <folder> --file <relative.afs> <source.afs>
apiforge run --project <folder> --scenario <relative.afs|id> [...]
```

`print` emits canonical source from a valid definition. `apply` requires valid
input and an exclusive project lock. `fmt` retains its current single-file
behaviour. Desktop adds `--folder <path>`; `--project FILE` retains its existing
session-only JSON meaning. Folder commands use explicit component revisions and
the same stable-read protocol as the GUI.

GUI Run settles pending analysis, then snapshots the selected document and exact
dependencies/configuration. It can execute valid unsaved work with an indicator.
Invalid source requires explicit consent to a verified last-valid snapshot; no
snapshot means no run. A conflict prevents Run. CLI executes saved valid sources
and fails rather than falling back to local cache. Edits never change an active
run and mark its report stale. Preserve ADR 0009's journaling/redaction behaviour.

## Upgrade migration

The migration is automatic for valid workspace v2, but activation is all-or-nothing
across its projects. Existing v1 handling is unchanged.

1. Acquire the legacy workspace writer lock and record hashes of the workspace,
   drafts and journal. Keep the original files intact.
2. Prepare each project under a migration staging directory in the application
   data directory. Destination folders are `projects/<project-id>/`.
3. Print all scenarios and every component revision with explicit call revisions.
   Transfer project/entity IDs, ordering-edge IDs, positions, `autoPlaced` and
   graph proxies, including proxy identities and their sources.
   Create each component's working file from its latest published revision.
4. Transfer scenario and inspector drafts with their contexts, original base
   hashes and cursor positions. Preserve an older component draft as a separate
   recovered working document with its original component ID and base revision;
   never overwrite it with the latest definition or publish it automatically.
5. Keep the old unresolved journal as a legacy global journal. Its records lack
   project IDs, so do not guess ownership from scenario IDs. Retain the global
   review/acknowledgement path after migration until entries are handled.
6. Reload every prepared project through the new loader and verify semantic
   equivalence, identity, layout and exact component revisions. Recheck input
   hashes before activation.
7. Publish prepared folders without replacing independently existing destinations.
   Atomically activate one versioned project catalogue containing the migration
   ID, input hashes, complete project-ID/path map and previous active project.
   This catalogue is also the completion marker; do not commit a second unrelated
   marker whose write could diverge from it.
8. Open the previous active project and report destination/original locations.

A durable migration index records prepared/published destinations and their hashes.
Restart resumes the same migration rather than duplicating projects. An existing
destination is reused only when it belongs to this migration and matches its
recorded state. A failure leaves the new catalogue inactive; originals remain
available read-only for export and retry. No automatic rollback writes old data
over new user changes. A desktop without a legacy workspace starts a new folder
project in the managed projects directory instead of creating another legacy JSON.

## Delivery and acceptance

Planning lands on `main` as a documentation-only commit. P1 was implemented on
`feat/stage-ii-code-workspace` and merged after review. Each later package uses
its own branch from `main` (for example `feat/stage-ii-p2-sessions`) and merges
after review and its exit evidence. Merged packages stay inactive in the normal
desktop/CLI paths until their integration package enables them. Keep unrelated local QA material out of
the planning commit. Remove the temporary `recenzja.md` after incorporating its
verified findings.

| Package | Implementation | Required exit evidence |
| --- | --- | --- |
| P0 | This specification, three ADRs and links/corrections to existing docs | Reviewed contracts; no claim that Stage II already works |
| P1 | Folder schemas, ProjectStore, stable reads, lock, transactions, recovery and trash | Fault injection at every persistence boundary; conflict handling |
| P2 | ProjectSession/DocumentSession, authoring validation, analysis tickets and histories | Independent documents, empty projects and rejected stale results |
| P2 gate | Create folder project, save, close and reload through the new loader | Matching semantics, IDs, layout and pinned revisions before migration work |
| P3 | Legacy workspace, draft and journal migration | All fixtures verified; crash/resume and catalogue activation tested |
| P4 | Code/Diagram/Split, tabs, shortcuts, focus and diagnostics | Mode/tab switching retains work and histories |
| P5 | Explorer, import, file operations, trash and external polling | Operations survive restart; no silent overwrite |
| P6 | Component publication, folder CLI and run integration | CLI/GUI interpret saved projects identically |
| P7 | Startup activation, native QA, performance and documentation | Release evidence on Linux and Windows |

Implement P1/P2 first and pass the P2 round-trip gate before P3. Until P7, folder
projects are opened explicitly for development; do not enable automatic migration
in the normal startup path before integration evidence is complete.

### Implementation progress

Open design feedback is tracked in [Stage II review notes](stage-ii-review-notes.md),
including checkpoint merge ergonomics, Git line-ending conversion and local head
behaviour during external checkouts.

The first P1 increment provides `apiforge_storage::project`: the version-one
registry schema, portable managed paths, cloneable exclusive/read-only stores,
stable-generation reads and staged multi-file transactions with explicit
roll-forward recovery. Source bytes are saved exactly, including invalid text.
Published source hashes are checked; existing revisions cannot be overwritten or
removed through the transaction API. New managed payloads are checked against
known session secrets.

Storage tests exercise interruption at staging, activation, each operation and
commit; interrupted replay; foreign target preservation; equal-length edits;
stable-read retries; directory ordering; path boundaries; and the 1 MiB source
limit. These are storage-level checks, not the P2 semantic/identity/layout gate.

Verification for this increment (Linux, 2026-10-24): formatting, workspace clippy
with warnings denied, fixture verification and workspace/all-target tests passed
(575 passed; one helper test ignored because its parent launches it separately).
The test run used isolated config/data directories and serial execution. Existing
HTTP tests required loopback access outside the restricted sandbox. Twenty new
folder-storage tests are included in those totals. No native GUI or Windows QA
was performed for this library-only increment.

The next P1 increment adds bounded source recovery with checked backup rotation,
explicit preview-bound backup restoration, exact invalid/CRLF text, inspector
drafts and directional UTF-8 selections. Both files and the registry participate
in conflict checks. Read-only sessions can preview but cannot persist or restore.
Known-secret checks also cover bytes copied into backups and explicit restoration.
Encoded JSON limits are enforced while serialising, including escape expansion;
an oversized payload never becomes a truncated successful save.

Version-one diagram DTOs now preserve core scenario/component snapshots and keep
working/published keys separate. Serialisation sorts records deterministically.
Recovery can retain an unsaved checkpoint locally without writing shared diagram
metadata. These are candidates: the future semantic loader must verify their
source/context/semantic digests before use. A shared authoring validator permits
empty/component-only projects while retaining legacy JSON import requirements.

Verification of the recovery/checkpoint increment (Linux, 2026-10-24): formatting,
workspace clippy with warnings denied, fixture verification and all-target
workspace tests passed (598 passed, one parent-launched helper ignored). The
serial test run used isolated config/data directories and loopback access for
existing HTTP tests. This adds 23 tests to the first increment's total, including
backup/restore races and failures, local unsaved-layout recovery, checkpoint
identity/proxy preservation and authoring/legacy validation separation. This does
not constitute native GUI/Windows QA or the P2 source/model round-trip gate.

The next P1 increment adds versioned trash/restore for registered working documents.
One transaction preserves exact saved bytes, checkpoint and dirty source/fields/
layout before removal, then updates the registry, shared checkpoint set and local
recovery with checked backup rotation. Restoration retains IDs and registry order,
requires an unused destination and reinstates unsaved work as recovery instead of
silently saving it. Published component revisions remain intact; trash has no
automatic expiry. Preview baselines, source hashes, path boundaries and known
secrets are checked before activation. Tests interrupt deletion/restoration at
each transaction boundary and verify restart/replay, occupied destinations,
multi-document operations, unrelated recovery and foreign metadata preservation.

Verification of the trash/restore increment (Linux, 2026-10-24): formatting,
workspace clippy with warnings denied, fixture verification and all-target
workspace tests passed (615 passed, one parent-launched helper ignored), with
isolated config/data directories, serial tests and loopback access for existing
HTTP tests. Seventeen new trash tests are included. Transaction-boundary tests
exercise every stage/operation in both delete and restore, then restart and
roll forward. These are storage-service checks; explorer/native QA and the P2
round-trip gate remain open.

The conflict-resolution increment adds read-only transaction previews and explicit
Finish intended operation. It preserves foreign bytes and versioned archive indexes
before recording exact-state approvals in a descriptor-bound receipt. Original
intent stays unchanged; restart verifies copies/receipts and resumes only an
already accepted fingerprint. New edits require a new decision. Failure, unknown
versions, damaged evidence, secret checks or unsafe paths leave recovery blocked.
Directories/type replacement and unreadable/oversized targets require manual
correction; preview never turns them into absent/empty data. Copies remain after
commit. Native recovery dialogs and export controls are not yet integrated.

Verification of the conflict-resolution increment (Linux, 2026-10-24): formatting,
workspace clippy with warnings denied, fixture verification and all-target
workspace tests passed (635 passed, one parent-launched helper ignored), using
isolated config/data directories, serial tests and loopback access for existing
HTTP tests. Twenty new tests cover read-only export candidates, preservation and
approval boundaries, restart/repeated interruption, stale decisions, changed
staging/proof inputs, damaged copies, copy limits, secret refusal and directory/link
rejection. This does not close native Windows durability or the P2 round-trip gate.

P1 is complete at the storage-service boundary on Linux. The final increment
adds verified, immutable last-valid candidates with exact source/provenance,
closed pinned dependencies, public configuration and full snapshot digests.
Invalid/future cache is preserved and ignored. Explicit cache/checkpoint repair
preserves original bytes under local metadata history before replacement.
Ordinary saves cannot bypass that preservation policy.

Persistence tests now inject disk-full and permission failures before and after
actual filesystem operations, including partial temporary writes, file/directory
sync, installation, removal and cleanup. Native child-process tests kill a writer
at every transaction I/O boundary, check stable readers and restart roll-forward.
See the [P1 acceptance record](stage-ii-p1-acceptance.md) for evidence and limits.

Explorer directory/unregistered-file operations and UI integration remain P5 work.
P2's shared semantic loader and document sessions remain pending; its round-trip
gate has not passed. Folder lowering/authoring validation is used only to verify
P1 cache candidates. Desktop/CLI folder integration and automatic migration are
not enabled. Windows directory durability remains the P7 release gate. No P2 work
is included in this completion increment.

The first P2 increment adds the `apiforge-project` crate. `ProjectSession` opens
an existing manifest (a folder without one is not written), creates a new empty
project explicitly and owns one `DocumentSession` per registered document.
Published revisions are derived from their immutable sources in dependency order;
a revision that does not lower is an integrity error. Documents keep exact text,
a source version, diagnostics, a current or last-valid model and a bounded model
history. Missing or non-UTF-8 sources are explicit unavailable states.

Analysis inputs are immutable and can run on a worker. Results apply only when
session epoch, document ID, source version and context generation all match;
configuration edits and reloads invalidate outstanding results. Identities come
from the session's previous model, otherwise from the shared checkpoint (adopted
as-is only when it describes exactly this source, closed context and semantics),
otherwise they are fresh and placed by a host-supplied placer. Graphical semantic
edits reprint canonical folder source; block movement changes layout only. Save
settles pending analysis, writes exact text (including invalid text), seals the
document's checkpoint only for valid source and adds dirty configuration. Events
carry a sequence and an origin, so several views can follow one log.

`prepare_project` writes a model-backed project as a folder in one transaction,
keeping entity, node and edge IDs, positions, `autoPlaced`, proxies and every
published revision; each component gets a working document at its latest
revision. It is the basis for P3 migration and does not touch originals.

The P2 round-trip gate passes headless: every fixture project, plus one with
variable/environment proxies, cleanup and an unused second revision, reopens with
identical scenario models and published revisions. Diagram layout and semantic
edits, then a code edit, survive save, close and reopen with unchanged IDs,
positions, proxies, cleanup and pinned revisions. Saved invalid source reopens as
text without a model and regains its identities when fixed. Session tests cover
stale results after source, configuration and reload changes, hidden pending
documents, an invalid document not locking another, component-only projects,
history projection, shared event logs, read-only second sessions and external
edit conflicts.

The second increment protects unsaved work. Recovery stores each dirty or
layout-dirty document's exact text, directional selection, field drafts and an
identity/layout checkpoint of its model. On reopen a draft whose base is still
the saved source is restored automatically (text, view state and unsaved
layout); any other draft is held for an explicit keep or discard, blocks edits
and saves of its document and survives later recovery writes. Unreadable,
unsupported or foreign recovery is preserved and blocks recovery writes; a
backup candidate needs explicit restoration. Saving a document removes its draft.
Valid models are also stored as verified last-valid snapshots: saved invalid
source reopens with its last-valid model only for the same closed configuration
and dependency context, and never as a current model. Creating a project adds
`/.apiforge/local/` to `.gitignore` without rewriting existing rules.

Closing a session succeeds only after unsaved work is written to recovery; a
read-only session or unusable recovery returns the session so the caller can
offer export, and an explicit discard removes the drafts.

P2 is complete at the session-library boundary. Inspector widgets that produce
field drafts arrive with P4; the session stores and recovers their drafts but
does not yet apply them to models. Item 5 of the
[P1 code review](stage-ii-review-notes.md#p1-code-review) remains open.
Desktop and CLI do not use folder sessions yet.

P3 adds `migrate_workspace` in `apiforge-project`. It runs under the exclusive
legacy workspace session and records SHA-256 hashes of the workspace, drafts and
journal files and their backups. Each project is prepared in
`migration/<migration-id>/<project-id>/` beside the workspace with
`prepare_project`. Legacy drafts move into the folder's local recovery with their
original base hashes and cursors. The folder printer produces the same text as
the legacy printer, so a draft that matched its definition is restored
automatically and any other draft is held. Inspector field drafts join their
owner's recovery entry, including those of inactive projects. A draft of an older
component revision becomes a separate `recovered` working document based on that
revision, with its own checkpoint; the primary working document stays on the
latest revision. Drafts of missing definitions are reported and remain in the
untouched original file.

Every prepared folder is reopened through the folder loader without recovery and
compared with the workspace: scenarios, all published revisions and working
copies must keep semantics, identities, layout and proxies. A marker records the
owning migration. The inputs are rechecked before publication; a renamed folder
never replaces an existing `projects/<project-id>/`. A durable index records each
prepared folder's digest and publication state. A retry with unchanged inputs
resumes the same migration and reuses only folders it published itself, unchanged;
anything else is reported as occupied and left alone. Changed inputs start a new
migration. A restart after a rename but before its index update also verifies
the destination's marker and digest before acknowledging that publication.
Migration indices, catalogues and markers are validated before use; unsupported
versions, unsafe IDs, inconsistent entries and linked paths are preserved and
rejected before migration can modify them. Writing `projects.json` activates the
catalogue: it holds the
migration ID, input hashes, every project path, the previously active project and
the legacy journal, whose unresolved runs stay under global review. Originals are
never modified. Startup does not call migration until P7.

Tests migrate the fixture workspace with proxies, cleanup, a second component
revision and active, inactive, field, older-revision and orphaned drafts, plus
every fixture project file. They also cover occupied destinations, resumption
after preparation, an interrupted publication, a lost catalogue with reused and
tampered folders, changed inputs and read-only sessions.

Acceptance must cover:

- Code → diagram → graphical edit → save → restart preserves semantics, identity,
  layout, variable proxies and exact revision bindings, including main and cleanup
  graphs.
- Same handle/kind retains ID; separately applied delete/recreate and kind changes
  get new IDs. Missing/corrupt/stale diagram records do not replace source semantics.
- Invalid saved source with valid, missing, foreign or obsolete cache; pending
  analysis in hidden tabs; independent text/model histories and restored carets.
- A publication cannot change an existing scenario; unqualified calls cannot
  silently acquire new behaviour. Old draft publication remains explicit.
- Context changes discard late analysis even if source did not change. Two test
  view subscribers observe the same version without feedback loops.
- Crashes before/after transaction activation, each file operation, commit and
  cleanup; stable readers never consume a mixed generation.
- External equal-length edits, deletes, renames and manifest changes, including
  races with saves and recovery. No automatic overwrite of a foreign version.
- Rename/move/delete/restore of dirty open tabs, directory deletion, case-only
  collisions, permissions, full disk and path-boundary rejection.
- Migration of multiple projects, inactive/invalid inspector drafts, older
  component drafts, unresolved journal, changed input and occupied destinations.
- Known secrets never appear in new managed payloads or diagnostics. Existing
  original files remain unmodified. A failed recovery write prevents close.
- Snapshot Run, last-valid confirmation invalidation, stale reports, journaling
  failure and interrupted-run acknowledgement.

Run the required commands in the [development guide](development.md): formatting,
clippy, workspace tests and fixture verification. Language changes also run the
existing fuzzers; layout changes run its release benchmarks. Preserve the 1000-line
analysis budget and test the existing 1 MiB source limit. Polling and file saves
must not block UI frames.

Native QA includes Linux and Windows, keyboard use, file dialogs, restart/recovery,
read-only locks and both 960 × 540 and larger windows. Windows directory durability
remains a release gate; Linux sandbox results do not close it. Record accessibility
evidence for new controls without claiming that this closes all of M7.

Baseline exploration at `d967a79` passed the script/storage tests and 180 desktop
tests with isolated config/data directories and serial desktop execution. One
desktop cleanup-close test was excluded because the sandbox disallowed its local
TCP listener. These are baseline observations, not Stage II acceptance or native QA.
