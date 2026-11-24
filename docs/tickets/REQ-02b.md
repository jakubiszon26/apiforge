# REQ-02b: The body editor — line numbers, Prettify, validity, placeholders

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: **Decided** ([REQ-00](REQ-00.md)). Mock-up approved
  ([Body](REQ-00.md#mock-up)). After REQ-02a.
  Issue [#459](https://github.com/jakubiszon26/apiforge/issues/459);
  with REQ-02a closes DIAG-04 (#365).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A request's body is edited in an editor that is large enough and says
whether the body is valid.

## What the owner will see

- New, on the Body tab: the editor fills the pane; line numbers;
  *Prettify*; *valid*, or the line that is not; `{{…}}` drawn as chips.

## Acceptance criteria

Tests in `folder/acceptance_tests/req02b.rs`.

1. A body that is not valid JSON is marked at its line and is not
   written to the document until it is valid; the pane says the document
   still holds the earlier body. `req02b_criterion_1_*`
2. *Prettify* changes only whitespace; a placeholder keeps its place and
   its type (quoted or bare). `req02b_criterion_2_*`
3. A body that is a calculated value says so and offers the existing
   source editor. `req02b_criterion_3_*`
4. Undo and redo act inside the editor and do not undo board edits.
   `req02b_criterion_4_*`
5. A `{{…}}` is drawn as a chip in the Body, the URL, the query and the
   header values; it stays text: the caret moves through it and a
   character can be deleted. Typing `{{` opens the completion the URL
   field has today (DIAG-18). `req02b_criterion_5_*`
6. At 960 × 540 the editor shows at least five lines beside the
   response. `req02b_criterion_6_*`

## Captures the owner judges on the pull request

A chip in each of the four places, with the caret inside one; an
invalid body with its line; the calculated body.

## Not in scope

How the formatter breaks a body in the `.afs` (EDIT-03 #369); where the
completion's fields come from (REQ-11 #538).

## Technical appendix

- Files: new `steps/body.rs`; `steps/request.rs` (the Body tab's slot);
  `inspector/fields.rs` only to share `request_body_editor`'s validity
  rule; the test file.
- Regression: the Inspector's body box keeps its behaviour (its tests
  unchanged); criterion 1 guards against writing a broken body.
