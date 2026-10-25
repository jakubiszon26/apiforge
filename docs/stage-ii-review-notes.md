# Stage II review notes

## Shared diagram checkpoint file and Git conflicts

- Recorded: 2026-10-24
- Source: user review
- Status: B7 verified in P7e: deterministic single-file independent-document
  merge and exact model reload passed; no format change
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

P8 retained the implemented single-file granularity. P7e verified a real merge
of two branches that edit different scenarios, followed by exact model reload.
The [portability evidence](qa/2026-10-25/p7-portability/verification.md) records
its covered case and limits. If the format changes, update
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
- Status: B8 implemented in P7e: narrow LF attributes on explicit creation/
  migration, exact-save/index/original preservation and actual Git checkout tests;
  Windows native evidence still open
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

The original review proposed emitting canonical/generated project files with LF and
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
- Status: B9 verified/deferred in P7e: no durable reload-generation bump;
  paused replacement tests with/without a writer demonstrate the best-effort
  external-writer boundary and ticket invalidation
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
  item 5 explicitly deferred by B10 in P8
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

## Plan consistency review

- Recorded: 2026-10-24
- Source: review of the Stage II plan, ADRs 0010–0012, these notes and the roadmap
  against P5 (`feat/stage-ii-p5-explorer`), re-checked against `main` at `9012180`
  after the P6 scope update in `692f199`
- Status: resolved by documentation-only P8 after P6 and before P7; the
  implementation/evidence gates assigned below remain open
- Applies to: [Stage II plan](stage-ii-code-workspace.md), ADRs 0002, 0005,
  0007–0012, [roadmap](roadmap.md)

P8 is documentation only. It corrects the statements below, assigns every
unowned requirement to one package and gives each open note in this file a
decided or deferred status. Code changes stay in the packages they are assigned
to. Format changes, if any, update ADRs 0010/0011 together with their code.

The input findings were pushed as `docs/stage-ii-p8-findings` through `cedabc3`.
P8 is based on the unmerged P6d tip `001414f`; it changes documentation only and
records implemented review branches without implying that a human merged them.

### Requirement ownership

| # | Requirement | Current state | Assign to |
| --- | --- | --- | --- |
| A1 | Open Folder, recent projects and project switching, with recovery written before a switch | Implemented in P7a with failed-recovery retention and read-only export; native chooser review remains P7f | P7a complete in review |
| A2 | Editing project settings marks the manifest dirty | Implemented in P7b for empty/component-only projects; public settings save independently, secret values stay in memory | P7b complete in review |
| A3 | Decision before a graphical edit or publication drops unattached comments | Implemented and tested in P6b, including history and obsolete consent | P6b complete in review |
| A4 | Export when closing a read-only session or after a failed recovery write | Implemented and tested in P6d; exact/public bundle, cancellation/failure and obsolete-export checks | P6d complete in review |
| A5 | Native transaction-recovery and conflict dialogs with export | Implemented in P7c with exact candidates, obsolete-decision refusal, real GTK export and preserved recovery reload; I/O moves to P7d | P7c complete in review |
| A6 | All I/O off the UI thread; one storage worker serialises mutations | Implemented in P7d: dequeue-time session snapshots, newer-edit preservation, asynchronous folder/journal/recovery operations, close ownership and stale chooser rejection; slow-I/O input tests and native syscall evidence | P7d complete in review |

### Decisions and remaining evidence

| # | Decision | Reason and remaining work |
| --- | --- | --- |
| B7 | Retain one deterministically ordered `.apiforge/diagram.json` | P7e passed a realistic Git merge test for independent scenario edits, including full identity/layout/proxy/cleanup snapshots and exact saved reload. No format migration is needed for this measured case; same-record/context conflicts remain possible |
| B8 | Emit canonical/generated files with LF; append LF attributes during explicit initialisation and migration; preserve existing attributes and exact source saves | P7e implemented code and real checkout tests. Use `*.afs text eol=lf` plus root `apiforge.project.json` and API Forge's shared diagram JSON, rather than changing every user JSON file. No index edits, renormalisation, original-source rewrite or newline-normalised hashes |
| B9 | Defer a durable local-head bump after external reload | It cannot prove an atomic uncoordinated Git checkout and adds another write/failure boundary. Keep exact fingerprint checks, analysis context/session invalidation and explicit dirty conflicts. P7e passed channel-controlled paused external replacement tests with and without a writer, documenting the limitation rather than claiming atomic checkout |
| B10 | Keep storage's language dependency for verified cache candidates; defer crate extraction | The dependency checks the cache's exact source/model equivalence, while general analysis policy remains in `apiforge-project`. Extract only when a concrete architectural change removes that verification need without duplicating policy; no Stage II gate depends on a crate split |

