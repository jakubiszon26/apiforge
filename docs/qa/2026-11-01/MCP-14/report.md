# MCP-14: `run`'s structured output matches its own output schema — QA report

- Branch and commit: `fix/mcp-14-run-output-schema`. Acceptance tests
  `8929d97`; fix `a66bfd7`.
- Built with: `cargo build --release --locked -p apiforge-cli` at `a66bfd7`.
- Mock-up: not applicable; nothing in the window changes. Issue:
  [#181](https://github.com/jakubiszon26/apiforge/issues/181). Ticket:
  [`docs/tickets/MCP-14.md`](../../../tickets/MCP-14.md).
- Machine: Linux 7.0, Rust 1.99.0, rmcp 3.8.0, schemars 1.2.2; validator
  `jsonschema` 0.59.0 (JSON Schema 2020-12, as the schemas declare).

## Linux

### Scenario 1: the demo shop simulation over real stdio

A small client (Rust, kept outside the repository) wrote a fresh demo-shop
checkout (`prepare_project` of the shop, without `.apiforge/local`), started
the release `apiforge mcp --folder <it>`, sent `initialize` (revision
`2025-11-25`), `tools/list` and `run` of
`scenarios/promotion-and-cart-price.afs`, and validated the result's
`structuredContent` against the `outputSchema` of `run` from that
`tools/list`. [transcript.txt](transcript.txt): `>>` sent, `<<` received
(cut to 220 characters), `--` the client's findings.

Expected: `run`'s schema requires `replayed` and `journal_unreadable`, and
`Step` requires `may_have_reached_api`; the result carries all three as
`false` and validates.
Observed: as expected. The schema requires `path, live, status, ended_by,
generation, used, redacted, step_count, replayed, interrupted,
journal_unreadable` and, per step, `handle, status, phase,
may_have_reached_api`. The result was `passed`, `completed`, 18 steps,
`replayed: false`, `journal_unreadable: false`, `may_have_reached_api: false`
on the first step and on its first nested step; the validator reported no
error. The process exited 0 at the end of stdin.

Before the fix, at the acceptance commit `8929d97`, the same check in the
stdio tests failed on every `run` result with `"replayed" is a required
property`, `"journal_unreadable" is a required property` and
`"may_have_reached_api" is a required property at "/steps/0"` (and every
other step): 9 stdio tests failed, the 3 new ones and 6 existing sessions
that run something.

### Scenario 2: every kind of run and every tool (criteria 1 to 3)

`cargo test -p apiforge-cli --test mcp --locked` at `a66bfd7`: 25 passed.

- `mcp14_criterion_1_simulated_run_results_match_the_output_schema`: passed
  with nested component steps, a component alone with `inputs`, `full: true`,
  a failed verdict (`expected: 81`), an unreadable journal
  (`journal_unreadable: true`), and a run past a 300 ms deadline
  (`cancelled`, `timed_out`) all validate; every flag is present.
- `mcp14_criterion_1_live_run_results_match_the_output_schema`: a live run
  against the demo API with a `request_id` (`replayed: false`), its replay
  (`replayed: true`), and a live run against a listener that never answers,
  past a 500 ms deadline, with exactly one step `may_have_reached_api: true`;
  all validate.
- `mcp14_criterion_2_every_tool_result_matches_its_output_schema`: all
  eleven tools answered with a result in one session, each validated against
  its own schema from `tools/list`.
- The harness also validated every successful result of the 22 existing stdio
  sessions, among them `interrupted` runs reported after a blocked cleanup and
  after a killed server.

Other mismatches found: none. Only `run` failed; the other ten tools' results
already matched their schemas.

## Windows

Not run on Windows by the implementing agent: the CI run of the branch runs
the same tests on `windows-2022`.

## Verification

At `a66bfd7`, on Linux:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
- `cargo test --workspace --all-targets --locked`: 61 test binaries, 1321
  passed, 0 failed, 34 ignored.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: passed.

At the merge of `main` (`ed68261`, REL-03; no Rust or Cargo file changed by
it), the CI's third-party notice check with cargo-about 0.9.2: `cargo about
generate --frozen --workspace --fail` passed with no stale clarification,
and `packaging/release.py check-notices` named all 332 crates. `jsonschema`
and the crates it adds are dev-only and so not in the notices
(`ignore-dev-dependencies`).

## Not verified

- A real MCP client (Claude Code or another) against this branch: left to
  the owner, who found the defect with one. The validation here is the
  `jsonschema` crate on the schema `tools/list` advertises.
- Error results (`isError: true`): their `structuredContent` is the error
  object (`code`, `message`, `details`, `wrote`), not shaped like the tool's
  `outputSchema`, and was not validated. A client that validates the
  structured content of error results too would reject them; unchanged
  here and out of the ticket's scope.
- Whether results carry properties a schema does not declare: the schemas
  allow additional properties, so a client does not reject them; not
  checked.
- The regression smoke list: not run; the change is in the MCP server's
  output only and nothing in the window changes.
