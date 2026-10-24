# Stage II: folder projects and the code workspace

- Status: P1–P5 merged; P6a–P6d implemented and pushed for review; P8 documentation consistency in review; P7 activation, integration and release gates pending
- Date: 2026-10-24
- Historical analysis baseline: `d967a79`; initial implementation baseline: `c9c5ba0`. Later package bases are recorded below.
- Delivery: one branch per package from current `main` or a required unmerged predecessor; a human merges after review. P6 and P8 review branches do not imply a merge to `main`.
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

System fullscreen, IPC, collaboration, binary/rich-document editing, new variable-
proxy functionality and a general symbol-rename/navigation feature are outside
this stage. By the user's scope update on 2026-10-24, P6 also opens and edits
project text files such as `.gitignore` inside the application. M3 variable
proxies were merged in `c9c5ba0` and must survive folder
storage, migration and reconciliation. M7 remains tracked by the
[original roadmap](roadmap.md).

## Historical analysis and verified review findings

At the analysis baseline `d967a79`, the language already parsed, lowered, printed
and reconciled scenarios/components, and desktop buffers supplied debounce,
invalid-source protection, draft recovery and selection synchronisation.
Folder projects were the missing foundation: the GUI imported JSON only,
`WorkspaceStore` persisted the whole workspace, and `App`/`Editor` combined
orchestration with one active document history. The packages below replace that
folder-mode architecture while retaining legacy JSON compatibility.

The independent review was checked against the code, rather than accepted as an
implementation specification:

