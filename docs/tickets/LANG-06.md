# LANG-06 with KTLO-37: Format keeps free comments; one attribute order

- Size: M · Owner review: 15 min · Risk: `format` (what *Format* writes)
- State: **Approved rules, ready for implementation after LANG-13 (#386)
  has merged** (the same writer). Decided in
  [LANG-00, section 3](LANG-00.md#3-edit-fix-format-three-operations-on-a-files-text)
  and [Confirmed at approval](LANG-00.md#confirmed-at-approval), 4: Format
  keeps a free comment where it stands; nothing formats as a side effect;
  one canonical attribute order for `print` and `fmt`, which the parser
  does not require and an ordinary board edit does not impose.
- Issues [#143](https://github.com/jakubiszon26/apiforge/issues/143) and
  [#211](https://github.com/jakubiszon26/apiforge/issues/211), one pull
  request · branch `feat/lang-06-format-keeps-comments`
- Amended on 2026-11-20 after an independent review: every place a
  comment can stand is named (criterion 1), and criterion 5 asks for the
  same order, not the same text. It shares `apiforge-script/src/edit.rs`
  with DIAG-06 (#391) and merges before it. Size raised to M.
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

*Format* never loses a comment, and a file formatted by `apiforge fmt` is
not changed again when the window writes it.

## What happens today

Measured with the binary of `main` (2026-11-20) on a file with a comment
at the top of the scenario, one between two statements with blank lines
around it, and one at the end: `apiforge fmt` deletes all three and warns
*This comment is not attached to a statement and will be dropped by
formatting*. A comment directly above a statement, or beside it, is its
note and is kept (the one beside is moved above). `fmt` keeps the
attributes in the order written; `print` and the window build `@label`
first (`print.rs`, `statement`).

## What the owner will see

- Changed: *Format* in the editor and `apiforge fmt` keep every comment:
  one that belongs to no statement stays between the same two statements,
  at the top or at the end of its braces, with one blank line under it
  when a statement follows, so that it does not become that statement's
  note.
- Changed: the warning above is no longer given: what it warns of no
  longer happens. The question asked before the window prints a whole file
  from the board, or publishes (#64), stays: only that print has no place
  for a free comment.
- Changed: `fmt` writes a statement's attributes in one order — `@label`,
  `@note`, then the rest as `print` builds them (`print.rs`, `statement`)
  — and the reference shows it. A board edit keeps the order the author
  wrote.
- A free comment does not reach a published revision: a revision is
  printed from the model (LANG-13, consequence 2). Unchanged.
- Removed or hidden, migrated, default: nothing else. A file changes only
  when its author formats it.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

Tests in `crates/apiforge-script/tests/lang06.rs` unless said.

1. **No comment is lost, wherever it stands**: the comments of the
   formatted text are those of the source, in the same order.
   - On a line of its own between statements, at the top or the end of a
     block, in a branch, in the cleanup, between the lines of the head
     (`description`, `param`, `input`), between the lines of a request's
     block, before the file's first line and after its last brace: kept at
     that place, on its own line.
   - Inside an expression or an object that Format writes on one line (a
     body, headers, a condition), or beside one of its fields: moved to its
     own line directly above the line that holds it, in the order met.
   Measured on `main`: all eight such comments of a test file are deleted
   today. Tests: `lang06_criterion_1_*`, one per place, and
   `lang06_criterion_1_every_comment_survives` counting comment tokens
   before and after over generated sources.
2. Formatting changes no note: the formatted text parses to the same note
   on every statement, and to the same free comments in the same order.
   Test: `lang06_criterion_2_notes_stay_notes` over every `.afs` under
   `fixtures/` and the file of criterion 1.
3. Formatting twice gives the text of formatting once.
   Test: `lang06_criterion_3_idempotent`, same files; the property test of
   `tests/properties.rs` extended to sources with free comments.
4. `fmt` no longer reports the warning; `check` reports no warning for a
   free comment. Test: `lang06_criterion_4_no_warning`;
   `crates/apiforge-cli/tests/fmt.rs` updated to the decided behaviour.
5. For a statement with several attributes written in any order, `fmt`
   writes them in the canonical order, the same order `print` uses for the
   attributes it writes; the same for the attributes of an `input` line.
   Format does not change a spelling: an `@note("…")` the author wrote
   stays an attribute, although `print` writes an ordinary note as a
   comment. What `print` writes is already canonical: `fmt(print(d)) ==
   print(d)`. Tests: `ktlo37_one_attribute_order` over every permutation,
   `ktlo37_print_is_canonical` over every working `.afs` under
   `fixtures/`.
6. A board edit that rewrites a statement whose author wrote `@note` above
   `@label` keeps that order on that statement and changes no other line.
   Test: `ktlo37_an_edit_keeps_the_authors_order` beside the LAY-34 tests
   of `crates/apiforge-script/src/edit.rs`.
7. Publishing a component whose source holds a free comment asks first, as
   today, and the revision holds no free comment.
   Test: the publication tests of #64 pass unchanged.
8. The reference's *Comments and labels* says what Format keeps and names
   the attribute order; its examples are in that order.
   Test: `crates/apiforge-project/tests/reference.rs`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| A comment that was a note becomes free, or the reverse, and a card's note changes | criterion 2 |
| The canonical form of every fixture (the reference test requires every example to be canonical) | criteria 3 and 5; `reference.rs` |
| LAY-34: a board edit touches only its statement | the `edit.rs` suite unchanged; criterion 6 |
| The comment-loss question of the window | `folder` tests of #64 unchanged |

## Not in scope

- Blank lines kept elsewhere, long bodies, a comment beside a statement
  staying beside it: EDIT-03 (#369).
- Comments in a published revision.

## Technical appendix

`crates/apiforge-script/src/parser.rs` (`attach_comments`: keep the free
ones in the tree with the statement they stand before, or the block's
end), `ast.rs` (their place in `File` and `Statement`; the warning's
constant goes), `format.rs` (`statements`), `print.rs` (one function gives
the attribute order to both), `edit.rs` (criterion 6; DIAG-06 moves
statements in the same file afterwards);
`crates/apiforge-project/src/comments.rs` (the question stays, the warning
goes); `docs/language/reference.md` (*Comments and labels* only);
`crates/apiforge-cli/tests/fmt.rs`. Starts from `main` with LANG-13 in it:
`print.rs`, `format.rs` and the reference are rewritten there.
