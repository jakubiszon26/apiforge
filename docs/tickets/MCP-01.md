# MCP-01: `apiforge mcp` answers Claude Code — overview, read, check, print

- Size: M · Owner review: 20 min · Risk: none
- State: Backlog (issue #60) · MCP-00 decisions 8 = A and 9 = B taken
  2026-10-27
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

In a project folder, `claude mcp add apiforge -- apiforge mcp` is enough for a
model to see the project the way the window does: what it contains, the text
of every scenario and component, what is wrong with it (with line and column),
and what its canonical form is. The model reads; nothing is written.

## What the owner will see

- New: nothing on screen. A new command, `apiforge mcp [--folder <dir>]`, and
  four tools in Claude Code's `/mcp` list: `project_overview`, `read_source`,
  `check`, `print`. A section *MCP server* in the development guide with the
  registration command and the `.mcp.json` snippet.
- Changed, removed, migrated: nothing. The window, the other CLI commands and
  the project on disk are untouched; a read creates no `.apiforge/local`.
- Default behaviour that changes: `apiforge mcp` without `--folder` uses the
  current directory (MCP-00 decision 8, A).

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `apiforge mcp --folder <dir>` speaks MCP over stdio: a client that sends
   `server/discover` (revision `2026-07-28`) or `initialize` (revision
   `2025-11-25`) gets the server's name, version and the `tools` capability;
   `tools/list` returns exactly `project_overview`, `read_source`, `check`,
   `print`, each with a description, an input schema and
   `readOnlyHint: true`, in a fixed order.
2. Nothing but protocol messages is written to stdout; diagnostics and logs
   go to stderr (a test captures both streams over a whole session).
3. `project_overview` on the demo shop returns the project name, base URL,
   environment variable names, secret names only, the two contracts, the four
   documents with kind, registered path, handle and base revision, the three
   published revisions, the server's crate version and the reference version
   (empty until MCP-02 lands).
4. `read_source` with a registered path returns the saved text, `valid: true`
   or `false`, and the diagnostics, each with `line`, `column`, `end_line`,
   `end_column` (1-based), `severity`, `message` and the fix's replacement
   when there is one; an unknown path is a tool error (`isError: true`) that
   names the registered paths.
5. `check` without a path reports every document; the two warnings of the
   demo shop come back with their file, line and column; with a path, only
   that document. A project whose saved source has errors is reported, never
   refused.
6. `print` returns the canonical text, byte for byte what `apiforge print`
   prints; for a document without a valid definition it is a tool error with
   the diagnostics.
7. The server opens the project read-only: after a session of all four tools
   on a clean checkout (no `.apiforge/local`), the folder is byte-identical
   and `.apiforge/local` does not exist.
8. Without `--folder` the server uses the current directory; outside a
   project every tool answers with the `NotAProject` message and the server
   stays up.
9. `claude mcp add apiforge -- <path>/apiforge mcp` registered in the demo
   shop folder shows the four tools in `/mcp` (manual QA step, recorded in the
   report with the Claude Code version).
10. The verification commands of the development guide pass; the
    development guide gains the *MCP server* section.

## Not in scope

Running a scenario and checking an unsaved text (MCP-03); the language
reference and resources (MCP-02); every write (MCP-04 to MCP-06); the window
(MCP-07); Streamable HTTP; the legacy JSON workspace (`--workspace`), which
the server does not serve.

## Technical appendix

- New library crate `crates/apiforge-mcp`: `tools.rs` with one plain function
  per tool over `ProjectSession` (`open_saved_read_only`, `documents()`,
  `document().text()/diagnostics()/analysis()`, `print_folder_document`),
  returning `serde_json::Value`, plus `lines.rs` (byte offset → line/column
  over the document text, UTF-8 aware). `server.rs` wires them to `rmcp`
  (`=3.8.0`, features `server`, `transport-io`, `macros`) with `#[tool_router]`;
  `structuredContent` carries the JSON, `content` its pretty text. Each call
  opens a fresh read-only session: the project is small, the stable read
  guards against a transaction in progress, and no session outlives a call.
- `crates/apiforge-cli/src/main.rs`: a `mcp` arm before the `fmt` fallback,
  `mcp.rs` parsing `--folder`; the tokio runtime starts only in this arm, so
  the other commands stay synchronous.
- Tests: `crates/apiforge-mcp/tests/acceptance.rs` calls the tool functions
  on `prepare_project(root, &demos::shop_project())` (criteria 3–8); one
  stdio test spawns the binary (`CARGO_BIN_EXE_apiforge`), writes the JSON
  lines of `server/discover`, `tools/list` and one `tools/call`, and asserts
  stdout holds nothing else (criteria 1, 2).
- Reuse: `folder.rs::select` and `check_document` in the CLI become
  `pub(crate)` helpers or move to the new crate; `run.rs::diagnostics` stays
  for the CLI's stderr format. `Cargo.lock` grows by `rmcp`, `rmcp-macros`,
  `schemars` and their tree; lock files are not counted in the size.
- Open question: whether Claude Code at the owner's version sends
  `initialize` or `server/discover`; `rmcp` accepts both, criterion 1 tests
  both.
