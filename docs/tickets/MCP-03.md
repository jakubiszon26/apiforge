# MCP-03: Run a simulation and check a proposed text from the server

- Size: S · Owner review: 15 min · Risk: none
- State: Backlog (issue #62) · after MCP-01
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

The model tries its work before anyone saves it: `check_text` says what a
proposed text would be wrong about in this project, and `run` simulates a
saved scenario and reports every step and the verdict — without sending a
request and without writing a file.

## What the owner will see

- New: two tools in `/mcp`: `check_text` and `run`. Nothing on screen.
- Changed, removed, migrated, defaults: nothing. `run` is a simulation only;
  `live` is refused until MCP-06 and its decision.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `check_text(path, text)` returns the diagnostics the text would have as the
   named document, with line and column as in `read_source`, and `valid`;
   the file on disk and `.apiforge/local` are unchanged afterwards (a
   read-only session edits in memory and is dropped).
2. `check_text` on a text that calls an unpublished revision, an unknown
   handle or a missing environment variable names each in a diagnostic, as
   the window does.
3. `run(path)` simulates the saved scenario and returns the status
   (`passed`/`failed`/…), one entry per step with its handle, label, status,
   message and, for a failed comparison, expected and actual; a component's
   inner steps are nested as in the CLI's output. Secrets are redacted as in
   the window. On the demo shop the result is `passed` with 18 steps.
4. A compact result is the default (status, steps, messages, expected and
   actual on failure); `full: true` returns the runner's complete JSON report;
   a test asserts the compact result of the demo shop is under 10 000
   characters.
5. `run` on a document with errors, on a component, or on a path that is not
   a scenario is a tool error naming the reason (the saved source has no
   valid definition; a component cannot run alone yet, see KTLO-23a).
6. `run(path, live: true)` is a tool error "live requests are not enabled on
   this server" and sends nothing (a test runs it with the demo API listening
   and asserts the API saw no request).
7. A run that does not finish within the CLI's deadline is cancelled and
   reported as a tool error; the server stays up.
8. The verification commands of the development guide pass.

## Not in scope

Live requests and secrets (MCP-06). Running a component on its own (waits
for KTLO-23a's runner target; a `component` run joins the tool then). Running
up to a step or a single step (the folder window has neither). Any write.

## Technical appendix

- `check_text`: `open_saved_read_only` → `edit_source(id, text,
  Origin::SESSION)` → `settle(id)` → `document.diagnostics()`;
  `edit_source` has no read-only check (`session.rs:441`) and the session is
  dropped afterwards, so nothing reaches disk.
- `run`: the CLI's `folder.rs::execute_run` without the `live` branch:
  `current_snapshot`, `RunRequest` in `RunMode::Simulation`, `RunnerService`
  polled as `run.rs::execute_request` does; the compact shape is
  `run.rs::print_steps` as JSON. Share the code with the CLI rather than copy
  it (move `execute_request` and `secrets` into `apiforge-mcp` or a small
  `apiforge-cli` library target).
- Tests in `crates/apiforge-mcp/tests/acceptance.rs` on the demo shop; the
  "no request sent" assertion uses `apiforge_demo_api::spawn` and a counting
  wrapper, or asserts on the demo API's in-memory cart count.
