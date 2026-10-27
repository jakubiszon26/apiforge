# MCP-06: Live requests from the server — the four ends of a run

- Size: M · Owner review: 25 min · Risk: none on screen; real requests leave
  the machine when the person allows it (decision 4, revised by the toggle
  of MCP-09)
- State: Backlog (issue #65) · MCP-00 decision 4 = A taken 2026-10-27 ·
  after MCP-04 · revised 2026-10-27 (MCP-00 revision 2: the run's four ends,
  the journal, lost responses, the permission slot that MCP-09 fills)
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

When the person allows it, the model can run a scenario against the real
API: `run(path, live: true)` sends the requests, records each one in the
project's run journal before it goes out, as the window does, and reports
the result — and says exactly what happened when the run was cancelled,
timed out, lost its client or lost its process.

## What the owner will see

- New: the start flag `apiforge mcp --allow-live [--secrets <file>]`, set by
  the person in `.mcp.json` or on `claude mcp add`; the `live` argument of
  `run` works only when live is allowed. Secrets come from
  `APIFORGE_SECRET_<NAME>` in the server's environment or the `--secrets`
  file, as for `apiforge run`. An interrupted live run is reported by the
  window at the next start, as today, and by the server's next `run`.
- Changed, removed, migrated: nothing.
- Default behaviour that changes: none; live stays off unless the flag is
  given, and the model cannot give it. (MCP-09 adds the window's toggle,
  which decides whenever a window is connected.)

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. Without `--allow-live`, `run(path, live: true)` is `live_disabled` and
   sends nothing; `project_overview.live_requests` is
   `{ allowed: false, source: "no --allow-live" }`.
2. With `--allow-live`, `run(path, live: true)` on the demo shop against
   `apiforge-demo-api` sends the requests, passes, and the report's values
   are redacted exactly as the window's (a scenario with a declared secret
   shows `[REDACTED]`); the result carries `generation`, `source_sha256`,
   `redacted: true`.
3. A live run needs the agent session (the journal is a project-local write):
   it takes or uses the session's lock and is refused with the holder while
   a window or another process holds the project; a simulation in the same
   situation still works.
4. The permission slot: the server holds a current permission
   (`flag` or `window { live }`); the effective rule is *window decides while
   a window is connected, the flag otherwise*; a test sets the slot through
   the test seam MCP-09 will wire and asserts both `live_disabled` with the
   message *live requests are off in this project's settings* and an allowed
   run without the flag.
5. The permission is checked immediately before the live run starts, under
   the lock; a change after the start does not stop the run (decision 16 A).
6. Cancelled: a `notifications/cancelled` during a live run against a slow
   endpoint ends the main phase at the next step, cleanup runs and its steps
   appear in the report, the journal entry is removed at the end, the result
   is `cancelled` with the in-flight request marked *may have reached the
   API*; the same for the ten-minute deadline with `ended_by: timed_out`.
7. Client disconnected: EOF on stdin during a live run cancels it, waits up to
   ten seconds for cleanup, releases the lock and exits; with cleanup finished
   in time the journal is empty afterwards; with cleanup blocked (test seam)
   the entry stays and the next `run` reports it under `interrupted`.
8. Process killed: a server killed mid-run (a spawned child in the test)
   leaves the journal entry; the next `run` (live or not) of a new server
   returns it under `interrupted` with method, URL and state, as the CLI
   prints them, and `project_overview` says `interrupted_runs: n`.
9. `request_id`: a second live `run` with a `request_id` the server has
   answered returns the recorded result and sends nothing; a new server
   answers `unknown_outcome` for an id it never saw, with the journal's
   interrupted entries if any.
10. No tool accepts a secret value as an argument and no result contains one:
    a test sets `APIFORGE_SECRET_TOKEN`, runs every tool of a session (reads,
    checks, a run, a refused write with the value inside `text`) and greps
    every result, every stderr line and the journal for the value.
11. A saved source that contains a known secret value is withheld by
    `read_source`, `print` and the resource with `secret_in_source` and the
    line, never a modified text.
12. The development guide's *MCP server* section explains the flag, the
    environment, the journal and the four ends in one paragraph each.
13. The verification commands of the development guide pass.

## Not in scope

The window's toggle and the `permission` message (MCP-09; this ticket leaves
the slot and its test seam). A per-call consent prompt inside the model's
session. Choosing the base URL per run. Cancelling a run from the window.

## Technical appendix

- The CLI's live branch (`folder.rs::execute_run`: `open_saved_exclusive`,
  `set_known_secrets`, `JournalStore::open_project`, `RunMode::Live`,
  `report_interrupted`) runs through the shared executor of MCP-03 with
  `live: bool`, on the agent session's store.
- Permission slot: `enum Permission { Flag(bool), Window { live: bool } }`
  behind a `watch` channel in the server state; MCP-09 writes `Window` from
  the control channel; EOF of the channel resets to `Flag`.
- Secrets: `run.rs::secrets(path)` as is; the `--secrets` path is reread at
  each call and `set_known_secrets` refreshed.
- `secret_in_source`: a scan of the saved bytes for every known value before
  a text leaves `read_source`, `print` or `resources/read`.
- Shutdown on EOF: `RunnerService::shutdown(Duration::from_secs(10))`, then
  drop the store.
- Tool annotation `openWorldHint: true` on `run`; the description says so in
  its first sentence.
- Tests: `apiforge_demo_api::spawn` in-process; the interrupted-run tests use
  a listener that accepts and never answers; the kill test spawns the server
  binary and kills it with the run in flight.
