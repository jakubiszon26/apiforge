# REQ-04a: The engine runs a scenario through a step, and one request alone with its checks

- Size: M · Owner review: 10 min · Risk: none (no interface); run semantics
- State: **Decided**: an internal design, found by reading the engine
  for [REQ-00](REQ-00.md#what-the-code-says-about-req-04-and-req-05).
  Issue [#599](https://github.com/jakubiszon26/apiforge/issues/599).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

*Run to here* and *Send alone* get the runs they need: today a run to a
target leaves out every check, and a single step runs the whole Cleanup.

## What the owner will see

Nothing: no caller uses it until REQ-04b and REQ-05.

## Acceptance criteria

Tests in `crates/apiforge-runner/tests/partial.rs` and unit tests of
`partial.rs`.

1. `RunRequest.partial: Option<Partial>`. `Partial::Through { node_id }`
   runs `sequence(graph)` up to the node and the data statements the
   engine reaches before the next step, so every check and value of the
   steps run; then Cleanup; nothing else. On `labels`, through *Weight
   0 kg*: 15 results in Main, checks included. `req04a_criterion_1_*`
2. Through a step in a branch not taken: the step is skipped with
   `OutsidePath`, as in a full run. `req04a_criterion_2_*`
3. `Partial::Alone { node_id, previous }` runs the request and the
   checks that read it (`graph::check_subjects`), with every other value
   from `previous` as `single_node_id` reads it; no other step and **no
   Cleanup**. On the project of `single-workflow-add`: no result of
   phase Cleanup, no request but one. `req04a_criterion_3_*`
4. A check of an *Alone* run whose other subject is missing from
   `previous` is not run, with a cause naming that step; the request is
   still sent. `req04a_criterion_4_*`
5. `previous` is validated as today (`validate_previous`: scenario,
   mode, finished); `partial` with `target_node_id` or `single_node_id`
   is `ConflictingModes`; an unknown node is `NodeNotFound`; the result
   is `diagnostic`. `req04a_criterion_5_*`
6. A live partial run begins and ends its journal as a full run.
   `req04a_criterion_6_*`
7. `run_corpus` passes with no fixture edited: `target_node_id` and
   `single_node_id` behave as before. Existing test.
8. The CLI, the MCP server and both windows pass `partial: None`; their
   behaviour and outputs are unchanged. Existing tests.

## Not in scope

Any caller; a result for the steps a partial run leaves out (REQ-04b
words them in the window); removing the two older fields.

## Technical appendix

- Files: new `apiforge-runner/src/partial.rs` (the two selections, pure
  functions of a graph); `request.rs`, `engine.rs` (the selection and
  the Cleanup condition); one line at each of the six places a
  `RunRequest` is built; a new ADR `docs/decisions/0016-partial-runs.md`.
- Regression potential: run semantics — criteria 7 and 8; Cleanup still
  runs after *Through* (criterion 1) and after every full run.
