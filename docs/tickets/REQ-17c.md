# REQ-17c: A try is kept with the person across a restart

- Size: S–M · Owner review: 10 min · Risk: `permission` (what of a
  secret is written to disk); no `format` of the project
- State: **Decided** ([REQ-17](REQ-17.md): 2 A) **except one
  question**. After REQ-17b. Issue [#608](https://github.com/jakubiszon26/apiforge/issues/608).
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

## The open question

A try can hold a secret typed as a value (an `Authorization` header,
a password in the body). A scenario file stores such a value as typed
today and the Auth tab warns. For the store:
**A — a value under a sensitive name (`redact.rs`,
`is_sensitive_key`) is not written: after a restart the field is
empty and says "not kept"; `{{env.NAME}}` is kept** · B — written as
typed, as a scenario file does, in the owner-only file · C — a try
holding such a value is not kept at all.

## Acceptance criteria

Tests in `folder/acceptance_tests/req17c.rs`; unit tests of the store.

1. A try is written within a second of its last edit and on close; a
   restart shows the same tries, names and order. `req17c_criterion_1_*`
2. Nothing is written under the project folder. `req17c_criterion_2_*`
3. The file holds no response and no value the run redacts.
   `req17c_criterion_3_*`
4. By the owner's answer: what is stored of a value under a sensitive
   name. `req17c_criterion_4_*`
5. An unreadable or newer file: the window starts with no tries, says
   so once and does not rewrite it. `req17c_criterion_5_*`
6. Two windows on one project do not lose each other's tries: a write
   reads the file again and replaces its own entries.
   `req17c_criterion_6_*`
7. REQ-17b's line "until the window closes" is gone. `req17c_criterion_7_*`

## Technical appendix

- Files: new `tries_store.rs` beside `run_mode.rs` (its model);
  `folder/tries.rs`; the test file. Not offered to an agent: the
  store is never read by `apiforge-mcp` or `apiforge-cli`.
