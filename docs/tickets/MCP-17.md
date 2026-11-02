# MCP-17: An error result carries nothing the tool's output schema rejects

- Size: S · Owner review: 10 min · Risk: MCP output compatibility (what a
  client reads on a refusal); no `format`, `migration`, `default`,
  `visible UI` or `removal`
- State: Implemented, awaiting acceptance (issue #188) · decision 1 taken
  2026-11-02: A, text only
- Analysis model: Claude Fable 5.1 · Implementation model: Claude Opus 5.5

## Goal

A model gets every refusal of the server — `held_by_window`, `stale_source`,
`live_disabled` and the rest — as feedback it can act on, through any MCP
client that validates tool results against the tool's `outputSchema`,
error results included.

## Cause

`crates/apiforge-mcp/src/server.rs::call` answers a `ToolError` with
`CallToolResult::structured_error(value)` (rmcp 3.8.0): `isError: true`,
`structuredContent` = `{ code, message, wrote, details }` and the same JSON
as text. Every tool advertises an `outputSchema` that describes its
successful result only (`schema_for_output::<T>()` for the eleven result
types), so the error object matches none of them. A client that validates
`structuredContent` against `outputSchema` whenever it is present rejects
every refusal the way #181 rejected every `run` result: the model receives
the client's own protocol error, not `code` and `message`.

### What the specification says

The MCP specification is identical on this point in 2025-06-18, 2025-11-25
and 2026-07-28 (the versions rmcp 3.8.0 negotiates, 2024-11-05 to
2026-07-28): *"If an output schema is provided: Servers MUST provide
structured results that conform to this schema. Clients SHOULD validate
structured results against this schema."* No exception is written for
`isError: true`. 2026-07-28 adds that `structuredContent` is *"any JSON value
… that conforms to the tool's `outputSchema` if one is defined"*. Every
example of a tool execution error in all three versions carries `content`
(a text block) and `isError: true` and **no** `structuredContent`.

### What the clients do

- The TypeScript SDK's `Client.callTool` (what Claude Code builds on). In
  1.x — verified at tags 1.20.0 and 1.25.0 and at commit 18c7e79 of
  2025-05-14 — the presence check skips error results (`if
  (!result.structuredContent && !result.isError) throw …`), but the
  validation branch is `if (result.structuredContent) { validate }` with
  only a comment saying "not when there's an error": an error result with
  a `structuredContent` that does not match throws `McpError(InvalidParams,
  "Structured content does not match the tool's output schema: …")`. The
  current main (`packages/client/src/client/client.ts`, 2.x) guards both
  branches with `!result.isError`.
- Claude Code 2.1.316, the build installed on the analysis machine, bundles
  both forms of that code: a `callTool` whose validation branch checks
  `!isError`, and a `callTool`/`callToolStream` pair whose branch is
  `if (s.structuredContent) try { validate }` without it. Which of the two
  handles its MCP tool calls could not be determined from outside the
  binary; the owner's finding in MCP-14 shows it validates successful
  results. The risk therefore cannot be excluded for the client in use.
- Codex: not verified.
- rmcp 3.8.0 validates nothing on the server side; `structured_error` only
  fills both fields.

## What the owner will see

- Changed (with decision 1 = A): a refusal is `isError: true` with one text
  block holding the same pretty JSON `{ code, message, wrote, details }` as
  today and **no** `structuredContent`. The model reads the same text as
  before. A client that read a refusal's `structuredContent` — in this
  repository the stdio harness `tool_error` and seven direct reads in
  `crates/apiforge-cli/tests/mcp.rs`, `apiforge_mcp::test_support::tool_error`
  and three probes in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`
  — reads the text instead.
- Nothing in the window changes. Removed, hidden, migrated, defaults: nothing.

## Decisions for the owner

1. **Where the error object travels on a refusal.**
   - **A (recommended): text only.** `isError: true`, `content[0].text` is the
     error JSON, no `structuredContent`. Matches every example of the
     specification; accepted by both forms of the SDK code (the presence
     check already skips errors). The model's text is unchanged; only a
     client that parsed `structuredContent` on errors must read the text.
   - **B: union schemas.** Keep `structuredContent` on errors and make every
     tool's `outputSchema` `anyOf: [<result>, <error object>]` (a
     `#[serde(untagged)]` wrapper over `T` and `ToolError`, which already
     derives `JsonSchema`). Valid for every client; costs eleven larger
     schemas that the model reads at every session, union types in any
     typed client, and the harness validates two arms.
   - **C: leave it.** Rely on the SDK's current guard. Rejected by the
     analysis: the installed client carries the older check and the
     specification writes no exception.

