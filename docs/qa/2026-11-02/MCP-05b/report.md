# MCP-05b: Publish request ids survive a server restart — QA report

- Branch and commits: `feat/mcp-05b-publish-ids-restart` from `main`
  `ebd9a6e`. Analysis `e3ccd14`, acceptance tests `d6e7dba`,
  implementation `e2f4ee0`, guide and tickets `0e1e0a7`.
- Built with: `cargo build --release --locked -p apiforge-cli` from the
  tree of `e2f4ee0`, which has the same code as `0e1e0a7` (the release
  binary `apiforge`).
- Mock-up: not applicable, since nothing in the window changes. Issue:
  [#121](https://github.com/jakubiszon26/apiforge/issues/121). Ticket:
  [`docs/tickets/MCP-05b.md`](../../../tickets/MCP-05b.md).
- Machine: Linux 7.0, Rust 1.99.0, Python 3.12 for the stdio client.

## Linux

A small Python client, kept outside the repository, drove the release
`apiforge mcp --folder <checkout>` over stdin and stdout with protocol
revision `2025-11-25` and client name `qa`. Each checkout is a fresh demo
shop: `prepare_project` of the shop without `.apiforge/local`, which is
what a fresh Git clone holds. A server ends when its stdin closes, so a
new process for the same folder is a restarted server. The whole
transcript is in [transcript.txt](transcript.txt):

- `>>` lines were sent.
- `<<` lines were received, cut to 220 characters.
- `--` lines are the client's summary of a structured result.
- `##` lines are actions outside the protocol: starting a server, writing
  or showing the record, and comparing the hashes of every file outside
  `.apiforge/local`.

### Scenario 1: a restarted server replays a recorded publication (criterion 1)

Tests: `mcp05b_criterion_1_a_new_server_replays_a_recorded_publication`.

1. Server A: `publish(components/product.afs, eba04ff2…, request_id:
   "qa-restart-1")`.
2. Close server A's stdin.
3. Server B, a new process for the same folder: the same call; then the
   same id with the hash `000…0`; then the same id for
   `components/active-promotion.afs` with that component's hash.

Expected:
- Server A publishes revision 2 and records the id with its result.
- Server B returns the same result with `replayed: true` and writes
  nothing.
- Each of the two other calls is `invalid_inputs`, and nothing is written.

Observed: as expected.
- Server A answered revision 2, `generation` 1 and `replayed: false`. The
  record then held one `publish` entry: id, path, `expectedSourceSha256`,
  `startedAt`, and the result with `changed` and `revision_path`. Server A
  exited 0 at the end of stdin.
- Server B answered revision 2 with the same `source_sha256`, `generation`
  1 and `replayed: true`. Every file outside `.apiforge/local` had the same
  hash before and after.
- The other hash and the other path were each `invalid_inputs` with
  `wrote: false` and `details.path` `components/product.afs`. The files were
  unchanged, and the overview listed `product` revisions 1 and 2 only.

### Scenario 2: an entry without a result is unknown_outcome (criterion 2)

Tests: `mcp05b_criterion_2_a_publication_an_earlier_server_started_is_unknown_outcome`.

The client wrote a `publish` entry without a result for `qa-lost`, which is
what a server leaves when it stops between writing the entry and recording
the result. Then a new server got `publish` with that id, path and hash.

Expected: `unknown_outcome` with the entry's `path`,
`expected_source_sha256` and `started_at`, the advice to read
`project_overview`, and nothing written.

Observed: as expected. The message says *project_overview lists every
revision with its source_sha256, which tells whether it was published*. The
files and the record were byte-identical after the call, and the overview
still listed `product` at revision 1 only.

Not shown over stdio: a server killed at exactly that moment. The timing
cannot be hit from outside. The test reaches the same state with a fault
injected after the transaction's activation (`recovery_required`): the
entry stays without a result, and after recovery a new server answers
`unknown_outcome`. The same test checks that an entry which cannot be
written (a record one entry short of the 16 MiB limit) refuses with
`project_unreadable` and publishes nothing.

### Scenario 3: a refusal before the commit leaves no entry (criterion 3)

Tests: `mcp05b_criterion_3_a_refused_publication_leaves_no_entry`.

In the same session, `publish` with `request_id: "qa-refused-first"` and a
stale hash, then the same id with the right hash.

Expected: the first call is `stale_source` and leaves no entry, and the
second publishes.

Observed: as expected. After the refusal, `qa-refused-first` was not in the
record. The second call published revision 2, and its entry holds the
result. The test also covers these refusals, each with the same id: held by
another process, a scenario, a component over 1 MiB, one with errors, one
that would lose a comment, and a commit that fails before its activation.

### Scenario 4: one set of rules (criterion 4)

Tests: `mcp05b_criterion_4_an_unreadable_record_refuses_a_publish_with_an_id`,
and the unit tests `the_record_keeps_the_newest_ids`,
`a_version_1_record_is_read_as_live_runs` and
`a_publication_round_trips_and_nothing_unknown_is_read` in
`crates/apiforge-mcp/src/requests.rs`.

1. Write `{ broken` to the record.
2. `publish` with an id; then a simulation; then `publish` without an id.
3. Replace the record with a version-1 record holding one live run, then
   `publish` with an id.

Expected:
- The publication with an id is `project_unreadable`, names the file, and
  publishes nothing. The simulation and the publication without an id
  work.
- The version-1 record is read and rewritten as version 2, and its live
  run is kept.

Observed: as expected.
- The publication with an id was `project_unreadable`. Its message reads
  *The record of request ids, .apiforge/local/agent-requests.json, cannot be
  read …*, with `details.file` naming the file. The files were unchanged,
  and the record still held `{ broken`, never overwritten.
- The simulation passed, and the publication without an id published
  revision 2.
- After the version-1 record, `qa-after-v1` published revision 3. The
  record became `version: 2` with the `liveRun` entry
  `qa-live-from-mcp-06` first and the `publish` entry after it.
- The test also fills a version-1 record with 1 000 live runs: after one
  publication, 1 000 entries remain, with the oldest live run dropped and
  the publication last.

### Scenario 5: no secret value in the record (criterion 5)

Tests: `mcp06_criterion_10_no_secret_value_leaves_the_server` in
`crates/apiforge-cli/tests/mcp.rs`, over stdio with a secret in the
environment and another in `--secrets`.

Expected: `publish` with a secret as its `request_id` is `secret_in_text`,
and the value is not echoed. A publication with a plain id is recorded. The
record, like every result, stderr line and the journal, holds neither
value.

Observed: as expected. The test passed at `e2f4ee0`, and at `d6e7dba` it
failed because no record existed. In the transcript above, every record
shown holds only ids, paths, hashes, times, revision numbers, handles,
revision paths, generations and changed paths.

### Scenario 6: MCP-06's live runs unchanged (criterion 6)

Tests: `mcp06_criterion_9_an_answered_request_id_returns_the_recorded_result`,
`mcp06_criterion_9_a_new_server_answers_unknown_outcome_for_an_id_an_earlier_server_started`
and `mcp06_review_a_request_id_names_one_run_and_is_never_run_twice`, all
unedited. All passed. MCP-05's in-memory `Answered` and its three unit
tests are removed.

### Scenario 7: documentation (criterion 7)

The `publish` bullet and *The record of request ids* in
`docs/development.md`, the `publish` tool description in `server.rs`, the
`request_id` field's description, and MCP-05 criterion 6 now say that a
restarted server answers from the record.

## Windows

Not run on Windows by the implementing agent. The CI run of the branch runs
the same tests on `windows-2022`.

## Verification

On Linux, at `0e1e0a7`:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
- `cargo test --workspace --all-targets --locked`: 61 test binaries, 1405 passed, 0 failed, 43 ignored. That run may have
  started before the last rewording of the `unknown_outcome` message, so
  `apiforge-mcp` and `apiforge-cli` were run again at `0e1e0a7`: 181 passed,
  1 ignored (the spawned MCP-12 helper).
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`:
  passed.

At the acceptance commit `d6e7dba`, the four `mcp05b_` tests and the stdio
`mcp06_criterion_10_…` failed for the reasons the ticket names:
- A new server published revision 2 instead of replaying.
- Nothing reached the record.

## Not verified

- A server killed between writing the entry and recording the result, over
  stdio: the moment cannot be hit from outside. The state is reached in the
  test with an injected fault after the activation, and over stdio with a
  hand-written entry.
- A failure to write the result after a successful commit. The code
  returns the revision and leaves the entry without a result, so a retry is
  `unknown_outcome`. No test makes that one write fail without making the
  first one fail too.
- A real MCP client (Claude Code or another) against this branch.
- One behaviour changes from MCP-05, as the ticket's *Not in scope* says:
  answering a repeated id now reads the record, so it needs the project.
  While a window or another process holds the project, the same server
  that published used to replay from memory; it is now refused with the
  holder (`held_by_window`, `held_by_other`), as a live run's repeat is.
  This is not covered by a new test.
- The regression smoke list was not run. The change is in the MCP server
  and its machine-local record only, and nothing in the window reads the
  record.

## Review round 1 (head `e5cf13d`)

- CodeQL `rust/cleartext-logging` (alerts 44 and 45) flagged two sites
  where a `mcp05b_` assertion formatted a tool error's message into its
  failure text. In `47d1c5f`, all four `mcp05b_` assertions that formatted
  a tool error (its message or `error_text`) stopped doing so. This is the
  same kind of fix MCP-05 made in `bb698bd`.
- cubic, the `publish` description: it now says that a replay needs the
  same path and hash, and that another path or hash is `invalid_inputs`.
- cubic, a failed removal of an entry after a refusal: the guide now
  says what happens. The entry stays and the id answers `unknown_outcome`,
  never a second revision. The code was already written that way.
- cubic, an id dropped after 1 000 newer entries publishes again; the
  version-1 read; the ticket's regression section: answered on the pull
  request with the reasons, and nothing changed.

After the round, `apiforge-mcp` and `apiforge-cli`: 181 passed, 1 ignored;
fmt and clippy clean.

CI on the review heads, windows-2022 only:

- `47d1c5f`: one of its two windows-2022 runs failed in
  `mcp06_criterion_5_a_change_after_the_start_does_not_stop_the_run` with
  `LiveDisabled`, and the other run passed.
  [Issue #122](https://github.com/jakubiszon26/apiforge/issues/122) (titled
  *KTLO-33: Four tests fail now and then on CI and pass on a rerun*) records
  this failure in the owner's comment of 2026-11-01. That comment names
  this test, the same `LiveDisabled`, and the cause: the test turns the
  consent off 250 ms after the run takes its slot.
- `4636fae`: one windows-2022 job was cancelled at the job's 30-minute
  limit, and the other passed in 29 min 18 s. This is not a test failure.
  It is not on #122, so the job was not rerun; the next push ran CI again.
