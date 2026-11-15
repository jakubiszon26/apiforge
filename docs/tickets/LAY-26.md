# LAY-26: Code and the command line warn where a far value is read directly

- Size: S–M · Owner review: 15 min · Risk: `visible UI`
- State: Decided by the owner on 2026-11-15 (issues #335, #341), the
  definition of far included; split from [LAY-25](LAY-25.md), which is
  merged (#339); checked against it on 2026-11-15
  ([what LAY-25 built](#what-lay-25-built)) · issue #341
- Analysis model: Fable · Implementation model: Opus

## Goal

The owner's addition of 2026-11-15: outside MCP, API Forge says where a
value would be better saved in a variable. A person writing a scenario as
text sees it in the Code view and in `apiforge check`, with a fix that does
what *Save as variable* does on the board (LAY-25).

## What the owner will see

- New, Code view: a warning under every direct read of a step's output by
  a far step — the warning underline, the gutter mark and a row in the
  problems list, as every warning has today — with *Apply fix*, which
  saves the value in a variable and makes every step that reads it read
  the variable.
- New, `apiforge check`: the same warnings in its output. The exit code is
  what it is today: a file with warnings only still passes.
- Not in MCP: `check` and `check_text` return no such warning (question 5
  of LAY-25, answer A).
- Removed, migrated, default behaviour: nothing. No file changes until a
  person applies a fix.

## When a read is far, in code

Code has no columns, so "far" is counted in the engine's order
(`graph::sequence`). The *step* of a request, a component call or a branch
is itself; the step of a check or a value is the last such step before it.
**A read is far when the reader's step is two or more steps after the
source's step, or the reader is in Cleanup and the source in Main.** A
step's own checks and values, the next step and what hangs on it are near.
Cleanup is as LAY-25 was built and recorded on #335: a Cleanup step that
reads a value of Main is a far reader like any other, and reads the
variable after the fix.

It is the board's rule read off the document: Tidy up gives each step a
column, so two steps later is two columns away. The board, which looks at
where the cards are, can call one more reader far — where a row wraps, or
on a board arranged by hand — never one fewer. One function in
`apiforge-core` answers it for the warning; after the fix the reader reads
a variable, which the board draws as a chip whatever the count (LAY-25,
answer 2 A).

The warning is on every far read, not from three readers on. A read of a
variable, a parameter or an environment value is never warned.

## Mock-up

No capture: the warning is drawn by what the Code view already has for a
warning (`a_warning_underlines_in_its_own_colour`, the problems row with
*Apply fix* in `folder/code_view.rs`); only its text is new. A capture
would need the pass itself. The review view of criterion 7 is the check.

## Acceptance criteria

Tests in `folder/acceptance_tests/lay26.rs`, the CLI's and the MCP's
acceptance tests, on `far-value.afs` and `shop-30.afs`.

1. `far-value.afs` in the Code view has one warning at each far read of
   `cart.cart.id`, `cart.cart.productId` and `session.body.currency`, a
   reference inside a `{{…}}` of an address included, and none at
   `quantity`, which is the step after `cart`, nor at any read of a
   variable, a parameter or an environment value. Its text is fixed and
   quotes nothing from the source. On `shop-30.afs` the two reads in
   `cleanup` (`cart.body.id`, `login.body.token`) are warned.
2. *Apply fix* on any of them saves that value as LAY-25's *Save as
   variable* does (`edit::save_values`): the same name, the `var` line
   right after the source's statement, every step of Main and Cleanup that
   read the value reading the variable; where a variable already holds the
   value no line is added and the readers read that one. It removes every
   warning of that value. On `far-value.afs` the three fixes, applied in
   any order, give the statements of `far-value-saved.afs`, and after each
   fix the text parses to the same model as the board's edit of the same
   value. The fix edits the text, it does not reprint it: in a copy of
   `far-value.afs` with a comment and irregular spacing between the source
   and its readers, every byte outside the new line and the renamed
   references is unchanged. One undo restores the text byte for byte.
3. `far-value-saved.afs` has no such warning.
4. `apiforge check` prints the warnings with their lines; its exit code on
   `far-value.afs` is the one it gives without them.
5. MCP `check` and `check_text` on `far-value.afs` return no such warning,
   and `write_source` accepts the text as before.
6. The rule is LAY-25's, through one function of `apiforge-core`: every
   read the warning names on `far-value` and `shop-30` is a read
   `edit::shared_values` counts as far, and on the tidied board it is
   either in Cleanup or not beside its source.
7. Errors come first: where the list is full a warning gives way to an
   error, as today. `review_lay26` renders the Code view with the warnings
   at both window sizes.

## Not in scope

A setting to turn the warning off; warnings in MCP; a warning on the
board; rewriting the demo scenarios (they will carry warnings where they
read far values directly — said here so that it is not a surprise).

## Estimate

**S–M · 15 min · `visible UI`.** One pass over the document and one fix;
no state. Uncertainty: the fix must give the board's edit as text.
Regression potential: tests that count the diagnostics of the demo and
reference scenarios.

## What LAY-25 built

Read on `main` at `40239e9`; nothing in it is against this ticket.

- `apiforge-core/src/edit/shared.rs`: `steps` is this ticket's rule
  already (private, named for LAY-26), with Cleanup counted on after Main;
  `shared_values` uses it but lists only values three or more steps read,
  so the warning needs the rule exposed for every far read. `propose`
  gives the name and the variable that already holds a value;
  `save_values` is the edit, on the model.
- The board's edit reprints the document; the fix is a text edit. They
  agree on a formatted file and on the model always, which is what
  criterion 2 now asks.
- `apiforge check` takes its diagnostics from
  `apiforge_mcp::folder::check_document`, the function MCP's `check` uses.
  The warnings are appended in the CLI (`apiforge-cli/src/folder.rs`),
  not in that function.
- `far-value.afs` and `far-value-saved.afs` are on `main`.

## Technical appendix

- Kept out of MCP by where it runs: the pass is its own function, not part
  of parsing or lowering, so nothing that calls those gets it. The Code
  view's analysis and the CLI's `check_document` call it and append;
  `apiforge-mcp`, whose `folder::check_document` the CLI shares, does not.
  The warning is recognised by a constant, as `UNATTACHED_COMMENT` is, and
  criterion 5 holds the door shut.
- The pass lives where `Diagnostic` and the ranges of references do
  (`apiforge-script`); the rule and the name come from
  `apiforge-core::edit::shared`.
- The fix fits the existing `Fix` (one range, one replacement): the range
  runs from the end of the source's statement to the end of the last
  reader, Cleanup included, and the replacement is that text with the
  `var` line added and the references renamed, everything between kept as
  written. So *Apply fix* and its undo need no change.
- `Diagnostic::message` is a static description: the warning names no
  value (the range shows it).
