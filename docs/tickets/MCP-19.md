# MCP-19: On Windows an agent's commit beside an observing window can answer `recovery_required`

- Size: M (complexity: the storage protocol's replacement primitive, ADR
  0011; regression potential bounded by the fault-injection suite and the
  Windows loop) · Owner review: 20 min · Risk: none visible; ADR 0011 is
  amended
- State: Analysis (issue #191); no owner decision — nothing the person
  sees changes except that the refusal stops happening
- Analysis model: Fable · Implementation model: —

## Goal

An agent session that holds the writer lock commits beside an observing
window on Windows exactly as on Linux. A reader that merely looks at a
file — the window following the agent, the window's own once-a-second
check beside its own save, a command-line reader — never turns a commit
into *recovery required*, and never leaves a tab open after a delete.

## What the owner will see

- New: nothing.
- Changed: on Windows, a write the agent makes while the window observes,
  and a delete or save the window makes while its own check runs, now
  succeed where they failed about once in a hundred times. The messages,
  codes and the recovery view are unchanged for the cases that remain
  (a third program holding a file open for longer than the bound below).
- Removed, hidden, migrated, default behaviour: nothing. The project file
  format and the MCP output are unchanged.

## Mock-up

Not applicable: no interface change.

## Cause

Established from the code and the CI evidence, with one measurement left
to the Windows loop (criterion 6). Confidence: high for the mechanism,
which the test suite itself has already met once (the comment on
`wait_for_journal` in `crates/apiforge-mcp/tests/acceptance.rs`: *polling it
meanwhile would race the runner's atomic replace, which Windows refuses
while the file is open*).

**What happened.** Run 41334167166 (windows-2022, PR #186, `9f801c5`):
`mcp10_criterion_3`'s `write_source` answered

> `recovery_required` — The write stopped after its commit began; whether
> the files changed is unknown and the project needs recovery

`wrote: unknown`. That text is `commit_failed`
(`crates/apiforge-mcp/src/agent.rs:845-858`): the transaction failed, the
agent reread head, found the transaction still active and said so. The
head was active because the failure came *after* activation, during
replay (ADR 0011 step 4) or at the commit's head write (step 5). Before
activation the same failure would have been a plain refusal with
`wrote: false`.

**Why it failed.** Every file the protocol installs goes through
`project::io::atomic` (`crates/apiforge-storage/src/project/io.rs:32`): a
`tempfile::NamedTempFile` in the target's directory, synced, then
`temporary.persist(path)`. On Windows, `persist` (tempfile 3.27,
`src/file/imp/windows.rs:92`) is `MoveFileExW(old, new,
MOVEFILE_REPLACE_EXISTING)` and nothing else. That call replaces the
target only when **no handle to the target is open**; with one open — even
a Rust reader, whose `File::open` shares read, write *and delete* — it
fails with `ERROR_ACCESS_DENIED`. (Rust's own `std::fs::rename`, since the
toolchain this project uses, tries `MoveFileExW` and, on exactly that
error, renames through `SetFileInformationByHandle(FileRenameInfoEx)`
with `FILE_RENAME_FLAG_POSIX_SEMANTICS | REPLACE_IF_EXISTS`, which
supersedes a target whose handles were opened with `FILE_SHARE_DELETE`;
the open handles keep the old bytes. `tempfile` has no such fallback.)
Linux has no restriction of this kind, so Linux has never shown it.

**Who held the file.** An observing window reads the project on its own
schedule: the watcher's check thread (`folder/watcher.rs::poll`) runs once
a second and at once after every `saved` from the agent, and it hashes
every source and the registry (`ProjectStore::scan_tree` →
`hash_file`), then reads what changed with a stable read (head, registry,
the files, head again; `transaction.rs::stable_read_with_hook`). Each
hash and each read opens the file for a moment. In `mcp10_criterion_3`
the agent writes the scenario twice and the window checks between; the
second write (`agent_breaks_the_price`) lands beside a check often enough
to be seen once in two Windows runs. Antivirus on the runner (Defender's
real-time scanning is on in the GitHub image) opens freshly written files
too, which widens the window further and explains the same refusal
without any API Forge reader (KTLO-33 flake 4).

**The same race inside one process.** `scan_tree` does not take the
store's worker lock on purpose (`explorer.rs:98`, so that the check never
waits behind a save), while `commit_locked` holds it for the whole
transaction. In the writer's own window the check thread therefore hashes
files while the window's own transaction replaces or removes them. Two
Windows consequences: a replace fails as above, and a `remove_file` of a
source the hasher has open succeeds but leaves the name *delete-pending*
until the handle closes, so the transaction's `remove_dir` of the parent
fails with `ERROR_DIR_NOT_EMPTY`. Both are after activation. That is KTLO-33
flake 6, `deleting_a_folder_closes_its_tabs_and_the_trash_restores_unsaved_work`
(run 40593992618, docs-only PR #108): the delete reported an error and the
tab stayed. The same primitive under the observing window's take-back is
KTLO-33 flake 4.

**Why the protocol's own retry does not help.** ADR 0011's stable read
retries three times — on the *reader's* side. The *writer* has no retry:
once activated it must finish, and an I/O error at a target is reported as
recoverable intent, correctly. The fault is not the protocol; it is the
primitive under it, which fails on a condition that is not a fault.

## Decision (the agent's, recorded because it amends ADR 0011)

Settled: **the replacement primitive gains POSIX rename semantics on
Windows, and the scan is serialised with the worker in the same
process.** Three parts, all in `apiforge-storage`:

1. `project::io::atomic` and `file::write_atomic` install the temporary
   file with `std::fs::rename` (`NamedTempFile::into_temp_path`, rename,
   `TempPath::keep`) instead of `NamedTempFile::persist`. On Linux this is
   the same `rename(2)`. On Windows it is `MoveFileExW`, and on
   `ERROR_ACCESS_DENIED` the POSIX-semantics rename, so a target that a
   Rust reader — this window, another window, the MCP server, `apiforge`
   on the command line — holds open is replaced and the reader keeps the
   bytes it opened. ADR 0011's sync-before-replace, no-clobber for new
   files (`persist_noclobber` stays: it has no target to collide with)
   and the fingerprint rules are unchanged.
2. A reader that does **not** share delete (a third program) still makes
   the rename fail. The `Replace` boundary then waits for the handle, on
   Windows only: the rename is tried again every 20 ms for at most one
   second on `ERROR_ACCESS_DENIED` and `ERROR_SHARING_VIOLATION`; the
   same for `RemoveDirectory` on `ERROR_DIR_NOT_EMPTY` when the directory
   holds only names the transaction itself removed. After the bound the
   error is what it is today — recoverable intent, the recovery view, an
   honest `recovery_required` — because a program that holds a project
   file for more than a second is a conflict the person should see. This
   is the practice of Cargo and rustup for the same Windows condition; it
   is bounded, Windows-only, and never silently drops a write.
3. `scan_tree` takes the store's worker lock for the duration of the scan
   when the store is a writer. The check thread then waits a few
   milliseconds behind a save rather than hashing beside it; a read-only
   store's scan is unchanged, since its worker lock guards nothing another
   process does. The `Busy`/`RecoveryRequired` reasoning on an active head
   (`explorer.rs:97-115`) stands.

Not chosen: making the observing window stop reading while the agent
writes (a protocol message `writing`): it would not cover the agent beside
a command-line reader, the window beside itself, or antivirus, and it adds
a channel message for a fault of the primitive. Not chosen: switching the
window's hashing to modification times: ADR 0011 compares bytes on
purpose (KTLO-15).

What it means for the person: a save, a delete or an agent's write on
Windows no longer fails because something was looking at the file.

## Acceptance criteria

1. **The replace beside a Rust reader.** A storage test, run on both
   platforms, holds a target file open with `File::open` (the default
   share mode) from another thread for the whole commit, synchronised
   through the fault seam (`Boundary::Replace`, `Phase::Before` blocks
   until the reader holds the file; `Phase::After` releases it). The
   commit succeeds, the reader's bytes are the old content, a stable read
   after it sees the new generation, and no transaction is left active.
   Before the fix it fails on Windows at the replace with *Access is
   denied* after activation (shown in the acceptance commit from a Windows
   CI run of the branch).
2. **The reader that does not share delete.** The same test, Windows only,
   with `OpenOptionsExt::share_mode(FILE_SHARE_READ)` and a reader that
   closes 200 ms after the replace is first attempted: the commit succeeds
   within the bound. A reader that holds for two seconds: the commit fails
   after activation exactly as today, head active, `recover()` finishes it
   once the handle is closed. Both cases name the elapsed time in their
   assertion messages.
3. **The scan beside the worker.** A storage test blocks a commit at
   `Boundary::Replace` and calls `scan_tree` from another thread on the
   writer store: the scan returns only after the commit, with the new
   generation; on a read-only store opened on the same folder it is not
   held. A desktop test deletes a folder while a watcher check is forced to
   run (`Watcher::finish`'s seam) and shows the tab closed and the folder
   gone, on both platforms.
4. **The agent beside the observer.** `mcp10_criterion_3` is unchanged;
   a new mcp10 acceptance test has the agent write ten times while the
   window follows with `set_interval` at its minimum, every write
   `wrote: true`, no `recovery_required`, the bar following each commit.
5. **ADR 0011 amended** (*Lock, versions and stable reads*): the
   replacement primitive on Windows (POSIX rename, the bounded wait for a
   non-sharing handle, what remains a fault), and the scan's
   serialisation; the status line's *native Windows durability
   unverified* stays.
6. **The loop.** `mcp10_criterion_3`,
   `deleting_a_folder_closes_its_tabs_and_the_trash_restores_unsaved_work`
   and `mcp07::a_transaction_left_by_the_holder_opens_the_recovery_view`
   pass 20 of 20 on windows-2022 and 20 of 20 on ubuntu-24.04 with
   KTLO-33's loop workflow (or its steps run ad hoc on this branch if
   KTLO-33 has not merged), run ids in the QA report; before the fix the
   same loop reproduces at least one failure of `mcp10_criterion_3` or
   `deleting_a_folder…` on windows-2022, recorded with its run id, or the
   report says it did not and names the attempts.
7. The existing fault-injection suite (`fault_tests.rs`, `transaction_tests.rs`)
   passes unchanged; no assertion is weakened.
8. The verification commands of the development guide pass.

## Not in scope

- A directory removed by one process while another process's handle
  keeps a child delete-pending (agent deletes a folder the window hashes):
  covered by the bounded wait of part 2 only; a longer hold stays a
  recoverable conflict.
- Directory-metadata durability on Windows (`sync_directory` is a no-op
  there): ADR 0011's open release gate, unchanged.
- The cost of hashing every source once a second: backlog, *Later*.
- KTLO-33's test-side changes and its take-back retry (decision 2 there).

## Technical appendix

- `crates/apiforge-storage/src/project/io.rs:32-80` (`atomic`),
  `crates/apiforge-storage/src/file.rs:314-355` (`write_atomic`),
  `crates/apiforge-storage/src/project/explorer.rs:95-135` (`scan_tree`),
  `crates/apiforge-storage/src/project/transaction.rs:434-512`
  (`commit_locked`: activation, replay, cleanup), `:293-394` (stable
  read), `crates/apiforge-mcp/src/agent.rs:845-880` (`commit_failed`),
  `crates/apiforge-desktop/src/folder/watcher.rs:143-215` (check thread,
  no worker lock), `crates/apiforge-desktop/src/folder/file_prompts.rs:569`
  (`queue_delete`).
- tempfile 3.27.0 `src/file/imp/windows.rs:92-115`: `SetFileAttributesW`
  then `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`; no fallback.
- Rust 1.99.0 `library/std/src/sys/fs/windows.rs`, `pub fn rename`:
  `MoveFileExW(…, MOVEFILE_REPLACE_EXISTING)`; on
  `WinError::ACCESS_DENIED` opens the source with `DELETE` access and
  calls `SetFileInformationByHandle(FileRenameInfoEx)` with
  `FILE_RENAME_FLAG_REPLACE_IF_EXISTS | FILE_RENAME_FLAG_POSIX_SEMANTICS`;
  `OpenOptions::new()` shares `FILE_SHARE_READ | FILE_SHARE_WRITE |
  FILE_SHARE_DELETE`. No new dependency: part 1 is `std` and `tempfile`'s
  `TempPath`; part 2 reads `io::Error::raw_os_error()` (32 and 5, 145)
  behind `#[cfg(windows)]`.
- The fault seam already has `Boundary::Replace` and `RemoveDirectory`
  with `Before`/`After` phases (`io.rs::probe`), which is what criteria 1
  and 3 synchronise on; `project::faults` exposes it to other crates
  behind the `fault-injection` feature.
- Why `MoveFileExW` refuses: the non-POSIX rename supersedes the target
  by deleting it, and a file with open handles cannot be superseded that
  way; the POSIX-semantics rename (Windows 10 1607+, NTFS) unlinks the
  name and lets existing handles finish. The runners are Windows Server
  2022 on NTFS. ReFS and SMB shares without POSIX rename fall back to the
  error, which part 2's bound then handles as today.
- Estimate: M · 20 min · about 420 changed lines (storage 60, tests 200,
  ADR 20, this file and the QA report the rest).
