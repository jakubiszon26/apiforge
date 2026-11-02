# MCP-17: A refusal carries nothing the tool's output schema rejects — QA report

- Branch: `docs/mcp-17-error-structured`. Owner's decision recorded in
  `dac1691`; acceptance tests `74a70ac`; fix `855d8f5`.
- Built with: `cargo build --release --locked -p apiforge-cli`, Rust 1.99.0,
  rmcp 3.8.0. "Before" is the release binary of `dac1691` (the server code of
  `main` at `ebd9a6e`); "after" is the release binary of `855d8f5`.
- Mock-up: not applicable; nothing in the window changes. Issue:
  [#188](https://github.com/jakubiszon26/apiforge/issues/188). Ticket:
  [`docs/tickets/MCP-17.md`](../../../tickets/MCP-17.md), decision 1 = A
  (text only), taken 2026-11-02.
- Analysis model: Fable. Implementation model: Claude Opus 5.5.

## Criteria and their evidence

| # | Criterion | Tests | Release binary |
| --- | --- | --- | --- |
| 1 | A refusal has no `structuredContent`; one text block whose JSON is exactly `{ code, message, wrote, details }`, with the values the tools gave before | `mcp17_criterion_1_a_refusal_travels_as_text`; the harness `tool_error`/`refusal` of `crates/apiforge-cli/tests/mcp.rs` on every refusal of every stdio session; `apiforge_mcp::test_support::tool_error` on every refusal of the desktop MCP tests | Scenario 1 |
| 2 | A client that validates every result carrying `structuredContent`, errors included, accepts every result | `Server::response` validates whenever `structuredContent` is present, in every stdio session; `mcp17_criterion_2_every_refusal_passes_a_validating_client` | Scenario 1 |
| 3 | Successful results are unchanged | `mcp14_criterion_*` unchanged and passing | Scenario 1, the last call |
| 4 | Codes, `wrote` and `details` unchanged; the contract bullet says where the error object travels | the criterion 1 test compares with the library's own error; `docs/development.md`, *The contract* | Scenario 1 |
| 5 | The JSON-RPC errors of `resources/read` are not touched | `mcp03_check_text_run_the_reference_and_resources_over_stdio` (its `path_rejected` read is still `error.data.code`) | — |

## Linux

### Scenario 1: refusals of the real stdio server, before and after

A small client (Python, kept outside the repository) wrote a fresh demo-shop
checkout (`prepare_project` of the shop, without `.apiforge/local`), started
the release `apiforge mcp --folder <it>`, sent `initialize` (revision
`2025-11-25`), `tools/list`, and then:

1. `read_source` of `../x.afs` (`path_rejected`);
2. `run` of the scenario with `live: true` on a server without
   `--allow-live` (`live_disabled`);
3. `write_source` of the product with an `expected_source_sha256` of
   64 zeros (`stale_source`);
4. `check` of `components/missing.afs` (`unknown_document`);
5. `check` of the whole project (a result).

For each answer it printed the raw line and what it found: whether
`structuredContent` is there, the keys of the text's JSON and, where
`structuredContent` is present, which properties the tool's `outputSchema`
requires and the object lacks. Transcripts: [before.txt](before.txt) and
[after.txt](after.txt); `>>` sent, `<<` received, `--` the client's
findings.

Expected before: every refusal carries `structuredContent`, the error object,
which lacks every property its tool's schema requires.
Observed before: as expected. The `path_rejected` refusal of `read_source`
lacked `path, text, generation, valid, diagnostics, omitted`; the
`live_disabled` refusal of `run` lacked all eleven required properties of
`run`, from `path` to `journal_unreadable`; `write_source`'s `stale_source`
lacked seven and `check`'s `unknown_document` three. A client that validates
whatever `structuredContent` it receives rejects each of them.

Expected after: `"isError": true`, `content` with one text block, no
`structuredContent` key, and the same text as before; the result unchanged.
Observed after: as expected. The four refusals are
`{"content":[{"type":"text","text":…}],"isError":true}`; each text is
byte for byte the text of the binary before, for example

```text
{
  "code": "live_disabled",
  "message": "live requests are not enabled on this server",
  "wrote": false,
  "details": {
    "source": "no --allow-live"
  }
}
```

The fifth answer, `check`'s result, is identical in both transcripts, its
`structuredContent` included. stderr carried one line per refusal in both
runs, and the process exited 0 at the end of stdin.

### Scenario 2: the tests before and after the implementation

At the acceptance commit `74a70ac`, with the server unchanged:

- `cargo test -p apiforge-cli --test mcp`: 15 of 29 failed, every one at
  its first refusal, in the harness's validation:
  `read_source's structured result does not match its outputSchema:
  "path" is a required property at ""`, and the like for the other tools.
  The 14 that passed are the sessions with no refusal.
- The desktop's MCP tests, `cargo test -p apiforge-desktop --locked --bins
  -- acceptance_tests::mcp08b acceptance_tests::mcp08c
  acceptance_tests::mcp09`: of the 39 tests the filters match, 15 failed, in
  `tool_error` on the `structuredContent` a refusal still carried; 19
  passed; 5 are ignored and were not run (three design-review views and two
  spawned helpers, `server_process` and `mcp12_window_process`). The run's
  output also shows a second `test result` line with one failure: it is
  `mcp12_window_process`, spawned as a child by the failing
  `mcp12_different_environments_handover_and_observation`, not a further
  test.

At `855d8f5`, the fix, with no assertion changed: all of them pass.

- `mcp17_criterion_1_a_refusal_travels_as_text`: the raw `result` of a
  `path_rejected` and a `live_disabled` refusal has exactly the keys
  `content` and `isError`; `content` is exactly one text block whose text
  equals `serde_json::to_string_pretty` of the error the library gives
  (`apiforge_mcp::tools::read_source(root, "../x.afs")`, and the
  `live_disabled` error with source `no --allow-live`).
- `mcp17_criterion_2_every_refusal_passes_a_validating_client`: `not_a_project`
  from `project_overview` outside a project; in the demo shop
  `path_rejected` from `read_source` and `check_text`, `unknown_document`
  from `check`, `print` and `publish`, `live_disabled` from `run`,
  `stale_source` from `write_source` and `path_exists` from
  `create_document`. The test asserts these nine tools were refused.
  `language_reference` and `finish_editing` have no refusal: they answer
  every call with a result (the ticket records it).
- Every refusal of the existing stdio sessions now goes through `refusal`,
  which asserts the shape of criterion 1: `path_rejected`,
  `unknown_document`, `not_a_project`, `live_disabled`, `run_in_progress`,
  `stale_source`, `secret_in_text`, `held_by_window`, `unknown_outcome`,
  `comments_would_be_lost`. The two loops that wait for a run's slot to
  free (`run_in_progress`) read the code from the text: read from
  `structuredContent`, they would have stopped at the first answer.

## Windows

Not run on Windows by the implementing agent: the CI run of the branch runs
the same tests on `windows-2022`.

## Verification

At `855d8f5`, on Linux:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
- `cargo test --workspace --all-targets --locked`: 1404 passed, 0 failed,
  43 ignored.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: passed.

## Not verified

- A real MCP client (Claude Code, Codex) against this branch: the validating
  client here is the `jsonschema` crate in the stdio harness and the Python
  client above. Which of the two `callTool` forms Claude Code 2.1.316 uses
  could not be determined (see the ticket); text-only refusals pass both.
- A client that read a refusal's `structuredContent`: outside this
  repository none is known; inside it, every reader now reads the text.
- `wrote: "unknown"` (a commit that failed after its activation) over
  stdio: no test drives it; it travels through the same code path as every
  other refusal.
- `result_too_large` over stdio: not driven; same path.
- The regression smoke list: not run; the change is in the MCP server's
  output only and nothing in the window changes.
