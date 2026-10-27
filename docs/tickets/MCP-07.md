# MCP-07: The window names who holds the project and becomes writable when they leave

- Size: S · Owner review: 20 min · Risk: `visible UI`, `default`
- State: Backlog (issue #66) · MCP-00 decision 6 = A taken 2026-10-27 ·
  waits for a mock-up (annotated capture) approved on the issue
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

When the project is held by something that is not another window — the MCP
server saving a file, a live run, `apiforge apply` — the window says so, and
as soon as the holder lets go the window takes the project back and can save
again, without being reopened.

## What the owner will see

- New: the read-only notice names the holder and when it took the project:
  *The API Forge MCP server (started 10:42) is editing this project. This
  window is read-only until it finishes.* For another window: *Another API
  Forge window (opened 10:40)…*; for the command line: *apiforge apply…*. The
  holder's description comes from the lock file; an older build that wrote
  nothing there gets today's wording.
- Changed: a read-only window checks once a second (with the existing
  external check) whether the writer is gone and, when it is, takes the lock,
  drops the *Read-only* badge and the notice, and enables *Save*, *Save all*
  and *Publish*. Unsaved work in a read-only window (today: nothing can be
  saved) becomes saveable at that moment.
- Removed, hidden, migrated: nothing.
- Default behaviour that changes: a read-only window becomes writable by
  itself instead of staying read-only until reopened (the `default` flag).

## Mock-up

Required before approval: an annotated capture of the folder window's header
and notice at 960 × 540 and 1600 × 1000 with the new wording for the MCP
server case, from a throwaway `review_*` test on a `mock/mcp-07-*` branch,
posted on the issue.

## Decisions for the owner (taken 2026-10-27: 1 = A; 2 waits for the mock-up)

1. MCP-00 decision 6 (the wording and the automatic take-back): A
   **(taken)** on #59.
2. The mock-up as a whole.

## Acceptance criteria

1. `ProjectStore::open_exclusive` writes the holder's description into
   `.apiforge/local/project.lock` (program, role, start time; never a path or
   a secret) and `ProjectStore::holder_description()` reads it from a
   read-only store; the file is still never unlinked or replaced (KTLO-21's
   tests pass unchanged).
2. A window opened while a process holds the lock shows the notice with that
   process's description; while a lock file from an older build (empty) is
   held, it shows today's wording.
3. When the holder releases the lock, the read-only window becomes the writer
   within two seconds: the badge and the notice disappear, *Save* is enabled,
   and a save succeeds (headless interaction test: take the lock in the test,
   open the window, release, settle, assert through named widgets).
4. The take-back never steals: while the holder still holds the lock, the
   probe (`has_live_writer`, KTLO-21) says so and the window stays read-only;
   the window never replaces the lock file.
5. Unsaved work made in the read-only window survives the take-back and is
   saved by the next *Save*.
6. At 960 × 540 the longer notice stays inside the window
   (`assert_within_window`) in Code, Diagram and Split.
7. The QA report compares the implementation against the approved mock-up;
   the verification commands of the development guide pass.

## Not in scope

A hand-over the other way (the window releasing the lock on a request from
the server); a status line that shows what the model is doing; naming the
model or the Claude Code session (the lock holds the program's name and
time, nothing about conversations).

## Technical appendix

- `transaction.rs::open_exclusive` (`:152`): after `take_confirmed_lock`,
  truncate and write a one-line JSON description (`{"program":"apiforge
  mcp","since":"2026-10-27T10:42:11Z"}`); the file is already opened
  read-write. `has_live_writer` (`:896`) gains a sibling that returns the
  description when a writer is live. `.apiforge/local` is not tracked by Git,
  so this is not a `format` change.
- Desktop: `folder/mod.rs:430` sets the notice once; move the text to a
  function of the description. The take-back: on each watcher result in a
  read-only session, `has_live_writer()`; when `false`, replace
  `self.session`'s store with `ProjectStore::open_exclusive` (a reopen of the
  saved session through `ProjectSession::open` with unsaved text carried over
  — the recovery path already carries drafts), then clear the notice and
  `read_only` in `chrome.rs:12,474`.
- Tests: `folder/acceptance_tests.rs` with the lock taken by the test
  process (`ProjectStore::open_exclusive` on the same folder, dropped
  mid-test); `storage/project/tests.rs` for the description.
- Size: storage 60, desktop 90, tests 120, captures and ticket 30 — S.
