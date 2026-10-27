# MCP-08b: Hand-over on request — the window gives the project to the agent and takes it back

- Size: M · Owner review: 30 min · Risk: `visible UI`, `default`
- State: Backlog (issue #91) · MCP-00 decisions 10 and 11 = A taken
  2026-10-27 · waits for mock-up M1 · after MCP-08a and MCP-04
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

The person keeps the window open; when the agent wants to edit, the window
hands the project over and watches; when the agent is done, or the person
wants it back, the window is theirs again. No closing, no reopening, no
file edited by hand.

## What the owner will see

- New: when an agent asks and nothing is unsaved, the window becomes the
  observer at once and the notice says who has the project; a *Take back*
  button in the notice; the agent's `saved` makes the window follow at once
  rather than at the next poll; when the agent finishes, the window is
  writable within two seconds. With unsaved work the window refuses: *An
  agent asked to edit this project; save your work first* in the window and
  the matching refusal in the agent's result (MCP-08c turns this into a
  prompt).
- Changed: nothing else on screen.
- Removed, hidden, migrated: nothing.
- Default behaviour that changes: a clean window hands the project over
  without asking (decision 11 A).

## Mock-up

M1 (the notice with *Take back*, shared with MCP-07, MCP-08a).

## Acceptance criteria

1. The holder listens: a window that holds the project writes an `endpoint`
   into its lock description and accepts connections there; so does the
   server during an agent session; a reader finds the endpoint through
   `ProjectStore::holder()`.
2. Window first: with the window open and clean, the server's first
   `write_source` asks over the channel; the window releases the lock, shows
   the observer notice naming the agent, and the write proceeds — the tool
   result has `wrote: true` and the window shows the change within two
   seconds, by the `saved` message (a test asserts the rescan happened before
   the next poll would have).
3. Window first, unsaved work: the write is refused with `handover_refused`
   and the reason *the window has unsaved work*; the window shows the
   matching notice; nothing is written; the person's text is untouched.
4. Server first: a window opened while the agent holds the project reads the
   holder, connects, and is an observer from the start; the agent's later
   commits appear within two seconds.
5. *Take back* while the agent is idle: the server finishes nothing, releases,
   answers `released`; the window is writable within two seconds; the
   agent's next write is `held_by_window` and asks again, which a clean
   window grants again.
6. *Take back* during an operation: a save in flight commits or fails whole
   before the release; a run in flight finishes, cleanup included, the
   window shows *waiting for the agent's run to finish*, then takes the
   project back.
7. The agent finishes (`finish_editing`) or its client disconnects (EOF): the
   window is writable within two seconds; the server process is gone after
   EOF.
8. The server dies (killed in the test) with an activated transaction: the
   window takes the project back and opens the recovery view (MCP-07
   criterion 5).
9. Two agents: the second server's write is refused with the first's
   description; the window still observes the first.
10. The window closes while observing: the server keeps the lock and its next
    write succeeds; the channel's loss is logged on stderr only.
11. A pause of longer than ten poll intervals between two agent writes
    changes nothing: the window stays the observer and the second write
    succeeds (no take-back by time).
12. The channel is same-user and carries no path or secret: the socket is
    under the runtime directory with a random name (the pipe
    `\\.\pipe\apiforge-<random>`); a malformed or unknown message is ignored
    and logged. Windows evidence for the pipe is recorded in the QA report
    or its absence is stated.
13. The QA report compares the notice against M1; the verification commands
    of the development guide pass.

## Not in scope

The prompt with unsaved work (MCP-08c); cancelling the agent's run from the
window; a hand-over between two agents; the live permission message
(MCP-09; this ticket leaves the message kind defined and ignored); run
status (MCP-10).

## Technical appendix

- Channel: newline-delimited JSON with `version: 1` over
  `tokio::net::UnixListener` / `named_pipe` (tokio is in the workspace with
  `net`); the window runs the listener and the client on a background thread
  with a current-thread runtime and feeds results to the UI through the
  watcher's channel pattern; the server runs it on its tokio runtime.
  Messages: `request { client, pid }`, `granted`, `refused { reason }`,
  `saved { generation, paths }`, `run { path, mode, state }`, `released`,
  `permission { live }`, `take_back`.
- Hand-over in the window: on `request`, `has_unsaved_work()` false → drop
  the writer store (reopen the saved session read-only with the same
  navigation, the inverse of MCP-07's take-back), answer `granted`; true →
  `refused`. The server then calls `open_exclusive`; a `Locked` there (a
  third process) is reported as `held_by_other`.
- Take-back: `take_back` → the server sets a flag; after the operation in
  flight it drops the store and answers `released`; the window's probe then
  succeeds as in MCP-07.
- The server's `request` waits at most 30 s for an answer; without one it
  returns `handover_pending`; the window keeps the request and answers the
  next one at once.
- Tests: an interaction test in `folder/acceptance_tests.rs` that spawns the
  server binary on the harness's project and drives it over stdio (the
  stdio helper of MCP-01's tests, shared through `apiforge-mcp`'s test
  support); the kill test and the pause test use the same spawn.
