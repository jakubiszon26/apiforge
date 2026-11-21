# LANG-22: A type check that cannot pass is reported when it is written

- Size: S · Owner review: 10 min · Risk: none of the five flags.
  Backward compatibility is touched and **decided by the owner**
  (2026-11-21, on #522): a type name outside the six is an **error**, so
  a file with such a line no longer passes `check`. First filed with
  `migration`; no data is converted or read by a new loader, so the flag
  was withdrawn when the decision was recorded
- State: **Approved, ready for implementation** (issue #522) · for 0.2.1
  · found by the planning model while reproducing the owner's finding of
  2026-11-21 (DIAG-08) · labelled `ready:opus`
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

Someone who writes a type check in the code and gets the type's name
wrong is told at once, in words that name the six types.

## Seen on v0.2.0 (`apiforge check`, `apiforge run`, `main` `dd4667f`)

- `assert cart.body.total is "nubmer"`: `check` reports nothing; the run
  fails with *expected nubmer, actual 160*.
- A type check that fails, `is "string"` on 160, says *expected string,
  actual 160* — the value, not its type, so the reader works the type out.
- `assert cart.body.total is number`, the natural first try: *Unknown
  block handle*.

## What the owner will see

- New: `is "nubmer"` is marked in the editor and by `check`: *Unknown
  type "nubmer". A type is "string", "number", "boolean", "object",
  "array" or "null".* It is an error (decided): the document does not
  run until the name is corrected.
- New: `is number` is marked *A type is written in quotes: is "number"*,
  with the fix on the mark.
- Changed: a failed type check reports *expected string, actual number
  (160)* in Run results, the command line and MCP.
- Mock-up: not applicable — texts in existing places.

## Acceptance criteria

Tests in `crates/apiforge-script/tests/lang22.rs` and
`crates/apiforge-runner/tests/lang22.rs`.

1. `is "nubmer"` gives the diagnostic above, as an error, at the
   literal's range, and the document does not run; the six names give
   none. `lang22_criterion_1_unknown_type_name`.
2. `is number` (and the other five bare words) gives the quote
   diagnostic with a fix that writes the quotes; a handle that really is
   called `number` is still read as a handle. `lang22_criterion_2_bare_type_word`.
3. A failed type check's result carries the actual type and the value.
   `lang22_criterion_3_failure_names_the_type`.
4. A type that is not a literal (`is param.kind`) is left to the run, as
   today. `lang22_criterion_4_computed_type`.
5. The reference's *Assertions and predicates* and *Diagnostics and what
   to do* name the two diagnostics.

## Not in scope

`is number` as accepted syntax: a change of the format, put to the owner
as a proposal on the issue and otherwise decided with LANG-19 (#472).

## Technical appendix

`apiforge-script` `lower.rs` (predicate `Type`), the diagnostics table;
`apiforge-runner` where `Predicate::Type` compares; `report.rs`
`expected_text`.
