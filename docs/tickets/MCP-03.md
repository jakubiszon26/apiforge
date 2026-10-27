# MCP-03: Run a simulation, check a proposal, serve the reference and resources

- Size: M · Owner review: 20 min · Risk: none
- State: Backlog (issue #62) · after MCP-01 and MCP-02 · revised 2026-10-27
  (MCP-00 revision 2: component runs, an executor that returns data,
  cancellation and timeout, the reference and resources from MCP-02)
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

The model tries its work before anyone saves it, and reads the language it
writes in: `check_text` says what a proposed text would be wrong about in
this project — for a document that exists or for one that does not yet —
`run` simulates a saved scenario or component and reports every step and the
verdict without sending a request or writing a file, and
`language_reference` and the resources hand the model the reference and the
sources.

## What the owner will see

- New: three tools in `/mcp`: `check_text`, `run`, `language_reference`;
  the resources `apiforge://reference` and `apiforge://source/<path>`
  (`@apiforge:apiforge://reference` in a prompt attaches it). Nothing on
  screen.
- Changed, removed, migrated, defaults: nothing. `run` is a simulation only;
  `live` is refused until MCP-06 lands (decision 4 = A).

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `check_text(path, text)` on a registered path returns the diagnostics the
   text would have as that document, with positions as in `read_source`,
   `valid`, the `generation` evaluated at and the `source_sha256` of every
   document the analysis used; the file on disk and `.apiforge/local` are
   unchanged afterwards.
2. `check_text` on a path that is **not** registered checks the text as a new
   document: kind from the header, the handle it would get, and the errors a
   registration would meet — the path exists on disk or collides
   case-insensitively, the handle is taken, the header has no kind — plus
   the ordinary diagnostics; the result says `registered: false`.
3. `check_text` on a text that calls an unpublished revision, an unknown
   handle or a missing environment variable names each in a diagnostic, as
   the window does.
4. `run(path)` simulates the saved scenario and returns `status`
   (`passed`/`failed`/`cancelled`), one entry per step with handle, label,
   status, message and, for a failed comparison, expected and actual, nested
   for a component call as the CLI nests them; `generation` and the
   scenario's `source_sha256`; `redacted: true`. On the demo shop the result
   is `passed` with 18 steps. A `failed` verdict is a successful call, not a
   tool error.
5. `run(path, inputs)` on a saved component runs it alone with the named
   inputs (JSON values by input name, defaults filling the rest, as the Run
   inputs dialog does since KTLO-23b/27); a missing input without a default
   is a tool error `invalid_inputs` that lists the missing names; an unknown
   name is refused the same way.
6. A compact result is the default; `full: true` returns the runner's
   complete JSON report; a result over 2 MiB is `result_too_large` with the
   advice to use `full: false` or one path, never cut; a test asserts the
   compact demo result is under 10 000 characters.
7. `run` on a document with errors or on an unregistered path is a tool
   error naming the reason (`invalid_text` with the diagnostics,
   `unknown_document`).
8. `run(path, live: true)` is a tool error `live_disabled` with the message
   *live requests are not enabled on this server* and sends nothing (a test
   runs it with the demo API listening and asserts the API saw no request).
9. The shared executor returns data: the CLI's `run` prints exactly what it
   printed before from the returned report (a test compares its output on
   the demo shop with the recorded one), and no `println!` remains in the
   shared path.
10. Cancellation: a `notifications/cancelled` for a `run` in progress (a
    slow sample step in a test fixture) ends it with `status: cancelled`,
    the steps done so far, and cleanup run; the server answers
    `project_overview` while the run is in progress; a second `run` meanwhile
    is `run_in_progress`.
11. A run that does not finish within ten minutes (a test lowers the deadline
    through a test seam) ends as `cancelled` with `ended_by: timed_out`; the
    in-flight step is marked as possibly delivered; the server stays up.
12. `language_reference` returns the reference embedded with `include_str!`
    and its version; `project_overview` now reports that version;
    `resources/list` returns `apiforge://reference` and one
    `apiforge://source/<path>` per registered document; `resources/read`
    returns the reference or the saved text as `text/plain`; a URI whose
    decoded path fails validation (`%2e%2e`, `.apiforge/…`) is refused.
13. The verification commands of the development guide pass; the *MCP
    server* section documents `check_text` versus `run` in one sentence.

## Not in scope

Live requests and secrets (MCP-06). Running up to a step or a single step.
Any write. Run progress notifications to the client (the result is returned
at the end).

## Technical appendix

- `check_text` on a registered path: `open_saved_read_only` →
  `edit_source(id, text, Origin::SESSION)` → `settle(id)` →
  `document.diagnostics()`; on an unregistered path the new session method
  `check_new_document(kind, path, text)` of MCP-00 (introduced here, used by
  MCP-04's `create_document`).
- The executor: `run.rs::execute_request` and `secrets` move to a library
  target of `apiforge-cli` (or `apiforge-mcp`) as
  `execute(request, journal, cancel, deadline) -> RunOutcome { report,
  interrupted_before, ended_by }`; the CLI's `print_steps`/`print_report`
  consume it. The deadline is a parameter with the CLI's 600 s as default.
- `run` for a component: `prepare_run` accepts components since KTLO-23;
  `RunTarget::Component { revision: base, inputs }` with `inputs` converted
  from JSON by the same rules as the dialog (`run_inputs_prompt`).
- The server runs the executor on a blocking task and polls `RunHandle` with
  the cancellation token `rmcp` provides for the request; one `Mutex<Option<
  RunHandle>>` per server enforces one run at a time.
- Resources in `rmcp`: `ServerHandler::list_resources`/`read_resource`; the
  `resources` capability joins `tools`.
- Tests in `crates/apiforge-mcp/tests/acceptance.rs` on the demo shop; the
  "no request sent" assertion uses `apiforge_demo_api::spawn` and its
  in-memory request count; the slow-step fixture uses a listener that
  accepts and never answers.
