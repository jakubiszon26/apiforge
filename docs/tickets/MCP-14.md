# MCP-14: `run`'s structured output matches its own output schema

- Size: S · Owner review: 10 min · Risk: none (non-UI hotfix)
- State: In progress (issue #181) · owner decision 2026-11-01: hotfix before
  other tickets, merge authorised on green CI
- Analysis model: Claude Opus 5.5 (a bug with no product decision) ·
  Implementation model: Claude Opus 5.5

## Goal

An MCP client that validates structured results (as clients do against a
tool's `outputSchema`) accepts every `run` result, simulated and live, so the
model gets the run's report.

## Cause

`RunReport.replayed`, `RunReport.journal_unreadable` and
`Step.may_have_reached_api` (`crates/apiforge-mcp/src/tools.rs`) were
`#[serde(skip_serializing_if = "std::ops::Not::not")]`, while the schema
generated for the tool (`schema_for_output::<RunReport>()`, rmcp 3.8.0 with
schemars 1.2.2) lists them as required. Every `false` was omitted, so almost
every `run` result failed the client's validation.

## What the owner will see

- Changed: `run` results always carry `replayed`, `journal_unreadable` and,
  on every step, `may_have_reached_api`; a `false` is sent instead of
  omitted. Names and meanings are unchanged (additive for clients).
- Nothing in the window changes. Removed, hidden, migrated, defaults:
  nothing.

## Acceptance criteria

1. Every kind of `run` result validates against the `run` tool's
   `outputSchema` from `tools/list`: a passed simulation with a component's
   steps nested, a component alone with inputs, the full report, a failed
   verdict, an unreadable journal, a run past the deadline (`cancelled`), a
   live run, its replay for a repeated `request_id`, and a live request on
   its way at the deadline (`may_have_reached_api: true`).
   Tests: `mcp14_criterion_1_simulated_run_results_match_the_output_schema`,
   `mcp14_criterion_1_live_run_results_match_the_output_schema`
   (`crates/apiforge-cli/tests/mcp.rs`). A run reporting `interrupted` runs is
   validated by `mcp06_criterion_7_with_the_cleanup_blocked_the_entry_stays`
   and `mcp06_criterion_8_a_killed_server_leaves_the_journal_entry` through
   the harness below.
2. The structured output of every tool is validated against that tool's
   advertised `outputSchema` over stdio, so no other tool has the same
   mismatch. The stdio harness of `crates/apiforge-cli/tests/mcp.rs` takes
   the schemas from `tools/list` of a real `apiforge mcp` and validates every
   successful `tools/call` result it reads, in every existing stdio session;
   `mcp14_criterion_2_every_tool_result_matches_its_output_schema` is one
   session in which all eleven tools answer with a result and asserts each
   was validated.
3. Fields keep their names and meanings; a `false` is sent instead of
   omitted. Checked in both criterion 1 tests (`replayed`,
   `journal_unreadable` and every step's `may_have_reached_api` are present
   as booleans, `false` where nothing happened).

## Validator

The [`jsonschema`](https://crates.io/crates/jsonschema) crate, pinned
`=0.59.0` in the workspace with `default-features = false` (no HTTP or file
reference resolution), as a dev-dependency of `apiforge-cli` only: it never
reaches a release binary. It and the crates it adds are MIT, MIT-0,
Unlicense/MIT or MIT/Apache-2.0, compatible with GPL-3.0-or-later; the
workspace has no `cargo deny` configuration to update.

## Out of scope

- Error results (`isError: true`) carry the error object as
  `structuredContent`, which is not shaped like the tool's `outputSchema`;
  this ticket does not change them (see the QA report).
- The CLI live-run refusal is MCP-15.
