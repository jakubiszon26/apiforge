# REQ-17a: The project session holds a document that is in no file and not in the manifest

- Size: M · Owner review: 10 min · Risk: none visible; regression
  across the session
- State: **Decided as a need** ([REQ-17](REQ-17.md): 1 A, 2 A); the
  design below is read from the code and **waits for a second model's
  read** before any label. Issue [#607](https://github.com/jakubiszon26/apiforge/issues/607).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A try is edited, checked and run by the session's own means, and
nothing that writes, lists or shares the project ever meets it.

## What the owner will see

Nothing: no caller until REQ-17b.

## Acceptance criteria

Tests in `crates/apiforge-project/tests/detached.rs`.

1. `ProjectSession::create_detached(kind, name, text) -> id` adds a
   document with no path and no manifest entry; the manifest, its saved
   state and the folder are byte-identical afterwards.
   `req17a_criterion_1_*`
2. `edit_source`, `apply_model_edit`, analysis, `prepare_run` and
   `validate_run` work on it as on a scenario, against the project's
   context (components, environment, base URL). `req17a_criterion_2_*`
3. It is left out of: `documents()` as the registry, the explorer and
   the MCP server read it (a separate `detached()` lists them);
   *Save*, *Save all* and `is_dirty` of the project; recovery drafts
   and the last-valid cache on disk; the diagram checkpoint; external
   changes and conflicts; publication; registration; the events an
   observer receives. One test per item. `req17a_criterion_3_*`
4. A window whose only changes are detached documents has no unsaved
   work: hand-over to an agent and closing ask nothing for them.
   `req17a_criterion_4_*`
5. `attach(id, path)` registers it as `create_document` would with
   its current text, in one transaction, keeping its id for the open
   tab; `discard(id)` forgets it. `req17a_criterion_5_*`
6. A reload of the project, a language update and a recovery keep the
   detached documents in memory and touch no file for them.
   `req17a_criterion_6_*`
7. `apiforge-cli` and `apiforge-mcp` build unchanged and their
   tests pass: neither can create or see one. Existing tests.

## Not in scope

Any interface (REQ-17b); storing it anywhere (REQ-17c).

## Technical appendix

- Files: `apiforge-project/src/session.rs`, `document.rs` (an entry
  that is `None`, or a flag on `DocumentSession`), new
  `detached.rs`, `recovery.rs`, `execution.rs`; the test file.
- Not verified by this analysis, for the second read: every place that
  walks `self.documents` (criterion 3 names the kinds, not the
  lines); whether `DocumentEntry.path` can be absent or needs a
  reserved path that no file may take.
- Regression: everything criterion 3 lists has a test of its own;
  the session's suites stay green with no test edited.
