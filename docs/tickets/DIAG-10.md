# DIAG-10: Output data reads as a tree: a field on one line

- Size: S · Owner review: 20 min · Risk: `visible UI` (and `removal`
  if question 2 or 3 takes something off the rows)
- State: **Analysis. Decision needed: questions 1–4** (issue #531) ·
  for 0.2.1 · raised by the owner on the v0.2.0 packages, 2026-11-21
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

"This whole view will also go to the new design." A response in the
Inspector's *Output data* is read at a glance — field, value, type,
nesting — and a field is checked or assigned from its own row.

## What is there today

Looked at in the running application (`review_output_data`, the request
`order`: seven fields, an object, a list of two objects, a long text, a
boolean, a null).

- Every field takes two lines: its name, then its value centred under
  it with ✓ and {} at the right. 5 of the 14 rows fit at 1600 × 1000.
- Nesting is an 8 px step that stops at the fourth level, and a nested
  row repeats its whole path (`items[0].name`).
- The type is in a hover only. A long value is cut at 60 characters.
- An output is a heading (*Response*) and then a row named `$`.
- The same value is drawn by a second widget in Run results (`body`,
  `Object (7)`, `List (2)`, *Check* and *To variable* as words), and the
  Inspector says `Object · 7`, `List · 2`: two looks and two sets of
  words for one tree.

## What the owner will see

- Changed: a field is one row — fold mark, name, value, ✓ and {} — 26 px
  high; nesting is a 14 px step at every level with a thin guide line;
  an item of a list is named `[0]`; a nested row shows its own name, the
  whole path on hover and in what is dragged.
- Changed: the value is written as the language writes it and coloured
  by its type (text green, number brown, `true`, `false` and `null`
  violet); an object or a list says `object · 7`, `array · 2`, the
  language's words (DIAG-08, decided).
- Changed: the note above is a mark and a sentence: *Sample* — *Not from
  an API call.*, or *Last run* — *Editing the step does not update it.*
- Unchanged: what ✓ and {} do (DIAG-08's menu, DIAG-09 before a run),
  dragging a name onto an argument, folding, the pages of a long list.
- Removed or hidden: only by questions 2 and 3 (the actions on rows not
  pointed at; the `$` row). Default behaviour that changes: none.

## Mock-up

Design review captures, `review_output_data` on `mock/output-data` at
`62cc288`, in `docs/qa/2026-11-21/DIAG-10/`. Boards: `rows.png`
(question 1), `actions.png` (2, with DIAG-08's menu and proposal 1),
`root.png` (3), `run-results.png` (4). Whole windows of A:
`window-960.png` (beside `window-960-today.png`), `window-1600.png`,
`window-1600-after-run.png`.

| Question | Options (recommended first) |
| --- | --- |
| 1 · the row | **A** one line, the value coloured by its type · B the same with a column of type words (the type always in view; a third of the value's width, and close to the chip not chosen in DIAG-08) · C the response as JSON text (what the API sends; closing braces take rows, and REP-08 already gives Run results a *JSON* view) |
| 2 · the actions | **On every row**, as today · only on the row pointed at (a calmer list; ✓ and {} must be found) |
| 3 · the first row | **The heading is the row**: *Response* folds and carries ✓ and {}; *HTTP status* is one line · keep the heading and a `$` row |
| 4 · Run results | **The same row there with REP-08** (#455, REQ sprint), which already rewrites that tree · now, as a second 0.2.1 ticket (S; the file REP-07 and REP-08 also edit) · the Inspector only |

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/diag10.rs`
on the fixture `order`.

1. Each field's name, value, ✓ and {} share one row at most 26 px high;
   with the sample at 1600 × 1000 all fourteen rows are in the panel
   without scrolling.
2. A child's name starts 14 px right of its parent's at every level to
   ten; the second item of `items` is named `[1]`; pointing at
   `name` under it shows `items[1].name` and `string`.
3. The value of `total` is drawn in the number colour, `id` in the text
   colour, `paid` and `coupon` in the third; `customer` reads
   `object · 2` and `items` `array · 2`.
4. `note` is cut with an ellipsis inside the panel and ✓ and {} stay
   within the window at 960 × 540 and 1600 × 1000; pointing at the value
   shows all of it.
5. Clicking ▸ on `customer` shows its two fields; clicking ▾ hides
   them; the state survives selecting another step and returning.
6. DIAG-08's and DIAG-09's tests and the tests of dragging a field pass
   unchanged; the accessible names of ✓ and {} are the same.
7. Before a run the note is *Sample*; after one, *Last run*.
8. The first row and the actions are as questions 3 and 2 decide.
9. Design review at 960 × 540 and 1600 × 1000, measured against the
   board.

## Not in scope

The ✓ menu and the type words: DIAG-08 (#521), merged before this
ticket, which keeps its menu as it is. Run results' tree, its opening
rule and *Tree | JSON*: REP-08 (#455), by question 4. A request beside
its response: REQ-02a (#458), which shows this row.

## How DIAG-08 relates

Kept apart, DIAG-08 first. DIAG-08 is decided, `ready:opus` and what
the owner asked for by name; folding it in would reopen it, make one
`M` of about 40 review minutes out of two `S`, and hold the type check
until four more questions are answered. The two touch one function in
different parts — DIAG-08 what the ✓ does, this ticket where the row's
parts sit — so the second to merge moves a call, not a design.

## Proposals beyond the request

| Proposal | Problem solved | Size, risk | Trade-offs | Where |
| --- | --- | --- | --- | --- |
| 1 · A long value opens on a click | A message or a token is cut at the panel's edge and whole only in a hover that cannot be selected | within this S | A row grows while open | Here, if taken (`actions.png`, fourth panel) |
| 2 · *Copy value* and *Copy path* on a row | The path of a nested field is retyped into code by hand | S · `visible UI` | A context menu to discover | With REP-04 (#148) |
| 3 · A field that a check already reads is marked | Nobody sees which part of a response is covered; Run results marks only checks added in this session | S–M · `visible UI` | One more mark on the row | REQ, with REP-07 (#381) |
| 4 · The Inspector opens a value by REP-08's rule | One-line rows make opening to the leaves affordable here too | S · `default` | The owner's decision; long responses scroll | With REP-08 |
| 5 · A filter by name above the tree | A field of a response of two hundred is found by scrolling | S · `visible UI` | A field above every tree | Later |
| 6 · The component dialog's path picker takes the same row | A third drawing of the same tree | S | None seen | Later |

## Technical appendix

`inspector/fields.rs` `json_tree` and `tree_row` (the row; the 60
character cut; `add_sized` is what centres the value today);
`inspector/mod.rs` `outputs` (the note, the heading); `report.rs`
`value_branch` and `describe_value` (the second widget — by question 4
REP-08 calls the row of this ticket instead of restating it: the row has
one owner, here). The paging and the 400-row limit of `json_tree` stay
and keep their unit tests. The mock branch has the variants behind
`mock::ROWS`, `ACTIONS`, `ROOT`.
