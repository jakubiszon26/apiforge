# MCP-11: The whole workflow, end to end — the sprint's acceptance

- Size: S · Owner review: 30 min (the owner watches the run) · Risk: none
- State: Backlog · after every other MCP ticket (its test is written first
  and ignored with this ticket's name until the parts land)
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md#the-sprints-key-acceptance-criterion)

## Goal

The sprint's key criterion, proven: the window is open, the agent gets to
work, creates a component, publishes it, creates a scenario that calls it,
fixes it twice and simulates — and the person sees every committed change in
the same window, with nothing closed, reloaded by hand or edited by hand,
and with pauses between the agent's saves longer than any interval the
window polls at.

## What the owner will see

- New: nothing on screen. One interaction test and one QA session with a
  real client that exercise everything the sprint built.
- Changed, removed, migrated, defaults: nothing.

## Mock-up

Not applicable.

## Acceptance criteria

1. The interaction test (headless window plus the server binary over stdio):
   the window is open on the demo shop; the agent writes a component through
   `create_document`, publishes it, creates a scenario calling
   `handle@1(…)`, gets an error from `check_text`, fixes it with
   `write_source` twice (the second after a `stale_source` provoked by a
   second read), runs a simulation; between each write the test waits more
   than ten watcher polls; after each commit the window shows the new state
   within two seconds (explorer, code, diagram identities, Problems, the
   revision); the window never reloaded by hand, no file was edited outside
   the server, the person's active tab survived.
2. Separate cases, each a test: unsaved work in the window at the request
   (refusal or prompt as the owner decided); an invalid proposal
   (`wrote: false`, nothing on disk); a stale write; the client lost (EOF)
   mid-session and the window writable within two seconds; the server killed
   with a transaction activated and the window's recovery view; *Take back*
   while idle; `finish_editing`.
3. The QA session: the owner's build on Linux, the real client (`claude
   --version` recorded), the exact prompts given, every tool call the client
   made and its result, captures of the window after each step at 1600 ×
   1000, compared against M1–M4 where they apply; the Windows section states
   what ran on Windows (the CI captures and the owner's local build) and what
   did not, item by item.
4. The regression smoke list runs on the release binary and is recorded.
5. The verification commands of the development guide pass.

## Not in scope

New behaviour. Any finding becomes a ticket, not a fix in this pull request,
unless it is a one-line defect in the tests themselves.

## Technical appendix

- `crates/apiforge-desktop/src/folder/mcp_workflow_tests.rs`, written in its
  own commit at the start of the sprint and `#[ignore = "MCP-11: waits for
  MCP-08b"]` until the parts have merged; the server is spawned with
  `CARGO_BIN_EXE_apiforge` and driven with the stdio helper of
  `apiforge-mcp`'s tests.
- The QA report at `docs/qa/<date>/MCP-11/report.md` is the evidence named by
  the sprint's exit criterion.