| Review point | Verified decision |
| --- | --- |
| Multi-file saves, cache provenance and dependency versions need explicit contracts | Accepted; ADRs 0010–0012 define them. |
| Component calls must retain their revision | Accepted. The printer emits revisions; legacy JSON lowering keeps implicit latest. Folder validation now requires explicit revisions. |
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
and read-only status. P6 adds a selectable project-root row, single-click file
opening and in-application editing of project text files such as `.gitignore`.
Other file types can still be revealed through the system. The explicit
[P6 UI acceptance requirements](#p6-ui-quality-and-text-file-acceptance) apply to
the explorer and tabs.

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
apiforge check --folder <folder> [--file <relative.afs>]
apiforge print --folder <folder> --file <relative.afs>
apiforge apply --folder <folder> --file <relative.afs> <source.afs>
apiforge run --folder <folder> --scenario <relative.afs|id> [...]
```

`print` emits canonical source from a valid definition. `apply` requires valid
input and an exclusive project lock. `fmt` retains its current single-file
behaviour. Desktop adds `--folder <path>`; `--project FILE` retains its existing
session-only JSON meaning. Folder commands use explicit component revisions and
the same stable-read protocol as the GUI. They are available as soon as P6c is
reviewed and merged, without waiting for P7 startup activation. `--folder` is the
folder selector in both applications; the earlier proposed CLI `--project`
spelling is withdrawn. Legacy JSON commands retain `--workspace`.

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

P0 planning landed on `main`; P1–P5 were merged after review. Each remaining
package uses a separate branch from the latest `main`, or from the tip of an
unmerged predecessor it requires. Record that dependency/base in its final docs
commit, together with implementation, limits and verification. Push the branch;
merging belongs to human review. Folder CLI commands become available behind
`--folder` with P6c. Default folder startup and automatic migration stay disabled
until P7's activation package passes its integration gates.

| Package | Implementation | Required exit evidence |
| --- | --- | --- |
| P0 | This specification, three ADRs and links/corrections to existing docs | Reviewed contracts; no claim that Stage II already works |
| P1 | Folder schemas, ProjectStore, stable reads, lock, transactions, recovery and trash | Fault injection at every persistence boundary; conflict handling |
| P2 | ProjectSession/DocumentSession, authoring validation, analysis tickets and histories | Independent documents, empty projects and rejected stale results |
| P2 gate | Create folder project, save, close and reload through the new loader | Matching semantics, IDs, layout and pinned revisions before migration work |
| P3 | Legacy workspace, draft and journal migration | All fixtures verified; crash/resume and catalogue activation tested |
| P4 | Code/Diagram/Split, tabs, shortcuts, focus and diagnostics | Mode/tab switching retains work and histories |
| P5 | Explorer, import, file operations, trash and external polling | Operations survive restart; no silent overwrite |
| P6a | Explorer/tab ergonomics, project text files and BUG-035–043 | Ten UI acceptance probes and native review |
| P6b | Component publication, comment-loss decisions (A3), revision pin fixes | Immutable/old/recovered revisions; version-bound consent |
| P6c | `check/print/apply/run --folder` over saved definitions | CLI/GUI equivalence; stable reads; no recovery/cache fallback |
| P6d | Folder Run/reports, project/global journal review and close export (A4) | Immutable snapshots, invalidated consent, stale reports and protected close |
| P8 | Documentation consistency and ownership of review decisions | Current status per package; B7–B10 decided or explicitly deferred |
| P7a | Open Folder, recent projects and recovery before switching (A1) | Switching/reopening retains work; failed recovery prevents switching |
| P7b | Project settings editor (A2) | Dirty manifest and independent save; session-only secrets |
| P7c | Native transaction/conflict recovery and export (A5) | Explicit preview-bound decisions; failure preserves work |
| P7d | Storage worker and asynchronous project I/O (A6) | Responsive frames during slow reads/writes; serial mutations and stale-result rejection |
| P7e | B7 merge evidence, B8 Git attributes and B9 external-writer boundary | Independent checkpoint merges; LF/CRLF and checkout tests |
| P7f | Performance, keyboard/accessibility review and user documentation | 1000-line/1 MiB budgets; Linux native and accessibility evidence |
| P7g | Startup activation through `migrate_workspace`; new managed folder projects | Migration/creation/restart integration; acceptance matrix; Windows remains an explicit release gate |

Implement P1/P2 first and pass the P2 round-trip gate before P3. Until P7, folder
projects are opened explicitly for development; do not enable automatic migration
in the normal startup path before integration evidence is complete.

### P6 UI quality and text-file acceptance

On 2026-10-24 the user approved merging P5 through `0fcff3e` and assigned the
remaining UI work to P6. That historical P5 decision retained tracked follow-up
work; P6a subsequently fixed the findings and passed the UI gate. The requirements
below remain part of P6 acceptance together with publication, CLI and Run/report
integration.

The user's four reported issues are mandatory P6 work:

1. **Integrated file tabs (BUG-036).** The close button belongs inside the tab's
   visible surface, with a stable clickable area. Name, dirty/error/conflict
   state and close control form one coherent tab; closing must affect only that
   file and preserve Save/Discard/Cancel protection.
2. **Predictable opening (BUG-037).** One explorer click opens or activates a
   file without Refresh or extra clicks. Enter remains supported. Repeated
   activation keeps one tab per file. The user's report mentioned three clicks
   and possibly Refresh; QA confirmed selection-only single clicks, while a
   controlled double-click and Enter opened the file without Refresh.
3. **Selectable project root (BUG-038).** Render a named root row that supports
   mouse and keyboard selection, collapse/expand and explicit destinations for
   new files/folders, import and drops. Creating an item must show its intended
   destination. The root itself cannot be renamed or deleted through ordinary
   explorer file actions.
4. **Project text files (BUG-039).** Open files such as `.gitignore` and
   `README.md` as text in application tabs. Define their identity, exact-text
   saving, dirty state, Save All, close decisions, recovery, read-only handling
   and external-change conflicts. These files do not become scenario/component
   definitions and cannot be run as scenarios. Keep project boundaries and
   hidden control directories protected; any raw registry editing needs an
   explicit validated reload policy.

The accompanying QA findings are part of the same P6 acceptance:

- Reveal the active tab when it is opened or selected by Ctrl+Tab, even when
  the strip overflows (BUG-040).
- Bound long tab labels and visibly disambiguate equal filenames with a minimal
  parent-path suffix, retaining full paths on hover (BUG-041).
- Expose named explorer items, selection and expansion state through
  accessibility metadata, and show keyboard focus (BUG-042).
- Scroll the active document into view in the explorer even when its row was
  outside the virtualised visible range (BUG-043).
- Fix new folder-project cards overlapping in Diagram and Split (BUG-035).

The UI must be visually coherent, modern and ergonomic: consistent typography,
spacing, control sizes, alignment and hover/focus/active/disabled states across
the header, tabs, explorer, menus and dialogs. Review Code, Diagram and Split at
960 × 540 and larger sizes, with long names, duplicate filenames, many files and
tabs, and dirty/invalid/conflicted/read-only documents. Controls and selection
must remain visible and usable. Visual review of the actual native window is
required in addition to functional tests.

Use real pointer and keyboard events in regression tests; calling an `Open`
action directly is insufficient evidence for click behaviour. The reproducible
acceptance probes are in [tools/qa/ui-quality](../tools/qa/ui-quality/run.sh),
with [baseline results and native evidence](qa/2026-10-24/p5-ui-quality/results.json).
Run `tools/qa/ui-quality/run.sh HEAD` against the P6 candidate. At the P5
baseline, two controls passed and eight acceptance probes failed; all ten pass
from P6a onwards. They remain separate from the standard workspace suite.
Windows and the broader release/accessibility review remain P7 gates. The gate
builds an identical package from a temporary checkout, so give it its own
`CARGO_TARGET_DIR`; sharing the repository's leaves a stale binary behind.

### Implementation progress

This is the current state per package. Detailed historical increments remain in
Git and the linked QA records. Implemented review branches stay distinct from
merged delivery. P8 uses P6d `001414f` as its unmerged prerequisite and incorporates
the input findings from `docs/stage-ii-p8-findings` at `cedabc3`.

| Package | State and implementation | Verification and remaining limits |
| --- | --- | --- |
| P0 | Approved plan and ADRs 0010–0012 on `main` | Contracts and package ownership documented; P8 corrects status drift |
| P1 | Merged: folder schema/store, stable reads, lock, redo transactions, recovery, trash, checkpoints and verified cache | Linux persistence/fault and child-process interruption tests; [P1 acceptance](stage-ii-p1-acceptance.md). Windows directory durability remains open |
| P2 | Merged: shared per-document sessions, exact source, ticketed analysis, independent histories and recovery; empty/component-only authoring | Round-trip gate passed before migration: fixture semantics, identities, layout, cleanup, proxies and explicit revision bindings survive restart. Late results, independent documents and two subscribers are tested |
| P3 | Merged: `migrate_workspace` prepares/verifies all projects before one catalogue activates them; original v2 workspace/drafts stay intact; older component drafts remain separate | Fixtures, multiple/inactive/invalid drafts, occupied destinations, changed inputs and crash/resume are tested. Library implementation exists; automatic startup invocation waits for P7g |
| P4 | Merged: Code/Diagram/Split, tabs, shortcuts, text/model history, diagnostics, field recovery and close decisions behind `--folder` | Reviewed baseline: 743 tests, two helpers ignored. Mode/tab/caret/history and 960 × 540 are tested. Source saves and ordinary recovery remain synchronous until P7d |
| P5 | Merged: real explorer, import/registration, file operations, trash and asynchronous external scans | 791 tests, two helpers ignored. Conflicts, missing dirty documents, bounded scans and case-only rename are covered. Its open UI findings were assigned to P6a |
| P6a | Pushed: `feat/stage-ii-p6-ui`, base `9012180`, tip `aa3c356`. Project text tabs, integrated/disambiguated tabs, root selection, one-click opening, named rows/reveal and measured card placement | 808 tests, two helpers ignored; UI gate 10/10 and [native review](qa/2026-10-24/p6-ui/verification.md). Broader accessibility and Windows QA remain P7 |
| P6b | Pushed: `feat/stage-ii-p6-publish`, base unmerged P6a `aa3c356`, tip `d04d4f2`. Atomic immutable publication, recovered drafts, explicit pin fixes and version-bound comment decisions (A3) | 821 tests, two helpers ignored; UI gate 10/10; both 60-second language fuzzers and [native publication review](qa/2026-10-24/p6-publish/verification.md) passed |
| P6c | Pushed: `feat/stage-ii-p6-cli`, base unmerged P6b `d04d4f2`, tip `bea2441`. Saved-only shared CLI loader, `--folder`, exact valid Apply, closed Run snapshots and abstract project journal ownership | 832 tests, two helpers ignored; UI gate 10/10. Seven CLI tests cover equivalence, lock/failure handling, explicit pins, recovery/cache exclusion and untouched checkouts; four new journal tests cover ownership and persistence |
| P6d | Pushed: `feat/stage-ii-p6-runs`, base unmerged P6c `bea2441`, tip `001414f`. Folder Run/reports, sealed earlier configurations, consent invalidation, global legacy review and exact/public close export (A4) | 843 tests, two helpers ignored; UI gate 10/10 and [native Run/export review](qa/2026-10-24/p6-runs/verification.md). Closing retains ownership through cancellation/cleanup. Ordinary project/journal I/O moves to P7d |
| P8 | Documentation-only review: current package summaries/statuses and decisions B7–B10; base unmerged P6d `001414f`, findings input `cedabc3` | 843 tests, two helpers ignored; formatting, clippy and fixtures passed. No runtime/format change. Decision implementation/evidence belongs to P7e; startup remains inactive |
| P7a–g | Pending, in the delivery order above, each on its own branch | Open Folder/recent/switching, settings, native recovery, storage worker, portability evidence, performance/accessibility/docs, then activation. Linux evidence cannot close Windows QA or Windows durability |

Every completed package runs formatting, all-target workspace clippy with warnings
denied, serial all-target workspace tests with isolated config/data directories,
and converter fixture checks. The separate UI quality gate uses its own target
directory. Counts above exclude that ten-probe gate. Language changes also run
both sustained smoke fuzzers; layout algorithm changes run release benchmarks.

P8 decisions are in [review notes](stage-ii-review-notes.md#plan-consistency-review).
B7 retains one deterministically ordered `diagram.json`, with a realistic
independent-document merge gate assigned to P7e. B8 adds LF Git attributes during
explicit initialisation/migration without rewriting source or the Git index;
P7e carries the code and tests. B9 defers a local-head reload bump because it
cannot make Git checkout atomic; existing ticket invalidation and best-effort
fingerprint checks remain. B10 keeps cache verification in storage, deferring a
crate split until it has a concrete architectural benefit.

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
