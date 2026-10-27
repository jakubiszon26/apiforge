# MCP-01: `apiforge mcp` answers Claude Code — overview, read, check, print

- Size: M · Owner review: 20 min · Risk: `default`
- State: Approved; `ready:sol` 2026-10-27 (issue #60) · MCP-00 decisions 8 = A and 9 = B taken
  2026-10-27 · revised 2026-10-27 (MCP-00 revision 2: versions, contract,
  demo without warnings)
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

In a project folder, `claude mcp add apiforge -- apiforge mcp` is enough for a
model to see the project the way the window does: what it contains, the text
of every scenario and component, what is wrong with it (with line and column),
what its canonical form is — and which version of each it is looking at. The
model reads; nothing is written.

## What the owner will see

- New: nothing on screen. A new command, `apiforge mcp [--folder <dir>]`, and
  four tools in Claude Code's `/mcp` list: `project_overview`, `read_source`,
  `check`, `print`. A section *MCP server* in the development guide with the
  registration command, the `.mcp.json` snippet and the contract (error
  codes, positions, limits).
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
   `print`, each with a description, an input schema, an output schema and
   `readOnlyHint: true`, in a fixed order.
2. Nothing but protocol messages is written to stdout over a whole session
   that includes a malformed line, an unknown method and a tool error;
   diagnostics and logs go to stderr; EOF on stdin ends the process within
   two seconds with exit code 0 (a test spawns the binary and drives both
   streams).
3. `project_overview` on the demo shop returns the project name, base URL,
   environment variable names, secret names only, the two contracts, the four
   documents with kind, registered path, handle, base revision and
   `source_sha256`, the three published revisions each with `source_sha256`,
   the project `generation`, `holder` (`none` on a clean checkout),
   `live_requests` (`allowed: false`, `source: "no --allow-live"`), the
   server's crate version and the reference version (empty until MCP-03).
4. `read_source` with a registered path returns the saved text,
   `source_sha256`, `generation`, `valid`, and the diagnostics, each with
   `line`, `column`, `end_line`, `end_column` (1-based, counted in Unicode
   scalar values, end exclusive, a `\r` before `\n` part of the break),
   `severity`, `message` and the fix's replacement when there is one. A test
   uses a source with multi-byte characters and one with CRLF line ends. An
   unknown path is a tool error `unknown_document` that names the registered
   paths.
5. `check` without a path reports every document; on a fixture whose
   `components/product.afs` carries the injected type mismatch of KTLO-25's
   `75c55f5` shape, the warning comes back with its file, line and column;
   with a path, only that document. A project whose saved source has errors
   is reported, never refused. Beyond 1 000 diagnostics per document the
   result says `omitted: n`; nothing is cut silently.
6. `print` returns the canonical text, byte for byte what `apiforge print`
   prints; for a document without a valid definition it is a tool error
   `invalid_text` with the diagnostics.
7. The server opens the project read-only: after a session of all four tools
   on a clean checkout (no `.apiforge/local`), the folder is byte-identical
   and `.apiforge/local` does not exist.
8. Without `--folder` the server uses the current directory; outside a
   project every tool answers `not_a_project` and the server stays up.
9. Every tool error is `isError: true` with `structuredContent`
   `{ code, message, wrote: false, details }`; the codes used by these four
   tools are from the fixed list in the development guide; a path argument
   that fails `ProjectPath::working_source` (`../x.afs`, an absolute path,
   `.apiforge/diagram.json`, a Windows reserved name) is `path_rejected`.
10. `claude mcp add apiforge -- <path>/apiforge mcp` registered in the demo
    shop folder shows the four tools in `/mcp`, and a prompt that reads a
    source through the server gets its text: a manual QA step recorded with
    `claude --version` and the prompt used.
11. The verification commands of the development guide pass; the
    development guide gains the *MCP server* section.

## Not in scope

Running, checking a proposal, the reference and resources (MCP-03); every
write (MCP-04 to MCP-06); the window (MCP-07 to MCP-10); Streamable HTTP; the
legacy JSON workspace (`--workspace`), which the server does not serve.

## Technical appendix

- New library crate `crates/apiforge-mcp`: `tools.rs` with one plain function
  per tool over `ProjectSession` (`open_saved_read_only`, `documents()`,
  `document().text()/diagnostics()`, `print_folder_document`) returning
  typed results (`serde` + `schemars`), `positions.rs` (byte range →
  line/column in scalar values, CRLF-aware), `error.rs` with the code enum.
  `server.rs` wires them to `rmcp` (`=3.8.0`, features `server`,
  `transport-io`, `macros`) with `#[tool_router]`; `structuredContent` carries
  the JSON, `content` its pretty text. Each call opens a fresh read-only
  session: the stable read guards against a transaction in progress.
- `crates/apiforge-cli/src/main.rs`: a `mcp` arm before the `fmt` fallback,
  `mcp.rs` parsing `--folder`; the tokio runtime starts only in this arm.
- `generation` is the head generation the store reads (`head()`),
  `source_sha256` the `FileState::File { sha256 }` the session keeps.
- Tests: `crates/apiforge-mcp/tests/acceptance.rs` on
  `prepare_project(root, &demos::shop_project())` (criteria 3–9) and a
  fixture derived from it with the injected warning; one stdio test spawns
  `CARGO_BIN_EXE_apiforge` (criteria 1, 2).
- Reuse: `folder.rs::select` and `check_document` move to the new crate;
  `run.rs::diagnostics` stays for the CLI's stderr format.
