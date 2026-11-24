# REQ-16: Send alone can send a secret the last run received

- Size: M · Owner review: 10 min · Risk: `permission` (a secret kept
  longer than a run), `default` (a refusal of REQ-05 becomes a send)
- State: **Decided** by the owner on 2026-11-24 ([#601](https://github.com/jakubiszon26/apiforge/issues/601)): A —
  the window keeps the last run's real values in memory for the
  session, used only for a single send; never shown, written or given
  to an agent. After REQ-05.
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A request behind a login is sent alone without logging in again each
time.

## What the owner will see

- Changed: where REQ-05 refuses a live *Send alone* because the last
  run holds a value only as `[REDACTED]`, the request is sent. The
  pane's line reads "values from the run of *hh:mm*, its secrets
  included".
- Mock-up: not applicable; one line of text in REQ-05's pane, judged on
  the pull request's capture.

## Acceptance criteria

Tests in `crates/apiforge-runner/tests/kept_values.rs` and
`folder/acceptance_tests/req16.rs`.

1. The engine redacts what it publishes, as today (`engine.rs`,
   `redact_run`). At the end of a run it also gives the handle the
   unredacted outputs of its steps as `KeptValues`: not
   `Serialize`, not `Clone`, its `Debug` prints step ids only,
   its memory zeroed on drop. It carries the secrets the run's
   `Redactor` learned. `req16_criterion_1_*`
2. The window keeps one `KeptValues` per document, of the run that
   is the document's report, and drops it when that report is replaced
   or goes stale, when the run-mode switch is changed or a run in the
   other mode starts (the session's `run_is_stale` does not know the
   mode: the window drops them itself), when the document or the
   project closes, and
   **when the window starts observing an agent or hands the project
   over**. `req16_criterion_2_*`
3. `Partial::Alone` takes them beside `previous` and reads from
   them only an input `previous` holds as `[REDACTED]`.
   `req16_criterion_3_*`
4. **The single send's `Redactor` is seeded, before anything is
   published, with every secret the first run learned and every value
   read from the kept values.** The token came from a response of the
   first run and is in no environment: a test's API echoes it in the
   response body, in a redirect's address and in an error text, and
   sends it under a name that is not sensitive; each is `[REDACTED]`
   in the single send's result. `req16_criterion_4_*`
5. A live *Send alone* of a request that reads a login's token sends
   the real token; REQ-05's criterion 4 holds only when no kept values
   exist (a new window, a run made by an agent or the CLI).
   `req16_criterion_5_*`
6. Everything the single send shows or keeps is redacted as a run's:
   *Request sent*, the response, the checks, Run results and
   *Export*. A test searches each for the token.
   `req16_criterion_6_*`
7. The token is in no file after the send: the project folder, the
   journal, the configuration and data folders, the log.
   `req16_criterion_7_*`
8. No agent can reach them: the handle gives them once, by a call only
   `apiforge-desktop` makes; a test reads the sources of
   `apiforge-mcp`, `apiforge-channel` and `apiforge-cli` and fails
   on the type's name or that call (the three crates see
   `apiforge-runner` whole, so the compiler cannot say it); an
   agent's `run` with a previous run behaves as before. Existing tests
   and `req16_criterion_8_*`.
9. A simulation never uses them. `req16_criterion_9_*`

## Not in scope

Keeping them across a restart; showing a secret; a try (REQ-17b reads
no step); *Copy as cURL* (REQ-07 states its own rule for a secret).

## Technical appendix

- Files: `apiforge-runner/src/engine.rs`, `service.rs` (the
  handle), `partial.rs` (REQ-04a's), new `kept.rs`;
  `folder/runs.rs`, `steps/single.rs`; the two test files. ADR
  0016 (partial runs) gains a section.
- Changes REQ-05's criterion 4 as said in criterion 5 here; REQ-05 is
  built first and unchanged by this file.
- Regression: redaction — criteria 5 and 6 and `redact.rs`'s tests;
  the corpus unchanged.
