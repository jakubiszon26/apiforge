# REQ-17c: Tries are listed in Files and kept with the person across a restart

- Size: M · Owner review: 15 min · Risk: `visible UI`, `permission`
  (what of a secret is written to disk); no `format` of the project
- State: **Decided** ([REQ-17](REQ-17.md): 2 A, 5 A; the secret: A, by
  the owner on 2026-11-24, #608). After REQ-17b. Issue [#608](https://github.com/jakubiszon26/apiforge/issues/608).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

Tomorrow the tries of yesterday are still there, and they were never in
the project.

## The store

`<data dir>/apiforge/tries/<manifest id>.json`, beside
`recent.json` (`dirs::data_dir()`), keyed like `run-mode.json`.
`{ "version": 1, "tries": [{ "id", "name", "text", "changedAt" }] }`:
the text of the request and nothing else — **no response, no run, no
header received**. Written atomically, owner-only on Unix, at most 50
tries and 1 MiB; an unknown version is neither read nor rewritten.

## Decided: a typed secret is not written

A — what a run would redact in the request is left out of the file;
after a restart the field is empty and says *not kept*;
`{{env.NAME}}` is kept. Not chosen: written as typed; such a try not
kept at all.

## Acceptance criteria

Tests in `folder/acceptance_tests/req17c.rs`; unit tests of the store.

1. *Files* lists the tries under *Requests, not saved*: one is opened,
   renamed and deleted there (a delete asks); closing a try's tab no
   longer asks, REQ-17b's question is gone. `req17c_criterion_1_*`
2. A try is written within a second of its last edit and on close; a
   restart shows the same tries, names and order. `req17c_criterion_2_*`
3. Nothing is written under the project folder. `req17c_criterion_3_*`
4. The file holds no response, no run and no header received.
   `req17c_criterion_4_*`
5. Left out of the written text, by one function of
   `apiforge-runner/src/redact.rs` that the store calls and a run's
   redaction shares: a literal value under a sensitive name in the
   headers, the body or the query (`is_sensitive_key`), a `Bearer`
   token, and a password in the address. A placeholder is kept as
   written. A test stores a try holding each and searches the file.
   `req17c_criterion_5_*`
6. After a restart such a field is empty, marked *not kept*, and the
   try says which fields; in the running window the value stays until
   the window closes. `req17c_criterion_6_*`
7. An unreadable or newer file: the window starts with no tries, says
   so once and does not rewrite it. `req17c_criterion_7_*`
8. Two windows on one project do not lose each other's tries: a write
   reads the file again and replaces its own entries.
   `req17c_criterion_8_*`
9. At 960 × 540 the section and its rows stay inside the panel.
   `assert_within_window`

## Technical appendix

- Files: new `tries_store.rs` beside `run_mode.rs` (its model);
  `folder/tries.rs`, `folder/explorer.rs` (the section);
  `apiforge-runner/src/redact.rs` (the function of criterion 4, made
  callable); the test file. Not offered to an agent: the
  store is never read by `apiforge-mcp` or `apiforge-cli`.