B7's recommendation is to keep the accepted format with measured merge evidence,
rather than pre-emptively split every record. If the gate demonstrates conflict
between independent documents, splitting becomes P7e code work and must revise
ADRs 0010/0011, managed paths, loaders and transaction/recovery tests together.

B8's recommendation is exact-byte integrity plus narrowly scoped Git attributes.
Existing/nested overriding attributes and already-tracked CRLF files can still
produce incompatible immutable revisions: detect and report them, preserving
bytes. P7e tests Linux `core.autocrlf=true` checkout behaviour with/without generated
attributes, nested overrides, exact CRLF saving and line-ending-only recovery
conflicts. Native Windows checkout and directory durability remain open gates;
Linux emulation does not close them.

### Consistency corrections

- Roadmap now records M3 complete and M7 pending, with P1–P5 merged, P6 in review
  branches and P7 activation pending. There is no universal implementation branch.
- ADRs 0002/0005/0007/0008/0009 distinguish implemented folder amendments from
  legacy JSON rules; ADRs 0010/0012 record implemented P6 integration and the
  remaining P7 worker/startup/release gates.
- Implementation progress is a current summary per package. The P2 round-trip
  gate has passed, the explorer is implemented and the folder CLI shares the
  session loader. Historical increment details remain in Git/QA records.
- The obsolete instruction to remove a temporary review file is removed.
  Historical analysis/initial implementation baselines are labelled as such;
  later package bases are explicit.

### Naming and scope

- **Decided at the start of P6:** one name selects a folder project everywhere.
  Both the CLI folder commands and the desktop take `--folder <path>`; the plan's
  earlier CLI spelling `--project <folder>` is withdrawn. Desktop
  `--project FILE` and CLI `--workspace FILE` keep their existing JSON meanings, so no established argument
  changes behaviour. P6c carries the specification change with its code.
- F2 follows keyboard focus: file rename belongs to the focused explorer; code
  symbol rename remains M7 work.
- Folder CLI commands are available with explicit `--folder` as soon as P6c is
  reviewed/merged. Default folder startup and automatic migration remain P7g.

P8 verification (Linux, 2026-10-24): documentation-only diff, local link targets
checked, formatting, all-target workspace clippy with warnings denied, converter
fixtures and all-target workspace tests passed: 843 passed, two parent-launched
helpers ignored, serially with isolated config/data directories. Runtime code is
unchanged from P6d. The separate UI quality gate passed 10/10 with
`CARGO_TARGET_DIR=/tmp/apiforge-gate`.
Windows QA and Windows directory durability remain open.

## Identity checkpoints at registration and import

- Recorded: 2026-10-25
- Status: open decision; a failing test is committed as `#[ignore]`

`prepare_project` and `create_document` seal a record in the shared
`.apiforge/diagram.json` describing a document's block identities and layout.
`initialise`, `register_existing` and `import` do not. A document that entered a
project through any of them is therefore given fresh block identities on every
load until something saves it: stored layout stops matching, and a diagram
selection can name a block that no longer exists.

Sealing at those four call sites would write a Git-tracked file while
registering sources the project did not author, which is what
"initialisation registers existing `.afs` without rewriting their content" was
written to avoid, and it touches B7's single deterministic checkpoint file and
the P7e portability policy together. The alternatives are to seal on first
successful analysis instead, or to accept the current behaviour and have the
first save settle it.

`a_registered_or_imported_document_keeps_its_block_identities_when_the_project_is_reopened`
in `crates/apiforge-project/tests/acceptance.rs` states the contract and is
ignored until this is decided.
