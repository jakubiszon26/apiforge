# LANG-05: A check that is one Boolean expression is written as the author wrote it

- Size: S · Owner review: 10 min · Risk: `format` (the canonical form of a
  check)
- State: **Analysis; waits for one owner decision** ([below](#owner-decision-needed)).
  After it: implementation follows LANG-13 (#386) and LANG-06 (#143), the
  same writer.
- Issue [#142](https://github.com/jakubiszon26/apiforge/issues/142) ·
  branch `fix/lang-05-condition-as-written`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A tester writes `if a == 1 && b != 2 {` and keeps reading exactly that —
in the editor after a board edit, after *Format*, and in the reference.

## What happens today

Measured with the binary of `main` (2026-11-20):

```afs
ok = assert a.status == 200 && a.status != 500
both = if a.status == 200 && a.status != 500 {
```

are read as written, and written back — by the window, by `apiforge
print`, at publication **and by `apiforge fmt`** — as

```afs
ok = assert (a.status == 200 && a.status != 500) == true
both = if (a.status == 200 && a.status != 500) == true {
```

The issue named the window only. It is the canonical form of the
language, not a defect of one writer: the reference states it
(*Assertions and predicates*: "A check that is a plain expression means
`expr == true`, and the formatter writes it in full: `assert a >= b`
becomes `assert (a >= b) == true`, `assert flag` becomes `assert flag ==
true`"), ADR 0005 decides it, and every fixture is written so. The model
stores one thing for both spellings: the predicate *equals*, the
expression, and `true`.

## Owner decision needed

Changing it changes what *Format* writes into existing files, so it is the
owner's (the file format).

- **A (recommended) — the short form is canonical for an expression that
  is already a comparison or a logical operation.** `assert a >= b`,
  `if x == 1 && y != 2 {`, `assert !done` are written as they are; a bare
  value keeps the long form (`assert flag == true`), because `assert flag`
  alone does not say what is expected. The parser reads both forms, as
  today. A file changes only where it is formatted or where the board
  rewrites that statement; `(a >= b) == true` written by 0.1.0 becomes
  `a >= b` then. The reference and ADR 0005 are amended; the fixtures are
  formatted in the ticket.
- **B — every plain expression is written short**, also `assert flag`.
  Shortest; `flag == true` written on purpose is shortened too.
- **C — nothing changes in the writer**; the reference gets a sentence
  that the short form may be written and is expanded. The issue closes as
  "by design".

What it means for the person at the window: with A or B the text they
typed stays as typed; with C it keeps growing `== true` after the first
board edit of that statement or the first *Format*.

## Acceptance criteria (for A; restated if B or C is chosen)

Tests in `crates/apiforge-script/tests/lang05.rs`.

1. `fmt` and `print` write a check whose expression is a comparison or a
   logical operation without `== true`, in `assert`, `if` and each check
   of a `condition`; a bare name, field or call keeps `== true`.
   Test: `lang05_criterion_1_*`, one per statement kind.
2. Text → document → text is unchanged for both spellings' canonical
   outcome, and the two spellings lower to equal documents.
   Test: `lang05_criterion_2_round_trip`.
3. `(a >= b) == false`, and a comparison with anything but `true`, are
   written as today. Test: `lang05_criterion_3_only_true_is_dropped`.
4. Every `.afs` under `fixtures/` and every example of the reference is in
   the new canonical form; the simulated run of each fixture is unchanged
   (statuses and order). Test: `reference.rs`; the fixture run tests.
5. The reference's *Assertions and predicates* and *Branches* say that any
   Boolean expression is a check, and ADR 0005 is amended.
   QA step: read in the pull request.

## Regression potential

The canonical form of every fixture changes where a check is compound:
`reference.rs` (every example canonical) and the round-trip tests of
`apiforge-script/tests` cover it; no run changes, because the model does
not (criterion 4).

## Not in scope

`>=` and `<=` as predicates of their own (they stay expressions); a value
from whichever branch ran (LANG-09, #353).

## Technical appendix

`crates/apiforge-script/src/print.rs` (`check`), `format.rs` (the same
rule for the tree it prints), `parser.rs` unchanged;
`docs/language/reference.md` (two paragraphs), `docs/decisions/0005-*.md`;
`fixtures/**.afs`.
