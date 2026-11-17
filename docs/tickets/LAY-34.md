# LAY-34: An edit on the board changes only the statement edited

- Size: M · Owner review: 20 min · Risk: `format` (what the window writes
  into an `.afs`)
- State: decided by the owner — option A on 2026-11-16 (#396); a statement
  added from the board is written directly after the step it follows,
  inside the same braces, on 2026-11-17 (#396, #382) · issue #396
- Analysis model: Fable · Implementation model: Opus

## Goal

From the LAY checkpoint (#296): on `shop-30`, adding one request from the
board also added `on_error stop` to the file and moved `refund` into the
braces of its `if`, because a board edit printed the whole document from
the graph. An edit made on the board changes the statement that was
edited, and nothing else in the file.

Edit, fix and format are three operations (LANG-00, #411): an edit replaces
or inserts the text of the statement edited; the rest of the text is not
read back from the model. Nothing formats as a side effect, and a board
edit does not impose the canonical attribute order on statements it does
not change.

## Acceptance criteria (amended 2026-11-17)

1. For every `.afs` in the repository: open, make one board edit, undo by a
   second edit — the file is byte for byte what it was. Text → window →
   text without an edit is unchanged.
2. A scenario without `on_error` gets none from a board edit; one that has
   it keeps it.
3. ~~A statement written outside the braces of the branch that orders it
   stays outside; one written inside stays inside.~~ Falls away with
   `@after` (owner, 2026-11-17). Replaced by: a statement added from a
   step's *Next* is written directly after that step — after the checks and
   values that follow it, before the next step (DIAG-07) — inside the same
   braces. Statements the edit does not change stay where they are written
   (criterion 1).
4. The pull request lists which repository files the printer changes today,
   before and after.

## What a board edit writes

- A statement whose own text the edit changes — what the window prints for
  it in the braces it stands in, without the statements of its own braces
  — is rewritten where it stands; its comments stay unless its note
  changed. A renamed block rewrites the statements that name it.
- A deleted statement takes its lines with it; a deleted `if` takes its
  braces, and the statements of its braces the document still has are
  written again as new ones.
- A new statement is written in the braces the window prints it in,
  directly after the statement before it in the document's order. A branch
  or a cleanup the text lacks is opened for it.
- The parts before the statements (`description`, `on_error`, `param`,
  `input`, `action`) are edited one by one: a part the author left out
  stays out until its value changes.
- Text that cannot be edited this way (a statement sharing a line with
  another where something is inserted), or an edit that would not read back
  as the edited document, is printed whole, as before.

A step added from a step's *Next* also follows the branches that step
follows (`apiforge-core`, `add_node`), so it stands in the same braces.
Runs do not change: the step it follows already waits for those branches.