## Decision taken (2026-11-02, on #188)

1. **A, text only.** A refusal is `isError: true` with the same pretty JSON
   `{ code, message, wrote, details }` in `content[0].text` and no
   `structuredContent`. It matches every example of the specification and
   both forms of the SDK's client check, and the model reads the same text
   as before. (B, union output schemas, was declined: eleven larger schemas
   read at every session and union types in any typed client; C, leaving
   it, was declined: the installed client carries the older check.)

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

Written for A; B changes criterion 1 to "the error object validates against
the tool's `outputSchema`" and drops nothing else.

1. Every error result over stdio has no `structuredContent` key; its
   `content` is one text block whose JSON is an object with exactly the keys
   `code`, `message`, `wrote` and `details`, with the values the tools gave
   before. The harness `tool_error` of `crates/apiforge-cli/tests/mcp.rs`
   asserts it, so every refusal the existing stdio sessions exercise is
   covered (`path_rejected`, `unknown_document`, `invalid_text`,
   `live_disabled`, `held_by_window`, `stale_source`, `run_in_progress`,
   `comments_would_be_lost`, `secret_in_text`, `unknown_outcome`, …);
   `mcp17_criterion_1_a_refusal_travels_as_text` is one session that asserts
   the raw response of a `path_rejected` and a `live_disabled` refusal.
2. A client that validates every result carrying `structuredContent`
   against the tool's `outputSchema`, error results included, accepts every
   result of this server: `Server::response` validates whenever
   `structuredContent` is present (the `isError != true` condition is
   removed), in every stdio session; `mcp17_criterion_2_every_refusal_passes_a_validating_client`
   drives at least one refusal of every tool and asserts each was seen.
   (Implementation note: `language_reference` and `finish_editing` have no
   refusal — they answer every call with a result — so the test drives the
   nine tools that can refuse and asserts exactly those.)
3. Successful results are unchanged: `structuredContent` and the same JSON
   as text, validated against the `outputSchema` (the MCP-14 tests
   `mcp14_criterion_*` pass unchanged).
4. The fixed list of error codes, `wrote` (`false` or `"unknown"`) and the
   `details` of each code are unchanged; the contract bullet in
   `docs/development.md` ("A call that could not do what was asked …") says
   where the error object travels. Earlier QA reports and tickets are
   history and are not rewritten.
5. The JSON-RPC errors of `resources/read` (`server.rs::blocking`) are not
   touched.

## Not in scope

- `project_overview.live_requests` before a window connects: MCP-18 (#190).
- Validating the shape of `details` per error code: the codes' details are
  documented, not schematised.
- A separate MCP contract version: MCP-12 D1 (none today) stands.

## Technical appendix

- **Files.** `crates/apiforge-mcp/src/server.rs::call`: the error arm
  becomes `CallToolResult::error(vec![ContentBlock::text(text)])` (rmcp
  3.8.0 has it; about six lines). `crates/apiforge-cli/tests/mcp.rs`:
  `tool_error` reads `content[0].text`, `Server::response` drops the
  `isError` condition, the seven direct `["structuredContent"]["code" |
  "message" | "details"]` reads on refusals move to the parsed text (a
  helper `refusal(&response) -> Value`). `crates/apiforge-mcp/src/test_support.rs::tool_error`
  and the three desktop probes (`acceptance_tests.rs` lines near 2006, 2701,
  4389) likewise. `docs/development.md` lines 743–744.
- **Order with MCP-18.** The two tickets have different causes (the result
  envelope here; the overview's content there) and different code; they
  are not one fix. Both add to `crates/apiforge-cli/tests/mcp.rs`: MCP-17
  changes its shared helpers, MCP-18 adds tests, so MCP-17 merges first or
  MCP-18 rebases its tests on it.
- **Estimate.** S: no uncertainty about the cause, no state machine, one
  envelope function; regression potential is a client that parsed
  `structuredContent` on errors, which criterion 1 names and the harness
  covers. About 150 changed lines: 10 in the server, 100 in tests, 40 in
  documentation. Owner review 10 minutes (an internal change with a report).
- **`ready:sol`.** Once decision 1 is taken, every item of the autonomy
  checklist holds: the files are named, the criteria name their tests, no
  format, migration or removal flag.
- **Probe.** No build was made for this analysis: the cause is read from
  `server.rs::call`, the generated schemas and the stdio tests that already
  assert `structuredContent.code` on refusals; the client behaviour is read
  from the SDK sources and the installed Claude Code binary.
