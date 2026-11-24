# REQ-17a: Tries are held beside the project session and edited by its own code

- Size: M · Owner review: 10 min · Risk: none visible; the session's
  edit code is factored
- State: **Decided as a need** ([REQ-17](REQ-17.md): 1 A, 2 A).
  **Redesigned on 2026-11-24 after a second model's read**: the first
  design, a document inside the session's list, is dropped (below).
  The new design was read again the same day and its findings are in
  the criteria. Approved by the owner with the files of #605 (#603).
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
   never a field of `ProjectSession`, made with the caller's `Placer`
   as a session is: `create(name, text)`,
   `get(id)`, `list()`, `rename`, `discard`. It has no store
   and no path on disk: no function of it takes or returns a
   `ProjectStore`. `req17a_criterion_1_*`
2. `edit_source`, `apply_model_edit`, `undo` and `redo` take
   what `ProjectSession::try_context()` gives (the context project
   and whether the project is in an older language: the one new,
   read-only function of the session) and give, for the same text,
   model, placer and edit, **the same text, model and diagnostics as
   the session gives for a scenario**. What `apply_edit` does to one
   document — the kind check, `normalise`, `kept_chain`, the "nothing
   changed" return, `install_model`, the history step — and
   `install_step` are functions both call, not copies. A table-driven
   test runs the same edits through both with the same placer.
   `req17a_criterion_2_*`
3. An edit that can only be written by printing the whole text, on a
   text with comments that belong to no statement, is refused with
   `CommentsWouldBeLost` exactly as the session refuses it, and
   `accept_comment_loss(id)` lifts it for that version of the text:
   the session's rule and state (`comments.rs`), no policy of a try's
   own. `req17a_criterion_3_*`
4. A try is analysed on every change, at once, with the context given;
   `refresh(&Project)` analyses all of them again when the context
   changed (a publication, a reload, a take-back).
   `req17a_criterion_4_*`
5. `run_project(id, context) -> Result<Project>` gives the project a
   runner needs: the context with the try as its one scenario; an
   error with the first diagnostic when the try does not check, and
   the session's language problem when the project is in an older
   language, as `prepare_run` refuses a run.
   `req17a_criterion_5_*`
6. `into_source(id)` gives the text to register with
   `create_document` (REQ-17d); nothing else leaves the set.
   `req17a_criterion_6_*`
7. `ProjectSession` has no new field or event and one new function,
   `try_context()`; with
   a `TrySet` alive beside it, save, *Save all*, a background save
   finishing, `reload_project`, `release_writer` and
   `take_writer` leave the set equal and the folder without a byte
   of it. One test each. `req17a_criterion_7_*`
8. The session's suites pass with no test edited; `apiforge-cli` and
   `apiforge-mcp` do not name `TrySet` (a test reads their
   sources). `req17a_criterion_8_*`

## Not in scope

Any interface (REQ-17b); the file of REQ-17c; handle collisions with
the project's components (a try declares no component).

## Technical appendix

- Files: new `apiforge-project/src/tries.rs`; `session.rs` (the
  shared function taken out of `install_model`, `install_step`
  and `step_history`: context, `Target`, placer, text, previous
  model, new model → model and text), `document.rs` (`History`
  reused); `lib.rs`; the test file.
- Checked in the code: `install_model` (`session.rs:1020`) needs the
  context, the target, the placer, the document's text and model, and
  the comment decision (`require_comment_decision`, `comments.rs:37`,
  which reads the document's own `comments_accepted` and
  `source_version`); `apply_edit` runs `normalise` and `kept_chain`
  before it. A try's `Target` is a scenario with an id of its own and
  `older` from `try_context()` (`session.rs:1390`).
- Regression: the factoring — criterion 2's table and the session's
  own suites (criterion 8); nothing else of the session changes.
