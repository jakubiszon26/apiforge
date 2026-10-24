# Stage II P1 acceptance — folder storage

- Date: 2026-10-24
- Branch: `feat/stage-ii-code-workspace`
- Scope: P1 storage services; no folder session, desktop activation or migration
- Result: implemented and verified on Linux; Windows durability remains a P7 release gate

## Implemented contracts

| Area | Evidence |
| --- | --- |
| Strict registry, head, managed paths, lock and stable reads | Schema/path tests, symlink/case-boundary rejection, native writer/read-only processes |
| Multi-file redo transactions | Exact old/target fingerprints, staged hashes, activation, idempotent replay, foreign-state rejection |
| Source/inspector recovery | Exact invalid/CRLF text, checked backup rotation, preview-bound restoration, stale/read-only/secret refusal |
| Diagram candidates | Typed identities/layout/proxies, deterministic ordering, working and published keys separated |
| Trash and restoration | Exact saved and unsaved work preserved separately; registry order/IDs retained; published revisions untouched |
| Explicit conflict resolution | Foreign bytes preserved before descriptor-bound approval; restart verifies receipts and copies |
| Last-valid cache | Source/model semantic equality, pinned transitive dependency closure, public configuration, provenance and full snapshot digest |
| Damaged/future metadata | Optional cache ignored with notice; explicit replacement archives original bytes before installing new metadata |
| Persistence failures | Disk-full/permission injection before and after real I/O boundaries; partial writes, replay failures, archive/recovery/cache failures |
| Native interruption | OS lock ownership/release and child-process kill at every transaction persistence boundary; stable readers and reopened recovery |

The cache wire DTO is private. Both capture and load verify source, model,
dependency closure and digests. Returned snapshots are immutable candidates,
never a declaration that a current source model exists. Folder-specific lowering
requires explicit revisions, supports component-only projects and permits a
working component to reference its own older publication. Legacy language policy
is unchanged. Session tickets, cache promotion policy and GUI run consent remain
future integration work.

Fault probes cover temporary creation, chunked writes, file sync, replacement,
no-clobber installation, directory sync, directory creation/removal, file removal
and transaction cleanup. They are test-only and cannot be enabled by production
environment variables. Process termination exercises the real filesystem; this
is not a physical power-loss test.

## Verification

All final commands passed:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked -- --test-threads=1`: **661 passed,
  zero failed, two ignored child helpers**, launched by their parent tests.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
- `cargo build -p apiforge-desktop --locked`
- `git diff --check`

The workspace test run used fresh `XDG_CONFIG_HOME`/`XDG_DATA_HOME`, serial test
execution and loopback TCP access for existing HTTP tests. An initial parallel
run using host preferences failed two existing desktop assertions expecting a
closed code panel; the isolated run passed. Storage's new fault/process tests
also passed with the default parallel test runner before the final aggregate
cache-limit test was added; that final test passed in the complete run above.

Language fuzzing completed for 60 seconds per fuzzer:

- Syntax seed `1592639710`: 4,693,435 cases, 6,571 valid.
- Semantics seed `2718281828`: 81,866 cases, 12,408 lowered without errors.

No layout algorithm changed, so the layout benchmark was not required. Native
interactive regression testing after the implementation commit passed the exercised
paths; see its [QA record](qa/2026-10-24/p1-native-interactive.md) and screenshots.
These library tests alone do not claim GUI coverage.

## Remaining stage gates

P2 has not passed its folder source/model/identity/layout/revision round-trip gate.
`ProjectSession`, independent document histories, folder loading and CLI/GUI
integration remain pending. P2 is not started by this completion increment.
Explorer integration, migration, startup activation and run confirmation follow
the existing delivery sequence.

Windows native filesystem verification, especially directory durability, remains
open for P7. Linux fsync and process-kill results cannot establish Windows or
physical power-loss guarantees. Existing review feedback about LF/Git attributes,
external checkout detection and shared checkpoint merge ergonomics remains in
[review notes](stage-ii-review-notes.md), for the appropriate initializer/session
integration work. Exact-byte transaction fingerprints are retained.
