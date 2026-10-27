# MCP-06: Live requests from the server behind `--allow-live`

- Size: S · Owner review: 20 min · Risk: none on screen; real requests leave
  the machine when the person enables it (decision 4)
- State: Backlog (issue #65) · waits for the MCP-00 decision 4 · after
  MCP-03
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

When the person decides so for a project, the model can run a scenario
against the real API: `run(path, live: true)` sends the requests, records
them in the project's run journal before they go out, as the window does, and
reports the result — and refuses everything of the kind when the server was
not started with `--allow-live`.

## What the owner will see

- New: the start flag `apiforge mcp --allow-live [--secrets <file>]`, set by
  the person in `.mcp.json` (`"args": ["mcp", "--allow-live"]`) or on
  `claude mcp add`; the `live` argument of `run` works only then. Secrets
  come from `APIFORGE_SECRET_<NAME>` in the server's environment (`.mcp.json`
  `env` with `${VAR}` expansion) or the `--secrets` file, as for `apiforge
  run`. An interrupted live run is reported by the window at the next start,
  as today, and by the server's next `run`.
- Changed, removed, migrated: nothing.
- Default behaviour that changes: none; live stays off unless the flag is
  given, and the model cannot give it.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. Without `--allow-live`, `run(path, live: true)` is the tool error of
   MCP-03 criterion 6 and sends nothing; `project_overview` says
   `live_requests: "disabled"`.
2. With `--allow-live`, `run(path, live: true)` on the demo shop against
   `apiforge-demo-api` sends the requests, passes, and the report's values are
   redacted exactly as the window's (a scenario with a declared secret shows
   `[REDACTED]`).
3. A live run takes the writer lock for its duration (the journal is a local
   write) and is refused with the lock message while the window holds the
   project; a simulation in the same situation still works.
4. Journal: a live run cancelled half-way (the deadline test of MCP-03
   criterion 7 with a slow endpoint) leaves its entries, and the next `run`
   (live or not) returns them under `interrupted` with method, URL and state,
   as the CLI prints them.
5. No tool accepts a secret value as an argument and no result contains one:
   `project_overview` lists names; a test sets `APIFORGE_SECRET_TOKEN` and
   greps every tool result of a session for the value.
6. The development guide's *MCP server* section explains the flag, the
   environment and the journal in one paragraph.
7. The verification commands of the development guide pass.

## Not in scope

A per-call consent prompt inside the model's session (Claude Code already
asks before a tool call; the flag is the project-level consent). Enabling
live from a tool. Choosing the base URL per run (the project's environment
decides, as in the CLI).

## Technical appendix

- The CLI's live branch (`folder.rs::execute_run`: `open_saved_exclusive`,
  `set_known_secrets`, `JournalStore::open_project`, `RunMode::Live`,
  `report_interrupted`) moved to the shared run function of MCP-03 with a
  `live: bool` and the server's `allow_live` setting checked before it.
- Secrets: `run.rs::secrets(path)` as is; the `--secrets` path is read at each
  call so a changed file is picked up without restarting Claude Code.
- Tool annotation `openWorldHint: true` on `run` when live is enabled; the
  description says so in its first sentence.
- Tests: `apiforge_demo_api::spawn` in-process; the interrupted-run test uses
  a listener that accepts and never answers, with the run cancelled through
  the handle.
