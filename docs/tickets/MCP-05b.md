# MCP-05b: Publish request ids survive a server restart, in MCP-06's record

- Size: S · Owner review: 10 min · Risk: none on screen. The machine-local
  record `.apiforge/local/agent-requests.json` changes shape; it is untracked,
  written only by `apiforge mcp`, never part of the project format and not
  the person's data, so no `format` or `migration` flag (a version-1 file is
  still read, see criterion 4).
- State: Analysis approved — the owner's decision of 2026-10-28 on #121
  (MCP-05 acceptance quiz on #64) fixed the goal and the estimate and amends
  MCP-05 criterion 6; this file is the analysis it asked for. Implementation
  starts after MCP-05 (#117) merges: it replaces the in-memory record of
  `crates/apiforge-mcp/src/agent.rs::publish`.
- Analysis model: Fable · Implementation model: —
- After MCP-05 (#117); builds on MCP-06's `requests.rs`

## Goal

A `publish` retried with its `request_id` after the server was restarted
returns the recorded result — or `unknown_outcome` when the earlier server
did not get to record one — instead of creating a second revision. Live runs
and publications share one record and one set of rules.

## What the owner will see

- Changed, for the agent only: a restarted server answers a repeated publish
  id from the record; the `publish` tool description and the guide say so.
  The record becomes version 2: every entry has a `kind` (`liveRun` or
  `publish`); a publish entry holds the path, the expected hash, when it
  started and, once known, the result.
- Removed or hidden, migrated, default behaviour: nothing the person sees.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. Replay across a restart: `publish(path, hash, request_id)` on server A;
   the same call on a new server B for the same folder returns the recorded
   result with `replayed: true`, the same revision, hash and generation, and
   the project's shared files are byte-identical before and after; the same
   id with another path or hash is `invalid_inputs`, nothing written. Test
   `mcp05b_criterion_1_a_new_server_replays_a_recorded_publication` in
   `crates/apiforge-mcp/tests/acceptance.rs`; the last part of
   `mcp05_criterion_6_…` (a fresh server publishes again) is replaced by it.
2. Ordering against the transaction: the entry (`kind: publish`, id, path,
   expected hash, `startedAt`, no result) is written atomically under the
   writer lock before `session.publish`, as a live run's is before its first
   request; the result is written into the entry right after the commit. A
   server that finds an entry without a result answers `unknown_outcome`
   with the entry's path, hash and `startedAt` and the advice that
   `project_overview` lists every revision with its hash; it writes nothing.
   Test `mcp05b_criterion_2_a_publication_an_earlier_server_started_is_unknown_outcome`
   writes such an entry as server A would have left it, then calls the new
   server. A failure to write the entry is `project_unreadable`, nothing
   published (same test file).
3. A refusal before the commit (`stale_source`, `invalid_text`,
   `comments_would_be_lost`, `not_a_component`, `too_large`, held by another)
   leaves no entry for the id, so the same id publishes once the cause is
   fixed. Test `mcp05b_criterion_3_a_refused_publication_leaves_no_entry`.
4. One set of rules: 256-byte ids (`too_large`), no known secret value in an
   id (`secret_in_text`), 1 000 entries kept across both kinds, oldest
   dropped; an unreadable record refuses `publish` with a `request_id`
   (`project_unreadable`, the message naming the file) and publishes nothing,
   while `publish` without an id, simulations and reads still work; a
   version-1 record is read as live-run entries and rewritten as version 2 on
   the next write. Unit tests in `requests.rs`:
   `the_record_keeps_the_newest_ids` across kinds,
   `a_version_1_record_is_read_as_live_runs`; acceptance
   `mcp05b_criterion_4_an_unreadable_record_refuses_a_publish_with_an_id`;
   MCP-05's `mcp05_a_request_id_is_bounded_and_never_a_secret` stays.
5. No secret value reaches the record: it holds ids, paths, hashes, times,
   revision numbers, handles, revision paths, generations and changed paths.
   `mcp06_criterion_10_…` (greps every result, stderr line and file for the
   token) gains a `publish` with a `request_id`.
6. The in-memory `Answered` of MCP-05 (64 results, 1 024 dropped ids) and
   its unit tests go; `replayed` keeps its meaning; MCP-06's live-run path
   keeps its in-memory results and durable *started* entries unchanged, and
   `mcp06_criterion_9_…` passes as before.
7. `docs/development.md` (the `publish` bullet and *The record of request
   ids*), the `publish` tool description and `docs/tickets/MCP-05.md`
   criterion 6 say a restarted server answers from the record.
8. The verification commands of the development guide pass.

## Not in scope

Durable live-run results (a `RunReport` can be 2 MiB; MCP-06 criterion 9
stands); replay without the agent session (reading the record needs the
project store, so a replay follows MCP-04's holder rules as a live run's
does); a retention policy by time.

## Regression potential and what covers it

MCP-06's live-run ids read the same file: criterion 4's unit tests and the
unchanged `mcp06_criterion_9_…` tests cover the shared reader and writer.
Nothing in the window reads the record.

## Technical appendix

- `requests.rs`: `VERSION = 2`; `#[serde(tag = "kind", rename_all =
  "camelCase")] enum Entry { LiveRun(Started), Publish(PublishEntry) }`;
  `PublishEntry { id, path, expected_source_sha256, started_at, result:
  Option<PublishResult> }` (`PublishResult` gains `Deserialize`). Reading a
  `version: 1` file deserialises `requests` as `Started`. New functions
  `find_publish`, `record_publish_started`, `record_publish_result`,
  `forget`; `find` becomes `find_live_run`.
- `agent.rs::publish`: after `self.store(&mut state)?` — `find_publish`:
  result → compare `(path, hash)` → replay or `invalid_inputs`; no result →
  `unknown_outcome`; absent → proceed. `record_publish_started` immediately
  before `session.publish`; an `Err` before activation → `forget`; a failure
  after activation (`publish_failed`, recovery required) keeps the entry;
  after the result is built → `record_publish_result`. If that last write
  fails the result is still returned (the revision exists) and the entry
  stays without a result, so a retry is `unknown_outcome`, never a second
  revision.
- Files: `crates/apiforge-mcp/src/{requests.rs,agent.rs,server.rs}`,
  `crates/apiforge-mcp/tests/acceptance.rs`, `docs/development.md`,
  `docs/tickets/MCP-05.md`.
