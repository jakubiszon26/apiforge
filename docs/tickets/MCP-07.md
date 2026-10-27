# MCP-07: The lock names its holder; the window says who and takes the project back when it is free

- Size: S · Owner review: 20 min · Risk: `visible UI`, `default`
- State: Merged in #106 on 2026-10-27 (issue #66) · MCP-00 decision 6 = A taken
  2026-10-27 · mock-up M1 approved 2026-10-27 in round 4 (the bottom status bar
  with the *Take back* button of MCP-08b) · revised 2026-10-27 (MCP-00 revision
  2: the holder description gains the endpoint field; the take-back happens only
  when the lock is free, never from a living process; the observer behaviour
  moved to MCP-08a)
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

When the project is held by something that is not another window — the MCP
server working for an agent, a live run, `apiforge apply` — the window says
so, and as soon as the holder lets go the window takes the project back and
can save again, without being reopened. It never takes the project from a
holder that is still there.

## What the owner will see

- New: a status bar at the bottom of the window (36 px, only while the
  window is not the writer) names the holder and when it took the project:
  *Claude Code via apiforge mcp · since 10:42 · editing is paused here; this
  window follows every change*. For another window: *Another API Forge
  window · since 10:40 · …*; for the command line: *apiforge apply · since
  10:41 · …*. The header badge reads *Observing*. The notice under the tabs
  that says *Another window is editing this project* goes. An older build
  that wrote no holder file gets a generic *Another process* wording
  in the bar. Mock-up M1, round 4, approved 2026-10-27.
- Changed: a read-only window checks once a second (with the existing
  external check) whether the holder is gone and, when it is, takes the lock,
  drops the badge and the bar, and enables *Save*, *Save all* and
  *Publish*. If the holder left an activated transaction, the window's
  recovery view opens as at start; if it left an interrupted live run, the
  journal notice appears as at start.
- Removed, hidden, migrated: nothing.
- Default behaviour that changes: a read-only window becomes writable by
  itself instead of staying read-only until reopened (the `default` flag).

## Mock-up

M1 (MCP-00), round 4, approved 2026-10-27:
`docs/qa/2026-10-27/MCP-00/m1-*.png` — the header badge and the bottom bar
at 960 × 540 and 1600 × 1000 for the agent, another window and `apply`.

## Decisions for the owner

1. MCP-00 decision 6 (the wording and the automatic take-back): A, taken
   2026-10-27 on #59.
2. The mock-up M1 as a whole.

## Acceptance criteria

1. `ProjectStore::open_exclusive` writes the holder's description into
   `.apiforge/local/holder.json`, replaced atomically after the lock is
   confirmed — program, role (`window`, `agent`, `command`), start time,
   optional client name, optional `endpoint`; never a path or a secret — and
   `ProjectStore::holder()` returns it from a read-only store **only while
   the probe says a writer is live**; a holder file with no live writer is
   ignored. The lock file itself stays empty (on Windows the lock is
   mandatory and the file cannot be read while held) and is still never
   unlinked or replaced (KTLO-21's tests pass unchanged).
2. A window opened while a process holds the lock shows the bar with that
   process's description; while the lock is held by an older build that
   wrote no holder file, or the holder file is unreadable, it shows the
   generic wording.
3. When the holder releases the lock, the read-only window becomes the writer
   within two seconds: the badge and the bar disappear, *Save* is enabled,
   and a save succeeds (headless interaction test: take the lock in the test,
   open the window, release, settle, assert through named widgets).
4. The take-back never steals: while the holder still holds the lock, the
   probe says so and the window stays read-only for as long as that is true
   — a test holds the lock for longer than any poll interval and asserts no
   change; the window never replaces the lock file; no timeout exists.
5. A holder that left an activated transaction: the window, on take-back,
   opens the recovery view it opens at start; a holder that left an
   interrupted live run: the journal notice appears, as at start.
6. At 960 × 540 the bar and the badge stay inside the window
   (`assert_within_window`) in Code, Diagram and Split; the Problems panel
   and the diagram's zoom bar sit above the bar without overlap.
7. The QA report compares the implementation against M1; the verification
   commands of the development guide pass.

## Not in scope

Pausing edits while read-only and the reload that follows commits (MCP-08a);
the hand-over on request and the control channel (MCP-08b) — this ticket
only reserves the `endpoint` field; a status line for what the agent does
(MCP-10); naming the conversation (the holder file holds the program's
name, the client's name and the time, nothing about conversations).

## Technical appendix

- `transaction.rs::open_exclusive` (`:152`): after `take_confirmed_lock`,
  write one JSON line
  `{"version":1,"program":"apiforge","role":"agent","client":"Claude Code","since":"2026-10-27T10:42:11Z","endpoint":null}`
  to `.apiforge/local/holder.json` with `write_atomic`. Why not the lock
  file: `std::fs::File::try_lock` is `LockFileEx` on Windows, a mandatory
  whole-file lock, so a reader gets `ERROR_LOCK_VIOLATION` exactly while a
  writer is live (planning decision of 2026-10-27 on #66, option A).
  `has_live_writer` (`:896`) gains
  `holder() -> Result<Option<Holder>>` that returns the description when a
  writer is live. `.apiforge/local` is not tracked by Git, so this is not a
  `format` change. The window writes `role: "window"`, the CLI `command` with
  the subcommand's name, the server `agent` with the client's name from
  `initialize`/`server/discover` when it has one.
- Desktop: `folder/mod.rs:455` sets the notice once; it goes, and a
  `TopBottomPanel::bottom` of 36 px in `chrome.rs` renders the holder from
  the description. The take-back: on each watcher result in a
  read-only session, `has_live_writer()`; when `false`, reopen the saved
  session as the writer (`ProjectSession::open` with the unsaved text carried
  over as the recovery path does), clear the bar and `read_only` in
  `chrome.rs`, then run the start-time checks (recovery required, journal).
- Tests: `folder/acceptance_tests.rs` with the lock taken by the test process
  (`ProjectStore::open_exclusive` on the same folder, dropped mid-test);
  `storage/project/tests.rs` for the description.
