# REQ-17a: Tries are held beside the project session and edited by its own code

- Size: M · Owner review: 10 min · Risk: none visible; the session's
  edit code is factored
- State: **Decided as a need** ([REQ-17](REQ-17.md): 1 A, 2 A).
  **Redesigned on 2026-11-24 after a second model's read**: the first
  design, a document inside the session's list, is dropped (below).
  The new design waits for one more read before a label.
  Issue [#607](https://github.com/jakubiszon26/apiforge/issues/607).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A try is edited, checked and undone by the code that edits a scenario,
and nothing that saves, reloads, hands over or shares the project can
meet it or lose it.

## Why not a document in the session's list

Read against `main`: the session is replaced whole in five places —
`complete_storage` (`storage_work.rs:248`), `take_writer` and
`release_writer` (`external.rs:619`, `671`), `reload_project`
and `follow_commit` — and `same_storage_inputs` compares its
documents one to one with a background save's. A document that is in no
file would have to be carried across each, left out of some fifteen
walkers of `self.documents` and kept in five others, and
`DocumentEntry.path` cannot be absent (it is the manifest's type).
One missed place loses a try or makes a save stale. A holder outside
the session needs none of these cases.

## What the owner will see

Nothing: no caller until REQ-17b.

## Acceptance criteria

Tests in `crates/apiforge-project/tests/tries.rs`.

1. `TrySet`, a type of `apiforge-project` owned by its caller and
   never a field of `ProjectSession`: `create(name, text)`,
   `get(id)`, `list()`, `rename`, `discard`. It has no store
   and no path on disk: no function of it takes or returns a
   `ProjectStore`. `req17a_criterion_1_*`
2. `edit_source`, `apply_model_edit`, `undo` and `redo` take
   the project's context (`&Project`) as an argument and give, for
   the same text, model and edit, **the same text and model as the
   session gives for a scenario**: the part of `install_model` and
   `install_step` that writes a model to text and reads it back is
   one function both call, not a copy. A table-driven test runs the
   same edits through both. `req17a_criterion_2_*`
3. A try is analysed on every change, at once, with the context given;
   `refresh(&Project)` analyses all of them again when the context
   changed (a publication, a reload, a take-back).
   `req17a_criterion_3_*`
4. `run_project(id, &Project) -> Project` gives the project a runner
   needs: the context with the try as its one scenario; refused with
   the first error when the try does not check.
   `req17a_criterion_4_*`
5. `into_source(id)` gives the text to register with
   `create_document` (REQ-17d); nothing else leaves the set.
   `req17a_criterion_5_*`
6. `ProjectSession` has no new public function, field or event; with
   a `TrySet` alive beside it, save, *Save all*, a background save
   finishing, `reload_project`, `release_writer` and
   `take_writer` leave the set equal and the folder without a byte
   of it. One test each. `req17a_criterion_6_*`
7. The session's suites pass with no test edited; `apiforge-cli` and
   `apiforge-mcp` do not name `TrySet` (a test reads their
   sources). `req17a_criterion_7_*`

## Not in scope

Any interface (REQ-17b); the file of REQ-17c; handle collisions with
the project's components (a try declares no component).

## Technical appendix

- Files: new `apiforge-project/src/tries.rs`; `session.rs` (the
  shared function taken out of `install_model`, `install_step`
  and `step_history`: context, `Target`, placer, text, previous
  model, new model → model and text), `document.rs` (`History`
  reused); `lib.rs`; the test file.
- Checked in the code: `install_model` needs only the context, the
  target, the placer and the document's text and model
  (`session.rs:1020`); `analysis::derive` is a function of the
  same. A try's `Target` is a scenario with an id of its own.
- Regression: the factoring — criterion 2's table and the session's
  own suites (criterion 7); nothing else of the session changes.
