# MCP-06: Live requests from the server — the four ends of a run — QA report

- Branch and commit: `feat/mcp-06-live-runs`. Acceptance tests `4485edc`;
  implementation `c60c935` (storage) and `66f65eb` (mcp, cli); development
  guide `f08038f`.
- Built with: `cargo build --release --locked -p apiforge-cli -p
  apiforge-demo-api` at `f08038f` (release binaries `apiforge` and
  `apiforge-demo-api`).
- Mock-up: not applicable; nothing in the window changes. Issue:
  [#65](https://github.com/jakubiszon26/apiforge/issues/65). Ticket:
  [`docs/tickets/MCP-06.md`](../../../tickets/MCP-06.md).
- Machine: Linux 7.0 (Ubuntu 24.04), Rust 1.99.0, rmcp 3.8.0, Python 3.12
  for the stdio client.

## Linux

Every session below ran the release binary over stdin and stdout against
its own copy of a fresh demo-shop checkout (the project `prepare_project`
writes, stripped of `.apiforge/local`, which is what a Git clone holds). A
small Python client sent protocol revision `2025-11-25` with client name
`qa`. Live requests went only to loopback: the release `apiforge-demo-api`
on `127.0.0.1:4399`, or a listener on a free loopback port that accepts
connections and never answers, so a request sent to it stays on its way.
The whole run is in [transcript.txt](transcript.txt): `>>` was sent, `<<`
received (cut to about 220 characters), `--` lines summarise a structured
result and `##` lines are actions outside the protocol (reading the journal,
`apiforge apply` to see whether the lock is held, a kill). The scratch path
of the secrets file is shown as `<scratch>`, and the value the client itself
sent inside one `check_text` argument as `<value of
APIFORGE_SECRET_TOKEN>`.

Whether another process holds the project is observed with `apiforge apply`
of the saved text: it takes the writer lock and fails while someone else
holds it.

### Scenario 1: without `--allow-live` (criterion 1)

Tests: `mcp06_criterion_1_without_allow_live_nothing_is_sent`,
`mcp06_criterion_1_without_allow_live_over_stdio`.

Expected: `run(live: true)` is `live_disabled`, nothing is sent and nothing
is taken; the overview says `{ allowed: false, source: "no --allow-live" }`.

Observed: as expected. The refusal read *live requests are not enabled on
this server*, `wrote: false`; the never-answering listener behind the base
URL accepted 0 connections; the checkout had no `.apiforge/local`
afterwards, so no lock was taken; stderr held one line, `apiforge mcp: run:
live_disabled`. A server started with `--allow-live` reported `{ allowed:
true, source: "--allow-live" }`. `tools/list` shows `run` with
`readOnlyHint: false`, `destructiveHint: true`, `idempotentHint: false`,
`openWorldHint: true`, and its description's first sentence says that
`live: true` sends real requests to the project's API only when the person
allowed them.

### Scenario 2: a live run against the demo API with a declared secret (criterion 2)

Test: `mcp06_criterion_2_a_live_run_against_the_demo_api_passes_redacted`.

The demo scenario gained two declared secrets, `TOKEN` (from
`APIFORGE_SECRET_TOKEN` in the server's environment) and `FILE` (from the
`--secrets` file), and one request that sends `TOKEN` in its query and
`FILE` in a header.

Observed: as expected. `live: true`, `status: passed`, `ended_by:
completed`, 19 steps, `generation: 0`, the scenario's `source_sha256`,
`redacted: true`, `interrupted: []`. With `full: true` the request's URL in
the report is `http://127.0.0.1:4399/demo/shop/products/124?q=[REDACTED]`;
`[REDACTED]` occurs four times in the report and neither value anywhere. The
journal file was gone after each run. The holder file named the agent
session (`"role":"agent","client":"qa"`), and `apiforge apply` was refused
while the session lasted.

### Scenario 3: another process holds the project (criterion 3)

Test: `mcp06_criterion_3_a_live_run_needs_the_agent_session` (a window and
`apply` as holders, then the run taking and keeping the session).

Observed: as expected. A second server on the same checkout, while the first
kept its agent session, answered a live run with `held_by_other`: *qa via
apiforge mcp has held this project since …; nothing was written*, with
`details.holder` (role, client, since). A simulation on that second server
in the same session passed with 19 steps. `finish_editing` on the first
server released the project. The window as holder (`held_by_window`) is
covered by the test only.

### Scenario 4: a repeated `request_id` (criterion 9, first half)

Test: `mcp06_criterion_9_an_answered_request_id_returns_the_recorded_result`.

Observed: as expected. A live run with `request_id: "qa-1"` passed against
the demo API. The base URL was then pointed at the counting listener, and the
same call returned the recorded result with `replayed: true`; the listener
accepted 0 connections.

The second half of criterion 9 — *a new server answers `unknown_outcome` for
an id it never saw* — is not implemented: see [Not verified](#not-verified).

### Scenario 5: cancelled and timed out (criterion 6)

Test: `mcp06_criterion_6_a_cancelled_or_timed_out_live_run`.

Observed: as expected. With the request on its way to the listener the
journal held one run with one entry, `GET …/products/123`, `state:
"inFlight"`, written before the request was sent (transcript, scenario 5).
`notifications/cancelled` for the call: the journal file was removed 0.04 s
later, the call got no response, and stderr said `apiforge mcp: run:
cancelled`. A server with the deadline shortened to 800 ms
(`APIFORGE_MCP_TEST_DEADLINE_MS`) answered `status: cancelled`, `ended_by:
timed_out`, with `fetch_product` marked `may_have_reached_api` and the
cleanup's two steps (`delete_cart`, `cart_was_deleted`, phase `cleanup`,
skipped because the cart was never created) in the report; the journal was
gone afterwards.

### Scenario 6: the client goes away (criterion 7)

Tests: `mcp06_criterion_7_eof_during_a_live_run_empties_the_journal`,
`mcp06_criterion_7_with_the_cleanup_blocked_the_entry_stays`.

Observed: as expected. Closing stdin while a live request was on its way
ended the process 0.1 s later with exit code 0 and `apiforge mcp: run:
cancelled` on stderr; the journal file was gone and `apiforge apply` could
take the project. With the cleanup held by the test seam
`APIFORGE_MCP_TEST_CLEANUP_DELAY_MS`, the process waited for it and exited
10.5 s after the end of stdin, exit code 0; the lock was released and the
journal kept the run, its entry now `uncertain` (*cancelled while the
request was being sent*). A new server without `--allow-live` reported
`interrupted_runs: 1` and its simulation returned the run under
`interrupted`: *Promotion and cart price*, `GET
http://127.0.0.1:…/demo/shop/products/123`, `Possibly performed`.

### Scenario 7: the server is killed (criterion 8)

Tests: `mcp06_criterion_8_a_killed_server_leaves_the_journal_entry` (stdio),
`mcp06_criterion_8_an_interrupted_run_is_reported_by_the_next_run`.

Observed: as expected. `SIGKILL` with the request on its way left the
journal's entry `inFlight`, and the operating system released the lock. For
comparison, `apiforge run --folder … --live` (pointed at the demo API)
printed it as *A live run of "Promotion and cart price" from … did not
finish. These requests may have changed the API: GET …/products/123 —
Sending*, and left the entry in place, as the CLI always has. A new server
reported `interrupted_runs: 1`; its simulation and, on another new server
with `--allow-live`, its passing live run both returned the same run under
`interrupted` with `GET`, the same URL and `Sending`. The entry stays until
the window marks it handled.

### Scenario 8: secrets (criteria 10 and 11)

Tests: `mcp06_criterion_10_no_secret_value_leaves_the_server` (stdio, every
tool), `mcp06_criterion_10_no_tool_accepts_a_secret_value`,
`mcp06_criterion_11_a_saved_source_with_a_secret_is_withheld`,
`mcp06_a_secrets_file_that_cannot_be_read_stops_the_server`.

Observed: as expected. With `components/product.afs` holding the value of
`APIFORGE_SECRET_TOKEN` in a comment on line 2, `read_source` and `print`
answered `secret_in_source` with `details.line: 2`, and `resources/read` the
JSON-RPC error with `data.code: secret_in_source`; no text was returned. A
`check_text` argument carrying the value was `secret_in_text`. stderr named
the tool and the code only.

**The grep.** Every line the eight scenarios' servers wrote to stdout and
stderr, every journal file read during the runs (including those taken with
a request on its way) and the CLI's output were collected: neither secret
value occurs in them (0 occurrences each). The only occurrence in the
transcript is the client's own `check_text` request. The stdio test of
criterion 10 runs every tool — overview, reference, read, check,
`check_text`, print, simulated and live runs with and without `full`, the
resources, `write_source`, `create_document`, `finish_editing` — plus
refused writes and a refused `request_id` carrying either value, and asserts
the same for stdout, stderr and a journal taken mid-run.

### Verification (criteria 12 and 13)

The development guide's *MCP server* section has a *Live requests* part with
one paragraph each for the flag, the environment and the journal, and *The
four ends of a live run* (test
`mcp06_criterion_12_the_development_guide_explains_live_runs`). At
`f08038f`: `cargo fmt --all -- --check`, `cargo clippy --workspace
--all-targets --locked -- -D warnings`, `cargo test --workspace --all-targets
--locked` (every binary passed; the MCP library acceptance file 72 passed and
1 ignored, the stdio file 20 passed) and `cargo run -p apiforge-convert-v1
--locked -- --fixtures --check` passed.

## Windows

Not run by hand: no Windows machine here. CI runs the whole suite, the
stdio tests among them, on `windows-2022` for this branch; the pull request
records that run's result. Two points are specific to Windows and are only
covered there by CI: the journal and lock of a killed process (the test
kills the child with `TerminateProcess`), and the end of stdin through a
pipe.

## Not verified

- **Criterion 9, second half: not implemented, an owner decision.** *A new
  server answers `unknown_outcome` for an id it never saw* cannot be met as
  written: a server cannot tell an id it never saw from the first use of a
  new id, so every first live run with a `request_id` would be refused. The
  server remembers ids in memory only (as MCP-05 plans for `publish`); a new
  server runs an unseen id, and every result carries the journal's
  `interrupted` runs, so a model that lost an answer across a restart sees
  what may have happened. The test is written as the ticket states it and
  ignored: `mcp06_criterion_9_a_new_server_answers_unknown_outcome_for_an_unseen_id`.
- **Claude Code with `--allow-live`: not verified by the implementing agent;
  left to the owner.** Registered with `claude mcp add apiforge --
  /path/to/apiforge mcp --allow-live`, `/mcp` should show `run` as a tool
  that is not read-only.
- The window's toggle and `permission` message: MCP-09. The slot and its
  seam are tested in process only.
- `held_by_window` for a live run: covered by the test with a window holder
  description, not with a running window.

## Regression smoke list

Not run: nothing in the window changes and the desktop crate is untouched.
The workspace suite, which covers items 3 to 10 headlessly, passes.

## Review round 1

The automated review of PR #113 (cubic) found eleven points and CodeQL three
cleartext-logging alerts, all in the acceptance tests. Fixed:

- **A dropped `request_id` would run again.** The server keeps the last 64
  live results; an older id it answered is now `unknown_outcome` and sends
  nothing. The same id for another document or other inputs is
  `invalid_inputs`. Test:
  `mcp06_review_a_request_id_names_one_run_and_is_never_run_twice`.
- **The server's own run counted as interrupted.** While a live run of this
  server is in progress, the overview and `run` report only the journal's
  runs from before it. **A journal that cannot be read** no longer fails a
  simulation that has run; the result says `journal_unreadable: true`. Test:
  `mcp06_review_the_server_own_run_is_not_interrupted`.
- **The journal read waited for a write.** The agent session's store is kept
  in a slot of its own, so the overview never waits for a write.
- **Encoded secrets in an older journal.** Interrupted entries are masked by
  the runner's redactor, which knows a value's percent- and form-encoded
  forms.
- The guide says that a journal entry exists while its run is in progress,
  and gives `run`'s annotations in the contract; the smoke-list line no
  longer cites a scenario that counted nothing; the stdio helpers fail when
  a fixture's text is not replaced; the EOF test waits for the connection
  before asserting it; assertion messages print the run's error, not the
  report (CodeQL).

On the second head, CI's Windows job failed
`mcp06_criterion_7_with_the_cleanup_blocked_the_entry_stays` once (*no
request was journalled* within ten seconds) and CodeQL still flagged four
assertion messages. The tests polled `journal.json` every 5 ms while the
runner replaced it, which Windows can refuse while the file is open; they
now wait for the request to reach the listener (it is journalled before it
is sent) and read the file only then, and the flagged messages are gone.
The server itself no longer reads the journal while its own live run
writes it: the overview and `run` answer from the runs read before that
run started.

The Windows job of the first CI run failed one MCP-07 window test
(`a_transaction_left_by_the_holder_opens_the_recovery_view`, the window not
taking the project back within ten seconds); the desktop crate is unchanged
here, and the same commit's second run passed it on Windows.
