# REQ-01a: Steps — the scenario read as a numbered list of steps with their checks

- Size: M · Owner review: 30 min · Risk: `visible UI`
- State: **Decided** ([REQ-00](REQ-00.md): 1, 5, 8 = A; rules 1 and 5.2
  ruled). Mock-up approved. After REP-07a (#588).
  Issue [#457](https://github.com/jakubiszon26/apiforge/issues/457).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A scenario is read top to bottom: each request with what it reads, what
it sends that is special, what is checked, and after a run what passed.

## What the owner will see

- New: *Steps | Canvas* on the right of the header above the board;
  *Steps* shows the list of [the boards](REQ-00.md#mock-up).
- Removed, hidden, default: nothing; the first open is *Canvas*.

## The rules of SENDS

Other tickets name these rules and do not restate them.

1. **Peers.** Requests of the same section (Main, or Cleanup) with the
   same method and the same URL once every `{{…}}` is blanked. A request
   whose URL has a literal segment exactly where the group has a `{{…}}`
   joins that group, the first such group in written order when two fit.
   Texts are compared as written (`template_text::print_template`), never
   as resolved.
2. **Common.** A place (a body field, a query parameter, a header, the
   path) has a common value when **more than half** of the group send
   the same text. A tie has none: every card then lists its own. A place
   more than half leave out is commonly absent: the cards that send it
   list it.
3. **A card lists** what it sends differently, and `no header X` for a
   common place it leaves out; *nothing differs* when the list is empty.
   Order: the path, body fields, query, headers.
4. **Fewer than three peers:** caption `SENDS`, the request's own places
   in the order of rule 3, without `Content-Type`; `no body` when there
   is none.
5. **Cuts.** At most four lines: with more, three and `+ N more`, whose
   hover lists the rest. A line is cut by the column's width, not by a
   count of characters; the place's name is never cut before its value;
   the hover is the full text.
6. SETS replaces SENDS for a step a later step reads (decision 7).

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/req01a.rs`
and unit tests of `steps/model.rs` and `steps/sends.rs`.

1. *Steps | Canvas* stands on the right of the board's header; the first
   open is *Canvas*; the choice is kept per project on this computer.
   With a title of 60 characters (WS-05a) at 960 × 540 the title and the
   switch do not overlap. `req01a_criterion_1_*`
2. On `labels` the rows and their numbers equal `sequence(graph)`, the
   written order and the board's badges. `req01a_criterion_2_*`
3. Every step is a row: a value and a call one line, an `if` a line with
   its branches indented under *then* and *else*. A check stands once,
   where it is written; under a step it does not read it keeps the read
   step's name. On `double-booking`, `not_booked` is under *else* and
   reads `first.status`, not `status`. `req01a_criterion_3_*`
4. SENDS follows the rules above. On `labels`: *No token* reads `no
   header Authorization`, *Weight 31 kg* `weight: 31`, *Unknown parcel*
   its path; a tie and a group of two are unit tests. The caption is one
   line at both sizes. `req01a_criterion_4_*`, `sends::tests`
5. SETS replaces SENDS, one line per field and reader.
   `req01a_criterion_5_*`
6. Cleanup is a section after Main headed *CLEANUP · runs after the
   test, also when a check fails*, numbered as the board; a Cleanup row
   that reads a Main step names it. `req01a_criterion_6_*`
7. Before a run no row carries a result; after one each row and check
   does, marked by REP-07a's function, a failed check with its actual
   value; a step in a branch not taken reads *Not run* with
   `not_run.rs`'s reason. `req01a_criterion_7_*`
8. Selecting a row selects the step in Canvas, Run results and Code, and
   back; the selected row is scrolled into view. `req01a_criterion_8_*`
9. At 960 × 540 and 1600 × 1000 nothing overlaps and no text is cut
   while its column has room; every cut text has its hover.
   `req01a_criterion_9_*` (`assert_disjoint`, `assert_within_window`)
10. `canvas/mod.rs` and `app.rs` are unchanged by this ticket; the model
    and SENDS have no egui type in their signatures. Review of the diff.

## Captures the owner judges on the pull request

Not on a board: the hover of the caption, which shows the common request
in full; a group of two (rule 4); a tie; `+ N more` and its hover; a
check under *else* with the read step's name; the header with a long
title.

## Not in scope

Adding and moving (REQ-01b, REQ-14); the panes and the rail (REQ-02a);
P2 and P3.

## Technical appendix

- Files: new `steps/mod.rs`, `steps/model.rs`, `steps/sends.rs`,
  `steps/list.rs`, `folder/steps_view.rs`; `folder/diagram.rs` (the
  switch, the call); `preferences.rs` (the choice); `testing.rs` (a
  review view); the test file.
- Reuse: `graph::sequence`, `graph::relations`, `canvas::check_text`
  (with `own = None` for criterion 3), `chain` for the branch a
  statement is written in — not `own_step`, which answers what a check
  reads and is why the mock misplaced `not_booked`; a scope over both
  graphs for Cleanup (`check_subjects_in`).
- Regression: the board is untouched (`header_tests.rs`, `ws05a.rs` when
  merged, stay green); selection sync is covered by criterion 8 in both
  directions.
